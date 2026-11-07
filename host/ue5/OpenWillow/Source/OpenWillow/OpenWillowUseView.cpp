#include "OpenWillowUseView.h"
#include "OpenWillowWalker.h"
#include "Camera/CameraComponent.h"
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

bool OpenWillowUseView::LooksAt(const AOpenWillowWalker& Walker, const FBox& Box, const AActor* Target)
{
    const UWorld* World = Walker.GetWorld();
    if (!World) return false;
    // The control rotation, as in UOpenWillowQuest::InTalkReach: current even in the frame of an aim change.
    const FVector Eye = EyeOf(Walker);
    const FVector Direction = Walker.GetViewRotation().Vector();
    double Distance = 0;
    if (!RayEntersBox(Eye, Direction, Box, Distance)) return false;
    FCollisionQueryParams Query(SCENE_QUERY_STAT(OWUseView), true, &Walker);
    if (Target) Query.AddIgnoredActor(Target);
    FHitResult Block;
    return !World->LineTraceSingleByChannel(Block, Eye, Eye + Direction * Distance, ECC_Visibility, Query);
}
