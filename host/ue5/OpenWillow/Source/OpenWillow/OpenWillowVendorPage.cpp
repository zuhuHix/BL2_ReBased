// AI-assisted (Claude). See OpenWillowVendorPage.h.
#include "OpenWillowVendorPage.h"
#include "SWebBrowser.h"
#include "Widgets/Layout/SBox.h"
#include "Engine/Engine.h"
#include "Engine/GameViewportClient.h"
#include "GameFramework/PlayerController.h"
#include "Framework/Application/SlateApplication.h"
#include "Misc/CommandLine.h"
#include "Misc/Parse.h"
#include "Framework/Application/IInputProcessor.h"
#include "InputCoreTypes.h"

namespace
{
// Above the HUD (5) and the inventory screen (10), so the shop covers both.
constexpr int32 VendorZOrder = 20;

// Host-side backstop: Escape asks the owner to close the shop even when the page has no focus or never loaded.
class FVendorEscapeProcessor : public IInputProcessor
{
public:
    explicit FVendorEscapeProcessor(TFunction<void()> InOnEscape) : OnEscape(MoveTemp(InOnEscape)) {}
    virtual void Tick(const float, FSlateApplication&, TSharedRef<ICursor>) override {}
    virtual bool HandleKeyDownEvent(FSlateApplication&, const FKeyEvent& Event) override
    {
        if (Event.GetKey() != EKeys::Escape || !OnEscape) return false;
        OnEscape();
        return true;
    }
private:
    TFunction<void()> OnEscape;
};
}

FOpenWillowVendorPage::FOpenWillowVendorPage(FOnMessage InOnMessage, FOnCloseRequest InOnCloseRequest)
    : OnMessage(MoveTemp(InOnMessage)), OnCloseRequest(MoveTemp(InOnCloseRequest))
{
}

FOpenWillowVendorPage::~FOpenWillowVendorPage()
{
    if (EscapeProcessor.IsValid() && FSlateApplication::IsInitialized())
        FSlateApplication::Get().UnregisterInputPreProcessor(EscapeProcessor);
    if (RootWidget.IsValid() && GEngine && GEngine->GameViewport)
        GEngine->GameViewport->RemoveViewportWidgetContent(RootWidget.ToSharedRef());
}

FString FOpenWillowVendorPage::PageUrl()
{
    // Port 8767 and the file name are the stand-in; serve.py serves the page once its PAGES entry is in.
    FString Url = TEXT("http://127.0.0.1:8767/vending.html");
    FParse::Value(FCommandLine::Get(), TEXT("owvendingpage="), Url);
    return Url;
}

void FOpenWillowVendorPage::EnsureCreated()
{
    if (Browser.IsValid()) return;
    Browser = SNew(SWebBrowser)
        .InitialURL(PageUrl())
        .ShowControls(false)
        .ShowAddressBar(false)
        .ShowErrorMessage(true)
        .ShowInitialThrobber(false)
        .SupportsTransparency(true)
        .BackgroundColor(FColor(0, 0, 0, 0))
        .BrowserFrameRate(30)
        .OnLoadCompleted_Lambda([this]
            {
                bLoaded = true;
                if (!LastState.IsEmpty()) PushScript();
            })
        .OnLoadError_Lambda([this]
            {
                UE_LOG(LogTemp, Warning, TEXT("OpenWillow vending page failed to load; is tools/hud_overlay/serve.py running on port 8767? Closing the shop."));
                bLoaded = false;
                bReady = false;
                if (bShown && OnCloseRequest) OnCloseRequest();
            })
        .OnConsoleMessage_Lambda([this](const FString& Message, const FString& Source, int32 Line, EWebBrowserConsoleLogSeverity)
            {
                UE_LOG(LogTemp, Display, TEXT("OpenWillow vending console: %s (%s:%d)"), *Message, *Source, Line);
                if (Message == TEXT("OWVENDREADY"))
                {
                    bReady = true;
                    if (bShown) TakeInput();
                }
                else if (Message.StartsWith(TEXT("OWVEND ")) && OnMessage) OnMessage(Message);
            });
    RootWidget = SNew(SBox).Visibility(EVisibility::Hidden)[Browser.ToSharedRef()];
    if (GEngine && GEngine->GameViewport)
        GEngine->GameViewport->AddViewportWidgetContent(RootWidget.ToSharedRef(), VendorZOrder);
}

void FOpenWillowVendorPage::Show(APlayerController* PC)
{
    EnsureCreated();
    if (RootWidget.IsValid()) RootWidget->SetVisibility(EVisibility::Visible);
    bShown = true;
    Controller = PC;
    if (!EscapeProcessor.IsValid() && FSlateApplication::IsInitialized())
    {
        EscapeProcessor = MakeShared<FVendorEscapeProcessor>([this] { if (bShown && OnCloseRequest) OnCloseRequest(); });
        FSlateApplication::Get().RegisterInputPreProcessor(EscapeProcessor);
    }
    if (bReady) TakeInput(); // else the page's OWVENDREADY line does it; until then the game keeps its input
}

void FOpenWillowVendorPage::TakeInput()
{
    APlayerController* PC = Controller.Get();
    if (!PC) return;
    // Same as the inventory screen: UI-only input with the page focused, so the walker stops reading the keys.
    FInputModeUIOnly Mode;
    Mode.SetWidgetToFocus(Browser);
    PC->SetInputMode(Mode);
    PC->SetShowMouseCursor(true);
}

void FOpenWillowVendorPage::Hide(APlayerController* PC)
{
    bShown = false;
    if (RootWidget.IsValid()) RootWidget->SetVisibility(EVisibility::Hidden);
    if (EscapeProcessor.IsValid() && FSlateApplication::IsInitialized())
        FSlateApplication::Get().UnregisterInputPreProcessor(EscapeProcessor);
    EscapeProcessor.Reset();
    Controller.Reset();
    if (PC)
    {
        PC->SetInputMode(FInputModeGameOnly());
        PC->SetShowMouseCursor(false);
    }
}

void FOpenWillowVendorPage::SendState(const FString& StateJson)
{
    LastState = StateJson;
    if (bLoaded) PushScript();
}

void FOpenWillowVendorPage::PushScript()
{
    if (Browser.IsValid()) Browser->ExecuteJavascript(FString::Printf(TEXT("window.owVending && window.owVending(%s)"), *LastState));
}
