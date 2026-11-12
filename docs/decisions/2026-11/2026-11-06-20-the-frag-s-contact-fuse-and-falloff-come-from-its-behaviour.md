## 2026-11-06: The frag's contact, fuse and falloff come from its behaviour graph

AI-assisted (Claude). `tools/grenade_mod.py` now reads the projectile's behaviour graph by event, because counting behaviours gave the
wrong grenade: it reported two bounces and an impact detonation. The graph says a contact only bounces (0.4 off the world, 0.2 off a
damagable object), the first contact starts a 1.8 s timer once, the spawn starts a 6 s timer, a timer detonates, and the explosion has no
falloff. The host follows that and refuses a block without these rules.

Verified live (one session, 4 throws of the frag's definition with the Sky Rocket mod still equipped): no contact exploded, the explosion
came 1.80-1.88 s after the first contact (frame-sampled, consistent with but not separately proving the start at contact), bounces lost
about the graph's speed, gravity read 0.8 x 1000, and pawns at several distances took the same damage. 63.28 at `InExpLevel` 1 matches the
damage formula at level 1 only. `UNVERIFIED`: a throw with the stock mod equipped (level source, the -120 radius slot, so the block's level 8
damage and radius), the 6 s timer, the FuseTime slot's effect, and what the first contact touched. The 20 uu/s rest limit is a host stand-in.
Host suite results: pending the next integration batch. Details: `docs/verification/WEAPON_PROJECTILES.md` section 14.
