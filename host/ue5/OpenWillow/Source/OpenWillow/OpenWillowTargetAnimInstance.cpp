#include "OpenWillowTargetAnimInstance.h"
#include "Animation/AnimSequence.h"
#include "AnimationRuntime.h"

void UOpenWillowTargetAnimInstance::Play(UAnimSequence* InClip, bool bInLoop, float InRate, float InBlendIn)
{
    Clip = InClip;
    bLoop = bInLoop;
    Rate = InRate;
    BlendIn = InBlendIn;
    ++Serial;
}

FAnimInstanceProxy* UOpenWillowTargetAnimInstance::CreateAnimInstanceProxy()
{
    return new FOpenWillowTargetProxy(this);
}

void FOpenWillowTargetProxy::PreUpdate(UAnimInstance* Instance, float DeltaSeconds)
{
    FAnimInstanceProxy::PreUpdate(Instance, DeltaSeconds);
    const auto* Target = CastChecked<UOpenWillowTargetAnimInstance>(Instance);
    DesiredClip = Target->Clip;
    bDesiredLoop = Target->bLoop;
    DesiredRate = Target->Rate;
    DesiredBlendIn = Target->BlendIn;
    DesiredSerial = Target->Serial;
}

void FOpenWillowTargetProxy::UpdateAnimationNode(const FAnimationUpdateContext& Context)
{
    UpdateCounter.Increment();
    const float DeltaSeconds = Context.GetDeltaTime();
    if (DesiredSerial != Serial)
    {
        Serial = DesiredSerial;
        // The pose blended from is what is on screen now: the current clip, or the previous one if the switch is early.
        Previous = Alpha >= 0.5f || !Previous.Clip ? Current : Previous;
        Current = {DesiredClip, 0.f, DesiredRate, bDesiredLoop};
        BlendIn = DesiredBlendIn;
        Alpha = BlendIn > 0.f && Previous.Clip ? 0.f : 1.f;
    }
    auto Advance = [DeltaSeconds](FPlaying& P)
    {
        if (!P.Clip) return;
        const float Length = P.Clip->GetPlayLength();
        if (Length <= 0) return;
        const float Time = P.Time + DeltaSeconds * P.Rate;
        P.Time = P.bLoop ? FMath::Fmod(Time, Length) : FMath::Min(Time, Length);   // non-looping clips hold the last frame
    };
    Advance(Current);
    if (Alpha < 1.f) Advance(Previous);
    Alpha = BlendIn > 0.f ? FMath::Min(1.f, Alpha + DeltaSeconds / BlendIn) : 1.f;
}

bool FOpenWillowTargetProxy::Evaluate(FPoseContext& Output)
{
    if (!Current.Clip)
    {
        Output.ResetToRefPose();
        return true;
    }
    auto Sample = [](const FPlaying& P, FPoseContext& Pose)
    {
        FAnimationPoseData Data(Pose);
        P.Clip->GetAnimationPose(Data, FAnimExtractContext(double(P.Time), false, {}, P.bLoop));
    };
    Sample(Current, Output);
    if (Alpha < 1.f && Previous.Clip)
    {
        FPoseContext From(Output);
        Sample(Previous, From);
        FAnimationPoseData OutputData(Output);
        const FAnimationPoseData FromData(From);
        FAnimationRuntime::BlendTwoPosesTogetherInPlace(OutputData, FromData, Alpha);
    }
    return true;
}
