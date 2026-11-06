#include "OpenWillowUsePrompt.h"
#include "OpenWillowInteractive.h"
#include "OpenWillowQuest.h"
#include "OpenWillowWalker.h"
#include "HAL/PlatformTime.h"

OpenWillowUsePrompt::FPrompt OpenWillowUsePrompt::Describe(EOpenWillowUseTarget Target, bool bTalk, bool bRewardPending, const FString& InteractiveVerb)
{
    if (bRewardPending) return {};
    if (bTalk) return {TEXT("talk"), TEXT("[E] TALK")};
    switch (Target)
    {
    case EOpenWillowUseTarget::Vendor: return {TEXT("shop"), TEXT("[E] SHOP")};
    case EOpenWillowUseTarget::Door: return {TEXT("open"), TEXT("[E] OPEN")};
    case EOpenWillowUseTarget::Pickup: return {TEXT("pickup"), TEXT("[E] PICK UP")};
    case EOpenWillowUseTarget::Interactive:
        if (InteractiveVerb == TEXT("use")) return {TEXT("use"), TEXT("[E] USE")};
        if (InteractiveVerb == TEXT("pickup")) return {TEXT("pickup"), TEXT("[E] PICK UP")};
        if (InteractiveVerb == TEXT("open")) return {TEXT("open"), TEXT("[E] OPEN")};
        return {};
    default: return {};
    }
}

namespace
{
    const FString& SetUsePromptField(OpenWillowUsePrompt::FState& State, const FString& Next)
    {
        if (Next != State.Field)
        {
            UE_LOG(LogTemp, Display, TEXT("OWUSEPROMPT %s"), *Next);
            State.Field = Next;
        }
        return State.Field;
    }
}

FString OpenWillowUsePrompt::StateField(const AOpenWillowWalker& Maya, FState& State)
{
    const UOpenWillowQuest* Quest = Maya.GetQuest();
    const bool bReward = Quest && Quest->IsRewardPending();
    // These first, so a prompt does not outlast them; only the search below is throttled.
    if (!Maya.CanUseNow() || bReward) return SetUsePromptField(State, TEXT("null"));
    const double Now = FPlatformTime::Seconds();
    if (Now < State.NextSearchAt) return State.Field;
    State.NextSearchAt = Now + .1;
    const EOpenWillowUseTarget Target = Maya.NearestUseTarget();
    const FPrompt Prompt = Describe(Target, Quest && Quest->CanTalkNow(), false,
        Target == EOpenWillowUseTarget::Interactive ? UOpenWillowInteractiveSubsystem::UseVerb(Maya) : FString());
    return SetUsePromptField(State, Prompt.Frame ? FString::Printf(TEXT("{\"frame\":\"%s\",\"text\":\"%s\"}"), Prompt.Frame, Prompt.Text) : FString(TEXT("null")));
}
