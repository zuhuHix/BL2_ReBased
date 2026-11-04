#pragma once
#include "CoreMinimal.h"

class AOpenWillowMayaHUD;

// -owstatusshots=<missions|map|skills|inventory>[,<name>]: once the HUD movie is ready, opens that status page, captures the
// screen four seconds later (OWStatus_<page>_<name>.png) and quits. For checking a page in a state a save sets up (for
// example the slice mission resumed with -owquestresume). Call every HUD frame; does nothing without the flag.
namespace OpenWillowStatusShots
{
    void Tick(AOpenWillowMayaHUD& Hud, UWorld& World);
}
