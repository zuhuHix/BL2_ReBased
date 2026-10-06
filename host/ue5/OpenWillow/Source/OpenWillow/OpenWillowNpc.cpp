#include "OpenWillowNpc.h"
#include "Animation/AnimSequence.h"
#include "Components/CapsuleComponent.h"
#include "Components/SkeletalMeshComponent.h"
#include "Engine/SkeletalMesh.h"

AOpenWillowNpc::AOpenWillowNpc()
{
    PrimaryActorTick.bCanEverTick = true;
    // The actor origin is the UE3 pawn location; the mesh hangs below it by the pawn's component Translation.
    Root = CreateDefaultSubobject<USceneComponent>(TEXT("PawnOrigin"));
    RootComponent = Root;
    Mesh = CreateDefaultSubobject<USkeletalMeshComponent>(TEXT("Mesh"));
    Mesh->SetupAttachment(Root);
    Mesh->SetCollisionEnabled(ECollisionEnabled::NoCollision);
}

bool AOpenWillowNpc::Setup(const FString& MeshPath, const FString& IdlePath, const FString& WalkPath, const FVector& MeshOffset)
{
    USkeletalMesh* Skeletal = LoadObject<USkeletalMesh>(nullptr, *MeshPath);
    IdleAnim = LoadObject<UAnimSequence>(nullptr, *IdlePath);
    WalkAnim = WalkPath.IsEmpty() ? nullptr : LoadObject<UAnimSequence>(nullptr, *WalkPath);
    if (!Skeletal || !IdleAnim || (!WalkPath.IsEmpty() && !WalkAnim)) return false;
    Mesh->SetSkeletalMesh(Skeletal);
    Mesh->SetRelativeLocation(MeshOffset);
    Play(IdleAnim);
    // Hit volume for the use ray: a capsule from the imported mesh's bounds (host-chosen shape, UNVERIFIED; the stock pawn's collision
    // cylinder is not read), query-only and blocking only the visibility channel, so it never blocks movement. The narrower horizontal
    // extent is the radius, as for the combat targets.
    const FBoxSphereBounds Bounds = Mesh->Bounds;
    HitVolume = NewObject<UCapsuleComponent>(this, TEXT("UseHitVolume"));
    HitVolume->SetupAttachment(Root);
    HitVolume->SetRelativeLocation(Root->GetComponentTransform().InverseTransformPosition(Bounds.Origin));
    HitVolume->SetCapsuleSize(FMath::Min(Bounds.BoxExtent.X, Bounds.BoxExtent.Y), Bounds.BoxExtent.Z);
    HitVolume->SetCollisionEnabled(ECollisionEnabled::QueryOnly);
    HitVolume->SetCollisionResponseToAllChannels(ECR_Ignore);
    HitVolume->SetCollisionResponseToChannel(ECC_Visibility, ECR_Block);
    HitVolume->SetHiddenInGame(true);
    HitVolume->RegisterComponent();
    return true;
}

void AOpenWillowNpc::Play(UAnimSequence* Anim)
{
    if (!Anim || Playing == Anim) return;
    Playing = Anim;
    Mesh->PlayAnimation(Anim, true);
}

void AOpenWillowNpc::StartWalk(const TArray<FNode>& Nodes, float GroundSpeed, float YawRateDegrees)
{
    Path = Nodes;
    Speed = GroundSpeed;
    YawRate = YawRateDegrees;
    Next = 0;
    Reached = 0;
    bWalking = Path.Num() > 0;
    LookAt.Reset();
    Play(WalkAnim ? WalkAnim.Get() : IdleAnim.Get());
}

TArray<int32> AOpenWillowNpc::TakeArrivals()
{
    TArray<int32> Out = MoveTemp(Arrivals);
    Arrivals.Reset();
    return Out;
}

void AOpenWillowNpc::TurnToward(const FVector& Direction, float DeltaSeconds)
{
    if (Direction.IsNearlyZero(1e-3)) return;
    FRotator Rotation = GetActorRotation();
    const float Wanted = Direction.Rotation().Yaw;
    const float Step = FMath::FindDeltaAngleDegrees(Rotation.Yaw, Wanted);
    Rotation.Yaw += FMath::Clamp(Step, -YawRate * DeltaSeconds, YawRate * DeltaSeconds);
    SetActorRotation(FRotator(0, Rotation.Yaw, 0));
}

bool AOpenWillowNpc::IsLookingAt(const AActor* Target, float ToleranceDegrees) const
{
    if (!Target) return false;
    const FVector To = Target->GetActorLocation() - GetActorLocation();
    return FMath::Abs(FMath::FindDeltaAngleDegrees(GetActorRotation().Yaw, To.Rotation().Yaw)) <= ToleranceDegrees;
}

void AOpenWillowNpc::Tick(float DeltaSeconds)
{
    Super::Tick(DeltaSeconds);
    float Budget = Speed * DeltaSeconds;
    while (bWalking && Next < Path.Num())
    {
        const FNode& Node = Path[Next];
        const FVector To = Node.Location - GetActorLocation();
        const float Planar = To.Size2D();
        if (Planar <= Node.ArrivalRadius)
        {
            Arrivals.Add(Next);
            ++Reached;
            if (++Next >= Path.Num()) { bWalking = false; Play(IdleAnim); }
            continue;
        }
        if (Budget <= 0) break;
        // Move along the 3D line toward the node; the planar part advances by the speed budget.
        const float Step = FMath::Min(Budget * Node.SpeedPercentage, Planar - Node.ArrivalRadius + 1.f);
        SetActorLocation(GetActorLocation() + To * (Step / Planar));
        TurnToward(To, DeltaSeconds);
        Budget = 0;
    }
    if (!bWalking && LookAt.IsValid())
        TurnToward(LookAt->GetActorLocation() - GetActorLocation(), DeltaSeconds);
}
