#include "OpenWillowCombatTarget.h"
#include "OpenWillowShotFx.h"
#include "Components/StaticMeshComponent.h"
#include "Engine/DamageEvents.h"
#include "Engine/StaticMesh.h"
#include "Engine/World.h"
#include "Materials/MaterialInstanceDynamic.h"

namespace
{
const FLinearColor BodyColor(0.20f, 0.07f, 0.04f);
const FLinearColor HeadColor(0.55f, 0.42f, 0.30f);
const FLinearColor PhaselockColor(0.55f, 0.18f, 1.f);
constexpr float LiftHeight = 170.f; // host estimate; BL2 lift height not read
constexpr float LiftSeconds = 0.7f; // installed Phaselock lift duration
constexpr float RespawnSeconds = 3.f;

UStaticMeshComponent* Part(AActor* Owner, USceneComponent* Parent, const TCHAR* Name,
    const TCHAR* Mesh, const FVector& Location, const FVector& Scale)
{
    auto* Component = Owner->CreateDefaultSubobject<UStaticMeshComponent>(Name);
    Component->SetupAttachment(Parent);
    Component->SetStaticMesh(LoadObject<UStaticMesh>(nullptr, Mesh));
    Component->SetRelativeLocation(Location);
    Component->SetRelativeScale3D(Scale);
    Component->SetCollisionEnabled(ECollisionEnabled::QueryOnly);
    Component->SetCollisionResponseToAllChannels(ECR_Ignore);
    Component->SetCollisionResponseToChannel(ECC_Visibility, ECR_Block);
    return Component;
}

UMaterialInstanceDynamic* Tint(AActor* Owner, UStaticMeshComponent* Component, const FLinearColor& Color)
{
    UMaterialInterface* Base = LoadObject<UMaterialInterface>(nullptr,
        TEXT("/Engine/BasicShapes/BasicShapeMaterial.BasicShapeMaterial"));
    if (!Base) return nullptr;
    auto* Material = UMaterialInstanceDynamic::Create(Base, Owner);
    Material->SetVectorParameterValue(TEXT("Color"), Color);
    Component->SetMaterial(0, Material);
    return Material;
}
}

AOpenWillowCombatTarget::AOpenWillowCombatTarget()
{
    PrimaryActorTick.bCanEverTick = true;
    Base = CreateDefaultSubobject<USceneComponent>(TEXT("Floor"));
    RootComponent = Base;
    Pivot = CreateDefaultSubobject<USceneComponent>(TEXT("Pivot"));
    Pivot->SetupAttachment(Base);
    // A plain training dummy: engine cylinder is 100 cm tall, sphere 100 cm wide.
    Post = Part(this, Pivot, TEXT("Post"), TEXT("/Engine/BasicShapes/Cylinder.Cylinder"),
        FVector(0, 0, 45), FVector(0.12f, 0.12f, 0.9f));
    Torso = Part(this, Pivot, TEXT("Torso"), TEXT("/Engine/BasicShapes/Cylinder.Cylinder"),
        FVector(0, 0, 125), FVector(0.55f, 0.4f, 0.75f));
    Head = Part(this, Pivot, TEXT("Head"), TEXT("/Engine/BasicShapes/Sphere.Sphere"),
        FVector(0, 0, 182), FVector(0.3f));
    LockSphere = CreateDefaultSubobject<UStaticMeshComponent>(TEXT("PhaselockSphere"));
    LockSphere->SetupAttachment(Pivot);
    LockSphere->SetStaticMesh(LoadObject<UStaticMesh>(nullptr, TEXT("/Engine/BasicShapes/Sphere.Sphere")));
    LockSphere->SetRelativeLocation(FVector(0, 0, 120));
    LockSphere->SetCollisionEnabled(ECollisionEnabled::NoCollision);
    LockSphere->SetCastShadow(false);
    LockSphere->SetHiddenInGame(true);
    // No point light: under Lumen even a dim one pooled violet on the road
    // far from the target, so the shell's emissive carries the glow alone.
}

void AOpenWillowCombatTarget::BeginPlay()
{
    Super::BeginPlay();
    HomeLocation = GetActorLocation();
    BodyMaterial = Tint(this, Torso, BodyColor);
    Tint(this, Post, FLinearColor(0.05f, 0.05f, 0.05f));
    HeadMaterial = Tint(this, Head, HeadColor);
    if (UMaterialInterface* Fx = LoadObject<UMaterialInterface>(nullptr,
        TEXT("/Game/OpenWillow/Weapons/InfinityProxy/M_OW_FxAdditive.M_OW_FxAdditive")))
    {
        LockMaterial = UMaterialInstanceDynamic::Create(Fx, this);
        LockMaterial->SetVectorParameterValue(TEXT("Color"), PhaselockColor);
        LockMaterial->SetScalarParameterValue(TEXT("Rim"), 1.f);
        LockSphere->SetMaterial(0, LockMaterial);
    }
}

FVector AOpenWillowCombatTarget::AimPoint() const
{
    return Torso->GetComponentLocation() + FVector(0, 0, 10);
}

bool AOpenWillowCombatTarget::BeginPhaselock(float Now, float Duration)
{
    if (bPhaselocked || bDead) return false;
    LockStartedAt = Now;
    LockEndsAt = Now + Duration;
    bPhaselocked = true;
    LockSphere->SetHiddenInGame(false);
    return true;
}

void AOpenWillowCombatTarget::Tick(float DeltaSeconds)
{
    Super::Tick(DeltaSeconds);
    const float Now = GetWorld()->GetTimeSeconds();
    Popups.RemoveAll([Now](const FOpenWillowDamagePopup& Popup) { return Now - Popup.Born > 1.2f; });
    if (bDead)
    {
        const float Since = Now - DiedAt;
        Pivot->SetRelativeScale3D(FVector(FMath::Max(0.01f, 1.f - Since / 0.25f)));
        if (Since < RespawnSeconds) return;
        bDead = false;
        Health = MaxHealth;
        SetActorLocation(HomeLocation);
        Pivot->SetRelativeScale3D(FVector(1.f));
        Pivot->SetRelativeLocation(FVector::ZeroVector);
        SetActorHiddenInGame(false);
        SetActorEnableCollision(true);
        return;
    }
    // Damped spring: hits kick the dummy, which settles back upright.
    WobbleVelocity += (-60.f * Wobble - 7.f * WobbleVelocity) * DeltaSeconds;
    Wobble += WobbleVelocity * DeltaSeconds;
    float Height = Pivot->GetRelativeLocation().Z;
    if (bPhaselocked)
    {
        const float Held = Now - LockStartedAt;
        const float Lift = FMath::InterpEaseOut(0.f, LiftHeight, FMath::Clamp(Held / LiftSeconds, 0.f, 1.f), 2.f);
        Height = Lift + 6.f * FMath::Sin(Held * 2.4f);
        Pivot->SetRelativeRotation(FRotator(Wobble.Y + 8.f * FMath::Sin(Held * 1.3f),
            Held * 25.f, Wobble.X + 6.f * FMath::Sin(Held * 1.7f)));
        const float Pulse = 0.85f + 0.15f * FMath::Sin(Held * 7.f);
        const float Grow = FMath::Clamp(Held / 0.25f, 0.f, 1.f);
        LockSphere->SetRelativeScale3D(FVector(2.2f * Grow * Pulse));
        if (LockMaterial) LockMaterial->SetScalarParameterValue(TEXT("Intensity"), 3.f * Pulse);
        FallVelocity = 0.f;
        if (Now >= LockEndsAt)
        {
            bPhaselocked = false;
            LockSphere->SetHiddenInGame(true);
            UE_LOG(LogTemp, Display, TEXT("OpenWillow Phaselock released %s"), *GetName());
        }
    }
    else
    {
        // Released targets drop back under BL2's -500 cm/s^2 world gravity.
        FallVelocity = Height > 0.f ? FallVelocity - 500.f * DeltaSeconds : 0.f;
        Height = FMath::Max(0.f, Height + FallVelocity * DeltaSeconds);
        Pivot->SetRelativeRotation(FRotator(Wobble.Y, Pivot->GetRelativeRotation().Yaw, Wobble.X));
    }
    Pivot->SetRelativeLocation(FVector(0, 0, Height));
    const float Flash = FMath::Clamp(1.f - (Now - LastHitAt) / 0.12f, 0.f, 1.f);
    if (BodyMaterial) BodyMaterial->SetVectorParameterValue(TEXT("Color"),
        FMath::Lerp(BodyColor, FLinearColor(1.f, 0.85f, 0.7f), Flash));
    if (HeadMaterial) HeadMaterial->SetVectorParameterValue(TEXT("Color"),
        FMath::Lerp(HeadColor, FLinearColor(1.f, 0.9f, 0.8f), Flash));
}

float AOpenWillowCombatTarget::TakeDamage(float DamageAmount, const FDamageEvent& DamageEvent,
    AController* EventInstigator, AActor* DamageCauser)
{
    if (bDead) return 0.f;
    const float Now = GetWorld()->GetTimeSeconds();
    const float Applied = FMath::Clamp(DamageAmount, 0.f, Health);
    Health -= Applied;
    LastHitAt = Now;
    FVector Where = AimPoint();
    FVector Direction = FVector::ZeroVector;
    if (DamageEvent.IsOfType(FPointDamageEvent::ClassID))
    {
        const auto& Point = static_cast<const FPointDamageEvent&>(DamageEvent);
        Where = Point.HitInfo.ImpactPoint;
        Direction = Point.ShotDirection;
    }
    const FVector Local = GetActorTransform().InverseTransformVectorNoScale(Direction);
    WobbleVelocity += FVector2D(Local.Y, -Local.X) * 220.f;
    Popups.Add({Where + FVector(FMath::FRandRange(-12.f, 12.f), FMath::FRandRange(-12.f, 12.f), 10.f), Applied, Now});
    UE_LOG(LogTemp, Display, TEXT("OpenWillow combat target hit for %.0f, health %.0f/%.0f"),
        Applied, Health, MaxHealth);
    if (Health <= 0.f)
    {
        bDead = true;
        bPhaselocked = false;
        DiedAt = Now;
        LockSphere->SetHiddenInGame(true);
        SetActorEnableCollision(false);
        AOpenWillowShotFx::Flash(GetWorld(), AimPoint(), FLinearColor(1.f, 0.55f, 0.2f), 70.f, 0.25f, 4000.f);
        UE_LOG(LogTemp, Display, TEXT("OpenWillow combat target destroyed; respawns in %.0fs"), RespawnSeconds);
    }
    return Applied;
}
