## 2026-10-03: Phaselock presentation round 4: matched-distance comparison, depth-biased darkening, release override

AI-assisted (Claude). Host and tooling; no change to `src/`, `CMakeLists.txt` or package parsing. The host is compared
with the matched-distance game capture (an Adult Bullymong at 650 uu, horizontal FOV 77.55 degrees) at the same
distance and FOV (`-owfov=62.15`). Details are in `docs/verification/PHASELOCK_STOCK_DATA.md`, "Round 4".

- **Stock data:** `Mat_SirenOrbBlackMOD` has a `DepthBias` parameter (-20), `Mat_SirenGlowMOD` a `Bias` (-15) and the
  smoke a `DepthBias` (-18). The host reads a negative bias as a camera-ward offset of the sprite plane, with the size
  scaled to keep the outline (UNVERIFIED). The black orb now covers the lifted target's front.
- **Darkness, calibrated against the one capture (stand-in):** the black orb is fully dark to 0.6 of its radius (thin,
  dim rim), with darkness capped at 0.9 on opaque geometry just behind it (the game keeps the target at about a third of
  its brightness) and at 0.96 over the background. The second cap compensates for UE5's float target, where the
  additive layers sum to about 2.5 before the black, against UE3's clamped 8-bit target.
- **Release:** a labelled per-emitter override gives the end template's `Brighten` the plain multiply. The release is a
  cyan-blue ring; the cast flashes keep the brightness-keeping tint.
- **Size: not changed, not confirmed.** The stock rule (mesh bounds sphere radius / 66.7, full-width sprites) puts this
  bullymong's rim at about 356 uu, while the capture measures 167-174 uu. Two readings fit and cannot be separated with
  this enemy, so the report carries a capture request for a second enemy type with an SDK read of the bubble emitter's
  draw scale.
- **Open:** the release timing (game 4.77 s in this capture, host 4.60 s; not changed); the floor glow seen in the
  matched frames against the light-channel stand-in; the magenta-leaning interior; the 0.6 s starburst; the host's
  default capture FOV (106 degrees against the game's 77.55).
- **Checks (automated):** quest suite first run PASS 79 checks / 0 errors and resume PASS 11 / 0; mover PASS 16 / 0;
  inventory actions PASS 45, FAIL 0, NOT_RUN 2, KNOWN_DIVERGENCE 2 (exit 1 from the NOT_RUN rows, as before); CTest
  10/10; `verify_packages.py` 9/9. **In-game check:** host frames compared by eye with the matched-distance game capture
  only.

