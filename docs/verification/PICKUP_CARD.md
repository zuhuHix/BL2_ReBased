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
  remaining gun logs `wall decoy_left=1`, and E from the same spot with the block gone logs `wall_removed decoy_taken=1`. The rule is the host's (UNVERIFIED: the game's use target is native).

## Local data

Convert the movie once (output under ignored `local/`):

```powershell
python tools/extract_swfmovie.py --reader build/Release/ow-package.exe --package "$env:OPENWILLOW_BL2/WillowGame/CookedPCConsole/Startup.upk" --movie UI_Inworld.Weapon_Card --output local/ui/gfx/Weapon_Card.gfx
python tools/gfx_to_swf.py --gfx local/ui/gfx/Weapon_Card.gfx --textures local/ui/tex/UI_Inworld --localization "$env:OPENWILLOW_BL2/WillowGame/Localization/INT" --inline-font-imports --output local/ui/run/UI_Inworld/Weapon_Card.swf
python tools/hud_harness_swf.py --movie Weapon_Card.swf --output local/ui/run/UI_Inworld/harness.swf
```

Without it there is no card: the frame logs `OpenWillow movie missing (...)` and `OpenWillow pickup card off: no movie`. Seen on pc-hummer: no
error page over the game. `UNVERIFIED`: that a pickup succeeds with the movie absent. In that run E went to a strongbox, because the aimed search failed at that moment (likely a
respawned enemy on the sight line), so nothing has shown it yet; the E code does not read the page. A card at least 30 s later asks again, so a file that arrives later is picked up. Before 2026-11-06 a missing movie put Ruffle's opaque error page over the whole game
(`tools/hud_overlay/movie_guard.js` now checks every page's movie first). `python tools/check_overlay_data.py` lists what is
missing. `tools/test_slice_gate.ps1` runs with the canvas HUD and reads the card from the host's field, so it passes with no movie at
all; only a run with the movie HUD (`tools/run_ue_flash_hud.ps1`, `-owpickupcardshots`) shows the card, and that capture now stops if
the movie is absent.

## Checks

- `tests/pickup_card_test.js` (synthetic): placement, the refill/show/hide plan, the HUD-to-frame forwarding and the shared
  card writer's rows and arrows.
- Pass `-d3d11` with this capture. On pc-hummer one run with it passed and one without it crashed in the D3D12 runtime (`dxilconv.dll` under `D3D12Core.dll`), about 10 s in: one of each, so likely D3D12 on that PC, not proven. The launcher's default is unchanged because it could change how frames look.
- In engine, `-owpickupcardshots` (with the movie HUD): rolls a gun (`ow.RollGun`, default the Hellfire balance at stage 34),
  drops it 150 uu ahead, and captures aimed away, aimed, aimed away, aimed again and after E picked it up; then the decoy
  (E must take the aimed gun, not the nearer one); then E with a test block in front of the remaining gun (it must stay); then
  E again with the block removed (it must be taken); then a last gun aimed at and Maya put down (the card must be gone).

## Open: what blocks the sight test

A 2026-11-06 scripted run had the card hide for about 0.25 s with Maya standing still and aimed at the drop, and E at that moment went to a strongbox. Reach, the view and the
driver do not explain it. An enemy respawned about 60 uu from the drop 37 ms before the card hid, which fits an enemy on the sight line (timing only, not proof). The test stops at anything
blocking `ECC_Visibility` except Maya and the pickup. `OWPICKUPCARD blocked by <actor> (<component>)` now names what it hit, at most twice a second, and changes nothing else.
This is not a confirmed bug: `NATIVE_USE_INTERACTION.md` describes one ray and the first usable hit, so a body in the way may block in the game too (`UNVERIFIED`).

A corpse, read from the code and not run: from death to the ragdoll the enemy has no collision. After that its mesh has the engine's `Ragdoll` profile, which ignores `ECC_Visibility`, and its
hit volume stays off, so only its crit sphere can block, a head-sized sphere on the floor, until it respawns (3 s after death by default). So a corpse on a drop should at most blank the card for a
moment. A range gun lands on a ring at least 60 uu from the corpse's centre, not under it. A live enemy's hit volume is the larger case.

## Open, for a real-game session

The look-at reach, cone and sight test, which item a weapon is compared against, the card's on-screen size and placement relative to the item, its frame style, the in/out timing,
whether the card hides at once when the player looks away, and whether the selected inventory card's outline turns red too for an item above the player's level (the pickup card,
on its default frame, turns red as a whole: the movie's own `SetLevelRequirement`; see INVENTORY_MOVIE_PROTOTYPE.md).
