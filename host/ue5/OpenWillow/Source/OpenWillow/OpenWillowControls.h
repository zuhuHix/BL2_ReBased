#pragma once
#include "CoreMinimal.h"

class AOpenWillowMayaHUD;
class AOpenWillowWalker;
class APlayerController;

// The player's basic verbs that have no other home (docs/verification/CONTROLS.md).
namespace OpenWillowControls
{
    // -owcontrolstest (tools/test_controls.ps1): presses the keys through the player controller's own input path, as the
    // inventory suite does, with every page closed: an open page holds a real keyboard, so a simulated press there would test
    // a path no player reaches. Logs an OWCONTROLS row per check and a summary, then quits (tools/check_controls.py grades
    // them). Call every HUD frame; does nothing without the flag. A pass says the host did this, nothing about the real game.
    void TickTest(AOpenWillowMayaHUD& Hud, AOpenWillowWalker& Maya, APlayerController& Player);
}
