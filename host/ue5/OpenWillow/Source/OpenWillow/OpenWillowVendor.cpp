// AI-assisted (Claude). See OpenWillowVendor.h. Stock rules first, then the machine actor, the subsystem and the
// ow.Vendor.* console commands.
#include "OpenWillowVendor.h"
#include "OpenWillowVendorPage.h"
#include "OpenWillowWalker.h"
#include "OpenWillowQuest.h"
#include "OpenWillowWeaponGen.h"
#include "Components/SceneComponent.h"
#include "Components/StaticMeshComponent.h"
#include "Components/TextRenderComponent.h"
#include "Engine/Engine.h"
#include "Engine/StaticMesh.h"
#include "Engine/World.h"
#include "GameFramework/PlayerController.h"
#include "HAL/IConsoleManager.h"
#include "Math/RandomStream.h"
#include "TimerManager.h"
#include "UnrealClient.h"
#include "Misc/App.h"
#include "GenericPlatform/GenericPlatformMisc.h"
#include "Misc/CommandLine.h"
#include "Misc/FileHelper.h"
#include "Misc/Parse.h"
#include "Dom/JsonObject.h"
#include "Serialization/JsonReader.h"
#include "Serialization/JsonSerializer.h"
#include "Serialization/JsonWriter.h"
#include "UObject/ConstructorHelpers.h"

namespace
{
// Only weapon balances are rolled in PR 1; ammo and health machines come later.
bool IsWeaponLeaf(const FOpenWillowVendorLeaf& Leaf)
{
    return Leaf.Class == TEXT("WeaponBalanceDefinition");
}

// A manifest number as an int32 in [Low, High]; false when it is not finite, not whole or out of range (the cast to
// int32 would otherwise be undefined or wrap).
bool ManifestInt(double Number, int32 Low, int32 High, int32& Out)
{
    if (!FMath::IsFinite(Number) || Number != FMath::FloorToDouble(Number) || Number < double(Low) || Number > double(High))
        return false;
    Out = int32(Number);
    return true;
}

// Money never wraps: sums are clamped to [0, MAX_int32].
int32 SaturatingAdd(int32 A, int32 B)
{
    return int32(FMath::Clamp(int64(A) + int64(B), int64(0), int64(MAX_int32)));
}

// Pools in the manifest: each pool has a probability and a list of leaf balances with weights.
void ReadPools(const TArray<TSharedPtr<FJsonValue>>& Pools, TArray<FOpenWillowVendorLeaf>& Out)
{
    Out.Reset();
    for (const TSharedPtr<FJsonValue>& PoolValue : Pools)
    {
        const TSharedPtr<FJsonObject>* Pool = nullptr;
        if (!PoolValue.IsValid() || !PoolValue->TryGetObject(Pool) || !Pool->IsValid()) continue;
        double Probability = 1.0;
        (*Pool)->TryGetNumberField(TEXT("probability"), Probability);
        const TArray<TSharedPtr<FJsonValue>>* Items = nullptr;
        if (!(*Pool)->TryGetArrayField(TEXT("items"), Items)) continue;
        for (const TSharedPtr<FJsonValue>& ItemValue : *Items)
        {
            const TSharedPtr<FJsonObject>* Item = nullptr;
            if (!ItemValue.IsValid() || !ItemValue->TryGetObject(Item) || !Item->IsValid()) continue;
            FOpenWillowVendorLeaf Leaf;
            (*Item)->TryGetStringField(TEXT("balance"), Leaf.Balance);
            (*Item)->TryGetStringField(TEXT("class"), Leaf.Class);
            double Weight = 0;
            (*Item)->TryGetNumberField(TEXT("weight"), Weight);
            Leaf.Weight = float(Probability * Weight);
            if (!Leaf.Balance.IsEmpty() && FMath::IsFinite(Leaf.Weight) && Leaf.Weight > 0) Out.Add(Leaf);
        }
    }
}

// [x, y, z] of numbers from the manifest, as-is.
bool ReadVector(const TSharedPtr<FJsonObject>& Obj, const TCHAR* Key, FVector& Out)
{
    const TArray<TSharedPtr<FJsonValue>>* Values = nullptr;
    if (!Obj->TryGetArrayField(Key, Values) || Values->Num() != 3) return false;
    for (int32 I = 0; I < 3; ++I)
    {
        double N = 0;
        if (!(*Values)[I].IsValid() || !(*Values)[I]->TryGetNumber(N) || !FMath::IsFinite(N)) return false;
        Out[I] = N;
    }
    return true;
}

// One line of JSON: TJsonWriter<> defaults to the pretty policy, which splits a report over hundreds of log lines.
FString Compact(const TSharedRef<FJsonObject>& Object)
{
    FString Out;
    const TSharedRef<TJsonWriter<TCHAR, TCondensedJsonPrintPolicy<TCHAR>>> Writer =
        TJsonWriterFactory<TCHAR, TCondensedJsonPrintPolicy<TCHAR>>::Create(&Out);
    FJsonSerializer::Serialize(Object, Writer);
    return Out;
}

// Card keys as inventory.js reads them (the same names UOpenWillowInventory::StateJson writes).
TSharedRef<FJsonObject> CardJson(const FOpenWillowWeaponItem& Gun)
{
    TSharedRef<FJsonObject> Card = MakeShared<FJsonObject>();
    Card->SetStringField(TEXT("type"), Gun.Type);
    Card->SetStringField(TEXT("manufacturer"), Gun.Manufacturer);
    Card->SetStringField(TEXT("element"), Gun.Element);
    if (!Gun.RarityColor.IsEmpty()) Card->SetStringField(TEXT("rarityColor"), Gun.RarityColor);
    Card->SetNumberField(TEXT("damage"), Gun.Damage);
    Card->SetNumberField(TEXT("fireRate"), Gun.FireRate);
    Card->SetNumberField(TEXT("reloadTime"), Gun.ReloadTime);
    Card->SetNumberField(TEXT("magazine"), Gun.Magazine);
    Card->SetNumberField(TEXT("spread"), Gun.Spread);
    if (Gun.bHasAccuracy) Card->SetNumberField(TEXT("accuracy"), Gun.Accuracy);
    Card->SetBoolField(TEXT("accuracyKnown"), Gun.bHasAccuracy && Gun.bAccuracyKnown);
    if (Gun.bHasSaleValue) Card->SetNumberField(TEXT("value"), Gun.SaleValue);
    Card->SetBoolField(TEXT("valueKnown"), Gun.bHasSaleValue && Gun.bSaleValueKnown);
    if (!Gun.FunStats.IsEmpty()) Card->SetStringField(TEXT("funStats"), Gun.FunStats);
    if (!Gun.FunStatsMarkup.IsEmpty()) Card->SetStringField(TEXT("funStatsMarkup"), Gun.FunStatsMarkup);
    if (Gun.Projectiles > 1) Card->SetNumberField(TEXT("projectiles"), Gun.Projectiles);
    return Card;
}

// One shop or buy-back row for the page.
// Price unset: no valid price (see BuyPrice); the row says so ("priceValid": false) and is never affordable.
TSharedRef<FJsonObject> ShopItemJson(const FOpenWillowVendorItem& Item, TOptional<int32> Price, int32 Money)
{
    TSharedRef<FJsonObject> Json = MakeShared<FJsonObject>();
    Json->SetStringField(TEXT("id"), Item.Id);
    Json->SetStringField(TEXT("name"), Item.Name);
    Json->SetStringField(TEXT("kind"), Item.Kind);
    Json->SetNumberField(TEXT("rarity"), Item.Rarity);
    Json->SetNumberField(TEXT("level"), Item.Level);
    Json->SetNumberField(TEXT("price"), Price.Get(0));
    Json->SetBoolField(TEXT("priceValid"), Price.IsSet());
    Json->SetNumberField(TEXT("value"), Item.SaleValue);   // the price formula's input, for the checker
    if (Item.bHasWeapon) Json->SetStringField(TEXT("gun"), Item.Weapon.Id);   // gen_<balance>_<stage>_<seed>: one per roll
    Json->SetBoolField(TEXT("affordable"), Price.IsSet() && Money >= Price.GetValue());
    Json->SetBoolField(TEXT("featured"), Item.bFeatured);
    Json->SetNumberField(TEXT("quantity"), 1);
    if (Item.bHasWeapon) Json->SetObjectField(TEXT("card"), CardJson(Item.Weapon));
    return Json;
}

// One backpack row for the page.
TSharedRef<FJsonObject> BackpackJson(const FOpenWillowWeaponItem& Gun, bool bEquipped)
{
    TSharedRef<FJsonObject> Json = MakeShared<FJsonObject>();
    Json->SetStringField(TEXT("id"), UOpenWillowInventory::StableId(Gun));
    Json->SetStringField(TEXT("kind"), TEXT("weapon")); // the page validates every row's kind (vending_model.js ITEM_KINDS)
    Json->SetStringField(TEXT("name"), Gun.Name);
    Json->SetNumberField(TEXT("rarity"), Gun.Rarity);
    Json->SetNumberField(TEXT("level"), Gun.Level);
    Json->SetNumberField(TEXT("sell"), Gun.bHasSaleValue ? Gun.SaleValue : 0);
    Json->SetBoolField(TEXT("equipped"), bEquipped);
    Json->SetObjectField(TEXT("card"), CardJson(Gun));
    return Json;
}

// Reach of the use key (GlobalsDefinition.PlayerInteractionDistance, 350 uu, NATIVE_USE_INTERACTION.md). The native
// use trace is not modelled: this is a distance check from the walker's origin, a labelled UNVERIFIED stand-in.
constexpr float InteractionReach = 350.f;
}

// ---- Stock rules -----------------------------------------------------------------------------------------------------

bool FOpenWillowVendorStock::Load(const TSharedPtr<FJsonObject>& Machine, int32 InStage, int32 InMinutesBetweenResets, FString& OutError)
{
    if (!Machine.IsValid())
    {
        OutError = TEXT("machine object missing");
        return false;
    }
    if (!Machine->TryGetStringField(TEXT("kind"), Kind))
    {
        OutError = TEXT("machine kind missing");
        return false;
    }
    Stage = InStage;
    MinutesBetweenResets = InMinutesBetweenResets > 0 ? InMinutesBetweenResets : DefaultMinutesBetweenResets;
    double Number = 0;
    FixedItemCost = FixedFeaturedCost = 0;
    if (Machine->TryGetNumberField(TEXT("fixed_item_cost"), Number) && !ManifestInt(Number, 0, MAX_int32, FixedItemCost))
    {
        OutError = TEXT("fixed_item_cost out of range");
        return false;
    }
    if (Machine->TryGetNumberField(TEXT("fixed_featured_cost"), Number) && !ManifestInt(Number, 0, MAX_int32, FixedFeaturedCost))
    {
        OutError = TEXT("fixed_featured_cost out of range");
        return false;
    }
    const TArray<TSharedPtr<FJsonValue>>* Pools = nullptr;
    if (Machine->TryGetArrayField(TEXT("inventory"), Pools)) ReadPools(*Pools, Leaves);
    else Leaves.Reset();
    if (Machine->TryGetArrayField(TEXT("featured"), Pools)) ReadPools(*Pools, FeaturedLeaves);
    else FeaturedLeaves.Reset();
    return true;
}

FOpenWillowVendorItem FOpenWillowVendorStock::MakeItem(const FOpenWillowWeaponItem& Gun, const FString& Id, bool bFeatured)
{
    FOpenWillowVendorItem Item;
    Item.Id = Id;
    Item.Kind = TEXT("weapon");
    Item.Name = Gun.Name;
    Item.Level = Gun.Level;
    Item.Rarity = Gun.Rarity;
    Item.SaleValue = Gun.bHasSaleValue ? Gun.SaleValue : 0;
    Item.bFeatured = bFeatured;
    Item.bHasWeapon = true;
    Item.Weapon = Gun;
    return Item;
}

TOptional<int32> FOpenWillowVendorStock::PriceOf(const FOpenWillowVendorItem& Item) const
{
    const int32 Fixed = Item.bFeatured ? FixedFeaturedCost : FixedItemCost;
    if (Fixed > 0) return Fixed;
    return BuyPrice(Item.SaleValue, Item.Level);
}

TOptional<int32> FOpenWillowVendorStock::BuyPrice(int32 Value, int32 Level)
{
    // price = floor(value * (level * SellExponentializer + BaseMarkupIncrease)), in single precision. Confirmed live
    // 2026-11-03: 79 of 79 stock and featured prices of Sanctuary's eight machines at player level 8
    // (docs/verification/VENDORS.md section 3).
    const float Markup = float(Level) * SellExponentializer + BaseMarkupIncrease;
    const float Price = float(Value) * Markup;
    // A price that is not a finite, non-negative int32 (absurd value or level) is no price at all: unset, refused by
    // Buy and shown as not for sale, never a sentinel that a large purse could afford.
    if (!FMath::IsFinite(Price) || Price < 0.f || Price >= 2147483520.f) return {}; // largest float below 2^31
    return FMath::FloorToInt(Price);
}

bool FOpenWillowVendorStock::NeedsRestock(double Now) const
{
    return !bStocked || Now - StockedAt >= double(MinutesBetweenResets) * 60.0;
}

double FOpenWillowVendorStock::SecondsToReset(double Now) const
{
    if (!bStocked) return 0;
    return FMath::Max(0.0, StockedAt + double(MinutesBetweenResets) * 60.0 - Now);
}

bool FOpenWillowVendorStock::RollWeapon(const TArray<FOpenWillowVendorLeaf>& Pool, uint32 GunSeed, FOpenWillowWeaponItem& OutGun) const
{
    // Weapon leaves only, picked by weight with a seeded stream, then rolled by the weapon generator.
    TArray<const FOpenWillowVendorLeaf*> Weapons;
    float Total = 0;
    for (const FOpenWillowVendorLeaf& Leaf : Pool)
    {
        if (!IsWeaponLeaf(Leaf) || Leaf.Weight <= 0) continue;
        Weapons.Add(&Leaf);
        Total += Leaf.Weight;
    }
    if (Weapons.IsEmpty()) return false;
    FRandomStream Rng{static_cast<int32>(GunSeed)};
    float Pick = Rng.FRand() * Total;
    const FOpenWillowVendorLeaf* Chosen = Weapons.Last();
    for (const FOpenWillowVendorLeaf* Leaf : Weapons)
    {
        if (Pick < Leaf->Weight)
        {
            Chosen = Leaf;
            break;
        }
        Pick -= Leaf->Weight;
    }
    return FOpenWillowWeaponGen::RollBalance(Chosen->Balance, Stage, GunSeed, OutGun);
}

void FOpenWillowVendorStock::Restock(double Now)
{
    Items.Reset();
    Featured = FOpenWillowVendorItem();
    bHasFeatured = false;
    Notice.Reset();
    bStocked = true;
    StockedAt = Now;
    const uint32 RestockSeed = Seed + uint32(RestockCount) * 1009u;
    ++RestockCount;
    if (Kind != TEXT("weapons"))
    {
        Notice = TEXT("Not stocked yet");
        return;
    }
    if (!Leaves.ContainsByPredicate([](const FOpenWillowVendorLeaf& Leaf) { return IsWeaponLeaf(Leaf); }))
    {
        Notice = TEXT("No weapons listed");
        return;
    }
    for (int32 I = 0; I < StockWeaponCount; ++I)
    {
        FOpenWillowWeaponItem Gun;
        if (!RollWeapon(Leaves, RestockSeed + 1u + uint32(I), Gun))
        {
            Notice = TEXT("No weapon could be rolled");
            break;
        }
        Items.Add(MakeItem(Gun, FString::Printf(TEXT("stock_%d"), Items.Num()), false));
    }
    FOpenWillowWeaponItem FeaturedGun;
    if (RollWeapon(FeaturedLeaves, RestockSeed + 1000u, FeaturedGun))
    {
        Featured = MakeItem(FeaturedGun, TEXT("featured"), true);
        bHasFeatured = true;
    }
}

const FOpenWillowVendorItem* FOpenWillowVendorStock::FindShopItem(const FString& Id) const
{
    if (const FOpenWillowVendorItem* Found = Items.FindByPredicate([&Id](const FOpenWillowVendorItem& Item) { return Item.Id == Id; }))
        return Found;
    if (bHasFeatured && Featured.Id == Id) return &Featured;
    return nullptr;
}

bool FOpenWillowVendorStock::Buy(FString Id, UOpenWillowInventory* Inventory, FString& OutError)
{
    const FOpenWillowVendorItem* Found = FindShopItem(Id);
    if (!Found || !Inventory)
    {
        OutError = TEXT("Unknown item");
        return false;
    }
    const FOpenWillowVendorItem Item = *Found; // a copy: the shelf changes below
    if (!Item.bHasWeapon)
    {
        OutError = TEXT("Not for sale yet");
        return false;
    }
    int32 Money = 0;
    Inventory->GetMoney(Money); // unknown money counts as 0
    const TOptional<int32> PriceOrUnset = PriceOf(Item);
    if (!PriceOrUnset.IsSet())
    {
        OutError = TEXT("No valid price");
        return false;
    }
    const int32 Price = PriceOrUnset.GetValue();
    if (Money < Price)
    {
        OutError = TEXT("Insufficient funds");
        return false;
    }
    TSet<FString> Before;
    for (const FOpenWillowWeaponItem& Owned : Inventory->Items()) Before.Add(UOpenWillowInventory::StableId(Owned));
    if (!Inventory->CanAddToBackpack() || !Inventory->AddToBackpack(Item.Weapon))
    {
        OutError = TEXT("Backpack full");
        return false;
    }
    // The backpack may give the copy a new instance id ("<id>#2"), so take the id that was not there before.
    for (const FOpenWillowWeaponItem& Owned : Inventory->Items())
        if (!Before.Contains(UOpenWillowInventory::StableId(Owned))) LastBoughtId = UOpenWillowInventory::StableId(Owned);
    Inventory->SetMoney(Money - Price);
    if (Item.bFeatured) bHasFeatured = false;
    else Items.RemoveAll([&Id](const FOpenWillowVendorItem& Shelf) { return Shelf.Id == Id; });
    return true;
}

bool FOpenWillowVendorStock::Sell(FString Id, UOpenWillowInventory* Inventory, FString& OutError,
    const TFunction<bool(const FOpenWillowWeaponItem&)>& IsProtected)
{
    const FOpenWillowWeaponItem* Owned = Inventory ? Inventory->FindItemById(Id) : nullptr;
    if (!Owned)
    {
        OutError = TEXT("Unknown item");
        return false;
    }
    if (IsProtected && IsProtected(*Owned))
    {
        OutError = TEXT("Mission items cannot be sold");
        return false;
    }
    for (int32 Slot = 0; Slot < UOpenWillowInventory::SlotCount; ++Slot)
    {
        const FOpenWillowWeaponItem* Equipped = Inventory->SlotItem(Slot);
        if (Equipped && UOpenWillowInventory::StableId(*Equipped) == Id)
        {
            OutError = TEXT("Equipped items cannot be sold");
            return false;
        }
    }
    FOpenWillowTakenInventoryItem Taken;
    if (!Inventory->TakeById(Id, Taken) || Taken.bIsGear)
    {
        OutError = TEXT("Unknown item");
        return false;
    }
    const FOpenWillowVendorItem Sold = MakeItem(Taken.Weapon, FString::Printf(TEXT("buyback_%d"), ++BuybackSerial), false);
    Buyback.Add(Sold);
    while (Buyback.Num() > BuybackCapacity) Buyback.RemoveAt(0);
    int32 Money = 0;
    Inventory->GetMoney(Money);
    Inventory->SetMoney(SaturatingAdd(Money, SellPriceOf(Sold)));
    return true;
}

bool FOpenWillowVendorStock::BuyBack(FString Id, UOpenWillowInventory* Inventory, FString& OutError)
{
    const int32 Index = Buyback.IndexOfByPredicate([&Id](const FOpenWillowVendorItem& Item) { return Item.Id == Id; });
    if (Index == INDEX_NONE || !Inventory)
    {
        OutError = TEXT("Unknown item");
        return false;
    }
    const FOpenWillowVendorItem Item = Buyback[Index];
    const int32 Cost = SellPriceOf(Item);
    int32 Money = 0;
    Inventory->GetMoney(Money);
    if (Money < Cost)
    {
        OutError = TEXT("Insufficient funds");
        return false;
    }
    if (!Inventory->CanAddToBackpack() || !Inventory->AddToBackpack(Item.Weapon))
    {
        OutError = TEXT("Backpack full");
        return false;
    }
    Inventory->SetMoney(Money - Cost);
    Buyback.RemoveAt(Index);
    return true;
}

// ---- Machine actor ---------------------------------------------------------------------------------------------------

AOpenWillowVendingMachine::AOpenWillowVendingMachine()
{
    PrimaryActorTick.bCanEverTick = false;
    USceneComponent* Root = CreateDefaultSubobject<USceneComponent>(TEXT("Root"));
    SetRootComponent(Root);
    Body = CreateDefaultSubobject<UStaticMeshComponent>(TEXT("Body"));
    Body->SetupAttachment(Root);
    // UNVERIFIED stand-in: the engine cube, scaled to about 120 x 80 x 211 cm and lifted so its base sits on the placed point.
    static ConstructorHelpers::FObjectFinder<UStaticMesh> Cube(TEXT("/Engine/BasicShapes/Cube.Cube"));
    if (Cube.Succeeded()) Body->SetStaticMesh(Cube.Object);
    Body->SetRelativeScale3D(FVector(1.2f, 0.8f, 2.11f));
    Body->SetRelativeLocation(FVector(0.f, 0.f, 105.5f));
    Label = CreateDefaultSubobject<UTextRenderComponent>(TEXT("Label"));
    Label->SetupAttachment(Root);
    Label->SetRelativeLocation(FVector(0.f, 0.f, 240.f));
    Label->SetWorldSize(40.f);
}

void AOpenWillowVendingMachine::Configure(const FString& InId, const FString& InTitle, const FString& InKind)
{
    Id = InId;
    Title = InTitle;
    Kind = InKind;
    if (Label) Label->SetText(FText::FromString(Title));
}

// ---- Subsystem -------------------------------------------------------------------------------------------------------

void UOpenWillowVendorSubsystem::OnWorldBeginPlay(UWorld& InWorld)
{
    Super::OnWorldBeginPlay(InWorld);
    if (!InWorld.IsGameWorld()) return;
    FString Path;
    if (!FParse::Value(FCommandLine::Get(), TEXT("owvendors="), Path))
    {
        UE_LOG(LogTemp, Display, TEXT("OpenWillow vendors: off (no -owvendors=<vendors.json>)"));
        return;
    }
    FString Text;
    if (!FFileHelper::LoadFileToString(Text, *Path))
    {
        UE_LOG(LogTemp, Warning, TEXT("OpenWillow vendors: cannot read %s; no machines"), *Path);
        return;
    }
    TSharedPtr<FJsonObject> Root;
    if (!FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Root) || !Root.IsValid())
    {
        UE_LOG(LogTemp, Warning, TEXT("OpenWillow vendors: %s is not JSON; no machines"), *Path);
        return;
    }
    FString Schema;
    Root->TryGetStringField(TEXT("schema"), Schema);
    if (Schema != TEXT("openwillow-vendors/1"))
    {
        UE_LOG(LogTemp, Warning, TEXT("OpenWillow vendors: schema '%s' is not openwillow-vendors/1; no machines"), *Schema);
        return;
    }
    double StageNumber = 1, MinutesNumber = FOpenWillowVendorStock::DefaultMinutesBetweenResets;
    Root->TryGetNumberField(TEXT("stage"), StageNumber);
    Root->TryGetNumberField(TEXT("minutes_between_resets"), MinutesNumber);
    int32 Stage = 1, Minutes = FOpenWillowVendorStock::DefaultMinutesBetweenResets;
    if (!ManifestInt(StageNumber, 1, 100, Stage) || !ManifestInt(MinutesNumber, 1, 24 * 60, Minutes))
    {
        UE_LOG(LogTemp, Warning, TEXT("OpenWillow vendors: stage %g or minutes_between_resets %g out of range in %s; no machines"),
            StageNumber, MinutesNumber, *Path);
        return;
    }
    const TArray<TSharedPtr<FJsonValue>>* MachinesJson = nullptr;
    if (!Root->TryGetArrayField(TEXT("machines"), MachinesJson))
    {
        UE_LOG(LogTemp, Warning, TEXT("OpenWillow vendors: no machines list in %s"), *Path);
        return;
    }
    Machines.Reset();
    for (const TSharedPtr<FJsonValue>& Value : *MachinesJson)
    {
        const TSharedPtr<FJsonObject>* MachineObject = nullptr;
        if (!Value.IsValid() || !Value->TryGetObject(MachineObject) || !MachineObject->IsValid()) continue;
        const TSharedPtr<FJsonObject>& Json = *MachineObject;
        FString Id, Title, Kind;
        Json->TryGetStringField(TEXT("id"), Id);
        Json->TryGetStringField(TEXT("title"), Title);
        Json->TryGetStringField(TEXT("kind"), Kind);
        bool bSupported = true;
        Json->TryGetBoolField(TEXT("supported"), bSupported);
        if (!bSupported)
        {
            UE_LOG(LogTemp, Display, TEXT("OpenWillow vendors: %s (%s, %s) is not supported yet; not spawned"), *Id, *Title, *Kind);
            continue;
        }
        FVector Location, Rotation;
        if (!ReadVector(Json, TEXT("location"), Location) || !ReadVector(Json, TEXT("rotation"), Rotation))
        {
            UE_LOG(LogTemp, Warning, TEXT("OpenWillow vendors: %s has no usable location/rotation; not spawned"), *Id);
            continue;
        }
        // Same convention as the slice manifests' "host" block: location as-is, rotation in degrees [Pitch, Yaw, Roll].
        const FTransform Pose(FRotator(Rotation.X, Rotation.Y, Rotation.Z), Location);
        FActorSpawnParameters Params;
        Params.SpawnCollisionHandlingOverride = ESpawnActorCollisionHandlingMethod::AlwaysSpawn;
        AOpenWillowVendingMachine* Machine = InWorld.SpawnActor<AOpenWillowVendingMachine>(AOpenWillowVendingMachine::StaticClass(), Pose, Params);
        if (!Machine) continue;
        Machine->Configure(Id, Title, Kind);
        FString Error;
        if (!Machine->Stock.Load(Json, Stage, Minutes, Error))
        {
            UE_LOG(LogTemp, Warning, TEXT("OpenWillow vendors: %s stock not loaded: %s"), *Id, *Error);
        }
        // Each machine rolls its own guns: the seed comes from its id, which is stable across runs.
        Machine->Stock.Seed = GetTypeHash(Id);
        Machines.Add(Machine);
    }
    UE_LOG(LogTemp, Display, TEXT("OpenWillow vendors: %d machine(s) spawned from %s"), Machines.Num(), *Path);
    if (FParse::Param(FCommandLine::Get(), TEXT("owvendortest")))
        InWorld.GetTimerManager().SetTimer(TestTimer, FTimerDelegate::CreateUObject(this, &UOpenWillowVendorSubsystem::TickVendorTest),
            2.f, true, 5.f); // one step every 2 s once the walker is there (the page needs a moment to load)
}

void UOpenWillowVendorSubsystem::Deinitialize()
{
    // Give input back and hide the page before it goes (a controller can outlive the world), and stop both timers.
    if (Page.IsValid() && Page->IsShown()) CloseShop();
    if (UWorld* World = GetWorld())
    {
        World->GetTimerManager().ClearTimer(OpenTimer);
        World->GetTimerManager().ClearTimer(TestTimer);
    }
    Page.Reset(); // removes the page from the viewport
    Machines.Reset();
    ActiveMachine.Reset();
    ActiveInventory.Reset();
    ActiveController.Reset();
    Super::Deinitialize();
}

bool UOpenWillowVendorSubsystem::TryUseNearest(AOpenWillowWalker& Walker)
{
    UWorld* World = Walker.GetWorld();
    UOpenWillowVendorSubsystem* Subsystem = World ? World->GetSubsystem<UOpenWillowVendorSubsystem>() : nullptr;
    return Subsystem && Subsystem->UseNearest(Walker);
}

bool UOpenWillowVendorSubsystem::NearestInReach(const AOpenWillowWalker& Walker, double& OutDistanceSquared)
{
    const UWorld* World = Walker.GetWorld();
    const UOpenWillowVendorSubsystem* Subsystem = World ? World->GetSubsystem<UOpenWillowVendorSubsystem>() : nullptr;
    if (!Subsystem) return false;
    if (Subsystem->Page.IsValid() && Subsystem->Page->IsShown())
    {
        OutDistanceSquared = 0;
        return true;
    }
    bool bFound = false;
    OutDistanceSquared = FMath::Square(double(InteractionReach));
    for (const TWeakObjectPtr<AOpenWillowVendingMachine>& Candidate : Subsystem->Machines)
    {
        const AOpenWillowVendingMachine* Machine = Candidate.Get();
        if (!Machine) continue;
        const double DistanceSquared = FVector::DistSquared(Walker.GetActorLocation(), Machine->GetActorLocation());
        if (DistanceSquared >= OutDistanceSquared) continue;
        OutDistanceSquared = DistanceSquared;
        bFound = true;
    }
    return bFound;
}

bool UOpenWillowVendorSubsystem::UseNearest(AOpenWillowWalker& Walker)
{
    // A second press while the shop is shown closes it (a backstop: until the page reports ready the game keeps input).
    if (Page.IsValid() && Page->IsShown()) { CloseShop(); return true; }
    const FVector WalkerLocation = Walker.GetActorLocation();
    AOpenWillowVendingMachine* Nearest = nullptr;
    double BestDistanceSquared = FMath::Square(double(InteractionReach));
    for (const TWeakObjectPtr<AOpenWillowVendingMachine>& Candidate : Machines)
    {
        AOpenWillowVendingMachine* Machine = Candidate.Get();
        if (!Machine) continue;
        const double DistanceSquared = FVector::DistSquared(WalkerLocation, Machine->GetActorLocation());
        if (DistanceSquared >= BestDistanceSquared) continue;
        BestDistanceSquared = DistanceSquared;
        Nearest = Machine;
    }
    return Nearest && OpenAt(Walker, *Nearest);
}

bool UOpenWillowVendorSubsystem::OpenAt(AOpenWillowWalker& Walker, AOpenWillowVendingMachine& Machine)
{
    APlayerController* PC = Cast<APlayerController>(Walker.GetController());
    UOpenWillowInventory* Inventory = Walker.GetInventory();
    if (!PC || !Inventory) return false;
    if (!Page.IsValid())
    {
        // The page reports through this callback; the weak pointer keeps a destroyed subsystem out of it.
        const TWeakObjectPtr<UOpenWillowVendorSubsystem> WeakThis(this);
        Page = MakeShared<FOpenWillowVendorPage>(
            [WeakThis](const FString& Line) { if (UOpenWillowVendorSubsystem* Subsystem = WeakThis.Get()) Subsystem->HandlePageMessage(Line); },
            [WeakThis]() { if (UOpenWillowVendorSubsystem* Subsystem = WeakThis.Get()) Subsystem->CloseShop(); });
    }
    ActiveMachine = &Machine;
    ActiveInventory = Inventory;
    ActiveController = PC;
    LastMessage.Reset();
    const bool bRestocked = Machine.Stock.EnsureStocked(GetWorld()->GetTimeSeconds());
    Page->Show(PC);
    PushState();
    // While open, push the state every second: the page's countdown is the host's world time, and a restock that falls
    // due while the shop is open shows at once.
    GetWorld()->GetTimerManager().SetTimer(OpenTimer, FTimerDelegate::CreateUObject(this, &UOpenWillowVendorSubsystem::PushState), 1.f, true);
    UE_LOG(LogTemp, Display, TEXT("OWVENDOR open id=%s kind=%s restocked=%d"), *Machine.Id, *Machine.Kind, bRestocked ? 1 : 0);
    return true;
}

void UOpenWillowVendorSubsystem::CloseShop()
{
    if (Page.IsValid()) Page->Hide(ActiveController.Get());
    if (UWorld* World = GetWorld()) World->GetTimerManager().ClearTimer(OpenTimer);
    ActiveMachine.Reset();
    ActiveInventory.Reset();
    ActiveController.Reset();
    LastMessage.Reset();
    UE_LOG(LogTemp, Display, TEXT("OWVENDOR close ok=1"));
}

void UOpenWillowVendorSubsystem::HandlePageMessage(const FString& Line)
{
    const FString Prefix = TEXT("OWVEND ");
    if (!Line.StartsWith(Prefix)) return;
    TSharedPtr<FJsonObject> Json;
    if (!FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Line.Mid(Prefix.Len())), Json) || !Json.IsValid())
    {
        UE_LOG(LogTemp, Warning, TEXT("OWVENDOR ignored a message that is not JSON: %s"), *Line);
        return;
    }
    FString Action, Id;
    double ReceivedSerial = -1;
    Json->TryGetNumberField(TEXT("serial"), ReceivedSerial);
    Json->TryGetStringField(TEXT("action"), Action);
    Json->TryGetStringField(TEXT("id"), Id);
    // The serial first, for close too: a close left over from an earlier opening must not close the shop just reopened.
    if (int64(ReceivedSerial) != Serial)
    {
        UE_LOG(LogTemp, Display, TEXT("OWVENDOR ignored stale %s (serial %lld, current %lld)"), *Action, int64(ReceivedSerial), Serial);
        return;
    }
    if (Action == TEXT("close"))
    {
        CloseShop();
        return;
    }
    FString Error;
    Command(Action, Id, Error);
}

bool UOpenWillowVendorSubsystem::Command(const FString& Action, FString Id, FString& OutError)
{
    AOpenWillowVendingMachine* Machine = ActiveMachine.Get();
    UOpenWillowInventory* Inventory = ActiveInventory.Get();
    bool bOk = false;
    if (!Machine || !Inventory) OutError = TEXT("No shop is open");
    else if (Id.IsEmpty() && Action != TEXT("close")) OutError = TEXT("No item given");
    // A restock that fell due is applied before the action, never after it. A buy aimed at the old shelf is refused:
    // its id may now name a different gun.
    else if (Machine->Stock.EnsureStocked(GetWorld()->GetTimeSeconds()) && Action == TEXT("buy"))
        OutError = TEXT("The stock changed");
    else if (Action == TEXT("buy")) bOk = Machine->Stock.Buy(Id, Inventory, OutError);
    else if (Action == TEXT("sell"))
        bOk = Machine->Stock.Sell(Id, Inventory, OutError, [this](const FOpenWillowWeaponItem& Item) { return IsQuestOwned(Item); });
    else if (Action == TEXT("buyback")) bOk = Machine->Stock.BuyBack(Id, Inventory, OutError);
    else OutError = TEXT("Unknown action");
    int32 Money = 0;
    if (Inventory) Inventory->GetMoney(Money);
    UE_LOG(LogTemp, Display, TEXT("OWVENDOR %s id=%s ok=%d money=%d error=%s"), *Action, *Id, bOk ? 1 : 0, Money, *OutError);
    LastMessage = bOk ? FString() : OutError;
    PushState();
    return bOk;
}

bool UOpenWillowVendorSubsystem::OpenByIndex(int32 Index, FString& OutError)
{
    if (!Machines.IsValidIndex(Index) || !Machines[Index].IsValid())
    {
        OutError = TEXT("No machine at that index");
        return false;
    }
    APlayerController* PC = GetWorld()->GetFirstPlayerController();
    AOpenWillowWalker* Walker = PC ? Cast<AOpenWillowWalker>(PC->GetPawn()) : nullptr;
    if (!Walker)
    {
        OutError = TEXT("No walker (start with -owwalk)");
        return false;
    }
    if (!OpenAt(*Walker, *Machines[Index].Get()))
    {
        OutError = TEXT("The shop did not open");
        return false;
    }
    return true;
}

bool UOpenWillowVendorSubsystem::BuyByIndex(int32 ShopIndex, FString& OutError)
{
    const AOpenWillowVendingMachine* Machine = ActiveMachine.Get();
    if (!Machine)
    {
        OutError = TEXT("No shop is open");
        return false;
    }
    if (!Machine->Stock.Items.IsValidIndex(ShopIndex))
    {
        OutError = TEXT("No shelf item at that index");
        return false;
    }
    return Command(TEXT("buy"), Machine->Stock.Items[ShopIndex].Id, OutError);
}

void UOpenWillowVendorSubsystem::RestockOpen()
{
    AOpenWillowVendingMachine* Machine = ActiveMachine.Get();
    if (!Machine) return;
    Machine->Stock.Restock(GetWorld()->GetTimeSeconds());
    LastMessage.Reset();
    UE_LOG(LogTemp, Display, TEXT("OWVENDOR restock id=%s ok=1"), *Machine->Id);
    PushState();
}

bool UOpenWillowVendorSubsystem::IsQuestOwned(const FOpenWillowWeaponItem& Item) const
{
    // The quest says which weapon it lent (and still owns); no item id is named here.
    const APlayerController* PC = ActiveController.Get();
    const AOpenWillowWalker* Walker = PC ? Cast<AOpenWillowWalker>(PC->GetPawn()) : nullptr;
    const UOpenWillowQuest* Quest = Walker ? Walker->GetQuest() : nullptr;
    return Quest && Quest->OwnsItem(Item);
}

bool UOpenWillowVendorSubsystem::SellLast(FString& OutError)
{
    const AOpenWillowVendingMachine* Machine = ActiveMachine.Get();
    if (!Machine || Machine->Stock.LastBoughtId.IsEmpty())
    {
        OutError = TEXT("Nothing bought from the open shop");
        return false;
    }
    return Command(TEXT("sell"), Machine->Stock.LastBoughtId, OutError);
}

void UOpenWillowVendorSubsystem::TickVendorTest()
{
    UWorld* World = GetWorld();
    APlayerController* PC = World ? World->GetFirstPlayerController() : nullptr;
    if (TestStep == 0 && !(PC && Cast<AOpenWillowWalker>(PC->GetPawn())))
    {
        if (++TestWaits > 60) { UE_LOG(LogTemp, Error, TEXT("OWVENDTEST FAIL no walker after 60 s")); FPlatformMisc::RequestExit(false); }
        return; // wait for the walker
    }
    FString Error;
    auto Report = [this]() { UE_LOG(LogTemp, Display, TEXT("OWVENDSTATE %s"), *ReportJson()); };
    switch (TestStep++)
    {
    case 0:
    {
        int32 Index = INDEX_NONE; // the first weapon machine: only those are stocked in PR 1
        for (int32 I = 0; I < Machines.Num() && Index == INDEX_NONE; ++I)
            if (Machines[I].IsValid() && Machines[I]->Kind == TEXT("weapons")) Index = I;
        UE_LOG(LogTemp, Display, TEXT("OWVENDTEST open index=%d"), Index);
        if (!OpenByIndex(Index, Error)) UE_LOG(LogTemp, Warning, TEXT("OWVENDTEST open refused: %s"), *Error);
        break;
    }
    case 1: Report(); break;
    case 2: if (!BuyByIndex(0, Error)) UE_LOG(LogTemp, Warning, TEXT("OWVENDTEST buy refused: %s"), *Error); break;
    case 3: Report(); break;
    case 4: if (!SellLast(Error)) UE_LOG(LogTemp, Warning, TEXT("OWVENDTEST sell refused: %s"), *Error); break;
    case 5: Report(); break;
    case 6: RestockOpen(); break;
    case 7: Report(); break;
    case 8: FScreenshotRequest::RequestScreenshot(TEXT("OWVendor_open.png"), false, false); break;
    case 10:
        UE_LOG(LogTemp, Display, TEXT("OWVENDTEST done"));
        World->GetTimerManager().ClearTimer(TestTimer);
        FPlatformMisc::RequestExit(false);
        break;
    default: break;
    }
}

void UOpenWillowVendorSubsystem::PushState()
{
    if (!Page.IsValid() || !Page->IsShown() || !ActiveMachine.IsValid()) return;
    ActiveMachine->Stock.EnsureStocked(GetWorld()->GetTimeSeconds());
    ++Serial;
    Page->SendState(StateJson());
}

FString UOpenWillowVendorSubsystem::ReportJson() const
{
    return StateJson();
}

FString UOpenWillowVendorSubsystem::StateJson() const
{
    const AOpenWillowVendingMachine* Machine = ActiveMachine.Get();
    const UOpenWillowInventory* Inventory = ActiveInventory.Get();
    TSharedRef<FJsonObject> State = MakeShared<FJsonObject>();
    State->SetNumberField(TEXT("serial"), double(Serial));
    if (!Machine || !Inventory)
    {
        State->SetBoolField(TEXT("open"), false);
        return Compact(State);
    }
    const double Now = GetWorld()->GetTimeSeconds();
    int32 Money = 0;
    Inventory->GetMoney(Money); // unknown money shows as 0
    const FOpenWillowVendorStock& Stock = Machine->Stock;

    State->SetBoolField(TEXT("open"), true);
    TSharedRef<FJsonObject> MachineJson = MakeShared<FJsonObject>();
    MachineJson->SetStringField(TEXT("id"), Machine->Id);
    MachineJson->SetStringField(TEXT("kind"), Stock.Kind);
    MachineJson->SetStringField(TEXT("title"), Machine->Title);
    State->SetObjectField(TEXT("machine"), MachineJson);
    State->SetNumberField(TEXT("money"), Money);
    State->SetNumberField(TEXT("eridium"), 0); // no eridium economy yet
    State->SetNumberField(TEXT("capacity"), Inventory->GetBackpackCapacity());
    State->SetNumberField(TEXT("used"), Inventory->BackpackCount());

    TArray<TSharedPtr<FJsonValue>> ShopJson;
    for (const FOpenWillowVendorItem& Item : Stock.Items)
        ShopJson.Add(MakeShared<FJsonValueObject>(ShopItemJson(Item, Stock.PriceOf(Item), Money)));
    State->SetArrayField(TEXT("shop"), ShopJson);
    if (Stock.bHasFeatured)
        State->SetObjectField(TEXT("featured"), ShopItemJson(Stock.Featured, Stock.PriceOf(Stock.Featured), Money));
    else
        State->SetField(TEXT("featured"), MakeShared<FJsonValueNull>());

    TArray<TSharedPtr<FJsonValue>> BackpackJsonList;
    for (const FOpenWillowWeaponItem& Gun : Inventory->Items())
    {
        bool bEquipped = false;
        for (int32 Slot = 0; Slot < UOpenWillowInventory::SlotCount; ++Slot)
        {
            const FOpenWillowWeaponItem* Held = Inventory->SlotItem(Slot);
            if (Held && UOpenWillowInventory::StableId(*Held) == UOpenWillowInventory::StableId(Gun)) bEquipped = true;
        }
        TSharedRef<FJsonObject> Row = BackpackJson(Gun, bEquipped);
        Row->SetBoolField(TEXT("sellable"), !bEquipped && !IsQuestOwned(Gun));
        BackpackJsonList.Add(MakeShared<FJsonValueObject>(Row));
    }
    State->SetArrayField(TEXT("backpack"), BackpackJsonList);

    TArray<TSharedPtr<FJsonValue>> BuybackJsonList;
    for (const FOpenWillowVendorItem& Item : Stock.Buyback)
        BuybackJsonList.Add(MakeShared<FJsonValueObject>(ShopItemJson(Item, TOptional<int32>(FOpenWillowVendorStock::SellPriceOf(Item)), Money)));
    State->SetArrayField(TEXT("buyback"), BuybackJsonList);

    State->SetNumberField(TEXT("resetSeconds"), FMath::FloorToInt(Stock.SecondsToReset(Now)));
    State->SetStringField(TEXT("message"), LastMessage.IsEmpty() ? Stock.Notice : LastMessage);
    return Compact(State);
}

// ---- Console commands ------------------------------------------------------------------------------------------------

namespace
{
UOpenWillowVendorSubsystem* ActiveVendorSubsystem()
{
    if (!GEngine) return nullptr;
    for (const FWorldContext& Context : GEngine->GetWorldContexts())
    {
        UWorld* World = Context.World();
        if (!World || !World->IsGameWorld()) continue;
        if (UOpenWillowVendorSubsystem* Subsystem = World->GetSubsystem<UOpenWillowVendorSubsystem>()) return Subsystem;
    }
    return nullptr;
}

void VendorOpenCommand(const TArray<FString>& Args)
{
    UOpenWillowVendorSubsystem* Subsystem = ActiveVendorSubsystem();
    if (!Subsystem)
    {
        UE_LOG(LogTemp, Warning, TEXT("ow.Vendor.Open: no game world"));
        return;
    }
    FString Error;
    if (!Subsystem->OpenByIndex(Args.IsValidIndex(0) ? FCString::Atoi(*Args[0]) : -1, Error))
        UE_LOG(LogTemp, Warning, TEXT("ow.Vendor.Open refused: %s"), *Error);
}

void VendorBuyCommand(const TArray<FString>& Args)
{
    UOpenWillowVendorSubsystem* Subsystem = ActiveVendorSubsystem();
    FString Error;
    if (!Subsystem || !Subsystem->BuyByIndex(Args.IsValidIndex(0) ? FCString::Atoi(*Args[0]) : -1, Error))
        UE_LOG(LogTemp, Warning, TEXT("ow.Vendor.Buy refused: %s"), Subsystem ? *Error : TEXT("no game world"));
}

void VendorSellCommand(const TArray<FString>& Args)
{
    UOpenWillowVendorSubsystem* Subsystem = ActiveVendorSubsystem();
    FString Error;
    if (!Subsystem || !Args.IsValidIndex(0) || !Subsystem->Command(TEXT("sell"), Args[0], Error))
        UE_LOG(LogTemp, Warning, TEXT("ow.Vendor.Sell refused: %s"), Subsystem ? *Error : TEXT("no game world"));
}

void VendorSellLastCommand(const TArray<FString>& /*Args*/)
{
    UOpenWillowVendorSubsystem* Subsystem = ActiveVendorSubsystem();
    FString Error;
    if (!Subsystem || !Subsystem->SellLast(Error))
        UE_LOG(LogTemp, Warning, TEXT("ow.Vendor.SellLast refused: %s"), Subsystem ? *Error : TEXT("no game world"));
}

void VendorRestockCommand(const TArray<FString>& /*Args*/)
{
    if (UOpenWillowVendorSubsystem* Subsystem = ActiveVendorSubsystem()) Subsystem->RestockOpen();
}

void VendorReportCommand(const TArray<FString>& /*Args*/)
{
    UOpenWillowVendorSubsystem* Subsystem = ActiveVendorSubsystem();
    UE_LOG(LogTemp, Display, TEXT("OWVENDSTATE %s"), Subsystem ? *Subsystem->ReportJson() : TEXT("{\"open\":false}"));
}

FAutoConsoleCommand GVendorOpenCommand(TEXT("ow.Vendor.Open"),
    TEXT("ow.Vendor.Open <index>: open the shop of the spawned machine <index>"),
    FConsoleCommandWithArgsDelegate::CreateStatic(&VendorOpenCommand));
FAutoConsoleCommand GVendorBuyCommand(TEXT("ow.Vendor.Buy"),
    TEXT("ow.Vendor.Buy <shelf index>: buy shelf item <index> of the open shop"),
    FConsoleCommandWithArgsDelegate::CreateStatic(&VendorBuyCommand));
FAutoConsoleCommand GVendorSellCommand(TEXT("ow.Vendor.Sell"),
    TEXT("ow.Vendor.Sell <backpack id>: sell a backpack item to the open shop"),
    FConsoleCommandWithArgsDelegate::CreateStatic(&VendorSellCommand));
FAutoConsoleCommand GVendorSellLastCommand(TEXT("ow.Vendor.SellLast"),
    TEXT("ow.Vendor.SellLast: sell the gun last bought from the open shop"),
    FConsoleCommandWithArgsDelegate::CreateStatic(&VendorSellLastCommand));
FAutoConsoleCommand GVendorRestockCommand(TEXT("ow.Vendor.Restock"),
    TEXT("ow.Vendor.Restock: force a restock of the open shop"),
    FConsoleCommandWithArgsDelegate::CreateStatic(&VendorRestockCommand));
FAutoConsoleCommand GVendorReportCommand(TEXT("ow.Vendor.Report"),
    TEXT("ow.Vendor.Report: log the open shop's state as OWVENDSTATE {json}"),
    FConsoleCommandWithArgsDelegate::CreateStatic(&VendorReportCommand));
}
