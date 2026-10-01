#pragma once
#include "CoreMinimal.h"
#include "GameFramework/Actor.h"
#include "OpenWillowNpc.generated.h"

// A placed NPC (Marcus on the slice route) with the imported skeletal mesh and clips. The quest component places
// it at the stock pose and drives it: it walks a move-node path handed over by the installed Kismet
// (WillowSeqAct_AIScripted) and reports each node it arrives at, so the caller can enter the matching
// SeqEvent_ArrivedAtMoveNode in the same sequence.
//
// Host movement model, all UNVERIFIED against the original (native PHYS_NavMeshWalking / AI code): straight lines
// between nodes at the data's GroundSpeed x the node's speed percentage, arrival when the planar distance is within
// the node's PawnArrivalRadius, yaw turned toward the travel direction (FocusStyle ESF_Path) at the class's yaw
// rate, height interpolated between the data's positions (no floor tracing), no collision, no SlowDownDist easing.
UCLASS()
class OPENWILLOW_API AOpenWillowNpc : public AActor
{
    GENERATED_BODY()
public:
    struct FNode { FVector Location; float ArrivalRadius = 0; float SpeedPercentage = 1; };
    AOpenWillowNpc();
    virtual void Tick(float DeltaSeconds) override;
    bool Setup(const FString& MeshPath, const FString& IdlePath, const FString& WalkPath, const FVector& MeshOffset);
    void StartWalk(const TArray<FNode>& Nodes, float GroundSpeed, float YawRateDegrees);
    // Node indices reached since the last call, in order.
    TArray<int32> TakeArrivals();
    bool IsWalking() const { return bWalking; }
    int32 NodesReached() const { return Reached; }
    void SetLookAt(AActor* Target) { LookAt = Target; }
    bool IsLookingAt(const AActor* Target, float ToleranceDegrees) const;
    bool IsPlaying(const class UAnimSequence* Anim) const { return Anim && Playing == Anim; }
    class UAnimSequence* Idle() const { return IdleAnim; }
    class UAnimSequence* WalkClip() const { return WalkAnim; }
    class USkeletalMeshComponent* GetMesh() const { return Mesh; }
private:
    void Play(class UAnimSequence* Anim);
    void TurnToward(const FVector& Direction, float DeltaSeconds);
    UPROPERTY() TObjectPtr<class USceneComponent> Root;
    UPROPERTY() TObjectPtr<class USkeletalMeshComponent> Mesh;
    UPROPERTY() TObjectPtr<class UAnimSequence> IdleAnim;
    UPROPERTY() TObjectPtr<class UAnimSequence> WalkAnim;
    UPROPERTY() TObjectPtr<class UAnimSequence> Playing;
    TWeakObjectPtr<AActor> LookAt;
    TArray<FNode> Path;
    TArray<int32> Arrivals;
    float Speed = 0;
    float YawRate = 0;
    int32 Next = 0;
    int32 Reached = 0;
    bool bWalking = false;
};
