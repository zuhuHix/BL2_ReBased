#pragma once

#include "CoreMinimal.h"
#include "OpenWillowRangeKind.h"
#include "OpenWillowSliceData.h"
#include "Subsystems/WorldSubsystem.h"
#include "OpenWillowRange.generated.h"

class AOpenWillowCombatTarget;
class AOpenWillowEnemy;
class UPrimitiveComponent;

// The range (PR1): a spot in the world with three static dummies and the manifest's moving enemies, plus the console
// commands (ow.Range, ow.RangeMode, ow.RangeLevel, ow.RangeClear, ow.RangeReport) and the -owrangetest driver.
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
    // The spot for ow.Range without arguments: -owrangeat=X,Y,Z[,Yaw], else local/range/range.json "spot", else the scout.
    bool ResolveSpot(FVector& OutSpot, float& OutYaw, FString& OutSource);
    // Scout (PR1 host choice, UNVERIFIED): the first open, flat spot around the town centre that passes the rules in the .cpp.
    bool ScoutSpot(FVector& OutSpot, float& OutYaw, FString& OutLog);
    // The floor of every formation slot at Spot facing Yaw (3 dummies, one slot per manifest enemy). False, with the first
    // slot that has no walkable floor in OutWhy, when the range does not fit there.
    bool FormationSlots(const FVector& Spot, float Yaw, TArray<FVector>& OutDummies, TArray<FVector>& OutEnemies, FString& OutWhy) const;
    // Enemies the manifest asks for (spawn entries of known kinds) and the dummy count: what a complete build holds.
    int32 ExpectedEnemies() const;
    static constexpr int32 ExpectedDummies = 3;
    // Enemy level for the next build: the override (ow.RangeLevel) or the walker's level.
    void SetLevelOverride(int32 Level) { LevelOverride = Level; }
    void Report() const;

private:
    void EnsureManifest();
    void HandleEnemyDied(AOpenWillowEnemy* Enemy, AActor* Killer);
    int32 EnemyLevel() const;
    bool LoadSliceData();
    void TickTest(float Now);
    void TestCheck(const TCHAR* Step, bool bPass, const FString& Detail);
    void TestSkip(const TCHAR* Step, const TCHAR* Reason);
    void TestHit(AOpenWillowCombatTarget* Target, float Amount, UPrimitiveComponent* Where);
    void TestShot(int32 Number);

    TArray<TWeakObjectPtr<AOpenWillowCombatTarget>> Dummies;
    TArray<TWeakObjectPtr<AOpenWillowEnemy>> Enemies;
    FOpenWillowRangeManifest Manifest;
    bool bManifestLoaded = false;
    FOpenWillowSliceData SliceData;
    bool bSliceTried = false;
    bool bSliceLoaded = false;
    bool bModeInvulnerable = false;
    bool bModePassive = false;
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
};
