#pragma once
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
    // ApplyExpPointsToExpLevel, called from the pool's per-frame update: while the pool has reached the next level's
    // requirement and the level is below the cap, the script ExpLevelUp(bCheated = false) runs on the controller.
    void updateExperiencePool();
    // What ExpEarn added to the pool (amount > 0) and the level the pool update raised the player to (level > 0), in order.
    // The host applies them to its own experience state and level display.
    struct Gain { int amount = 0; int level = 0; };
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
    struct MissionLists { std::vector<std::string> eligible, inProgress, redeemable; };   // mission paths, the script's own order
    MissionLists missionLists(const std::set<std::string>& completed);
    // Runs a script behavior's ApplyBehaviorToContext on the VM (e.g. Behavior_UpdateMissionObjective: the world's tracker, UpdateObjective).
    void applyBehavior(ObjectPtr behavior, ObjectPtr context);
    // The tracker's observer notifications reach the VM observers (a registered waypoint) as their MissionReaction* script events.
    void notify(MissionSystem::Notification kind);

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
    void bindUse();
    BehaviorProvider* registeredProvider(const Value& handle, const Value& provider);
    int objectiveUpdates_ = 0;
    std::vector<ObjectPtr> observers_;                 // IMission observers registered with the tracker (the waypoint)
    std::set<std::string> completed_;
    std::map<std::string, size_t> stubs_;
    std::vector<std::string> notes_;
    std::vector<ExpEarn> expEarned_;
    std::vector<Gain> gains_;
    AttributeEvaluator evaluator_;
    std::unique_ptr<ExperienceCurve> curve_;
    ObjectPtr pri_;
    float pool_ = 0;                       // the experience resource pool's CurrentValue
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
    const Value& rewardData(bool alternate);
    ObjectPtr loadRef(const Value& reference);
    void notImplemented(const std::string& what);   // a labelled not-implemented path: listed with the stubs
};

} // namespace vm
