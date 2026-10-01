#pragma once
#include "CoreMinimal.h"
#include "Components/ActorComponent.h"
#include "Dom/JsonObject.h"
#include "mover.hpp"
#include "OpenWillowMover.generated.h"

// A world-acting Kismet op reached in the installed sequence that this component does not run itself
// (AIScripted, AttachToActor, Destroy...). The quest component executes the ones it binds.
struct FOpenWillowKismetRequest
{
    FString Class, Op, Input;
};

UCLASS()
class OPENWILLOW_API UOpenWillowMover : public UActorComponent
{
    GENERATED_BODY()
public:
    UOpenWillowMover();
    virtual void BeginPlay() override;
    virtual void EndPlay(const EEndPlayReason::Type Reason) override;
    virtual void TickComponent(float Delta, ELevelTick Type, FActorComponentTickFunction* Function) override;
    bool TryInteract();
    // Stock activation: a remote event through the installed Kismet sequence owning the door's Matinee action.
    // Returns whether any event node in the sequence matched. Unsupported ops fail the component explicitly.
    bool RemoteEvent(const FString& Name);
    // A mission behavior's remote event (the Kismet nodes bound to that mission definition path).
    bool MissionEvent(const FString& MissionPath, const FString& Name);
    // Host entry points into the same installed sequence (see vm::Mover).
    bool SequenceEvent(const FString& OpName);
    bool OriginatorEvent(const FString& ObjectPath, TArray<FString>& Entered);
    bool SequenceOutput(const FString& OpName, const FString& Desc);
    TArray<vm::Mover::Variable> SequenceVariables(const FString& OpName, const FString& Desc);
    // World ops reached since the last call that neither the door nor the bound track ran.
    TArray<FOpenWillowKismetRequest> DrainRequests();
    // Binds a second Matinee action of the same sequence (ow-mover-binding-v1, one group) to the loaded scene: its
    // group actor's mesh moves, the actors attached to it in the data follow. Throws std::runtime_error.
    void BindTrack(const TSharedPtr<FJsonObject>& Binding);
    class UStaticMeshComponent* TrackCarrier() const { return TrackMesh; }
    bool TrackRunning() const { return bTrackRunning; }
    FVector TrackOffset() const;                  // carrier location minus its placed location
    int32 TrackForwardEnds = 0, TrackReverseEnds = 0;
    // Closed-position centre of the door and a standing point beside it (test/host positioning).
    bool Anchor(FVector& Out) const;
    bool StandPoint(FVector& Out) const;
    bool DoorRunning() const { return Running; }
    FVector DoorPlacedLocation() const { return Initial.GetLocation(); }
    int32 LastEventMatched = 0;
    int32 LastEventBoundary = 0;
    int32 LastEventMotion = 0;
    int32 DoorOpenStarts = 0, DoorCloseStarts = 0, DoorOpenEnds = 0, DoorCloseEnds = 0, DoorTurnArounds = 0;
    bool DoorClosed() const { return !Running && Time <= 0; }
private:
    void StartMotion(bool NextReverse);
    bool ApplyDispatch(const vm::Mover::Dispatch& Dispatch, const FString& Name);
    void StartTrack(bool bReverse);
    void RestoreTrack();
    void TickTrack(float Delta);
    void FireTrackKeys(float From, float To, bool bInclusiveFrom);
    struct FImpl;
    TSharedPtr<FImpl> Impl;
    void Fail(const FString& Error);
    void RunTest(float Delta);
    void Check(bool Good, const TCHAR* Name);
    bool Ray(bool ClosedSpace) const;
    TEnumAsByte<EComponentMobility::Type> OriginalMobility;
    FKey PendingRelease;
    bool HasPendingRelease = false;
    bool PoseCaptured = false;
    bool Failed = false;
    bool Testing = false;
    bool Running = false;
    bool Reverse = false;
    float Time = 0;
    float Duration = 0;
    float TestWait = 0;
    int32 TestStep = 0;
    int32 Checks = 0;
    int32 Errors = 0;
    int64 ScriptSteps = 0;
    FTransform Initial;
    FVector RayStart, RayEnd;
    UPROPERTY() TObjectPtr<class UStaticMeshComponent> Mesh;
    // Second bound Matinee (the target carrier on the slice route).
    UPROPERTY() TObjectPtr<class UStaticMeshComponent> TrackMesh;
    FTransform TrackInitial;
    bool bTrackRunning = false;
    bool bTrackReverse = false;
    float TrackTime = 0;
};
