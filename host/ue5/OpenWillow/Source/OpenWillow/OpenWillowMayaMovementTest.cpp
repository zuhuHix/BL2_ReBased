// AI-assisted (Claude). Synthetic checks of the host's movement-feel rules (OpenWillowMovement): the sprint input cone, the
// walk bob, the eye height easing, the sprint FOV and falling damage. Inputs are invented; the expected values follow from
// the rules' own constants (NATIVE_PLAYER_MOVEMENT.md), so this pins the host's arithmetic, not the game.
// Run: UnrealEditor.exe <uproject> -game -nullrhi -ExecCmds="Automation RunTests OpenWillow.MayaMovement; Quit"
#include <limits>
#include "Misc/AutomationTest.h"

#if WITH_DEV_AUTOMATION_TESTS
#include "OpenWillowMayaMovement.h"
#include "OpenWillowWalker.h"
#include "GameFramework/CharacterMovementComponent.h"

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
    // Within a degree of the view turn measured on the real game's frames (about 79.5 and 98.5 horizontal at 16:9).
    const float Wide = 16.f / 9.f;
    TestTrue(TEXT("aimed FOV at zoom 45"), FMath::IsNearlyEqual(AimedFov(45.f, Wide, 90.f, -1.f), 79.5f, 1.f));
    TestTrue(TEXT("aimed FOV at zoom 60"), FMath::IsNearlyEqual(AimedFov(60.f, Wide, 90.f, -1.f), 98.5f, 1.f));
    // The rule's own values, worked out by hand from its formula; no real frame checks these aspects or that setting.
    TestTrue(TEXT("aimed FOV at 4:3"), FMath::IsNearlyEqual(AimedFov(45.f, 4.f / 3.f, 90.f, -1.f), 64.72f, 0.05f));
    TestTrue(TEXT("aimed FOV at 21:9"), FMath::IsNearlyEqual(AimedFov(45.f, 21.f / 9.f, 90.f, -1.f), 95.91f, 0.05f));
    TestTrue(TEXT("aimed FOV in a portrait window"), FMath::IsNearlyEqual(AimedFov(45.f, 9.f / 16.f, 90.f, -1.f), 29.93f, 0.05f));
    TestTrue(TEXT("aimed FOV at setting 70"), FMath::IsNearlyEqual(AimedFov(45.f, Wide, 70.f, -1.f), 61.21f, 0.05f));
    // Just inside the zoom's limits it still gives an angle; at them and beyond, the fallback.
    TestTrue(TEXT("a zoom just above 1"), FMath::IsNearlyEqual(AimedFov(1.01f, Wide, 90.f, -1.f), 2.06f, 0.05f));
    TestTrue(TEXT("a zoom just below 180"), FMath::IsNearlyEqual(AimedFov(179.f, Wide, 90.f, -1.f), 179.51f, 0.05f));
    const float NotANumber = std::numeric_limits<float>::quiet_NaN();
    const float Infinite = std::numeric_limits<float>::infinity();
    for (const float Zoom : {0.f, 1.f, -45.f, 180.f, 181.f, 400.f, NotANumber, Infinite})
    {
        TestEqual(TEXT("a zoom that is no angle falls back"), AimedFov(Zoom, Wide, 90.f, 106.f), 106.f);
    }
    TestEqual(TEXT("a setting of 180 falls back"), AimedFov(45.f, Wide, 180.f, 106.f), 106.f);
    TestEqual(TEXT("an aspect of 0 falls back"), AimedFov(45.f, 0.f, 90.f, 106.f), 106.f);
    TestEqual(TEXT("an aspect that is no number falls back"), AimedFov(45.f, NotANumber, 90.f, 106.f), 106.f);
    float Fov = 90.f;
    for (int32 I = 0; I < 60; ++I) Fov = EaseFov(Fov, 100.f, 1.f / 60.f);
    TestTrue(TEXT("FOV eases most of the way in a second"), Fov > 99.f && Fov <= 100.f);
    TestEqual(TEXT("FOV snaps on a full step"), EaseFov(90.f, 100.f, 0.2f), 100.f);

    // Short fall (section 10): walking, or a fall under 0.3 s; a jump sets 0.5 s, so it never counts.
    FFallTimer Fall;
    TestTrue(TEXT("on the ground counts"), Fall.OnGroundOrShortFall(true));
    Fall.Tick(0.2f, true);
    TestTrue(TEXT("a 0.2 s fall off a ledge still counts"), Fall.OnGroundOrShortFall(false));
    Fall.Tick(0.2f, true);
    TestFalse(TEXT("a 0.4 s fall does not"), Fall.OnGroundOrShortFall(false));
    Fall.Tick(0.1f, false);
    Fall.Jumped();
    TestFalse(TEXT("a jump never counts as a short fall"), Fall.OnGroundOrShortFall(false));
    // Sprint state machine (section 10).
    {
        FSprintInput Ready; Ready.bInCone = Ready.bCanSprint = Ready.bGroundOrShortFall = Ready.bCanContinue = true;
        bool bBegan = false, bCancel = false;
        FSprint S1;
        TestTrue(TEXT("a press in the cone acts and cancels crouch"), S1.Press(Ready, bBegan) && bBegan && S1.bActive);
        FSprintInput Firing = Ready; Firing.bCanSprint = false; Firing.bCanContinue = false;
        FSprint S2;
        TestTrue(TEXT("a press CanSprint rejects is not kept"), S2.Press(Firing, bBegan) && !bBegan && !S2.bActive && !S2.bWants && !S2.bTry);
        TestTrue(TEXT("and does not start when firing stops"), S2.Tick(Ready, bCancel) == FSprint::EEvent::None && !S2.bActive);
        FSprintInput Strafe = Ready; Strafe.bInCone = false;
        FSprint S3;
        TestFalse(TEXT("a press while strafing waits and keeps crouch"), S3.Press(Strafe, bBegan));
        TestTrue(TEXT("still waiting, crouch kept"), S3.Tick(Strafe, bCancel) == FSprint::EEvent::None && !bCancel && S3.bTry);
        TestTrue(TEXT("entering the cone acts: crouch cancelled, sprint on"), S3.Tick(Ready, bCancel) == FSprint::EEvent::Began && bCancel);
        TestTrue(TEXT("leaving the cone ends it"), S3.Tick(Strafe, bCancel) == FSprint::EEvent::Ended);
        FSprint S4;
        S4.Press(Strafe, bBegan);
        S4.Release();
        TestTrue(TEXT("a release drops a waiting press"), S4.Tick(Ready, bCancel) == FSprint::EEvent::None && !S4.bActive);
        FSprintInput Air = Ready; Air.bGroundOrShortFall = false;
        FSprint S5;
        S5.Press(Air, bBegan);
        TestTrue(TEXT("a press in the air is wanted"), !bBegan && S5.bWants);
        TestTrue(TEXT("and begins on landing"), S5.Tick(Ready, bCancel) == FSprint::EEvent::Began);
    }
    // Falling (section 7): fluid friction 0.3 decays the whole velocity; no horizontal cap.
    const FVector V = FallingVelocityAfterFriction(FVector(1000.f, 0.f, -500.f), 0.1f);
    TestTrue(TEXT("falling friction decays lateral and vertical speed"), V.Equals(FVector(970.f, 0.f, -485.f), 0.01f));
    TestTrue(TEXT("no horizontal cap in the air"), NoAirSpeedCap >= TerminalVelocity);
    // Teleports end a tracked fall.
    TestTrue(TEXT("a 2000 uu jump between frames is a teleport"), LooksLikeTeleport(FVector::ZeroVector, FVector(2000, 0, 0), FVector(0, 0, -500), 1.f / 60.f));
    TestFalse(TEXT("a fast fall is not"), LooksLikeTeleport(FVector::ZeroVector, FVector(0, 0, -60), FVector(0, 0, -3600), 1.f / 60.f));
    TestEqual(TEXT("no falling damage without a tracked fall"), FallingDamage(-3000.f, 500.f, false), 0.f);
    // Without -owmaya the walker keeps the earlier estimates (the class defaults; Maya's numbers are applied in BeginPlay only).
    const UCharacterMovementComponent* Defaults = GetDefault<AOpenWillowWalker>()->GetCharacterMovement();
    TestTrue(TEXT("non-Maya defaults unchanged"), Defaults && Defaults->MaxWalkSpeed == LegacyWalkSpeed && Defaults->JumpZVelocity == LegacyJumpZ
        && FMath::IsNearlyEqual(Defaults->GetWalkableFloorAngle(), LegacyFloorAngle, 0.01f) && Defaults->MaxStepHeight == MaxStepHeight);
    TestTrue(TEXT("non-Maya sprint estimates"), LegacySprintSpeed == 650.f && LegacyWalkSpeed == 450.f);
    TestEqual(TEXT("no falling damage above the threshold"), FallingDamage(-1400.f, 500.f), 0.f);
    TestTrue(TEXT("falling damage at twice the threshold is 10 % of the maximum"), FMath::IsNearlyEqual(FallingDamage(-3000.f, 500.f), 50.f, 0.01f));
    return true;
}
#endif
