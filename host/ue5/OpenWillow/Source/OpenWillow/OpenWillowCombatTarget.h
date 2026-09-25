#pragma once

#include "CoreMinimal.h"
#include "GameFramework/Actor.h"
#include "OpenWillowCombatTarget.generated.h"

// Local Sanctuary combat stand-in until enemy pawn/AI data is hosted.
UCLASS()
class OPENWILLOW_API AOpenWillowCombatTarget : public AActor
{
    GENERATED_BODY()
public:
    AOpenWillowCombatTarget();
    virtual void Tick(float DeltaSeconds) override;
    virtual float TakeDamage(float DamageAmount, struct FDamageEvent const& DamageEvent,
        class AController* EventInstigator, AActor* DamageCauser) override;
    bool BeginPhaselock(float Now, float Duration);
private:
    UPROPERTY() TObjectPtr<class UStaticMeshComponent> Visual;
    FVector LockStart = FVector::ZeroVector;
    float LockStartedAt = 0;
    float LockEndsAt = 0;
    float Health = 100.f;
    bool bPhaselocked = false;
};
