#pragma once
#include "CoreMinimal.h"
#include "Components/ActorComponent.h"
#include "GameFramework/DamageType.h"
#include "OpenWillowQuest.generated.h"

// Fire-element damage tag for the lent mission pistol. Host classification only: the stock game's damage-type
// objects are not wired; this stands in for the type the dummy's CompareObject behavior inspects (UNVERIFIED).
UCLASS()
class OPENWILLOW_API UOpenWillowFireDamageType : public UDamageType
{
    GENERATED_BODY()
};

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
//  - mission dialog is looked up in the audio manifest and logged (nothing is decoded or played).
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
    // A combat target took damage; only the quest's own dummy counts. The host classifies the damage type.
    void OnDummyDamaged(class AOpenWillowCombatTarget* Target, bool bFire);
    bool LentWeaponIsFire() const { return bWeaponLent; }
    void NotifyRespawn();
    int32 Status() const;
    // Maya's maximum health at Level from the recovered formula; false without slice data.
    bool PlayerMaxHealth(int32 Level, float& Out) const;
    // Respawn location by the decoded station selection (no station activation is modelled); false without data.
    bool RespawnPoint(const FVector& DeathLocation, FTransform& Out);

private:
    struct FImpl;
    TSharedPtr<FImpl> Impl;
    bool bEnabled = false;
    bool bTesting = false;
    bool bResume = false;
    bool bWeaponLent = false;
    bool bFailed = false;
    int32 Rewards = 0;
    int32 Respawns = 0;
    int32 Checks = 0;
    int32 Errors = 0;
    int32 TestStep = 0;
    float TestWait = 0;
    FString SavePath;

    void Fail(const FString& Error);
    void Check(bool bGood, const TCHAR* Name);
    void Pump();
    void RouteRequests();
    void Save();
    void RunTest(float Delta);
    bool PlayerTouchesTrigger() const;
    bool InTalkReach() const;
    void SpawnMarcus();
    void SpawnDummy();
    void ProcessArrivals();
    void UpdateHints();
    // Test helpers: put the player somewhere (looking at Look), press the use key through the player controller.
    void PlacePlayer(const FVector& Location, const FVector& Look);
    void PressUse();
    void Shot(const TCHAR* Name);

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
    int32 DialogLookups = 0, DialogMisses = 0, DialogPlayed = 0;
    TArray<int32> ArrivalMotions;       // door motion requested by each entered ArrivedAtMoveNode event
    FVector DummySpawnedAt = FVector::ZeroVector;
    bool bReleaseUse = false;
    bool bTrackBound = false;
    bool bTalkHintLogged = false;
};
