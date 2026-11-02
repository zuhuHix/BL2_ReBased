#include "OpenWillowStatusEffect.h"

#include "Engine/World.h"
#include "OpenWillowShotFx.h"
#include "OpenWillowWalker.h"

namespace
{
FLinearColor StatusColor(const FString& Type)
{
    if (Type.Contains(TEXT("Burning")) || Type.Contains(TEXT("Ignite"))) return FLinearColor(1.f, 0.35f, 0.05f);
    if (Type.Contains(TEXT("Shock"))) return FLinearColor(0.25f, 0.55f, 1.f);
    if (Type.Contains(TEXT("Corrosive"))) return FLinearColor(0.45f, 1.f, 0.1f);
    if (Type.Contains(TEXT("Amp"))) return FLinearColor(0.7f, 0.2f, 1.f); // slag
    return FLinearColor(1.f, 1.f, 1.f);
}
}

UOpenWillowStatusEffects::UOpenWillowStatusEffects()
{
    PrimaryComponentTick.bCanEverTick = true;
}

void UOpenWillowStatusEffects::Apply(AActor* Target, AOpenWillowWalker* Shooter, const FOpenWillowStatus& Status,
    const FString& DamageType)
{
    if (!Target || !Status.bValid || Status.Duration <= 0.f) return;
    UOpenWillowStatusEffects* Effects = Target->FindComponentByClass<UOpenWillowStatusEffects>();
    if (!Effects)
    {
        Effects = NewObject<UOpenWillowStatusEffects>(Target, TEXT("OpenWillowStatusEffects"));
        Effects->RegisterComponent();
    }
    const float Now = Target->GetWorld()->GetTimeSeconds();
    // Observed in the running game (2026-10-23): a new shock proc restarts the one active shock; fire and corrosive procs
    // run side by side, each with its own clock. Slag (Amp) and the native stacking rule were not observed.
    FActive* Entry = Status.Type.Contains(TEXT("Shock"))
        ? Effects->Active.FindByPredicate([&](const FActive& A) { return A.Type == Status.Type; }) : nullptr;
    if (!Entry)
    {
        Entry = &Effects->Active.AddDefaulted_GetRef();
        Entry->Type = Status.Type;
        Entry->Clock = 0;
        Entry->UpdatedAt = Now;
    }
    // A refreshed shock keeps its tick clock and restarts its duration (live).
    Entry->DamageType = DamageType;
    Entry->Dps = Status.Dps;
    Entry->EndsAt = Now + Status.Duration;
    Entry->Shooter = Shooter;
    UE_LOG(LogTemp, Display, TEXT("OWFIRING status %s applied to %s: %.1f s, %.1f dps"), *Status.Type,
        *Target->GetName(), Status.Duration, Status.Dps);
}

bool UOpenWillowStatusEffects::IsActive(const FString& Type) const
{
    const float Now = GetWorld() ? GetWorld()->GetTimeSeconds() : 0.f;
    return Active.ContainsByPredicate([&](const FActive& A) { return A.Type == Type && A.EndsAt > Now; });
}

bool UOpenWillowStatusEffects::AnyActive(const AActor* Target)
{
    const UOpenWillowStatusEffects* Effects = Target ? Target->FindComponentByClass<UOpenWillowStatusEffects>() : nullptr;
    return Effects && Effects->Active.Num() > 0;
}

void UOpenWillowStatusEffects::ClearAll(AActor* Target)
{
    if (UOpenWillowStatusEffects* Effects = Target ? Target->FindComponentByClass<UOpenWillowStatusEffects>() : nullptr)
        Effects->Active.Reset();
}

void UOpenWillowStatusEffects::TickComponent(float DeltaTime, ELevelTick TickType, FActorComponentTickFunction* ThisTickFunction)
{
    Super::TickComponent(DeltaTime, TickType, ThisTickFunction);
    const float Now = GetWorld()->GetTimeSeconds();
    AActor* Target = GetOwner();
    for (int32 I = Active.Num() - 1; I >= 0; --I)
    {
        FActive& Effect = Active[I];
        // The measured clock (weapon_firing::dotAdvance): time builds up to the 0.33 s period, each tick pays the
        // built-up time, and the end pays what is left.
        const float Until = FMath::Min(Now, Effect.EndsAt);
        double Clock = Effect.Clock;
        double Amount = weapon_firing::dotAdvance(Clock, FMath::Max(0.f, Until - Effect.UpdatedAt), Effect.Dps);
        Effect.UpdatedAt = Until;
        const bool bEnded = Now >= Effect.EndsAt;
        if (bEnded) Amount += weapon_firing::dotFinish(Clock, Effect.Dps);
        Effect.Clock = Clock;
        if (Amount > 0.0 && Effect.Shooter.IsValid())
        {
            UE_LOG(LogTemp, Display, TEXT("OWFIRING status tick %s on %s: %.2f%s"), *Effect.Type, *Target->GetName(),
                float(Amount), bEnded ? TEXT(" (final)") : TEXT(""));
            ++OpenWillowFiring::Counts.Ticks;
            FHitResult Hit;
            Hit.ImpactPoint = Hit.Location = Target->GetActorLocation() + FVector(0, 0, 60);
            Effect.Shooter->ApplyShotDamage(Target, float(Amount), Effect.DamageType, FVector::ZeroVector, Hit);
            AOpenWillowShotFx::Flash(GetWorld(), Hit.ImpactPoint, StatusColor(Effect.Type), 14.f, 0.25f, 600.f);
        }
        if (bEnded)
        {
            UE_LOG(LogTemp, Display, TEXT("OWFIRING status %s ended on %s"), *Effect.Type, *Target->GetName());
            Active.RemoveAt(I);
        }
    }
}
