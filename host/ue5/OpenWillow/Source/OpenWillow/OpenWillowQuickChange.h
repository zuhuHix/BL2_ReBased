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

// What one use press does: the first asks, a second within ConfirmSeconds confirms. Plain data for the automation test.
struct FOpenWillowQuickChangePress
{
    static constexpr double ConfirmSeconds = 5.0;
    double PromptedAt = -1.0;   // world time of the last unanswered prompt, negative when there is none
    // True when this press confirms an open prompt (the prompt is used up); false when it opens one.
    bool Press(double Now);
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

    FString Id;

private:
    UPROPERTY() TObjectPtr<UTextRenderComponent> Label;
    FOpenWillowQuickChangePress Press;
};
