#pragma once
#include "CoreMinimal.h"
#include "Components/ActorComponent.h"
#include "OpenWillowInventory.h"
#include "OpenWillowQuest.generated.h"

class FJsonObject;

// Host binding of the Sanctuary slice mission (-owquest). Runs the installed "Rock, Paper, Genocide: Fire Weapons!"
// mission and the target dummy's own behavior provider through vm::FireMissionSlice, and the map's installed Kismet
// sequence through the door's UOpenWillowMover (one sequence instance). World data comes from the ignored manifests
// (-owslice=world.json, -ownpcs=npc_assets.json, -owaudio=audio.json; see FOpenWillowSliceData):
//  - Marcus is a placed NPC at his stock pose; the use key accepts / turns in the mission near him; the installed
//    WillowSeqAct_AIScripted walk runs on the recovered move nodes and each arrival enters the sequence's
//    ArrivedAtMoveNode event (which opens/closes the door through the installed links);
//  - the GoToRange objective completes when the player's capsule touches the stock waypoint cylinder;
//  - the stock dummy pawn spawns at its population point when the Fire objective becomes active, the sequence's
//    populated events attach it to the target carrier, and the dummy provider's own remote events drive the
//    target Matinee;
//  - Maya's health follows the recovered formula and respawn follows the decoded station selection;
//  - mission dialog is looked up in the audio manifest and logged (nothing is decoded or played);
//  - the mission weapon (MissionWeapon, a MissionWeaponBalanceDefinition) is the matching recipe under -owitems=<dir>
//    (tools/weapon_slice_gear.py), lent to Maya with the imported Maliwan mesh; her shots hand the held item's stock
//    damage type path to the dummy's OnTakeDamage;
//  - turn-in adds trunc(percentage x curve span) at the mission level to Maya's experience; the mission level is
//    Sanctuary's region game stage, fixed from her level the first time it is asked for (session start) and kept
//    in the save (rules read from native code, NATIVE_PROGRESSION.md section 2: UNVERIFIED in game);
//  - the save (-owquestsave=) also carries Maya's level, experience and skill grades, which win over -owlevel.
// Every host-chosen value or rule is labelled UNVERIFIED where it is used and in
// docs/verification/SANCTUARY_RPG_MISSION.md ("Host loop with stock world data").
UCLASS()
class OPENWILLOW_API UOpenWillowQuest : public UActorComponent
{
    GENERATED_BODY()
public:
    UOpenWillowQuest();
    virtual void BeginPlay() override;
    virtual void EndPlay(const EEndPlayReason::Type Reason) override;
    virtual void TickComponent(float Delta, ELevelTick Type, FActorComponentTickFunction* Function) override;

    bool Enabled() const { return bEnabled; }
    // The use key near Marcus: accept when not started, turn in when ready; true when it was consumed.
    bool TryUse();
    // Direct mission calls (the use key goes through these; kept callable for tests).
    bool Accept();
    bool TurnIn();
    // A combat target took damage; only the quest's own dummy counts. DamageType: stock object path of the shot's
    // damage type ("" = None).
    void OnDummyDamaged(class AOpenWillowCombatTarget* Target, const FString& DamageType);
    void NotifyRespawn();
    int32 Status() const;
    // Maya's maximum health at Level from the recovered formula; false without slice data.
    bool PlayerMaxHealth(int32 Level, float& Out) const;
    // Respawn location by the decoded station selection (no station activation is modelled); false without data.
    bool RespawnPoint(const FVector& DeathLocation, FTransform& Out);
    // The walker calls this once its start level is set: the loaded save's "progression" block (level, experience,
    // skill grades) replaces that state. False when the save has no such block (older saves) or it was rejected.
    // Then fixes Sanctuary's region stage from the resulting level unless the save already holds one.
    bool RestoreProgression(class UOpenWillowSkills& Skills);
    // The dummy's state written by its own provider's world behaviors (run in Pump):
    //  - Behavior_RegisterTargetable: whether the actor is in the host's targetable list (the stand-in for the global
    //    TargetableList). Phaselock's target choice reads it: the stock dummy is a candidate only once registered
    //    (AOpenWillowWalker::PreferredPhaselockTarget); host-made targets are not in this list and count as registered;
    //  - Behavior_Transform: WillowAIPawn.TransformType and the target name it selects (GetTargetName).
    bool IsRegisteredTargetable(const AActor* Actor) const;
    FString DummyTargetName() const;

private:
    struct FImpl;
    TSharedPtr<FImpl> Impl;
    bool bEnabled = false;
    bool bTesting = false;
    bool bResume = false;
    bool bWeaponLent = false;
    bool bFailed = false;
    int32 Rewards = 0;
    // The use key's press on Marcus runs his stock OnUsed chain on the VM (FireMissionSlice::useMarcus): how many times it opened the mission
    // interface (the host answers that call), the movie definition it named, and the behaviors of the last run ("name(sequence) -> output ids").
    int32 MissionInterfacesOpened = 0;
    FString LastInterfaceMovie;
    // The on-use dialog his chain asked for (WillowAIPawn.PlayOnUseDialog -> the component's TriggerEvent): the global VO_NPC_OnUse_* tag of the last press.
    // Reported only: the line is not played (the generic group's no-match output and Trigger act, then Marcus's own group, are not dispatched here).
    FString LastOnUseTag;
    TArray<FString> LastUseCascade;
    int32 Respawns = 0;
    int32 Checks = 0;
    int32 Errors = 0;
    int32 TestStep = 0;
    float TestWait = 0;
    FString SavePath;
    // Save() runs from Pump() every tick; the file is rewritten only when its text changed.
    FString LastSavedText;
    int32 SaveCalls = 0, SaveWrites = 0;
    double SaveWriteSeconds = 0;
    TSharedPtr<FJsonObject> SavedProgression;   // the loaded save's "progression" block; null for older saves
    bool bProgressionRestored = false;

    void Fail(const FString& Error);
    void Check(bool bGood, const TCHAR* Name);
    void Pump();
    void RouteRequests();
    void Save();
    void RunTest(float Delta);
    bool PlayerTouchesTrigger() const;
    bool TouchesTrigger(const class AActor* Actor) const;
    bool bPlayerInRange = false, bMarcusInRange = false;     // the waypoint cylinder overlap last reported to the slice
    bool InTalkReach() const;
    // What Marcus's mission screen offers for this mission (NATIVE_USE_INTERACTION.md): his own list scripts run on the VM; the screen itself is
    // not hosted. bOffered: the mission is in the redeemable or eligible list (redeemable entries come first).
    void MissionScreen(bool& bRedeemable, bool& bEligible) const;
    void SpawnMarcus();
    void SpawnDummy();
    void ProcessArrivals();
    void UpdateHints();
    // Test helpers: put the player somewhere (looking at Look), press the use key through the player controller.
    void PlacePlayer(const FVector& Location, const FVector& Look);
    void PressUse();
    void Shot(const TCHAR* Name);
    void LendMissionWeapon();
    void ApplyScriptExperience(int32 Amount);
    void SyncScriptInputs();
    bool RunPhaselockTest();            // the Phaselock steps after the mission; false while waiting
    bool PhaselockTableMatchesManifest() const;

    FOpenWillowWeaponItem MissionWeapon;    // the lent recipe (identity, card stats, damage type, mesh path)
    // Turn-in loot STAND-IN (not stock: the mission reward and the dummy carry no items), prepared by
    // tools/weapon_slice_gear.py --reward-only; dropped as a pickup at turn-in when present.
    FOpenWillowWeaponItem RewardItem;
    bool bHasReward = false;
    void DropReward();
    UPROPERTY() TObjectPtr<class AOpenWillowInventoryPickup> RewardPickup;
    FString ItemDir;
    int32 GearLevel = 0;                // slice_manifest.json "level": the level the slice gear was rolled at
    // The mission's level: the GameStageRegion's stage, fixed per player and playthrough the first time it is asked
    // for (0 = not yet). RegionStageLevel is the player level it was computed from.
    int32 RegionStage = 0, RegionStageLevel = 0;
    bool bRegionStageFromSave = false;
    void FixRegionStage(int32 PlayerLevel);
    void LogScriptStubs(const TCHAR* When) const;   // the natives the mission script reached that have no implementation
    int32 LastXpAmount = 0;
    int64 ExperienceBeforeReward = 0;
    int32 LevelBeforeReward = 0;
    FString LastDummyDamageType;
    int32 DummyShots = 0;
    FString WrongId, WrongType;         // test: the equipped non-incendiary gun
    int32 PointsBeforeReward = 0;
    float HealthBeforeReward = 0, MaxHealthBeforeReward = 0;
    float PhaselockCastSeen = 0;
    bool bLendPending = false;
    int32 PhaselockStep = 0;
    float PhaselockWait = 0;
    UPROPERTY() TObjectPtr<class AOpenWillowCombatTarget> PhaselockDummy;
    UPROPERTY() TObjectPtr<AActor> PhaselockCeiling;   // suite fixture for the lift's ceiling clamp

    UPROPERTY() TObjectPtr<class AOpenWillowNpc> Marcus;
    UPROPERTY() TObjectPtr<class AOpenWillowCombatTarget> Dummy;
    TSet<FString> DummyVariables;       // sequence variables the populated events bound to the spawned dummy
    bool bDummySpawned = false;
    bool bWalkStarted = false;
    bool bLookAtFromSequence = false;
    bool bDummyAttached = false;
    bool bDummyDestroyedBySequence = false;
    bool bSendBackFromProvider = false;
    int32 MissionEventsMatched = 0;
    bool bAcceptChecked = false;        // test: the use_key_accepts_mission check ran (step 4 may wait for the walk)
    int32 DialogLookups = 0, DialogMisses = 0, DialogPlayed = 0, DialogAkMismatches = 0;
    TArray<int32> ArrivalMotions;       // door motion requested by each entered ArrivedAtMoveNode event
    FVector DummySpawnedAt = FVector::ZeroVector;
    FVector DummyAttachedAt = FVector::ZeroVector;   // dummy location right after the attach op ran
    FVector AttachCarrierOffset = FVector::ZeroVector; // carrier offset at that moment
    bool bAttachSocketApplied = false;
    // Provider-level state of the dummy (the host's provider outlives the actor; applied to whichever dummy exists).
    FString DummyTransform;             // EAITransformed name set by Behavior_Transform ("" = never set)
    FString TransformSequence;          // sequence of the Behavior_Transform call that set it
    bool bDummyTargetable = false;
    int32 TargetableCalls = 0;
    FString TargetableSequence;         // sequence of the last Behavior_RegisterTargetable call
    bool bReleaseUse = false;
    bool bTrackBound = false;
    bool bTalkHintLogged = false;
};
