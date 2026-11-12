## 2026-10-04: Phaselock presentation rounds 6-7: effect materials from own-words notes on the compiled shaders

AI-assisted (Claude). Host and tooling only; no change to `src/`, `CMakeLists.txt`, the reader or package parsing.
Details: `docs/verification/PHASELOCK_STOCK_DATA.md`, "Round 6" and "Round 7".

- **Method:** the cooked effect-material graphs are stripped. Their compiled pixel shaders in
  `RefShaderCache-PC-D3D-SM3.upk` were read with `research/d3d9_bytecode.py`, the same method as the weapon paint model
  (2026-10-02). What each one computes was written down in our own words, and the 17 effect materials in
  `host/ue5/import_phaselock_fx.py` were written from those notes as UE Custom nodes, with plain constants and prose
  comments. The listings stay under ignored `local/`.
- **Findings that replace earlier rounds** (UNVERIFIED in game unless compared below):
  - `DepthBias` is a soft-particle fade distance, not a camera-ward shift.
  - The bubble sphere is a warped read of the bubble texture.
  - The tattoo mask is the B channel of one quadrant of the mask texture; rounds 1-5 lit the whole sleeve.
  - The bubble ring sits at 0.78 of the sprite half-width.
  - The sigil under the target is HUD, not an effect.
- **Host calibrations (UNVERIFIED):**
  - bubble warp strength driven by the collapse value;
  - core haze ×3;
  - layer colours above 1 normalised;
  - blue tints on the rim and haze;
  - a per-emitter darkening cap and fade floor;
  - floor light gain ×4 (the data's brightness is still what `PhaselockLightIntensity()` reports);
  - lighter swirl, star-burst and brighten.
- **Visual review:** an independent critic agent compared matched host and real-game frames. It scored round 6 at 5/10
  and round 7 at 6/10 (hand 6, bubble 7, release 5). Largest remaining gaps:
  - the release does not collapse the sphere;
  - the palm orb is lost around 0.55 s;
  - the interior is more see-through than the game's.
- **Target animation:** the stock clips exist only on enemy AnimSets and the dummy has none, so it is still not
  exercised.
- **Checks:**
  - CTest 10/10; `verify_packages` 9/9 match; `particle_system_test` OK.
  - Quest suite first run PASS 79/0, resume PASS 11/0 on the round-7 build.
  - Mover and inventory suites not run for this lane (untouched areas); the end-of-session run covers them.

