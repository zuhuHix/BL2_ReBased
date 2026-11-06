#pragma once
#include "CoreMinimal.h"

class AOpenWillowMayaHUD;
class AOpenWillowWalker;
class APlayerController;

// The player's basic verbs that have no other home (docs/verification/CONTROLS.md).
namespace OpenWillowControls
{
    // -owcontrolstest (tools/test_controls.ps1): presses the keys through the player controller's own input path, as the
    // inventory suite does, logs an OWCONTROLS row per check and a summary, then quits. Call every HUD frame; does nothing
    // without the flag. A pass says the host did this; it says nothing about the real game.
    void TickTest(AOpenWillowMayaHUD& Hud, AOpenWillowWalker& Maya, APlayerController& Player);
}
