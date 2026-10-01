#include "OpenWillowCombatTarget.h"
#include "OpenWillowShotFx.h"
#include "OpenWillowQuest.h"
#include "OpenWillowWalker.h"
#include "Animation/AnimSequence.h"
#include "Components/CapsuleComponent.h"
#include "Components/SkeletalMeshComponent.h"
#include "Components/StaticMeshComponent.h"
#include "Engine/SkeletalMesh.h"
#include "Engine/DamageEvents.h"
#include "Engine/StaticMesh.h"
#include "Engine/World.h"
#include "Materials/MaterialInstanceDynamic.h"

namespace
{
const FLinearColor BodyColor(0.20f, 0.07f, 0.04f);
const FLinearColor HeadColor(0.55f, 0.42f, 0.30f);
const FLinearColor PhaselockColor(0.55f, 0.18f, 1.f);
constexpr float RespawnSeconds = 3.f;

// Height above the floor at Held seconds into a lock (LiftActionSkill.UpdateLiftedPawn / GetBobLocation, read not
// run): reach SnapHeightPct of the height in SnapTimePct of the lift, the rest by the end of the lift, then a sine
// bob of BobAmplitude at BobFrequency (sin(t x frequency x pi)). The curve shapes between those points (quadratic in,
// ease-out) and the bob's time origin (end of the lift) are host choices: UNVERIFIED. The ground trace, collision
// height and ceiling clamp of GetLiftLocation are not applied (height is above the target's own origin).
float LiftHeightAt(const FOpenWillowPhaselockData& D, float Held)
{
    const float H = D.HeightFromGround;
    if (Held >= D.LiftDuration)
        return H + D.BobAmplitude * FMath::Sin((Held - D.LiftDuration) * D.BobFrequency * PI);
    const float U = D.LiftDuration > 0.f ? Held / D.LiftDuration : 1.f;
    if (U < D.SnapTimePct)
        return D.SnapHeightPct * H * FMath::Square(U / D.SnapTimePct);
    return D.SnapHeightPct * H + (1.f - D.SnapHeightPct) * H
        * FMath::InterpEaseOut(0.f, 1.f, (U - D.SnapTimePct) / FMath::Max(1.f - D.SnapTimePct, KINDA_SMALL_NUMBER), 2.f);
}

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

bool AOpenWillowCombatTarget::UseStockPawn(const FString& MeshPath, const FString& IdlePath, const FString& DeathPath,
    const FVector& MeshOffset)
{
    USkeletalMesh* Mesh = LoadObject<USkeletalMesh>(nullptr, *MeshPath);
    UAnimSequence* Idle = LoadObject<UAnimSequence>(nullptr, *IdlePath);
    StockDeath = LoadObject<UAnimSequence>(nullptr, *DeathPath);
    if (!Mesh || !Idle || !StockDeath) return false;
    for (UStaticMeshComponent* Shape : {Post.Get(), Torso.Get(), Head.Get()})
    {
        Shape->SetHiddenInGame(true);
        Shape->SetCollisionEnabled(ECollisionEnabled::NoCollision);
    }
    StockMesh = NewObject<USkeletalMeshComponent>(this, TEXT("StockPawnMesh"));
    StockMesh->SetupAttachment(Pivot);
    StockMesh->SetSkeletalMesh(Mesh);
    StockMesh->SetRelativeLocation(MeshOffset);
    StockMesh->SetCollisionEnabled(ECollisionEnabled::NoCollision);
    StockMesh->RegisterComponent();
    StockMesh->PlayAnimation(Idle, true);
    // Hit volume: a capsule from the imported mesh's bounds (host-chosen shape; the stock pawn's collision cylinder
    // is not read). The narrower horizontal extent is the radius, so outstretched bind-pose arms do not widen it.
    const FBoxSphereBounds Bounds = Mesh->GetBounds();
    HitVolume = NewObject<UCapsuleComponent>(this, TEXT("StockHitVolume"));
    HitVolume->SetupAttachment(Pivot);
    HitVolume->SetRelativeLocation(MeshOffset + Bounds.Origin);
    HitVolume->SetCapsuleSize(FMath::Min(Bounds.BoxExtent.X, Bounds.BoxExtent.Y), Bounds.BoxExtent.Z);
    HitVolume->SetCollisionEnabled(ECollisionEnabled::QueryOnly);
    HitVolume->SetCollisionResponseToAllChannels(ECR_Ignore);
    HitVolume->SetCollisionResponseToChannel(ECC_Visibility, ECR_Block);
    HitVolume->SetHiddenInGame(true);
    HitVolume->RegisterComponent();
    return true;
}

float AOpenWillowCombatTarget::PhaselockReleasedAt() const { return ReleasedAt; }
float AOpenWillowCombatTarget::LiftedHeight() const { return Pivot->GetRelativeLocation().Z; }

FVector AOpenWillowCombatTarget::AimPoint() const
{
    if (HitVolume) return HitVolume->GetComponentLocation();
    return Torso->GetComponentLocation() + FVector(0, 0, 10);
}

bool AOpenWillowCombatTarget::BeginPhaselock(float Now, const FOpenWillowPhaselockData& Data, const FOpenWillowPhaselockTimeline& Timeline)
{
    if (bPhaselocked || bDead) return false;
    Lock = Data;
    LockStartedAt = Now;
    LockEndsAt = Now + Timeline.ReleasedAt;
    DropStartedAt = -10;
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
        if (StockMesh) return; // the stock dummy keeps its death pose until the sequence destroys it
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
        Height = LiftHeightAt(Lock, Held);
        // The script moves the lifted pawn without rotating it; only hit wobble remains.
        Pivot->SetRelativeRotation(FRotator(Wobble.Y, Pivot->GetRelativeRotation().Yaw, Wobble.X));
        // Host shell (presentation only, not the stock bubble effect): grows in, then fades over the outro
        // (LockFadeOutTime before the release).
        const float Pulse = 0.85f + 0.15f * FMath::Sin(Held * 7.f);
        const float Grow = FMath::Clamp(Held / 0.25f, 0.f, 1.f)
            * FMath::Clamp((LockEndsAt - Now) / FMath::Max(Lock.LockFadeOutTime, KINDA_SMALL_NUMBER), 0.f, 1.f);
        LockSphere->SetRelativeScale3D(FVector(2.2f * FMath::Max(Grow, 0.01f) * Pulse));
        if (LockMaterial) LockMaterial->SetScalarParameterValue(TEXT("Intensity"), 3.f * Pulse * Grow);
        FallVelocity = 0.f;
        if (Now >= LockEndsAt)
        {
            // ReleaseTarget: drop over DropTime; OnReleasedTarget activates Skill_Phaselock_DiminishingReturns on the
            // target for its InitialDuration.
            bPhaselocked = false;
            ReleasedAt = Now;
            DropStartedAt = Now;
            DropFromHeight = Height;
            DiminishedUntil = Now + Lock.DiminishingSeconds;
            LockSphere->SetHiddenInGame(true);
            UE_LOG(LogTemp, Display, TEXT("OpenWillow Phaselock released %s after %.2f s"), *GetName(), Held);
        }
    }
    else if (Now - DropStartedAt < Lock.DropTime)
    {
        // Drop back over DropTime (quadratic ease-in: host shape, UNVERIFIED; the stock drop plays DropAnim).
        Height = DropFromHeight * (1.f - FMath::Square((Now - DropStartedAt) / Lock.DropTime));
        FallVelocity = 0.f;
        Pivot->SetRelativeRotation(FRotator(Wobble.Y, Pivot->GetRelativeRotation().Yaw, Wobble.X));
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
    // The dummy's own behavior provider (OnTakeDamage) runs in the quest component of the shooter, with the stock
    // damage type path of the shot in flight ("" = None for damage that is not one of Maya's shots).
    if (const auto* Shooter = Cast<AOpenWillowWalker>(DamageCauser))
        if (UOpenWillowQuest* Quest = Shooter->GetQuest(); Quest && Quest->Enabled())
            Quest->OnDummyDamaged(this, Shooter->ShotDamageType());
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
        if (StockMesh && StockDeath) StockMesh->PlayAnimation(StockDeath, false);
        AOpenWillowShotFx::Flash(GetWorld(), AimPoint(), FLinearColor(1.f, 0.55f, 0.2f), 70.f, 0.25f, 4000.f);
        UE_LOG(LogTemp, Display, TEXT("OpenWillow combat target destroyed; respawns in %.0fs"), RespawnSeconds);
    }
    return Applied;
}
