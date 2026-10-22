#pragma once
#include "CoreMinimal.h"

class AOpenWillowWalker;
class APlayerController;

// The "minimap" field of the Flash HUD state (tools/hud_overlay/minimap.js maps it to the HUD movie's minimap clips;
// docs/verification/HUD_MINIMAP.md). The level's map movie, the map's centre and the stock icons (shops, health,
// travel and customization stations) come from the manifest tools/prepare_minimap.py writes, given by
// -owminimap=<json> (default <project>/../../../local/slice/minimap.json); the quest adds its mission icons. Without a
// manifest no field is sent and the movie's minimap stays as authored. Positions are world units (the host keeps the
// game's coordinates) and the yaw is the view's, in degrees, as the game's widget uses.
namespace OpenWillowMinimap
{
    // JSON object text for the field, or empty when there is no manifest. Logs "OWMINIMAP ..." once per change of the
    // sent state (quantized to 1 uu and 0.1 degree, the precision sent).
    FString StateField(const AOpenWillowWalker& Maya, const APlayerController& Player);

    // -owminimapshots (with -owflashhud): puts the player at the real-game capture spots and view yaws, captures the
    // screen with the HUD at each (OWMinimap_<n>.png), then quits. Call every HUD frame; does nothing without the flag.
    void TickShots(AOpenWillowWalker& Maya, APlayerController& Player);
}
