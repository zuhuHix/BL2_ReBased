#include "OpenWillowStatusShots.h"
#include "OpenWillowMayaHUD.h"
#include "Engine/World.h"
#include "GameFramework/PlayerController.h"
#include "Misc/CommandLine.h"
#include "UnrealClient.h"

void OpenWillowStatusShots::Tick(AOpenWillowMayaHUD& Hud, UWorld& World)
{
    static FString Spec;
    static bool bRead = false, bDone = false;
    static int32 Step = 0;
    static float At = 0.f;
    if (!bRead) { bRead = true; FParse::Value(FCommandLine::Get(), TEXT("owstatusshots="), Spec, false); } // keep the comma
    if (Spec.IsEmpty() || bDone) return;
    FString Page = Spec, Name = TEXT("shot");
    Spec.Split(TEXT(","), &Page, &Name);
    const float Now = World.GetTimeSeconds();
    if (Now < 10.f || Now < At) return;
    if (Step == 0)
    {
        const bool bOpen = Page == TEXT("missions") ? Hud.ToggleMissions() : Page == TEXT("map") ? Hud.ToggleMap()
            : Page == TEXT("inventory") ? Hud.ToggleInventory() : (Hud.ToggleSkills(), Hud.IsSkillsOpen());
        UE_LOG(LogTemp, Display, TEXT("OWSTATUSSHOTS open %s=%d"), *Page, bOpen ? 1 : 0);
        At = Now + 4.f;
        Step = 1;
        return;
    }
    if (Step == 1)
    {
        const FString File = FString::Printf(TEXT("OWStatus_%s_%s.png"), *Page, *Name);
        FScreenshotRequest::RequestScreenshot(File, true, false);
        UE_LOG(LogTemp, Display, TEXT("OWSTATUSSHOTS shot %s"), *File);
        At = Now + 1.f;
        Step = 2;
        return;
    }
    UE_LOG(LogTemp, Display, TEXT("OWSTATUSSHOTS done"));
    if (APlayerController* Player = World.GetFirstPlayerController()) Player->ConsoleCommand(TEXT("quit"));
    bDone = true;
}
