// AI-assisted (Claude). Synthetic checks of the host's Phaselock rules (FOpenWillowPhaselockData): targeting range and
// magnetism, the cast gate, the timeline, the lock duration and the diminishing returns. Every number below is invented
// for the test; nothing is read from the game or the manifest. The rules themselves are host readings of native code and
// script (NATIVE_PHASELOCK_TARGETING.md) and stay UNVERIFIED in game; this test only pins the host's arithmetic.
// The evaluator classes come from FOpenWillowPhaselockData's own constants, not quoted here.
// Run: UnrealEditor.exe <uproject> -game -nullrhi -ExecCmds="Automation RunTests OpenWillow.Phaselock; Quit"
#include "Misc/AutomationTest.h"

#if WITH_DEV_AUTOMATION_TESTS
#include "OpenWillowPhaselock.h"

namespace
{
FOpenWillowPhaselockData SyntheticData()
{
    FOpenWillowPhaselockData D;
    D.bLoaded = true;
    D.LiftDuration = 0.5f;
    D.LockFadeOutTime = 1.f;
    D.ReleaseBufferTime = 0.25f;
    D.LockDurationBase = 4.f;
    D.TimeScaleDefault = 1.f;
    D.DiminishingScale = -0.5f;
    D.DurationPostAdd = {0.f, 0.5f, 1.f};
    D.TargetMinDistance = 100.f;
    D.TargetMaxDistance = 2048.f;   // log2 = 11
    D.MaxSnapAngle = 0.2f;
    D.RadiusMultiplier = 1.f;
    D.DistanceOffset = 0.f;
    D.Constraints = {
        {FOpenWillowPhaselockData::WeaponActionEvaluator, true, false},
        {FOpenWillowPhaselockData::HealthStateEvaluator, true, true},
        {FOpenWillowPhaselockData::VehiclePassengerEvaluator, true, true},
    };
    return D;
}
}

IMPLEMENT_SIMPLE_AUTOMATION_TEST(FOpenWillowPhaselockTest, "OpenWillow.Phaselock",
    EAutomationTestFlags::ClientContext | EAutomationTestFlags::EngineFilter)

bool FOpenWillowPhaselockTest::RunTest(const FString& Parameters)
{
    const FOpenWillowPhaselockData D = SyntheticData();
    const float TanHalfFov = 1.f;

    // Targeting: a centred target inside the range scores; outside it, or off the magnetism radius, it does not.
    const FOpenWillowPhaselockAim Near = D.ScoreAim(500.f, 0.f, 0.f, 10.f, TanHalfFov);
    const FOpenWillowPhaselockAim Far = D.ScoreAim(1500.f, 0.f, 0.f, 10.f, TanHalfFov);
    TestTrue(TEXT("centred target in range scores"), Near.Score > 0.f && Near.Rejected.IsEmpty());
    TestTrue(TEXT("nearer target scores higher"), Near.Score > Far.Score && Far.Score > 0.f);
    TestEqual(TEXT("beyond MaxTargetDistance is rejected"), D.ScoreAim(2100.f, 0.f, 0.f, 10.f, TanHalfFov).Rejected,
        FString(TEXT("outside the distance range")));
    TestEqual(TEXT("closer than MinTargetDistance is rejected"), D.ScoreAim(50.f, 0.f, 0.f, 10.f, TanHalfFov).Rejected,
        FString(TEXT("outside the distance range")));
    // At depth 1024 the reach is 1 + 11 - 10 = 2, so the magnetism radius is 0.2 x 2 / 11.
    const FOpenWillowPhaselockAim Edge = D.ScoreAim(1024.f, 0.f, 0.f, 1.f, TanHalfFov);
    TestTrue(TEXT("magnetism radius on the log2 depth scale"), FMath::IsNearlyEqual(Edge.MagnetRadius, 0.4f / 11.f, 1e-5f));
    TestEqual(TEXT("off the magnetism radius is rejected"), D.ScoreAim(1024.f, 0.05f, 0.f, 1.f, TanHalfFov).Rejected,
        FString(TEXT("outside the magnetism radius")));
    TestTrue(TEXT("a large target widens its own radius"), D.ScoreAim(1024.f, 0.05f, 0.f, 100.f, TanHalfFov).Score > 0.f);
    TestTrue(TEXT("score halves the offset share"), FMath::IsNearlyEqual(Near.Score, 0.5f * (2.f - 500.f / 2048.f), 1e-5f));

    // Cast gate: reloading does not block; a holstered weapon, being down or in a vehicle does.
    FString Failed;
    FOpenWillowPhaselockGateState S;
    S.bHoldsWeapon = true;
    S.bReloading = true;
    TestTrue(TEXT("reloading does not refuse the cast"), D.GateOpen(S, true, Failed));
    FOpenWillowPhaselockGateState Holstered = S;
    Holstered.bWeaponInactive = true;
    TestTrue(TEXT("holstered weapon refuses the cast"), !D.GateOpen(Holstered, true, Failed) && Failed == FOpenWillowPhaselockData::WeaponActionEvaluator);
    TestTrue(TEXT("holstering while held does not end the lock"), D.GateOpen(Holstered, false, Failed));
    FOpenWillowPhaselockGateState Down = S;
    Down.bInjured = true;
    TestTrue(TEXT("going down refuses the cast"), !D.GateOpen(Down, true, Failed) && Failed == FOpenWillowPhaselockData::HealthStateEvaluator);
    TestTrue(TEXT("going down ends a running lock"), !D.GateOpen(Down, false, Failed) && Failed == FOpenWillowPhaselockData::HealthStateEvaluator);
    FOpenWillowPhaselockGateState Vehicle = S;
    Vehicle.bInVehicle = true;
    TestTrue(TEXT("a vehicle refuses the cast"), !D.GateOpen(Vehicle, true, Failed) && Failed == FOpenWillowPhaselockData::VehiclePassengerEvaluator);

    // The targetable list: only the mission's dummy can be refused, and only while unregistered with the quest on. A range pawn is
    // not the mission's dummy, so it stays a candidate with the quest on.
    TestTrue(TEXT("the mission's unregistered dummy is refused"), OpenWillowPhaselockListRefuses(true, true, false));
    TestFalse(TEXT("the mission's registered dummy is a candidate"), OpenWillowPhaselockListRefuses(true, true, true));
    TestFalse(TEXT("a range pawn is a candidate with the quest on"), OpenWillowPhaselockListRefuses(true, false, false));
    TestFalse(TEXT("nothing is refused with the quest off"), OpenWillowPhaselockListRefuses(false, true, false));

    // Duration and timeline: lift + lock x time scale; outro LockFadeOutTime before the release; end after the buffer.
    TestTrue(TEXT("lock duration adds the grade's PostAdd"), FMath::IsNearlyEqual(D.LockDuration(2), 5.f) && FMath::IsNearlyEqual(D.LockDuration(7), 4.f));
    const FOpenWillowPhaselockTimeline T = D.Timeline(D.LockDuration(1), D.TargetTimeScale(false));
    TestTrue(TEXT("timeline"), FMath::IsNearlyEqual(T.SkillDuration, 5.f) && FMath::IsNearlyEqual(T.LockedAt, 0.5f)
        && FMath::IsNearlyEqual(T.OutroAt, 4.f) && FMath::IsNearlyEqual(T.ReleasedAt, 5.f) && FMath::IsNearlyEqual(T.EndSkillAt, 5.25f));
    const FOpenWillowPhaselockTimeline Diminished = D.Timeline(D.LockDuration(0), D.TargetTimeScale(true));
    TestTrue(TEXT("diminishing returns scale the lock, not the lift"), FMath::IsNearlyEqual(Diminished.SkillDuration, 2.5f));
    return true;
}
#endif
