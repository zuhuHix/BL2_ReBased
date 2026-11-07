#pragma once
#include "CoreMinimal.h"

class AActor;
class AOpenWillowWalker;
class UWorld;

// Does Maya look at a use target? For vending machines, lootables, Quick Change stations and doors (docs/verification/CONTROLS.md, "Use prompt").
// From the native reading (NATIVE_USE_INTERACTION.md): one ray from the camera along the view, first usable hit before a blocking hit. The real
// game only gave a lead (8 of 11 targets stopped being usable on a 15 to 30 degree turn): UNVERIFIED as the game's rule. Host targets have no
// reliable collision, so the ray tests a box that stands in for the usable's collision. Pickups and Marcus keep their own rules.
namespace OpenWillowUseView
{
    constexpr double RayLength = 350.;   // GlobalsDefinition.PlayerInteractionDistance, as NATIVE_USE_INTERACTION.md reads it

    // The rule with no engine state: true when the ray enters Box within RayLength, with the distance to that point (0 from inside the box).
    // Direction is a unit vector.
    bool RayEntersBox(const FVector& Eye, const FVector& Direction, const FBox& Box, double& OutDistance);

    // Where the ray starts: the walker's camera.
    FVector EyeOf(const AOpenWillowWalker& Walker);

    // The imported level's own mesh for a placed object, by the source path the importer tags it with, within 50 uu of Near. OutBox holds all its sections' bounds.
    // Null when the map has none. A usable that is in the map this way is two actors in the host: its mesh must not count as something in front of it.
    AActor* FindPlacedBody(UWorld& World, const FString& ObjectName, const FVector& Near, FBox& OutBox);

    // The ray from Walker's current view enters Box, and nothing but Walker, Target and Target's Body blocks it on the way.
    bool LooksAt(const AOpenWillowWalker& Walker, const FBox& Box, const AActor* Target, const AActor* Body = nullptr);

    // For a test's log: the same ray and trace as LooksAt, spelled out: where it starts, where it points, the box, and what stopped it.
    FString Describe(const AOpenWillowWalker& Walker, const FBox& Box, const AActor* Target, const AActor* Body = nullptr);
}
