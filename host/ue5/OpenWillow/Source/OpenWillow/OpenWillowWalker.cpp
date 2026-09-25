#include "OpenWillowWalker.h"
#include "OpenWillowArmsAnimInstance.h"
#include "OpenWillowCombatTarget.h"
#include "OpenWillowShotFx.h"
#include "Animation/AnimSequence.h"
#include "Camera/CameraComponent.h"
#include "Components/CapsuleComponent.h"
#include "Components/InputComponent.h"
#include "Components/SkeletalMeshComponent.h"
#include "Engine/Engine.h"
#include "Engine/GameViewportClient.h"
#include "Engine/SkeletalMesh.h"
#include "Misc/CommandLine.h"
#include "Misc/Parse.h"
#include "GameFramework/CharacterMovementComponent.h"
#include "Kismet/GameplayStatics.h"
#include "Materials/MaterialInterface.h"
#include "GameFramework/PlayerController.h"
#include "UnrealClient.h"

namespace
{
const TCHAR* MayaRoot = TEXT("/Game/OpenWillow/Characters/Maya");
UAnimSequence* LoadArmsAnim(const TCHAR* Set, const TCHAR* Clip)
{
    const FString Path = FString::Printf(TEXT("%s/FirstPerson/Anim_%s_%s.Anim_%s_%s"),
        MayaRoot, Set, Clip, Set, Clip);
    UAnimSequence* Anim = LoadObject<UAnimSequence>(nullptr, *Path);
    if (!Anim) UE_LOG(LogTemp, Warning, TEXT("OpenWillow arms animation not found: %s"), *Path);
    return Anim;
}
}

AOpenWillowWalker::AOpenWillowWalker()
{
    PrimaryActorTick.bCanEverTick = true;
    GetCapsuleComponent()->InitCapsuleSize(34, 88);
    Camera = CreateDefaultSubobject<UCameraComponent>(TEXT("WalkingCamera"));
    Camera->SetupAttachment(GetCapsuleComponent());
    Camera->SetRelativeLocation(FVector(0, 0, 64));
    Camera->bUsePawnControlRotation = true;
    Arms = CreateDefaultSubobject<USkeletalMeshComponent>(TEXT("FirstPersonArms"));
    Arms->SetupAttachment(Camera);
    Arms->SetCollisionEnabled(ECollisionEnabled::NoCollision);
    Arms->SetCastShadow(false);
    // The animation root moves the mesh far from its bind-pose bounds.
    Arms->SetBoundsScale(4);
    WeaponVisual = CreateDefaultSubobject<USkeletalMeshComponent>(TEXT("InfinityVisualProxy"));
    WeaponVisual->SetupAttachment(Arms, TEXT("R_Weapon_Bone"));
    WeaponVisual->SetCollisionEnabled(ECollisionEnabled::NoCollision);
    WeaponVisual->SetCastShadow(false);
    WeaponVisual->SetHiddenInGame(true);
    GetCharacterMovement()->MaxWalkSpeed = 450;
    GetCharacterMovement()->JumpZVelocity = 420;
    GetCharacterMovement()->MaxStepHeight = 35;
    GetCharacterMovement()->SetWalkableFloorAngle(45);
}

void AOpenWillowWalker::BeginPlay()
{
    Super::BeginPlay();
    // BL2's installed DefaultGame.ini sets Engine.WorldInfo.DefaultGravityZ
    // to -500 cm/s^2. Scale the host world's gravity to that magnitude rather
    // than inheriting UE5's default -980, so the jump arc uses BL2 gravity.
    const float HostGravity = GetWorld() ? FMath::Abs(GetWorld()->GetGravityZ()) : 0.f;
    if (HostGravity > KINDA_SMALL_NUMBER)
    {
        GetCharacterMovement()->GravityScale = 500.f / HostGravity;
    }
    // JumpZVelocity=420 is still an estimate: at -500 cm/s^2 it predicts a
    // 176 cm apex and 1.68 s ideal flight, pending a measured BL2 jump.
    bMayaActive = FParse::Param(FCommandLine::Get(), TEXT("owmaya"));
    if (!bMayaActive) return;
    // GD_Siren_Streaming.Pawn_Siren: CylinderComponent CollisionRadius 42,
    // CollisionHeight 80 (UE3 half-height) and BaseEyeHeight 70 above the
    // pawn centre, so a 150 cm standing eye. Its serialized EyeHeight is 77;
    // using BaseEyeHeight (UE3's standing target) is an assumption.
    GetCapsuleComponent()->SetCapsuleSize(42, 80);
    Camera->SetRelativeLocation(FVector(0, 0, 70));
    // BL2 keeps vertical FOV (DefaultEngine.ini: AspectRatio_MaintainYFOV) and,
    // as UE3 does, reads its FOV setting as horizontal at 4:3. UE's camera FOV
    // is horizontal at the actual aspect, so convert. -owfov=<BL2 setting>; the
    // 90 default matches a maintainer capture by eye, not a read setting.
    float Bl2Fov = 90;
    FParse::Value(FCommandLine::Get(), TEXT("owfov="), Bl2Fov);
    float Aspect = 16.f / 9.f;
    if (GEngine && GEngine->GameViewport)
    {
        FVector2D ViewportSize = FVector2D::ZeroVector;
        GEngine->GameViewport->GetViewportSize(ViewportSize);
        if (ViewportSize.Y > KINDA_SMALL_NUMBER)
            Aspect = ViewportSize.X / ViewportSize.Y;
    }
    Camera->SetFieldOfView(FMath::RadiansToDegrees(2 * FMath::Atan(
        FMath::Tan(FMath::DegreesToRadians(Bl2Fov) / 2) * Aspect / (4.f / 3.f))));
    const FString MeshPath = FString::Printf(TEXT("%s/Meshes/Hands_Siren/SkeletalMeshes/Hands_Siren.Hands_Siren"), MayaRoot);
    USkeletalMesh* ArmsMesh = LoadObject<USkeletalMesh>(nullptr, *MeshPath);
    if (!ArmsMesh) { UE_LOG(LogTemp, Warning, TEXT("OpenWillow arms mesh not found: %s"), *MeshPath); return; }
    Arms->SetSkeletalMesh(ArmsMesh);
    Arms->SetAnimInstanceClass(UOpenWillowArmsAnimInstance::StaticClass());
    ArmsAnim = Cast<UOpenWillowArmsAnimInstance>(Arms->GetAnimInstance());
    DrawPistolAnim = LoadArmsAnim(TEXT("PistolCombat"), TEXT("Draw"));
    FirePistolAnim = LoadArmsAnim(TEXT("PistolCombat"), TEXT("ADD_Fire_Recoil"));
    PhaselockAnim = LoadArmsAnim(TEXT("SirenCombat"), TEXT("Phase_Lock_Lift"));
    PhaselockFailAnim = LoadArmsAnim(TEXT("SirenCombat"), TEXT("Phase_Lock_Fail"));
    // The local UModel export is a shared pistol gestalt, not a verified
    // Infinity part assembly. Keep that distinction in the asset path/log.
    const TCHAR* VisualPath = TEXT("/Game/OpenWillow/Weapons/InfinityProxy/SK_InfinityProxy.SK_InfinityProxy");
    if (USkeletalMesh* PistolMesh = LoadObject<USkeletalMesh>(nullptr, VisualPath))
    {
        WeaponVisual->SetSkeletalMesh(PistolMesh);
        // UE3 gestalt guns point along -Y; the hand's weapon bone expects X.
        // The yaw is an observed fit (see the barrel-axis log), not read data.
        float WeaponYaw = 90.f;
        FParse::Value(FCommandLine::Get(), TEXT("owweaponyaw="), WeaponYaw);
        WeaponVisual->SetRelativeRotation(FRotator(0, WeaponYaw, 0));
    }
    else UE_LOG(LogTemp, Warning, TEXT("OpenWillow optional pistol visual proxy missing: %s"), VisualPath);
    EquipInfinity();
    // -owcombattest: spawn the stand-in target once Maya has landed, so it
    // stands on the ground in front of her rather than at the drop height.
    bWantsCombatTarget = FParse::Param(FCommandLine::Get(), TEXT("owcombattest"));
}

void AOpenWillowWalker::Tick(float DeltaSeconds)
{
    Super::Tick(DeltaSeconds);
    if (!bSpawnProbeLogged && FParse::Param(FCommandLine::Get(), TEXT("owspawnprobe"))
        && GetWorld()->GetTimeSeconds() > 5)
    {
        bSpawnProbeLogged = true;
        const auto* Floor = GetCharacterMovement()->CurrentFloor.HitResult.GetActor();
        const FString Source = Floor && Floor->Tags.Num() ? Floor->Tags[0].ToString() : TEXT("none");
        UE_LOG(LogTemp, Display, TEXT("OpenWillow spawn probe grounded=%d pawn=%s floor=%s"),
            GetCharacterMovement()->IsMovingOnGround(), *GetActorLocation().ToString(), *Source);
    }
    // -owautowalk: walk a slow circle unattended so captures can check the run
    // clip without running into a wall.
    static const bool bAutoWalk = FParse::Param(FCommandLine::Get(), TEXT("owautowalk"));
    if (bAutoWalk && Controller)
    {
        Controller->SetControlRotation(Controller->GetControlRotation() + FRotator(0, 60 * DeltaSeconds, 0));
        AddMovementInput(FRotator(0, GetControlRotation().Yaw, 0).Vector(), 1);
    }
    const float Now = GetWorld()->GetTimeSeconds();
    if (bWantsCombatTarget && !CombatTarget.IsValid() && Now > 1.f
        && GetCharacterMovement()->IsMovingOnGround())
    {
        bWantsCombatTarget = false;
        SpawnCombatTarget();
    }
    if (bMayaActive)
    {
        // Arms lag slightly behind fast look input and settle back.
        Sway = FMath::Vector2DInterpTo(Sway, FVector2D(
            FMath::Clamp(-LookInput.X * 1.2f, -4.f, 4.f), FMath::Clamp(LookInput.Y * 1.2f, -3.f, 3.f)),
            DeltaSeconds, 8.f);
        Arms->SetRelativeRotation(FRotator(Sway.Y, Sway.X, 0));
        LookInput = FVector2D::ZeroVector;
    }
    if (bMayaActive && Now < PhaselockBeamUntil && CombatTarget.IsValid()
        && Arms->GetBoneIndex(TEXT("L_Hand")) != INDEX_NONE)
    {
        const float Fade = FMath::Clamp((PhaselockBeamUntil - Now) / 0.3f, 0.f, 1.f);
        AOpenWillowShotFx::Tracer(GetWorld(), Arms->GetBoneLocation(TEXT("L_Hand")) + Camera->GetForwardVector() * 8.f,
            CombatTarget->AimPoint(), FLinearColor(0.6f, 0.2f, 1.f) * Fade, 0.9f, 0.02f);
    }
    if (bMayaActive && !bBarrelAxisLogged && Now > 6.f && WeaponVisual->GetSkinnedAsset())
    {
        bBarrelAxisLogged = true;
        const FVector Axis = (WeaponVisual->GetBoneLocation(TEXT("Barrel"))
            - WeaponVisual->GetBoneLocation(TEXT("WeaponOffset"))).GetSafeNormal();
        UE_LOG(LogTemp, Display, TEXT("OpenWillow barrel axis in view: forward=%.2f right=%.2f up=%.2f"),
            FVector::DotProduct(Axis, Camera->GetForwardVector()), FVector::DotProduct(Axis, Camera->GetRightVector()),
            FVector::DotProduct(Axis, Camera->GetUpVector()));
    }
    // -owcombatshots: an unattended equip/fire/Phaselock sequence with
    // numbered captures, so presentation reviews use repeatable views.
    static const bool bCombatShots = FParse::Param(FCommandLine::Get(), TEXT("owcombatshots"));
    if (bCombatShots && bMayaActive && Controller) RunCombatShots(Now);
    if (bMayaActive && bInfinityEquipped && bFireHeld && Now >= NextShotAt)
    {
        FireInfinity();
        NextShotAt = Now + 0.1f; // local 10 Hz prototype; final rate needs BL2 attribute evaluation
    }
    if (!ArmsAnim) return;
    // BL2's own AnimTree is not reproduced; the native arms instance blends
    // the imported poses according to measured speed and the jump state.
    const bool bFalling = GetCharacterMovement()->IsFalling();
    if (bWasFalling && !bFalling && LandAnim)
        LandUntil = Now + LandAnim->GetPlayLength();
    bWasFalling = bFalling;
    ArmsAnim->SetMovement(GetVelocity().Size2D(), bFalling, Now < LandUntil);
}

void AOpenWillowWalker::SetupPlayerInputComponent(UInputComponent* Input)
{
    Super::SetupPlayerInputComponent(Input);
    Input->BindAxis(TEXT("OWForward"), this, &AOpenWillowWalker::Forward);
    Input->BindAxis(TEXT("OWRight"), this, &AOpenWillowWalker::Right);
    Input->BindAxis(TEXT("OWTurn"), this, &AOpenWillowWalker::Turn);
    Input->BindAxis(TEXT("OWLook"), this, &AOpenWillowWalker::Look);
    Input->BindAction(TEXT("OWJump"), IE_Pressed, this, &ACharacter::Jump);
    Input->BindAction(TEXT("OWJump"), IE_Released, this, &ACharacter::StopJumping);
    Input->BindAction(TEXT("OWSprint"), IE_Pressed, this, &AOpenWillowWalker::SprintPressed);
    Input->BindAction(TEXT("OWSprint"), IE_Released, this, &AOpenWillowWalker::SprintReleased);
    Input->BindAction(TEXT("OWEquipInfinity"), IE_Pressed, this, &AOpenWillowWalker::EquipInfinity);
    Input->BindAction(TEXT("OWHolsterInfinity"), IE_Pressed, this, &AOpenWillowWalker::HolsterInfinity);
    Input->BindAction(TEXT("OWFire"), IE_Pressed, this, &AOpenWillowWalker::FirePressed);
    Input->BindAction(TEXT("OWFire"), IE_Released, this, &AOpenWillowWalker::FireReleased);
    Input->BindAction(TEXT("OWPhaselock"), IE_Pressed, this, &AOpenWillowWalker::UsePhaselock);
}
void AOpenWillowWalker::EquipInfinity()
{
    if (!bMayaActive || !ArmsAnim) return;
    bInfinityEquipped = true;
    WeaponVisual->SetHiddenInGame(false);
    IdleAnim = LoadArmsAnim(TEXT("Pistol"), TEXT("Idle"));
    RunAnim = LoadArmsAnim(TEXT("Pistol"), TEXT("Run_F"));
    SprintAnim = LoadArmsAnim(TEXT("Pistol"), TEXT("Sprint"));
    JumpAnim = LoadArmsAnim(TEXT("Pistol"), TEXT("Jump_Idle"));
    LandAnim = LoadArmsAnim(TEXT("Pistol"), TEXT("Jump_End"));
    ArmsAnim->SetClips(IdleAnim, RunAnim, SprintAnim, JumpAnim, LandAnim);
    if (DrawPistolAnim) ArmsAnim->PlayAction(DrawPistolAnim);
    UE_LOG(LogTemp, Display, TEXT("OpenWillow Maya equipped Infinity (pistol visual is an unverified shared-gestalt proxy)"));
}
void AOpenWillowWalker::HolsterInfinity()
{
    if (!bMayaActive || !ArmsAnim) return;
    bInfinityEquipped = false;
    bFireHeld = false;
    WeaponVisual->SetHiddenInGame(true);
    IdleAnim = LoadArmsAnim(TEXT("Unarmed"), TEXT("Idle"));
    RunAnim = LoadArmsAnim(TEXT("Unarmed"), TEXT("Run_F"));
    SprintAnim = LoadArmsAnim(TEXT("Unarmed"), TEXT("Sprint"));
    JumpAnim = LoadArmsAnim(TEXT("Unarmed"), TEXT("Jump_Idle"));
    LandAnim = LoadArmsAnim(TEXT("Unarmed"), TEXT("Jump_End"));
    ArmsAnim->SetClips(IdleAnim, RunAnim, SprintAnim, JumpAnim, LandAnim);
    UE_LOG(LogTemp, Display, TEXT("OpenWillow Maya holstered Infinity"));
}
void AOpenWillowWalker::FirePressed()
{
    if (!bMayaActive || !bInfinityEquipped) return;
    bFireHeld = true;
    // The installed Infinity barrel has spinning enabled and a 0.8 second
    // spin-up formula. Its other attribute modifiers are not yet evaluated.
    NextShotAt = GetWorld()->GetTimeSeconds() + 0.8f;
}
void AOpenWillowWalker::FireReleased() { bFireHeld = false; }
void AOpenWillowWalker::FireInfinity()
{
    // ADD_Fire_Recoil is a UE3 additive clip; layer it on the held pose.
    if (ArmsAnim && FirePistolAnim) ArmsAnim->PlayAdditive(FirePistolAnim, 0.45f);
    // A deterministic figure-eight is a visual proxy for the installed
    // FiringPatternLines array, whose serialized elements remain unsupported.
    const float Phase = InfinityShot++ * (PI / 8.f);
    const float Spread = FMath::Tan(FMath::DegreesToRadians(2.1f));
    const FVector ForwardAim = Camera->GetForwardVector();
    const FVector Direction = (ForwardAim + Spread * (FMath::Sin(Phase) * Camera->GetRightVector()
        + 0.5f * FMath::Sin(2.f * Phase) * Camera->GetUpVector())).GetSafeNormal();
    const FVector Start = Camera->GetComponentLocation();
    const FVector End = Start + Direction * 10000.f;
    FHitResult Hit;
    FCollisionQueryParams Query(SCENE_QUERY_STAT(OpenWillowInfinity), true, this);
    const bool bHit = GetWorld()->LineTraceSingleByChannel(Hit, Start, End, ECC_Visibility, Query);
    const FVector Impact = bHit ? Hit.ImpactPoint : End;
    const FVector Muzzle = MuzzleLocation();
    AOpenWillowShotFx::Flash(GetWorld(), Muzzle, FLinearColor(1.f, 0.5f, 0.15f), 3.5f, 0.05f, 40.f);
    AOpenWillowShotFx::Tracer(GetWorld(), Muzzle + (Impact - Muzzle).GetSafeNormal() * 30.f, Impact);
    if (!bHit) return;
    AOpenWillowShotFx::Flash(GetWorld(), Impact + Hit.ImpactNormal * 2.f,
        FLinearColor(1.f, 0.75f, 0.35f), 5.f, 0.3f, 60.f);
    AOpenWillowCombatTarget* Target = Cast<AOpenWillowCombatTarget>(Hit.GetActor());
    if (!Target)
    {
        if (UMaterialInterface* Hole = LoadObject<UMaterialInterface>(nullptr,
            TEXT("/Game/OpenWillow/Weapons/InfinityProxy/M_OW_BulletHole.M_OW_BulletHole")))
            UGameplayStatics::SpawnDecalAtLocation(GetWorld(), Hole, FVector(6.f, 7.f, 7.f), Impact,
                (-Hit.ImpactNormal).Rotation(), 12.f);
        return;
    }
    {
        // Placeholder per-shot damage; BL2 damage needs level/attribute evaluation.
        UGameplayStatics::ApplyPointDamage(Target, 87.f, Direction, Hit,
            GetController(), this, UDamageType::StaticClass());
        TargetHitAt = GetWorld()->GetTimeSeconds();
    }
}
void AOpenWillowWalker::UsePhaselock()
{
    if (!bMayaActive) return;
    const float Now = GetWorld()->GetTimeSeconds();
    if (Now < PhaselockReadyAt) return;
    const FVector Start = Camera->GetComponentLocation();
    FHitResult Hit;
    FCollisionQueryParams Query(SCENE_QUERY_STAT(OpenWillowPhaselock), true, this);
    // A small sphere sweep gives the aim assist a skill cast needs.
    if (!GetWorld()->SweepSingleByChannel(Hit, Start, Start + Camera->GetForwardVector() * 2500.f,
        FQuat::Identity, ECC_Visibility, FCollisionShape::MakeSphere(30.f), Query))
    {
        if (ArmsAnim && PhaselockFailAnim) ArmsAnim->PlayAction(PhaselockFailAnim);
        return;
    }
    AOpenWillowCombatTarget* Target = Cast<AOpenWillowCombatTarget>(Hit.GetActor());
    // ActionSkill_Phaselock.LockDurationFormula -> Att_Phaselock_Duration
    // base 5 s, times PhaselockTimeScale (default 1). Skill mods not applied.
    if (!Target || !Target->BeginPhaselock(Now, 5.f))
    {
        if (ArmsAnim && PhaselockFailAnim) ArmsAnim->PlayAction(PhaselockFailAnim);
        return;
    }
    if (ArmsAnim && PhaselockAnim) ArmsAnim->PlayAction(PhaselockAnim);
    // Host cast cue: Tick draws a violet beam from Maya's raised left hand to
    // the target for the lift, so it follows the Phase_Lock_Lift pose.
    PhaselockBeamUntil = Now + 0.9f;
    PhaselockReadyAt = Now + PhaselockCooldownSeconds;
    UE_LOG(LogTemp, Display, TEXT("OpenWillow Maya Phaselock activated on %s"), *Target->GetName());
}
float AOpenWillowWalker::PhaselockRemaining() const
{
    return GetWorld() ? FMath::Max(0.f, PhaselockReadyAt - GetWorld()->GetTimeSeconds()) : 0.f;
}
void AOpenWillowWalker::RunCombatShots(float Now)
{
    auto Shot = [](const TCHAR* Name)
    {
        FScreenshotRequest::RequestScreenshot(FString::Printf(TEXT("OWCombat_%s.png"), Name), false, false);
        UE_LOG(LogTemp, Display, TEXT("OpenWillow combat capture %s"), Name);
    };
    auto Turn = [this](float Yaw, float Pitch)
    {
        Controller->SetControlRotation(Controller->GetControlRotation() + FRotator(Pitch, Yaw, 0));
    };
    const FVector TargetPoint = CombatTarget.IsValid() ? CombatTarget->AimPoint() : FVector::ZeroVector;
    // Each step runs once, in order, at its world time in seconds.
    switch (CombatShotStep)
    {
    case 0: if (Now < 7.f || !CombatTarget.IsValid()) return; AimAt(TargetPoint); break;
    case 1: if (Now < 8.f) return; Shot(TEXT("1_Idle")); break;
    case 2: if (Now < 8.5f) return; FirePressed(); break;
    case 3: if (Now < 9.55f) return; Shot(TEXT("2_Firing")); break;
    case 4: if (Now < 9.8f) return; FireReleased(); break;
    case 5: if (Now < 10.5f) return; AimAt(TargetPoint); UsePhaselock(); break;
    case 6: if (Now < 11.1f) return; Shot(TEXT("3_PhaselockCast")); break;
    case 7: if (Now < 12.2f) return; AimAt(TargetPoint); FirePressed(); break;
    case 8: if (Now < 13.3f) return; Shot(TEXT("4_FiringLocked")); break;
    case 9: if (Now < 13.5f) return; FireReleased(); Turn(35.f, 0.f); break;
    case 10: if (Now < 14.f) return; FirePressed(); break;
    case 11: if (Now < 15.1f) return; Shot(TEXT("5_FiringWall")); break;
    case 12: if (Now < 15.3f) return; FireReleased(); break;
    case 13: if (Now < 16.3f) return;
        if (APlayerController* PC = Cast<APlayerController>(Controller)) PC->ConsoleCommand(TEXT("quit"));
        break;
    default: return;
    }
    ++CombatShotStep;
}
void AOpenWillowWalker::SpawnCombatTarget()
{
    // Stand the dummy on whatever floor is 650 cm ahead of the pawn.
    const FVector Ahead = GetActorLocation() + FRotator(0, GetControlRotation().Yaw, 0).Vector() * 650.f;
    FHitResult Floor;
    FCollisionQueryParams Query(SCENE_QUERY_STAT(OpenWillowTargetFloor), true, this);
    const float FeetZ = GetActorLocation().Z - GetCapsuleComponent()->GetScaledCapsuleHalfHeight();
    FVector Location(Ahead.X, Ahead.Y, FeetZ);
    if (GetWorld()->LineTraceSingleByChannel(Floor, Ahead + FVector(0, 0, 300), Ahead - FVector(0, 0, 600),
        ECC_Visibility, Query)) Location = Floor.ImpactPoint;
    FActorSpawnParameters Params;
    Params.Owner = this;
    const FRotator Facing(0, GetControlRotation().Yaw + 180.f, 0);
    CombatTarget = GetWorld()->SpawnActor<AOpenWillowCombatTarget>(Location, Facing, Params);
    UE_LOG(LogTemp, Display, TEXT("OpenWillow combat target spawned at %s"), *Location.ToString());
}
void AOpenWillowWalker::AimAt(const FVector& Point)
{
    if (Controller) Controller->SetControlRotation((Point - Camera->GetComponentLocation()).Rotation());
}
FVector AOpenWillowWalker::MuzzleLocation() const
{
    // Pistol_Barrel_Vladof's gestalt bounds end ~27 cm ahead of its Barrel
    // bone. Projecting along the view approximates the unhosted Muzzle socket.
    if (WeaponVisual->GetSkinnedAsset() && WeaponVisual->GetBoneIndex(TEXT("Barrel")) != INDEX_NONE)
        return WeaponVisual->GetBoneLocation(TEXT("Barrel")) + Camera->GetForwardVector() * 27.f;
    return Camera->GetComponentLocation() + Camera->GetForwardVector() * 60.f
        + Camera->GetRightVector() * 12.f - Camera->GetUpVector() * 10.f;
}
void AOpenWillowWalker::SprintPressed()
{
    bSprintHeld = true;
    GetCharacterMovement()->MaxWalkSpeed = 650.f; // estimated pending BL2 timing measurement
    UE_LOG(LogTemp, Display, TEXT("OpenWillow sprint on, max speed 650 cm/s"));
}
void AOpenWillowWalker::SprintReleased()
{
    bSprintHeld = false;
    GetCharacterMovement()->MaxWalkSpeed = 450.f;
    UE_LOG(LogTemp, Display, TEXT("OpenWillow sprint off, max speed 450 cm/s"));
}
void AOpenWillowWalker::Forward(float Value) { AddMovementInput(FRotator(0, GetControlRotation().Yaw, 0).Vector(), Value); }
void AOpenWillowWalker::Right(float Value) { AddMovementInput(FRotationMatrix(FRotator(0, GetControlRotation().Yaw, 0)).GetUnitAxis(EAxis::Y), Value); }
void AOpenWillowWalker::Turn(float Value) { LookInput.X += Value; AddControllerYawInput(Value); }
void AOpenWillowWalker::Look(float Value) { LookInput.Y += Value; AddControllerPitchInput(Value); }
