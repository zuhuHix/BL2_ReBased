#include "Misc/AutomationTest.h"

#if WITH_DEV_AUTOMATION_TESTS
#include "Engine/Engine.h"
#include "Engine/World.h"
#include "GameFramework/PlayerController.h"
#include "GameFramework/Pawn.h"
#include "Camera/PlayerCameraManager.h"
#include "HAL/PlatformTime.h"
#include "HAL/PlatformMemory.h"
#include "Misc/App.h"
#include "Misc/Paths.h"
#include "UnrealClient.h"

// Run in a standalone -game instance. These are engine-input checks; physical
// keyboard/mouse capture and gesture feel still require interactive inspection.
class FOpenWillowViewerCheck : public IAutomationLatentCommand
{
public:
    explicit FOpenWillowViewerCheck(FAutomationTestBase* InTest)
        : Test(InTest), Started(FPlatformTime::Seconds()) {}

    bool Update() override
    {
        const double Now = FPlatformTime::Seconds();
        UWorld* World = nullptr;
        for (const FWorldContext& Context : GEngine->GetWorldContexts())
        {
            if (Context.WorldType == EWorldType::Game)
            {
                World = Context.World();
                break;
            }
        }
        if (Now - Started > 120)
        {
            Test->AddError(TEXT("Viewer check timed out"));
            return true;
        }
        APlayerController* Player = World ? World->GetFirstPlayerController() : nullptr;
        APawn* Pawn = Player ? Player->GetPawn() : nullptr;
        if (!Pawn || !Player->PlayerCameraManager) return false;
        if (Stage == 0)
        {
            if (Now - Started < 15) return false;
            if (!Test->TestTrue(TEXT("Possessed pawn is the view target"), Player->GetViewTarget() == Pawn)) return true;
            Origin = Pawn->GetActorLocation();
            InitialRotation = Player->GetControlRotation();
            Capture(World, TEXT("start"));
            Stage = 1;
            StageStarted = Now;
        }
        else if (Stage == 1)
        {
            if (Now - StageStarted < 3) return false;
            FrameTimes.Add(FApp::GetDeltaTime() * 1000.0);
            Pawn->AddMovementInput(FVector::ForwardVector, 1.0f, true);
            if (++MovementFrames < 90) return false;
            const double Distance = FVector::Dist(Origin, Pawn->GetActorLocation());
            Test->TestTrue(TEXT("Movement input changes pawn position"), Distance > 10);
            Test->AddInfo(FString::Printf(TEXT("Pawn displacement: %.2f cm"), Distance));
            FrameTimes.Sort();
            double TotalMilliseconds = 0;
            for (double FrameTime : FrameTimes) TotalMilliseconds += FrameTime;
            const FPlatformMemoryStats Memory = FPlatformMemory::GetStats();
            Test->AddInfo(FString::Printf(
                TEXT("Viewer diagnostic: map=%s samples=%d mean_frame_ms=%.2f p95_frame_ms=%.2f process_memory_mb=%.1f peak_process_memory_mb=%.1f"),
                *World->GetMapName(), FrameTimes.Num(), TotalMilliseconds / FrameTimes.Num(),
                FrameTimes[FMath::CeilToInt(FrameTimes.Num() * .95) - 1],
                Memory.UsedPhysical / (1024.0 * 1024.0), Memory.PeakUsedPhysical / (1024.0 * 1024.0)));
            Player->SetControlRotation(FRotator(-15, InitialRotation.Yaw + 45, 0));
            Stage = 2;
            StageStarted = Now;
        }
        else if (Stage == 2)
        {
            if (Now - StageStarted < 5) return false;
            Test->TestTrue(TEXT("Camera follows pawn after movement"),
                FVector::Dist(Player->PlayerCameraManager->GetCameraLocation(), Pawn->GetActorLocation()) < 100);
            Test->TestTrue(TEXT("Camera follows control rotation"),
                Player->PlayerCameraManager->GetCameraRotation().Equals(Player->GetControlRotation(), .1));
            Capture(World, TEXT("moved"));
            Stage = 3;
            StageStarted = Now;
        }
        else if (Stage == 3)
        {
            if (Now - StageStarted < 3) return false;
            Pawn->SetActorLocation(Origin + FVector(0, 0, 2000));
            Player->SetControlRotation(FRotator(-35, InitialRotation.Yaw + 180, 0));
            Stage = 4;
            StageStarted = Now;
        }
        else if (Stage == 4)
        {
            if (Now - StageStarted < 5) return false;
            Capture(World, TEXT("overview"));
            Stage = 5;
            StageStarted = Now;
        }
        else if (Stage == 5 && Now - StageStarted > 3)
        {
            return true;
        }
        return false;
    }

private:
    void Capture(UWorld* World, const TCHAR* View)
    {
        const FString Name = FPaths::Combine(FPaths::ScreenShotDir(), World->GetMapName() + TEXT("_") + View);
        FScreenshotRequest::RequestScreenshot(Name, false, true);
        Test->AddInfo(TEXT("Requested screenshot: ") + Name);
    }
    FAutomationTestBase* Test;
    double Started;
    double StageStarted = 0;
    int32 Stage = 0;
    int32 MovementFrames = 0;
    // A short sample during the existing correctness run, not a benchmark.
    TArray<double> FrameTimes;
    FVector Origin;
    FRotator InitialRotation;
};

IMPLEMENT_SIMPLE_AUTOMATION_TEST(FOpenWillowViewerTest, "OpenWillow.Viewer",
    EAutomationTestFlags::ClientContext | EAutomationTestFlags::ProductFilter)

bool FOpenWillowViewerTest::RunTest(const FString& Parameters)
{
    ADD_LATENT_AUTOMATION_COMMAND(FOpenWillowViewerCheck(this));
    return true;
}
#endif
