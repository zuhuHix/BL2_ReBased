## 2026-09-30: Inventory backdrop grading excludes Maya

AI-assisted bounded host rendering change; existing UModel 1590 payloads and
object identity/import paths remain unchanged. No new extraction or third-party
code. `import_character_menu_look.py` now also generates `M_OW_MenuBackdrop`
locally. After-tonemap gain, desaturation and vignette use custom stencil 247
on the menu body, head and ink hulls, with a scene-depth visibility check. The
renderer enables depth/stencil (`r.CustomDepth=3`). The display owns the
blendable; destroying it on close removes the effect. Missing material logs a
warning and leaves world grading disabled. Depth of field remains focused on
Maya. Lighting reduced from 14/18 to 4/5 lux because Maya no longer needs to
compensate for global dimming.

Visual: fresh 1280x720 engine captures show yellow suit panels rather than
brown, world dimming and the inventory idle/outline. The 300 cm capture clipped
her elbow; default distance restored to 345 cm and visually checked again.
`OWCombat_9_AfterSkills` shows gameplay colour/FOV restored after menu close.
Local before/after evidence: `local/inventory/{before,after}-backdrop.png`;
captures remain ignored. Material generation and UE 5.8 module build passed.
CTest: 6/6; package verification: all nine decoded byte/count/export-field
checks match; in-engine inventory action regression: 26/26 with Slate inputs.
Detailed runtime results and limits are recorded in
`docs/verification/INVENTORY_MOVIE_PROTOTYPE.md`.

UNVERIFIED: exact BL2 shader/lighting parity, stencil occlusion under arbitrary
camera/world geometry, temporal edge stability, and an independent critic
re-score. Self-review against the local real-game reference still finds a
brighter face, different outfit and approximate blurred backdrop/glass; the
previous independent 5.5/10 score is not updated by this visual check.

