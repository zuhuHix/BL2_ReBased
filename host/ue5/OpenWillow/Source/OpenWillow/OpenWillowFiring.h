#pragma once

#include "CoreMinimal.h"
#include "Math/RandomStream.h"
#include "CollisionQueryParams.h"
#include "weapon_firing.hpp"
#include "OpenWillowShotFx.h"

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
    float Momentum = 0;          // MomentumFormula; each target gets radius scale x this along explosion -> target (3.1)
    FString DamageType;
};

// One ExplosionDefinition.ExplosionScale band (tools/weapon_projectile_fx.py): the band whose radius range holds the
// explosion's radius plays its particle system, at a particle scale lerped across the band (UNVERIFIED: read from the
// field names, not from the native).
struct FOpenWillowExplosionBand
{
    float MinRadius = 0, MaxRadius = 0;
    float MinScale = 1, MaxScale = 1;
    TArray<FOpenWillowFlashLayer> Layers;
};

struct FOpenWillowStatus
{
    bool bValid = false;
    FString Definition;
    FString Type;                // STATUS_EFFECT_Burning, ...
    TMap<FString, float> ChanceBySurface; // percent: surface BaseChance x the weapon's chance modifiers
    float Duration = 0;          // s
    float Dps = 0;               // the weapon's StatusEffectDamage (damage-over-time effects only)
    // GlobalsDefinition.StatusEffectChanceScales (status.level_chance_scales): the chance factor by level difference.
    std::vector<weapon_firing::LevelChanceScale> LevelScales;
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
    // Momentum of a bullet or trace hit: WeaponTypeDefinition.InstantHitMomentum x 100, along the ray (confirmed live:
    // 800 / 700 / 1,000 for types with 8 / 7 / 10, PROJECTILE_FIRING_PATTERNS_LIVE R6). Without a value in the block:
    // the class default 10 (x 100), UNVERIFIED for that case.
    float Momentum = 1000;
    // FiringModeDefinition.TraceExtent: half extents of the axis-aligned box each bullet segment is swept with; zero
    // sweeps a line. The light-projectile manager sweeps with it (NATIVE_WEAPON_TRIGGER_AMMO); box shape and axes UNVERIFIED.
    FVector TraceExtent = FVector::ZeroVector;
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
    float FuseTime = 0;          // s from launch to the explosion (grenades: the spawn timer); 0 = no fuse
    float ContactFuse = 0;       // s from the first contact to the explosion (grenades); 0 = contact starts nothing
    float TargetElasticity = 0.2f; // speed kept on a bounce off a damagable object (grenades); Elasticity is off the world
    float UpwardVelocity = 0;    // uu/s added to the launch velocity's Z (grenades)
    bool bExplodeOnImpact = false;
    int32 Bounces = 0;
    // Timing.
    int32 BurstCount = 0;        // AutomaticBurstCount; 0 = hold to fire
    bool bBurstFromManifest = false; // a generated gun whose burst values came from the manifest (older manifests lack them)
    float BurstInterval = 0;
    float SpinUpDuration = 0;
    float SpinDownDuration = 0;
    float SpinStartMultiplier = 1;
    bool bSpinsToFullRate = false;
    // The player's accuracy pool while this weapon is held (accuracy_pool) and the shot's impulse and kick (recoil).
    bool bHasAccuracyPool = false;  // accuracy_pool present: the shot's impulse feeds the pool and its cone
    bool bHasKick = false;          // recoil present: the shot kicks the view (independent of the pool)
    bool bPoolStartsAtMax = true;
    weapon_firing::AccuracyPool Pool;
    float AccuracyImpulse = 0;      // PerShotAccuracyImpulse (degrees; negative on reverse-recoil types)
    float BurstImpulseScale = 1;    // BurstShotAccuracyImpulseScale inside an automatic burst
    weapon_firing::KickData KickData;
    FOpenWillowSplash Splash;
    FOpenWillowStatus Status;
    FString TracerTemplate;
    // Cooked particle layers (the recipe's fx block): the rocket's in-flight trail and the explosion's bands.
    TArray<FOpenWillowFlashLayer> TrailLayers;
    float TrailScale = 1;
    TArray<FOpenWillowExplosionBand> ExplosionBands;
    // The band for an explosion of this radius (the one whose range holds it, else the nearest), and its particle scale.
    const FOpenWillowExplosionBand* BandFor(float Radius, float& OutScale) const;
    TArray<FString> Unsupported;  // the tool's "unsupported": data the host does not act on (beams, child projectiles)
    // Tediore: the thrown gun on reload (reload_throw). Its damage is the card damage x the rounds left in the
    // magazine (observed live); half hits what it strikes and half is the splash (DirectScale / splash scale).
    TSharedPtr<FOpenWillowFiringData> ReloadThrow;
    // Fills Out from a recipe's "firing" object; card values (damage, spread, pellets) come from the item already read.
    static bool Parse(const FJsonObject& Firing, float CardDamage, FOpenWillowFiringData& Out);
    static void ParseProjectile(const FJsonObject& Projectile, float CardDamage, FOpenWillowFiringData& Out);
    // A grenade mod (tools/grenade_mod.py "grenade" block): a falling projectile with the mod's speed, gravity, bounces,
    // fuse, radius and damage, and its explosion bands. FiringMode is "grenade".
    static bool ParseGrenade(const FJsonObject& Grenade, FOpenWillowFiringData& Out);
    // False with what is missing when the block cannot fire as its kind: a positive finite speed for a rocket's projectile, a
    // light bullet and a thrown gun, and a firing mode. tools/weapon_gen_firing.py incomplete() is the same rule for its templates.
    bool CompleteForShotKind(FString& OutMissing) const;
};

// Per-weapon state that the rules carry between shots.
struct FOpenWillowFiringState
{
    int32 PatternIndex = 0;      // NextFiringPatternIndex
    float SpinPercent = 0;       // BarrelSpinUpPercent
    FRandomStream Rng{0x0B12};   // host stream; the game's random sequence is not reproduced
};

// The player's accuracy pool and view kick between shots (one per shooter). Live, 2026-11-03 (WEAPON_PROJECTILES.md
// section 10): the pool holds 0.2 s after a shot and then falls at its idle rate; the kick is impulse x 182.0444 units, its
// targets fall linearly to 0 over WeaponKickRecoveryTime and the current amounts chase them at WeaponKickSpeed per second;
// the controller rotation itself never moves, so the kick is a view offset that returns.
struct FOpenWillowAimState
{
    weapon_firing::AccuracyPool Pool;
    weapon_firing::Kick Kick;
    FString Weapon;              // instance id whose pool limits are in Pool
    bool bStarted = false;       // StartWithMaxValue applies once, at the first weapon
    double AppliedPitch = 0;     // kick already added to the view (rotator units)
    double AppliedYaw = 0;
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
    int32 MomentumHits = 0;  // damage applications to combat targets that carried momentum
    int32 Pushes = 0;        // ... whose target velocity change equalled momentum / mass
    float LastMomentum = 0;  // length of the last momentum passed to a combat target
    int32 TimedImpacts = 0;  // constant-speed light bullets that reached a surface or target
    int32 OnTimeImpacts = 0; // ... after distance / speed seconds (to within one frame)
    int32 HitEvents = 0;     // hit events sent (ApplyShotDamage; any target)
    int32 HitKinds[4] = {0, 0, 0, 0}; // ... by EOpenWillowHitKind: direct, splash, status tick, throw
    TMap<FString, int32> TickWeapons;  // status tick events by the weapon id they carry
    TMap<FString, int32> ImpactsByWeapon;   // projectiles that reached a surface or target, by the fired weapon's id
    TMap<FString, float> DamageByWeapon;    // damage passed to combat targets, by the weapon id its hit event carries
    int32 UnknownSources = 0;          // hit events whose Source was left Unknown (every firing path sets one)
    int32 GrenadeHits = 0;             // hit events whose Source is Grenade
    float GrenadeContactAge = -1;      // s from the launch to the grenade's first contact; -1 = none
    float GrenadeFuseAge = -1;         // s from the launch to the grenade's explosion; -1 = none
    FString GrenadeFirstContact;       // what the grenade touched first; empty = nothing
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
// One bullet or trace segment: a line trace, or a sweep of the firing mode's TraceExtent box (axis-aligned) when it has
// one, so a wide bullet hits what passes beside its centre line.
bool SweepShot(const UWorld* World, FHitResult& Hit, const FVector& From, const FVector& To, const FVector& Extent,
    const FCollisionQueryParams& Query);
bool IsElemental(const FString& Element);
// Once per frame: the held weapon's pool limits (a new weapon clamps the value into them), the pool's idle fall and the
// kick's decay and chase. Returns the view change to add this frame (degrees, pitch and yaw).
FRotator TickAim(FOpenWillowAimState& State, const FOpenWillowFiringData* Held, const FString& HeldInstance, float Dt);
// Whether the accuracy pool and kick act on the player's aim: off in every scripted test or capture mode (any -ow...shots,
// -ow...test or -owautowalk switch), so their aim stays where the script put it; -owaimmodel turns it on again.
bool AimModelEnabled();
// The shot's aim: the view plus one draw in the pool's cone (x 1 - zoom), shared by every pellet of the shot.
FRotator ShotAim(const FOpenWillowAimState& State, const FRotator& View, float ZoomEffect, FRandomStream& Rng);
// After a shot: the pool impulse (x the burst scale inside an automatic burst) and the view kick.
void OnShot(FOpenWillowAimState& State, const FOpenWillowFiringData& Data, float ZoomEffect, float FovRatio, bool bBurstShot,
    FRandomStream& Rng);
// World gravity for falling projectiles (uu/s^2): the Tediore pistol throw fell at 850 = 0.85 (its GravityScaling)
// x 1000 in Ice_P (live, 2026-10-23); whether every map uses 1000 is UNVERIFIED.
constexpr float WorldGravityZ = -1000.f;
}
