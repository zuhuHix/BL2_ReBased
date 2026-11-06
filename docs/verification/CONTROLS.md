# The player's keys (controls lane, 2026-11-06)

AI-assisted (Claude). Host work so a person at the keyboard can do the slice's basic verbs. A passing
`tools/test_controls.ps1` says the host did this; it does not say the host matches the game.

## Where the stock keys come from

In the default keyboard layout, M and L map to the stock map and logs actions, and V and G to melee and grenade. That was read from
the game's data (the keyboard input device object, through the OpenBLCMM dump, `tools/blcmm_dumps.py`), not pressed in the running game:
**UNVERIFIED** in play. The `.ini` files do not hold these keys, and a player's rebinds live elsewhere and were not read.

## Map and missions keys

Before this lane no gameplay key opened these pages (`DefaultInput.ini` had no mapping and the Walker no binding); they opened
only through the status menu's tabs.

`AOpenWillowWalker::ToggleMapPage` and `ToggleMissionsPage` (`OpenWillowControls.cpp`) run the steps `ToggleSkills` runs (the
trigger and the aim end, a legacy inventory widget gives way), then call the HUD's existing `ToggleMap` / `ToggleMissions`.
What those already refuse stays refused: nothing opens over a vending shop, and with no page served the key does nothing and
logs `OWCONTROLS map page did not open`.

Host rules, **UNVERIFIED**:

- With every page closed, the key opens its page. The script reaches the stock menu through `ShowStatusMenu` with a chosen first
  screen (`WillowPlayerController.ShowMap`), and it refuses through `CanShowModalMenu`, whose conditions were not read. So the host
  also opens these pages while Maya is down, as its inventory and skills keys already do.
- With a page holding the keyboard, a real key goes to the page's own script, not to these bindings. `map.js` closes on M and goes to
  the Missions on L; `missions.js` closes on L but has no M route; `inventory.js` and `skills.js` have neither. Not changed here: what
  M and L do on an open page in the real game is for the real-game capture to read, and the routes follow from that (`inventory.js`
  stays with the inventory lane).

## Checks

- `tests/controls_keys_test.py` (ctest `controls-keys`): every `BindAction` has a mapping and every mapping a binding (UE ignores
  a mismatch silently), no key runs two actions, M and L are as above. Our own config and source only.
- `tools/test_controls.ps1` (`-owcontrolstest`, in engine): presses through the player controller's input path, as the inventory
  suite does, with every page closed, because a simulated press reaches the bindings even where a real key could not. Rows:
  `start_closed`, `m_opens_map`, `map_closed`, `l_opens_missions`, `closed_at_end`, then `OWCONTROLS SUMMARY passed=5 failed=0`.
  `tools/check_controls.py` grades the run's own log: the expected count, failed=0 and each expected row exactly once as PASS.
- `tests/check_controls_test.py` (ctest `controls-check`): the grader on invented logs, including a short count, a renamed row, a
  repeated row and a missing summary.
