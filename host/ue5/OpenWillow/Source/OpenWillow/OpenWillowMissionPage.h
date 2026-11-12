// AI-assisted (Claude). Marcus's mission screen: a web page (tools/hud_overlay/mission_screen.html) over the viewport, drawing BL2's own movie
// under Ruffle. The host owns the keys and the page only draws, so a page with no focus or a failed movie cannot trap the player. Same pattern as
// FOpenWillowVendorPage. docs/verification/MISSION_SCREEN.md.
#pragma once

#include "CoreMinimal.h"
#include "InputCoreTypes.h"

class APlayerController;
class IInputProcessor;
class SWebBrowser;
class SWidget;

class FOpenWillowMissionPage
{
public:
    // bRelease: the key's release; only Enter and Escape are passed on at both edges, every other key at its press only.
    using FOnKey = TFunction<void(const FKey& Key, bool bRelease)>;
    explicit FOpenWillowMissionPage(FOnKey InOnKey);
    ~FOpenWillowMissionPage();

    // -owflashmissionscreen=<url>, else the -owflashskills URL with mission_screen.html in its place; empty without either, as in the suites that use the
    // canvas HUD: the quest then runs its screenless path.
    static FString PageUrl();

    // Creates the browser, hidden, so the page is ready before the first use.
    void Preload();
    // The page has logged OWMISSIONREADY: with or without the movie, it can draw.
    bool IsReady() const { return bReady; }
    bool IsShown() const { return bShown; }
    // Shows it and takes the keys; every key press, and the release of Enter and Escape, goes to OnKey until Hide. The world goes on running.
    void Show(APlayerController* PC);
    void Hide(APlayerController* PC);
    // Runs window.owMissionScreenState(json).
    void SendState(const FString& StateJson);
    // Forwards a scroll key to the page.
    void SendKey(const TCHAR* Key);
    // The page's last "OWMISSIONPAGE {...}" line, for the log and the suite.
    const FString& LastReport() const { return Report; }

private:
    FOnKey OnKey;
    TSharedPtr<IInputProcessor> KeyProcessor;
    TSharedPtr<SWebBrowser> Browser;
    TSharedPtr<SWidget> RootWidget;
    FString Report;
    bool bShown = false;
    bool bReady = false;
};
