## 2026-10-04: Phaselock presentation round 8: release size, hand timing, interior measured against the game

AI-assisted (Claude). Host only; no change to `src/`, `CMakeLists.txt` or package parsing. Details:
`docs/verification/PHASELOCK_STOCK_DATA.md`, "Round 8".

- **Release "does not collapse":** this was a test-aid error, not an effect rule.
  - No bubble shader reads the collapse value. The game sizes each bubble template from the pawn's mesh bounds at that
    template's spawn, and the bounds shrink as the pawn is lifted: 290 uu at the lock and 193 uu lifted, read with the
    SDK driver in the real game.
  - `-owbubbleradius` now takes one value per template (`290,260,185` stands in for an adult bullymong).
- **Hand (host calibrations, UNVERIFIED):**
  - The effect starts 0.08 s before the clip's notify.
  - The cast clip plays at 0.85 speed (`PlayAction` gained a rate argument).
  - Orb texture ×0.6 and star-burst alpha ×0.2.
  - Basis: the game frames show an opaque palm orb at 0.44 s and the arm dropping about 0.05 s later than the host's.
- **Interior opacity, measured on the game frames:** about a quarter of the background's contrast survives inside the
  bubble (regression slope 0.26 at 1.5 s, 0.21 at 3.0 s).
  - The host darkening caps (0.72 loop, 0.45 end) bring the host to 0.11 / 0.26.
  - The host bubble centre in the measurement is approximate.
  - The method and its script are kept local.
- **Floor light:** wider, softer and paler (UNVERIFIED calibration).
- **Visual review:**
  - An independent critic gave an absolute score of 5.5/10, against 6/10 for round 7.
  - A blind A/B critic, judging against the same game frames, preferred round 8 on the hand and release sheets and called
    the bubble sheet a tie.
  - Absolute scores from separate critic runs vary by about a point, so the A/B result decided the commit.
  - Remaining gaps:
    - the bubble is about 20% large and sits high-left;
    - the floor glow is now too faint;
    - the 0.30-0.40 s black sphere and swirl are small;
    - the tattoos are overexposed after 0.55 s;
    - no ice-shard flash facets;
    - straight release shards;
    - no fist clench;
    - no target animation.
- **Checks:** quest suite first run PASS 79/0 and resume PASS 11/0 on the round-8 build; CTest and `verify_packages`
  were not rerun for this commit (no `src/` change).

