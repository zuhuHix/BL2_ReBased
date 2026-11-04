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
| State update cadence | every 0.2 s per enemy (the line-of-sight trace is taken on these updates only, and not at all while passive, lifted or dead); den fall-back check every 1.0 s | AI_MELEE_TARGETING_LIVE 4; AI_AWARENESS_LIVE 2 |
| Notice by sight | the previous update's line of sight (a Visibility trace, stand-in for the exposure iterator), Maya in the front half-plane, Maya within the den's ThreatRadius (4,665.625, the one den read) of the den centre | AI_AWARENESS_LIVE 0 |
| Notice by proximity | 3D distance minus the enemy's collision radius below 768 (AICloseThreatRadius), Maya within the den's PatrolRadius (3,667.19) | AI_AWARENESS_LIVE 0 |
| Notice stamps | level 2, AggroTime = now, ForgetTime = now + 10 (MinEngagementTime), refreshed while a rule holds | AI_AWARENESS_LIVE 2 |
| Damage by Maya | level 3, ForgetTime = now + 25 (MinProvokedTime) | AI_AWARENESS_LIVE 3 |
| Fall-back | at the first den update after ForgetTime the level drops to 1 (record removal beyond the list radius not modelled) | AI_AWARENESS_LIVE 2 |
| Awareness | attack decisions only after AggroTime + 0.1 s | AI_AWARENESS_LIVE 2 |
| Own target, apart from the threat record | each enemy keeps its own target: valid while the den level is 2 or more and Maya is within the den's ThreatRadius of its centre; it goes for where it last saw her (line of sight) or heard her (proximity), and after losing the target it walks to that spot once, then patrols | AI_AWARENESS_LIVE 2 (targets drop at the member's next update once its rules fail); the validity rule is a host stand-in |
| Approach | turn at 360 deg/s, run at `run_speed` (400 uu/s, stand-in) with the run clip (walk as fallback) | host stand-in |
| Blocked chase | under 10 uu of progress in 1 s counts as blocked: side-steps (70 degrees, alternating sides); blocked for 3 s, the enemy drops the target for 5 s | host choice |
| Melee volume | horizontal 640 from a point 100 uu behind the enemy, 64 degree half angle, Maya's centre -143 to +339 uu from the enemy's location | AI_MELEE_TARGETING_LIVE 5 (fitted) |
| Swing timing | damage 0.41 s after the swing start if Maya is still in the volume; next swing 1.25 s after the previous start | AI_MELEE_TARGETING_LIVE 4 |
| Melee damage | the VM's swing rows per kind and level (`arrival` x `ratio`, both from `ow-package --combat-run maya:L spawn:<kind>:L swing:<n>:melee`), x the level scale (1.0 at or below Maya, 1.1 / 1.2 one / two levels up; a straight line to 3.0 at sixteen beyond that, stand-in) | COMBAT_MELEE (the VM rows reproduce adult L5/6/7 73.6974 / 83.2781 / 94.1042, regular L6 24.9834, baby L6 12.4917, ratios 0.715 / 0.728 / 0.741) |
| Delivery | one function, `RangeDamagePlayer`: UE ApplyDamage on Maya until the maya lane's `OpenWillowApplyDamageToPlayer` (PR #72) merges | host |

Not modelled: momentum on Maya (75,000 for the adult swing), leaps and the slinger's throw (the slinger has melee numbers from its rock but no throw),
the stock AI tree's perch, random and leap nodes.

Test steps added to `-owrangetest` and `tools/check_range.py`: notice_proximity, notice_sight, no_notice_far, provoked, approach, melee_hit, melee_interval,
melee_miss, forget, passive_no_melee.

## Range spot: candidates and the textured-floor rule (PR2 follow-up)

`tools/prepare_range.py` writes `spot_candidates` from the level import's terrain probes (`local/sanctuary/terrain-runtime.json`), Sanctuary_P
Terrain_2 first (the terrain the gate's floor lies in, SANCTUARY_ARTIFACT_PASS.md), then the other terrains by distance from the plaza. The host tries
them (each point, then rings of 600 and 1,200 uu) before its ring scout. Every formation slot needs a **textured** floor: the face material the trace
hits must not be null, an engine material or a collision / fallback / neutral placeholder, **and it must sample at least one texture that is not an
engine texture** (`GetUsedTextures`; batch 1's scouted canyon floor drew dark with no texture). `tools/test_range.ps1 -Survey` saves one frame per
candidate (`local/range/shots/survey_<i>.png`) with the material and the reason it was or was not taken; the default spot will be pinned from those
frames. `prepare_range.py` refuses an `--out` outside `local/` (the manifest holds game-derived coordinates).

## Loot on death (PR3)

`tools/prepare_range.py` kills each spawned pawn in the VM (`hit:<n>:999999:DmgType_Normal:Shotgun`) and keeps its death roll per kind and level: the
item stage and every pool of the pawn's item pool list with the chance the VM evaluated (COMBAT_LOOT.md). For the adult at level 7 they are COMBAT_LOOT's
table: guns and gear 0.085, vehicle skins 0.05, money 0.25, eridium bar 0.0015, stick 0.008, health 0.03, the two ammo pools 0.2 each. The host
(`OpenWillowRangeLoot.cpp`) rolls them itself in COMBAT_LOOT's order: one draw per pool, tried when the chance is above 0 and the draw is at most the
chance, from a seeded stream (`-owrangeseed=N`, default 20261009, so a run repeats; the game's own seed is unknown).

| Pool kind | What the host does | Status |
|---|---|---|
| weapon (`Pool_GunsAndGear`) | `FOpenWillowWeaponGen::RollPoolWeapon` at the item stage (the roll `ow.RollGun` uses), dropped through the inventory pickup's existing API | the pool's own gun choice is the weapon generator's; a pick that is not a gun (shield, grenade mod) drops nothing here |
| money (`Pool_Money_1or2`) | quantity a random integer in -1 to 2, at least 1 (the data's range); one range pickup per item, worth `round(4 x 1.13^level)` | **stand-in value, UNVERIFIED**: the value of `ItemGrade_Currency_Money_Little` is not decoded (a live read is wanted) |
| ammo (the two ammo pools) | one pickup of the held gun's ammo type, a magazine's worth, only when that reserve is below half | stand-in: COMBAT_LOOT shows these pools drop nothing while ammo is not low; the threshold and amount are host choices |
| other (skins, eridium, health) | logged as not hosted | |

`ow.RangeLoot normal|always|off`: `always` tries every weapon, money and ammo pool (for testing guns), `off` drops nothing. The range pickups (money,
ammo) are engine-shape stand-ins collected on touch. Test steps: loot_roll, loot_money_pickup, loot_weapon_item (may SKIP without the weapon
generator manifest), loot_off, loot_seeded_roll.
