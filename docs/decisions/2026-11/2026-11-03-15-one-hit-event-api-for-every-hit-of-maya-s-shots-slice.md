## 2026-11-03: one hit-event API for every hit of Maya's shots (slice projectiles lane)

AI-assisted (Claude). Record: `docs/verification/WEAPON_PROJECTILES.md` section 9. Host behaviour change, no new game rule.

- Every hit (bullet, trace, rocket splash, impact explosion, thrown gun, status tick) goes through
  `AOpenWillowWalker::ApplyShotDamage` as an `FOpenWillowHitEvent` (who, where, damage, damage type, element, crit inputs,
  momentum), delivered as the target's `TakeDamage` event (`FOpenWillowDamageEvent`, a point damage event) and then on
  `OpenWillowHit::OnHit()`.
- Explosions and status rolls now reach every actor that implements `IOpenWillowHitTarget`, not only the host dummy class,
  so the range lane's enemies take splash. The dummy implements it with its existing geometry and level (no number changes).
- Crits stay the victim's decision as in BL2 (hit region, base bonus 2.0 confirmed live); the event carries the bonus, the
  target sets the result. Splash crits are taken as none (UNVERIFIED). In-engine confirmation: the hub's firing-suite run.

