#pragma once
#include "CoreMinimal.h"
#include "Components/ActorComponent.h"
#include "mover.hpp"
#include "OpenWillowMover.generated.h"

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
    // Closed-position centre of the door and a standing point beside it (test/host positioning).
    bool Anchor(FVector& Out) const;
    bool StandPoint(FVector& Out) const;
    int32 LastEventMatched = 0;
    int32 LastEventBoundary = 0;
private:
    void StartMotion(bool NextReverse);
    bool ApplyDispatch(const vm::Mover::Dispatch& Dispatch, const FString& Name);
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
};
