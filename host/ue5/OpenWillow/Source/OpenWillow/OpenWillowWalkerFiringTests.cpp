// The weapons-projectile lane's in-engine harnesses, split from OpenWillowWalker.cpp to keep the fire path readable:
// -owfiringtest (RunFiringTest: fires each -owgunids= gun at the dummy and a wall and logs OWFIRING result lines for
// tools/check_weapon_firing.py) and -owfiringshots (RunFiringShots: frames at the real frames' pose for
// tools/firing_contact_sheet.py). Test code only; the game logic never calls it.
#include "OpenWillowWalker.h"
#include "InputCoreTypes.h"
#include "InputKeyEventArgs.h"
#include "OpenWillowMover.h"
#include "OpenWillowQuest.h"
#include "Engine/DamageEvents.h"
#include "GameFramework/WorldSettings.h"
#include "OpenWillowArmsAnimInstance.h"
#include "OpenWillowCombatTarget.h"
#include "OpenWillowInventory.h"
#include "OpenWillowInventoryActionTest.h"
#include "OpenWillowInventoryPickup.h"
#include "OpenWillowInventoryWidget.h"
#include "OpenWillowMayaHUD.h"
#include "OpenWillowSkills.h"
#include "Blueprint/UserWidget.h"
#include "Misc/Paths.h"
#include "Misc/PackageName.h"
#include "Misc/FileHelper.h"
#include "Dom/JsonObject.h"
#include "Serialization/JsonSerializer.h"
#include "OpenWillowShotFx.h"
#include "OpenWillowProjectile.h"
#include "OpenWillowStatusEffect.h"
#include "OpenWillowSkillEffects.h"
#include "OpenWillowWeaponGen.h"
#include "OpenWillowGunLook.h"
#include "OpenWillowInventoryPreviewActor.h"
#include "HAL/FileManager.h"
#include "ShaderCompiler.h"
#include "Animation/AnimSequence.h"
#include "Camera/CameraComponent.h"
#include "Components/CapsuleComponent.h"
#include "Components/InputComponent.h"
#include "Components/SkeletalMeshComponent.h"
#include "Engine/Engine.h"
#include "EngineUtils.h"
#include "Engine/GameViewportClient.h"
#include "Engine/SkeletalMesh.h"
#include "Engine/StaticMesh.h"
#include "Engine/StaticMeshActor.h"
#include "Components/StaticMeshComponent.h"
#include "Misc/CommandLine.h"
#include "Misc/Parse.h"
#include "GameFramework/CharacterMovementComponent.h"
#include "Kismet/GameplayStatics.h"
#include "Materials/MaterialInterface.h"
#include "Materials/MaterialInstanceDynamic.h"
#include "Engine/Texture2D.h"
#include "GameFramework/PlayerController.h"
#include "UnrealClient.h"
#include "Rendering/DrawElements.h"
#include "Styling/CoreStyle.h"

// Shared with RunGunShots (OpenWillowWalker.cpp).
TArray<FString> GunIdsFromCommandLine(FString& List);

static const TCHAR* ShotKindName(const FOpenWillowFiringData& Firing)
{
    if (!Firing.bLoaded) return TEXT("Trace");
    switch (Firing.Kind)
    {
    case EOpenWillowShotKind::Rocket: return TEXT("Rocket");
    case EOpenWillowShotKind::Bullet: return TEXT("Bullet");
    default: return TEXT("Trace");
    }
}
void AOpenWillowWalker::RunFiringShots(float Now)
{
    // -owfiringshots -owgunids=a,b,c: per gun, equip it, pin the view to the real reference's pose (pitch 0, yaw 90, FOV 77.55),
    // put a flat wall 465.9 uu ahead of the eye, let the gun settle, then one trigger pull with a frame every ~46 ms. The shot is
    // frame 4 and the captures run from frame 02 to frame 24, the numbering of the real index.json. Files are
    // OWFiring_<gun>_NN.png in Saved/Screenshots/WindowsEditor; tools/capture_firing_frames.ps1 copies them out.
    static const float Settle = [] { float Seconds = 3.f; FParse::Value(FCommandLine::Get(), TEXT("owfiringshotsettle="), Seconds); return Seconds; }();
    static const float Interval = [] { float Seconds = 0.046f; FParse::Value(FCommandLine::Get(), TEXT("owfiringinterval="), Seconds); return Seconds; }();
    static const float Fov = [] { float Degrees = 77.55f; FParse::Value(FCommandLine::Get(), TEXT("owfiringshotsfov="), Degrees); return Degrees; }();
    const float Yaw = 90.f;                 // the reference's view_rot yaw (16384 UE units)
    const float WallDistance = 465.9f;      // the reference's wall, measured along the view from the eye
    const float WallHalfThickness = 50.f;   // the engine cube is 100 uu thick at scale 1
    const int32 FirstFrame = 2, ShotFrame = 4, LastFrame = 24;
    static TArray<FString> Ids;
    static TWeakObjectPtr<AStaticMeshActor> ShotWall;
    static int32 Gun = 0, Phase = 0, Frame = 0, ShotsBefore = 0;
    static float PhaseAt = 0.f, ShotAt = 0.f, FinishedAt = -1.f;
    static bool bPressed = false;
    if (Ids.Num() == 0)
    {
        FString List;
        Ids = GunIdsFromCommandLine(List);
        UE_LOG(LogTemp, Display, TEXT("OpenWillow firing shots parsed %d recipe IDs: %s"), Ids.Num(), *List);
    }
    if (Ids[0] == TEXT("none") || Gun >= Ids.Num())
    {
        if (ShotWall.IsValid()) ShotWall->Destroy();
        if (FinishedAt < 0.f) FinishedAt = Now;
        if (Now > FinishedAt + 1.f)
            if (APlayerController* PC = Cast<APlayerController>(Controller)) PC->ConsoleCommand(TEXT("quit"));
        return;
    }
    // Pinned every tick: the view stays at the reference's pose and camera recoil cannot move it. The FOV is set after UpdateAim.
    Controller->SetControlRotation(FRotator(0.f, Yaw, 0.f));
    Camera->SetFieldOfView(Fov);
    const FString Id = Ids[Gun];
    // Release the trigger once the shot has happened (or after 0.25 s), whatever the phase.
    if (bPressed && (OpenWillowFiring::Counts.Shots > ShotsBefore || Now >= ShotAt + 0.25f))
    {
        FireReleased();
        bPressed = false;
    }
    switch (Phase)
    {
    case 0:   // equip the gun, then place the wall ahead of the eye
    {
        FireReleased();
        int32 Index = INDEX_NONE;
        for (int32 I = 0; I < Inventory->Items().Num(); ++I)
            if (Inventory->Items()[I].Id == Id) { Index = I; break; }
        if (Index == INDEX_NONE || !EquipItem(Index, 0) || !bWeaponOut || !Inventory->ActiveWeapon() || Inventory->ActiveWeapon()->Id != Id)
        {
            UE_LOG(LogTemp, Error, TEXT("OpenWillow firing shots skipped %s: named item could not be equipped"), *Id);
            ++Gun;
            return;
        }
        UE_LOG(LogTemp, Display, TEXT("OpenWillow firing shots equipped id=%s name=%s"), *Id, *Inventory->ActiveWeapon()->Name);
        const FVector Eye = Camera->GetComponentLocation();
        const FVector Forward = FRotator(0.f, Yaw, 0.f).Vector();
        FHitResult Clear;
        FCollisionQueryParams Query(SCENE_QUERY_STAT(OpenWillowFiringShotsClear), true, this);
        if (GetWorld()->LineTraceSingleByChannel(Clear, Eye, Eye + Forward * 2000.f, ECC_Visibility, Query) && Clear.Distance < WallDistance - 10.f)
            UE_LOG(LogTemp, Warning, TEXT("OpenWillow firing shots: geometry %.1f uu ahead of the eye; the wall is placed at %.1f uu anyway"),
                Clear.Distance, WallDistance);
        AStaticMeshActor* Wall = ShotWall.Get();
        if (!Wall)
        {
            UStaticMesh* Cube = LoadObject<UStaticMesh>(nullptr, TEXT("/Engine/BasicShapes/Cube.Cube"));
            Wall = GetWorld()->SpawnActor<AStaticMeshActor>(FVector::ZeroVector, FRotator::ZeroRotator);
            if (!Cube || !Wall)
            {
                UE_LOG(LogTemp, Error, TEXT("OpenWillow firing shots: no wall (cube %s, actor %s); skipped %s"),
                    Cube ? TEXT("loaded") : TEXT("missing"), Wall ? TEXT("spawned") : TEXT("missing"), *Id);
                ++Gun;
                return;
            }
            UStaticMeshComponent* WallMesh = Wall->GetStaticMeshComponent();
            WallMesh->SetMobility(EComponentMobility::Movable);
            WallMesh->SetStaticMesh(Cube);
            WallMesh->SetCollisionProfileName(TEXT("BlockAll"));
            ShotWall = Wall;
        }
        // Scale 1 along the view (100 uu thick), 40 across and up (4000 uu): the near face sits exactly WallDistance ahead.
        Wall->SetActorRotation(FRotator(0.f, Yaw, 0.f));
        Wall->SetActorScale3D(FVector(1.f, 40.f, 40.f));
        Wall->SetActorLocation(Eye + Forward * (WallDistance + WallHalfThickness));
        UE_LOG(LogTemp, Display, TEXT("OpenWillow firing shots wall for %s: eye %s, near face %.1f uu along yaw %.0f, FOV %.2f"),
            *Id, *Eye.ToString(), WallDistance, Yaw, Fov);
        PhaseAt = Now;
        Phase = 1;
        break;
    }
    case 1:   // let the new gun's materials and the shader compiler settle
        if (Now < PhaseAt + Settle || (GShaderCompilingManager && GShaderCompilingManager->IsCompiling())) return;
        Frame = FirstFrame;
        PhaseAt = Now;   // frame FirstFrame is captured now; the shot is ShotFrame - FirstFrame intervals later
        Phase = 2;
        break;
    case 2:   // one capture per tick at the reference's interval; the trigger is pulled with frame ShotFrame
    {
        if (Frame > LastFrame)
        {
            FinishedAt = Now;
            Phase = 3;
            break;
        }
        if (Now < PhaseAt + (Frame - FirstFrame) * Interval) return;
        if (Frame == ShotFrame)
        {
            FOpenWillowWeaponItem* Item = Inventory->ActiveWeaponMutable();
            if (!Item || Item->Id != Id)
            {
                UE_LOG(LogTemp, Error, TEXT("OpenWillow firing shots lost %s: the active weapon changed"), *Id);
                Phase = 3;
                FinishedAt = Now;
                break;
            }
            if (UOpenWillowInventory::MagazineLeft(*Item) < 2) Item->MagazineLeft = -1;   // refill: no reload in this capture
            ShotsBefore = OpenWillowFiring::Counts.Shots;
            ShotAt = Now;
            bPressed = true;
            FirePressed();
        }
        FScreenshotRequest::RequestScreenshot(FString::Printf(TEXT("OWFiring_%s_%02d.png"), *Id, Frame), true, false);
        UE_LOG(LogTemp, Display, TEXT("OpenWillow firing shots capture %s frame %02d at %.4f s (shot %s)"), *Id, Frame, Now,
            Frame == ShotFrame ? TEXT("here") : TEXT("no"));
        ++Frame;
        break;
    }
    case 3:   // leftover projectiles are removed so the next gun starts clean
        if (Now < FinishedAt + 0.5f) return;
        {
            int32 Leftover = 0;
            for (TActorIterator<AOpenWillowProjectile> It(GetWorld()); It; ++It)
            {
                ++Leftover;
                It->Destroy();
            }
            UE_LOG(LogTemp, Display, TEXT("OpenWillow firing shots done %s: %d projectiles removed, %d shots"), *Id, Leftover,
                OpenWillowFiring::Counts.Shots);
        }
        FireReleased();
        ++Gun;
        Phase = 0;
        break;
    default:
        return;
    }
}
void AOpenWillowWalker::RunFiringTest(float Now)
{
    // -owfiringtest -owgunids=a,b,c (with -owcombattest): per gun, equip it, then five pulls at the dummy, three at a wall
    // and, for a gun with a status block, twenty more at the dummy. A pull is one press and one release, so the shot and
    // the fire interval are the host's own. After the last pull the projectiles and status effects get up to 6 s, then one
    // OWFIRING result line counts what happened. The magazine is refilled before a pull that would run it dry.
    struct FSegment
    {
        bool bWall;
        int32 Pulls;
        bool bThrow = false;   // Tediore: reload with 5 rounds left at the dummy, then log the refill
        int32 Lifecycle = 0;   // burst guns: 1 = reload mid-burst, 2 = respawn mid-burst; no shot may follow either
        bool bWide = false;    // one pull beside the dummy as a line, then one with a TraceExtent box (wide bullet)
    };
    static const float Settle = [] { float Seconds = 2.f; FParse::Value(FCommandLine::Get(), TEXT("owfiringsettle="), Seconds); return Seconds; }();
    static TArray<FString> Ids;
    static TArray<FSegment> Segments;
    static int32 Gun = 0, Step = 0, Segment = 0, PullsLeft = 0, ShotsBefore = 0;
    static bool bPulling = false;
    static float StepAt = 0.f, PullAt = 0.f, FinishAt = 0.f, FinishedAt = -1.f;
    static FString Kind;
    if (Ids.Num() == 0)
    {
        FString List;
        Ids = GunIdsFromCommandLine(List);
        UE_LOG(LogTemp, Display, TEXT("OpenWillow firing test parsed %d recipe IDs: %s"), Ids.Num(), *List);
    }
    // After the last gun: one grenade at the dummy, by pressing G through the player controller's input path (the same
    // action mapping a keyboard press goes through): the binding, the ammo, the flight, the explosion and its hit events.
    // Then the pool is emptied and G pressed again: the throw must be refused.
    static int32 GrenadeStep = 0, GrenadeAmmoBefore = 0, GrenadeAmmoAfter = 0;
    static float GrenadeAt = 0.f, GrenadeHealthBefore = 0.f;
    static bool bGrenadeDummyDied = false, bGrenadeAimed = false;
    if (Ids[0] != TEXT("none") && Gun >= Ids.Num() && GrenadeStep < 5 && CombatTarget.IsValid())
    {
        APlayerController* KeyPC = Cast<APlayerController>(Controller);
        const auto PressG = [KeyPC](EInputEvent Event) { if (KeyPC) KeyPC->InputKey(FInputKeyEventArgs::CreateSimulated(EKeys::G, Event, 1.f)); };
        const auto InFlight = [this]() { int32 Count = 0; for (TActorIterator<AOpenWillowProjectile> It(GetWorld()); It; ++It) ++Count; return Count; };
        // A dummy that dies respawns with full health, so the health difference alone would not show the kill.
        if (GrenadeStep == 2 && CombatTarget->IsDead()) bGrenadeDummyDied = true;
        // InputKey only queues the press: the data loads when the binding runs, a tick or so later, and that also sets the pending
        // release, which reads the view 0.52 s after the press. A throw at the dummy's centre passes over its torso: the upward bonus
        // and gravity lift the lob, and it starts 80 uu ahead and 20 uu left of the eye (AOpenWillowWalker::ReleaseGrenade). So aim where
        // the lob meets the dummy, as a player leads a throw. Only while the release is pending, once, so aimed=1 means the aim came
        // before it; and not in the steps below, whose timeout must always get its turn.
        if (GrenadeStep == 2 && !bGrenadeAimed && GrenadeData.bLoaded && GrenadeReleaseAt > 0.f)
        {
            const FVector Eye = Camera->GetComponentLocation();
            FVector Aim = CombatTarget->AimPoint();
            const FVector Toward = (Aim - Eye).GetSafeNormal2D();
            const float Flight = (float(FVector::Dist2D(Eye, Aim)) - 80.f) / GrenadeData.ProjectileSpeed;
            const float GravityZ = OpenWillowFiring::WorldGravityZ * GrenadeData.GravityScale;
            Aim.Z -= GrenadeData.UpwardVelocity * Flight + 0.5f * GravityZ * Flight * Flight;
            Aim += FVector::CrossProduct(FVector::UpVector, Toward) * 20.f;
            AimAt(Aim);
            bGrenadeAimed = true;
        }
        if (GrenadeStep == 0)
        {
            UOpenWillowStatusEffects::ClearAll(CombatTarget.Get());
            CombatTarget->ResetForTest();
            CombatTarget->RestoreHealth();
            AimAt(CombatTarget->AimPoint());
            OpenWillowFiring::Counts.Reset();
            GrenadeAt = Now;
            GrenadeStep = 1;
        }
        else if (GrenadeStep == 1 && Now >= GrenadeAt + 0.3f)
        {
            GrenadeAmmoBefore = Inventory->AmmoPool(EOpenWillowAmmoType::Grenade).Current;
            GrenadeHealthBefore = CombatTarget->GetHealth();
            PressG(IE_Pressed);
            GrenadeAt = Now;
            GrenadeStep = 2;
        }
        else if (GrenadeStep == 2 && ((Now >= GrenadeAt + 1.2f && InFlight() == 0) || Now >= GrenadeAt + 15.f))
        {
            PressG(IE_Released);
            GrenadeAmmoAfter = Inventory->AmmoPool(EOpenWillowAmmoType::Grenade).Current;
            // Past the 1.05 s lockout, so an empty pool is the only reason left for a refusal.
            Inventory->SetAmmoCurrent(EOpenWillowAmmoType::Grenade, 0);
            PressG(IE_Pressed);
            GrenadeAt = Now;
            GrenadeStep = 3;
        }
        else if (GrenadeStep == 3 && Now >= GrenadeAt + 0.3f)
        {
            PressG(IE_Released);
            const bool bRefusedEmpty = GrenadeReleaseAt == 0.f && InFlight() == 0 && Inventory->AmmoPool(EOpenWillowAmmoType::Grenade).Current == 0;
            Inventory->SetAmmoCurrent(EOpenWillowAmmoType::Grenade, GrenadeAmmoAfter);
            const FOpenWillowFiringStats& C = OpenWillowFiring::Counts;
            UE_LOG(LogTemp, Display, TEXT("OWFIRING grenade thrown=%d ammo_before=%d ammo_after=%d refused_empty=%d damage=%.2f splash_hits=%d splash=%.2f grenade_hits=%d unknown_sources=%d contact_age=%.3f fuse_age=%.3f contact_fuse=%.3f first_contact=%s dummy=%s health_before=%.2f health_after=%.2f dummy_died=%d aimed=%d"),
                GrenadeAmmoAfter < GrenadeAmmoBefore ? 1 : 0, GrenadeAmmoBefore, GrenadeAmmoAfter, bRefusedEmpty ? 1 : 0,
                GrenadeData.bLoaded ? GrenadeData.Splash.Damage : 0.f, C.SplashHits, C.Splash, C.GrenadeHits, C.UnknownSources,
                C.GrenadeContactAge, C.GrenadeFuseAge, GrenadeData.ContactFuse,
                C.GrenadeFirstContact.IsEmpty() ? TEXT("none") : *C.GrenadeFirstContact,
                *CombatTarget->GetName(), GrenadeHealthBefore, CombatTarget->GetHealth(), bGrenadeDummyDied ? 1 : 0, bGrenadeAimed ? 1 : 0);
            GrenadeStep = 5;
        }
        return;
    }
    // After the last gun: rolled guns (tools/check_weapon_firing.py check_rolled). A gun with burst count 1 must fire one shot
    // from one hold, and one with burst count 0 more than one. The hold is long enough for further shots, derived from the gun's
    // own fire interval, so one shot cannot just mean a short hold. Third: one that launches a projectile, which only the
    // firing template can make it do.
    static int32 RolledGun = 0, RolledStep = 0, RolledShotsBefore = 0, RolledLaunchesBefore = 0, RolledImpactsBefore = 0;
    static float RolledAt = 0.f, RolledHold = 0.f, RolledHeld = 0.f, RolledDamageBefore = 0.f;
    static int32 RolledStage = 1;     // what the gun was rolled at: the card is re-read from the same roll
    static bool bRolledDummyDied = false;
    static float RolledWobbleAtPress = 0.f;
    static float RolledPerShot = 0.f;      // what one trigger shot should deal, from the same value firing uses; -1 = can't be said
    static float RolledSavedMaxHealth = -1.f;
    // The rows are over: does a dummy killed by a hit come back at rest, as its respawn says? No row can show this, since none kills it.
    // tools/check_weapon_firing.py check_respawn_rest.
    static int32 RestStep = 0;
    static float RestAt = 0.f;
    if (Ids[0] != TEXT("none") && Gun >= Ids.Num() && RolledGun >= 4 && RestStep < 2 && CombatTarget.IsValid())
    {
        if (RestStep == 0)
        {
            if (CombatTarget->IsDead()) return;
            CombatTarget->SetMaxHealth(1.f);
            FOpenWillowHitEvent Lethal;
            Lethal.Instigator = this;
            Lethal.Target = CombatTarget.Get();
            Lethal.Kind = EOpenWillowHitKind::Direct;
            Lethal.Source = EOpenWillowHitSource::Bullet;
            Lethal.Damage = 1000.f;
            Lethal.Direction = (CombatTarget->AimPoint() - Camera->GetComponentLocation()).GetSafeNormal();
            FHitResult LethalHit;
            LethalHit.ImpactPoint = CombatTarget->AimPoint();
            LethalHit.Location = LethalHit.ImpactPoint;
            ApplyShotDamage(CombatTarget.Get(), Lethal, LethalHit);
            RestAt = Now;
            RestStep = 1;
            if (!CombatTarget->IsDead())
            {
                UE_LOG(LogTemp, Error, TEXT("OWFIRING respawn_rest unavailable=not_killed"));
                RestStep = 2;
            }
            return;
        }
        if (CombatTarget->IsDead() && Now < RestAt + 10.f) return;
        // The first tick it is alive: a swing left from the lethal hit shows here.
        if (CombatTarget->IsDead()) { UE_LOG(LogTemp, Error, TEXT("OWFIRING respawn_rest unavailable=no_respawn")); }
        else
        {
            UE_LOG(LogTemp, Display, TEXT("OWFIRING respawn_rest waited=%.2f wobble=%.3f wobble_speed=%.3f"), Now - RestAt, CombatTarget->WobbleSize(), CombatTarget->WobbleSpeed());
        }
        RestStep = 2;
    }
    // Then the dummy gets its own health back.
    if (RolledGun >= 4 && RolledSavedMaxHealth > 0.f && CombatTarget.IsValid())
    {
        CombatTarget->SetMaxHealth(RolledSavedMaxHealth);
        RolledSavedMaxHealth = -1.f;
    }
    if (Ids[0] != TEXT("none") && Gun >= Ids.Num() && RolledGun < 4 && CombatTarget.IsValid())
    {
        const int32 Wanted = RolledGun == 0 ? 1 : RolledGun == 1 ? 0 : RolledGun == 2 ? FOpenWillowWeaponGen::BurstTestRocket : FOpenWillowWeaponGen::BurstTestPool;
        const auto Unavailable = [Wanted](const TCHAR* Reason)
        {
            UE_LOG(LogTemp, Error, TEXT("OWFIRING rolled unavailable=%s want=%d"), Reason, Wanted);
            ++RolledGun;
            RolledStep = 0;
        };
        const FOpenWillowWeaponItem* Held = Inventory->ActiveWeapon();
        if (RolledStep >= 2 && CombatTarget->IsDead()) bRolledDummyDied = true;
        if (RolledStep == 0)
        {
            FireReleased();
            AimReleased();
            FOpenWillowWeaponItem Rolled;
            FString Reason;
            RolledStage = FMath::Max(1, Skills->GetLevel());
            if (!FOpenWillowWeaponGen::FindGunForBurstTest(Wanted, RolledStage, Rolled, Reason)) { Unavailable(*Reason); return; }
            const FString RolledId = Rolled.Id;
            if (!Inventory->AddToBackpack(MoveTemp(Rolled))) { Unavailable(TEXT("backpack_full")); return; }
            int32 Index = INDEX_NONE;
            for (int32 I = Inventory->Items().Num() - 1; I >= 0 && Index == INDEX_NONE; --I)
                if (Inventory->Items()[I].Id == RolledId) Index = I;
            if (Index == INDEX_NONE || !EquipItem(Index, 0) || !bWeaponOut || !Inventory->ActiveWeapon() || Inventory->ActiveWeapon()->Id != RolledId)
            {
                Unavailable(TEXT("not_equipped"));
                return;
            }
            RolledAt = Now;
            RolledStep = 1;
        }
        else if (!Held) Unavailable(TEXT("no_weapon"));   // the test must not wait for ever on a weapon that went away
        else if (RolledStep == 1 && Now >= RolledAt + Settle)
        {
            // Each row stands alone: a living dummy at home and at full health. An earlier gun can have killed it (it respawns in 3 s).
            if (CombatTarget->IsDead())
            {
                if (Now >= RolledAt + Settle + 10.f) Unavailable(TEXT("dummy_dead"));
                return;
            }
            UOpenWillowStatusEffects::ClearAll(CombatTarget.Get());
            CombatTarget->ResetForTest();
            // More health than any row deals, so no row can kill it: a death can't hide a miss, and respawn stays out of the row.
            if (RolledSavedMaxHealth < 0.f) RolledSavedMaxHealth = CombatTarget->GetMaxHealth();
            CombatTarget->SetMaxHealth(1e8f);
            bRolledDummyDied = false;
            RolledWobbleAtPress = CombatTarget->WobbleSize();
            // As OpenWillowWalker.cpp FireWeapon computes it: each pellet's direct hit, or a rocket's explosion, plus a bullet's
            // impact explosion, the splash following the shot's damage. Not said for damage over time, or falloff with distance.
            const FOpenWillowFiringData& Fired = Held->Firing;
            const float ShotAmount = ShotDamage(*Held, Now);
            const float SplashAmount = Fired.Splash.bValid && Held->Damage > 0.f ? Fired.Splash.Damage * (ShotAmount / Held->Damage) : 0.f;
            const bool bSaid = !(Fired.Status.bValid && Fired.Status.Dps > 0.f) && (!Fired.Splash.bValid || Fired.Splash.bFullDamage);
            RolledPerShot = bSaid ? (Fired.Kind == EOpenWillowShotKind::Rocket ? 0.f : ShotAmount) + SplashAmount : -1.f;
            RolledHold = 3.f / Held->FireRate + 0.1f;
            RolledShotsBefore = OpenWillowFiring::Counts.Shots;
            RolledLaunchesBefore = OpenWillowFiring::Counts.Launches;
            // By this gun's id: a status tick left by an earlier gun must not count as this gun's damage. The same gun can be picked
            // twice (a launcher with burst 1), so these are differences.
            RolledImpactsBefore = OpenWillowFiring::Counts.ImpactsByWeapon.FindOrAdd(Held->Id);
            RolledDamageBefore = OpenWillowFiring::Counts.DamageByWeapon.FindOrAdd(Held->Id);
            AimAt(CombatTarget->AimPoint());
            FirePressed();
            RolledAt = Now;
            RolledStep = 2;
        }
        else if (RolledStep == 2 && Now >= RolledAt + RolledHold)
        {
            RolledHeld = Now - RolledAt;
            FireReleased();
            RolledAt = Now;
            RolledStep = 3;
        }
        else if (RolledStep == 3)
        {
            // What was launched lands first (or the wait runs out): the checker needs its impact or damage, not only the launch.
            int32 Flying = 0;
            for (TActorIterator<AOpenWillowProjectile> It(GetWorld()); It; ++It) ++Flying;
            if (Flying > 0 && Now < RolledAt + 10.f) return;
            const FOpenWillowFiringStats& C = OpenWillowFiring::Counts;
            // The pool on the held gun against the card read apart from it: the only line tying those numbers to the running host.
            float PoolCard[3], PoolBase[3];
            FOpenWillowWeaponGen::RolledPoolValues(Held->Balance, RolledStage, 1u, PoolCard, PoolBase);
            const float ExpectedDamage = RolledPerShot >= 0.f ? (C.Shots - RolledShotsBefore) * FMath::Max(1, Held->Firing.Pellets) * RolledPerShot : -1.f;
            UE_LOG(LogTemp, Display, TEXT("OWFIRING rolled want=%d id=%s type=%s burst=%d from_manifest=%d firing_loaded=%d interval=%.3f hold=%.3f magazine=%.0f shots=%d launches=%d impacts=%d damage=%.2f flying=%d expected_damage=%.2f dummy_died=%d wobble_at_press=%.3f pool_min=%.6f pool_max=%.6f pool_rate=%.6f card_min=%.6f card_max=%.6f card_rate=%.6f base_min=%.6f base_max=%.6f base_rate=%.6f"),
                Wanted, *Held->Id, *Held->Type.Replace(TEXT(" "), TEXT("_")), Held->Firing.BurstCount, Held->Firing.bBurstFromManifest ? 1 : 0,
                Held->Firing.bLoaded ? 1 : 0, 1.f / Held->FireRate, RolledHeld, Held->Magazine, C.Shots - RolledShotsBefore,
                C.Launches - RolledLaunchesBefore, C.ImpactsByWeapon.FindRef(Held->Id) - RolledImpactsBefore,
                C.DamageByWeapon.FindRef(Held->Id) - RolledDamageBefore, Flying, ExpectedDamage, bRolledDummyDied ? 1 : 0, RolledWobbleAtPress,
                float(Held->Firing.Pool.minValue), float(Held->Firing.Pool.maxValue), float(Held->Firing.Pool.idleRate),
                PoolCard[0], PoolCard[1], PoolCard[2], PoolBase[0], PoolBase[1], PoolBase[2]);
            // Past the wait: the row fails on flying above 0, and what is still in the air can't count towards a later row of this gun.
            if (Flying > 0)
                for (TActorIterator<AOpenWillowProjectile> It(GetWorld()); It; ++It) It->Destroy();
            ++RolledGun;
            RolledStep = 0;
        }
        return;
    }
    // After the last gun: a rolled gun's damage stack (tools/check_weapon_firing.py check_rolled_damage). Its terms must rebuild its
    // damage, and AOpenWillowWalker::ShotDamage must use them with no Scale, a running one and an expired one. The Scale lasts
    // 0.01 s, read within this tick, so it cannot reach anything fired after this.
    static bool bRolledDamageChecked = false;
    if (Ids[0] != TEXT("none") && Gun >= Ids.Num() && !bRolledDamageChecked && CombatTarget.IsValid())
    {
        bRolledDamageChecked = true;
        const float TestScale = 1.f;
        FOpenWillowWeaponItem Rolled;
        FString Reason;
        if (!FOpenWillowWeaponGen::FindGunWithDamageStack(FMath::Max(1, Skills->GetLevel()), TestScale, Rolled, Reason))
        {
            UE_LOG(LogTemp, Error, TEXT("OWFIRING rolled_damage unavailable=%s"), *Reason);
        }
        else
        {
            const float Zero = ShotDamage(Rolled, Now);
            AddWeaponDamageScale(TestScale, 0.01f);
            const float Active = ShotDamage(Rolled, Now);
            const float Expired = ShotDamage(Rolled, Now + 0.02f);
            const float Rebuilt = (Rolled.DamageBase + Rolled.DamagePreAdd) * (1.f + Rolled.DamageScaleUp) / (1.f + Rolled.DamageScaleDown) + Rolled.DamagePostAdd;
            const float Expected = (Rolled.DamageBase + Rolled.DamagePreAdd) * (1.f + Rolled.DamageScaleUp + TestScale) / (1.f + Rolled.DamageScaleDown) + Rolled.DamagePostAdd;
            UE_LOG(LogTemp, Display, TEXT("OWFIRING rolled_damage id=%s has_stack=%d skills_empty=%d damage=%.4f rebuilt=%.4f zero=%.4f active=%.4f expired=%.4f expected=%.4f stand_in=%.4f"),
                *Rolled.Id, Rolled.bHasDamageStack ? 1 : 0, OpenWillowSkillEffects::Stack(this, OpenWillowSkillEffects::Attr::WeaponDamage).IsEmpty() ? 1 : 0,
                Rolled.Damage, Rebuilt, Zero, Active, Expired, Expected, Rolled.Damage * (1.f + TestScale));
        }
    }
    if (Ids[0] == TEXT("none") || Gun >= Ids.Num())
    {
        if (FinishedAt < 0.f) FinishedAt = Now;
        if (Now > FinishedAt + 1.f)
            if (APlayerController* PC = Cast<APlayerController>(Controller)) PC->ConsoleCommand(TEXT("quit"));
        return;
    }
    if (!CombatTarget.IsValid() || Now < 5.f) return;   // the stand-in target spawns once Maya has landed (-owcombattest)
    const FString Id = Ids[Gun];
    auto Finish = [this, Now]()
    {
        FireReleased();
        AimReleased();
        bPulling = false;
        FinishAt = Now;
        Step = 4;
    };
    auto ProjectilesInFlight = [this]()
    {
        int32 Count = 0;
        for (TActorIterator<AOpenWillowProjectile> It(GetWorld()); It; ++It) ++Count;
        return Count;
    };
    // A wall in a horizontal direction turned away from the dummy first (no hit within 1.5 m is skipped).
    auto FindWall = [this](FVector& Out)
    {
        const FVector Eye = Camera->GetComponentLocation();
        const float Toward = (CombatTarget->AimPoint() - Eye).Rotation().Yaw;
        for (const float Turn : {180.f, 135.f, 225.f, 90.f, 270.f})
        {
            FHitResult Hit;
            FCollisionQueryParams Query(SCENE_QUERY_STAT(OpenWillowFiringWall), true, this);
            Query.AddIgnoredActor(CombatTarget.Get());
            const FVector Far = Eye + FRotator(0.f, Toward + Turn, 0.f).Vector() * 3000.f;
            if (GetWorld()->LineTraceSingleByChannel(Hit, Eye, Far, ECC_Visibility, Query) && Hit.Distance > 150.f)
            {
                Out = Hit.ImpactPoint;
                return true;
            }
        }
        return false;
    };
    switch (Step)
    {
    case 0:   // equip the gun
    {
        FireReleased();
        AimReleased();
        int32 Index = INDEX_NONE;
        for (int32 I = 0; I < Inventory->Items().Num(); ++I)
            if (Inventory->Items()[I].Id == Id) { Index = I; break; }
        if (Index == INDEX_NONE)
        {
            UE_LOG(LogTemp, Error, TEXT("OWFIRING skipped id=%s reason=not in the backpack"), *Id);
            ++Gun;
            return;
        }
        if (!EquipItem(Index, 0) || !bWeaponOut || !Inventory->ActiveWeapon() || Inventory->ActiveWeapon()->Id != Id)
        {
            UE_LOG(LogTemp, Error, TEXT("OWFIRING skipped id=%s reason=not equipped (level %d, skill level %d)"), *Id,
                Inventory->Items()[Index].Level, Skills->GetLevel());
            ++Gun;
            return;
        }
        const FOpenWillowWeaponItem* Item = Inventory->ActiveWeapon();
        if (!UOpenWillowInventory::LoadWeaponMesh(*Item))
            UE_LOG(LogTemp, Warning, TEXT("OWFIRING warning id=%s no imported mesh: the stand-in is drawn, firing uses the card and firing block"), *Id);
        if (!Item->Firing.bLoaded)
            UE_LOG(LogTemp, Warning, TEXT("OWFIRING warning id=%s no firing block: fired as a trace"), *Id);
        Kind = ShotKindName(Item->Firing);
        Segments.Reset();
        Segments.Add(FSegment{false, 5});
        Segments.Add(FSegment{true, 3});
        if (Item->Firing.bLoaded && Item->Firing.Status.bValid) Segments.Add(FSegment{false, 20});
        if (Item->Firing.ReloadThrow) Segments.Add(FSegment{false, 0, true});
        // Wide bullet: single straight pellets only (no pattern, no own motion: neither air friction nor acceleration, no
        // splash), so the line's miss is certain; not on burst guns, whose later burst shots would land while the check counts.
        if (Item->Firing.bLoaded && Item->Firing.Kind != EOpenWillowShotKind::Rocket && Item->Projectiles <= 1
            && Item->Firing.Pattern.lines.empty() && Item->Firing.AirFriction == 0.f && Item->Firing.Acceleration.IsZero()
            && !Item->Firing.Splash.bValid
            && Item->Firing.BurstCount <= 1)
            Segments.Add(FSegment{false, 0, false, 0, true});
        if (Item->Firing.BurstCount > 1)
        {
            Segments.Add(FSegment{false, 0, false, 1});
            Segments.Add(FSegment{false, 0, false, 2});   // last: the respawn moves Maya
        }
        OpenWillowFiring::Counts.Reset();
        Segment = 0;
        StepAt = Now;
        Step = 1;
        UE_LOG(LogTemp, Display, TEXT("OWFIRING equipped id=%s kind=%s name=%s"), *Id, *Kind, *Item->Name);
        break;
    }
    case 1:   // let the new gun's materials and arms settle
        if (Now < StepAt + Settle) return;
        Step = 2;
        break;
    case 2:   // aim at the dummy or at a wall, then start the segment's pulls
    {
        if (Segment >= Segments.Num())
        {
            Finish();
            break;
        }
        const FSegment Part = Segments[Segment];
        // Each segment starts on a full dummy: the elemental guns' impact explosions roughly double their damage, and a
        // dummy killed mid-segment would cut the segment short. Each gun starts with the dummy at home and at rest (before
        // the refill, a dying dummy respawned there; without it the dummy drifted and the Tediore throw missed it). Done
        // before the aim is read, so the aim point is the reset position.
        if (Segment == 0) CombatTarget->ResetForTest();
        CombatTarget->RestoreHealth();
        FVector Point = FVector::ZeroVector;
        if (Part.bWall)
        {
            if (!FindWall(Point))
            {
                UE_LOG(LogTemp, Warning, TEXT("OWFIRING no wall in reach for id=%s: wall pulls skipped"), *Id);
                ++Segment;
                break;
            }
        }
        else Point = CombatTarget->AimPoint();
        AimAt(Point);
        PullsLeft = Part.Pulls;
        bPulling = false;
        StepAt = Now;   // the camera follows the new aim a frame later: no pull before AimSettle
        Step = Part.bWide ? 8 : Part.bThrow ? 6 : Part.Lifecycle ? 7 : 3;
        break;
    }
    case 8:   // wide bullet: aim beside the dummy (centre line passes Offset from its centre), fire as a line, then as a box
    {
        // The firing mode's TraceExtent is a data value on 7 of 150 modes; here a host-chosen box (half size Half) is put
        // on the held gun for one pull and removed after: this checks the host's sweep, not a game value.
        static int32 Phase = 0, HitsBefore = 0, LineHits = 0;
        static FVector SavedExtent = FVector::ZeroVector;
        static float SavedSpread = 0.f;
        FOpenWillowWeaponItem* Item = Inventory->ActiveWeaponMutable();
        if (!Item) { Finish(); break; }
        const float Offset = CombatTarget->AutoAimRadius() + 12.f, Half = 24.f;
        auto Pull = [&](const FVector& Extent)
        {
            Item->Firing.TraceExtent = Extent;
            Item->MagazineLeft = -1;
            HitsBefore = OpenWillowFiring::Counts.TargetHits;
            ShotsBefore = OpenWillowFiring::Counts.Shots;
            PullAt = Now;
            FirePressed();
        };
        auto Landed = [&]() { return OpenWillowFiring::Counts.Shots > ShotsBefore && ProjectilesInFlight() == 0; };
        if (Phase == 0)
        {
            const FVector Eye = Camera->GetComponentLocation();
            const FVector Toward = (CombatTarget->AimPoint() - Eye).GetSafeNormal2D();
            AimAt(CombatTarget->AimPoint() + FVector::CrossProduct(Toward, FVector::UpVector) * Offset);
            SavedExtent = Item->Firing.TraceExtent;
            SavedSpread = Item->Spread;
            Item->Spread = 0.f;   // the pellet goes exactly where the aim is
            StepAt = Now;
            Phase = 1;
            break;
        }
        if (Phase == 1) { if (Now < StepAt + 0.3f) break; Pull(FVector::ZeroVector); Phase = 2; break; }
        if (Phase == 2)
        {
            if (!Landed() && Now < PullAt + 3.f) break;
            FireReleased();
            LineHits = OpenWillowFiring::Counts.TargetHits - HitsBefore;
            StepAt = Now;
            Phase = 3;
            break;
        }
        if (Phase == 3) { if (Now < StepAt + 1.f / FMath::Max(0.1f, Item->FireRate) + 0.3f) break; Pull(FVector(Half)); Phase = 4; break; }
        if (!Landed() && Now < PullAt + 3.f) break;
        FireReleased();
        UE_LOG(LogTemp, Display, TEXT("OWFIRING wide id=%s offset=%.1f half=%.1f line_hits=%d box_hits=%d"), *Id, Offset, Half,
            LineHits, OpenWillowFiring::Counts.TargetHits - HitsBefore);
        Item->Firing.TraceExtent = SavedExtent;
        Item->Spread = SavedSpread;
        Phase = 0;
        ++Segment;
        Step = 2;
        break;
    }
    case 7:   // burst lifecycle: press, after the burst's first shot release and reload (or respawn); count later shots
    {
        static int32 Phase = 0, ShotsAfter = 0;
        FOpenWillowWeaponItem* Item = Inventory->ActiveWeaponMutable();
        const bool bReload = Segments[Segment].Lifecycle == 1;
        if (!Item) { Finish(); break; }
        if (Phase == 0)
        {
            if (Now < StepAt + 0.2f) break;
            Item->MagazineLeft = -1;
            ShotsBefore = OpenWillowFiring::Counts.Shots;
            PullAt = Now;
            FirePressed();
            Phase = 1;
            break;
        }
        if (Phase == 1)
        {
            if (OpenWillowFiring::Counts.Shots <= ShotsBefore && Now < PullAt + 2.f) break;
            FireReleased();
            if (bReload) ReloadPressed(); else Respawn();
            ShotsAfter = OpenWillowFiring::Counts.Shots;
            PullAt = Now;
            Phase = 2;
            break;
        }
        if ((bReloading || Now < PullAt + 1.f) && Now < PullAt + 8.f) break;
        UE_LOG(LogTemp, Display, TEXT("OWFIRING lifecycle id=%s case=%s first_shot=%d extra_shots=%d"), *Id,
            bReload ? TEXT("reload") : TEXT("respawn"), ShotsAfter > ShotsBefore ? 1 : 0, OpenWillowFiring::Counts.Shots - ShotsAfter);
        Phase = 0;
        ++Segment;
        Step = 2;
        break;
    }
    case 6:   // Tediore throw: 5 rounds left, reload, then the refill once the reload ends
    {
        FOpenWillowWeaponItem* Item = Inventory->ActiveWeaponMutable();
        if (!Item) { Finish(); break; }
        static FOpenWillowFiringStats Before;
        if (!bPulling)
        {
            if (Now < StepAt + 0.2f) break;
            Before = OpenWillowFiring::Counts;
            Item->MagazineLeft = 5;
            ReloadPressed();
            PullAt = Now;
            bPulling = true;
            break;
        }
        if ((bReloading || ProjectilesInFlight() > 0) && Now < PullAt + 8.f) break;
        const FOpenWillowFiringStats& C = OpenWillowFiring::Counts;
        UE_LOG(LogTemp, Display, TEXT("OWFIRING throw id=%s rounds=5 moved=%d magazine=%d size=%d target_hits=%d direct=%.2f splash_hits=%d splash=%.2f"),
            *Id, ReloadAmmoMoved, UOpenWillowInventory::MagazineLeft(*Item), UOpenWillowInventory::MagazineSize(*Item),
            C.TargetHits - Before.TargetHits, C.Direct - Before.Direct, C.SplashHits - Before.SplashHits, C.Splash - Before.Splash);
        bPulling = false;
        FinishAt = Now;
        Step = 5;
        break;
    }
    case 3:   // one pull per shot: press, then release once the shot has happened (or after 2 s)
    {
        FOpenWillowWeaponItem* Item = Inventory->ActiveWeaponMutable();
        if (!Item || Item->Id != Id)
        {
            UE_LOG(LogTemp, Error, TEXT("OWFIRING lost id=%s: the active weapon changed"), *Id);
            Finish();
            break;
        }
        if (bPulling)
        {
            // A press waits out the card's spin-up before its shot (FirePressed), so the wait allows for it.
            const float Wait = 2.f + Item->SpinUp;
            if (OpenWillowFiring::Counts.Shots <= ShotsBefore && Now < PullAt + Wait) break;
            if (OpenWillowFiring::Counts.Shots <= ShotsBefore)
                UE_LOG(LogTemp, Warning, TEXT("OWFIRING pull without a shot id=%s after %.1f s"), *Id, Wait);
            FireReleased();
            if (IsBurstFiring() && Now < PullAt + Wait) break;   // a burst finishes its shots before the next pull
            bPulling = false;
            break;
        }
        if (PullsLeft <= 0)
        {
            FinishAt = Now;   // the segment's projectiles land before its counts are logged
            Step = 5;
            break;
        }
        if (Now < StepAt + 0.2f) break;   // AimSettle: the first shot of a segment fires at the aim, not the old view
        // Refill: this test does not reload; a burst needs its whole count in the magazine.
        if (UOpenWillowInventory::MagazineLeft(*Item) < FMath::Max(2, Item->Firing.BurstCount + 1)) Item->MagazineLeft = -1;
        ShotsBefore = OpenWillowFiring::Counts.Shots;
        PullAt = Now;
        --PullsLeft;
        bPulling = true;
        FirePressed();
        break;
    }
    case 5:   // cumulative counts after a segment's projectiles land (the checker reads segment 0, the dummy pulls)
    {
        if (Now < FinishAt + 0.5f) break;
        if (ProjectilesInFlight() > 0 && Now < FinishAt + 6.f) break;
        UE_LOG(LogTemp, Display, TEXT("OWFIRING segment id=%s index=%d wall=%d shots=%d target_hits=%d"), *Id, Segment,
            Segments[Segment].bWall ? 1 : 0, OpenWillowFiring::Counts.Shots, OpenWillowFiring::Counts.TargetHits);
        if (Segment == 0)
        {
            // How far the dummy slid along the shots since the segment began (momentum / mass, then the friction stand-in).
            const FVector Along = (CombatTarget->AimPoint() - Camera->GetComponentLocation()).GetSafeNormal2D();
            const FOpenWillowWeaponItem* Held = Inventory->ActiveWeapon();
            UE_LOG(LogTemp, Display, TEXT("OWFIRING push id=%s momentum_rule=%.1f momentum_last=%.1f momentum_hits=%d pushes=%d mass=%.1f moved=%.3f"),
                *Id, Held && Held->Firing.bLoaded ? Held->Firing.Momentum : 0.f, OpenWillowFiring::Counts.LastMomentum,
                OpenWillowFiring::Counts.MomentumHits, OpenWillowFiring::Counts.Pushes, CombatTarget->GetMass(),
                float(FVector::DotProduct(CombatTarget->Displacement(), Along)));
        }
        ++Segment;
        Step = 2;
        break;
    }
    case 4:   // let the projectiles (up to 6 s) and status effects (up to 10 s: corrosive lasts 8 s) finish, then report
    {
        if (Now < FinishAt + 0.5f) break;
        const bool bStatus = UOpenWillowStatusEffects::AnyActive(CombatTarget.Get());
        // Weapon swap during the status ticks: the ticks still to come must name the gun that caused the proc, not the one
        // now in hand (the hit event stores the source weapon at the proc). Every status gun whose effect deals damage over
        // time (duration and dps above 0) swaps once; with no effect running it first pulls once more at the dummy (up to
        // 40 extra pulls, logged) until one runs. A status without damage ticks has nothing to attribute: slag (Amp) has no
        // DOT in BL2, and the recipe's slag duration is unresolved, so the host never runs it. It logs a skip instead. Each
        // outcome is logged: the checker requires a swap and ticks after it for ticking statuses only.
        static int32 SwappedGun = -1, ExtraPulls = 0, ExtraFor = -1;
        if (ExtraFor != Gun) { ExtraFor = Gun; ExtraPulls = 0; }
        const FOpenWillowWeaponItem* Held = Inventory->ActiveWeapon();
        if (SwappedGun != Gun && Held && Held->Id == Id && Held->Firing.bLoaded && Held->Firing.Status.bValid
            && !(Held->Firing.Status.Duration > 0.f && Held->Firing.Status.Dps > 0.f))
        {
            SwappedGun = Gun;
            UE_LOG(LogTemp, Display, TEXT("OWFIRING swap id=%s skipped=no_ticks duration=%.2f dps=%.2f"), *Id,
                Held->Firing.Status.Duration, Held->Firing.Status.Dps);
        }
        if (SwappedGun != Gun && Held && Held->Id == Id && Held->Firing.bLoaded && Held->Firing.Status.bValid)
        {
            if (!bStatus && ExtraPulls < 40)
            {
                ++ExtraPulls;
                Segments.Add(FSegment{false, 1});
                Step = 2;
                break;
            }
            SwappedGun = Gun;
            FString To;
            if (bStatus)
                for (int32 I = 0; I < Inventory->Items().Num(); ++I)
                    if (Inventory->Items()[I].Id != Id && EquipItem(I, 0) && Inventory->ActiveWeapon() && Inventory->ActiveWeapon()->Id != Id)
                    {
                        To = Inventory->ActiveWeapon()->Id;
                        break;
                    }
            if (!To.IsEmpty())
            {
                UE_LOG(LogTemp, Display, TEXT("OWFIRING swap id=%s to=%s ticks_before=%d extra_pulls=%d"), *Id, *To,
                    OpenWillowFiring::Counts.Ticks, ExtraPulls);
            }
            else
            {
                UE_LOG(LogTemp, Warning, TEXT("OWFIRING swap id=%s failed=%s ticks_before=%d extra_pulls=%d"), *Id,
                    bStatus ? TEXT("no_other_gun_equipped") : TEXT("no_effect_running"), OpenWillowFiring::Counts.Ticks, ExtraPulls);
            }
            break;   // the next pass waits for the remaining ticks
        }
        if (ProjectilesInFlight() > 0 && Now < FinishAt + 6.f) break;
        if (bStatus && Now < FinishAt + 10.f) break;
        int32 Leftover = 0;
        for (TActorIterator<AOpenWillowProjectile> It(GetWorld()); It; ++It)
        {
            ++Leftover;
            It->Destroy();
        }
        if (Leftover > 0) UE_LOG(LogTemp, Warning, TEXT("OWFIRING id=%s removed %d projectiles still in flight after 6 s"), *Id, Leftover);
        // A status effect that outlives the wait is ended here, so its ticks cannot count toward the next gun.
        if (bStatus)
        {
            UE_LOG(LogTemp, Warning, TEXT("OWFIRING id=%s status effect still on the dummy after 10 s: ended"), *Id);
            UOpenWillowStatusEffects::ClearAll(CombatTarget.Get());
        }
        const FOpenWillowFiringStats& C = OpenWillowFiring::Counts;
        const FOpenWillowWeaponItem* Item = Inventory->ActiveWeapon();
        FString TickWeapons;
        for (const TPair<FString, int32>& Pair : C.TickWeapons)
            TickWeapons += FString::Printf(TEXT("%s%s:%d"), TickWeapons.IsEmpty() ? TEXT("") : TEXT(","),
                Pair.Key.IsEmpty() ? TEXT("-") : *Pair.Key, Pair.Value);
        UE_LOG(LogTemp, Display, TEXT("OWFIRING dummy id=%s health=%.3f"), *Id, CombatTarget->HealthFraction());
        UE_LOG(LogTemp, Display, TEXT("OWFIRING result id=%s kind=%s pellets=%d shots=%d launches=%d impacts=%d target_hits=%d damage=%.2f direct=%.2f splash=%.2f splash_hits=%d status_rolls=%d procs=%d ticks=%d timed=%d on_time=%d hit_events=%d hit_direct=%d hit_splash=%d hit_tick=%d hit_throw=%d tick_weapons=%s unknown_sources=%d"),
            *Id, *Kind, C.Pellets, C.Shots, C.Launches, C.Impacts, C.TargetHits, C.Damage, C.Direct, C.Splash, C.SplashHits,
            C.StatusRolls, C.Procs, C.Ticks, C.TimedImpacts, C.OnTimeImpacts, C.HitEvents, C.HitKinds[0], C.HitKinds[1],
            C.HitKinds[2], C.HitKinds[3], TickWeapons.IsEmpty() ? TEXT("-") : *TickWeapons, C.UnknownSources);
        if ((!Item || Item->Id != Id) && SwappedGun != Gun)
            UE_LOG(LogTemp, Warning, TEXT("OWFIRING id=%s reported after the weapon changed"), *Id);
        ++Gun;
        Step = 0;
        break;
    }
    default:
        break;
    }
}

void AOpenWillowWalker::RunReloadTest(float Now)
{
    // -owreloadtest: the first loaded gun that spends rounds and can equip. Its magazine is set low, the trigger goes down
    // through the same FirePressed a key uses, and the empty magazine starts the reload by itself. An automatic gun stays held;
    // a gun that fires per press gets repeated presses. The magazine is logged during the reload, then one result line compares
    // the magazine and the reserve before and after with the rounds moved. Then the run quits. Test code only.
    static int32 Phase = 0, Start = 0, Cost = 0, Size = 0, ReserveBefore = 0, ExpectedShots = 0, ShotsBefore = 0, ShotsFired = 0;
    static int32 MagazineAtReload = 0, Presses = 0, RisesBeforeRefill = 0, LastMagazine = 0;
    static float At = 0.f, ReloadStartedAt = 0.f, NextToggle = 0.f, NextSample = 0.f;
    static bool bHolding = false;
    static EOpenWillowAmmoType Type = EOpenWillowAmmoType::Grenade;
    static FString Id;
    const auto Finish = [this](const FString& Result)
    {
        Phase = 4;
        FireReleased();
        UE_LOG(LogTemp, Display, TEXT("OWRELOAD result=%s"), *Result);
        if (APlayerController* PC = Cast<APlayerController>(Controller)) PC->ConsoleCommand(TEXT("quit"));
    };
    if (Phase == 4) return;
    const FOpenWillowWeaponItem* Held = Inventory->ActiveWeapon();
    if (Phase == 0)
    {
        if (Now < 5.f) return;
        int32 Candidates = 0;
        for (int32 I = 0; I < Inventory->Items().Num() && Id.IsEmpty(); ++I)
        {
            const FOpenWillowWeaponItem& Item = Inventory->Items()[I];
            const FString ItemId = Item.Id;
            if (UOpenWillowInventory::ShotCostRounds(Item) < 1 || UOpenWillowInventory::MagazineSize(Item) <= UOpenWillowInventory::ShotCostRounds(Item)
                || !UOpenWillowInventory::ResolveAmmoType(Item, Type)) continue;
            ++Candidates;
            if (EquipItem(I, 0) && bWeaponOut && Inventory->ActiveWeapon() && Inventory->ActiveWeapon()->Id == ItemId) Id = ItemId;
        }
        if (Id.IsEmpty())
        {
            Finish(FString::Printf(TEXT("FAIL no gun that spends rounds equipped (%d items, %d candidates: check -owitems and -owlevel)"), Inventory->Items().Num(), Candidates));
            return;
        }
        At = Now + 2.5f;   // the draw
        Phase = 1;
        return;
    }
    if (!Held || Held->Id != Id) { Finish(TEXT("FAIL the gun left the hand")); return; }
    if (Phase == 1)
    {
        if (Now < At) return;
        FOpenWillowWeaponItem* Item = Inventory->ActiveWeaponMutable();
        Cost = UOpenWillowInventory::ShotCostRounds(*Item);
        Size = UOpenWillowInventory::MagazineSize(*Item);
        Item->MagazineLeft = FMath::Min(Cost * 2, Size - 1);
        Start = UOpenWillowInventory::MagazineLeft(*Item);
        ExpectedShots = Start / Cost;
        ReserveBefore = Inventory->AmmoPool(Type).Current;
        if (ReserveBefore <= 0) { Finish(TEXT("FAIL the reserve is empty before firing")); return; }
        UE_LOG(LogTemp, Display, TEXT("OWRELOAD start id=%s name=%s cost=%d size=%d magazine=%d reserve=%d burst=%d expected_shots=%d"), *Id, *Item->Name, Cost, Size, Start,
            ReserveBefore, Item->Firing.BurstCount, ExpectedShots);
        ShotsBefore = OpenWillowFiring::Counts.Shots;
        FirePressed();
        bHolding = true;
        Presses = 1;
        NextToggle = Now + 0.15f;
        At = Now + 30.f;
        Phase = 2;
        return;
    }
    const int32 Magazine = UOpenWillowInventory::MagazineLeft(*Held);
    if (Phase == 2)
    {
        if (bReloading)
        {
            FireReleased();
            bHolding = false;
            ShotsFired = OpenWillowFiring::Counts.Shots - ShotsBefore;
            MagazineAtReload = LastMagazine = Magazine;
            ReloadStartedAt = NextSample = Now;
            At = ReloadEndsAt + 5.f;
            UE_LOG(LogTemp, Display, TEXT("OWRELOAD reload started after shots=%d presses=%d magazine=%d, refill due +%.2f, ends +%.2f"), ShotsFired, Presses, Magazine,
                ReloadRefillAt - Now, ReloadEndsAt - Now);
            Phase = 3;
        }
        else if (Now >= At)
        {
            Finish(FString::Printf(TEXT("FAIL no reload started in 30 s: shots=%d presses=%d magazine=%d"), OpenWillowFiring::Counts.Shots - ShotsBefore, Presses, Magazine));
        }
        else if (Held->Firing.BurstCount >= 1 && Now >= NextToggle)
        {
            // Fires per press: let go and press again, so the press after the last round finds the magazine empty.
            if (bHolding) { FireReleased(); bHolding = false; }
            else { FirePressed(); bHolding = true; ++Presses; }
            NextToggle = Now + 0.15f;
        }
        return;
    }
    // Phase 3: during the reload. The real game's magazine count was reported rising here; this records what the host shows.
    if (bReloading)
    {
        if (!bReloadRefilled && Magazine > LastMagazine) ++RisesBeforeRefill;
        LastMagazine = Magazine;
        if (Now >= NextSample)
        {
            UE_LOG(LogTemp, Display, TEXT("OWRELOAD during t=%.2f magazine=%d refilled=%d"), Now - ReloadStartedAt, Magazine, bReloadRefilled ? 1 : 0);
            NextSample = Now + 0.25f;
        }
        if (Now >= At) Finish(FString::Printf(TEXT("FAIL the reload did not end: magazine=%d"), Magazine));
        return;
    }
    const int32 ReserveAfter = Inventory->AmmoPool(Type).Current;
    const int32 Moved = ReloadAmmoMoved;
    const int32 Expected = FMath::Min(Size, MagazineAtReload + ReserveBefore);
    FString Failed;
    if (ShotsFired != ExpectedShots) Failed += FString::Printf(TEXT(" shots %d, expected %d;"), ShotsFired, ExpectedShots);
    if (MagazineAtReload != Start - ExpectedShots * Cost) Failed += FString::Printf(TEXT(" magazine at the reload %d, expected %d;"), MagazineAtReload, Start - ExpectedShots * Cost);
    if (Moved <= 0) Failed += TEXT(" nothing moved;");
    if (ReserveBefore - ReserveAfter != Moved) Failed += FString::Printf(TEXT(" the reserve fell by %d, moved %d;"), ReserveBefore - ReserveAfter, Moved);
    if (Magazine != MagazineAtReload + Moved) Failed += FString::Printf(TEXT(" the magazine rose by %d, moved %d;"), Magazine - MagazineAtReload, Moved);
    if (Magazine != Expected) Failed += FString::Printf(TEXT(" magazine %d, expected %d;"), Magazine, Expected);
    Finish(FString::Printf(TEXT("%s id=%s cost=%d size=%d shots=%d magazine %d -> %d (fired down to %d) reserve %d -> %d moved=%d took=%.2f rises_before_refill=%d%s"),
        Failed.IsEmpty() ? TEXT("PASS") : TEXT("FAIL"), *Id, Cost, Size, ShotsFired, Start, Magazine, MagazineAtReload, ReserveBefore, ReserveAfter, Moved, Now - ReloadStartedAt,
        RisesBeforeRefill, *Failed));
}
