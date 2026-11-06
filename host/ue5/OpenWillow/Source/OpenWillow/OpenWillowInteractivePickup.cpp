// AI-assisted (Claude). See OpenWillowInteractivePickup.h. Each kind is applied through the plain rules in
// OpenWillowInteractive.h; the walker's own health and inventory are the only things changed.
#include "OpenWillowInteractivePickup.h"
#include "OpenWillowWalker.h"
#include "OpenWillowInventory.h"
#include "Components/StaticMeshComponent.h"
#include "Components/TextRenderComponent.h"
#include "Engine/StaticMesh.h"
#include "Engine/World.h"
#include "UObject/ConstructorHelpers.h"

namespace
{
const TCHAR* PickupAssetRoot = TEXT("/Game/OpenWillow/Pickups/");
// The cap of money and eridium (no cap beyond the int32 they are stored in).
constexpr double MoneyCap = MAX_int32;
}

AOpenWillowLootPickup::AOpenWillowLootPickup()
{
    PrimaryActorTick.bCanEverTick = false;
    USceneComponent* SceneRoot = CreateDefaultSubobject<USceneComponent>(TEXT("Root"));
    SetRootComponent(SceneRoot);
    // UNVERIFIED stand-in: a small sphere until the pickup mesh is imported.
    Visual = CreateDefaultSubobject<UStaticMeshComponent>(TEXT("PickupVisual"));
    Visual->SetupAttachment(SceneRoot);
    Visual->SetCollisionEnabled(ECollisionEnabled::NoCollision);
    Visual->SetCastShadow(false);
    Visual->SetRelativeLocation(FVector(0.f, 0.f, 15.f));
    Visual->SetRelativeScale3D(FVector(0.2f));
    static ConstructorHelpers::FObjectFinder<UStaticMesh> Sphere(TEXT("/Engine/BasicShapes/Sphere.Sphere"));
    if (Sphere.Succeeded()) Visual->SetStaticMesh(Sphere.Object);
    Label = CreateDefaultSubobject<UTextRenderComponent>(TEXT("PickupLabel"));
    Label->SetupAttachment(SceneRoot);
    Label->SetRelativeLocation(FVector(0.f, 0.f, 50.f));
    Label->SetWorldSize(16.f);
    Label->SetHorizontalAlignment(EHTA_Center);
}

void AOpenWillowLootPickup::Configure(const FOpenWillowPickupSpec& InSpec, int32 InValue, const FString& InBalance)
{
    Spec_ = InSpec;
    Value_ = InValue;
    Balance_ = InBalance;
    if (!Spec_.Mesh.IsEmpty())
    {
        if (UStaticMesh* Mesh = LoadObject<UStaticMesh>(nullptr, *(FString(PickupAssetRoot) + Spec_.Mesh)))
        {
            Visual->SetStaticMesh(Mesh);
            Visual->SetRelativeScale3D(FVector(1.f));
        }
    }
    FString Text;
    if (Spec_.Kind == TEXT("cash")) Text = FString::Printf(TEXT("$%d"), Value_);
    else if (Spec_.Kind == TEXT("ammo")) Text = FString::Printf(TEXT("%s +%d"), *Spec_.AmmoType, Spec_.Amount);
    else if (Spec_.Kind == TEXT("eridium")) Text = FString::Printf(TEXT("ERIDIUM +%d"), Spec_.Amount);
    else Text = FString::Printf(TEXT("HEALTH +%d%%"), FMath::RoundToInt(Spec_.HealFraction * 100.f));
    Label->SetText(FText::FromString(Text));
}

bool AOpenWillowLootPickup::TryCollect(AOpenWillowWalker& Walker)
{
    FString Reason;
    double Before = 0, After = 0, Cap = 0, Amount = 0;
    bool bOk = false;
    if (Spec_.Kind == TEXT("cash"))
    {
        Amount = Value_;
        bOk = CollectMoney(Walker, Reason, Before, After, Cap);
    }
    else if (Spec_.Kind == TEXT("eridium"))
    {
        Amount = Spec_.Amount;
        bOk = CollectEridium(Walker, Reason, Before, After, Cap);
    }
    else if (Spec_.Kind == TEXT("ammo"))
    {
        Amount = Spec_.Amount;
        bOk = CollectAmmo(Walker, Reason, Before, After, Cap);
    }
    else if (Spec_.Kind == TEXT("health"))
    {
        Amount = Spec_.HealFraction; // the fraction of max health; cap is the max health it heals toward
        bOk = CollectHealth(Walker, Reason, Before, After, Cap);
    }
    else
    {
        Reason = TEXT("unknown kind");
    }
    UE_LOG(LogTemp, Display, TEXT("OWPICKUP kind=%s amount=%.2f before=%.2f after=%.2f cap=%.2f ok=%d reason=%s"),
        *Spec_.Kind, Amount, Before, After, Cap, bOk ? 1 : 0, *Reason);
    if (bOk) Destroy();
    return bOk;
}

bool AOpenWillowLootPickup::CollectMoney(AOpenWillowWalker& Walker, FString& OutReason, double& OutBefore, double& OutAfter,
    double& OutCap)
{
    UOpenWillowInventory* Inventory = Walker.GetInventory();
    int32 Money = 0;
    OutCap = MoneyCap;
    if (!Inventory || !Inventory->GetMoney(Money))
    {
        OutReason = TEXT("purse not known");
        return false;
    }
    const int32 After = OpenWillowInteractiveRules::SaturatingAdd(Money, Value_);
    Inventory->SetMoney(After);
    OutBefore = Money;
    OutAfter = After;
    return true;
}

bool AOpenWillowLootPickup::CollectEridium(AOpenWillowWalker& Walker, FString& OutReason, double& OutBefore, double& OutAfter,
    double& OutCap)
{
    UOpenWillowInventory* Inventory = Walker.GetInventory();
    int32 Eridium = 0;
    OutCap = MoneyCap;
    if (!Inventory || !Inventory->GetEridium(Eridium))
    {
        OutReason = TEXT("eridium not known");
        return false;
    }
    const int32 After = OpenWillowInteractiveRules::SaturatingAdd(Eridium, Spec_.Amount);
    Inventory->SetEridium(After);
    OutBefore = Eridium;
    OutAfter = After;
    return true;
}

bool AOpenWillowLootPickup::CollectAmmo(AOpenWillowWalker& Walker, FString& OutReason, double& OutBefore, double& OutAfter,
    double& OutCap)
{
    UOpenWillowInventory* Inventory = Walker.GetInventory();
    EOpenWillowAmmoType Type = EOpenWillowAmmoType::Pistol;
    if (!Inventory || !OpenWillowInteractiveRules::AmmoTypeFromName(Spec_.AmmoType, Type))
    {
        OutReason = TEXT("unknown ammo type");
        return false;
    }
    const FOpenWillowAmmoPool Pool = Inventory->AmmoPool(Type);
    OutBefore = Pool.Current;
    OutAfter = Pool.Current;
    OutCap = Pool.Max;
    int32 After = Pool.Current;
    if (!OpenWillowInteractiveRules::AmmoAfter(Pool.Current, Pool.Max, Spec_.Amount, After))
    {
        OutReason = Pool.Current >= Pool.Max ? TEXT("ammo pool full") : TEXT("amount not positive");
        return false;
    }
    Inventory->SetAmmoCurrent(Type, After);
    OutAfter = After;
    return true;
}

bool AOpenWillowLootPickup::CollectHealth(AOpenWillowWalker& Walker, FString& OutReason, double& OutBefore, double& OutAfter,
    double& OutCap)
{
    const float Health = Walker.GetHealth();
    const float MaxHealth = Walker.GetMaxHealth();
    OutBefore = Health;
    OutAfter = Health;
    OutCap = MaxHealth;
    float After = Health;
    if (!OpenWillowInteractiveRules::HealAfter(Health, MaxHealth, Spec_.HealFraction, Walker.IsDown(), After))
    {
        OutReason = Walker.IsDown() ? TEXT("down") : Health >= MaxHealth ? TEXT("at full health") : TEXT("no heal");
        return false;
    }
    // Only the current health changes; the maximum is the same value it had.
    // Current health only: SetHealth would write the effective maximum into the base one and compound the skills'
    // health modifiers (PR #99 review). HealAfter already refused a heal while down or at full health.
    Walker.GetMayaState().Health = After;
    OutAfter = After;
    return true;
}
