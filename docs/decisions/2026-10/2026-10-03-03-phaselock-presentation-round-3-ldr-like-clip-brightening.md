## 2026-10-03: Phaselock presentation round 3: LDR-like clip, brightening screen tint, burst time in seconds

AI-assisted (Claude). Host and tooling; no change to `src/`, `CMakeLists.txt` or package parsing. This round follows an
independent critic's score of 4.5/10 for the round-2 side-by-side. Details are in `docs/verification/PHASELOCK_STOCK_DATA.md`,
"Round 3". Frames and side-by-sides stay under ignored `local/phaselock/`.

- **Common cause checked first.** Auto-exposure is already off project-wide; the difference is UE5's filmic tone curve
  against UE3's per-channel clip. Host stand-in (UNVERIFIED): additive FX layers cap each channel at 1, translucent ones
  at 1 / opacity, so that HDR particle colours stay saturated instead of turning white. The global tone mapper is
  unchanged.
- **Screen effect.** Its tint now divides the colour by its luminance, so it brightens toward blue instead of darkening
  to grey-brown (stand-in). Burst `Time` is now read as seconds of emitter time (UNVERIFIED). Only the screen burst
  moves (1.05 s -> 0.70 s), matching the game's tint onset of about 0.82 s.
- **Modulate readings (stand-ins).** `Mat_SirenGlowMOD` reads its colour as a brightness-keeping tint (cobalt cast
  flashes, as in the game frames). `Mat_SirenOrbBlackMOD` reads darkness as mask x (1 - alpha), the one reading that
  fits its three emitters. It adds the game's dark blob around the raised hand at about 0.27 s and keeps the bubble's
  dark core. Its host disc is softer (full to half the radius), so the hold rim reads violet-magenta.
- **Not changed:** the bubble size (stock draw-scale rule; a matched-distance game capture or the bullymong's bounds
  radius is needed to compare) and the palm orb size (it follows the hand, which is about 2.5x smaller on screen in
  the host: arms placement or FOV). The shots add a 5.0 s capture.
- **Open:** the release ring is blue-violet where the game's is cyan-white (the same `Mat_SirenGlowMOD` reading that
  fixes the cast flashes weakens it); the 0.6 s white starburst; the dummy's lifted pose (no stock clips);
  `SphereCollapse`.
- **Checks (automated):** quest suite first run PASS 79 checks / 0 errors and resume PASS 11 / 0; mover PASS 16 / 0;
  inventory actions PASS 45, FAIL 0, NOT_RUN 2, KNOWN_DIVERGENCE 2 (exit 1 from the NOT_RUN rows, as before); CTest
  10/10; `verify_packages.py` 9/9. **In-game check:** host frames compared by eye with the 2026-10-02 game captures
  only; no new game capture.

