#include "OpenWillowUseView.h"
#include "OpenWillowWalker.h"
#include "Camera/CameraComponent.h"
#include "Components/PrimitiveComponent.h"
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

// The one ray both callers below use. Returns whether it may use the target; OutEnters whether the ray enters the box, OutBlocked whether something else stopped it first.
static bool TraceUseRay(const AOpenWillowWalker& Walker, const FBox& Box, const AActor* Target, const OpenWillowUseView::FPlacedBody* Body, bool& OutEnters, double& OutDistance,
    bool& OutBlocked, FHitResult& OutBlock)
{
    OutBlocked = false;
    OutEnters = false;
    const UWorld* World = Walker.GetWorld();
    if (!World) return false;
    // The control rotation, as in UOpenWillowQuest::InTalkReach: current even in the frame of an aim change.
    const FVector Eye = OpenWillowUseView::EyeOf(Walker);
    const FVector Direction = Walker.GetViewRotation().Vector();
    OutEnters = OpenWillowUseView::RayEntersBox(Eye, Direction, Box, OutDistance);
    FCollisionQueryParams Query(SCENE_QUERY_STAT(OWUseView), true, &Walker);
    if (Target) Query.AddIgnoredActor(Target);
    if (Body)
    {
        const bool bHit = World->LineTraceSingleByChannel(OutBlock, Eye, Eye + Direction * OpenWillowUseView::RayLength, ECC_Visibility, Query);
        const bool bOwn = bHit && Body->Contains(OutBlock.GetActor());
        OutBlocked = bHit && !bOwn;
        return bOwn;
    }
    if (!OutEnters) return false;
    OutBlocked = World->LineTraceSingleByChannel(OutBlock, Eye, Eye + Direction * OutDistance, ECC_Visibility, Query);
    return !OutBlocked;
}

bool OpenWillowUseView::FindPlacedBody(UWorld& World, const FString& OwnerPath, const FVector& Near, FPlacedBody& OutBody, FString& OutProblem)
{
    OutBody = FPlacedBody();
    const FString Prefix = OwnerPath + TEXT(".");
    FPlacedBody Found;
    TMap<FName, FVector> PlacedAt;
    TMap<FName, FBox> ComponentBoxes;
    for (TActorIterator<AStaticMeshActor> It(&World); It; ++It)
    {
        const FName* Tag = It->Tags.FindByPredicate([&Prefix](const FName& Candidate)
        {
            const FString Name = Candidate.ToString();
            return Name.StartsWith(Prefix) && !Name.Mid(Prefix.Len()).Contains(TEXT("."));
        });
        if (!Tag) continue;
        // One component's sections share one transform, however far their geometry reaches.
        const FVector* Seen = PlacedAt.Find(*Tag);
        if (Seen && !Seen->Equals(It->GetActorLocation(), 1.))
        {
            OutProblem = FString::Printf(TEXT("%s is placed twice"), *Tag->ToString());
            return false;
        }
        PlacedAt.Add(*Tag, It->GetActorLocation());
        const FBox SectionBox = It->GetComponentsBoundingBox(true);
        ComponentBoxes.FindOrAdd(*Tag, FBox(ForceInit)) += SectionBox;
        Found.Box += SectionBox;
        Found.Sections.Add(*It);
    }
    if (Found.Sections.Num() == 0)
    {
        OutProblem = FString::Printf(TEXT("no imported mesh for %s"), *OwnerPath);
        return false;
    }
    for (const TPair<FName, FBox>& Component : ComponentBoxes)
    {
        if (!Component.Value.ExpandBy(50.).IsInsideOrOn(Near))
        {
            OutProblem = FString::Printf(TEXT("%s does not reach the placed point"), *Component.Key.ToString());
            return false;
        }
    }
    OutBody = MoveTemp(Found);
    return true;
}

bool OpenWillowUseView::LooksAt(const AOpenWillowWalker& Walker, const FBox& Box, const AActor* Target, const FPlacedBody* Body)
{
    bool bEnters = false, bBlocked = false;
    double Distance = 0;
    FHitResult Block;
    return TraceUseRay(Walker, Box, Target, Body, bEnters, Distance, bBlocked, Block);
}

FString OpenWillowUseView::Describe(const AOpenWillowWalker& Walker, const FBox& Box, const AActor* Target, const FPlacedBody* Body)
{
    bool bEnters = false, bBlocked = false;
    double Distance = 0;
    FHitResult Block;
    const bool bUsable = TraceUseRay(Walker, Box, Target, Body, bEnters, Distance, bBlocked, Block);
    const AActor* HitActor = Block.bBlockingHit ? Block.GetActor() : nullptr;
    const UPrimitiveComponent* HitComponent = Block.bBlockingHit ? Block.GetComponent() : nullptr;
    return FString::Printf(TEXT("walker=(%s) eye=(%s) dir=(%s) box=(%s) enters=%d distance=%.1f blocked=%d start_penetrating=%d hit=%s/%s hit_distance=%.1f usable=%d body_sections=%d"),
        *Walker.GetActorLocation().ToCompactString(), *EyeOf(Walker).ToCompactString(), *Walker.GetViewRotation().Vector().ToCompactString(), *Box.ToString(),
        bEnters ? 1 : 0, Distance, bBlocked ? 1 : 0, Block.bBlockingHit && Block.bStartPenetrating ? 1 : 0, HitActor ? *HitActor->GetName() : TEXT("none"), HitComponent ? *HitComponent->GetName() : TEXT("none"),
        Block.bBlockingHit ? Block.Distance : 0.f, bUsable ? 1 : 0, Body ? Body->Sections.Num() : 0);
}
