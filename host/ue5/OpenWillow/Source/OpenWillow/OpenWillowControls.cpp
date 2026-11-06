#include "OpenWillowControls.h"
#include "OpenWillowInventoryWidget.h"
#include "OpenWillowMayaHUD.h"
#include "OpenWillowWalker.h"
#include "Engine/World.h"
#include "GameFramework/PlayerController.h"
#include "InputCoreTypes.h"
#include "Misc/CommandLine.h"

void AOpenWillowWalker::ToggleMapOrMissionsPage(bool bMap)
{
    // The same steps as ToggleSkills: the trigger and the aim end, and a legacy inventory widget gives way to the page.
    APlayerController* PC = Cast<APlayerController>(Controller);
    if (!bMayaActive || !PC) return;
    bFireHeld = false;
    AimReleased();
    if (InventoryScreen && InventoryScreen->IsInViewport())
    {
        InventoryScreen->RemoveFromParent();
        PC->SetInputMode(FInputModeGameOnly());
        PC->SetShowMouseCursor(false);
    }
    AOpenWillowMayaHUD* HUD = Cast<AOpenWillowMayaHUD>(PC->GetHUD());
    // False: the page is not served in this run, or a vending shop refused it (OpenStatusMenu). The log says the key did nothing.
    if (!HUD || !(bMap ? HUD->ToggleMap() : HUD->ToggleMissions()))
        UE_LOG(LogTemp, Display, TEXT("OWCONTROLS %s page did not open"), bMap ? TEXT("map") : TEXT("missions"));
}
void AOpenWillowWalker::ToggleMapPage() { ToggleMapOrMissionsPage(true); }
void AOpenWillowWalker::ToggleMissionsPage() { ToggleMapOrMissionsPage(false); }

namespace
{
    int32 TestStep = 0, Passed = 0, Failed = 0;
    float NextAt = 0.f;
    FKey HeldKey;
    bool bKeyHeld = false;

    void Check(const TCHAR* Name, bool bOk, const FString& Detail)
    {
        ++(bOk ? Passed : Failed);
        UE_LOG(LogTemp, Display, TEXT("OWCONTROLS %s %s %s"), Name, bOk ? TEXT("PASS") : TEXT("FAIL"), *Detail);
    }
    void Press(APlayerController& Player, const FKey& Key)
    {
        Player.InputKey(FInputKeyEventArgs::CreateSimulated(Key, IE_Pressed, 1.f));
        HeldKey = Key;
        bKeyHeld = true;
    }
}

void OpenWillowControls::TickTest(AOpenWillowMayaHUD& Hud, AOpenWillowWalker& Maya, APlayerController& Player)
{
    static const bool bTest = FParse::Param(FCommandLine::Get(), TEXT("owcontrolstest"));
    if (!bTest || TestStep < 0 || !Maya.IsMayaActive() || !Maya.GetWorld()) return;
    if (bKeyHeld)
    {
        Player.InputKey(FInputKeyEventArgs::CreateSimulated(HeldKey, IE_Released, 0.f));
        bKeyHeld = false;
    }
    const float Now = Maya.GetWorld()->GetTimeSeconds();
    constexpr float Start = 10.f, Settle = 1.5f;
    if (Now < Start || Now < NextAt) return;
    NextAt = Now + Settle;
    const auto State = [&Hud]() { return FString::Printf(TEXT("map=%d missions=%d page=%d"), Hud.IsMapOpen(), Hud.IsMissionsOpen(), Hud.IsSkillsOpen()); };
    switch (TestStep)
    {
    case 0: Check(TEXT("start_closed"), !Hud.IsSkillsOpen(), State()); Press(Player, EKeys::M); break;
    case 1: Check(TEXT("m_opens_map"), Hud.IsMapOpen() && !Hud.IsMissionsOpen(), State()); Hud.CloseSkills(); break;
    case 2: Check(TEXT("map_closed"), !Hud.IsSkillsOpen(), State()); Press(Player, EKeys::L); break;
    case 3: Check(TEXT("l_opens_missions"), Hud.IsMissionsOpen() && !Hud.IsMapOpen(), State()); Hud.CloseSkills(); break;
    default:
        Check(TEXT("closed_at_end"), !Hud.IsSkillsOpen(), State());
        UE_LOG(LogTemp, Display, TEXT("OWCONTROLS SUMMARY passed=%d failed=%d"), Passed, Failed);
        Player.ConsoleCommand(TEXT("quit"));
        TestStep = -1;
        return;
    }
    ++TestStep;
}
