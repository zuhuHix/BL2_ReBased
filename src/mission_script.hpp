#pragma once
#include "combat.hpp"
#include "mission.hpp"
#include "progression.hpp"
#include "vm.hpp"

#include <functional>
#include <map>
#include <memory>
#include <set>
#include <string>
#include <vector>

namespace vm {

class BehaviorProvider;

// The script half of accepting and turning in a mission (docs/verification/NATIVE_MISSION_SCRIPT_BRIDGE.md).
//
// In this build the controller functions AcceptMission, ServerCompleteMission, UpdateMissionStatus and
// ServerGrantMissionRewards are script; they call a few natives on the MissionTracker. This class builds the small
// object graph that script reads (a local controller with authority, WorldInfo.GRI as a WillowGameReplicationInfo
// whose MissionTracker is a VM tracker object, the installed MissionDefinition) and binds those natives, scoped to this
// object like src/mover.cpp, onto the existing MissionSystem, which stays the single owner of the mission's state.
//
// Natives implemented here (all UNVERIFIED, read from native code in the bridge note and NATIVE_PROGRESSION.md):
// MissionTracker.ActivateMission, CompleteMission, PlayTurnIn, GetMissionStatus, CanStartMission, CanEndMission, GetCompletedBranch, IsDataValid;
// WillowPlayerController.NativeGetMissionIndex, ExpEarn, GetMaxExpLevel, GetExpPointsRequiredForLevel;
// MissionDefinition.GetExperienceReward, GetGameStage, GetCurrencyRewardType, GetCurrencyReward, ShouldGrantAlternateReward,
// GetItemRewardsForPlayer (empty rewards only). The experience pool and its level-up (ApplyExpPointsToExpLevel, run from
// updateExperiencePool) live here as C++ state with the VM controller's PlayerReplicationInfo as the level's home.
// Swap 7 (NATIVE_PROGRESSION.md, NATIVE_SKILLS.md section 1, NATIVE_ATTRIBUTES.md): AttributeInitializationDefinition.EvaluateInitializationData
// (the evaluator of src/progression.*, with the player controller as the context source), so the installed ExpLevelUp awards its skill points
// from GlobalsDefinition.GeneralSkillPointsPerLevelUp.
// Swap 7b (NATIVE_LEVEL_UP_ATTRIBUTES.md, UNVERIFIED; the refill was observed in two live captures): the VM controller owns a ResourcePoolManager
// with one health ResourcePool built from Maya's stock HealthPoolDefinition. WillowPlayerController.RecalculateAttributeInitializedState rebases every
// pool of the manager through the installed script ResourcePool.CalculateBaseValues(true) (the natives it needs: ResourcePoolManager.RecalculateBaseValues,
// ResourcePool.GetMaxValue, SetCurrentValue, UpdateLastValues, PoolIsNowFull), so the base maximum comes from the data, the stack is kept and current
// follows the new maximum only for a pool that was full. OnExpLevelChange's pawn branch runs in the VM (its 1 s guard is the script's): the stock
// OnLevelUp collection reaches BehaviorBase.RunBehaviors, a STAND-IN here that applies the Behavior_AttributeEffect skills' effects by their data to
// the pools the VM has (the heal; see runBehaviors). The class-level damage modifiers and status resistances, the shield and cooldown pools are not modelled.
// Swap 9 (lane I7): a natural level-up (ExpLevelUp(false): OnExpLevelChange's bNaturalLevelup) also reaches RunBehaviors with PlayerClass.OnLevelUpNaturally; its
// skill's timed Scale effect on the held weapon's damage is not a pool of the VM, so it is handed to the host as a WeaponDamageScale gain (scale and duration
// read from the skill's data), which adds it to the weapon's damage scale sum for that long.
// Swap 10 (lane I7): the first OnLevelUp skill's second effect, Scale of the action-skill cooldown's current value by the pool's minimum (0), targets a pool whose
// current value the host keeps: it is handed to the host as an ActionSkillCooldown gain (modifier type and value from the data), natural or not (the script's rule).
// Swap 3 (NATIVE_CONTROLLER_HELPERS.md): MissionTracker.IsDataValid (the bDataValidated flag) and ValidateData;
// WillowPlayerController.GetCurrentPlaythrough, GetHUDMovie, UpdateLcdMissionStatus and PlayUIAkEvent (presentation: no-ops),
// PlayerController.IsPrimaryPlayer, WorldInfo.IsMenuLevel, GetWillowGlobals / GetGearboxGlobals / GetBehaviorKernel /
// GetGlobalsDefinition (one VM globals object).
// Every other native the script reaches stays a logged stub, listed by stubs().
class MissionScript {
public:
    struct ExpEarn { int amount = 0; int source = 0; int type = -1; };   // type -1: the optional argument was omitted

    MissionScript(Runtime& runtime, MissionSystem& mission);
    ~MissionScript();

    // AcceptMission(Mission, MissionDirector = None) on the controller; `completed` is what MissionDependenciesMet sees.
    // Returns true when the mission became Active through it.
    bool accept(const std::set<std::string>& completed);
    // ServerCompleteMission(Mission, None). Returns true when the mission became Complete through it.
    bool turnIn();
    // After MissionSystem::loadState: the controller's own record of the mission follows the restored status, as the
    // level-load replay would leave it (the script's old-status check needs the record).
    void syncRestored();

    // Inputs the host owns. The region's game stage (what WillowRegionDefinition.GetRegionGameStage answers now: fixed per
    // player and playthrough by the host); the Active status locks it into the mission, as the status routine does.
    void setRegionGameStage(int stage) { regionStage_ = stage; }
    // The player's experience level and experience pool value (the host's Maya), read before the pool is updated.
    void setPlayerExperience(int level, int64_t experience);
    // The player's current health, the host's: the VM health pool is rebuilt for the player's level with this current value (a full pool follows the
    // next level-up's maximum, a partly empty one does not). Call after setPlayerExperience. Without it the pool starts full.
    void setPlayerHealth(float current);
    // Game time: WorldInfo.TimeSeconds, which OnExpLevelChange's 1 s guard reads. The slice's tick advances it.
    void advanceTime(double seconds);
    // ApplyExpPointsToExpLevel, called from the pool's per-frame update: while the pool has reached the next level's
    // requirement and the level is below the cap, the script ExpLevelUp(bCheated = false) runs on the controller.
    void updateExperiencePool();
    // One ExpLevelUp(bCheated = true) on the controller: the not-natural level-up (the console cheat's script argument). Nothing in the slice calls it; it is
    // here so that the natural/not-natural rule of OnExpLevelChange can be checked end to end (--slice-run step `cheatlevel`). Reports like the pool update.
    void cheatLevelUp();
    // What the script's level-up path produced, in order, for the host: what ExpEarn added to the pool (Experience), the level the pool
    // update raised the player to (Level), the unspent skill points that level-up awarded (SkillPoints: GeneralSkillPoints after minus before,
    // sent with a level change even when 0), the new maximum health (MaxHealth: the health pool's effective maximum, rebased by
    // RecalculateAttributeInitializedState) and the health after the level-up (Health: current and maximum, the OnLevelUp refill included). The host
    // applies the experience and the health and compares the rest with its own numbers. A natural level-up also hands over the weapon damage scale its
    // OnLevelUpNaturally skill adds (WeaponDamageScale, after Health): the host holds the weapon, so the host applies it.
    struct Gain {
        enum class Kind { Experience, Level, SkillPoints, MaxHealth, Health, WeaponDamageScale, ActionSkillCooldown } kind = Kind::Experience;
        int amount = 0;          // Experience: the amount, Level: the level, SkillPoints: the count, ActionSkillCooldown: the modifier type (0 Scale, 1 PreAdd, 2 PostAdd)
        float health = 0;        // MaxHealth: the maximum, Health: the current value
        float maximum = 0;       // Health: the maximum
        float scale = 0;         // WeaponDamageScale: the value added to the weapon's damage scale sum; ActionSkillCooldown: the modifier's value
        float duration = 0;      // WeaponDamageScale: seconds of game time the skill lasts
    };
    std::vector<Gain> takeGains() { auto result = std::move(gains_); gains_.clear(); return result; }
    const std::vector<ExpEarn>& expEarned() const { return expEarned_; }
    float experiencePool() const { return pool_; }
    int playerLevel();

    // The stock waypoint actor of the GoToRange objective (NATIVE_OBJECTIVE_TRIGGERS.md, UNVERIFIED): the placed WillowWaypoint of `package`
    // at `path` runs its own script on the VM (PostBeginPlay at placement, Touch, the observer reaction over its Touching list). The host
    // owns the overlap test and reports it: touch / untouch of the player pawn or of Marcus. False when the package has no such actor.
    enum class Toucher { Player, Marcus };
    bool placeWaypoint(const std::shared_ptr<const Package>& package, const std::string& path);
    bool hasWaypoint() const { return waypoint_ != nullptr; }
    int objectiveUpdates() const { return objectiveUpdates_; }
    // GlobalsDefinition.PlayerInteractionDistance (0 when the data has no globals definition).
    float playerInteractionDistance();
    void touch(Toucher who);
    void untouch(Toucher who);
    // The mission screen's lists (NATIVE_USE_INTERACTION.md, "Marcus's chain (data) and the lists (script)", UNVERIFIED): a VM Marcus built from
    // the stock archetype `path` of `package` (a WillowAIPawn whose MissionDirectivesDefinition table is the data's; authority, the VM world)
    // stands for the one MissionDirector. Its own script builds the eligible / in-progress / redeemable lists over that table and the tracker's
    // CanStartMission / CanEndMission / GetMissionStatus / GetCompletedBranch, which read the MissionSystem; the screen's movie itself is not
    // hosted. AcceptMission / ServerCompleteMission then get Marcus as their director, so OnPlayerAcceptedMission / OnPlayerTurnedInMission
    // run as script. False when the package has no such pawn (the director stays None).
    bool placeMarcus(const std::shared_ptr<const Package>& package, const std::string& path);
    bool hasMarcus() const { return director_ != nullptr; }
    // The use key's press on Marcus, after the host's use ray chose him (NATIVE_MARCUS_USE_CHAIN.md, UNVERIFIED): AIDefinition.OnUsed raises the
    // event "OnUsed" with link filter 2 (Generic) on his AI-definition provider (GD_Marcus.Character.AIDef_Marcus.AIBehaviorProviderDefinition_0), the
    // payload being the instigator (the player's VM pawn) and the used component (None). His chain of script behaviors then runs on the VM through the
    // BehaviorProvider kernel: the nine Behavior_IsSequenceEnabled checks (each "enabled" fires its remote event and stops the cascade), then the
    // mission-context dialog, Behavior_HasMissions and Behavior_ShowMissionInterface, which ends in the controller's ClientGFxPlayMovie. That call
    // is presentation and is not run: it is reported as `interfaceOpened` (the movie definition and the director) for the host to answer.
    // The two later raises of UseObject (filters 0 and 1) reach no link of Marcus's provider and are not made.
    struct MarcusUse {
        bool interfaceOpened = false;
        std::string movie, director;           // object paths
        std::vector<std::string> cascade;      // every script behavior of the run: "name(sequence) -> selected output ids"
        // The on-use dialog (Behavior_PlayAIMissionContextDialog -> WillowAIPawn.PlayOnUseDialog, NATIVE_BEHAVIOR_CONTEXT.md): the tag it passes to the
        // pawn's GearboxDialogComponent.TriggerEvent (the global VO_NPC_OnUse_* tag), the speaker (the component's owner, Marcus) and the other object
        // (the player pawn's class). Empty when the dialog stayed silent. Playing the line needs the two-step group dispatch and is not done.
        std::string onUseTag, onUseSpeaker, onUseTarget;
        // The speaker's list of dialog groups (WillowPawn.GetDialogGroups, NATIVE_DIALOG_GROUPS.md, UNVERIFIED): its size and its first and last entries.
        size_t onUseGroupCount = 0;
        std::string onUseFirstGroup, onUseLastGroup;
    };
    // `completed` is what MissionDependenciesMet sees (as for accept()).
    MarcusUse useMarcus(const std::set<std::string>& completed);
    // Marcus's mission screen after its open (swap 11, FIRE_MISSION_GOLDEN_TRACE.md "The mission screen's close and the reward page"). The movie is not hosted: a
    // VM stand-in object of the installed class QuestAcceptGFxMovie answers WillowGFxUIManager.GetPlayingMovie while the screen is open, so the controller's
    // own script (ClientUpdateUIMissionList, ClientSpawnMissionRewardUI) finds a movie and the movie's own entry-list script (DetermineQuestEntries,
    // UpdateMissionList, GotoCorrectPartOfMovie) decides what happens. What the host answers is the movie's native Close (the screen's end: the script of its OnClose,
    // which ends the use of Marcus and tells him the screen closed) and DisplayRewardsPage (presentation: the page waits for the player).
    //   Accept with the screen open: the entry list is empty afterwards, so the movie closes inside UpdateMissionStatus(Active), as observed.
    //   Turn-in with the screen open: ClientSpawnMissionRewardUI shows the reward page and MissionRewardsReceived waits (rewardPending()); without a
    //   screen the script takes the shortcut and MissionRewardsReceived runs inside the turn-in.
    // closeScreen is the player leaving the screen (Escape); confirmReward is the player confirming the reward page (Enter): MissionRewardsReceived. Both
    // return false when there is nothing to do.
    bool screenOpen() const { return screenOpen_; }
    bool rewardPending() const { return !rewardMission_.empty(); }
    bool closeScreen();
    bool confirmReward();
    struct MissionLists { std::vector<std::string> eligible, inProgress, redeemable; };   // mission paths, the script's own order
    MissionLists missionLists(const std::set<std::string>& completed);
    // Runs a script behavior's ApplyBehaviorToContext on the VM (e.g. Behavior_UpdateMissionObjective: the world's tracker, UpdateObjective).
    void applyBehavior(ObjectPtr behavior, ObjectPtr context);
    // The tracker's observer notifications reach the VM observers (a registered waypoint) as their MissionReaction* script events.
    void notify(MissionSystem::Notification kind);

    // The pawn health and hit chain (src/combat.*, src/combat_hit.cpp): the CombatSystem this bridge owns, the game stage the mission is played at (the
    // locked one once Active, else the region's; 0 when the host set none) and a hit by the player on `victim`. The hit runs the victim's installed script
    // TakeDamage with the VM controller as the instigator (its pawn linked for the call only, as the level-up does) under the bridge's script guard.
    CombatSystem& combat() { return *combat_; }
    // The VM controller (the player, the context source of the evaluator and the owner of the experience pool) and its pawn (made on first use, linked to the health pool):
    // for the combat run, which reads the player's level, health and experience and lists the controller as the owner of its replication info.
    ObjectPtr playerController() const { return controller_; }
    ObjectPtr playerPawnObject() { return pawnFor(Toucher::Player); }
    int currentGameStage() const { return gameStage(); }
    CombatSystem::HitReport hitFromPlayer(const ObjectPtr& victim, double damage, const std::string& damageTypePath, const std::string& sourceClass);

    // The controller's record of the mission (MissionPlaythroughs[0].MissionList[...].Status), -1 when it has none.
    int controllerStatus();
    bool controllerNeedsRewards();

    // Natives without an implementation that the script reached, "Class.Name(types) xN", and other VM diagnostics.
    std::vector<std::string> stubs() const;
    std::vector<std::string> notes() const { return notes_; }
    std::vector<std::string> errors;     // a script run that threw: the state is left as the native code made it

private:
    Runtime& runtime_;
    MissionSystem& mission_;
    ObjectPtr controller_, tracker_, world_, replication_, globals_, globalsDefinition_;
    ObjectPtr waypoint_, playerPawn_, marcusPawn_, director_;
    std::unique_ptr<BehaviorProvider> marcusProvider_;      // Marcus's AI-definition provider, registered on his consumer (PID marcusPid_)
    static constexpr int marcusPid_ = 0;                   // the first handle the consumer registration gives out (UNVERIFIED: the allocator's base); -1 is invalid
    ObjectPtr dialogComponent_, dialogGlobals_;            // his GearboxDialogComponent (archetype) and GD_Globals.Dialog.DialogGlobals
    std::vector<int>* selected_ = nullptr;                 // the running script behavior's recorded output ids
    MarcusUse use_;
    bool screenOpen_ = false;                              // Marcus's mission screen is open (its movie is not hosted): BeginUse ran; closeMovie ends it
    ObjectPtr uiManager_, screenMovie_;                    // the controller's WillowGFxUIManager and the stand-in QuestAcceptGFxMovie (null: this package set has none)
    bool closing_ = false;                                 // the screen's OnClose is running: a TriggerEvent on Marcus's component is the dismissal dialog
    std::string rewardMission_;                            // the mission whose reward page is shown and waits for the player ("" = none)
    void bindScreen();
    void openScreen();
    void closeMovie();
    bool turningIn_ = false;                               // ServerCompleteMission is running: a TriggerEvent on Marcus's component is the turned-in dialog
    void bindUse();
    BehaviorProvider* registeredProvider(const Value& handle, const Value& provider);
    int objectiveUpdates_ = 0;
    std::vector<ObjectPtr> observers_;                 // IMission observers registered with the tracker (the waypoint)
    std::set<std::string> completed_;
    std::map<std::string, size_t> stubs_;
    std::vector<std::string> notes_;
    std::vector<ExpEarn> expEarned_;
    std::vector<Gain> gains_;
    std::vector<Gain> levelUpEffects_;     // what the level-up's skills hand to the host (WeaponDamageScale), appended to gains_ after the level's own events
    AttributeEvaluator evaluator_;
    std::unique_ptr<CombatSystem> combat_;  // pool references and natives, the pawn health natives, EvaluateInitializationData (src/combat.*)
    std::unique_ptr<ExperienceCurve> curve_;
    ObjectPtr pri_;
    float pool_ = 0;                       // the experience resource pool's CurrentValue
    ObjectPtr playerClass_;                // the player's class definition (the host's Maya): the health pool's data and the OnLevelUp behaviors
    ObjectPtr cooldownPool_;               // the action-skill cooldown pool, minimum only (cooldownPool()); not in the manager
    bool cooldownPoolFailed_ = false;
    ObjectPtr manager_, healthPool_;       // the controller's ResourcePoolManager and its one pool (built on first use from the class's HealthPoolDefinition)
    bool healthPoolFailed_ = false;        // the data for it is not in the packages: no health events
    bool inLevelUp_ = false;               // the pool update is running ExpLevelUp: RunBehaviors is the level-up stand-in
    int64_t pawnGameStage_ = 0;            // what OnExpLevelChange told the pawn (WillowPawn.SetGameStage; recorded only)
    int regionStage_ = 0, lockedStage_ = 0, playThroughCount_ = 1, maxLevel_ = 50;
    unsigned depth_ = 0;

    void bind(const char* path, NativeFn fn);
    Value outsideBinding(NativeCall& call);
    void run(const std::function<void()>& script);     // outermost script run: collects stubs, catches VM errors
    void updateMissionStatus(int nativeStatus);
    Value* missionList();
    int currentPlaythrough();
    ObjectPtr pawnFor(Toucher who);
    ObjectPtr globalsDefinition();
    Value objectiveValue(const std::string& objectivePath);
    void objectiveUpdated(const std::string& objectivePath, int bit);
    ExperienceCurve& curve();
    int gameStage() const { return lockedStage_ ? lockedStage_ : regionStage_; }
    int experienceReward(bool alternate);
    void expEarn(int amount, int source, int type);
    AttributeContext attributeContext(const Value& source);   // what EvaluateInitializationData reads for this context source
    ObjectPtr playerClass();
    bool ensureHealthPool();
    void linkPlayerHealth();                   // the player pawn's HealthPool reference to the pool (once both exist)
    void seedHealthPool(float current);        // current < 0: full
    ObjectPtr poolFor(const Value& resource);
    void clampPool(Object& pool);
    void rebasePools(const ObjectPtr& manager);
    void runBehaviors(const Value& behaviors, int depth);
    void applySkillEffects(const ObjectPtr& skill);
    float effectAmount(const Value& effect, int grade);       // the effect's modifier value at the grade (NATIVE_SKILLS.md section 4.3)
    void weaponEffect(const ObjectPtr& skill, const Value& effect, const std::string& property, int grade);
    void runLevelUps(bool cheated);
    ObjectPtr cooldownPool();
    void cooldownEffect(const Value& effect, const std::string& property, float amount);
    void bindOptional(const char* path, NativeFn fn);
    const Value& rewardData(bool alternate);
    ObjectPtr loadRef(const Value& reference);
    void notImplemented(const std::string& what);   // a labelled not-implemented path: listed with the stubs
};

} // namespace vm
