// AI-assisted (Claude). The Quick Change station's respec: the use key near the placed machine (its mesh is already in
// the imported scene) resets the skill tree through UOpenWillowSkills::PurchaseReset and charges the money it takes.
// The customization menu itself is not built. The two-press confirmation stands in for its dialog (UNVERIFIED host choice):
// a respec costs money and wipes the tree, so a stray press must not do it.
#pragma once

#include "CoreMinimal.h"
#include "GameFramework/Actor.h"
#include "OpenWillowUseView.h"
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
    TWeakObjectPtr<const UObject> PromptedBy;   // the walker it asked; a destroyed walker's prompt can never confirm
    // True when this press confirms the open prompt (the prompt is used up). False when it opens one: there was none, it
    // expired, another walker pressed, or the price is no longer the one shown.
    bool Press(double Now, const UObject* Who, int32 Cost);
    // Why an open prompt must end now, or null when it may stay. Current: the current walker, null when there is none.
    const TCHAR* CancelReason(double Now, const UObject* Current, bool bCurrentIsDown, bool bSelected) const;
    bool IsOpen() const { return PromptedAt >= 0.0; }
    bool Expired(double Now) const { return IsOpen() && Now - PromptedAt > ConfirmSeconds; }
    void Cancel() { PromptedAt = -1.0; PromptedCost = 0; PromptedBy.Reset(); }
};

UCLASS()
class OPENWILLOW_API AOpenWillowQuickChangeStation : public AActor
{
    GENERATED_BODY()
public:
    AOpenWillowQuickChangeStation();
    void Configure(const FString& InId) { Id = InId; }
    // The imported machine in the level, other actors than this one: its own collision is what the use key's view ray must hit first. Null without one.
    void SetPlacedBody(const OpenWillowUseView::FPlacedBody& Body) { PlacedBody = Body; }
    const OpenWillowUseView::FPlacedBody* GetPlacedBody() const { return PlacedBody.Sections.Num() > 0 ? &PlacedBody : nullptr; }
    // The machine's bounds, where to aim and stand clear of. A map without that mesh gets a machine-sized box on the placed point, which the view ray then tests: UNVERIFIED stand-in.
    FBox UseBox() const
    {
        return PlacedBody.Sections.Num() > 0 ? PlacedBody.Box : FBox::BuildAABB(GetActorLocation() + FVector(0.f, 0.f, 105.f), FVector(60.f, 60.f, 105.f));
    }
    // One use press from this walker. True: the press was used, whatever it did.
    bool Use(AOpenWillowWalker& Walker);
    // Called often while a prompt may be open, with or without a current walker: it ends when it expires, when that walker
    // is gone or down, or when this station stops being its selected use target (bSelected false: walked out of reach, looked away, or
    // something else is now nearer). The label clears too.
    void TickPrompt(double Now, const AOpenWillowWalker* Walker, bool bSelected);
    // Ends an open prompt now: the press went elsewhere (Marcus's talk takes the use key before the nearest-target rule).
    void CancelPrompt(const TCHAR* Reason);

    FString Id;

private:
    UPROPERTY() TObjectPtr<UTextRenderComponent> Label;
    OpenWillowUseView::FPlacedBody PlacedBody;
    FOpenWillowQuickChangePress Press;
};
