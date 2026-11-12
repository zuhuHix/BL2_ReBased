// AI-assisted (Claude). See OpenWillowMayaState.h and docs/verification/COMBAT_DOWN_RESPAWN.md ("The host's state").
#include "OpenWillowMayaState.h"
#include "Engine/World.h"
#include "Kismet/GameplayStatics.h"
#include "Misc/FileHelper.h"
#include "OpenWillowWalker.h"
#include "Serialization/JsonReader.h"
#include "Serialization/JsonSerializer.h"

float FOpenWillowStandInShieldRule::CapacityAt(int32 Level) const
{
    if (!IsSet()) return 0.f;
    return FMath::RoundToFloat(Capacity * FMath::Pow(Growth, float(FMath::Max(1, Level) - AtLevel)));
}

bool FOpenWillowStandInShieldRule::Parse(const FString& Json, FString& OutError)
{
    *this = FOpenWillowStandInShieldRule();
    TSharedPtr<FJsonObject> Root;
    double Level = 0, InCapacity = 0, InGrowth = 0;
    if (!FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Json), Root) || !Root
        || !Root->TryGetNumberField(TEXT("at_level"), Level) || !Root->TryGetNumberField(TEXT("capacity"), InCapacity)
        || !Root->TryGetNumberField(TEXT("growth_per_level"), InGrowth))
    {
        OutError = TEXT("expected a JSON object with at_level, capacity and growth_per_level");
        return false;
    }
    if (!(Level >= 1 && Level <= 100 && Level == FMath::FloorToDouble(Level)) || !(InCapacity > 0 && InCapacity < 1e7) || !(InGrowth >= 1 && InGrowth <= 2))
    {
        OutError = TEXT("at_level must be a whole number in 1..100, capacity above 0 and growth_per_level in 1..2");
        return false;
    }
    AtLevel = int32(Level);
    Capacity = float(InCapacity);
    Growth = float(InGrowth);
    return true;
}

bool FOpenWillowStandInShieldRule::LoadFile(const FString& Path, FString& OutError)
{
    *this = FOpenWillowStandInShieldRule();
    FString Text, Error;
    if (!FFileHelper::LoadFileToString(Text, *Path))
    {
        OutError = FString::Printf(TEXT("no %s, so a shield without a capacity stat protects nothing"), *Path);
        return false;
    }
    if (!Parse(Text, Error))
    {
        OutError = FString::Printf(TEXT("%s ignored, so a shield without a capacity stat protects nothing: %s"), *Path, *Error);
        return false;
    }
    return true;
}

FOpenWillowDamageResult FOpenWillowMayaState::ApplyDamage(float Amount, float Now)
{
    FOpenWillowDamageResult Result;
    if (IsDown())
    {
        Result.bIgnored = true;
        return Result;
    }
    if (!FMath::IsFinite(Amount) || Amount <= 0.f) return Result;   // NaN or infinity never reaches the pools
    // Live (COMBAT_GOLDEN_TRACE.md step 9): each hit came off the shield; the hit that emptied it sent the remainder to
    // health in the same hit. Every hit restarts the recharge delay (step 10), a health-only hit included.
    LastDamageAt = Now;
    Result.ToShield = FMath::Min(Amount, Shield);
    Shield -= Result.ToShield;
    Result.ToHealth = FMath::Min(Amount - Result.ToShield, Health);
    Health -= Result.ToHealth;
    if (Health <= 0.f)
    {
        GoDown(Now);
        Result.bWentDown = true;
    }
    return Result;
}

void FOpenWillowMayaState::GoDown(float Now)
{
    // BeginState of the injured state (COMBAT_DOWN_RESPAWN.md "Bleed-out"): the cut uses the counter as it was before
    // this injury, at least 1 s is left; only then is the counter raised (an injury within 30 s of the previous one) or reset.
    const FOpenWillowMayaStats S = Effective();
    const float Total = BaseBleedout * S.BleedoutMultiplier;
    Health = 0.f;
    Phase = EOpenWillowDownPhase::Injured;
    DownAt = Now;
    BleedSeconds = FMath::Max(Total - Total / MaxSuccessive * SuccessiveInjuries, 1.f);
    SuccessiveInjuries = Now - LastInjuryAt < SuccessiveWindow ? SuccessiveInjuries + 1 : 0;
    LastInjuryAt = Now;
}

FOpenWillowMayaStats FOpenWillowMayaState::Effective() const
{
    if (!Modifiers) return Base;
    // The byte compare is safe only while FOpenWillowMayaStats holds plain floats and ints (no FString, TArray or other
    // heap or padding-sensitive members); if it gains one, compare field by field instead.
    if (CachedFrame != GFrameCounter || FMemory::Memcmp(&CachedBase, &Base, sizeof(Base)) != 0)
    {
        CachedBase = Base;
        CachedStats = Base;
        Modifiers(CachedStats);
        CachedFrame = GFrameCounter;
    }
    return CachedStats;
}

FOpenWillowMayaState::ETickEvent FOpenWillowMayaState::Tick(float Now, float DeltaSeconds)
{
    const FOpenWillowMayaStats S = Effective();
    // A modifier that ends (a skill respec lowering the maximum, Ward or Restoration) leaves the pools above their new
    // maxima; clamp them back. Host rule, UNVERIFIED as stock behaviour.
    Health = FMath::Min(Health, S.MaxHealth);
    Shield = FMath::Min(Shield, S.ShieldCapacity);
    if (Phase == EOpenWillowDownPhase::Up)
    {
        // The shield waits the delay after the last hit, then rises at a constant rate (live: it rose one frame past the pool's
        // read delay, at the read rate; COMBAT_GOLDEN_TRACE.md step 10). Not while down (host rule, UNVERIFIED).
        if (Now - LastDamageAt >= S.ShieldRechargeDelay && Shield < S.ShieldCapacity)
            Shield = FMath::Min(S.ShieldCapacity, Shield + S.ShieldRechargeRate * DeltaSeconds);
        if (S.HealthRegenRate > 0.f && Health > 0.f)
            Health = FMath::Min(S.MaxHealth, Health + S.HealthRegenRate * DeltaSeconds);
        return ETickEvent::None;
    }
    if (Phase == EOpenWillowDownPhase::Injured && Now - (DownAt + TargetedDelay) > BleedSeconds)
    {
        Phase = EOpenWillowDownPhase::Dead;
        DeadAt = Now;
        return ETickEvent::BledOut;
    }
    if (Phase == EOpenWillowDownPhase::Dead && Now - DeadAt >= DeathCamera) return ETickEvent::RespawnDue;
    return ETickEvent::None;
}

bool FOpenWillowMayaState::SecondWind(float Now)
{
    if (Phase != EOpenWillowDownPhase::Injured) return false;
    const FOpenWillowMayaStats S = Effective();
    Health = FMath::Max(Health, S.MaxHealth * S.SecondWindHealthPct * S.RevivalHealthMultiplier);
    Shield = FMath::Max(Shield, S.ShieldCapacity * S.SecondWindShieldPct);
    Phase = EOpenWillowDownPhase::Up;
    SecondWindAt = Now;
    return true;
}

void FOpenWillowMayaState::Respawn(float Now)
{
    const FOpenWillowMayaStats S = Effective();
    Health = S.MaxHealth * S.RespawnHealthPct;
    Shield = S.ShieldCapacity;
    Phase = EOpenWillowDownPhase::Up;
    SuccessiveInjuries = 0;
    RespawnAt = Now;
}

bool FOpenWillowMayaState::SetHealth(float NewMax, float NewCurrent, float Now)
{
    Base.MaxHealth = NewMax;
    Health = NewCurrent >= NewMax ? Effective().MaxHealth : NewCurrent;
    if (Phase != EOpenWillowDownPhase::Injured || Health <= 0.f) return false;
    Phase = EOpenWillowDownPhase::Up;
    SecondWindAt = Now;
    return true;
}

void FOpenWillowMayaState::EquipShield(float Capacity, float RechargeDelay, float RechargeRate, bool bStartFull, float Now)
{
    Base.ShieldCapacity = FMath::Max(0.f, Capacity);
    Base.ShieldRechargeDelay = FMath::Max(0.f, RechargeDelay);
    Base.ShieldRechargeRate = FMath::Max(0.f, RechargeRate);
    Shield = bStartFull ? Effective().ShieldCapacity : 0.f;
    LastDamageAt = Now;
}

bool FOpenWillowMayaState::WeaponReady(float Now) const
{
    if (Phase == EOpenWillowDownPhase::Up) return true;
    return Phase == EOpenWillowDownPhase::Injured && Now >= DownAt + TargetedDelay;
}

float FOpenWillowMayaState::BleedLeft(float Now) const
{
    if (Phase == EOpenWillowDownPhase::Up) return 1.f;
    if (Phase == EOpenWillowDownPhase::Dead) return 0.f;
    const float Bled = Now - (DownAt + TargetedDelay);
    return FMath::Clamp(1.f - FMath::Max(0.f, Bled) / BleedSeconds, 0.f, 1.f);
}

float FOpenWillowMayaState::HealthFraction() const
{
    return FMath::Clamp(Health / FMath::Max(1.f, Effective().MaxHealth), 0.f, 1.f);
}

float FOpenWillowMayaState::ShieldFraction() const
{
    const float Capacity = Effective().ShieldCapacity;
    return Capacity > 0.f ? FMath::Clamp(Shield / Capacity, 0.f, 1.f) : 0.f;
}

float OpenWillowApplyDamageToPlayer(const UObject* WorldContext, float Amount, AActor* Causer, const FVector& SourceLocation)
{
    auto* Maya = Cast<AOpenWillowWalker>(UGameplayStatics::GetPlayerPawn(WorldContext, 0));
    return Maya ? Maya->ApplyDamageToPlayer(Amount, Causer, SourceLocation) : 0.f;
}
