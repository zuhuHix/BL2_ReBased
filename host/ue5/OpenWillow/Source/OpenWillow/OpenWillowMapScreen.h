#pragma once
#include "CoreMinimal.h"

class AOpenWillowMayaHUD;
class AOpenWillowWalker;
class APlayerController;

// The status menu's Map screen (tools/hud_overlay/map.html): BL2's StatusMenu movie on its "map" frame with the level's
// tactical map movie loaded into it, as StatusMenuMapGFxObject does (docs/verification/MAP_SCREEN.md). The page gets the
// same state the HUD minimap gets (OpenWillowMinimap::StateField: map movie, volume centre, fog blobs, Maya's position and
// view yaw, the stock and mission icons) plus the map's display name.
namespace OpenWillowMapScreen
{
    // JSON object text for window.owMap: {minimap: <the minimap field or null>, name}. The name is the map package's name
    // without its "_P" suffix, upper case (the game's SetMapHeader is native; UNVERIFIED).
    FString StateJson(const AOpenWillowWalker& Maya, const APlayerController& Player);

    // -owmapshots (with the status pages): opens the Map screen, captures it (OWMap_<n>_<name>.png), turns Maya 90 degrees
    // and captures again, zooms in, closes it with Escape, opens the Inventory and presses 2 (the Map tab), captures, and
    // quits. Call every HUD frame once the HUD movie is ready; does nothing without the flag.
    void TickShots(AOpenWillowMayaHUD& Hud, AOpenWillowWalker& Maya, APlayerController& Player);
}
