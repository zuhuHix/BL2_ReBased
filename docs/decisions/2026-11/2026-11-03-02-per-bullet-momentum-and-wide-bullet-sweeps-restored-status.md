## 2026-11-03: per-bullet momentum and wide-bullet sweeps restored; status chance level scale; firing suite runs again (Weapons)

AI-assisted (Claude). Record and numbers: `docs/verification/WEAPON_PROJECTILES.md` sections 2-5.

- **Maintainer decision (2026-11-02).** Re-add per-bullet momentum and the wide-bullet hit sweep on top of main's
  part-driven firing (#56), dropped in the merge of the earlier branch.
- **What.** `tools/weapon_firing.py` writes `instant_hit_momentum` (weapon type, class default 10 as fallback), the splash
  `momentum` (`MomentumFormula`) and `status.level_chance_scales` (`GD_Globals.General.Globals.StatusEffectChanceScales`).
  The host multiplies the momentum by 100 (`weapon_firing::hitMomentum`), passes it with every direct, trace and splash hit
  (splash x the radius scale) and changes the dummy's velocity by momentum / `Mass` (`Default__Pawn` 100,
  `weapon_firing::momentumVelocity`); the dummy slides, slowed by `GroundFriction` 8 as v x (1 - 8 dt) per frame (a host
  stand-in). A firing mode's `TraceExtent` turns light-bullet ticks and traces into box sweeps (`OpenWillowFiring::SweepShot`).
  The status roll multiplies the card chance by the level-difference row (`weapon_firing::statusLevelScale`: Maya's level
  against the target's; `-owtargetlevel=N` sets the dummy's, without it the factor is 1).
- **Retired.** `tools/weapon_refresh_firing.py` (the L7 live-dump firing block), its test and its `add_test` line in
  `CMakeLists.txt`; the cooked firing block replaces it. `tools/weapon_firing_coverage.py` regenerates the firing test's
  coverage recipes and firing blocks on a fresh machine.
- **Firing test.** New `OWFIRING push` (momentum rule, every hit's velocity change, the dummy moved), `OWFIRING wide` (a pull
  beside the dummy misses as a line and hits with a host-chosen 24 uu box; single straight non-burst bullets only, its two
  pulls counted in the shots) and the restored flight-timing check (`timed` / `on_time`). `tools/check_weapon_firing.py`
  grades them, including each splash hit's momentum against the recipe x radius scale; the new lines are required unless
  `--legacy` is passed. A dead target stops sliding; a respawned one starts at rest.
- **Verified / UNVERIFIED.** The momentum values match live reads (pistol 8, shotgun 7, Torgue 10, rocket splash 10,000).
  The x100 factor is read from native code, the effect on a pawn and the friction are not measured: UNVERIFIED. No local
  recipe has a `TraceExtent`, so the sweep is checked with a host box only. The level scale is consistent with the live
  Maliwan counts (about 2 x the card chance at Maya 54 against a level-7 bullymong); a same-level live check is still to do:
  UNVERIFIED. All of it is host behaviour checked in the host, not parity with the running game.

