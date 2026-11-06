# The status menu's Map screen (2026-11-04)

AI-assisted (Claude). Inventory lane, pc-hummer. BL2's own StatusMenu movie on its Map frame with the level's tactical map
movie loaded into it, driven by the page the way `StatusMenuMapGFxObject` drives it. Script listings and the movie's
ActionScript listing (our own local AVM1 lister, from the public SWF format) stay under ignored `local/analysis/inv24/map/`;
captures under ignored `local/inv24/mapshots/`. Nothing below was observed in the running game.

## What the game does (script, installed class defaults and the movie's ActionScript)

- `TrySwitchToScreen(2)` and `GotoPrevTab` / `GotoNextTab` go to the StatusMenu movie's `map` frame. Its `map` clip
  (`StatusMenuMapGFxObject`) has `mapPlaceholder`, `landmarkContainer` (templates `players` and `landmarks`), `crosshairs`,
  `legend`, `playersDisplay`, `header.mapName`, `infoPanel` and `mask`.
- `StatusMenuMapGFxObject.Init` reads the level's `WillowMapInfo` (`TacticalMapMovie`, `TacticalMapVolume`, its
  `NorthOffsetInDegreesClockwise`), starts at the saved scale or `MinScale`, and calls the clip's own `setMapMovie(path)`.
  The clip loads the movie into `mapPlaceholder` and calls `extStatusMenuMapPostMapMovieLoad` with its rectangle; the
  script then toggles the fog blobs of the discovered areas and runs `PostMapMovieLoad` (header, landmarks, objects, players:
  native).
- The clip's ActionScript: `setMapScale(s)` sets the map movie's `_xscale` / `_yscale` to `s`; `addLandmark(clip, x, y, r,
  scale, frame, innerFrame, bOptional)` duplicates a template in `landmarkContainer`, sets its frame and places it through
  `updateLandmark`; `removeLandmark`, `toggleLegend` (legend and player list), `toggleFogOfWarBlob(name, enabled)`.
  Landmark frames: objective, rstation, poi, transition, vss, bb, health, shop, newu, customization, questionmark, Vehicle,
  CustomObjective, missionComplete, missionNew, areaWaypoint; player frames: player, buddy1-3.
- Class defaults: `MinScale` 135, `MaxScale` 395, `PanIncrement` 0.05, `PanZoomFactor` 0.25, `ZoomSpeedFactor` 0.05,
  `CompassIconFrames` (bb, shop, health, vss, rstation, customization, transition). `AnalogScale` zooms by a factor and
  clamps to MinScale .. MaxScale; `PanHorz` / `PanVert` move the centre by PanIncrement of the map's size times
  lerp(1, scale / MinScale, PanZoomFactor); both also multiply by a native frame-time term (`GetDeltaTimeScalar`).
- Keys (`HandleMapInputKey`): arrows pan; PageUp, `-`, `_` zoom out; PageDown, `=`, `+` zoom in; the wheel zooms; Q toggles
  the legend; Enter or the right mouse button places a custom waypoint. `SetTooltipsText` sets nothing on this screen: the
  hint bar is the movie's own `$<StringAliasMap:map_bar>` token ("[Enter] Waypoint [+/-] Zoom [Q] Legend [Escape] Close"
  on PC). There is no fast-travel list on this screen (fast travel is the station's own movie).

## What the host does

- The host opens `map.html` as a fourth status page (`AOpenWillowMayaHUD`: `ToggleMap`, `/__ow_tab_map`, `/__ow_close_map`)
  and pushes `OpenWillowMapScreen::StateJson`: the HUD minimap's state (`OpenWillowMinimap::StateField`: map movie, volume
  centre, fog blobs, Maya's position and view yaw, the stock icons and the quest's markers) and the map name.
- `map.js` loads the map movie through the clip's `setMapMovie`, scales it with `setMapScale` (135 on open), places Maya's
  arrow and the icons with `addLandmark` / `updateLandmark`, and fills the header and the hint bar
  (`tools/prepare_map_screen.py` writes the resolved hint to ignored `local/ui/run/map.json`).
- Positions use the tactical map movie's own units as measured for the minimap in the running game (128 uu per unit, origin
  at the volume's bounds centre, x along world +Y, y along world -X; `HUD_MINIMAP.md`), scaled with the map; the volume
  class's `UnrealUnitsPerPixel` (32), which the native uses, is not read further.
- **UNVERIFIED host choices:** the map opens centred on Maya under the crosshairs; the arrow's rotation is the view yaw with
  north up; one key press zooms by 1.2 and pans one PanIncrement (the native frame-time term is not read); icon scale 100;
  the header name is the map package's name without `_P`, upper case (`SetMapHeader` is native); fog stays "not uncovered";
  Enter (custom waypoint), mouse drag, the info box and the player list's names are not driven.
- Failures are visible (PR #97 review 1): the loading panel stays up until the map movie has loaded and says why when the
  host sent no state or no map movie (no minimap manifest), when the map movie does not load within 10 s, or when the
  StatusMenu movie is not ready within 15 s (the polling then stops); Escape, Tab and M close the page in every case.
- Pan is limited so the map point under the crosshairs stays inside the map movie's own rectangle (`getRect`) at the current
  zoom, re-applied after a zoom and when Maya moves (host rule; the game clamps natively, UNVERIFIED).
- Tabs from the Map: 1 Missions, 3 Inventory, 4 Skills, 5 nothing (no Challenges page), Comma Missions, Period Inventory,
  Escape / Tab / M close; I, K, L as on the other pages. The Inventory's key 2, Comma and its Map tab now open this page.

## Checks

- `tests/map_screen_test.js` (10, synthetic): the movie-unit projection, scale clamp, pan step and pan limits, layout
  (centring, icons with the map, pan), the landmark frames, the tab routes, and the page part with a fake Ruffle: the 15 s
  status-menu deadline (polling stops, message shown, Escape closes) and the no-state, no-movie and map-load messages. `tests/prepare_map_screen_test.py` (4): token resolution and
  the local/ output guard.
- In engine, `-owmapshots` (the slice quest resumed from a local save with the Fire mission active): opens the Map, captures
  it, turns Maya 90 degrees, zooms in twice, pans, toggles the legend, closes with Escape, opens the Inventory and presses 2.

- Over the vending page (vendors #81): the shop page holds the keyboard, so no key opens a status page over it. Forced
  through `ToggleMap` (`-owmapshots` with `-owvendors`), the Map opened on top and closing it handed the input back to the
  game under the still-open shop (cursor hidden, shop still open). Since 2026-11-04 no status page opens while a shop is
  open (`UOpenWillowVendorSubsystem::IsShopOpen`), and the Walker's legacy inventory widget does not open in its place (PR
  #101 review); the capture logs both refusals (`map_over_vending open=0`, `inventory_over_vending page=0 widget=0`).

## Open, for a real-game session

The centring and scale on open, the arrow's rotation and look, one key press's zoom and pan, the icon set and scale for
Sanctuary (and the objective marker while the Fire mission is active), the header name, and how the custom waypoint looks.
