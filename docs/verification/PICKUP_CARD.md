# World pickup card (2026-11-04)

AI-assisted (Claude). Inventory lane, pc-hummer. The card BL2 shows while the player looks at a dropped item, drawn with the
game's own card movie through the HUD overlay. Script listings stay under ignored `local/analysis/inv24/`; captures under
ignored `local/inv24/pickup/`.

## What the game does (read from script and installed data; not observed in the running game)

- The card is its own movie: `ItemPickupGFxMovie`, defined by `UI_Inworld.Weapon_Card_DEF` (`ItemPickupGFxDefinition`) whose
  `SwfMovie` is `UI_Inworld.Weapon_Card` (Startup.upk). The movie imports the shared `item card` clip from
  SharedWillowComponents and places it at `inventory.card1`; `inventory` is a 10-frame clip with the labels `off`, `in` and
  `out`, and the movie's own `TurnOnPickupCard` / `TurnOffPickupCard` play it in and out.
- `ItemPickupGFxMovie.Start` renders the movie to a texture (`RenderTextureMode` 2). The definition draws that texture with a
  particle (`fx_shared_items.Particles.Part_ItemCard`) whose material (`UI_Inworld.FlashMaterials.MatItemCard`) samples it, so the
  card floats in the world at the item. Where exactly is native (`WillowHUD.ShowPickupCard`, `GetPickupCardLocation`): not read.
- Which item: `WillowPlayerController.GetCurrentPickupable` returns the touched pickup when there is one (and it is not an
  automatic pickup), else the seen one (`SawPickupable` / `ClearSeenPickupable`, set by native look-at code: not read).
- The card is filled through `ItemCardGFxObject` (`SetItemCardEx`, `SetTitle`, `SetTopStat`, `SetFunStats`, `SetValue`,
  `SetLevelRequirement`), the same class the inventory's cards use.

## What the host does

- `OpenWillowPickupCard.cpp` finds the aimed pickup: within 220 uu of Maya (the reach `AOpenWillowWalker::PickupNearby` uses),
  within 12 degrees of the view direction and in sight of the view (a visibility trace ignoring Maya and the pickup, so no
  card through walls). Ties go by angle, then distance from Maya, then name, so the choice never depends on actor order; this
  is the one selection rule meant for E too. **UNVERIFIED** (reach, cone and the sight test are host choices; the game's test
  is native). The search runs at most 10 times a second and the card JSON is rebuilt only when the target, the compared item
  or the level changes. No card while Maya is down or dead (`IsDown`). It sends the HUD state field `pickupCard`: the item as the inventory page receives it
  (`UOpenWillowInventory::WeaponItemJson` / `GearItemJson`, moved out of `StateJson` unchanged), the equipped analogue for the
  compare arrows (the held weapon whatever its type: `WillowInventoryManager.GetEquippedAnalogue` returns the pawn's current
  weapon for any weapon, and the menu compares the same way; for gear the equipped item of the same slot. Whether the native
  pickup card uses that analogue is not read: UNVERIFIED), Maya's level and the item's screen point. `OWPICKUPCARD shown|hidden` log lines record each change.
- The HUD page forwards the field to a transparent same-origin frame (`pickup_card.html`), which plays the converted movie in
  its own Ruffle player and fills the card with `item_card.js`, the card code the inventory page uses (moved out of
  `inventory.js` unchanged). The frame keeps the card's canvas away from `hud_warp.js`, which redraws the newest WebGL canvas
  of the HUD document in place of the HUD movie (a second player in the HUD document blanked the HUD in the first capture).
- Card size (30 % of the viewport height), placement (centred over the item, its bottom 3 % of the height above it), the frame
  style ('default') and the show/hide timing (the movie's in/out frames, at once) are **UNVERIFIED** host fits.
- **E takes the aimed pickup (2026-11-04, after vendors #81).** `AOpenWillowWalker::PickupNearby` first takes the pickup
  `FindAimedPickup` returns (the one the card shows); without one, the nearest use target in reach wins as before (vending
  machine, door, pickup). Marcus's talk check still comes first. The nearest-pickup search uses the card's sight test too
  (`InSight`, PR #101 review), so E takes nothing through a wall. The decoy capture (a gun 70 uu to Maya's right, nearer and
  not aimed at, and one 150 uu ahead, aimed at) logs `decoy aimed_taken=1 nearer_left=1`; E with a test block in front of the
  remaining gun logs `wall decoy_left=1`. The rule is the host's (UNVERIFIED: the game's use target is native).

## Local data

Convert the movie once (output under ignored `local/`):

```powershell
python tools/extract_swfmovie.py --reader build/Release/ow-package.exe --package "$env:OPENWILLOW_BL2/WillowGame/CookedPCConsole/Startup.upk" --movie UI_Inworld.Weapon_Card --output local/ui/gfx/Weapon_Card.gfx
python tools/gfx_to_swf.py --gfx local/ui/gfx/Weapon_Card.gfx --textures local/ui/tex/UI_Inworld --localization "$env:OPENWILLOW_BL2/WillowGame/Localization/INT" --inline-font-imports --output local/ui/run/UI_Inworld/Weapon_Card.swf
python tools/hud_harness_swf.py --movie Weapon_Card.swf --output local/ui/run/UI_Inworld/harness.swf
```

Without it the frame logs `OpenWillow pickup card movie failed` and the HUD is unaffected.

## Checks

- `tests/pickup_card_test.js` (synthetic): placement, the refill/show/hide plan, the HUD-to-frame forwarding and the shared
  card writer's rows and arrows.
- In engine, `-owpickupcardshots` (with the movie HUD): rolls a gun (`ow.RollGun`, default the Hellfire balance at stage 34),
  drops it 150 uu ahead, and captures aimed away, aimed, aimed away, aimed again and after E picked it up; then the decoy
  (E must take the aimed gun, not the nearer one); then E with a test block in front of the remaining gun (it must stay); then
  the block removed, the gun aimed at and Maya put down (the card must be gone).

## Open, for a real-game session

The look-at reach, cone and sight test, which item a weapon is compared against, the card's on-screen size and placement relative to the item, its frame style, the in/out timing,
whether the card hides at once when the player looks away, and whether the selected inventory card's outline turns red too for an item above the player's level (the pickup card,
on its default frame, turns red as a whole: the movie's own `SetLevelRequirement`; see INVENTORY_MOVIE_PROTOTYPE.md).
