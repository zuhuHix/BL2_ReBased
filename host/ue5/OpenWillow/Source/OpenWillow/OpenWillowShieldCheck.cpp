// AI-assisted (Claude). Test-only, runs only with -owshieldcheck (tools/run_hud_shots.ps1 -StandInShield): an unattended check of a
// stand-in shield in a live host. It adds a shield with no stats (the shape vendor and loot shields have), equips it as the inventory
// screen does, so the walker's TickShield applies it, then hits Maya through the real damage entry point and waits for the recharge.
// Numbers here are read back from the host; docs/verification/COMBAT_DOWN_RESPAWN.md says what this does and does not show.
#include "CoreMinimal.h"
#include "GameFramework/PlayerController.h"
#include "OpenWillowInventory.h"
#include "OpenWillowMayaState.h"
#include "OpenWillowSkills.h"
#include "OpenWillowWalker.h"
#include "UnrealClient.h"

void OpenWillowTickShieldCheck(AOpenWillowWalker& Maya, APlayerController& Player)
{
    static int32 Step = 0;
    static float At = 0.f, Capacity = 0.f, HealthBefore = 0.f, AfterHit = 0.f;
    const float Now = Maya.GetWorld()->GetTimeSeconds();
    if (Step > 4 || Now < At) return;
    if (Step == 4)
    {
        // Past the last screenshot's write.
        Step = 5;
        Player.ConsoleCommand(TEXT("quit"));
        return;
    }

    auto Shot = [Now](const TCHAR* Name)
    {
        FScreenshotRequest::RequestScreenshot(FString::Printf(TEXT("OWShield_%s.png"), Name), true, false);
        UE_LOG(LogTemp, Display, TEXT("OWSHIELDCHECK shot %s t=%.3f"), Name, Now);
    };
    auto Finish = [Now](bool bPass, const FString& Why)
    {
        UE_LOG(LogTemp, Display, TEXT("OWSHIELDCHECK result %s: %s"), bPass ? TEXT("pass") : TEXT("fail"), *Why);
        Step = 4;
        At = Now + 1.f;
    };

    if (Step == 0)
    {
        // After the HUD has settled, as -owhudshots waits.
        if (At == 0.f) { At = FMath::Max(Now + 4.f, 8.f); return; }
        UOpenWillowInventory* Inventory = Maya.GetInventory();
        const int32 Level = Maya.GetSkills() ? Maya.GetSkills()->GetLevel() : 1;
        FOpenWillowGearItem Gear;
        Gear.Id = TEXT("owshieldcheck_standin");
        Gear.ItemType = TEXT("shield");
        Gear.Name = TEXT("Shield Check Stand-in");
        Gear.Level = Level;
        Gear.bLevelKnown = true;
        Gear.Rarity = 1;
        if (!Inventory || !Inventory->AddGearToBackpack(Gear) || !Inventory->EquipGearById(Gear.Id, TEXT("shield"), Level))
        {
            Finish(false, TEXT("the shield could not be added or equipped"));
            return;
        }
        UE_LOG(LogTemp, Display, TEXT("OWSHIELDCHECK equipped a stat-less shield at level %d"), Level);
        Step = 1;
        At = Now + 2.f;
    }
    else if (Step == 1)
    {
        Capacity = Maya.GetShieldCapacity();
        UE_LOG(LogTemp, Display, TEXT("OWSHIELDCHECK full shield %.1f of %.1f, health %.1f"), Maya.GetShield(), Capacity, Maya.GetHealth());
        Shot(TEXT("full"));
        if (Capacity <= 0.f || Maya.GetShield() < Capacity)
        {
            Finish(false, FString::Printf(TEXT("shield %.1f of %.1f after the equip"), Maya.GetShield(), Capacity));
            return;
        }
        HealthBefore = Maya.GetHealth();
        OpenWillowApplyDamageToPlayer(&Maya, Capacity * 0.5f);
        Step = 2;
        At = Now + 0.3f;
    }
    else if (Step == 2)
    {
        AfterHit = Maya.GetShield();
        UE_LOG(LogTemp, Display, TEXT("OWSHIELDCHECK hit shield %.1f of %.1f, health %.1f"), AfterHit, Capacity, Maya.GetHealth());
        Shot(TEXT("hit"));
        if (AfterHit >= Capacity || Maya.GetHealth() < HealthBefore)
        {
            Finish(false, FString::Printf(TEXT("after the hit shield %.1f, health %.1f (was %.1f)"), AfterHit, Maya.GetHealth(), HealthBefore));
            return;
        }
        Step = 3;
        At = Now + Maya.GetMayaState().Effective().ShieldRechargeDelay + 2.f;
    }
    else
    {
        UE_LOG(LogTemp, Display, TEXT("OWSHIELDCHECK recharge shield %.1f of %.1f"), Maya.GetShield(), Capacity);
        Shot(TEXT("recharge"));
        const bool bRose = Maya.GetShield() > AfterHit;
        Finish(bRose, bRose ? TEXT("full, hit and recharged") : TEXT("the shield did not recharge"));
    }
}
