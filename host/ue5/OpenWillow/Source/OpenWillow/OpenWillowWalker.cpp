#include "OpenWillowWalker.h"
#include "OpenWillowArmsAnimInstance.h"
#include "OpenWillowCombatTarget.h"
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
#include "DrawDebugHelpers.h"
#include "Kismet/GameplayStatics.h"

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
    }
    else UE_LOG(LogTemp, Warning, TEXT("OpenWillow optional pistol visual proxy missing: %s"), VisualPath);
    EquipInfinity();
    if (FParse::Param(FCommandLine::Get(), TEXT("owcombattest")))
    {
        FActorSpawnParameters Params;
        Params.Owner = this;
        const FVector TargetPosition = Camera->GetComponentLocation() + Camera->GetForwardVector() * 650.f;
        GetWorld()->SpawnActor<AOpenWillowCombatTarget>(TargetPosition, FRotator::ZeroRotator, Params);
        UE_LOG(LogTemp, Display, TEXT("OpenWillow combat target spawned at %s"), *TargetPosition.ToString());
    }
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
    if (ArmsAnim && FirePistolAnim) ArmsAnim->PlayAction(FirePistolAnim, 0.35f);
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
    DrawDebugLine(GetWorld(), Start, Impact, FColor(255, 190, 80), false, 0.12f, 0, 1.5f);
    if (bHit && Hit.GetActor())
    {
        UGameplayStatics::ApplyPointDamage(Hit.GetActor(), 10.f, Direction, Hit,
            GetController(), this, UDamageType::StaticClass());
        DrawDebugSphere(GetWorld(), Impact, 8.f, 8, FColor::Yellow, false, 0.15f);
        UE_LOG(LogTemp, Display, TEXT("OpenWillow Infinity hit %s; shot=%d ammo cost=0"),
            *Hit.GetActor()->GetName(), InfinityShot);
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
    if (!GetWorld()->LineTraceSingleByChannel(Hit, Start,
        Start + Camera->GetForwardVector() * 2500.f, ECC_Visibility, Query))
    {
        if (ArmsAnim && PhaselockFailAnim) ArmsAnim->PlayAction(PhaselockFailAnim);
        return;
    }
    AOpenWillowCombatTarget* Target = Cast<AOpenWillowCombatTarget>(Hit.GetActor());
    if (!Target || !Target->BeginPhaselock(Now, 5.5f))
    {
        if (ArmsAnim && PhaselockFailAnim) ArmsAnim->PlayAction(PhaselockFailAnim);
        return;
    }
    if (ArmsAnim && PhaselockAnim) ArmsAnim->PlayAction(PhaselockAnim);
    PhaselockReadyAt = Now + 13.f; // host prototype; duration/cooldown formulas still need evaluation
    UE_LOG(LogTemp, Display, TEXT("OpenWillow Maya Phaselock activated on %s"), *Target->GetName());
}
float AOpenWillowWalker::PhaselockRemaining() const
{
    return GetWorld() ? FMath::Max(0.f, PhaselockReadyAt - GetWorld()->GetTimeSeconds()) : 0.f;
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
void AOpenWillowWalker::Turn(float Value) { AddControllerYawInput(Value); }
void AOpenWillowWalker::Look(float Value) { AddControllerPitchInput(Value); }
