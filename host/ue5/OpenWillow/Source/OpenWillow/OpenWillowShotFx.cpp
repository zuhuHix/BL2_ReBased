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
const TCHAR* FlameMaterialPath = TEXT("/Game/OpenWillow/Weapons/InfinityProxy/M_OW_MuzzleFlame.M_OW_MuzzleFlame");
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
    float Life, float LightIntensity, bool bSoftFlame, float CoreStrength)
{
    Visual->SetStaticMesh(Mesh);
    UMaterialInterface* Base = LoadObject<UMaterialInterface>(nullptr, bSoftFlame ? FlameMaterialPath : FxMaterialPath);
    if (!Base && bSoftFlame) Base = LoadObject<UMaterialInterface>(nullptr, FxMaterialPath);
    if (Base)
    {
        Material = UMaterialInstanceDynamic::Create(Base, this);
        Material->SetVectorParameterValue(TEXT("Color"), Color);
        Material->SetScalarParameterValue(TEXT("Intensity"), Intensity);
        if (bSoftFlame)
        {
            Material->SetVectorParameterValue(TEXT("CoreColor"), FLinearColor(1.f, 0.68f, 0.12f, 1.f));
            Material->SetScalarParameterValue(TEXT("CoreStrength"), CoreStrength);
        }
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

void AOpenWillowShotFx::MuzzleFlash(UWorld* World, const FVector& Location, float Radius, float Life,
    const FLinearColor& Color)
{
    if (!World) return;
    const float SafeRadius = FMath::Max(0.1f, Radius);
    const float SafeLife = FMath::Max(0.01f, Life);
    // Host-made, soft additive quads. The selected muzzle socket anchors every layer; the mask shape,
    // size, tint and short render life are UNVERIFIED visual fits to the lane E burst references.
    const float PlumeLife = FMath::Min(SafeLife, 0.12f);
    const float Extent = FMath::Clamp(SafeRadius * 1.25f, 4.5f, 6.f);
    UStaticMesh* Plane = LoadObject<UStaticMesh>(nullptr, TEXT("/Engine/BasicShapes/Plane.Plane"));
    if (!Plane) return;
    FRotator CameraRotation = FRotator::ZeroRotator;
    if (APlayerController* PC = World->GetFirstPlayerController())
        if (PC->PlayerCameraManager) CameraRotation = PC->PlayerCameraManager->GetCameraRotation();
    const FRotationMatrix CameraBasis(CameraRotation);
    const FVector ViewForward = CameraBasis.GetUnitAxis(EAxis::X);
    const FQuat Facing = FQuat::FindBetweenNormals(FVector::UpVector, -ViewForward);
    for (int32 I = 0; I < 6; ++I)
    {
        const float Height = Extent * FMath::FRandRange(1.1f, 1.75f);
        const float Width = Extent * FMath::FRandRange(0.55f, 0.95f);
        const float Roll = FMath::FRandRange(0.f, 360.f);
        const FQuat Twist(-ViewForward, FMath::DegreesToRadians(Roll));
        const FTransform Pose(Twist * Facing, Location,
            FVector(Width / 100.f, Height / 100.f, 1.f));
        auto* Fx = World->SpawnActorDeferred<AOpenWillowShotFx>(StaticClass(), Pose);
        Fx->FinishSpawning(Pose);
        Fx->Setup(Plane, Color, 1.3f, PlumeLife, 0.f, true, 0.f);
        Fx->Visual->SetFirstPersonPrimitiveType(EFirstPersonPrimitiveType::FirstPerson);
    }
    const float CoreSize = Extent * 0.44f;
    const FTransform CorePose(Facing, Location, FVector(CoreSize / 100.f, CoreSize / 100.f, 1.f));
    auto* Core = World->SpawnActorDeferred<AOpenWillowShotFx>(StaticClass(), CorePose);
    Core->FinishSpawning(CorePose);
    Core->Setup(Plane, Color, 2.5f, PlumeLife, 90.f, true, 1.f);
    Core->Visual->SetFirstPersonPrimitiveType(EFirstPersonPrimitiveType::FirstPerson);
}

void AOpenWillowShotFx::Casing(UWorld* World, const FVector& Location, const FVector& Velocity, float Scale)
{
    if (!World) return;
    const FVector SafeScale(FMath::Max(0.001f, 0.003f * Scale), FMath::Max(0.001f, 0.003f * Scale),
        FMath::Max(0.001f, 0.012f * Scale));
    // UNVERIFIED host stand-ins: short cylinder, orientation, subdued brass tint, intensity and lifetime.
    const FTransform Pose(FRotator(90.f, 0.f, 0.f), Location, SafeScale);
    auto* Fx = World->SpawnActorDeferred<AOpenWillowShotFx>(StaticClass(), Pose);
    Fx->FinishSpawning(Pose);
    Fx->Setup(LoadObject<UStaticMesh>(nullptr, TEXT("/Engine/BasicShapes/Cylinder.Cylinder")),
        FLinearColor(0.72f, 0.34f, 0.055f, 1.f), 0.55f, 0.22f, 0.f);
    Fx->Visual->SetFirstPersonPrimitiveType(EFirstPersonPrimitiveType::FirstPerson);
    Fx->bIsCasing = true;
    Fx->CasingVelocity = Velocity;
}
