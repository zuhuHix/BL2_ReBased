#pragma once

#include "CoreMinimal.h"
#include "Math/RandomStream.h"
#include "weapon_firing.hpp"

class FJsonObject;

// What one shot of a weapon does, chosen by its parts: the recipe's "firing" block, written by tools/weapon_firing.py
// from the barrel's or type's FiringModeDefinition, its ProjectileDefinition and the damage type's status effect. Any
// part list works without new code: everything here is read from that block. The maths is src/weapon_firing.* (rules in
// docs/verification/NATIVE_WEAPON_FIRING.md, UNVERIFIED in game unless WEAPON_PROJECTILES.md says otherwise).
enum class EOpenWillowShotKind : uint8
{
    Trace,   // EWWFT_HitScan, and recipes without a firing block (the older host path)
    Bullet,  // EWWFT_Bullet / Beam: a light bullet that flies at the firing mode's Speed; damage when it arrives
    Rocket,  // EWWFT_Rocket: a projectile actor from the ProjectileDefinition; damage from its explosion
};

struct FOpenWillowSplash
{
    bool bValid = false;
    float Radius = 0;            // uu, after the 0 -> 256 rule
    float Damage = 0;            // per explosion
    bool bFullDamage = false;    // bDisableRadiusDamageFalloff (weapon rockets and gyrojets; confirmed live 2026-10-23)
    weapon_firing::Falloff Falloff; // the damage type's MaxDamageRadius / MinDamageRadius / MinDamagePercent
    float DirectScale = 0;       // part of the damage dealt to what the projectile hits (Tediore throw: 0.5, observed)
    FString DamageType;
};

struct FOpenWillowStatus
{
    bool bValid = false;
    FString Definition;
    FString Type;                // STATUS_EFFECT_Burning, ...
    TMap<FString, float> ChanceBySurface; // percent: surface BaseChance x the weapon's chance modifiers
    float Duration = 0;          // s
    float Dps = 0;               // the weapon's StatusEffectDamage (damage-over-time effects only)
    float ChancePercent(const FString& Surface) const;
};

struct FOpenWillowFiringData
{
    bool bLoaded = false;
    EOpenWillowShotKind Kind = EOpenWillowShotKind::Trace;
    FString FiringMode;
    int32 Pellets = 1;
    float Spread = 0;            // degrees (WeaponSpread)
    bool bSuppressSpread = false; // bSuppressWeaponSpread: only the pattern moves the pellets
    weapon_firing::Pattern Pattern;
    float TraceRange = 16384;    // WeaponTypeDefinition.Range (trace length)
    // Light bullets.
    float BulletSpeed = 0;       // uu/s
    float AirFriction = 0;
    FVector Acceleration = FVector::ZeroVector;
    float Lifetime = 5;          // s (FiringModeDefinition.Lifetime, class default 5)
    // Rockets.
    float ProjectileSpeed = 0;   // uu/s (SpeedFormula x speed multiplier)
    float GravityScale = 0;
    float ProjectileLifetime = 0; // MaxLifespan
    bool bFalls = false;         // gravity applies (thrown guns; weapon rockets fly straight, observed live)
    float Elasticity = 0.4f;     // speed kept on a bounce (the first Bounce behaviour's elasticity)
    bool bExplodeOnImpact = false;
    int32 Bounces = 0;
    // Timing.
    int32 BurstCount = 0;        // AutomaticBurstCount; 0 = hold to fire
    float BurstInterval = 0;
    float SpinUpDuration = 0;
    float SpinDownDuration = 0;
    float SpinStartMultiplier = 1;
    bool bSpinsToFullRate = false;
    FOpenWillowSplash Splash;
    FOpenWillowStatus Status;
    FString TracerTemplate;
    TArray<FString> Unsupported;  // the tool's "unsupported": data the host does not act on (beams, child projectiles)
    // Tediore: the thrown gun on reload (reload_throw). Its damage is the card damage x the rounds left in the
    // magazine (observed live); half hits what it strikes and half is the splash (DirectScale / splash scale).
    TSharedPtr<FOpenWillowFiringData> ReloadThrow;
    // Fills Out from a recipe's "firing" object; card values (damage, spread, pellets) come from the item already read.
    static bool Parse(const FJsonObject& Firing, float CardDamage, FOpenWillowFiringData& Out);
    static void ParseProjectile(const FJsonObject& Projectile, float CardDamage, FOpenWillowFiringData& Out);
};

// Per-weapon state that the rules carry between shots.
struct FOpenWillowFiringState
{
    int32 PatternIndex = 0;      // NextFiringPatternIndex
    float SpinPercent = 0;       // BarrelSpinUpPercent
    FRandomStream Rng{0x0B12};   // host stream; the game's random sequence is not reproduced
};

// What the shots did, for the -owfiringtest report (OWFIRING result). Filled by the walker, the projectiles and the status
// effects; the test resets it per gun. The game logic never reads it.
struct FOpenWillowFiringStats
{
    int32 Shots = 0;         // trigger shots (FireWeapon calls)
    int32 Pellets = 0;       // pellet directions
    int32 Launches = 0;      // projectiles spawned
    int32 Impacts = 0;       // projectiles and traces that reached a surface or a target
    int32 TargetHits = 0;    // direct hits on a combat target (a rocket's direct impact included, with no direct damage)
    int32 SplashHits = 0;    // explosion damage applications to combat targets
    int32 StatusRolls = 0;   // status-effect rolls (direct and splash hits)
    int32 Procs = 0;         // rolls that applied the effect
    int32 Ticks = 0;         // damage-over-time ticks that dealt damage
    float Damage = 0;        // all damage passed to combat targets (direct, splash and ticks)
    float Direct = 0;        // direct hit damage
    float Splash = 0;        // explosion damage (per target: damage x the radius scale)
    void Reset() { *this = FOpenWillowFiringStats(); }
};

namespace OpenWillowFiring
{
inline FOpenWillowFiringStats Counts;
// One direction per pellet from the aim: pattern offset, then spread, both in the aim's local frame.
void PelletDirections(const FOpenWillowFiringData& Data, float SpreadDegrees, int32 Pellets, const FRotator& Aim,
    FOpenWillowFiringState& State, TArray<FVector>& Out);
// Seconds to the next shot from the card's fire interval and the barrel spin-up.
float FireInterval(const FOpenWillowFiringData& Data, float BaseInterval, const FOpenWillowFiringState& State);
// Host colour of an element's muzzle flash, bullet streak and impact (a stand-in for the per-element particle
// templates; picked by eye against the 2026-10-23 real frames). White-ish orange for no element.
FLinearColor ElementColor(const FString& Element);
bool IsElemental(const FString& Element);
// World gravity for falling projectiles (uu/s^2): the Tediore pistol throw fell at 850 = 0.85 (its GravityScaling)
// x 1000 in Ice_P (live, 2026-10-23); whether every map uses 1000 is UNVERIFIED.
constexpr float WorldGravityZ = -1000.f;
}
