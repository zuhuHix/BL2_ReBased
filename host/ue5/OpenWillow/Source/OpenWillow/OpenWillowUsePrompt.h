#pragma once
#include "CoreMinimal.h"

class AOpenWillowWalker;
enum class EOpenWillowUseTarget : uint8;

// The HUD's use prompt: what the use key would do now, on the movie's own prompt clip (p1.tools; docs/verification/CONTROLS.md, "Use
// prompt"). It only reads the use-target choice and never changes what the key does. The frames and words are the stock icons' from the
// game's data; which icon the game gives each target, and how it writes the key, are UNVERIFIED until the real-game read.
namespace OpenWillowUsePrompt
{
    struct FPrompt
    {
        const TCHAR* Frame = nullptr;   // a frame label of p1.tools; null: no prompt
        const TCHAR* Text = nullptr;    // for its desc field
    };

    // What to show, in the order the key goes: a reward page up takes the press and has its own wording, so no world prompt; then Marcus,
    // who outranks the nearest-target rule (PickupNearby); then the target. InteractiveVerb is UOpenWillowInteractiveSubsystem::UseVerb:
    // "use", "pickup" or "open".
    FPrompt Describe(EOpenWillowUseTarget Target, bool bTalk, bool bRewardPending, const FString& InteractiveVerb);

    // The last field, kept by its owner so it ends with the world.
    struct FState
    {
        double NextSearchAt = 0.;
        FString Field = TEXT("null");
    };

    // JSON text for the "usePrompt" field of the Flash HUD state: null, or {frame, text}. Null at once when the use key would do nothing
    // (AOpenWillowWalker::CanUseNow, the key's own check) or a reward page is up; the target itself is searched at most 10 times a
    // second. Logs "OWUSEPROMPT ..." once per change.
    FString StateField(const AOpenWillowWalker& Maya, FState& State);
}
