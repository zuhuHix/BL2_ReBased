## 2026-10-05: Sanctuary ambient citizens round 5: ink line sized in pixels and on worn pieces

AI-assisted (Claude). Host and tools (`OpenWillowAmbient.cpp/.h`, `prepare_ambient_world.py`,
`ambient_npc_attach_editor.py`, `seed_ambient_npc_assets.ps1`); no change to `src/`, `CMakeLists.txt` or package
parsing. Details: `docs/verification/SANCTUARY_AMBIENT_NPCS.md` section 11.

- The ink hull was a fixed 0.5 cm, about 1 px at tour distance; the real game's line looks 2-3 px at any distance
  (by eye on one frame). The hull thickness now follows the camera distance for a target of 3 px (`UNVERIFIED`), and
  hats, hair and packs get their own hull. `-Steps outline` rebuilds only the ink material (a failed editor run had
  deleted it during this round; rebuilt).
- The floating "hair bun" at stop 00 is the raised fist of the wall-bang clip seen behind the head, not a worn piece.
  Whether the fist meets the wall depends on the real perch's wall distance (`UNVERIFIED`). Packs confirmed on the
  back from behind.
- **Visual review:** a blind A/B critic preferred round 5 over round 4 on every stop, about 6.0 against 3.4 (absolute
  scores drift between critics). This lane stops here. Open: line width still uneven (1.3-5 px) and navy rather than
  black, a gap between the line and the body on thick lines, creases drawn inside the silhouette, a flat untextured
  cap and a featureless face from above, body garment variants, Resistance patrols, matched real close-ups.
- **Checks:** `tools/test_ambient.ps1 -Seconds 60` PASS (35 pawns); quest suite with ambient on and off 79/0 and
  11/0; CTest 10/10; `verify_packages` exit 0; `tests/ambient_transform_test.py` OK; module build exit 0.

