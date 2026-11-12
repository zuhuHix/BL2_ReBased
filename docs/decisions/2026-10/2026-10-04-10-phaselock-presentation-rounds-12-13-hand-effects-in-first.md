## 2026-10-04: Phaselock presentation rounds 12-13: hand effects in first-person space, saturated swirl, deeper orb

AI-assisted (Claude). Host and tooling; no change to `src/`, `CMakeLists.txt` or package parsing. Details:
`docs/verification/PHASELOCK_STOCK_DATA.md` ("Round 12", "Round 13"). All calibrations UNVERIFIED.

- **First-person space:** with the weapon lane's `-owfpfov` (the game's separate foreground FOV for arms and gun) the
  hand sprites render as first-person primitives, so they stay on the hand; the bubble, screen effect and light stay in
  world space. With it the swirl, disc and orb are about game-sized.
- **Round 12's "enlarging the void fades it"** was a misreading: the wider core did grow, but its soft gradient let
  the street show through. The normal-FOV widening and a thick rim halo were tried and reverted after a blind A/B.
- **Hand look:** energy swirl alpha 0.8 with a more saturated cyan; palm orb a deeper blue with its texture veins.
- **Diagnostic:** `-owfxscalar` sets one effect material scalar per emitter at run time (TOOLING).
- **Visual review:** round 12 6/10, round 13 6/10. Blind A/B: round 13's hand beat round 12's (both close); the bubble
  and release were close to ties with rounds 11 and 12. Scores have stayed between 5.5 and 6.3 since round 7.
- **Open:** near-black violet void and interior, a compact ground disc, side streaks, thick release ribbons, orb vein
  cracks, the forward fist, the 0.80 s whiteout, ice-shard facets, target animation.
- **Checks:** quest suite with `-owfpfov=45` 79/0 and 11/0 (via a local copy of `tools/test_quest.ps1` that adds the
  flag).

