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
constexpr int32 DropSpots = 6;                  // spots 60 degrees apart on each ring
constexpr int32 DropRings = 3;                  // rings at 1, 2 and 3 x DropRadius, tried in turn when a spot has no room
constexpr float DropRoomRadius = 30.f;          // the room a drop needs: a free sphere this size just above the floor
constexpr float WeaponDropLifeSeconds = 120.f;
constexpr int32 AlwaysWeaponTries = 8;          // ow.RangeLoot always: seeds tried until the weapon pool gives a gun  // a range gun drop expires after this (the range's own pickups after 60 s)

// The money of one pickup at Level: round(4 x 1.13^level), saturated to the int32 range. The value of
// ItemGrade_Currency_Money_Little is not decoded.
int32 MoneyStandIn(int32 Level)
{
    const double Value = 4.0 * FMath::Pow(1.13, double(FMath::Max(0, Level)));
    return Value >= double(MAX_int32) ? MAX_int32 : int32(FMath::RoundToDouble(Value));
}

// A + B without wrapping: saturated to 0..MAX_int32 (the money and ammo counters).
int32 SaturatingAdd(int32 A, int32 B)
{
    return int32(FMath::Clamp(int64(A) + int64(B), int64(0), int64(MAX_int32)));
}

// The leaf of a pool path (the part after the last dot; the whole path when there is none).
FString PoolLeaf(const FString& Pool)
{
    int32 Dot = INDEX_NONE;
    return Pool.FindLastChar(TEXT('.'), Dot) ? Pool.Mid(Dot + 1) : Pool;
}

// Drop spot Index (host stand-in): points on rings around the enemy's collision centre, starting at spot Index of the first ring;
// a point needs a floor under it and room above it (no world geometry in a DropRoomRadius sphere just above the floor). The
// first point that has both wins, DropLift above its floor; when none has, the enemy's feet.
FVector DropSpot(UWorld* World, const AOpenWillowEnemy* Enemy, int32 Index)
{
    FVector Centre = FVector::ZeroVector;
    float HalfHeight = 0.f;
    Enemy->CollisionCentre(Centre, HalfHeight);
    FCollisionObjectQueryParams Solid;
    Solid.AddObjectTypesToQuery(ECC_WorldStatic);
    Solid.AddObjectTypesToQuery(ECC_WorldDynamic);
    FCollisionQueryParams Query(SCENE_QUERY_STAT(OpenWillowRangeDropRoom), false, Enemy);
    for (int32 Try = 0; Try < DropSpots * DropRings; ++Try)
    {
        const int32 Ring = Try / DropSpots;
        const float Angle = FMath::DegreesToRadians(360.f / DropSpots * float((Index + Try) % DropSpots));
        const FVector2D XY = FVector2D(Centre.X, Centre.Y) + FVector2D(FMath::Cos(Angle), FMath::Sin(Angle)) * DropRadius * (Ring + 1);
        FVector Floor = FVector::ZeroVector;
        if (!AOpenWillowEnemy::TraceFloor(World, XY, Centre.Z + 100.f, Centre.Z - 600.f, Enemy, Floor)) continue;
        const FVector Room = Floor + FVector(0.f, 0.f, DropLift + DropRoomRadius + 2.f);
        if (World->OverlapAnyTestByObjectType(Room, FQuat::Identity, Solid, FCollisionShape::MakeSphere(DropRoomRadius), Query)) continue;
        return Floor + FVector(0.f, 0.f, DropLift);
    }
    UE_LOG(LogTemp, Display, TEXT("OWRANGE loot drop_spot no room near %s: at its feet"), *Enemy->GetName());
    return FVector(Centre.X, Centre.Y, Centre.Z - HalfHeight + DropLift);
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
    Pickup->SetLifeSpan(WeaponDropLifeSeconds);   // a range drop expires (host choice); the inventory's own drops do not
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
        const int32 After = SaturatingAdd(Before, Amount);
        Inventory->SetMoney(After);
        UE_LOG(LogTemp, Display, TEXT("OWRANGE pickup kind=money amount=%d money=%d->%d"), Amount, Before, After);
    }
    else
    {
        const int32 Before = Inventory->AmmoPool(AmmoType).Current;
        Inventory->SetAmmoCurrent(AmmoType, SaturatingAdd(Before, Amount));   // clamped to the pool's maximum by the inventory
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
    // The item stage of this death: one draw in the kind's range (the adult's random offset), from the loot stream, before the
    // pools (host order, UNVERIFIED; the game draws the offset at spawn).
    const int32 ItemStage = Entry->ItemStageMax > Entry->ItemStageMin
        ? Stream.RandRange(Entry->ItemStageMin, Entry->ItemStageMax) : FMath::Max(Entry->ItemStage, Entry->ItemStageMin);
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
            // The gun's seed is drawn for every tried weapon pool, with or without the generator, so the later draws of the death
            // (money, ammo) do not depend on whether the generator's manifest is installed.
            const uint32 Seed = Stream.GetUnsignedInt();
            if (!FOpenWillowWeaponGen::IsAvailable())
            {
                What = TEXT("weapon_unavailable");
                UE_LOG(LogTemp, Display, TEXT("OWRANGE loot weapon_unavailable enemy=%s pool=%s"), *KindName, *Leaf);
            }
            else
            {
                FOpenWillowWeaponItem Item;
                // One roll as the stock death does; in always mode (gun testing) up to AlwaysWeaponTries seeds, so a pick that is a
                // shield or a grenade mod does not leave the tester without a gun. The tries' seeds follow the first one, so the loot
                // stream is not touched again.
                const int32 Tries = Mode == EOpenWillowRangeLootMode::Always ? AlwaysWeaponTries : 1;
                bool bRolled = false;
                for (int32 Try = 0; Try < Tries && !bRolled; ++Try)
                {
                    TArray<FString> Drops;
                    bRolled = FOpenWillowWeaponGen::RollPoolWeapon(Pool.Pool, ItemStage, Seed + uint32(Try) * 7919u, Item, &Drops);
                    UE_LOG(LogTemp, Display, TEXT("OWRANGE loot weapon_try %d pool=%s stage=%d gun=%d drops=%s"), Try, *Leaf, ItemStage,
                        bRolled ? 1 : 0, *FString::Join(Drops, TEXT(",")));
                }
                if (bRolled)
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
    UE_LOG(LogTemp, Display, TEXT("OWRANGE loot enemy=%s level=%d item_stage=%d weapons=%d money=%d ammo=%d"), *KindName, Level,
        ItemStage, Result.Weapons, Result.Money, Result.Ammo);
    return Result;
}
