#pragma once

#include "Animation/AnimInstance.h"
#include "Animation/AnimInstanceProxy.h"
#include "OpenWillowArmsAnimInstance.generated.h"

class UAnimSequence;

// The imported UE3 clips are local payloads. Our pose selection and blending
// live here so the walk test does not depend on a hand-built Anim Blueprint.
UCLASS(Transient)
class OPENWILLOW_API UOpenWillowArmsAnimInstance : public UAnimInstance
{
    GENERATED_BODY()
public:
    void SetClips(UAnimSequence* InIdle, UAnimSequence* InRun, UAnimSequence* InSprint,
        UAnimSequence* InJump, UAnimSequence* InLand);
    void SetMovement(float InGroundSpeed, bool bInFalling, bool bInLanding);

    UPROPERTY(Transient) TObjectPtr<UAnimSequence> Idle;
    UPROPERTY(Transient) TObjectPtr<UAnimSequence> Run;
    UPROPERTY(Transient) TObjectPtr<UAnimSequence> Sprint;
    UPROPERTY(Transient) TObjectPtr<UAnimSequence> Jump;
    UPROPERTY(Transient) TObjectPtr<UAnimSequence> Land;
    float GroundSpeed = 0;
    bool bFalling = false;
    bool bLanding = false;

protected:
    virtual FAnimInstanceProxy* CreateAnimInstanceProxy() override;
};

struct FOpenWillowArmsProxy : FAnimInstanceProxy
{
    FOpenWillowArmsProxy(UAnimInstance* Instance) : FAnimInstanceProxy(Instance) {}
    virtual void PreUpdate(UAnimInstance* Instance, float DeltaSeconds) override;
    virtual void UpdateAnimationNode(const FAnimationUpdateContext& Context) override;
    virtual bool Evaluate(FPoseContext& Output) override;

private:
    const UAnimSequence* Idle = nullptr;
    const UAnimSequence* Run = nullptr;
    const UAnimSequence* Sprint = nullptr;
    const UAnimSequence* Jump = nullptr;
    const UAnimSequence* DesiredOverlay = nullptr;
    const UAnimSequence* Overlay = nullptr;
    const UAnimSequence* PreviousOverlay = nullptr;
    float DesiredSpeed = 0;
    float SmoothedSpeed = 0;
    float IdleTime = 0;
    float RunTime = 0;
    float SprintTime = 0;
    float OverlayTime = 0;
    float PreviousOverlayTime = 0;
    float OverlayAlpha = 0;
    float OverlayCrossfade = 1;
};
