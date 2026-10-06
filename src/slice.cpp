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
    // The tracker's observers in registration order: the VM's own (the waypoint, registered at level start) first, then the dummy's conditions.
    mission_->onNotification = [this](MissionSystem::Notification kind) { script_->notify(kind); applyConditions(); };
    script_ = std::make_unique<MissionScript>(runtime, *mission_);
    auto package = runtime.package(dummyProviderPackage);
    const int32_t index = runtime.findExport(*package, dummyProviderPath);
    if (index <= 0) throw RuntimeError("dummy behavior provider not found: " + dummyProviderPath);
    dummy_ = std::make_unique<BehaviorProvider>(runtime, package, index);
    // The stock GoToRange waypoint (placed in the same level package), if the package has it.
    script_->placeWaypoint(package, "TheWorld.PersistentLevel.WillowWaypoint_9");
    // Marcus, the mission director: the stock archetype with his own directive table (NATIVE_USE_INTERACTION.md); the placed pawn is an instance of it.
    script_->placeMarcus(package, "GD_Marcus.Character.Pawn_Marcus");
    dummyName_ = dummyProviderPath.substr(dummyProviderPath.rfind('.') + 1);
    auto& d = *dummy_;
    // Behavior_UpdateMissionObjective is script (NATIVE_OBJECTIVE_TRIGGERS.md): it runs on the VM and reaches the tracker's UpdateObjective.
    // Its context object would be the dummy pawn, which has no VM object here; None casts to no IMissionObjective, as a pawn does, so the
    // bit is 0 either way.
    d.handle("WillowGame.Behavior_UpdateMissionObjective", [this](BehaviorProvider&, BehaviorProvider::Behavior& b, const std::string&) {
        script_->applyBehavior(b.object, nullptr);
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
    // Reached since the provider registers like the stock one (the bEnabledOnSpawn sequences fire OnBehaviorSequenceEnabled, section C of
    // NATIVE_BEHAVIOR_POPULATION.md): a special-move (animation) request on the pawn; no animation system here, listed at the boundary.
    d.reportAtBoundary("GearboxFramework.Behavior_SpecialMove");
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
        if (components && providerPathLeaf(components) == dummyName_ && sequence)
            changeSequence(sequence->s, nameOf(actions, p.runtime().property(*b.object, "Action")));
        else p.errors.push_back("sequence change targets another provider: " + providerPathLeaf(components));
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

// BehaviorSequenceEnableByMission (NATIVE_BEHAVIOR_POPULATION.md section B, read from native code: UNVERIFIED). Data: LinkedMission,
// MissionStatesToLinkTo (bNotStarted, bActive, bRequiredObjectivesComplete, bReadyToTurnIn, bComplete, bFailed), bIsObjectiveSpecific,
// LinkedObjective, ObjectiveStatesToLinkTo (bNotStarted, bActive, bComplete), ObjectiveSetRestrictions. The verdict: a mission that
// is not objective-specific looks up its status bit; an objective-specific one has no linked objective -> false, else classifies the
// objective (MissionSystem::objectiveState: Complete / Active / NotStarted, with the status gating) and looks up that bit; then a
// non-empty ObjectiveSetRestrictions keeps a true verdict only while one listed set is the active set. A mission other than the
// slice's counts as Complete when it is in the completed set, else NotStarted, and its objectives as NotStarted. RequiredObjectivesComplete
// and Failed are not modelled by MissionSystem. Not modelled: per-instance objectives (bRememberItemsWithinObjective consumers).
bool FireMissionSlice::conditionHolds(Object& condition) {
    Runtime& r = runtime_;
    const std::string mission = refPath(r.property(condition, "LinkedMission"));
    const bool own = mission == mission_->path();
    static const char* statusFields[] = {"bNotStarted", "bActive", "bReadyToTurnIn", "bComplete"};
    bool verdict;
    const Value* specific = r.property(condition, "bIsObjectiveSpecific");
    if (!specific || !specific->truth()) {
        const std::string missionState = own ? statusFields[int(mission_->status())]
                                             : (completedMissions_.count(mission) ? "bComplete" : "bNotStarted");
        const Value* missionStates = r.property(condition, "MissionStatesToLinkTo");
        const Value* match = missionStates ? missionStates->field(missionState) : nullptr;
        verdict = match && match->truth();
    } else {
        const std::string objective = refPath(r.property(condition, "LinkedObjective"));
        if (objective.empty()) return false;
        const std::string state = own ? mission_->objectiveState(objective) : "NotStarted";
        if (state.empty()) { errors_.push_back("enable condition names an objective outside its mission: " + objective); return false; }
        const Value* objectiveStates = r.property(condition, "ObjectiveStatesToLinkTo");
        const Value* match = objectiveStates ? objectiveStates->field("b" + state) : nullptr;
        verdict = match && match->truth();
    }
    if (verdict)
        if (const Value* restrictions = r.property(condition, "ObjectiveSetRestrictions");
            restrictions && restrictions->kind == Value::Kind::Array && !restrictions->elements().empty()) {
            verdict = false;
            if (own)
                for (const auto& set : restrictions->elements())
                    if (mission_->setActive(refPath(&set))) { verdict = true; break; }
        }
    return verdict;
}

// What every mission notification does to the registered dummy: recompute each condition's verdict from the tracker's current state
// and apply it to its sequence (the notification's own arguments are ignored). setSequenceEnabled changes nothing, and fires no
// event, when the sequence already is in that state, so events come only from real transitions.
void FireMissionSlice::applyConditions() {
    if (!dummyRegistered_) return;
    if (++notifyDepth_ > 16) { errors_.push_back("mission notifications did not settle"); --notifyDepth_; return; }
    for (const auto& name : dummy_->sequenceNames()) {
        auto condition = dummy_->enableCondition(name);
        if (!condition) continue;
        if (condition->cls->path != "WillowGame.BehaviorSequenceEnableByMission") {
            const std::string line = "unsupported enable condition class " + condition->cls->path;
            if (std::find(errors_.begin(), errors_.end(), line) == errors_.end()) errors_.push_back(line);
            continue;
        }
        dummy_->setSequenceEnabled(name, conditionHolds(*condition));
    }
    --notifyDepth_;
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
        case K::Dialog: events_.push_back({HostEvent::Kind::Dialog, effect.a, effect.b, effect.c, effect.detail}); break;
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

// What the script's experience natives did since the last call, as host events: ExpEarn's pool gains and the levels the pool update reached.
void FireMissionSlice::drainExperience() {
    for (const auto& gain : script_->takeGains())
        events_.push_back(gain.level > 0 ? HostEvent{HostEvent::Kind::Level, std::to_string(gain.level), "", "", ""}
                                         : HostEvent{HostEvent::Kind::Experience, std::to_string(gain.amount), "", "", ""});
}

void FireMissionSlice::pump() {
    drainExperience();
    // Dummy sequence changes (made by the mission's notifications) can fire behaviors that change mission state again: repeat
    // until the effects are quiet (bounded).
    for (int round = 0; round < 16; ++round)
        if (!drainMission()) return;
    errors_.push_back("mission/dummy state did not settle");
}

// What Marcus's mission screen offers, from his own list scripts (NATIVE_USE_INTERACTION.md); the screen itself is not hosted.
MissionScript::MissionLists FireMissionSlice::screen(const std::set<std::string>& completed) {
    completedMissions_ = completed;
    return script_->missionLists(completed);
}

// The confirm button on an entry (bridge note, "Calls into the C1 natives"): a thin wrapper kept for the CLI and the tests. When Marcus is
// placed, the entry must be one his screen offers (the button only exists for an offered entry); WillowPlayerController.AcceptMission then
// runs as script with Marcus as the director, and its native ActivateMission drives MissionSystem. The kickoff is the pending record that the
// next tick() plays (bridge note); the mission's first objective set follows from it.
bool FireMissionSlice::accept(const std::set<std::string>& completed) {
    completedMissions_ = completed;
    if (script_->hasMarcus()) {
        const auto lists = script_->missionLists(completed);
        if (std::find(lists.eligible.begin(), lists.eligible.end(), mission_->path()) == lists.eligible.end()) return false;
    }
    const bool ok = script_->accept(completed);
    pump();
    return ok;
}

// The host reports the overlap of an actor with the waypoint's cylinder; the waypoint's own script decides what it means.
void FireMissionSlice::touchWaypoint(bool player, bool begin) {
    using W = MissionScript::Toucher;
    if (begin) script_->touch(player ? W::Player : W::Marcus); else script_->untouch(player ? W::Player : W::Marcus);
    pump();
}

// The old direct call, kept as a thin wrapper: the player enters the cylinder. Without the stock waypoint (data without it) the objective
// is updated directly as before.
bool FireMissionSlice::enterRange() {
    const auto before = mission_->objectiveProgress("RockPaper_GoToRange");
    if (script_->hasWaypoint()) touchWaypoint(true, true);
    else { mission_->updateObjective("RockPaper_GoToRange"); pump(); }
    return mission_->objectiveProgress("RockPaper_GoToRange") != before;
}

bool FireMissionSlice::damageDummy(const std::string& damageTypePath, const std::string& damageSourcePath) {
    dummy_->fireEvent("OnTakeDamage", {{"DamageType", damageTypePath}, {"DamageSource", damageSourcePath}});
    pump();
    return true;
}

// The stock spawn registers the dummy's provider on its pawn before OnSpawned (IntializeBehaviorProviderForConsumer: the
// bEnabledOnSpawn sequences are enabled, then each condition observes the mission and delivers its verdict at once), so
// FireDamage is already enabled when OnSpawned arrives. The provider is not a consumer, and observes nothing, before that.
bool FireMissionSlice::spawnDummy() {
    if (!dummyRegistered_) {
        dummy_->registerConsumer();
        dummyRegistered_ = true;
        notifyDepth_ = 0;
        applyConditions();                    // the LevelLoad verdict
    }
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
    applyConditions();                        // a loaded mission is announced to its observers (LevelLoad)
    pump();
    return true;
}

// WillowPlayerController.ServerCompleteMission runs as script (CompleteMission, then PlayTurnIn) with Marcus as the director. The real turn-in
// screen offers it only for a redeemable entry (CanEndMission holds), so a mission his lists do not offer is refused here, before the script.
bool FireMissionSlice::turnIn() {
    if (script_->hasMarcus()) {
        const auto lists = script_->missionLists(completedMissions_);
        if (std::find(lists.redeemable.begin(), lists.redeemable.end(), mission_->path()) == lists.redeemable.end()) return false;
    } else if (!mission_->canEnd()) return false;
    const bool ok = script_->turnIn();
    pump();
    return ok;
}

void FireMissionSlice::tick(double seconds) {
    mission_->tick(seconds);
    script_->updateExperiencePool();       // the experience pool update, every frame (ApplyExpPointsToExpLevel)
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
