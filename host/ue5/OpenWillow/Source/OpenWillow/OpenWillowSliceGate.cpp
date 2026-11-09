#include "OpenWillowSliceGate.h"
#include "OpenWillowAudio.h"
#include "OpenWillowCombatTarget.h"
#include "OpenWillowInventory.h"
#include "OpenWillowInventoryPickup.h"
#include "OpenWillowNpc.h"
#include "OpenWillowPickupCard.h"
#include "OpenWillowQuest.h"
#include "OpenWillowRange.h"
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
constexpr float AttackWait = 15.f;        // fire is held at most this long into the fight, for the selected enemy's first hit
constexpr float WoundShotInterval = 0.5f; // until the wound lands: longer than a shot's flight here, so only one is in the air
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
    UE_LOG(LogTemp, Display, TEXT("OWSLICEGATE link=%s result=%s fixtures=[%s] detail=%s (t=%.1fs)"), Name, Result,
        *FString::Join(Fixtures, TEXT("; ")), *Detail, RunTime);
    Fixtures.Reset();
}

void UOpenWillowSliceGate::Finish(const FString& Why)
{
    if (!Why.IsEmpty()) UE_LOG(LogTemp, Display, TEXT("OWSLICEGATE note run ended early: %s"), *Why);
    UE_LOG(LogTemp, Display, TEXT("OWSLICEGATE SUMMARY pass=%d fail=%d na=%d"), Pass, Failed, NotAvailable);
    OpenWillowAudio::StopRecording(this);
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
    Fixture(TEXT("teleport"));
    W->SetActorLocation(Location, false, nullptr, ETeleportType::TeleportPhysics);
    W->GetCharacterMovement()->StopMovementImmediately();
    AimAt(Look);
}

// -owslicegatemag=<n> (test only): sets the held gun's magazine to n rounds, once, before the first shot at the dummy, so a listening run
// hears a reload. The reserve is left alone, so the reload draws from it. The magazine is never set below one shot's cost: a gun whose
// shot costs 2 rounds (the lent pistol) cannot fire with 1 round, and ShootAt reloads only from an empty magazine, so 1 round would stall
// the step with no shot and no reload (found in the run of 2026-11-09, 19:30). With the magazine at one shot's cost, the first shot
// empties it and ShootAt presses reload (WITH_EDITOR builds only: UnrealEditor.exe -game runs the editor target, which has it).
void UOpenWillowSliceGate::ApplyMagazineFixture(AOpenWillowWalker* W)
{
    if (MagazineFixture <= 0 || !W) return;
    UOpenWillowInventory* Inventory = W->GetInventory();
    FOpenWillowWeaponItem* Held = Inventory ? Inventory->ActiveWeaponMutable() : nullptr;
    if (!Held || UOpenWillowInventory::ShotCostRounds(*Held) == 0)
    {
        UE_LOG(LogTemp, Display, TEXT("OWSLICEGATE note test fixture: -owslicegatemag=%d not applied, no magazine-fed gun in hand"), MagazineFixture);
        return;
    }
    const int32 Size = UOpenWillowInventory::MagazineSize(*Held);
    const int32 Cost = UOpenWillowInventory::ShotCostRounds(*Held);
    const int32 Set = FMath::Clamp(FMath::Max(MagazineFixture, Cost), 1, Size);
    Held->MagazineLeft = Set;
    Fixture(FString::Printf(TEXT("magazine of %s set to %d rounds"), *Held->Id, Set));
    UE_LOG(LogTemp, Display, TEXT("OWSLICEGATE note test fixture: magazine of %s set to %d rounds (-owslicegatemag=%d, shot cost %d, size %d)"),
        *Held->Id, Set, MagazineFixture, Cost, Size);
}

// After the dummy link has passed, the magazine fixture's own reload (SLICE_AUDIO_CHAIN.md section 5a). The first shot at the dummy usually
// completes the objective, so the step moves on at once and no later trigger press would reload the magazine that shot emptied. This asks
// for one reload through the same call ShootAt uses, waits for it to finish, and gives up after a timeout (a fixture note, not a link
// failure). Returns true while it waits. Off (and false at once) without -owslicegatemag, or once done, so the switch-absent gate is unchanged.
bool UOpenWillowSliceGate::HoldForFixtureReload(AOpenWillowWalker* W, float Delta)
{
    constexpr float FixtureReloadTimeout = 10.f;   // host choice: the lent pistol's reload is about 2.2 s; a longer wait means the reload is stuck
    if (MagazineFixture <= 0 || FixtureReload == 2 || !W) return false;
    UOpenWillowInventory* Inventory = W->GetInventory();
    FOpenWillowWeaponItem* Held = Inventory ? Inventory->ActiveWeaponMutable() : nullptr;
    if (FixtureReload == 0)
    {
        // Nothing to do unless the shot left the magazine empty and no reload is running.
        if (!Held || W->IsReloading() || UOpenWillowInventory::MagazineLeft(*Held) > 0 || UOpenWillowInventory::ShotCostRounds(*Held) == 0)
        {
            FixtureReload = 2;
            return false;
        }
        EOpenWillowAmmoType Ammo;
        if (!Inventory->CanReload(*Held) && UOpenWillowInventory::ResolveAmmoType(*Held, Ammo))
        {
            Inventory->SetAmmoCurrent(Ammo, Inventory->AmmoPool(Ammo).Max);
            Fixture(TEXT("ammo reserve refilled"));
            UE_LOG(LogTemp, Display, TEXT("OWSLICEGATE note test fixture: %s reserve refilled to %d"),
                UOpenWillowInventory::AmmoTypeKey(Ammo), Inventory->AmmoPool(Ammo).Max);
        }
#if WITH_EDITOR
        W->TestReloadInput();
        if (!W->IsReloading())
        {
            FixtureReload = 2;
            UE_LOG(LogTemp, Display, TEXT("OWSLICEGATE note test fixture: magazine fixture reload did not start (not ready, or no ammo to draw)"));
            return false;
        }
        FixtureReload = 1;
        FixtureReloadWait = 0.f;
        if (!bMagazineReloadLogged)
        {
            bMagazineReloadLogged = true;
            Fixture(TEXT("reload started after the magazine fixture"));
            UE_LOG(LogTemp, Display, TEXT("OWSLICEGATE note test fixture: reload started after the magazine fixture (-owslicegatemag=%d)"), MagazineFixture);
        }
        return true;
#else
        FixtureReload = 2;
        UE_LOG(LogTemp, Display, TEXT("OWSLICEGATE note test fixture: magazine fixture reload not requested: this build has no reload key (not WITH_EDITOR)"));
        return false;
#endif
    }
    // FixtureReload == 1: the reload was requested; wait while it runs.
    FixtureReloadWait += Delta;
    if (W->IsReloading())
    {
        if (FixtureReloadWait < FixtureReloadTimeout) return true;
        FixtureReload = 2;
        UE_LOG(LogTemp, Display, TEXT("OWSLICEGATE note test fixture: magazine fixture reload did not finish within %.0f s; the step moves on"), FixtureReloadTimeout);
        return false;
    }
    FixtureReload = 2;
    if (Held && UOpenWillowInventory::MagazineLeft(*Held) > 0)
    {
        UE_LOG(LogTemp, Display, TEXT("OWSLICEGATE note test fixture: magazine fixture reload finished, magazine %d after %.1f s"), UOpenWillowInventory::MagazineLeft(*Held), FixtureReloadWait);
    }
    else
    {
        UE_LOG(LogTemp, Display, TEXT("OWSLICEGATE note test fixture: magazine fixture reload ended with no magazine (cancelled)"));
    }
    return false;
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
        Fixture(TEXT("ammo reserve refilled"));
        UE_LOG(LogTemp, Display, TEXT("OWSLICEGATE note test fixture: %s reserve refilled to %d"),
            UOpenWillowInventory::AmmoTypeKey(Ammo), Inventory->AmmoPool(Ammo).Max);
    }
#if WITH_EDITOR
    W->TestReloadInput();
    if (MagazineFixture > 0 && !bMagazineReloadLogged && W->IsReloading())
    {
        bMagazineReloadLogged = true;
        Fixture(TEXT("reload started after the magazine fixture"));
        UE_LOG(LogTemp, Display, TEXT("OWSLICEGATE note test fixture: reload started after the magazine fixture (-owslicegatemag=%d)"), MagazineFixture);
    }
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

FString UOpenWillowSliceGate::PickupId(const AOpenWillowInventoryPickup* Pickup)
{
    if (!Pickup) return TEXT("none");
    const FOpenWillowTakenInventoryItem& Item = Pickup->GetItem();
    return Item.bIsGear ? Item.Gear.Id : UOpenWillowInventory::StableId(Item.Weapon);
}

// Rolls a gun (ow.RollGun RollSource, the player's level, Seed) and drops it on the floor under Near. Null, with the
// reason, when no floor is found within 450 uu below Near (the drop is never left in the air) or the roll adds nothing.
AOpenWillowInventoryPickup* UOpenWillowSliceGate::DropRolledGun(uint32 Seed, const FVector& Near, FString& OutWhy)
{
    AOpenWillowWalker* W = Walker();
    FHitResult Ground;
    FCollisionQueryParams Query(SCENE_QUERY_STAT(OWSliceGateDrop), false, W);
    if (!GetWorld()->LineTraceSingleByChannel(Ground, Near + FVector(0, 0, 100), Near - FVector(0, 0, 450), ECC_WorldStatic, Query))
    {
        OutWhy = FString::Printf(TEXT("no floor under the test drop point %s"), *Near.ToCompactString());
        return nullptr;
    }
    SnapshotBackpack();
    const int32 Stage = FMath::Clamp(W->GetSkills()->GetLevel(), 1, 80);
    GEngine->Exec(GetWorld(), *FString::Printf(TEXT("ow.RollGun %s %d %u"), *RollSource, Stage, Seed));
    const FString Rolled = NewBackpackItem();
    FOpenWillowTakenInventoryItem Taken;
    if (Rolled.IsEmpty() || !W->TakeInventoryItemById(Rolled, Taken))
    {
        OutWhy = FString::Printf(TEXT("ow.RollGun %s %d %u added nothing to drop (see its log line)"), *RollSource, Stage, Seed);
        return nullptr;
    }
    FActorSpawnParameters Spawn;
    Spawn.SpawnCollisionHandlingOverride = ESpawnActorCollisionHandlingMethod::AlwaysSpawn;
    const FVector At = Ground.ImpactPoint + FVector(0, 0, 12);
    AOpenWillowInventoryPickup* Pickup = GetWorld()->SpawnActor<AOpenWillowInventoryPickup>(AOpenWillowInventoryPickup::StaticClass(), FTransform(At), Spawn);
    if (!Pickup) { OutWhy = FString::Printf(TEXT("the test drop of %s did not spawn"), *Rolled); return nullptr; }
    Pickup->Initialize(MoveTemp(Taken));
    UE_LOG(LogTemp, Display, TEXT("OWSLICEGATE note test drop: %s at %s"), *Rolled, *At.ToCompactString());
    return Pickup;
}

// How the hold on fire ended, as a fixture. Only a real hit says "until"; tools/check_slice_gate.py reads the Maya down text.
FString UOpenWillowSliceGate::HoldFixture() const
{
    if (HoldEnd == TEXT("hit")) return TEXT("fire held until the selected enemy hit Maya");
    if (HoldEnd.IsEmpty()) return TEXT("fire held for the selected enemy's first hit");
    return FString::Printf(TEXT("fire held, ended by %s"), *HoldEnd);
}

// Why the test cannot roll its drops, or empty. This is the test's own setup or local data, not something the host lacks.
FString UOpenWillowSliceGate::RollSetupGap() const
{
    if (RollSource.IsEmpty()) return TEXT("test setup gap, not a missing host capability: no ow.RollGun source (-owslicegategun or -owitems)");
    if (!FOpenWillowWeaponGen::IsAvailable()) return TEXT("test setup gap, not a missing host capability: no weapon generator manifest on this machine");
    return FString();
}

bool UOpenWillowSliceGate::PickupAccepted() const
{
    const AOpenWillowWalker* W = Walker();
    return W->PickupAttemptCount() != PickupAttemptsBefore && W->LastPickupAccepted();
}

// Stands Maya 80 uu from Target, aims at it until she is on the ground, then in one tick reads the card (the id in the
// card state, OpenWillowPickupCard::StateField) and presses E. Sets DropId, CardId and PickedId ("none" when absent).
// True once the press has been answered (or timed out).
bool UOpenWillowSliceGate::RunAimedPickup(AOpenWillowInventoryPickup* Target)
{
    AOpenWillowWalker* W = Walker();
    const APlayerController* PC = Cast<APlayerController>(W->GetController());
    switch (PickupPhase)
    {
    case 0: {
        DropId = PickupId(Target);
        CardId = PickedId = TEXT("none");
        SnapshotBackpack();
        PickupAttemptsBefore = W->PickupAttemptCount();
        const FVector Away = (W->GetActorLocation() - Target->GetActorLocation()).GetSafeNormal2D();
        Place(Target->GetActorLocation() + Away * 80.f + FVector(0, 0, 100.f), Target->GetActorLocation());
        PickupPhase = 1;
        PickupWait = 0.f;
        return false;
    }
    case 1: {
        PickupWait += GetWorld()->GetDeltaSeconds();
        AimAt(Target->GetActorLocation());
        if (!PC || ((!W->GetCharacterMovement()->IsMovingOnGround() || PickupWait < 0.5f) && PickupWait < 3.f)) return false;
        // The card the player would see at the moment of the press.
        const FString State = OpenWillowPickupCard::StateField(*W, *PC);
        const int32 At = State.Find(TEXT("\"id\":\""));
        if (At != INDEX_NONE)
        {
            const int32 Start = At + 6;
            const int32 End = State.Find(TEXT("\""), ESearchCase::CaseSensitive, ESearchDir::FromStart, Start);
            if (End > Start) CardId = State.Mid(Start, End - Start);
        }
        UE_LOG(LogTemp, Display, TEXT("OWSLICEGATE note card at the press: aimed pickup %s, card id %s, grounded %d"),
            *PickupId(OpenWillowPickupCard::FindAimedPickup(*W, *PC)), *CardId, W->GetCharacterMovement()->IsMovingOnGround());
        PressUse();
        PickupPhase = 2;
        PickupWait = 0.f;
        return false;
    }
    default: {
        PickupWait += GetWorld()->GetDeltaSeconds();
        if (W->PickupAttemptCount() == PickupAttemptsBefore && PickupWait < 2.f) return false;
        const FString New = NewBackpackItem();
        if (!New.IsEmpty()) PickedId = New;
        PickupPhase = 0;
        return true;
    }
    }
}

// The links each step starts, for the guard below (a step that continues a link, such as Trigger, has none of its own).
static TArray<const TCHAR*> LinksFrom(uint8 StepValue, uint8 PickupStep, uint8 PickupDecoyStep, uint8 EquipStep,
    uint8 PhaselockStep, uint8 AcceptStep, uint8 WaitWalkStep, uint8 TurnInStep)
{
    if (StepValue == PickupStep) return {TEXT("pick_up_gun"), TEXT("pick_up_aimed_not_nearest")};
    if (StepValue == PickupDecoyStep) return {TEXT("pick_up_aimed_not_nearest")};
    if (StepValue == EquipStep) return {TEXT("equip_gun")};
    if (StepValue == PhaselockStep) return {TEXT("phaselock_enemy")};
    if (StepValue == AcceptStep) return {TEXT("mission_accept"), TEXT("mission_objective"), TEXT("mission_turn_in")};
    if (StepValue == WaitWalkStep) return {TEXT("mission_objective"), TEXT("mission_turn_in")};
    if (StepValue == TurnInStep) return {TEXT("mission_turn_in")};
    return {};
}

// A link group that would start with Maya down (from an earlier link) is logged NOT_AVAILABLE and skipped. Only at the
// start of a step (StepTime is still the first tick) and never for the down links themselves.
bool UOpenWillowSliceGate::SkipWhileDown()
{
    AOpenWillowWalker* W = Walker();
    if (!W->IsDown() || StepTime > GetWorld()->GetDeltaSeconds() + KINDA_SMALL_NUMBER) return false;
    const TArray<const TCHAR*> Links = LinksFrom(uint8(Step), uint8(EStep::Pickup), uint8(EStep::PickupDecoy), uint8(EStep::Equip),
        uint8(EStep::Phaselock), uint8(EStep::Accept), uint8(EStep::WaitWalk), uint8(EStep::TurnIn));
    if (Links.IsEmpty()) return false;
    const FString Why = FString::Printf(TEXT("Maya down from a previous link (%s, health %.0f/%.0f) when this link was due to start"),
        W->GetDownPhase() == AOpenWillowWalker::EDownPhase::Dead ? TEXT("dead") : TEXT("injured"), W->GetHealth(), W->GetMaxHealth());
    for (const TCHAR* Name : Links) MissingLink(Name, Why);
    const EStep After = Step == EStep::Pickup || Step == EStep::PickupDecoy ? EStep::Equip
        : Step == EStep::Equip ? EStep::Phaselock : Step == EStep::Phaselock ? EStep::Accept : EStep::Down;
    Next(After);
    return true;
}

void UOpenWillowSliceGate::TickComponent(float Delta, ELevelTick Type, FActorComponentTickFunction* Function)
{
    Super::TickComponent(Delta, Type, Function);
    AOpenWillowWalker* W = Walker();
    if (!W || !GetWorld()) return;
    RunTime += Delta;
    UseHeldFor += Delta;
    if (bUseHeld && UseHeldFor >= 0.1f)
    {
        if (APlayerController* PC = Cast<APlayerController>(W->GetController()))
            PC->InputKey(FInputKeyEventArgs::CreateSimulated(EKeys::E, IE_Released, 0.f));
        bUseHeld = false;
    }
    if (HoldLeft > 0.f) { HoldLeft -= Delta; return; }
    // Names in EStep's order, for a recording's cue log; Cue does nothing without -owrecordaudio.
    static const TCHAR* const StepNames[] = {TEXT("Start"), TEXT("Spawn"), TEXT("Arena"), TEXT("Fight"), TEXT("Recover"), TEXT("Loot"), TEXT("Pickup"),
        TEXT("PickupDecoy"), TEXT("Equip"), TEXT("Phaselock"), TEXT("Accept"), TEXT("WaitWalk"), TEXT("Trigger"), TEXT("ShootDummy"), TEXT("TurnIn"),
        TEXT("Confirm"), TEXT("Down"), TEXT("Bleed"), TEXT("Done")};
    static_assert(UE_ARRAY_COUNT(StepNames) == uint8(EStep::Done) + 1, "a name per step");
    if (StepTime == 0.f) OpenWillowAudio::Cue(this, FString(TEXT("step ")) + StepNames[uint8(Step)]);
    StepTime += Delta;
    UOpenWillowQuest* Quest = W->GetQuest();
    UOpenWillowInventory* Inventory = W->GetInventory();
    if (SkipWhileDown()) return;
    switch (Step)
    {
    case EStep::Start:
        FParse::Value(FCommandLine::Get(), TEXT("owslicegatepause="), StepPause);
        StepPause = FMath::Clamp(StepPause, 0.f, 10.f);
        // -owslicegatemag=<n> (test only, off by default): see ApplyMagazineFixture.
        FParse::Value(FCommandLine::Get(), TEXT("owslicegatemag="), MagazineFixture);
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
            bPhaselockPointGranted = bSpent;   // reported on the Phaselock link, the one that relies on it
            UE_LOG(LogTemp, Display, TEXT("OWSLICEGATE note test fixture: action skill point spent=%d %s"), bSpent, *Reason);
        }
        // -owslicegateshield: Maya fights with a shield, so the fight path can run without the restore fixture. A shield
        // from the gear manifest when one is there (equipped as the inventory equips gear). If all of them are above her
        // level, the lowest one, by passing its own level to the equip call: a test override of the level rule, not how
        // equipping works, and the fixture's name says so. With no manifest shield, a host stand-in pool with her maximum
        // health as its capacity and the default recharge (not stock data). Named on the fight links.
        if (FParse::Param(FCommandLine::Get(), TEXT("owslicegateshield")))
        {
            const int32 MayaLevel = W->GetSkills()->GetLevel();
            const FOpenWillowGearItem* Lowest = nullptr;
            for (const FOpenWillowGearItem& Gear : Inventory->GearItemList())
            {
                if (Gear.ItemType != TEXT("shield")) continue;
                if (Inventory->EquipGearById(Gear.Id, TEXT("shield"), MayaLevel))
                {
                    ShieldFixture = FString::Printf(TEXT("shield %s equipped from the gear manifest"), *Gear.Id);
                    break;
                }
                if (Gear.bLevelKnown && (!Lowest || Gear.Level < Lowest->Level)) Lowest = &Gear;
            }
            if (ShieldFixture.IsEmpty() && Lowest && Inventory->EquipGearById(Lowest->Id, TEXT("shield"), Lowest->Level))
                ShieldFixture = FString::Printf(TEXT("level rule overridden by the test: shield %s of level %d equipped on Maya of level %d"), *Lowest->Id, Lowest->Level, MayaLevel);
            if (ShieldFixture.IsEmpty())
            {
                const FOpenWillowMayaStats Defaults;
                W->GetMayaState().EquipShield(W->GetMaxHealth(), Defaults.ShieldRechargeDelay, Defaults.ShieldRechargeRate, true, GetWorld()->GetTimeSeconds());
                ShieldFixture = FString::Printf(TEXT("host stand-in shield of %.0f (no shield could be equipped from the gear manifest; not stock data)"), W->GetShieldCapacity());
            }
            UE_LOG(LogTemp, Display, TEXT("OWSLICEGATE note test fixture: %s, shield %.0f/%.0f"), *ShieldFixture, W->GetShield(), W->GetShieldCapacity());
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
                // A range kill rolls the stock gun chance (0.085), so most kills drop no gun; the range's switch forces a gun
                // (it retries up to 8 seeds). Named as a fixture on the loot link.
                if (IConsoleManager::Get().FindConsoleObject(TEXT("ow.RangeLoot")))
                {
                    GEngine->Exec(GetWorld(), TEXT("ow.RangeLoot always"));
                    bLootForced = true;
                }
                GEngine->Exec(GetWorld(), TEXT("ow.Range"));
                return;
            }
            if (StepTime < 3.f) return;   // the range spawns its targets
            // A refused range is a data problem, not a walking one: named here, and no other target in the world is taken for its enemy.
            if (const UOpenWillowRangeSubsystem* Range = GetWorld()->GetSubsystem<UOpenWillowRangeSubsystem>(); Range && !Range->LastRefusal().IsEmpty())
            {
                FailLink(TEXT("reach_fight_area"), FString::Printf(TEXT("range refused: %s"), *Range->LastRefusal()));
                Finish(TEXT("range refused"));
                return;
            }
            Enemy = NearestLiveTarget(true);
            if (!Enemy) { FailLink(TEXT("reach_fight_area"), TEXT("ow.Range ran but no live target is in the world")); Finish(TEXT("no enemy")); return; }
            // Stated up front: the fight and the Phaselock then run on the engine's shapes, not on the stock pawn.
            if (!Enemy->IsStockPawn()) Fixture(TEXT("selected enemy on fallback shapes, no stock mesh"));
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
        HitsBeforeFight = W->GetHitCount();
        FightStartedAt = GetWorld()->GetTimeSeconds();
        W->SetHitCauserTracking(true);   // off again once link 4 has read it
        if (!ShieldFixture.IsEmpty())
            UE_LOG(LogTemp, Display, TEXT("OWSLICEGATE note Maya's shield at the start of the fight: %.0f/%.0f"), W->GetShield(), W->GetShieldCapacity());
        Next(EStep::Fight);
        break;
    }

    case EStep::Fight: {
        if (!ShieldFixture.IsEmpty()) Fixture(ShieldFixture);
        if (bSlowFired) Fixture(TEXT("one shot at a time until the selected enemy was wounded"));
        // The first down of the fight: had the selected enemy hit her by then? A hit after she is Up again does not undo this.
        // DownAt also catches a down and a second wind between two of these ticks; a down from before the fight is older.
        const int32 SelectedHits = Enemy ? W->GetHitCountFrom(Enemy.Get()) : 0;
        if ((W->IsDown() || W->GetMayaState().DownAt >= FightStartedAt) && !bDownSeenInFight)
        {
            bDownSeenInFight = true;
            bDownBeforeEnemyHit = SelectedHits == 0;
        }
        const bool bEnemyAlive = Enemy && Enemy->IsAutoAimTarget();
        if (bEnemyAlive) LastEnemyLocation = Enemy->GetActorLocation();   // where it stands while alive (a dead actor may be moved away)
        // Range mode: wound the selected enemy so the den is alerted, then hold fire until it has hit Maya, so a quick kill
        // cannot end the fight before it attacks. A test shortcut, named as a fixture with how the hold ended. It ends if she
        // goes down (a kill can still give her a second wind), if the enemy dies, or after AttackWait, and it never restarts.
        if (bRangeMode && Enemy && HoldEnd.IsEmpty() && Enemy->HealthFraction() < 1.f)
        {
            const TCHAR* Ended = SelectedHits > 0 ? TEXT("hit") : bDownSeenInFight ? TEXT("Maya down") : !bEnemyAlive ? TEXT("enemy died")
                : StepTime >= AttackWait ? TEXT("timeout") : nullptr;
            if (!Ended)
            {
                AimAt(Enemy->AimPoint());
                HeldFireFor += Delta;
                return;
            }
            if (HeldFireFor > 0.f) HoldEnd = Ended;
        }
        if (bEnemyAlive)
        {
            // Until the wound lands, one shot at a time: FireOnce has no fire-rate gate and a shot takes time to land, so
            // firing every tick puts enough in flight to kill a weak enemy before the hold can start. Named as a fixture.
            const bool bUnwounded = bRangeMode && HoldEnd.IsEmpty() && SelectedHits == 0 && !bDownSeenInFight && Enemy->HealthFraction() >= 1.f;
            bSlowFired |= bUnwounded;
            if (StepTime < KillTimeout) { if (ShootAt(Enemy, bUnwounded ? WoundShotInterval : 0.f)) ++KillShots; return; }
            if (HeldFireFor > 0.f) Fixture(HoldFixture());
            FailLink(TEXT("kill_enemy"), FString::Printf(TEXT("%s still alive after %.0fs and %d shots (health %.0f%%)"),
                *Enemy->GetName(), StepTime, KillShots, 100.f * Enemy->HealthFraction()));
        }
        else if (Enemy)
        {
            KillLocation = LastEnemyLocation;
            const FOpenWillowWeaponItem* Held = Inventory->ActiveWeapon();
            if (HeldFireFor > 0.f) Fixture(HoldFixture());
            PassLink(TEXT("kill_enemy"), FString::Printf(TEXT("%s killed with %d FireOnce shots from %s in %.1fs, %.1fs of it with fire held (hold ended by: %s)"), *Enemy->GetName(), KillShots,
                Held ? *Held->Name : TEXT("?"), StepTime, HeldFireFor, HoldEnd.IsEmpty() ? TEXT("no hold") : *HoldEnd));
        }
        else FailLink(TEXT("kill_enemy"), TEXT("the enemy disappeared before it died"));
        // Only a hit whose causer is the selected enemy counts; others are reported, not credited. Link cleared the list,
        // and this link relied on the shield too.
        if (!ShieldFixture.IsEmpty()) Fixture(ShieldFixture);
        if (bSlowFired) Fixture(TEXT("one shot at a time until the selected enemy was wounded"));
        if (HeldFireFor > 0.f) Fixture(HoldFixture());
        // The fight's hits by causer: the selected enemy, another actor, or none given.
        const int32 EnemyHits = Enemy ? W->GetHitCountFrom(Enemy.Get()) : 0;
        const int32 UnattributedHits = W->GetHitCountFrom(nullptr);
        const int32 OtherHits = W->GetHitCount() - HitsBeforeFight - EnemyHits - UnattributedHits;
        W->SetHitCauserTracking(false);
        const FString Others = FString::Printf(TEXT("other hits: %d by other causers, %d with no causer"), OtherHits, UnattributedHits);
        if (bDownBeforeEnemyHit) FailLink(TEXT("enemy_attacks_maya"), FString::Printf(TEXT("Maya went down during the fight before the selected enemy had hit her (%d later hit(s) by it not credited; %s)"),
            EnemyHits, *Others));
        else if (EnemyHits > 0) PassLink(TEXT("enemy_attacks_maya"), FString::Printf(TEXT("%d hit(s) on Maya caused by %s during the fight (%s), health %.0f/%.0f"),
            EnemyHits, *Enemy->GetName(), *Others, W->GetHealth(), W->GetMaxHealth()));
        // The range's enemies attack, so no hit from the selected one is a real miss. Only the stand-in lacks an attack.
        else if (bRangeMode) FailLink(TEXT("enemy_attacks_maya"), FString::Printf(TEXT("no hit on Maya from the selected enemy during the fight (%s)"), *Others));
        else MissingLink(TEXT("enemy_attacks_maya"), FString::Printf(TEXT("the stand-in target has no attack (%s)"), *Others));
        Next(EStep::Recover);
        break;
    }

    case EStep::Recover: {
        // Maya has no shield, so a pack of range enemies can put her down during the fight; the later links need her Up.
        // Test fixtures, named on the next link: the range's enemies go passive (they stay targets for Phaselock), and
        // Maya is set Up with full health. A bled-out Maya is left to the walker's own respawn (no fixture needed).
        if (!bRecoverStarted)
        {
            bRecoverStarted = true;
            if (bRangeMode && IConsoleManager::Get().FindConsoleObject(TEXT("ow.RangeMode")))
            {
                GEngine->Exec(GetWorld(), TEXT("ow.RangeMode passive"));
                Fixture(TEXT("range set passive after the fight"));
            }
            // -owslicegatedownafterfight: a fight has never left Maya down (a kill gives her a second wind), so this puts
            // her down here to run the revive below. The damage lands now; the next tick sees her Injured.
            if (FParse::Param(FCommandLine::Get(), TEXT("owslicegatedownafterfight")) && !W->IsDown())
            {
                Fixture(TEXT("lethal damage at the end of the fight"));
                FHitResult Hit;
                UGameplayStatics::ApplyPointDamage(W, 1.0e6f, FVector::ForwardVector, Hit, nullptr, nullptr, UDamageType::StaticClass());
                return;
            }
        }
        if (W->GetDownPhase() == AOpenWillowWalker::EDownPhase::Dead && StepTime < BleedTimeout) return;   // respawn due
        FOpenWillowMayaState& State = W->GetMayaState();
        const bool bWasDown = W->IsDown();
        // With a shield (-owslicegateshield) only a downed Maya is restored: her health after the fight is kept. A Maya
        // still Dead after the wait is not touched (SetHealth does not revive from Dead): the next links log NOT_AVAILABLE.
        if (W->GetDownPhase() != AOpenWillowWalker::EDownPhase::Dead && (bWasDown || (ShieldFixture.IsEmpty() && W->GetHealth() < W->GetMaxHealth())))
        {
            const float FromHealth = W->GetHealth();
            State.SetHealth(State.Base.MaxHealth, State.Base.MaxHealth, GetWorld()->GetTimeSeconds());
            UE_LOG(LogTemp, Display, TEXT("OWSLICEGATE note test fixture: Maya restored from %s, health %.0f -> %.0f/%.0f, down now %d"),
                bWasDown ? TEXT("down") : TEXT("up"), FromHealth, W->GetHealth(), W->GetMaxHealth(), W->IsDown());
            if (!bWasDown) Fixture(TEXT("Maya healed to full after the fight"));
            else if (!W->IsDown()) Fixture(TEXT("Maya revived to full health after the fight"));
        }
        Next(EStep::Loot);
        break;
    }

    case EStep::Loot: {
        if (!Drop)
            for (TActorIterator<AOpenWillowInventoryPickup> It(GetWorld()); It; ++It)
                if (!PickupsBeforeKill.Contains(*It) && FVector::Dist(It->GetActorLocation(), KillLocation) < LootRadius) { Drop = *It; break; }
        if (!Drop && StepTime < LootWait) return;
        // A new pickup elsewhere is a drop in the wrong place: a FAIL with where it is, not "no drop".
        AOpenWillowInventoryPickup* FarDrop = nullptr;
        if (!Drop)
            for (TActorIterator<AOpenWillowInventoryPickup> It(GetWorld()); It; ++It)
                if (!PickupsBeforeKill.Contains(*It)) { FarDrop = *It; break; }
        if (bLootForced) Fixture(TEXT("guaranteed loot roll"));
        if (Drop) PassLink(TEXT("loot_drop_on_kill"), FString::Printf(TEXT("pickup \"%s\" %.0f uu from the corpse"), *Drop->DisplayName(),
            FVector::Dist(Drop->GetActorLocation(), KillLocation)));
        else if (FarDrop) FailLink(TEXT("loot_drop_on_kill"), FString::Printf(TEXT("a pickup \"%s\" appeared after the kill, but at %s, %.0f uu from the corpse at %s (more than %.0f)"),
            *FarDrop->DisplayName(), *FarDrop->GetActorLocation().ToCompactString(), FVector::Dist(FarDrop->GetActorLocation(), KillLocation),
            *KillLocation.ToCompactString(), LootRadius));
        // With the roll forced, a kill with no drop is a real miss; otherwise the build has no sure drop to wait for.
        else if (bLootForced) FailLink(TEXT("loot_drop_on_kill"), FString::Printf(TEXT("no pickup appeared within %.0f uu of the kill in %.0fs, with the gun roll forced"), LootRadius, LootWait));
        else MissingLink(TEXT("loot_drop_on_kill"), FString::Printf(TEXT("no pickup appeared within %.0f uu of the kill in %.0fs, and this build cannot force a gun roll"), LootRadius, LootWait));
        Next(EStep::Pickup);
        break;
    }

    case EStep::Pickup: {
        // The roll source is read even when a real drop exists: the decoy link's two test drops always need it.
        if (!bPickupStarted)
        {
            if (!FParse::Value(FCommandLine::Get(), TEXT("owslicegategun="), RollSource))
            {
                FString ItemDir;
                if (FParse::Value(FCommandLine::Get(), TEXT("owitems="), ItemDir)) RollSource = FOpenWillowWeaponGen::RewardSource(ItemDir);
            }
        }
        // Until enemies drop loot: roll a gun and drop it 150 uu ahead of Maya (as -owpickupcardshots does), named as a
        // fixture on the link, so the pickup path (card, use key) still runs. A real drop is used as it is.
        const bool bFirstTick = !bPickupStarted;
        bPickupStarted = true;
        if (!Drop && bFirstTick)
        {
            if (const FString Gap = RollSetupGap(); !Gap.IsEmpty())
            {
                MissingLink(TEXT("pick_up_gun"), Gap + TEXT(", and the kill left no drop to pick up"));
                Next(EStep::Equip);
                return;
            }
            FString Why;
            Drop = DropRolledGun(1, W->GetActorLocation() + FRotator(0, W->GetControlRotation().Yaw, 0).Vector() * 150.f, Why);
            if (!Drop) { FailLink(TEXT("pick_up_gun"), Why); Next(EStep::Equip); return; }
            bTestDrop = true;
            return;
        }
        if (!RunAimedPickup(Drop)) return;
        if (bTestDrop) Fixture(TEXT("gun rolled and dropped by the test"));
        GunId = PickedId == TEXT("none") ? FString() : PickedId;
        // PASS only when the card named the aimed drop and E picked up that same item.
        const bool bAccepted = PickupAccepted();
        const FString Ids = FString::Printf(TEXT("aimed_drop=%s card_id=%s picked_id=%s accepted=%d"), *DropId, *CardId, *PickedId, bAccepted);
        if (CardId == DropId && PickedId == DropId && bAccepted)
            PassLink(TEXT("pick_up_gun"), FString::Printf(TEXT("%s: the pickup card named the aimed drop at the use-key press and E picked it up (card state only; HUD drawing not checked)"), *Ids));
        else FailLink(TEXT("pick_up_gun"), FString::Printf(TEXT("%s: the card and the picked-up item must both be the aimed drop"), *Ids));
        Next(EStep::PickupDecoy);
        break;
    }

    case EStep::PickupDecoy: {
        // The aimed drop with a nearer decoy beside Maya, outside the card's cone: the card must name the aimed one, and E
        // must take it (PR #101: an aimed pickup in reach wins over the nearest). Taking the decoy is a FAIL.
        if (!DecoyAimed)
        {
            if (const FString Gap = RollSetupGap(); !Gap.IsEmpty())
            {
                MissingLink(TEXT("pick_up_aimed_not_nearest"), Gap + TEXT(", so the two test drops cannot be made"));
                Next(EStep::Equip);
                return;
            }
            const FVector From = W->GetActorLocation();
            const FVector Forward = FRotator(0, W->GetControlRotation().Yaw, 0).Vector();
            FString Why;
            DecoyAimed = DropRolledGun(2, From + Forward * 150.f, Why);
            // RunAimedPickup stands Maya 80 uu from the aimed drop on the side she comes from; the decoy goes 45 uu to her
            // right there: nearer than the aimed drop, and about 90 degrees off her aim.
            const FVector Stand = DecoyAimed ? DecoyAimed->GetActorLocation() + (From - DecoyAimed->GetActorLocation()).GetSafeNormal2D() * 80.f : From;
            DecoyNear = DecoyAimed ? DropRolledGun(3, Stand + FRotator(0, W->GetControlRotation().Yaw + 90.f, 0).Vector() * 45.f, Why) : nullptr;
            if (!DecoyAimed || !DecoyNear)
            {
                // Not the two-guns fixture: the pair was never complete. The detail says how far the setup got.
                FailLink(TEXT("pick_up_aimed_not_nearest"), FString::Printf(TEXT("test drops incomplete: aimed drop %s, nearer decoy not made: %s"),
                    DecoyAimed ? *PickupId(DecoyAimed) : TEXT("not made"), *Why));
                if (IsValid(DecoyAimed)) DecoyAimed->Destroy();
                Next(EStep::Equip);
                return;
            }
            Fixture(TEXT("two guns rolled and dropped by the test"));
            NearId = PickupId(DecoyNear);
            // Rolled ids come from the balance and the seed: two rolls can share one, and then the ids cannot tell which
            // drop E took. No result is possible from such a pair.
            if (NearId == PickupId(DecoyAimed))
            {
                FailLink(TEXT("pick_up_aimed_not_nearest"), FString::Printf(TEXT("the two test drops share the id %s, so E's pick cannot be told apart"), *NearId));
                DecoyAimed->Destroy();
                DecoyNear->Destroy();
                Next(EStep::Equip);
                return;
            }
            return;
        }
        if (!RunAimedPickup(DecoyAimed)) return;
        const bool bAccepted = PickupAccepted();
        const FString Ids = FString::Printf(TEXT("aimed_drop=%s nearer_decoy=%s card_id=%s picked_id=%s accepted=%d"), *DropId, *NearId, *CardId, *PickedId, bAccepted);
        if (CardId != DropId) FailLink(TEXT("pick_up_aimed_not_nearest"), Ids + TEXT(": the card did not name the aimed drop"));
        else if (PickedId == DropId && NearId != DropId && bAccepted) PassLink(TEXT("pick_up_aimed_not_nearest"), Ids + TEXT(": E picked up the aimed drop, not the nearer decoy"));
        else if (PickedId == NearId) FailLink(TEXT("pick_up_aimed_not_nearest"), Ids + TEXT(": E took the nearer decoy, not the aimed drop"));
        else FailLink(TEXT("pick_up_aimed_not_nearest"), Ids + TEXT(": E did not pick up the aimed drop"));
        for (AOpenWillowInventoryPickup* Left : {DecoyAimed.Get(), DecoyNear.Get()}) if (IsValid(Left)) Left->Destroy();
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
        // Before any result, so that a failing path names it too.
        if (bPhaselockPointGranted) Fixture(TEXT("phaselock point granted"));
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
                // A miss leaves no target and a short cooldown, so the last retry usually falls inside that cooldown. The misses
                // are the result, not the refusal; a cast that never went out reports its reason.
                FailLink(TEXT("phaselock_enemy"), PhaselockCasts > 0
                    ? FString::Printf(TEXT("%d cast(s) went out and all missed: no preferred target (the log's \"Phaselock targeting\" line says why)"), PhaselockCasts)
                    : FString::Printf(TEXT("no cast went out: %s"), *Reason));
                Next(EStep::Accept);
                return;
            }
            Enemy = Target;
            W->UsePhaselock();
            ++PhaselockCasts;
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
            // The range is done with: clear it (fixture) so no range actor takes the mission's shots or attacks Maya.
            if (bRangeMode && IConsoleManager::Get().FindConsoleObject(TEXT("ow.RangeClear")))
            {
                GEngine->Exec(GetWorld(), TEXT("ow.RangeClear"));
                Fixture(TEXT("range cleared before the mission"));
            }
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
        if (!bMagazineFixtureSet) { bMagazineFixtureSet = true; ApplyMagazineFixture(W); }
        if (Quest->Status() == 1 && MissionDummy && StepTime < DummyTimeout) { if (ShootAt(MissionDummy, 0.6f)) ++DummyShots; return; }
        if (Quest->Status() == 2)
        {
            if (!bDummyLinkPassed) { bDummyLinkPassed = true; PassLink(TEXT("mission_objective"), FString::Printf(TEXT("trigger touched, dummy registered, %d shot(s) with the lent weapon completed the objectives"), DummyShots)); }
            if (HoldForFixtureReload(W, Delta)) return;   // the magazine fixture's reload, after the link; false at once when the switch is absent
        }
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
    {
        // All three, as SLICE_GATE.md records them: the mission complete, the experience reward, and the lent pistol
        // taken back (neither in the backpack nor held).
        const int64 Gained = W->GetSkills()->GetExperience() - ExperienceBeforeTurnIn;
        const FOpenWillowWeaponItem* Held = Inventory->ActiveWeapon();
        const bool bReturned = !LentId.IsEmpty() && Inventory->FindItemIndexById(LentId) == INDEX_NONE
            && !(Held && UOpenWillowInventory::StableId(*Held) == LentId);
        const FString Detail = FString::Printf(TEXT("status %d (want 3), experience %+lld (want > 0), lent weapon %s %s"), Quest->Status(), Gained,
            LentId.IsEmpty() ? TEXT("(none recorded)") : *LentId, bReturned ? TEXT("taken back") : TEXT("NOT taken back"));
        if (Quest->Status() == 3 && Gained > 0 && bReturned) PassLink(TEXT("mission_turn_in"), Detail);
        else FailLink(TEXT("mission_turn_in"), Detail);
    }
        Next(EStep::Down);
        break;

    case EStep::Down:
        if (StepTime < 0.5f) return;
        if (RespawnAtBefore == 0.f)
        {
            RespawnAtBefore = W->LastRespawnAt();
            DeathLocation = W->GetActorLocation();
            if (W->IsDown())
            {
                MissingLink(TEXT("go_down"), TEXT("Maya down from a previous link before the lethal damage was due"));
                Next(EStep::Bleed);
                return;
            }
            bLethalApplied = true;
            // Test fixture: lethal damage through the walker's damage entry (no enemy can kill Maya on this build yet).
            Fixture(TEXT("lethal damage applied by the test"));
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
        if (bLethalApplied) Fixture(TEXT("down state from the test's lethal damage"));
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
        if (StepTime > 2.f && OpenWillowAudio::RecordingSettled() && !FParse::Param(FCommandLine::Get(), TEXT("owslicegatestay"))) { FPlatformMisc::RequestExit(false); SetComponentTickEnabled(false); }
        break;
    }
}
