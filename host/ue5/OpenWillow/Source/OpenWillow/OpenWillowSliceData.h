#pragma once
#include "CoreMinimal.h"
#include "Dom/JsonObject.h"

// Game-derived data for the Sanctuary slice route, read at run time from the ignored local/ manifests (nothing
// here is compiled in): world.json (schema ow-slice-world-v1, tools/prepare_slice_world.py), npc_assets.json and
// its sibling npc_identity.json (tools/slice_npc_assets.py) and audio.json (ow-audio-v1, tools/audio_slice_chain.py).
// Positions use the manifests' "host" blocks: the same transform tools/prepare_level.py applies to the loaded map
// (location as-is, rotation degrees [Pitch, Yaw, Roll]). Load() throws std::runtime_error on missing/invalid data.
struct FOpenWillowSliceNode
{
    FString Name;
    FVector Location = FVector::ZeroVector;
    float ArrivalRadius = 0;
    float SpeedPercentage = 1;
    TArray<FString> ArrivalEvents;       // Kismet event op names entered when the pawn arrives here
};

struct FOpenWillowSliceStation
{
    FString Object;
    FVector Location = FVector::ZeroVector;
    bool bCanResurrect = false;
    TArray<FTransform> Exits;           // exit points in data order
};

struct FOpenWillowNpcAssets
{
    FString Mesh, Idle, Walk, Death;    // UE object paths (Package.Asset)
    FVector MeshOffset = FVector::ZeroVector; // the pawn's SkeletalMeshComponent Translation
};

struct FOpenWillowSliceData
{
    // Range trigger: WillowWaypoint cylinder linked to the GoToRange objective.
    FString TriggerObject, TriggerObjective;
    FVector TriggerCenter = FVector::ZeroVector;
    float TriggerRadius = 0, TriggerHalfHeight = 0;
    // Marcus: placed pawn and the WillowSeqAct_AIScripted walk to the range.
    FVector MarcusLocation = FVector::ZeroVector;
    FRotator MarcusRotation = FRotator::ZeroRotator;
    float MarcusSpeed = 0, MarcusYawRate = 0;
    FString WalkOp;
    TArray<FOpenWillowSliceNode> Walk;
    TArray<FString> LookAtPlayerOps;    // ops on the walk's Finished output that set Flag_LookAtPlayer = true
    // Target dummy: population den/point of the Fire objective, attachment holder, target Matinee binding.
    FString DummyObjective, DenObject, PointObject, HolderObject;
    FVector DummyLocation = FVector::ZeroVector;
    FRotator DummyRotation = FRotator::ZeroRotator;
    TSharedPtr<FJsonObject> TargetBinding;
    // Attach point (world.json target_dummy.holder.attach, optional): the SeqAct_AttachToActor op, the holder's placed
    // pose, the holder SocketComponent the op's BoneName resolves to (pose on the holder) and the tool's own world
    // location for that socket (an oracle for the host's composition).
    bool bHasAttachSocket = false;
    FString AttachOp;
    FTransform HolderPose, AttachSocketLocal;
    FVector AttachSocketWorldOracle = FVector::ZeroVector;
    // Target names from the dummy's balance (playthrough 1 entry, optional): display name and the transformed name per
    // EAITransformed value (WillowAIPawn.GetTargetName; the per-type lookup is native, UNVERIFIED).
    bool bHasDummyNames = false;
    FString DummyDisplayName;
    TMap<FString, FString> DummyTransformedNames;
    // Respawn stations and Maya's health formula.
    TArray<FOpenWillowSliceStation> Stations;
    // The recovery tool's own answer for a death at the range trigger (an oracle for the host's selection).
    FVector OracleDeathLocation = FVector::ZeroVector, OracleRespawnLocation = FVector::ZeroVector;
    double HealthMultiplier = 0, HealthScaler = 0, HealthMin = 0;
    // Mission XP (values.xp): the reward attribute and its playthrough-1 percentage, the experience-required formula
    // f(L) = Multiplier x (L^Power + Offset), and the tool's own amounts and integer curve (oracles for MissionXp and
    // UOpenWillowSkills::ExperienceForLevel; the curve table is absent from manifests written before 2026-10-02).
    FString XpRewardAttribute;
    double XpPercentage = 0, XpMultiplier = 0, XpPower = 0, XpOffset = 0;
    TMap<int32, int32> XpCandidateByLevel;
    TMap<int32, int64> XpRequiredPointsByLevel;
    // The mission's GameStageRegion entry of GlobalsDefinition.RegionBalanceData, playthrough 1
    // (values.xp.region_stage, tools/slice_values.py). Manifests written before 2026-10-02 lack it: the host then uses
    // a labelled STAND-IN with the bounds NATIVE_PROGRESSION.md section 2 quotes (bRegionStageFromData false).
    struct FStageOverride { FString Mission; int32 Min = 0, Max = 0; };
    FString StageRegion;
    int32 StageBoost = 0, StageDefaultMin = 0, StageDefaultMax = 0;
    TArray<FStageOverride> StageOverrides;
    bool bRegionStageFromData = false;
    // Imported NPC assets and the audio lookup (key -> entry).
    FOpenWillowNpcAssets Marcus, Dummy;
    FString PistolMesh;                 // imported rolled sample of the lent Maliwan pistol (npc_assets use.MaliwanPistol)
    TMap<FString, TSharedPtr<FJsonObject>> Audio;

    void Load(const FString& WorldFile, const FString& NpcFile, const FString& AudioFile);
    // health(L) = max(min, multiplier x scaler^L); the attributes replace the 94 constant (NATIVE_PROGRESSION.md
    // section 4, read from native code, UNVERIFIED in game).
    float HealthForLevel(int32 Level) const;
    // Mission XP at mission level L (the region game stage): trunc(percentage x (R(L+1) - R(L))) on the integer curve
    // R from the manifest's formula, playthrough multiplier 1 (playthrough 1 below level 50). Read from native code
    // (NATIVE_PROGRESSION.md section 2), UNVERIFIED in game.
    int32 MissionXp(int32 MissionLevel) const;
    // Region game stage for a player level: the largest clamp(level + boost, min, max) over the mission overrides whose
    // mission is complete, else clamp(level + boost, default min, default max) (NATIVE_PROGRESSION.md section 2,
    // UNVERIFIED in game). Fixing the value the first time it is asked for is the caller's job.
    int32 RegionStage(int32 PlayerLevel, TFunctionRef<bool(const FString&)> IsComplete) const;
    // "Sanctuary_Dynamic:TheWorld.PersistentLevel.X" -> "TheWorld.PersistentLevel.X"; op name = last path part.
    static FString ObjectPath(const FString& Value);
    static FString OpName(const FString& Value);
};
