#pragma once
#include "CoreMinimal.h"
#include "GameFramework/Actor.h"
#include "OpenWillowAmbient.generated.h"

class UAnimSequence;
class USkeletalMeshComponent;

// Sanctuary ambient NPCs: the citizens, a Resistance fighter and others that stand at "perches" and walk the town's
// move-node graph. Everything game-derived is read at run time from the ignored manifest written by
// tools/prepare_ambient_world.py (schema ow-ambient-world-v1): spawn points, the weighted node graph, perch
// definitions and the imported meshes/clips. Nothing here is compiled in. Positions use the manifest's host
// transform (location as-is, yaw in degrees), like OpenWillowSliceData.
//
// Host movement model, all UNVERIFIED against the original (the population system, Action_ScriptedNPC and
// PHYS_NavMeshWalking are native): a pawn is placed at its spawn point, walks straight lines to its first
// destination node at the AI class's GroundSpeed x WalkingPct, and at a perch node plays the perch definition's start
// clip, loops its idle clips for a random time inside the definition's LoopTime (or the node's override), plays the
// stop clip and moves to one of the node's NextNodes chosen by weight. "Idle" pawns (PopDef_NPC_ScriptedIdle) stay at
// their first perch. Floors come from a visibility trace, not from navigation data.
struct FOpenWillowAmbientNode
{
    FString Name;
    FVector Location = FVector::ZeroVector;
    float Yaw = 0;                  // degrees, from the node's rotation (used when bFaceNodeDirection)
    float ArrivalRadius = 32;
    bool bFaceNodeDirection = false;
    FString Perch;                  // perch definition name, empty for a plain move node
    FVector2D LoopOverride = FVector2D(-1, -1);   // node-level LoopTimeOverride, (-1,-1) when absent
    TArray<TPair<int32, float>> Next;   // node index, weight
};

struct FOpenWillowAmbientPerch
{
    FString Start, Stop;            // clip roles (empty when the definition has none)
    TArray<TPair<FString, float>> Idle;   // clip role, weight (SpecialMove_PerchRandomLoop)
    FVector2D LoopTime = FVector2D(3, 5);
    float LerpTime = 0.2f;          // seconds to ease onto the perch pose
};

struct FOpenWillowAmbientKind
{
    FString DisplayName, Mesh;
    FVector MeshOffset = FVector::ZeroVector;
    float Speed = 294, YawRate = 90;
    TMap<FString, FString> Clips;   // role -> UE AnimSequence path
};

struct FOpenWillowAmbientSpawn
{
    FString Id, Den, Population, Kind;
    FVector Location = FVector::ZeroVector;
    float Yaw = 0;
    int32 StartNode = INDEX_NONE;
    bool bWander = false;           // false = "Perch Only AI" (snapped to the first perch, stays there)
    bool bHold = false;             // stand and idle where spawned (observed in the real game)
    bool bLoadBalanced = false;     // Action_ScriptedNPC variant with bLoadBalanceNPC: paths only when the balancer admits it
};

struct FOpenWillowAmbientView
{
    FString Name;
    FVector Camera = FVector::ZeroVector, Look = FVector::ZeroVector;
};

struct FOpenWillowAmbientWorld
{
    TMap<FString, FOpenWillowAmbientKind> Kinds;
    TMap<FString, FOpenWillowAmbientPerch> Perches;
    TArray<FOpenWillowAmbientNode> Nodes;
    TArray<FOpenWillowAmbientSpawn> Spawns;
    TArray<FOpenWillowAmbientView> Views;
    TArray<FString> Showcase;       // spawn ids the capture tour visits (one camera each, chosen by tracing)
    // Throws std::runtime_error on missing or invalid data.
    void Load(const FString& File);
};

UCLASS()
class OPENWILLOW_API AOpenWillowAmbientNpc : public AActor
{
    GENERATED_BODY()
public:
    AOpenWillowAmbientNpc();
    virtual void Tick(float DeltaSeconds) override;
    bool Setup(TSharedPtr<const FOpenWillowAmbientWorld> InWorld, const FOpenWillowAmbientSpawn& Spawn);
    // Counters for the self-test summary.
    int32 NodesReached() const { return Reached; }
    int32 PerchCycles() const { return PerchStops; }
    float DistanceWalked() const { return Walked; }
    FString Phase() const;
    FString Label() const { return SpawnData.Id; }
    const FString& KindName() const { return SpawnData.Kind; }
    bool IsPlayingRole(const FString& RoleName) const;
    // NPCLoadBalancer stand-in (see the director): a waiting pawn stands still until admitted to path.
    bool IsHeld() const { return SpawnData.bHold; }
    bool IsWaiting() const { return State == EPhase::Waiting; }
    bool IsPathing() const { return State == EPhase::Walking && SpawnData.bLoadBalanced; }
    float WaitedSeconds() const { return PhaseTime; }
    void AdmitToPath();
private:
    enum class EPhase : uint8 { Waiting, Walking, PerchStart, PerchLoop, PerchStop, Holding };
    void Play(const FString& RoleName, bool bLoop);
    UAnimSequence* Clip(const FString& RoleName) const;
    void SnapToFloor(float DeltaSeconds, bool bImmediate);
    void TurnToward(float WantedYaw, float DeltaSeconds);
    void EaseOntoPerch();
    void Arrive();
    void BeginLoop();
    void AfterPerch();
    void BeginTravel();
    int32 PickNext() const;
    float LoopSeconds() const;
    UPROPERTY() TObjectPtr<USceneComponent> Root;
    UPROPERTY() TObjectPtr<USkeletalMeshComponent> Mesh;
    UPROPERTY() TMap<FString, TObjectPtr<UAnimSequence>> Loaded;
    TSharedPtr<const FOpenWillowAmbientWorld> World;
    FOpenWillowAmbientSpawn SpawnData;
    const FOpenWillowAmbientKind* Kind = nullptr;
    EPhase State = EPhase::Holding;
    FString PlayingRole;
    int32 Target = INDEX_NONE;
    float PhaseTime = 0, PhaseLength = 0, DwellLeft = 0;
    int32 Cycles = 0;               // idle-clip cycles started (seeds the clip choice)
    float FloorZ = 0;
    bool bHaveFloor = false, bLoggedJump = false;
    int32 Reached = 0, PerchStops = 0;
    float Walked = 0;
    FVector LerpFrom = FVector::ZeroVector;
    float LerpFromYaw = 0;
};

// Spawns the manifest's pawns (-owambient=<file> or OPENWILLOW_AMBIENT) and, for a capture run
// (-owambientshots), walks the player through the manifest's viewpoints taking screenshots. -owambienttest prints
// a summary after a fixed time and exits, like the quest suite.
UCLASS()
class OPENWILLOW_API AOpenWillowAmbientDirector : public AActor
{
    GENERATED_BODY()
public:
    AOpenWillowAmbientDirector();
    virtual void Tick(float DeltaSeconds) override;
    // False when no manifest was named; throws nothing (errors are logged and the director stays empty).
    static AOpenWillowAmbientDirector* SpawnIfRequested(UWorld* InWorld);
    const TArray<TObjectPtr<AOpenWillowAmbientNpc>>& Npcs() const { return Pawns; }
private:
    bool Start(const FString& File);
    void RunShots(float DeltaSeconds);
    void Summary();
    void Balance(float DeltaSeconds);
    bool ChooseCamera(const AOpenWillowAmbientNpc* Npc, FVector& OutCamera, FString& OutNote, bool bSide = false) const;
    TSharedPtr<FOpenWillowAmbientWorld> World;
    UPROPERTY() TArray<TObjectPtr<AOpenWillowAmbientNpc>> Pawns;
    float Age = 0, BalanceClock = 0;
    bool bTest = false, bShots = false, bSummaryDone = false;
    float TestSeconds = 60;
    int32 ShotStep = -1;
    float ShotClock = 0;
    int32 LastShot = -1;
    bool bSkipView = false;
    TWeakObjectPtr<AOpenWillowAmbientNpc> Current, PreviousWalker;
};
