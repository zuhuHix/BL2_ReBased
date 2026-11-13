// AI-assisted (Claude Code). Synthetic checks of the range bullymongs' leap and throw rules (OpenWillowLeapThrow.h): the arc solver against the
// worked table of docs/verification/NATIVE_AI_LEAP_THROW.md section 3.2 and 4.2, the lead, the attack draw's odds and the leap's landing
// volume. Inputs are invented; the expected values are the note's own (its table rounds seconds to 0.01 and heights to 1 uu). This pins the
// host's arithmetic, not the game: every rule here is UNVERIFIED in the running game.
// Run: UnrealEditor.exe <uproject> -game -nullrhi -ExecCmds="Automation RunTests OpenWillow.LeapThrow; Quit"
#include "Misc/AutomationTest.h"

#if WITH_DEV_AUTOMATION_TESTS
#include "OpenWillowLeapThrow.h"

IMPLEMENT_SIMPLE_AUTOMATION_TEST(FOpenWillowLeapThrowTest, "OpenWillow.LeapThrow",
    EAutomationTestFlags::ClientContext | EAutomationTestFlags::EngineFilter)

bool FOpenWillowLeapThrowTest::RunTest(const FString&)
{
    // The note's worked table: flat ground, no lead. Seconds to 0.01 and the peak to 1 uu, as the note prints them.
    struct FRow { const TCHAR* Name; float Speed, AnglePct, Angle, Distance, Flight, Peak; };
    const FRow Rows[] = {
        {TEXT("adult leap"), 2500.f, 0.4f, 36.f, 1024.f, 0.51f, 186.f}, {TEXT("adult leap"), 2500.f, 0.4f, 36.f, 1600.f, 0.79f, 291.f},
        {TEXT("adult leap"), 2500.f, 0.4f, 36.f, 2048.f, 1.01f, 372.f},
        {TEXT("regular leap"), 2000.f, 0.4f, 36.f, 1024.f, 0.63f, 186.f}, {TEXT("regular leap"), 2000.f, 0.4f, 36.f, 1600.f, 0.99f, 291.f},
        {TEXT("regular leap"), 2000.f, 0.4f, 36.f, 2048.f, 1.27f, 372.f},
        {TEXT("baby leap"), 2500.f, 0.45f, 40.5f, 1024.f, 0.54f, 219.f}, {TEXT("baby leap"), 2500.f, 0.45f, 40.5f, 1600.f, 0.84f, 342.f},
        {TEXT("baby leap"), 2500.f, 0.45f, 40.5f, 2048.f, 1.08f, 437.f},
        {TEXT("rock"), 2000.f, 0.3f, 27.f, 1024.f, 0.57f, 130.f}, {TEXT("rock"), 2000.f, 0.3f, 27.f, 1600.f, 0.90f, 204.f},
        {TEXT("rock"), 2000.f, 0.3f, 27.f, 2048.f, 1.15f, 261.f}, {TEXT("rock"), 2000.f, 0.3f, 27.f, 3000.f, 1.68f, 382.f},
    };
    const FVector Start(100.f, -200.f, 50.f);
    for (const FRow& Row : Rows)
    {
        const FVector Aim = Start + FVector(Row.Distance * 0.6f, Row.Distance * 0.8f, 0.f);
        const FOpenWillowArc Arc = OpenWillowSolveArc(Start, Aim, FVector::ZeroVector, Row.Speed, Row.AnglePct, 0.f);
        const FString What = FString::Printf(TEXT("%s at %.0f uu"), Row.Name, Row.Distance);
        TestTrue(What + TEXT(": solved"), Arc.bValid);
        TestTrue(What + TEXT(": launch angle"), FMath::IsNearlyEqual(Arc.AngleDegrees, Row.Angle, 0.01f));
        TestTrue(What + FString::Printf(TEXT(": flight %.4f s, table %.2f"), Arc.FlightTime, Row.Flight), FMath::Abs(Arc.FlightTime - Row.Flight) <= 0.005f);
        TestTrue(What + FString::Printf(TEXT(": peak %.1f uu, table %.0f"), Arc.Peak(), Row.Peak), FMath::Abs(Arc.Peak() - Row.Peak) <= 0.5f);
        // On flat ground the peak is distance x tan(angle) / 4 whatever the speed (note 3.2).
        TestTrue(What + TEXT(": peak = d tan(a) / 4"),
            FMath::IsNearlyEqual(Arc.Peak(), Row.Distance * FMath::Tan(FMath::DegreesToRadians(Row.Angle)) / 4.f, 0.05f));
        TestTrue(What + TEXT(": lands on the aim point"), FVector::Dist(Arc.At(Arc.FlightTime), Aim) < 0.05f);
        TestTrue(What + TEXT(": gravity scale is -g / 1000"), FMath::IsNearlyEqual(Arc.GravityScale(), -Arc.Gravity / 1000.f));
        TestTrue(What + TEXT(": fixed speed"), FMath::IsNearlyEqual(float(Arc.Velocity.Size()), Row.Speed, 0.01f));
    }

    // A raised aim point: the angle is the line's elevation plus the fraction of what is left to straight up, and the arc still lands on it.
    {
        const FVector Aim = Start + FVector(1500.f, 0.f, 300.f);
        const FOpenWillowArc Arc = OpenWillowSolveArc(Start, Aim, FVector::ZeroVector, 2500.f, 0.4f, 92.f);
        const float Elevation = FMath::RadiansToDegrees(FMath::Atan2(300.f, 1500.f));
        TestTrue(TEXT("raised aim: angle"), FMath::IsNearlyEqual(Arc.AngleDegrees, Elevation + 0.4f * (90.f - Elevation), 0.01f));
        TestTrue(TEXT("raised aim: lands on it"), FVector::Dist(Arc.At(Arc.FlightTime), Aim) < 0.05f);
        TestTrue(TEXT("raised aim: no lead without a moving target"), Arc.Lead == 0.f && Arc.D1 == Arc.D0);
    }

    // The lead (note 3.2 step 4): the arc meets a point moving with the target; a lead longer than the cap is cut to it; a cap of 0 is none;
    // no positive root (a target running away faster than the horizontal speed) is none.
    {
        const FVector Aim = Start + FVector(1000.f, 0.f, 0.f);
        const FVector Sideways(0.f, 200.f, 0.f);
        const FOpenWillowArc Free = OpenWillowSolveArc(Start, Aim, Sideways, 2000.f, 0.3f, 480.f);
        const FVector Meets = Aim + Sideways * Free.FlightTime;
        TestTrue(TEXT("lead under its cap: the arc meets the moving point"), FVector::Dist(Free.At(Free.FlightTime), Meets) < 0.5f);
        TestTrue(TEXT("lead under its cap: its length"), FMath::IsNearlyEqual(Free.Lead, 200.f * Free.FlightTime, 0.5f) && Free.Lead < 480.f);
        const FOpenWillowArc Capped = OpenWillowSolveArc(Start, Aim, FVector(0.f, 600.f, 0.f), 2500.f, 0.4f, 92.f);
        TestTrue(TEXT("lead over its cap is cut to it"), FMath::IsNearlyEqual(Capped.Lead, 92.f, 0.01f));
        TestTrue(TEXT("a capped lead still lands on its final aim point"), FVector::Dist(Capped.At(Capped.FlightTime), Capped.Aim) < 0.05f);
        const FOpenWillowArc NoCap = OpenWillowSolveArc(Start, Aim, Sideways, 2000.f, 0.4f, 0.f);
        TestTrue(TEXT("cap 0: no lead (the regular's leap)"), NoCap.Lead == 0.f && FVector::Dist(NoCap.Aim, Aim) < 0.01f);
        const FOpenWillowArc Away = OpenWillowSolveArc(Start, Aim, FVector(5000.f, 0.f, 0.f), 2000.f, 0.3f, 480.f);
        TestTrue(TEXT("no positive root: no lead"), Away.bValid && Away.Lead == 0.f);
        TestFalse(TEXT("an aim point straight above is not solved"), OpenWillowSolveArc(Start, Start + FVector(0.f, 0.f, 500.f), FVector::ZeroVector, 2000.f, 0.3f, 0.f).bValid);
    }

    // The draw (note 2.2): adult 1.0 : 0.25 : 0.25, regular 1.0 : 0.5; a child that cannot run drops out; nothing runnable is None.
    {
        const FOpenWillowKindAttacks* Adult = OpenWillowFindKindAttacks(TEXT("bullymong_adult"));
        const FOpenWillowKindAttacks* Regular = OpenWillowFindKindAttacks(TEXT("bullymong"));
        const FOpenWillowKindAttacks* Baby = OpenWillowFindKindAttacks(TEXT("bullymong_baby"));
        const FOpenWillowKindAttacks* Slinger = OpenWillowFindKindAttacks(TEXT("bullymong_slinger"));
        TestTrue(TEXT("rows for the four bullymong kinds, none for the psycho"), Adult && Regular && Baby && Slinger && !OpenWillowFindKindAttacks(TEXT("psycho")));
        if (!Adult || !Regular || !Baby || !Slinger) return false;
        auto Odds = [](const FOpenWillowKindAttacks& Row, bool bMelee, bool bLeap, bool bThrow, float& OutMelee, float& OutLeap, float& OutThrow, int32& OutNone)
        {
            FRandomStream Stream(20261113);
            constexpr int32 Draws = 60000;
            int32 Melee = 0, Leap = 0, Throw = 0;
            OutNone = 0;
            for (int32 Index = 0; Index < Draws; ++Index)
            {
                switch (OpenWillowDrawAttack(Row, bMelee, bLeap, bThrow, Stream))
                {
                case EOpenWillowAttack::Melee: ++Melee; break;
                case EOpenWillowAttack::Leap: ++Leap; break;
                case EOpenWillowAttack::Throw: ++Throw; break;
                default: ++OutNone; break;
                }
            }
            OutMelee = float(Melee) / Draws;
            OutLeap = float(Leap) / Draws;
            OutThrow = float(Throw) / Draws;
        };
        float M = 0.f, L = 0.f, T = 0.f;
        int32 None = 0;
        Odds(*Adult, true, true, true, M, L, T, None);
        TestTrue(FString::Printf(TEXT("adult, all can run: 4 : 1 : 1 (%.3f %.3f %.3f)"), M, L, T),
            FMath::Abs(M - 4.f / 6.f) < 0.01f && FMath::Abs(L - 1.f / 6.f) < 0.01f && FMath::Abs(T - 1.f / 6.f) < 0.01f);
        Odds(*Adult, true, false, true, M, L, T, None);
        TestTrue(FString::Printf(TEXT("adult, no leap: 4 : 1 (%.3f %.3f %.3f)"), M, L, T), FMath::Abs(M - 0.8f) < 0.01f && L == 0.f && FMath::Abs(T - 0.2f) < 0.01f);
        Odds(*Regular, true, true, false, M, L, T, None);
        TestTrue(FString::Printf(TEXT("regular: 2 : 1 (%.3f %.3f %.3f)"), M, L, T), FMath::Abs(M - 2.f / 3.f) < 0.01f && FMath::Abs(L - 1.f / 3.f) < 0.01f && T == 0.f);
        Odds(*Regular, true, true, true, M, L, T, None);
        TestTrue(TEXT("regular: no throw in its random node"), T == 0.f);
        Odds(*Baby, false, true, false, M, L, T, None);
        TestTrue(TEXT("baby outside its grab band: always the leap"), L == 1.f);
        Odds(*Baby, false, false, false, M, L, T, None);
        TestTrue(TEXT("nothing can run: None"), None == 60000);
        Odds(*Slinger, false, true, true, M, L, T, None);
        TestTrue(TEXT("slinger: the throw only"), T == 1.f);
        // The rows themselves (the full rows are held to tools/check_range.py by tests/range_tools_test.py).
        TestTrue(TEXT("regular: leap 1024 to 2048 at 2000, no lead, no resource, no throw"), Regular->LeapMin == 1024.f && Regular->LeapMax == 2048.f
            && Regular->LeapSpeed == 2000.f && Regular->LeapLeadCap == 0.f && !Regular->bLeapResource && Regular->ThrowMax == 0.f && Regular->CloseBand == 320.f);
        TestTrue(TEXT("adult: leap at 2500 with the resource, throw 512 to 3072 in front, close band 384"), Adult->LeapSpeed == 2500.f && Adult->bLeapResource
            && Adult->ThrowMin == 512.f && Adult->ThrowMax == 3072.f && Adult->bThrowNeedsFront && Adult->bThrowResource && Adult->CloseBand == 384.f);
        TestTrue(TEXT("adult throw clip: release 1.064 s, clip 1.733 s"), Adult->ThrowClips == 1 && Adult->Clip[0].Releases == 1
            && FMath::IsNearlyEqual(Adult->Clip[0].Release[0], 1.064f) && FMath::IsNearlyEqual(Adult->Clip[0].ClipSeconds, 1.733f));
        TestTrue(TEXT("slinger: no leap, the two-rock clip releases at 0.68 and 1.66 s"), Slinger->LeapMax == 0.f && Slinger->Clip[2].Releases == 2
            && FMath::IsNearlyEqual(Slinger->Clip[2].Release[1], 1.66f));
    }

    // The landing volume (note 3.3): a circle of 320 around the point 200 ahead of the leaper; the vertical stand-in 192 either way.
    {
        const FVector Leaper(0.f, 0.f, 100.f);
        const FVector Facing(1.f, 0.f, 0.f);
        TestTrue(TEXT("landing: on the leaper"), OpenWillowLeapLandingHits(Leaper, Facing, Leaper));
        TestTrue(TEXT("landing: 520 ahead (the far edge)"), OpenWillowLeapLandingHits(Leaper, Facing, Leaper + FVector(520.f, 0.f, 0.f)));
        TestFalse(TEXT("landing: 521 ahead"), OpenWillowLeapLandingHits(Leaper, Facing, Leaper + FVector(521.f, 0.f, 0.f)));
        TestTrue(TEXT("landing: 120 behind (the near edge)"), OpenWillowLeapLandingHits(Leaper, Facing, Leaper + FVector(-120.f, 0.f, 0.f)));
        TestFalse(TEXT("landing: 121 behind"), OpenWillowLeapLandingHits(Leaper, Facing, Leaper + FVector(-121.f, 0.f, 0.f)));
        TestTrue(TEXT("landing: 192 above"), OpenWillowLeapLandingHits(Leaper, Facing, Leaper + FVector(0.f, 0.f, 192.f)));
        TestFalse(TEXT("landing: 193 below"), OpenWillowLeapLandingHits(Leaper, Facing, Leaper + FVector(0.f, 0.f, -193.f)));
    }
    return true;
}
#endif
