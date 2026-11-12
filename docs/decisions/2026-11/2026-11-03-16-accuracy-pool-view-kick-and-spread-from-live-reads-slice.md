## 2026-11-03: accuracy pool, view kick and spread from live reads (slice projectiles lane)

AI-assisted (Claude). Record: `docs/verification/WEAPON_PROJECTILES.md` section 10. Behaviour change in the host and the firing
tool; one real-game session (hub heavy slot, saves restored and hash-identical).

- **Confirmed live** (one gun, the save's Jakobs shotgun, with Maya's skills on; rules only, not per-gun values): the accuracy
  pool rises by the impulse at each shot, holds 0.2 s, then falls linearly to its floor. The kick is impulse x 182.0444, times
  `WeaponKickZoomMultiplier` when fully zoomed. The kick targets decay linearly to 0 over `WeaponKickRecoveryTime` and the
  current amounts chase them at exactly `WeaponKickSpeed` per second. The controller rotation never moves, so the kick is a
  returning view offset. `AddSpread` draws yaw uniformly and pitch uniformly on the chord.
- **Host:** `src/weapon_firing` `AccuracyPool` and `Kick` (CTest). `tools/weapon_firing.py` writes `accuracy_pool` (the pool
  definition scaled by the weapon's external slots and effects; UNVERIFIED per gun) and the full kick fields. The walker draws
  the pool cone once per shot (x 1 - zoom), adds the impulse and kick after it and applies the kick as a view offset. The
  firing suite runs without it unless `-owaimmodel` is given.
- **Not settled:** the zoomed yaw offsets (view wander, suspected), the kick's horizontal sign, the Hyperion alternative kick,
  per-gun pool values. The equal-level status chance came out about x1.8 of the card chance with skills on (section 10), so the
  level-scale reading of the older x2 is now in doubt; a skills-off rerun is queued.
- **PR #84 review 1 (same day):** the kick chase keeps the live per-frame linear step for parity (the exponential form does
  not fit the 8.00 measurement) and substeps frames over 1/30 s; signed kicks; non-finite and inverted inputs are guarded; pool and
  kick are gated separately; the aim model is off in every scripted shot or test mode unless `-owaimmodel`.

