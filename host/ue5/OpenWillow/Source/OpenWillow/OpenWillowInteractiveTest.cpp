// AI-assisted (Claude). Synthetic checks of the lootable and pickup rules (OpenWillowInteractive.h): weighted picks, the
// configuration and attachment roll, the cash formula, the ammo cap, the heal clamp and the down refusal, saturating money.
// Every number below is invented for the test; no manifest or game data is read.
// Run: UnrealEditor.exe <uproject> -game -nullrhi -ExecCmds="Automation RunTests OpenWillow.Interactive; Quit"
#include "Misc/AutomationTest.h"

#if WITH_DEV_AUTOMATION_TESTS
#include "OpenWillowInteractive.h"
#include "Math/RandomStream.h"

IMPLEMENT_SIMPLE_AUTOMATION_TEST(FOpenWillowInteractiveLootTest, "OpenWillow.Interactive.Loot",
    EAutomationTestFlags::ClientContext | EAutomationTestFlags::EngineFilter)

bool FOpenWillowInteractiveLootTest::RunTest(const FString& Parameters)
{
    // Weighted pick: zero weights are never chosen; unit 0 takes the first weighted entry, unit near 1 the last.
    const TArray<float> Weights{0.f, 1.f, 3.f};
    TestEqual(TEXT("unit 0 takes the first weighted entry"), FOpenWillowLootTable::WeightedIndex(Weights, 0.f), 1);
    TestEqual(TEXT("a unit of 0.2 lands in the first entry (0.8 of 4)"), FOpenWillowLootTable::WeightedIndex(Weights, 0.2f), 1);
    TestEqual(TEXT("a unit of 0.99 lands in the heavy entry"), FOpenWillowLootTable::WeightedIndex(Weights, 0.99f), 2);
    TestEqual(TEXT("no weight means no pick"), FOpenWillowLootTable::WeightedIndex(TArray<float>{0.f, 0.f}, 0.5f), INDEX_NONE);

    // A table with two configurations: one with a probability-0 point and one that always fires its only point.
    FOpenWillowLootTable Table;
    {
        FOpenWillowLootConfig Never;
        Never.Name = TEXT("never");
        Never.Weight = 1.f;
        FOpenWillowLootAttachment Dud;
        Dud.Probability = 0.f;
        Dud.Leaves.Add({TEXT("Synthetic.Item.Dud"), TEXT("InventoryBalanceDefinition"), 1.f});
        Never.Attachments.Add(Dud);
        Table.Configs.Add(Never);
    }
    {
        FOpenWillowLootConfig Sure;
        Sure.Name = TEXT("sure");
        Sure.Weight = 0.f; // never picked: weight 0
        FOpenWillowLootAttachment Always;
        Always.Probability = 1.f;
        Always.Leaves.Add({TEXT("Synthetic.Item.Sure"), TEXT("InventoryBalanceDefinition"), 1.f});
        Sure.Attachments.Add(Always);
        Table.Configs.Add(Sure);
    }
    FRandomStream Rng(12345);
    bool bDudFired = false, bSureNamed = false;
    for (int32 Roll = 0; Roll < 200; ++Roll)
    {
        TArray<FOpenWillowLootLeaf> Leaves;
        FString Config;
        Table.Roll(Rng, Leaves, Config);
        if (Config != TEXT("never")) bSureNamed = true;
        for (const FOpenWillowLootLeaf& Leaf : Leaves) if (Leaf.Balance == TEXT("Synthetic.Item.Dud")) bDudFired = true;
    }
    TestFalse(TEXT("a probability-0 point never fires"), bDudFired);
    TestFalse(TEXT("a zero-weight configuration is never picked"), bSureNamed);

    // Cash formula on invented constants: calc(L) = 10 x 2^L, amount = calc x (1 + 0.5 x grade). L = 3 gives 80; grade 4 gives 240.
    FOpenWillowPickupSpec Cash;
    Cash.Kind = TEXT("cash");
    Cash.Multiplier = 10.f;
    Cash.LevelBase = 2.f;
    Cash.BaseScale = 1.f;
    Cash.PerGradeScale = 0.5f;
    Cash.GradeMin = 0;
    Cash.GradeMax = 9;
    TestEqual(TEXT("cash at grade 0"), Cash.CashAmount(3, 0), 80);
    TestEqual(TEXT("cash at grade 4"), Cash.CashAmount(3, 4), 240);
    // Rounded down: L = 0 gives calc 10; grade 1 with PerGradeScale 0.55 gives 10 x 1.55 = 15.5, which becomes 15.
    Cash.PerGradeScale = 0.55f;
    TestEqual(TEXT("cash rounds down"), Cash.CashAmount(0, 1), 15);
    Cash.PerGradeScale = 0.5f;

    // Gear value: trunc(part x scaler^L x base) with invented numbers: 2 x 1.5^2 x 10 = 45.
    FOpenWillowLootItem Gear;
    Gear.Kind = TEXT("shield");
    Gear.bHasValue = true;
    Gear.ValuePartTotal = 2.f;
    Gear.ValueScaler = 1.5f;
    Gear.ValueBaseCost = 10.f;
    TestEqual(TEXT("gear value at level 2"), Gear.GearValueAt(2), 45);
    Gear.bHasValue = false;
    TestEqual(TEXT("gear without a formula has no value"), Gear.GearValueAt(2), -1);

    // Ammo: refused when full; otherwise the pool takes the rounds up to its maximum.
    int32 After = 0;
    TestTrue(TEXT("ammo fits"), OpenWillowInteractiveRules::AmmoAfter(10, 20, 6, After));
    TestEqual(TEXT("ammo after a fitting pickup"), After, 16);
    TestTrue(TEXT("ammo is capped at the pool maximum"), OpenWillowInteractiveRules::AmmoAfter(15, 20, 18, After));
    TestEqual(TEXT("ammo clamps to the maximum"), After, 20);
    TestFalse(TEXT("a full pool refuses the pickup"), OpenWillowInteractiveRules::AmmoAfter(20, 20, 18, After));
    TestFalse(TEXT("a zero amount is refused"), OpenWillowInteractiveRules::AmmoAfter(5, 20, 0, After));

    EOpenWillowAmmoType Type = EOpenWillowAmmoType::Pistol;
    TestTrue(TEXT("ar maps to the assault rifle pool"), OpenWillowInteractiveRules::AmmoTypeFromName(TEXT("ar"), Type));
    TestEqual(TEXT("ar pool"), int32(Type), int32(EOpenWillowAmmoType::AssaultRifle));
    TestFalse(TEXT("an unknown ammo name is refused"), OpenWillowInteractiveRules::AmmoTypeFromName(TEXT("laser"), Type));

    // Health: heals a fraction of the maximum, clamped to the maximum; refused while down or at full health.
    float Health = 0;
    TestTrue(TEXT("a heal below the maximum"), OpenWillowInteractiveRules::HealAfter(50.f, 100.f, 0.25f, false, Health));
    TestEqual(TEXT("25% of 100 on 50 gives 75"), Health, 75.f);
    TestTrue(TEXT("a heal past the maximum is clamped"), OpenWillowInteractiveRules::HealAfter(90.f, 100.f, 0.25f, false, Health));
    TestEqual(TEXT("clamped to the maximum"), Health, 100.f);
    TestFalse(TEXT("a down walker is refused"), OpenWillowInteractiveRules::HealAfter(10.f, 100.f, 0.25f, true, Health));
    TestFalse(TEXT("a walker at full health is refused"), OpenWillowInteractiveRules::HealAfter(100.f, 100.f, 0.25f, false, Health));

    // Money and eridium saturate instead of wrapping.
    TestEqual(TEXT("a plain add"), OpenWillowInteractiveRules::SaturatingAdd(100, 23), 123);
    TestEqual(TEXT("an add past int32 saturates"), OpenWillowInteractiveRules::SaturatingAdd(MAX_int32 - 5, 10), MAX_int32);
    TestEqual(TEXT("a negative sum floors at zero"), OpenWillowInteractiveRules::SaturatingAdd(0, -7), 0);
    return true;
}
#endif
