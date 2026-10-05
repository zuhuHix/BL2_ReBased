#pragma once
#include "mission.hpp"
#include "behavior.hpp"
#include "mission_script.hpp"

#include <memory>

namespace vm {

// The first complete data-driven route of the Sanctuary slice, bound end to end without the engine:
// stock mission (MissionSystem) + the target dummy's own behavior provider (BehaviorProvider). The mission's
// Kismet remote events are handed to the host (which owns the Kismet/mover binding). The host supplies the
// world: placing the player in the range, damage and the stock path of its damage type, and the Kismet world ops.
//
// Recovered from the packages (docs/verification/BEHAVIOR_DATA_DECODE.md):
//  - which of the dummy's damage sequences runs: each has a CustomEnableCondition (BehaviorSequenceEnableByMission:
//    LinkedMission, bIsObjectiveSpecific, LinkedObjective, MissionStatesToLinkTo, ObjectiveStatesToLinkTo). FireDamage's
//    names M_RockPaperGenocide_Fire, objective Fire, states {Active}. The evaluation itself is native (the class has no
//    script); the rule used here (mission state in the mission set, and for objective-specific conditions the objective
//    state in the objective set) is UNVERIFIED;
//  - the CompareObject verdict: ObjectA is the OnTakeDamage DamageType output, ObjectB a constant in the provider's
//    variable value block (FireDamage: GD_Incendiary.DamageType.DmgType_Incendiary_Impact).
// Accepting and turning in run the installed script (WillowPlayerController.AcceptMission / ServerCompleteMission, see
// src/mission_script.hpp and docs/verification/NATIVE_MISSION_SCRIPT_BRIDGE.md, UNVERIFIED); the kickoff after acceptance is
// a pending record that the next tick() consumes.
// Still host stand-ins: the damage type of the host's shot, the world ops listed in dummy().boundaryCalls, the XP amount
// the script's GetExperienceReward returns (setExperienceReward).
class FireMissionSlice {
public:
    struct HostEvent {
        enum class Kind { RemoteEvent, Dialog, StatusEffect, MissionWeaponGranted, MissionWeaponRemoved, Reward, Status, ObjectiveSet, ObjectiveComplete };
        Kind kind;
        std::string a, b, c;
    };

    FireMissionSlice(Runtime& runtime, const std::string& missionPath, const std::string& dummyProviderPackage,
                     const std::string& dummyProviderPath);

    MissionSystem& mission() { return *mission_; }
    BehaviorProvider& dummy() { return *dummy_; }
    bool accept(const std::set<std::string>& completedMissions);
    bool enterRange();                       // the GoToRange objective
    // OnSpawned on the dummy: call when the host spawns it (the stock spawn is a population den tied to the Fire
    // objective; not decoded here). With FireDamage enabled it emits RocksPaper_MoveTargetForward.
    bool spawnDummy();
    // OnTakeDamage on the dummy. `damageTypePath` is the stock object path of the damage type the host dealt (e.g.
    // "GD_Incendiary.DamageType.DmgType_Incendiary_Impact"); "" for None. It and `damageSourcePath` are written to the
    // variables the event links as its DamageType / DamageSource outputs.
    bool damageDummy(const std::string& damageTypePath, const std::string& damageSourcePath = "");
    // Legacy convenience for the current host (true = the incendiary impact damage type, false = None). Prefer
    // damageDummy with the stock damage type of the shot.
    bool hitDummy(bool fireDamage);
    bool turnIn();
    // The XP amount the script's GetExperienceReward returns (HOST STAND-IN, the formula is not in src/): set it before
    // turnIn() so the script reaches ExpEarn. expEarned() is what ExpEarn was called with (recorded, no other effect).
    void setExperienceReward(int amount) { script_->setExperienceReward(amount); }
    const std::vector<MissionScript::ExpEarn>& expEarned() const { return script_->expEarned(); }
    // The natives the mission script reached that have no implementation ("name xN") and other VM diagnostics.
    std::vector<std::string> scriptStubs() const { return script_->stubs(); }
    std::vector<std::string> scriptNotes() const { return script_->notes(); }
    // The VM controller's own record of the mission (EMissionStatus number, -1 when it has none) and its bNeedsRewards.
    int scriptPlayerStatus() { return script_->controllerStatus(); }
    bool scriptPlayerNeedsRewards() { return script_->controllerNeedsRewards(); }
    // Persistence: the mission state only (the caller keeps the completed-mission set and rewards). The dummy's
    // sequences follow from the restored mission state through their enable conditions.
    std::string saveState() const { return mission_->saveState(); }
    bool loadState(const std::string& state);
    void tick(double seconds);
    std::vector<HostEvent> drain();
    std::vector<std::string> errors() const;

private:
    Runtime& runtime_;
    std::unique_ptr<MissionSystem> mission_;
    std::unique_ptr<MissionScript> script_;
    std::unique_ptr<BehaviorProvider> dummy_;
    std::vector<HostEvent> events_;
    std::set<std::string> completedMissions_;
    std::string dummyName_;
    std::vector<std::string> errors_;
    void pump();
    bool drainMission();
    bool syncSequences();                    // applies the dummy's enable conditions; true if a sequence changed
    bool conditionHolds(Object& condition);
    void changeSequence(const std::string& sequence, const std::string& action);
};

} // namespace vm
