// The weapons-projectile lane's in-engine harnesses, split from OpenWillowWalker.cpp to keep the fire path readable:
// -owfiringtest (RunFiringTest: fires each -owgunids= gun at the dummy and a wall and logs OWFIRING result lines for
// tools/check_weapon_firing.py) and -owfiringshots (RunFiringShots: frames at the real frames' pose for
// tools/firing_contact_sheet.py). Test code only; the game logic never calls it.
#include "OpenWillowWalker.h"
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
        Step = Part.bThrow ? 6 : Part.Lifecycle ? 7 : 3;
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
        ++Segment;
        Step = 2;
        break;
    }
    case 4:   // let the projectiles (up to 6 s) and status effects (up to 10 s: corrosive lasts 8 s) finish, then report
    {
        if (Now < FinishAt + 0.5f) break;
        const bool bStatus = UOpenWillowStatusEffects::AnyActive(CombatTarget.Get());
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
        UE_LOG(LogTemp, Display, TEXT("OWFIRING dummy id=%s health=%.3f"), *Id, CombatTarget->HealthFraction());
        UE_LOG(LogTemp, Display, TEXT("OWFIRING result id=%s kind=%s pellets=%d shots=%d launches=%d impacts=%d target_hits=%d damage=%.2f direct=%.2f splash=%.2f splash_hits=%d status_rolls=%d procs=%d ticks=%d"),
            *Id, *Kind, C.Pellets, C.Shots, C.Launches, C.Impacts, C.TargetHits, C.Damage, C.Direct, C.Splash, C.SplashHits,
            C.StatusRolls, C.Procs, C.Ticks);
        if (!Item || Item->Id != Id) UE_LOG(LogTemp, Warning, TEXT("OWFIRING id=%s reported after the weapon changed"), *Id);
        ++Gun;
        Step = 0;
        break;
    }
    default:
        break;
    }
}
