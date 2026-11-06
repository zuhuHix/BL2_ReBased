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

    // What to show. bTalk: Marcus would take the press, which outranks the nearest-target rule (PickupNearby). InteractiveVerb is
    // UOpenWillowInteractiveSubsystem::UseVerb: "use", "pickup" or "open".
    FPrompt Describe(EOpenWillowUseTarget Target, bool bTalk, const FString& InteractiveVerb);

    // JSON text for the "usePrompt" field of the Flash HUD state: null, or {frame, text}. Null while Maya is down (the key does nothing
    // then) or a menu is up. The target is searched at most 10 times a second. Logs "OWUSEPROMPT ..." once per change.
    FString StateField(const AOpenWillowWalker& Maya);
}
