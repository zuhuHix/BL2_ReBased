// AI-assisted (Claude). The vending web page: an SWebBrowser in the viewport, above the HUD. The host pushes state with
// window.owVending(json); the page talks back through console lines "OWVEND {json}". Pattern copied from the HUD's
// status browsers (OpenWillowMayaHUD.cpp CreateStatusBrowser), kept small on purpose.
#pragma once

#include "CoreMinimal.h"

class APlayerController;
class IInputProcessor;
class SWebBrowser;
class SWidget;

class FOpenWillowVendorPage
{
public:
    // Receives every "OWVEND ..." console line from the page.
    using FOnMessage = TFunction<void(const FString& Line)>;

    // Asks the owner to close the shop: the page failed to load, or the host-side Escape backstop was pressed.
    using FOnCloseRequest = TFunction<void()>;

    FOpenWillowVendorPage(FOnMessage InOnMessage, FOnCloseRequest InOnCloseRequest);
    ~FOpenWillowVendorPage();

    // -owvendingpage=<url>; the default is the vending page served by tools/hud_overlay/serve.py.
    static FString PageUrl();

    // Creates the browser on first use and shows it. The controller keeps game input until the page reports ready
    // ("OWVENDREADY"); only then does the page get UI-only input, so a page that never loads cannot trap the player.
    void Show(APlayerController* PC);
    bool IsReady() const { return bReady; }
    // Hides it and gives input back to the game.
    void Hide(APlayerController* PC);
    bool IsShown() const { return bShown; }

    // Remembers the state and runs window.owVending(json) now, or after the page finishes loading.
    void SendState(const FString& StateJson);

private:
    void EnsureCreated();
    void PushScript();
    void TakeInput();

    FOnMessage OnMessage;
    FOnCloseRequest OnCloseRequest;
    TWeakObjectPtr<APlayerController> Controller;
    // Escape closes the shop whatever has focus (registered while shown).
    TSharedPtr<IInputProcessor> EscapeProcessor;
    TSharedPtr<SWebBrowser> Browser;
    TSharedPtr<SWidget> RootWidget;
    FString LastState;
    bool bShown = false;
    bool bLoaded = false;
    bool bReady = false;   // the page logged OWVENDREADY (it stays loaded between opens)
};
