#include "OpenWillowInventory.h"
#include "Dom/JsonObject.h"
#include "HAL/FileManager.h"
#include "Misc/FileHelper.h"
#include "Misc/Paths.h"
#include "Serialization/JsonReader.h"
#include "Serialization/JsonSerializer.h"

namespace
{
float Number(const TSharedPtr<FJsonObject>& Card, const TCHAR* Field, float Fallback)
{
    double Value = Fallback;
    return Card->TryGetNumberField(Field, Value) ? float(Value) : Fallback;
}

bool IsGearItemType(const FString& Type)
{
    return Type == TEXT("shield") || Type == TEXT("grenade_mod")
        || Type == TEXT("class_mod") || Type == TEXT("relic");
}

// BaseMaxValue.BaseValueConstant decoded with ow-package --properties
// <index> --property-offset 4 from Startup.upk, D_Resourcepools.AmmoPools.
// Ammo_<pool>_Pool (2026-09-29). Ammo SDUs (UpgradeLevelAttribute) and any
// class or skill modifier are NOT applied.
constexpr int32 BaseAmmoMax[UOpenWillowInventory::AmmoTypeCount] =
{
    200, // Ammo_Repeater_Pistol_Pool
    360, // Ammo_Patrol_SMG_Pool
    280, // Ammo_Combat_Rifle_Pool
    80,  // Ammo_Combat_Shotgun_Pool
    48,  // Ammo_Sniper_Rifle_Pool
    12,  // Ammo_Rocket_Launcher_Pool
    3,   // Ammo_Grenade_Protean_Pool
};

// Card type label for weapons whose recipe carries no display type (the
// Infinity recipes); the same names weapon_stats.py writes and
// ResolveAmmoType matches. Grenades are not weapons here.
const TCHAR* WeaponTypeLabel(EOpenWillowAmmoType Type)
{
    switch (Type)
    {
    case EOpenWillowAmmoType::Pistol: return TEXT("Pistol");
    case EOpenWillowAmmoType::SMG: return TEXT("SMG");
    case EOpenWillowAmmoType::AssaultRifle: return TEXT("Assault Rifle");
    case EOpenWillowAmmoType::Shotgun: return TEXT("Shotgun");
    case EOpenWillowAmmoType::Sniper: return TEXT("Sniper Rifle");
    case EOpenWillowAmmoType::Launcher: return TEXT("Launcher");
    default: return TEXT("");
    }
}

const TCHAR* GearTypeForSlot(int32 Slot)
{
    switch (Slot)
    {
    case 0: return TEXT("shield");
    case 1: return TEXT("grenade_mod");
    case 2: return TEXT("class_mod");
    case 3: return TEXT("relic");
    default: return TEXT("");
    }
}
}

UOpenWillowInventory::UOpenWillowInventory()
{
    for (int32 Type = 0; Type < AmmoTypeCount; ++Type)
    {
        AmmoPools[Type].Max = BaseAmmoMax[Type];
        AmmoPools[Type].Current = BaseAmmoMax[Type]; // UNVERIFIED: full at start
    }
}

int32 UOpenWillowInventory::LoadRecipes(const FString& Directory)
{
    TArray<FString> Files;
    IFileManager::Get().FindFiles(Files, *FPaths::Combine(Directory, TEXT("*.json")), true, false);
    Files.Sort();
    // Optional <dir>/load_order.txt: one recipe id per line, loaded first in that order (the rest keep
    // the sorted order). It only orders a local demo library so the backpack is not eight copies of one
    // gun in a row; ids are the stable recipe stems.
    FString OrderText;
    if (FFileHelper::LoadFileToString(OrderText, *FPaths::Combine(Directory, TEXT("load_order.txt"))))
    {
        TArray<FString> Ordered;
        TArray<FString> Lines;
        OrderText.ParseIntoArrayLines(Lines);
        for (const FString& Line : Lines)
        {
            const FString File = Line.TrimStartAndEnd() + TEXT(".json");
            if (Files.Remove(File) > 0) Ordered.Add(File);
        }
        Ordered.Append(Files);
        Files = MoveTemp(Ordered);
    }
    for (const FString& File : Files)
    {
        const FString ItemId = FPaths::GetBaseFilename(File);
        // Recipe stems are stable IDs for this recipe-backed prototype. One
        // recipe currently represents one item instance, so loading twice
        // must not duplicate that instance.
        if (IsKnownItemId(ItemId)) continue;
        FString Text;
        if (!FFileHelper::LoadFileToString(Text, *FPaths::Combine(Directory, File))) continue;
        TSharedPtr<FJsonObject> Recipe;
        if (!FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Recipe) || !Recipe) continue;
        const TSharedPtr<FJsonObject>* Stats = nullptr;
        const TSharedPtr<FJsonObject>* Card = nullptr;
        // Only recipes that tools/weapon_stats.py has evaluated are items.
        if (!Recipe->TryGetObjectField(TEXT("stats"), Stats) || !(*Stats)->TryGetObjectField(TEXT("card"), Card))
            continue;
        FOpenWillowWeaponItem Item;
        Item.Id = ItemId;
        Item.InstanceId = ItemId;
        Item.Name = Recipe->GetStringField(TEXT("name"));
        Recipe->TryGetStringField(TEXT("type"), Item.Type);
        Item.Balance = Recipe->GetStringField(TEXT("balance"));
        (*Card)->TryGetStringField(TEXT("manufacturer"), Item.Manufacturer);
        (*Card)->TryGetStringField(TEXT("element"), Item.Element);
        Item.Rarity = int32(Number(*Card, TEXT("rarity"), 1));
        Item.Level = int32(Number(*Card, TEXT("level"), 1));
        Item.Damage = Number(*Card, TEXT("damage"), 0);
        Item.FireRate = FMath::Max(0.1f, Number(*Card, TEXT("fire_rate"), 1));
        Item.ReloadTime = Number(*Card, TEXT("reload_time"), 0);
        Item.Magazine = Number(*Card, TEXT("magazine"), 0);
        Item.Spread = Number(*Card, TEXT("spread"), 0);
        Item.ShotCost = Number(*Card, TEXT("shot_cost"), 1);
        Item.SpinUp = Number(*Card, TEXT("spin_up"), 0);
        (*Card)->TryGetStringField(TEXT("spin_mode"), Item.SpinMode);
        Item.SpinStartIntervalScale = Number(*Card, TEXT("spin_start_interval_scale"), 1);
        double Extra = 0;
        if ((*Card)->TryGetNumberField(TEXT("accuracy"), Extra) && FMath::IsFinite(Extra))
        {
            Item.Accuracy = float(FMath::Clamp(Extra, 0.0, 100.0));
            Item.bHasAccuracy = true;
            (*Card)->TryGetBoolField(TEXT("accuracy_known"), Item.bAccuracyKnown);
        }
        if ((*Card)->TryGetNumberField(TEXT("sale_value"), Extra) && FMath::IsFinite(Extra) && Extra >= 0 && Extra <= MAX_int32)
        {
            Item.SaleValue = int32(Extra);
            Item.bHasSaleValue = true;
            (*Card)->TryGetBoolField(TEXT("sale_value_known"), Item.bSaleValueKnown);
        }
        (*Card)->TryGetStringField(TEXT("fun_stats"), Item.FunStats);
        Recipe->TryGetStringArrayField(TEXT("gestalt_fragments"), Item.Fragments);
        if (!CanAddToBackpack())
        {
            UE_LOG(LogTemp, Warning, TEXT("OpenWillow inventory: backpack full (%d), skipped recipe %s"), BackpackCapacity, *ItemId);
            continue;
        }
        Backpack.Add(MoveTemp(Item));
    }
    UE_LOG(LogTemp, Display, TEXT("OpenWillow inventory loaded %d weapon recipe(s) from %s"), Backpack.Num(), *Directory);
    return Backpack.Num();
}

int32 UOpenWillowInventory::LoadGearManifest(const FString& FilePath)
{
    FString Text;
    if (!FFileHelper::LoadFileToString(Text, *FilePath))
    {
        UE_LOG(LogTemp, Display, TEXT("OpenWillow gear manifest unavailable: %s"), *FilePath);
        return 0;
    }
    TSharedPtr<FJsonObject> Manifest;
    if (!FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Manifest) || !Manifest)
    {
        UE_LOG(LogTemp, Warning, TEXT("OpenWillow gear manifest is invalid JSON: %s"), *FilePath);
        return 0;
    }
    double Version = 0;
    const TArray<TSharedPtr<FJsonValue>>* Items = nullptr;
    if (!Manifest->TryGetNumberField(TEXT("schemaVersion"), Version) || Version != 1
        || !Manifest->TryGetArrayField(TEXT("items"), Items))
    {
        UE_LOG(LogTemp, Warning, TEXT("OpenWillow gear manifest requires schemaVersion 1 and an items array: %s"), *FilePath);
        return 0;
    }

    int32 Loaded = 0;
    for (const TSharedPtr<FJsonValue>& Value : *Items)
    {
        const TSharedPtr<FJsonObject> ItemObject = Value.IsValid() ? Value->AsObject() : nullptr;
        if (!ItemObject) continue;
        FOpenWillowGearItem Item;
        ItemObject->TryGetStringField(TEXT("id"), Item.Id);
        ItemObject->TryGetStringField(TEXT("assetId"), Item.AssetId);
        ItemObject->TryGetStringField(TEXT("itemType"), Item.ItemType);
        ItemObject->TryGetStringField(TEXT("name"), Item.Name);
        ItemObject->TryGetStringField(TEXT("manufacturer"), Item.Manufacturer);
        ItemObject->TryGetStringField(TEXT("rarityColor"), Item.RarityColor);
        ItemObject->TryGetStringField(TEXT("funStats"), Item.FunStats);
        ItemObject->TryGetStringField(TEXT("funStatsMarkup"), Item.FunStatsMarkup);
        if (Item.Id.IsEmpty() || Item.Name.IsEmpty() || !IsGearItemType(Item.ItemType) || IsKnownItemId(Item.Id))
            continue;

        double NumberValue = 0;
        if (ItemObject->TryGetNumberField(TEXT("level"), NumberValue) && FMath::IsFinite(NumberValue)
            && NumberValue >= 1 && NumberValue <= MAX_int32 && NumberValue == FMath::FloorToDouble(NumberValue))
        {
            Item.Level = int32(NumberValue);
            Item.bLevelKnown = true;
        }
        if (ItemObject->TryGetNumberField(TEXT("rarity"), NumberValue) && FMath::IsFinite(NumberValue)
            && NumberValue >= 1 && NumberValue <= 5 && NumberValue == FMath::FloorToDouble(NumberValue))
        {
            Item.Rarity = int32(NumberValue);
            Item.bRarityKnown = true;
        }
        if (ItemObject->TryGetNumberField(TEXT("saleValue"), NumberValue) && FMath::IsFinite(NumberValue)
            && NumberValue >= 0 && NumberValue <= MAX_int32 && NumberValue == FMath::FloorToDouble(NumberValue))
        {
            Item.SaleValue = int32(NumberValue);
            Item.bSaleValueKnown = true;
        }
        Item.bFavoriteKnown = ItemObject->TryGetBoolField(TEXT("favorite"), Item.bFavorite);
        Item.bTrashKnown = ItemObject->TryGetBoolField(TEXT("trash"), Item.bTrash);

        const TArray<TSharedPtr<FJsonValue>>* Stats = nullptr;
        if (ItemObject->TryGetArrayField(TEXT("stats"), Stats))
        {
            for (const TSharedPtr<FJsonValue>& StatValue : *Stats)
            {
                const TSharedPtr<FJsonObject> StatObject = StatValue.IsValid() ? StatValue->AsObject() : nullptr;
                if (!StatObject) continue;
                FOpenWillowGearStat Stat;
                if (!StatObject->TryGetStringField(TEXT("label"), Stat.Label)
                    || !StatObject->TryGetStringField(TEXT("value"), Stat.Value)
                    || Stat.Label.IsEmpty()) continue;
                Stat.bHasBetterDirection = StatObject->TryGetBoolField(TEXT("higherIsBetter"), Stat.bHigherIsBetter);
                StatObject->TryGetStringField(TEXT("icon"), Stat.Icon);
                Item.Stats.Add(MoveTemp(Stat));
            }
        }

        if (AddGearToBackpack(MoveTemp(Item))) ++Loaded;
    }
    UE_LOG(LogTemp, Display, TEXT("OpenWillow gear manifest loaded %d item(s) from %s"), Loaded, *FilePath);
    return Loaded;
}

bool UOpenWillowInventory::Equip(int32 Item, int32 Slot)
{
    if (!Backpack.IsValidIndex(Item) || !IsWeaponSlotUnlocked(Slot)) return false;
    // Moving an already-equipped weapon to another slot swaps the two slots.
    // Equipping a backpack weapon into a filled slot returns the old weapon to
    // the backpack because it remains in Backpack but loses its slot link.
    for (int32 OtherSlot = 0; OtherSlot < SlotCount; ++OtherSlot)
    {
        if (Slots[OtherSlot] != Item) continue;
        if (OtherSlot != Slot) Swap(Slots[OtherSlot], Slots[Slot]);
        return true;
    }
    Slots[Slot] = Item;
    return true;
}

bool UOpenWillowInventory::SetWeaponSlotsUnlocked(int32 Count)
{
    if (Count < MinWeaponSlotsUnlocked || Count > SlotCount) return false;
    for (int32 Slot = Count; Slot < SlotCount; ++Slot)
        if (Backpack.IsValidIndex(Slots[Slot])) return false;
    WeaponSlotsUnlocked = Count;
    return true;
}

bool UOpenWillowInventory::Unequip(int32 Slot)
{
    if (Slot < 0 || Slot >= SlotCount || !Backpack.IsValidIndex(Slots[Slot]) || !CanAddToBackpack()) return false;
    Slots[Slot] = INDEX_NONE;
    if (ActiveSlot == Slot) ActiveSlot = INDEX_NONE;
    return true;
}

int32 UOpenWillowInventory::FindItemIndexById(const FString& Id) const
{
    if (Id.IsEmpty()) return INDEX_NONE;
    return Backpack.IndexOfByPredicate([&Id](const FOpenWillowWeaponItem& Item)
        { return StableId(Item) == Id; });
}

const FOpenWillowWeaponItem* UOpenWillowInventory::FindItemById(const FString& Id) const
{
    const int32 Index = FindItemIndexById(Id);
    return Backpack.IsValidIndex(Index) ? &Backpack[Index] : nullptr;
}

bool UOpenWillowInventory::ToggleFavoriteById(const FString& Id)
{
    const int32 Index = FindItemIndexById(Id);
    if (Backpack.IsValidIndex(Index))
    {
        FOpenWillowWeaponItem& Item = Backpack[Index];
        Item.bFavorite = !Item.bFavorite;
        Item.bFavoriteKnown = true;
        if (Item.bFavorite) { Item.bTrash = false; Item.bTrashKnown = true; }
        return true;
    }
    const int32 GearIndex = FindGearIndexById(Id);
    if (!GearItems.IsValidIndex(GearIndex)) return false;
    FOpenWillowGearItem& Item = GearItems[GearIndex];
    Item.bFavorite = !Item.bFavorite;
    Item.bFavoriteKnown = true;
    if (Item.bFavorite) { Item.bTrash = false; Item.bTrashKnown = true; }
    return true;
}

bool UOpenWillowInventory::ToggleTrashById(const FString& Id)
{
    const int32 Index = FindItemIndexById(Id);
    if (Backpack.IsValidIndex(Index))
    {
        FOpenWillowWeaponItem& Item = Backpack[Index];
        Item.bTrash = !Item.bTrash;
        Item.bTrashKnown = true;
        if (Item.bTrash) { Item.bFavorite = false; Item.bFavoriteKnown = true; }
        return true;
    }
    const int32 GearIndex = FindGearIndexById(Id);
    if (!GearItems.IsValidIndex(GearIndex)) return false;
    FOpenWillowGearItem& Item = GearItems[GearIndex];
    Item.bTrash = !Item.bTrash;
    Item.bTrashKnown = true;
    if (Item.bTrash) { Item.bFavorite = false; Item.bFavoriteKnown = true; }
    return true;
}

bool UOpenWillowInventory::AddToBackpack(FOpenWillowWeaponItem Item)
{
    if (Item.Id.IsEmpty() || !CanAddToBackpack()) return false;
    if (Item.InstanceId.IsEmpty()) Item.InstanceId = Item.Id;
    if (IsKnownItemId(Item.InstanceId))
    {
        // Recipe identity is shared by copies; inventory identity is not.
        // Suffixes are deterministic for a given acquisition order and stay
        // stable through equipment changes and sorting.
        for (int32 Copy = 2; ; ++Copy)
        {
            const FString Candidate = FString::Printf(TEXT("%s#%d"), *Item.Id, Copy);
            if (!IsKnownItemId(Candidate))
            {
                Item.InstanceId = Candidate;
                break;
            }
        }
    }
    Backpack.Add(MoveTemp(Item));
    return true;
}

bool UOpenWillowInventory::AddGearToBackpack(FOpenWillowGearItem Item)
{
    if (Item.Id.IsEmpty() || Item.Name.IsEmpty() || !IsGearItemType(Item.ItemType) || !CanAddToBackpack())
        return false;
    if (IsKnownItemId(Item.Id))
    {
        const FString BaseId = Item.Id;
        for (int32 Copy = 2; ; ++Copy)
        {
            const FString Candidate = FString::Printf(TEXT("%s#%d"), *BaseId, Copy);
            if (!IsKnownItemId(Candidate))
            {
                Item.Id = Candidate;
                break;
            }
        }
    }
    GearItems.Add(MoveTemp(Item));
    return true;
}

int32 UOpenWillowInventory::GearSlotIndex(const FString& GearSlot)
{
    if (GearSlot.Equals(TEXT("shield"), ESearchCase::IgnoreCase)) return 0;
    if (GearSlot.Equals(TEXT("grenadeMod"), ESearchCase::IgnoreCase)
        || GearSlot.Equals(TEXT("grenade_mod"), ESearchCase::IgnoreCase)) return 1;
    if (GearSlot.Equals(TEXT("classMod"), ESearchCase::IgnoreCase)
        || GearSlot.Equals(TEXT("class_mod"), ESearchCase::IgnoreCase)) return 2;
    if (GearSlot.Equals(TEXT("relic"), ESearchCase::IgnoreCase)
        || GearSlot.Equals(TEXT("artifact"), ESearchCase::IgnoreCase)) return 3;
    return INDEX_NONE;
}

const TCHAR* UOpenWillowInventory::GearSlotName(int32 Slot)
{
    switch (Slot)
    {
    case 0: return TEXT("shield");
    case 1: return TEXT("grenadeMod");
    case 2: return TEXT("classMod");
    case 3: return TEXT("relic");
    default: return TEXT("");
    }
}

int32 UOpenWillowInventory::FindGearIndexById(const FString& Id) const
{
    if (Id.IsEmpty()) return INDEX_NONE;
    return GearItems.IndexOfByPredicate([&Id](const FOpenWillowGearItem& Item) { return Item.Id == Id; });
}

const FOpenWillowGearItem* UOpenWillowInventory::FindGearById(const FString& Id) const
{
    const int32 Index = FindGearIndexById(Id);
    return GearItems.IsValidIndex(Index) ? &GearItems[Index] : nullptr;
}

const FOpenWillowGearItem* UOpenWillowInventory::GearSlotItem(const FString& GearSlot) const
{
    const int32 Slot = GearSlotIndex(GearSlot);
    return Slot == INDEX_NONE ? nullptr : FindGearById(GearSlots[Slot]);
}

bool UOpenWillowInventory::EquipGearById(const FString& Id, const FString& GearSlot, int32 PlayerLevel)
{
    const int32 Slot = GearSlotIndex(GearSlot);
    const int32 ItemIndex = FindGearIndexById(Id);
    if (Slot == INDEX_NONE || !GearItems.IsValidIndex(ItemIndex)) return false;
    const FOpenWillowGearItem& Item = GearItems[ItemIndex];
    if (Item.ItemType != GearTypeForSlot(Slot) || !Item.bLevelKnown || Item.Level > PlayerLevel) return false;
    for (int32 OtherSlot = 0; OtherSlot < GearSlotCount; ++OtherSlot)
    {
        if (GearSlots[OtherSlot] != Id) continue;
        return OtherSlot == Slot;
    }
    // Replacing a slot moves its previous item back into the shared backpack.
    GearSlots[Slot] = Id;
    return true;
}

bool UOpenWillowInventory::UnequipGear(const FString& GearSlot)
{
    const int32 Slot = GearSlotIndex(GearSlot);
    if (Slot == INDEX_NONE || GearSlots[Slot].IsEmpty() || !CanAddToBackpack()) return false;
    GearSlots[Slot].Reset();
    return true;
}

bool UOpenWillowInventory::IsKnownItemId(const FString& Id) const
{
    return FindItemIndexById(Id) != INDEX_NONE || FindGearIndexById(Id) != INDEX_NONE;
}

bool UOpenWillowInventory::TakeById(const FString& Id, FOpenWillowTakenInventoryItem& OutItem)
{
    OutItem = FOpenWillowTakenInventoryItem();
    const int32 WeaponIndex = FindItemIndexById(Id);
    if (Backpack.IsValidIndex(WeaponIndex))
    {
        OutItem.Weapon = Backpack[WeaponIndex];
        for (int32 Slot = 0; Slot < SlotCount; ++Slot)
        {
            if (Slots[Slot] == WeaponIndex)
            {
                Slots[Slot] = INDEX_NONE;
                if (ActiveSlot == Slot) ActiveSlot = INDEX_NONE;
            }
            else if (Slots[Slot] > WeaponIndex)
            {
                --Slots[Slot];
            }
        }
        Backpack.RemoveAt(WeaponIndex);
        return true;
    }

    const int32 GearIndex = FindGearIndexById(Id);
    if (!GearItems.IsValidIndex(GearIndex)) return false;
    OutItem.bIsGear = true;
    OutItem.Gear = GearItems[GearIndex];
    for (FString& EquippedId : GearSlots)
        if (EquippedId == Id) EquippedId.Reset();
    GearItems.RemoveAt(GearIndex);
    return true;
}

bool UOpenWillowInventory::CanAddToBackpack() const
{
    return BackpackCount() < BackpackCapacity;
}

int32 UOpenWillowInventory::BackpackCount() const
{
    TSet<int32> EquippedWeapons;
    for (const int32 Slot : Slots)
        if (Backpack.IsValidIndex(Slot)) EquippedWeapons.Add(Slot);
    TSet<FString> EquippedGear;
    for (const FString& Id : GearSlots)
        if (FindGearIndexById(Id) != INDEX_NONE) EquippedGear.Add(Id);
    return Backpack.Num() + GearItems.Num() - EquippedWeapons.Num() - EquippedGear.Num();
}

bool UOpenWillowInventory::SetBackpackCapacity(int32 Capacity)
{
    if (Capacity < DefaultBackpackCapacity || Capacity > MaximumBackpackCapacity || Capacity < BackpackCount()) return false;
    BackpackCapacity = Capacity;
    return true;
}

const FOpenWillowWeaponItem* UOpenWillowInventory::SlotItem(int32 Slot) const
{
    if (Slot < 0 || Slot >= SlotCount || !Backpack.IsValidIndex(Slots[Slot])) return nullptr;
    return &Backpack[Slots[Slot]];
}

const FOpenWillowWeaponItem* UOpenWillowInventory::ActiveWeapon() const
{
    return SlotItem(ActiveSlot);
}

FOpenWillowWeaponItem* UOpenWillowInventory::ActiveWeaponMutable()
{
    return const_cast<FOpenWillowWeaponItem*>(SlotItem(ActiveSlot));
}

const TCHAR* UOpenWillowInventory::AmmoTypeKey(EOpenWillowAmmoType Type)
{
    switch (Type)
    {
    case EOpenWillowAmmoType::Pistol: return TEXT("pistol");
    case EOpenWillowAmmoType::SMG: return TEXT("smg");
    case EOpenWillowAmmoType::AssaultRifle: return TEXT("ar");
    case EOpenWillowAmmoType::Shotgun: return TEXT("shotgun");
    case EOpenWillowAmmoType::Sniper: return TEXT("sniper");
    case EOpenWillowAmmoType::Launcher: return TEXT("launcher");
    case EOpenWillowAmmoType::Grenade: return TEXT("grenade");
    default: return TEXT("");
    }
}

bool UOpenWillowInventory::ResolveAmmoType(const FOpenWillowWeaponItem& Item, EOpenWillowAmmoType& OutType)
{
    struct FMatch { const TCHAR* Name; const TCHAR* BalancePrefix; EOpenWillowAmmoType Type; };
    // Display names are what tools/weapon_stats.py writes to the recipe;
    // balance prefixes are the package names of the weapon balance objects
    // (the Infinity recipes carry no display type, only GD_Weap_Pistol.*).
    static const FMatch Matches[] =
    {
        {TEXT("Pistol"), TEXT("GD_Weap_Pistol"), EOpenWillowAmmoType::Pistol},
        {TEXT("SMG"), TEXT("GD_Weap_SMG"), EOpenWillowAmmoType::SMG},
        {TEXT("Assault Rifle"), TEXT("GD_Weap_AssaultRifle"), EOpenWillowAmmoType::AssaultRifle},
        {TEXT("Shotgun"), TEXT("GD_Weap_Shotgun"), EOpenWillowAmmoType::Shotgun},
        {TEXT("Sniper Rifle"), TEXT("GD_Weap_SniperRifle"), EOpenWillowAmmoType::Sniper},
        {TEXT("Launcher"), TEXT("GD_Weap_Launchers"), EOpenWillowAmmoType::Launcher},
    };
    for (const FMatch& Match : Matches)
    {
        if (Item.Type.Equals(Match.Name, ESearchCase::IgnoreCase)
            || (Item.Type.IsEmpty() && Item.Balance.StartsWith(Match.BalancePrefix)))
        {
            OutType = Match.Type;
            return true;
        }
    }
    return false;
}

bool UOpenWillowInventory::SetAmmoCurrent(EOpenWillowAmmoType Type, int32 Current)
{
    if (Type >= EOpenWillowAmmoType::Count) return false;
    FOpenWillowAmmoPool& Pool = AmmoPools[int32(Type)];
    Pool.Current = FMath::Clamp(Current, 0, Pool.Max);
    return true;
}

int32 UOpenWillowInventory::ShotCostRounds(const FOpenWillowWeaponItem& Item)
{
    return Item.ShotCost > 0.f ? FMath::Max(1, FMath::RoundToInt(Item.ShotCost)) : 0;
}

int32 UOpenWillowInventory::MagazineSize(const FOpenWillowWeaponItem& Item)
{
    return FMath::Max(1, FMath::RoundToInt(Item.Magazine));
}

int32 UOpenWillowInventory::MagazineLeft(const FOpenWillowWeaponItem& Item)
{
    return Item.MagazineLeft < 0 ? MagazineSize(Item) : FMath::Min(Item.MagazineLeft, MagazineSize(Item));
}

bool UOpenWillowInventory::ConsumeShot(FOpenWillowWeaponItem& Item) const
{
    const int32 Cost = ShotCostRounds(Item);
    if (Cost == 0) return true; // Infinity: no ammo, no magazine change
    const int32 Left = MagazineLeft(Item);
    if (Left < Cost) return false;
    Item.MagazineLeft = Left - Cost;
    return true;
}

bool UOpenWillowInventory::CanReload(const FOpenWillowWeaponItem& Item) const
{
    EOpenWillowAmmoType Type;
    if (ShotCostRounds(Item) == 0 || !ResolveAmmoType(Item, Type)) return false;
    return MagazineLeft(Item) < MagazineSize(Item) && AmmoPools[int32(Type)].Current > 0;
}

int32 UOpenWillowInventory::CompleteReload(FOpenWillowWeaponItem& Item)
{
    EOpenWillowAmmoType Type;
    if (!CanReload(Item) || !ResolveAmmoType(Item, Type)) return 0;
    FOpenWillowAmmoPool& Pool = AmmoPools[int32(Type)];
    const int32 Moved = FMath::Min(MagazineSize(Item) - MagazineLeft(Item), Pool.Current);
    Pool.Current -= Moved;
    Item.MagazineLeft = MagazineLeft(Item) + Moved;
    return Moved;
}

FString UOpenWillowInventory::StateJson(int32 Level) const
{
    TSharedRef<FJsonObject> State = MakeShared<FJsonObject>();
    State->SetNumberField(TEXT("activeSlot"), ActiveSlot);
    State->SetNumberField(TEXT("level"), Level);
    State->SetNumberField(TEXT("backpackCount"), BackpackCount());
    State->SetNumberField(TEXT("backpackCapacity"), BackpackCapacity);
    // items[] lists every held weapon and gear item, equipped ones included;
    // backpackCount excludes the equipped ones, as BL2's capacity does.
    State->SetNumberField(TEXT("slotsUnlocked"), WeaponSlotsUnlocked);
    if (bMoneyKnown) State->SetNumberField(TEXT("money"), Money);
    if (bEridiumKnown) State->SetNumberField(TEXT("eridium"), Eridium);
    TSharedRef<FJsonObject> AmmoJson = MakeShared<FJsonObject>();
    for (int32 Type = 0; Type < AmmoTypeCount; ++Type)
    {
        TSharedRef<FJsonObject> PoolJson = MakeShared<FJsonObject>();
        PoolJson->SetNumberField(TEXT("current"), AmmoPools[Type].Current);
        PoolJson->SetNumberField(TEXT("max"), AmmoPools[Type].Max);
        AmmoJson->SetObjectField(AmmoTypeKey(EOpenWillowAmmoType(Type)), PoolJson);
    }
    State->SetObjectField(TEXT("ammo"), AmmoJson);
    EOpenWillowAmmoType ActiveAmmo;
    if (const FOpenWillowWeaponItem* Held = ActiveWeapon())
        if (ResolveAmmoType(*Held, ActiveAmmo)) State->SetStringField(TEXT("activeAmmoType"), AmmoTypeKey(ActiveAmmo));
    TArray<TSharedPtr<FJsonValue>> ItemsJson, SlotsJson;
    for (const FOpenWillowWeaponItem& Item : Backpack)
    {
        TSharedRef<FJsonObject> Value = MakeShared<FJsonObject>();
        Value->SetStringField(TEXT("id"), StableId(Item));
        Value->SetStringField(TEXT("assetId"), Item.Id);
        Value->SetStringField(TEXT("itemType"), TEXT("weapon"));
        Value->SetStringField(TEXT("name"), Item.Name);
        EOpenWillowAmmoType ItemAmmo;
        const bool bHasAmmo = ResolveAmmoType(Item, ItemAmmo);
        Value->SetStringField(TEXT("type"), Item.Type.IsEmpty() && bHasAmmo ? FString(WeaponTypeLabel(ItemAmmo)) : Item.Type);
        if (bHasAmmo) Value->SetStringField(TEXT("ammoType"), AmmoTypeKey(ItemAmmo));
        Value->SetStringField(TEXT("manufacturer"), Item.Manufacturer);
        Value->SetStringField(TEXT("element"), Item.Element);
        Value->SetNumberField(TEXT("rarity"), Item.Rarity);
        Value->SetBoolField(TEXT("rarityKnown"), true);
        Value->SetNumberField(TEXT("level"), Item.Level);
        Value->SetBoolField(TEXT("levelKnown"), true);
        Value->SetNumberField(TEXT("damage"), Item.Damage);
        Value->SetNumberField(TEXT("fireRate"), Item.FireRate);
        Value->SetNumberField(TEXT("reloadTime"), Item.ReloadTime);
        Value->SetNumberField(TEXT("magazine"), Item.Magazine);
        Value->SetNumberField(TEXT("spread"), Item.Spread);
        // Absent unless derived; the Known flags mark whether it was checked.
        if (Item.bHasAccuracy) Value->SetNumberField(TEXT("accuracy"), Item.Accuracy);
        Value->SetBoolField(TEXT("accuracyKnown"), Item.bHasAccuracy && Item.bAccuracyKnown);
        if (Item.bHasSaleValue) Value->SetNumberField(TEXT("value"), Item.SaleValue);
        Value->SetBoolField(TEXT("valueKnown"), Item.bHasSaleValue && Item.bSaleValueKnown);
        if (!Item.FunStats.IsEmpty()) Value->SetStringField(TEXT("funStats"), Item.FunStats);
        Value->SetNumberField(TEXT("shotCost"), Item.ShotCost);
        Value->SetNumberField(TEXT("spinUp"), Item.SpinUp);
        Value->SetStringField(TEXT("spinMode"), Item.SpinMode);
        Value->SetNumberField(TEXT("spinStartIntervalScale"), Item.SpinStartIntervalScale);
        Value->SetBoolField(TEXT("favorite"), Item.bFavorite);
        Value->SetBoolField(TEXT("trash"), Item.bTrash);
        Value->SetBoolField(TEXT("favoriteKnown"), Item.bFavoriteKnown);
        Value->SetBoolField(TEXT("trashKnown"), Item.bTrashKnown);
        ItemsJson.Add(MakeShared<FJsonValueObject>(Value));
    }
    for (const FOpenWillowGearItem& Item : GearItems)
    {
        TSharedRef<FJsonObject> Value = MakeShared<FJsonObject>();
        Value->SetStringField(TEXT("id"), Item.Id);
        Value->SetStringField(TEXT("assetId"), Item.AssetId);
        Value->SetStringField(TEXT("itemType"), Item.ItemType);
        Value->SetStringField(TEXT("name"), Item.Name);
        Value->SetStringField(TEXT("manufacturer"), Item.Manufacturer);
        Value->SetNumberField(TEXT("rarity"), Item.Rarity);
        Value->SetBoolField(TEXT("rarityKnown"), Item.bRarityKnown);
        Value->SetStringField(TEXT("rarityColor"), Item.RarityColor);
        Value->SetNumberField(TEXT("level"), Item.Level);
        Value->SetBoolField(TEXT("levelKnown"), Item.bLevelKnown);
        Value->SetNumberField(TEXT("value"), Item.SaleValue);
        Value->SetBoolField(TEXT("valueKnown"), Item.bSaleValueKnown);
        Value->SetBoolField(TEXT("favorite"), Item.bFavorite);
        Value->SetBoolField(TEXT("trash"), Item.bTrash);
        Value->SetBoolField(TEXT("favoriteKnown"), Item.bFavoriteKnown);
        Value->SetBoolField(TEXT("trashKnown"), Item.bTrashKnown);
        Value->SetStringField(TEXT("funStats"), Item.FunStats);
        if (!Item.FunStatsMarkup.IsEmpty()) Value->SetStringField(TEXT("funStatsMarkup"), Item.FunStatsMarkup);
        TArray<TSharedPtr<FJsonValue>> StatsJson;
        for (const FOpenWillowGearStat& Stat : Item.Stats)
        {
            TSharedRef<FJsonObject> StatValue = MakeShared<FJsonObject>();
            StatValue->SetStringField(TEXT("label"), Stat.Label);
            if (!Stat.Icon.IsEmpty()) StatValue->SetStringField(TEXT("icon"), Stat.Icon);
            StatValue->SetStringField(TEXT("value"), Stat.Value);
            if (Stat.bHasBetterDirection) StatValue->SetBoolField(TEXT("higherIsBetter"), Stat.bHigherIsBetter);
            StatsJson.Add(MakeShared<FJsonValueObject>(StatValue));
        }
        Value->SetArrayField(TEXT("stats"), StatsJson);
        ItemsJson.Add(MakeShared<FJsonValueObject>(Value));
    }
    for (int32 Slot = 0; Slot < SlotCount; ++Slot)
    {
        const FOpenWillowWeaponItem* Item = SlotItem(Slot);
        if (Item) SlotsJson.Add(MakeShared<FJsonValueString>(StableId(*Item)));
        else SlotsJson.Add(MakeShared<FJsonValueNull>());
    }
    State->SetArrayField(TEXT("items"), ItemsJson);
    State->SetArrayField(TEXT("slots"), SlotsJson);
    TSharedRef<FJsonObject> GearSlotsJson = MakeShared<FJsonObject>();
    for (int32 Slot = 0; Slot < GearSlotCount; ++Slot)
    {
        const FString SlotId = GearSlots[Slot];
        if (!SlotId.IsEmpty()) GearSlotsJson->SetStringField(GearSlotName(Slot), SlotId);
        else GearSlotsJson->SetField(GearSlotName(Slot), MakeShared<FJsonValueNull>());
    }
    State->SetObjectField(TEXT("gearSlots"), GearSlotsJson);
    FString Json;
    FJsonSerializer::Serialize(State, TJsonWriterFactory<TCHAR, TCondensedJsonPrintPolicy<TCHAR>>::Create(&Json));
    return Json;
}

bool UOpenWillowInventory::RunSelfTest()
{
    UOpenWillowInventory* Inv = NewObject<UOpenWillowInventory>(GetTransientPackage());
    int32 Failures = 0;
    auto Check = [&Failures](bool bOk, const TCHAR* What)
    {
        if (!bOk) { ++Failures; UE_LOG(LogTemp, Error, TEXT("OpenWillow inventory self-test FAILED: %s"), What); }
    };
    auto Make = [](const TCHAR* Id, const TCHAR* Type)
    {
        FOpenWillowWeaponItem Item;
        Item.Id = Id;
        Item.Name = Id;
        Item.Type = Type;
        Item.Magazine = 3.f;
        Item.Level = 5;
        return Item;
    };
    Check(Inv->AddToBackpack(Make(TEXT("a"), TEXT("Pistol"))), TEXT("add a"));
    Check(Inv->AddToBackpack(Make(TEXT("a"), TEXT("Pistol"))), TEXT("add duplicate a"));
    Check(Inv->AddToBackpack(Make(TEXT("b"), TEXT("Shotgun"))), TEXT("add b"));
    Check(Inv->FindItemById(TEXT("a#2")) != nullptr, TEXT("duplicate gets a stable #2 id"));
    Check(Inv->BackpackCount() == 3, TEXT("count 3 before equip"));
    Check(Inv->SetWeaponSlotsUnlocked(2), TEXT("lock slots 3-4 when empty"));
    Check(!Inv->Equip(Inv->FindItemIndexById(TEXT("a")), 2), TEXT("equip into locked slot refused"));
    Check(Inv->Equip(Inv->FindItemIndexById(TEXT("a")), 0), TEXT("equip a into slot 1"));
    Check(Inv->Equip(Inv->FindItemIndexById(TEXT("b")), 1), TEXT("equip b into slot 2"));
    Check(Inv->BackpackCount() == 1, TEXT("equipped items leave the backpack count"));
    Check(!Inv->SetWeaponSlotsUnlocked(1), TEXT("cannot lock below two slots"));
    Check(Inv->SetWeaponSlotsUnlocked(4) && Inv->Equip(Inv->FindItemIndexById(TEXT("a#2")), 3),
        TEXT("unlock and equip a#2 into slot 4"));
    Check(!Inv->SetWeaponSlotsUnlocked(2), TEXT("cannot lock a slot that holds a weapon"));
    Check(Inv->Unequip(3) && Inv->SlotItem(3) == nullptr, TEXT("unequip slot 4"));
    Check(Inv->ToggleFavoriteById(TEXT("a#2")) && Inv->FindItemById(TEXT("a#2"))->bFavorite, TEXT("favorite"));
    Check(Inv->ToggleTrashById(TEXT("a#2")) && Inv->FindItemById(TEXT("a#2"))->bTrash
        && !Inv->FindItemById(TEXT("a#2"))->bFavorite, TEXT("trash clears favorite"));
    FOpenWillowTakenInventoryItem Taken;
    Check(Inv->TakeById(TEXT("a"), Taken) && Taken.Weapon.InstanceId == TEXT("a"), TEXT("drop equipped a by id"));
    Check(Inv->SlotItem(0) == nullptr && Inv->SlotItem(1) && Inv->SlotItem(1)->Id == TEXT("b"),
        TEXT("slot indices stay correct after removal"));
    Check(Inv->AddToBackpack(Taken.Weapon) && Inv->FindItemById(TEXT("a")) != nullptr, TEXT("pick a back up with its id"));
    // Magazine and ammo pool.
    FOpenWillowWeaponItem* Gun = const_cast<FOpenWillowWeaponItem*>(Inv->FindItemById(TEXT("b")));
    EOpenWillowAmmoType Ammo = EOpenWillowAmmoType::Pistol;
    Check(Gun && ResolveAmmoType(*Gun, Ammo) && Ammo == EOpenWillowAmmoType::Shotgun,
        TEXT("shotgun resolves to the shotgun pool"));
    if (Gun)
    {
        Check(Inv->ConsumeShot(*Gun) && Inv->ConsumeShot(*Gun) && Inv->ConsumeShot(*Gun) && !Inv->ConsumeShot(*Gun),
            TEXT("three shots empty a three-round magazine"));
        Inv->SetAmmoCurrent(EOpenWillowAmmoType::Shotgun, 2);
        Check(Inv->CanReload(*Gun) && Inv->CompleteReload(*Gun) == 2
            && Inv->AmmoPool(EOpenWillowAmmoType::Shotgun).Current == 0 && MagazineLeft(*Gun) == 2,
            TEXT("reload moves only what the pool holds"));
        Check(!Inv->CanReload(*Gun), TEXT("an empty pool cannot reload"));
    }
    UE_LOG(LogTemp, Display, TEXT("OpenWillow inventory self-test: %s"), Failures ? TEXT("FAILED") : TEXT("passed"));
    return Failures == 0;
}
