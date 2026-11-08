// AI-assisted (Claude). Synthetic checks of the mission page's view rule (OpenWillowFocusCam): the aim puts the point where it was asked, the zoom,
// and the local file's checks. Inputs are invented, so this pins the host's arithmetic, not the game's camera.
// Run: UnrealEditor.exe <uproject> -game -nullrhi -ExecCmds="Automation RunTests OpenWillow.FocusCam; Quit"
#include "Misc/AutomationTest.h"

#if WITH_DEV_AUTOMATION_TESTS
#include "OpenWillowFocusCam.h"
#include "HAL/FileManager.h"
#include "Misc/FileHelper.h"
#include "Misc/Guid.h"
#include "Misc/Paths.h"

IMPLEMENT_SIMPLE_AUTOMATION_TEST(FOpenWillowFocusCamTest, "OpenWillow.FocusCam",
    EAutomationTestFlags::ClientContext | EAutomationTestFlags::EngineFilter)

bool FOpenWillowFocusCamTest::RunTest(const FString&)
{
    using namespace OpenWillowFocusCam;
    TestTrue(TEXT("zoom 1 keeps the FOV"), FMath::IsNearlyEqual(ZoomedFov(90., 1.), 90., 1e-6));
    TestTrue(TEXT("zoom 2 halves the half-angle tangent"), FMath::IsNearlyEqual(ZoomedFov(90., 2.), FMath::RadiansToDegrees(2. * FMath::Atan(0.5)), 1e-6));
    TestTrue(TEXT("a zoom under 1 never widens"), FMath::IsNearlyEqual(ZoomedFov(70., 0.5), 70., 1e-6));

    // The centre of the screen is the plain look-at.
    {
        const FVector Eye(0, 0, 100), Point(300, 200, 40);
        FRotator View;
        TestTrue(TEXT("centre solves"), Aim(Eye, Point, FVector2D::ZeroVector, 80., 16. / 9., View));
        TestTrue(TEXT("centre is the look-at"), View.Equals((Point - Eye).Rotation(), 1e-3));
        TestTrue(TEXT("no roll"), FMath::IsNearlyZero(View.Roll));
    }
    // Aim then Project lands on the asked point, over eyes above and below, behind the origin, and several FOVs, aspects and screen points.
    struct FCase { FVector Eye, Point; FVector2D Screen; double Fov, Aspect; };
    const FCase Cases[] = {
        { FVector(0, 0, 100), FVector(110, 0, 60), FVector2D(0.37, 0.32), 60., 16. / 9. },
        { FVector(500, -200, 0), FVector(380, -150, 40), FVector2D(-0.5, -0.4), 90., 4. / 3. },
        { FVector(0, 0, 0), FVector(-50, -400, -30), FVector2D(0.8, 0.8), 40., 21. / 9. },
        { FVector(10, 10, 10), FVector(10, 200, 10), FVector2D(0.2, -0.7), 100., 1. },
    };
    for (int32 I = 0; I < UE_ARRAY_COUNT(Cases); ++I)
    {
        const FCase& C = Cases[I];
        FRotator View;
        FVector2D Landed;
        const bool bSolved = Aim(C.Eye, C.Point, C.Screen, C.Fov, C.Aspect, View);
        const bool bFront = Project(C.Eye, View, C.Fov, C.Aspect, C.Point, Landed);
        TestTrue(FString::Printf(TEXT("case %d solves"), I), bSolved);
        TestTrue(FString::Printf(TEXT("case %d in front"), I), bFront);
        TestTrue(FString::Printf(TEXT("case %d lands (%.4f, %.4f) on (%.2f, %.2f)"), I, Landed.X, Landed.Y, C.Screen.X, C.Screen.Y), Landed.Equals(C.Screen, 1e-4));
        TestTrue(FString::Printf(TEXT("case %d has no roll"), I), FMath::IsNearlyZero(View.Roll));
    }
    // Straight above the eye cannot be put low on the screen with no roll: no solution, and the look-at is given.
    {
        FRotator View;
        TestFalse(TEXT("no solution straight above for a low point"), Aim(FVector::ZeroVector, FVector(0, 0, 100), FVector2D(0., -0.9), 90., 1., View));
        TestTrue(TEXT("its fallback is the look-at"), View.Equals(FVector(0, 0, 1).Rotation(), 1e-3));
        FVector2D Landed;
        TestFalse(TEXT("a point behind does not project"), Project(FVector::ZeroVector, FRotator::ZeroRotator, 90., 1., FVector(-10, 0, 0), Landed));
    }

    // The local file: a good one reads, each bad field is refused, a missing file is reported.
    const FString Dir = FPaths::Combine(FPaths::AutomationTransientDir(), FString::Printf(TEXT("focuscam_%s"), *FGuid::NewGuid().ToString()));
    TestTrue(TEXT("temp folder made"), IFileManager::Get().MakeDirectory(*Dir, true));
    // Each file must really be written, or a refused Load below would pass for the wrong reason.
    const auto Write = [this, &Dir](const TCHAR* Name, const TCHAR* Json)
    {
        const FString Path = FPaths::Combine(Dir, Name);
        TestTrue(FString::Printf(TEXT("wrote %s"), Name), FFileHelper::SaveStringToFile(Json, *Path));
        return Path;
    };
    FParams Params;
    FString Error;
    TestTrue(TEXT("good file reads"), Load(Write(TEXT("good.json"), TEXT("{\"bone\":\"B\",\"screen\":[0.25,-0.5],\"zoom\":1.5,\"source\":\"invented\"}")), Params, Error));
    TestTrue(TEXT("bone"), Params.Bone == FName(TEXT("B")));
    TestTrue(TEXT("screen"), Params.Screen.Equals(FVector2D(0.25, -0.5), 1e-9));
    TestTrue(TEXT("zoom"), FMath::IsNearlyEqual(Params.Zoom, 1.5));
    TestEqual(TEXT("source"), Params.Source, FString(TEXT("invented")));
    TestFalse(TEXT("missing file"), Load(FPaths::Combine(Dir, TEXT("none.json")), Params, Error));
    TestEqual(TEXT("missing says so"), Error, FString(TEXT("missing")));
    TestFalse(TEXT("not JSON"), Load(Write(TEXT("text.json"), TEXT("bone")), Params, Error));
    TestFalse(TEXT("no bone"), Load(Write(TEXT("nobone.json"), TEXT("{\"screen\":[0,0],\"zoom\":1.5}")), Params, Error));
    TestFalse(TEXT("one screen number"), Load(Write(TEXT("short.json"), TEXT("{\"bone\":\"B\",\"screen\":[0],\"zoom\":1.5}")), Params, Error));
    TestFalse(TEXT("screen off the view"), Load(Write(TEXT("off.json"), TEXT("{\"bone\":\"B\",\"screen\":[1.2,0],\"zoom\":1.5}")), Params, Error));
    TestFalse(TEXT("zoom that widens"), Load(Write(TEXT("wide.json"), TEXT("{\"bone\":\"B\",\"screen\":[0,0],\"zoom\":0.8}")), Params, Error));
    TestFalse(TEXT("no zoom"), Load(Write(TEXT("nozoom.json"), TEXT("{\"bone\":\"B\",\"screen\":[0,0]}")), Params, Error));
    IFileManager::Get().DeleteDirectory(*Dir, false, true);
    return true;
}
#endif
