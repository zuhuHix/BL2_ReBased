#include "OpenWillowControls.h"
#include "OpenWillowCombatTarget.h"
#include "OpenWillowHitEvent.h"
#include "OpenWillowInventory.h"
#include "OpenWillowSkills.h"
#include "OpenWillowInteractive.h"
#include "OpenWillowInventoryWidget.h"
#include "OpenWillowUsePrompt.h"
#include "OpenWillowInventoryPickup.h"
#include "OpenWillowQuest.h"
#include "OpenWillowNpc.h"
#include "Camera/CameraComponent.h"
#include "OpenWillowMayaHUD.h"
#include "OpenWillowWalker.h"
#include "Engine/World.h"
#include "GameFramework/PlayerController.h"
#include "Engine/DamageEvents.h"
#include "InputCoreTypes.h"
#include "Misc/CommandLine.h"

void AOpenWillowWalker::ToggleMapOrMissionsPage(bool bMap)
{
    // The same steps as ToggleSkills: the trigger and the aim end, and a legacy inventory widget gives way to the page.
    APlayerController* PC = Cast<APlayerController>(Controller);
    if (!bMayaActive || !PC) return;
    // The stock keys did nothing while down (read live, one press each), and the set of actions allowed while down has no menu in it.
    if (IsDown())
    {
        ++PagesRefusedWhileDown;
        UE_LOG(LogTemp, Display, TEXT("OWCONTROLS %s page refused: Maya is down"), bMap ? TEXT("map") : TEXT("missions"));
        return;
    }
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
    // What the frames saw of the swing since the last V: that it started, and when the target first lost health, on the world clock.
    bool bSwingSeen = false, bHitSeen = false;
    float HitAt = 0.f, WaitEndBy = 0.f;
    int32 MagazineAtSwing = -1;
    // The melee hit events since the last V.
    int32 MeleeEvents = 0;
    float EventDamage = 0.f, EventApplied = 0.f;
    FDelegateHandle HitHandle;
    int32 GrenadesBefore = 0;
    float ReloadStartBy = 0.f, BurstBy = 0.f, RouteBy = 0.f;
    bool bWeaponOutBefore = false, bReloadStarted = false, bBeforeRefill = false, bBurstOwed = false;
    int32 PagesRefusedBefore = 0, RefusedBefore = 0, GrenadesRefusedBefore = 0, HolstersRefusedBefore = 0, BurstCountBefore = 0, BurstMagazineBefore = 0;

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
    // V, and forget the last swing. bWaitForEnd: the next step runs once this swing has started and ended (at most 5 s).
    void PressV(APlayerController& Player, float Now, bool bWaitForEnd)
    {
        Press(Player, EKeys::V);
        bSwingSeen = bHitSeen = false;
        MeleeEvents = 0;
        EventDamage = EventApplied = 0.f;
        WaitEndBy = bWaitForEnd ? Now + 5.f : 0.f;
    }
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
    float Lost() { return MeleeDummy.IsValid() ? HealthBefore - MeleeDummy->GetHealth() : -1.f; }
    // A swing that expected no hit: it did start, and it hit nothing.
    void CheckMissed(const TCHAR* Name)
    {
        Check(Name, bSwingSeen && MeleeEvents == 0 && Lost() == 0.f, FString::Printf(TEXT("started=%d events=%d lost=%.2f"), bSwingSeen, MeleeEvents, Lost()));
    }
    int32 GrenadesLeft(const AOpenWillowWalker& Maya)
    {
        return Maya.GetInventory() ? Maya.GetInventory()->AmmoPool(EOpenWillowAmmoType::Grenade).Current : -1;
    }
    // The held weapon with one round in the magazine and ammo to refill it; null without one.
    FOpenWillowWeaponItem* EmptyHeldWeapon(AOpenWillowWalker& Maya)
    {
        UOpenWillowInventory* Inventory = Maya.GetInventory();
        FOpenWillowWeaponItem* Held = Inventory ? Inventory->ActiveWeaponMutable() : nullptr;
        EOpenWillowAmmoType Type;
        if (!Held || !UOpenWillowInventory::ResolveAmmoType(*Held, Type)) return nullptr;
        Inventory->SetAmmoCurrent(Type, 100);
        Held->MagazineLeft = 1;
        return Held;
    }
    // The selector rows: where Maya went, the Marcus fixture's gun, and a field with no throttle behind it.
    FVector AirFrom = FVector::ZeroVector, MarcusAt = FVector::ZeroVector;
    TWeakObjectPtr<AOpenWillowInventoryPickup> PromptDrop;
    bool WasTalk = false;
    FString FreshField(const AOpenWillowWalker& Maya)
    {
        OpenWillowUsePrompt::FState Fresh;
        return OpenWillowUsePrompt::StateField(Maya, Fresh);
    }
    void ViewAt(const AOpenWillowWalker& Maya, APlayerController& Player, const FVector& Target)
    {
        const UCameraComponent* Camera = Maya.FindComponentByClass<UCameraComponent>();
        if (Camera) Player.SetControlRotation((Target - Camera->GetComponentLocation()).Rotation());
    }
    // The held weapon on the floor below At, as a pickup.
    void DropHeldWeapon(AOpenWillowWalker& Maya, const FVector& At)
    {
        const UOpenWillowInventory* Inventory = Maya.GetInventory();
        const FOpenWillowWeaponItem* Held = Inventory ? Inventory->ActiveWeapon() : nullptr;
        FOpenWillowTakenInventoryItem Taken;
        if (!Held || !Maya.TakeInventoryItemById(UOpenWillowInventory::StableId(*Held), Taken)) return;
        FHitResult Ground;
        FCollisionQueryParams Query(SCENE_QUERY_STAT(OWControlsDrop), false, &Maya);
        const bool bGround = Maya.GetWorld()->LineTraceSingleByChannel(Ground, At + FVector(0.f, 0.f, 100.f), At - FVector(0.f, 0.f, 450.f), ECC_WorldStatic, Query);
        FActorSpawnParameters Spawn;
        Spawn.SpawnCollisionHandlingOverride = ESpawnActorCollisionHandlingMethod::AlwaysSpawn;
        AOpenWillowInventoryPickup* Pickup = Maya.GetWorld()->SpawnActor<AOpenWillowInventoryPickup>(AOpenWillowInventoryPickup::StaticClass(),
            FTransform(bGround ? Ground.ImpactPoint + FVector(0.f, 0.f, 12.f) : At), Spawn);
        if (Pickup) Pickup->Initialize(MoveTemp(Taken));
        PromptDrop = Pickup;
    }
    int32 HeldMagazine(const AOpenWillowWalker& Maya)
    {
        const FOpenWillowWeaponItem* Held = Maya.GetInventory() ? Maya.GetInventory()->ActiveWeapon() : nullptr;
        return Held ? UOpenWillowInventory::MagazineLeft(*Held) : -1;
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
    if (!HitHandle.IsValid())
        HitHandle = OpenWillowHit::OnHit().AddLambda([](const FOpenWillowHitEvent& Event)
        {
            if (Event.Source != EOpenWillowHitSource::Melee) return;
            ++MeleeEvents;
            EventDamage = Event.Damage;
            EventApplied = Event.Applied;
        });
    // Every frame, so the times are the frame the thing happened on rather than when a step happens to look.
    if (Maya.MeleeBusy() && !bSwingSeen) { bSwingSeen = true; MagazineAtSwing = HeldMagazine(Maya); }
    if (MeleeDummy.IsValid() && !bHitSeen && MeleeDummy->GetHealth() < HealthBefore) { bHitSeen = true; HitAt = Now; }
    constexpr float Start = 10.f, Settle = 1.5f;
    if (Now < Start || Now < NextAt) return;
    if (Now < WaitEndBy && (!bSwingSeen || Maya.MeleeBusy())) return;
    WaitEndBy = 0.f;
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
        using OpenWillowUsePrompt::Describe;
        const FString None;
        const bool bPrompts = !Describe(EOpenWillowUseTarget::None, false, false, None).Frame
            && FCString::Strcmp(Describe(EOpenWillowUseTarget::Vendor, false, false, None).Frame, TEXT("shop")) == 0
            && FCString::Strcmp(Describe(EOpenWillowUseTarget::Door, false, false, None).Frame, TEXT("open")) == 0
            && FCString::Strcmp(Describe(EOpenWillowUseTarget::Pickup, false, false, None).Frame, TEXT("pickup")) == 0
            && FCString::Strcmp(Describe(EOpenWillowUseTarget::Interactive, false, false, TEXT("use")).Frame, TEXT("use")) == 0
            && FCString::Strcmp(Describe(EOpenWillowUseTarget::Interactive, false, false, TEXT("open")).Frame, TEXT("open")) == 0
            && !Describe(EOpenWillowUseTarget::Interactive, false, false, None).Frame
            && FCString::Strcmp(Describe(EOpenWillowUseTarget::Vendor, true, false, None).Frame, TEXT("talk")) == 0
            && !Describe(EOpenWillowUseTarget::Vendor, true, true, None).Frame;
        Check(TEXT("use_prompt_rule"), bPrompts, TEXT("none, each target's frame, talking outranks a nearer target, and a reward page shows none"));
        Check(TEXT("melee_damage_rule"), FMath::IsNearlyEqual(OpenWillowMelee::DamageAtLevel(0), 20.f) && FMath::IsNearlyEqual(OpenWillowMelee::DamageAtLevel(2), 25.538f, 0.01f),
            FString::Printf(TEXT("level0=%.3f level2=%.3f"), OpenWillowMelee::DamageAtLevel(0), OpenWillowMelee::DamageAtLevel(2)));
        using OpenWillowMelee::FCandidate;
        const FCandidate Near{FVector(200, 40, 0), true}, Straight{FVector(300, 0, 0), true}, Blocked{FVector(100, 0, 0), false}, Far{FVector(400, 0, 0), true}, Side{FVector(50, 200, 0), true};
        const int32 Pick = OpenWillowMelee::ChooseVictim(FVector::ZeroVector, FVector::ForwardVector, {Near, Straight, Blocked, Far, Side});
        Check(TEXT("melee_cone_rule"), Pick == 1 && OpenWillowMelee::ChooseVictim(FVector::ZeroVector, FVector::ForwardVector, {Blocked, Far, Side}) == INDEX_NONE, FString::Printf(TEXT("pick=%d"), Pick));
        OpenWillowMelee::FSwing Swing;
        const bool bFirst = Swing.Press(0.f, 1.f), bSecond = Swing.Press(.5f, 1.f), bEarly = Swing.ConsumeHit(.2f), bDue = Swing.ConsumeHit(.3f), bOnce = Swing.ConsumeHit(.4f), bAgain = Swing.Press(1.f, 1.f);
        const bool bHeld = FMath::IsNearlyEqual(OpenWillowMelee::HeldSeconds(20.f / 30.f, 1. / 30.), 21.f / 30.f, 1e-5f);
        Check(TEXT("melee_swing_rule"), bFirst && !bSecond && !bEarly && bDue && !bOnce && bAgain && bHeld, TEXT("press, refused, not early, due, once, press again; 21 keys hold 0.70 s"));
        // Well inside the reach and dead ahead, from Maya's own key.
        PlaceDummy(Maya, Player, 150.f, 0.f);
        PressV(Player, Now, true);
        break;
    }
    case 5:
    {
        // The delay, from when the press was taken to the frame the hit landed. The upper end only allows for a slow frame.
        const float Delay = bSwingSeen && bHitSeen ? HitAt - Maya.MeleeStartedAt() : -1.f;
        Check(TEXT("v_not_hit_before_the_delay"), Delay >= OpenWillowMelee::HitDelay - 1e-3f && Delay < OpenWillowMelee::HitDelay + .5f, FString::Printf(TEXT("delay=%.3f"), Delay));
        const float Expected = OpenWillowMelee::DamageAtLevel(Maya.GetSkills() ? Maya.GetSkills()->GetLevel() : 1);
        Check(TEXT("v_hits_target_ahead"), MeleeEvents == 1 && FMath::IsNearlyEqual(EventDamage, Expected, .01f) && EventApplied > 0.f && FMath::IsNearlyEqual(Lost(), EventApplied, .01f),
            FString::Printf(TEXT("events=%d damage=%.2f expected=%.2f applied=%.2f lost=%.2f"), MeleeEvents, EventDamage, Expected, EventApplied, Lost()));
        PlaceDummy(Maya, Player, 150.f, 180.f);   // behind her
        PressV(Player, Now, true);
        break;
    }
    case 6:
        CheckMissed(TEXT("v_misses_target_behind"));
        PlaceDummy(Maya, Player, 700.f, 0.f);   // past the reach
        PressV(Player, Now, true);
        break;
    case 7:
        CheckMissed(TEXT("v_misses_target_out_of_reach"));
        PlaceDummy(Maya, Player, 150.f, 0.f);
        PressV(Player, Now, false);
        NextAt = Now;
        break;
    case 8:   // a status page inside the delay
        if (!bSwingSeen) { NextAt = Now; return; }
        Hud.ToggleMap();
        NextAt = Now + OpenWillowMelee::HitDelay + .4f;
        break;
    case 9:
        Check(TEXT("page_does_not_cancel_swing"), Hud.IsMapOpen() && MeleeEvents == 1 && Lost() > 0.f,
            FString::Printf(TEXT("%s events=%d lost=%.2f"), *State(), MeleeEvents, Lost()));
        Hud.CloseSkills();
        if (MeleeDummy.IsValid()) MeleeDummy->Destroy();   // clear of the grenade below
        if (UOpenWillowInventory* Inventory = Maya.GetInventory()) Inventory->SetAmmoCurrent(EOpenWillowAmmoType::Grenade, 3);
        GrenadesBefore = 3;
        break;
    case 10:   // V inside a throw's lockout
        Press(Player, EKeys::G);
        NextAt = Now + .2f;
        break;
    case 11:
        RefusedBefore = Maya.MeleeRefusedByThrow;
        PressV(Player, Now, false);
        NextAt = Now + .3f;
        break;
    case 12:
        Check(TEXT("v_refused_in_grenade_lockout"), GrenadesLeft(Maya) == GrenadesBefore - 1 && !bSwingSeen && Maya.MeleeRefusedByThrow == RefusedBefore + 1,
            FString::Printf(TEXT("grenades=%d from %d swing=%d refused=%d"), GrenadesLeft(Maya), GrenadesBefore, bSwingSeen, Maya.MeleeRefusedByThrow - RefusedBefore));
        GrenadesBefore = GrenadesLeft(Maya);
        Press(Player, EKeys::One);   // a weapon in hand, for the holster and reload steps
        NextAt = Now + 2.f;   // the lockout ends
        break;
    case 13:   // G and the holster inside a swing
        bWeaponOutBefore = Maya.HasWeaponOut();
        GrenadesRefusedBefore = Maya.GrenadesRefusedBySwing;
        HolstersRefusedBefore = Maya.HolstersRefusedBySwing;
        PressV(Player, Now, false);
        NextAt = Now;
        break;
    case 14:
        if (!bSwingSeen) { NextAt = Now; return; }
        Press(Player, EKeys::G);
        NextAt = Now;
        break;
    case 15:
        Press(Player, EKeys::Zero);
        NextAt = Now + .1f;
        break;
    case 16:
    {
        const bool bStillSwinging = Maya.MeleeBusy();
        const int32 GrenadesRefused = Maya.GrenadesRefusedBySwing - GrenadesRefusedBefore, HolstersRefused = Maya.HolstersRefusedBySwing - HolstersRefusedBefore;
        Check(TEXT("g_refused_during_swing"), bStillSwinging && GrenadesLeft(Maya) == GrenadesBefore && GrenadesRefused == 1, FString::Printf(TEXT("busy=%d grenades=%d from %d refused=%d"), bStillSwinging, GrenadesLeft(Maya), GrenadesBefore, GrenadesRefused));
        Check(TEXT("holster_refused_during_swing"), bStillSwinging && bWeaponOutBefore && Maya.HasWeaponOut() && HolstersRefused == 1, FString::Printf(TEXT("busy=%d out_before=%d out=%d refused=%d"), bStillSwinging, bWeaponOutBefore, Maya.HasWeaponOut(), HolstersRefused));
        WaitEndBy = Now + 5.f;
        break;
    }
    case 17:   // V inside a reload, before its refill point
        if (!EmptyHeldWeapon(Maya)) { Check(TEXT("v_stops_reload_before_refill"), false, TEXT("no weapon in hand")); Check(TEXT("v_keeps_refill_after_it"), false, TEXT("no weapon in hand")); TestStep = 23; return; }
        Press(Player, EKeys::R);
        ReloadStartBy = Now + 1.f;
        NextAt = Now;
        break;
    case 18:
        if (!Maya.IsReloading() && Now < ReloadStartBy) { NextAt = Now; return; }
        bReloadStarted = Maya.IsReloading();
        bBeforeRefill = bReloadStarted && Now < Maya.ReloadRefillTime();
        PressV(Player, Now, true);
        break;
    case 19:
        Check(TEXT("v_stops_reload_before_refill"), bReloadStarted && bBeforeRefill && bSwingSeen && !Maya.IsReloading() && HeldMagazine(Maya) == 1,
            FString::Printf(TEXT("reload_started=%d before_refill=%d swing=%d reloading=%d magazine=%d"), bReloadStarted, bBeforeRefill, bSwingSeen, Maya.IsReloading(), HeldMagazine(Maya)));
        break;
    case 20:   // and after it
        if (!EmptyHeldWeapon(Maya)) { Check(TEXT("v_keeps_refill_after_it"), false, TEXT("no weapon in hand")); TestStep = 23; return; }
        Press(Player, EKeys::R);
        ReloadStartBy = Now + 1.f;
        NextAt = Now;
        break;
    case 21:
        // Until the reload has started and passed its refill point. A reload that never starts fails the row.
        if (!Maya.IsReloading() && Now < ReloadStartBy) { NextAt = Now; return; }
        bReloadStarted = Maya.IsReloading();
        if (bReloadStarted && Now < Maya.ReloadRefillTime()) { NextAt = Now; return; }
        PressV(Player, Now, true);
        break;
    case 22:
        Check(TEXT("v_keeps_refill_after_it"), bReloadStarted && bSwingSeen && !Maya.IsReloading() && HeldMagazine(Maya) > 1,
            FString::Printf(TEXT("reload_started=%d swing=%d reloading=%d magazine=%d"), bReloadStarted, bSwingSeen, Maya.IsReloading(), HeldMagazine(Maya)));
        break;
    case 23:   // a burst owing shots when the swing starts: the held gun as a three-round burst for this step
    {
        FOpenWillowWeaponItem* Held = Maya.GetInventory() ? Maya.GetInventory()->ActiveWeaponMutable() : nullptr;
        if (!Held) { Check(TEXT("burst_ends_on_swing"), false, TEXT("no weapon in hand")); TestStep = 27; return; }
        BurstCountBefore = Held->Firing.BurstCount;
        Held->Firing.BurstCount = 3;
        BurstMagazineBefore = HeldMagazine(Maya);
        Maya.TestFireInput(true);
        BurstBy = Now + 2.f;
        NextAt = Now;
        break;
    }
    case 24:
        if (HeldMagazine(Maya) == BurstMagazineBefore && Now < BurstBy) { NextAt = Now; return; }
        Maya.TestFireInput(false);
        bBurstOwed = Maya.BurstActive();
        PressV(Player, Now, true);
        break;
    case 25:   // past the end of the swing, when owed shots would have resumed
        NextAt = Now + .6f;
        break;
    case 26:
    {
        Check(TEXT("burst_ends_on_swing"), bBurstOwed && bSwingSeen && !Maya.BurstActive() && HeldMagazine(Maya) == MagazineAtSwing,
            FString::Printf(TEXT("owed_at_press=%d swing=%d active=%d magazine=%d at_swing=%d"), bBurstOwed, bSwingSeen, Maya.BurstActive(), HeldMagazine(Maya), MagazineAtSwing));
        if (FOpenWillowWeaponItem* Held = Maya.GetInventory() ? Maya.GetInventory()->ActiveWeaponMutable() : nullptr) Held->Firing.BurstCount = BurstCountBefore;
        break;
    }
    case 27:   // nothing to use, far above everything
        AirFrom = Maya.GetActorLocation();
        Maya.SetActorLocation(AirFrom + FVector(0.f, 0.f, 30000.f));
        NextAt = Now;
        break;
    case 28:
    {
        const FString Field = FreshField(Maya);
        Check(TEXT("prompt_nothing_in_open_air"), Maya.NearestUseTarget() == EOpenWillowUseTarget::None && Field == TEXT("null"), Field);
        Maya.SetActorLocation(AirFrom);
        break;
    }
    case 29:
    {
        // Before Marcus, with a dropped gun nearest and to one side: the world target is real, and he still takes the press.
        const AActor* Marcus = Maya.GetQuest() ? Maya.GetQuest()->GetMarcus() : nullptr;
        if (!Marcus)
        {
            Check(TEXT("prompt_marcus_over_world_target"), false, TEXT("no Marcus"));
            Check(TEXT("prompt_aimed_pickup"), false, TEXT("no Marcus"));
            Check(TEXT("e_takes_the_named_pickup"), false, TEXT("no Marcus"));
            TestStep = 33;
            return;
        }
        MarcusAt = Marcus->GetActorLocation();
        const FVector Toward = (Maya.GetActorLocation() - MarcusAt).GetSafeNormal2D();
        Maya.SetActorLocation(MarcusAt + Toward * 150.f + FVector(0.f, 0.f, 30.f));
        ViewAt(Maya, Player, MarcusAt);
        DropHeldWeapon(Maya, Maya.GetActorLocation() + FVector::CrossProduct(FVector::UpVector, Toward) * 120.f);
        break;
    }
    case 30:
    {
        const FString Field = FreshField(Maya);
        Check(TEXT("prompt_marcus_over_world_target"), PromptDrop.IsValid() && Maya.NearestUseTarget() != EOpenWillowUseTarget::None && Field.Contains(TEXT("\"talk\"")),
            FString::Printf(TEXT("drop=%d target=%d %s"), PromptDrop.IsValid(), int32(Maya.NearestUseTarget()), *Field));
        if (PromptDrop.IsValid()) ViewAt(Maya, Player, PromptDrop->GetActorLocation());
        break;
    }
    case 31:
    {
        const FString Field = FreshField(Maya);
        Check(TEXT("prompt_aimed_pickup"), Maya.NearestUseTarget() == EOpenWillowUseTarget::Pickup && Field.Contains(TEXT("\"pickup\"")),
            FString::Printf(TEXT("target=%d %s"), int32(Maya.NearestUseTarget()), *Field));
        Press(Player, EKeys::E);
        break;
    }
    case 32:
        Check(TEXT("e_takes_the_named_pickup"), !PromptDrop.IsValid(), FString::Printf(TEXT("drop_left=%d"), PromptDrop.IsValid()));
        ViewAt(Maya, Player, MarcusAt);
        break;
    case 33:   // last: going down inside the delay, with Marcus in view: she then bleeds out.
        WasTalk = FreshField(Maya).Contains(TEXT("\"talk\""));
        PlaceDummy(Maya, Player, 150.f, 0.f);
        PressV(Player, Now, false);
        NextAt = Now;
        break;
    case 34:
        if (!bSwingSeen) { NextAt = Now; return; }
        Maya.TakeDamage(Maya.GetShield() + Maya.GetHealth() + 1.f, FDamageEvent(), nullptr, nullptr);
        NextAt = Now + OpenWillowMelee::HitDelay + .4f;
        break;
    case 35:
    {
        Check(TEXT("down_cancels_swing"), Maya.IsDown() && !Maya.MeleeBusy() && MeleeEvents == 0 && Lost() == 0.f,
            FString::Printf(TEXT("down=%d busy=%d events=%d lost=%.2f"), Maya.IsDown(), Maya.MeleeBusy(), MeleeEvents, Lost()));
        const FString Field = FreshField(Maya);
        Check(TEXT("prompt_nothing_while_down"), WasTalk && Maya.IsDown() && Field == TEXT("null"), FString::Printf(TEXT("talk_before=%d down=%d %s"), WasTalk, Maya.IsDown(), *Field));
        if (MeleeDummy.IsValid()) MeleeDummy->Destroy();
        PagesRefusedBefore = Maya.PagesRefusedWhileDown;
        Press(Player, EKeys::M);
        break;
    }
    case 36:
        Check(TEXT("m_refused_while_down"), !Hud.IsSkillsOpen() && Maya.PagesRefusedWhileDown == PagesRefusedBefore + 1, FString::Printf(TEXT("%s refused=%d"), *State(), Maya.PagesRefusedWhileDown - PagesRefusedBefore));
        Press(Player, EKeys::L);
        break;
    case 37:
        Check(TEXT("l_refused_while_down"), !Hud.IsSkillsOpen() && Maya.PagesRefusedWhileDown == PagesRefusedBefore + 2, FString::Printf(TEXT("%s refused=%d"), *State(), Maya.PagesRefusedWhileDown - PagesRefusedBefore));
        Hud.ForgetMissionsReport();
        Hud.ToggleMissions();   // a page already up when she went down, by the HUD's own call
        RouteBy = Now + 12.f;
        NextAt = Now;
        break;
    case 38:
        // Once the page says it is ready. The key is a keydown event sent into the page, which tests the page's handler and the host's refusal
        // of its route; it does not prove a real key press reaches the page, since it skips Slate.
        if (!Hud.AskMissionsReady())
        {
            if (Now < RouteBy) { NextAt = Now + .2f; return; }
            Check(TEXT("route_refused_while_down"), false, TEXT("the Missions page never said it was ready"));
            Hud.CloseSkills();
            TestStep = 40;
            return;
        }
        PagesRefusedBefore = Hud.TabRoutesRefusedWhileDown;
        Hud.SendPageKey(TEXT("m"));
        RouteBy = Now + 5.f;
        NextAt = Now;
        break;
    case 39:
        if (Hud.TabRoutesRefusedWhileDown == PagesRefusedBefore && Now < RouteBy) { NextAt = Now + .1f; return; }
        Check(TEXT("route_refused_while_down"), Hud.IsMissionsOpen() && !Hud.IsMapOpen() && Hud.TabRoutesRefusedWhileDown == PagesRefusedBefore + 1,
            FString::Printf(TEXT("%s refused=%d"), *State(), Hud.TabRoutesRefusedWhileDown - PagesRefusedBefore));
        Hud.CloseSkills();
        break;
    default:
        Check(TEXT("closed_at_end"), !Hud.IsSkillsOpen(), State());
        UE_LOG(LogTemp, Display, TEXT("OWCONTROLS SUMMARY passed=%d failed=%d"), Passed, Failed);
        OpenWillowHit::OnHit().Remove(HitHandle);
        Player.ConsoleCommand(TEXT("quit"));
        TestStep = -1;
        return;
    }
    ++TestStep;
}
