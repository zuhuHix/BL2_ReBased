#include "OpenWillowRangeLoot.h"
#include "OpenWillowEnemy.h"
#include "OpenWillowInventoryPickup.h"
#include "OpenWillowWalker.h"
#include "OpenWillowWeaponGen.h"
#include "Components/SphereComponent.h"
#include "Components/StaticMeshComponent.h"
#include "Engine/StaticMesh.h"
#include "Engine/World.h"
#include "Kismet/GameplayStatics.h"
#include "Materials/MaterialInstanceDynamic.h"
#include "Materials/MaterialInterface.h"

// Host stand-ins (UNVERIFIED in game): the sizes, the spin and the life of a range pickup, the drop circle and the money value.
namespace
{
constexpr float PickupRadius = 80.f;            // the overlap sphere (overlaps Pawn)
constexpr float PickupLifeSeconds = 60.f;
constexpr float PickupSpinDegreesPerSecond = 90.f;
constexpr float DropRadius = 60.f;              // drop spots lie on a circle this far from the enemy's centre
constexpr float DropLift = 12.f;                // above the floor under the spot (as the HUD's drop)
constexpr int32 DropSpots = 6;                  // spots 60 degrees apart; later drops share them

// The money of one pickup at Level: round(4 x 1.13^level). The value of ItemGrade_Currency_Money_Little is not decoded.
int32 MoneyStandIn(int32 Level)
{
    return FMath::RoundToInt(4.f * FMath::Pow(1.13f, float(Level)));
}

// The leaf of a pool path (the part after the last dot; the whole path when there is none).
FString PoolLeaf(const FString& Pool)
{
    int32 Dot = INDEX_NONE;
    return Pool.FindLastChar(TEXT('.'), Dot) ? Pool.Mid(Dot + 1) : Pool;
}

// Drop spot Index, on the floor (host stand-in): the enemy's collision centre, then a point on the drop circle, then the floor
// under that point (the enemy's feet when there is none), DropLift above it.
FVector DropSpot(UWorld* World, const AOpenWillowEnemy* Enemy, int32 Index)
{
    FVector Centre = FVector::ZeroVector;
    float HalfHeight = 0.f;
    Enemy->CollisionCentre(Centre, HalfHeight);
    const float Angle = FMath::DegreesToRadians(360.f / DropSpots * float(Index % DropSpots));
    const FVector2D XY = FVector2D(Centre.X, Centre.Y) + FVector2D(FMath::Cos(Angle), FMath::Sin(Angle)) * DropRadius;
    float FloorZ = Centre.Z - HalfHeight;
    FVector Floor = FVector::ZeroVector;
    if (AOpenWillowEnemy::TraceFloor(World, XY, Centre.Z + 100.f, Centre.Z - 600.f, Enemy, Floor)) FloorZ = Floor.Z;
    return FVector(XY.X, XY.Y, FloorZ + DropLift);
}

// Spawns one money or ammo pickup at At and counts it in Result.
void SpawnRangePickup(UWorld* World, const FVector& At, EOpenWillowRangePickupKind Kind, int32 Amount, EOpenWillowAmmoType Ammo,
    FOpenWillowRangeLootResult& Result)
{
    FActorSpawnParameters Params;
    Params.SpawnCollisionHandlingOverride = ESpawnActorCollisionHandlingMethod::AlwaysSpawn;
    AOpenWillowRangePickup* Pickup = World->SpawnActor<AOpenWillowRangePickup>(At, FRotator::ZeroRotator, Params);
    if (!Pickup) return;
    Pickup->Initialize(Kind, Amount, Ammo);
    Result.Spawned.Add(Pickup);
    if (Kind == EOpenWillowRangePickupKind::Money) ++Result.Money;
    else ++Result.Ammo;
}

// Spawns a weapon pickup holding Item at At (the inventory pickup's public API: SpawnActor and Initialize).
void SpawnWeaponPickup(UWorld* World, const FVector& At, const FOpenWillowWeaponItem& Item, FOpenWillowRangeLootResult& Result)
{
    FActorSpawnParameters Params;
    Params.SpawnCollisionHandlingOverride = ESpawnActorCollisionHandlingMethod::AlwaysSpawn;
    AOpenWillowInventoryPickup* Pickup = World->SpawnActor<AOpenWillowInventoryPickup>(At, FRotator::ZeroRotator, Params);
    if (!Pickup) return;
    FOpenWillowTakenInventoryItem Taken;
    Taken.bIsGear = false;
    Taken.Weapon = Item;
    Pickup->Initialize(Taken);
    Result.Spawned.Add(Pickup);
    ++Result.Weapons;
}
}

AOpenWillowRangePickup::AOpenWillowRangePickup()
{
    PrimaryActorTick.bCanEverTick = true;
    Body = CreateDefaultSubobject<USphereComponent>(TEXT("PickupBody"));
    Body->InitSphereRadius(PickupRadius);
    Body->SetCollisionEnabled(ECollisionEnabled::QueryOnly);
    Body->SetCollisionObjectType(ECC_WorldDynamic);
    Body->SetCollisionResponseToAllChannels(ECR_Ignore);
    Body->SetCollisionResponseToChannel(ECC_Pawn, ECR_Overlap);
    Body->SetGenerateOverlapEvents(true);
    SetRootComponent(Body);
    Visual = CreateDefaultSubobject<UStaticMeshComponent>(TEXT("PickupVisual"));
    Visual->SetupAttachment(Body);
    Visual->SetRelativeLocation(FVector(0.f, 0.f, 20.f));
    Visual->SetCollisionEnabled(ECollisionEnabled::NoCollision);
    Visual->SetCastShadow(false);
    Body->OnComponentBeginOverlap.AddDynamic(this, &AOpenWillowRangePickup::OnBodyBeginOverlap);
}

void AOpenWillowRangePickup::Initialize(EOpenWillowRangePickupKind InKind, int32 InAmount, EOpenWillowAmmoType InAmmo)
{
    PickupKind = InKind;
    Amount = FMath::Max(0, InAmount);
    AmmoType = InAmmo;
    const bool bMoney = PickupKind == EOpenWillowRangePickupKind::Money;
    Visual->SetStaticMesh(LoadObject<UStaticMesh>(nullptr, bMoney ? TEXT("/Engine/BasicShapes/Cylinder.Cylinder")
        : TEXT("/Engine/BasicShapes/Cube.Cube")));
    Visual->SetRelativeScale3D(bMoney ? FVector(0.3f, 0.3f, 0.1f) : FVector(0.3f));
    if (UMaterialInterface* Base = LoadObject<UMaterialInterface>(nullptr, TEXT("/Engine/BasicShapes/BasicShapeMaterial.BasicShapeMaterial")))
    {
        if (UMaterialInstanceDynamic* Tint = Visual->CreateDynamicMaterialInstance(0, Base))
            Tint->SetVectorParameterValue(TEXT("Color"), bMoney ? FLinearColor(0.9f, 0.7f, 0.1f) : FLinearColor(0.5f, 0.5f, 0.5f));
    }
    SetLifeSpan(PickupLifeSeconds);
}

void AOpenWillowRangePickup::Tick(float DeltaSeconds)
{
    Super::Tick(DeltaSeconds);
    AddActorLocalRotation(FRotator(0.f, PickupSpinDegreesPerSecond * DeltaSeconds, 0.f));
}

void AOpenWillowRangePickup::OnBodyBeginOverlap(UPrimitiveComponent* OverlappedComponent, AActor* OtherActor,
    UPrimitiveComponent* OtherComp, int32 OtherBodyIndex, bool bFromSweep, const FHitResult& SweepResult)
{
    if (bCollected) return;
    if (AOpenWillowWalker* Walker = Cast<AOpenWillowWalker>(OtherActor)) Collect(Walker);
}

void AOpenWillowRangePickup::Collect(AOpenWillowWalker* Walker)
{
    UOpenWillowInventory* Inventory = Walker->GetInventory();
    if (!Inventory) return;
    bCollected = true;
    if (PickupKind == EOpenWillowRangePickupKind::Money)
    {
        // The money is 0 while the host has not set it (GetMoney's flag is not needed here).
        int32 Before = 0;
        Inventory->GetMoney(Before);
        Inventory->SetMoney(Before + Amount);
        UE_LOG(LogTemp, Display, TEXT("OWRANGE pickup kind=money amount=%d money=%d->%d"), Amount, Before, Before + Amount);
    }
    else
    {
        const int32 Before = Inventory->AmmoPool(AmmoType).Current;
        Inventory->SetAmmoCurrent(AmmoType, Before + Amount);   // clamped to the pool's maximum by the inventory
        UE_LOG(LogTemp, Display, TEXT("OWRANGE pickup kind=ammo amount=%d ammo=%s %d->%d"), Amount,
            UOpenWillowInventory::AmmoTypeKey(AmmoType), Before, Inventory->AmmoPool(AmmoType).Current);
    }
    Destroy();
}

FOpenWillowRangeLootResult RollRangeLoot(UWorld* World, AOpenWillowEnemy* Enemy, const FString& KindName,
    const FOpenWillowRangeKind& Kind, int32 Level, FRandomStream& Stream, EOpenWillowRangeLootMode Mode)
{
    FOpenWillowRangeLootResult Result;
    if (!World || !Enemy) return Result;
    if (Mode == EOpenWillowRangeLootMode::Off)
    {
        UE_LOG(LogTemp, Display, TEXT("OWRANGE loot off enemy=%s: nothing rolled"), *KindName);
        return Result;
    }
    const FOpenWillowRangeLoot* Entry = Kind.LootFor(Level);
    if (!Entry)
    {
        UE_LOG(LogTemp, Display, TEXT("OWRANGE loot no_entry enemy=%s level=%d"), *KindName, Level);
        return Result;
    }
    // The held weapon's ammo decides the ammo stand-in (no walker or no weapon: not needed).
    const AOpenWillowWalker* Walker = Cast<AOpenWillowWalker>(UGameplayStatics::GetPlayerPawn(World, 0));
    const UOpenWillowInventory* Inventory = Walker ? Walker->GetInventory() : nullptr;
    int32 DropIndex = 0;
    for (const FOpenWillowRangeLootPool& Pool : Entry->Pools)
    {
        const float Draw = Stream.FRand();
        const bool bRolls = Pool.Kind == TEXT("weapon") || Pool.Kind == TEXT("money") || Pool.Kind == TEXT("ammo");
        const float Chance = (Mode == EOpenWillowRangeLootMode::Always && bRolls) ? 1.f : Pool.Chance;
        const bool bTried = Chance > 0.f && Draw <= Chance;
        const FString Leaf = PoolLeaf(Pool.Pool);
        FString What = TEXT("-");
        if (!bTried)
        {
            // Not tried: nothing is drawn after the FRand above.
        }
        else if (Pool.Kind == TEXT("weapon"))
        {
            if (!FOpenWillowWeaponGen::IsAvailable())
            {
                What = TEXT("weapon_unavailable");
                UE_LOG(LogTemp, Display, TEXT("OWRANGE loot weapon_unavailable enemy=%s pool=%s"), *KindName, *Leaf);
            }
            else
            {
                FOpenWillowWeaponItem Item;
                const uint32 Seed = Stream.GetUnsignedInt();
                if (FOpenWillowWeaponGen::RollPoolWeapon(Pool.Pool, Entry->ItemStage, Seed, Item))
                {
                    const FVector At = DropSpot(World, Enemy, DropIndex++);
                    SpawnWeaponPickup(World, At, Item, Result);
                    What = FString::Printf(TEXT("weapon %s"), *Item.Name);
                }
                else
                {
                    What = TEXT("no_weapon");
                }
            }
        }
        else if (Pool.Kind == TEXT("money"))
        {
            // Quantity: a random integer in -1..2, at least 1 (COMBAT_LOOT: Pool_Money_1or2). Each piece is one pickup.
            const int32 Quantity = FMath::Max(1, Stream.RandRange(-1, 2));
            const int32 Each = MoneyStandIn(Level);
            for (int32 Copy = 0; Copy < Quantity; ++Copy)
                SpawnRangePickup(World, DropSpot(World, Enemy, DropIndex++), EOpenWillowRangePickupKind::Money, Each,
                    EOpenWillowAmmoType::Pistol, Result);
            What = FString::Printf(TEXT("money x%d of %d"), Quantity, Each);
        }
        else if (Pool.Kind == TEXT("ammo"))
        {
            // Host stand-in: one pickup of the held weapon's ammo, only when that reserve is below half its maximum.
            // Amount: the held weapon's magazine size.
            EOpenWillowAmmoType AmmoType = EOpenWillowAmmoType::Pistol;
            const FOpenWillowWeaponItem* Weapon = Inventory ? Inventory->ActiveWeapon() : nullptr;
            if (Weapon && UOpenWillowInventory::ResolveAmmoType(*Weapon, AmmoType))
            {
                const FOpenWillowAmmoPool Reserve = Inventory->AmmoPool(AmmoType);
                const int32 Amount = UOpenWillowInventory::MagazineSize(*Weapon);
                if (Reserve.Current * 2 < Reserve.Max && Amount > 0)
                {
                    SpawnRangePickup(World, DropSpot(World, Enemy, DropIndex++), EOpenWillowRangePickupKind::Ammo, Amount, AmmoType,
                        Result);
                    What = FString::Printf(TEXT("ammo %s x%d"), UOpenWillowInventory::AmmoTypeKey(AmmoType), Amount);
                }
                else
                {
                    What = TEXT("ammo_not_needed");
                    UE_LOG(LogTemp, Display, TEXT("OWRANGE loot ammo_not_needed enemy=%s pool=%s"), *KindName, *Leaf);
                }
            }
            else
            {
                What = TEXT("ammo_not_needed");
                UE_LOG(LogTemp, Display, TEXT("OWRANGE loot ammo_not_needed enemy=%s pool=%s"), *KindName, *Leaf);
            }
        }
        else
        {
            What = TEXT("not_hosted");
            UE_LOG(LogTemp, Display, TEXT("OWRANGE loot not_hosted pool=%s"), *Leaf);
        }
        UE_LOG(LogTemp, Display, TEXT("OWRANGE loot enemy=%s pool=%s kind=%s chance=%.4f roll=%.6f tried=%d result=%s"), *KindName,
            *Leaf, *Pool.Kind, Chance, Draw, bTried ? 1 : 0, *What);
        Result.Trace += FString::Printf(TEXT("%s:%d:%.6f;"), *Leaf, bTried ? 1 : 0, Draw);
    }
    UE_LOG(LogTemp, Display, TEXT("OWRANGE loot enemy=%s level=%d weapons=%d money=%d ammo=%d"), *KindName, Level, Result.Weapons,
        Result.Money, Result.Ammo);
    return Result;
}
