#pragma once

#include "CoreMinimal.h"
#include "GameFramework/Actor.h"
#include "OpenWillowFiring.h"
#include "OpenWillowProjectile.generated.h"

// One fired pellet that travels: a light bullet (EWWFT_Bullet; damage when it arrives) or a rocket (EWWFT_Rocket;
// damage from its explosion). Flight, splash and status rules are src/weapon_firing.* (NATIVE_WEAPON_FIRING.md);
// the look (a stretched additive streak, a glowing slug for rockets, a host flash for explosions) is a stand-in, not
// BL2's particle systems.
UCLASS()
class OPENWILLOW_API AOpenWillowProjectile : public AActor
{
    GENERATED_BODY()
public:
    AOpenWillowProjectile();
    virtual void Tick(float DeltaSeconds) override;

    struct FLaunch
    {
        const FOpenWillowFiringData* Data = nullptr; // copied; the item may change while the pellet flies
        class AOpenWillowWalker* Shooter = nullptr;
        FVector Start = FVector::ZeroVector;
        FVector Direction = FVector::ForwardVector;
        FVector VisualStart = FVector::ZeroVector;   // muzzle: the streak is drawn from here toward the flight path
        float Damage = 0;                            // per pellet (card damage; a Tediore throw: card x rounds left)
        float SplashDamage = -1;                     // < 0: the firing data's splash damage
        FString DamageType;                          // stock damage type path the target reads
        bool bDrawStreak = true;
        FString Element;                             // the card element (impact colour)
        FLinearColor Color = FLinearColor(1.f, 0.78f, 0.45f);
    };
    static AOpenWillowProjectile* Launch(UWorld* World, const FLaunch& Launch);

    // Splash from an explosion at Origin (also used by trace shots of guns whose firing mode explodes on impact).
    static void Explode(UWorld* World, class AOpenWillowWalker* Shooter, const FOpenWillowFiringData& Data,
        const FVector& Origin, const FVector& Normal, float SplashDamage);
    // Host impact and explosion looks (stand-ins for the particle systems).
    static void ImpactFx(UWorld* World, const FVector& Point, const FVector& Normal, const FString& Element);
    static void ExplosionFx(UWorld* World, const FVector& Origin, const FVector& Normal, float Radius,
        const FString& DamageType);
    // A damaging hit on a target: damage, then the status effect roll (NATIVE_WEAPON_FIRING 4.1-4.2).
    static void HitTarget(class AOpenWillowWalker* Shooter, const FOpenWillowFiringData& Data, AActor* Target,
        float Damage, const FString& DamageType, const FVector& Direction, const FHitResult& Hit, float StatusScale);

private:
    void Impact(const FHitResult& Hit);
    UPROPERTY() TObjectPtr<class UStaticMeshComponent> Visual;
    UPROPERTY() TObjectPtr<class UPointLightComponent> Light;
    UPROPERTY() TObjectPtr<class UMaterialInstanceDynamic> Material;
    FOpenWillowFiringData Data;
    TWeakObjectPtr<class AOpenWillowWalker> Shooter;
    weapon_firing::Flight Flight;
    weapon_firing::Motion Motion;
    float Damage = 0;
    float SplashDamage = 0;
    FString DamageType;
    float MaxAge = 5;
    int32 BouncesLeft = 0;
    bool bRocket = false;
    bool bDrawStreak = true;
    FString Element;
    FVector VisualOffset = FVector::ZeroVector; // muzzle minus start, faded out over the first metres
    float Travelled = 0;
};
