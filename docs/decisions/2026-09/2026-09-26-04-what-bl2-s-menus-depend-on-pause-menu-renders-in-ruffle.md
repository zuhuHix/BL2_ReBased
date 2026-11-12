## 2026-09-26: What BL2's menus depend on; pause menu renders in Ruffle

Asked "how could we mass import the menu logic", measured the dependency
first. Counts come from `ow-package --exports` over the install and from
string scans of decompressed movies. The movies' script listings stay local
(docs/LEGAL.md, "UI movies").

- Movies: 914 packages scanned, 0 failures, 64 hold `GFxUI.SwfMovie` exports,
  280 distinct movies. 137 are skill/action-skill icons, about 40 are ECHO
  portraits and small icons, one is a tactical map per level, and about 30 are
  real screens. Examples: `UI_StatusMenu.StatusMenu` (inventory, skills,
  missions and map), `UI_FrontEnd_TitleMenusClik`, `UI_Options`,
  `UI_VendingMachine`, `UI_FastTravelStation`, `UI_Mission`, `UI_Trading`.
- The logic that drives them is UnrealScript. Classes whose names match UI
  patterns own 3,205 of WillowGame's 11,945 functions, including
  `StatusMenuExGFxMovie` 196, `FrontendGFxMovie` 192 and `WillowHUDGFxMovie`
  127. Add 162 in GearboxFramework, 73 in Engine, and GFxUI's 196: the
  `GFxMoviePlayer`/`GFxObject` bridge, mostly natives. This is name-pattern
  counting, so it is approximate in both directions. The functions read game
  state (inventory, item definitions, skills, missions, player controller),
  which is why ROADMAP.md places the menus in Phase 4 on top of the Phase 2 VM.
- Movie to game: the movies call named ExternalInterface functions. The HUD
  uses 13 `ext*` names; StatusMenu uses 41 in script constant pools and 55
  `ext*` byte strings overall. In Ruffle, clicking StatusMenu's Skills tab
  called `extGenericButtonClicked("skills")` on the page. No call fired at
  load, so the movie appears to wait for the game to drive it first
  (UNVERIFIED).

Converter changes (`tools/gfx_to_swf.py`):
- Import URLs have `\` replaced by `/`. StatusMenu imports
  `..\SharedWillowInventory\...`, which Scaleform on Windows accepts and a web
  player does not.
- Script localization no longer skips streams with branches. After strings
  change length, every Jump/If offset and every DefineFunction(2), With and
  Try size is recomputed from old-to-new action positions. A stream is left
  unchanged, and counted as `script_skipped`, if any distance does not end on
  an action boundary. `tests/gfx_to_swf_test.py` (synthetic AVM1) covers
  forward and backward branches, non-spanning jumps, function and Try sizes,
  the misaligned fallback, and URL normalization. It is not yet registered
  with ctest because that needs a CMakeLists.txt change. On the real outputs:
  HUD 40, StatusMenu 30 and SharedWillowInventory 4 tokens translated, 0
  skipped; 2,168 branches checked, 0 off an action boundary.

StatusMenu (with `SharedWillowInventory`, textures decoded with `ow-package
--texture`) now renders in the Ruffle bench: tab bar, localized title
"INVENTORY", close button, background. Its panels stay empty because nothing
plays the game side.

