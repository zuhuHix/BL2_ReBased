## 2026-10-03: Phaselock presentation round 5: bubble size from the confirmed rule, floor light, interior colour

AI-assisted (Claude). Host and tooling; no change to `src/`, `CMakeLists.txt` or package parsing. Details are in
`docs/verification/PHASELOCK_STOCK_DATA.md`, "Round 5".

- **Size rule, confirmed in game on 2026-10-03** (the real-game lane's SDK reads of the bubble emitters' `DrawScale` plus
  frames, three bullymong variants):
  - each bubble emitter's `DrawScale` = the lifted pawn's mesh bounds sphere radius at its own spawn / 66.7;
  - the loop's visible rim sits 48.6 uu per `DrawScale` unit.
- **Host calibration (UNVERIFIED cause):**
  - each template now takes its own draw scale at spawn;
  - all three bubble templates are drawn at 48.6 / (0.44 x the `Sphere` StartSize) of it, because the host drew the rim
    ridge at 0.88 of the `Sphere` half-width (about 1.8 times the game's size);
  - size check on host frames at the matched FOV: rim radius 79.7 uu at 1.5, 3.0 and 4.5 s against the expected
    48.6 x 1.640 = 79.7 uu.
- **Floor light:** it reaches the floor again, as its data says. The 10-03 game frames show a pale blue pool, which
  undoes the round-1 stand-in.
- **Interior (stand-in, calibrated against one capture):** the bubble's black orb blends toward a deep blue-violet
  instead of black, standing in for UE3's clamp after every blend. Host interior (38, 37, 84), close to the game's
  and slightly lower in blue. The hand orb stays black.
- **Timing:** first locks in game release at 4.608 s, which the host matches; no change.
- **Open:** the dummy stays lit in front of the smaller bubble; the floor pool is fainter on dark asphalt; the 0.6 s
  starburst.
- **Checks (automated):** quest suite first run PASS 79 checks / 0 errors and resume PASS 11 / 0; mover PASS 16 / 0;
  inventory actions PASS 45, FAIL 0, NOT_RUN 2, KNOWN_DIVERGENCE 2 (exit 1 from the NOT_RUN rows, as before); CTest
  10/10; `verify_packages.py` 9/9. **In-game check:** host frames compared with the matched-distance and size-rule game
  captures only.

