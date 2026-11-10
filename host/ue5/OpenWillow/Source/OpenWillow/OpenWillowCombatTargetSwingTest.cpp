// AI-assisted (Claude). Synthetic checks of the practice dummy's hit swing (AOpenWillowCombatTarget::StepHitSpring): a long
// frame must not make the swing bigger or unstable, and frames of 1/30 s or less must integrate as before. The kick values
// are invented for the test; the spring constants are the host's own (OpenWillowCombatTarget.cpp).
// Run: UnrealEditor.exe <uproject> -game -nullrhi -ExecCmds="Automation RunTests OpenWillow.CombatTarget.Swing; Quit"
#include "Misc/AutomationTest.h"

#if WITH_DEV_AUTOMATION_TESTS
#include "OpenWillowCombatTarget.h"

IMPLEMENT_SIMPLE_AUTOMATION_TEST(FOpenWillowCombatTargetSwingTest, "OpenWillow.CombatTarget.Swing",
    EAutomationTestFlags::ClientContext | EAutomationTestFlags::EngineFilter)

bool FOpenWillowCombatTargetSwingTest::RunTest(const FString& Parameters)
{
    const FVector2D Kick(220.f, -90.f);

    // One 0.6 s stall is the same as 18 frames of 1/30 s.
    FVector2D StallAngle = FVector2D::ZeroVector, StallSpeed = Kick;
    AOpenWillowCombatTarget::StepHitSpring(StallAngle, StallSpeed, 0.6f);
    FVector2D FrameAngle = FVector2D::ZeroVector, FrameSpeed = Kick;
    for (int32 Frame = 0; Frame < 18; ++Frame) AOpenWillowCombatTarget::StepHitSpring(FrameAngle, FrameSpeed, 1.f / 30.f);
    TestTrue(TEXT("0.6 s stall equals 18 frames of 1/30 s"),
        StallAngle.Equals(FrameAngle, 1e-3) && StallSpeed.Equals(FrameSpeed, 1e-3));

    // The stall stays bounded: one explicit step of 0.6 s would throw the angle past 400 degrees; the kick's own amplitude is about 32.
    FVector2D Angle = FVector2D::ZeroVector, Speed = Kick;
    float Peak = 0.f;
    for (int32 Frame = 0; Frame < 18; ++Frame)
    {
        AOpenWillowCombatTarget::StepHitSpring(Angle, Speed, 1.f / 30.f);
        Peak = FMath::Max(Peak, FMath::Abs(float(Angle.X)));
    }
    TestTrue(TEXT("swing during a 0.6 s stall stays bounded"), Peak < 40.f);

    // Frames up to 1/30 s are one step, exactly the old formula (WobbleVelocity += ... dt; Wobble += velocity dt).
    const float Frames[] = {1.f / 30.f, 1.f / 60.f, 1.f / 144.f};
    for (const float Dt : Frames)
    {
        FVector2D A(3.f, -4.f), V(-40.f, 12.f);
        AOpenWillowCombatTarget::StepHitSpring(A, V, Dt);
        FVector2D OldV(-40.f, 12.f), OldA(3.f, -4.f);
        OldV += (-60.f * OldA - 7.f * OldV) * Dt;
        OldA += OldV * Dt;
        TestTrue(TEXT("normal frame integrates as one old step"), A.Equals(OldA, 1e-6) && V.Equals(OldV, 1e-6));
    }

    // A stall longer than 2 s is cut at 2 s (the swing has decayed to about 0.1% by then).
    FVector2D LongAngle = FVector2D::ZeroVector, LongSpeed = Kick;
    AOpenWillowCombatTarget::StepHitSpring(LongAngle, LongSpeed, 10.f);
    FVector2D CutAngle = FVector2D::ZeroVector, CutSpeed = Kick;
    AOpenWillowCombatTarget::StepHitSpring(CutAngle, CutSpeed, 2.f);
    TestTrue(TEXT("a 10 s stall equals a 2 s stall"), LongAngle.Equals(CutAngle, 1e-3) && LongSpeed.Equals(CutSpeed, 1e-3));

    return true;
}

#endif //WITH_DEV_AUTOMATION_TESTS
