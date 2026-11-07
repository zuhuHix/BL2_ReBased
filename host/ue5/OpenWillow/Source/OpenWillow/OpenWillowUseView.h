#pragma once
#include "CoreMinimal.h"
#include "GameFramework/Actor.h"

class AOpenWillowWalker;
class UWorld;

// Does Maya look at a use target? For vending machines, lootables, Quick Change stations and doors (docs/verification/CONTROLS.md, "Use prompt").
// From the native reading (NATIVE_USE_INTERACTION.md): one ray from the camera along the view, first usable hit before a blocking hit. The real
// game only gave a lead (8 of 11 targets stopped being usable on a 15 to 30 degree turn): UNVERIFIED as the game's rule. Host targets have no
// reliable collision, so the ray tests a box that stands in for the usable's collision. Pickups and Marcus keep their own rules.
namespace OpenWillowUseView
{
    // The imported level's own mesh for a placed usable, which in the host is other actors than the usable: each of its sections is one, and any may have collision.
    struct FPlacedBody
    {
        TArray<TWeakObjectPtr<AActor>> Sections;
        FBox Box = FBox(ForceInit);
        bool Contains(const AActor* Actor) const
        {
            return Actor && Sections.ContainsByPredicate([Actor](const TWeakObjectPtr<AActor>& Section) { return Section.Get() == Actor; });
        }
    };

    constexpr double RayLength = 350.;   // GlobalsDefinition.PlayerInteractionDistance, as NATIVE_USE_INTERACTION.md reads it

    // The rule with no engine state: true when the ray enters Box within RayLength, with the distance to that point (0 from inside the box).
    // Direction is a unit vector.
    bool RayEntersBox(const FVector& Eye, const FVector& Direction, const FBox& Box, double& OutDistance);

    // Where the ray starts: the walker's camera.
    FVector EyeOf(const AOpenWillowWalker& Walker);

    // Every section of the object at OwnerPath (the level's object path; the importer tags each section with OwnerPath.Component). False with OutProblem when there is none or it
    // cannot be told for certain: one component at two places (the same name in two levels: the tag carries no level), or a component whose bounds do not reach Near.
    bool FindPlacedBody(UWorld& World, const FString& OwnerPath, const FVector& Near, FPlacedBody& OutBody, FString& OutProblem);

    // Without a Body: the ray from Walker's current view enters Box, and nothing but Walker and Target blocks it on the way.
    // With a Body: the first thing the whole ray hits, Target aside, is a section of it, as the native note has the first usable hit be the object's own collision. Box is not used.
    bool LooksAt(const AOpenWillowWalker& Walker, const FBox& Box, const AActor* Target, const FPlacedBody* Body = nullptr);

    // For a test's log: the same ray and trace as LooksAt, spelled out: where it starts, where it points, the box, and what it hit.
    FString Describe(const AOpenWillowWalker& Walker, const FBox& Box, const AActor* Target, const FPlacedBody* Body = nullptr);
}
