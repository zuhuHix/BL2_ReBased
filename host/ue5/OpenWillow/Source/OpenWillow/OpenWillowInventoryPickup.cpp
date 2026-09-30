#include "OpenWillowInventoryPickup.h"
#include "Components/SceneComponent.h"
#include "Components/SkeletalMeshComponent.h"
#include "Components/StaticMeshComponent.h"
#include "Components/TextRenderComponent.h"
#include "Engine/SkeletalMesh.h"
#include "Engine/StaticMesh.h"

AOpenWillowInventoryPickup::AOpenWillowInventoryPickup()
{
    PrimaryActorTick.bCanEverTick = false;
    SceneRoot = CreateDefaultSubobject<USceneComponent>(TEXT("SceneRoot"));
    SetRootComponent(SceneRoot);
    WeaponVisual = CreateDefaultSubobject<USkeletalMeshComponent>(TEXT("DroppedWeapon"));
    WeaponVisual->SetupAttachment(SceneRoot);
    WeaponVisual->SetCollisionEnabled(ECollisionEnabled::NoCollision);
    WeaponVisual->SetCastShadow(false);
    WeaponVisual->SetRelativeLocation(FVector(0.f, 0.f, 30.f));
    WeaponVisual->SetRelativeScale3D(FVector(0.8f));
    WeaponVisual->SetVisibility(false);
    GearVisual = CreateDefaultSubobject<UStaticMeshComponent>(TEXT("DroppedGear"));
    GearVisual->SetupAttachment(SceneRoot);
    GearVisual->SetCollisionEnabled(ECollisionEnabled::NoCollision);
    GearVisual->SetCastShadow(false);
    GearVisual->SetRelativeLocation(FVector(0.f, 0.f, 30.f));
    GearVisual->SetRelativeScale3D(FVector(0.3f));
    GearVisual->SetVisibility(false);
    Label = CreateDefaultSubobject<UTextRenderComponent>(TEXT("PickupLabel"));
    Label->SetupAttachment(SceneRoot);
    Label->SetRelativeLocation(FVector(0.f, 0.f, 75.f));
    Label->SetWorldSize(18.f);
    Label->SetHorizontalAlignment(EHTA_Center);
    Label->SetTextRenderColor(FColor(235, 245, 255));
}

void AOpenWillowInventoryPickup::Initialize(FOpenWillowTakenInventoryItem InItem)
{
    Item = MoveTemp(InItem);
    bInitialized = true;
    Label->SetText(FText::FromString(DisplayName() + TEXT("\n[E] PICK UP")));
    if (Item.bIsGear)
    {
        // The UI-traced gear record has no validated world mesh yet.
        UStaticMesh* Marker = LoadObject<UStaticMesh>(nullptr, TEXT("/Engine/BasicShapes/Sphere.Sphere"));
        GearVisual->SetStaticMesh(Marker);
        GearVisual->SetVisibility(Marker != nullptr);
    }
    else
    {
        // Recipe ID identifies an imported mesh; instance ID may have a #N
        // suffix and must never be used as a package path.
        const FString& AssetId = Item.Weapon.Id;
        const FString Path = FString::Printf(TEXT("/Game/OpenWillow/Weapons/Items/SK_%s.SK_%s"), *AssetId, *AssetId);
        USkeletalMesh* Mesh = LoadObject<USkeletalMesh>(nullptr, *Path);
        WeaponVisual->SetSkeletalMesh(Mesh);
        WeaponVisual->SetVisibility(Mesh != nullptr);
    }
}

bool AOpenWillowInventoryPickup::TryPickup(UOpenWillowInventory* Inventory)
{
    if (!bInitialized || !IsValid(Inventory)) return false;
    const bool bAdded = Item.bIsGear
        ? Inventory->AddGearToBackpack(Item.Gear)
        : Inventory->AddToBackpack(Item.Weapon);
    if (bAdded) Destroy();
    return bAdded;
}

FString AOpenWillowInventoryPickup::DisplayName() const
{
    return Item.bIsGear ? Item.Gear.Name : Item.Weapon.Name;
}
