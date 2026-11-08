// AI-assisted (Claude). The mission page's in-engine suite: -owmissionpagetest, run by tools/test_mission_page.ps1 and graded by tools/check_mission_page.py. E goes through the
// player's input binding when it should reach the quest, and through Slate, as the keyboard sends it, when the page should take it; each no-op row also checks the press arrived.
// A pass says what the host did, not that it matches the game. The objectives are completed by direct slice calls: the objective path has its own suite (tools/test_quest.ps1).
// docs/verification/MISSION_SCREEN.md.
#include "OpenWillowQuest.h"
#include "OpenWillowInventory.h"
#include "OpenWillowMayaHUD.h"
#include "OpenWillowMissionPage.h"
#include "OpenWillowNpc.h"
#include "OpenWillowFocusCam.h"
#include "OpenWillowWalker.h"
#include "Framework/Application/SlateApplication.h"
#include "Camera/CameraComponent.h"
#include "Camera/PlayerCameraManager.h"
#include "Engine/Engine.h"
#include "Engine/GameViewportClient.h"
#include "GameFramework/PlayerController.h"
#include "InputKeyEventArgs.h"
#include "Misc/CommandLine.h"
#include "Misc/Parse.h"
#include "UnrealClient.h"

namespace
{
// A press and release through Slate, as the keyboard sends them. The codes are Windows' (OpenWillowMayaHUD's SendSlateKey has the same ones).
void PressKey(const FKey& Key)
{
    if (!FSlateApplication::IsInitialized()) return;
    const uint32 Code = Key == EKeys::Enter ? 13 : Key == EKeys::Escape ? 27 : 'E';
    const uint32 Char = Key == EKeys::Enter ? 13 : Key == EKeys::Escape ? 0 : 'e';
    FSlateApplication& Slate = FSlateApplication::Get();
    Slate.ProcessKeyDownEvent(FKeyEvent(Key, Slate.GetModifierKeys(), 0, false, Char, Code));
    Slate.ProcessKeyUpEvent(FKeyEvent(Key, Slate.GetModifierKeys(), 0, false, Char, Code));
}
// Through the player's input binding, as the use key and the trigger reach the walker.
void BindingKey(APlayerController* PC, const FKey& Key, EInputEvent Event)
{
    if (PC) PC->InputKey(FInputKeyEventArgs::CreateSimulated(Key, Event, Event == IE_Pressed ? 1.f : 0.f));
}
// With the UI: the page is a Slate widget. Not called Shot: inside the member function that name finds UOpenWillowQuest::Shot, whose frames leave Slate out.
void PageShot(const TCHAR* Name)
{
    AOpenWillowMayaHUD::CaptureShot(TEXT("missionpage"), FString::Printf(TEXT("OWMissionPage_%s.png"), Name));
}
}

void UOpenWillowQuest::TickScreenPageTest()
{
    if (!bScreenPageTest || !Impl || !ScreenPage.IsValid() || !GetWorld()) return;
    AOpenWillowWalker* Maya = Cast<AOpenWillowWalker>(GetOwner());
    APlayerController* PC = GetWorld()->GetFirstPlayerController();
    if (!Maya || !PC) return;
    // A run that asked for the movie HUD holds the test until it is ready. If it fails, the suite fails with no frames.
    if (ScreenTestStep == 0)
    {
        const EOpenWillowCaptureHud MovieHud = AOpenWillowMayaHUD::CaptureHudGate(GetWorld(), TEXT("missionpage"));
        if (MovieHud == EOpenWillowCaptureHud::Wait) return;
        if (MovieHud == EOpenWillowCaptureHud::Failed)
        {
            UE_LOG(LogTemp, Display, TEXT("OWMISSIONPAGETEST SUMMARY passed=0 failed=1"));
            bScreenPageTest = false;
            PC->ConsoleCommand(TEXT("quit"));
            return;
        }
    }
    const double Now = FPlatformTime::Seconds();
    if (ScreenTestAt == 0.) ScreenTestAt = Now + 60.;   // the page's load limit
    const auto Row = [this](const TCHAR* Name, bool bOk, const FString& Detail)
    {
        ++(bOk ? ScreenTestPassed : ScreenTestFailed);
        UE_LOG(LogTemp, Display, TEXT("OWMISSIONPAGETEST %s %s %s"), Name, bOk ? TEXT("PASS") : TEXT("FAIL"), *Detail);
    };
    const auto State = [&] { return FString::Printf(TEXT("status=%d page=%s shown=%d screen=%d pending=%d keys=%d uses=%d arms=%d report=%s"), Status(), *ScreenPageOn, ScreenPage->IsShown(),
        ScreenOpenInVm(), RewardPendingInVm(), ScreenKeysSeen, ScreenUsesSeen, Maya->AreArmsShown(), *ScreenPage->LastReport()); };
    const auto Finish = [&]
    {
        UE_LOG(LogTemp, Display, TEXT("OWMISSIONPAGETEST SUMMARY passed=%d failed=%d"), ScreenTestPassed, ScreenTestFailed);
        bScreenPageTest = false;
        PC->ConsoleCommand(TEXT("quit"));
    };
    // 110 uu in front of him, looking at his chest: the stand of the real capture (fire_mission_probe.py, fm_stand_in_front), so the frames compare.
    const auto Stand = [this] { PlacePlayer(Marcus->GetActorLocation() + Marcus->GetActorForwardVector() * 110.f + FVector(0, 0, 20), Marcus->GetActorLocation() + FVector(0, 0, 20)); };
    // The gun with a full magazine, so the page is the only thing that changes between the shots.
    const auto StockMagazine = [Maya] { if (FOpenWillowWeaponItem* Held = Maya->GetInventory()->ActiveWeaponMutable()) Held->MagazineLeft = -1; };
    const auto Magazine = [Maya] { const FOpenWillowWeaponItem* Held = Maya->GetInventory()->ActiveWeapon(); return Held ? Held->MagazineLeft : -2; };
    // The binding fires on the player's next input tick, not inside InputKey: this only sends the press. The row that needs it runs at the next step, against ScreenTestUses.
    const auto UseE = [&] { ScreenTestUses = ScreenUsesSeen; BindingKey(PC, EKeys::E, IE_Pressed); BindingKey(PC, EKeys::E, IE_Released); };
    const auto UseArrived = [&] { return ScreenUsesSeen == ScreenTestUses + 1; };
    // A press the page must take: true when exactly one key reached OnScreenKey.
    const auto PageKey = [&](const FKey& Key) { const int32 Before = ScreenKeysSeen; PressKey(Key); return ScreenKeysSeen == Before + 1; };
    const bool bExpectMovie = !FParse::Param(FCommandLine::Get(), TEXT("owmissionpagenomovie"));
    // The gameplay HUD behind the page and after each way out. Under -owcanvashud this shows only the canvas path; the report says which.
    const auto HudRow = [&](const TCHAR* Name, bool bHidden)
    {
        const AOpenWillowMayaHUD* GameHud = Cast<AOpenWillowMayaHUD>(PC->GetHUD());
        Row(Name, GameHud && (bHidden ? GameHud->IsGameplayHudHidden() : GameHud->IsGameplayHudBack()), GameHud ? GameHud->GameplayHudReport() : FString(TEXT("no HUD")));
    };
    // The page's view on Marcus (OpenWillowFocusCam): with the page up, his bone lands on the local data's screen point in the view the player sees (the camera
    // manager's, one frame old); with it closed, the view is the control rotation again, and the control rotation and FOV are those from just before the
    // page opened (ViewBefore). Without the local data the "on" rows fail and say so.
    const auto ViewBefore = [&] { ScreenTestControl = PC->GetControlRotation(); ScreenTestFov = PC->PlayerCameraManager ? PC->PlayerCameraManager->GetFOVAngle() : 0.f; };
    const auto FocusRow = [&](const TCHAR* Name, bool bOn)
    {
        const UCameraComponent* Camera = Maya->FindComponentByClass<UCameraComponent>();
        const APlayerCameraManager* View = PC->PlayerCameraManager;
        if (!Camera || !View || !Marcus) { Row(Name, false, TEXT("no camera or no Marcus")); return; }
        double Aspect = 16. / 9.;
        if (GEngine && GEngine->GameViewport)
        {
            FVector2D Size = FVector2D::ZeroVector;
            GEngine->GameViewport->GetViewportSize(Size);
            if (Size.X > 1. && Size.Y > 1.) Aspect = Size.X / Size.Y;
        }
        const FString Seen = FString::Printf(TEXT("view=%s control=%s fov=%.1f pawn_rotation=%d before: control=%s fov=%.1f"), *View->GetCameraRotation().ToString(),
            *PC->GetControlRotation().ToString(), View->GetFOVAngle(), Camera->bUsePawnControlRotation ? 1 : 0, *ScreenTestControl.ToString(), ScreenTestFov);
        if (!bOn)
        {
            const bool bBack = Camera->bUsePawnControlRotation && View->GetCameraRotation().Equals(PC->GetControlRotation(), 0.5f)
                && PC->GetControlRotation().Equals(ScreenTestControl, 0.5f) && FMath::Abs(View->GetFOVAngle() - ScreenTestFov) < 0.5f;
            Row(Name, bBack, Seen);
            return;
        }
        if (!FocusParams) { Row(Name, false, FString::Printf(TEXT("no focus camera data (%s) %s"), *OpenWillowFocusCam::DefaultPath(), *Seen)); return; }
        FVector Point;
        const bool bBone = OpenWillowFocusCam::FocusPoint(*Marcus, FocusParams->Bone, Point);
        FVector2D Landed;
        const bool bFront = OpenWillowFocusCam::Project(View->GetCameraLocation(), View->GetCameraRotation(), View->GetFOVAngle(), Aspect, Point, Landed);
        const bool bOk = bBone && bFront && !Camera->bUsePawnControlRotation && FVector2D::Distance(Landed, FocusParams->Screen) < 0.06;
        Row(Name, bOk, FString::Printf(TEXT("bone=%d landed=(%.3f, %.3f) wanted=(%.3f, %.3f) %s"), bBone ? 1 : 0, Landed.X, Landed.Y, FocusParams->Screen.X, FocusParams->Screen.Y, *Seen));
    };
    // The page on show, not any page: a late report from the last one must not pass.
    const auto Drawn = [&] { const FString& R = ScreenPage->LastReport();
        return !ScreenPageOn.IsEmpty() && R.Contains(FString::Printf(TEXT("\"page\":\"%s\""), *ScreenPageOn))
            && R.Contains(TEXT("\"drawn\":true")) && R.Contains(bExpectMovie ? TEXT("\"movie\":true") : TEXT("\"movie\":false")); };
    // Steps run after a time and a few ticks, and rows that need a state wait for it (Await): one frame can stall for many seconds, and everything due by the clock would then
    // run in the frame after it, before the page has had a chance to answer.
    ++ScreenStepTicks;
    const auto Await = [&](bool bState) { return bState || (Now - ScreenStepAt >= 30. && ScreenStepTicks >= 30); };
    if (ScreenTestStep != 0 && ScreenTestStep != 11 && (Now < ScreenTestAt || ScreenStepTicks < 5)) return;
    double Wait = 0.;
    switch (ScreenTestStep)
    {
    case 0:   // wait for the page, then put a gun in her hands
    {
        if (!ScreenPage->IsReady())
        {
            if (Now >= ScreenTestAt) { Row(TEXT("page_ready"), false, TEXT("no OWMISSIONREADY in 60 s")); Finish(); }
            return;
        }
        Row(TEXT("page_ready"), true, TEXT(""));
        UOpenWillowInventory* Inventory = Maya->GetInventory();
        for (int32 Slot = 0; Slot < UOpenWillowInventory::SlotCount && !Maya->HasWeaponOut(); ++Slot)
            if (const FOpenWillowWeaponItem* Item = Inventory->SlotItem(Slot)) Maya->DrawItemById(UOpenWillowInventory::StableId(*Item));
        for (int32 Item = 0; Item < Inventory->Items().Num() && !Maya->HasWeaponOut(); ++Item) Maya->EquipItem(Item, 0);
        Stand();
        // A direct call, since no key reaches the quest under a status page: the mission page must refuse, and the VM must open no screen.
        if (AOpenWillowMayaHUD* Hud = Cast<AOpenWillowMayaHUD>(PC->GetHUD()))
        {
            const bool bOpen = Hud->ToggleMissions() && Hud->IsSkillsOpen();
            const bool bUsed = TryUse();
            Row(TEXT("e_refused_under_a_status_page"), bOpen && !bUsed && !ScreenPage->IsShown() && !ScreenOpenInVm() && Status() == 0 && Hud->IsSkillsOpen(), State());
            Hud->CloseSkills();
        }
        else Row(TEXT("e_refused_under_a_status_page"), false, TEXT("no HUD"));
        Wait = 2.;
        break;
    }
    case 1:
        StockMagazine();
        Row(TEXT("control_gun_fires_before_the_page"), Maya->FireOnce() && Maya->AreArmsShown(), State());
        BindingKey(PC, EKeys::LeftMouseButton, IE_Pressed);   // held through the page's open, released under it
        ViewBefore();
        UseE();
        break;
    case 2:
        if (!Await(ScreenPage->IsShown())) return;
        Row(TEXT("first_e_opens_the_page"), UseArrived() && ScreenPage->IsShown() && ScreenPageOn == TEXT("offer") && Status() == 0 && ScreenOpenInVm(), State());
        StockMagazine();
        break;
    case 3:
        if (!Await(Drawn())) return;
        Row(TEXT("offer_drawn"), Drawn(), State());
        Wait = 2.;   // for the frame, and two seconds with the trigger held
        break;
    case 4:
    {
        HudRow(TEXT("hud_hidden_behind_the_offer"), true);
        FocusRow(TEXT("camera_on_marcus_offer"), true);
        PageShot(TEXT("1_Offer"));
        // The trigger held and a full magazine: the magazine has not moved, a direct shot is refused, and arms and gun are put away.
        const int32 Left = Magazine();
        Row(TEXT("held_trigger_does_not_fire_under_the_page"), Left == -1 && !Maya->FireOnce() && !Maya->AreArmsShown(), FString::Printf(TEXT("magazine=%d %s"), Left, *State()));
        BindingKey(PC, EKeys::LeftMouseButton, IE_Released);
        Row(TEXT("offer_e_does_nothing"), PageKey(EKeys::E) && Status() == 0 && ScreenPage->IsShown() && ScreenPageOn == TEXT("offer"), State());
        Wait = 1.;
        break;
    }
    case 5:
        Row(TEXT("offer_escape_closes"), PageKey(EKeys::Escape) && Status() == 0 && !ScreenPage->IsShown() && !ScreenOpenInVm(), State());
        Wait = 1.5;
        break;
    case 6:
        HudRow(TEXT("hud_back_after_escape"), false);
        FocusRow(TEXT("camera_back_after_escape"), false);
        PageShot(TEXT("2_AfterEscape"));
        StockMagazine();
        Row(TEXT("gun_fires_again_after_the_close"), Maya->AreArmsShown() && Maya->FireOnce(), State());
        UseE();
        break;
    case 7:
        if (!Await(ScreenPage->IsShown())) return;
        Row(TEXT("e_opens_the_offer_again"), UseArrived() && ScreenPage->IsShown() && ScreenPageOn == TEXT("offer"), State());
        break;
    case 8:
        if (!Await(Drawn())) return;
        Row(TEXT("offer_drawn_again"), Drawn(), State());
        Wait = 1.;
        break;
    case 9:
        Row(TEXT("offer_enter_accepts_and_closes"), PageKey(EKeys::Enter) && Status() == 1 && !ScreenPage->IsShown() && !ScreenOpenInVm() && RewardPages == 0, State());
        Wait = 1.5;
        break;
    case 10:
        HudRow(TEXT("hud_back_after_accepting"), false);
        PageShot(TEXT("3_Accepted"));
        ScreenTestRetries = 0;
        break;
    case 11:   // the objectives by direct calls; the dummy's spawn has timed steps, so retry
        if (Now < ScreenTestAt) return;
        if (Status() != 2)
        {
            TestAdvanceObjectives();
            ScreenTestAt = Now + 1.;
            if (++ScreenTestRetries <= 30) return;
        }
        Row(TEXT("mission_ready_to_turn_in"), Status() == 2, State());
        if (Status() != 2) { Finish(); return; }
        Stand();
        Wait = 1.;
        break;
    case 12:
        ViewBefore();
        UseE();
        break;
    case 13:
        if (!Await(ScreenPage->IsShown())) return;
        Row(TEXT("e_opens_the_turn_in"), UseArrived() && ScreenPageOn == TEXT("turnin") && ScreenPage->IsShown() && Status() == 2, State());
        break;
    case 14:
        if (!Await(Drawn())) return;
        Row(TEXT("turn_in_drawn"), Drawn(), State());
        Wait = 1.;
        break;
    case 15:
        HudRow(TEXT("hud_hidden_behind_the_turn_in"), true);
        FocusRow(TEXT("camera_on_marcus_turn_in"), true);
        PageShot(TEXT("4_TurnIn"));
        Row(TEXT("turn_in_e_does_nothing"), PageKey(EKeys::E) && Status() == 2 && ScreenPageOn == TEXT("turnin"), State());
        PageKey(EKeys::Enter);
        break;
    case 16:
        if (!Await(ScreenPageOn == TEXT("reward") && Drawn())) return;
        Row(TEXT("enter_shows_the_reward_page"), Status() == 3 && ScreenPageOn == TEXT("reward") && ScreenPage->IsShown() && RewardPendingInVm()
            && RewardPages == 1 && RewardsReceived == 0 && Drawn(), State());
        Wait = 1.;
        break;
    case 17:
        HudRow(TEXT("hud_hidden_behind_the_reward"), true);
        FocusRow(TEXT("camera_on_marcus_reward"), true);
        PageShot(TEXT("5_Reward"));
        Row(TEXT("reward_waits_for_enter"), PageKey(EKeys::E) && PageKey(EKeys::Escape) && ScreenPageOn == TEXT("reward") && ScreenPage->IsShown() && RewardPendingInVm() && RewardsReceived == 0, State());
        break;
    case 18:
        Row(TEXT("enter_takes_the_reward_and_closes"), PageKey(EKeys::Enter) && Status() == 3 && !ScreenPage->IsShown() && !ScreenOpenInVm() && !RewardPendingInVm() && RewardsReceived == 1, State());
        Wait = 1.5;
        break;
    case 19:
        HudRow(TEXT("hud_back_after_the_reward"), false);
        FocusRow(TEXT("camera_back_after_the_reward"), false);
        PageShot(TEXT("6_Closed"));
        Row(TEXT("first_person_back_after_the_reward"), Maya->AreArmsShown(), State());
        UseE();
        break;
    case 20:
        // Nothing here changes when the press lands, so the row waits for the press itself.
        if (!Await(UseArrived())) return;
        Row(TEXT("e_afterwards_opens_nothing"), UseArrived() && !ScreenPage->IsShown(), State());
        // Last, since a failed quest stops everything: the page put up directly, so the view is on Marcus, then a failure must close it and give the view back.
        ViewBefore();
        ShowScreenPage(TEXT("offer"));
        Wait = 1.;
        break;
    case 21:
    {
        const UCameraComponent* Camera = Maya->FindComponentByClass<UCameraComponent>();
        const bool bWasOn = bFocusOn && Camera && !Camera->bUsePawnControlRotation;
        Fail(TEXT("mission page suite: a failure forced with the page open"));
        const bool bBack = Camera && Camera->bUsePawnControlRotation && FMath::Abs(Camera->FieldOfView - ScreenTestFov) < 0.5f && PC->GetControlRotation().Equals(ScreenTestControl, 0.5f);
        Row(TEXT("failure_closes_the_page_and_gives_the_view_back"), bWasOn && !ScreenPage->IsShown() && !bFocusOn && bBack,
            FString::Printf(TEXT("was_on=%d fov=%.1f before=%.1f %s"), bWasOn ? 1 : 0, Camera ? Camera->FieldOfView : 0.f, ScreenTestFov, *State()));
        Finish();
        return;
    }
    default: return;
    }
    ++ScreenTestStep;
    ScreenTestAt = Now + Wait;
    ScreenStepAt = Now;
    ScreenStepTicks = 0;
}
