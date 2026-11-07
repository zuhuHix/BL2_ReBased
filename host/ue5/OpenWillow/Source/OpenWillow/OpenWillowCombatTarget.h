#pragma once

#include "CoreMinimal.h"
#include "GameFramework/Actor.h"
#include "OpenWillowPhaselock.h"
#include "OpenWillowPhaselockFx.h"
#include "OpenWillowHitEvent.h"
#include "OpenWillowCombatTarget.generated.h"

struct FOpenWillowDamagePopup
{
    FVector Location;
    float Amount = 0;
    float Born = 0;
    bool bCritical = false;   // hit on the critical region (range enemies only; see AdjustIncomingDamage)
};

// Local Sanctuary combat stand-in until enemy pawn/AI data is hosted. It is a
// host-made training dummy (engine shapes), not a BL2 enemy mesh or behavior.
UCLASS()
class OPENWILLOW_API AOpenWillowCombatTarget : public AActor, public IOpenWillowHitTarget
{
    GENERATED_BODY()
public:
    // IOpenWillowHitTarget (OpenWillowHitEvent.h): splash distance to the collision centre, the auto-aim radius for the
    // status chance falloff, and the level below.
    virtual void GetHitCentre(FVector& OutCentre, float& OutRadius) const override
    {
        float HalfHeight = 0;
        CollisionCentre(OutCentre, HalfHeight);
        OutRadius = AutoAimRadius();
    }
    virtual int32 GetHitLevel() const override { return Level; }
    // A dead target (dying, ragdolled or waiting to respawn) takes no hits: explosions and status rolls skip it.
    virtual bool CanTakeHits() const override { return !bDead; }
    AOpenWillowCombatTarget();
    virtual void BeginPlay() override;
    virtual void Tick(float DeltaSeconds) override;
    virtual float TakeDamage(float DamageAmount, struct FDamageEvent const& DamageEvent,
        class AController* EventInstigator, AActor* DamageCauser) override;
    // Lifts and holds the target on the stock timeline (FOpenWillowPhaselockData, LiftActionSkill script reading):
    // snap lift over LiftDuration to the lift end (PhaselockLiftHeight), smoothed sine bob timed from the cast while
    // locked, release at ReleasedAt, drop over DropTime, then the diminishing-returns modifier for its duration. With
    // Fx loaded it also draws the stock presentation: the light from the cast, the bubble intro at the lock, the loop
    // with its collapse parameter, the end template at the outro. False when the target is already phaselocked or dead
    // (CanPhaseLockTarget).
    bool BeginPhaselock(float Now, const FOpenWillowPhaselockData& Data, const FOpenWillowPhaselockTimeline& Timeline,
        const FOpenWillowPhaselockFxData* Fx = nullptr);
    // The skill was deactivated before its timeline ended (a while-active constraint failed): the outro starts now if
    // it has not, and the target is released now (LiftActionSkill reading; UNVERIFIED).
    void EndPhaselockNow(float Now);
    // Auto-aim stand-ins for the ITargetable queries the native strategy makes (radius and aim point are not read from
    // the stock pawn): the horizontal radius of the hit volume (stock pawn) or of the torso shape, and AimPoint (chest).
    float AutoAimRadius() const;
    bool IsAutoAimTarget() const { return !bDead; }
    // Bubble draw-scale source: the stock mesh's bounds sphere radius (Pawn.Mesh.Bounds.SphereRadius in the script), or
    // for the engine-shape dummy the radius of its shapes' bounds (host stand-in).
    float MeshBoundsRadius() const;
    // Presentation state for checks: bubble stage (0 none, 1 intro, 2 loop, 3 end), the loop's collapse parameter,
    // the light's current intensity.
    int32 BubbleStage() const { return BubbleStageNow; }
    float BubbleCollapse() const { return CollapseNow; }
    float PhaselockLightIntensity() const;
    FString PresentationReport() const;
    // LiftActionSkill.BeginLifting's lift end, as a height above the target's collision centre now: ground within
    // HeightFromGround below the centre -> ground + collision half height + HeightFromGround, lowered to a surface met on
    // the way up minus the half height; no ground in reach -> 0 (no lift). See the .cpp for the host choices.
    float PhaselockLiftHeight(const FOpenWillowPhaselockData& Data) const;
    // Lift end of the running or last lock, above the target's rest (uu; same frame as LiftedHeight).
    float PhaselockLiftEnd() const { return LiftTo; }
    // Centre and half height of the target's colliding components: the host stand-ins for the pawn's Location and
    // CylinderComponent.CollisionHeight (the stock dummy's collision cylinder is not read; UNVERIFIED).
    void CollisionCentre(FVector& OutCentre, float& OutHalfHeight) const;
    // LiftActionSkill.CanPhaseLockTarget as far as the host can say: alive and not already phaselocked. Friendliness is
    // not modelled (every host target is hostile).
    bool CanPhaseLockTarget() const { return !bDead && !bPhaselocked; }
    // Stands for the AI flag Flag_Skills_CanPhaseLock that CanLiftTargetIf tests (in the data it is computed from
    // Flag_Skills_DisablePhaseLock, PhaseLockOnHold and IsPlayer, none of which the host has). True by default: that
    // the stock dummy can be lifted is UNVERIFIED. False -> TargetBlocked (no lift).
    bool CanPhaseLockFlag() const { return bCanPhaseLockFlag; }
    void SetCanPhaseLockFlag(bool bCan) { bCanPhaseLockFlag = bCan; }
    // PhaselockTimeScale on this target now (default, or with Skill_Phaselock_DiminishingReturns while it runs).
    float PhaselockTimeScale(float Now, const FOpenWillowPhaselockData& Data) const { return Data.TargetTimeScale(Now < DiminishedUntil); }
    float PhaselockReleasedAt() const;
    float LiftedHeight() const;          // current lift above the target's origin (uu)
    FVector AimPoint() const;
    bool IsPhaselocked() const { return bPhaselocked; }
    float HealthFraction() const { return Health / MaxHealth; }
    // Slice route: show the imported stock pawn (GD_TargetDummy) instead of the engine shapes. The actor origin is
    // then the UE3 pawn location and the mesh hangs below it by the pawn's component Translation. Hits use the
    // mesh's physics asset (UModel/glTF import, not the stock collision). A stock dummy does not respawn after
    // death: the installed Kismet destroys it. Returns false when an asset does not load.
    // Hits use a hidden capsule sized from the imported mesh bounds (host-chosen; stock collision not read).
    bool UseStockPawn(const FString& MeshPath, const FString& IdlePath, const FString& DeathPath, const FVector& MeshOffset);
    // Test target only (-owcombattest): with -owcombatpawn=<Id>, show the stock enemy that
    // tools/import_phaselock_target.ps1 imported (local/phaselock/target/host_target.json; e.g. the bullymong, whose AnimSet
    // has the four PhaseLock clips). The actor origin stays on the floor; the hit volume is the pawn's collision cylinder.
    // Returns false (engine shapes kept) when the switch is absent or the data or an asset is missing.
    bool UseCommandLinePawn();
    bool IsStockPawn() const { return StockMesh != nullptr; }
    // Momentum of a hit (Pawn.TakeDamage, engine script, NATIVE_DAMAGE_DEATH 0.1): the velocity change is momentum / Mass
    // (100 x momentum when Mass <= 0.01), added to the pawn's velocity (HandleMomentum). The extra upward part some damage
    // types ask for is not applied (which types do is not decoded). The host target then slides by that velocity, slowed
    // by a ground friction stand-in (KnockFriction, UNVERIFIED), and not while phaselocked or dead.
    static FVector MomentumVelocity(const FVector& Momentum, float Mass);
    float GetMass() const { return Mass; }
    // Experience level for the status-chance level scale; 0 = none of its own (taken as the shooter's: factor 1).
    // The stock dummy's and the stock pawns' levels are not read (UNVERIFIED); -owtargetlevel=N sets it for checks.
    int32 GetLevel() const { return Level; }
    void SetLevel(int32 NewLevel) { Level = FMath::Max(0, NewLevel); }
    const FVector& KnockVelocity() const { return Knock; }
    FVector LastHitMomentum() const { return LastMomentum; }
    // The last simulated ragdoll (kept after the respawn, for the range test and the log).
    bool CorpseMeasured() const { return bCorpseMeasured; }
    float CorpsePeakRise() const { return CorpseRise; }
    float CorpsePeakDrift() const { return CorpseDrift; }
    float CorpsePeakSpeed() const { return CorpseSpeed; }
    int32 CorpseSpeedCaps() const { return CorpseCaps; }
    // Test only: throws one simulating body upward, to see the cap act on a known velocity. False without a simulating body.
    bool KickCorpseForTest(float UpSpeed);
    float CorpseKickedRiseSpeed() const;
    FVector LastHitVelocityChange() const { return LastVelocityChange; }
    FVector Displacement() const { return GetActorLocation() - KnockOrigin; }  // since the last ResetKnock
    void ResetKnock() { Knock = FVector::ZeroVector; KnockOrigin = GetActorLocation(); }
    // Test fixture: a living dummy back to full health (the firing test refills it per segment).
    void RestoreHealth() { if (!bDead) Health = MaxHealth; }
    // Test fixture: a living dummy back at its home spot, at rest and at full health, as a respawn would leave it (the firing
    // test calls it before each gun, so a dummy that never dies does not drift with the pushes of the earlier guns).
    void ResetForTest()
    {
        if (bDead) return;
        Health = MaxHealth;
        SetActorLocation(HomeLocation);
        Knock = FVector::ZeroVector;
        LastMomentum = FVector::ZeroVector;
        LastVelocityChange = FVector::ZeroVector;
        KnockOrigin = HomeLocation;
        ClearSwing();
    }
    // PR1 range additions (host stand-ins, UNVERIFIED in game). The defaults keep the Fire dummy, the firing tests and the
    // Phaselock tests as they were: a stock pawn does not respawn, nothing is invulnerable, no hit is critical.
    void SetMaxHealth(float NewMax) { if (NewMax > 0.f) { MaxHealth = NewMax; Health = NewMax; } }
    float GetHealth() const { return Health; }
    float GetMaxHealth() const { return MaxHealth; }
    bool IsDead() const { return bDead; }
    // How far and how fast the dummy is swinging from hits (degrees, degrees per second).
    float WobbleSize() const { return float(Wobble.Size()); }
    float WobbleSpeed() const { return float(WobbleVelocity.Size()); }
    // Invulnerable: a hit is still recorded (popup, flash, wobble, log, quest hook, momentum) but does not lower health.
    void SetInvulnerable(bool bOn) { bInvulnerable = bOn; }
    bool IsInvulnerable() const { return bInvulnerable; }
    // A stock pawn respawns only when enabled here. Engine-shape targets always respawn; the delay is 3 s unless set here.
    void SetRespawn(bool bEnabled, float DelaySeconds);
    float GetRespawnDelay() const { return RespawnDelay; }
    // Sets the home spot (the respawn spot) and the knock origin.
    void SetHome(const FVector& NewHome) { HomeLocation = NewHome; KnockOrigin = NewHome; }
    FVector GetHome() const { return HomeLocation; }
    class USkeletalMeshComponent* GetStockMesh() const { return StockMesh; }
    TArray<FOpenWillowDamagePopup> Popups;
protected:   // was private: the range enemy (OpenWillowEnemy.cpp) builds on these; no name changed
    UPROPERTY() TObjectPtr<class USkeletalMeshComponent> StockMesh;
    void AttachWornMeshes(const class FJsonObject& Pawn);
    UPROPERTY() TObjectPtr<class UAnimSequence> StockDeath;
    UPROPERTY() TObjectPtr<class UCapsuleComponent> HitVolume;
    UPROPERTY() TObjectPtr<class USceneComponent> Base;
    UPROPERTY() TObjectPtr<class USceneComponent> Pivot;
    UPROPERTY() TObjectPtr<class UStaticMeshComponent> Post;
    UPROPERTY() TObjectPtr<class UStaticMeshComponent> Torso;
    UPROPERTY() TObjectPtr<class UStaticMeshComponent> Head;
    UPROPERTY() TObjectPtr<class UMaterialInstanceDynamic> BodyMaterial;
    UPROPERTY() TObjectPtr<class UMaterialInstanceDynamic> HeadMaterial;
    // Stock presentation (FOpenWillowPhaselockFxData): bubble emitters and the PhaselockLight stand-in.
    UPROPERTY() TObjectPtr<UOpenWillowFxComponent> BubbleIntro;
    UPROPERTY() TObjectPtr<UOpenWillowFxComponent> BubbleLoop;
    UPROPERTY() TObjectPtr<UOpenWillowFxComponent> BubbleOutro;
    UPROPERTY() TObjectPtr<class UPointLightComponent> LockLight;
    // The pawn's PhaseLock_* clips when its imported AnimSet has them (the slice dummy's does not).
    UPROPERTY() TObjectPtr<class UAnimSequence> StockIdle;
    UPROPERTY() TObjectPtr<class UAnimSequence> LiftClip;
    UPROPERTY() TObjectPtr<class UAnimSequence> LoopClip;
    UPROPERTY() TObjectPtr<class UAnimSequence> FallClip;
    UPROPERTY() TObjectPtr<class UAnimSequence> LandClip;
    // Plays a clip on the stock mesh through UOpenWillowTargetAnimInstance (crossfade over BlendIn seconds).
    void PlayStock(class UAnimSequence* Clip, bool bLoop, float Rate = 1.f, float BlendIn = 0.f);
    // SpecialMove_PhaseLock blends read in NATIVE_PHASELOCK_PRESENTATION.md section 2.1 (UNVERIFIED in game): the default
    // definition's lift move overrides BlendInTime to 0.4 s; loop, drop and land keep 0.1 s. EC_OnAnimEnd and
    // EC_StopOnLastFrame force their blend-out to 0, so the return to the idle is a cut.
    static constexpr float LiftBlendIn = 0.4f;
    static constexpr float MoveBlendIn = 0.1f;
    UOpenWillowFxComponent* SpawnBubble(const FString& TemplateName, float Now);
    // Draw scale for a bubble emitter spawned now (mesh bounds / BubbleFXScale, with the host's size calibration).
    float BubbleDrawScale() const;
    void UpdatePresentation(float Now, float Held);
    void ClearPresentation();
    // No swing left from hits: upright and still. The swing only ticks while alive, so a death would otherwise freeze it for the respawn.
    void ClearSwing();
    FOpenWillowPhaselockFxData Fx;
    float LockLightData = 0;            // the light's brightness in the data's units (the UE5 intensity is this x a host gain)
    bool bFx = false;
    FOpenWillowPhaselockTimeline LockTimeline;
    FVector BubbleOffset = FVector::ZeroVector;   // lift-end location in Pivot space
    float BubbleScale = 1;
    float CollapseStartAt = 0;
    float CollapseNow = 0;
    int32 BubbleStageNow = 0;
    float BobHeight = 0;                // smoothed bob (VInterpTo state)
    bool bBobStarted = false;
    float AnimClipEndsAt = 0;
    FVector StockMeshBaseOffset = FVector::ZeroVector;   // the stock mesh's rest offset in Pivot space
    float MeshExtraZ = 0;               // LiftActionSkill.UpdateLiftedPawnMeshOffset: extra Z that centres the mesh bounds
    float PawnLocationAbovePivot = 0;   // where the UE3 pawn Location (the point the mesh is centred on) is above Pivot
    int32 AnimStage = 0;                // 0 none, 1 lift, 2 loop, 3 fall, 4 land
    FVector HomeLocation = FVector::ZeroVector;
    FVector2D Wobble = FVector2D::ZeroVector;
    FVector2D WobbleVelocity = FVector2D::ZeroVector;
    float LockStartedAt = 0;
    float LockEndsAt = 0;
    FOpenWillowPhaselockData Lock;      // the data of the running lock
    float LiftFrom = 0;                 // Pivot height when the lock began
    float LiftTo = 0;                   // Pivot height at the lift end
    bool bCanPhaseLockFlag = true;
    float DropStartedAt = -10;
    float DropFromHeight = 0;
    float ReleasedAt = -10;
    float DiminishedUntil = -10;
    float LastHitAt = -10;
    float DiedAt = -10;
    float FallVelocity = 0;
    // Pawn Mass: Default__Pawn.Mass in Engine.upk is 100 (read from the package); a pawn archetype's own Mass is not read.
    float Mass = 100.f;
    int32 Level = 0;
    // 1/s: Default__PhysicsVolume.GroundFriction (8, read from Engine.upk). How the walking physics applies it to a pushed
    // pawn was not read: velocity x (1 - friction dt) per frame is a host stand-in (UNVERIFIED).
    static constexpr float KnockFriction = 8.f;
    FVector Knock = FVector::ZeroVector;
    FVector KnockOrigin = FVector::ZeroVector;
    FVector LastMomentum = FVector::ZeroVector;
    FVector LastVelocityChange = FVector::ZeroVector;
    float MaxHealth = 20000.f; // host dummy; sized for level-30 item damage
    float Health = 20000.f;
    bool bPhaselocked = false;
    bool bDead = false;
    // PR1 range state (off by default).
    bool bInvulnerable = false;         // SetInvulnerable
    bool bRespawnStock = false;         // SetRespawn: a stock pawn respawns (engine shapes always do)
    bool bRagdoll = false;              // the stock mesh is limp after its death (TickStockRespawn)
    bool bRagdollSimulating = false;    // ... and physics runs it (a physics asset was found)
    void TrackCorpse();                 // caps the bodies' speed and measures how far the body went
    bool bCorpseMeasured = false;
    FVector CorpseFrom = FVector::ZeroVector;
    float CorpseRise = 0.f;
    float CorpseDrift = 0.f;
    float CorpseSpeed = 0.f;
    int32 CorpseCaps = 0;
    FName CorpseKickBone;
    float RespawnDelay = 3.f;           // death to respawn, seconds (3 s is the engine-shape value the Fire dummy keeps)
    // Death to ragdoll, seconds: the value src/combat_run.cpp uses for the player's ragdoll. For targets UNVERIFIED.
    static constexpr float RagdollDelay = 0.84f;
    // Host stand-ins for a limp body (no game values): how fast bodies may push out of an overlap when physics takes over, and how fast
    // any body may rise or travel sideways. A fall stays free.
    static constexpr float RagdollDepenetrationSpeed = 100.f;
    static constexpr float RagdollMaxRiseSpeed = 300.f;
    static constexpr float RagdollMaxFlatSpeed = 600.f;
    void TickStockRespawn(float Now);
    // Hooks for subclasses. Defaults: the damage passes through, no critical hit, nothing on death or respawn.
    // AdjustIncomingDamage runs after the death and quest checks and before health is lowered; its result is the damage
    // taken from health (the popup and the return value use it). OnDied runs at the end of the death branch.
    virtual float AdjustIncomingDamage(float Amount, const FDamageEvent& DamageEvent, bool& bOutCritical);
    virtual void OnDied(AActor* DamageCauser) {}
    virtual void OnRespawned() {}
};
