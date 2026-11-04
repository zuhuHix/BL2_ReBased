#pragma once

#include "CoreMinimal.h"
#include "Math/RandomStream.h"
#include "OpenWillowEnemyAI.h"
#include "OpenWillowRangeKind.h"
#include "OpenWillowRangeLoot.h"
#include "OpenWillowSliceData.h"
#include "Subsystems/WorldSubsystem.h"
#include "OpenWillowRange.generated.h"

class AController;
class AOpenWillowCombatTarget;
class AOpenWillowEnemy;
class UPrimitiveComponent;

// The range (PR1): a spot in the world with three static dummies and the manifest's moving enemies, plus the console
// commands (ow.Range, ow.RangeMode, ow.RangeLevel, ow.RangeClear, ow.RangeReport, ow.RangeLoot from PR3) and the -owrangetest driver.
// Host stand-in layout (UNVERIFIED in game): the placement distances are the host's choice, not stock data.
UCLASS()
class OPENWILLOW_API UOpenWillowRangeSubsystem : public UTickableWorldSubsystem
{
    GENERATED_BODY()
public:
    virtual void Initialize(FSubsystemCollectionBase& Collection) override;
    virtual bool DoesSupportWorldType(const EWorldType::Type WorldType) const override;
    virtual void Tick(float DeltaTime) override;
    virtual TStatId GetStatId() const override;

    // Builds the range at Spot facing Yaw (any earlier range is removed first). False when the spot is unusable.
    bool BuildRange(const FVector& Spot, float Yaw);
    void ClearRange();
    // Rebuilds at the last spot (ow.RangeLevel uses it after a level change).
    void RebuildLast();
    // Toggles for every range actor: invulnerable (dummies and enemies), passive (enemies only).
    void SetMode(bool bInvulnerableNow, bool bPassiveNow);
    // AI switch for every range enemy (ow.RangeAI): off = patrol only, the shared threat record reset.
    void SetAI(bool bOn);
    // The spot for ow.Range without arguments: -owrangeat=X,Y,Z[,Yaw], else local/range/range.json "spot", else the manifest's
    // candidates (PR2 follow-up), else the scout.
    bool ResolveSpot(FVector& OutSpot, float& OutYaw, FString& OutSource);
    // Scout (PR1 host choice, UNVERIFIED): the first open, flat, textured spot around the town centre that passes the rules in the .cpp.
    bool ScoutSpot(FVector& OutSpot, float& OutYaw, FString& OutLog);
    // The floor of every formation slot at Spot facing Yaw (3 dummies, one slot per manifest enemy). False, with the first
    // slot that has no walkable floor in OutWhy, when the range does not fit there. bRequireTexturedFloor: every slot must
    // also stand on a textured material (the candidates and the scout set it).
    bool FormationSlots(const FVector& Spot, float Yaw, TArray<FVector>& OutDummies, TArray<FVector>& OutEnemies, FString& OutWhy,
        bool bRequireTexturedFloor = false) const;
    // One manifest candidate (index into Manifest.Candidates): the first accepted point of the candidate and its rings. False,
    // with the reason the candidate point failed in OutWhy, when no point is accepted.
    bool ResolveCandidate(int32 Index, FVector& OutSpot, float& OutYaw, FString& OutMaterial, FString& OutWhy);
    // Enemies the manifest asks for (spawn entries of known kinds) and the dummy count: what a complete build holds.
    int32 ExpectedEnemies() const;
    // The lowest and highest level of the manifest's tables (false when it has none: the stand-in kind).
    bool TableLevels(int32& OutLow, int32& OutHigh);
    static constexpr int32 ExpectedDummies = 3;
    // Enemy level for the next build: the override (ow.RangeLevel) or the walker's level.
    void SetLevelOverride(int32 Level) { LevelOverride = Level; }
    void Report() const;
    // The range's threat record (PR2): one den for all of its enemies.
    const FOpenWillowThreatDen& GetDen() const { return Den; }
    // PR3 loot: ow.RangeLoot normal|always|off, and the death roll's stream restarted at its seed (the test repeats a roll).
    void SetLootMode(EOpenWillowRangeLootMode Mode) { LootMode = Mode; }
    void ResetLootStream() { LootStream.Initialize(LootSeed); }

private:
    void EnsureManifest();
    void HandleEnemyDied(AOpenWillowEnemy* Enemy, AActor* Killer);
    int32 EnemyLevel() const;
    bool LoadSliceData();
    void TickTest(float Now);
    void TickAITest(float Now);
    void TickLootTest(float Now);
    void TestCheck(const TCHAR* Step, bool bPass, const FString& Detail);
    void TestSkip(const TCHAR* Step, const TCHAR* Reason);
    void TestHit(AOpenWillowCombatTarget* Target, float Amount, UPrimitiveComponent* Where, AController* Instigator = nullptr,
        AActor* Causer = nullptr);
    void TestShot(int32 Number);
    // -owrangesurvey driver (PR2 follow-up; see TickSurvey in the .cpp).
    void TickSurvey(float Now);

    FOpenWillowThreatDen Den;          // PR2: the threat record of the range den (centre = the spot)

    // PR3 loot: the death roll's mode and stream (seed -owrangeseed=N, default 20261009), the kind name of each enemy the range
    // built (the roll looks its loot up by it), and the last roll (the test driver reads it).
    EOpenWillowRangeLootMode LootMode = EOpenWillowRangeLootMode::Normal;
    FRandomStream LootStream;
    int32 LootSeed = 20261009;
    TMap<const AOpenWillowEnemy*, FString> EnemyKinds;
    FOpenWillowRangeLootResult LastLoot;
    TArray<TWeakObjectPtr<AActor>> LootActors;   // every drop of the range (ClearRange destroys them)

    TArray<TWeakObjectPtr<AOpenWillowCombatTarget>> Dummies;
    TArray<TWeakObjectPtr<AOpenWillowEnemy>> Enemies;
    FOpenWillowRangeManifest Manifest;
    bool bManifestLoaded = false;
    FOpenWillowSliceData SliceData;
    bool bSliceTried = false;
    bool bSliceLoaded = false;
    bool bModeInvulnerable = false;
    bool bModePassive = false;
    bool bModeAI = true;
    int32 LevelOverride = 0;            // 0 = the walker's level
    bool bBuilt = false;
    FVector SpotNow = FVector::ZeroVector;
    float SpotYawNow = 0.f;
    int32 DeathsSeen = 0;

    // -owrangetest driver (Tick state machine; see TickTest).
    bool bTestMode = false;
    bool bTestDone = false;
    int32 TestStep = 0;
    float TestArmedAt = -1.f;
    float StepStart = 0.f;
    float DeathAt = 0.f;
    int32 TestPass = 0, TestFail = 0, TestSkipped = 0;
    FVector TestSpot = FVector::ZeroVector;
    TArray<FVector> Recorded;           // enemy locations at the start of a movement check
    int32 DeathsBefore = 0;
    int32 PopupsBefore = 0;
    // PR2 driver state (TickAITest): the highest threat level seen in a window, the swing and health baselines, the distance
    // at the start of a chase, and when Maya was moved away from a swing.
    int32 TestMaxLevel = 0;
    int32 TestSwingBase = 0;
    int32 TestResolvedBase = 0;
    float TestHealthBefore = 0.f;
    float TestDistance = 0.f;
    float TestMovedAt = -1.f;
    // PR3 driver state (TickLootTest): the money pickup under test and its baseline, the first seeded trace, the last death.
    TWeakObjectPtr<AActor> LootMoneyPickup;
    int32 LootMoneyBefore = 0;
    int32 LootMoneyAmount = 0;
    FString LootTraceA;
    float LootDeathAt = 0.f;

    // -owrangesurvey driver state: one candidate per step (teleport, then a shot 2.5 s later, then a second's settle).
    bool bSurveyMode = false;
    bool bSurveyDone = false;
    int32 SurveyCount = 8;              // -owrangesurveycount=N
    int32 SurveyIndex = 0;
    float SurveyArmedAt = -1.f;
    float SurveyStepAt = -1.f;          // the teleport of this step (-1: the next step has not started)
    bool bSurveyShot = false;
};
