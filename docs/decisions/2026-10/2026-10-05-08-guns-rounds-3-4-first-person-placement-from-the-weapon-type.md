## 2026-10-05: Guns rounds 3-4: first-person placement from the weapon type's view offset and mesh FOV

AI-assisted (Claude). Host and tools (`OpenWillowWalker.cpp/.h`, new `tools/weapon_view_model.py`, a debug mode in
`import_weapon_paint.py`); no change to `src/`, `CMakeLists.txt` or package parsing. Details:
`docs/verification/WEAPON_VISUALS.md` sections 9-10.

- **Round 3** read the foreground FOV 45 as a vertical angle. A blind A/B split by type: better pistols, worse long
  guns. Not committed on its own.
- **Round 4, cause:** the host never placed the arms as the game does. Read through the SDK on five held weapons:
  the arms origin is the view point plus the weapon type's `PlayerViewOffset`, and the controller's foreground FOV is
  the type's `FirstPersonMeshFOV` (45; SMG 50). The cooked values are read from the weapon types by the new script
  into an ignored `weapon_view.json`; the walker applies them on weapon select, plus the idle clip's Camera-bone
  offset. Host bone positions then match the live ones to about 0.1 cm. With that placement the 45 fits as a
  horizontal angle on all six guns (silhouette widths within 3-12% of the real frames). The data values and the live
  equality are read facts; how the engine applies them and the angle's axis are `UNVERIFIED`.
- **Exposure:** the maintainer saw all-white guns. Those were the lane's solid-white silhouette runs
  (`OW_Debug` 4, measurement only). In normal runs the in-silhouette brightness is within about 25 levels of the real
  frames; the real Jakobs metal is near-white too and the real frames are lit blue. No exposure change.
- **Visual review:** a blind A/B critic preferred round 4 over the committed round 2 on five of six guns (one tie),
  about 7.0 against 5.3 per gun. Main gaps: paint too warm (Infinity, rifle and shotgun barrels), the plain pistol
  slightly low and large, long-gun bodies running a little far right, the rifles' forearm tint.
- **Checks:** CTest 10/10; `tests/weapon_paint_test.py` 34 OK, `weapon_recipe_test` 14 OK, `weapon_stats_test` 28 OK;
  `verify_packages` OK; quest suite 79/0 and 11/0. The Phaselock cast hand moves with the arms; re-checked separately.

