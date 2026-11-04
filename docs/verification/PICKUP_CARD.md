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

- `OpenWillowPickupCard.cpp` finds the aimed pickup: within 220 uu of Maya (the reach `AOpenWillowWalker::PickupNearby` uses)
  and within 12 degrees of the view direction, the one nearest the view's centre line. **UNVERIFIED** (reach and cone are host
  choices; the game's test is native). It sends the HUD state field `pickupCard`: the item as the inventory page receives it
  (`UOpenWillowInventory::WeaponItemJson` / `GearItemJson`, moved out of `StateJson` unchanged), the equipped analogue for the
  compare arrows (the held weapon, as `WillowInventoryManager.GetEquippedAnalogue` does for the menu; for gear the equipped item
  of the same slot), Maya's level and the item's screen point. `OWPICKUPCARD shown|hidden` log lines record each change.
- The HUD page forwards the field to a transparent same-origin frame (`pickup_card.html`), which plays the converted movie in
  its own Ruffle player and fills the card with `item_card.js`, the card code the inventory page uses (moved out of
  `inventory.js` unchanged). The frame keeps the card's canvas away from `hud_warp.js`, which redraws the newest WebGL canvas
  of the HUD document in place of the HUD movie (a second player in the HUD document blanked the HUD in the first capture).
- Card size (30 % of the viewport height), placement (centred over the item, its bottom 3 % of the height above it), the frame
  style ('default') and the show/hide timing (the movie's in/out frames, at once) are **UNVERIFIED** host fits.
- **Known gap:** E still takes the nearest pickup within reach (`PickupNearby`), not the aimed one, so with two drops close
  together the card and E can disagree. `OpenWillowPickupCard::FindAimedPickup` is exposed for that change, which waits for the
  vendors lane's rewrite of `PickupNearby`.

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
  drops it 150 uu ahead, and captures aimed away, aimed, aimed away, aimed again and after E picked it up.

## Open, for a real-game session

The look-at reach and cone, the card's on-screen size and placement relative to the item, its frame style, the in/out timing,
whether the card hides at once when the player looks away, and what the card shows for an item above the player's level.
