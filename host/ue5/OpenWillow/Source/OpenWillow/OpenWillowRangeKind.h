#pragma once

#include "CoreMinimal.h"
#include "Dom/JsonObject.h"

// The entry of the nearest level at or below Level (an entry of 0 counts as missing). False when there is none, and then
// OutValue is not written.
inline bool OpenWillowValueAtLevel(const TMap<int32, float>& Table, int32 Level, float& OutValue)
{
    int32 BestLevel = -1;
    for (const auto& Pair : Table)
        if (Pair.Key <= Level && Pair.Key > BestLevel && Pair.Value > 0.f)
        {
            BestLevel = Pair.Key;
            OutValue = Pair.Value;
        }
    return BestLevel >= 0;
}

// One pool of a loot entry (PR3, manifest "loot_by_level"). Chance is the VM's per-death probability for the pool (its
// draw is the host's stand-in, UNVERIFIED). Kind is weapon, money, ammo or other.
struct FOpenWillowRangeLootPool
{
    FString Pool;                        // the pool's object path (from the ignored manifest)
    float Chance = 0.f;
    FString Kind;
};

// The loot of one level entry: the item stage the weapon pools roll at, and the pools in the VM's order.
struct FOpenWillowRangeLoot
{
    // The pawn's item game stage: ItemStageMin..ItemStageMax (the adult's stage carries a random offset, COMBAT_LOOT); the
    // host draws one per death from the loot stream. Min == Max for a kind without an offset.
    int32 ItemStage = 0;
    int32 ItemStageMin = 0;
    int32 ItemStageMax = 0;
    TArray<FOpenWillowRangeLootPool> Pools;
};

// One range enemy kind, read at run time from local/range/range_manifest.json (schema ow-range-v1, written by
// tools/prepare_range.py). Nothing here is game data: the asset paths and numbers come from the ignored local/ folder.
// Host stand-ins (UNVERIFIED in game) are labelled where they are used.
struct FOpenWillowRangeKind
{
    TSharedPtr<FJsonObject> Json;        // the manifest's entry, for what is read straight from it (the worn meshes)
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
    float RunSpeed = 400.f;              // uu/s, host stand-in for the chase run (manifest run_speed)
    float MeleeStartRange = 0.f;         // uu, the stock attack action's range: a swing starts inside it (0 = an old manifest)
    float Shield = 0.f;
    float ShieldDelay = 0.f;             // seconds without a hit before the shield regenerates
    float ShieldRate = 0.f;              // shield per second once regenerating (0 = none)
    TMap<int32, float> HealthByLevel;    // level -> max health (entries of 0 are missing data)
    TMap<int32, float> MeleeArrivalByLevel;  // level -> melee arrival damage (the VM swing rows; 0 = missing: no melee)
    TMap<int32, float> MeleeRatioByLevel;    // level -> the AdjustDamage ratio of that level (0 = missing: no melee)
    TMap<int32, FOpenWillowRangeLoot> LootByLevel;  // PR3: level -> the loot entry (the death roll reads it)

    // Health for Level: the entry of the nearest level at or below it. False when there is none (use the stand-in).
    bool HealthFor(int32 Level, float& OutHealth) const { return OpenWillowValueAtLevel(HealthByLevel, Level, OutHealth); }

    // The loot entry of the nearest level at or below Level (an entry of level 0 counts). Null when there is none.
    const FOpenWillowRangeLoot* LootFor(int32 Level) const
    {
        const FOpenWillowRangeLoot* Found = nullptr;
        int32 FoundLevel = -1;
        for (const auto& Pair : LootByLevel)
            if (Pair.Key <= Level && Pair.Key > FoundLevel)
            {
                FoundLevel = Pair.Key;
                Found = &Pair.Value;
            }
        return Found;
    }
};

struct FOpenWillowRangeSpawn
{
    FString Kind;
    int32 Count = 0;
};

// A spot candidate from the manifest's "spot_candidates" (written by tools/prepare_range.py, gate terrain first). The
// location is a point on a terrain near the town gate; the host tries it, and points around it, in ResolveSpot.
struct FOpenWillowRangeCandidate
{
    FVector Location = FVector::ZeroVector;
    FString Source;                      // the terrain the point was probed on
    bool bGateTerrain = false;
};

// The whole manifest. Load() is in OpenWillowRange.cpp; a missing or invalid file gives one engine-shape stand-in kind.
struct FOpenWillowRangeManifest
{
    float RespawnSeconds = 8.f;
    TArray<FOpenWillowRangeSpawn> Spawns;
    TMap<FString, FOpenWillowRangeKind> Kinds;
    TArray<FOpenWillowRangeCandidate> Candidates;
    // From ignored local data only (tools/prepare_range.py): the plaza's centre (scout rings, "away from town"), and the index of the
    // candidate that is the range's default spot (-1: none). No map coordinate is compiled in.
    FVector2D ScoutCentre = FVector2D::ZeroVector;
    bool bHasScoutCentre = false;
    int32 DefaultSpotCandidate = -1;   // empty for the stand-in manifest
    bool bFromFile = false;

    static FOpenWillowRangeManifest Load(const FString& Path, FString& OutLog);
};
