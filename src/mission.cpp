#include "mission.hpp"
#include "behavior.hpp"

#include <algorithm>
#include <sstream>

namespace vm {
namespace {
std::string refPath(const Value* value) {
    if (!value || value->kind != Value::Kind::Object || !value->o || !value->o->resourcePackage) return "";
    return value->o->resourcePackage->path(value->o->resourceIndex);
}
std::string text(Runtime& runtime, Object& object, const char* name) {
    const Value* value = runtime.property(object, name);
    return value && (value->kind == Value::Kind::String || value->kind == Value::Kind::Name) ? value->s : "";
}
ObjectPtr load(Runtime& runtime, const Value* reference) {
    if (!reference || reference->kind != Value::Kind::Object || !reference->o || !reference->o->resourcePackage) return nullptr;
    return runtime.instantiateExport(reference->o->resourcePackage, reference->o->resourceIndex, 4);
}
// A flag with its class default; `fallback` only when the class does not declare it.
bool flag(Runtime& runtime, Object& object, const char* name, bool fallback) {
    const Value* value = runtime.property(object, name);
    return value ? value->truth() : fallback;
}
int bits(int mask) {
    int count = 0;
    for (unsigned v = unsigned(mask); v; v &= v - 1) ++count;
    return count;
}
// EMissionStatus numbers (NotStarted 0, Active 1, RequiredObjectivesComplete 2, ReadyToTurnIn 3, Complete 4, Failed 5).
int nativeStatus(MissionSystem::Status status) {
    static const int numbers[] = {0, 1, 3, 4};
    return numbers[int(status)];
}
// Mission event link ids (NATIVE_MISSION_DISPATCH.md B7, UNVERIFIED).
enum LinkId { ObjectiveCompleted = 2, ObjectiveProgress = 3, SetActivated = 4, SetCompleted = 5, StatusBase = 6, Kickoff = 12, KickoffDialogOnly = 13, TurnIn = 14 };
}

struct MissionSystem::Impl {
    std::unique_ptr<BehaviorProvider> provider;
};

MissionSystem::MissionSystem(Runtime& runtime, const std::string& package, const std::string& missionPath)
    : runtime_(runtime), package_(runtime.package(package)), missionPath_(missionPath), impl_(std::make_shared<Impl>()), dialog_(runtime, package_) {
    dialog_.onLine = [this](const DialogSystem::Line& line, const std::string& outcome) {
        emit(Effect::Kind::Dialog, line.eventTag, line.group, line.talker);
        effects_.back().detail = "act=" + line.talkAct + ";ak=" + line.akEvent + ";talker=" + (line.echo ? "echo" : "pawn") + ";outcome=" + outcome +
                                 ";line=" + std::to_string(line.id);
    };
    const int32_t index = runtime_.findExport(*package_, missionPath);
    if (index <= 0) throw RuntimeError("mission not found: " + missionPath);
    auto mission = runtime_.instantiateExport(package_, index, 4);
    definition_ = mission;
    if (mission->cls->path != "WillowGame.MissionDefinition") throw RuntimeError("not a MissionDefinition: " + missionPath);
    missionName_ = text(runtime_, *mission, "MissionName");
    description_ = text(runtime_, *mission, "MissionDescription");
    giver_ = text(runtime_, *mission, "MissionGiver");
    turnIn_ = text(runtime_, *mission, "MissionTurnInLocation");
    weapon_ = refPath(runtime_.property(*mission, "MissionWeapon"));
    initialSet_ = refPath(runtime_.property(*mission, "InitialObjectiveSet"));
    activateInitialSet_ = flag(runtime_, *mission, "bActivateInitialObjectiveSet", true);
    // ObjectiveDependency { Objective, Status: EODS_Complete 0 / EODS_Active 1 } (B6).
    if (const Value* dependency = runtime_.property(*mission, "ObjectiveDependency"); dependency && dependency->kind == Value::Kind::Struct) {
        dependencyObjective_ = refPath(dependency->field("Objective"));
        const Value* status = dependency->field("Status");
        dependencyActive_ = status && status->integer() == 1;
    }
    // The lent mission weapon is tied to one objective by its definition.
    if (const Value* weapon = runtime_.property(*mission, "MissionWeapon"); weapon && weapon->o)
        if (auto definition = load(runtime_, weapon)) weaponObjective_ = refPath(runtime_.property(*definition, "MissionObjective"));
    if (const Value* dependencies = runtime_.property(*mission, "Dependencies"))
        if (dependencies->kind == Value::Kind::Array)
            for (const auto& dependency : dependencies->elements()) dependencies_.push_back(refPath(&dependency));
    if (const Value* reward = runtime_.property(*mission, "Reward"))
        if (const Value* xp = reward->field("ExperienceRewardPercentage"))
            if (const Value* attribute = xp->field("BaseValueAttribute")) xpAttribute_ = refPath(attribute);

    if (const Value* setDefs = runtime_.property(*mission, "ObjectiveSetDefs"); setDefs && setDefs->kind == Value::Kind::Array) {
        for (const auto& reference : setDefs->elements()) {
            auto object = load(runtime_, &reference);
            if (!object) throw RuntimeError("unresolved objective set in " + missionPath);
            ObjectiveSet set;
            set.path = refPath(&reference);
            set.name = text(runtime_, *object, "ObjectiveSetName");
            if (set.name.empty()) set.name = object->name;
            set.next = refPath(runtime_.property(*object, "NextSet"));
            set.cls = object->cls->path;
            set.canCompleteMission = flag(runtime_, *object, "bCanCompleteMission", true);
            set.autoEnableNext = flag(runtime_, *object, "bAutoEnableNextSet", false);
            if (const Value* objectives = runtime_.property(*object, "ObjectiveDefinitions"); objectives && objectives->kind == Value::Kind::Array) {
                for (const auto& objectiveRef : objectives->elements()) {
                    auto objective = load(runtime_, &objectiveRef);
                    if (!objective) throw RuntimeError("unresolved objective in " + set.path);
                    std::string objectiveName = text(runtime_, *objective, "ObjectiveName");
                    if (objectiveName.empty()) objectiveName = objective->name;
                    set.objectives.push_back(objectiveName);
                    set.objectivePaths.push_back(refPath(&objectiveRef));
                    Objective parsed{refPath(&objectiveRef)};
                    if (const Value* count = runtime_.property(*objective, "ObjectiveCount")) parsed.count = int(count->integer());
                    parsed.mask = flag(runtime_, *objective, "bRememberItemsWithinObjective", false);
                    parsed.optional = flag(runtime_, *objective, "bObjectiveIsOptional", false);
                    objectives_[objectiveName] = parsed;
                }
            }
            sets_.push_back(std::move(set));
        }
    }

    // The mission's behavior provider; the mission behaviors are bound below.
    if (const Value* reference = runtime_.property(*mission, "BehaviorProvider"); reference && reference->o && reference->o->resourcePackage) {
        impl_->provider = std::make_unique<BehaviorProvider>(runtime_, reference->o->resourcePackage, reference->o->resourceIndex);
        auto& provider = *impl_->provider;
        provider.onTime = [this](double time) { dialog_.advanceTo(time); };
        provider.handle("WillowGame.Behavior_AdvanceObjectiveSet", [this](BehaviorProvider& p, BehaviorProvider::Behavior& b, const std::string&) {
            requestAdvance(refPath(p.runtime().property(*b.object, "ObjectiveSetToAdvanceTo")));
            return std::vector<int>();
        });
        provider.handle("WillowGame.Behavior_MissionRemoteEvent", [this](BehaviorProvider& p, BehaviorProvider::Behavior& b, const std::string&) {
            emit(Effect::Kind::RemoteEvent, text(p.runtime(), *b.object, "EventName"));
            return std::vector<int>();
        });
        provider.handle("WillowGame.Behavior_UpdateMissionObjective", [this](BehaviorProvider& p, BehaviorProvider::Behavior& b, const std::string&) {
            updateObjectiveByPath(refPath(p.runtime().property(*b.object, "MissionObjective")));
            return std::vector<int>();
        });
        // Outputs ETriggerDialogEventOutputLinks: Out 0, Finished 1. Out on the first run, the dialog one kernel wake later, Finished
        // when the live line ends (at once when no line starts): src/dialog.* (NATIVE_DIALOG.md, UNVERIFIED).
        provider.handle("GearboxFramework.Behavior_TriggerDialogEvent", [this](BehaviorProvider& p, BehaviorProvider::Behavior& b, const std::string&) {
            return dialog_.behavior(p, b);
        });
        // Action is an ITargetable.EChangeStatus (CHANGE_Toggle, CHANGE_Enable, CHANGE_Disable; class default Enable).
        const auto actions = enumNames(runtime_, "Engine", "ITargetable.EChangeStatus");
        provider.handle("GearboxFramework.Behavior_ChangeRemoteBehaviorSequenceState", [this, actions](BehaviorProvider& p, BehaviorProvider::Behavior& b, const std::string&) {
            const Value* path = p.runtime().property(*b.object, "ProviderDefinitionPathName");
            const Value* components = path ? path->field("PathComponentNames") : nullptr;
            const Value* action = p.runtime().property(*b.object, "Action");
            const int64_t index = action ? action->integer() : -1;
            emit(Effect::Kind::SetSequence, providerPathLeaf(components), text(p.runtime(), *b.object, "SequenceName"),
                 index >= 0 && size_t(index) < actions.size() ? actions[size_t(index)] : "");
            return std::vector<int>();
        });
    }
}

bool MissionSystem::hasOptionalObjective() const {
    return std::any_of(objectives_.begin(), objectives_.end(), [](const auto& entry) { return entry.second.optional; });
}

// The tracked mission (its tracker's ActiveMission) while Active or ReadyToTurnIn: its MissionDialogGroup gives dialog its priority floor.
void MissionSystem::updateTrackedMission() {
    const bool tracked = status_ == Status::Active || status_ == Status::ReadyToTurnIn;
    const Value* group = runtime_.property(*definition_, "MissionDialogGroup");
    const Value* plot = runtime_.property(*definition_, "bPlotCritical");
    dialog_.setTrackedMission(tracked ? refPath(group) : "", plot && plot->truth());
}

int MissionSystem::statusNumber() const { return nativeStatus(status_); }

bool MissionSystem::available(const std::set<std::string>& completed, const std::map<std::string, std::string>& objectiveStates) const {
    for (const auto& dependency : dependencies_)
        if (!completed.count(dependency)) return false;
    if (dependencyObjective_.empty()) return true;
    // B6 (UNVERIFIED): the objective complete, or, for an Active dependency, the objective currently updatable.
    const auto state = objectiveStates.find(dependencyObjective_);
    if (state == objectiveStates.end()) return false;
    return state->second == "Complete" || (dependencyActive_ && state->second == "Active");
}

void MissionSystem::emit(Effect::Kind kind, std::string a, std::string b, std::string c) {
    effects_.push_back({kind, std::move(a), std::move(b), std::move(c), now_});
}

std::vector<MissionSystem::Effect> MissionSystem::drain() {
    auto result = std::move(effects_);
    effects_.clear();
    return result;
}

// B4 (UNVERIFIED): ReadyToTurnIn only from Active, Complete only from ReadyToTurnIn; every accepted change fires
// "Default" with id 6 + the EMissionStatus number. Bridge note ("MissionTracker.SetMissionStatus"): the script hook
// UpdateMissionStatus runs inside the status branch, before the observers and the "Default" event, and the Active branch
// writes the pending kickoff (SetActiveMission) before that tail.
bool MissionSystem::setStatus(Status status) {
    const bool allowed = (status == Status::Active && status_ == Status::NotStarted) ||
                         (status == Status::ReadyToTurnIn && status_ == Status::Active) ||
                         (status == Status::Complete && status_ == Status::ReadyToTurnIn);
    if (!allowed) return false;
    status_ = status;
    if (status == Status::Complete) activeSet_.clear();
    updateTrackedMission();
    if (onStatusChanged) onStatusChanged(nativeStatus(status));
    // SetActiveMission(mission, fromActivation): a record is written only when the kickoff was not heard and none is pending.
    if (status == Status::Active && !heardKickoff_ && !kickoffPending_) { kickoffPending_ = true; kickoffFromActivation_ = true; }
    static const char* names[] = {"NotStarted", "Active", "ReadyToTurnIn", "Complete"};
    emit(Effect::Kind::StatusChanged, names[int(status)]);
    if (onNotification) onNotification(Notification::StatusChanged);
    fireEvent("Default", StatusBase + nativeStatus(status));
    return true;
}

void MissionSystem::fireEvent(const std::string& name, int linkId) {
    if (impl_->provider) impl_->provider->fireEvent(name, {}, linkId);
}

// The tracker tick consumes the pending kickoff record (bridge note): the accepting controller's IsMissionMoviePlaying
// runs (its result is not used), then PlayKickoff (id 12) when the record came from an activation, else
// PlayKickoffDialogOnly (13); the mission's bHeardKickoff is set and the record cleared. UNVERIFIED. Not modelled: the
// "Loader" level wait, the tracked-mission switch and the dialog request (the Fire mission lists no DialogEvent).
void MissionSystem::tick(double seconds) {
    now_ += seconds;
    if (kickoffPending_) {
        if (onKickoffTick) onKickoffTick();
        fireEvent("Default", kickoffFromActivation_ ? Kickoff : KickoffDialogOnly);
        heardKickoff_ = true;
        kickoffPending_ = false;
    }
    // The dialog components' per-frame update follows the kernel's time (each wake of a thread), or the frame's without a provider.
    if (impl_->provider) impl_->provider->tick(seconds); else dialog_.tick(seconds);
    collectProviderErrors();
}

void MissionSystem::collectProviderErrors() {
    if (!impl_->provider) return;
    for (const auto& line : impl_->provider->errors)
        if (std::find(errors.begin(), errors.end(), line) == errors.end()) errors.push_back(line);
}

const MissionSystem::ObjectiveSet* MissionSystem::findSet(const std::string& path) const {
    const auto found = std::find_if(sets_.begin(), sets_.end(), [&](const ObjectiveSet& s) { return s.path == path; });
    return found == sets_.end() ? nullptr : &*found;
}

// B3 (UNVERIFIED): only the active set's NextSet, or the InitialObjectiveSet while no set is active; anything else is
// ignored silently.
void MissionSystem::requestAdvance(const std::string& target) {
    if (target.empty()) return;
    const ObjectiveSet* active = findSet(activeSet_);
    if ((active && active->next == target) || (activeSet_.empty() && target == initialSet_)) activateSet(target);
}

// B3 (UNVERIFIED): refused when ReadyToTurnIn or Complete; set event id 4; then the B2 evaluation when the new set
// can complete the mission.
bool MissionSystem::activateSet(const std::string& setPath) {
    if (status_ == Status::ReadyToTurnIn || status_ == Status::Complete) return false;
    const ObjectiveSet* set = findSet(setPath);
    if (!set) { errors.push_back("advance to unknown objective set " + setPath); return false; }
    if (set->cls != "WillowGame.MissionObjectiveSetDefinition") {
        errors.push_back("unsupported objective set class " + set->cls + " (" + set->name + ")");
        return false;
    }
    activeSet_ = setPath;
    emit(Effect::Kind::ObjectiveSetActive, set->name);
    // UNVERIFIED: the mission weapon is lent while the objective it belongs to is active (the native reading grants it
    // at status Active and removes it at Complete; not adopted yet, see DECISIONS.md 2026-10-02).
    if (!weapon_.empty() && std::find(set->objectivePaths.begin(), set->objectivePaths.end(), weaponObjective_) != set->objectivePaths.end())
        emit(Effect::Kind::MissionWeaponGranted, weapon_);
    const std::string name = set->name;
    const bool evaluate = set->canCompleteMission;
    if (onNotification) onNotification(Notification::ObjectiveSetChanged);
    fireEvent(name, SetActivated);
    if (evaluate && activeSet_ == setPath) evaluateSet();
    return true;
}

// B2 (UNVERIFIED): a complete set fires id 5, then bCanCompleteMission -> ReadyToTurnIn, else bAutoEnableNextSet -> NextSet,
// else nothing (a behavior must advance it). Only optional objectives left in a bCanCompleteMission set: ReadyToTurnIn.
void MissionSystem::evaluateSet() {
    const ObjectiveSet* set = findSet(activeSet_);
    if (!set) return;
    bool all = true, required = true;
    for (const auto& objective : set->objectives) {
        if (completedObjectives_.count(objective)) continue;
        all = false;
        const auto found = objectives_.find(objective);
        if (!set->canCompleteMission || found == objectives_.end() || !found->second.optional) required = false;
    }
    if (all) {
        if (!completedSets_.insert(set->path).second) return;
        const std::string name = set->name, next = set->next;
        const bool canComplete = set->canCompleteMission, autoNext = set->autoEnableNext;
        fireEvent(name, SetCompleted);
        if (canComplete) setStatus(Status::ReadyToTurnIn);
        else if (autoNext && !next.empty()) activateSet(next);
    } else if (required && status_ == Status::Active) setStatus(Status::ReadyToTurnIn);
}

bool MissionSystem::accept(const std::set<std::string>& completed) {
    if (status_ != Status::NotStarted || !available(completed)) return false;
    // B4 (UNVERIFIED): which runs first, the status event or the initial set, was not settled; status first here.
    setStatus(Status::Active);
    if (activateInitialSet_ && activeSet_.empty() && !initialSet_.empty()) activateSet(initialSet_);
    collectProviderErrors();
    return true;
}

bool MissionSystem::kickoff(bool dialogOnly) {
    if (status_ != Status::Active) return false;
    fireEvent("Default", dialogOnly ? KickoffDialogOnly : Kickoff);
    heardKickoff_ = true;
    kickoffPending_ = false;
    collectProviderErrors();
    return true;
}

// PlayTurnIn with a mission: the "Default" event, id 14, and nothing else (bridge note).
void MissionSystem::playTurnIn() {
    fireEvent("Default", TurnIn);
    collectProviderErrors();
}

int MissionSystem::objectiveProgress(const std::string& objectiveName) const {
    const auto value = progress_.find(objectiveName);
    if (value == progress_.end()) return 0;
    const auto objective = objectives_.find(objectiveName);
    // UNVERIFIED: TranslateObjectiveCount of a bit mask is taken as its number of set bits.
    return objective != objectives_.end() && objective->second.mask ? bits(value->second) : value->second;
}

// B1 (UNVERIFIED): updates are queued; one raised while another is applied waits its turn instead of nesting.
bool MissionSystem::updateObjective(const std::string& objectiveName, int bit) {
    updates_.push_back({objectiveName, bit});
    if (draining_) return true;
    draining_ = true;
    bool first = true, result = false;
    try {
        while (!updates_.empty()) {
            const auto [name, value] = updates_.front();
            updates_.pop_front();
            const bool applied = applyUpdate(name, value);
            if (first) { result = applied; first = false; }
        }
    } catch (...) {
        draining_ = false;
        updates_.clear();
        throw;
    }
    draining_ = false;
    collectProviderErrors();
    return result;
}

// B1 (UNVERIFIED): id 3 on every accepted update; complete when the count equals ObjectiveCount; then the set evaluation,
// and only then id 2.
bool MissionSystem::applyUpdate(const std::string& objectiveName, int bit) {
    if (status_ != Status::Active) return false;
    const ObjectiveSet* set = findSet(activeSet_);
    if (!set || std::find(set->objectives.begin(), set->objectives.end(), objectiveName) == set->objectives.end()) return false;
    const auto found = objectives_.find(objectiveName);
    if (found == objectives_.end()) return false;
    const Objective objective = found->second;
    if (objectiveProgress(objectiveName) >= objective.count) return false;
    if (objective.mask) {
        if (bit == 0) return false;
        progress_[objectiveName] |= bit;
    } else ++progress_[objectiveName];
    const int count = objectiveProgress(objectiveName);
    emit(Effect::Kind::ObjectiveUpdated, objectiveName, std::to_string(count));
    if (onNotification) onNotification(Notification::ObjectiveUpdated);
    fireEvent(objectiveName, ObjectiveProgress);
    if (count != objective.count) return true;
    completedObjectives_.insert(objectiveName);
    emit(Effect::Kind::ObjectiveComplete, objectiveName);
    if (!weapon_.empty() && objective.path == weaponObjective_) emit(Effect::Kind::MissionWeaponRemoved, weapon_);
    if (onNotification) onNotification(Notification::ObjectiveComplete);
    evaluateSet();
    fireEvent(objectiveName, ObjectiveCompleted);
    return true;
}

bool MissionSystem::updateObjectiveByPath(const std::string& objectivePath, int bit) {
    for (const auto& [name, objective] : objectives_)
        if (objective.path == objectivePath) return updateObjective(name, bit);
    errors.push_back("objective is not part of this mission: " + objectivePath);
    return false;
}

// NATIVE_BEHAVIOR_POPULATION.md section B (UNVERIFIED), the tracker's own classification: Complete when the mission is Active, ReadyToTurnIn
// or Complete and the progress equals the count exactly; else Active when the mission is Active, the objective is in the active
// set and the progress is below the count; else NotStarted. "" when the path is not one of the mission's objectives.
std::string MissionSystem::objectiveState(const std::string& objectivePath) const {
    bool known = false, inActiveSet = false;
    std::string name;
    for (const auto& set : sets_)
        for (size_t i = 0; i < set.objectivePaths.size(); ++i) {
            if (set.objectivePaths[i] != objectivePath) continue;
            known = true;
            name = set.objectives[i];
            if (set.path == activeSet_) inActiveSet = true;
        }
    if (!known) return "";
    const auto objective = objectives_.find(name);
    if (objective == objectives_.end()) return "";
    const int progress = objectiveProgress(name);
    if ((status_ == Status::Active || status_ == Status::ReadyToTurnIn || status_ == Status::Complete) && progress == objective->second.count)
        return "Complete";
    if (status_ == Status::Active && inActiveSet && progress < objective->second.count) return "Active";
    return "NotStarted";
}

// RunMissionCustomEvent: id 0, unless the mission is Complete (B4, UNVERIFIED).
bool MissionSystem::customEvent(const std::string& name) {
    if (status_ == Status::Complete) return false;
    fireEvent(name, 0);
    collectProviderErrors();
    return true;
}

bool MissionSystem::turnInMission() {
    if (!setStatus(Status::Complete)) return false;
    emit(Effect::Kind::Reward, xpAttribute_);
    collectProviderErrors();
    return true;
}

std::string MissionSystem::saveState() const {
    std::ostringstream out;
    out << "status=" << int(status_) << "\nactive=" << activeSet_ << '\n';
    for (const auto& o : completedObjectives_) out << "objective=" << o << '\n';
    for (const auto& [o, value] : progress_)
        if (!completedObjectives_.count(o)) out << "progress=" << o << ':' << value << '\n';
    for (const auto& s : completedSets_) out << "set=" << s << '\n';
    return out.str();
}

bool MissionSystem::loadState(const std::string& state) {
    std::istringstream in(state);
    std::string line;
    Status status = Status::NotStarted;
    std::string active;
    std::set<std::string> objectives, setsDone;
    std::map<std::string, int> progress;
    bool sawStatus = false;
    while (std::getline(in, line)) {
        const auto eq = line.find('=');
        if (eq == std::string::npos) return false;
        const std::string key = line.substr(0, eq), value = line.substr(eq + 1);
        if (key == "status") {
            if (value.size() != 1 || value[0] < '0' || value[0] > '3') return false;
            status = Status(value[0] - '0'); sawStatus = true;
        } else if (key == "active") active = value;
        else if (key == "objective") objectives.insert(value);
        else if (key == "set") setsDone.insert(value);
        else if (key == "progress") {
            const auto colon = value.rfind(':');
            if (colon == std::string::npos || colon + 1 == value.size()) return false;
            try { progress[value.substr(0, colon)] = std::stoi(value.substr(colon + 1)); } catch (const std::exception&) { return false; }
        }
        else return false;
    }
    if (!sawStatus) return false;
    if (!active.empty() && std::none_of(sets_.begin(), sets_.end(), [&](const ObjectiveSet& s) { return s.path == active; })) return false;
    // A completed objective's progress is its count (an older save lists completed objectives only).
    for (const auto& o : objectives) {
        const auto found = objectives_.find(o);
        progress[o] = found == objectives_.end() ? 1 : found->second.mask ? (1 << found->second.count) - 1 : found->second.count;
    }
    status_ = status; activeSet_ = active; completedObjectives_ = objectives; completedSets_ = setsDone; progress_ = progress;
    // A restored mission has played its kickoff (bHeardKickoff is not saved; this is an assumption, UNVERIFIED).
    kickoffPending_ = false;
    heardKickoff_ = status != Status::NotStarted;
    updateTrackedMission();
    return true;
}

} // namespace vm
