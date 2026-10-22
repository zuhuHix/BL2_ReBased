#include "OpenWillowShotFx.h"
#include "Components/PointLightComponent.h"
#include "Components/StaticMeshComponent.h"
#include "Engine/StaticMesh.h"
#include "Engine/World.h"
#include "Camera/PlayerCameraManager.h"
#include "GameFramework/PlayerController.h"
#include "Materials/MaterialInstanceDynamic.h"

namespace
{
const TCHAR* FxMaterialPath = TEXT("/Game/OpenWillow/Weapons/InfinityProxy/M_OW_FxAdditive.M_OW_FxAdditive");
}

AOpenWillowShotFx::AOpenWillowShotFx()
{
    PrimaryActorTick.bCanEverTick = true;
    Visual = CreateDefaultSubobject<UStaticMeshComponent>(TEXT("FxVisual"));
    RootComponent = Visual;
    Visual->SetCollisionEnabled(ECollisionEnabled::NoCollision);
    Visual->SetCastShadow(false);
    Light = CreateDefaultSubobject<UPointLightComponent>(TEXT("FxLight"));
    Light->SetupAttachment(Visual);
    Light->SetCastShadows(false);
    Light->SetIntensityUnits(ELightUnits::Candelas);
    Light->SetIntensity(0.f);
}

void AOpenWillowShotFx::Setup(UStaticMesh* Mesh, const FLinearColor& Color, float Intensity,
    float Life, float LightIntensity)
{
    Visual->SetStaticMesh(Mesh);
    if (UMaterialInterface* Base = LoadObject<UMaterialInterface>(nullptr, FxMaterialPath))
    {
        Material = UMaterialInstanceDynamic::Create(Base, this);
        Material->SetVectorParameterValue(TEXT("Color"), Color);
        Material->SetScalarParameterValue(TEXT("Intensity"), Intensity);
        Visual->SetMaterial(0, Material);
    }
    Light->SetLightColor(Color);
    Light->SetIntensity(LightIntensity);
    Light->SetAttenuationRadius(LightIntensity > 0.f ? 220.f : 0.f);
    Born = GetWorld()->GetTimeSeconds();
    Lifetime = Life;
    StartIntensity = Intensity;
    StartLight = LightIntensity;
    SetLifeSpan(Life + 0.05f);
}

void AOpenWillowShotFx::Tick(float DeltaSeconds)
{
    Super::Tick(DeltaSeconds);
    if (bIsCasing)
    {
        // UNVERIFIED host stand-in: gravity and spin keep the brass visible without a physics body per shot.
        CasingVelocity.Z -= 980.f * DeltaSeconds;
        AddActorWorldOffset(CasingVelocity * DeltaSeconds, false);
        AddActorLocalRotation(FRotator(540.f * DeltaSeconds, 1080.f * DeltaSeconds, 720.f * DeltaSeconds));
    }
    const float Fade = 1.f - FMath::Clamp((GetWorld()->GetTimeSeconds() - Born) / Lifetime, 0.f, 1.f);
    if (Material) Material->SetScalarParameterValue(TEXT("Intensity"), StartIntensity * Fade);
    Light->SetIntensity(StartLight * Fade);
}

void AOpenWillowShotFx::Tracer(UWorld* World, const FVector& Start, const FVector& End,
    const FLinearColor& Color, float Radius, float Life)
{
    const FVector Span = End - Start;
    const float Length = Span.Size();
    if (!World || Length < 1.f) return;
    // The engine cylinder is 100 cm tall along Z with a 50 cm radius.
    const FTransform Pose(FRotationMatrix::MakeFromZ(Span / Length).Rotator(),
        Start + Span * 0.5f, FVector(Radius / 50.f, Radius / 50.f, Length / 100.f));
    auto* Fx = World->SpawnActorDeferred<AOpenWillowShotFx>(StaticClass(), Pose);
    Fx->FinishSpawning(Pose);
    Fx->Setup(LoadObject<UStaticMesh>(nullptr, TEXT("/Engine/BasicShapes/Cylinder.Cylinder")),
        Color, 6.f, Life, 0.f);
}

void AOpenWillowShotFx::Flash(UWorld* World, const FVector& Location, const FLinearColor& Color,
    float Radius, float Life, float LightIntensity)
{
    if (!World) return;
    const FTransform Pose(FRotator(FMath::FRandRange(0.f, 360.f), FMath::FRandRange(0.f, 360.f), 0.f),
        Location, FVector(Radius / 50.f));
    auto* Fx = World->SpawnActorDeferred<AOpenWillowShotFx>(StaticClass(), Pose);
    Fx->FinishSpawning(Pose);
    Fx->Setup(LoadObject<UStaticMesh>(nullptr, TEXT("/Engine/BasicShapes/Sphere.Sphere")),
        Color, 8.f, Life, LightIntensity);
}

void AOpenWillowShotFx::MuzzleFlash(UWorld* World, const FVector& Location, float Radius, float Life)
{
    if (!World) return;
    const float SafeRadius = FMath::Max(0.1f, Radius);
    const float SafeLife = FMath::Max(0.01f, Life);
    // Host-made four-point plume. The live type provides the muzzle socket and duration, but not a scalar size or
    // screen shape. The compact dimensions and 0.16 s life are UNVERIFIED fits to the lane E ~0.12 s post-shot frame.
    const float PlumeLife = FMath::Min(SafeLife, 0.16f);
    const float Extent = FMath::Clamp(SafeRadius * 0.38f, 0.75f, 1.35f);
    const float RayLength = Extent * 2.1f;
    const float RayWidth = Extent * 0.34f;
    const UStaticMesh* Cone = LoadObject<UStaticMesh>(nullptr, TEXT("/Engine/BasicShapes/Cone.Cone"));
    FRotator CameraRotation = FRotator::ZeroRotator;
    if (APlayerController* PC = World->GetFirstPlayerController())
        if (PC->PlayerCameraManager) CameraRotation = PC->PlayerCameraManager->GetCameraRotation();
    const FRotationMatrix CameraBasis(CameraRotation);
    const FVector ViewForward = CameraBasis.GetUnitAxis(EAxis::X);
    const FVector Directions[] = {
        CameraBasis.GetUnitAxis(EAxis::Y), -CameraBasis.GetUnitAxis(EAxis::Y),
        CameraBasis.GetUnitAxis(EAxis::Z), -CameraBasis.GetUnitAxis(EAxis::Z),
    };
    for (int32 I = 0; Cone && I < UE_ARRAY_COUNT(Directions); ++I)
    {
        const float Length = RayLength * ((I == 0 || I == 3) ? 1.f : 0.76f);
        const FVector Direction = Directions[I];
        const FQuat Rotation = FQuat::FindBetweenNormals(FVector::UpVector, Direction);
        const FVector RayLocation = Location + Direction * (Length * 0.48f) + ViewForward * 0.25f;
        const FVector Scale(RayWidth / 50.f, RayWidth / 50.f, Length / 100.f);
        const FTransform Pose(Rotation, RayLocation, Scale);
        auto* Fx = World->SpawnActorDeferred<AOpenWillowShotFx>(StaticClass(), Pose);
        Fx->FinishSpawning(Pose);
        Fx->Setup(const_cast<UStaticMesh*>(Cone), FLinearColor(1.f, 0.23f, 0.015f, 1.f), 6.5f, PlumeLife, 0.f);
        Fx->Visual->SetFirstPersonPrimitiveType(EFirstPersonPrimitiveType::FirstPerson);
    }
    const float CoreScale = Extent * 0.38f / 50.f;
    const FTransform CorePose(CameraRotation, Location + ViewForward * 0.5f, FVector(CoreScale));
    auto* Core = World->SpawnActorDeferred<AOpenWillowShotFx>(StaticClass(), CorePose);
    Core->FinishSpawning(CorePose);
    Core->Setup(LoadObject<UStaticMesh>(nullptr, TEXT("/Engine/BasicShapes/Sphere.Sphere")),
        FLinearColor(1.f, 0.86f, 0.24f, 1.f), 8.f, FMath::Min(PlumeLife, 0.12f), 0.f);
    Core->Visual->SetFirstPersonPrimitiveType(EFirstPersonPrimitiveType::FirstPerson);
}

void AOpenWillowShotFx::Casing(UWorld* World, const FVector& Location, const FVector& Velocity, float Scale)
{
    if (!World) return;
    const FVector SafeScale(FMath::Max(0.001f, 0.008f * Scale), FMath::Max(0.001f, 0.008f * Scale),
        FMath::Max(0.001f, 0.012f * Scale));
    // UNVERIFIED host stand-ins: cylinder scale, orientation, subdued brass tint, intensity and lifetime.
    const FTransform Pose(FRotator(90.f, 0.f, 0.f), Location, SafeScale);
    auto* Fx = World->SpawnActorDeferred<AOpenWillowShotFx>(StaticClass(), Pose);
    Fx->FinishSpawning(Pose);
    Fx->Setup(LoadObject<UStaticMesh>(nullptr, TEXT("/Engine/BasicShapes/Cylinder.Cylinder")),
        FLinearColor(0.72f, 0.34f, 0.055f, 1.f), 0.55f, 0.65f, 0.f);
    Fx->Visual->SetFirstPersonPrimitiveType(EFirstPersonPrimitiveType::FirstPerson);
    Fx->bIsCasing = true;
    Fx->CasingVelocity = Velocity;
}
