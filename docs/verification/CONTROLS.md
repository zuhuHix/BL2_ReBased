# The player's keys (controls lane, 2026-11-06)

AI-assisted (Claude). Host work so a person at the keyboard can do the slice's basic verbs. A passing
`tools/test_controls.ps1` says the host did this; it does not say the host matches the game.

## Where the stock keys come from

The stock keyboard table is the data object `GD_Input.Devices.InputDevice_Keyboard`, read through the OpenBLCMM object dump
(`tools/blcmm_dumps.py`, which is the game's own `obj dump` output). Its `Buttons` list pairs a key with input actions. The
`.ini` files do not hold these keys: `DefaultInput.ini` has only engine and debug bindings, and `DefaultGame.ini` only the menu
labels (`GBA_Map` and so on). The table is the default layout; a player's rebinds live elsewhere and were not read.

| Key | Stock action | Stock command | Host |
|---|---|---|---|
| M | `InputAction_ShowMap` | `ShowMap` | `OWMap`, since this lane |
| L | `InputAction_ShowLogs` | `ShowStatusMenu_Logs` | `OWMissions`, since this lane |
| V | `InputAction_Melee` | `PlayerBehavior_Melee` | none yet |
| G | `InputAction_ThrowGrenade` | `PlayerBehavior_ThrowGrenade` | projectiles lane |

Read from data, not pressed in the running game: **UNVERIFIED** that a stock profile behaves this way, though the table leaves
little room.

## Map and missions keys

Before this lane no gameplay key opened these pages (`DefaultInput.ini` had no mapping and the Walker no binding); they opened
only through the status menu's tabs.

`AOpenWillowWalker::ToggleMapPage` and `ToggleMissionsPage` (`OpenWillowControls.cpp`) run the steps `ToggleSkills` runs (the
trigger and the aim end, a legacy inventory widget gives way), then call the HUD's existing `ToggleMap` / `ToggleMissions`.
What those already refuse stays refused: nothing opens over a vending shop, and with no page served the key does nothing and
logs `OWCONTROLS map page did not open`.

Host rules, **UNVERIFIED**:

- A press with another status page open switches to the asked page, and a press on the same page closes it. The script reaches
  the stock menu through `ShowStatusMenu` with a chosen first screen (`WillowPlayerController.ShowMap`), and it refuses through
  `CanShowModalMenu`, whose conditions were not read. So the host also opens these pages while Maya is down, as its
  inventory and skills keys already do.
- With a page holding the keyboard, the page's own script answers the key. `map.js` closes on M and goes to the Missions on L;
  `missions.js` closes on L but has no M route, and `inventory.js` and `skills.js` have neither. Not changed here: the status
  pages' internal keys are a separate question, and `inventory.js` belongs to the inventory lane.

## Checks

- `tests/controls_keys_test.py` (ctest `controls-keys`): every `BindAction` has a mapping and every mapping a binding (UE ignores
  a mismatch silently), no key runs two actions, M and L are as above. Our own config and source only.
- `tools/test_controls.ps1` (`-owcontrolstest`, in engine): presses through the player controller's input path, as the inventory
  suite does. Rows: `start_closed`, `m_opens_map`, `m_again_closes_map`, `l_opens_missions`, `m_over_missions_is_map`,
  `l_over_map_is_missions`, `closed_at_end`, then `OWCONTROLS SUMMARY passed=7 failed=0`. A simulated press reaches the bindings
  even while a page has the keyboard, which a real key does not, so the switch rows test the Walker and HUD path, not what
  a real press on an open page does (see above).
