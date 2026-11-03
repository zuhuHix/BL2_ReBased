#pragma once

#include "CoreMinimal.h"

// One range enemy kind, read at run time from local/range/range_manifest.json (schema ow-range-v1, written by
// tools/prepare_range.py). Nothing here is game data: the asset paths and numbers come from the ignored local/ folder.
// Host stand-ins (UNVERIFIED in game) are labelled where they are used.
struct FOpenWillowRangeKind
{
    FString Name;                        // the manifest key, e.g. "bullymong"
    FString Label;
    FString Mesh, Idle, Death;           // UE object paths; the stock pawn needs all three
    FString Walk, Run, Hit, Melee;       // optional clips (Walk and Hit are used in PR1)
    float MeshOffsetZ = 0.f;
    float MeshScale = 1.f;
    float CollisionRadius = 0.f;         // the pawn's cylinder (0 = the bounds capsule stays)
    float CollisionHeight = 0.f;         // half height of that cylinder
    FString HeadBone;                    // empty: the first bone whose name has "head" (not "headset")
    float HeadRadius = 18.f;             // critical region radius (uu)
    float WalkSpeed = 150.f;             // uu/s, host stand-in for the patrol walk
    float PatrolRadius = 500.f;          // uu around the home spot
    float Shield = 0.f;
    float ShieldDelay = 0.f;             // seconds without a hit before the shield regenerates
    float ShieldRate = 0.f;              // shield per second once regenerating (0 = none)
    TMap<int32, float> HealthByLevel;    // level -> max health (entries of 0 are missing data)

    // Health for Level: the entry of the nearest level at or below it. False when there is none (use the stand-in).
    bool HealthFor(int32 Level, float& OutHealth) const
    {
        int32 BestLevel = -1;
        for (const auto& Pair : HealthByLevel)
            if (Pair.Key <= Level && Pair.Key > BestLevel && Pair.Value > 0.f)
            {
                BestLevel = Pair.Key;
                OutHealth = Pair.Value;
            }
        return BestLevel >= 0;
    }
};

struct FOpenWillowRangeSpawn
{
    FString Kind;
    int32 Count = 0;
};

// The whole manifest. Load() is in OpenWillowRange.cpp; a missing or invalid file gives one engine-shape stand-in kind.
struct FOpenWillowRangeManifest
{
    float RespawnSeconds = 8.f;
    TArray<FOpenWillowRangeSpawn> Spawns;
    TMap<FString, FOpenWillowRangeKind> Kinds;
    bool bFromFile = false;

    static FOpenWillowRangeManifest Load(const FString& Path, FString& OutLog);
};
