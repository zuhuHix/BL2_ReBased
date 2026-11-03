// AI-assisted (Claude). The vending web page: an SWebBrowser in the viewport, above the HUD. The host pushes state with
// window.owVending(json); the page talks back through console lines "OWVEND {json}". Pattern copied from the HUD's
// status browsers (OpenWillowMayaHUD.cpp CreateStatusBrowser), kept small on purpose.
#pragma once

#include "CoreMinimal.h"

class APlayerController;
class SWebBrowser;
class SWidget;

class FOpenWillowVendorPage
{
public:
    // Receives every "OWVEND ..." console line from the page.
    using FOnMessage = TFunction<void(const FString& Line)>;

    explicit FOpenWillowVendorPage(FOnMessage InOnMessage);
    ~FOpenWillowVendorPage();

    // -owvendingpage=<url>; the default is the vending page served by tools/hud_overlay/serve.py.
    static FString PageUrl();

    // Creates the browser on first use, shows it and takes UI input on the controller.
    void Show(APlayerController* PC);
    // Hides it and gives input back to the game.
    void Hide(APlayerController* PC);
    bool IsShown() const { return bShown; }

    // Remembers the state and runs window.owVending(json) now, or after the page finishes loading.
    void SendState(const FString& StateJson);

private:
    void EnsureCreated();
    void PushScript();

    FOnMessage OnMessage;
    TSharedPtr<SWebBrowser> Browser;
    TSharedPtr<SWidget> RootWidget;
    FString LastState;
    bool bShown = false;
    bool bLoaded = false;
};
