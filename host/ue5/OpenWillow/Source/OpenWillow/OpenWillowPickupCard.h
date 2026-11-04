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
    // The pickup Maya aims at: within the reach PickupNearby uses (220 uu from Maya) and within a small cone around the
    // view direction, the one nearest the view's centre line. The cone is a host choice (UNVERIFIED: the game's look-at
    // test is native). Null when none.
    AOpenWillowInventoryPickup* FindAimedPickup(const AOpenWillowWalker& Maya, const APlayerController& Player);

    // JSON text for the "pickupCard" field: null, or {id, x, y, level, item, compare}. `item` and `compare` are the
    // inventory page's item objects (UOpenWillowInventory::WeaponItemJson / GearItemJson); `compare` is the equipped
    // analogue (the held weapon, or the equipped gear of the item's slot) or null. x and y are the item's screen point
    // as fractions of the viewport. Logs "OWPICKUPCARD ..." once per change of the shown item.
    FString StateField(const AOpenWillowWalker& Maya, const APlayerController& Player);

    // -owpickupcardshots (with the movie HUD): rolls a gun with ow.RollGun, drops it ahead of Maya, captures the screen
    // aimed at it, aimed away, aimed again and after picking it up (OWPickupCard_<n>_<name>.png), then quits. Call every
    // HUD frame once the movie is ready; does nothing without the flag.
    void TickShots(AOpenWillowWalker& Maya, APlayerController& Player);
}
