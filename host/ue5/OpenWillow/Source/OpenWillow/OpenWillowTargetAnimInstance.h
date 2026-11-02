#pragma once

#include "Animation/AnimInstance.h"
#include "Animation/AnimInstanceProxy.h"
#include "OpenWillowTargetAnimInstance.generated.h"

class UAnimSequence;

// One clip at a time with a linear crossfade from the previous one, for an imported stock pawn (the Phaselock test target,
// the slice dummy). UE5's single-clip player cannot blend, and the stock special moves blend (SpecialMove_PhaseLock: lift in
// over 0.4 s, the others 0.1 s; NATIVE_PHASELOCK_PRESENTATION.md section 2.3). The blend curve shape is not read (linear).
UCLASS(Transient)
class OPENWILLOW_API UOpenWillowTargetAnimInstance : public UAnimInstance
{
    GENERATED_BODY()
public:
    // Starts Clip at time 0, crossfading from what plays now over BlendIn seconds (0 = cut).
    void Play(UAnimSequence* Clip, bool bLoop, float Rate = 1.f, float BlendIn = 0.f);

    UPROPERTY(Transient) TObjectPtr<UAnimSequence> Clip;
    bool bLoop = false;
    float Rate = 1.f;
    float BlendIn = 0.f;
    int32 Serial = 0;

protected:
    virtual FAnimInstanceProxy* CreateAnimInstanceProxy() override;
};

struct FOpenWillowTargetProxy : FAnimInstanceProxy
{
    FOpenWillowTargetProxy(UAnimInstance* Instance) : FAnimInstanceProxy(Instance) {}
    virtual void PreUpdate(UAnimInstance* Instance, float DeltaSeconds) override;
    virtual void UpdateAnimationNode(const FAnimationUpdateContext& Context) override;
    virtual bool Evaluate(FPoseContext& Output) override;

private:
    struct FPlaying
    {
        const UAnimSequence* Clip = nullptr;
        float Time = 0;
        float Rate = 1;
        bool bLoop = false;
    };
    FPlaying Current, Previous;
    float Alpha = 1;        // weight of Current against Previous
    float BlendIn = 0;
    int32 Serial = 0;
    // Copied in PreUpdate.
    const UAnimSequence* DesiredClip = nullptr;
    bool bDesiredLoop = false;
    float DesiredRate = 1;
    float DesiredBlendIn = 0;
    int32 DesiredSerial = 0;
};
