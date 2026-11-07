// AI-assisted (Claude). Synthetic checks of the lootable and pickup rules (OpenWillowInteractive.h): weighted picks, the
// configuration and attachment roll, the cash formula, the ammo cap, the heal clamp and the down refusal, saturating money.
// Every number below is invented for the test; no manifest or game data is read.
// Run: UnrealEditor.exe <uproject> -game -nullrhi -ExecCmds="Automation RunTests OpenWillow.Interactive; Quit"
#include "Misc/AutomationTest.h"

#if WITH_DEV_AUTOMATION_TESTS
#include "OpenWillowInteractive.h"
#include "OpenWillowQuickChange.h"
#include "OpenWillowInventory.h"
#include "Math/RandomStream.h"
#include "Dom/JsonObject.h"
#include "Serialization/JsonReader.h"
#include "Serialization/JsonSerializer.h"

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

    // A leaf remembers the point that rolled it, which is what places it. Names and numbers invented.
    {
        const TSharedPtr<FJsonObject> Balance = MakeShared<FJsonObject>();
        const FString Json = TEXT("{\"configurations\":[{\"name\":\"c\",\"weight\":1,\"attachments\":[{\"point\":\"Gun7\",\"probability\":1,")
            TEXT("\"items\":[{\"balance\":\"Synthetic.Item.Placed\",\"class\":\"InventoryBalanceDefinition\",\"weight\":1}]}]}],")
            TEXT("\"visual\":{\"components\":[{\"skeletal_mesh\":\"P.Skel_Synthetic\",\"sockets\":[")
            TEXT("{\"name\":\"Gun7\",\"bone\":\"Base\",\"location\":[1,2,3]},{\"name\":\"Broken\",\"bone\":\"Base\",\"location\":[1,2]}]}]}}");
        TSharedPtr<FJsonObject> Parsed;
        const TSharedRef<TJsonReader<>> JsonReader = TJsonReaderFactory<>::Create(Json);
        TestTrue(TEXT("the synthetic balance parses"), FJsonSerializer::Deserialize(JsonReader, Parsed));
        FOpenWillowLootTable Placed;
        FString Error;
        TestTrue(TEXT("the synthetic balance loads"), Placed.Load(Parsed, Error));
        FRandomStream PlacedRng(7);
        TArray<FOpenWillowLootLeaf> PlacedLeaves;
        FString PlacedConfig;
        Placed.Roll(PlacedRng, PlacedLeaves, PlacedConfig);
        TestEqual(TEXT("one leaf rolled"), PlacedLeaves.Num(), 1);
        TestTrue(TEXT("the leaf names its point"), PlacedLeaves.Num() == 1 && PlacedLeaves[0].Point == TEXT("Gun7"));
        const FOpenWillowLootSocket* Socket = Placed.Sockets.Find(TEXT("Gun7"));
        TestTrue(TEXT("the socket is read"), Socket != nullptr);
        TestTrue(TEXT("its bone"), Socket && Socket->Bone == FName(TEXT("Base")));
        TestTrue(TEXT("its offset"), Socket && Socket->Offset.Equals(FVector(1.0, 2.0, 3.0)));
        TestFalse(TEXT("a socket with a broken location is left out"), Placed.Sockets.Contains(TEXT("Broken")));
    }

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

    // The lid capture leaves a balance's first lootable to the test's own open, unless it is the only one.
    TestTrue(TEXT("no lootable, no candidate"), OpenWillowInteractiveRules::LidCandidateOrder(0).IsEmpty());
    TestTrue(TEXT("a lone lootable is its own candidate"), OpenWillowInteractiveRules::LidCandidateOrder(1) == TArray<int32>{0});
    TestTrue(TEXT("last first, the first left out"), OpenWillowInteractiveRules::LidCandidateOrder(3) == TArray<int32>({2, 1}));

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
    // The use key: the nearest target in its own reach wins, whichever kind it is. Squared distances are invented.
    {
        const auto Pick = [](TOptional<double> Vendor, TOptional<double> Interactive, TOptional<double> Door, TOptional<double> Pickup)
        {
            FOpenWillowUseDistances Targets;
            Targets.Vendor = Vendor;
            Targets.Interactive = Interactive;
            Targets.Door = Door;
            Targets.Pickup = Pickup;
            return Targets.Nearest();
        };
        const TOptional<double> Unset;
        TestTrue(TEXT("nothing in reach"), Pick(Unset, Unset, Unset, Unset) == EOpenWillowUseTarget::None);
        TestTrue(TEXT("a lootable at 90 beats a vendor at 180"), Pick(180.0 * 180.0, 90.0 * 90.0, Unset, Unset) == EOpenWillowUseTarget::Interactive);
        TestTrue(TEXT("a vendor at 90 beats a lootable at 180"), Pick(90.0 * 90.0, 180.0 * 180.0, Unset, Unset) == EOpenWillowUseTarget::Vendor);
        TestTrue(TEXT("a lootable at 90 beats a door at 180"), Pick(Unset, 90.0 * 90.0, 180.0 * 180.0, Unset) == EOpenWillowUseTarget::Interactive);
        TestTrue(TEXT("a door at 90 beats a lootable at 180"), Pick(Unset, 180.0 * 180.0, 90.0 * 90.0, Unset) == EOpenWillowUseTarget::Door);
        TestTrue(TEXT("a dropped gun at 90 beats a lootable at 180"), Pick(Unset, 180.0 * 180.0, Unset, 90.0 * 90.0) == EOpenWillowUseTarget::Pickup);
        TestTrue(TEXT("a lootable at 90 beats a dropped gun at 180"), Pick(Unset, 90.0 * 90.0, Unset, 180.0 * 180.0) == EOpenWillowUseTarget::Interactive);
        TestTrue(TEXT("all four: the nearest wins"), Pick(300.0, 200.0, 400.0, 100.0) == EOpenWillowUseTarget::Pickup);

        // Equal distances keep one fixed order.
        FOpenWillowUseDistances Tied;
        Tied.Vendor = Tied.Interactive = Tied.Door = Tied.Pickup = 100.0;
        const TArray<EOpenWillowUseTarget> TieOrder = Tied.Ordered();
        TestEqual(TEXT("all four tied are listed"), TieOrder.Num(), 4);
        if (TieOrder.Num() == 4)
        {
            TestTrue(TEXT("tie: vendor first"), TieOrder[0] == EOpenWillowUseTarget::Vendor);
            TestTrue(TEXT("tie: interactive second"), TieOrder[1] == EOpenWillowUseTarget::Interactive);
            TestTrue(TEXT("tie: door third"), TieOrder[2] == EOpenWillowUseTarget::Door);
            TestTrue(TEXT("tie: pickup last"), TieOrder[3] == EOpenWillowUseTarget::Pickup);
        }

        // A nearer target that refuses lets the next one take the press, and nothing further is tried.
        FOpenWillowUseDistances Refusing;
        Refusing.Interactive = 100.0; // a lootable that cannot be opened
        Refusing.Door = 200.0;
        Refusing.Pickup = 300.0;
        TArray<EOpenWillowUseTarget> Tried;
        const bool bTaken = Refusing.Offer([&Tried](EOpenWillowUseTarget Target)
        {
            Tried.Add(Target);
            return Target != EOpenWillowUseTarget::Interactive;
        });
        TestTrue(TEXT("the door behind a refusing pickup takes the press"), bTaken);
        TestEqual(TEXT("the third target is not offered"), Tried.Num(), 2);
        TestTrue(TEXT("the refusing target was offered first"), Tried.Num() > 0 && Tried[0] == EOpenWillowUseTarget::Interactive);
        // A pickup that would refuse is not a target at all, so what is behind it keeps the global order: door, then chest.
        FOpenWillowUseDistances BehindRefusing;
        BehindRefusing.Door = 100.0;
        BehindRefusing.Interactive = 150.0;
        TestTrue(TEXT("the door is offered before the further chest"), BehindRefusing.Nearest() == EOpenWillowUseTarget::Door);
        int32 HeldAmmoLeft = 0;
        TestFalse(TEXT("a full pool would refuse, so the pickup is no target"),
            OpenWillowInteractiveRules::AmmoAfter(20, 20, 5, HeldAmmoLeft));
        TestTrue(TEXT("every target refusing means nothing took it"), !Refusing.Offer([](EOpenWillowUseTarget) { return false; }));
    }

    // The Quick Change press: the first asks, a second inside the window confirms, a late one asks again.
    const UObject* WalkerA = NewObject<UOpenWillowInventory>(GetTransientPackage()); // any two objects stand in for walkers
    const UObject* WalkerB = NewObject<UOpenWillowInventory>(GetTransientPackage());
    FOpenWillowQuickChangePress QuickChange;
    TestFalse(TEXT("the first press only asks"), QuickChange.Press(10.0, WalkerA, 200));
    TestTrue(TEXT("a second press within the window confirms"), QuickChange.Press(12.0, WalkerA, 200));
    TestFalse(TEXT("after a confirm the next press asks again"), QuickChange.Press(13.0, WalkerA, 200));
    TestFalse(TEXT("a press after the window asks again"), QuickChange.Press(13.0 + FOpenWillowQuickChangePress::ConfirmSeconds + 0.1, WalkerA, 200));
    TestFalse(TEXT("another walker's press does not confirm"), QuickChange.Press(19.0, WalkerB, 200));
    TestFalse(TEXT("nor does the first walker confirm the other's prompt"), QuickChange.Press(20.0, WalkerA, 200));
    TestFalse(TEXT("a changed price asks again"), QuickChange.Press(21.0, WalkerA, 300));
    TestEqual(TEXT("the new prompt holds the new price"), QuickChange.PromptedCost, 300);
    TestTrue(TEXT("the new price can then be confirmed"), QuickChange.Press(22.0, WalkerA, 300));
    TestFalse(TEXT("a cancelled prompt leaves nothing open"), QuickChange.IsOpen());
    QuickChange.Press(30.0, WalkerA, 200);
    QuickChange.Cancel();
    TestFalse(TEXT("a press after a cancel only asks"), QuickChange.Press(31.0, WalkerA, 200));
    TestFalse(TEXT("a null walker never confirms"), QuickChange.Press(32.0, nullptr, 200));

    // Why an open prompt ends. It is open for WalkerA from time 40 at price 200.
    FOpenWillowQuickChangePress Open;
    TestTrue(TEXT("no prompt, no reason"), Open.CancelReason(40.0, WalkerA, false, true) == nullptr);
    Open.Press(40.0, WalkerA, 200);
    TestTrue(TEXT("a selected, up walker keeps it"), Open.CancelReason(41.0, WalkerA, false, true) == nullptr);
    TestEqual(TEXT("down ends it"), FString(Open.CancelReason(41.0, WalkerA, true, true)), FString(TEXT("down")));
    TestEqual(TEXT("not selected ends it"), FString(Open.CancelReason(41.0, WalkerA, false, false)), FString(TEXT("not the selected target")));
    TestEqual(TEXT("another walker ends it"), FString(Open.CancelReason(41.0, WalkerB, false, true)), FString(TEXT("another walker")));
    TestEqual(TEXT("no current walker ends it"), FString(Open.CancelReason(41.0, nullptr, false, false)), FString(TEXT("no walker")));
    TestEqual(TEXT("expiry needs no walker"), FString(Open.CancelReason(40.0 + FOpenWillowQuickChangePress::ConfirmSeconds + 0.1, nullptr, false, false)), FString(TEXT("expired")));

    return true;
}
#endif
