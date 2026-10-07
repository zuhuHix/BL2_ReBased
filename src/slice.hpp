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
// Experience also runs the script path (docs/verification/NATIVE_PROGRESSION.md, UNVERIFIED): the turn-in script reaches
// GetExperienceReward (formula in src/progression.*) and ExpEarn (raises the VM-side pool), and tick() runs the pool update that
// calls ExpLevelUp. The host supplies what it owns (region stage, the player's level and experience) and applies the Experience
// event to its own state, adopts the Health event (maximum and current health after the level-up) and compares the Level, SkillPoints and MaxHealth events with its own.
// Still host stand-ins: the damage type of the host's shot, the world ops listed in dummy().boundaryCalls.
class FireMissionSlice {
public:
    struct HostEvent {
        enum class Kind { RemoteEvent, Dialog, StatusEffect, MissionWeaponGranted, MissionWeaponRemoved, Reward, Status, ObjectiveSet, ObjectiveComplete,
                          Experience, Level,   // Experience: a = experience the pool gained; Level: a = the level the pool update reached
                          MissionInterface,    // the mission screen was opened (ClientGFxPlayMovie): a = movie definition path, b = the director's path
                          OnUseDialog,         // the on-use dialog's TriggerEvent: a = the global VO_NPC_OnUse_* tag path, b = the speaker (Marcus), c = the other object's class
                          SkillPoints,         // a = the unspent skill points the script's level-up awarded (the data's per-level formula; sent with a Level event, 0 included)
                          MaxHealth,           // a = the new maximum health (the VM health pool's effective maximum, rebased by RecalculateAttributeInitializedState)
                          Health };            // a = current health, b = maximum health after the level-up (the OnLevelUp refill included; sent with MaxHealth)
        Kind kind;
        std::string a, b, c;
        std::string detail;       // Dialog: "act=...;ak=...;talker=echo|pawn;outcome=...;line=<id>" (see MissionSystem::Effect)
    };

    FireMissionSlice(Runtime& runtime, const std::string& missionPath, const std::string& dummyProviderPackage,
                     const std::string& dummyProviderPath);

    MissionSystem& mission() { return *mission_; }
    BehaviorProvider& dummy() { return *dummy_; }
    bool accept(const std::set<std::string>& completedMissions);
    // The use key's press on Marcus (after the host's use ray chose him): his stock OnUsed chain runs on the VM (MissionScript::useMarcus). The
    // result says whether it reached the mission interface (a MissionInterface host event is queued) and lists the behaviors it ran.
    MissionScript::MarcusUse useMarcus(const std::set<std::string>& completedMissions);
    // The mission screen's three lists (Marcus's own script over his directive table and the tracker's availability queries): the host's use key
    // confirms the entry offered here (a host stand-in for the button press) with accept / turnIn.
    MissionScript::MissionLists screen(const std::set<std::string>& completedMissions);
    bool enterRange();                       // the GoToRange objective: the player enters the waypoint's cylinder (touchWaypoint)
    // The host's overlap of the waypoint's cylinder with the player pawn (or Marcus): begin / end. The stock WillowWaypoint script runs on
    // the VM: Touch for a player-owned pawn updates GoToRange when it is updatable, and the set-changed reaction re-checks the actors
    // already inside (NATIVE_OBJECTIVE_TRIGGERS.md, UNVERIFIED).
    void touchWaypoint(bool player, bool begin);
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
    // Inputs the host owns (see MissionScript): the region's game stage (set before accept, it is locked when the mission becomes
    // Active) and the player's level and experience (set before turnIn and whenever the host changes them).
    void setRegionGameStage(int stage) { script_->setRegionGameStage(stage); }
    void setPlayerExperience(int level, int64_t experience) { script_->setPlayerExperience(level, experience); }
    // The host's current health (call after setPlayerExperience): the VM health pool is rebuilt with it, so the level-up's rebase and refill start from it.
    void setPlayerHealth(float current) { script_->setPlayerHealth(current); }
    // GlobalsDefinition.PlayerInteractionDistance of the installed data (350 uu): how far the use ray reaches (NATIVE_USE_INTERACTION.md).
    float playerInteractionDistance() { return script_->playerInteractionDistance(); }
    int scriptObjectiveUpdates() const { return script_->objectiveUpdates(); }   // applied objective updates the VM controller was told about
    int scriptPlayerLevel() { return script_->playerLevel(); }
    float scriptExperiencePool() const { return script_->experiencePool(); }
    // expEarned() is what ExpEarn was called with.
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
    void drainExperience();
    bool drainMission();
    // The dummy's enable conditions (NATIVE_BEHAVIOR_POPULATION.md section B, UNVERIFIED): the verdict of one condition from the
    // tracker's current state, and the application of every condition's verdict (what each mission notification does).
    bool conditionHolds(Object& condition);
    void applyConditions();
    bool dummyRegistered_ = false;           // the dummy has been spawned: its provider is a consumer and its conditions observe the mission
    int notifyDepth_ = 0;
    void changeSequence(const std::string& sequence, const std::string& action);
};

} // namespace vm
