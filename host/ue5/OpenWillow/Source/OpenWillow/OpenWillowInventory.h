#pragma once

#include "CoreMinimal.h"
#include "Components/ActorComponent.h"
#include "OpenWillowInventory.generated.h"

// BL2 ammo pools, in the order the inventory screen lists them. The pool
// names in the game data are Repeater_Pistol, Patrol_SMG, Combat_Rifle,
// Combat_Shotgun, Sniper_Rifle, Rocket_Launcher and Grenade_Protean.
enum class EOpenWillowAmmoType : uint8
{
    Pistol,
    SMG,
    AssaultRifle,
    Shotgun,
    Sniper,
    Launcher,
    Grenade,
    Count
};

// Reserve ammo the player holds (magazine contents are not part of it) and
// the pool's capacity.
struct FOpenWillowAmmoPool
{
    int32 Current = 0;
    int32 Max = 0;
};

// One rolled weapon, loaded from a tools/weapon_recipe.py recipe that
// tools/weapon_stats.py has filled in. Values are the evaluated item-card
// stats; see DECISIONS.md for which combination rules are UNVERIFIED.
USTRUCT()
struct FOpenWillowWeaponItem
{
    GENERATED_BODY()
    FString Id;            // recipe file stem, also the imported mesh name
    FString InstanceId;    // stable inventory identity; duplicates use stem#2, stem#3, ...
    FString Name;          // prefix + title, e.g. "Burning Infinity"
    FString Type;          // evaluated recipe display type, e.g. Shotgun
    FString Manufacturer;
    FString Element;       // "None", "Fire", "Shock", ...
    FString Balance;
    int32 Rarity = 1;      // 1 Common .. 5 Legendary
    int32 Level = 1;
    float Damage = 0;
    float FireRate = 1;    // shots per second
    float ReloadTime = 0;
    float Magazine = 0;
    float Spread = 0;
    float ShotCost = 1;    // 0 means firing costs no ammo (Infinity)
    float SpinUp = 0;      // seconds; 0 when the barrel does not spin
    FString SpinMode;      // WeaponTypeDefinition.BarrelSpinMode, e.g. BSM_SpinUpToFullFireRate
    float SpinStartIntervalScale = 1; // barrel StartingSpinUpFireIntervalMultiplier
    // Item-card extras from tools/weapon_stats.py (docs/verification/INVENTORY_CARD_STATS.md).
    // Each is forwarded to the menu only when present; the *Known flags say
    // whether the value was checked against real cards (false = UNVERIFIED).
    float Accuracy = 0;    // percent, presentation remap of the modelled spread
    bool bHasAccuracy = false;
    bool bAccuracyKnown = false;
    int32 SaleValue = 0;   // dollars, price calculator output rounded down
    bool bHasSaleValue = false;
    bool bSaleValueKnown = false;
    FString FunStats;      // red flavour line(s), '; ' separated like the gear schema
    TArray<FString> Fragments;
    bool bFavorite = false;
    bool bTrash = false;
    bool bFavoriteKnown = false;
    bool bTrashKnown = false;
    int32 MagazineLeft = -1; // rounds in the magazine; -1 means a full magazine, not yet fired
};

// Generic, host-authored gear entry. Optional card fields are only populated
// from the explicit local manifest; missing values remain unknown.
USTRUCT()
struct FOpenWillowGearStat
{
    GENERATED_BODY()
    FString Label;
    FString Value;
    FString Icon; // observed movie frame name, optional
    bool bHasBetterDirection = false;
    bool bHigherIsBetter = false;
};

USTRUCT()
struct FOpenWillowGearItem
{
    GENERATED_BODY()
    FString Id;            // stable inventory-instance identity from the local manifest
    FString AssetId;       // optional imported visual asset key; empty when unresolved
    FString ItemType;      // shield, grenade_mod, class_mod, or relic
    FString Name;
    FString Manufacturer;
    FString RarityColor;   // display color may be known when the numeric tier is not
    FString FunStats;
    FString FunStatsMarkup; // observed Flash text formatting, never browser HTML
    int32 Level = 0;
    int32 Rarity = 0;
    int32 SaleValue = 0;
    bool bLevelKnown = false;
    bool bRarityKnown = false;
    bool bSaleValueKnown = false;
    bool bFavorite = false;
    bool bTrash = false;
    bool bFavoriteKnown = false;
    bool bTrashKnown = false;
    TArray<FOpenWillowGearStat> Stats;
};

// The removed payload is preserved for the later world-pickup actor path.
struct FOpenWillowTakenInventoryItem
{
    bool bIsGear = false;
    FOpenWillowWeaponItem Weapon;
    FOpenWillowGearItem Gear;
};

// Backpack plus BL2's four weapon slots. Holds data only; the walker asks it
// for the active weapon and the HUD/inventory screens read it.
UCLASS()
class OPENWILLOW_API UOpenWillowInventory : public UActorComponent
{
    GENERATED_BODY()
public:
    static constexpr int32 SlotCount = 4;
    // A new BL2 character has two weapon slots; the "SDU Weapon Equip Slot"
    // item (present in the game data) unlocks the rest. How many slots a new
    // character starts with and how each SDU counts are NOT decoded, so the
    // host default is all four and -owslots=<2..4> overrides it (UNVERIFIED).
    static constexpr int32 MinWeaponSlotsUnlocked = 2;
    static constexpr int32 GearSlotCount = 4;
    static constexpr int32 AmmoTypeCount = int32(EOpenWillowAmmoType::Count);
    // UNVERIFIED against a live BL2 run: expected base backpack and SDU cap.
    static constexpr int32 DefaultBackpackCapacity = 12;
    static constexpr int32 MaximumBackpackCapacity = 39;
    // Loads every *.json recipe with stats under Directory; returns the count.
    int32 LoadRecipes(const FString& Directory);
    // Reads only the explicit host-authored JSON manifest under local/.
    int32 LoadGearManifest(const FString& FilePath);
    // Puts backpack item Item into weapon slot Slot (0-3). Locked slots refuse.
    bool Equip(int32 Item, int32 Slot);
    int32 GetWeaponSlotsUnlocked() const { return WeaponSlotsUnlocked; }
    bool IsWeaponSlotUnlocked(int32 Slot) const { return Slot >= 0 && Slot < WeaponSlotsUnlocked; }
    // Fails when a slot that would lock still holds a weapon.
    bool SetWeaponSlotsUnlocked(int32 Count);
    // Unequipping returns the weapon to the backpack.
    bool Unequip(int32 Slot);
    int32 FindItemIndexById(const FString& Id) const;
    const FOpenWillowWeaponItem* FindItemById(const FString& Id) const;
    bool ToggleFavoriteById(const FString& Id);
    bool ToggleTrashById(const FString& Id);
    // New loot must fit in the backpack and have a unique, stable ID.
    bool AddToBackpack(FOpenWillowWeaponItem Item);
    bool AddGearToBackpack(FOpenWillowGearItem Item);
    bool EquipGearById(const FString& Id, const FString& GearSlot, int32 PlayerLevel);
    bool UnequipGear(const FString& GearSlot);
    int32 FindGearIndexById(const FString& Id) const;
    const FOpenWillowGearItem* FindGearById(const FString& Id) const;
    const FOpenWillowGearItem* GearSlotItem(const FString& GearSlot) const;
    bool TakeById(const FString& Id, FOpenWillowTakenInventoryItem& OutItem);
    bool CanAddToBackpack() const;
    int32 BackpackCount() const;
    int32 GetBackpackCapacity() const { return BackpackCapacity; }
    bool SetBackpackCapacity(int32 Capacity);
    // Selects the slot to hold; an empty slot or -1 means unarmed.
    void SetActiveSlot(int32 Slot) { ActiveSlot = Slot >= 0 && Slot < SlotCount ? Slot : INDEX_NONE; }
    int32 GetActiveSlot() const { return ActiveSlot; }
    const FOpenWillowWeaponItem* ActiveWeapon() const;
    FOpenWillowWeaponItem* ActiveWeaponMutable();

    // Ammo pools. Capacity is the decoded base BaseMaxValue of each
    // D_Resourcepools.AmmoPools.*_Pool object with no ammo SDUs applied; the
    // starting reserve is a host choice (full), not decoded data.
    static const TCHAR* AmmoTypeKey(EOpenWillowAmmoType Type);
    // Maps a weapon to its pool from the evaluated type name, else from the
    // balance path prefix (GD_Weap_Pistol, ...). False when neither is known.
    static bool ResolveAmmoType(const FOpenWillowWeaponItem& Item, EOpenWillowAmmoType& OutType);
    const FOpenWillowAmmoPool& AmmoPool(EOpenWillowAmmoType Type) const { return AmmoPools[int32(Type)]; }
    bool SetAmmoCurrent(EOpenWillowAmmoType Type, int32 Current);
    // Rounds one shot draws from the magazine; 0 when the weapon costs no ammo.
    static int32 ShotCostRounds(const FOpenWillowWeaponItem& Item);
    static int32 MagazineSize(const FOpenWillowWeaponItem& Item);
    static int32 MagazineLeft(const FOpenWillowWeaponItem& Item);
    // Takes one shot from the magazine; false when the magazine is short.
    bool ConsumeShot(FOpenWillowWeaponItem& Item) const;
    // True when the magazine is not full and its pool holds ammo.
    bool CanReload(const FOpenWillowWeaponItem& Item) const;
    // Moves ammo from the pool into the magazine; returns rounds moved.
    int32 CompleteReload(FOpenWillowWeaponItem& Item);

    // Currency. Unset (unknown) until the host sets it; there is no vendor or
    // pickup economy yet, so nothing changes it during play.
    void SetMoney(int32 Value) { Money = FMath::Max(0, Value); bMoneyKnown = true; }
    void SetEridium(int32 Value) { Eridium = FMath::Max(0, Value); bEridiumKnown = true; }
    // Synthetic-item round trip of equip/unequip/take/re-add/favorite/trash
    // and the slot lock; logs each failure. Uses a transient instance.
    static bool RunSelfTest();
    const FOpenWillowWeaponItem* SlotItem(int32 Slot) const;
    const TArray<FOpenWillowWeaponItem>& Items() const { return Backpack; }
    const TArray<FOpenWillowGearItem>& GearItemList() const { return GearItems; }
    FString StateJson(int32 Level) const;
    static FString StableId(const FOpenWillowWeaponItem& Item)
    {
        return Item.InstanceId.IsEmpty() ? Item.Id : Item.InstanceId;
    }
    bool IsKnownItemId(const FString& Id) const;
    static int32 GearSlotIndex(const FString& GearSlot);
    static const TCHAR* GearSlotName(int32 Slot);
private:
    TArray<FOpenWillowWeaponItem> Backpack;
    TArray<FOpenWillowGearItem> GearItems;
    int32 Slots[SlotCount] = {INDEX_NONE, INDEX_NONE, INDEX_NONE, INDEX_NONE};
    FString GearSlots[GearSlotCount]; // order: shield, grenade mod, class mod, relic
    int32 ActiveSlot = INDEX_NONE;
    int32 BackpackCapacity = DefaultBackpackCapacity;
    int32 WeaponSlotsUnlocked = SlotCount;
    FOpenWillowAmmoPool AmmoPools[AmmoTypeCount];
    int32 Money = 0;
    int32 Eridium = 0;
    bool bMoneyKnown = false;
    bool bEridiumKnown = false;
public:
    UOpenWillowInventory();
};
