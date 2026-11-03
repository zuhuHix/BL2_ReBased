// AI-assisted (Claude). Synthetic checks of the vending stock rules (FOpenWillowVendorStock): prices, buy with enough and
// too little money, a full backpack, sell and its refusal for an equipped gun, buy-back, and the reset clock. Every number
// below is invented for the test; no weapon generator, manifest or game data is read. The prices use the buy formula
// confirmed live (docs/verification/VENDORS.md section 3); the sale and buy-back prices stay UNVERIFIED in game.
// Run: UnrealEditor.exe <uproject> -game -nullrhi -ExecCmds="Automation RunTests OpenWillow.Vendor.Stock; Quit"
#include "Misc/AutomationTest.h"

#if WITH_DEV_AUTOMATION_TESTS
#include "OpenWillowVendor.h"

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
    TestEqual(TEXT("price is floor(value x markup)"), Stock.PriceOf(Stock.Items[0]), 484);
    TestEqual(TEXT("second gun's price"), Stock.PriceOf(Stock.Items[1]), 196);
    TestEqual(TEXT("formula on its own, level 10: floor(51 x 5.5)"), FOpenWillowVendorStock::BuyPrice(51, 10), 280);
    TestEqual(TEXT("an absurd value is unaffordable, not wrapped"), FOpenWillowVendorStock::BuyPrice(MAX_int32, 100), MAX_int32);
    TestEqual(TEXT("sell price is the card value"), FOpenWillowVendorStock::SellPriceOf(Stock.Items[0]), 101);

    // Too little money: refused, nothing taken, the gun stays on the shelf.
    FString Error;
    UOpenWillowInventory* Poor = SyntheticInventory(150);
    TestFalse(TEXT("buy with too little money is refused"), Stock.Buy(TEXT("stock_0"), Poor, Error));
    TestEqual(TEXT("refusal text"), Error, FString(TEXT("Insufficient funds")));
    int32 Money = 0;
    Poor->GetMoney(Money);
    TestEqual(TEXT("money unchanged after a refused buy"), Money, 150);
    TestEqual(TEXT("shelf unchanged after a refused buy"), Stock.Items.Num(), 2);
    TestFalse(TEXT("unknown id is refused"), Stock.Buy(TEXT("stock_9"), Poor, Error));
    TestEqual(TEXT("unknown id text"), Error, FString(TEXT("Unknown item")));

    // Enough money: buy gun A (484) then gun B (196) from 1000.
    UOpenWillowInventory* Inventory = SyntheticInventory(1000);
    TestTrue(TEXT("buy with enough money"), Stock.Buy(TEXT("stock_0"), Inventory, Error));
    Inventory->GetMoney(Money);
    TestEqual(TEXT("money after buying A"), Money, 516);
    TestEqual(TEXT("A is in the backpack"), Inventory->Items().Num(), 1);
    TestEqual(TEXT("A left the shelf"), Stock.Items.Num(), 1);
    TestTrue(TEXT("buy B"), Stock.Buy(TEXT("stock_1"), Inventory, Error));
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
    TestFalse(TEXT("buy into a full backpack is refused"), Stock.Buy(TEXT("stock_2"), Inventory, Error));
    TestEqual(TEXT("full backpack text"), Error, FString(TEXT("Backpack full")));
    Inventory->GetMoney(Money);
    TestEqual(TEXT("money unchanged after a refused buy into a full backpack"), Money, 320);

    // Featured item uses its own fixed cost when the manifest sets one.
    Stock.FixedFeaturedCost = 50;
    FOpenWillowVendorItem Featured = FOpenWillowVendorStock::MakeItem(SyntheticGun(TEXT("synthetic_d"), 100), TEXT("featured"), true);
    TestEqual(TEXT("fixed featured cost wins over the markup"), Stock.PriceOf(Featured), 50);
    Stock.FixedItemCost = 70;
    TestEqual(TEXT("fixed item cost wins over the markup"), Stock.PriceOf(Stock.Items[0]), 70);

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
#endif
