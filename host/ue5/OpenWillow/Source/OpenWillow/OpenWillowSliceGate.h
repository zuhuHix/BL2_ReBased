#pragma once
#include "CoreMinimal.h"
#include "Components/ActorComponent.h"
#include "OpenWillowSliceGate.generated.h"

// -owslicegate: one unattended run of the Phase 4 slice gate (ROADMAP.md) on Sanctuary with Maya, chaining the pieces
// other lanes build: spawn, fight and kill an enemy, loot (or roll) a gun, equip it, Phaselock an enemy, the Fire
// mission from accept to turn-in, then go down, bleed out and respawn. It drives the host through its public calls
// (the use key, FireOnce, EquipItem, UsePhaselock) and implements no gameplay itself. Each link logs one line:
//   OWSLICEGATE link=<name> result=PASS|FAIL|NOT_AVAILABLE fixtures=[<test fixtures it relied on>] detail=<why>
// and the run ends with OWSLICEGATE SUMMARY; tools/check_slice_gate.py grades the log (tools/test_slice_gate.ps1).
// NOT_AVAILABLE marks a piece the host does not have yet (for example no loot drop on a kill); the run then uses a
// stated fallback so the later links still run. Host behaviour only: nothing here is checked against the real game.
// Added by the walker in BeginPlay when the switch is on; needs -owwalk -owmaya -owquest (and the quest's manifests).
UCLASS()
class OPENWILLOW_API UOpenWillowSliceGate : public UActorComponent
{
    GENERATED_BODY()
public:
    UOpenWillowSliceGate();
    virtual void TickComponent(float Delta, ELevelTick Type, FActorComponentTickFunction* Function) override;

private:
    enum class EStep : uint8
    {
        Start, Spawn, Arena, Fight, Recover, Loot, Pickup, PickupDecoy, Equip, Phaselock,
        Accept, WaitWalk, Trigger, ShootDummy, TurnIn, Confirm, Down, Bleed, Done
    };
    EStep Step = EStep::Start;
    float StepTime = 0.f;           // seconds in the current step
    float RunTime = 0.f;
    int32 Pass = 0, Failed = 0, NotAvailable = 0;
    bool bUseHeld = false;          // the simulated E key is down (released after 0.1 s)
    float UseHeldFor = 0.f;
    bool bPlacedAtMarcus = false, bPlacedForTurnIn = false, bRolled = false;   // one-shot actions of their steps
    bool bRangeMode = false;        // ow.Range existed and built the range
    bool bAcceptStarted = false;    // the accept step waited for the Phaselock to end
    float ShotClock = 0.f;

    UPROPERTY() TObjectPtr<class AOpenWillowCombatTarget> Enemy;     // the enemy the fight link kills
    UPROPERTY() TObjectPtr<class AOpenWillowCombatTarget> MissionDummy;
    UPROPERTY() TObjectPtr<class AOpenWillowInventoryPickup> Drop;
    UPROPERTY() mutable TObjectPtr<class AOpenWillowNpc> Marcus;   // found once at the giver marker
    TSet<TObjectKey<AActor>> PickupsBeforeKill;
    FVector KillLocation = FVector::ZeroVector;
    FVector LastEnemyLocation = FVector::ZeroVector;
    int32 KillShots = 0;
    int32 PickupAttemptsBefore = 0;
    TSet<FString> BackpackBefore;   // stable item ids before an action that should add one
    FString GunId;                  // the looted or rolled gun the equip link draws
    FString LentId;                 // the Fire mission's lent pistol
    int32 DummyShots = 0;
    float RespawnAtBefore = 0.f;
    FVector DeathLocation = FVector::ZeroVector;
    int64 ExperienceBeforeTurnIn = 0;

    class AOpenWillowWalker* Walker() const;
    void Link(const TCHAR* Name, const TCHAR* Result, const FString& Detail);
    // A test fixture the next link relies on; Link prints the list as fixtures=[a; b] and clears it.
    void Fixture(const FString& What) { Fixtures.AddUnique(What); }
    TArray<FString> Fixtures;
    bool bPhaselockPointGranted = false;
    bool bTestDrop = false;         // the pickup link: the test dropped the gun
    bool bLootForced = false;       // ow.RangeLoot always ran before the fight
    bool bRecoverStarted = false;
    bool bLethalApplied = false;
    bool SkipWhileDown();
    FString RollSource;             // ow.RollGun source for the test drops
    FString DropId, CardId, PickedId, NearId;   // the aimed drop, the card's id at the press, what E picked up, the decoy
    int32 PickupPhase = 0;
    float PickupWait = 0.f;
    UPROPERTY() TObjectPtr<class AOpenWillowInventoryPickup> DecoyAimed;
    UPROPERTY() TObjectPtr<class AOpenWillowInventoryPickup> DecoyNear;
    static FString PickupId(const class AOpenWillowInventoryPickup* Pickup);
    class AOpenWillowInventoryPickup* DropRolledGun(uint32 Seed, const FVector& Near, FString& OutWhy);
    bool RunAimedPickup(class AOpenWillowInventoryPickup* Target);
    int32 LastHitCount = 0, EnemyHits = 0, OtherHits = 0, UnattributedHits = 0;   // Maya's hits during the fight, by causer
    void PassLink(const TCHAR* Name, const FString& Detail) { Link(Name, TEXT("PASS"), Detail); }
    void FailLink(const TCHAR* Name, const FString& Detail) { Link(Name, TEXT("FAIL"), Detail); }
    void MissingLink(const TCHAR* Name, const FString& Detail) { Link(Name, TEXT("NOT_AVAILABLE"), Detail); }
    void Next(EStep To) { Step = To; StepTime = 0.f; ShotClock = 0.f; }
    void Finish(const FString& Why);
    void PressUse();
    void Place(const FVector& Location, const FVector& Look);
    void AimAt(const FVector& Point);
    bool ShootAt(class AOpenWillowCombatTarget* Target, float Interval);
    class AOpenWillowCombatTarget* NearestLiveTarget(bool bPreferEnemyClass) const;
    class AOpenWillowNpc* FindMarcus() const;
    bool QuestMarker(const TCHAR* Kind, FVector& Out) const;
    void SnapshotBackpack();
    FString NewBackpackItem() const;
    bool EquipAndDraw(const FString& Id, FString& OutWhy);
};
