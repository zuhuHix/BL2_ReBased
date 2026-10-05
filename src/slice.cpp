#include "slice.hpp"

#include <algorithm>

namespace vm {
namespace {
std::string refPath(const Value* value) {
    if (!value || value->kind != Value::Kind::Object || !value->o || !value->o->resourcePackage) return "";
    return value->o->resourcePackage->path(value->o->resourceIndex);
}
std::string nameOf(const std::vector<std::string>& names, const Value* value) {
    const int64_t index = value ? value->integer() : 0;
    return index >= 0 && size_t(index) < names.size() ? names[size_t(index)] : "?" + std::to_string(index);
}
}

FireMissionSlice::FireMissionSlice(Runtime& runtime, const std::string& missionPath, const std::string& dummyProviderPackage,
                                   const std::string& dummyProviderPath)
    : runtime_(runtime) {
    mission_ = std::make_unique<MissionSystem>(runtime, "Startup", missionPath);
    script_ = std::make_unique<MissionScript>(runtime, *mission_);
    auto package = runtime.package(dummyProviderPackage);
    const int32_t index = runtime.findExport(*package, dummyProviderPath);
    if (index <= 0) throw RuntimeError("dummy behavior provider not found: " + dummyProviderPath);
    dummy_ = std::make_unique<BehaviorProvider>(runtime, package, index);
    dummyName_ = dummyProviderPath.substr(dummyProviderPath.rfind('.') + 1);
    auto& d = *dummy_;
    d.handle("WillowGame.Behavior_UpdateMissionObjective", [this](BehaviorProvider& p, BehaviorProvider::Behavior& b, const std::string&) {
        mission_->updateObjectiveByPath(refPath(p.runtime().property(*b.object, "MissionObjective")));
        return std::vector<int>();
    });
    // Behavior_CompareObject runs through the provider's built-in handler (inputs from the variable data).
    d.handle("WillowGame.Behavior_AttemptStatusEffect", [this](BehaviorProvider& p, BehaviorProvider::Behavior& b, const std::string&) {
        events_.push_back({HostEvent::Kind::StatusEffect, refPath(p.runtime().property(*b.object, "StatusEffect")), "", ""});
        return std::vector<int>();
    });
    // World ops with no binding yet: reported with the decoded fields the host needs to run them.
    const auto contexts = enumNames(runtime, "Engine", "BehaviorBase.EBehaviorContext");
    const auto context = [contexts](BehaviorProvider& p, BehaviorProvider::Behavior& b) {
        const Value* data = p.runtime().property(*b.object, "Context");
        return nameOf(contexts, data ? data->field("BehaviorContext") : nullptr);
    };
    const auto transforms = enumNames(runtime, "WillowGame", "AIPawnBalanceDefinition.EAITransformed");
    // Installed script: sets WillowAIPawn.TransformType = Transform on the context's IBodyPawn (no amount, no duration).
    d.reportAtBoundary("WillowGame.Behavior_Transform", [transforms, context](BehaviorProvider& p, BehaviorProvider::Behavior& b) {
        return std::map<std::string, std::string>{{"Transform", nameOf(transforms, p.runtime().property(*b.object, "Transform"))},
                                                  {"Context", context(p, b)},
                                                  {"Sets", "WillowAIPawn.TransformType"}};
    });
    // Installed script: calls ITargetable.Behavior_RegisterTargetable(bUnregister) on the context object.
    d.reportAtBoundary("WillowGame.Behavior_RegisterTargetable", [context](BehaviorProvider& p, BehaviorProvider::Behavior& b) {
        const Value* unregister = p.runtime().property(*b.object, "bUnregister");
        return std::map<std::string, std::string>{{"bUnregister", unregister && unregister->truth() ? "true" : "false"},
                                                  {"Context", context(p, b)},
                                                  {"Calls", "ITargetable.Behavior_RegisterTargetable"}};
    });
    // OnSpawned (Default sequence): native Behavior_IntMath (no script) computes a value that Behavior_ChangeInstanceDataSwitch
    // hands to IBodyCompositionInstance.ChangeInstanceDataSwitch(SwitchName, NewValue) (installed script): a body-composition
    // switch (RatHead, RatMasks), i.e. cosmetic, not the damage-sequence choice. Neither is computed here.
    const auto operations = enumNames(runtime, "WillowGame", "Behavior_SimpleMath.EBinaryMathOperation");
    d.reportAtBoundary("WillowGame.Behavior_IntMath", [operations](BehaviorProvider& p, BehaviorProvider::Behavior& b) {
        const auto a = p.intInput(b, "A"), bValue = p.intInput(b, "B");
        return std::map<std::string, std::string>{{"Operation", nameOf(operations, p.runtime().property(*b.object, "Operation"))},
                                                  {"A", a ? std::to_string(*a) : "?"}, {"B", bValue ? std::to_string(*bValue) : "?"}};
    });
    d.reportAtBoundary("WillowGame.Behavior_ChangeInstanceDataSwitch", [context](BehaviorProvider& p, BehaviorProvider::Behavior& b) {
        bool linked = false;
        const auto value = p.intInput(b, "NewValue", &linked);
        const Value* name = p.runtime().property(*b.object, "SwitchName");
        return std::map<std::string, std::string>{{"SwitchName", name ? name->s : ""},
                                                  {"NewValue", linked ? "variable (written by a behavior not run here)" : (value ? std::to_string(*value) : "?")},
                                                  {"Context", context(p, b)},
                                                  {"Calls", "IBodyCompositionInstance.ChangeInstanceDataSwitch"}};
    });
    d.handle("GearboxFramework.Behavior_AIHold", [](BehaviorProvider&, BehaviorProvider::Behavior&, const std::string&) {
        return std::vector<int>();
    });
    d.handle("Engine.Behavior_RemoteEvent", [this](BehaviorProvider& p, BehaviorProvider::Behavior& b, const std::string&) {
        const Value* name = p.runtime().property(*b.object, "EventName");
        events_.push_back({HostEvent::Kind::RemoteEvent, name ? name->s : "", "", ""});
        return std::vector<int>();
    });
    const auto actions = enumNames(runtime, "Engine", "ITargetable.EChangeStatus");
    d.handle("GearboxFramework.Behavior_ChangeRemoteBehaviorSequenceState", [this, actions](BehaviorProvider& p, BehaviorProvider::Behavior& b, const std::string&) {
        const Value* path = p.runtime().property(*b.object, "ProviderDefinitionPathName");
        const Value* components = path ? path->field("PathComponentNames") : nullptr;
        const Value* sequence = p.runtime().property(*b.object, "SequenceName");
        if (components && components->s == dummyName_ && sequence)
            changeSequence(sequence->s, nameOf(actions, p.runtime().property(*b.object, "Action")));
        else p.errors.push_back("sequence change targets another provider: " + (components ? components->s : std::string()));
        return std::vector<int>();
    });
}

// Behavior_ChangeRemoteBehaviorSequenceState.Action (ITargetable.EChangeStatus). Its script hands the action to the
// native BehaviorKernel.ChangeBehaviorSequenceActivationStatus; that Toggle flips the current state is UNVERIFIED.
void FireMissionSlice::changeSequence(const std::string& sequence, const std::string& action) {
    if (action == "CHANGE_Enable") dummy_->setSequenceEnabled(sequence, true);
    else if (action == "CHANGE_Disable") dummy_->setSequenceEnabled(sequence, false);
    else if (action == "CHANGE_Toggle") dummy_->setSequenceEnabled(sequence, !dummy_->sequenceEnabled(sequence));
    else errors_.push_back("unknown sequence change action '" + action + "' for " + sequence);
}

// BehaviorSequenceEnableByMission. The data: LinkedMission, MissionStatesToLinkTo (bNotStarted, bActive,
// bRequiredObjectivesComplete, bReadyToTurnIn, bComplete, bFailed; class default {bActive}), bIsObjectiveSpecific,
// LinkedObjective, ObjectiveStatesToLinkTo (bNotStarted, bActive, bComplete; class default {bActive}),
// ObjectiveSetRestrictions. Its evaluation is native; this rule is UNVERIFIED. A mission other than the slice's
// is Complete when it is in the completed set, else NotStarted.
bool FireMissionSlice::conditionHolds(Object& condition) {
    Runtime& r = runtime_;
    const std::string mission = refPath(r.property(condition, "LinkedMission"));
    const bool own = mission == mission_->path();
    static const char* statusFields[] = {"bNotStarted", "bActive", "bReadyToTurnIn", "bComplete"};
    const std::string missionState = own ? statusFields[int(mission_->status())]
                                         : (completedMissions_.count(mission) ? "bComplete" : "bNotStarted");
    const Value* missionStates = r.property(condition, "MissionStatesToLinkTo");
    const Value* missionMatch = missionStates ? missionStates->field(missionState) : nullptr;
    if (!missionMatch || !missionMatch->truth()) return false;
    if (const Value* restrictions = r.property(condition, "ObjectiveSetRestrictions");
        restrictions && restrictions->kind == Value::Kind::Array && !restrictions->elements().empty()) {
        errors_.push_back("unsupported ObjectiveSetRestrictions on " + condition.name);
        return false;
    }
    const Value* specific = r.property(condition, "bIsObjectiveSpecific");
    if (!specific || !specific->truth()) return true;
    const std::string objective = refPath(r.property(condition, "LinkedObjective"));
    const std::string state = own ? mission_->objectiveState(objective) : "NotStarted";
    if (state.empty()) { errors_.push_back("enable condition names an objective outside its mission: " + objective); return false; }
    const Value* objectiveStates = r.property(condition, "ObjectiveStatesToLinkTo");
    const Value* objectiveMatch = objectiveStates ? objectiveStates->field("b" + state) : nullptr;
    return objectiveMatch && objectiveMatch->truth();
}

bool FireMissionSlice::syncSequences() {
    bool changed = false;
    for (const auto& name : dummy_->sequenceNames()) {
        auto condition = dummy_->enableCondition(name);
        if (!condition) continue;
        if (condition->cls->path != "WillowGame.BehaviorSequenceEnableByMission") {
            const std::string line = "unsupported enable condition class " + condition->cls->path;
            if (std::find(errors_.begin(), errors_.end(), line) == errors_.end()) errors_.push_back(line);
            continue;
        }
        const bool enabled = conditionHolds(*condition);
        if (dummy_->sequenceEnabled(name) == enabled) continue;
        dummy_->setSequenceEnabled(name, enabled);
        changed = true;
    }
    return changed;
}

bool FireMissionSlice::drainMission() {
    // Mission effects are routed: sequence changes for the dummy go to its provider, the rest to the host.
    bool any = false;
    for (auto& effect : mission_->drain()) {
        any = true;
        using K = MissionSystem::Effect::Kind;
        switch (effect.kind) {
        case K::SetSequence:
            if (effect.a == dummyName_) changeSequence(effect.b, effect.c);
            else errors_.push_back("mission sequence change targets an unbound provider: " + effect.a);
            break;
        case K::RemoteEvent: events_.push_back({HostEvent::Kind::RemoteEvent, effect.a, mission_->path(), ""}); break;
        case K::Dialog: events_.push_back({HostEvent::Kind::Dialog, effect.a, effect.b, effect.c}); break;
        case K::MissionWeaponGranted: events_.push_back({HostEvent::Kind::MissionWeaponGranted, effect.a, "", ""}); break;
        case K::MissionWeaponRemoved: events_.push_back({HostEvent::Kind::MissionWeaponRemoved, effect.a, "", ""}); break;
        case K::Reward: events_.push_back({HostEvent::Kind::Reward, effect.a, "", ""}); break;
        case K::StatusChanged: events_.push_back({HostEvent::Kind::Status, effect.a, "", ""}); break;
        case K::ObjectiveSetActive: events_.push_back({HostEvent::Kind::ObjectiveSet, effect.a, "", ""}); break;
        case K::ObjectiveComplete: events_.push_back({HostEvent::Kind::ObjectiveComplete, effect.a, "", ""}); break;
        case K::ObjectiveUpdated: break;   // progress only; completion is reported separately
        }
    }
    return any;
}

void FireMissionSlice::pump() {
    // Mission state changes can toggle dummy sequences, whose behaviors can change mission state again: repeat
    // until both are quiet (bounded).
    for (int round = 0; round < 16; ++round) {
        const bool drained = drainMission();
        const bool changed = syncSequences();
        if (!drained && !changed) return;
    }
    errors_.push_back("mission/dummy state did not settle");
}

// WillowPlayerController.AcceptMission runs as script; its native ActivateMission drives MissionSystem. The kickoff is the
// pending record that the next tick() plays (bridge note); the mission's first objective set follows from it.
bool FireMissionSlice::accept(const std::set<std::string>& completed) {
    completedMissions_ = completed;
    const bool ok = script_->accept(completed);
    pump();
    return ok;
}

bool FireMissionSlice::enterRange() {
    const bool ok = mission_->updateObjective("RockPaper_GoToRange");
    pump();
    return ok;
}

bool FireMissionSlice::damageDummy(const std::string& damageTypePath, const std::string& damageSourcePath) {
    dummy_->fireEvent("OnTakeDamage", {{"DamageType", damageTypePath}, {"DamageSource", damageSourcePath}});
    pump();
    return true;
}

bool FireMissionSlice::spawnDummy() {
    dummy_->fireEvent("OnSpawned");
    pump();
    return true;
}

bool FireMissionSlice::hitDummy(bool fireDamage) {
    return damageDummy(fireDamage ? "GD_Incendiary.DamageType.DmgType_Incendiary_Impact" : "");
}

bool FireMissionSlice::loadState(const std::string& state) {
    if (!mission_->loadState(state)) return false;
    script_->syncRestored();
    pump();
    return true;
}

// WillowPlayerController.ServerCompleteMission runs as script (CompleteMission, then PlayTurnIn). The real turn-in screen
// offers it only when MissionTracker.CanEndMission holds, so a mission that is not ready is refused here, before the script.
bool FireMissionSlice::turnIn() {
    if (mission_->status() != MissionSystem::Status::ReadyToTurnIn) return false;
    const bool ok = script_->turnIn();
    pump();
    return ok;
}

void FireMissionSlice::tick(double seconds) {
    mission_->tick(seconds);
    dummy_->tick(seconds);
    pump();
}

std::vector<FireMissionSlice::HostEvent> FireMissionSlice::drain() {
    auto result = std::move(events_);
    events_.clear();
    return result;
}

std::vector<std::string> FireMissionSlice::errors() const {
    std::vector<std::string> all = errors_;
    for (const auto& line : mission_->errors) all.push_back(line);
    for (const auto& line : script_->errors) all.push_back(line);
    for (const auto& line : dummy_->errors) all.push_back(line);
    return all;
}

} // namespace vm
