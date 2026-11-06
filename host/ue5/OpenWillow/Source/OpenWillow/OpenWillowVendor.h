// AI-assisted (Claude). Vending machines: the stock rules, the placed machine actor and the world subsystem that spawns the
// machines from the ignored local/ vendors.json (tools/prepare_vendors.py, schema openwillow-vendors/1). PR 2b adds the
// Ammo Dump (ammo), Dr. Zed (health and shields) and the grenade mods. Game-derived values come only from that file at run
// time; nothing here is game data. Rules marked UNVERIFIED are host choices or stand-ins, not behaviour confirmed in the
// running game.
#pragma once

#include "CoreMinimal.h"
#include "GameFramework/Actor.h"
#include "Subsystems/WorldSubsystem.h"
#include "Engine/TimerHandle.h"
#include "OpenWillowInventory.h"
#include "OpenWillowVendor.generated.h"

class AOpenWillowWalker;
class AOpenWillowVendingMachine;
class FOpenWillowVendorPage;
class APlayerController;
class FJsonObject;
class FOpenWillowMayaState;
struct FOpenWillowCurrencyMachine;

// The monetary value terms of a non-gun leaf (the manifest's item.value block): value at a level is
// trunc(PartTotal * Scaler^level * BaseCost), evaluated in single precision (VENDORS.md section 6).
struct FOpenWillowValueTerms
{
    bool bSet = false;            // false: no price data, so the leaf is never stocked
    float BaseCost = 0;           // the calculator's base cost constant (GD_Economy.BaseCost.Att_BaseCost_*)
    float Scaler = 1;             // GD_Economy.Universal.Att_UniversalPriceIncreasePerLevelScaler (1.12)
    float PartTotal = 1;          // the first listed part total (UNVERIFIED pick when a grade lists several)
};

// Leaf balance of a machine's pool: the weight is the pool's probability times the item's weight. Non-gun leaves also carry
// the manifest's item block (kind, ammo type, heal, value terms); a leaf with no usable kind is kept but never stocked.
struct FOpenWillowVendorLeaf
{
    FString Balance;                  // game object path (or the host's stand-in name)
    FString Class;                    // "WeaponBalanceDefinition", "InventoryBalanceDefinition", ...
    float Weight = 0;
    FString Kind;                     // item.kind: ammo, health, shield, grenade_mod, usable; empty for weapons
    FString Name;                     // item.name, the definition's base name (may be empty)
    FString AmmoType;                 // ammo leaves: pistol, smg, ar, shotgun, sniper, launcher or grenade
    int32 Amount = 0;                 // ammo leaves: rounds one purchase adds
    float HealFraction = 0;           // health leaves: fraction of max health one purchase heals
    FOpenWillowValueTerms Value;
};

// One item on sale, or in the buy-back list. Kind: weapon, ammo, health, shield or grenade_mod.
struct FOpenWillowVendorItem
{
    FString Id;                       // "stock_<n>", "featured", or "buyback_<n>"
    FString Kind = TEXT("weapon");
    FString Name;
    int32 Level = 1;
    int32 Rarity = 1;
    int32 SaleValue = 0;              // the item's value in dollars (0 when unknown); the sell price and the buy-back cost
    bool bFeatured = false;
    bool bHasWeapon = false;
    FOpenWillowWeaponItem Weapon;     // the gun that enters the backpack on purchase
    bool bHasGear = false;            // a shield or grenade mod: enters the backpack as gear
    FOpenWillowGearItem Gear;
    FString AmmoType;                 // ammo rows: the pool key (pistol, smg, ...)
    int32 Amount = 0;                 // ammo rows: rounds one purchase adds
    float HealFraction = 0;           // health rows: fraction of max health one purchase heals
    FOpenWillowValueTerms Terms;      // non-gun rows: the value terms the price comes from (the state shows them to the checker)
};

// The stock of one machine: what it sells now, its buy-back list and the reset clock. Plain data with the rules;
// no UObject, so the automation test can drive it with synthetic items.
struct FOpenWillowVendorStock
{
    // Seen live (docs/verification/VENDORS.md section 2): a weapon machine shows 7 guns plus the featured one. How the
    // game rolls them is native code not read; the host's roll is UNVERIFIED.
    static constexpr int32 WeaponShelfSize = 7;
    // Live (VENDORS.md section 6): the Ammo Dump shows 10 to 12 rows (one per ammo type, then grenade mods); the Dr. Zed
    // health machine shows 8 (one health row, then shields). The sizes are UNVERIFIED host choices for the rolled part.
    static constexpr int32 ItemsShelfSize = 11;
    static constexpr int32 HealthShelfSize = 8;
    // The live values of GD_Economy.VendingMachine.Att_SellExponentializer and Att_BaseVendingMarkupIncrease (read
    // 2026-11-03; they have no value in the packages). See BuyPrice.
    static constexpr float SellExponentializer = 0.14f;
    static constexpr float BaseMarkupIncrease = 4.1f;
    static constexpr int32 BuybackCapacity = 10;          // UNVERIFIED host choice: newest sales kept for buy-back
    static constexpr int32 DefaultMinutesBetweenResets = 20; // GlobalsDefinition.MinutesBetweenShopResets class default

    FString Kind = TEXT("weapons");   // the machine kind from the manifest: weapons, items, health, blackmarket
    int32 Stage = 1;                  // the item level the pools were evaluated at (manifest "stage")
    int32 MinutesBetweenResets = DefaultMinutesBetweenResets;
    int32 FixedItemCost = 0;          // manifest fixed_item_cost; 0 = use the markup
    int32 FixedFeaturedCost = 0;      // manifest fixed_featured_cost; 0 = use the markup
    uint32 Seed = 1;                  // base seed; each restock moves it on
    TArray<FOpenWillowVendorLeaf> Leaves;          // the machine's inventory pools, flattened
    TArray<FOpenWillowVendorLeaf> FeaturedLeaves;  // the featured pools, flattened

    TArray<FOpenWillowVendorItem> Items;           // the shelf, in shelf order
    FOpenWillowVendorItem Featured;                // the item of the day (valid when bHasFeatured)
    bool bHasFeatured = false;
    TArray<FOpenWillowVendorItem> Buyback;         // sold items, oldest first (not saved: UNVERIFIED whether the game keeps it)
    FString Notice;                                // why the shelf is empty or short, for the page message line
    // What was bought since the last restock: the shelf ids and whether the featured item went. Saved in the currency file.
    // Ammo and health rows stay on the shelf after a purchase, so they are never listed here.
    TArray<FString> BoughtSinceRestock;
    bool bFeaturedBoughtSinceRestock = false;

    bool bStocked = false;
    double StockedAt = 0;             // world seconds of the last restock
    int32 RestockCount = 0;
    int32 BuybackSerial = 0;
    FString LastBoughtId;             // backpack (stable) id of the last gun bought, for ow.Vendor.SellLast

    // Reads one machine object of the manifest (fixed costs, pools). Stage and the reset interval come from the
    // manifest's top level. False with OutError when the object is unusable.
    bool Load(const TSharedPtr<FJsonObject>& Machine, int32 InStage, int32 InMinutesBetweenResets, FString& OutError);

    // Shelf item from a rolled or hand-made gun. Sell value comes from the card's SaleValue (0 when absent).
    static FOpenWillowVendorItem MakeItem(const FOpenWillowWeaponItem& Gun, const FString& Id, bool bFeatured);
    // Shelf-size of this machine's kind (0 for a kind that is not stocked).
    int32 ShelfSize() const;

    // What buying costs: the fixed cost when the manifest sets one, else BuyPrice(SaleValue, Level). Unset when there is
    // no valid price; Buy refuses such an item.
    TOptional<int32> PriceOf(const FOpenWillowVendorItem& Item) const;
    // The shop's price for an item of this value and level (confirmed live, see the .cpp).
    // Unset when the single-precision result is not a finite int32.
    static TOptional<int32> BuyPrice(int32 Value, int32 Level);
    // The monetary value of a non-gun leaf at an item level (VENDORS.md section 6, reproduced 63/63 live). Unset when the
    // result is not a finite, non-negative int32.
    static TOptional<int32> ItemValue(const FOpenWillowValueTerms& Terms, int32 Level);
    // A non-gun leaf the shelf may stock: a shield, grenade mod, ammo or health leaf with price data.
    static bool IsStockableLeaf(const FOpenWillowVendorLeaf& Leaf);
    // What selling pays and what buying back costs: the card value (UNVERIFIED for shops).
    static int32 SellPriceOf(const FOpenWillowVendorItem& Item) { return Item.SaleValue; }

    // Stock resets once MinutesBetweenResets have passed since the last restock (world seconds).
    bool NeedsRestock(double Now) const;
    double SecondsToReset(double Now) const;
    // Rerolls the shelf and the featured item. A kind with no stock rule gets an empty shelf and a Notice.
    void Restock(double Now);
    // Restocks when due. True when it did.
    bool EnsureStocked(double Now) { if (!NeedsRestock(Now)) return false; Restock(Now); return true; }

    // The actions. Each returns false with OutError set and changes nothing on refusal. Maya is the walker's health state
    // (health rows only; null when there is no walker); Now is the world time for the health refill.
    // The actions take the id by value: a caller may pass a shelf item's own Id, and the shelf changes during the action.
    bool Buy(FString Id, UOpenWillowInventory* Inventory, FOpenWillowMayaState* Maya, double Now, FString& OutError);
    // IsProtected: items the caller will not let go (a weapon the quest lends); refused with "Mission items cannot be sold".
    bool Sell(FString Id, UOpenWillowInventory* Inventory, FString& OutError,
        const TFunction<bool(const FOpenWillowWeaponItem&)>& IsProtected = nullptr);
    bool BuyBack(FString Id, UOpenWillowInventory* Inventory, FString& OutError);

    // Puts a saved machine back (currency file): rebuilds the shelf for the saved restock count from the seed, takes the
    // bought items off again and starts the reset clock at the saved seconds. Needs Load and Seed first. False with
    // OutError when the saved clock is out of range; the stock is then left as it was.
    bool Restore(const FOpenWillowCurrencyMachine& Saved, double Now, FString& OutError);
    // The checks of Restore without changing anything (ranges and shelf ids), for validating a whole save first.
    bool CanRestore(const FOpenWillowCurrencyMachine& Saved, FString& OutError) const;

    const FOpenWillowVendorItem* FindShopItem(const FString& Id) const;

private:
    // Rolls one gun from the weapon balances of Pool, picked by weight. False when there is none or it does not roll.
    bool RollWeapon(const TArray<FOpenWillowVendorLeaf>& Pool, uint32 GunSeed, FOpenWillowWeaponItem& OutGun) const;
    // The shelf fillers, one per kind. Each reads the restock's seed and appends rows to Items.
    void FillWeaponShelf(uint32 RestockSeed);
    void FillItemsShelf(uint32 RestockSeed);
    void FillHealthShelf(uint32 RestockSeed);
    // Appends one row per leaf of the given kind, then rolled rows (by weight) until the shelf is full.
    void FillRolledRows(const TCHAR* LeafKind, uint32 RestockSeed);
    void FillFeatured(uint32 RestockSeed);
    // Appends the leaf as the next shelf row (false when it cannot be stocked).
    bool AddLeafRow(const FOpenWillowVendorLeaf& Leaf);
    // A shelf row for a non-gun leaf. Shields and grenade mods also get the gear they become when bought.
    bool MakeLeafItem(const FOpenWillowVendorLeaf& Leaf, const FString& Id, bool bFeatured, FOpenWillowVendorItem& OutItem) const;
    // The backpack id a shelf gear row gets: unique per machine, restock and shelf position.
    FString GearIdFor(const FString& ShelfId) const;
    // The purchase effects, one per kind. Each returns false with OutError and changes nothing on refusal.
    bool ApplyBuy(const FOpenWillowVendorItem& Item, UOpenWillowInventory* Inventory, FOpenWillowMayaState* Maya, double Now,
        FString& OutError);
    // Takes a bought row off the shelf (ammo and health rows stay; a featured item is taken off the day's slot).
    void RemoveBoughtRow(const FOpenWillowVendorItem& Item, const FString& Id);
};

// The placed machine. The body is the engine's cube stand-in (UNVERIFIED layout; the real skeletal mesh is not
// imported yet), with the title as a text render.
UCLASS()
class OPENWILLOW_API AOpenWillowVendingMachine : public AActor
{
    GENERATED_BODY()
public:
    AOpenWillowVendingMachine();
    void Configure(const FString& InId, const FString& InTitle, const FString& InKind);

    FString Id;
    FString Title;
    FString Kind;
    FOpenWillowVendorStock Stock;

private:
    UPROPERTY() TObjectPtr<class UStaticMeshComponent> Body;
    UPROPERTY() TObjectPtr<class UTextRenderComponent> Label;
};

// Spawns the machines of -owvendors=<vendors.json> when the world begins, and runs the shop: nearest-machine use,
// the page bridge and the ow.Vendor.* console commands. Without the switch the subsystem does nothing.
UCLASS()
class OPENWILLOW_API UOpenWillowVendorSubsystem : public UWorldSubsystem
{
    GENERATED_BODY()
public:
    virtual void OnWorldBeginPlay(UWorld& InWorld) override;
    virtual void Deinitialize() override;

    // Called from AOpenWillowWalker::PickupNearby. True when the press was used (a machine opened, or the shop is
    // already open); false lets the pickup go on.
    static bool TryUseNearest(AOpenWillowWalker& Walker);
    // The reach test of TryUseNearest on its own: the squared distance to the nearest machine in reach (0 while the shop
    // is shown, so a press closes it). The walker compares it with its other use targets.
    static bool NearestInReach(const AOpenWillowWalker& Walker, double& OutDistanceSquared);

    // Page bridge: one console line "OWVEND {json}" from the page.
    void HandlePageMessage(const FString& Line);

    // Console commands (ow.Vendor.*).
    bool OpenByIndex(int32 Index, FString& OutError);
    bool BuyByIndex(int32 ShopIndex, FString& OutError);
    // Runs buy, sell or buyback on the open shop and pushes the new state. Action names as the page sends them.
    bool Command(const FString& Action, FString Id, FString& OutError);
    void RestockOpen();
    // Sells the gun the open shop sold last (ow.Vendor.SellLast; the in-engine suite cannot know the id in advance).
    bool SellLast(FString& OutError);
    FString ReportJson() const;
    // True while a shop page is shown (the status pages do not open over it).
    bool IsShopOpen() const { return ActiveMachine.IsValid(); }

private:
    bool OpenAt(AOpenWillowWalker& Walker, AOpenWillowVendingMachine& Machine);
    bool UseNearest(AOpenWillowWalker& Walker);
    void CloseShop();
    void PushState();
    // The one-second push while the shop is open. It keeps the serial unless the stock changed, so a press that crosses it is
    // not refused as stale.
    void PushTimerState();
    FString StateJson() const;
    // The walker the shop is open for (null when none); its health state is what a health purchase changes.
    AOpenWillowWalker* ActiveWalker() const;

    TArray<TWeakObjectPtr<AOpenWillowVendingMachine>> Machines;
    TWeakObjectPtr<AOpenWillowVendingMachine> ActiveMachine;
    TWeakObjectPtr<UOpenWillowInventory> ActiveInventory;
    TWeakObjectPtr<APlayerController> ActiveController;
    TSharedPtr<FOpenWillowVendorPage> Page;
    // -owvendortest: a scripted open / buy / sell / restock sequence with reports and a screenshot, then quit
    // (tools/test_vendors.ps1). Driven by a world timer so it starts only once the walker exists.
    void TickVendorTest();
    // The index of the first spawned machine of this kind, or INDEX_NONE.
    int32 FirstMachineIndexOfKind(const TCHAR* Kind) const;
    // True while the quest lends this weapon (it must not be sold or lost to buy-back).
    bool IsQuestOwned(const FOpenWillowWeaponItem& Item) const;

    // Currency save (-owcurrencysave=<file>, OpenWillowCurrency.h). Loaded once the walker has begun play, so the saved
    // purse replaces the start purse (not when -owmoney / -owerid are on the command line). Saves write the file atomically.
    void TickCurrencyLoad();
    void LoadCurrency(AOpenWillowWalker& Walker);
    void SaveCurrency();
    FString CurrencyPath;             // the resolved save path; empty = the feature is off
    FString ManifestId;               // identity of the spawned machine set (see OnWorldBeginPlay)
    TWeakObjectPtr<UOpenWillowInventory> CurrencyInventory;
    bool bCurrencyReady = false;      // loaded (or there was no file) with the walker found: saves are allowed
    bool bCurrencyRefused = false;    // the file was unreadable or malformed: saves stay off so it is not overwritten
    int32 CurrencyWaits = 0;
    FTimerHandle CurrencyTimer;
    FTimerHandle OpenTimer;           // pushes the state every second while the shop is open
    FTimerHandle TestTimer;
    int32 TestStep = 0;
    int32 TestWaits = 0;
    int64 Serial = 0;                 // counts the states pushed to the page
    int64 Opening = 0;                // counts the shop's openings; the page starts its selection over on a new one
    FString LastMessage;             // the last action's refusal text, empty when it went through
};
