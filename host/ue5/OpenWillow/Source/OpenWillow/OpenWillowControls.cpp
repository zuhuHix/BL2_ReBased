#include "OpenWillowControls.h"
#include "OpenWillowCombatTarget.h"
#include "OpenWillowInventoryWidget.h"
#include "OpenWillowMayaHUD.h"
#include "OpenWillowWalker.h"
#include "Engine/World.h"
#include "GameFramework/PlayerController.h"
#include "InputCoreTypes.h"
#include "Misc/CommandLine.h"

void AOpenWillowWalker::ToggleMapOrMissionsPage(bool bMap)
{
    // The same steps as ToggleSkills: the trigger and the aim end, and a legacy inventory widget gives way to the page.
    APlayerController* PC = Cast<APlayerController>(Controller);
    if (!bMayaActive || !PC) return;
    bFireHeld = false;
    AimReleased();
    if (InventoryScreen && InventoryScreen->IsInViewport())
    {
        InventoryScreen->RemoveFromParent();
        PC->SetInputMode(FInputModeGameOnly());
        PC->SetShowMouseCursor(false);
    }
    AOpenWillowMayaHUD* HUD = Cast<AOpenWillowMayaHUD>(PC->GetHUD());
    // False: the page is not served in this run, or a vending shop refused it (OpenStatusMenu). The log says the key did nothing.
    if (!HUD || !(bMap ? HUD->ToggleMap() : HUD->ToggleMissions()))
        UE_LOG(LogTemp, Display, TEXT("OWCONTROLS %s page did not open"), bMap ? TEXT("map") : TEXT("missions"));
}
void AOpenWillowWalker::ToggleMapPage() { ToggleMapOrMissionsPage(true); }
void AOpenWillowWalker::ToggleMissionsPage() { ToggleMapOrMissionsPage(false); }

namespace
{
    int32 TestStep = 0, Passed = 0, Failed = 0;
    float NextAt = 0.f;
    FKey HeldKey;
    bool bKeyHeld = false;
    TWeakObjectPtr<AOpenWillowCombatTarget> MeleeDummy;
    float HealthBefore = 0.f;

    // A target at a bearing from Maya's view, level with the view, at this distance from her.
    AOpenWillowCombatTarget* PlaceDummy(AOpenWillowWalker& Maya, const APlayerController& Player, float Distance, float BearingDegrees)
    {
        if (!MeleeDummy.IsValid())
        {
            FActorSpawnParameters Params;
            Params.Owner = &Maya;
            Params.SpawnCollisionHandlingOverride = ESpawnActorCollisionHandlingMethod::AlwaysSpawn;
            MeleeDummy = Maya.GetWorld()->SpawnActor<AOpenWillowCombatTarget>(Maya.GetActorLocation(), FRotator::ZeroRotator, Params);
        }
        if (!MeleeDummy.IsValid()) return nullptr;
        const FRotator Aim(0.f, Player.GetControlRotation().Yaw + BearingDegrees, 0.f);
        MeleeDummy->SetActorLocation(Maya.GetActorLocation() + Aim.Vector() * Distance);
        HealthBefore = MeleeDummy->GetHealth();
        return MeleeDummy.Get();
    }

    void Check(const TCHAR* Name, bool bOk, const FString& Detail)
    {
        ++(bOk ? Passed : Failed);
        UE_LOG(LogTemp, Display, TEXT("OWCONTROLS %s %s %s"), Name, bOk ? TEXT("PASS") : TEXT("FAIL"), *Detail);
    }
    void Press(APlayerController& Player, const FKey& Key)
    {
        Player.InputKey(FInputKeyEventArgs::CreateSimulated(Key, IE_Pressed, 1.f));
        HeldKey = Key;
        bKeyHeld = true;
    }
}

void OpenWillowControls::TickTest(AOpenWillowMayaHUD& Hud, AOpenWillowWalker& Maya, APlayerController& Player)
{
    static const bool bTest = FParse::Param(FCommandLine::Get(), TEXT("owcontrolstest"));
    if (!bTest || TestStep < 0 || !Maya.IsMayaActive() || !Maya.GetWorld()) return;
    if (bKeyHeld)
    {
        Player.InputKey(FInputKeyEventArgs::CreateSimulated(HeldKey, IE_Released, 0.f));
        bKeyHeld = false;
    }
    const float Now = Maya.GetWorld()->GetTimeSeconds();
    constexpr float Start = 10.f, Settle = 1.5f;
    if (Now < Start || Now < NextAt) return;
    if (TestStep >= 6 && TestStep <= 8 && Maya.MeleeBusy()) return;   // the swing must end: a press inside it is refused
    NextAt = Now + Settle;
    const auto State = [&Hud]() { return FString::Printf(TEXT("map=%d missions=%d page=%d"), Hud.IsMapOpen(), Hud.IsMissionsOpen(), Hud.IsSkillsOpen()); };
    switch (TestStep)
    {
    case 0: Check(TEXT("start_closed"), !Hud.IsSkillsOpen(), State()); Press(Player, EKeys::M); break;
    case 1: Check(TEXT("m_opens_map"), Hud.IsMapOpen() && !Hud.IsMissionsOpen(), State()); Hud.CloseSkills(); break;
    case 2: Check(TEXT("map_closed"), !Hud.IsSkillsOpen(), State()); Press(Player, EKeys::L); break;
    case 3: Check(TEXT("l_opens_missions"), Hud.IsMissionsOpen() && !Hud.IsMapOpen(), State()); Hud.CloseSkills(); break;
    case 4:
    {
        // The plain rules first, on invented numbers.
        Check(TEXT("melee_damage_rule"), FMath::IsNearlyEqual(OpenWillowMelee::DamageAtLevel(0), 20.f) && FMath::IsNearlyEqual(OpenWillowMelee::DamageAtLevel(2), 25.538f, 0.01f),
            FString::Printf(TEXT("level0=%.3f level2=%.3f"), OpenWillowMelee::DamageAtLevel(0), OpenWillowMelee::DamageAtLevel(2)));
        using OpenWillowMelee::FCandidate;
        const FCandidate Near{FVector(200, 40, 0), true}, Straight{FVector(300, 0, 0), true}, Blocked{FVector(100, 0, 0), false}, Far{FVector(400, 0, 0), true}, Side{FVector(50, 200, 0), true};
        const int32 Pick = OpenWillowMelee::ChooseVictim(FVector::ZeroVector, FVector::ForwardVector, {Near, Straight, Blocked, Far, Side});
        Check(TEXT("melee_cone_rule"), Pick == 1 && OpenWillowMelee::ChooseVictim(FVector::ZeroVector, FVector::ForwardVector, {Blocked, Far, Side}) == INDEX_NONE, FString::Printf(TEXT("pick=%d"), Pick));
        OpenWillowMelee::FSwing Swing;
        const bool bFirst = Swing.Press(0.f, 1.f), bSecond = Swing.Press(.5f, 1.f), bEarly = Swing.ConsumeHit(.2f), bDue = Swing.ConsumeHit(.3f), bOnce = Swing.ConsumeHit(.4f), bAgain = Swing.Press(1.f, 1.f);
        Check(TEXT("melee_swing_rule"), bFirst && !bSecond && !bEarly && bDue && !bOnce && bAgain, TEXT("press, refused, not early, due, once, press again"));
        // Well inside the reach and dead ahead, from Maya's own key.
        if (!PlaceDummy(Maya, Player, 150.f, 0.f)) { Check(TEXT("melee_target"), false, TEXT("no dummy")); }
        Press(Player, EKeys::V);
        NextAt = Now + OpenWillowMelee::HitDelay * .5f;
        break;
    }
    case 5:
        Check(TEXT("v_not_hit_before_the_delay"), MeleeDummy.IsValid() && MeleeDummy->GetHealth() == HealthBefore && Maya.MeleeBusy(), FString::Printf(TEXT("busy=%d"), Maya.MeleeBusy()));
        NextAt = Now + 2.f;
        break;
    case 6:
    {
        const float Lost = MeleeDummy.IsValid() ? HealthBefore - MeleeDummy->GetHealth() : 0.f;
        Check(TEXT("v_hits_target_ahead"), Lost > 0.f, FString::Printf(TEXT("lost=%.2f"), Lost));
        PlaceDummy(Maya, Player, 150.f, 180.f);   // behind her
        Press(Player, EKeys::V);
        NextAt = Now + 2.f;
        break;
    }
    case 7:
        Check(TEXT("v_misses_target_behind"), MeleeDummy.IsValid() && MeleeDummy->GetHealth() == HealthBefore, FString::Printf(TEXT("lost=%.2f"), MeleeDummy.IsValid() ? HealthBefore - MeleeDummy->GetHealth() : -1.f));
        PlaceDummy(Maya, Player, 700.f, 0.f);   // past the reach
        Press(Player, EKeys::V);
        NextAt = Now + 2.f;
        break;
    case 8:
        Check(TEXT("v_misses_target_out_of_reach"), MeleeDummy.IsValid() && MeleeDummy->GetHealth() == HealthBefore, FString::Printf(TEXT("lost=%.2f"), MeleeDummy.IsValid() ? HealthBefore - MeleeDummy->GetHealth() : -1.f));
        if (MeleeDummy.IsValid()) MeleeDummy->Destroy();
        break;
    default:
        Check(TEXT("closed_at_end"), !Hud.IsSkillsOpen(), State());
        UE_LOG(LogTemp, Display, TEXT("OWCONTROLS SUMMARY passed=%d failed=%d"), Passed, Failed);
        Player.ConsoleCommand(TEXT("quit"));
        TestStep = -1;
        return;
    }
    ++TestStep;
}
