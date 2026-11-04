// AI-assisted (Claude). Synthetic checks of the vending stock rules (FOpenWillowVendorStock): prices, buy with enough and
// too little money, a full backpack, sell and its refusal for an equipped gun, buy-back, and the reset clock. Every number
// below is invented for the test; no weapon generator, manifest or game data is read. The prices use the buy formula
// confirmed live (docs/verification/VENDORS.md section 3); the sale and buy-back prices stay UNVERIFIED in game.
// The currency save (OpenWillow.Vendor.Persist): the file format round trip, refusals and the restore of a saved clock.
// Run: UnrealEditor.exe <uproject> -game -nullrhi -ExecCmds="Automation RunTests OpenWillow.Vendor; Quit"
#include "Misc/AutomationTest.h"

#if WITH_DEV_AUTOMATION_TESTS
#include "OpenWillowVendor.h"
#include "HAL/FileManager.h"
#include "Misc/FileHelper.h"
#include "Misc/Paths.h"
#include "OpenWillowCurrency.h"
#include "OpenWillowMayaState.h"

namespace
{
FOpenWillowWeaponItem SyntheticGun(const TCHAR* Id, int32 SaleValue)
{
    FOpenWillowWeaponItem Gun;
    Gun.Id = Id;
    Gun.Name = FString(TEXT("Synthetic ")) + Id;
    Gun.Rarity = 1;
    Gun.Level = 5;
    Gun.bHasSaleValue = true;
    Gun.SaleValue = SaleValue;
    return Gun;
}

UOpenWillowInventory* SyntheticInventory(int32 Money)
{
    UOpenWillowInventory* Inventory = NewObject<UOpenWillowInventory>(GetTransientPackage());
    Inventory->SetMoney(Money);
    return Inventory;
}
}

IMPLEMENT_SIMPLE_AUTOMATION_TEST(FOpenWillowVendorStockTest, "OpenWillow.Vendor.Stock",
    EAutomationTestFlags::ClientContext | EAutomationTestFlags::EngineFilter)

bool FOpenWillowVendorStockTest::RunTest(const FString& Parameters)
{
    // Two invented level-5 guns. Markup at level 5 = 5 x 0.14 + 4.1 = 4.8: 101 -> floor(484.8) = 484, 41 -> 196.
    FOpenWillowVendorStock Stock;
    Stock.Kind = TEXT("weapons");
    Stock.Items.Add(FOpenWillowVendorStock::MakeItem(SyntheticGun(TEXT("synthetic_a"), 101), TEXT("stock_0"), false));
    Stock.Items.Add(FOpenWillowVendorStock::MakeItem(SyntheticGun(TEXT("synthetic_b"), 41), TEXT("stock_1"), false));
    TestEqual(TEXT("price is floor(value x markup)"), Stock.PriceOf(Stock.Items[0]).Get(-1), 484);
    TestEqual(TEXT("second gun's price"), Stock.PriceOf(Stock.Items[1]).Get(-1), 196);
    TestEqual(TEXT("formula on its own, level 10: floor(51 x 5.5)"), FOpenWillowVendorStock::BuyPrice(51, 10).Get(-1), 280);
    TestFalse(TEXT("an absurd value has no price (not a sentinel)"), FOpenWillowVendorStock::BuyPrice(MAX_int32, 100).IsSet());
    TestEqual(TEXT("sell price is the card value"), FOpenWillowVendorStock::SellPriceOf(Stock.Items[0]), 101);

    // Too little money: refused, nothing taken, the gun stays on the shelf.
    FString Error;
    UOpenWillowInventory* Poor = SyntheticInventory(150);
    TestFalse(TEXT("buy with too little money is refused"), Stock.Buy(TEXT("stock_0"), Poor, nullptr, 0.0, Error));
    TestEqual(TEXT("refusal text"), Error, FString(TEXT("Insufficient funds")));
    int32 Money = 0;
    Poor->GetMoney(Money);
    TestEqual(TEXT("money unchanged after a refused buy"), Money, 150);
    TestEqual(TEXT("shelf unchanged after a refused buy"), Stock.Items.Num(), 2);
    TestFalse(TEXT("unknown id is refused"), Stock.Buy(TEXT("stock_9"), Poor, nullptr, 0.0, Error));
    TestEqual(TEXT("unknown id text"), Error, FString(TEXT("Unknown item")));

    // A shelf item with no valid price is refused even with the largest purse.
    {
        FOpenWillowVendorStock Odd;
        FOpenWillowWeaponItem Huge = SyntheticGun(TEXT("synthetic_huge"), MAX_int32);
        Huge.Level = 100;
        Odd.Items.Add(FOpenWillowVendorStock::MakeItem(Huge, TEXT("stock_0"), false));
        UOpenWillowInventory* Rich = SyntheticInventory(MAX_int32);
        TestFalse(TEXT("item without a valid price is refused"), Odd.Buy(TEXT("stock_0"), Rich, nullptr, 0.0, Error));
        TestEqual(TEXT("invalid price text"), Error, FString(TEXT("No valid price")));
    }

    // The id may be passed straight from the shelf (as ow.Vendor.Buy does): the gun bought is that one, not the next.
    {
        FOpenWillowVendorStock Pair;
        Pair.Items.Add(FOpenWillowVendorStock::MakeItem(SyntheticGun(TEXT("synthetic_p"), 101), TEXT("stock_0"), false));
        Pair.Items.Add(FOpenWillowVendorStock::MakeItem(SyntheticGun(TEXT("synthetic_q"), 41), TEXT("stock_1"), false));
        UOpenWillowInventory* Buyer = SyntheticInventory(1000);
        TestTrue(TEXT("buy by the shelf's own id"), Pair.Buy(Pair.Items[0].Id, Buyer, nullptr, 0.0, Error));
        TestEqual(TEXT("one gun left"), Pair.Items.Num(), 1);
        TestEqual(TEXT("the other gun stays"), Pair.Items.Num() == 1 ? Pair.Items[0].Id : FString(), FString(TEXT("stock_1")));
        int32 Left = 0;
        Buyer->GetMoney(Left);
        TestEqual(TEXT("charged the first gun's price"), Left, 1000 - 484);
    }

    // Enough money: buy gun A (484) then gun B (196) from 1000.
    UOpenWillowInventory* Inventory = SyntheticInventory(1000);
    TestTrue(TEXT("buy with enough money"), Stock.Buy(TEXT("stock_0"), Inventory, nullptr, 0.0, Error));
    Inventory->GetMoney(Money);
    TestEqual(TEXT("money after buying A"), Money, 516);
    TestEqual(TEXT("A is in the backpack"), Inventory->Items().Num(), 1);
    TestEqual(TEXT("A left the shelf"), Stock.Items.Num(), 1);
    TestTrue(TEXT("buy B"), Stock.Buy(TEXT("stock_1"), Inventory, nullptr, 0.0, Error));
    Inventory->GetMoney(Money);
    TestEqual(TEXT("money after buying B"), Money, 320);

    // Equipped guns cannot be sold; the unequipped one can.
    TestTrue(TEXT("equip A"), Inventory->Equip(0, 0));
    TestFalse(TEXT("equipped gun is not sold"), Stock.Sell(TEXT("synthetic_a"), Inventory, Error));
    TestEqual(TEXT("equipped refusal text"), Error, FString(TEXT("Equipped items cannot be sold")));
    // A protected item (the quest's lent weapon in the game) is refused and stays in the backpack.
    TestFalse(TEXT("protected gun is not sold"), Stock.Sell(TEXT("synthetic_b"), Inventory, Error,
        [](const FOpenWillowWeaponItem& Item) { return Item.Id == TEXT("synthetic_b"); }));
    TestEqual(TEXT("protected refusal text"), Error, FString(TEXT("Mission items cannot be sold")));
    TestEqual(TEXT("protected gun still owned"), Inventory->Items().Num(), 2);
    TestTrue(TEXT("sell B"), Stock.Sell(TEXT("synthetic_b"), Inventory, Error));
    Inventory->GetMoney(Money);
    TestEqual(TEXT("money after selling B (card value 41)"), Money, 361);
    TestEqual(TEXT("B is in the buy-back list"), Stock.Buyback.Num(), 1);
    TestEqual(TEXT("A is still in the backpack"), Inventory->Items().Num(), 1);

    // Buy-back costs the sell price (41) and returns the gun.
    const FString BuybackId = Stock.Buyback[0].Id;
    TestTrue(TEXT("buy back B"), Stock.BuyBack(BuybackId, Inventory, Error));
    Inventory->GetMoney(Money);
    TestEqual(TEXT("money after buy-back"), Money, 320);
    TestEqual(TEXT("buy-back list empty after buying back"), Stock.Buyback.Num(), 0);
    TestEqual(TEXT("B is back in the backpack"), Inventory->Items().Num(), 2);

    // Full backpack: 12 is the default capacity. Equipped A does not count, so 11 more fill it.
    for (int32 I = 0; I < 11; ++I)
    {
        FOpenWillowWeaponItem Filler = SyntheticGun(TEXT("synthetic_fill"), 1);
        Filler.InstanceId = FString::Printf(TEXT("synthetic_fill_%d"), I);
        TestTrue(TEXT("filler added"), Inventory->AddToBackpack(Filler));
    }
    TestEqual(TEXT("backpack count is at capacity"), Inventory->BackpackCount(), Inventory->GetBackpackCapacity());
    Stock.Items.Add(FOpenWillowVendorStock::MakeItem(SyntheticGun(TEXT("synthetic_c"), 10), TEXT("stock_2"), false));
    TestFalse(TEXT("buy into a full backpack is refused"), Stock.Buy(TEXT("stock_2"), Inventory, nullptr, 0.0, Error));
    TestEqual(TEXT("full backpack text"), Error, FString(TEXT("Backpack full")));
    Inventory->GetMoney(Money);
    TestEqual(TEXT("money unchanged after a refused buy into a full backpack"), Money, 320);

    // Featured item uses its own fixed cost when the manifest sets one.
    Stock.FixedFeaturedCost = 50;
    FOpenWillowVendorItem Featured = FOpenWillowVendorStock::MakeItem(SyntheticGun(TEXT("synthetic_d"), 100), TEXT("featured"), true);
    TestEqual(TEXT("fixed featured cost wins over the markup"), Stock.PriceOf(Featured).Get(-1), 50);
    Stock.FixedItemCost = 70;
    TestEqual(TEXT("fixed item cost wins over the markup"), Stock.PriceOf(Stock.Items[0]).Get(-1), 70);

    // Reset clock: 20 minutes by default, counted from the last restock. Restock with no weapon balances rolls nothing
    // (no generator call), so this part is synthetic too.
    FOpenWillowVendorStock Shelf;
    Shelf.Kind = TEXT("weapons");
    TestTrue(TEXT("a new shelf needs a restock"), Shelf.NeedsRestock(1000.0));
    Shelf.Restock(1000.0);
    TestFalse(TEXT("no reset 19 minutes after the restock"), Shelf.NeedsRestock(1000.0 + 19 * 60.0));
    TestTrue(TEXT("reset 20 minutes after the restock"), Shelf.NeedsRestock(1000.0 + 20 * 60.0));
    TestEqual(TEXT("seconds to reset one minute in"), Shelf.SecondsToReset(1060.0), 1140.0);
    TestEqual(TEXT("empty weapon list says so"), Shelf.Notice, FString(TEXT("No weapons listed")));

    return true;
}

namespace
{
// Invented non-gun leaf: its kind, a weight, and value terms (base cost, part total; scaler 1.12 as in the game).
FOpenWillowVendorLeaf SyntheticLeaf(const TCHAR* Balance, const TCHAR* Kind, float Weight, float BaseCost, float PartTotal)
{
    FOpenWillowVendorLeaf Leaf;
    Leaf.Balance = Balance;
    Leaf.Class = TEXT("InventoryBalanceDefinition");
    Leaf.Weight = Weight;
    Leaf.Kind = Kind;
    Leaf.Name = FString(TEXT("Synthetic ")) + Kind;
    Leaf.Value.bSet = true;
    Leaf.Value.BaseCost = BaseCost;
    Leaf.Value.Scaler = 1.12f;
    Leaf.Value.PartTotal = PartTotal;
    return Leaf;
}

FOpenWillowVendorLeaf SyntheticAmmoLeaf(const TCHAR* AmmoType, int32 Amount)
{
    FOpenWillowVendorLeaf Leaf = SyntheticLeaf(*FString::Printf(TEXT("AmmoSynthetic_%s"), AmmoType), TEXT("ammo"), 1.f, 50.f, 1.f);
    Leaf.AmmoType = AmmoType;
    Leaf.Amount = Amount;
    return Leaf;
}
}

IMPLEMENT_SIMPLE_AUTOMATION_TEST(FOpenWillowVendorItemsTest, "OpenWillow.Vendor.Items",
    EAutomationTestFlags::ClientContext | EAutomationTestFlags::EngineFilter)

bool FOpenWillowVendorItemsTest::RunTest(const FString& Parameters)
{
    // Value formula on invented terms, level 5, scaler 1.12: 50 x 1.12^5 = 88.1 -> 88; 0.96 x 28 x 1.12^5 = 47.4 -> 47.
    FOpenWillowValueTerms Terms;
    Terms.bSet = true;
    Terms.BaseCost = 50.f;
    Terms.Scaler = 1.12f;
    Terms.PartTotal = 1.f;
    TestEqual(TEXT("value of 50 at level 5"), FOpenWillowVendorStock::ItemValue(Terms, 5).Get(-1), 88);
    TestEqual(TEXT("value at level 0 is the base cost"), FOpenWillowVendorStock::ItemValue(Terms, 0).Get(-1), 50);
    FOpenWillowValueTerms NoPrice;
    TestFalse(TEXT("terms without price data give no value"), FOpenWillowVendorStock::ItemValue(NoPrice, 5).IsSet());

    // The Ammo Dump's shape: one row per ammo type, then grenade mods up to 11 rows. Price = floor(88 x 4.8) = 422 at level 5.
    FOpenWillowVendorStock Dump;
    Dump.Kind = TEXT("items");
    Dump.Stage = 5;
    Dump.Seed = 11;
    Dump.Leaves.Add(SyntheticAmmoLeaf(TEXT("pistol"), 54));
    Dump.Leaves.Add(SyntheticLeaf(TEXT("GM_Synthetic"), TEXT("grenade_mod"), 1.f, 25.f, 1.f));
    Dump.FeaturedLeaves.Add(SyntheticLeaf(TEXT("GM_Featured"), TEXT("grenade_mod"), 1.f, 25.f, 1.f));
    Dump.Restock(0.0);
    TestEqual(TEXT("items shelf holds 11 rows"), Dump.Items.Num(), FOpenWillowVendorStock::ItemsShelfSize);
    TestEqual(TEXT("the first row is the ammo type"), Dump.Items[0].Kind, FString(TEXT("ammo")));
    TestEqual(TEXT("ammo row value 88"), Dump.Items[0].SaleValue, 88);
    TestEqual(TEXT("ammo row price 422"), Dump.PriceOf(Dump.Items[0]).Get(-1), 422);
    TestEqual(TEXT("the grenade mod rows follow"), Dump.Items[1].Kind, FString(TEXT("grenade_mod")));
    TestTrue(TEXT("a grenade mod row carries its gear"), Dump.Items[1].bHasGear);
    TestTrue(TEXT("gear rows get distinct backpack ids"), Dump.Items[1].Gear.Id != Dump.Items[2].Gear.Id);
    TestTrue(TEXT("the featured grenade mod is stocked"), Dump.bHasFeatured);

    // Buying ammo: the pool gets the rounds up to its maximum, nothing else changes, and the row stays on the shelf.
    UOpenWillowInventory* Inventory = SyntheticInventory(1000);
    FString Error;
    Inventory->SetAmmoCurrent(EOpenWillowAmmoType::Pistol, 0);
    const int32 PistolMax = Inventory->AmmoPool(EOpenWillowAmmoType::Pistol).Max;
    TestTrue(TEXT("buy pistol ammo"), Dump.Buy(TEXT("stock_0"), Inventory, nullptr, 0.0, Error));
    TestEqual(TEXT("pool gets the rounds, capped at the maximum"), Inventory->AmmoPool(EOpenWillowAmmoType::Pistol).Current,
        FMath::Min(PistolMax, 54));
    int32 Money = 0;
    Inventory->GetMoney(Money);
    TestEqual(TEXT("ammo is charged its price"), Money, 578);
    TestEqual(TEXT("ammo row stays on the shelf"), Dump.Items.Num(), FOpenWillowVendorStock::ItemsShelfSize);
    Inventory->SetAmmoCurrent(EOpenWillowAmmoType::Pistol, PistolMax);
    TestFalse(TEXT("a full pool refuses ammo"), Dump.Buy(TEXT("stock_0"), Inventory, nullptr, 0.0, Error));
    TestEqual(TEXT("ammo full text"), Error, FString(TEXT("Ammo full")));
    Inventory->GetMoney(Money);
    TestEqual(TEXT("nothing is charged for refused ammo"), Money, 578);

    // The health machine: one health row (heals 25% of max), then shields. Health 100 of 400 heals to 200 for 249.
    FOpenWillowVendorStock Dr;
    Dr.Kind = TEXT("health");
    Dr.Stage = 5;
    Dr.Seed = 12;
    FOpenWillowVendorLeaf HealLeaf = SyntheticLeaf(TEXT("HealSynthetic"), TEXT("health"), 1.f, 30.f, 1.f);
    HealLeaf.HealFraction = 0.25f;
    Dr.Leaves.Add(HealLeaf);
    Dr.Leaves.Add(SyntheticLeaf(TEXT("ShieldSynthetic"), TEXT("shield"), 1.f, 28.f, 0.96f));
    Dr.Restock(0.0);
    TestEqual(TEXT("health shelf holds 8 rows"), Dr.Items.Num(), FOpenWillowVendorStock::HealthShelfSize);
    TestEqual(TEXT("the first row is the health item"), Dr.Items[0].Kind, FString(TEXT("health")));
    TestEqual(TEXT("health row value 52"), Dr.Items[0].SaleValue, 52);
    TestEqual(TEXT("health row price 249"), Dr.PriceOf(Dr.Items[0]).Get(-1), 249);
    TestEqual(TEXT("shield rows follow"), Dr.Items[1].Kind, FString(TEXT("shield")));
    TestEqual(TEXT("shield value 47 (0.96 x 28 x 1.12^5)"), Dr.Items[1].SaleValue, 47);

    FOpenWillowMayaState Maya;
    Maya.Health = 100.f;
    Inventory->SetMoney(1000);
    TestTrue(TEXT("buy a heal"), Dr.Buy(TEXT("stock_0"), Inventory, &Maya, 0.0, Error));
    TestTrue(TEXT("heal adds a quarter of max health"), FMath::IsNearlyEqual(Maya.Health, 200.f));
    Inventory->GetMoney(Money);
    TestEqual(TEXT("heal is charged its price"), Money, 751);
    TestEqual(TEXT("health row stays on the shelf"), Dr.Items.Num(), FOpenWillowVendorStock::HealthShelfSize);
    Maya.Health = Maya.Effective().MaxHealth;
    TestFalse(TEXT("full health refuses a heal"), Dr.Buy(TEXT("stock_0"), Inventory, &Maya, 0.0, Error));
    TestEqual(TEXT("health full text"), Error, FString(TEXT("Health full")));
    TestFalse(TEXT("a heal with no walker is refused"), Dr.Buy(TEXT("stock_0"), Inventory, nullptr, 0.0, Error));

    // Gear: buy a shield (225 = floor(47 x 4.8)), sell it back (47), buy it back (47). The gear keeps its backpack id.
    const FString ShieldShelfId = TEXT("stock_1");
    const FString ShieldGearId = Dr.Items[1].Gear.Id;
    TestTrue(TEXT("buy a shield"), Dr.Buy(*ShieldShelfId, Inventory, &Maya, 0.0, Error));
    Inventory->GetMoney(Money);
    TestEqual(TEXT("shield is charged its price"), Money, 526);
    TestEqual(TEXT("the shield is in the gear backpack"), Inventory->GearItemList().Num(), 1);
    TestEqual(TEXT("the bought shield leaves the shelf"), Dr.Items.Num(), FOpenWillowVendorStock::HealthShelfSize - 1);
    TestTrue(TEXT("sell the shield"), Dr.Sell(ShieldGearId, Inventory, Error));
    Inventory->GetMoney(Money);
    TestEqual(TEXT("selling pays the value"), Money, 573);
    TestEqual(TEXT("the shield is in the buy-back list"), Dr.Buyback.Num(), 1);
    TestEqual(TEXT("the shield left the backpack"), Inventory->GearItemList().Num(), 0);
    TestTrue(TEXT("buy the shield back"), Dr.BuyBack(Dr.Buyback[0].Id, Inventory, Error));
    Inventory->GetMoney(Money);
    TestEqual(TEXT("buy-back costs the sell price"), Money, 526);
    TestEqual(TEXT("the shield is back in the gear backpack"), Inventory->GearItemList().Num(), 1);
    return true;
}

namespace
{
// A currency file with the given money and one machine row; the numbers are typed in as JSON text so bad values can be tried.
// An invented 40-hex-digit manifest identity.
const TCHAR* const SyntheticManifest = TEXT("0123456789abcdef0123456789abcdef01234567");

FString SaveText(const TCHAR* Money, const TCHAR* RestockCount, const TCHAR* Seconds, const FString& BoughtList)
{
    return FString::Printf(TEXT("{\"schema\":\"openwillow-currency/2\",\"money\":%s,\"eridium\":0,\"manifest\":\"%s\",\"machines\":{\"Synthetic:Point_0\":"
        "{\"restock_count\":%s,\"seconds_to_reset\":%s,\"bought\":[%s],\"featured_bought\":false}}}"),
        Money, SyntheticManifest, RestockCount, Seconds, *BoughtList);
}

// N invented shelf ids as JSON strings, comma-separated.
FString BoughtListOf(int32 Count, int32 Length = 7)
{
    FString List;
    for (int32 I = 0; I < Count; ++I) List += FString::Printf(TEXT("%s\"%s\""), I ? TEXT(",") : TEXT(""), *FString::ChrN(Length, 'x'));
    return List;
}
}

IMPLEMENT_SIMPLE_AUTOMATION_TEST(FOpenWillowVendorPersistTest, "OpenWillow.Vendor.Persist",
    EAutomationTestFlags::ClientContext | EAutomationTestFlags::EngineFilter)

bool FOpenWillowVendorPersistTest::RunTest(const FString& Parameters)
{
    // An invented save round-trips through its JSON text.
    FOpenWillowCurrencySave Save;
    Save.Money = 1234;
    Save.Eridium = 5;
    Save.Manifest = SyntheticManifest;
    FOpenWillowCurrencyMachine Row;
    Row.RestockCount = 3;
    Row.SecondsToReset = 900;
    Row.Bought.Add(TEXT("stock_1"));
    Row.Bought.Add(TEXT("stock_4"));
    Row.bFeaturedBought = true;
    Save.Machines.Add(TEXT("Synthetic:Point_0"), Row);
    FOpenWillowCurrencySave Back;
    FString Error;
    TestTrue(TEXT("a written save reads back"), FOpenWillowCurrencySave::FromJson(Save.ToJson(), Back, Error));
    TestEqual(TEXT("money round trip"), Back.Money, 1234);
    TestEqual(TEXT("eridium round trip"), Back.Eridium, 5);
    TestEqual(TEXT("manifest round trip"), Back.Manifest, FString(SyntheticManifest));
    const FOpenWillowCurrencyMachine* Got = Back.Machines.Find(TEXT("Synthetic:Point_0"));
    TestTrue(TEXT("machine row round trip"), Got != nullptr);
    if (Got)
    {
        TestEqual(TEXT("restock count round trip"), Got->RestockCount, 3);
        TestEqual(TEXT("seconds round trip"), Got->SecondsToReset, 900);
        TestEqual(TEXT("bought ids round trip"), Got->Bought.Num(), 2);
        TestTrue(TEXT("featured flag round trip"), Got->bFeaturedBought);
    }

    // Good values read; bad ones are refused and leave the output alone.
    FOpenWillowCurrencySave Ok;
    TestTrue(TEXT("a plain save reads"), FOpenWillowCurrencySave::FromJson(SaveText(TEXT("10"), TEXT("1"), TEXT("0"), TEXT("")), Ok, Error));
    TestEqual(TEXT("minimum values read"), Ok.Money, 10);
    FOpenWillowCurrencySave Untouched;
    Untouched.Money = 77;
    TestFalse(TEXT("not JSON is refused"), FOpenWillowCurrencySave::FromJson(TEXT("{"), Untouched, Error));
    TestFalse(TEXT("another schema is refused"), FOpenWillowCurrencySave::FromJson(
        TEXT("{\"schema\":\"openwillow-currency/0\",\"money\":1,\"eridium\":0,\"machines\":{}}"), Untouched, Error));
    TestEqual(TEXT("output unchanged after a refusal"), Untouched.Money, 77);
    TestFalse(TEXT("negative money is refused"), FOpenWillowCurrencySave::FromJson(SaveText(TEXT("-1"), TEXT("1"), TEXT("0"), TEXT("")), Untouched, Error));
    TestFalse(TEXT("fractional money is refused"), FOpenWillowCurrencySave::FromJson(SaveText(TEXT("1.5"), TEXT("1"), TEXT("0"), TEXT("")), Untouched, Error));
    TestFalse(TEXT("money above int32 is refused"), FOpenWillowCurrencySave::FromJson(SaveText(TEXT("2147483648"), TEXT("1"), TEXT("0"), TEXT("")), Untouched, Error));
    TestFalse(TEXT("a version-1 save (no manifest identity) is refused"), FOpenWillowCurrencySave::FromJson(
        TEXT("{\"schema\":\"openwillow-currency/1\",\"money\":1,\"eridium\":0,\"machines\":{}}"), Untouched, Error));
    TestFalse(TEXT("a restock count above the bound is refused"), FOpenWillowCurrencySave::FromJson(SaveText(TEXT("1"), TEXT("2147483647"), TEXT("0"), TEXT("")), Untouched, Error));
    TestFalse(TEXT("restock count 0 is refused"), FOpenWillowCurrencySave::FromJson(SaveText(TEXT("1"), TEXT("0"), TEXT("0"), TEXT("")), Untouched, Error));
    TestFalse(TEXT("reset clock above a day is refused"), FOpenWillowCurrencySave::FromJson(SaveText(TEXT("1"), TEXT("1"), TEXT("86401"), TEXT("")), Untouched, Error));
    TestFalse(TEXT("too many bought ids are refused"), FOpenWillowCurrencySave::FromJson(SaveText(TEXT("1"), TEXT("1"), TEXT("0"), BoughtListOf(65)), Untouched, Error));
    TestFalse(TEXT("an over-long bought id is refused"), FOpenWillowCurrencySave::FromJson(SaveText(TEXT("1"), TEXT("1"), TEXT("0"), BoughtListOf(1, 33)), Untouched, Error));
    TestEqual(TEXT("output still unchanged"), Untouched.Money, 77);

    // Restore: a synthetic stock (no weapon balances, so the shelf is empty) keeps the saved clock. The bought id is not on
    // the rebuilt shelf, so it is dropped from the record.
    FOpenWillowVendorStock Shelf;
    Shelf.Kind = TEXT("weapons");
    Shelf.Seed = 7;
    TestTrue(TEXT("restore a saved stock"), Shelf.Restore(Row, 1000.0, Error));
    TestEqual(TEXT("restock count restored"), Shelf.RestockCount, 3);
    TestEqual(TEXT("reset clock restored"), Shelf.SecondsToReset(1000.0), 900.0);
    TestFalse(TEXT("no reset 899 s after the restore"), Shelf.NeedsRestock(1000.0 + 899.0));
    TestTrue(TEXT("reset at the saved time"), Shelf.NeedsRestock(1000.0 + 900.0));
    TestEqual(TEXT("an id not on the shelf is not kept"), Shelf.BoughtSinceRestock.Num(), 0);
    FOpenWillowCurrencyMachine TooLate = Row;
    TooLate.SecondsToReset = 1201; // above the 20-minute interval
    TestFalse(TEXT("a clock above the interval is refused"), Shelf.Restore(TooLate, 1000.0, Error));
    TestEqual(TEXT("refused restore leaves the restock count"), Shelf.RestockCount, 3);

    // A bought id that is not a shelf id is refused by the whole-file validation (CanRestore), before anything is applied.
    FOpenWillowCurrencyMachine Odd = Row;
    Odd.Bought = {TEXT("stock_99")};
    TestFalse(TEXT("a bought id off the shelf range is refused"), Shelf.CanRestore(Odd, Error));
    Odd.Bought = {TEXT("featured")};
    TestFalse(TEXT("a non-shelf bought id is refused"), Shelf.CanRestore(Odd, Error));

    // The manifest identity is stable for the same lines and differs for different ones.
    const FString IdA = FOpenWillowCurrencySave::ManifestIdentity({TEXT("stage 8"), TEXT("machine a")});
    TestEqual(TEXT("identity is 40 hex digits"), IdA.Len(), 40);
    TestEqual(TEXT("identity is stable"), FOpenWillowCurrencySave::ManifestIdentity({TEXT("stage 8"), TEXT("machine a")}), IdA);
    TestNotEqual(TEXT("identity follows the machine set"), FOpenWillowCurrencySave::ManifestIdentity({TEXT("stage 8"), TEXT("machine b")}), IdA);

    // Paths: only .json files under this checkout's local/ are accepted.
    FString Resolved;
    TestFalse(TEXT("a local/ folder elsewhere is refused"), FOpenWillowCurrencySave::ResolvePath(TEXT("C:/temp/local/x.json"), Resolved, Error));
    const FString Inside = FPaths::Combine(FPaths::ProjectDir(), TEXT("../../../local/vendors/automation/currency_test.json"));
    TestTrue(TEXT("a file under <repo>/local is accepted"), FOpenWillowCurrencySave::ResolvePath(Inside, Resolved, Error));
    TestFalse(TEXT("a non-json file is refused"), FOpenWillowCurrencySave::ResolvePath(Inside + TEXT(".txt"), Resolved, Error));

    // Writing twice replaces the file in one step and keeps the previous good save as .bak.
    FOpenWillowCurrencySave::ResolvePath(Inside, Resolved, Error);
    IFileManager::Get().Delete(*Resolved, false, true, true);
    IFileManager::Get().Delete(*(Resolved + TEXT(".bak")), false, true, true);
    TestTrue(TEXT("first write"), FOpenWillowCurrencySave::WriteFile(Resolved, TEXT("first"), Error));
    TestTrue(TEXT("second write"), FOpenWillowCurrencySave::WriteFile(Resolved, TEXT("second"), Error));
    FString Text;
    FFileHelper::LoadFileToString(Text, *Resolved);
    TestEqual(TEXT("the file holds the second write"), Text, FString(TEXT("second")));
#if PLATFORM_WINDOWS
    FFileHelper::LoadFileToString(Text, *(Resolved + TEXT(".bak")));
    TestEqual(TEXT("the backup holds the first write"), Text, FString(TEXT("first")));
#endif
    IFileManager::Get().Delete(*Resolved, false, true, true);
    IFileManager::Get().Delete(*(Resolved + TEXT(".bak")), false, true, true);

    return true;
}
#endif
