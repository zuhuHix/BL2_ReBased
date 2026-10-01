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
    // required(L) = Multiplier x L^Power + Offset, and the tool's own candidate amounts (an oracle for MissionXp).
    FString XpRewardAttribute;
    double XpPercentage = 0, XpMultiplier = 0, XpPower = 0, XpOffset = 0;
    TMap<int32, int32> XpCandidateByLevel;
    // Imported NPC assets and the audio lookup (key -> entry).
    FOpenWillowNpcAssets Marcus, Dummy;
    FString PistolMesh;                 // imported rolled sample of the lent Maliwan pistol (npc_assets use.MaliwanPistol)
    TMap<FString, TSharedPtr<FJsonObject>> Audio;

    void Load(const FString& WorldFile, const FString& NpcFile, const FString& AudioFile);
    // health(L) = max(min, multiplier x scaler^L); see SLICE_WORLD_PLACEMENT.md 2b (94-constant reading UNVERIFIED).
    float HealthForLevel(int32 Level) const;
    // CANDIDATE mission XP at mission level L: percentage x (required(L+1) - required(L)), rounded.
    // MissionDefinition.GetExperienceReward is native, so this rule is UNVERIFIED (tools/slice_values.py).
    int32 MissionXp(int32 MissionLevel) const;
    // "Sanctuary_Dynamic:TheWorld.PersistentLevel.X" -> "TheWorld.PersistentLevel.X"; op name = last path part.
    static FString ObjectPath(const FString& Value);
    static FString OpName(const FString& Value);
};
