// AI-assisted (Claude). Synthetic checks of the host's survival rules (FOpenWillowMayaState): shield first then health, the
// recharge delay and rate, going down, the bleed-out cut for successive injuries, the second wind, death and the respawn.
// Every number below is invented for the test or is the rule's own constant; nothing is read from the game. The rules are
// the ones of docs/verification/COMBAT_DOWN_RESPAWN.md; this test only pins the host's arithmetic.
// Run: UnrealEditor.exe <uproject> -game -nullrhi -ExecCmds="Automation RunTests OpenWillow.MayaState; Quit"
#include "Misc/AutomationTest.h"

#if WITH_DEV_AUTOMATION_TESTS
#include "OpenWillowMayaState.h"

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

IMPLEMENT_SIMPLE_AUTOMATION_TEST(FOpenWillowMayaStateTest, "OpenWillow.MayaState",
    EAutomationTestFlags::ClientContext | EAutomationTestFlags::EngineFilter)

bool FOpenWillowMayaStateTest::RunTest(const FString&)
{
    using ETick = FOpenWillowMayaState::ETickEvent;
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
        TestTrue(TEXT("infinite damage is ignored"), S.ApplyDamage(TNumericLimits<float>::Infinity(), 22.f).ToShield == 0.f && S.Health == 90.f
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
        S.EquipShield(0.f, 1.f, 5.f, false, 220.f);
        TestTrue(TEXT("no shield: fraction 0, hits go to health"), S.ShieldFraction() == 0.f && S.ApplyDamage(10.f, 221.f).ToHealth == 10.f);
    }
    return true;
}
#endif
