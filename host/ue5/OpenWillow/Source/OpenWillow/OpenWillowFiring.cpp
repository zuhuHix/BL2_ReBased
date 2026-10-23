#include "OpenWillowFiring.h"

#include "Dom/JsonObject.h"

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
    const FJsonObject* Splash = ObjectOrNull(Projectile, TEXT("splash"));
    if (!Splash) return;
    Out.Splash.bValid = true;
    Out.Splash.Radius = float(weapon_firing::explosionRadius(NumberOr(*Splash, TEXT("radius"), 0)));
    // An evaluated damage when the tool resolved one; else the damage formula's attribute scaled by its constant. A
    // rocket's ProjectileDamage follows the weapon's damage through the type's ProjectileBaseValues
    // (NATIVE_WEAPON_FIRING 2.1): the live Bandit rocket's splash equalled its card damage (285.84, 2026-10-23).
    double Damage = 0;
    if (!Splash->TryGetNumberField(TEXT("damage"), Damage) || !FMath::IsFinite(Damage))
        Damage = CardDamage * NumberOr(*Splash, TEXT("damage_scale"), 1);
    Out.Splash.Damage = float(Damage);
    // "falloff" is !bDisableRadiusDamageFalloff of the Behavior_Explode; false means full damage everywhere in the
    // radius (weapon rockets and gyrojets, confirmed live). The bands are the damage type's.
    bool bFalloff = true;
    Splash->TryGetBoolField(TEXT("falloff"), bFalloff);
    Out.Splash.bFullDamage = !bFalloff;
    if (const FJsonObject* Bands = ObjectOrNull(*Splash, TEXT("falloff_bands")))
    {
        Out.Splash.Falloff.maxDamageRadius = NumberOr(*Bands, TEXT("MaxDamageRadius"), Out.Splash.Falloff.maxDamageRadius);
        Out.Splash.Falloff.minDamageRadius = NumberOr(*Bands, TEXT("MinDamageRadius"), Out.Splash.Falloff.minDamageRadius);
        Out.Splash.Falloff.minDamagePercent = NumberOr(*Bands, TEXT("MinDamagePercent"), Out.Splash.Falloff.minDamagePercent);
    }
    Splash->TryGetStringField(TEXT("damage_type"), Out.Splash.DamageType);
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
    Out.SpinDownDuration = float(NumberOr(Firing, TEXT("spin_down"), 0));
    Out.TraceRange = float(NumberOr(Firing, TEXT("trace_range"), 16384));
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
