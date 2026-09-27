#include "OpenWillowMayaHUD.h"
#include "OpenWillowCombatTarget.h"
#include "OpenWillowInventory.h"
#include "OpenWillowSkills.h"
#include "OpenWillowWalker.h"
#include "CanvasItem.h"
#include "Engine/Canvas.h"
#include "Engine/Engine.h"
#include "Engine/Font.h"
#include "EngineUtils.h"
#include "GameFramework/PlayerController.h"
#include "Engine/GameViewportClient.h"
#include "Misc/CommandLine.h"
#include "Misc/Parse.h"
#include "Dom/JsonObject.h"
#include "Serialization/JsonReader.h"
#include "Serialization/JsonSerializer.h"
#include "SWebBrowser.h"
#include "WebBrowserModule.h"
#include "Widgets/Layout/SBox.h"

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
    if (SkillsBrowser) { CloseSkills(); return; }
    if (SkillsUrl.IsEmpty() || !GEngine || !GEngine->GameViewport || !PlayerOwner) return;
    if (!IWebBrowserModule::Get().IsWebModuleAvailable())
    {
        UE_LOG(LogTemp, Warning, TEXT("OpenWillow Skills: the engine's web browser is not available"));
        return;
    }
    SkillsBrowser = SNew(SWebBrowser)
        .InitialURL(SkillsUrl)
        .ShowControls(false)
        .ShowAddressBar(false)
        .ShowErrorMessage(true)
        .ShowInitialThrobber(false)
        .SupportsTransparency(true)
        .BackgroundColor(FColor(0, 0, 0, 0))
        .BrowserFrameRate(30)
        .OnBeforeNavigation_Lambda([this](const FString& NextUrl, const FWebNavigationRequest&)
            {
                if (!NextUrl.EndsWith(TEXT("/__ow_close_skills"))) return false;
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
    SkillsRoot = SNew(SBox)[SkillsBrowser.ToSharedRef()];
    GEngine->GameViewport->AddViewportWidgetContent(SkillsRoot.ToSharedRef(), 20);
    if (FlashHudRoot) FlashHudRoot->SetVisibility(EVisibility::Collapsed);
    FInputModeUIOnly Mode;
    Mode.SetWidgetToFocus(SkillsBrowser.ToSharedRef());
    PlayerOwner->SetInputMode(Mode);
    PlayerOwner->SetShowMouseCursor(true);
    NextSkillsPush = 0.f;
    UE_LOG(LogTemp, Display, TEXT("OpenWillow Skills overlay: %s"), *SkillsUrl);
}

void AOpenWillowMayaHUD::CloseSkills()
{
    if (!SkillsBrowser) return;
    if (SkillsRoot && GEngine && GEngine->GameViewport)
        GEngine->GameViewport->RemoveViewportWidgetContent(SkillsRoot.ToSharedRef());
    SkillsRoot.Reset();
    SkillsBrowser.Reset();
    bCloseSkillsRequested = false;
    PendingSpends.Reset();
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
    SkillsBrowser->ExecuteJavascript(FString::Printf(TEXT("window.owSkills && window.owSkills(%s)"), *Skills->StateJson()));
}

void AOpenWillowMayaHUD::RequestSkillsCloseFromPage()
{
    if (SkillsBrowser)
        SkillsBrowser->ExecuteJavascript(TEXT("window.location.href='/__ow_close_skills'"));
}

void AOpenWillowMayaHUD::EndPlay(const EEndPlayReason::Type Reason)
{
    CloseSkills();
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
    const FString State = FString::Printf(
        TEXT("{\"character\":\"siren\",\"health\":1,\"shield\":1,\"healthText\":\"\",\"shieldText\":\"\","
             "\"xp\":%.3f,\"levelText\":\"%d\",\"grenades\":null,\"weaponOut\":%s,\"ammo\":1,\"ammoText\":\"%s\"}"),
        Skills ? Skills->LevelProgress() : 0.f,
        Skills ? Skills->GetLevel() : 1,
        bArmed ? TEXT("true") : TEXT("false"),
        bArmed ? *FString::Printf(TEXT("%.0f"), Weapon->Magazine) : TEXT(""));
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
    if (bCloseSkillsRequested) CloseSkills();
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

    // Shield over health, bottom left. Maya takes no damage yet: both full.
    const float BarW = W * 0.19f;
    SlantBar(*Canvas, 46.f, H - 86.f, BarW, 12.f, 8.f, 1.f, ShieldBlue);
    SlantBar(*Canvas, 40.f, H - 62.f, BarW, 20.f, 10.f, 1.f, HealthRed);

    // Experience bar with Maya's level, bottom centre (no XP system yet).
    const float XpW = W * 0.34f;
    SlantBar(*Canvas, X - XpW * 0.5f, H - 26.f, XpW, 7.f, 4.f, 0.f, Experience);
    Outlined(*this, TEXT("1"), Experience, X - XpW * 0.5f - 22.f, H - 34.f, Large, 1.f);

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
            Outlined(*this, FString::Printf(TEXT("%.0f"), Weapon->Magazine), FLinearColor::White,
                PanelX + PanelW - 58.f, H - 92.f, Large, 1.2f);
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
