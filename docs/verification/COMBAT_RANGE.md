# The gun range outside Sanctuary: static dummies and moving enemies in the UE host (range lane, PR1, 2026-11-03)

AI-assisted (Claude; host C++ drafted by a Claude Haiku sub-agent from a written spec and reviewed by the lead). A host test area for trying guns,
not a stock map feature: BL2 has no gun range. Everything in this record is **host behaviour**; no rule here was checked in the running game,
so each is `UNVERIFIED` unless a line says which record it reproduces.

## What it is

- `ow.Range [x y z [yaw]]` (console) builds the range and teleports Maya to it. Without arguments the spot comes from `-owrangeat=X,Y,Z[,Yaw]`,
  else `local/range/range.json` (`{"spot": {"x", "y", "z", "yaw"}}`), else the **scout**: rings of 6,000 to 14,000 uu around the plaza
  (the centre of the inspection views), 24 candidates per ring, closest ring first. A candidate is accepted on a flat floor
  (normal Z >= 0.9) within 1,500 uu of the plaza's height, with 8 probes on a 700 uu circle at the same height (+-100 uu), nothing solid in a
  250 uu sphere 300 uu up, and open sky above. That the scout's winner is "outside the town gate" is **not shown** until a frame of it is
  looked at (the integration run captures one); a spot found to be good is fixed in `local/range/range.json`.
- Three **static dummies** in a row 900 uu ahead: the Fire mission's stock target dummy (`AOpenWillowCombatTarget::UseStockPawn` with the slice
  manifests, as the quest spawns it), else the engine-shape dummy. On the range they respawn (the mission's dummy still does not).
- **Moving enemies** (`AOpenWillowEnemy`, a subclass of the combat target, so every existing hit path reaches them): the manifest's spawns on an
  arc 1,600 uu ahead. PR1 gives them a patrol (random floor spots within 500 uu of home, 150 uu/s, pauses of 1 to 3 s); noticing Maya,
  approaching and melee are PR2.
- `ow.RangeMode normal|invulnerable|passive|both`: invulnerable targets record every hit (damage number, flash, log) but keep their health
  and shield; passive enemies stand still. `ow.RangeLevel N` (0 = Maya's level) rebuilds with that enemy level. `ow.RangeClear`, `ow.RangeReport`.

## Numbers and where they come from

| Item | Value | Source | Status |
|---|---|---|---|
| Enemy max health per level | `max_health.<class>.L<level>` of `ow-package --combat-run spawn:<kind>:<level>` (regular, adult, baby, ranged; levels 1 to 50) | the VM runs the pawn's installed spawn script and health pool (COMBAT_HIT_CHAIN.md, COMBAT_DEATH.md) | UNVERIFIED: no live row of a fresh pawn's maximum. The L3 adult at level 5 started the recorded fight at 360.876, the VM's adult L5 maximum is 614.084; the live pawn may not have been at full health (open item) |
| Critical multiplier | 2.0 | `CurrentInstantHitCriticalHitBonus` class default (NATIVE_COMBAT_GROUNDWORK 3.4; the projectiles lane reports it confirmed live) | skill modifiers not applied |
| Critical region | a sphere (radius 18 uu, manifest `head_radius`) on the first bone whose name has "head"; the engine-shape enemy's head sphere | host stand-in | the stock body class's hit regions are not read |
| Shield | shield first, the rest to health | the order recorded for the player (COMBAT_GOLDEN_TRACE item 9) | bullymongs have no shield (L3: "no shield"); the order on an enemy shield is UNVERIFIED and only exercised by a test fixture |
| Ragdoll | 0.84 s after death | the delay `src/combat_run.cpp` uses for the player | UNVERIFIED for enemies |
| Respawn | 8 s (manifest `respawn_seconds`) | host choice | a den's respawn rules are not modelled |
| Health bar | a red bar (blue shield bar above when there is a shield) over the enemy, facing the camera | host stand-in for BL2's HUD target bar | UNVERIFIED look |
| Critical damage number | orange, 1.25x | host stand-in | UNVERIFIED look |

The body capsule (from the pawn's collision cylinder) encloses the head, so a shot at the head meets the capsule first. The enemy then follows the
shot line from the impact through the body and counts the hit as critical when the line crosses the head sphere (host stand-in). When the hit-event
API of the projectiles lane lands, the bone and `bCanCrit` / `CritBonus` of that event replace this, in `AOpenWillowEnemy::IsCritRegion`.

## Data (all ignored, under `local/`)

- `python tools/prepare_range.py [--levels 1-50]` writes `local/range/range_manifest.json` (schema `ow-range-v1`): the health tables from one VM run
  (about 2 minutes, most of it the first load of Ice_Dynamic) and the enemy assets from `local/phaselock/target/host_target.json`
  (`tools/import_phaselock_target.ps1`, the bullymong). Walk, run, hit and melee clips are picked by name from the imported clips next to the idle
  (a name match; the manifest says which file was taken). One imported bullymong serves all four bullymong kinds (the adult, baby and slinger
  meshes are not imported). Without the manifest the host uses one engine-shape stand-in kind with 400 health and says so.
- Without the imported bullymong, the enemies are engine shapes (logged `fallback=shapes`). The bandit and the psycho are not in the VM's
  spawnable kinds, so they are not on the range yet.

## Test

`tools/test_range.ps1 [-At X,Y,Z[,Yaw]] [-Shots]` (heavy: one editor, the machine-wide UE lock) starts Sanctuary_P with `-owrangetest`; the host's
driver logs one `OWRANGETEST step=... result=PASS|FAIL|SKIP` line per step and `tools/check_range.py` grades them: teleport, placement, body_hit,
crit_hit, crit_trace, shield_order, invulnerable, passive, patrol, death, ragdoll (may SKIP without a stock mesh or physics asset), respawn,
popup_crit, dummy_respawn. The hits are applied with `ApplyPointDamage` as a shot would apply them; crit_trace is a real camera trace at the head.
`tests/range_tools_test.py` covers the two Python tools with synthetic data.

Results: none yet (the hub's integration run). Automated: ctest 21/21; verify_packages 9 packages, all class bodies exact.
