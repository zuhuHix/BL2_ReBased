#include "OpenWillowUseView.h"
#include "OpenWillowWalker.h"
#include "Camera/CameraComponent.h"
#include "Components/PrimitiveComponent.h"
#include "Components/StaticMeshComponent.h"
#include "Engine/StaticMeshActor.h"
#include "EngineUtils.h"
#include "Engine/World.h"

bool OpenWillowUseView::RayEntersBox(const FVector& Eye, const FVector& Direction, const FBox& Box, double& OutDistance)
{
    if (!Box.IsValid) return false;
    FVector HitLocation, HitNormal;
    float HitTime = 0.f;
    if (!FMath::LineExtentBoxIntersection(Box, Eye, Eye + Direction * RayLength, FVector::ZeroVector, HitLocation, HitNormal, HitTime)) return false;
    OutDistance = HitTime * RayLength;
    return true;
}

FVector OpenWillowUseView::EyeOf(const AOpenWillowWalker& Walker)
{
    const UCameraComponent* Camera = Walker.FindComponentByClass<UCameraComponent>();
    return Camera ? Camera->GetComponentLocation() : Walker.GetPawnViewLocation();
}

// The one ray both callers below use. True when it enters the box; OutBlocked when something stops it first.
static bool TraceUseRay(const AOpenWillowWalker& Walker, const FBox& Box, const AActor* Target, const AActor* Body, double& OutDistance, bool& OutBlocked, FHitResult& OutBlock)
{
    OutBlocked = false;
    const UWorld* World = Walker.GetWorld();
    if (!World) return false;
    // The control rotation, as in UOpenWillowQuest::InTalkReach: current even in the frame of an aim change.
    const FVector Eye = OpenWillowUseView::EyeOf(Walker);
    const FVector Direction = Walker.GetViewRotation().Vector();
    if (!OpenWillowUseView::RayEntersBox(Eye, Direction, Box, OutDistance)) return false;
    FCollisionQueryParams Query(SCENE_QUERY_STAT(OWUseView), true, &Walker);
    if (Target) Query.AddIgnoredActor(Target);
    if (Body) Query.AddIgnoredActor(Body);
    OutBlocked = World->LineTraceSingleByChannel(OutBlock, Eye, Eye + Direction * OutDistance, ECC_Visibility, Query);
    return true;
}

AActor* OpenWillowUseView::FindPlacedBody(UWorld& World, const FString& ObjectName, const FVector& Near, FBox& OutBox)
{
    const FString Needle = TEXT(".") + ObjectName + TEXT(".");
    AActor* Body = nullptr;
    OutBox = FBox(ForceInit);
    for (TActorIterator<AStaticMeshActor> It(&World); It; ++It)
    {
        const bool bNamed = It->Tags.ContainsByPredicate([&Needle](const FName& Tag) { return Tag.ToString().Contains(Needle); });
        if (!bNamed || FVector::DistSquared(It->GetActorLocation(), Near) > FMath::Square(50.)) continue;
        OutBox += It->GetComponentsBoundingBox(true);
        // Each section is its own actor, and only the first has collision: that one is what a ray would hit.
        const UStaticMeshComponent* Component = It->GetStaticMeshComponent();
        if (!Body || (Component && Component->IsCollisionEnabled())) Body = *It;
    }
    return Body;
}

bool OpenWillowUseView::LooksAt(const AOpenWillowWalker& Walker, const FBox& Box, const AActor* Target, const AActor* Body)
{
    double Distance = 0;
    bool bBlocked = false;
    FHitResult Block;
    return TraceUseRay(Walker, Box, Target, Body, Distance, bBlocked, Block) && !bBlocked;
}

FString OpenWillowUseView::Describe(const AOpenWillowWalker& Walker, const FBox& Box, const AActor* Target, const AActor* Body)
{
    double Distance = 0;
    bool bBlocked = false;
    FHitResult Block;
    const bool bEnters = TraceUseRay(Walker, Box, Target, Body, Distance, bBlocked, Block);
    const AActor* HitActor = bBlocked ? Block.GetActor() : nullptr;
    const UPrimitiveComponent* HitComponent = bBlocked ? Block.GetComponent() : nullptr;
    return FString::Printf(TEXT("walker=(%s) eye=(%s) dir=(%s) box=(%s) enters=%d distance=%.1f blocked=%d start_penetrating=%d hit=%s/%s hit_distance=%.1f"),
        *Walker.GetActorLocation().ToCompactString(), *EyeOf(Walker).ToCompactString(), *Walker.GetViewRotation().Vector().ToCompactString(), *Box.ToString(),
        bEnters ? 1 : 0, Distance, bBlocked ? 1 : 0, bBlocked && Block.bStartPenetrating ? 1 : 0, HitActor ? *HitActor->GetName() : TEXT("none"), HitComponent ? *HitComponent->GetName() : TEXT("none"), bBlocked ? Block.Distance : 0.f);
}
