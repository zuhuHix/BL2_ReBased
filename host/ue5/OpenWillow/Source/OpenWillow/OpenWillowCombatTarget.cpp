#include "OpenWillowCombatTarget.h"
#include "Components/StaticMeshComponent.h"
#include "DrawDebugHelpers.h"
#include "Engine/StaticMesh.h"
#include "Engine/World.h"

AOpenWillowCombatTarget::AOpenWillowCombatTarget()
{
    PrimaryActorTick.bCanEverTick = true;
    Visual = CreateDefaultSubobject<UStaticMeshComponent>(TEXT("TargetVisual"));
    RootComponent = Visual;
    Visual->SetStaticMesh(LoadObject<UStaticMesh>(nullptr, TEXT("/Engine/BasicShapes/Sphere.Sphere")));
    Visual->SetWorldScale3D(FVector(1.2f));
    Visual->SetCollisionEnabled(ECollisionEnabled::QueryOnly);
    Visual->SetCollisionResponseToAllChannels(ECR_Ignore);
    Visual->SetCollisionResponseToChannel(ECC_Visibility, ECR_Block);
}

bool AOpenWillowCombatTarget::BeginPhaselock(float Now, float Duration)
{
    if (bPhaselocked || Health <= 0.f) return false;
    LockStart = GetActorLocation();
    LockStartedAt = Now;
    LockEndsAt = Now + Duration;
    bPhaselocked = true;
    return true;
}

void AOpenWillowCombatTarget::Tick(float DeltaSeconds)
{
    Super::Tick(DeltaSeconds);
    if (!bPhaselocked) return;
    const float Now = GetWorld()->GetTimeSeconds();
    if (Now >= LockEndsAt)
    {
        bPhaselocked = false;
        SetActorLocation(LockStart);
        UE_LOG(LogTemp, Display, TEXT("OpenWillow Phaselock released %s"), *GetName());
        return;
    }
    // Installed action skill data gives a 0.7 s lift. This stand-in has no AI
    // to suspend, so its position and a purple ring show the lock state.
    const float Lift = FMath::Clamp((Now - LockStartedAt) / 0.7f, 0.f, 1.f);
    SetActorLocation(LockStart + FVector(0, 0, 110.f * Lift));
    DrawDebugSphere(GetWorld(), GetActorLocation(), 90.f, 20, FColor(170, 70, 255), false, 0.f, 0, 3.f);
}

float AOpenWillowCombatTarget::TakeDamage(float DamageAmount, const FDamageEvent& DamageEvent,
    AController* EventInstigator, AActor* DamageCauser)
{
    const float Applied = FMath::Clamp(DamageAmount, 0.f, Health);
    Health -= Applied;
    UE_LOG(LogTemp, Display, TEXT("OpenWillow combat target health %.0f/100"), Health);
    if (Health <= 0.f) Destroy();
    return Applied;
}
