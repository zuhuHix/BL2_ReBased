#pragma once

#include "CoreMinimal.h"
#include "GameFramework/HUD.h"
#include "OpenWillowMayaHUD.generated.h"

class SWebBrowser;
class SWidget;

UCLASS()
class OPENWILLOW_API AOpenWillowMayaHUD : public AHUD
{
    GENERATED_BODY()
public:
    virtual void BeginPlay() override;
    virtual void EndPlay(const EEndPlayReason::Type Reason) override;
    virtual void DrawHUD() override;
    void ToggleSkills();
    void CloseSkills();
    void RequestSkillsCloseFromPage();
    bool IsSkillsOpen() const { return SkillsBrowser.IsValid(); }

private:
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
    bool bCloseSkillsRequested = false;
    float NextSkillsPush = 0.f;
    // Spends the page reported (branch, tier, cell), applied in DrawHUD.
    void OnSkillsConsole(const FString& Message);
    void PushSkillsState();
    TArray<FIntVector> PendingSpends;
};
