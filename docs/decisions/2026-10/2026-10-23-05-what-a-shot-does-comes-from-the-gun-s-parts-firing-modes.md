## 2026-10-23: What a shot does comes from the gun's parts: firing modes, light bullets, rockets, splash, status effects (Weapons 3)

AI-assisted (Claude, with Claude and Codex sub-agents). Record and numbers: `docs/verification/WEAPON_PROJECTILES.md`;
native notes: `docs/verification/NATIVE_WEAPON_FIRING.md` (new, own words, raw output under ignored `local/analysis/projectiles/`).

- **What.** `tools/weapon_firing.py` adds a `firing` block to any evaluated recipe, resolved from its parts (barrel or type
  firing mode, `FireType`, pattern lines, projectile definition and behaviours, the explode behaviour's splash with the
  damage type's falloff bands, the status effect, the Tediore reload throw). The host reads that block
  (`FOpenWillowWeaponItem::Firing`, one new field) and fires accordingly: `src/weapon_firing.*` (rules as maths, CTest
  `weapon-firing-math`), `OpenWillowFiring`, `OpenWillowProjectile`, `OpenWillowStatusEffect`, and a smaller
  `OpenWillowWalker::FireWeapon` (one direction per pellet; trace, light bullet or rocket). No per-gun code.
- **Behaviour change.** Bullets now take time to arrive (light bullets, as in the game: damage when they reach the
  target); the card spread and pattern place the pellets (the old fixed figure-eight is gone; the Infinity fires along
  its own pattern lines); `AutomaticBurstCount` 1 guns (slice pistol and rifle) fire once per press; only
  `BSM_SpinUpToFullFireRate` ramps the fire interval (the former wait for the spin-up on other modes is gone). The
  quest suite's two shot checks now wait for the hit.
- **Confirmed in game** (three bl-sdk sessions, skills off, saves byte-identical): light bullets fly straight at their
  speed; `AirFriction` grows speed x(1 - F dt) per frame; rocket speed = `SpeedFormula` x projectile speed multiplier
  (the tool's first version left the multiplier out: fixed); weapon rockets do not fall, thrown guns fall at
  GravityScaling x 1,000; per-pellet damage = card on 4 of 5 guns; rocket and gyrojet splash has no falloff, the Tediore
  throw's follows the damage type's bands; the DOT clock (0.33 s, DPS x built-up time, final partial tick); the Tediore
  throw (card damage x rounds left, half direct, half splash).
- **UNVERIFIED / open.** The spread distribution (no impact points could be logged); the status proc rate (game 2-3 x the
  card chance on Maliwan pistols; the host uses the card chance); elemental impact explosions of elemental and Torgue
  bullets, element-vs-surface factors, slag, beams, ricochets, homing and kick are not modelled; host shot effects are
  additive stand-ins, not BL2's particle systems: matched-frame critic (Codex) 2.1, 3.9, 4.2 / 10 over three rounds,
  stalled; the rest needs the stock particle templates.
- **For other lanes.** The Dahl AR's card damage is 9.5 % below the live gun with the same parts (generation lane). A
  small `AOpenWillowShotFx::Spark` and an `Intensity` argument on `Flash` were added for impacts and explosions.
- **Checks.** See the PR: CTest, packages, Python tests, in-engine firing test, quest and action-skill HUD suites.

