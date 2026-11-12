## 2026-10-03: Phaselock presentation compared with the game: shader warm-up, capture clock, dark bubble core

AI-assisted (Claude). Host and tooling; no change to `src/`, `CMakeLists.txt` or package parsing. Smart App Control was
off and the module built and loaded. The host's Sanctuary dummy was compared with the 2026-10-02 game captures (a
bullymong in Three Horns). Details, causes and the remaining differences are in `docs/verification/PHASELOCK_STOCK_DATA.md`,
"Host presentation pass, second round". Frames and the side-by-side stay under ignored `local/phaselock/`.

- **Invisible hold bubble: a first-draw shader compile, not the effect data.** Every import recreates the host
  materials and compiles nothing (`-nullrhi`), so the next game run compiled them on first draw and skipped the
  translucent quads for about 2.5 s. An unchanged rerun drew the bubble on time. `FOwFxTemplate::Preload` now loads all
  Phaselock materials and meshes when the manifest loads and compiles the parent materials synchronously in editor
  builds. The first run after a fresh import then drew every effect on time (checked once, 17:55 run).
- **Capture clock.** Screenshot stalls (0.36 s, then about 0.13 s per shot) had moved the labelled shots late (the
  "0.25 s" frame was at +0.52 s). That made the arm look twice as fast as in the game and the hand orb look absent.
  `-owphaselockshots` now caps game time at 1/60 s per frame and logs each shot's actual time. With that, the arm
  timing matches the game to the eye. The stock data gives the cast clip `PlayRate` 1 (no `RateScale`); the caller's
  `SpecialMoveData` was not resolved.
- **Emitter playback (UE3 conventions, UNVERIFIED here):** spawn-time distributions are read at the emitter's time in its
  loop, and a linear sub-image layout follows the SubUV module's `SubImageIndex`. The second change removed the black
  wedge at the collapse: the end smoke now runs its frames 15 -> 0 instead of 0 -> 15.
- **Host stand-ins, chosen against the capture (UNVERIFIED):** all host FX parents render before DOF. The darkening
  modulates (`Mat_SirenOrbBlackMOD`, `_NoBias`, `Mat_SirenOrbEnergySpikesMOD`) are drawn as translucent black, because
  UE5 applies modulate apart from the additive layer. The textureless black orb uses a disc mask. `Mat_SirenGlowMOD`
  weights by alpha. The screen particle is a modulate by the colour's hue. The lock light reaches only the target,
  although its data (`LAC_DYNAMIC_AND_STATIC_AFFECTING`) says it lights the floor; the capture shows no pool. The
  hold now reads as a near-black core with a violet rim, the release as a cyan-white ring that shrinks, and
  1.2-1.5 s as a blue screen tint.
- **Open:** the game's dark blob around the raised hand at about 0.27 s and its solid blue palm orb (the host shows
  flashes and swirls); the 0.8 s intro burst draws as a white band; the screen tint starts at 1.05 s, against about
  0.82 s in game (a burst `Time` in seconds would fit; not changed); what `SphereCollapse` drives in the stripped graph.
- Also: the `OpenWillowPhaselock.h` lock-duration comment now cites the modifier stack read from native code instead
  of the old fitted rule; FX logs print mesh-particle scales with decimals and each quad's blend, sort priority and
  visibility.
- **Checks (automated):** quest suite first run PASS 79 checks / 0 errors and resume PASS 11 / 0; mover PASS 16 / 0;
  inventory actions PASS 45, FAIL 0, NOT_RUN 2, KNOWN_DIVERGENCE 2 (exit 1 from the NOT_RUN rows, as before); CTest
  10/10; `verify_packages.py` 9/9. **In-game check:** host frames compared by eye with the game captures only; no new
  game capture was made.

