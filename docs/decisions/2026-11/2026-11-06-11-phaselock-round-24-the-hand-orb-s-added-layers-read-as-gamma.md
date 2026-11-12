## 2026-11-06: Phaselock round 24: the hand orb's added layers read as gamma-space colours (maya lane)

AI-assisted (Claude). Record: `docs/verification/PHASELOCK_STOCK_DATA.md` "Round 23 results" and "Round 24". Presentation change.

- Round 23 scored 5.9 / 10 as a best-per-time mix of two facings (round 22 rescored 5.7); target 8 not reached. The target's fur now reads
  tan and brown. The orb's blue-constant conversion had no measurable effect (host 40 px mean 159, 208, 251 to 158, 208, 251; the game's orb is markedly darker in red and green).
- With the tone curve off the orb reads 10, 158, 253: most of its paleness is the film curve on an over-bright blue, not its own colour.
- The hand orb's ribbons, swirl and star-burst read their colour as gamma-space values (a `GammaOut` material parameter, set by the hand
  template's emitters only). UNVERIFIED until the capture, which also retests the tone curve off on the corrected target.
- Facing stays open: critics prefer 0 for the held frames, the default stays 180 as a choice, and the live probe now records the target's rotation.
- Result (last critic round, each setting scored on its own): offset 0 about 5.7, the default offset 180 5.1, tone curve off 5.3; target 8 not reached.
  `GammaOut` showed no gain over round 23.

