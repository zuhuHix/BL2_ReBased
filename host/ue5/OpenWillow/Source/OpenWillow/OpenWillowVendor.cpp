// AI-assisted (Claude). See OpenWillowVendor.h. Stock rules first, then the machine actor, the subsystem and the
// ow.Vendor.* console commands.
#include "OpenWillowVendor.h"
#include "OpenWillowVendorPage.h"
#include "OpenWillowMayaHUD.h"
#include "OpenWillowUseView.h"
#include "OpenWillowCurrency.h"
#include "OpenWillowWalker.h"
#include "OpenWillowMayaState.h"
#include "OpenWillowQuest.h"
#include "OpenWillowWeaponGen.h"
#include "Components/SceneComponent.h"
#include "Components/StaticMeshComponent.h"
#include "Components/TextRenderComponent.h"
#include "Engine/Engine.h"
#include "Engine/StaticMesh.h"
#include "Engine/World.h"
#include "GameFramework/PlayerController.h"
#include "HAL/FileManager.h"
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
// Only weapon balances are rolled as guns; the non-gun leaves carry the manifest's item block (see ReadLeafItem).
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

// The ammo pool key of the manifest (tools/prepare_vendors.py maps the pool to these) as the host's enum; false when unknown.
bool AmmoTypeFromKey(const FString& Key, EOpenWillowAmmoType& OutType)
{
    for (int32 Type = 0; Type < UOpenWillowInventory::AmmoTypeCount; ++Type)
    {
        if (Key == UOpenWillowInventory::AmmoTypeKey(EOpenWillowAmmoType(Type)))
        {
            OutType = EOpenWillowAmmoType(Type);
            return true;
        }
    }
    return false;
}

// A manifest number as a float, for the value terms and the heal fraction: false when it is not finite.
bool ManifestFloat(const TSharedPtr<FJsonObject>& Obj, const TCHAR* Key, float& Out)
{
    double Number = 0;
    if (!Obj->TryGetNumberField(Key, Number) || !FMath::IsFinite(Number)) return false;
    Out = float(Number);
    return true;
}

// The item block of one leaf (tools/prepare_vendors.py, docs/verification/VENDORS.md section 6). A block that does not
// read cleanly leaves the leaf with no kind, so it is never stocked (and never fails the whole machine).
void ReadLeafItem(const TSharedPtr<FJsonObject>& Item, FOpenWillowVendorLeaf& Leaf)
{
    if (!Item.IsValid() || !Item->TryGetStringField(TEXT("kind"), Leaf.Kind)) return;
    Item->TryGetStringField(TEXT("name"), Leaf.Name);
    const bool bKindOk = Leaf.Kind == TEXT("ammo") || Leaf.Kind == TEXT("health") || Leaf.Kind == TEXT("shield")
        || Leaf.Kind == TEXT("grenade_mod") || Leaf.Kind == TEXT("usable");
    if (!bKindOk)
    {
        Leaf.Kind.Reset();
        return;
    }
    // Ammo: the pool key and the rounds one purchase adds (a whole number in [1, 9999]).
    if (Leaf.Kind == TEXT("ammo"))
    {
        double Amount = 0;
        EOpenWillowAmmoType Type = EOpenWillowAmmoType::Pistol;
        if (!Item->TryGetStringField(TEXT("ammo_type"), Leaf.AmmoType) || !AmmoTypeFromKey(Leaf.AmmoType, Type)
            || !Item->TryGetNumberField(TEXT("amount"), Amount) || !ManifestInt(Amount, 1, 9999, Leaf.Amount))
        {
            Leaf.Kind.Reset();
            return;
        }
    }
    // Health: the fraction of max health one purchase heals, in (0, 1].
    if (Leaf.Kind == TEXT("health"))
    {
        const TSharedPtr<FJsonObject>* Heal = nullptr;
        if (!Item->TryGetObjectField(TEXT("heal"), Heal) || !Heal->IsValid() || !ManifestFloat(*Heal, TEXT("fraction_of_max_health"), Leaf.HealFraction)
            || Leaf.HealFraction <= 0.f || Leaf.HealFraction > 1.f)
        {
            Leaf.Kind.Reset();
            return;
        }
    }
    // Value terms: the first part total is the one the host uses (UNVERIFIED pick when a grade lists several).
    const TSharedPtr<FJsonObject>* Value = nullptr;
    if (!Item->TryGetObjectField(TEXT("value"), Value) || !Value->IsValid()) return; // null: no price data, never stocked
    FOpenWillowValueTerms& Terms = Leaf.Value;
    const TArray<TSharedPtr<FJsonValue>>* Totals = nullptr;
    if (ManifestFloat(*Value, TEXT("base_cost"), Terms.BaseCost) && ManifestFloat(*Value, TEXT("scaler"), Terms.Scaler)
        && (*Value)->TryGetArrayField(TEXT("part_totals"), Totals) && Totals->Num() > 0 && (*Totals)[0].IsValid())
    {
        double Total = 0;
        if ((*Totals)[0]->TryGetNumber(Total) && FMath::IsFinite(Total) && Total >= 0 && Terms.BaseCost > 0 && Terms.Scaler > 0)
        {
            Terms.PartTotal = float(Total);
            Terms.bSet = true;
        }
    }
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
            const TSharedPtr<FJsonObject>* ItemBlock = nullptr;
            if ((*Item)->TryGetObjectField(TEXT("item"), ItemBlock)) ReadLeafItem(*ItemBlock, Leaf);
            if (!Leaf.Balance.IsEmpty() && FMath::IsFinite(Leaf.Weight) && Leaf.Weight > 0) Out.Add(Leaf);
        }
    }
}

// Leaves that can go on a shelf as a non-gun row.
bool IsStockKind(const FString& Kind)
{
    return Kind == TEXT("ammo") || Kind == TEXT("health") || Kind == TEXT("shield") || Kind == TEXT("grenade_mod");
}

// The kind a gear item shows as on the page: the page knows shield, grenade mod and class mod; any other gear (a relic)
// is listed as a plain item.
FString GearPageKind(const FString& ItemType)
{
    return (ItemType == TEXT("shield") || ItemType == TEXT("grenade_mod") || ItemType == TEXT("class_mod")) ? ItemType : TEXT("item");
}

// Picks one leaf of Candidates by weight with a stream seeded from Seed (the draw the gun shelf already uses). Null when
// there is none.
const FOpenWillowVendorLeaf* PickByWeight(const TArray<const FOpenWillowVendorLeaf*>& Candidates, uint32 Seed)
{
    if (Candidates.IsEmpty()) return nullptr;
    float Total = 0;
    for (const FOpenWillowVendorLeaf* Leaf : Candidates) Total += Leaf->Weight;
    FRandomStream Rng{static_cast<int32>(Seed)};
    float Pick = Rng.FRand() * Total;
    for (const FOpenWillowVendorLeaf* Leaf : Candidates)
    {
        if (Pick < Leaf->Weight) return Leaf;
        Pick -= Leaf->Weight;
    }
    return Candidates.Last();
}

// The leaves of Pool that Keep accepts, in pool order.
template <typename Predicate>
TArray<const FOpenWillowVendorLeaf*> LeavesWhere(const TArray<FOpenWillowVendorLeaf>& Pool, Predicate Keep)
{
    TArray<const FOpenWillowVendorLeaf*> Out;
    for (const FOpenWillowVendorLeaf& Leaf : Pool)
        if (Keep(Leaf)) Out.Add(&Leaf);
    return Out;
}

// Identity line of one leaf for the manifest identity: everything that changes what the shelf can hold.
FString LeafIdentity(const TCHAR* Prefix, const FOpenWillowVendorLeaf& Leaf)
{
    return FString::Printf(TEXT("%s %s %s %.9g %s %s %d %.9g %d %.9g %.9g %.9g"), Prefix, *Leaf.Balance, *Leaf.Class, Leaf.Weight,
        *Leaf.Kind, *Leaf.AmmoType, Leaf.Amount, Leaf.HealFraction, Leaf.Value.bSet ? 1 : 0, Leaf.Value.BaseCost,
        Leaf.Value.Scaler, Leaf.Value.PartTotal);
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
    // Kind-specific fields: the ammo type and rounds, the heal fraction, and the value terms the price came from (the
    // checker recomputes the value from them; VENDORS.md section 6).
    if (Item.Kind == TEXT("ammo"))
    {
        Json->SetStringField(TEXT("ammoType"), Item.AmmoType);
        Json->SetNumberField(TEXT("amount"), Item.Amount);
    }
    if (Item.Kind == TEXT("health")) Json->SetNumberField(TEXT("healFraction"), Item.HealFraction);
    if (Item.Terms.bSet)
    {
        TSharedRef<FJsonObject> Terms = MakeShared<FJsonObject>();
        Terms->SetNumberField(TEXT("base_cost"), Item.Terms.BaseCost);
        Terms->SetNumberField(TEXT("scaler"), Item.Terms.Scaler);
        Terms->SetNumberField(TEXT("part_total"), Item.Terms.PartTotal);
        Json->SetObjectField(TEXT("valueTerms"), Terms);
    }
    return Json;
}

// One backpack row for a gun.
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

// One backpack row for a shield, grenade mod or other gear (no card: the stand-in gear has no stats).
TSharedRef<FJsonObject> GearBackpackJson(const FOpenWillowGearItem& Gear, bool bEquipped)
{
    TSharedRef<FJsonObject> Json = MakeShared<FJsonObject>();
    Json->SetStringField(TEXT("id"), Gear.Id);
    Json->SetStringField(TEXT("kind"), GearPageKind(Gear.ItemType));
    Json->SetStringField(TEXT("name"), Gear.Name);
    Json->SetNumberField(TEXT("rarity"), FMath::Clamp(Gear.Rarity, 1, 5));
    Json->SetNumberField(TEXT("level"), FMath::Max(1, Gear.Level));
    Json->SetNumberField(TEXT("sell"), FMath::Max(0, Gear.SaleValue));
    Json->SetBoolField(TEXT("equipped"), bEquipped);
    return Json;
}

// Reach of the use key (GlobalsDefinition.PlayerInteractionDistance, 350 uu, NATIVE_USE_INTERACTION.md). The native
// use trace is modelled on the machine's stand-in box (OpenWillowUseView.h); this reach from the walker's origin also stays, to order several: UNVERIFIED.
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
    const FOpenWillowVendorLeaf* Chosen = PickByWeight(
        LeavesWhere(Pool, [](const FOpenWillowVendorLeaf& Leaf) { return IsWeaponLeaf(Leaf) && Leaf.Weight > 0; }), GunSeed);
    return Chosen && FOpenWillowWeaponGen::RollBalance(Chosen->Balance, Stage, GunSeed, OutGun);
}

int32 FOpenWillowVendorStock::ShelfSize() const
{
    if (Kind == TEXT("weapons")) return WeaponShelfSize;
    if (Kind == TEXT("items")) return ItemsShelfSize;
    if (Kind == TEXT("health")) return HealthShelfSize;
    return 0; // the black market is listed, not stocked (PR 2b)
}

TOptional<int32> FOpenWillowVendorStock::ItemValue(const FOpenWillowValueTerms& Terms, int32 Level)
{
    // The same single-precision steps as the tool's check (tools/prepare_vendors.py item_value, which reproduces the live
    // rows): the scaler to the level in double and then rounded to float, the part total times that, then the base cost,
    // each step rounded to float. Doubles here could land on the other side of a whole number at some level.
    if (!Terms.bSet || Level < 0) return {};
    const float ScalerToLevel = float(FMath::Pow(double(Terms.Scaler), double(Level)));
    const float Partial = Terms.PartTotal * ScalerToLevel;
    const float Value = Partial * Terms.BaseCost;
    if (!FMath::IsFinite(Value) || Value < 0.f || Value >= 2147483520.f) return {};
    return FMath::TruncToInt(Value);
}

bool FOpenWillowVendorStock::IsStockableLeaf(const FOpenWillowVendorLeaf& Leaf)
{
    return IsStockKind(Leaf.Kind) && Leaf.Value.bSet;
}

FString FOpenWillowVendorStock::GearIdFor(const FString& ShelfId) const
{
    // A gear row's backpack id: unique per machine (the seed), restock (the count) and shelf position.
    return FString::Printf(TEXT("vend_%u_%d_%s"), Seed, RestockCount, *ShelfId);
}

bool FOpenWillowVendorStock::MakeLeafItem(const FOpenWillowVendorLeaf& Leaf, const FString& Id, bool bFeatured, FOpenWillowVendorItem& OutItem) const
{
    if (!IsStockableLeaf(Leaf)) return false;
    const TOptional<int32> Value = ItemValue(Leaf.Value, Stage);
    if (!Value.IsSet()) return false;
    FOpenWillowVendorItem Item;
    Item.Id = Id;
    Item.Kind = Leaf.Kind;
    Item.Name = Leaf.Name.IsEmpty() ? Leaf.Kind : Leaf.Name;
    Item.Level = FMath::Max(1, Stage);
    Item.Rarity = 1; // UNVERIFIED stand-in: the grade of a non-gun row is not read yet
    Item.SaleValue = Value.GetValue();
    Item.bFeatured = bFeatured;
    Item.AmmoType = Leaf.AmmoType;
    Item.Amount = Leaf.Amount;
    Item.HealFraction = Leaf.HealFraction;
    Item.Terms = Leaf.Value;
    if (Leaf.Kind == TEXT("shield") || Leaf.Kind == TEXT("grenade_mod"))
    {
        // Stand-in gear: the name, level (the stage) and value are the shelf's; the stats and the grade are not modelled.
        // The value matches the live rows (VENDORS.md section 6), so it is marked known.
        Item.bHasGear = true;
        Item.Gear.Id = GearIdFor(Id);
        Item.Gear.ItemType = Leaf.Kind;
        Item.Gear.Name = Item.Name;
        Item.Gear.Level = Item.Level;
        Item.Gear.bLevelKnown = true;
        Item.Gear.Rarity = Item.Rarity;
        Item.Gear.SaleValue = Item.SaleValue;
        Item.Gear.bSaleValueKnown = true;
    }
    OutItem = MoveTemp(Item);
    return true;
}

bool FOpenWillowVendorStock::AddLeafRow(const FOpenWillowVendorLeaf& Leaf)
{
    FOpenWillowVendorItem Row;
    if (!MakeLeafItem(Leaf, FString::Printf(TEXT("stock_%d"), Items.Num()), false, Row)) return false;
    Items.Add(MoveTemp(Row));
    return true;
}

void FOpenWillowVendorStock::FillWeaponShelf(uint32 RestockSeed)
{
    if (!Leaves.ContainsByPredicate([](const FOpenWillowVendorLeaf& Leaf) { return IsWeaponLeaf(Leaf); }))
    {
        Notice = TEXT("No weapons listed");
        return;
    }
    for (int32 I = 0; I < ShelfSize(); ++I)
    {
        FOpenWillowWeaponItem Gun;
        if (!RollWeapon(Leaves, RestockSeed + 1u + uint32(I), Gun))
        {
            Notice = TEXT("No weapon could be rolled");
            break;
        }
        Items.Add(MakeItem(Gun, FString::Printf(TEXT("stock_%d"), Items.Num()), false));
    }
}

void FOpenWillowVendorStock::FillItemsShelf(uint32 RestockSeed)
{
    // One row per ammo type (the first valued leaf of each type, in pool order), then grenade mods rolled by weight.
    TSet<FString> Types;
    for (const FOpenWillowVendorLeaf& Leaf : Leaves)
    {
        if (Items.Num() >= ShelfSize()) break;
        if (Leaf.Kind != TEXT("ammo") || !IsStockableLeaf(Leaf) || Types.Contains(Leaf.AmmoType)) continue;
        Types.Add(Leaf.AmmoType);
        AddLeafRow(Leaf);
    }
    FillRolledRows(TEXT("grenade_mod"), RestockSeed);
}

void FOpenWillowVendorStock::FillHealthShelf(uint32 RestockSeed)
{
    // One row per distinct health balance, then shields rolled by weight.
    TSet<FString> Balances;
    for (const FOpenWillowVendorLeaf& Leaf : Leaves)
    {
        if (Items.Num() >= ShelfSize()) break;
        if (Leaf.Kind != TEXT("health") || !IsStockableLeaf(Leaf) || Balances.Contains(Leaf.Balance)) continue;
        Balances.Add(Leaf.Balance);
        AddLeafRow(Leaf);
    }
    FillRolledRows(TEXT("shield"), RestockSeed);
}

void FOpenWillowVendorStock::FillRolledRows(const TCHAR* LeafKind, uint32 RestockSeed)
{
    // Each row is one weighted draw with its own seed (the gun shelf's rule: RestockSeed + 1 + the row index).
    while (Items.Num() < ShelfSize())
    {
        const FOpenWillowVendorLeaf* Leaf = PickByWeight(
            LeavesWhere(Leaves, [LeafKind](const FOpenWillowVendorLeaf& Candidate)
                { return Candidate.Kind == LeafKind && IsStockableLeaf(Candidate); }),
            RestockSeed + 1u + uint32(Items.Num()));
        // Stops when nothing is left to draw or a draw does not become a row (the loop must always make progress).
        if (!Leaf || !AddLeafRow(*Leaf))
        {
            Notice = FString::Printf(TEXT("No %ss listed"), LeafKind);
            break;
        }
    }
}

void FOpenWillowVendorStock::FillFeatured(uint32 RestockSeed)
{
    if (Kind == TEXT("weapons"))
    {
        FOpenWillowWeaponItem FeaturedGun;
        if (RollWeapon(FeaturedLeaves, RestockSeed + 1000u, FeaturedGun))
        {
            Featured = MakeItem(FeaturedGun, TEXT("featured"), true);
            bHasFeatured = true;
        }
        return;
    }
    // Non-gun machines: one weighted draw from the featured leaves that have a value.
    const FOpenWillowVendorLeaf* Leaf = PickByWeight(
        LeavesWhere(FeaturedLeaves, [](const FOpenWillowVendorLeaf& Candidate) { return IsStockableLeaf(Candidate); }),
        RestockSeed + 1000u);
    bHasFeatured = Leaf && MakeLeafItem(*Leaf, TEXT("featured"), true, Featured);
}

void FOpenWillowVendorStock::Restock(double Now)
{
    Items.Reset();
    Featured = FOpenWillowVendorItem();
    bHasFeatured = false;
    Notice.Reset();
    BoughtSinceRestock.Reset();
    bFeaturedBoughtSinceRestock = false;
    bStocked = true;
    StockedAt = Now;
    const uint32 RestockSeed = Seed + uint32(RestockCount) * 1009u; // unsigned: wraps by definition
    // Saturates at the bound a save accepts, so a saved count always loads again.
    if (RestockCount < FOpenWillowCurrencySave::MaxRestockCount) ++RestockCount;
    if (Kind == TEXT("weapons")) FillWeaponShelf(RestockSeed);
    else if (Kind == TEXT("items")) FillItemsShelf(RestockSeed);
    else if (Kind == TEXT("health")) FillHealthShelf(RestockSeed);
    else
    {
        Notice = TEXT("Not stocked yet");
        return;
    }
    FillFeatured(RestockSeed);
}

const FOpenWillowVendorItem* FOpenWillowVendorStock::FindShopItem(const FString& Id) const
{
    if (const FOpenWillowVendorItem* Found = Items.FindByPredicate([&Id](const FOpenWillowVendorItem& Item) { return Item.Id == Id; }))
        return Found;
    if (bHasFeatured && Featured.Id == Id) return &Featured;
    return nullptr;
}

// ---- Purchase effects, one per kind ------------------------------------------------------------------------------------

namespace
{
// Puts a gun in the backpack. OutNewId is the id the backpack gave the copy (it may differ from the shelf's: "<id>#2").
bool AddWeaponToPack(const FOpenWillowWeaponItem& Gun, UOpenWillowInventory* Inventory, FString& OutNewId, FString& OutError)
{
    TSet<FString> Before;
    for (const FOpenWillowWeaponItem& Owned : Inventory->Items()) Before.Add(UOpenWillowInventory::StableId(Owned));
    if (!Inventory->CanAddToBackpack() || !Inventory->AddToBackpack(Gun))
    {
        OutError = TEXT("Backpack full");
        return false;
    }
    // The backpack may give the copy a new instance id, so take the id that was not there before.
    for (const FOpenWillowWeaponItem& Owned : Inventory->Items())
        if (!Before.Contains(UOpenWillowInventory::StableId(Owned))) OutNewId = UOpenWillowInventory::StableId(Owned);
    return true;
}

// Puts a shield or grenade mod in the backpack.
bool AddGearToPack(const FOpenWillowGearItem& Gear, UOpenWillowInventory* Inventory, FString& OutError)
{
    if (!Inventory->CanAddToBackpack() || !Inventory->AddGearToBackpack(Gear))
    {
        OutError = TEXT("Backpack full");
        return false;
    }
    return true;
}

// True when a gun or gear item with this backpack id is in a weapon slot or a gear slot.
bool IsEquippedId(const UOpenWillowInventory* Inventory, const FString& Id)
{
    for (int32 Slot = 0; Slot < UOpenWillowInventory::SlotCount; ++Slot)
    {
        const FOpenWillowWeaponItem* Equipped = Inventory->SlotItem(Slot);
        if (Equipped && UOpenWillowInventory::StableId(*Equipped) == Id) return true;
    }
    for (int32 Slot = 0; Slot < UOpenWillowInventory::GearSlotCount; ++Slot)
    {
        const FOpenWillowGearItem* Worn = Inventory->GearSlotItem(UOpenWillowInventory::GearSlotName(Slot));
        if (Worn && Worn->Id == Id) return true;
    }
    return false;
}

// A gear item sold to the shop, as a buy-back row. Its rarity and level are clamped to what the page accepts (1 to 5, 1 up).
FOpenWillowVendorItem MakeGearItem(const FOpenWillowGearItem& Gear, const FString& Id)
{
    FOpenWillowVendorItem Item;
    Item.Id = Id;
    Item.Kind = GearPageKind(Gear.ItemType);
    Item.Name = Gear.Name;
    Item.Level = FMath::Max(1, Gear.Level);
    Item.Rarity = FMath::Clamp(Gear.Rarity, 1, 5);
    Item.SaleValue = FMath::Max(0, Gear.SaleValue);
    Item.bHasGear = true;
    Item.Gear = Gear;
    return Item;
}

// Ammo: the pool gets the rounds, up to its maximum. Nothing is charged here (Buy charges after this succeeds). The row
// stays on the shelf (UNVERIFIED: the game may use the stock up).
bool ApplyAmmoBuy(const FOpenWillowVendorItem& Item, UOpenWillowInventory* Inventory, FString& OutError)
{
    EOpenWillowAmmoType Type = EOpenWillowAmmoType::Pistol;
    if (!AmmoTypeFromKey(Item.AmmoType, Type))
    {
        OutError = TEXT("Unknown item");
        return false;
    }
    const FOpenWillowAmmoPool Pool = Inventory->AmmoPool(Type);
    if (Pool.Current >= Pool.Max)
    {
        OutError = TEXT("Ammo full");
        return false;
    }
    const int32 After = int32(FMath::Min<int64>(int64(Pool.Max), int64(Pool.Current) + int64(Item.Amount)));
    Inventory->SetAmmoCurrent(Type, After);
    UE_LOG(LogTemp, Display, TEXT("OWVENDAMMO type=%s before=%d after=%d max=%d"), UOpenWillowInventory::AmmoTypeKey(Type),
        Pool.Current, After, Pool.Max);
    return true;
}

// Health: heals a fraction of the maximum, up to the maximum. The row stays on the shelf (UNVERIFIED, as for ammo).
bool ApplyHealthBuy(const FOpenWillowVendorItem& Item, FOpenWillowMayaState* Maya, double Now, FString& OutError)
{
    if (!Maya)
    {
        OutError = TEXT("No walker");
        return false;
    }
    // Refused while Injured or Dead, before anything is charged (UNVERIFIED: the game's shops are not usable then anyway).
    if (Maya->IsDown())
    {
        OutError = TEXT("Cannot heal while down");
        return false;
    }
    const float Max = Maya->Effective().MaxHealth;
    const float Before = Maya->Health;
    if (Before >= Max)
    {
        OutError = TEXT("Health full");
        return false;
    }
    // Only the current health changes, clamped to the effective maximum; the base maximum is left alone (writing the
    // effective value into it would compound the skills' health modifiers).
    const float After = FMath::Min(Max, Before + Item.HealFraction * Max);
    Maya->Health = After;
    UE_LOG(LogTemp, Display, TEXT("OWVENDHEAL before=%.3f after=%.3f max=%.3f"), Before, After, Max);
    return true;
}
}

bool FOpenWillowVendorStock::ApplyBuy(const FOpenWillowVendorItem& Item, UOpenWillowInventory* Inventory, FOpenWillowMayaState* Maya,
    double Now, FString& OutError)
{
    if (Item.bHasWeapon)
    {
        FString NewId;
        if (!AddWeaponToPack(Item.Weapon, Inventory, NewId, OutError)) return false;
        if (!NewId.IsEmpty()) LastBoughtId = NewId;
        return true;
    }
    if (Item.bHasGear) return AddGearToPack(Item.Gear, Inventory, OutError);
    if (Item.Kind == TEXT("ammo")) return ApplyAmmoBuy(Item, Inventory, OutError);
    if (Item.Kind == TEXT("health")) return ApplyHealthBuy(Item, Maya, Now, OutError);
    OutError = TEXT("Not for sale yet");
    return false;
}

void FOpenWillowVendorStock::RemoveBoughtRow(const FOpenWillowVendorItem& Item, const FString& Id)
{
    if (Item.bFeatured)
    {
        bHasFeatured = false;
        bFeaturedBoughtSinceRestock = true;
        return;
    }
    // Ammo and health rows stay on the shelf; guns and gear leave it (and are kept for the save).
    if (Item.Kind == TEXT("ammo") || Item.Kind == TEXT("health")) return;
    Items.RemoveAll([&Id](const FOpenWillowVendorItem& Shelf) { return Shelf.Id == Id; });
    BoughtSinceRestock.Add(Id);
}

bool FOpenWillowVendorStock::Buy(FString Id, UOpenWillowInventory* Inventory, FOpenWillowMayaState* Maya, double Now,
    FString& OutError)
{
    const FOpenWillowVendorItem* Found = FindShopItem(Id);
    if (!Found || !Inventory)
    {
        OutError = TEXT("Unknown item");
        return false;
    }
    const FOpenWillowVendorItem Item = *Found; // a copy: the shelf changes below
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
    // The effect first (a refusal changes nothing), then the charge, then the shelf.
    if (!ApplyBuy(Item, Inventory, Maya, Now, OutError)) return false;
    Inventory->SetMoney(Money - Price);
    RemoveBoughtRow(Item, Id);
    return true;
}

bool FOpenWillowVendorStock::CanRestore(const FOpenWillowCurrencyMachine& Saved, FString& OutError) const
{
    const double IntervalSeconds = double(MinutesBetweenResets) * 60.0;
    if (Saved.RestockCount < 1 || Saved.RestockCount > FOpenWillowCurrencySave::MaxRestockCount || Saved.SecondsToReset < 0
        || double(Saved.SecondsToReset) > IntervalSeconds)
    {
        OutError = TEXT("saved reset clock or restock count out of range");
        return false;
    }
    for (const FString& Id : Saved.Bought)
    {
        // Shelf ids are "stock_<n>" with n below this machine's shelf size.
        const int32 Index = Id.StartsWith(TEXT("stock_")) ? FCString::Atoi(*Id.Mid(6)) : -1;
        if (Index < 0 || Index >= ShelfSize() || Id != FString::Printf(TEXT("stock_%d"), Index))
        {
            OutError = FString::Printf(TEXT("bought id %s is not a shelf id"), *Id);
            return false;
        }
    }
    return true;
}

bool FOpenWillowVendorStock::Restore(const FOpenWillowCurrencyMachine& Saved, double Now, FString& OutError)
{
    if (!CanRestore(Saved, OutError)) return false;
    const double IntervalSeconds = double(MinutesBetweenResets) * 60.0;
    // Restock() rolls from RestockCount and then counts one more, so one below the saved count rebuilds the saved shelf.
    RestockCount = Saved.RestockCount - 1;
    Restock(Now);
    // Restock() started the clock now; move its start back so that SecondsToReset(Now) is the saved value. UNVERIFIED
    // host choice: world time does not run while the game is closed, so the saved seconds are taken as they are.
    StockedAt = Now - (IntervalSeconds - double(Saved.SecondsToReset));
    for (const FString& Id : Saved.Bought)
    {
        // An id that is not on the rebuilt shelf (the manifest changed) is dropped, not kept in the record.
        if (Items.RemoveAll([&Id](const FOpenWillowVendorItem& Shelf) { return Shelf.Id == Id; }) > 0) BoughtSinceRestock.Add(Id);
    }
    if (Saved.bFeaturedBought)
    {
        bHasFeatured = false;
        bFeaturedBoughtSinceRestock = true;
    }
    return true;
}

bool FOpenWillowVendorStock::Sell(FString Id, UOpenWillowInventory* Inventory, FString& OutError,
    const TFunction<bool(const FOpenWillowWeaponItem&)>& IsProtected)
{
    // A gun or a gear item (shield, grenade mod, ...) from the backpack; a mission gun is refused first.
    const FOpenWillowWeaponItem* Owned = Inventory ? Inventory->FindItemById(Id) : nullptr;
    if (!Inventory || (!Owned && !Inventory->FindGearById(Id)))
    {
        OutError = TEXT("Unknown item");
        return false;
    }
    if (Owned && IsProtected && IsProtected(*Owned))
    {
        OutError = TEXT("Mission items cannot be sold");
        return false;
    }
    if (IsEquippedId(Inventory, Id))
    {
        OutError = TEXT("Equipped items cannot be sold");
        return false;
    }
    FOpenWillowTakenInventoryItem Taken;
    if (!Inventory->TakeById(Id, Taken))
    {
        OutError = TEXT("Unknown item");
        return false;
    }
    const FString BuybackId = FString::Printf(TEXT("buyback_%d"), ++BuybackSerial);
    const FOpenWillowVendorItem Sold = Taken.bIsGear ? MakeGearItem(Taken.Gear, BuybackId) : MakeItem(Taken.Weapon, BuybackId, false);
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
    // Back into the backpack as it was sold: a gear item as gear, a gun as a gun.
    FString NewId;
    const bool bAdded = Item.bHasGear ? AddGearToPack(Item.Gear, Inventory, OutError) : AddWeaponToPack(Item.Weapon, Inventory, NewId, OutError);
    if (!bAdded) return false;
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

FBox AOpenWillowVendingMachine::UseBox() const
{
    return Body->Bounds.GetBox();
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
    // The currency save runs with or without the machines: its load waits for the walker (TickCurrencyLoad).
    FString CurrencyGiven, CurrencyError;
    if (FParse::Value(FCommandLine::Get(), TEXT("owcurrencysave="), CurrencyGiven))
    {
        if (FOpenWillowCurrencySave::ResolvePath(CurrencyGiven, CurrencyPath, CurrencyError))
        {
            CurrencyWaits = 0;
            InWorld.GetTimerManager().SetTimer(CurrencyTimer, FTimerDelegate::CreateUObject(this, &UOpenWillowVendorSubsystem::TickCurrencyLoad),
                0.25f, true);
        }
        else
        {
            UE_LOG(LogTemp, Warning, TEXT("OWCURRENCY off: %s (%s)"), *CurrencyGiven, *CurrencyError);
        }
    }
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
    // The identity of what a save depends on: the stage, the reset interval and each spawned machine's id, kind, seed and
    // pools (the shelf is rolled from these).
    // No identity (and so no saving) without at least one machine: stage and minutes alone describe no shelf.
    if (Machines.Num() == 0) return;
    TArray<FString> IdentityLines{FString::Printf(TEXT("stage %d minutes %d"), Stage, Minutes)};
    for (const TWeakObjectPtr<AOpenWillowVendingMachine>& Candidate : Machines)
    {
        const AOpenWillowVendingMachine* Machine = Candidate.Get();
        if (!Machine) continue;
        IdentityLines.Add(FString::Printf(TEXT("machine %s %s %u"), *Machine->Id, *Machine->Stock.Kind, Machine->Stock.Seed));
        for (const FOpenWillowVendorLeaf& Leaf : Machine->Stock.Leaves) IdentityLines.Add(LeafIdentity(TEXT("leaf"), Leaf));
        for (const FOpenWillowVendorLeaf& Leaf : Machine->Stock.FeaturedLeaves) IdentityLines.Add(LeafIdentity(TEXT("featured"), Leaf));
    }
    ManifestId = FOpenWillowCurrencySave::ManifestIdentity(IdentityLines);
    if (FParse::Param(FCommandLine::Get(), TEXT("owvendortest")))
        InWorld.GetTimerManager().SetTimer(TestTimer, FTimerDelegate::CreateUObject(this, &UOpenWillowVendorSubsystem::TickVendorTest),
            2.f, true, 5.f); // one step every 2 s once the walker is there (the page needs a moment to load)
}

void UOpenWillowVendorSubsystem::Deinitialize()
{
    // Give input back and hide the page before it goes (a controller can outlive the world), and stop both timers.
    if (Page.IsValid() && Page->IsShown()) CloseShop();
    SaveCurrency(); // a no-op unless the currency load finished (and the inventory still exists)
    if (UWorld* World = GetWorld())
    {
        World->GetTimerManager().ClearTimer(OpenTimer);
        World->GetTimerManager().ClearTimer(TestTimer);
        World->GetTimerManager().ClearTimer(CurrencyTimer);
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
        if (DistanceSquared >= OutDistanceSquared || !OpenWillowUseView::LooksAt(Walker, Machine->UseBox(), Machine)) continue;
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
        if (DistanceSquared >= BestDistanceSquared || !OpenWillowUseView::LooksAt(Walker, Machine->UseBox(), Machine)) continue;
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
    if (bRestocked) SaveCurrency();
    Page->Show(PC);
    ++Opening;
    PushState();
    // While open, push the state every second: the page's countdown is the host's world time, and a restock that falls
    // due while the shop is open shows at once.
    GetWorld()->GetTimerManager().SetTimer(OpenTimer, FTimerDelegate::CreateUObject(this, &UOpenWillowVendorSubsystem::PushTimerState), 1.f, true);
    UE_LOG(LogTemp, Display, TEXT("OWVENDOR open id=%s kind=%s restocked=%d"), *Machine.Id, *Machine.Kind, bRestocked ? 1 : 0);
    return true;
}

void UOpenWillowVendorSubsystem::CloseShop()
{
    if (Page.IsValid()) Page->Hide(ActiveController.Get());
    if (UWorld* World = GetWorld()) World->GetTimerManager().ClearTimer(OpenTimer);
    SaveCurrency();
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
        // This press did nothing, and the page is about to catch up. A refusal already showing stays; otherwise say so. The
        // serial does not move here, so the next press cannot keep missing.
        if (Action != TEXT("close") && ActiveMachine.IsValid())
        {
            if (LastMessage.IsEmpty()) LastMessage = TEXT("Try again");
            PushTimerState();
        }
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
    bool bRestocked = false;
    if (!Machine || !Inventory) OutError = TEXT("No shop is open");
    else if (Id.IsEmpty() && Action != TEXT("close")) OutError = TEXT("No item given");
    // A restock that fell due is applied before the action, never after it. A buy aimed at the old shelf is refused:
    // its id may now name a different gun.
    else if ((bRestocked = Machine->Stock.EnsureStocked(GetWorld()->GetTimeSeconds())) && Action == TEXT("buy"))
        OutError = TEXT("The stock changed");
    else if (Action == TEXT("buy"))
    {
        AOpenWillowWalker* Walker = ActiveWalker();
        bOk = Machine->Stock.Buy(Id, Inventory, Walker ? &Walker->GetMayaState() : nullptr, GetWorld()->GetTimeSeconds(), OutError);
    }
    else if (Action == TEXT("sell"))
        bOk = Machine->Stock.Sell(Id, Inventory, OutError, [this](const FOpenWillowWeaponItem& Item) { return IsQuestOwned(Item); });
    else if (Action == TEXT("buyback")) bOk = Machine->Stock.BuyBack(Id, Inventory, OutError);
    else OutError = TEXT("Unknown action");
    int32 Money = 0;
    if (Inventory) Inventory->GetMoney(Money);
    UE_LOG(LogTemp, Display, TEXT("OWVENDOR %s id=%s ok=%d money=%d error=%s"), *Action, *Id, bOk ? 1 : 0, Money, *OutError);
    LastMessage = bOk ? FString() : OutError;
    if (bOk) SaveCurrency();
    // The serial moves only when what a request refers to changed. A refusal changes nothing, so a repeat press still gets it.
    if (bOk || bRestocked) PushState();
    else PushTimerState();
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
    SaveCurrency();
    PushState();
}

void UOpenWillowVendorSubsystem::TickCurrencyLoad()
{
    UWorld* World = GetWorld();
    APlayerController* PC = World ? World->GetFirstPlayerController() : nullptr;
    AOpenWillowWalker* Walker = PC ? Cast<AOpenWillowWalker>(PC->GetPawn()) : nullptr;
    // Wait until the walker's BeginPlay has set its start purse; loading earlier would be overwritten by it.
    if (!World) return;
    if (!Walker || !Walker->HasActorBegunPlay() || !Walker->GetInventory())
    {
        if (++CurrencyWaits > 480) // 120 s at 0.25 s
        {
            World->GetTimerManager().ClearTimer(CurrencyTimer);
            UE_LOG(LogTemp, Warning, TEXT("OWCURRENCY load none (no walker after 120 s); saves are off"));
        }
        return;
    }
    World->GetTimerManager().ClearTimer(CurrencyTimer);
    LoadCurrency(*Walker);
}

void UOpenWillowVendorSubsystem::LoadCurrency(AOpenWillowWalker& Walker)
{
    UOpenWillowInventory* Inventory = Walker.GetInventory();
    CurrencyInventory = Inventory;
    FOpenWillowCurrencySave Save;
    bool bHaveFile = false;
    // Without a manifest identity (no usable -owvendors) a save could never be checked: keep saving off.
    if (ManifestId.IsEmpty())
    {
        bCurrencyRefused = true;
        UE_LOG(LogTemp, Warning, TEXT("OWCURRENCY off: no vendor machines spawned (-owvendors missing or unusable), so a save cannot be checked"));
        return;
    }
    // A missing save with a backup beside it (a replace interrupted half way) is read from the backup; the next save writes
    // the main file again.
    FString ReadPath = CurrencyPath;
    if (!IFileManager::Get().FileExists(*CurrencyPath) && IFileManager::Get().FileExists(*(CurrencyPath + TEXT(".bak"))))
    {
        ReadPath = CurrencyPath + TEXT(".bak");
        UE_LOG(LogTemp, Warning, TEXT("OWCURRENCY %s is missing; reading the backup %s"), *CurrencyPath, *ReadPath);
    }
    if (IFileManager::Get().FileExists(*ReadPath))
    {
        FString Text, Error;
        if (!FFileHelper::LoadFileToString(Text, *ReadPath) || !FOpenWillowCurrencySave::FromJson(Text, Save, Error))
        {
            bCurrencyRefused = true;
            UE_LOG(LogTemp, Warning, TEXT("OWCURRENCY load refused: %s (%s); saves are off so the file is not overwritten"),
                *CurrencyPath, *Error);
            return;
        }
        bHaveFile = true;
    }
    // Validate the whole file against this session before applying anything: same machine set (manifest identity),
    // every saved machine spawned, every row restorable. An incompatible save is left untouched and saves stay off.
    if (bHaveFile)
    {
        FString Why;
        TMap<FString, AOpenWillowVendingMachine*> ById;
        for (const TWeakObjectPtr<AOpenWillowVendingMachine>& Candidate : Machines)
            if (Candidate.IsValid()) ById.Add(Candidate->Id, Candidate.Get());
        if (Save.Manifest != ManifestId) Why = TEXT("written against a different vendors.json (manifest identity differs)");
        for (const TPair<FString, FOpenWillowCurrencyMachine>& Pair : Save.Machines)
        {
            if (!Why.IsEmpty()) break;
            AOpenWillowVendingMachine* const* Machine = ById.Find(Pair.Key);
            if (!Machine) Why = FString::Printf(TEXT("machine %s is not spawned in this session"), *Pair.Key);
            else if (FString Error; !(*Machine)->Stock.CanRestore(Pair.Value, Error))
                Why = FString::Printf(TEXT("machine %s: %s"), *Pair.Key, *Error);
        }
        if (!Why.IsEmpty())
        {
            bCurrencyRefused = true;
            UE_LOG(LogTemp, Warning, TEXT("OWCURRENCY load refused: %s (%s); nothing applied, saves are off so the file is kept"),
                *CurrencyPath, *Why);
            return;
        }
    }
    // -owmoney / -owerid replace the saved values (the walker already applied them in its BeginPlay); a value that is not a
    // whole number in range is ignored with a warning and the saved amount stays.
    int32 CommandMoney = 0, CommandEridium = 0;
    bool bMoneyInvalid = false, bEridiumInvalid = false;
    const bool bMoneyGiven = FOpenWillowCurrencySave::ParsePurseSwitch(TEXT("owmoney="), CommandMoney, bMoneyInvalid);
    const bool bEridiumGiven = FOpenWillowCurrencySave::ParsePurseSwitch(TEXT("owerid="), CommandEridium, bEridiumInvalid);
    if (bMoneyInvalid) UE_LOG(LogTemp, Warning, TEXT("OWCURRENCY -owmoney= is not a whole number in range; ignored"));
    if (bEridiumInvalid) UE_LOG(LogTemp, Warning, TEXT("OWCURRENCY -owerid= is not a whole number in range; ignored"));
    if (bHaveFile && !bMoneyGiven) Inventory->SetMoney(Save.Money);
    if (bHaveFile && !bEridiumGiven) Inventory->SetEridium(Save.Eridium);

    int32 Restored = 0;
    for (const TPair<FString, FOpenWillowCurrencyMachine>& Pair : Save.Machines)
    {
        for (const TWeakObjectPtr<AOpenWillowVendingMachine>& Candidate : Machines)
        {
            if (!Candidate.IsValid() || Candidate->Id != Pair.Key) continue;
            FString Error;
            if (Candidate->Stock.Restore(Pair.Value, GetWorld()->GetTimeSeconds(), Error)) ++Restored;
            else UE_LOG(LogTemp, Error, TEXT("OWCURRENCY machine %s not restored after validation: %s"), *Pair.Key, *Error);
        }
    }
    bCurrencyReady = true;
    if (!bHaveFile)
    {
        UE_LOG(LogTemp, Display, TEXT("OWCURRENCY load none (no file at %s)"), *CurrencyPath);
        return;
    }
    int32 Money = 0;
    Inventory->GetMoney(Money);
    int32 Eridium = 0;
    Inventory->GetEridium(Eridium);
    UE_LOG(LogTemp, Display, TEXT("OWCURRENCY load money=%d eridium=%d machines=%d"), Money, Eridium, Restored);
}

void UOpenWillowVendorSubsystem::SaveCurrency()
{
    UOpenWillowInventory* Inventory = CurrencyInventory.Get();
    if (CurrencyPath.IsEmpty() || !bCurrencyReady || bCurrencyRefused || !Inventory) return;
    FOpenWillowCurrencySave Save;
    Inventory->GetMoney(Save.Money); // unknown money is 0
    Inventory->GetEridium(Save.Eridium);
    Save.Manifest = ManifestId;
    const double Now = GetWorld() ? GetWorld()->GetTimeSeconds() : 0.0;
    for (const TWeakObjectPtr<AOpenWillowVendingMachine>& Candidate : Machines)
    {
        const AOpenWillowVendingMachine* Machine = Candidate.Get();
        if (!Machine || !Machine->Stock.bStocked) continue; // a machine never opened has nothing to keep
        FOpenWillowCurrencyMachine& Row = Save.Machines.Add(Machine->Id);
        Row.RestockCount = Machine->Stock.RestockCount;
        Row.SecondsToReset = FMath::FloorToInt(Machine->Stock.SecondsToReset(Now));
        Row.Bought = Machine->Stock.BoughtSinceRestock;
        Row.bFeaturedBought = Machine->Stock.bFeaturedBoughtSinceRestock;
    }
    FString Error;
    if (!FOpenWillowCurrencySave::WriteFile(CurrencyPath, Save.ToJson(), Error))
    {
        UE_LOG(LogTemp, Warning, TEXT("OWCURRENCY save failed: %s"), *Error);
        return;
    }
    UE_LOG(LogTemp, Display, TEXT("OWCURRENCY save money=%d machines=%d"), Save.Money, Save.Machines.Num());
}

bool UOpenWillowVendorSubsystem::IsQuestOwned(const FOpenWillowWeaponItem& Item) const
{
    // The quest says which weapon it lent (and still owns); no item id is named here.
    const APlayerController* PC = ActiveController.Get();
    const AOpenWillowWalker* Walker = PC ? Cast<AOpenWillowWalker>(PC->GetPawn()) : nullptr;
    const UOpenWillowQuest* Quest = Walker ? Walker->GetQuest() : nullptr;
    return Quest && Quest->OwnsItem(Item);
}

AOpenWillowWalker* UOpenWillowVendorSubsystem::ActiveWalker() const
{
    const APlayerController* PC = ActiveController.Get();
    return PC ? Cast<AOpenWillowWalker>(PC->GetPawn()) : nullptr;
}

int32 UOpenWillowVendorSubsystem::FirstMachineIndexOfKind(const TCHAR* Kind) const
{
    for (int32 Index = 0; Index < Machines.Num(); ++Index)
        if (Machines[Index].IsValid() && Machines[Index]->Kind == Kind) return Index;
    return INDEX_NONE;
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
    // A run that asked for the movie HUD holds the test until it is ready. If it fails, the test's own exit, with no frames.
    if (TestStep == 0)
    {
        const EOpenWillowCaptureHud MovieHud = AOpenWillowMayaHUD::CaptureHudGate(World, TEXT("vendor"));
        if (MovieHud == EOpenWillowCaptureHud::Wait) return;
        if (MovieHud == EOpenWillowCaptureHud::Failed)
        {
            World->GetTimerManager().ClearTimer(TestTimer);
            FPlatformMisc::RequestExit(false);
            return;
        }
    }
    FString Error;
    auto Report = [this]() { UE_LOG(LogTemp, Display, TEXT("OWVENDSTATE %s"), *ReportJson()); };
    // Closes any open shop, then opens the first spawned machine of the kind (the page stays the same widget).
    auto OpenFirstOfKind = [this, &Error](const TCHAR* Kind)
    {
        if (Page.IsValid() && Page->IsShown()) CloseShop();
        const int32 Index = FirstMachineIndexOfKind(Kind);
        UE_LOG(LogTemp, Display, TEXT("OWVENDTEST open index=%d kind=%s"), Index, Kind);
        if (Index == INDEX_NONE) Error = FString::Printf(TEXT("no %s machine spawned"), Kind);
        if (Index == INDEX_NONE || !OpenByIndex(Index, Error)) UE_LOG(LogTemp, Warning, TEXT("OWVENDTEST open refused: %s"), *Error);
    };
    switch (TestStep++)
    {
    case 0: OpenFirstOfKind(TEXT("weapons")); break; // only the first weapon machine is stocked with guns
    case 1: Report(); break;
    case 2: if (!BuyByIndex(0, Error)) UE_LOG(LogTemp, Warning, TEXT("OWVENDTEST buy refused: %s"), *Error); break;
    case 3: Report(); break;
    case 4: if (!SellLast(Error)) UE_LOG(LogTemp, Warning, TEXT("OWVENDTEST sell refused: %s"), *Error); break;
    case 5: Report(); break;
    case 6: RestockOpen(); break;
    case 7: Report(); break;
    case 8: AOpenWillowMayaHUD::CaptureShot(TEXT("vendor"), TEXT("OWVendor_open.png")); break; // UI on: the shop page is a Slate widget
    // Ammo: the first items machine, report, buy shelf row 0 (an ammo row), report.
    case 9: OpenFirstOfKind(TEXT("items")); break;
    case 10:
        // Test fixture only: the pools start full, so half-empty the pool of shelf row 0 first (else "Ammo full"). It runs
        // before the report, which the checker compares with the pool the buy starts from.
        if (const AOpenWillowVendingMachine* Machine = ActiveMachine.Get())
        {
            EOpenWillowAmmoType Type = EOpenWillowAmmoType::Pistol;
            UOpenWillowInventory* Inventory = ActiveInventory.Get();
            if (Inventory && Machine->Stock.Items.IsValidIndex(0) && AmmoTypeFromKey(Machine->Stock.Items[0].AmmoType, Type))
            {
                const int32 Half = Inventory->AmmoPool(Type).Max / 2;
                Inventory->SetAmmoCurrent(Type, Half);
                UE_LOG(LogTemp, Display, TEXT("OWVENDTEST ammo %s set to %d for the ammo buy"), *Machine->Stock.Items[0].AmmoType, Half);
            }
        }
        Report();
        break;
    case 11: if (!BuyByIndex(0, Error)) UE_LOG(LogTemp, Warning, TEXT("OWVENDTEST ammo buy refused: %s"), *Error); break;
    case 12: Report(); break;
    // Health: the first health machine, the same.
    case 13: OpenFirstOfKind(TEXT("health")); break;
    case 14:
        // Test fixture only: half health first, so the heal can be measured ("Health full" refuses at full health). Before
        // the report, as for ammo.
        if (AOpenWillowWalker* Walker = PC ? Cast<AOpenWillowWalker>(PC->GetPawn()) : nullptr)
        {
            FOpenWillowMayaState& State = Walker->GetMayaState();
            const float Max = State.Effective().MaxHealth;
            State.Health = Max * 0.5f; // current health only (the base maximum stays as it is)
            UE_LOG(LogTemp, Display, TEXT("OWVENDTEST health set to %.1f of %.1f for the heal buy"), Max * 0.5f, Max);
        }
        Report();
        break;
    case 15: if (!BuyByIndex(0, Error)) UE_LOG(LogTemp, Warning, TEXT("OWVENDTEST heal buy refused: %s"), *Error); break;
    case 16: Report(); break;
    case 17:
    {
        // A refused press, the same press again, then one with an old serial, through the page's own message path (no page keys).
        const auto Press = [this](int64 PressSerial)
        {
            HandlePageMessage(FString::Printf(TEXT("OWVEND {\"serial\":%lld,\"action\":\"buy\",\"id\":\"no_such_row\"}"), PressSerial));
        };
        const int64 Start = Serial;
        Press(Serial);
        Press(Serial);
        const int64 AfterRefusals = Serial;
        const FString Refusal = LastMessage;
        Press(Serial - 1);
        const FString AfterStale = LastMessage;
        LastMessage.Reset();
        Press(Serial - 1);
        UE_LOG(LogTemp, Display, TEXT("OWVENDTEST stale_press start=%lld after_refusals=%lld after_stale=%lld refusal=[%s] kept=[%s] alone=[%s] opening=%lld"),
            Start, AfterRefusals, Serial, *Refusal, *AfterStale, *LastMessage, Opening);
        break;
    }
    case 18:
        if (Page.IsValid() && Page->IsShown()) CloseShop();
        UE_LOG(LogTemp, Display, TEXT("OWVENDTEST done"));
        World->GetTimerManager().ClearTimer(TestTimer);
        FPlatformMisc::RequestExit(false);
        break;
    default: break;
    }
}

void UOpenWillowVendorSubsystem::PushTimerState()
{
    if (!Page.IsValid() || !Page->IsShown() || !ActiveMachine.IsValid()) return;
    if (ActiveMachine->Stock.EnsureStocked(GetWorld()->GetTimeSeconds())) ++Serial;
    Page->SendState(StateJson());
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
    int32 Eridium = 0;
    Inventory->GetEridium(Eridium);
    State->SetNumberField(TEXT("eridium"), Eridium);
    State->SetNumberField(TEXT("capacity"), Inventory->GetBackpackCapacity());
    State->SetNumberField(TEXT("used"), Inventory->BackpackCount());

    // Ammo pools and health, for the page's detail line and the checker (ammo_buy, heal_buy).
    TSharedRef<FJsonObject> AmmoJson = MakeShared<FJsonObject>();
    for (int32 Type = 0; Type < UOpenWillowInventory::AmmoTypeCount; ++Type)
    {
        const FOpenWillowAmmoPool& Pool = Inventory->AmmoPool(EOpenWillowAmmoType(Type));
        TSharedRef<FJsonObject> PoolJson = MakeShared<FJsonObject>();
        PoolJson->SetNumberField(TEXT("current"), Pool.Current);
        PoolJson->SetNumberField(TEXT("max"), Pool.Max);
        AmmoJson->SetObjectField(UOpenWillowInventory::AmmoTypeKey(EOpenWillowAmmoType(Type)), PoolJson);
    }
    State->SetObjectField(TEXT("ammo"), AmmoJson);
    const AOpenWillowWalker* Walker = ActiveWalker();
    TSharedRef<FJsonObject> HealthJson = MakeShared<FJsonObject>();
    HealthJson->SetNumberField(TEXT("current"), Walker ? Walker->GetHealth() : 0.f);
    HealthJson->SetNumberField(TEXT("max"), Walker ? Walker->GetMaxHealth() : 0.f);
    State->SetObjectField(TEXT("health"), HealthJson);

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
    // Shields and grenade mods (and other gear) are listed after the guns; gear is sellable unless it is worn.
    for (const FOpenWillowGearItem& Gear : Inventory->GearItemList())
    {
        const bool bWorn = IsEquippedId(Inventory, Gear.Id);
        TSharedRef<FJsonObject> Row = GearBackpackJson(Gear, bWorn);
        Row->SetBoolField(TEXT("sellable"), !bWorn);
        BackpackJsonList.Add(MakeShared<FJsonValueObject>(Row));
    }
    State->SetArrayField(TEXT("backpack"), BackpackJsonList);

    TArray<TSharedPtr<FJsonValue>> BuybackJsonList;
    for (const FOpenWillowVendorItem& Item : Stock.Buyback)
        BuybackJsonList.Add(MakeShared<FJsonValueObject>(ShopItemJson(Item, TOptional<int32>(FOpenWillowVendorStock::SellPriceOf(Item)), Money)));
    State->SetArrayField(TEXT("buyback"), BuybackJsonList);

    State->SetNumberField(TEXT("resetSeconds"), FMath::FloorToInt(Stock.SecondsToReset(Now)));
    State->SetNumberField(TEXT("opening"), double(Opening));
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
