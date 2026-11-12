## 2026-10-05: Sanctuary ambient citizens round 4: the attachment frame, read from a live sample

AI-assisted (Claude). Tools and a synthetic test (`prepare_ambient_world.py`, the real-game `ambient_npcs.py` script,
`tests/ambient_transform_test.py`); no change to `src/`, `CMakeLists.txt`, package parsing or host C++. Details:
`docs/verification/SANCTUARY_AMBIENT_NPCS.md` section 10.

- **Cause of floating packs, disc heads and sideways hats:** the bone frame and the worn piece's own transform were
  combined in the wrong convention. One live sample (bone world matrices of two bones on 33 pawns, saves backed up
  and restored, driver removed) showed that the extracted bone frames are the live ones turned 180 degrees about the
  bone's X axis, and that imported static meshes keep the original coordinates while the imported skeleton is
  mirrored. Only the new composition puts the pack behind the middle of the back with the live matrices. The result
  is a reflection, carried as one negative scale; the synthetic test checks the decomposition (4 OK, needs numpy).
  `UNVERIFIED` beyond that one capture and two bones; the helper's attempt to read the pieces' own world matrices
  failed, so the live bounds were not captured.
- Palette and idles unchanged (palette still `UNVERIFIED`; the night-lit blue-grey in some real frames is lighting).
- **Visual review:** a blind A/B critic preferred round 4 over round 3 on six of seven stops (one tie), about 6.6
  against 4.4 per stop. Main gaps: a hair bun floating behind one head, a background pawn's mask, flat faces from
  above, weak ink outline, packs unconfirmed on front-facing pawns.
- **Checks:** `tools/test_ambient.ps1 -Seconds 60` PASS (35 pawns); quest suite with ambient on and off 79/0 and
  11/0; CTest 10/10; `verify_packages` all packages matched.

