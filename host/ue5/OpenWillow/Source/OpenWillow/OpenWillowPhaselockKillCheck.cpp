// AI-assisted (Claude). Test-only, runs only with -owphaselockkillcheck (tools/test_action_skill_hud.ps1 -KillHeld): casts Phaselock at the
// -owcombattest dummy and kills it during the hold, through the target's ordinary damage entry point, then reads what the skill does.
// tools/check_phaselock_kill.py grades these lines with the HUD state lines on the same clock (docs/verification/ACTION_SKILL_HUD.md).
#include "CoreMinimal.h"
#include "EngineUtils.h"
#include "GameFramework/DamageType.h"
#include "GameFramework/PlayerController.h"
#include "Kismet/GameplayStatics.h"
#include "OpenWillowCombatTarget.h"
#include "OpenWillowWalker.h"
#include "UnrealClient.h"

void OpenWillowTickPhaselockKillCheck(AOpenWillowWalker& Maya, APlayerController& Player)
{
    static int32 Step = 0;
    static float At = 0.f, CastAt = 0.f, RemainingAfter = 0.f;
    static TWeakObjectPtr<AOpenWillowCombatTarget> Target;
    const float Now = Maya.GetWorld()->GetTimeSeconds();
    if (Step > 5 || Now < At) return;

    auto Finish = [Now](bool bPass, const FString& Why)
    {
        UE_LOG(LogTemp, Display, TEXT("OWPHASELOCKKILL result %s: %s"), bPass ? TEXT("pass") : TEXT("fail"), *Why);
        Step = 5;
        At = Now + 1.f;     // past the last screenshot's write
    };

    if (Step == 0)
    {
        // As -owskillhudshots: the movie has loaded and the dummy stands, then a level aim at it.
        for (TActorIterator<AOpenWillowCombatTarget> It(Maya.GetWorld()); It; ++It) { Target = *It; break; }
        if (Now < 12.f || !Target.IsValid() || !Player.PlayerCameraManager) return;
        const FVector Aim = Target->AimPoint();
        const FVector View = Player.PlayerCameraManager->GetCameraLocation();
        Player.SetControlRotation(((Target->IsStockPawn() ? FVector(Aim.X, Aim.Y, View.Z) : Aim - FVector(0, 0, 20)) - View).Rotation());
        Step = 1;
        At = Now + 1.5f;
    }
    else if (Step == 1)
    {
        Maya.UsePhaselock();
        CastAt = Now;
        UE_LOG(LogTemp, Display, TEXT("OWPHASELOCKKILL cast t=%.3f hit=%d"), Now, Maya.LastPhaselockHit() ? 1 : 0);
        if (!Maya.LastPhaselockHit() || !Target.IsValid())
        {
            Finish(false, TEXT("the cast did not hit the dummy"));
            return;
        }
        Step = 2;
        At = Now + 2.f;
    }
    else if (Step == 2)
    {
        // Held, as in the real game's read: the kill comes 2 s after the cast.
        const bool bHeld = Target.IsValid() && Target->IsPhaselocked() && Maya.IsPhaselockActive();
        UE_LOG(LogTemp, Display, TEXT("OWPHASELOCKKILL held=%d t=%.3f remaining=%.3f"), bHeld ? 1 : 0, Now, Maya.PhaselockRemaining());
        if (!bHeld)
        {
            Finish(false, TEXT("the target is not held 2 s after the cast"));
            return;
        }
        Target->SetInvulnerable(false);
        UGameplayStatics::ApplyDamage(Target.Get(), 1.e7f, &Player, &Maya, UDamageType::StaticClass());
        UE_LOG(LogTemp, Display, TEXT("OWPHASELOCKKILL kill t=%.3f dead=%d"), Now, Target->IsDead() ? 1 : 0);
        if (!Target->IsDead())
        {
            Finish(false, TEXT("the hit did not kill the target"));
            return;
        }
        Step = 3;
        At = Now + 0.2f;
    }
    else if (Step == 3)
    {
        const bool bActive = Maya.IsPhaselockActive();
        RemainingAfter = Maya.PhaselockRemaining();
        UE_LOG(LogTemp, Display, TEXT("OWPHASELOCKKILL after active=%d reason=%s remaining=%.3f since_cast=%.3f"), bActive ? 1 : 0,
            *Maya.LastPhaselockEndReason(), RemainingAfter, Now - CastAt);
        FScreenshotRequest::RequestScreenshot(TEXT("OWPhaselockKill_after.png"), true, false);
        if (bActive)
        {
            Finish(false, TEXT("the skill is still active after the held target died"));
            return;
        }
        Step = 4;
        At = Now + 1.f;
    }
    else if (Step == 4)
    {
        // The cooldown drains from the release at the base rate; it does not wait for the timeline's end.
        const float Remaining = Maya.PhaselockRemaining();
        UE_LOG(LogTemp, Display, TEXT("OWPHASELOCKKILL drain remaining=%.3f was=%.3f"), Remaining, RemainingAfter);
        Finish(Remaining < RemainingAfter, Remaining < RemainingAfter ? TEXT("ended on the kill, cooldown draining") : TEXT("the cooldown did not drain after the kill"));
    }
    else
    {
        Step = 6;
        Player.ConsoleCommand(TEXT("quit"));
    }
}
