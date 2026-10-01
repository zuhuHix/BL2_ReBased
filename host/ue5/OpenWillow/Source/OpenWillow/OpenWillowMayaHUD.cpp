#include "OpenWillowMayaHUD.h"
#include "OpenWillowCombatTarget.h"
#include "OpenWillowInventory.h"
#include "OpenWillowInventoryVm.h"
#include "HAL/PlatformMisc.h"
#include "Misc/Paths.h"
#include "OpenWillowInventoryPickup.h"
#include "OpenWillowInventoryMayaDisplay.h"
#include "OpenWillowInventoryPreviewActor.h"
#include "OpenWillowSkills.h"
#include "OpenWillowWalker.h"
#include "CanvasItem.h"
#include "Engine/Canvas.h"
#include "Engine/Engine.h"
#include "Engine/Font.h"
#include "EngineUtils.h"
#include "GameFramework/PlayerController.h"
#include "Camera/PlayerCameraManager.h"
#include "Engine/GameViewportClient.h"
#include "Misc/CommandLine.h"
#include "Misc/Parse.h"
#include "Dom/JsonObject.h"
#include "Serialization/JsonReader.h"
#include "Serialization/JsonSerializer.h"
#include "SWebBrowser.h"
#include "WebBrowserModule.h"
#include "Widgets/Layout/SBox.h"
#include "Framework/Application/SlateApplication.h"
#include "InputCoreTypes.h"
#include "GlobalRenderResources.h"

// Host HUD laid out after BL2's (slanted shield/health bottom-left, XP bar
// bottom-centre with the action skill, angled ammo panel bottom-right). It is
// drawn with canvas primitives, not BL2's Scaleform movies, so shapes, fonts
// and icons are approximations. Controls: LMB fire, F Phaselock, 1/0 equip.
namespace
{
const FLinearColor Backing(0.02f, 0.03f, 0.04f, 0.65f);
const FLinearColor Frame(0.85f, 0.85f, 0.8f, 0.9f);
const FLinearColor Legendary(1.f, 0.55f, 0.08f);
const FLinearColor ShieldBlue(0.2f, 0.7f, 1.f);
const FLinearColor HealthRed(0.85f, 0.1f, 0.08f);
const FLinearColor Experience(0.95f, 0.75f, 0.15f);
const FLinearColor PhaselockViolet(0.7f, 0.35f, 1.f);

// BL2's item rarity colours: white, green, blue, purple, orange.
FLinearColor RarityColor(int32 Rarity)
{
    switch (Rarity)
    {
    case 2: return FLinearColor(0.25f, 0.9f, 0.2f);
    case 3: return FLinearColor(0.2f, 0.5f, 1.f);
    case 4: return FLinearColor(0.65f, 0.3f, 1.f);
    case 5: return Legendary;
    default: return FLinearColor::White;
    }
}

// A parallelogram slanted like BL2's bars; Skew shifts the top edge right.
void Slant(UCanvas& Canvas, float X, float Y, float W, float H, float Skew, const FLinearColor& Color)
{
    if (W <= 0.f) return;
    const FVector2D A(X, Y + H), B(X + W, Y + H), C(X + W + Skew, Y), D(X + Skew, Y);
    FCanvasTriangleItem First(A, B, C, GWhiteTexture);
    First.SetColor(Color);
    First.BlendMode = SE_BLEND_Translucent;
    Canvas.DrawItem(First);
    FCanvasTriangleItem Second(A, C, D, GWhiteTexture);
    Second.SetColor(Color);
    Second.BlendMode = SE_BLEND_Translucent;
    Canvas.DrawItem(Second);
}

void SlantBar(UCanvas& Canvas, float X, float Y, float W, float H, float Skew, float Fill, const FLinearColor& Color)
{
    Slant(Canvas, X - 3.f, Y - 3.f, W + 6.f, H + 6.f, Skew, Backing);
    Slant(Canvas, X, Y, W, H, Skew, Color * 0.3f);
    Slant(Canvas, X, Y, W * FMath::Clamp(Fill, 0.f, 1.f), H, Skew, Color);
    // A lighter top strip gives the bars BL2's bevelled look.
    Slant(Canvas, X + Skew * 0.6f, Y, W * FMath::Clamp(Fill, 0.f, 1.f), H * 0.3f, Skew * 0.4f,
        FLinearColor(1.f, 1.f, 1.f, 0.18f));
}

void Outlined(AHUD& Hud, const FString& Text, const FLinearColor& Color, float X, float Y, UFont* Font, float Scale)
{
    const FLinearColor Edge(0.f, 0.f, 0.f, Color.A);
    for (const FVector2D Offset : {FVector2D(-2, 0), FVector2D(2, 0), FVector2D(0, -2), FVector2D(0, 2)})
        Hud.DrawText(Text, Edge, X + Offset.X, Y + Offset.Y, Font, Scale);
    Hud.DrawText(Text, Color, X, Y, Font, Scale);
}
}

void AOpenWillowMayaHUD::BeginPlay()
{
    Super::BeginPlay();
    FParse::Value(FCommandLine::Get(), TEXT("owflashskills="), SkillsUrl);
    FParse::Value(FCommandLine::Get(), TEXT("owflashinventory="), InventoryUrl);
    if (!InventoryUrl.IsEmpty() && GEngine && GEngine->GameViewport
        && IWebBrowserModule::Get().IsWebModuleAvailable())
    {
        CachedInventoryBrowser = CreateStatusBrowser(InventoryUrl);
        CachedInventoryRoot = SNew(SBox).Visibility(EVisibility::Hidden)[CachedInventoryBrowser.ToSharedRef()];
        GEngine->GameViewport->AddViewportWidgetContent(CachedInventoryRoot.ToSharedRef(), 20);
        UE_LOG(LogTemp, Display, TEXT("OpenWillow Inventory movie preloading: %s"), *InventoryUrl);
    }
    FString Url;
    if (!FParse::Value(FCommandLine::Get(), TEXT("owflashhud="), Url) || !GEngine || !GEngine->GameViewport) return;
    // SWebBrowser only creates a window once the WebBrowser module is loaded;
    // the WebBrowserWidget plugin normally does that, and we do not use it.
    if (!IWebBrowserModule::Get().IsWebModuleAvailable())
    {
        UE_LOG(LogTemp, Warning, TEXT("OpenWillow Flash HUD: the engine's web browser (CEF) is not available"));
        return;
    }
    // Transparent and hit-test invisible, so mouse and keys still reach the game.
    FlashHud = SNew(SWebBrowser)
        .InitialURL(Url)
        .ShowControls(false)
        .ShowAddressBar(false)
        .ShowErrorMessage(true)
        .ShowInitialThrobber(false)
        .SupportsTransparency(true)
        .BackgroundColor(FColor(0, 0, 0, 0))
        .BrowserFrameRate(30)
        .OnLoadCompleted_Lambda([] { UE_LOG(LogTemp, Display, TEXT("OpenWillow Flash HUD page loaded")); })
        .OnLoadError_Lambda([] { UE_LOG(LogTemp, Warning, TEXT("OpenWillow Flash HUD page failed to load; is tools/hud_overlay/serve.py running?")); })
        .OnConsoleMessage_Lambda([](const FString& Message, const FString& Source, int32 Line, EWebBrowserConsoleLogSeverity)
            { UE_LOG(LogTemp, Display, TEXT("OpenWillow Flash HUD console: %s (%s:%d)"), *Message, *Source, Line); });
    FlashHudRoot = SNew(SBox).Visibility(EVisibility::HitTestInvisible)[FlashHud.ToSharedRef()];
    GEngine->GameViewport->AddViewportWidgetContent(FlashHudRoot.ToSharedRef(), 5);
    UE_LOG(LogTemp, Display, TEXT("OpenWillow Flash HUD overlay: %s"), *Url);
}

void AOpenWillowMayaHUD::ToggleSkills()
{
    if (SkillsBrowser && !bInventoryOpen) { CloseSkills(); return; }
    CloseSkills();
    OpenStatusMenu(false);
}

bool AOpenWillowMayaHUD::ToggleInventory()
{
    if (InventoryUrl.IsEmpty()) return false;
    if (SkillsBrowser && bInventoryOpen) { CloseSkills(); return true; }
    CloseSkills();
    OpenStatusMenu(true);
    return SkillsBrowser.IsValid();
}

void AOpenWillowMayaHUD::OpenStatusMenu(bool bInventory)
{
    const FString& MenuUrl = bInventory ? InventoryUrl : SkillsUrl;
    if (MenuUrl.IsEmpty() || !GEngine || !GEngine->GameViewport || !PlayerOwner) return;
    if (!IWebBrowserModule::Get().IsWebModuleAvailable())
    {
        UE_LOG(LogTemp, Warning, TEXT("OpenWillow Skills: the engine's web browser is not available"));
        return;
    }
    bInventoryOpen = bInventory;
    if (bInventory)
    {
        AOpenWillowWalker* Maya = Cast<AOpenWillowWalker>(PlayerOwner->GetPawn());
        if (Maya) Maya->SetInventoryPresentation(true);
        FActorSpawnParameters Spawn;
        Spawn.Owner = Maya;
        Spawn.SpawnCollisionHandlingOverride = ESpawnActorCollisionHandlingMethod::AlwaysSpawn;
        InventoryMayaDisplay = GetWorld()->SpawnActor<AOpenWillowInventoryMayaDisplay>(
            AOpenWillowInventoryMayaDisplay::StaticClass(), FTransform::Identity, Spawn);
    }
    if (bInventory && CachedInventoryBrowser && CachedInventoryRoot)
    {
        SkillsBrowser = CachedInventoryBrowser;
        SkillsRoot = CachedInventoryRoot;
        CachedInventoryRoot->SetVisibility(EVisibility::Visible);
    }
    else
    {
        SkillsBrowser = CreateStatusBrowser(MenuUrl);
        SkillsRoot = SNew(SBox)[SkillsBrowser.ToSharedRef()];
        GEngine->GameViewport->AddViewportWidgetContent(SkillsRoot.ToSharedRef(), 20);
    }
    if (bInventory) SkillsBrowser->ExecuteJavascript(TEXT("window.owRefreshMenuPreview && window.owRefreshMenuPreview()"));
    if (bInventory)
    {
        if (!InventoryVm)
        {
            const FString Game = FPlatformMisc::GetEnvironmentVariable(TEXT("OPENWILLOW_BL2"));
            InventoryVm = MakeShared<FOpenWillowInventoryVm>(FPaths::Combine(Game, TEXT("WillowGame/CookedPCConsole")));
            UE_LOG(LogTemp, Display, TEXT("OWINVVM initialized ready=%d error=%s"), InventoryVm->IsReady(), *InventoryVm->Error());
        }
    }
    if (FlashHudRoot) FlashHudRoot->SetVisibility(EVisibility::Collapsed);
    FInputModeUIOnly Mode;
    Mode.SetWidgetToFocus(SkillsBrowser.ToSharedRef());
    PlayerOwner->SetInputMode(Mode);
    PlayerOwner->SetShowMouseCursor(true);
    NextSkillsPush = 0.f;
    UE_LOG(LogTemp, Display, TEXT("OpenWillow Status menu overlay: %s"), *MenuUrl);
}

TSharedPtr<SWebBrowser> AOpenWillowMayaHUD::CreateStatusBrowser(const FString& MenuUrl)
{
    return SNew(SWebBrowser)
        .InitialURL(MenuUrl)
        .ShowControls(false)
        .ShowAddressBar(false)
        .ShowErrorMessage(true)
        .ShowInitialThrobber(false)
        .SupportsTransparency(true)
        .BackgroundColor(FColor(0, 0, 0, 0))
        .BrowserFrameRate(30)
        .OnBeforeNavigation_Lambda([this](const FString& NextUrl, const FWebNavigationRequest&)
            {
                // Header tab routes: consumed like the close routes, applied in DrawHUD.
                if (NextUrl.EndsWith(TEXT("/__ow_tab_inventory")) || NextUrl.EndsWith(TEXT("/__ow_tab_skills")))
                {
                    PendingTabSwitch = NextUrl.EndsWith(TEXT("/__ow_tab_inventory")) ? 1 : 2;
                    return true;
                }
                if (!NextUrl.EndsWith(TEXT("/__ow_close_skills")) && !NextUrl.EndsWith(TEXT("/__ow_close_inventory"))) return false;
                bCloseSkillsRequested = true;
                UE_LOG(LogTemp, Display, TEXT("OpenWillow Skills close requested by page"));
                return true; // Consume the page's close route without navigating away.
            })
        .OnLoadCompleted_Lambda([] { UE_LOG(LogTemp, Display, TEXT("OpenWillow Skills page loaded")); })
        .OnLoadError_Lambda([] { UE_LOG(LogTemp, Warning, TEXT("OpenWillow Skills page failed to load")); })
        .OnConsoleMessage_Lambda([this](const FString& Message, const FString& Source, int32 Line, EWebBrowserConsoleLogSeverity)
            {
                UE_LOG(LogTemp, Display, TEXT("OpenWillow Skills console: %s (%s:%d)"), *Message, *Source, Line);
                OnSkillsConsole(Message);
            });
}

void AOpenWillowMayaHUD::CloseSkills()
{
    if (!SkillsBrowser) return;
    if (bInventoryOpen) SkillsBrowser->ExecuteJavascript(TEXT("window.owCancelInventoryVm && window.owCancelInventoryVm()"));
    if (IsValid(InspectActor)) InspectActor->Destroy();
    InspectActor = nullptr;
    PendingInspectRequest.Reset();
    bMenuPreviewRequested = false;
    if (bInventoryOpen)
    {
        if (AOpenWillowWalker* Maya = PlayerOwner ? Cast<AOpenWillowWalker>(PlayerOwner->GetPawn()) : nullptr)
            Maya->SetInventoryPresentation(false);
        if (IsValid(InventoryMayaDisplay)) InventoryMayaDisplay->Destroy();
        InventoryMayaDisplay = nullptr;
    }
    if (bInventoryOpen && CachedInventoryRoot)
        CachedInventoryRoot->SetVisibility(EVisibility::Hidden);
    else if (SkillsRoot && GEngine && GEngine->GameViewport)
        GEngine->GameViewport->RemoveViewportWidgetContent(SkillsRoot.ToSharedRef());
    SkillsRoot.Reset();
    SkillsBrowser.Reset();
    bCloseSkillsRequested = false;
    PendingTabSwitch = 0;
    PendingSpends.Reset();
    PendingInventoryActions.Reset();
    PendingInventoryVmMoves.Reset();
    bInventoryOpen = false;
    if (FlashHudRoot) FlashHudRoot->SetVisibility(EVisibility::HitTestInvisible);
    if (PlayerOwner)
    {
        PlayerOwner->SetInputMode(FInputModeGameOnly());
        PlayerOwner->SetShowMouseCursor(false);
    }
    UE_LOG(LogTemp, Display, TEXT("OpenWillow Skills overlay closed"));
}

void AOpenWillowMayaHUD::OnSkillsConsole(const FString& Message)
{
    if (bInventoryOpen)
    {
        if (Message.StartsWith(TEXT("OWINVMOVE "), ESearchCase::CaseSensitive))
        {
            if (Message.Len() <= 1024 && PendingInventoryVmMoves.Num() < 64)
                PendingInventoryVmMoves.Add(Message.Mid(10));
            return;
        }
        if (Message.StartsWith(TEXT("OWMENUPREVIEW "), ESearchCase::CaseSensitive) && Message.Len() <= 1024)
        {
            TSharedPtr<FJsonObject> Request;
            if (FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Message.Mid(14)), Request) && Request
                && Request->TryGetStringField(TEXT("id"), PendingMenuPreviewId)) bMenuPreviewRequested = true;
            return;
        }
        if (Message.StartsWith(TEXT("OWINSPECT "), ESearchCase::CaseSensitive))
        {
            if (Message.Len() <= 1024) PendingInspectRequest = Message.Mid(10);
            return;
        }
        // -owinventoryactions test hook: the page's answer to RequestPageReport.
        if (Message.StartsWith(TEXT("OWINVPAGE "), ESearchCase::CaseSensitive))
        {
            LastPageReport = Message.Mid(10);
            ++PageReportCount;
            return;
        }
        // Requests are queued and validated against the host inventory on the
        // game thread. Accept the previous equip prefix during adapter updates.
        if (Message.StartsWith(TEXT("OWITEM "), ESearchCase::CaseSensitive)
            || Message.StartsWith(TEXT("OWEQUIP "), ESearchCase::CaseSensitive))
        {
            if (Message.Len() <= 4096 && PendingInventoryActions.Num() < 64)
                PendingInventoryActions.Add(Message);
        }
        return;
    }
    // The page reports a skill click as console.log('OWSKILL {"branch":B,
    // "tier":T,"cell":C}'); the host decides whether it is a valid spend.
    static const FString Prefix = TEXT("OWSKILL ");
    if (!Message.StartsWith(Prefix, ESearchCase::CaseSensitive)) return;
    const FString Json = Message.Mid(Prefix.Len());
    TSharedPtr<FJsonObject> Click;
    int32 Branch = 0, Tier = 0, Cell = 0;
    if (!FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Json), Click) || !Click
        || !Click->TryGetNumberField(TEXT("branch"), Branch) || !Click->TryGetNumberField(TEXT("tier"), Tier)
        || !Click->TryGetNumberField(TEXT("cell"), Cell))
    {
        UE_LOG(LogTemp, Warning, TEXT("OpenWillow Skills: unreadable page message %s"), *Message);
        return;
    }
    PendingSpends.Add(FIntVector(Branch, Tier, Cell));
}

void AOpenWillowMayaHUD::PushSkillsState()
{
    const AOpenWillowWalker* Maya = PlayerOwner ? Cast<AOpenWillowWalker>(PlayerOwner->GetPawn()) : nullptr;
    const UOpenWillowSkills* Skills = Maya ? Maya->GetSkills() : nullptr;
    NextSkillsPush = GetWorld()->GetRealTimeSeconds() + 1.f;
    if (!SkillsBrowser || !Skills) return;
    if (bInventoryOpen)
    {
        const UOpenWillowInventory* Inventory = Maya->GetInventory();
        SkillsBrowser->ExecuteJavascript(FString::Printf(TEXT("window.owConfigureInventoryVm && window.owConfigureInventoryVm(%s)"),
            InventoryVm && InventoryVm->IsReady() ? TEXT("true") : TEXT("false")));
        if (Inventory) SkillsBrowser->ExecuteJavascript(FString::Printf(TEXT("window.owInventory && window.owInventory(%s)"),
            *Inventory->StateJson(Skills->GetLevel())));
        return;
    }
    SkillsBrowser->ExecuteJavascript(FString::Printf(TEXT("window.owSkills && window.owSkills(%s)"), *Skills->StateJson()));
}

void AOpenWillowMayaHUD::SendPageKey(const FString& Key)
{
    if (!SkillsBrowser || Key.Len() > 16) return;
    SkillsBrowser->ExecuteJavascript(FString::Printf(
        TEXT("window.dispatchEvent(new KeyboardEvent('keydown',{key:'%s',bubbles:true}))"), *Key));
}

bool AOpenWillowMayaHUD::SendSlateKey(const FString& Key)
{
    if (!SkillsBrowser || !FSlateApplication::IsInitialized()) return false;
    // Page key names -> (UE key, Windows virtual-key code, character code).
    struct FKeyMap { const TCHAR* Name; FKey UeKey; uint32 VirtualKey; uint32 Character; };
    static const FKeyMap Map[] = {
        {TEXT("ArrowDown"), EKeys::Down, 40, 0}, {TEXT("ArrowUp"), EKeys::Up, 38, 0},
        {TEXT("ArrowLeft"), EKeys::Left, 37, 0}, {TEXT("ArrowRight"), EKeys::Right, 39, 0},
        {TEXT("PageUp"), EKeys::PageUp, 33, 0}, {TEXT("PageDown"), EKeys::PageDown, 34, 0},
        {TEXT("Enter"), EKeys::Enter, 13, 13}, {TEXT("Delete"), EKeys::Delete, 46, 0},
        {TEXT("Escape"), EKeys::Escape, 27, 0}, {TEXT("Tab"), EKeys::Tab, 9, 9},
        {TEXT("1"), EKeys::One, '1', '1'}, {TEXT("2"), EKeys::Two, '2', '2'},
        {TEXT("3"), EKeys::Three, '3', '3'}, {TEXT("4"), EKeys::Four, '4', '4'},
        {TEXT("e"), EKeys::E, 'E', 'e'}, {TEXT("f"), EKeys::F, 'F', 'f'},
        {TEXT("q"), EKeys::Q, 'Q', 'q'}, {TEXT("t"), EKeys::T, 'T', 't'},
        {TEXT("v"), EKeys::V, 'V', 'v'}, {TEXT("u"), EKeys::U, 'U', 'u'},
        {TEXT("k"), EKeys::K, 'K', 'k'}, {TEXT("i"), EKeys::I, 'I', 'i'},
        {TEXT("["), EKeys::LeftBracket, 0xDB, '['}, {TEXT("]"), EKeys::RightBracket, 0xDD, ']'},
    };
    for (const FKeyMap& Entry : Map)
    {
        if (!Key.Equals(Entry.Name, ESearchCase::CaseSensitive)) continue;
        const FKeyEvent Event(Entry.UeKey, FModifierKeysState(), uint32(0), false, Entry.Character, Entry.VirtualKey);
        FSlateApplication& Slate = FSlateApplication::Get();
        Slate.ProcessKeyDownEvent(Event);
        Slate.ProcessKeyUpEvent(Event);
        return true;
    }
    return false;
}

void AOpenWillowMayaHUD::RequestPageReport()
{
    if (!SkillsBrowser) return;
    // Reads the page's own globals (selection, target slot, the last snapshot
    // it accepted); a page that has not loaded yet reports the error instead.
    SkillsBrowser->ExecuteJavascript(TEXT(
        "try{console.log('OWINVPAGE '+JSON.stringify({ready:!!ready,hasState:!!state,"
        "sel:selectedId,target:targetSlot,gear:targetGearSlot,cat:categoryIndex,sort:sortIndex,"
        "inspect:inspectMode,inspectFrames:inspectFrameCount,inspectImageBytes:inspectImage.length,inspectYaw:inspectYaw,"
        "vm:{enabled:inventoryVm.enabled,calls:inventoryVm.calls,errors:inventoryVm.errors,steps:inventoryVm.steps,discarded:inventoryVm.discarded},"
        "transfer:transferSourceId,transferFromEquipped:transferFromEquipped,compare:compareId,"
        "backpack:backpackItems().map(i=>i.id),firstRow:firstRow,visibleBackpack:Array.from(document.querySelectorAll('[data-kind=backpack]:not([data-partial=true])')).map(n=>n.dataset.itemId),"
        "backpackHeaderAnchored:(function(){var row=document.querySelector('[data-kind=backpack]');var buttons=Array.from(document.querySelectorAll('[data-kind=category]'));"
        "return !!row&&buttons.length===2&&buttons.every(function(n){var b=n.getBoundingClientRect(),r=row.getBoundingClientRect();return b.bottom<=r.top+2&&b.top>=r.top-60})})(),"
        "backpackRowsAligned:(function(){var stage=document.getElementById('stage').getBoundingClientRect(),scale=stage.width/1280;"
        "return Array.from(document.querySelectorAll('[data-kind=backpack]:not([data-partial=true])')).every(function(n,i){"
        "var a=readBounds(INV+'.storagePanel.owRows.owRow'+i+'.hitTestClip'),b=n.getBoundingClientRect();"
        "return !!a&&Math.abs(stage.left+a.xMin*scale-b.left)<2&&Math.abs(stage.top+a.yMin*scale-b.top)<2})})(),"
        "mainCardBounds:readBounds(INV+'.mainCard.bkgd'),compareCardBounds:readBounds(INV+'.compareCard.bkgd'),"
        "compareStatsVisible:!!get(INV+'.mainCard.stat1.mainField','_visible')&&!!get(INV+'.compareCard.stat1.mainField','_visible'),"
        "level:state?state.level:null,slots:state?state.slots:null,gearSlots:state?state.gearSlots:null,"
        "count:state?state.backpackCount:null,"
        "items:state?state.items.map(function(i){return [i.id,i.favorite?1:0,i.trash?1:0]}):[]}))}"
        "catch(e){console.log('OWINVPAGE '+JSON.stringify({error:String(e)}))}"));
}

void AOpenWillowMayaHUD::InjectPageRequest(const FString& Json)
{
    if (!SkillsBrowser || Json.Contains(TEXT("'")) || Json.Contains(TEXT("\\"))) return;
    SkillsBrowser->ExecuteJavascript(FString::Printf(TEXT("console.log('OWITEM %s')"), *Json));
}

void AOpenWillowMayaHUD::SendPageDrag(const FString& Id, int32 DestinationSlot)
{
    if (!SkillsBrowser) return;
    // JSON serialization keeps item IDs out of JavaScript source quoting.
    TSharedPtr<FJsonObject> Args = MakeShared<FJsonObject>();
    Args->SetStringField(TEXT("id"), Id);
    Args->SetNumberField(TEXT("slot"), DestinationSlot);
    FString Json;
    FJsonSerializer::Serialize(Args.ToSharedRef(), TJsonWriterFactory<>::Create(&Json));
    SkillsBrowser->ExecuteJavascript(FString::Printf(TEXT(
        "(function(a){var source=Array.from(document.querySelectorAll('[data-item-id]')).find(function(n){return n.dataset.itemId===a.id});"
        "var target=a.slot<0?document.querySelector('[data-kind=backpack-zone]'):document.querySelector('[data-kind=slot][data-slot=\"'+a.slot+'\"]');"
        "if(!source||!target){console.log('OWDRAGTEST missing source/target');return;}"
        "var transfer=new DataTransfer();"
        "source.dispatchEvent(new DragEvent('dragstart',{bubbles:true,cancelable:true,dataTransfer:transfer}));"
        "target.dispatchEvent(new DragEvent('dragover',{bubbles:true,cancelable:true,dataTransfer:transfer}));"
        "target.dispatchEvent(new DragEvent('drop',{bubbles:true,cancelable:true,dataTransfer:transfer}));"
        "source.dispatchEvent(new DragEvent('dragend',{bubbles:true,dataTransfer:transfer}));})(%s)"), *Json));
}

void AOpenWillowMayaHUD::SendPageInspectDrag()
{
    if (!SkillsBrowser) return;
    SkillsBrowser->ExecuteJavascript(TEXT("(()=>{const p=document.getElementById('inspect-preview');"
        "for(const [type,x] of [['pointerdown',100],['pointermove',220],['pointerup',220]])"
        "p.dispatchEvent(new PointerEvent(type,{bubbles:true,pointerId:7,button:0,clientX:x,clientY:100}));})()"));
}

void AOpenWillowMayaHUD::SendPageBackpackWheel(int32 PixelDelta)
{
    if (!SkillsBrowser) return;
    SkillsBrowser->ExecuteJavascript(FString::Printf(TEXT("document.querySelector('[data-kind=backpack-zone]')"
        "?.dispatchEvent(new WheelEvent('wheel',{bubbles:true,cancelable:true,deltaY:%d,deltaMode:0}))"), PixelDelta));
}

void AOpenWillowMayaHUD::RequestSkillsCloseFromPage()
{
    if (SkillsBrowser)
        SkillsBrowser->ExecuteJavascript(TEXT("window.location.href='/__ow_close_skills'"));
}

void AOpenWillowMayaHUD::EndPlay(const EEndPlayReason::Type Reason)
{
    CloseSkills();
    if (CachedInventoryRoot && GEngine && GEngine->GameViewport)
        GEngine->GameViewport->RemoveViewportWidgetContent(CachedInventoryRoot.ToSharedRef());
    CachedInventoryRoot.Reset();
    CachedInventoryBrowser.Reset();
    if (FlashHudRoot && GEngine && GEngine->GameViewport)
        GEngine->GameViewport->RemoveViewportWidgetContent(FlashHudRoot.ToSharedRef());
    FlashHudRoot.Reset();
    FlashHud.Reset();
    Super::EndPlay(Reason);
}

void AOpenWillowMayaHUD::PushFlashHudState(const AOpenWillowWalker& Maya)
{
    // Only state the host really has: Maya takes no damage and has no
    // grenade or magazine tracking yet, so vitals stay full, grenades are
    // hidden and the ammo text is the magazine size. XP and the level come
    // from UOpenWillowSkills. Field meanings are in
    // tools/hud_overlay/index.html.
    const UOpenWillowInventory* Inventory = Maya.GetInventory();
    const UOpenWillowSkills* Skills = Maya.GetSkills();
    const FOpenWillowWeaponItem* Weapon = Inventory ? Inventory->ActiveWeapon() : nullptr;
    const bool bArmed = Maya.HasWeaponOut() && Weapon;
    // Rounds left in the held magazine (a no-cost weapon like the Infinity
    // shows its full magazine size, as before).
    const int32 MagazineLeft = bArmed ? UOpenWillowInventory::MagazineLeft(*Weapon) : 0;
    const float MagazineFill = bArmed ? float(MagazineLeft) / UOpenWillowInventory::MagazineSize(*Weapon) : 1.f;
    const FString State = FString::Printf(
        TEXT("{\"character\":\"siren\",\"health\":1,\"shield\":1,\"healthText\":\"\",\"shieldText\":\"\","
             "\"xp\":%.3f,\"levelText\":\"%d\",\"grenades\":null,\"weaponOut\":%s,\"ammo\":%.3f,\"ammoText\":\"%s\"}"),
        Skills ? Skills->LevelProgress() : 0.f,
        Skills ? Skills->GetLevel() : 1,
        bArmed ? TEXT("true") : TEXT("false"),
        MagazineFill,
        bArmed ? *FString::Printf(TEXT("%d"), MagazineLeft) : TEXT(""));
    // Resend once a second too: the page may not have loaded the first time.
    const float Now = GetWorld()->GetRealTimeSeconds();
    if (State == LastFlashState && Now < NextFlashPush) return;
    FlashHud->ExecuteJavascript(FString::Printf(TEXT("window.owHud && window.owHud(%s)"), *State));
    LastFlashState = State;
    NextFlashPush = Now + 1.f;
}

void AOpenWillowMayaHUD::DrawDamagePopups(UFont* Font)
{
    // Damage numbers pop in large, rise and fade over each hit location.
    const float Now = GetWorld()->GetTimeSeconds();
    for (TActorIterator<AOpenWillowCombatTarget> It(GetWorld()); It; ++It)
    {
        for (const FOpenWillowDamagePopup& Popup : It->Popups)
        {
            const float Age = Now - Popup.Born;
            const FVector Screen = Project(Popup.Location + FVector(0, 0, 70.f * Age), false);
            if (Screen.Z <= 0.f) continue;
            const float Alpha = FMath::Clamp(1.f - Age / 1.2f, 0.f, 1.f);
            const float Scale = 1.9f + 0.8f * FMath::Max(0.f, 1.f - Age / 0.15f);
            Outlined(*this, FString::Printf(TEXT("%.0f"), Popup.Amount),
                FLinearColor(1.f, 0.95f, 0.8f, Alpha), Screen.X, Screen.Y, Font, Scale);
        }
    }
}

void AOpenWillowMayaHUD::DrawHUD()
{
    Super::DrawHUD();
    if (bInventoryOpen && bMenuPreviewRequested && IsValid(InventoryMayaDisplay))
    {
        bMenuPreviewRequested = false;
        const AOpenWillowWalker* Maya = PlayerOwner ? Cast<AOpenWillowWalker>(PlayerOwner->GetPawn()) : nullptr;
        const UOpenWillowInventory* Inventory = Maya ? Maya->GetInventory() : nullptr;
        const FOpenWillowWeaponItem* Item = Inventory ? Inventory->FindItemById(PendingMenuPreviewId) : nullptr;
        InventoryMayaDisplay->SetPreviewWeapon(Item ? Item->Id : FString());
    }
    if (SkillsBrowser && bInventoryOpen && !PendingInspectRequest.IsEmpty()
        && GetWorld()->GetRealTimeSeconds() >= NextInspectFrame)
    {
        NextInspectFrame = GetWorld()->GetRealTimeSeconds() + .1;
        TSharedPtr<FJsonObject> Request;
        FString Id;
        double Yaw = 0, Pitch = 0;
        const AOpenWillowWalker* Maya = PlayerOwner ? Cast<AOpenWillowWalker>(PlayerOwner->GetPawn()) : nullptr;
        const UOpenWillowInventory* Inventory = Maya ? Maya->GetInventory() : nullptr;
        if (FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(PendingInspectRequest), Request) && Request
            && Request->TryGetStringField(TEXT("id"), Id) && Request->TryGetNumberField(TEXT("yaw"), Yaw)
            && Request->TryGetNumberField(TEXT("pitch"), Pitch) && FMath::IsFinite(Yaw) && FMath::IsFinite(Pitch) && Inventory)
        {
            const FOpenWillowWeaponItem* Item = Inventory->FindItemById(Id);
            FString InspectPng;
            if (Item)
            {
                if (!IsValid(InspectActor)) InspectActor = GetWorld()->SpawnActor<AOpenWillowInventoryPreviewActor>(
                    AOpenWillowInventoryPreviewActor::StaticClass(), FVector(0, 0, -100000), FRotator::ZeroRotator);
                if (InspectActor && InspectActor->SetItemPreview(Item->Id))
                    InspectPng = InspectActor->InspectFrame(FMath::Fmod(Yaw, 360.0), FMath::Clamp(Pitch, -80.0, 80.0));
            }
            TSharedRef<FJsonObject> Reply = MakeShared<FJsonObject>();
            Reply->SetStringField(TEXT("id"), Id);
            Reply->SetStringField(TEXT("image"), InspectPng);
            FString Json;
            FJsonSerializer::Serialize(Reply, TJsonWriterFactory<>::Create(&Json));
            SkillsBrowser->ExecuteJavascript(FString::Printf(TEXT("window.owInspectFrame && window.owInspectFrame(%s)"), *Json));
        }
        PendingInspectRequest.Reset();
    }
    if (bInventoryOpen && IsValid(InventoryMayaDisplay) && PlayerOwner && PlayerOwner->PlayerCameraManager)
        InventoryMayaDisplay->UpdateView(PlayerOwner->PlayerCameraManager->GetCameraLocation(),
            PlayerOwner->PlayerCameraManager->GetCameraRotation());
    if (bCloseSkillsRequested) CloseSkills();
    if (PendingTabSwitch)
    {
        // Only a page that is open can ask; a stale request after a close is dropped.
        const bool bToInventory = PendingTabSwitch == 1;
        PendingTabSwitch = 0;
        if (SkillsBrowser && bInventoryOpen != bToInventory)
        {
            CloseSkills();
            OpenStatusMenu(bToInventory);
        }
    }
    if (SkillsBrowser && bInventoryOpen && InventoryVm && PendingInventoryVmMoves.Num())
    {
        for (const FString& Request : PendingInventoryVmMoves)
            SkillsBrowser->ExecuteJavascript(FString::Printf(TEXT("window.owInventoryVmResult && window.owInventoryVmResult(%s)"), *InventoryVm->Move(Request)));
        PendingInventoryVmMoves.Reset();
    }
    if (SkillsBrowser && bInventoryOpen && PendingInventoryActions.Num())
    {
        AOpenWillowWalker* Maya = PlayerOwner ? Cast<AOpenWillowWalker>(PlayerOwner->GetPawn()) : nullptr;
        UOpenWillowInventory* Inventory = Maya ? Maya->GetInventory() : nullptr;
        if (Inventory && Maya) for (const FString& Message : PendingInventoryActions)
        {
            const bool bLegacyEquip = Message.StartsWith(TEXT("OWEQUIP "), ESearchCase::CaseSensitive);
            const FString Json = Message.Mid(bLegacyEquip ? 8 : 7);
            TSharedPtr<FJsonObject> Request;
            FString Action, Id, GearSlot;
            bool bAccepted = false;
            double SlotValue = -1;
            const bool bValidJson = FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Json), Request) && Request;
            if (bValidJson)
            {
                if (bLegacyEquip) Action = TEXT("equip");
                else Request->TryGetStringField(TEXT("action"), Action);
                Request->TryGetStringField(TEXT("id"), Id);
                Request->TryGetStringField(TEXT("gearSlot"), GearSlot);
                const bool bHasSlot = Request->TryGetNumberField(TEXT("slot"), SlotValue)
                    && FMath::IsFinite(SlotValue) && SlotValue >= 0
                    && SlotValue < UOpenWillowInventory::SlotCount
                    && SlotValue == FMath::FloorToDouble(SlotValue);
                if (Action == TEXT("equip") && bHasSlot && !Id.IsEmpty())
                {
                    const int32 ItemIndex = Inventory->FindItemIndexById(Id);
                    bAccepted = ItemIndex != INDEX_NONE && Maya->EquipItem(ItemIndex, int32(SlotValue));
                }
                else if (Action == TEXT("equip") && !GearSlot.IsEmpty() && !Id.IsEmpty())
                {
                    const UOpenWillowSkills* Skills = Maya->GetSkills();
                    bAccepted = Skills && Inventory->EquipGearById(Id, GearSlot, Skills->GetLevel());
                }
                else if (Action == TEXT("unequip") && bHasSlot)
                {
                    bAccepted = Maya->UnequipSlot(int32(SlotValue));
                }
                else if (Action == TEXT("unequip") && !GearSlot.IsEmpty())
                {
                    bAccepted = Inventory->UnequipGear(GearSlot);
                }
                else if (Action == TEXT("drop") && !Id.IsEmpty())
                {
                    // Spawn first so a failed world spawn never removes the
                    // item from the player's inventory.
                    const FVector Ahead = Maya->GetActorLocation() + Maya->GetActorForwardVector() * 160.f;
                    FHitResult Ground;
                    FCollisionQueryParams Query(SCENE_QUERY_STAT(OpenWillowDrop), false, Maya);
                    const bool bFoundGround = GetWorld()->LineTraceSingleByChannel(Ground,
                        Ahead + FVector(0.f, 0.f, 100.f), Ahead - FVector(0.f, 0.f, 450.f),
                        ECC_WorldStatic, Query);
                    const FVector DropLocation = bFoundGround ? Ground.ImpactPoint + FVector(0.f, 0.f, 12.f) : Ahead;
                    FActorSpawnParameters Spawn;
                    Spawn.SpawnCollisionHandlingOverride = ESpawnActorCollisionHandlingMethod::AlwaysSpawn;
                    AOpenWillowInventoryPickup* Pickup = GetWorld()->SpawnActor<AOpenWillowInventoryPickup>(
                        AOpenWillowInventoryPickup::StaticClass(), FTransform(DropLocation), Spawn);
                    if (Pickup)
                    {
                        FOpenWillowTakenInventoryItem Dropped;
                        bAccepted = Maya->TakeInventoryItemById(Id, Dropped);
                        if (bAccepted) Pickup->Initialize(MoveTemp(Dropped));
                        else Pickup->Destroy();
                    }
                }
                else if (Action == TEXT("favorite") && !Id.IsEmpty())
                {
                    bAccepted = Maya->GetInventory()->ToggleFavoriteById(Id);
                }
                else if (Action == TEXT("trash") && !Id.IsEmpty())
                {
                    bAccepted = Maya->GetInventory()->ToggleTrashById(Id);
                }
            }
            LastAction.Serial++;
            LastAction.Action = Action;
            LastAction.Id = Id;
            LastAction.bAccepted = bAccepted;
            UE_LOG(LogTemp, Display, TEXT("OpenWillow Inventory action %s id=%s slot=%.0f gearSlot=%s: %s"),
                *Action, *Id, SlotValue, *GearSlot,
                bAccepted ? TEXT("accepted") : TEXT("rejected: invalid request, item, locked slot, or level requirement"));
        }
        PendingInventoryActions.Reset();
        PushSkillsState();
    }
    if (SkillsBrowser && PendingSpends.Num())
    {
        AOpenWillowWalker* Maya = PlayerOwner ? Cast<AOpenWillowWalker>(PlayerOwner->GetPawn()) : nullptr;
        UOpenWillowSkills* Skills = Maya ? Maya->GetSkills() : nullptr;
        for (const FIntVector& Spend : PendingSpends)
        {
            FString Reason = TEXT("no skill state");
            const bool bAccepted = Skills && Skills->TrySpend(Spend.X, Spend.Y, Spend.Z, Reason);
            UE_LOG(LogTemp, Display, TEXT("OpenWillow Skills spend (%d,%d,%d) %s; %d points left"),
                Spend.X, Spend.Y, Spend.Z, bAccepted ? TEXT("accepted") : *(TEXT("refused: ") + Reason),
                Skills ? Skills->AvailablePoints() : 0);
        }
        PendingSpends.Reset();
        PushSkillsState();
    }
    // Resend once a second too: the page may not have loaded the first time.
    if (SkillsBrowser && GetWorld()->GetRealTimeSeconds() >= NextSkillsPush) PushSkillsState();
    if (SkillsBrowser) return; // The gameplay HUD is hidden behind the menu.
    if (!Canvas || !PlayerOwner) return;
    const AOpenWillowWalker* Maya = Cast<AOpenWillowWalker>(PlayerOwner->GetPawn());
    if (!Maya) return;
    UFont* Small = GEngine->GetSmallFont();
    UFont* Large = GEngine->GetLargeFont();
    if (FlashHud)
    {
        // The movie draws the crosshair, bars and weapon panel; damage
        // numbers are not driven in it yet.
        PushFlashHudState(*Maya);
        DrawDamagePopups(Large);
        return;
    }
    const float W = Canvas->ClipX;
    const float H = Canvas->ClipY;
    const float Now = GetWorld()->GetTimeSeconds();

    // Crosshair: four ticks; a brief X marks a target hit.
    const float X = W * 0.5f;
    const float Y = H * 0.5f;
    const FLinearColor Cross(1.f, 1.f, 1.f, 0.85f);
    DrawLine(X - 12.f, Y, X - 5.f, Y, Cross, 2.f);
    DrawLine(X + 5.f, Y, X + 12.f, Y, Cross, 2.f);
    DrawLine(X, Y - 12.f, X, Y - 5.f, Cross, 2.f);
    DrawLine(X, Y + 5.f, X, Y + 12.f, Cross, 2.f);
    if (Now - Maya->LastTargetHitAt() < 0.12f)
    {
        const FLinearColor Hit(1.f, 0.9f, 0.6f);
        DrawLine(X - 14.f, Y - 14.f, X - 7.f, Y - 7.f, Hit, 2.f);
        DrawLine(X + 14.f, Y - 14.f, X + 7.f, Y - 7.f, Hit, 2.f);
        DrawLine(X - 14.f, Y + 14.f, X - 7.f, Y + 7.f, Hit, 2.f);
        DrawLine(X + 14.f, Y + 14.f, X + 7.f, Y + 7.f, Hit, 2.f);
    }

    DrawDamagePopups(Large);

    // Shield over health, bottom left. No shield is modelled (full bar); health is Maya's own.
    const float BarW = W * 0.19f;
    SlantBar(*Canvas, 46.f, H - 86.f, BarW, 12.f, 8.f, 1.f, ShieldBlue);
    SlantBar(*Canvas, 40.f, H - 62.f, BarW, 20.f, 10.f, FMath::Clamp(Maya->GetHealth() / FMath::Max(1.f, Maya->GetMaxHealth()), 0.f, 1.f), HealthRed);

    // Experience bar with Maya's level and progress, bottom centre (UOpenWillowSkills).
    const UOpenWillowSkills* Skills = Maya->GetSkills();
    const float XpW = W * 0.34f;
    SlantBar(*Canvas, X - XpW * 0.5f, H - 26.f, XpW, 7.f, 4.f, Skills ? Skills->LevelProgress() : 0.f, Experience);
    Outlined(*this, FString::FromInt(Skills ? Skills->GetLevel() : 1), Experience, X - XpW * 0.5f - 22.f, H - 34.f, Large, 1.f);

    // Phaselock icon above the XP bar: a ring that refills over the cooldown.
    const float Remaining = Maya->PhaselockRemaining();
    const float Ready = 1.f - Remaining / FMath::Max(1.f, Maya->PhaselockCooldown());
    const float IconY = H - 72.f;
    const int32 Segments = 40;
    for (int32 I = 0; I < Segments; ++I)
    {
        const float A0 = 2.f * PI * I / Segments - PI * 0.5f;
        const float A1 = 2.f * PI * (I + 1) / Segments - PI * 0.5f;
        const bool bFilled = float(I) / Segments < Ready;
        DrawLine(X + 27.f * FMath::Cos(A0), IconY + 27.f * FMath::Sin(A0),
            X + 27.f * FMath::Cos(A1), IconY + 27.f * FMath::Sin(A1),
            bFilled ? PhaselockViolet : FLinearColor(0.15f, 0.15f, 0.2f, 0.8f), 4.f);
    }
    Slant(*Canvas, X - 16.f, IconY - 16.f, 30.f, 32.f, 2.f,
        Remaining > 0.f ? FLinearColor(0.12f, 0.06f, 0.18f, 0.85f) : FLinearColor(0.4f, 0.16f, 0.62f, 0.9f));
    // Phaselock glyph: a small orb with a ring, not BL2's icon texture.
    const FLinearColor Glyph = Remaining > 0.f ? FLinearColor(0.5f, 0.4f, 0.6f) : FLinearColor::White;
    for (int32 I = 0; I < 16; ++I)
    {
        const float A0 = 2.f * PI * I / 16.f;
        const float A1 = 2.f * PI * (I + 1) / 16.f;
        DrawLine(X + 9.f * FMath::Cos(A0), IconY + 5.f * FMath::Sin(A0),
            X + 9.f * FMath::Cos(A1), IconY + 5.f * FMath::Sin(A1), Glyph, 1.5f);
    }
    DrawRect(Glyph, X - 3.f, IconY - 3.f, 6.f, 6.f);
    if (Remaining > 0.f)
        Outlined(*this, FString::Printf(TEXT("%.0f"), FMath::CeilToFloat(Remaining)), FLinearColor::White,
            X + 30.f, IconY - 12.f, Large, 1.f);

    // Angled weapon panel, bottom right: rarity-coloured name, manufacturer,
    // ammo (infinite when the evaluated shot cost is 0) and the four slots.
    const UOpenWillowInventory* Inventory = Maya->GetInventory();
    const FOpenWillowWeaponItem* Weapon = Inventory ? Inventory->ActiveWeapon() : nullptr;
    EOpenWillowAmmoType HeldAmmo = EOpenWillowAmmoType::Pistol;
    const float PanelW = 270.f;
    const float PanelX = W - PanelW - 50.f;
    const FLinearColor Tint = Weapon ? RarityColor(Weapon->Rarity) : FLinearColor::White;
    Slant(*Canvas, PanelX, H - 96.f, PanelW, 58.f, 14.f, Backing);
    Slant(*Canvas, PanelX, H - 42.f, PanelW, 4.f, 1.f, Tint);
    if (Maya->HasWeaponOut() && Weapon)
    {
        Outlined(*this, Weapon->Name, Tint, PanelX + 26.f, H - 92.f, Large, 1.f);
        Outlined(*this, Weapon->Manufacturer, FLinearColor(0.75f, 0.75f, 0.75f), PanelX + 26.f, H - 64.f, Small, 1.f);
        if (Weapon->ShotCost <= 0.f)
            Outlined(*this, FString::Chr(TCHAR(0x221E)), FLinearColor::White, PanelX + PanelW - 58.f, H - 100.f, Large, 2.f);
        else
        {
            Outlined(*this, FString::Printf(TEXT("%d"), UOpenWillowInventory::MagazineLeft(*Weapon)), FLinearColor::White,
                PanelX + PanelW - 58.f, H - 92.f, Large, 1.2f);
            if (UOpenWillowInventory::ResolveAmmoType(*Weapon, HeldAmmo))
                Outlined(*this, FString::Printf(TEXT("%d"), Inventory->AmmoPool(HeldAmmo).Current),
                    FLinearColor(0.75f, 0.75f, 0.75f), PanelX + PanelW - 58.f, H - 64.f, Small, 1.f);
        }
    }
    else
    {
        Outlined(*this, TEXT("Unarmed"), FLinearColor::White, PanelX + 26.f, H - 92.f, Large, 1.f);
    }
    for (int32 Slot = 0; Inventory && Slot < UOpenWillowInventory::SlotCount; ++Slot)
    {
        const FOpenWillowWeaponItem* Held = Inventory->SlotItem(Slot);
        const bool bActive = Slot == Inventory->GetActiveSlot();
        const FLinearColor Pip = Held ? RarityColor(Held->Rarity) * (bActive ? 1.f : 0.45f) : FLinearColor(0.2f, 0.2f, 0.2f, 0.6f);
        Slant(*Canvas, PanelX + 26.f + Slot * 30.f, H - 30.f, 24.f, bActive ? 8.f : 5.f, 3.f, Pip);
    }
}
