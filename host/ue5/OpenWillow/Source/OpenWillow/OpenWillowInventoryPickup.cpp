#include "OpenWillowInventoryPickup.h"
#include "OpenWillowWeaponAssembly.h"
#include "Components/SceneComponent.h"
#include "Components/SkeletalMeshComponent.h"
#include "Components/StaticMeshComponent.h"
#include "Components/TextRenderComponent.h"
#include "Engine/SkeletalMesh.h"
#include "Engine/StaticMesh.h"
#include "Materials/MaterialInstanceDynamic.h"
#include "Misc/CommandLine.h"
#include "Misc/Parse.h"

// The world-drop look (docs/verification/WEAPON_VISUALS.md section 20). Read from the cooked data, all UNVERIFIED
// against the running game: the globals' PartPickup effect (Part_ItemLocatorStick) draws a thin vertical stick, 3 x 120
// units, tinted through a colour parameter; the tint is the item's rarity colour (GlobalsDefinition.RarityLevelColors).
// Which tiers show the stick is not read (the spawn is native); the host shows it on every tier.
namespace
{
const TCHAR* BeamMaterialPath = TEXT("/Game/OpenWillow/Weapons/InfinityProxy/M_OW_FxAdditive.M_OW_FxAdditive");
constexpr float BeamWidth = 3.f, BeamHeight = 120.f;
// Gap between the pickup actor's origin and the ground: the drop callers place it 12 units above the hit point.
constexpr float GroundGap = 12.f;
}

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
    RarityBeam = CreateDefaultSubobject<UStaticMeshComponent>(TEXT("RarityBeam"));
    RarityBeam->SetupAttachment(SceneRoot);
    RarityBeam->SetCollisionEnabled(ECollisionEnabled::NoCollision);
    RarityBeam->SetCastShadow(false);
    // The engine cylinder is 100 units tall along Z with a 50 unit radius, centred on its origin.
    RarityBeam->SetRelativeScale3D(FVector(BeamWidth / 100.f, BeamWidth / 100.f, BeamHeight / 100.f));
    RarityBeam->SetRelativeLocation(FVector(0.f, 0.f, BeamHeight * 0.5f - GroundGap));
    RarityBeam->SetVisibility(false);
}

FLinearColor AOpenWillowInventoryPickup::RarityBeamColor(const FString& HexColor)
{
    return HexColor.Len() == 7 && HexColor[0] == TEXT('#') ? FLinearColor(FColor::FromHex(HexColor)) : FLinearColor::White;
}

void AOpenWillowInventoryPickup::Initialize(FOpenWillowTakenInventoryItem InItem)
{
    Item = MoveTemp(InItem);
    bInitialized = true;
    Label->SetText(FText::FromString(DisplayName() + TEXT("\n[E] PICK UP")));
    // BL2 shows no floating text over a drop (the item card appears when the player aims at it); the label is a host debug
    // aid, shown only with -owdroplabel.
    Label->SetVisibility(FParse::Param(FCommandLine::Get(), TEXT("owdroplabel")));
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
        OpenWillowWeaponAssembly::FReport Report;
        USkeletalMesh* Mesh = OpenWillowWeaponAssembly::Show(WeaponVisual, &Item.Weapon, false, &Report);
        WeaponVisual->SetVisibility(Mesh != nullptr);
        if (Mesh)
        {
            // Lying on its side at full size (host choice, UNVERIFIED: the game drops the third-person mesh onto the ground
            // with rigid-body physics; its scale for a dropped gun was not read). The gun's barrel runs along mesh -Y, so a
            // quarter turn about Y lays it flat; its shown bounds put it centred on the beam with its lowest point on the ground.
            const FRotator Flat(90.f, 0.f, 0.f);
            const FBox Box = Report.Bounds.IsValid ? Report.Bounds : WeaponVisual->GetLocalBounds().GetBox();
            WeaponVisual->SetRelativeScale3D(FVector(1.f));
            WeaponVisual->SetRelativeRotation(Flat);
            const FVector Centre = Flat.RotateVector(Box.GetCenter());
            const float HalfHeight = FMath::Abs(Flat.RotateVector(Box.GetExtent()).Z);
            WeaponVisual->SetRelativeLocation(FVector(-Centre.X, -Centre.Y, -Centre.Z + HalfHeight - GroundGap));
        }
    }
    const FString& Hex = Item.bIsGear ? Item.Gear.RarityColor : Item.Weapon.RarityColor;
    UMaterialInterface* BeamBase = LoadObject<UMaterialInterface>(nullptr, BeamMaterialPath);
    UStaticMesh* Cylinder = LoadObject<UStaticMesh>(nullptr, TEXT("/Engine/BasicShapes/Cylinder.Cylinder"));
    if (BeamBase && Cylinder && !FParse::Param(FCommandLine::Get(), TEXT("owdropnobeam")))
    {
        float Intensity = 4.f;  // host fit, -owdropbeam=<n> for calibration
        FParse::Value(FCommandLine::Get(), TEXT("owdropbeam="), Intensity);
        UMaterialInstanceDynamic* Beam = UMaterialInstanceDynamic::Create(BeamBase, this);
        Beam->SetVectorParameterValue(TEXT("Color"), RarityBeamColor(Hex));
        Beam->SetScalarParameterValue(TEXT("Intensity"), Intensity);
        RarityBeam->SetStaticMesh(Cylinder);
        RarityBeam->SetMaterial(0, Beam);
        RarityBeam->SetVisibility(true);
    }
}

AOpenWillowInventoryPickup* AOpenWillowInventoryPickup::SpawnShowcase(AActor* Viewer, const FOpenWillowWeaponItem& Weapon,
                                                                      float Distance, float LifeSeconds)
{
    UWorld* World = Viewer ? Viewer->GetWorld() : nullptr;
    if (!World) return nullptr;
    const FVector Ahead = Viewer->GetActorLocation() + Viewer->GetActorForwardVector() * Distance;
    FHitResult Ground;
    FCollisionQueryParams Query(SCENE_QUERY_STAT(OpenWillowDropShowcase), false, Viewer);
    if (!World->LineTraceSingleByChannel(Ground, Ahead + FVector(0, 0, 100), Ahead - FVector(0, 0, 450), ECC_WorldStatic, Query))
        return nullptr;
    FActorSpawnParameters Spawn;
    Spawn.SpawnCollisionHandlingOverride = ESpawnActorCollisionHandlingMethod::AlwaysSpawn;
    // Same 12 unit gap above the hit point as the inventory's drop action; the actor faces the viewer's yaw, so the barrel (mesh -Y)
    // runs across the view and the gun is seen from the side.
    AOpenWillowInventoryPickup* Pickup = World->SpawnActor<AOpenWillowInventoryPickup>(
        StaticClass(), FTransform(FRotator(0, Viewer->GetActorRotation().Yaw, 0), Ground.ImpactPoint + FVector(0, 0, GroundGap)), Spawn);
    if (!Pickup) return nullptr;
    FOpenWillowTakenInventoryItem Item;
    Item.Weapon = Weapon;
    Pickup->Initialize(MoveTemp(Item));
    Pickup->SetLifeSpan(LifeSeconds);
    return Pickup;
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
