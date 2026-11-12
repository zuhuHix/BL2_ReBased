// AI-assisted (Claude). Synthetic checks of the host's survival rules (FOpenWillowMayaState): shield first then health, the
// recharge delay and rate, going down, the bleed-out cut for successive injuries, the second wind, death and the respawn.
// Every number below is invented for the test or is the rule's own constant; nothing is read from the game. The rules are
// the ones of docs/verification/COMBAT_DOWN_RESPAWN.md; this test only pins the host's arithmetic.
// Run: UnrealEditor.exe <uproject> -game -nullrhi -ExecCmds="Automation RunTests OpenWillow.MayaState; Quit" (runs OpenWillow.MayaState.Core
// and OpenWillow.MayaState.RespawnWaitSplit). No test here may take the bare path OpenWillow.MayaState: a path that is also the
// folder of other tests is never run (tests/automation_paths_test.py checks this).
#include "Misc/AutomationTest.h"

#if WITH_DEV_AUTOMATION_TESTS
#include "HAL/FileManager.h"
#include "Misc/FileHelper.h"
#include "Misc/Guid.h"
#include "Misc/Paths.h"
#include "OpenWillowMayaState.h"
#include "OpenWillowInventory.h"
#include "OpenWillowWalker.h"
#include "OpenWillowRespawnFee.h"
#include <limits>

namespace
{
FOpenWillowMayaState Synthetic()
{
    FOpenWillowMayaState S;
    S.Base.MaxHealth = 100.f;
    S.Health = 100.f;
    S.EquipShield(50.f, 2.f, 10.f, true, 0.f);
    return S;
}
// Steps the state from From to To at 1/60 s, returning the last event that was not None.
FOpenWillowMayaState::ETickEvent Run(FOpenWillowMayaState& S, float From, float To)
{
    FOpenWillowMayaState::ETickEvent Last = FOpenWillowMayaState::ETickEvent::None;
    for (float T = From; T < To; T += 1.f / 60.f)
    {
        const auto E = S.Tick(T, 1.f / 60.f);
        if (E != FOpenWillowMayaState::ETickEvent::None) Last = E;
        if (E == FOpenWillowMayaState::ETickEvent::RespawnDue) { S.Respawn(T); break; }
    }
    return Last;
}
}

IMPLEMENT_SIMPLE_AUTOMATION_TEST(FOpenWillowMayaStateTest, "OpenWillow.MayaState.Core",
    EAutomationTestFlags::ClientContext | EAutomationTestFlags::EngineFilter)

bool FOpenWillowMayaStateTest::RunTest(const FString&)
{
    using ETick = FOpenWillowMayaState::ETickEvent;
    {
        // The stand-in rule (invented numbers): parsing, growth by level, and a card without a capacity through equip, damage and recharge.
        FOpenWillowStandInShieldRule Rule;
        FString Error;
        TestFalse(TEXT("an unset rule gives no capacity"), Rule.IsSet() || Rule.CapacityAt(5) != 0.f);
        for (const TCHAR* Bad : {TEXT("not json"), TEXT("{}"), TEXT("{\"at_level\":0,\"capacity\":10,\"growth_per_level\":1.5}"),
                                 TEXT("{\"at_level\":2.5,\"capacity\":10,\"growth_per_level\":1.5}"), TEXT("{\"at_level\":2,\"capacity\":0,\"growth_per_level\":1.5}"),
                                 TEXT("{\"at_level\":2,\"capacity\":10,\"growth_per_level\":3}")})
            TestTrue(TEXT("a malformed rule is refused and stays unset"), !Rule.Parse(Bad, Error) && !Error.IsEmpty() && !Rule.IsSet());
        TestTrue(TEXT("a good rule parses"), Rule.Parse(TEXT("{\"at_level\":2,\"capacity\":100,\"growth_per_level\":1.5}"), Error));
        TestEqual(TEXT("capacity at the anchor"), Rule.CapacityAt(2), 100.f);
        TestEqual(TEXT("capacity grows per level"), Rule.CapacityAt(4), 225.f);
        TestEqual(TEXT("capacity falls below the anchor"), Rule.CapacityAt(1), 67.f);

        // The file read, on a temporary file with a new name each run, so a leftover cannot make this pass. Rule is still set here: the
        // failures must clear it.
        const FString RuleFile = FPaths::Combine(FPaths::AutomationTransientDir(), FString::Printf(TEXT("stand_in_shield_%s.json"), *FGuid::NewGuid().ToString()));
        {
            FOpenWillowStandInShieldRule Loaded = Rule;
            TestFalse(TEXT("a missing file is refused"), Loaded.LoadFile(RuleFile, Error));
            TestTrue(TEXT("a missing file names the path and leaves the rule unset"), Error.Contains(RuleFile) && !Loaded.IsSet());
            TestTrue(TEXT("the temporary file is written"), FFileHelper::SaveStringToFile(TEXT("{\"at_level\":3,\"capacity\":40,\"growth_per_level\":2}"), *RuleFile));
            TestTrue(TEXT("a good file loads"), Loaded.LoadFile(RuleFile, Error));
            TestEqual(TEXT("the good file's numbers are used"), Loaded.CapacityAt(4), 80.f);
            TestTrue(TEXT("the temporary file is rewritten"), FFileHelper::SaveStringToFile(TEXT("{\"at_level\":3}"), *RuleFile));
            TestFalse(TEXT("a bad file is refused"), Loaded.LoadFile(RuleFile, Error));
            TestTrue(TEXT("a bad file names the path and leaves the rule unset"), Error.Contains(RuleFile) && !Loaded.IsSet() && Loaded.CapacityAt(4) == 0.f);
            IFileManager::Get().Delete(*RuleFile);
            TestFalse(TEXT("the temporary file is gone"), FPaths::FileExists(RuleFile));
        }

        TestEqual(TEXT("no shield, no capacity"), AOpenWillowWalker::ReadShieldStats(nullptr, Rule).Capacity, 0.f);
        FOpenWillowGearItem StandIn;
        StandIn.ItemType = TEXT("shield");
        StandIn.Level = 4;
        const AOpenWillowWalker::FShieldStats FromRule = AOpenWillowWalker::ReadShieldStats(&StandIn, Rule);
        TestTrue(TEXT("a card with no capacity takes the rule's"), FromRule.bStandIn && FromRule.Capacity == 225.f);
        TestEqual(TEXT("without the rule it still gives nothing"), AOpenWillowWalker::ReadShieldStats(&StandIn, FOpenWillowStandInShieldRule()).Capacity, 0.f);
        FOpenWillowGearItem Carded = StandIn;
        FOpenWillowGearStat Stat;
        Stat.Label = TEXT("Capacity");
        Stat.Value = TEXT("1,250");
        Carded.Stats.Add(Stat);
        const AOpenWillowWalker::FShieldStats FromCard = AOpenWillowWalker::ReadShieldStats(&Carded, Rule);
        TestTrue(TEXT("a card's own capacity wins"), !FromCard.bStandIn && FromCard.Capacity == 1250.f);
        // The recharge stand-ins (invented host defaults) apply only to a card without its own recharge stats; the values are not pinned here.
        const FOpenWillowMayaStats Defaults;
        TestTrue(TEXT("the recharge stand-ins are set"), Defaults.ShieldRechargeDelay > 0.f && Defaults.ShieldRechargeRate > 0.f);
        TestTrue(TEXT("a card without recharge stats keeps the stand-ins"),
            FromRule.RechargeDelay == Defaults.ShieldRechargeDelay && FromRule.RechargeRate == Defaults.ShieldRechargeRate
            && FromCard.RechargeDelay == Defaults.ShieldRechargeDelay && FromCard.RechargeRate == Defaults.ShieldRechargeRate);
        FOpenWillowGearItem Recharged = Carded;
        Stat.Label = TEXT("Recharge Delay");
        Stat.Value = TEXT("3.5");
        Recharged.Stats.Add(Stat);
        Stat.Label = TEXT("Recharge Rate");
        Stat.Value = TEXT("12");
        Recharged.Stats.Add(Stat);
        const AOpenWillowWalker::FShieldStats FromRecharge = AOpenWillowWalker::ReadShieldStats(&Recharged, Rule);
        TestTrue(TEXT("a card's own recharge stats win"), FromRecharge.RechargeDelay == 3.5f && FromRecharge.RechargeRate == 12.f);

        FOpenWillowMayaState Maya = Synthetic();
        Maya.EquipShield(FromRule.Capacity, FromRule.RechargeDelay, FromRule.RechargeRate, true, 0.f);
        TestEqual(TEXT("the first stand-in shield starts full"), Maya.Shield, 225.f);
        const float HealthBefore = Maya.Health;
        FOpenWillowDamageResult Hit = Maya.ApplyDamage(100.f, 1.f);
        TestTrue(TEXT("the stand-in shield takes the hit"), Hit.ToShield == 100.f && Hit.ToHealth == 0.f && Maya.Health == HealthBefore);
        Run(Maya, 1.f, 1.f + FromRule.RechargeDelay + 2.f);
        TestTrue(TEXT("it recharges after the delay"), Maya.Shield > 125.f);
    }
    {
        // The fee's rule, with invented cash values and text.
        TestEqual(TEXT("fee rounds half up"), FOpenWillowRespawnFee::FeeFor(50), 4);
        TestEqual(TEXT("fee rounds down below a half"), FOpenWillowRespawnFee::FeeFor(1000), 70);
        TestEqual(TEXT("fee of nothing"), FOpenWillowRespawnFee::FeeFor(0), 0);
        TestEqual(TEXT("fee of a little rounds to nothing"), FOpenWillowRespawnFee::FeeFor(7), 0);
        const FOpenWillowRespawnFee::FStrings None = FOpenWillowRespawnFee::ParseStrings(TEXT("[Other]\nRespawnDialogString=wrong section\n"));
        TestEqual(TEXT("another section is not read"), FOpenWillowRespawnFee::Message(None, 5), FString(TEXT("Fee: $5")));
        TestFalse(TEXT("the neutral text says so"), None.bLoaded);
        TestFalse(TEXT("two of three strings is not loaded"), FOpenWillowRespawnFee::ParseStrings(TEXT("[WillowHUD]\nRespawnDialogTitleString=T\nRespawnDialogString=B\n")).bLoaded);
        TestFalse(TEXT("a repeated string does not stand in for a missing one"), FOpenWillowRespawnFee::ParseStrings(TEXT("[WillowHUD]\nRespawnDialogTitleString=T\nRespawnDialogString=B\nRespawnDialogString=C\n")).bLoaded);
        const FOpenWillowRespawnFee::FStrings Empty = FOpenWillowRespawnFee::ParseStrings(TEXT("[WillowHUD]\nRespawnDialogTitleString=T\nRespawnDialogString=B\nRespawnDialogFeeWaivedString=\n"));
        TestFalse(TEXT("an empty string is not found"), Empty.bLoaded);
        TestEqual(TEXT("an empty string keeps the neutral text"), Empty.Waived, None.Waived);
        const FOpenWillowRespawnFee::FStrings Read = FOpenWillowRespawnFee::ParseStrings(TEXT("[WillowHUD]\r\nRespawnDialogTitleString=T\r\nRespawnDialogString=Paid %c\\n\\nDone\r\nRespawnDialogFeeWaivedString=Free\r\n"));
        TestEqual(TEXT("the title is read"), Read.Title, FString(TEXT("T")));
        TestTrue(TEXT("all three strings is loaded"), Read.bLoaded);
        TestEqual(TEXT("the fee goes in and line breaks are real"), FOpenWillowRespawnFee::Message(Read, 9), FString(TEXT("Paid 9\n\nDone")));
        TestEqual(TEXT("no fee uses the waived text"), FOpenWillowRespawnFee::Message(Read, 0), FString(TEXT("Free")));
        const TArray<FString> Lines = FOpenWillowRespawnFee::WrapLines(TEXT("aa bb cc\n\ndd"), 5, 10);
        TestTrue(TEXT("wrap at spaces, keeping the blank line"), Lines == TArray<FString>({TEXT("aa bb"), TEXT("cc"), TEXT(""), TEXT("dd")}));
        const TArray<FString> Cut = FOpenWillowRespawnFee::WrapLines(TEXT("a bbbbbbb c"), 3, 10);
        TestTrue(TEXT("a word too long for a line is cut"), Cut == TArray<FString>({TEXT("a"), TEXT("bbb"), TEXT("bbb"), TEXT("b c")}));
        TestEqual(TEXT("the line count is limited"), FOpenWillowRespawnFee::WrapLines(TEXT("a\nb\nc\nd"), 3, 2).Num(), 2);
        TestEqual(TEXT("a long word stops at the limit too"), FOpenWillowRespawnFee::WrapLines(TEXT("bbbbbbbbbbbbbbbbbbbb"), 3, 2).Num(), 2);
        FOpenWillowRespawnFee Fee;
        FOpenWillowRespawnTimeline Respawn;
        Respawn.Begin(10.f);
        Fee.Schedule(Respawn);
        Fee.Tick(21.f, nullptr);
        TestEqual(TEXT("not due before the delay"), Fee.LastFee(), -1);
        Fee.Tick(21.9f, nullptr);
        TestFalse(TEXT("an unknown purse charges nothing and shows nothing"), Fee.LastFee() >= 0 || Fee.IsMessagePending() || Fee.IsMessageShown(21.9f));
        Fee.BeginMessage(22.f);
        TestFalse(TEXT("nothing pending, nothing begins"), Fee.IsMessageShown(22.f));
    }
    {
        FOpenWillowMayaState S = Synthetic();
        TestEqual(TEXT("first shield starts full"), S.Shield, 50.f);
        FOpenWillowDamageResult R = S.ApplyDamage(30.f, 1.f);
        TestTrue(TEXT("a hit smaller than the shield only takes shield"), R.ToShield == 30.f && R.ToHealth == 0.f && S.Health == 100.f);
        R = S.ApplyDamage(30.f, 1.5f);
        TestTrue(TEXT("the hit that empties the shield sends the rest to health"), R.ToShield == 20.f && R.ToHealth == 10.f && S.Health == 90.f && S.Shield == 0.f);
        Run(S, 1.5f, 3.4f);
        TestEqual(TEXT("no recharge before the delay"), S.Shield, 0.f);
        Run(S, 3.5f, 4.5f);
        TestTrue(TEXT("recharge at the rate after the delay"), S.Shield > 9.f && S.Shield < 11.f);
        S.ApplyDamage(1.f, 4.5f);
        const float After = S.Shield;
        Run(S, 4.5f, 6.4f);
        TestEqual(TEXT("each hit restarts the delay"), S.Shield, After);
        Run(S, 6.5f, 20.f);
        TestEqual(TEXT("recharge stops at the capacity"), S.Shield, 50.f);
        TestEqual(TEXT("health does not regenerate at rate 0"), S.Health, 90.f);
        const float NaN = FMath::Sqrt(-1.f);
        TestTrue(TEXT("NaN damage is ignored"), S.ApplyDamage(NaN, 21.f).ToHealth == 0.f && S.Health == 90.f && S.Shield == 50.f);
        TestTrue(TEXT("infinite damage is ignored"), S.ApplyDamage(std::numeric_limits<float>::infinity(), 22.f).ToShield == 0.f && S.Health == 90.f
            && S.Shield == 50.f && !S.IsDown());
    }
    {
        FOpenWillowMayaState S = Synthetic();
        const FOpenWillowDamageResult R = S.ApplyDamage(1000.f, 100.f);
        TestTrue(TEXT("health 0 puts Maya down"), R.bWentDown && S.Phase == EOpenWillowDownPhase::Injured && S.Health == 0.f && R.ToHealth == 100.f);
        TestTrue(TEXT("damage while down is ignored"), S.ApplyDamage(10.f, 101.f).bIgnored);
        TestTrue(TEXT("no weapon during the put-away"), !S.WeaponReady(101.f) && S.WeaponReady(100.f + FOpenWillowMayaState::TargetedDelay));
        TestEqual(TEXT("first injury: the full bleed-out"), S.BleedSeconds, FOpenWillowMayaState::BaseBleedout);
        TestEqual(TEXT("bleed bar full until the timer starts"), S.BleedLeft(102.f), 1.f);
        TestTrue(TEXT("a second wind while Injured"), S.SecondWind(105.f) && S.Phase == EOpenWillowDownPhase::Up);
        TestEqual(TEXT("second wind health is 25 % of the maximum"), S.Health, 25.f);
        TestEqual(TEXT("second wind shield is 90 % of the capacity"), S.Shield, 45.f);
        TestTrue(TEXT("no second wind when up"), !S.SecondWind(106.f));
        // Successive injuries: 9 s apart, so the counter grows; the cut uses the counter before the injury.
        S.ApplyDamage(1000.f, 110.f);   // counter before 0 -> full; after: 1
        TestEqual(TEXT("second injury within 30 s: still full"), S.BleedSeconds, 12.f);
        S.SecondWind(111.f);
        S.ApplyDamage(1000.f, 120.f);   // counter before 1 -> 12 - 4 = 8
        TestEqual(TEXT("third successive injury: 8 s"), S.BleedSeconds, 8.f);
        TestEqual(TEXT("counter after the third"), S.SuccessiveInjuries, 2);
        const ETick E = Run(S, 120.f, 120.f + FOpenWillowMayaState::TargetedDelay + 8.f + 0.1f);
        TestTrue(TEXT("bleed-out ends in the dead state"), E == ETick::BledOut && S.Phase == EOpenWillowDownPhase::Dead);
        const float DeadAt = S.DeadAt;
        Run(S, DeadAt, DeadAt + FOpenWillowMayaState::DeathCamera + 0.1f);
        TestTrue(TEXT("respawn after the death camera"), S.Phase == EOpenWillowDownPhase::Up && S.Health == 100.f && S.Shield == 50.f
            && S.SuccessiveInjuries == 0 && FMath::IsNearlyEqual(S.RespawnAt - DeadAt, FOpenWillowMayaState::DeathCamera, 0.02f));
    }
    {
        FOpenWillowMayaState S = Synthetic();
        S.ApplyDamage(1000.f, 200.f);
        TestTrue(TEXT("a refill while Injured revives"), S.SetHealth(120.f, 120.f, 201.f) && !S.IsDown() && S.HealthFraction() == 1.f);
        S.EquipShield(80.f, 1.f, 5.f, false, 210.f);
        TestTrue(TEXT("a swapped shield starts empty"), S.Shield == 0.f && S.ShieldFraction() == 0.f);
        Run(S, 210.f, 212.f);
        TestTrue(TEXT("and recharges after its own delay"), S.Shield > 4.f && S.Shield < 6.f);
        // A shield swapped while down: the second wind uses the new capacity.
        S.ApplyDamage(1000.f, 215.f);
        S.EquipShield(200.f, 1.f, 5.f, false, 216.f);
        TestTrue(TEXT("second wind after a swap while down uses the new shield"), S.SecondWind(217.f) && S.Shield == 180.f);
        S.EquipShield(0.f, 1.f, 5.f, false, 220.f);
        TestTrue(TEXT("no shield: fraction 0, hits go to health"), S.ShieldFraction() == 0.f && S.ApplyDamage(10.f, 221.f).ToHealth == 10.f);
    }
    {
        // Skill modifiers through Effective(): raised maxima fill by regeneration and recharge; when the modifier ends the
        // next tick clamps health and shield back to the base maxima (host rule, UNVERIFIED).
        FOpenWillowMayaState S = Synthetic();
        bool bBoost = true;
        S.Modifiers = [&bBoost](FOpenWillowMayaStats& Stats)
        {
            if (!bBoost) return;
            Stats.MaxHealth *= 1.5f;
            Stats.ShieldCapacity *= 1.25f;
            Stats.HealthRegenRate += 30.f;
        };
        TestTrue(TEXT("Effective applies the modifiers to Base"), S.Effective().MaxHealth == 150.f && S.Base.MaxHealth == 100.f);
        Run(S, 300.f, 305.f);
        TestTrue(TEXT("boosted pools fill past the base maxima"), S.Health > 100.f && S.Shield > 50.f);
        bBoost = false;
        S.InvalidateEffective();   // what the skills component does on a grade change
        S.Tick(305.1f, 1.f / 60.f);
        TestTrue(TEXT("modifier ended: health and shield clamp to the base maxima"), S.Health == 100.f && S.Shield == 50.f);
        bBoost = true;
        S.InvalidateEffective();
        S.SetHealth(120.f, 120.f, 306.f);
        TestTrue(TEXT("a level-up refill fills to the effective maximum"), S.Health == 180.f && S.Base.MaxHealth == 120.f);
        S.SetHealth(120.f, 60.f, 307.f);
        TestTrue(TEXT("a non-refill keeps the given current health"), S.Health == 60.f);
        int32 Calls = 0;
        S.Modifiers = [&Calls](FOpenWillowMayaStats& Stats) { ++Calls; Stats.GroundSpeed += 1.f; };
        S.InvalidateEffective();
        const float First = S.Effective().GroundSpeed, Second = S.Effective().GroundSpeed;
        TestTrue(TEXT("one evaluation per frame: reads agree"), Calls == 1 && First == Second);
        S.Base.GroundSpeed += 10.f;
        TestTrue(TEXT("a Base change recomputes within the frame"), S.Effective().GroundSpeed == S.Base.GroundSpeed + 1.f && Calls == 2);
        S.Modifiers = nullptr;
        TestTrue(TEXT("no modifiers: Effective is Base"), S.Effective().MaxHealth == 120.f);
    }
    return true;
}

// respawn_wait_split: the station respawn's parts as named moments (FOpenWillowRespawnTimeline) and the fee read from them. The
// numbers checked are the rule's own constants (observed in the real game, COMBAT_DOWN_RESPAWN.md); the start times, the frame
// steps and the purse are invented.
IMPLEMENT_SIMPLE_AUTOMATION_TEST(FOpenWillowRespawnWaitSplitTest, "OpenWillow.MayaState.RespawnWaitSplit",
    EAutomationTestFlags::ClientContext | EAutomationTestFlags::EngineFilter)

bool FOpenWillowRespawnWaitSplitTest::RunTest(const FString&)
{
    using FTimeline = FOpenWillowRespawnTimeline;
    using EMoment = FTimeline::EMoment;
    constexpr float Frame = 1.f / 60.f;   // one host frame
    // The parts: the hold after the start, then the moments after the arrival.
    TestTrue(TEXT("respawn_wait_split: the hold ends 3.0 s after the start"), FMath::Abs(FTimeline::Offset(EMoment::Arrival) - 3.0f) <= Frame);
    struct FPart { EMoment Moment; float AfterArrival; };
    const FPart Parts[] = {{EMoment::EffectStart, 1.5f}, {EMoment::GearBack, 4.5f}, {EMoment::EffectEnd, 6.5f}, {EMoment::CostDisplay, 6.79f},
        {EMoment::Fee, 8.79f}};
    for (const FPart& Part : Parts)
        TestTrue(FString::Printf(TEXT("respawn_wait_split: %s about %.2f s after the arrival"), FTimeline::Name(Part.Moment), Part.AfterArrival),
            FMath::Abs(FTimeline::Offset(Part.Moment) - FTimeline::Offset(EMoment::Arrival) - Part.AfterArrival) <= Frame);
    TestEqual(TEXT("respawn_wait_split: input back at the cost display (UNVERIFIED to the frame)"), FTimeline::Offset(EMoment::InputBack),
        FTimeline::Offset(EMoment::CostDisplay));
    TestTrue(TEXT("respawn_wait_split: the fee 11.8 s after the start in all, the fee's own delay"),
        FMath::Abs(FTimeline::FeeSeconds - 11.8f) <= Frame && FOpenWillowRespawnFee::DelaySeconds == FTimeline::FeeSeconds);

    // A run at host frames from a start that falls between two frames: every moment comes once, in order, on the first frame at or
    // after it; the hold is hidden, without input and gear; nothing is held before a start or after input is back.
    FTimeline T;
    TestFalse(TEXT("respawn_wait_split: nothing held before a respawn"), T.HasStarted() || T.IsHeld() || T.IsHidden() || T.IsInputLocked()
        || T.Advance(5.f) != EMoment::Count);
    const double Start = 41.0037;
    T.Begin(float(Start));
    UOpenWillowInventory* Purse = NewObject<UOpenWillowInventory>(GetTransientPackage());
    Purse->SetMoney(1000);
    FOpenWillowRespawnFee Fee;
    Fee.Schedule(T);
    TArray<EMoment> Order;
    bool bHoldOk = true, bAfterHoldOk = true;
    int32 HoldFrames = 0;
    for (int32 I = 0; I <= 13 * 60; ++I)
    {
        const float Now = float(Start + I * double(Frame));
        for (EMoment M = T.Advance(Now); M != EMoment::Count; M = T.Advance(Now)) Order.Add(M);
        Fee.Tick(Now, Purse);
        if (Now < T.At(EMoment::Arrival))
        {
            ++HoldFrames;
            bHoldOk &= T.IsHeld() && T.IsHidden() && T.IsGearAway() && T.IsInputLocked();
        }
        else if (Now < T.At(EMoment::EffectStart)) bAfterHoldOk &= !T.IsHeld() && T.IsHidden() && T.IsInputLocked();
        else if (Now < T.At(EMoment::GearBack)) bAfterHoldOk &= !T.IsHidden() && T.IsGearAway() && T.IsInputLocked();
        else if (Now < T.At(EMoment::InputBack)) bAfterHoldOk &= !T.IsGearAway() && T.IsInputLocked();
        else bAfterHoldOk &= !T.IsHeld() && !T.IsHidden() && !T.IsGearAway() && !T.IsInputLocked();
    }
    TestTrue(TEXT("respawn_wait_split: the 3.0 is a hold, hidden and unable to act, in every frame of it"), bHoldOk && HoldFrames >= 179);
    TestTrue(TEXT("respawn_wait_split: hidden until the effect starts, gear until gear back, input until input back"), bAfterHoldOk);
    const TArray<EMoment> Expected = {EMoment::Arrival, EMoment::EffectStart, EMoment::GearBack, EMoment::EffectEnd, EMoment::CostDisplay,
        EMoment::InputBack, EMoment::Fee};
    TestTrue(TEXT("respawn_wait_split: each moment once, in order"), Order == Expected);
    for (EMoment M : Expected)
    {
        const float Late = T.ReachedAt(M) - T.At(M);
        TestTrue(FString::Printf(TEXT("respawn_wait_split: %s on the first frame at or after it (%.4f s late)"), FTimeline::Name(M), Late),
            Late >= -KINDA_SMALL_NUMBER && Late < Frame);
    }
    int32 Money = 0;
    Purse->GetMoney(Money);
    TestTrue(TEXT("respawn_wait_split: the fee falls due with the timeline's fee moment and its rule is unchanged"),
        Fee.LastFee() == 70 && Money == 930 && Fee.LastChargedAt() == T.ReachedAt(EMoment::Fee));

    // One long frame past the whole timeline (a hitch, a pause): every moment still comes, in order, each once, in that frame.
    FTimeline Jump;
    Jump.Begin(200.f);
    TArray<EMoment> InOneFrame;
    for (EMoment M = Jump.Advance(200.f + FTimeline::FeeSeconds + 5.f); M != EMoment::Count; M = Jump.Advance(200.f + FTimeline::FeeSeconds + 5.f))
        InOneFrame.Add(M);
    TestTrue(TEXT("respawn_wait_split: one frame past the end gives every moment once, in order"), InOneFrame == Expected
        && !Jump.IsHeld() && !Jump.IsHidden() && !Jump.IsGearAway() && !Jump.IsInputLocked()
        && Jump.Advance(300.f) == EMoment::Count);

    // A second respawn before the charge starts the timeline again and replaces the fee, as before the split.
    FTimeline Twice;
    FOpenWillowRespawnFee Replaced;
    Purse->SetMoney(1000);
    Twice.Begin(100.f);
    Replaced.Schedule(Twice);
    Twice.Advance(103.5f);
    Twice.Begin(105.f);
    Replaced.Schedule(Twice);
    TestTrue(TEXT("respawn_wait_split: a second respawn holds again from its own start"), Twice.IsHeld() && Twice.ReachedAt(EMoment::Arrival) < 0.f
        && Twice.Advance(107.9f) == EMoment::Count && Twice.Advance(108.f) == EMoment::Arrival);
    Replaced.Tick(112.f, Purse);
    TestEqual(TEXT("respawn_wait_split: the first respawn's fee is not charged"), Replaced.LastFee(), -1);
    Replaced.Tick(116.9f, Purse);
    TestEqual(TEXT("respawn_wait_split: the second one's is, 11.8 s after its start"), Replaced.LastFee(), 70);
    return true;
}
#endif
