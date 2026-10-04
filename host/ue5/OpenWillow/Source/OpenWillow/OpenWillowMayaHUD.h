#pragma once

#include "CoreMinimal.h"
#include "GameFramework/HUD.h"
#include "OpenWillowMayaHUD.generated.h"

class SWebBrowser;
class SWidget;
class SBox;
class AOpenWillowInventoryMayaDisplay;
class AOpenWillowInventoryPreviewActor;
class FOpenWillowInventoryVm;

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
    // Test: dispatch the browser's native drag/drop DOM events over live cells.
    void SendPageDrag(const FString& Id, int32 DestinationSlot);
    void SendPageInspectDrag();
    void SendPageBackpackWheel(int32 PixelDelta);

private:
    void OpenStatusMenu(bool bInventory);
    TSharedPtr<SWebBrowser> CreateStatusBrowser(const FString& MenuUrl);
    void CreateInventoryVm();
    FString InventoryUrl;
    bool bInventoryOpen = false;
    UPROPERTY() TObjectPtr<AOpenWillowInventoryMayaDisplay> InventoryMayaDisplay;
    UPROPERTY() TObjectPtr<AOpenWillowInventoryPreviewActor> InspectActor;
    FString PendingInspectRequest;
    FString PendingMenuPreviewId;
    bool bMenuPreviewRequested = false;
    double NextInspectFrame = 0;
    TArray<FString> PendingInventoryActions;
    TSharedPtr<FOpenWillowInventoryVm> InventoryVm;
    TArray<FString> PendingInventoryVmMoves;
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
    // The skills page the same way: loaded hidden at level start so K shows a populated tree at once.
    TSharedPtr<SWebBrowser> CachedSkillsBrowser;
    TSharedPtr<SBox> CachedSkillsRoot;
    bool bCloseSkillsRequested = false;
    // Header-tab clicks on either status page: 1 = inventory, 2 = skills (0 = none).
    int32 PendingTabSwitch = 0;
    float NextSkillsPush = 0.f;
    // Spends the page reported (branch, tier, cell), applied in DrawHUD.
    void OnSkillsConsole(const FString& Message);
    void PushSkillsState();
    TArray<FIntVector> PendingSpends;

    // Inventory open-time instrumentation: "OWINVTIME <event> t=<ms since the
    // open request> ..." log lines from the host and (prefixed js_) from
    // inventory.js, which is told the host's open time in Unix ms.
    void TimeLog(const TCHAR* Event, const FString& Detail = FString()) const;
    double OpenStartedAt = 0;      // FPlatformTime::Seconds() of the last open request
    uint64 OpenFrame = 0;          // GFrameCounter at that request
    bool bOpenFrameLogged = true, bOpenPushLogged = true, bOpenPreviewLogged = true;
    double PreloadCreatedAt = 0;
    // Inventory display assets loaded at BeginPlay (see AOpenWillowInventoryMayaDisplay::PreloadAssets).
    UPROPERTY() TArray<TObjectPtr<UObject>> PreloadedMenuAssets;
    bool bWeaponMeshesPreloaded = false;
    int64 OpenEpoch = 0;           // Unix ms of the open request, passed to the page
    bool bOpenTimingAcked = true;  // the page logged js_open for the current open
    // -owinvopenbench=<runs> [-owinvopenbenchdelay=<s>]: open the inventory,
    // wait until the page reports it painted, hold, close, repeat; then quit.
    void TickOpenBench();
    // -owinvshots: an unattended capture of the inventory page the way a player drives it (open, backpack,
    // compare from either origin, inspect, tab to Skills one second after the tab), screenshots named
    // OWCombat_D*.png. It is independent of -owcombatshots, whose key script predates the stock navigation.
    void TickInvShots();
    bool bInvShots = false;
    bool bInventoryPageReady = false, bSkillsPagePopulated = false; // set from the pages' OWINVTIME lines
    int32 InvShotStep = 0;
    double InvShotAt = 0;
    int32 OpenBenchRuns = 0, OpenBenchDone = 0, OpenBenchPhase = 0;
    double OpenBenchAt = 0;
    float OpenBenchDelay = 20.f;
    bool bOpenBenchPainted = false;
};
