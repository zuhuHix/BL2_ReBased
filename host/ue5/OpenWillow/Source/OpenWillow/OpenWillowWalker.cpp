#include "OpenWillowWalker.h"
#include "OpenWillowArmsAnimInstance.h"
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

namespace
{
const TCHAR* MayaRoot = TEXT("/Game/OpenWillow/Characters/Maya");
UAnimSequence* LoadArmsAnim(const TCHAR* Clip)
{
    const FString Path = FString::Printf(TEXT("%s/FirstPerson/Anim_Unarmed_%s.Anim_Unarmed_%s"), MayaRoot, Clip, Clip);
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
    if (!FParse::Param(FCommandLine::Get(), TEXT("owmaya"))) return;
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
    // The walker has no equipped weapon yet. Use Maya's unarmed set.
    IdleAnim = LoadArmsAnim(TEXT("Idle"));
    RunAnim = LoadArmsAnim(TEXT("Run_F"));
    SprintAnim = LoadArmsAnim(TEXT("Sprint"));
    JumpAnim = LoadArmsAnim(TEXT("Jump_Idle"));
    LandAnim = LoadArmsAnim(TEXT("Jump_End"));
    Arms->SetAnimInstanceClass(UOpenWillowArmsAnimInstance::StaticClass());
    ArmsAnim = Cast<UOpenWillowArmsAnimInstance>(Arms->GetAnimInstance());
    if (ArmsAnim)
    {
        ArmsAnim->SetClips(IdleAnim, RunAnim, SprintAnim, JumpAnim, LandAnim);
        UE_LOG(LogTemp, Display, TEXT("OpenWillow Maya first-person arms active (walk/sprint/jump blends)"));
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
    if (!ArmsAnim) return;
    // BL2's own AnimTree is not reproduced; the native arms instance blends
    // the imported poses according to measured speed and the jump state.
    const bool bFalling = GetCharacterMovement()->IsFalling();
    const float Now = GetWorld()->GetTimeSeconds();
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
