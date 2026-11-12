## 2026-10-22: The HUD minimap drives BL2's own minimap clip from rules read in the live game

The maintainer asked for the HUD's map. The minimap's driver (`HUDWidget_Minimap.Init/Update/SetMapMovie`) is native,
so its rules were read from the running game through the mod SDK (AI-assisted; record
`docs/verification/HUD_MINIMAP.md`; no native code was disassembled):
- each level has a tactical map movie (`WillowMapInfo.TacticalMapMovie`; Sanctuary: one bitmap and two fog blobs) that
  the HUD movie's own `MinimapController.setMapMovie` loads into `mapPlaceholder`; the map's origin is the
  `WillowTacticalMapVolume`'s bounds centre, 128 uu per map unit, shown at 102.4 %;
- the map turns by -yaw (view yaw) about the player; one minimap unit is WorldRadius / 48 = 125 uu; station icons hide
  outside a 58 x 48 box, mission and objective icons are pulled onto its edge; the objective shrinks beyond 6000 uu as
  140 x (1 - (d - 6000) / 18000) (fitted, seen to 9000 uu); the "N" marker sits on the box edge;
- which world objects get icons follows their interactive-object definition's `CompassIcon`, reached for vending
  machines through their population opportunity point's definition chain.
`tools/prepare_minimap.py` converts the map movie and writes the icon manifest (11 of the 12 live icon objects found
in the packages; one health machine is spawned another way), `tools/hud_overlay/minimap.js` maps the state to the
clips (exact against 652 live values; the "N" within one twip), `OpenWillowMinimap.cpp` sends the state with the
quest's markers (`UOpenWillowQuest::MinimapMarkers`; the marker's actor for this mission is UNVERIFIED). The panel's
3D turn is drawn by PR #54's perspective warp over the panel's own rectangle (the clip's visible children include the
movie's parked library icons); without the warp a least-squares affine stand-in in `minimap.js` replaces `hud3d.js`'s,
which sized it from bounds inflated by those templates. Checked after rebasing onto #54: node 16, Python 19 + 15 + 3,
in-engine 15/15, action-skill suite 19/19, HUD shots 16/16, CTest 10/10, packages 9/9; critic 7.5, 8.0 (affine), 8.2/10
(warp). Not done: threats, allies,
vehicles, fog state other than "not uncovered", the absolute-minimap option, the full-screen map. Review 1: the
capture script and the prep tool refuse output paths outside the ignored folders (game data cannot be written into
tracked folders), and the in-engine checker requires each capture spot exactly once.

