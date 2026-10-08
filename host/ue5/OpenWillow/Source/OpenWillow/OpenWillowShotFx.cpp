#include "OpenWillowShotFx.h"
#include "Components/PointLightComponent.h"
#include "Components/StaticMeshComponent.h"
#include "Engine/StaticMesh.h"
#include "Engine/World.h"
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
    Visual->SetMobility(EComponentMobility::Movable); // TracerFlight moves the actor
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
    if (bFlying)
    {
        const float Run = FlightSpeed * (GetWorld()->GetTimeSeconds() - Born);
        const float Head = FMath::Min(Run, FlightDistance);
        const float Tail = FMath::Clamp(Run - FlightLength, 0.f, FlightDistance);
        SetActorLocation(FlightStart + FlightDir * (Head + Tail) * 0.5f);
        SetActorScale3D(FVector(GetActorScale3D().X, GetActorScale3D().Y, FMath::Max(Head - Tail, 0.01f) / 100.f));
        return;
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

void AOpenWillowShotFx::TracerFlight(UWorld* World, const FVector& Start, const FVector& End, float Speed, float Length,
    const FLinearColor& Color, float Radius)
{
    const FVector Span = End - Start;
    const float Distance = Span.Size();
    if (!World || Distance < 1.f || Speed < 1.f) return;
    const FVector Direction = Span / Distance;
    const FTransform Pose(FRotationMatrix::MakeFromZ(Direction).Rotator(), Start, FVector(Radius / 50.f, Radius / 50.f, 0.0001f));
    auto* Fx = World->SpawnActorDeferred<AOpenWillowShotFx>(StaticClass(), Pose);
    Fx->FinishSpawning(Pose);
    // The streak is gone once its tail has reached the end.
    Fx->Setup(LoadObject<UStaticMesh>(nullptr, TEXT("/Engine/BasicShapes/Cylinder.Cylinder")),
        Color, 6.f, (Distance + Length) / Speed, 0.f);
    Fx->bFlying = true;
    Fx->FlightStart = Start;
    Fx->FlightDir = Direction;
    Fx->FlightDistance = Distance;
    Fx->FlightSpeed = Speed;
    Fx->FlightLength = Length;
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
