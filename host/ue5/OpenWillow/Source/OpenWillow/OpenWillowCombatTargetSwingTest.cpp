// AI-assisted (Claude). Synthetic checks of the practice dummy's frame-time split (AOpenWillowCombatTarget::StepHitSpring,
// KnockSlide, StepFall): a long frame must not make the swing, the slide or the fall bigger, unstable or early, and frames of
// 1/30 s or less must integrate exactly as the old single step did. The values are invented for the test; the spring, friction
// and gravity constants are the host's own (OpenWillowCombatTarget.cpp).
// Run: UnrealEditor.exe <uproject> -game -nullrhi -ExecCmds="Automation RunTests OpenWillow.CombatTarget.Swing; Quit"
#include "Misc/AutomationTest.h"

#if WITH_DEV_AUTOMATION_TESTS
#include "OpenWillowCombatTarget.h"

namespace
{
// The old single-step code, written out for comparison.
void OldSpring(FVector2D& Angle, FVector2D& AngleVelocity, float Dt)
{
    AngleVelocity += (-60.f * Angle - 7.f * AngleVelocity) * Dt;
    Angle += AngleVelocity * Dt;
}
}

IMPLEMENT_SIMPLE_AUTOMATION_TEST(FOpenWillowCombatTargetSwingTest, "OpenWillow.CombatTarget.Swing",
    EAutomationTestFlags::ClientContext | EAutomationTestFlags::EngineFilter)

bool FOpenWillowCombatTargetSwingTest::RunTest(const FString& Parameters)
{
    const FVector2D Kick(220.f, -90.f);
    const float Full = 1.f / 30.f;

    // One 0.6 s stall is the same as 18 frames of 1/30 s.
    FVector2D StallAngle = FVector2D::ZeroVector, StallSpeed = Kick;
    AOpenWillowCombatTarget::StepHitSpring(StallAngle, StallSpeed, 0.6f);
    FVector2D FrameAngle = FVector2D::ZeroVector, FrameSpeed = Kick;
    for (int32 Frame = 0; Frame < 18; ++Frame) AOpenWillowCombatTarget::StepHitSpring(FrameAngle, FrameSpeed, Full);
    TestTrue(TEXT("0.6 s stall equals 18 frames of 1/30 s"),
        StallAngle.Equals(FrameAngle, 1e-3) && StallSpeed.Equals(FrameSpeed, 1e-3));

    // The stall stays bounded: one explicit step of 0.6 s would throw the angle past 400 degrees; the kick's own amplitude is about 32.
    FVector2D Angle = FVector2D::ZeroVector, Speed = Kick;
    float Peak = 0.f;
    for (int32 Frame = 0; Frame < 18; ++Frame)
    {
        AOpenWillowCombatTarget::StepHitSpring(Angle, Speed, Full);
        Peak = FMath::Max(Peak, FMath::Abs(float(Angle.X)));
    }
    TestTrue(TEXT("swing during a 0.6 s stall stays bounded"), Peak < 40.f);

    // Boundary: exactly 1/30 s and just below it are one old step; just above it is two steps (no undercount).
    const float Below = Full - 1e-7f;
    const float Above = Full + 1e-5f;
    const float Dts[] = {Full, Below, 1.f / 60.f, 1.f / 144.f};
    for (const float Dt : Dts)
    {
        FVector2D A(3.f, -4.f), V(-40.f, 12.f);
        AOpenWillowCombatTarget::StepHitSpring(A, V, Dt);
        FVector2D OldA(3.f, -4.f), OldV(-40.f, 12.f);
        OldSpring(OldA, OldV, Dt);
        TestTrue(TEXT("frame at or below 1/30 s integrates as one old step"), A.Equals(OldA, 1e-6) && V.Equals(OldV, 1e-6));
    }
    {
        FVector2D A(3.f, -4.f), V(-40.f, 12.f);
        AOpenWillowCombatTarget::StepHitSpring(A, V, Above);
        FVector2D OldA(3.f, -4.f), OldV(-40.f, 12.f);
        OldSpring(OldA, OldV, Full);
        OldSpring(OldA, OldV, Above - Full);
        TestTrue(TEXT("frame just above 1/30 s is a full step plus a remainder"), A.Equals(OldA, 1e-6) && V.Equals(OldV, 1e-6));
    }

    // dt = 0: nothing moves.
    {
        FVector2D A(3.f, -4.f), V(-40.f, 12.f);
        AOpenWillowCombatTarget::StepHitSpring(A, V, 0.f);
        TestTrue(TEXT("zero frame leaves the swing unchanged"), A.Equals(FVector2D(3.f, -4.f), 1e-9) && V.Equals(FVector2D(-40.f, 12.f), 1e-9));
    }

    // A stall longer than 2 s is cut at 2 s (the swing has decayed to about 0.1% by then).
    FVector2D LongAngle = FVector2D::ZeroVector, LongSpeed = Kick;
    AOpenWillowCombatTarget::StepHitSpring(LongAngle, LongSpeed, 10.f);
    FVector2D CutAngle = FVector2D::ZeroVector, CutSpeed = Kick;
    AOpenWillowCombatTarget::StepHitSpring(CutAngle, CutSpeed, 2.f);
    TestTrue(TEXT("a 10 s stall equals a 2 s stall"), LongAngle.Equals(CutAngle, 1e-3) && LongSpeed.Equals(CutSpeed, 1e-3));

    // Knock slide (friction 8 per second, the host's KnockFriction): a 0.6 s stall slides as far as 18 frames of 1/30 s, and
    // leaves the knock where 18 frames would.
    const FVector KnockKick(300.f, -120.f, 0.f);
    float StallFactor = 1.f;
    const FVector StallOffset = AOpenWillowCombatTarget::KnockSlide(KnockKick, 0.6f, 8.f, StallFactor);
    const FVector StallKnock = KnockKick * StallFactor;
    FVector FrameKnock = KnockKick;
    FVector FrameOffset = FVector::ZeroVector;
    for (int32 Frame = 0; Frame < 18; ++Frame)
    {
        float FrameFactor = 1.f;
        FrameOffset += AOpenWillowCombatTarget::KnockSlide(FrameKnock, Full, 8.f, FrameFactor);
        FrameKnock *= FrameFactor;
    }
    TestTrue(TEXT("0.6 s knock slide equals 18 frames of 1/30 s"),
        StallOffset.Equals(FrameOffset, 1e-2) && StallKnock.Equals(FrameKnock, 1e-3));

    // A normal frame is one old step: offset = knock x dt (X, Y only), then the knock x (1 - 8 dt).
    const float KnockDt = 1.f / 60.f;
    float OneFactor = 1.f;
    const FVector OneOffset = AOpenWillowCombatTarget::KnockSlide(KnockKick, KnockDt, 8.f, OneFactor);
    const FVector OneKnock = KnockKick * OneFactor;
    const FVector OldOffset(KnockKick.X * KnockDt, KnockKick.Y * KnockDt, 0.f);
    const FVector OldKnock = KnockKick * FMath::Max(0.f, 1.f - 8.f * KnockDt);
    TestTrue(TEXT("normal knock frame integrates as one old step"),
        OneOffset.Equals(OldOffset, 1e-6) && OneKnock.Equals(OldKnock, 1e-6));

    // Zero frame: no slide, friction factor 1.
    float ZeroFactor = 0.f;
    const FVector ZeroOffset = AOpenWillowCombatTarget::KnockSlide(KnockKick, 0.f, 8.f, ZeroFactor);
    TestTrue(TEXT("zero knock frame neither slides nor slows"), ZeroOffset.IsNearlyZero(1e-9) && FMath::IsNearlyEqual(ZeroFactor, 1.f, 1e-9f));

    // A 10 s stall slides as far as a 2 s stall.
    float LongFactor = 1.f, CutFactor = 1.f;
    const FVector LongOffset = AOpenWillowCombatTarget::KnockSlide(KnockKick, 10.f, 8.f, LongFactor);
    const FVector CutOffset = AOpenWillowCombatTarget::KnockSlide(KnockKick, 2.f, 8.f, CutFactor);
    TestTrue(TEXT("a 10 s knock stall equals a 2 s stall"),
        LongOffset.Equals(CutOffset, 1e-2) && FMath::IsNearlyEqual(LongFactor, CutFactor, 1e-6f));

    // Released fall under -500 cm/s^2 from 300 uu: 0.6 s as a stall equals 18 frames of 1/30 s. The analytic drop is 0.5 * 500 * 0.6^2 = 90 uu
    // (210 uu left); the semi-implicit steps drop 500 * (1/30)^2 * (1 + ... + 18) = 95 uu, leaving 205 uu.
    float StallHeight = 300.f, StallFall = 0.f;
    AOpenWillowCombatTarget::StepFall(StallHeight, StallFall, 0.6f);
    float FrameHeight = 300.f, FrameFall = 0.f;
    for (int32 Frame = 0; Frame < 18; ++Frame) AOpenWillowCombatTarget::StepFall(FrameHeight, FrameFall, Full);
    TestTrue(TEXT("0.6 s fall equals 18 frames of 1/30 s"),
        FMath::IsNearlyEqual(StallHeight, FrameHeight, 1e-2f) && FMath::IsNearlyEqual(StallFall, FrameFall, 1e-2f));
    TestTrue(TEXT("0.6 s fall lands near 205 uu (semi-implicit; analytic 210)"), StallHeight > 200.f && StallHeight < 212.f);

    // Fall from 50 uu over 0.6 s lands on the floor in both ways, with no speed left.
    float LowStall = 50.f, LowStallFall = 0.f;
    AOpenWillowCombatTarget::StepFall(LowStall, LowStallFall, 0.6f);
    float LowFrame = 50.f, LowFrameFall = 0.f;
    for (int32 Frame = 0; Frame < 18; ++Frame) AOpenWillowCombatTarget::StepFall(LowFrame, LowFrameFall, Full);
    TestTrue(TEXT("a low fall through a stall lands like the frames"),
        FMath::IsNearlyZero(LowStall, 1e-4f) && FMath::IsNearlyZero(LowFrame, 1e-4f) && FMath::IsNearlyEqual(LowStallFall, LowFrameFall, 1e-3f));

    // A normal fall frame is one old step (dt = 1/60 from 100 uu, falling at 20 uu/s).
    float OneHeight = 100.f, OneFall = -20.f;
    AOpenWillowCombatTarget::StepFall(OneHeight, OneFall, KnockDt);
    const float OldFall = 100.f > 0.f ? -20.f - 500.f * KnockDt : 0.f;
    const float OldHeight = FMath::Max(0.f, 100.f + OldFall * KnockDt);
    TestTrue(TEXT("normal fall frame integrates as one old step"),
        FMath::IsNearlyEqual(OneHeight, OldHeight, 1e-6f) && FMath::IsNearlyEqual(OneFall, OldFall, 1e-6f));

    // Zero frame on the floor: the old code cleared the fall speed; so does the split.
    float FloorHeight = 0.f, FloorFall = -50.f;
    AOpenWillowCombatTarget::StepFall(FloorHeight, FloorFall, 0.f);
    TestTrue(TEXT("zero frame on the floor clears the fall speed as before"),
        FMath::IsNearlyZero(FloorHeight, 1e-6f) && FMath::IsNearlyZero(FloorFall, 1e-6f));

    return true;
}

#endif //WITH_DEV_AUTOMATION_TESTS
