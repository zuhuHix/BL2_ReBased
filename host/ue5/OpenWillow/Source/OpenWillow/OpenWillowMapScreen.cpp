#include "OpenWillowMapScreen.h"
#include "OpenWillowMayaHUD.h"
#include "OpenWillowMinimap.h"
#include "OpenWillowWalker.h"
#include "Engine/World.h"
#include "GameFramework/PlayerController.h"
#include "Kismet/GameplayStatics.h"
#include "Misc/CommandLine.h"
#include "UnrealClient.h"

FString OpenWillowMapScreen::StateJson(const AOpenWillowWalker& Maya, const APlayerController& Player)
{
    const FString Minimap = OpenWillowMinimap::StateField(Maya, Player);
    FString Name = UGameplayStatics::GetCurrentLevelName(&Maya, true);
    Name.RemoveFromEnd(TEXT("_P"));
    Name = Name.ToUpper().ReplaceCharWithEscapedChar();
    return FString::Printf(TEXT("{\"minimap\":%s,\"name\":\"%s\"}"), Minimap.IsEmpty() ? TEXT("null") : *Minimap, *Name);
}

namespace
{
    int32 ShotStep = 0;
    float ShotAt = 0.f;

    void Shot(int32 Index, const TCHAR* Name, float Now)
    {
        const FString File = FString::Printf(TEXT("OWMap_%d_%s.png"), Index, Name);
        FScreenshotRequest::RequestScreenshot(File, true, false);
        UE_LOG(LogTemp, Display, TEXT("OWMAPSHOTS shot %s t=%.3f"), *File, Now);
    }
}

void OpenWillowMapScreen::TickShots(AOpenWillowMayaHUD& Hud, AOpenWillowWalker& Maya, APlayerController& Player)
{
    static const bool bShots = FParse::Param(FCommandLine::Get(), TEXT("owmapshots"));
    if (!bShots || ShotStep < 0) return;
    // The movie HUD settles a few seconds after it is ready; each step then waits for the page (the map movie loads after
    // the StatusMenu movie).
    constexpr float Start = 10.f;
    static const float Waits[] = {4.f, 1.5f, 1.5f, 1.5f, 1.5f, 1.5f, 6.f, 5.f};
    const float Now = Maya.GetWorld()->GetTimeSeconds();
    if (Now < Start) return;
    if (ShotStep > 0 && ShotStep <= int32(UE_ARRAY_COUNT(Waits)) && Now < ShotAt + Waits[ShotStep - 1]) return;
    switch (ShotStep)
    {
    case 0:
        UE_LOG(LogTemp, Display, TEXT("OWMAPSHOTS open=%d"), Hud.ToggleMap() ? 1 : 0);
        break;
    case 1:
        Shot(1, TEXT("Open"), Now);
        // Turn Maya's view 90 degrees: the arrow turns, the map stays north up.
        Player.SetControlRotation(Player.GetControlRotation() + FRotator(0.f, 90.f, 0.f));
        break;
    case 2: Shot(2, TEXT("Turned"), Now); Hud.SendPageKey(TEXT("PageDown")); Hud.SendPageKey(TEXT("PageDown")); break;
    case 3: Shot(3, TEXT("ZoomedIn"), Now); Hud.SendPageKey(TEXT("ArrowLeft")); Hud.SendPageKey(TEXT("ArrowLeft")); break;
    case 4: Shot(4, TEXT("Panned"), Now); Hud.SendPageKey(TEXT("q")); break;
    case 5: Shot(5, TEXT("Legend"), Now); Hud.SendPageKey(TEXT("Escape")); break;
    case 6:
        Shot(6, TEXT("Closed"), Now);
        UE_LOG(LogTemp, Display, TEXT("OWMAPSHOTS closed=%d"), Hud.IsSkillsOpen() ? 0 : 1);
        Hud.ToggleInventory();
        break;
    case 7: Hud.SendPageKey(TEXT("2")); break; // the Inventory's Map tab key
    case 8:
        Shot(7, TEXT("FromInventoryKey2"), Now);
        UE_LOG(LogTemp, Display, TEXT("OWMAPSHOTS map_from_inventory=%d"), Hud.IsMapOpen() ? 1 : 0);
        break;
    default:
        UE_LOG(LogTemp, Display, TEXT("OWMAPSHOTS done"));
        Player.ConsoleCommand(TEXT("quit"));
        ShotStep = -1;
        return;
    }
    ShotAt = Now;
    ++ShotStep;
}
