#pragma once

#include "CoreMinimal.h"
#include "OpenWillowCombatTarget.h"
#include "OpenWillowEnemyAI.h"
#include "OpenWillowRangeKind.h"
#include "OpenWillowEnemy.generated.h"

class AOpenWillowEnemy;
class UPrimitiveComponent;
class UWorld;

// Broadcast when a range enemy dies (PR3 loot hangs on this). Killer is the damage causer, possibly null.
DECLARE_MULTICAST_DELEGATE_TwoParams(FOnRangeEnemyDied, AOpenWillowEnemy*, AActor*);

// A range enemy: the combat target with a kind from the range manifest. PR1 = patrol; PR2 = notice, chase and melee (the
// brain in OpenWillowEnemyAI.h); no loot (PR3).
// Host stand-in throughout (UNVERIFIED in game): the health bar, the walk speed, the shield, the critical region and its
// multiplier. The critical multiplier 2.0 is the CurrentInstantHitCriticalHitBonus default (NATIVE_COMBAT_GROUNDWORK 3.4).
UCLASS()
class OPENWILLOW_API AOpenWillowEnemy : public AOpenWillowCombatTarget
{
    GENERATED_BODY()
public:
    AOpenWillowEnemy();
    virtual void BeginPlay() override;
    virtual void EndPlay(const EEndPlayReason::Type Reason) override;
    virtual void Tick(float DeltaSeconds) override;

    // Sets the kind up: the stock pawn when its assets load (else the engine shapes, logged as fallback=shapes), the level,
    // the max health of that level (stand-in 400 when the table has none), the shield, respawn after RespawnSeconds.
    // Returns true when the stock pawn is in use.
    bool SetupKind(const FOpenWillowRangeKind& InKind, int32 InLevel, float RespawnSeconds);
    void SetPassive(bool bOn) { bPassive = bOn; }
    // AI off: the brain is stopped and the enemy only patrols (ow.RangeAI off; the range test's target steps run this way).
    void SetAIEnabled(bool bOn) { bAIEnabled = bOn; }
    bool IsAIEnabled() const { return bAIEnabled; }
    bool IsPassive() const { return bPassive; }
    // The range's threat record (shared by the range's enemies). Null: the enemy patrols only.
    void SetDen(FOpenWillowThreatDen* InDen) { Den = InDen; }
    const FOpenWillowEnemyBrain& GetBrain() const { return Brain; }
    // The kind's swing cycle: its live row (FindKindMelee), else the stand-in OpenWillowThreat::SwingInterval.
    float SwingIntervalNow() const { return KindMelee ? KindMelee->SwingInterval : OpenWillowThreat::SwingInterval; }
    // The kind's live row (cycle and stand distance), or null when no live read covers the kind.
    const FOpenWillowKindMelee* GetKindMelee() const { return KindMelee; }
    // The kind's leap and throw (OpenWillowLeapThrow.h), or null for a kind with neither (the psycho, the stand-in kind).
    const FOpenWillowKindAttacks* GetKindAttacks() const { return KindAttacks; }
    // Leaps and throws on or off for this enemy (on by default). Off is a test fixture: the range test's kind_melee step measures the plain
    // swing alone. Turning them off ends one in progress.
    void SetRangedAttacks(bool bOn);
    bool AreRangedAttacksOn() const { return bRangedAttacks; }
    // The brain's draw stream (the range seeds it per enemy, -owrangeaiseed=N) and the test's request for the next attack.
    void SeedAttackDraw(int32 Seed) { Brain.SeedAttackDraw(Seed); }
    void ForceAttackForTest(EOpenWillowAttack Attack) { Brain.ForceAttackForTest(Attack); }
    // A rock of this enemy came down (AOpenWillowRangeRock): written into the brain's last-rock record when it is that rock.
    void RecordRockImpact(float ReleaseAt, const TCHAR* What, float ImpactAt, float MissDistance, bool bHit, float Applied, float KnockSpeed);
    // Test fixture (the range test's AI steps): the enemy back at home at full health, facing Yaw, with no target, no swing and
    // no chase, and its patrol held for HoldSeconds, so each step's scene is laid out from the same place.
    void ResetForAITest(float Now, float Yaw, float HoldSeconds);
    // The exposure stand-in (UNVERIFIED): a clear Visibility line from the enemy's centre to Maya's centre, or from the top of its
    // body to her eye height (a low bump of the ground blocks only the first). Maya and the enemy themselves are ignored.
    bool HasLineOfSightTo(const FVector& MayaCentre, const AActor* Maya) const;
    // Hits from Maya (her pawn, its controller, or an actor she instigated) set the "hit by Maya" flag; the brain reads it.
    virtual float TakeDamage(float DamageAmount, struct FDamageEvent const& DamageEvent, class AController* EventInstigator,
        AActor* DamageCauser) override;
    // Test-only: sets the shield (max and current) to Amount.
    void SetShieldForTest(float Amount) { ShieldMax = Amount; ShieldCurrent = Amount; }
    float ShieldNow() const { return ShieldCurrent; }
    float ShieldMaxNow() const { return ShieldMax; }
    // The colliding body volume (stock: the hit capsule; engine shapes: the torso) and the critical region (stock: a sphere on
    // the head bone; engine shapes: the head). Null when the stock mesh has no head bone.
    UPrimitiveComponent* BodyComponent() const;
    UPrimitiveComponent* CritComponent() const;
    // The one critical-hit decision: true when the hit component is the critical region, the hit bone's name contains "head"
    // (case-insensitive), or the shot entered the body volume on a line that passes through the head sphere (the body capsule
    // encloses the head, so a shot at the head meets the capsule first; host stand-in, UNVERIFIED). AdjustIncomingDamage calls
    // it with the point hit's component, bone, impact point and shot direction (zero direction: no line test).
    bool IsCritRegion(const UPrimitiveComponent* HitComponent, FName Bone, const FVector& Impact, const FVector& ShotDirection) const;
    FString StateReport() const;
    // The floor under XY: the first Visibility hit of a trace from FromZ down to ToZ, ignoring Ignore. False when there is none.
    static bool TraceFloor(UWorld* World, const FVector2D& XY, float FromZ, float ToZ, const AActor* Ignore, FVector& OutFloor);

    FOnRangeEnemyDied OnEnemyDied;

protected:
    virtual float AdjustIncomingDamage(float Amount, const FDamageEvent& DamageEvent, bool& bOutCritical) override;
    virtual void OnDied(AActor* DamageCauser) override;
    virtual void OnRespawned() override;

private:
    void MakeCritVolume();
    void UpdatePatrol(float Now, float DeltaSeconds);
    void TurnToward(const FVector& Dir, float DeltaSeconds);
    void UpdateBrain(float Now, float DeltaSeconds);
    void UpdateChase(const FVector& Target, bool bRun, float StepLimit, float Now, float DeltaSeconds);
    void PlayMelee(float Now);
    // Leaps and throws: what the brain's orders ask of the body (clips, the flight along the arc, the landing, the rocks).
    void UpdateAttack(float Now, float DeltaSeconds, const FOpenWillowBrainInput& In, const FOpenWillowBrainOrders& Orders);
    void EndAttackMotion();
    void TurnWithin(const FVector& Dir, float SecondsLeft, float DeltaSeconds);
    FVector HandLocation(float HalfHeight) const;
    void SetMoveAnim(bool bMoving, bool bRun = false);
    void UpdateHealthBars();

    FOpenWillowRangeKind Kind;
    const FOpenWillowKindMelee* KindMelee = nullptr;   // a row of the static table in OpenWillowEnemyAI.cpp (set by SetupKind)
    bool bStandOnContactLogged = false;                // the "stand inside body contact" line is logged once per setup
    UPROPERTY() TObjectPtr<class USphereComponent> CritVolume;
    UPROPERTY() TObjectPtr<class UAnimSequence> WalkClip;
    UPROPERTY() TObjectPtr<class UAnimSequence> RunClip;
    UPROPERTY() TObjectPtr<class UAnimSequence> MeleeClip;
    UPROPERTY() TObjectPtr<class UAnimSequence> HitClip;
    // Leap and throw clips from the manifest (null: the import has none; the body then holds its idle, a host stand-in).
    UPROPERTY() TObjectPtr<class UAnimSequence> LeapStartClip;
    UPROPERTY() TObjectPtr<class UAnimSequence> LeapIdleClip;
    UPROPERTY() TObjectPtr<class UAnimSequence> LeapEndClip;
    UPROPERTY() TObjectPtr<class UAnimSequence> ThrowClip;
    UPROPERTY() TObjectPtr<class UStaticMeshComponent> BarBack;
    UPROPERTY() TObjectPtr<class UStaticMeshComponent> BarFill;
    UPROPERTY() TObjectPtr<class UStaticMeshComponent> BarShield;

    FRandomStream Rand;
    bool bPassive = false;
    bool bAIEnabled = true;
    bool bHasTarget = false;
    // PR2 (range AI): the brain, the den's record, the hit-by-Maya flag and the melee clip's hold.
    FOpenWillowEnemyBrain Brain;
    FOpenWillowThreatDen* Den = nullptr;
    bool bDamagedByMaya = false;
    bool bMeleePlaying = false;
    float MeleeEndsAt = 0.f;
    // Leaps and throws.
    const FOpenWillowKindAttacks* KindAttacks = nullptr;   // a row of the static table in OpenWillowLeapThrow.h (set by SetupKind)
    bool bRangedAttacks = true;
    bool bAttackPlaying = false;        // a leap or throw owns the clips (the move clips wait)
    bool bWasAirborne = false;          // last tick it was in the air: if the leap ends otherwise than by landing, it is put on the floor
    FName HandBone;                     // where rocks leave (None: a stand-in point in front of the chest)
    TArray<TWeakObjectPtr<AActor>> Rocks;   // its rocks in flight (removed with the enemy, and by the test's reset)
    FVector PatrolTarget = FVector::ZeroVector;
    float PauseUntil = 0.f;
    UAnimSequence* MoveClipNow = nullptr;   // the clip the move state last started (avoids restarts)
    bool bHitPlaying = false;
    float HitEndsAt = 0.f;
    float LastHitReactAt = -10.f;
    float ShieldMax = 0.f;
    float ShieldCurrent = 0.f;
    float ShieldRate = 0.f;             // per second, after ShieldDelay without a hit
    float ShieldDelay = 0.f;
    // The brain's last line-of-sight sample (taken on its state updates only).
    bool bLineOfSightSample = false;
    // Chase progress watch (UpdateChase; host choice, UNVERIFIED).
    static constexpr float ChaseCheckSeconds = 1.f;
    static constexpr float ChaseMinProgress = 10.f;
    static constexpr float ChaseSideStepDegrees = 70.f;
    static constexpr float ChaseGiveUpSeconds = 3.f;
    static constexpr float ChaseRetrySeconds = 5.f;
    float ChaseCheckAt = -1.f;
    FVector ChaseCheckFrom = FVector::ZeroVector;
    float ChaseBlockedSince = -1.f;
    float ChaseSideSign = 1.f;
};

// One thrown rock (NATIVE_AI_LEAP_THROW 4.2 and 4.3): it follows its solved arc, damages Maya on a touch of her capsule (the plain melee's
// numbers, Behavior_CauseDamage), pushes her with the momentum 50,000, and ends on static world geometry or after 20 s. Host stand-ins
// (UNVERIFIED): an engine sphere for the look, a touch radius of 20 uu, other enemies not hit, no radius damage (none was found in the data).
UCLASS()
class OPENWILLOW_API AOpenWillowRangeRock : public AActor
{
    GENERATED_BODY()
public:
    AOpenWillowRangeRock();
    virtual void Tick(float DeltaSeconds) override;
    void Launch(AOpenWillowEnemy* InThrower, const FOpenWillowArc& InArc, float InDamage, float InReleaseAt, const FString& InKind);

private:
    void Finish(const TCHAR* What, const FVector& At, bool bHit, float Applied, float Knock);
    UPROPERTY() TObjectPtr<class UStaticMeshComponent> Visual;
    TWeakObjectPtr<AOpenWillowEnemy> Thrower;
    FString ThrowerName;
    FString Kind;
    FOpenWillowArc Arc;
    float Damage = 0.f;
    float ReleaseAt = 0.f;
    float Age = 0.f;
    bool bDone = true;
};
