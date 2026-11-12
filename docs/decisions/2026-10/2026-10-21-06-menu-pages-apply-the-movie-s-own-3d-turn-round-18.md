## 2026-10-21: Menu pages apply the movie's own 3D turn (round 18)

AI-assisted (Claude). New `tools/hud_overlay/menu3d.js` (served by `serve.py`), `inventory.*`, `skills.*`, the page report in
`OpenWillowMayaHUD.cpp`, `tests/inventory_navigation_test.js`.

- The StatusMenu movie turns the inventory clip 11 degrees and the skills clip 7.24 degrees about the vertical axis (`_yrotation`,
  read live); Ruffle ignores it. The pages apply the same turn as a CSS 3D transform at Flash's default 55-degree field of view
  (focal length 1229 px; Scaleform's value not read, UNVERIFIED; one panel-edge ratio agrees to 0.001). This replaces the "perspective
  tilt Ruffle ignores" open item for the inventory and Skills views; the curved glass sheet (`ring`, far behind with a brightening
  blend) is still not reproduced.
- Measured screen fits are mapped back through the projection before use; the Skills overview poses were refitted.
- The page report's row-alignment check uses layout offsets, since the hit layer and the movie share the turn.
- **Visual review:** critic 7.67/10 (round 17: 7.21). **Checks:** navigation 30/30; inventory suite 49 / 0 / 0 / 0.

