#include "OpenWillowFiring.h"

#include "Dom/JsonObject.h"
#include "Misc/CommandLine.h"
#include "Misc/Parse.h"
#include "Engine/World.h"

namespace
{
double NumberOr(const FJsonObject& Object, const TCHAR* Field, double Fallback)
{
    double Value = 0;
    return Object.TryGetNumberField(Field, Value) && FMath::IsFinite(Value) ? Value : Fallback;
}
const FJsonObject* ObjectOrNull(const FJsonObject& Object, const TCHAR* Field)
{
    const TSharedPtr<FJsonObject>* Child = nullptr;
    return Object.TryGetObjectField(Field, Child) && Child->IsValid() ? Child->Get() : nullptr;
}
int32 RotatorField(const FJsonObject* Rotator, const TCHAR* Field)
{
    return Rotator ? int32(NumberOr(*Rotator, Field, 0)) : 0;
}
}

float FOpenWillowStatus::ChancePercent(const FString& Surface) const
{
    if (const float* Chance = ChanceBySurface.Find(Surface)) return *Chance;
    const float* Generic = ChanceBySurface.Find(TEXT("generic"));
    return Generic ? *Generic : 0.f;
}

// A Behavior_Explode block (a projectile's "splash" or a firing mode's "impact_explosion") into Out.
static void ParseSplash(const FJsonObject& Splash, float CardDamage, FOpenWillowSplash& Out)
{
    Out.bValid = true;
    Out.Radius = float(weapon_firing::explosionRadius(NumberOr(Splash, TEXT("radius"), 0)));
    // An evaluated damage when the tool resolved one; else the damage formula's attribute scaled by its constant. A
    // rocket's ProjectileDamage follows the weapon's damage through the type's ProjectileBaseValues
    // (NATIVE_WEAPON_FIRING 2.1): the live Bandit rocket's splash equalled its card damage (285.84, 2026-10-23).
    double Damage = 0;
    if (!Splash.TryGetNumberField(TEXT("damage"), Damage) || !FMath::IsFinite(Damage))
        Damage = CardDamage * NumberOr(Splash, TEXT("damage_scale"), 1);
    Out.Damage = float(Damage);
    // "falloff" is !bDisableRadiusDamageFalloff of the Behavior_Explode; false means full damage everywhere in the
    // radius (weapon rockets and gyrojets, confirmed live). The bands are the damage type's.
    bool bFalloff = true;
    Splash.TryGetBoolField(TEXT("falloff"), bFalloff);
    Out.bFullDamage = !bFalloff;
    if (const FJsonObject* Bands = ObjectOrNull(Splash, TEXT("falloff_bands")))
    {
        Out.Falloff.maxDamageRadius = NumberOr(*Bands, TEXT("MaxDamageRadius"), Out.Falloff.maxDamageRadius);
        Out.Falloff.minDamageRadius = NumberOr(*Bands, TEXT("MinDamageRadius"), Out.Falloff.minDamageRadius);
        Out.Falloff.minDamagePercent = NumberOr(*Bands, TEXT("MinDamagePercent"), Out.Falloff.minDamagePercent);
    }
    Splash.TryGetStringField(TEXT("damage_type"), Out.DamageType);
    Out.Momentum = float(NumberOr(Splash, TEXT("momentum"), 0));
}

void FOpenWillowFiringData::ParseProjectile(const FJsonObject& Projectile, float CardDamage, FOpenWillowFiringData& Out)
{
    // Own-motion bullets (gyrojets) keep their light-bullet speed; AirFriction and Acceleration change it in flight.
    Out.AirFriction = float(NumberOr(Projectile, TEXT("air_friction"), 0));
    if (const FJsonObject* Accel = ObjectOrNull(Projectile, TEXT("acceleration")))
        Out.Acceleration = FVector(NumberOr(*Accel, TEXT("X"), 0), NumberOr(*Accel, TEXT("Y"), 0), NumberOr(*Accel, TEXT("Z"), 0));
    Out.Lifetime = float(NumberOr(Projectile, TEXT("lifetime"), 5));
    Out.ProjectileSpeed = float(NumberOr(Projectile, TEXT("speed"), 0));
    Out.GravityScale = float(NumberOr(Projectile, TEXT("gravity_scale"), 0));
    Out.ProjectileLifetime = float(NumberOr(Projectile, TEXT("max_lifespan"), NumberOr(Projectile, TEXT("lifetime"), 5)));
    Projectile.TryGetBoolField(TEXT("explode_on_impact"), Out.bExplodeOnImpact);
    if (const FJsonObject* Bounces = ObjectOrNull(Projectile, TEXT("bounces")))
    {
        Out.Bounces = FMath::Max(0, int32(NumberOr(*Bounces, TEXT("count"), 0)));
        const TArray<TSharedPtr<FJsonValue>>* Elasticity = nullptr;
        double First = 0;
        if (Bounces->TryGetArrayField(TEXT("elasticity"), Elasticity) && Elasticity->Num() > 0 && (*Elasticity)[0].IsValid()
            && (*Elasticity)[0]->TryGetNumber(First))
            Out.Elasticity = float(First);
    }
    if (const FJsonObject* Splash = ObjectOrNull(Projectile, TEXT("splash"))) ParseSplash(*Splash, CardDamage, Out.Splash);
}

namespace
{
// The block's "fx" (tools/weapon_projectile_fx.py): the trail's layers and scale, the explosion's bands.
void ParseFx(const FJsonObject& Block, FOpenWillowFiringData& Out)
{
    const FJsonObject* Fx = ObjectOrNull(Block, TEXT("fx"));
    if (!Fx) return;
    const TArray<TSharedPtr<FJsonValue>>* Rows = nullptr;
    if (const FJsonObject* Trail = ObjectOrNull(*Fx, TEXT("trail")))
    {
        if (Trail->TryGetArrayField(TEXT("layers"), Rows)) AOpenWillowShotFx::ParseLayers(*Rows, Out.TrailLayers);
        Out.TrailScale = float(NumberOr(*Trail, TEXT("scale"), 1));
    }
    const FJsonObject* Explosion = ObjectOrNull(*Fx, TEXT("explosion"));
    const TArray<TSharedPtr<FJsonValue>>* Bands = nullptr;
    if (Explosion && Explosion->TryGetArrayField(TEXT("bands"), Bands))
        for (const TSharedPtr<FJsonValue>& Value : *Bands)
        {
            const TSharedPtr<FJsonObject>* Band = nullptr;
            if (!Value.IsValid() || !Value->TryGetObject(Band) || !Band || !Band->IsValid()) continue;
            FOpenWillowExplosionBand Entry;
            Entry.MinRadius = float(NumberOr(**Band, TEXT("min_radius"), 0));
            Entry.MaxRadius = float(NumberOr(**Band, TEXT("max_radius"), 0));
            Entry.MinScale = float(NumberOr(**Band, TEXT("min_scale"), 1));
            Entry.MaxScale = float(NumberOr(**Band, TEXT("max_scale"), 1));
            if ((*Band)->TryGetArrayField(TEXT("layers"), Rows)) AOpenWillowShotFx::ParseLayers(*Rows, Entry.Layers);
            if (!Entry.Layers.IsEmpty()) Out.ExplosionBands.Add(MoveTemp(Entry));
        }
}
}

const FOpenWillowExplosionBand* FOpenWillowFiringData::BandFor(float Radius, float& OutScale) const
{
    OutScale = 1.f;
    const FOpenWillowExplosionBand* Best = nullptr;
    float BestGap = TNumericLimits<float>::Max();
    for (const FOpenWillowExplosionBand& Band : ExplosionBands)
    {
        const float Gap = Radius < Band.MinRadius ? Band.MinRadius - Radius : Radius > Band.MaxRadius ? Radius - Band.MaxRadius : 0.f;
        if (Gap < BestGap) { BestGap = Gap; Best = &Band; }
    }
    if (!Best) return nullptr;
    const float Span = Best->MaxRadius - Best->MinRadius;
    const float T = Span > 0.f ? FMath::Clamp((Radius - Best->MinRadius) / Span, 0.f, 1.f) : 1.f;
    OutScale = FMath::Lerp(Best->MinScale, Best->MaxScale, T);
    return Best;
}

bool FOpenWillowFiringData::ParseGrenade(const FJsonObject& Grenade, FOpenWillowFiringData& Out)
{
    Out = FOpenWillowFiringData();
    const FJsonObject* Projectile = ObjectOrNull(Grenade, TEXT("projectile"));
    const FJsonObject* Radius = ObjectOrNull(Grenade, TEXT("radius"));
    const FJsonObject* Damage = ObjectOrNull(Grenade, TEXT("damage"));
    if (!Projectile || !Radius || !Damage) return false;
    Out.Kind = EOpenWillowShotKind::Rocket;
    Out.FiringMode = TEXT("grenade");
    Out.ProjectileSpeed = float(NumberOr(*Projectile, TEXT("speed"), 0));
    Out.GravityScale = float(NumberOr(*Projectile, TEXT("gravity_scale"), 1));
    Out.bFalls = true;   // PHYS_Falling
    Out.Bounces = FMath::Max(0, int32(NumberOr(*Projectile, TEXT("bounces"), 0)));
    Projectile->TryGetBoolField(TEXT("explode_on_impact"), Out.bExplodeOnImpact);
    Out.ProjectileLifetime = float(NumberOr(*Projectile, TEXT("max_lifespan"), 0));
    // ProjectileDefinition.UpwardVelocityBonus: added to the launch velocity's Z (NATIVE_WEAPON_FIRING 2.1, read; UNVERIFIED).
    Out.UpwardVelocity = float(NumberOr(*Projectile, TEXT("upward_velocity_bonus"), 0));
    if (const FJsonObject* Fuse = ObjectOrNull(Grenade, TEXT("fuse_time"))) Out.FuseTime = float(NumberOr(*Fuse, TEXT("value"), 0));
    Out.Splash.bValid = true;
    Out.Splash.Radius = float(weapon_firing::explosionRadius(NumberOr(*Radius, TEXT("value"), 0)));
    Out.Splash.Damage = float(NumberOr(*Damage, TEXT("value"), 0));
    Grenade.TryGetStringField(TEXT("damage_type"), Out.Splash.DamageType);
    // The grenade's falloff bands are not in the block: the class defaults (0.2 / 0.8 / 0.2) stand in (UNVERIFIED).
    ParseFx(Grenade, Out);
    // A grenade without positive damage does not load: an unresolved value (null in the block) must not throw a harmless one.
    Out.bLoaded = Out.ProjectileSpeed > 0.f && Out.Splash.Radius > 0.f && Out.Splash.Damage > 0.f;
    return Out.bLoaded;
}

bool FOpenWillowFiringData::Parse(const FJsonObject& Firing, float CardDamage, FOpenWillowFiringData& Out)
{
    Out = FOpenWillowFiringData();
    if (NumberOr(Firing, TEXT("schema"), 0) != 1) return false;
    Firing.TryGetStringField(TEXT("firing_mode"), Out.FiringMode);
    FString FireType;
    Firing.TryGetStringField(TEXT("fire_type"), FireType);
    // FireType decides (NATIVE_WEAPON_FIRING 1.1); cooked data omits the class default, EWWFT_Bullet. Beams are not
    // supported: they fire light bullets here and the recipe lists the gap in "unsupported".
    if (FireType == TEXT("EWWFT_HitScan")) Out.Kind = EOpenWillowShotKind::Trace;
    else if (FireType == TEXT("EWWFT_Rocket")) Out.Kind = EOpenWillowShotKind::Rocket;
    else Out.Kind = EOpenWillowShotKind::Bullet;
    Out.Pellets = FMath::Max(1, int32(NumberOr(Firing, TEXT("pellets"), 1)));
    Out.Spread = float(NumberOr(Firing, TEXT("spread"), 0));
    Out.BulletSpeed = float(NumberOr(Firing, TEXT("bullet_speed"), 0));
    // Accuracy pool while the weapon is held, and the type's kick (NATIVE_WEAPON_TRIGGER_AMMO sections 4-5, UNVERIFIED).
    if (const FJsonObject* Pool = ObjectOrNull(Firing, TEXT("accuracy_pool")))
    {
        Out.bHasAccuracyPool = true;
        Out.Pool.minValue = NumberOr(*Pool, TEXT("min"), Out.Pool.minValue);
        Out.Pool.maxValue = NumberOr(*Pool, TEXT("max"), Out.Pool.maxValue);
        Out.Pool.idleRate = NumberOr(*Pool, TEXT("idle_rate"), Out.Pool.idleRate);
        Out.Pool.idleDelay = NumberOr(*Pool, TEXT("idle_delay"), Out.Pool.idleDelay);
        Pool->TryGetBoolField(TEXT("start_with_max"), Out.bPoolStartsAtMax);
    }
    if (const FJsonObject* Recoil = ObjectOrNull(Firing, TEXT("recoil")))
    {
        Out.bHasKick = true;
        Out.AccuracyImpulse = float(NumberOr(*Recoil, TEXT("per_shot_accuracy_impulse"), 0));
        Out.BurstImpulseScale = float(NumberOr(*Recoil, TEXT("burst_accuracy_impulse_scale"), 1));
        Out.KickData.up = NumberOr(*Recoil, TEXT("kick_up"), Out.KickData.up);
        Out.KickData.down = NumberOr(*Recoil, TEXT("kick_down"), Out.KickData.down);
        Out.KickData.left = NumberOr(*Recoil, TEXT("kick_left"), Out.KickData.left);
        Out.KickData.right = NumberOr(*Recoil, TEXT("kick_right"), Out.KickData.right);
        Out.KickData.minVertical = NumberOr(*Recoil, TEXT("min_vertical"), Out.KickData.minVertical);
        Out.KickData.minHorizontal = NumberOr(*Recoil, TEXT("min_horizontal"), Out.KickData.minHorizontal);
        Out.KickData.speed = NumberOr(*Recoil, TEXT("kick_speed"), Out.KickData.speed);
        Out.KickData.recoveryTime = NumberOr(*Recoil, TEXT("kick_recovery_time"), Out.KickData.recoveryTime);
        Out.KickData.zoomMultiplier = NumberOr(*Recoil, TEXT("kick_zoom_multiplier"), Out.KickData.zoomMultiplier);
    }
    Out.BurstCount = FMath::Max(0, int32(NumberOr(Firing, TEXT("burst_count"), 0)));
    Out.BurstInterval = float(NumberOr(Firing, TEXT("burst_interval"), 0));
    Out.SpinUpDuration = float(NumberOr(Firing, TEXT("spin_up"), 0));
    Firing.TryGetStringField(TEXT("tracer_template"), Out.TracerTemplate);
    Firing.TryGetStringArrayField(TEXT("unsupported"), Out.Unsupported);
    Firing.TryGetBoolField(TEXT("pattern_fire_randomly"), Out.Pattern.fireRandomly);
    const TArray<TSharedPtr<FJsonValue>>* Lines = nullptr;
    if (Firing.TryGetArrayField(TEXT("pattern_lines"), Lines))
        for (const TSharedPtr<FJsonValue>& Value : *Lines)
        {
            const TSharedPtr<FJsonObject>* Line = nullptr;
            if (!Value.IsValid() || !Value->TryGetObject(Line)) continue;
            const FJsonObject* Start = ObjectOrNull(**Line, TEXT("StartPoint"));
            const FJsonObject* End = ObjectOrNull(**Line, TEXT("EndPoint"));
            weapon_firing::PatternLine Entry;
            Entry.startPitch = RotatorField(Start, TEXT("Pitch"));
            Entry.startYaw = RotatorField(Start, TEXT("Yaw"));
            Entry.endPitch = RotatorField(End, TEXT("Pitch"));
            Entry.endYaw = RotatorField(End, TEXT("Yaw"));
            (*Line)->TryGetBoolField(TEXT("bUseStartPointOnly"), Entry.startOnly);
            Out.Pattern.lines.push_back(Entry);
        }
    if (const FJsonObject* Projectile = ObjectOrNull(Firing, TEXT("projectile"))) ParseProjectile(*Projectile, CardDamage, Out);
    ParseFx(Firing, Out);
    // A trace or light-bullet firing mode's own Behavior_Explode (OnAnyImpact): it explodes on every impact, the bullet's
    // own damage first (native read 2026-11-03, WEAPON_PROJECTILES.md section 3; the Maliwan pistol's 0.8 x matches live).
    if (!Out.Splash.bValid)
        if (const FJsonObject* Impact = ObjectOrNull(Firing, TEXT("impact_explosion"))) ParseSplash(*Impact, CardDamage, Out.Splash);
    Out.SpinDownDuration = float(NumberOr(Firing, TEXT("spin_down"), 0));
    Out.TraceRange = float(NumberOr(Firing, TEXT("trace_range"), 16384));
    Out.Momentum = float(weapon_firing::hitMomentum(NumberOr(Firing, TEXT("instant_hit_momentum"), 10)));
    const TArray<TSharedPtr<FJsonValue>>* Extent = nullptr;
    if (Firing.TryGetArrayField(TEXT("trace_extent"), Extent) && Extent->Num() == 3)
        for (int32 Axis = 0; Axis < 3; ++Axis)
        {
            double Value = 0;
            if ((*Extent)[Axis].IsValid() && (*Extent)[Axis]->TryGetNumber(Value) && FMath::IsFinite(Value))
                Out.TraceExtent[Axis] = FMath::Max(0.f, float(Value));
        }
    Firing.TryGetBoolField(TEXT("suppress_spread"), Out.bSuppressSpread);
    Firing.TryGetBoolField(TEXT("pattern_scale_by_spread"), Out.Pattern.scaleBySpread);
    Out.Pattern.basePatternSpread = NumberOr(Firing, TEXT("pattern_base_spread"), 1);
    Out.Pattern.minPatternScale = NumberOr(Firing, TEXT("pattern_min_scale"), 0);
    if (const FJsonObject* Throw = ObjectOrNull(Firing, TEXT("reload_throw")))
        if (const FJsonObject* Thrown = ObjectOrNull(*Throw, TEXT("projectile")))
        {
            // The thrown gun falls (live) and deals half its damage to what it strikes (live, 2026-10-23).
            Out.ReloadThrow = MakeShared<FOpenWillowFiringData>();
            Out.ReloadThrow->Kind = EOpenWillowShotKind::Rocket;
            Out.ReloadThrow->FiringMode = TEXT("reload_throw");
            ParseProjectile(*Thrown, CardDamage, *Out.ReloadThrow);
            Out.ReloadThrow->bFalls = true;
            Out.ReloadThrow->Splash.DirectScale = 0.5f;
            ParseFx(*Throw, *Out.ReloadThrow);
            Out.ReloadThrow->bLoaded = true;
        }
    if (const FJsonObject* Status = ObjectOrNull(Firing, TEXT("status")))
    {
        Out.Status.bValid = true;
        Status->TryGetStringField(TEXT("definition"), Out.Status.Definition);
        Status->TryGetStringField(TEXT("type"), Out.Status.Type);
        Out.Status.Duration = float(NumberOr(*Status, TEXT("duration"), 0));
        Out.Status.Dps = float(NumberOr(*Status, TEXT("dps"), 0));
        if (const FJsonObject* Surfaces = ObjectOrNull(*Status, TEXT("chance_by_surface")))
            for (const auto& Pair : Surfaces->Values)
            {
                double Chance = 0;
                if (Pair.Value.IsValid() && Pair.Value->TryGetNumber(Chance) && FMath::IsFinite(Chance))
                    Out.Status.ChanceBySurface.Add(FString(Pair.Key), float(Chance));
            }
        const TArray<TSharedPtr<FJsonValue>>* Rows = nullptr;
        if (Status->TryGetArrayField(TEXT("level_chance_scales"), Rows))
            for (const TSharedPtr<FJsonValue>& Value : *Rows)
            {
                const TSharedPtr<FJsonObject>* Row = nullptr;
                if (!Value.IsValid() || !Value->TryGetObject(Row)) continue;
                weapon_firing::LevelChanceScale Scale;
                Scale.levelDifference = int32(NumberOr(**Row, TEXT("LevelDifference"), 0));
                Scale.higher = NumberOr(**Row, TEXT("HigherLevelChanceScale"), 1);
                Scale.lower = NumberOr(**Row, TEXT("LowerLevelChanceScale"), 1);
                Out.Status.LevelScales.push_back(Scale);
            }
    }
    Out.bLoaded = true;
    return true;
}

void OpenWillowFiring::PelletDirections(const FOpenWillowFiringData& Data, float SpreadDegrees, int32 Pellets,
    const FRotator& Aim, FOpenWillowFiringState& State, TArray<FVector>& Out)
{
    Out.Reset();
    const FQuat AimFrame = Aim.Quaternion();
    for (int32 Pellet = 0; Pellet < FMath::Max(1, Pellets); ++Pellet)
    {
        int32 Index = State.PatternIndex;
        const double Pick = State.Rng.FRand(), T = State.Rng.FRand();
        const weapon_firing::Offset Pattern = weapon_firing::patternOffset(Data.Pattern, SpreadDegrees, Index, Pick, T);
        State.PatternIndex = Index;
        const double R1 = State.Rng.FRand(), R2 = State.Rng.FRand();
        const weapon_firing::Offset Spread = Data.bSuppressSpread ? weapon_firing::Offset()
            : weapon_firing::spreadOffset(SpreadDegrees, R1, R2);
        // Each offset is a local rotation composed after the aim (aim frame times offset).
        const double ToDegrees = 1.0 / weapon_firing::RotatorUnitsPerDegree;
        const FQuat PatternTurn = FRotator(Pattern.pitch * ToDegrees, Pattern.yaw * ToDegrees, 0).Quaternion();
        const FQuat SpreadTurn = FRotator(Spread.pitch * ToDegrees, Spread.yaw * ToDegrees, 0).Quaternion();
        Out.Add((AimFrame * PatternTurn * SpreadTurn).GetForwardVector());
    }
}

float OpenWillowFiring::FireInterval(const FOpenWillowFiringData& Data, float BaseInterval, const FOpenWillowFiringState& State)
{
    return float(weapon_firing::fireInterval(BaseInterval, Data.bSpinsToFullRate, Data.SpinStartMultiplier, State.SpinPercent));
}

bool OpenWillowFiring::IsElemental(const FString& Element)
{
    return Element == TEXT("Fire") || Element == TEXT("Shock") || Element == TEXT("Corrosive") || Element == TEXT("Slag");
}

FLinearColor OpenWillowFiring::ElementColor(const FString& Element)
{
    if (Element == TEXT("Fire")) return FLinearColor(1.f, 0.3f, 0.04f);
    if (Element == TEXT("Shock")) return FLinearColor(0.25f, 0.55f, 1.f);
    if (Element == TEXT("Corrosive")) return FLinearColor(0.45f, 1.f, 0.08f);
    if (Element == TEXT("Slag")) return FLinearColor(0.65f, 0.2f, 1.f);
    if (Element == TEXT("Explosive")) return FLinearColor(1.f, 0.62f, 0.12f);
    return FLinearColor(1.f, 0.75f, 0.35f);
}

bool OpenWillowFiring::SweepShot(const UWorld* World, FHitResult& Hit, const FVector& From, const FVector& To,
    const FVector& Extent, const FCollisionQueryParams& Query)
{
    if (!World) return false;
    if (Extent.IsNearlyZero()) return World->LineTraceSingleByChannel(Hit, From, To, ECC_Visibility, Query);
    return World->SweepSingleByChannel(Hit, From, To, FQuat::Identity, ECC_Visibility, FCollisionShape::MakeBox(Extent), Query);
}

FRotator OpenWillowFiring::TickAim(FOpenWillowAimState& State, const FOpenWillowFiringData* Held, const FString& HeldInstance,
    float Dt)
{
    if (Held && Held->bHasAccuracyPool && State.Weapon != HeldInstance)
    {
        const double Value = State.Pool.value;
        State.Pool.minValue = Held->Pool.minValue;
        State.Pool.maxValue = Held->Pool.maxValue;
        State.Pool.idleRate = Held->Pool.idleRate;
        State.Pool.idleDelay = Held->Pool.idleDelay;
        if (!State.bStarted) State.Pool.reset(Held->bPoolStartsAtMax);
        else State.Pool.value = FMath::Clamp(Value, State.Pool.minValue, State.Pool.maxValue);
        State.bStarted = true;
        State.Weapon = HeldInstance;
    }
    State.Pool.tick(Dt);
    State.Kick.tick(Dt, Held ? Held->KickData : weapon_firing::KickData());
    const double Pitch = State.Kick.currentPitch - State.AppliedPitch;
    const double Yaw = State.Kick.currentYaw - State.AppliedYaw;
    State.AppliedPitch = State.Kick.currentPitch;
    State.AppliedYaw = State.Kick.currentYaw;
    return FRotator(float(Pitch / weapon_firing::RotatorUnitsPerDegree), float(Yaw / weapon_firing::RotatorUnitsPerDegree), 0.f);
}

FRotator OpenWillowFiring::ShotAim(const FOpenWillowAimState& State, const FRotator& View, float ZoomEffect, FRandomStream& Rng)
{
    if (!State.bStarted) return View;
    const weapon_firing::Offset O = weapon_firing::accuracyOffset(State.Pool.value, ZoomEffect, Rng.FRand(), Rng.FRand());
    // In the aim's local frame, as the pellets' pattern and spread offsets.
    const FQuat Local = FRotator(float(O.pitch / weapon_firing::RotatorUnitsPerDegree), float(O.yaw / weapon_firing::RotatorUnitsPerDegree), 0.f).Quaternion();
    return (View.Quaternion() * Local).Rotator();
}

void OpenWillowFiring::OnShot(FOpenWillowAimState& State, const FOpenWillowFiringData& Data, float ZoomEffect, float FovRatio,
    bool bBurstShot, FRandomStream& Rng)
{
    // The pool and the kick are gated separately: a recipe can carry the kick fields without a pool block.
    const double Impulse = Data.AccuracyImpulse * (bBurstShot ? Data.BurstImpulseScale : 1.f);
    if (Data.bHasAccuracyPool) State.Pool.impulse(Impulse);
    if (!Data.bHasKick) return;
    const double Amount = weapon_firing::Kick::amount(Impulse, ZoomEffect, Data.KickData.zoomMultiplier);
    State.Kick.add(Amount, Data.KickData, FovRatio, Rng.FRand(), Rng.FRand());
}

bool OpenWillowFiring::AimModelEnabled()
{
    static const bool bEnabled = []()
    {
        const TCHAR* Line = FCommandLine::Get();
        if (FParse::Param(Line, TEXT("owaimmodel"))) return true;
        FString Token;
        while (FParse::Token(Line, Token, false))
        {
            if (!Token.StartsWith(TEXT("-ow"))) continue;
            const FString Name = Token.Mid(1).ToLower();
            const int32 Eq = Name.Find(TEXT("="));
            const FString Switch = Eq == INDEX_NONE ? Name : Name.Left(Eq);
            if (Switch.EndsWith(TEXT("shots")) || Switch.EndsWith(TEXT("test")) || Switch == TEXT("owautowalk")) return false;
        }
        return true;
    }();
    return bEnabled;
}
