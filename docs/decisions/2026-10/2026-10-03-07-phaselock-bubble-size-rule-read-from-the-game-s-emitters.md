## 2026-10-03: Phaselock bubble size rule read from the game's emitters (real-game lane)

- **What.** A follow-up to the matched 650 uu capture, requested by the Phaselock lane: read the spawned bubble emitters at lock
  time on three enemies (Baby, Adult and Ranged Bullymong, Ice_P, level 8 Maya, saving blocked, saves restored byte for byte) and
  measure the rim in frames. New `tools/real_game/scripts/phaselock_size_rule.py` (probe), `tools/real_game/bubble_frames.py`
  (frames at chosen times and a rim width), `aim()` in `phaselock_matched.py`. Frames, probe lines and notes stay in ignored
  `local/realgame/phaselock/size_rule/`.
- **Measured in game (one map, level and skill build).** The two bubble emitters of one pawn (`Part_SirenASEnemyOrbBegin` at lock,
  `Part_SirenASEnemyOrb` 0.2 s later) have different `DrawScale` values (adult 4.2825 and 3.9026, ranged 4.3144 and 3.7926, baby
  1.8181 and 1.9218) while the collision radius never changes (150, 150, 64). The intro value times `BubbleFXScale` (66.7) is within
  1 to 2.3 percent of the pawn's mesh bounds sphere radius read at +0.72 s (285.6 against 290.3, 287.8 against 290.0, 121.3 against
  124.1). So the size input is the mesh bounds sphere at spawn time, not the collision radius. `DrawScale3D` and the particle
  component's scale are 1 on every emitter. The visible rim radius per unit of the loop emitter's `DrawScale` is 47.6, 48.6 and 49.5
  uu on the baby, adult and ranged runs (about 0.73 times 66.7): a constant of the particle template, not of the enemy.
- **Limit.** All three pawns have mesh sphere over collision radius of 1.93 to 1.94 at the lock pose (the same animation), and no
  enemy with a very different ratio (a Skag, a Brut) was on the map, so size ratios alone do not separate the readings; the
  emitters changing with the pose and the baby's half-size bubble following its 0.5 mesh scale do.
- **Timing, game clock.** Lift 0.70 s, hold 3.9 s, release 1.1 s (`LockFadeOutTime`), duration 5.7 s; release began 4.608 s after the
  skill start in all three casts. The earlier matched capture read 4.786 s (hold 0.175 s longer, on a second lock of the same pawn;
  cause not read, the skill's tick rate is 0). `LockDurationFormula` is a 7.0 constant plus the designer attribute
  `Att_Phaselock_Duration` (its value was not read).
- **Method findings.** A cast at pitch 0 did not lock a baby at 650 uu (no target within the auto-aim); aiming the crosshair at the
  target's origin did. Several pawns were found dead or gone after their neighbours' controllers had been detached (cause not
  read), so one capture needs its setup and cast within seconds.
- **Not done.** No host comparison; a Skag or Brut; other levels; the duration attribute's value.
- **Checks.** None automated (game captures only; no code under src/ or host/). No sensitive files touched.

