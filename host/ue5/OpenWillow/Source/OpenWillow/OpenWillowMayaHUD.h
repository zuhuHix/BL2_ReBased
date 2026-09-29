#pragma once

#include "CoreMinimal.h"
#include "GameFramework/HUD.h"
#include "OpenWillowMayaHUD.generated.h"

class SWebBrowser;
class SWidget;
class SBox;
class AOpenWillowInventoryMayaDisplay;

UCLASS()
class OPENWILLOW_API AOpenWillowMayaHUD : public AHUD
{
    GENERATED_BODY()
public:
    virtual void BeginPlay() override;
    virtual void EndPlay(const EEndPlayReason::Type Reason) override;
    virtual void DrawHUD() override;
    void ToggleSkills();
    bool ToggleInventory();
    void CloseSkills();
    void RequestSkillsCloseFromPage();
    bool IsSkillsOpen() const { return SkillsBrowser.IsValid(); }
    // Capture-only: dispatches a keydown on the open status-menu page.
    void SendPageKey(const FString& Key);

    // Test hooks for -owinventoryactions (UOpenWillowInventoryActionTest).
    // What the host did with the page's OWITEM requests, in order.
    struct FInventoryActionRecord
    {
        int32 Serial = 0;
        FString Action;
        FString Id;
        bool bAccepted = false;
    };
    bool IsInventoryOpen() const { return SkillsBrowser.IsValid() && bInventoryOpen; }
    const FInventoryActionRecord& LastInventoryAction() const { return LastAction; }
    // Sends a key to the page as a real Slate key event (down then up) so it
    // travels the same focus path as a player's key press; the page sees it
    // only if the browser really has keyboard focus. False when the key name
    // has no mapping. Names are the page's KeyboardEvent.key values.
    bool SendSlateKey(const FString& Key);
    // Asks the page to log its own selection and snapshot state as
    // "OWINVPAGE {json}"; PageReportSerial() advances when it arrives.
    void RequestPageReport();
    int32 PageReportSerial() const { return PageReportCount; }
    const FString& PageReport() const { return LastPageReport; }
    // Makes the page log a raw OWITEM request (skips the page's own checks),
    // so host-side validation can be tested on its own.
    void InjectPageRequest(const FString& Json);

private:
    void OpenStatusMenu(bool bInventory);
    TSharedPtr<SWebBrowser> CreateStatusBrowser(const FString& MenuUrl);
    FString InventoryUrl;
    bool bInventoryOpen = false;
    UPROPERTY() TObjectPtr<AOpenWillowInventoryMayaDisplay> InventoryMayaDisplay;
    TArray<FString> PendingInventoryActions;
    FInventoryActionRecord LastAction;
    FString LastPageReport;
    int32 PageReportCount = 0;
    void DrawDamagePopups(UFont* Font);
    // -owflashhud=<url>: BL2's own HUD movie in Ruffle, in a transparent
    // web page over the viewport (tools/hud_overlay). Prototype only.
    void PushFlashHudState(const class AOpenWillowWalker& Maya);

    TSharedPtr<SWebBrowser> FlashHud;
    TSharedPtr<SWidget> FlashHudRoot;
    FString LastFlashState;
    float NextFlashPush = 0.f;
    // -owflashskills=<url>: StatusMenu Skills frame, populated from local
    // package data by tools/hud_overlay/skills.js.
    FString SkillsUrl;
    TSharedPtr<SWebBrowser> SkillsBrowser;
    TSharedPtr<SWidget> SkillsRoot;
    // Loaded once in the background so opening inventory does not reload SWFs.
    TSharedPtr<SWebBrowser> CachedInventoryBrowser;
    TSharedPtr<SBox> CachedInventoryRoot;
    bool bCloseSkillsRequested = false;
    // Header-tab clicks on either status page: 1 = inventory, 2 = skills (0 = none).
    int32 PendingTabSwitch = 0;
    float NextSkillsPush = 0.f;
    // Spends the page reported (branch, tier, cell), applied in DrawHUD.
    void OnSkillsConsole(const FString& Message);
    void PushSkillsState();
    TArray<FIntVector> PendingSpends;
};
