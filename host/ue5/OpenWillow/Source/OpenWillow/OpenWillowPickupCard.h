#pragma once
#include "CoreMinimal.h"

class AOpenWillowInventoryPickup;
class AOpenWillowWalker;
class APlayerController;

// The world pickup card: the item card BL2 shows while the player looks at a dropped item. In the game it is its own
// movie (ItemPickupGFxMovie, UI_Inworld.Weapon_Card: the shared "item card" clip at inventory.card1), rendered to a
// texture that the native HUD places near the item (WillowHUD.ShowPickupCard / GetPickupCardLocation, not read). The
// item it shows is the touched pickup, else the seen one (WillowPlayerController.GetCurrentPickupable). Here the HUD page
// draws the converted movie (tools/hud_overlay/pickup_card.js) from the "pickupCard" field of the Flash HUD state.
namespace OpenWillowPickupCard
{
    // The pickup Maya aims at: within the reach PickupNearby uses (220 uu from Maya), within a small cone around the view
    // direction and in sight of the view (a visibility trace that ignores Maya and the pickup). Ties go by angle, then
    // distance from Maya, then name, so the choice never depends on actor order. The one selection rule for the card and,
    // later, for E. Reach and cone are host choices (UNVERIFIED: the game's look-at test is native). Null when none.
    AOpenWillowInventoryPickup* FindAimedPickup(const AOpenWillowWalker& Maya, const APlayerController& Player);

    // The sight test: a visibility trace from the view to the pickup, ignoring Maya and the pickup. E uses it for every
    // pickup it may take, aimed or nearest.
    bool InSight(const AOpenWillowWalker& Maya, const APlayerController& Player, const AOpenWillowInventoryPickup& Pickup);

    // JSON text for the "pickupCard" field: null (also while Maya is down or dead), or {id, level, item, compare, x, y}.
    // `item` and `compare` are the inventory page's item objects (UOpenWillowInventory::WeaponItemJson / GearItemJson);
    // `compare` is the equipped analogue (the held weapon, or the equipped gear of the item's slot) or null. x and y are
    // the item's screen point as fractions of the viewport. The target is searched at most 10 times a second and the card
    // body rebuilt only when the target, the analogue or the level changes. Logs "OWPICKUPCARD ..." once per change.
    FString StateField(const AOpenWillowWalker& Maya, const APlayerController& Player);

    // -owpickupcardshots (with the movie HUD): rolls a gun with ow.RollGun, drops it ahead of Maya, captures the screen
    // aimed at it, aimed away, aimed again and after E picked it up; then the decoy case (a gun 70 uu to her right and one
    // 150 uu ahead, aimed at: E must take the aimed one, logged "decoy aimed_taken=1 nearer_left=1"); then E with a test
    // block between the view and the decoy (logged "wall decoy_left=1"), and again with the block gone (logged "wall_removed
    // decoy_taken=1"); then a last gun aimed at and Maya put down (the card must hide). Captures OWPickupCard_<n>_<name>.png and quits. Call every HUD frame once
    // the movie is ready; does nothing without the flag.
    void TickShots(AOpenWillowWalker& Maya, APlayerController& Player);
}
