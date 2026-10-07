#include "dialog.hpp"

#include <algorithm>
#include <climits>
#include <set>

namespace vm {
namespace {
std::string pathOf(const Value* value) {
    if (!value || value->kind != Value::Kind::Object || !value->o || !value->o->resourcePackage) return "";
    return value->o->resourcePackage->path(value->o->resourceIndex);
}
bool flag(const Value* value) { return value && value->truth(); }
}

void DialogSystem::notImplemented(const std::string& what) const { throw RuntimeError("dialog: not implemented: " + what); }

ObjectPtr DialogSystem::load(const Value& reference) {
    if (reference.kind != Value::Kind::Object || !reference.o || !reference.o->resourcePackage) throw RuntimeError("dialog: unresolved object reference");
    std::shared_ptr<const Package> owner = reference.o->resourcePackage;
    int32_t index = reference.o->resourceIndex;
    if (index <= 0 && package_) {      // an import that no installed package file resolves (e.g. a body class's name tag): the same path among the dialog package's exports
        index = runtime_.findExport(*package_, pathOf(&reference));
        owner = package_;
    }
    if (index <= 0) throw RuntimeError("dialog: unresolved object reference " + pathOf(&reference));
    const std::string key = pathOf(&reference) + "@" + owner->packageName;
    auto found = loaded_.find(key);
    if (found == loaded_.end()) found = loaded_.emplace(key, runtime_.instantiateExport(owner, index, 4)).first;
    return found->second;
}

// The dialog globals (GD_Globals.Dialog.DialogGlobals in the mission's package): Priorities (a lower index is more important) and the
// tracked-mission floors. Without them every priority index is -1 (the manager's answer for "no globals").
void DialogSystem::loadGlobals() {
    if (globalsLoaded_) return;
    globalsLoaded_ = true;
    const int32_t index = package_ ? runtime_.findExport(*package_, "GD_Globals.Dialog.DialogGlobals") : 0;
    if (index <= 0) return;
    ObjectPtr globals = runtime_.instantiateExport(package_, index, 4);
    globals_ = globals;
    if (const Value* list = runtime_.property(*globals, "Priorities"))
        for (const auto& priority : list->elements()) priorities_.push_back(pathOf(&priority));
    startIndex_ = indexOf(pathOf(runtime_.property(*globals, "ActiveMissionMinPriorityStart")));
    sideFloor_ = indexOf(pathOf(runtime_.property(*globals, "ActiveSideMissionMinPriority")));
    plotFloor_ = indexOf(pathOf(runtime_.property(*globals, "ActivePlotMissionMinPriority")));
}

int DialogSystem::indexOf(const std::string& priorityPath) {
    loadGlobals();
    if (priorities_.empty()) return -1;
    const auto found = std::find(priorities_.begin(), priorities_.end(), priorityPath);
    return found == priorities_.end() || priorityPath.empty() ? INT_MAX : int(found - priorities_.begin());
}

DialogSystem::Tag DialogSystem::tagOf(const Value& reference) {
    ObjectPtr object = load(reference);
    Tag tag;
    tag.path = pathOf(&reference);
    tag.priority = indexOf(pathOf(runtime_.property(*object, "Priority")));
    tag.echo = flag(runtime_.property(*object, "bIsEchoEvent"));
    tag.noOverrideSame = flag(runtime_.property(*object, "bDoesNotOverrideSamePriority"));
    tag.groupEvent = flag(runtime_.property(*object, "bGroupEvent"));
    tag.soundEffect = flag(runtime_.property(*object, "bSoundEffect"));
    tag.oncePerSession = flag(runtime_.property(*object, "bOncePerSession"));
    tag.multiplayerOnly = flag(runtime_.property(*object, "bMultiplayerOnly"));
    return tag;
}

// A tag is invalid when it is once-per-session and was played, or multiplayer-only with fewer than two players (one here).
bool DialogSystem::tagValid(const Tag& tag) const {
    if (tag.oncePerSession && played_.count(tag.path)) return false;
    return !tag.multiplayerOnly;
}

// The group state is keyed by the root group (ParentGroup chain).
std::string DialogSystem::rootGroupOf(const Value& group) {
    Value current = group;
    for (int depth = 0; depth < 8; ++depth) {
        const Value* parent = runtime_.property(*load(current), "ParentGroup");
        if (!parent || !parent->o || !parent->o->resourcePackage) break;
        current = *parent;
    }
    return pathOf(&current);
}

void DialogSystem::setTestLineLength(double seconds) { testLength_ = seconds; player_ = nullptr; }

void DialogSystem::registerTalker(const std::string& nameTagPath) {
    for (const auto& talker : talkers_) if (!talker.echo && talker.nameTag == nameTagPath) return;
    Talker talker;
    talker.nameTag = nameTagPath;
    talkers_.push_back(talker);
}

std::string DialogSystem::objectPath(const Value& value) { return pathOf(&value); }

std::string DialogSystem::identityOf(const Value& group) const {
    return group.o && group.o->resourcePackage ? pathOf(&group) + "@" + group.o->resourcePackage->packageName : "";
}

// WillowPawn.GetDialogGroups (NATIVE_DIALOG_GROUPS.md, UNVERIFIED): see the declaration. The globals steps are skipped when the globals cannot be reached.
DialogSystem::PawnDialog DialogSystem::pawnDialog(const Value& bodyClass) {
    PawnDialog result;
    if (bodyClass.kind != Value::Kind::Object || !bodyClass.o || !bodyClass.o->resourcePackage || bodyClass.o->resourceIndex <= 0) return result;
    ObjectPtr body = load(bodyClass);
    const Value* name = runtime_.property(*body, "DialogName");
    result.nameTag = pathOf(name);
    const auto append = [&](const Value* list) {
        if (list) for (const Value& group : list->elements()) result.groups.push_back(group);
    };
    append(runtime_.property(*body, "DialogGroups"));
    loadGlobals();
    if (flag(runtime_.property(*body, "bNPCDialog"))) {
        if (name && name->o && name->o->resourcePackage)
            if (const Value* expansion = runtime_.property(*load(*name), "DlcExpansion"); expansion && expansion->o && expansion->o->resourcePackage)
                append(runtime_.property(*load(*expansion), "NPCDialogGroups"));
        if (globals_) append(runtime_.property(*globals_, "NPCDialogGroups"));
    }
    if (globals_) {
        const Value* defaultTemplate = runtime_.property(*globals_, "DefaultTemplateGroup");
        result.groups.push_back(defaultTemplate ? *defaultTemplate : Value());   // one entry, even when the field is None
    }
    return result;
}

// GetMatchingEvent (no group given; NATIVE_DIALOG_GROUPS.md, UNVERIFIED): the talker's groups in order; the first with an enabled entry for the tag wins; a
// group without one appends its ParentGroup to the END of the search unless it is already in it (identity). Template groups (GearboxDialogTemplateGroup) are
// skipped, adding no parent, unless the caller allows them.
Value DialogSystem::matchingGroup(const std::vector<Value>& groups, const Tag& tag, bool allowTemplates) {
    std::vector<Value> queue = groups;
    std::set<std::string> seen;
    for (const Value& group : queue) seen.insert(identityOf(group));
    for (size_t i = 0; i < queue.size(); ++i) {
        if (!queue[i].o || !queue[i].o->resourcePackage) continue;
        ObjectPtr group = load(queue[i]);
        bool templateGroup = false;
        for (const Class* cls = group->cls; cls; cls = cls->super) if (cls->name == "GearboxDialogTemplateGroup") templateGroup = true;
        if (templateGroup && !allowTemplates) continue;
        search_.push_back(pathOf(&queue[i]));
        if (const Value* events = runtime_.property(*group, "DialogEvents"))
            for (const auto& entry : events->elements())
                if (pathOf(entry.field("Tag")) == tag.path && flag(entry.field("bEnabled"))) return queue[i];
        if (const Value* parent = runtime_.property(*group, "ParentGroup"); parent && parent->o && parent->o->resourcePackage && seen.insert(identityOf(*parent)).second)
            queue.push_back(*parent);
    }
    return Value();
}

bool DialogSystem::triggerOnComponent(const std::string& speakerNameTag, const std::vector<Value>& groups, const Value& tagRef) {
    const Tag tag = tagOf(tagRef);
    if (!tagValid(tag)) return false;
    registerTalker(speakerNameTag);
    int speaker = -1;
    for (size_t i = 0; i < talkers_.size(); ++i)
        if (!talkers_[i].echo && talkers_[i].nameTag == speakerNameTag) speaker = int(i);
    talkers_[size_t(speaker)].groups = groups;
    search_.clear();
    const Value group = matchingGroup(groups, tag, true);      // a fresh trigger (no event data reused): template groups are searched
    if (!group.o) return false;                          // no group has an event for the tag: nothing happens
    const int savedInstigator = instigator_;
    instigator_ = speaker;                               // TriggerEvent: Instigator := the component's owner
    lineStarted_ = false;
    run(Handle(), group, tag);
    instigator_ = savedInstigator;
    return lineStarted_;
}

// The talker for a name tag: the first registered pawn with exactly that tag; failing that (echo events only) the echo caller for it,
// created when absent. -1 when there is none.
int DialogSystem::resolveTalker(const std::string& nameTag, bool echo) {
    for (size_t i = 0; i < talkers_.size(); ++i)
        if (!talkers_[i].echo && talkers_[i].nameTag == nameTag) return int(i);
    if (!echo) return -1;
    for (size_t i = 0; i < talkers_.size(); ++i)
        if (talkers_[i].echo && talkers_[i].nameTag == nameTag) return int(i);
    Talker talker;
    talker.nameTag = nameTag;
    talker.echo = true;
    talkers_.push_back(talker);
    return int(talkers_.size()) - 1;
}

void DialogSystem::stopTalking(int index) {
    EventData& data = data_[size_t(index)];
    if (data.talker >= 0 && talkers_[size_t(data.talker)].liveData == index) talkers_[size_t(data.talker)].liveData = -1;
    groupIndex_.erase(data.rootGroup);
    data.live = false;
    data.playing = false;
    data.talker = -1;
}

void DialogSystem::lineEnded(int lineId) {
    for (auto& data : data_)
        if (data.live && data.playing && data.lineId == lineId) {
            data.playing = false;
            data.finishTime = now_ + data.outputDelay;
        }
}

// The dialog components' per-frame update (NATIVE_DIALOG.md "Line end"): a finished audio sets the finish time (the act's
// OutputDelay later); a live act with no playing audio and a reached finish time ends (output 0 of a talk act has no link here). A
// live act that never had audio ends on the next update.
void DialogSystem::tick(double seconds) {
    now_ += seconds;
    for (size_t i = 0; i < data_.size(); ++i) {
        EventData& data = data_[i];
        if (data.live && data.playing && data.testEnd >= 0 && now_ >= data.testEnd) lineEnded(data.lineId);
        if (data.live && !data.playing && now_ >= data.finishTime) stopTalking(int(i));
    }
}

// An act from a talk node object. Only plain talk acts are supported.
DialogSystem::Act DialogSystem::actOf(const ObjectPtr& node) {
    bool talkAct = false;
    for (const Class* cls = node->cls; cls; cls = cls->super) if (cls->name == "GearboxDialogAct_Talk") talkAct = true;
    if (!talkAct) notImplemented("dialog node class " + node->cls->path);
    Act act;
    // Output links: output 1 is the no-match output (NATIVE_DIALOG.md, Act_Talk.Activate: no talker and bEnableNoMatch); another output with a link is
    // not modelled.
    if (const Value* links = runtime_.property(*node, "OutputLinks"))
        for (size_t i = 0; i < links->elements().size(); ++i)
            if (const Value* targets = links->elements()[i].field("Links"); targets && !targets->elements().empty()) {
                if (i != 1 || !flag(runtime_.property(*node, "bEnableNoMatch"))) notImplemented("an output link on a talk act");
                act.noMatchNode = targets->elements().front();
            }
    act.noMatch = flag(runtime_.property(*node, "bEnableNoMatch"));
    if (const Value* variable = runtime_.property(*node, "TalkerVariable"); variable && variable->o) notImplemented("a talker variable");
    act.path = node->resourcePackage ? node->resourcePackage->path(node->resourceIndex) : node->name;
    if (const Value* delay = runtime_.property(*node, "OutputDelay")) act.outputDelay = delay->number();
    act.instigatorTalker = flag(runtime_.property(*node, "bInstigatorTalker"));
    if (const Value* talk = runtime_.property(*node, "TalkData"))
        for (const auto& entry : talk->elements()) act.talk.push_back({pathOf(entry.field("NameTag")), pathOf(entry.field("TalkAkEvent"))});
    return act;
}

// Act_ObjectParameterSwitch.Activate (NATIVE_DIALOG.md, UNVERIFIED): the event data's ObjectParameter is compared with each entry of Outputs, output i is
// activated for every equal entry and, with none, the last output. The ObjectParameter is None on every route here (the callers pass none), so only a None
// entry could match. Only an output without a link is modelled (the chain ends there, as on the generic mission-turned-in event, whose single entry names a
// mission); a chosen output that has a link throws "not implemented".
void DialogSystem::objectParameterSwitch(const ObjectPtr& node) {
    const Value* outputs = runtime_.property(*node, "Outputs");
    const Value* links = runtime_.property(*node, "OutputLinks");
    std::vector<size_t> chosen;
    if (outputs)
        for (size_t i = 0; i < outputs->elements().size(); ++i)
            if (!outputs->elements()[i].o) chosen.push_back(i);
    if (chosen.empty() && links && !links->elements().empty()) chosen.push_back(links->elements().size() - 1);
    for (const size_t i : chosen)
        if (links && i < links->elements().size())
            if (const Value* targets = links->elements()[i].field("Links"); targets && !targets->elements().empty())
                notImplemented("an output link on an object parameter switch");
}

// FindEvent: the last enabled DialogEvents entry for the tag; its act is the inline OutputAction or, with none, the target of the
// group's link table for (the entry's 1-based id, link 0): a TalkActs template (ids after the events) or a node by NodeID.
int DialogSystem::findAct(const Value& groupRef, const Tag& info, Act& act) {
    ObjectPtr group = load(groupRef);
    const Value* events = runtime_.property(*group, "DialogEvents");
    int found = 0;
    if (events)
        for (size_t i = 0; i < events->elements().size(); ++i) {
            const Value& entry = events->elements()[i];
            if (pathOf(entry.field("Tag")) == info.path && flag(entry.field("bEnabled"))) found = int(i) + 1;
        }
    if (!found) return -1;                              // no enabled entry for the tag
    const Value& entry = events->elements()[size_t(found) - 1];
    if (const Value* action = entry.field("OutputAction"); action && action->o) {
        ObjectPtr node = load(*action);
        for (const Class* cls = node->cls; cls; cls = cls->super)
            if (cls->name == "GearboxDialogAct_ObjectParameterSwitch") { objectParameterSwitch(node); return 0; }   // an event with nothing linked: the chain ends
        act = actOf(node);
        return 1;
    }
    const Value* links = runtime_.property(*group, "OutputLinksToStructs");
    if (!links) return 0;
    for (const auto& link : links->elements()) {
        if (!link.field("FromNodeID") || link.field("FromNodeID")->integer() != found || link.field("LinkNumber")->integer() != 0) continue;
        const int to = int(link.field("ToNodeID")->integer());
        const int eventCount = int(events->elements().size());
        const Value* templates = runtime_.property(*group, "TalkActs");
        if (templates && to > eventCount && size_t(to - eventCount) <= templates->elements().size()) {
            const Value& entry2 = templates->elements()[size_t(to - eventCount) - 1];
            if (const Value* variable = entry2.field("TalkerVariable"); variable && variable->o) notImplemented("a talker variable");
            if (const Value* output = entry2.field("OutputAction"); output && output->o) notImplemented("an output action on a template talk act");
            act = Act();
            act.path = pathOf(&groupRef) + ".TalkActs[" + std::to_string(to - eventCount - 1) + "]";
            if (const Value* delay = entry2.field("OutputDelay")) act.outputDelay = delay->number();
            act.instigatorTalker = flag(entry2.field("bInstigatorTalker"));
            if (const Value* talk = entry2.field("TalkData"))
                for (const auto& item : talk->elements()) act.talk.push_back({pathOf(item.field("NameTag")), pathOf(item.field("TalkAkEvent"))});
            return 1;
        }
        if (const Value* nodes = runtime_.property(*group, "Nodes"))
            for (const auto& node : nodes->elements()) {
                ObjectPtr object = load(node);
                const Value* id = runtime_.property(*object, "NodeID");
                if (id && id->integer() == to) { act = actOf(object); return 1; }
            }
        notImplemented("a link to node " + std::to_string(to));
    }
    return 0;                                           // an event with nothing linked: the chain ends
}

DialogSystem::Handle DialogSystem::trigger(const Value& group, const Value& tagRef) {
    const Tag tag = tagOf(tagRef);
    if (!tagValid(tag)) return {};
    if (!group.o) notImplemented("a dialog event without a group (talker-owned events)");
    return run(Handle(), group, tag);
}

// TriggerGroupEvent from the found event on: the event data (the caller's `reuse` keeps its identity, else a pooled one), then the act chain.
DialogSystem::Handle DialogSystem::run(Handle reuse, const Value& group, const Tag& tag) {
    if (tag.soundEffect) notImplemented("sound-effect dialog events");
    Act act;
    const int found = findAct(group, tag, act);
    if (found < 0) return reuse;                         // no event for the tag: no event data
    Handle handle = reuse;
    if (handle.index < 0) {
        // An event data object: the first pooled one that is not live, reused (its use count rises) or a new one.
        size_t slot = 0;
        while (slot < data_.size() && data_[slot].live) ++slot;
        if (slot == data_.size()) data_.emplace_back();
        EventData& data = data_[slot];
        const int useCount = data.useCount + 1;
        data = EventData();
        data.useCount = useCount;
        handle = Handle{int(slot), useCount};
    }
    if (found == 0) return handle;
    talk(handle, act, tag, pathOf(&group), rootGroupOf(group));
    return handle;
}

// Output 1 of a talk act (no talker, bEnableNoMatch): the linked node. Only a Trigger act is modelled.
void DialogSystem::followNoMatch(Handle handle, const Act& act) {
    if (!act.noMatchNode.o) return;                      // not linked: the chain ends
    ObjectPtr node = load(act.noMatchNode);
    bool trigger = false;
    for (const Class* cls = node->cls; cls; cls = cls->super) if (cls->name == "WillowDialogAct_Trigger") trigger = true;
    if (!trigger) notImplemented("a no-match output into " + node->cls->path);
    runTrigger(handle, node);
}

// Act_Trigger.Activate (NATIVE_DIALOG.md): the talker variable's talkers (the event's Instigator variable here), those that can talk the DialogEvent (a
// group of theirs has an enabled entry for it); one is chosen (the only one here) and the DialogEvent is triggered on it with the same event data.
// The act's own output 0 (after the line) has no link in the stock data and is not followed.
void DialogSystem::runTrigger(Handle handle, const ObjectPtr& node) {
    const Value* eventRef = runtime_.property(*node, "DialogEvent");
    if (!eventRef || !eventRef->o) return;
    if (const Value* links = runtime_.property(*node, "OutputLinks"))
        for (const auto& link : links->elements())
            if (const Value* targets = link.field("Links"); targets && !targets->elements().empty()) notImplemented("an output link on a trigger act");
    std::vector<int> talkers;
    if (const Value* variables = runtime_.property(*node, "VariableLinks"))
        for (const auto& link : variables->elements())
            if (const Value* targets = link.field("Links"))
                for (const auto& target : targets->elements()) {
                    if (!target.o) continue;
                    ObjectPtr variable = load(target);
                    bool instigator = false;
                    for (const Class* cls = variable->cls; cls; cls = cls->super) if (cls->name == "GearboxDialogVar_Instigator") instigator = true;
                    if (!instigator) notImplemented("a dialog variable of class " + variable->cls->path);
                    if (instigator_ >= 0) talkers.push_back(instigator_);
                }
    const Tag tag = tagOf(*eventRef);
    if (!tagValid(tag)) return;
    for (const int talker : talkers) {
        const Value group = matchingGroup(talkers_[size_t(talker)].groups, tag, false);   // reused event data: no template groups
        if (!group.o) continue;                          // this talker cannot talk the event
        run(handle, group, tag);                         // the component's TriggerEvent, reusing the same event data
        return;
    }
}

// The talk act's Activate and GearboxDialogComponent.Talk. A pass-through for an act without audio; the talker; the gates (audio
// device, priority); then the live state, the group silence, the interruption of the talker's own line and the start of the audio.
void DialogSystem::talk(Handle handle, const Act& act, const Tag& tag, const std::string& groupPath, const std::string& root) {
    const bool hasAudio = std::any_of(act.talk.begin(), act.talk.end(), [](const TalkEntry& e) { return !e.akEvent.empty(); });
    if (!hasAudio) return;                               // no audio event: output 0 at once
    // The talker: an instigator that is an actor (the dialog's instigator here is the mission, never an actor: no talker), else a random
    // TalkData entry resolved by its exact name tag; an echo event may create the echo caller. A line with no talker is still
    // reported ("no talker") so the host can see what the stock dialog would have said.
    Line line;
    line.id = nextLine_++;
    line.eventTag = tag.path;
    line.group = groupPath;
    line.talkAct = act.path;
    line.outputDelay = act.outputDelay;
    const auto report = [&](const char* outcome) { if (onLine) onLine(line, outcome); };
    // Choosing the talker (NATIVE_DIALOG.md): the event's instigator when the act says so and it is a talker with an entry for its own name tag (exact
    // tag; the ParentTag ancestors are not modelled); otherwise, as before, the mission is no actor (no talker). With no talker and bEnableNoMatch the
    // act takes its output 1.
    const TalkEntry* entry = nullptr;
    int talker = -1;
    if (act.instigatorTalker && instigator_ >= 0) {
        for (const auto& candidate : act.talk)
            if (candidate.nameTag == talkers_[size_t(instigator_)].nameTag) { entry = &candidate; talker = instigator_; break; }
        if (talker < 0 && act.noMatch) { followNoMatch(handle, act); return; }
        if (talker < 0) entry = &act.talk.front();
    } else if (act.instigatorTalker) {
        entry = &act.talk.front();
    } else {
        entry = &act.talk[size_t(rng_() % act.talk.size())];
        talker = resolveTalker(entry->nameTag, tag.echo);
    }
    const TalkEntry& chosen = *entry;
    line.akEvent = chosen.akEvent;
    line.talker = chosen.nameTag;
    if (talker < 0) { report("no talker"); return; }
    line.echo = talkers_[size_t(talker)].echo;
    if (!player_ && testLength_ < 0) { report("no audio device"); return; }      // Talk does nothing without an audio device
    // Priority arbitration: a lower index is more important; the tracked mission's group is floored.
    int index = tag.priority;
    if (!trackedGroup_.empty() && groupPath == trackedGroup_ && index > startIndex_) index = std::min(index, trackedPlot_ ? plotFloor_ : sideFloor_);
    const bool mayOverride = tag.echo && !tag.noOverrideSame;
    const auto blocks = [&](int existing) { return mayOverride ? existing < index : existing <= index; };
    const auto groupLive = groupIndex_.find(root);
    if ((groupLive != groupIndex_.end() && blocks(groupLive->second)) ||
        (talkers_[size_t(talker)].liveData >= 0 && blocks(talkers_[size_t(talker)].liveIndex))) { report("blocked by priority"); return; }
    EventData& data = data_[size_t(handle.index)];
    data.live = true;
    data.talker = talker;
    data.rootGroup = root;
    data.outputDelay = act.outputDelay;
    data.finishTime = 0;
    data.lineId = line.id;
    if (tag.groupEvent || tag.echo) {                    // the group is silenced, then this event is the group's current one
        for (size_t i = 0; i < data_.size(); ++i)
            if (int(i) != handle.index && data_[i].live && data_[i].rootGroup == root) stopTalking(int(i));
        groupIndex_[root] = index;
    }
    Talker& speaker = talkers_[size_t(talker)];
    if (speaker.liveData >= 0 && speaker.liveData != handle.index) stopTalking(speaker.liveData);      // interrupts its own line
    speaker.liveData = handle.index;
    speaker.liveIndex = index;
    if (chosen.akEvent.empty()) { report("no audio event"); return; }          // live until the next update ends it
    data.playing = player_ ? player_(line) : true;
    if (testLength_ >= 0) data.testEnd = now_ + testLength_;
    if (tag.oncePerSession) played_.insert(tag.path);
    lineStarted_ = true;
    report(data.playing ? "started" : "not playing");
}

// Behavior_TriggerDialogEvent.ApplyBehaviorToContext (NATIVE_DIALOG.md): see the class comment.
std::vector<int> DialogSystem::behavior(BehaviorProvider& provider, BehaviorProvider::Behavior& behavior) {
    Runtime& r = provider.runtime();
    auto& run = provider.run();
    const Value* tag = r.property(*behavior.object, "EventTag");
    const Value* group = r.property(*behavior.object, "Group");
    const bool immediate = flag(r.property(*behavior.object, "bForcePlayImmediate"));
    std::vector<int> ids;
    if (tag && tag->o) {
        if (run.initialRun && !immediate) {
            run.wait = 0.001;                            // latent for one kernel wake; Out is selected below
        } else {
            Handle handle;
            if (run.state) handle = *static_cast<Handle*>(run.state.get());
            if (handle.index < 0 || immediate) { handle = trigger(group ? *group : Value(), *tag); run.state = std::make_shared<Handle>(handle); }
            if (handle.index < 0 || !active(handle) || immediate || !run.hasLinkedOutputs) ids.push_back(1);   // Finished
            else run.wait = 0.1;                         // poll while the line is live
            if (immediate) run.state.reset();
        }
    } else {
        ids.push_back(1);
    }
    if (run.initialRun) ids.push_back(0);               // Out, at the end of the first run
    return ids;
}

} // namespace vm
