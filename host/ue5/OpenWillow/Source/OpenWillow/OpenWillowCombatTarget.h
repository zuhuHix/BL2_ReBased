#pragma once

#include "CoreMinimal.h"
#include "GameFramework/Actor.h"
#include "OpenWillowPhaselock.h"
#include "OpenWillowPhaselockFx.h"
#include "OpenWillowCombatTarget.generated.h"

struct FOpenWillowDamagePopup
{
    FVector Location;
    float Amount = 0;
    float Born = 0;
};

// Local Sanctuary combat stand-in until enemy pawn/AI data is hosted. It is a
// host-made training dummy (engine shapes), not a BL2 enemy mesh or behavior.
UCLASS()
class OPENWILLOW_API AOpenWillowCombatTarget : public AActor
{
    GENERATED_BODY()
public:
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
    bool IsStockPawn() const { return StockMesh != nullptr; }
    class USkeletalMeshComponent* GetStockMesh() const { return StockMesh; }
    TArray<FOpenWillowDamagePopup> Popups;
private:
    UPROPERTY() TObjectPtr<class USkeletalMeshComponent> StockMesh;
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
    UOpenWillowFxComponent* SpawnBubble(const FString& TemplateName, float Now);
    // Draw scale for a bubble emitter spawned now (mesh bounds / BubbleFXScale, with the host's size calibration).
    float BubbleDrawScale() const;
    void UpdatePresentation(float Now, float Held);
    void ClearPresentation();
    FOpenWillowPhaselockFxData Fx;
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
    float MaxHealth = 20000.f; // host dummy; sized for level-30 item damage
    float Health = 20000.f;
    bool bPhaselocked = false;
    bool bDead = false;
};
