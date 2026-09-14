#include "Misc/AutomationTest.h"

#if WITH_DEV_AUTOMATION_TESTS
#include "Engine/Engine.h"
#include "Engine/World.h"
#include "GameFramework/PlayerController.h"
#include "GameFramework/Pawn.h"
#include "HAL/PlatformMemory.h"
#include "HAL/PlatformTime.h"
#include "Misc/App.h"
#include "RenderTimer.h"
#include "RHI.h"
#include "RHIGlobals.h"
#include "RHIStats.h"
#include "DynamicRHI.h"

// Run in a standalone -game instance. Records the per-thread frame times that
// `stat unit` displays, plus RHI draw-call counts, from a fixed viewpoint
// after a settle period. A diagnostic sample on whatever hardware runs it,
// not a controlled benchmark; nothing here changes renderer settings.
class FOpenWillowProfileCheck : public IAutomationLatentCommand
{
public:
    explicit FOpenWillowProfileCheck(FAutomationTestBase* InTest)
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
        if (Now - Started > 240)
        {
            Test->AddError(TEXT("Profile check timed out"));
            return true;
        }
        APlayerController* Player = World ? World->GetFirstPlayerController() : nullptr;
        APawn* Pawn = Player ? Player->GetPawn() : nullptr;
        if (!Pawn) return false;

        if (Stage == 0)
        {
            // Let streaming, shader compilation and PSO warm-up settle first.
            if (Now - Started < 45) return false;
            Stage = 1;
            StageStarted = Now;
        }
        else if (Stage == 1)
        {
            Sample(TEXT("start"), World);
            if (Frames.Num() < 300) return false;
            Report(TEXT("start view"), World);
            Frames.Reset();
            // A second sample after turning 180 degrees so one viewpoint does
            // not dominate the record.
            Player->SetControlRotation(FRotator(-10, Player->GetControlRotation().Yaw + 180, 0));
            Stage = 2;
            StageStarted = Now;
        }
        else if (Stage == 2)
        {
            if (Now - StageStarted < 5) return false;
            Sample(TEXT("turned"), World);
            if (Frames.Num() < 300) return false;
            Report(TEXT("turned view"), World);
            // One-frame GPU pass breakdown; the RHI writes it to the log.
            GEngine->Exec(World, TEXT("ProfileGPU"));
            Stage = 3;
            StageStarted = Now;
        }
        else if (Stage == 3 && Now - StageStarted > 5)
        {
            return true;
        }
        return false;
    }

private:
    struct FFrame
    {
        double FrameMs, GameMs, RenderMs, RHIMs, GPUMs;
        int32 DrawCalls, Primitives;
    };

    void Sample(const TCHAR*, UWorld*)
    {
        FFrame Frame;
        Frame.FrameMs = FApp::GetDeltaTime() * 1000.0;
        Frame.GameMs = FPlatformTime::ToMilliseconds(GGameThreadTime);
        Frame.RenderMs = FPlatformTime::ToMilliseconds(GRenderThreadTime);
        Frame.RHIMs = FPlatformTime::ToMilliseconds(GRHIThreadTime);
        Frame.GPUMs = FPlatformTime::ToMilliseconds(RHIGetGPUFrameCycles());
        Frame.DrawCalls = GNumDrawCallsRHI[0];
        Frame.Primitives = GNumPrimitivesDrawnRHI[0];
        Frames.Add(Frame);
    }

    static double Mean(const TArray<double>& Values)
    {
        double Total = 0;
        for (double V : Values) Total += V;
        return Values.Num() ? Total / Values.Num() : 0;
    }

    static double P95(TArray<double> Values)
    {
        Values.Sort();
        return Values.Num() ? Values[FMath::CeilToInt(Values.Num() * .95) - 1] : 0;
    }

    void Report(const TCHAR* View, UWorld* World)
    {
        TArray<double> FrameMs, GameMs, RenderMs, RHIMs, GPUMs, Draws, Prims;
        for (const FFrame& F : Frames)
        {
            FrameMs.Add(F.FrameMs); GameMs.Add(F.GameMs); RenderMs.Add(F.RenderMs);
            RHIMs.Add(F.RHIMs); GPUMs.Add(F.GPUMs); Draws.Add(F.DrawCalls); Prims.Add(F.Primitives);
        }
        const FPlatformMemoryStats Memory = FPlatformMemory::GetStats();
        Test->AddInfo(FString::Printf(
            TEXT("Profile %s: map=%s rhi=%s adapter=%s samples=%d frame_ms=%.1f/%.1f fps=%.1f game_ms=%.1f/%.1f render_ms=%.1f/%.1f rhi_ms=%.1f/%.1f gpu_ms=%.1f/%.1f draw_calls=%.0f primitives=%.0f process_memory_mb=%.0f"),
            View, *World->GetMapName(), GDynamicRHI ? GDynamicRHI->GetName() : TEXT("?"), *GRHIAdapterName, Frames.Num(),
            Mean(FrameMs), P95(FrameMs), 1000.0 / FMath::Max(Mean(FrameMs), 0.001),
            Mean(GameMs), P95(GameMs), Mean(RenderMs), P95(RenderMs), Mean(RHIMs), P95(RHIMs),
            Mean(GPUMs), P95(GPUMs), Mean(Draws), Mean(Prims), Memory.UsedPhysical / (1024.0 * 1024.0)));
    }

    FAutomationTestBase* Test;
    double Started;
    double StageStarted = 0;
    int32 Stage = 0;
    TArray<FFrame> Frames;
};

IMPLEMENT_SIMPLE_AUTOMATION_TEST(FOpenWillowProfileTest, "OpenWillow.Profile",
    EAutomationTestFlags::ClientContext | EAutomationTestFlags::ProductFilter)

bool FOpenWillowProfileTest::RunTest(const FString& Parameters)
{
    ADD_LATENT_AUTOMATION_COMMAND(FOpenWillowProfileCheck(this));
    return true;
}
#endif
