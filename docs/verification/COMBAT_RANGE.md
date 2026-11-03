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
| Critical multiplier | the hit event's `CritBonus` (2.0 base) | `CurrentInstantHitCriticalHitBonus` (NATIVE_COMBAT_GROUNDWORK 3.4; base confirmed live, COMBAT_GOLDEN_TRACE) | the shooter fills the bonus; 2.0 for damage without an event |
| Critical region | a sphere (radius 18 uu, manifest `head_radius`) on the first bone whose name has "head"; the engine-shape enemy's head sphere | host stand-in | the stock body class's hit regions are not read |
| Shield | shield first, the rest to health | the order recorded for the player (COMBAT_GOLDEN_TRACE item 9) | bullymongs have no shield (L3: "no shield"); the order on an enemy shield is UNVERIFIED and only exercised by a test fixture |
| Ragdoll | 0.84 s after death | the delay `src/combat_run.cpp` uses for the player | UNVERIFIED for enemies |
| Respawn | 8 s (manifest `respawn_seconds`) | host choice | a den's respawn rules are not modelled |
| Health bar | a red bar (blue shield bar above when there is a shield) over the enemy, facing the camera | host stand-in for BL2's HUD target bar | UNVERIFIED look |
| Critical damage number | orange, 1.25x | host stand-in | UNVERIFIED look |

**Critical hits use the hit event** (`FOpenWillowHitEvent`, the projectiles lane's API, `OpenWillowHitEvent.h`): the enemy crits only when the event allows it
(`bCanCrit`, never a `StatusTick`), multiplies by the event's `CritBonus` (2.0 without an event: test hits and other sources), and writes `bCritical`
and `Applied` back into the event, so `OpenWillowHit::OnHit()` listeners see the same result as the damage number. The region test: the event's bone
(else the point hit's) containing "head", the critical sphere as the hit component, or, because the body capsule (from the pawn's collision cylinder)
encloses the head, a shot whose line from the impact through the body crosses the head sphere (host stand-in).

**Dead targets take no hits:** `CanTakeHits()` is false while dead (dying, ragdolled, waiting to respawn), so explosions skip corpses. A status effect
the shooter rolls after the lethal hit is cleared from a dead range target (`UOpenWillowStatusEffects::ClearAll`) so it does not tick into the next life;
gating the roll itself on `CanTakeHits()` is in the projectiles lane's `HitTarget` (asked for, not this lane's file).

**The formation is validated as a whole:** every slot (3 dummies 900 uu ahead, one arc slot per manifest enemy 1,600 uu ahead) needs walkable floor
(normal Z >= 0.85, within 150 uu of the spot's height) before anything spawns, for the scout's candidates and for `ow.Range` at a given spot; an
incomplete range is a failed build and nothing is left standing. The test's placement step checks the dummy and enemy counts against the manifest.

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

## Enemy AI (PR2): notice, approach, melee

A host state machine (`FOpenWillowEnemyBrain`, `OpenWillowEnemyAI.cpp`), not the stock AI tree. The rules and numbers are the live records'; each is
`UNVERIFIED` as a host reproduction (nothing was compared with the running game in this lane).

| Rule | Host | Record |
|---|---|---|
| One threat record per den, shared by its members | the range is one den, centred on the spot | AI_AWARENESS_LIVE 0 |
| State update cadence | every 0.2 s per enemy; den fall-back check every 1.0 s | AI_MELEE_TARGETING_LIVE 4; AI_AWARENESS_LIVE 2 |
| Notice by sight | the previous update's line of sight (a Visibility trace, stand-in for the exposure iterator), Maya in the front half-plane, Maya within the den's ThreatRadius (4,665.625, the one den read) of the den centre | AI_AWARENESS_LIVE 0 |
| Notice by proximity | 3D distance minus the enemy's collision radius below 768 (AICloseThreatRadius), Maya within the den's PatrolRadius (3,667.19) | AI_AWARENESS_LIVE 0 |
| Notice stamps | level 2, AggroTime = now, ForgetTime = now + 10 (MinEngagementTime), refreshed while a rule holds | AI_AWARENESS_LIVE 2 |
| Damage by Maya | level 3, ForgetTime = now + 25 (MinProvokedTime) | AI_AWARENESS_LIVE 3 |
| Fall-back | at the first den update after ForgetTime the level drops to 1 (record removal beyond the list radius not modelled) | AI_AWARENESS_LIVE 2 |
| Awareness | attack decisions only after AggroTime + 0.1 s | AI_AWARENESS_LIVE 2 |
| Approach | turn at 360 deg/s, run at `run_speed` (400 uu/s, stand-in) with the run clip (walk as fallback) | host stand-in |
| Melee volume | horizontal 640 from a point 100 uu behind the enemy, 64 degree half angle, Maya's centre -143 to +339 uu from the enemy's location | AI_MELEE_TARGETING_LIVE 5 (fitted) |
| Swing timing | damage 0.41 s after the swing start if Maya is still in the volume; next swing 1.25 s after the previous start | AI_MELEE_TARGETING_LIVE 4 |
| Melee damage | the VM's swing rows per kind and level (`arrival` x `ratio`, both from `ow-package --combat-run maya:L spawn:<kind>:L swing:<n>:melee`), x the level scale (1.0 at or below Maya, 1.1 / 1.2 one / two levels up; a straight line to 3.0 at sixteen beyond that, stand-in) | COMBAT_MELEE (the VM rows reproduce adult L5/6/7 73.6974 / 83.2781 / 94.1042, regular L6 24.9834, baby L6 12.4917, ratios 0.715 / 0.728 / 0.741) |
| Delivery | one function, `RangeDamagePlayer`: UE ApplyDamage on Maya until the maya lane's `OpenWillowApplyDamageToPlayer` (PR #72) merges | host |

Not modelled: momentum on Maya (75,000 for the adult swing), leaps and the slinger's throw (the slinger has melee numbers from its rock but no throw),
the stock AI tree's perch, random and leap nodes.

Test steps added to `-owrangetest` and `tools/check_range.py`: notice_proximity, notice_sight, no_notice_far, provoked, approach, melee_hit, melee_interval,
melee_miss, forget, passive_no_melee.
