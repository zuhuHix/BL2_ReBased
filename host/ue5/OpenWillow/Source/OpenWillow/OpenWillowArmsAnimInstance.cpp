#include "OpenWillowArmsAnimInstance.h"
#include "Animation/AnimSequence.h"
#include "Animation/AnimationAsset.h"
#include "AnimationRuntime.h"

namespace
{
constexpr float WalkSpeed = 450.f;
constexpr float SprintSpeed = 650.f;
constexpr float BlendSeconds = 0.12f;

void Advance(const UAnimSequence* Clip, float& Time, float DeltaSeconds, bool bLoop)
{
    if (!Clip) return;
    const float Length = Clip->GetPlayLength();
    if (Length <= 0) return;
    Time = bLoop ? FMath::Fmod(Time + DeltaSeconds, Length) : FMath::Min(Time + DeltaSeconds, Length);
}

void Sample(const UAnimSequence* Clip, float Time, bool bLoop, FPoseContext& Pose)
{
    FAnimationPoseData Data(Pose);
    Clip->GetAnimationPose(Data, FAnimExtractContext(Time, false, {}, bLoop));
}

void Mix(FPoseContext& Base, FPoseContext& Added, float AddedWeight)
{
    FAnimationPoseData BaseData(Base);
    const FAnimationPoseData AddedData(Added);
    FAnimationRuntime::BlendTwoPosesTogetherInPlace(BaseData, AddedData, 1.f - AddedWeight);
}
}

void UOpenWillowArmsAnimInstance::SetClips(UAnimSequence* InIdle, UAnimSequence* InRun,
    UAnimSequence* InSprint, UAnimSequence* InJump, UAnimSequence* InLand)
{
    Idle = InIdle;
    Run = InRun;
    Sprint = InSprint;
    Jump = InJump;
    Land = InLand;
}

void UOpenWillowArmsAnimInstance::SetMovement(float InGroundSpeed, bool bInFalling, bool bInLanding)
{
    GroundSpeed = InGroundSpeed;
    bFalling = bInFalling;
    bLanding = bInLanding;
}

void UOpenWillowArmsAnimInstance::PlayAction(UAnimSequence* InAction, float InWeight)
{
    Action = InAction;
    ActionWeight = FMath::Clamp(InWeight, 0.f, 1.f);
    ++ActionSerial;
}

FAnimInstanceProxy* UOpenWillowArmsAnimInstance::CreateAnimInstanceProxy()
{
    return new FOpenWillowArmsProxy(this);
}

void FOpenWillowArmsProxy::PreUpdate(UAnimInstance* Instance, float DeltaSeconds)
{
    FAnimInstanceProxy::PreUpdate(Instance, DeltaSeconds);
    const auto* Arms = CastChecked<UOpenWillowArmsAnimInstance>(Instance);
    Idle = Arms->Idle;
    Run = Arms->Run;
    Sprint = Arms->Sprint;
    Jump = Arms->Jump;
    DesiredOverlay = Arms->bFalling ? Arms->Jump.Get()
        : Arms->bLanding ? Arms->Land.Get() : nullptr;
    DesiredSpeed = Arms->GroundSpeed;
    DesiredAction = Arms->Action;
    DesiredActionSerial = Arms->ActionSerial;
    DesiredActionWeight = Arms->ActionWeight;
}

void FOpenWillowArmsProxy::UpdateAnimationNode(const FAnimationUpdateContext& Context)
{
    UpdateCounter.Increment();
    const float DeltaSeconds = Context.GetDeltaTime();
    SmoothedSpeed = FMath::FInterpTo(SmoothedSpeed, DesiredSpeed, DeltaSeconds, 12.f);
    Advance(Idle, IdleTime, DeltaSeconds, true);
    Advance(Run, RunTime, DeltaSeconds, true);
    Advance(Sprint, SprintTime, DeltaSeconds, true);

    if (DesiredOverlay && DesiredOverlay != Overlay)
    {
        PreviousOverlay = Overlay;
        PreviousOverlayTime = OverlayTime;
        Overlay = DesiredOverlay;
        OverlayTime = 0;
        OverlayCrossfade = PreviousOverlay ? 0.f : 1.f;
    }
    Advance(Overlay, OverlayTime, DeltaSeconds, Overlay == Jump);
    Advance(PreviousOverlay, PreviousOverlayTime, DeltaSeconds, PreviousOverlay == Jump);
    OverlayCrossfade = FMath::Min(1.f, OverlayCrossfade + DeltaSeconds / BlendSeconds);
    OverlayAlpha = FMath::FInterpConstantTo(OverlayAlpha,
        DesiredOverlay ? 1.f : 0.f, DeltaSeconds, 1.f / BlendSeconds);
    if (!DesiredOverlay && OverlayAlpha <= 0.f)
    {
        Overlay = nullptr;
        PreviousOverlay = nullptr;
    }
    if (DesiredActionSerial != ActiveActionSerial)
    {
        ActiveActionSerial = DesiredActionSerial;
        ActiveAction = DesiredAction;
        ActionTime = 0.f;
        ActionAlpha = 0.f;
    }
    if (ActiveAction)
    {
        Advance(ActiveAction, ActionTime, DeltaSeconds, false);
        const bool bEnded = ActionTime >= ActiveAction->GetPlayLength();
        ActionAlpha = FMath::FInterpConstantTo(ActionAlpha,
            bEnded ? 0.f : DesiredActionWeight, DeltaSeconds, 12.f);
        if (bEnded && ActionAlpha <= 0.f) ActiveAction = nullptr;
    }
}

bool FOpenWillowArmsProxy::Evaluate(FPoseContext& Output)
{
    if (!Idle)
    {
        Output.ResetToRefPose();
        return true;
    }
    Sample(Idle, IdleTime, true, Output);
    if (Run && SmoothedSpeed > 0.f)
    {
        FPoseContext RunPose(Output);
        Sample(Run, RunTime, true, RunPose);
        Mix(Output, RunPose, FMath::Clamp(SmoothedSpeed / WalkSpeed, 0.f, 1.f));
    }
    if (Sprint && SmoothedSpeed > WalkSpeed)
    {
        FPoseContext SprintPose(Output);
        Sample(Sprint, SprintTime, true, SprintPose);
        Mix(Output, SprintPose, FMath::Clamp((SmoothedSpeed - WalkSpeed) /
            (SprintSpeed - WalkSpeed), 0.f, 1.f));
    }
    if (Overlay && OverlayAlpha > 0.f)
    {
        FPoseContext OverlayPose(Output);
        Sample(Overlay, OverlayTime, Overlay == Jump, OverlayPose);
        if (PreviousOverlay && OverlayCrossfade < 1.f)
        {
            FPoseContext PreviousPose(Output);
            Sample(PreviousOverlay, PreviousOverlayTime, PreviousOverlay == Jump, PreviousPose);
            Mix(PreviousPose, OverlayPose, OverlayCrossfade);
            Mix(Output, PreviousPose, OverlayAlpha);
        }
        else
        {
            Mix(Output, OverlayPose, OverlayAlpha);
        }
    }
    if (ActiveAction && ActionAlpha > 0.f)
    {
        FPoseContext ActionPose(Output);
        Sample(ActiveAction, ActionTime, false, ActionPose);
        Mix(Output, ActionPose, ActionAlpha);
    }
    return true;
}
