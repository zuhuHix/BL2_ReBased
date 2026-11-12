## 2026-11-04: the status menu's Map screen (inventory lane)

AI-assisted (Claude). Record: `docs/verification/MAP_SCREEN.md`. New host page; read from script, installed class defaults
and the StatusMenu movie's own ActionScript, not observed in the running game.

- Key 2, Comma and the Map tab open `map.html`: BL2's StatusMenu movie on its Map frame with the level's tactical map movie,
  Maya's arrow, the stock icons and the quest's markers placed through the clip's own `addLandmark`, fed with the HUD
  minimap's state (`OpenWillowMapScreen`). Scale 135 on open, zoom clamped to 135-395 (class defaults); the hint bar is the
  movie's own `map_bar` text (`tools/prepare_map_screen.py`, local output only).
- UNVERIFIED host choices: centring on Maya, the arrow's rotation, one key press's zoom (x1.2) and pan (one PanIncrement),
  the header name; the custom waypoint, mouse drag and info box are not driven. No fast-travel list exists on this screen.

