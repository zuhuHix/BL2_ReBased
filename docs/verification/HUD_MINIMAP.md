# The HUD minimap (Sanctuary)

Minimap lane, 2026-10-22. AI-assisted (Claude Opus 5.5 orchestrating; Python tests by a Claude Haiku 5.5 sub-agent;
visual critic GPT-6-Luna). The host drives BL2's own HUD movie (`UI_HUD.HUD`, converted to SWF, run in Ruffle over UE5
with `-owflashhud`); the minimap is the movie's `p1.minimapEcho.minimap` clip with the level's own map movie loaded
into it. Nothing is redrawn natively. Captures, converted movies, the manifest and live reads stay under ignored
`local/` (`local/realgame/minimap`, `local/ui/run`, `local/slice/minimap.json`).

## What drives it in the game

The driver, `HUDWidget_Minimap` (`Init`, `InitOverheadMapVolume`, `Update`, `SetMapMovie`, `UpdateTargetRadius`), is
native; only `InitFogOfWarBlobs` and `ToggleFogOfWarBlob` are script. So the rules below were read from the running game
(widget properties and the clips it writes), with the movie's own ActionScript and the packages for the static parts.
No native code was disassembled for this lane.

**Static parts (packages and movies).**
* `WorldInfo.MyMapInfo` (`WillowMapInfo`) names the level's `TacticalMapMovie` (Sanctuary: `UI_TacticalMap_Sanctuary.
  Sanctuary_P`, one bitmap and two fog-of-war blob instances `SANCTUARY_PWDA_0/1`, the blob symbol imported from
  `SharedWillowTacMaps`) and its `TacticalMapVolume` (`WillowTacticalMapVolume_0`; the status-menu map reads the same
  pair in script, `StatusMenuMapGFxObject`).
* The HUD movie's `minimap` symbol is the AS2 class `MinimapController`: `setMapMovie(path)` loads the map movie into
  `mapPlaceholder` with a `MovieClipLoader`; its `onLoadInit` calls `ExternalInterface` `extInitMinimapFogOfWar`, which
  the widget answers with `toggleFogOfWarBlob(name, HasBeenUncovered)` for the player's `DiscoveredWorldAreas`. The
  symbol also holds `mapmask` (116 x 96), `DirArrow`, the `north` text field ("N"), `waypointsmask`, `flash`, and
  library templates of the icons parked about 400 px to its right.

**Live reads** (`tools/real_game/scripts/minimap.py`, Maya level 8 in Sanctuary on the "Plan B" save, 1280x720,
saving blocked; the save folder compared afterwards: only the known `Save0008.sav.bak` and the resolution in
`WillowEngine.ini` changed, both restored). Widget properties: `bPlayerRelative` true, `WorldRadius` 6000,
`UnrealUnitsPerPixel` 128, `OuterRadius` 9410.76 (the box corner: sqrt(58^2 + 48^2) x 125), `NorthMarkerOffset`
(-16, -18.800003), `MapYawOffset` 0, `CachedPlayerYaw` following the controller's (view) yaw, not the pawn's. Volume:
`UnrealUnitsPerPixel` 32, `NorthOffsetInDegreesClockwise` 0, bounds centre (-3072, -10240). Then yaw sweeps (0-315 in
45 degree steps), moves of 500 uu along X and Y, a radial walk away from the objective (1000-9000 uu) and one
per-frame recording while turning and walking (621 frames).

| What | Rule (all exact to the clip's twip unless noted) |
|---|---|
| Map rotation | `mapPlaceholder._rotation = -yaw`, in [-180, 180) (the game reads -180 at yaw 180); one frame of lag in the per-frame recording, as for the action-skill clips |
| Map scale | 102.4 % = 128 / 125 |
| Map position | the player's map point at the clip origin: map point = ((Y - cy) / 128, -(X - cx) / 128) x 1.024, turned by -yaw, negated; (cx, cy) the volume's bounds centre. Twips rounded down |
| Minimap unit | 125 uu = WorldRadius / 48 (a 500 uu move shifts icons and map by 4.00 units) |
| Icon position | offset (dY, -dX) / 125 turned by -yaw (view direction up). The icon clip's `_x/_y` equal the widget's `MapPos` |
| Panel box | 58 x 48 units each side of the player. Shop, health, fast-travel and customization icons hide outside it (a box, not an ellipse: an icon at (-48.1, 35.0) shows); mission-giver ("!") and objective icons are pulled back onto its edge along their direction |
| Objective scale | 140 % up to 6000 uu; beyond, 140 x (1 - (d - 6000) / 18000) (five reads at 6500-9000 uu fit to 0.05 %; beyond 9000 uu not seen) |
| "N" marker | where north (world +X for a north offset of 0) meets the box edge, plus `NorthMarkerOffset`; within one twip (8 of 74 reads one twip off, float noise in the game's trigonometry) |
| Arrow | `DirArrow._rotation` 0 (player-relative mode, the default; the absolute-minimap option was not read) |
| Fog | blob `SANCTUARY_PWDA_0` on its `miniMap` frame and hidden (`HasBeenUncovered` false in this save) |

Icon clips are attached under fixed names: `shop0..`, `hstation0..`, `ftstation0`, `cstation0`, `eligible0..`,
`objective0`, with `_xscale` 100 (objective 140), and the objective clip's `above`, `below`, `edgeArrow` and
`objective.optional` hidden in every read (even 1100 uu below it and clamped on the edge). Which library symbol each
kind uses was matched by size (`icon_shop` 21.6 x 23.2, `icon_healthstation` 21.6 x 21.6, `icon_travel station` 22.4 x
23.2, `icon_customize` 22.4 x 28.8, `icon_new mission` 12 x 20.8, `objective clip`), not read. Stacking seen in the
frames: map, station icons, "!" and the objective above them, the arrow on top.

**Which objects get icons.** The widget's icon arrays (`Icons_Shops`, `Icons_HealthStations`,
`Icons_FastTravelStations`, `Icons_CustomizationStations`, `Icons_LevelTravelStations`, `Icons_MissionEligible`,
`Icons_MissionRedeemable`, `Icons_Objective`, ...) were filled, at the capture spots, from the live interactive objects
whose definition's `CompassIcon` is `RadarIconType_Shop`, `_Health`, `_TravelStation`, `_CustomizationStation`
(`_LevelTransition` exists but was never on screen; `_BountyBoard` has no array). Every visible icon matched one such
object at the 1/125 scale (12 objects; mission-giver and objective icons come from mission state).

## Host implementation

* `tools/prepare_minimap.py` reads the map's `WillowMapInfo`, the volume's bounds centre (the `BrushComponent`'s
  `BrushAggGeom` convex box plus the actor's `Location`; -3072.0, -10240.0, the live value), converts the map movie and
  `SharedWillowTacMaps` (the map texture is decoded first, as for the action-skill icon), takes the fog blob names from
  the converted movie's root instances, and lists icon sources: placed actors by `InteractiveObjectDefinition`, and
  population opportunity points by `PopulationDef` -> factory -> `ObjectBalanceDefinition` -> `DefaultInteractiveObject`,
  each by its `CompassIcon`. Sanctuary: 11 of the 12 live icon objects (1 fast travel, 1 customization, 1 level
  travel, 6 shops including Crazy Earl's from `Sanctuary_Dynamic`, 2 of 3 health machines). Not found: the health
  machine at (8388, -4248), spawned by something other than a population point (not traced).
* `tools/hud_overlay/minimap.js` turns the host state into clip calls with the rules above, through the movie's own
  `setMapMovie` and `toggleFogOfWarBlob`, attaches the icons from the movie's library, and logs `OWMINIMAPPAGE` lines
  with values read back from the clips. It also re-projects `minimapEcho` for the 3D-turn stand-in (`hud3d.js`): that
  stand-in sizes a turned clip from `getBounds`, which counts the hidden icon templates (the game reads
  `minimap._width` 731 too) and put the panel about 30 px too far right and high. `placeEcho` uses the map mask's
  extent and a least-squares 2D affine map of the same projection over a 5 x 5 grid (worst residual 1.3 px), split
  into `minimapEcho`'s rotation and scale and a rotation of the `minimap` inside it, so the panel's top edge slants as
  in the game (a true trapezoid is not possible in a 2D player).
* `OpenWillowMinimap.cpp` sends `{movie, centre, blobs, x, y, yaw, icons}` with the view yaw and the pawn location,
  plus the quest's markers (`UOpenWillowQuest::MinimapMarkers`: Marcus "eligible" before accept, "turnin" when ready,
  "objective" on the range cylinder then the dummy; which actor the game's marker follows for this mission is
  UNVERIFIED). `-owminimapshots` puts Maya at the eight capture spots and yaws and captures the screen.

Not reproduced or UNVERIFIED: the panel's trapezoid (Ruffle draws no 3D; the stand-in is affine), fog-of-war state
other than "not uncovered", threats (enemy dots), allies, vehicles and the vehicle radius change, area objectives,
the objective's height arrows and edge arrow (never seen shown), the absolute-minimap option, the one-frame lag,
health machine 3 and the level-travel and turn-in icons' symbols.

## Checks

* `node tests/hud_minimap_test.js` (15): the rules as plain arithmetic, names, clamping and hiding, objective scale,
  the call economy of `plan`, fog calls, the affine split and `placeEcho`'s nested clips against the projection.
* `python tests/check_hud_minimap_test.py` (18) and `python tests/prepare_minimap_test.py` (15): the log checker's pass and
  failure paths with synthetic logs and PNGs (including review 1's case: the shots must cover each capture spot once,
  under distinct files; eight shots of one spot fail); PlaceObject name parsing, fog blob names, volume centre and the
  population chain with fake readers; the tool's outputs refused outside the repository's `local/`.
* `python tests/real_game_minimap_paths_test.py` (3): the capture script's labels must be plain file names that stay
  directly under its output folder (`../../../docs/x` and the like are refused before anything is written), and the
  output folder itself (`RG_OUT`, from `realgame.ps1 -OutFolder`) must be inside a repository's `local/`.
* Accepted, not guarded (PR #53 review 2; the guards stop accidental writes by our own tools, not a hostile
  filesystem): a junction or symlink placed at `local/` itself, hardlinks to tracked files, reserved device names
  (`NUL`, `CON`, ...) and path components taken from package data.
* Local oracle (not committed: it reads the live captures): `minimap.js` against all 652 live values of the yaw sweeps,
  moves and radial walk: map position, rotation and scale exact, "N" within one twip, every visible icon's position
  within 0.15 units, and the visible count of each kind equal at every pose.
* `tools/test_hud_minimap.ps1`: in-engine run and `tools/check_hud_minimap.py`, 15 checks: manifest loaded; host state
  logged; page read-backs; map movie drawn; map position, rotation and "N" recomputed in Python from the rules (not from
  `minimap.js`); every read-back a state the host logged; fog initialised by the movie's own callback; icons shown;
  8 shots at their spots and states, read back from the movie; 8 complete 1280x720 captures showing the panel (cyan map
  lines) and the arrow (gold) at its centre.

## Results (2026-10-22, this PC)

* In-engine: 15/15 (worst map position 0.062 units, rotation 0.000 degrees, "N" 0.054 units).
* `tools/test_action_skill_hud.ps1` with the minimap hooks: 19/19.
* Visual (independent critic, GPT-6-Luna, host and real 1280x720 frames at eight matched spots and yaws, crops at 3x;
  frames under ignored `local/minimap/pairs_r1`, `pairs_r2`, reports `critic_r1`, `critic_r2`): round 1 **7.5/10**
  (panel a level rectangle where the game's slants: its top edge rises 8-12 px to the right, the right side 6-10 px
  taller); round 2, with the affine fit, **8.0/10** (top edge rise 8 px against 11 px). Remaining, ranked: the
  trapezoid itself (needs a true perspective warp, below), the cyan border and halo about 1 px thicker with 2-4 px more
  glow (Ruffle's rendering of the movie's own art and filters, as in the action-skill record), the plate 10-15 %
  lighter by eye, and map strokes and icons within 2-4 px (the affine fit's error, worst residual 1.3 px). The plate is
  translucent: measured, it is 49.5 luma over black in a browser, 61.8 in the game over a background of 12 and 69.6 in
  the host over 30.9, so most of that gap is the brighter host world behind it (Sanctuary lighting lane), not the
  minimap. Mission icons differ by design (the real save is on "Plan B").
* Next step, not done here: the parallel HUD lane's perspective warp (`hud_warp.js`, branch `feature/hud-parity`)
  draws true perspective for the other turned clusters but leaves `minimapEcho` on the affine stand-in. Once it is on
  main, letting the warp draw `minimapEcho` over the map mask's extent (not `getBounds`, which counts the hidden icon
  templates) should close the trapezoid and the residual alignment; `placeEcho` then has to be skipped.
