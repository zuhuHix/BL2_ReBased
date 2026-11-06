#include "OpenWillowUsePrompt.h"
#include "OpenWillowInteractive.h"
#include "OpenWillowQuest.h"
#include "OpenWillowWalker.h"
#include "HAL/PlatformTime.h"

OpenWillowUsePrompt::FPrompt OpenWillowUsePrompt::Describe(EOpenWillowUseTarget Target, bool bTalk, const FString& InteractiveVerb)
{
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

FString OpenWillowUsePrompt::StateField(const AOpenWillowWalker& Maya)
{
    static double NextSearchAt = 0.;
    static FString Field = TEXT("null");
    const double Now = FPlatformTime::Seconds();
    if (Now < NextSearchAt) return Field;
    NextSearchAt = Now + .1;

    FPrompt Prompt;
    if (!Maya.IsDown() && !Maya.IsInventoryPresented())
    {
        const EOpenWillowUseTarget Target = Maya.NearestUseTarget();
        const UOpenWillowQuest* Quest = Maya.GetQuest();
        Prompt = Describe(Target, Quest && Quest->CanTalkNow(), Target == EOpenWillowUseTarget::Interactive ? UOpenWillowInteractiveSubsystem::UseVerb(Maya) : FString());
    }
    const FString Next = Prompt.Frame ? FString::Printf(TEXT("{\"frame\":\"%s\",\"text\":\"%s\"}"), Prompt.Frame, Prompt.Text) : FString(TEXT("null"));
    if (Next != Field)
    {
        UE_LOG(LogTemp, Display, TEXT("OWUSEPROMPT %s"), *Next);
        Field = Next;
    }
    return Field;
}
