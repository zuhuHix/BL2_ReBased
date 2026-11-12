## 2026-10-03: Phaselock at a matched 650 uu in the real game (real-game lane)

- **What.** A requested capture for the Phaselock lane: the real game (Ice_P, level 8 Maya, saving blocked, saves restored byte for
  byte afterwards) casting Phaselock at one Adult Bullymong placed 650.0 uu away (horizontal; 652.9 in 3-D), every other enemy
  held still at least 2,500 uu off. New `tools/real_game/scripts/phaselock_matched.py` (list, bounds, isolate, place, measure,
  mark_lock); `phaselock.py` now samples the skill instance that moves (see below). Frames, sampler lines and a notes file stay in
  ignored `local/realgame/phaselock/matched_650/`.
- **Measured in game (one enemy type, one level, one cast pair; not a rule).** The bubble is 404-427 px across on the 1280x720
  frame, about 0.32 of the width: radius 167-174 uu at 650 uu depth (horizontal FOV 77.55 degrees, focal length 796.7 px). That is
  about 1.1 times the target's collision radius (150) and 0.55-0.58 times its mesh bounding sphere (300.8; the sphere moves with
  the animation: 241.9 and 315.3 on other reads). Inside the rim the view is deep violet-blue, with a near-black darkest tenth; the held target stays visible at roughly a third of its normal brightness. Timeline from the per-frame
  sampler, measured from the skill start (15 ms after the key): lift to 0.70 s (target 190 uu higher), hold to 4.77 s, release to
  5.88 s; the screen vignette is up at 0.25 s, the hand orb at 0.5 s, a cyan-white burst at 0.8 s, the bubble from about 1.5 s,
  gone by 5.0 s. A second cast of the same setup (run1) agrees by eye.
- **Method findings.** `SetLocation()` returned False for AI pawns and for Maya and moved nothing; assigning `Location` moves
  her (the capture uses that) and `Destroy()` removed nothing. A target whose AI controller was detached was not locked: the
  skill ended at once and the target took damage (cause not read from script). Freezing pawns with `CustomTimeDilation = 0`
  held them only for a while. What worked: detach the controllers of every other pawn, keep the target's controller and set its
  `GroundSpeed` to 0. The sampler's `lift_skill()` took the last listed `LiftActionSkill`, which was the idle data object; it now
  takes the instance whose `SkillStartTime` is largest (the world instance moves; the `GD_Siren_Skills` object stays at 0).
- **Not done.** No host run compared with these frames; other enemy sizes and distances; a cast at other levels.
- **Checks.** None automated (game captures only; no code under src/ or host/). No sensitive files touched.

