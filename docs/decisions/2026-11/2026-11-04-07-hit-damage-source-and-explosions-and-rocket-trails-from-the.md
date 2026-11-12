## 2026-11-04: hit damage source, and explosions and rocket trails from the cooked particle systems (slice projectiles lane)

AI-assisted (Claude). Records: `docs/verification/WEAPON_PROJECTILES.md` sections 9 and 11. Host behaviour change and a new
data tool; no live session.

- **Hit source (skills lane request).** `FOpenWillowHitEvent::Source` (`EOpenWillowHitSource`) says what dealt the damage,
  separately from `Kind`. A rocket's explosion is Rocket with Kind Splash; a bullet's impact explosion is Explosion (the game
  sends it as a grenade source); the default is Unknown. The firing checker requires every firing path to set one.
- **Particle data.** `ExplosionDefinition.ExplosionScale` decodes as `ExplosionScaleData` radius bands, each naming its
  particle system and scale. `tools/weapon_projectile_fx.py` writes the tracer, trail and explosion layers into the recipes.
  The host draws the band for the explosion's radius, and the rocket trail, as camera-facing quads with the cooked values.
  The band choice and the drawing are UNVERIFIED; the critic round is pending.

