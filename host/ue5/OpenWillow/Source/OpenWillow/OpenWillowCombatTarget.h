#pragma once

#include "CoreMinimal.h"
#include "GameFramework/Actor.h"
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
    bool BeginPhaselock(float Now, float Duration);
    FVector AimPoint() const;
    bool IsPhaselocked() const { return bPhaselocked; }
    float HealthFraction() const { return Health / MaxHealth; }
    TArray<FOpenWillowDamagePopup> Popups;
private:
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
    float LastHitAt = -10;
    float DiedAt = -10;
    float FallVelocity = 0;
    float MaxHealth = 20000.f; // host dummy; sized for level-30 item damage
    float Health = 20000.f;
    bool bPhaselocked = false;
    bool bDead = false;
};
