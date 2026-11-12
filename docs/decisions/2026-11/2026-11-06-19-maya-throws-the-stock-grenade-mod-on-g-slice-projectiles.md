## 2026-11-06: Maya throws the stock grenade mod on G (slice projectiles lane)

AI-assisted (Claude). Records: `docs/verification/WEAPON_PROJECTILES.md` sections 12 and 13. Host behaviour change and a new data
tool. **The stock frag has not been thrown live:** the one session's save carries the Sky Rocket mod, so its release delay, lockout
and spawn offset are used as UNVERIFIED stand-ins for the frag, and none of its fuse, damage or radius is recorded as frag values.

- `tools/grenade_mod.py` resolves the stock grenade mod's speed, gravity, bounces, radius, damage and explosion into ignored
  `local/items/grenades/`. The fuse is not settled by the data: the part's slot gives 0 and the projectile holds two timers.
- The host spends one grenade from the pool and launches a falling projectile with those values (hit source `Grenade`). The
  launch point, the lockout between throws, the falloff and the missing animation are stand-ins, labelled UNVERIFIED.
- The firing test throws one grenade at the dummy; the checker requires the result.
- **Status chance:** x1 at equal levels is consistent with a skills-off live run (8 of 42 against 23.4 %; small sample, different
  class). The mechanism behind the skills-on ~x2 stays UNVERIFIED (it is not in the five controller modifier fields), and the
  higher-level scale is unmeasured.


