#pragma once

#include "CoreMinimal.h"
#include "Components/ActorComponent.h"
#include "OpenWillowFiring.h"
#include "OpenWillowStatusEffect.generated.h"

// Elemental status effects on a target (burning, shock, corrosive, slag), added to the target when an effect first
// lands. Damage over time = the weapon's StatusEffectDamage per second for the definition's BaseDuration, paid on the
// clock measured in the running game (weapon_firing::dotAdvance). The native application and stacking were not read
// (NATIVE_WEAPON_FIRING 4.3); the stacking per element follows the live observation (UNVERIFIED as a rule).
UCLASS()
class OPENWILLOW_API UOpenWillowStatusEffects : public UActorComponent
{
    GENERATED_BODY()
public:
    UOpenWillowStatusEffects();
    virtual void TickComponent(float DeltaTime, ELevelTick TickType, FActorComponentTickFunction* ThisTickFunction) override;
    static void Apply(AActor* Target, class AOpenWillowWalker* Shooter, const FOpenWillowStatus& Status,
        const FString& DamageType);
    bool IsActive(const FString& Type) const;
    // True while any status effect is on Target (the -owfiringtest wait), and the reset that ends them all.
    static bool AnyActive(const AActor* Target);
    static void ClearAll(AActor* Target);
private:
    struct FActive
    {
        FString Type;
        FString DamageType;
        float Dps = 0;
        float EndsAt = 0;
        double Clock = 0;        // time built up toward the next tick
        float UpdatedAt = 0;
        TWeakObjectPtr<class AOpenWillowWalker> Shooter;
    };
    TArray<FActive> Active;
};
