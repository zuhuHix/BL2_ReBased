// AI-assisted (Claude). Synthetic checks of the host's movement-feel rules (OpenWillowMovement): the sprint input cone, the
// walk bob, the eye height easing, the sprint FOV and falling damage. Inputs are invented; the expected values follow from
// the rules' own constants (NATIVE_PLAYER_MOVEMENT.md), so this pins the host's arithmetic, not the game.
// Run: UnrealEditor.exe <uproject> -game -nullrhi -ExecCmds="Automation RunTests OpenWillow.MayaMovement; Quit"
#include "Misc/AutomationTest.h"

#if WITH_DEV_AUTOMATION_TESTS
#include "OpenWillowMayaMovement.h"

IMPLEMENT_SIMPLE_AUTOMATION_TEST(FOpenWillowMayaMovementTest, "OpenWillow.MayaMovement",
    EAutomationTestFlags::ClientContext | EAutomationTestFlags::EngineFilter)

bool FOpenWillowMayaMovementTest::RunTest(const FString&)
{
    using namespace OpenWillowMovement;
    TestTrue(TEXT("straight ahead allows sprint"), SprintInputAllowed(1.f, 0.f));
    TestTrue(TEXT("forward plus strafe (45 degrees) allows sprint"), SprintInputAllowed(1.f, 0.99f));
    TestFalse(TEXT("pure strafe does not"), SprintInputAllowed(0.f, 1.f));
    TestFalse(TEXT("backward does not"), SprintInputAllowed(-1.f, 0.f));
    TestFalse(TEXT("no input does not"), SprintInputAllowed(0.f, 0.f));

    FBob B;
    float MaxUp = 0.f, MaxSide = 0.f;
    for (int32 I = 0; I < 600; ++I)
    {
        B.Tick(1.f / 60.f, 400.f, true, false, 400.f);
        MaxUp = FMath::Max(MaxUp, FMath::Abs(B.Up));
        MaxSide = FMath::Max(MaxSide, FMath::Abs(B.Side));
    }
    // b = 0.005: side amplitude b x speed = 2.0, up 0.75 x that = 1.5; BobTime advances (0.7 + 0.3) per second at speed = ground speed.
    TestTrue(TEXT("walk bob side amplitude"), FMath::IsNearlyEqual(MaxSide, 2.f, 0.05f));
    TestTrue(TEXT("walk bob up amplitude"), FMath::IsNearlyEqual(MaxUp, 1.5f, 0.05f));
    TestTrue(TEXT("bob time at ground speed"), FMath::IsNearlyEqual(B.BobTime, 10.f, 0.01f));
    for (int32 I = 0; I < 60; ++I) B.Tick(1.f / 60.f, 400.f, false, false, 400.f);
    TestTrue(TEXT("bob resets and decays in the air"), B.BobTime == 0.f && FMath::Abs(B.Up) < 0.01f);
    FBob Sprint;
    for (int32 I = 0; I < 120; ++I) Sprint.Tick(1.f / 60.f, 500.f, true, true, 400.f);
    TestTrue(TEXT("sprint bob is the base amplitude x 0.01"), FMath::Abs(Sprint.Side) <= 0.005f * 0.01f * 500.f + 1e-4f);

    FEyeHeight Eye;
    float Height = 0.f;
    for (int32 I = 0; I < 120; ++I) Height = Eye.Tick(1.f / 60.f, true, false);
    TestTrue(TEXT("crouched eye height"), FMath::IsNearlyEqual(Height, CrouchedEyeHeight, 0.01f));
    for (int32 I = 0; I < 120; ++I) Height = Eye.Tick(1.f / 60.f, false, true);
    TestTrue(TEXT("sprint divides the eye height by 1.02"), FMath::IsNearlyEqual(Height, BaseEyeHeight / 1.02f, 0.01f));

    // The factor is tan(1.35 x 21.5 deg) / tan(21.5 deg) for the 43-degree vertical default; at 90 degrees it is atan of that.
    const float Expected = FMath::RadiansToDegrees(2.f * FMath::Atan(FMath::Tan(FMath::DegreesToRadians(1.35f * 43.0f / 2.f))
        / FMath::Tan(FMath::DegreesToRadians(43.0f / 2.f))));
    TestTrue(TEXT("sprint FOV from 90"), FMath::IsNearlyEqual(SprintFov(90.f), Expected, 0.2f));
    TestTrue(TEXT("sprint FOV at the 16:9 vertical default is x 1.35"), FMath::IsNearlyEqual(SprintFov(43.0f), 43.0f * 1.35f, 0.2f));
    float Fov = 90.f;
    for (int32 I = 0; I < 60; ++I) Fov = EaseFov(Fov, 100.f, 1.f / 60.f);
    TestTrue(TEXT("FOV eases most of the way in a second"), Fov > 99.f && Fov <= 100.f);
    TestEqual(TEXT("FOV snaps on a full step"), EaseFov(90.f, 100.f, 0.2f), 100.f);

    TestEqual(TEXT("no falling damage above the threshold"), FallingDamage(-1400.f, 500.f), 0.f);
    TestTrue(TEXT("falling damage at twice the threshold is 10 % of the maximum"), FMath::IsNearlyEqual(FallingDamage(-3000.f, 500.f), 50.f, 0.01f));
    return true;
}
#endif
