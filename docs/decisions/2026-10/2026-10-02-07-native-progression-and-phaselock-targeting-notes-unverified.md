## 2026-10-02: native progression and Phaselock targeting notes (UNVERIFIED)

AI-assisted. Notes only, no code change apart from `tools/ghidra/class_layout.py` sizing Gearbox attribute properties
(it stopped at the first one before). Read locally from `Borderlands2.exe` in Ghidra and from script, written in our
own words; every rule is UNVERIFIED and each section of the notes names the in-game check that would confirm it.

- `docs/verification/NATIVE_PROGRESSION.md`: balance formulas evaluate `Multiplier x (Level^Power + Offset)`
  (`tools/weapon_recipe.py` and `tools/weapon_stats.py` put the offset outside; no slice number changes, a census is
  pending); mission XP is truncated, not rounded (395 at level 8 where the host gives 396) and the mission level is
  Sanctuary's region game stage (clamp(player level, 7, 9) on playthrough 1); the level curve is
  `max(0, trunc(60 x (n^2.8 + 7.33)) - 499)` (it reproduces the one real-game threshold on record, 2,715,586 at level
  46; the host is one point low at most levels), level cap 50; skill points `max(0, L - 4)` match the host; max health
  `max(20, 80 x 1.13^L)` matches the host (the 94 constant is never used).
- `docs/verification/NATIVE_PHASELOCK_TARGETING.md`: Phaselock takes the auto-aim strategy's instantaneous best
  target (screen-space magnetism cone, score favouring the crosshair then distance, line of sight to the aim point),
  where the host uses a view ray and a 30 cm sweep; reloading does not block the cast but putting a weapon away does;
  an injured Maya cannot cast and going down ends the lock; the lift bob is timed from the cast and smoothed by
  `VInterpTo` at speed 1 (about 16 units visible, not 30).

