// AI-assisted (Claude). Vending machines (PR 1): the stock rules, the placed machine actor and the world subsystem that
// spawns the machines from the ignored local/ vendors.json (tools/prepare_vendors.py, schema openwillow-vendors/1).
// Game-derived values come only from that file at run time; nothing here is game data. Rules marked UNVERIFIED are
// host choices or stand-ins, not behaviour confirmed in the running game.
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

// One item on sale, or in the buy-back list. Kind is "weapon" for PR 1; ammo and health machines come later.
struct FOpenWillowVendorItem
{
    FString Id;                       // "stock_<n>", "featured", or "buyback_<n>"
    FString Kind = TEXT("weapon");
    FString Name;
    int32 Level = 1;
    int32 Rarity = 1;
    int32 SaleValue = 0;              // the item's card value in dollars (0 when the recipe did not carry one)
    bool bFeatured = false;
    bool bHasWeapon = false;
    FOpenWillowWeaponItem Weapon;     // the gun that enters the backpack on purchase
};

// Leaf balance of a machine's pool: the weight is the pool's probability times the item's weight.
struct FOpenWillowVendorLeaf
{
    FString Balance;                  // game object path (or the host's stand-in name)
    FString Class;                    // "WeaponBalanceDefinition", "InventoryBalanceDefinition", ...
    float Weight = 0;
};

// The stock of one machine: what it sells now, its buy-back list and the reset clock. Plain data with the rules;
// no UObject, so the automation test can drive it with synthetic items.
struct FOpenWillowVendorStock
{
    // Seen live (docs/verification/VENDORS.md section 2): a weapon machine shows 7 guns plus the featured one. How the
    // game rolls them is native code not read; the host's roll is UNVERIFIED.
    static constexpr int32 StockWeaponCount = 7;
    // The live values of GD_Economy.VendingMachine.Att_SellExponentializer and Att_BaseVendingMarkupIncrease (read
    // 2026-11-03; they have no value in the packages). See BuyPrice.
    static constexpr float SellExponentializer = 0.14f;
    static constexpr float BaseMarkupIncrease = 4.1f;
    static constexpr int32 BuybackCapacity = 10;          // UNVERIFIED host choice: newest sales kept for buy-back
    static constexpr int32 DefaultMinutesBetweenResets = 20; // GlobalsDefinition.MinutesBetweenShopResets class default

    FString Kind = TEXT("weapons");   // the machine kind from the manifest: weapons, items, health
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
    TArray<FOpenWillowVendorItem> Buyback;         // sold items, oldest first
    FString Notice;                                // why the shelf is empty or short, for the page message line

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

    // What buying costs: the fixed cost when the manifest sets one, else BuyPrice(SaleValue, Level).
    int32 PriceOf(const FOpenWillowVendorItem& Item) const;
    // The shop's price for an item of this value and level (confirmed live, see the .cpp).
    static int32 BuyPrice(int32 Value, int32 Level);
    // What selling pays and what buying back costs: the card value (UNVERIFIED for shops).
    static int32 SellPriceOf(const FOpenWillowVendorItem& Item) { return Item.SaleValue; }

    // Stock resets once MinutesBetweenResets have passed since the last restock (world seconds).
    bool NeedsRestock(double Now) const;
    double SecondsToReset(double Now) const;
    // Rerolls the shelf and the featured item. Weapon machines only; other kinds get an empty shelf and a Notice.
    void Restock(double Now);
    // Restocks when due. True when it did.
    bool EnsureStocked(double Now) { if (!NeedsRestock(Now)) return false; Restock(Now); return true; }

    // The actions. Each returns false with OutError set and changes nothing on refusal.
    bool Buy(const FString& Id, UOpenWillowInventory* Inventory, FString& OutError);
    bool Sell(const FString& Id, UOpenWillowInventory* Inventory, FString& OutError);
    bool BuyBack(const FString& Id, UOpenWillowInventory* Inventory, FString& OutError);

    const FOpenWillowVendorItem* FindShopItem(const FString& Id) const;

private:
    // Rolls one gun from the weapon balances of Pool, picked by weight. False when there is none or it does not roll.
    bool RollWeapon(const TArray<FOpenWillowVendorLeaf>& Pool, uint32 GunSeed, FOpenWillowWeaponItem& OutGun) const;
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

    // Page bridge: one console line "OWVEND {json}" from the page.
    void HandlePageMessage(const FString& Line);

    // Console commands (ow.Vendor.*).
    bool OpenByIndex(int32 Index, FString& OutError);
    bool BuyByIndex(int32 ShopIndex, FString& OutError);
    // Runs buy, sell or buyback on the open shop and pushes the new state. Action names as the page sends them.
    bool Command(const FString& Action, const FString& Id, FString& OutError);
    void RestockOpen();
    // Sells the gun the open shop sold last (ow.Vendor.SellLast; the in-engine suite cannot know the id in advance).
    bool SellLast(FString& OutError);
    FString ReportJson() const;

private:
    bool OpenAt(AOpenWillowWalker& Walker, AOpenWillowVendingMachine& Machine);
    bool UseNearest(AOpenWillowWalker& Walker);
    void CloseShop();
    void PushState();
    FString StateJson() const;

    TArray<TWeakObjectPtr<AOpenWillowVendingMachine>> Machines;
    TWeakObjectPtr<AOpenWillowVendingMachine> ActiveMachine;
    TWeakObjectPtr<UOpenWillowInventory> ActiveInventory;
    TWeakObjectPtr<APlayerController> ActiveController;
    TSharedPtr<FOpenWillowVendorPage> Page;
    // -owvendortest: a scripted open / buy / sell / restock sequence with reports and a screenshot, then quit
    // (tools/test_vendors.ps1). Driven by a world timer so it starts only once the walker exists.
    void TickVendorTest();
    FTimerHandle TestTimer;
    int32 TestStep = 0;
    int32 TestWaits = 0;
    int64 Serial = 0;                 // counts the states pushed to the page
    FString LastMessage;             // the last action's refusal text, empty when it went through
};
