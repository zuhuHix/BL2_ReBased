#pragma once

#include "CoreMinimal.h"
#include "Engine/DamageEvents.h"
#include "UObject/Interface.h"
#include "OpenWillowHitEvent.generated.h"

// One hit of one of Maya's shots, the single shape every receiver reads (WEAPON_PROJECTILES.md section 9).
//
// The shooter fills it and sends it as the DamageEvent of the target's own TakeDamage (an FOpenWillowDamageEvent, which is
// also an FPointDamageEvent, so code that reads point damage keeps working). After TakeDamage returns, the same event goes
// to every listener of OpenWillowHit::OnHit() with what the target reported (applied damage, critical).
//
// Game side (what each field stands for): UE3 TakeDamage(Damage, InstigatedBy, HitLocation, Momentum, DamageType, HitInfo,
// DamageCauser). In BL2 the victim decides critical hits: its hit region from the bone, critical only for a region with
// bCriticalHit and never for status-effect damage, then x the instigator's CurrentInstantHitCriticalHitBonus (base 2.0,
// confirmed live in COMBAT_GOLDEN_TRACE.md). So the shooter only passes the bonus, and the target sets the result.

enum class EOpenWillowHitKind : uint8
{
    Direct,      // a bullet, trace pellet or rocket body that reached the target
    Splash,      // an explosion's radius damage (rocket, gyrojet, elemental impact explosion, Tediore throw splash)
    StatusTick,  // a damage-over-time tick of a status effect (never critical)
    Throw,       // the direct half of a thrown Tediore gun
};

// What dealt the damage, independent of Kind (a rocket's explosion is Source Rocket with Kind Splash). In the running
// game the light bullet's own hit arrives as a weapon-type damage source (WillowDmgSource_Pistol, _Shotgun ...), a rocket
// as WillowDmgSource_Rocket, and a bullet's impact explosion (Maliwan, Torgue gyrojet) as WillowDmgSource_Grenade
// (PROJECTILE_FIRING_PATTERNS_LIVE R6, R7; WEAPON_PROJECTILES.md section 10): those map to Bullet, Rocket and Explosion.
enum class EOpenWillowHitSource : uint8
{
    Unknown,     // the default: a caller that does not say
    Bullet,      // a light bullet or trace pellet hitting what it struck
    Rocket,      // a rocket projectile, its direct hit and its explosion
    Explosion,   // an explosion that is not a rocket's: a bullet's impact explosion (Maliwan, gyrojet)
    Throw,       // a thrown Tediore gun, its strike and its explosion
    Status,      // a status effect's damage over time
    Skill,       // a skill's own damage (set by the skills lane)
    Grenade,     // a thrown grenade mod's projectile and its explosion
};

enum class EOpenWillowElement : uint8
{
    None,
    Fire,        // GD_Incendiary damage types
    Shock,
    Corrosive,
    Slag,        // GD_Amp damage types
    Explosive,
};

struct FOpenWillowHitEvent
{
    // Who.
    TWeakObjectPtr<AActor> Instigator;   // the shooting pawn (Maya)
    TWeakObjectPtr<AActor> Target;
    FString WeaponId;                    // the fired item's id (recipe stem); "" when unknown
    EOpenWillowHitKind Kind = EOpenWillowHitKind::Direct;
    EOpenWillowHitSource Source = EOpenWillowHitSource::Unknown;
    // Where.
    FVector Location = FVector::ZeroVector;   // impact point (for splash: the target's centre, see Origin)
    FVector Origin = FVector::ZeroVector;     // the explosion centre for splash, else the shot start
    FVector Direction = FVector::ZeroVector;  // unit shot (or explosion -> target) direction; zero for status ticks
    FName Bone = NAME_None;                   // bone of the hit, when the hit component has bones
    // What.
    float Damage = 0;                    // before the target's own scaling (crit, resistances)
    FString DamageType;                  // stock damage type object path (e.g. GD_Shock.DamageType.DmgType_Shock_Impact)
    EOpenWillowElement Element = EOpenWillowElement::None;
    FVector Momentum = FVector::ZeroVector;   // UE3 Momentum: direction x InstantHitMomentum x 100 (bullets), splash x scale
    float RadiusScale = 1;               // splash: the falloff scale already in Damage and Momentum; 1 otherwise
    bool bCanCrit = true;                // false for status ticks and splash (BL2: no crit for status damage; splash UNVERIFIED)
    float CritBonus = 2.f;               // the instigator's CurrentInstantHitCriticalHitBonus (base 2.0, live)
    double Time = 0;                     // world seconds
    // Result, written by the target inside TakeDamage (mutable: the engine passes the event as const).
    mutable bool bCritical = false;
    mutable float Applied = -1;          // -1 until the shooter stores TakeDamage's return value
};

// The damage event that carries it. Receivers: `if (E.IsOfType(FOpenWillowDamageEvent::ClassID))
// { const auto& Hit = static_cast<const FOpenWillowDamageEvent&>(E).Hit; ... }`.
struct FOpenWillowDamageEvent : public FPointDamageEvent
{
    FOpenWillowHitEvent Hit;
    static const int32 ClassID = 0x4F57;    // "OW"; FPointDamageEvent is 1, FRadialDamageEvent 2
    virtual int32 GetTypeID() const override { return ClassID; }
    virtual bool IsOfType(int32 InID) const override { return InID == ClassID || FPointDamageEvent::IsOfType(InID); }
};

// Opt-in for actors that take splash and status effects from Maya's shots (the host dummy, range's enemies). Direct
// bullets and traces damage any actor they hit; explosions only look for actors that implement this.
UINTERFACE(MinimalAPI)
class UOpenWillowHitTarget : public UInterface
{
    GENERATED_BODY()
};

class OPENWILLOW_API IOpenWillowHitTarget
{
    GENERATED_BODY()
public:
    // The point splash distance is measured to (the game uses the first radius-damage hit region's bone) and the radius the
    // status chance falloff subtracts (the collision cylinder's). Default: the actor's simple collision cylinder.
    virtual void GetHitCentre(FVector& OutCentre, float& OutRadius) const;
    // Experience level for the status chance level-difference scale; 0 = unknown (counts as the shooter's level).
    virtual int32 GetHitLevel() const { return 0; }
    // False while the target cannot take damage (dead, despawned); explosions skip it.
    virtual bool CanTakeHits() const { return true; }
};

DECLARE_MULTICAST_DELEGATE_OneParam(FOnOpenWillowHit, const FOpenWillowHitEvent&);

namespace OpenWillowHit
{
// Every hit after the target's TakeDamage returned (HUD numbers, AI reactions, test counters).
OPENWILLOW_API FOnOpenWillowHit& OnHit();
// Element of a stock damage type path; Fallback (the item's "Element") when the path names none.
OPENWILLOW_API EOpenWillowElement ElementOf(const FString& DamageType, const FString& Fallback = FString());
OPENWILLOW_API const TCHAR* ElementName(EOpenWillowElement Element);
OPENWILLOW_API const TCHAR* KindName(EOpenWillowHitKind Kind);
OPENWILLOW_API const TCHAR* SourceName(EOpenWillowHitSource Source);
// The actor as a splash/status target, or null.
OPENWILLOW_API IOpenWillowHitTarget* AsTarget(AActor* Actor);
}
