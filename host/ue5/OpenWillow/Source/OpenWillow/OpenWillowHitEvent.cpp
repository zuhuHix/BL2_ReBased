#include "OpenWillowHitEvent.h"

#include "GameFramework/Actor.h"

void IOpenWillowHitTarget::GetHitCentre(FVector& OutCentre, float& OutRadius) const
{
    const AActor* Self = Cast<AActor>(this);
    float HalfHeight = 0;
    OutRadius = 0;
    OutCentre = Self ? Self->GetActorLocation() : FVector::ZeroVector;
    if (Self) Self->GetSimpleCollisionCylinder(OutRadius, HalfHeight);
}

FOnOpenWillowHit& OpenWillowHit::OnHit()
{
    static FOnOpenWillowHit Delegate;
    return Delegate;
}

EOpenWillowElement OpenWillowHit::ElementOf(const FString& DamageType, const FString& Fallback)
{
    // The stock damage type packages name the element: GD_Incendiary, GD_Shock, GD_Corrosive, GD_Amp (slag), GD_Explosive.
    const auto Has = [&](const TCHAR* Word) { return DamageType.Contains(Word, ESearchCase::IgnoreCase); };
    if (Has(TEXT("Incendiary")) || Has(TEXT("Fire"))) return EOpenWillowElement::Fire;
    if (Has(TEXT("Shock"))) return EOpenWillowElement::Shock;
    if (Has(TEXT("Corrosive"))) return EOpenWillowElement::Corrosive;
    if (Has(TEXT("GD_Amp")) || Has(TEXT("Type_Amp")) || Has(TEXT("Slag"))) return EOpenWillowElement::Slag;
    if (Has(TEXT("Explosive"))) return EOpenWillowElement::Explosive;
    // The item's own element word ("Fire", "Shock", ...) matches the same words; no further fallback after it.
    if (!Fallback.IsEmpty() && Fallback != TEXT("None")) return ElementOf(Fallback);
    return EOpenWillowElement::None;
}

const TCHAR* OpenWillowHit::ElementName(EOpenWillowElement Element)
{
    switch (Element)
    {
    case EOpenWillowElement::Fire: return TEXT("Fire");
    case EOpenWillowElement::Shock: return TEXT("Shock");
    case EOpenWillowElement::Corrosive: return TEXT("Corrosive");
    case EOpenWillowElement::Slag: return TEXT("Slag");
    case EOpenWillowElement::Explosive: return TEXT("Explosive");
    default: return TEXT("None");
    }
}

const TCHAR* OpenWillowHit::KindName(EOpenWillowHitKind Kind)
{
    switch (Kind)
    {
    case EOpenWillowHitKind::Splash: return TEXT("splash");
    case EOpenWillowHitKind::StatusTick: return TEXT("tick");
    case EOpenWillowHitKind::Throw: return TEXT("throw");
    default: return TEXT("direct");
    }
}

const TCHAR* OpenWillowHit::SourceName(EOpenWillowHitSource Source)
{
    switch (Source)
    {
    case EOpenWillowHitSource::Bullet: return TEXT("bullet");
    case EOpenWillowHitSource::Rocket: return TEXT("rocket");
    case EOpenWillowHitSource::Explosion: return TEXT("explosion");
    case EOpenWillowHitSource::Throw: return TEXT("throw");
    case EOpenWillowHitSource::Status: return TEXT("status");
    case EOpenWillowHitSource::Skill: return TEXT("skill");
    case EOpenWillowHitSource::Grenade: return TEXT("grenade");
    default: return TEXT("unknown");
    }
}

IOpenWillowHitTarget* OpenWillowHit::AsTarget(AActor* Actor)
{
    return Actor ? Cast<IOpenWillowHitTarget>(Actor) : nullptr;
}
