#include "OpenWillowSliceGate.h"
#include "OpenWillowCombatTarget.h"
#include "OpenWillowInventory.h"
#include "OpenWillowInventoryPickup.h"
#include "OpenWillowNpc.h"
#include "OpenWillowQuest.h"
#include "OpenWillowSkills.h"
#include "OpenWillowWalker.h"
#include "OpenWillowWeaponGen.h"
#include "Camera/CameraComponent.h"
#include "Engine/Engine.h"
#include "Engine/World.h"
#include "EngineUtils.h"
#include "GameFramework/CharacterMovementComponent.h"
#include "GameFramework/PlayerController.h"
#include "HAL/IConsoleManager.h"
#include "HAL/PlatformMisc.h"
#include "InputKeyEventArgs.h"
#include "Kismet/GameplayStatics.h"
#include "Misc/CommandLine.h"
#include "Misc/Parse.h"

// Timeouts per link (seconds of game time). Generous: the run reports a slow link as FAIL with its state, not a hang.
namespace
{
constexpr float SpawnTimeout = 30.f;
constexpr float KillTimeout = 240.f;      // the stand-in has 20,000 health (sized for level-30 guns)
constexpr float LootWait = 4.f;           // a drop has this long after the kill to appear
constexpr float LootRadius = 1500.f;      // ...within this distance of the corpse
constexpr float PhaselockTargetWait = 12.f;
constexpr float WalkTimeout = 45.f;
constexpr float DummyTimeout = 20.f;
constexpr float BleedTimeout = 40.f;
}

UOpenWillowSliceGate::UOpenWillowSliceGate()
{
    PrimaryComponentTick.bCanEverTick = true;
    PrimaryComponentTick.bStartWithTickEnabled = true;
}

AOpenWillowWalker* UOpenWillowSliceGate::Walker() const { return Cast<AOpenWillowWalker>(GetOwner()); }

void UOpenWillowSliceGate::Link(const TCHAR* Name, const TCHAR* Result, const FString& Detail)
{
    const FString R(Result);
    if (R == TEXT("PASS")) ++Pass; else if (R == TEXT("FAIL")) ++Failed; else ++NotAvailable;
    UE_LOG(LogTemp, Display, TEXT("OWSLICEGATE link=%s result=%s detail=%s (t=%.1fs)"), Name, Result, *Detail, RunTime);
}

void UOpenWillowSliceGate::Finish(const FString& Why)
{
    if (!Why.IsEmpty()) UE_LOG(LogTemp, Display, TEXT("OWSLICEGATE note run ended early: %s"), *Why);
    UE_LOG(LogTemp, Display, TEXT("OWSLICEGATE SUMMARY pass=%d fail=%d na=%d"), Pass, Failed, NotAvailable);
    Next(EStep::Done);
}

void UOpenWillowSliceGate::PressUse()
{
    if (APlayerController* PC = Walker() ? Cast<APlayerController>(Walker()->GetController()) : nullptr)
    {
        PC->InputKey(FInputKeyEventArgs::CreateSimulated(EKeys::E, IE_Pressed, 1.f));
        bUseHeld = true;
        UseHeldFor = 0.f;
        // What the use ray sees: the first thing on the view line within 400 uu.
        const UCameraComponent* Camera = Walker()->FindComponentByClass<UCameraComponent>();
        const FVector Eye = Camera ? Camera->GetComponentLocation() : Walker()->GetActorLocation();
        FHitResult Hit;
        FCollisionQueryParams Query(SCENE_QUERY_STAT(OWSliceGateUse), true, Walker());
        const bool bHit = GetWorld()->LineTraceSingleByChannel(Hit, Eye, Eye + Walker()->GetViewRotation().Vector() * 400.f, ECC_Visibility, Query);
        UE_LOG(LogTemp, Display, TEXT("OWSLICEGATE note use key pressed: eye %s view %s, view line hits %s at %.0f uu"), *Eye.ToCompactString(),
            *Walker()->GetViewRotation().ToCompactString(), bHit && Hit.GetActor() ? *Hit.GetActor()->GetName() : TEXT("nothing"), bHit ? Hit.Distance : 0.f);
    }
}

void UOpenWillowSliceGate::AimAt(const FVector& Point)
{
    AOpenWillowWalker* W = Walker();
    if (!W || !W->GetController()) return;
    const UCameraComponent* Camera = W->FindComponentByClass<UCameraComponent>();
    const FVector Eye = Camera ? Camera->GetComponentLocation() : W->GetActorLocation() + FVector(0, 0, 70);
    W->GetController()->SetControlRotation((Point - Eye).Rotation());
}

// A test shortcut, as in the quest suite: the run teleports between the places the slice uses instead of walking there.
void UOpenWillowSliceGate::Place(const FVector& Location, const FVector& Look)
{
    AOpenWillowWalker* W = Walker();
    W->SetActorLocation(Location, false, nullptr, ETeleportType::TeleportPhysics);
    W->GetCharacterMovement()->StopMovementImmediately();
    AimAt(Look);
}

// One FireOnce at the target every Interval seconds (0: every tick). Reloads an empty magazine through the reload key;
// when the reserve is empty too, refills it (test fixture, logged once per refill). False when the shot did not go out.
bool UOpenWillowSliceGate::ShootAt(AOpenWillowCombatTarget* Target, float Interval)
{
    AOpenWillowWalker* W = Walker();
    if (!Target) return false;
    AimAt(Target->AimPoint());
    ShotClock += GetWorld()->GetDeltaSeconds();
    if (ShotClock < Interval) return false;
    ShotClock = 0.f;
    if (W->FireOnce()) return true;
    UOpenWillowInventory* Inventory = W->GetInventory();
    FOpenWillowWeaponItem* Held = Inventory ? Inventory->ActiveWeaponMutable() : nullptr;
    if (!Held || W->IsReloading() || UOpenWillowInventory::MagazineLeft(*Held) > 0) return false;
    EOpenWillowAmmoType Ammo;
    if (!Inventory->CanReload(*Held) && UOpenWillowInventory::ResolveAmmoType(*Held, Ammo))
    {
        Inventory->SetAmmoCurrent(Ammo, Inventory->AmmoPool(Ammo).Max);
        UE_LOG(LogTemp, Display, TEXT("OWSLICEGATE note test fixture: %s reserve refilled to %d"),
            UOpenWillowInventory::AmmoTypeKey(Ammo), Inventory->AmmoPool(Ammo).Max);
    }
#if WITH_EDITOR
    W->TestReloadInput();
#endif
    return false;
}

AOpenWillowCombatTarget* UOpenWillowSliceGate::NearestLiveTarget(bool bPreferEnemyClass) const
{
    const FVector From = Walker()->GetActorLocation();
    AOpenWillowCombatTarget* Best = nullptr;
    float BestScore = TNumericLimits<float>::Max();
    for (TActorIterator<AOpenWillowCombatTarget> It(GetWorld()); It; ++It)
    {
        if (!It->IsAutoAimTarget() || *It == MissionDummy) continue;
        float Score = FVector::Dist(From, It->GetActorLocation());
        // The range's moving enemies (AOpenWillowEnemy, PR #76) are subclasses; prefer them over its static dummies.
        if (bPreferEnemyClass && !It->GetClass()->GetName().Contains(TEXT("Enemy"))) Score += 1.0e6f;
        if (Score < BestScore) { BestScore = Score; Best = *It; }
    }
    return Best;
}

// Marcus is the NPC standing at the quest's giver marker (eligible or turn-in); ambient NPCs are AOpenWillowNpc too.
AOpenWillowNpc* UOpenWillowSliceGate::FindMarcus() const
{
    FVector At;
    if (!QuestMarker(TEXT("eligible"), At) && !QuestMarker(TEXT("turnin"), At)) return Marcus;
    AOpenWillowNpc* Best = nullptr;
    float BestDistance = 50.f;
    for (TActorIterator<AOpenWillowNpc> It(GetWorld()); It; ++It)
        if (const float D = FVector::Dist(It->GetActorLocation(), At); D < BestDistance) { BestDistance = D; Best = *It; }
    if (Best) Marcus = Best;
    return Marcus;
}

bool UOpenWillowSliceGate::QuestMarker(const TCHAR* Kind, FVector& Out) const
{
    UOpenWillowQuest* Quest = Walker()->GetQuest();
    if (!Quest || !Quest->Enabled()) return false;
    TArray<TPair<FString, FVector>> Markers;
    Quest->MinimapMarkers(Markers);
    for (const auto& Marker : Markers) if (Marker.Key == Kind) { Out = Marker.Value; return true; }
    return false;
}

void UOpenWillowSliceGate::SnapshotBackpack()
{
    BackpackBefore.Reset();
    for (const FOpenWillowWeaponItem& Item : Walker()->GetInventory()->Items()) BackpackBefore.Add(UOpenWillowInventory::StableId(Item));
}

FString UOpenWillowSliceGate::NewBackpackItem() const
{
    for (const FOpenWillowWeaponItem& Item : Walker()->GetInventory()->Items())
        if (!BackpackBefore.Contains(UOpenWillowInventory::StableId(Item))) return UOpenWillowInventory::StableId(Item);
    return FString();
}

// The inventory page's equip action calls AOpenWillowWalker::EquipItem (OpenWillowMayaHUD.cpp); the page clicks
// themselves are tools/test_inventory_actions.ps1's job. Slot: the first empty unlocked one, else the last unlocked.
bool UOpenWillowSliceGate::EquipAndDraw(const FString& Id, FString& OutWhy)
{
    AOpenWillowWalker* W = Walker();
    UOpenWillowInventory* Inventory = W->GetInventory();
    int32 Slot = Inventory->GetWeaponSlotsUnlocked() - 1;
    for (int32 Candidate = 0; Candidate < Inventory->GetWeaponSlotsUnlocked(); ++Candidate)
        if (!Inventory->SlotItem(Candidate)) { Slot = Candidate; break; }
    const int32 Index = Inventory->FindItemIndexById(Id);
    if (Index == INDEX_NONE) { OutWhy = FString::Printf(TEXT("%s is not in the backpack"), *Id); return false; }
    if (!W->EquipItem(Index, Slot)) { OutWhy = FString::Printf(TEXT("EquipItem(%d, slot %d) refused %s"), Index, Slot, *Id); return false; }
    if (!W->DrawItemById(Id)) { OutWhy = FString::Printf(TEXT("DrawItemById refused %s after equipping it in slot %d"), *Id, Slot); return false; }
    const FOpenWillowWeaponItem* Held = Inventory->ActiveWeapon();
    if (!Held || UOpenWillowInventory::StableId(*Held) != Id || !W->HasWeaponOut())
    {
        OutWhy = FString::Printf(TEXT("held weapon is %s, not %s"), Held ? *UOpenWillowInventory::StableId(*Held) : TEXT("none"), *Id);
        return false;
    }
    OutWhy = FString::Printf(TEXT("%s \"%s\" (%s, level %d) equipped in slot %d and drawn"), *Id, *Held->Name, *Held->Type, Held->Level, Slot + 1);
    return true;
}

void UOpenWillowSliceGate::TickComponent(float Delta, ELevelTick Type, FActorComponentTickFunction* Function)
{
    Super::TickComponent(Delta, Type, Function);
    AOpenWillowWalker* W = Walker();
    if (!W || !GetWorld()) return;
    RunTime += Delta;
    StepTime += Delta;
    UseHeldFor += Delta;
    if (bUseHeld && UseHeldFor >= 0.1f)
    {
        if (APlayerController* PC = Cast<APlayerController>(W->GetController()))
            PC->InputKey(FInputKeyEventArgs::CreateSimulated(EKeys::E, IE_Released, 0.f));
        bUseHeld = false;
    }
    UOpenWillowQuest* Quest = W->GetQuest();
    UOpenWillowInventory* Inventory = W->GetInventory();
    switch (Step)
    {
    case EStep::Start:
        UE_LOG(LogTemp, Display, TEXT("OWSLICEGATE start map=%s quest=%d range_command=%d"), *GetWorld()->GetMapName(),
            Quest && Quest->Enabled(), IConsoleManager::Get().FindConsoleObject(TEXT("ow.Range")) != nullptr);
        Next(EStep::Spawn);
        break;

    case EStep::Spawn: {
        // Maya has landed and the quest has placed Marcus (the quest spawns him in its first ticks).
        const bool bReady = W->IsMayaActive() && W->GetCharacterMovement()->IsMovingOnGround() && StepTime > 3.f;
        if (!bReady && StepTime < SpawnTimeout) return;
        if (!W->IsMayaActive()) { FailLink(TEXT("spawn_maya"), TEXT("Maya is not active (start with -owmaya)")); Finish(TEXT("no Maya")); return; }
        const FOpenWillowWeaponItem* Held = Inventory ? Inventory->ActiveWeapon() : nullptr;
        const bool bGood = bReady && W->HasWeaponOut() && Held && W->GetHealth() > 0.f && W->GetHealth() == W->GetMaxHealth();
        const FString Detail = FString::Printf(TEXT("level %d, health %.0f/%.0f, grounded %d, holding %s"), W->GetSkills()->GetLevel(),
            W->GetHealth(), W->GetMaxHealth(), W->GetCharacterMovement()->IsMovingOnGround(), Held ? *Held->Name : TEXT("nothing"));
        if (bGood) PassLink(TEXT("spawn_maya"), Detail); else FailLink(TEXT("spawn_maya"), Detail);
        // Test fixture: Phaselock needs one point in the action skill; spend it the way the skills page does.
        if (W->GetSkills()->GetActionGrade() < 1)
        {
            FString Reason;
            const bool bSpent = W->GetSkills()->TrySpend(-1, -1, -1, Reason);
            UE_LOG(LogTemp, Display, TEXT("OWSLICEGATE note test fixture: action skill point spent=%d %s"), bSpent, *Reason);
        }
        Next(EStep::Arena);
        break;
    }

    case EStep::Arena: {
        if (StepTime < 0.1f) return;
        if (IConsoleManager::Get().FindConsoleObject(TEXT("ow.Range")))
        {
            if (!bRangeMode)
            {
                bRangeMode = true;
                GEngine->Exec(GetWorld(), TEXT("ow.Range"));
                return;
            }
            if (StepTime < 3.f) return;   // the range spawns its targets
            Enemy = NearestLiveTarget(true);
            if (!Enemy) { FailLink(TEXT("reach_fight_area"), TEXT("ow.Range ran but no live target is in the world")); Finish(TEXT("no enemy")); return; }
            PassLink(TEXT("reach_fight_area"), FString::Printf(TEXT("ow.Range: Maya at %s, target %s (%s) %.0f uu away"),
                *W->GetActorLocation().ToCompactString(), *Enemy->GetName(), *Enemy->GetClass()->GetName(),
                FVector::Dist(W->GetActorLocation(), Enemy->GetActorLocation())));
        }
        else
        {
            // No range on this build: the host's stand-in target, 600 uu ahead of Maya on the floor (as -owcombattest).
            const FVector Ahead = W->GetActorLocation() + FRotator(0, W->GetControlRotation().Yaw, 0).Vector() * 600.f;
            FHitResult Floor;
            FCollisionQueryParams Query(SCENE_QUERY_STAT(OWSliceGateFloor), true, W);
            FVector At = Ahead;
            if (GetWorld()->LineTraceSingleByChannel(Floor, Ahead + FVector(0, 0, 300), Ahead - FVector(0, 0, 600), ECC_Visibility, Query)) At = Floor.ImpactPoint;
            FActorSpawnParameters Params;
            Params.Owner = W;
            Enemy = GetWorld()->SpawnActor<AOpenWillowCombatTarget>(At, FRotator(0, W->GetControlRotation().Yaw + 180.f, 0), Params);
            if (!Enemy) { FailLink(TEXT("reach_fight_area"), TEXT("could not spawn the stand-in target")); Finish(TEXT("no enemy")); return; }
            MissingLink(TEXT("reach_fight_area"), FString::Printf(TEXT("no ow.Range command on this build (range lane, PR #76); "
                "fallback: the host stand-in target at %s next to Maya in Sanctuary"), *At.ToCompactString()));
        }
        for (TActorIterator<AOpenWillowInventoryPickup> It(GetWorld()); It; ++It) PickupsBeforeKill.Add(*It);
        HitsAtFightStart = W->GetHitCount();
        Next(EStep::Fight);
        break;
    }

    case EStep::Fight: {
        if (Enemy && Enemy->IsAutoAimTarget())
        {
            if (StepTime < KillTimeout) { if (ShootAt(Enemy, 0.f)) ++KillShots; return; }
            FailLink(TEXT("kill_enemy"), FString::Printf(TEXT("%s still alive after %.0fs and %d shots (health %.0f%%)"),
                *Enemy->GetName(), StepTime, KillShots, 100.f * Enemy->HealthFraction()));
        }
        else if (Enemy)
        {
            KillLocation = Enemy->GetActorLocation();
            const FOpenWillowWeaponItem* Held = Inventory->ActiveWeapon();
            PassLink(TEXT("kill_enemy"), FString::Printf(TEXT("%s killed with %d FireOnce shots from %s in %.1fs"), *Enemy->GetName(), KillShots,
                Held ? *Held->Name : TEXT("?"), StepTime));
        }
        else FailLink(TEXT("kill_enemy"), TEXT("the enemy disappeared before it died"));
        const int32 Hits = W->GetHitCount() - HitsAtFightStart;
        if (Hits > 0) PassLink(TEXT("enemy_attacks_maya"), FString::Printf(TEXT("%d hit(s) on Maya during the fight, health %.0f/%.0f"), Hits, W->GetHealth(), W->GetMaxHealth()));
        else MissingLink(TEXT("enemy_attacks_maya"), bRangeMode ? TEXT("no enemy hit Maya during the fight (enemy AI: range lane, PR #83)")
            : TEXT("the stand-in target has no attack; enemy AI is the range lane's PR #83"));
        Next(EStep::Loot);
        break;
    }

    case EStep::Loot: {
        if (!Drop)
            for (TActorIterator<AOpenWillowInventoryPickup> It(GetWorld()); It; ++It)
                if (!PickupsBeforeKill.Contains(*It) && FVector::Dist(It->GetActorLocation(), KillLocation) < LootRadius) { Drop = *It; break; }
        if (!Drop && StepTime < LootWait) return;
        if (Drop) PassLink(TEXT("loot_drop_on_kill"), FString::Printf(TEXT("pickup \"%s\" %.0f uu from the corpse"), *Drop->DisplayName(),
            FVector::Dist(Drop->GetActorLocation(), KillLocation)));
        else MissingLink(TEXT("loot_drop_on_kill"), FString::Printf(TEXT("no pickup appeared within %.0f uu of the kill in %.0fs: no loot drop on kill yet (range lane, loot PR3)"),
            LootRadius, LootWait));
        Next(EStep::Pickup);
        break;
    }

    case EStep::Pickup: {
        if (Drop)
        {
            if (!bPlacedAtDrop)
            {
                bPlacedAtDrop = true;
                SnapshotBackpack();
                PickupAttemptsBefore = W->PickupAttemptCount();
                const FVector Away = (W->GetActorLocation() - Drop->GetActorLocation()).GetSafeNormal2D();
                Place(Drop->GetActorLocation() + Away * 80.f + FVector(0, 0, 100.f), Drop->GetActorLocation());
                return;
            }
            if (StepTime < 0.5f) return;
            if (W->PickupAttemptCount() == PickupAttemptsBefore && StepTime < 0.6f) { if (!bUseHeld) PressUse(); return; }
            if (W->PickupAttemptCount() == PickupAttemptsBefore && StepTime < 2.f) return;
            GunId = NewBackpackItem();
            if (W->PickupAttemptCount() > PickupAttemptsBefore && W->LastPickupAccepted() && !GunId.IsEmpty())
                PassLink(TEXT("pick_up_gun"), FString::Printf(TEXT("use key picked up %s"), *GunId));
            else FailLink(TEXT("pick_up_gun"), FString::Printf(TEXT("use key: attempts %d -> %d, accepted %d, new backpack item '%s'"),
                PickupAttemptsBefore, W->PickupAttemptCount(), W->LastPickupAccepted(), *GunId));
            Next(EStep::Equip);
            return;
        }
        // Fallback until drops exist: ow.RollGun rolls a gun from the generator into the backpack (once).
        if (!bRolled)
        {
            bRolled = true;
            SnapshotBackpack();
            FString Source;
            if (!FParse::Value(FCommandLine::Get(), TEXT("owslicegategun="), Source))
            {
                FString ItemDir;
                if (FParse::Value(FCommandLine::Get(), TEXT("owitems="), ItemDir)) Source = FOpenWillowWeaponGen::RewardSource(ItemDir);
            }
            if (Source.IsEmpty() || !FOpenWillowWeaponGen::IsAvailable())
            {
                MissingLink(TEXT("pick_up_gun"), TEXT("no drop to pick up, and the ow.RollGun fallback has no source or generator manifest"));
                Next(EStep::Equip);
                return;
            }
            const int32 Stage = FMath::Clamp(W->GetSkills()->GetLevel(), 1, 80);
            GEngine->Exec(GetWorld(), *FString::Printf(TEXT("ow.RollGun %s %d 1"), *Source, Stage));
            UE_LOG(LogTemp, Display, TEXT("OWSLICEGATE note fallback ow.RollGun %s %d 1"), *Source, Stage);
            return;
        }
        GunId = NewBackpackItem();
        if (!GunId.IsEmpty()) MissingLink(TEXT("pick_up_gun"), FString::Printf(TEXT("no drop to pick up; fallback ow.RollGun added %s \"%s\" to the backpack"),
            *GunId, *Inventory->FindItemById(GunId)->Name));
        else MissingLink(TEXT("pick_up_gun"), TEXT("no drop to pick up, and the ow.RollGun fallback added nothing (see its log line)"));
        Next(EStep::Equip);
        break;
    }

    case EStep::Equip: {
        if (StepTime < 0.3f) return;
        FString Why;
        if (GunId.IsEmpty()) FailLink(TEXT("equip_gun"), TEXT("blocked: no looted or rolled gun in the backpack"));
        else if (EquipAndDraw(GunId, Why)) PassLink(TEXT("equip_gun"), Why);
        else FailLink(TEXT("equip_gun"), Why);
        Next(EStep::Phaselock);
        break;
    }

    case EStep::Phaselock: {
        // The killed enemy respawns (stand-in 3 s, range 8 s); any other live target near Maya serves too.
        AOpenWillowCombatTarget* Target = Enemy && Enemy->CanPhaseLockTarget() ? Enemy.Get() : NearestLiveTarget(true);
        if (StepTime < 1.f || (!W->LastPhaselockTarget() && !Target && StepTime < PhaselockTargetWait)) return;
        if (!W->LastPhaselockTarget() || W->LastPhaselockCastAt() < RunTime - StepTime)
        {
            if (!Target) { FailLink(TEXT("phaselock_enemy"), TEXT("no live target to cast at")); Next(EStep::Accept); return; }
            AimAt(Target->AimPoint());
            FString Reason;
            if (!W->CanCastPhaselock(Reason))
            {
                if (StepTime < PhaselockTargetWait) return;   // e.g. still drawing the new gun
                FailLink(TEXT("phaselock_enemy"), FString::Printf(TEXT("cast refused: %s"), *Reason));
                Next(EStep::Accept);
                return;
            }
            Enemy = Target;
            W->UsePhaselock();
            return;
        }
        // The cast went out on an earlier tick.
        if (W->LastPhaselockHit() && W->LastPhaselockTarget() == Enemy && Enemy->IsPhaselocked())
            PassLink(TEXT("phaselock_enemy"), FString::Printf(TEXT("%s lifted, lock %.1fs left"), *Enemy->GetName(), W->PhaselockRemaining()));
        else FailLink(TEXT("phaselock_enemy"), FString::Printf(TEXT("cast hit=%d blocked=%d target=%s"), W->LastPhaselockHit(), W->LastPhaselockBlocked(),
            W->LastPhaselockTarget() ? *W->LastPhaselockTarget()->GetName() : TEXT("none")));
        Next(EStep::Accept);
        break;
    }

    case EStep::Accept: {
        // Let the lock end before the mission, then start the step's clock again.
        if (!bAcceptStarted)
        {
            if (W->IsPhaselockActive() && StepTime < 20.f) return;
            bAcceptStarted = true;
            StepTime = 0.f;
            return;
        }
        AOpenWillowNpc* Giver = FindMarcus();
        FVector At;
        if (!Quest || !Quest->Enabled() || !Giver)
        {
            FailLink(TEXT("mission_accept"), TEXT("no Fire mission in this run (needs -owquest and its manifests)"));
            Next(EStep::Down);
            return;
        }
        if (Quest->Status() == 0 && StepTime < 0.5f)
        {
            if (!bPlacedAtMarcus && QuestMarker(TEXT("eligible"), At))
            {
                bPlacedAtMarcus = true;
                Place(At + Giver->GetActorForwardVector() * 150.f + FVector(0, 0, 20), At + FVector(0, 0, 70));
                UE_LOG(LogTemp, Display, TEXT("OWSLICEGATE note at Marcus: %s %s, Maya at %s"), *Giver->GetName(), *At.ToCompactString(),
                    *W->GetActorLocation().ToCompactString());
            }
            SnapshotBackpack();
            return;
        }
        // One press, and a second one a second later if the first did not reach him.
        if (Quest->Status() == 0 && (StepTime < 0.6f || (StepTime >= 1.5f && StepTime < 1.6f))) { if (!bUseHeld) PressUse(); return; }
        if (Quest->Status() == 0 && StepTime < 3.f) return;
        LentId = NewBackpackItem();
        if (Quest->Status() == 1) PassLink(TEXT("mission_accept"), FString::Printf(TEXT("use key at Marcus accepted the Fire mission; lent weapon %s"),
            LentId.IsEmpty() ? TEXT("not in the backpack") : *LentId));
        else { FailLink(TEXT("mission_accept"), FString::Printf(TEXT("mission status %d after the use key at Marcus"), Quest->Status())); Next(EStep::Down); return; }
        Next(EStep::WaitWalk);
        break;
    }

    case EStep::WaitWalk: {
        // Marcus walks to the range and the door closes behind him; then the trigger objective is the marker.
        AOpenWillowNpc* Giver = FindMarcus();
        FVector Trigger;
        const bool bReady = Giver && !Giver->IsWalking() && QuestMarker(TEXT("objective"), Trigger) && StepTime > 2.f;
        if (!bReady && StepTime < WalkTimeout) return;
        if (!bReady) { FailLink(TEXT("mission_objective"), FString::Printf(TEXT("Marcus still walking or no objective marker after %.0fs"), StepTime)); Next(EStep::Down); return; }
        FString Why;
        if (LentId.IsEmpty() || !EquipAndDraw(LentId, Why))
        {
            FailLink(TEXT("mission_objective"), FString::Printf(TEXT("could not ready the lent weapon: %s"), LentId.IsEmpty() ? TEXT("none lent") : *Why));
            Next(EStep::Down);
            return;
        }
        UE_LOG(LogTemp, Display, TEXT("OWSLICEGATE note lent weapon readied: %s"), *Why);
        Place(Trigger, Trigger + Giver->GetActorForwardVector() * 500.f);
        Next(EStep::Trigger);
        break;
    }

    case EStep::Trigger: {
        // Touching the trigger spawns the dummy; the provider registers it as targetable.
        if (!MissionDummy)
            for (TActorIterator<AOpenWillowCombatTarget> It(GetWorld()); It; ++It)
                if (Quest->IsRegisteredTargetable(*It) && It->IsAutoAimTarget()) { MissionDummy = *It; break; }
        if (!MissionDummy && StepTime < DummyTimeout) return;
        if (!MissionDummy) { FailLink(TEXT("mission_objective"), FString::Printf(TEXT("no targetable mission dummy %.0fs after the trigger (status %d)"), StepTime, Quest->Status())); Next(EStep::Down); return; }
        Next(EStep::ShootDummy);
        break;
    }

    case EStep::ShootDummy:
        if (StepTime < 1.5f) { if (MissionDummy) AimAt(MissionDummy->AimPoint()); return; }   // the target Matinee carries it out
        if (Quest->Status() == 1 && MissionDummy && StepTime < DummyTimeout) { if (ShootAt(MissionDummy, 0.6f)) ++DummyShots; return; }
        if (Quest->Status() == 2) PassLink(TEXT("mission_objective"), FString::Printf(TEXT("trigger touched, dummy registered, %d shot(s) with the lent weapon completed the objectives"), DummyShots));
        else { FailLink(TEXT("mission_objective"), FString::Printf(TEXT("status %d after %d shot(s) at the dummy"), Quest->Status(), DummyShots)); Next(EStep::Down); return; }
        Next(EStep::TurnIn);
        break;

    case EStep::TurnIn: {
        AOpenWillowNpc* Giver = FindMarcus();
        FVector At;
        if (!bPlacedForTurnIn)
        {
            bPlacedForTurnIn = true;
            if (Giver && QuestMarker(TEXT("turnin"), At)) Place(At + Giver->GetActorForwardVector() * 150.f + FVector(0, 0, 20), At + FVector(0, 0, 70));
            ExperienceBeforeTurnIn = W->GetSkills()->GetExperience();
            return;
        }
        if (StepTime < 0.5f) return;
        PressUse();   // turns the mission in; the reward page then waits for a second press
        Next(EStep::Confirm);
        break;
    }

    case EStep::Confirm:
        if (StepTime < 1.0f) return;
        if (StepTime < 1.05f) { if (!bUseHeld) PressUse(); return; }
        if (StepTime < 2.5f) return;
        if (Quest->Status() == 3)
            PassLink(TEXT("mission_turn_in"), FString::Printf(TEXT("turned in at Marcus: experience +%lld, lent weapon %s"),
                W->GetSkills()->GetExperience() - ExperienceBeforeTurnIn,
                Inventory->FindItemIndexById(LentId) == INDEX_NONE ? TEXT("taken back") : TEXT("STILL in the backpack")));
        else FailLink(TEXT("mission_turn_in"), FString::Printf(TEXT("status %d after two use-key presses at Marcus"), Quest->Status()));
        Next(EStep::Down);
        break;

    case EStep::Down:
        if (StepTime < 0.5f) return;
        if (RespawnAtBefore == 0.f)
        {
            RespawnAtBefore = W->LastRespawnAt();
            DeathLocation = W->GetActorLocation();
            // Test fixture: lethal damage through the walker's damage entry (no enemy can kill Maya on this build yet).
            FHitResult Hit;
            UGameplayStatics::ApplyPointDamage(W, 1.0e6f, FVector::ForwardVector, Hit, nullptr, nullptr, UDamageType::StaticClass());
            return;
        }
        if (W->IsDown() && W->GetHealth() == 0.f)
            PassLink(TEXT("go_down"), FString::Printf(TEXT("lethal damage (test fixture) put Maya in Fight For Your Life, bleed-out %.1fs"), W->DownBleedTotal()));
        else FailLink(TEXT("go_down"), FString::Printf(TEXT("after lethal damage: down %d, health %.0f"), W->IsDown(), W->GetHealth()));
        Next(EStep::Bleed);
        break;

    case EStep::Bleed: {
        const bool bRespawned = W->LastRespawnAt() > RespawnAtBefore;
        if (!bRespawned && StepTime < BleedTimeout) return;
        if (bRespawned && !W->IsDown() && W->GetHealth() == W->GetMaxHealth())
            PassLink(TEXT("bleed_out_respawn"), FString::Printf(TEXT("bled out with no kill and respawned %.1fs after going down at %s, %.0f uu from the death spot, health %.0f/%.0f, mission status %d"),
                StepTime, *W->GetActorLocation().ToCompactString(), FVector::Dist(W->GetActorLocation(), DeathLocation), W->GetHealth(), W->GetMaxHealth(),
                Quest ? Quest->Status() : -1));
        else FailLink(TEXT("bleed_out_respawn"), FString::Printf(TEXT("respawned %d, down %d, health %.0f/%.0f after %.0fs"),
            bRespawned, W->IsDown(), W->GetHealth(), W->GetMaxHealth(), StepTime));
        Finish(FString());
        break;
    }

    case EStep::Done:
        if (StepTime > 2.f && !FParse::Param(FCommandLine::Get(), TEXT("owslicegatestay"))) { FPlatformMisc::RequestExit(false); SetComponentTickEnabled(false); }
        break;
    }
}
