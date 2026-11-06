// AI-assisted (Claude). See OpenWillowMissionPage.h.
#include "OpenWillowMissionPage.h"
#include "SWebBrowser.h"
#include "Widgets/Layout/SBox.h"
#include "Engine/Engine.h"
#include "Engine/GameViewportClient.h"
#include "GameFramework/PlayerController.h"
#include "Framework/Application/IInputProcessor.h"
#include "Framework/Application/SlateApplication.h"
#include "Misc/CommandLine.h"
#include "Misc/Parse.h"

namespace
{
// Above the HUD (5), and the same level as the shop: the two are never open together, since this one takes the keys.
constexpr int32 MissionPageZOrder = 20;

// While the page is shown every key press goes to the owner, so the walker reads none of them. Key releases pass, so a key held
// when the page opened is released as usual. Alt stays with the system (Alt+F4).
class FMissionPageKeys : public IInputProcessor
{
public:
    explicit FMissionPageKeys(FOpenWillowMissionPage::FOnKey InOnKey) : OnKey(MoveTemp(InOnKey)) {}
    virtual void Tick(const float, FSlateApplication&, TSharedRef<ICursor>) override {}
    virtual bool HandleKeyDownEvent(FSlateApplication&, const FKeyEvent& Event) override
    {
        if (Event.IsAltDown()) return false;
        if (!Event.IsRepeat() && OnKey) OnKey(Event.GetKey());
        return true;
    }
private:
    FOpenWillowMissionPage::FOnKey OnKey;
};
}

FOpenWillowMissionPage::FOpenWillowMissionPage(FOnKey InOnKey) : OnKey(MoveTemp(InOnKey))
{
}

FOpenWillowMissionPage::~FOpenWillowMissionPage()
{
    if (KeyProcessor.IsValid() && FSlateApplication::IsInitialized())
        FSlateApplication::Get().UnregisterInputPreProcessor(KeyProcessor);
    if (RootWidget.IsValid() && GEngine && GEngine->GameViewport)
        GEngine->GameViewport->RemoveViewportWidgetContent(RootWidget.ToSharedRef());
}

FString FOpenWillowMissionPage::PageUrl()
{
    FString Url;
    if (FParse::Value(FCommandLine::Get(), TEXT("owflashmissionscreen="), Url)) return Url;
    FString Skills;
    if (FParse::Value(FCommandLine::Get(), TEXT("owflashskills="), Skills) && Skills.Contains(TEXT("skills.html")))
        return Skills.Replace(TEXT("skills.html"), TEXT("mission_screen.html"));
    return FString();
}

void FOpenWillowMissionPage::Preload()
{
    if (Browser.IsValid() || !GEngine || !GEngine->GameViewport) return;
    Browser = SNew(SWebBrowser)
        .InitialURL(PageUrl())
        .ShowControls(false)
        .ShowAddressBar(false)
        .ShowErrorMessage(false)
        .ShowInitialThrobber(false)
        .SupportsTransparency(true)
        .BackgroundColor(FColor(0, 0, 0, 0))
        .BrowserFrameRate(30)
        .OnLoadError_Lambda([this]
            {
                UE_LOG(LogTemp, Warning, TEXT("OpenWillow mission page failed to load (is tools/hud_overlay/serve.py running?); the quest goes on without a page"));
                bReady = false;
            })
        .OnConsoleMessage_Lambda([this](const FString& Message, const FString& Source, int32 Line, EWebBrowserConsoleLogSeverity)
            {
                UE_LOG(LogTemp, Display, TEXT("OpenWillow mission page console: %s (%s:%d)"), *Message, *Source, Line);
                if (Message == TEXT("OWMISSIONREADY")) bReady = true;
                else if (Message.StartsWith(TEXT("OWMISSIONPAGE "))) Report = Message.RightChop(14);
            });
    // Never hit-tested: nothing on the page takes the mouse.
    RootWidget = SNew(SBox).Visibility(EVisibility::Hidden)[Browser.ToSharedRef()];
    GEngine->GameViewport->AddViewportWidgetContent(RootWidget.ToSharedRef(), MissionPageZOrder);
}

void FOpenWillowMissionPage::Show(APlayerController* PC)
{
    Preload();
    if (RootWidget.IsValid()) RootWidget->SetVisibility(EVisibility::HitTestInvisible);
    bShown = true;
    if (!KeyProcessor.IsValid() && FSlateApplication::IsInitialized())
    {
        KeyProcessor = MakeShared<FMissionPageKeys>(OnKey);
        FSlateApplication::Get().RegisterInputPreProcessor(KeyProcessor);
    }
    if (PC)
    {
        PC->SetInputMode(FInputModeUIOnly());
        PC->SetShowMouseCursor(false);
    }
}

void FOpenWillowMissionPage::Hide(APlayerController* PC)
{
    bShown = false;
    if (RootWidget.IsValid()) RootWidget->SetVisibility(EVisibility::Hidden);
    if (KeyProcessor.IsValid() && FSlateApplication::IsInitialized())
        FSlateApplication::Get().UnregisterInputPreProcessor(KeyProcessor);
    KeyProcessor.Reset();
    if (PC)
    {
        PC->SetInputMode(FInputModeGameOnly());
        PC->SetShowMouseCursor(false);
    }
}

void FOpenWillowMissionPage::SendState(const FString& StateJson)
{
    if (Browser.IsValid()) Browser->ExecuteJavascript(FString::Printf(TEXT("window.owMissionScreenState && window.owMissionScreenState(%s)"), *StateJson));
}

void FOpenWillowMissionPage::SendKey(const TCHAR* Key)
{
    if (Browser.IsValid()) Browser->ExecuteJavascript(FString::Printf(TEXT("window.owMissionScreenKey && window.owMissionScreenKey('%s')"), Key));
}
