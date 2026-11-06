// AI-assisted (Claude). The Quick Change station's respec: the use key near the placed machine (its mesh is already in
// the imported scene) resets the skill tree through UOpenWillowSkills::PurchaseReset and charges the money it takes.
// The customization menu itself is not built. The two-press confirmation stands in for its dialog (UNVERIFIED host choice):
// a respec costs money and wipes the tree, so a stray press must not do it.
#pragma once

#include "CoreMinimal.h"
#include "GameFramework/Actor.h"
#include "OpenWillowQuickChange.generated.h"

class AOpenWillowWalker;
class UTextRenderComponent;

// What one use press does: the first asks, a second within ConfirmSeconds confirms, but only from the same walker at the
// price that was shown. Plain data for the automation test.
struct FOpenWillowQuickChangePress
{
    static constexpr double ConfirmSeconds = 5.0;
    double PromptedAt = -1.0;           // world time of the open prompt, negative when there is none
    int32 PromptedCost = 0;             // the price the prompt showed
    const void* PromptedBy = nullptr;   // the walker it asked
    // True when this press confirms the open prompt (the prompt is used up). False when it opens one: there was none, it
    // expired, another walker pressed, or the price is no longer the one shown.
    bool Press(double Now, const void* Who, int32 Cost);
    bool IsOpen() const { return PromptedAt >= 0.0; }
    bool Expired(double Now) const { return IsOpen() && Now - PromptedAt > ConfirmSeconds; }
    void Cancel() { PromptedAt = -1.0; PromptedCost = 0; PromptedBy = nullptr; }
};

UCLASS()
class OPENWILLOW_API AOpenWillowQuickChangeStation : public AActor
{
    GENERATED_BODY()
public:
    AOpenWillowQuickChangeStation();
    void Configure(const FString& InId) { Id = InId; }
    // One use press from this walker. True: the press was used, whatever it did.
    bool Use(AOpenWillowWalker& Walker);
    // Called often while a prompt may be open: it ends when it expires, or when this station stops being that walker's
    // selected use target (bSelected false: walked out of reach, or something else is now nearer). The label clears too.
    void TickPrompt(double Now, const AOpenWillowWalker& Walker, bool bSelected);

    FString Id;

private:
    UPROPERTY() TObjectPtr<UTextRenderComponent> Label;
    FOpenWillowQuickChangePress Press;
};
