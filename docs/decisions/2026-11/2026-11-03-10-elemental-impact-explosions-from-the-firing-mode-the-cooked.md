## 2026-11-03: elemental impact explosions from the firing mode, the cooked muzzle flash and casing (Weapons parity 2)

AI-assisted (Claude). Records: `docs/verification/WEAPON_PROJECTILES.md` section 8, `WEAPON_VISUALS.md` section 18,
`NATIVE_WEAPON_TRIGGER_AMMO.md` (`ProcessInstantHitBullet` branches). The planned live session could not start on this PC
(Borderlands 2 stopped at a "Steam must be running" message box), so nothing here was checked against new live reads.

- **Impact explosions (behaviour change).** A trace or light-bullet firing mode whose own behaviours hold a
  `Behavior_Explode` explodes at every impact after the bullet's damage; `ExplosionOverrideDefinition` only picks the
  explosion definition (so the damage type) per element. Source: a clean-room native read (Ghidra, read-only; own-words
  note under ignored `local/`) and the cooked data. Only the Maliwan firing mode has one among the regenerated recipes:
  radius 106, 0.8 x weapon damage, momentum 1000, no falloff. It reproduces the one live number (45.30 x 0.8 = 36.24);
  the rest is UNVERIFIED in game. `tools/weapon_firing.py` writes `impact_explosion`; the host plays it as the gun's
  splash (light bullets and traces); shock, corrosive and slag explosions draw a small element-coloured burst.
- **Muzzle flash and casing (behaviour change, host look).** `tools/weapon_muzzle_fx.py` reads the type's muzzle-flash
  particle systems (per element) and casing system from the cooked data into `weapon_view.json`; the host draws each
  emitter as a soft quad with the cooked size, offset, life, colour and axis lock. The lookup-table layout is fitted and the
  quad fills are host fits (UNVERIFIED). Only the assault rifle type has a casing system; it now uses the cooked life,
  speed and gravity.
- **Pistol ADS fit.** The side term of the pistol's frame fit went from -3.2 to -0.6 cm (critic measurement); UNVERIFIED.
- **Tests and tools.** Checker: the impact explosion counts as the gun's splash; the firing test refills the dummy per
  segment; quest check `lent_pistol_shot_carries_its_damage_type_to_dummy` now expects the bullet hit first and, for an
  exploding gun, the explosion hit after it. New `tools/real_game/weapon_parity2_capture.ps1` and its probe (not yet run to
  completion).
- **Results.** Critic 6.3 -> 6.5 / 10 (firing 6.1 -> 6.5); target 8 not reached. Firing suite 14 PASS / 0 FAIL after review
  round 1 (the round-0 Tediore failure came from the test's per-segment dummy refill: a dummy that never died never respawned
  home and drifted; the test now returns it home before each gun). Shot effects have a per-frame cosmetic budget (96 actors;
  damage unaffected). `test_quest` 111/112: the failing check
  (`natural_level_up_scales_weapon_damage_for_duration`) needs the recipes' `damage_stack`, which this PC's local slice
  recipes lack (local data).
- **Open, for a live session:** world FOV 77.55 vs 90 (HUD-canvas projection), the equal-level status chance, the zoomed
  foreground FOV (AR/SMG ADS width), reload mid-poses, the particle values in the running game.

