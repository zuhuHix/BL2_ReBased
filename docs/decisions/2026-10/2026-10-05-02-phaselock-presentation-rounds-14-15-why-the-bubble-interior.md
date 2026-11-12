## 2026-10-05: Phaselock presentation rounds 14-15: why the bubble interior read navy, violet interior

AI-assisted (Claude). Host and import script only (`OpenWillowPhaselockFx.cpp`, `import_phaselock_fx.py`); no change to
`src/`, `CMakeLists.txt` or package parsing. Details: `docs/verification/PHASELOCK_STOCK_DATA.md` (Round 14).

- **Cause:** blend mode, draw order and the colour the template feeds the dark layer all match the game (checked
  with `-owfxscalar` and against the own-words shader notes). The navy, see-through interior came from the host's own
  round 5-8 compensations (dark-layer cap and fade floor, a blue-tinted haze, a blue ring tint).
- **Changes (UNVERIFIED calibrations):** the ring texture's dim disc is tinted violet while its bright rim keeps the
  white-blue tint; dark-layer cap 0.8 and fade floor 0.65; the purple wisps at alpha x0.6 and colour x5. A compact
  ground light (round 14) made the ground disc disappear and was reverted in round 15.
- **Visual review:** a blind A/B critic preferred round 14's interior but not its missing ground disc; round 15 against
  round 13 was a tie on the bubble and release sheets and narrowly better on the hand sheet (about 6/10). This lane
  stops here. Open: interior still too translucent, side streaks (the streak sprites take a random rotation whose
  rule was not read), the 0.80 s whiteout, straight release shards, ice-shard flash, fist clench, target animation.
- **Checks:** quest suite with the foreground FOV default 79/0 and 11/0; CTest and `verify_packages` unaffected
  (no `src/` change) and green in this session.

