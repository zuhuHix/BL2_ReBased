#pragma once
#include "CoreMinimal.h"
#include "Components/ActorComponent.h"
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
private:
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
