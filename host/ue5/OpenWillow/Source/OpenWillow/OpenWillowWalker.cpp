#include "OpenWillowWalker.h"
#include "Camera/CameraComponent.h"
#include "Components/CapsuleComponent.h"
#include "Components/InputComponent.h"
#include "Components/StaticMeshComponent.h"
#include "Engine/StaticMesh.h"
#include "Misc/CommandLine.h"
#include "Misc/Parse.h"
#include "GameFramework/CharacterMovementComponent.h"

AOpenWillowWalker::AOpenWillowWalker()
{
    GetCapsuleComponent()->InitCapsuleSize(34, 88);
    Camera = CreateDefaultSubobject<UCameraComponent>(TEXT("WalkingCamera"));
    Camera->SetupAttachment(GetCapsuleComponent());
    Camera->SetRelativeLocation(FVector(0, 0, 64));
    Camera->bUsePawnControlRotation = true;
    Arms = CreateDefaultSubobject<UStaticMeshComponent>(TEXT("FirstPersonArms"));
    Arms->SetupAttachment(Camera);
    Arms->SetCollisionEnabled(ECollisionEnabled::NoCollision);
    Arms->SetCastShadow(false);
    GetCharacterMovement()->MaxWalkSpeed = 450;
    GetCharacterMovement()->JumpZVelocity = 420;
    GetCharacterMovement()->MaxStepHeight = 35;
    GetCharacterMovement()->SetWalkableFloorAngle(45);
}

void AOpenWillowWalker::BeginPlay()
{
    Super::BeginPlay();
    FString Path;
    if (!FParse::Value(FCommandLine::Get(), TEXT("owarms="), Path)) return;
    UStaticMesh* ArmsMesh = LoadObject<UStaticMesh>(nullptr, *Path);
    if (!ArmsMesh) { UE_LOG(LogTemp, Warning, TEXT("OpenWillow arms mesh not found: %s"), *Path); return; }
    Arms->SetStaticMesh(ArmsMesh);
    // BL2 keeps vertical FOV (DefaultEngine.ini: AspectRatio_MaintainYFOV) and,
    // as UE3 does, reads its FOV setting as horizontal at 4:3. UE's camera FOV
    // is horizontal at the actual aspect, so convert. -owfov=<BL2 setting>; the
    // 90 default matches a maintainer capture by eye, not a read setting.
    float Bl2Fov = 90;
    FParse::Value(FCommandLine::Get(), TEXT("owfov="), Bl2Fov);
    const float Aspect = Camera->AspectRatio > 0 ? Camera->AspectRatio : 16.f / 9.f;
    Camera->SetFieldOfView(FMath::RadiansToDegrees(2 * FMath::Atan(
        FMath::Tan(FMath::DegreesToRadians(Bl2Fov) / 2) * Aspect / (4.f / 3.f))));
    UE_LOG(LogTemp, Display, TEXT("OpenWillow first-person arms: %s"), *Path);
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
}
void AOpenWillowWalker::Forward(float Value) { AddMovementInput(FRotator(0, GetControlRotation().Yaw, 0).Vector(), Value); }
void AOpenWillowWalker::Right(float Value) { AddMovementInput(FRotationMatrix(FRotator(0, GetControlRotation().Yaw, 0)).GetUnitAxis(EAxis::Y), Value); }
void AOpenWillowWalker::Turn(float Value) { AddControllerYawInput(Value); }
void AOpenWillowWalker::Look(float Value) { AddControllerPitchInput(Value); }
