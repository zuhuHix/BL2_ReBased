## 2026-11-12: Range den: baby collision, per-kind speeds and the adult swing cycle from the live read

AI-assisted (Claude Code). Record: `docs/verification/COMBAT_RANGE.md`, acceptance table rows 1, 5 and 10 and the brain rules'
Approach and Swing timing rows. Host behaviour, prep tool and tests; no parsing change.

Session 2 of 2026-11-12 recorded six den fights on Three Horns Divide with a level-8 Maya (SDK mods loaded, the three loot mods off
in memory). Three of the range den's stand-ins disagreed with it, and this change brings them into the host.

- **The baby's collision.** The baby kind borrows the adult's imported asset, so it had the adult's cylinder (radius 150) and stood
  at 192 uu, outside its live band of 106 to 118. `tools/prepare_range.py` now gives it its own cylinder, radius 64 and half height
  48 (`OBSERVED_COLLISION`), as every baby pawn read in the session had it. Its 189 swing calls all came at 106.0 uu or more from
  Maya, which is 64 plus her 42. The host's contact floor is now 106 standing, so the stand 111 applies. The baby still wears the
  adult's mesh, so its shot capsule is smaller than the body it shows.
- **Run and walk per kind.** The adult and the slinger run at 500 uu/s and walk at 125; the baby runs at 300 and walks at 75
  (`OBSERVED_SPEED`: the pawns' `GroundSpeed` and `WalkingPct` 0.25, with moving medians that agree: slinger 4 of 4 trials, adult 6
  of 11, baby 4 of 8). The regular bullymong keeps 400 and 150, and so does the psycho, which has no read. The range has no badass
  kind, so its 500 is not used.
- **The adult swing cycle is 1.222 s** (was 1.25). The earlier decision kept 1.25 because two reads disagreed, and waited for a
  third. The third read has 407 gaps over four recordings with medians of 1.220 to 1.223 s, on the side of the den read of
  2026-11-07 (1.216 to 1.217 s). The checker's band is now 1.211 to 1.228. Kinds without a row keep the 1.25 s stand-in.

Verified live (one map, one level, one session): the baby's cylinder 64 / 48 and the standing contact 106; `GroundSpeed` 500 / 300
and `WalkingPct` 0.25 for the adult, slinger and baby; the adult's cycle value at about 120 frames per second.

Tested in the host only: the light tests (`tests/range_tools_test.py`, `tests/range_kind_melee_test.py`) hold the manifest values,
the contact floor inside the baby's band and the host's cycle row; each fails if its value goes back. Nothing has run in the engine
yet, and the default range spawns no baby, so its stand row is not exercised there.

UNVERIFIED:
- the regular bullymong's chase speed. Its field reads 500, but its chase median was 375 in 4 of 6 trials, and nothing read explains
  that;
- the crouched contact floor, 85 (64 + 21), which was never observed;
- the rule behind the adult's cycle (clip length, rounding to frames, any dependence on the frame rate), because the session did not
  fix the frame rate. Why the older single-fight read had a median of 1.249 s is also open.
