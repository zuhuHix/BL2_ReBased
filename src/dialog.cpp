#include "dialog.hpp"

#include <algorithm>
#include <climits>

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
    const std::string key = pathOf(&reference) + "@" + reference.o->resourcePackage->packageName;
    auto found = loaded_.find(key);
    if (found == loaded_.end())
        found = loaded_.emplace(key, runtime_.instantiateExport(reference.o->resourcePackage, reference.o->resourceIndex, 4)).first;
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
    if (const Value* links = runtime_.property(*node, "OutputLinks"))
        for (const auto& link : links->elements())
            if (const Value* targets = link.field("Links"); targets && !targets->elements().empty()) notImplemented("an output link on a talk act");
    if (const Value* variable = runtime_.property(*node, "TalkerVariable"); variable && variable->o) notImplemented("a talker variable");
    Act act;
    act.path = node->resourcePackage ? node->resourcePackage->path(node->resourceIndex) : node->name;
    if (const Value* delay = runtime_.property(*node, "OutputDelay")) act.outputDelay = delay->number();
    act.instigatorTalker = flag(runtime_.property(*node, "bInstigatorTalker"));
    if (const Value* talk = runtime_.property(*node, "TalkData"))
        for (const auto& entry : talk->elements()) act.talk.push_back({pathOf(entry.field("NameTag")), pathOf(entry.field("TalkAkEvent"))});
    return act;
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
    if (const Value* action = entry.field("OutputAction"); action && action->o) { act = actOf(load(*action)); return 1; }
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
    if (tag.soundEffect) notImplemented("sound-effect dialog events");
    Act act;
    const int found = findAct(group, tag, act);
    if (found < 0) return {};                            // no event for the tag: no event data
    // An event data object: the first pooled one that is not live, reused (its use count rises) or a new one.
    size_t slot = 0;
    while (slot < data_.size() && data_[slot].live) ++slot;
    if (slot == data_.size()) data_.emplace_back();
    EventData& data = data_[slot];
    const int useCount = data.useCount + 1;
    data = EventData();
    data.useCount = useCount;
    const Handle handle{int(slot), useCount};
    if (found == 0) return handle;
    talk(handle, act, tag, pathOf(&group), rootGroupOf(group));
    return handle;
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
    const TalkEntry& chosen = act.instigatorTalker ? act.talk.front() : act.talk[size_t(rng_() % act.talk.size())];
    line.akEvent = chosen.akEvent;
    line.talker = chosen.nameTag;
    const int talker = act.instigatorTalker ? -1 : resolveTalker(chosen.nameTag, tag.echo);
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
