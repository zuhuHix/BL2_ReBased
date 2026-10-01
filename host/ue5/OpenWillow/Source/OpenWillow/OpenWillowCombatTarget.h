#pragma once

#include "CoreMinimal.h"
#include "GameFramework/Actor.h"
#include "OpenWillowPhaselock.h"
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
    // snap lift over LiftDuration to HeightFromGround, sine bob while locked, shell fade over the outro, release at
    // ReleasedAt, drop over DropTime, then the diminishing-returns modifier for its duration. False when the target is
    // already phaselocked or dead (CanPhaseLockTarget).
    bool BeginPhaselock(float Now, const FOpenWillowPhaselockData& Data, const FOpenWillowPhaselockTimeline& Timeline);
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
    UPROPERTY() TObjectPtr<class UStaticMeshComponent> LockSphere;
    UPROPERTY() TObjectPtr<class UMaterialInstanceDynamic> BodyMaterial;
    UPROPERTY() TObjectPtr<class UMaterialInstanceDynamic> HeadMaterial;
    UPROPERTY() TObjectPtr<class UMaterialInstanceDynamic> LockMaterial;
    FVector HomeLocation = FVector::ZeroVector;
    FVector2D Wobble = FVector2D::ZeroVector;
    FVector2D WobbleVelocity = FVector2D::ZeroVector;
    float LockStartedAt = 0;
    float LockEndsAt = 0;
    FOpenWillowPhaselockData Lock;      // the data of the running lock
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
