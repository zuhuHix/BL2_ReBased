# Weapon projectiles: what a shot does, chosen by the parts (2026-10-23)

AI-assisted (Claude, with Claude and Codex sub-agents). Lane "Weapons 3" of the weapons work: firing modes, pellets and
spread, bursts and spin-up, light bullets, rockets, splash, Tediore throws and elemental status effects, in the data tool
and the UE5 host. Native rules: [NATIVE_WEAPON_FIRING.md](NATIVE_WEAPON_FIRING.md) (own-words notes from a local Ghidra
reading, all `UNVERIFIED` by themselves). This record says which of them the running game confirmed, how, and what the
host does. Every live dump, log and frame is game-derived and stays under ignored `local/realgame/firing/` (rounds 1-3)
and `local/firing/` (host runs); nothing here is copied from them except the numbers needed to state a rule.

## 1. Pieces

| Piece | What it does |
|---|---|
| `tools/weapon_firing.py` | Adds a `firing` block to any recipe (weapon_recipe.py + weapon_stats.py output): firing mode (barrel's `CustomFiringModeDefinition`, else the type's default), `FireType`, bullet speed, pattern lines and flags, pellets, spread, burst, spin-up/down, trace range, the `ProjectileDefinition` with its behaviours (bounce, detonate, `Behavior_Explode` splash with the damage type's falloff bands), the damage type's status effect, the Tediore type's reload throw. Generic over parts: no per-gun table. `WillowGame.upk` fills class defaults. |
| `src/weapon_firing.{hpp,cpp}` | The rules as plain maths (pattern, spread, spin-up, burst interval, per-pellet split, falloff, status chance, flight step, the DOT clock). CTest `weapon-firing-math`. |
| `OpenWillowFiring.{h,cpp}` | Parses the block into the item (`FOpenWillowWeaponItem::Firing`), pellet directions, fire interval. |
| `OpenWillowProjectile.{h,cpp}` | Light bullets and rockets as host actors: flight, per-tick collision, direct damage, splash, status roll. |
| `OpenWillowStatusEffect.{h,cpp}` | Status effects on targets: duration, the measured DOT clock, stacking per element. |
| `OpenWillowWalker` fire path | `FireWeapon` asks for one direction per pellet and traces, launches bullets or rockets; refire follows spin-up and bursts; `StartReloadThrow` throws a Tediore gun on reload; `ApplyShotDamage` is the one damage entry. |
| `tools/test_weapon_firing.ps1`, `tools/check_weapon_firing.py` | In-engine firing test (`-owfiringtest`) and its log checker. |
| `tools/real_game/scripts/weapon_firing_probe.py` | The live probe (definition dump, hooks, matched frames). |
| `tools/capture_firing_frames.ps1`, `tools/firing_contact_sheet.py` | Host frames at the real frames' pose, and real/host contact sheets for the critic. |

## 2. Coverage

Coverage recipes (ignored `local/items/firing/coverage/`, rolled at stage 8, seed 1 from `A_Weapons*` balances): every
manufacturer (Bandit, Dahl, Hyperion, Jakobs, Maliwan, Tediore, Torgue, Vladof), every type (pistol, assault rifle, SMG,
shotgun, sniper, launcher), every element (fire, shock, corrosive, slag, explosive). Plus the six slice recipes and
Infinity rolls 1-6. 28 guns in all; firing kinds: 4 rocket firing modes, 2 own-motion light bullets (gyrojets), 22
plain light bullets; no hit-scan firing mode among them.

**Regenerating them (2026-11-02).** The 2026-10-23 set was not recorded by id, so `tools/weapon_firing_coverage.py` now fixes
the list (16 coverage ids, each with its balance) and rebuilds everything the firing test reads, in a fresh worktree:

```
cmake --build build --config Release                     # ow-package.exe
python tools/weapon_slice_gear.py --game "<BL2 folder>"   # local/items/slice (or copy it from another worktree's local/)
python tools/weapon_firing_coverage.py --game "<BL2 folder>"
```

The last command rolls each coverage gun with `tools/weapon_recipe.py --stage 8 --seed 1` from Startup.upk, evaluates its
card with `tools/weapon_stats.py --level 8`, writes `local/items/firing/coverage/<id>.json`, then adds the `firing` block
(`tools/weapon_firing.py`) to every coverage, slice and Infinity recipe (`--only-firing` skips the rolls). About a minute
per gun on the second machine. The reconstructed list keeps the ids the test and the frame captures name and adds
`ar_vladof`, `smg_hyperion`, `sr_vladof`, `rl_maliwan` and `rl_tediore` so that every manufacturer and type is present;
its rolls are not guaranteed to equal the 2026-10-23 ones (that run's generator setting was not recorded).

## 3. Rules and their evidence

"Live" = measured in the running game with bl-sdk hooks (three sessions, 2026-10-22/23, Sanctuary_P, Glacial_P and
Ice_P, Maya with her skills switched off in memory, saving blocked, saves byte-identical afterwards).

| Rule | Source | Status |
|---|---|---|
| `FireType` picks the path; most guns are `EWWFT_Bullet` (light bullets, no actor), rockets spawn actors | native 1.1 | **confirmed live**: slice guns created no projectile actor; their bullets are entries in Maya's `WillowLightProjectileManager` |
| Light bullets fly straight at the firing mode's `Speed` (x `WeaponProjectileSpeedMultiplier`), no gravity, damage when they arrive | native 2.2 (flight not read) | **confirmed live**: pistol 21,500 and Jakobs 30,000 uu/s constant; damage lands in the frame the bullet reaches the target |
| `AirFriction` F changes the speed each frame by x(1 - F dt), uncapped | not read | **confirmed live**: gyrojet (F -10) and Torgue shotgun (F -6) grew x(1 + 10 dt) / x(1 + 6 dt) per frame to 3-4 decimals over 30+ steps, up to 120,000 uu/s |
| Rocket `MaxSpeed` = `SpeedFormula` x the weapon's projectile speed multiplier | native 2.1 | **confirmed live**: 3,000 x 2.6 = 7,800 (Bandit, Maliwan), 3,300 x 2.6 = 8,580 (Tediore) |
| Weapon rockets do not fall (GravityScaling 0.1 or 1.0 has no effect under their physics); thrown guns fall at GravityScaling x 1,000 | observed | live: rockets' speed constant over 1-5 s; Tediore pistol throw vz slope -850 = 0.85 x 1,000 |
| Pellets: one aim per shot, `ProjectilesPerShot` pellets, each pattern offset then spread | native 1.2-1.4 | read; pellet count confirmed live (Jakobs 7 per shot); the spread distribution was not measured (no impact points: no hook fires at a bullet's wall hit) |
| Spread: yaw uniform across a disc of diameter `Spread` degrees, pitch uniform on the chord | native 1.4 | read only |
| Pattern: one random point on the current `FiringPatternLines` line, scaled by spread when flagged, index per pellet | native 1.3 | read; the cooked lines equal the live dump (shotgun Yaw +-455) |
| Per-pellet damage = card damage (`InstantHitDamage`); flat additions split over pellets (the split, `weapon_firing::multiProjectileDamage`, is **not used by the host**: it has no runtime modifier stack and fires the evaluated card damage) | native 1.7 | **confirmed live** on 4 of 5 guns (pistol 26.261, Jakobs 20.888, SMG 19.353, sniper 60.191 = card); a skill's flat +46,940 split over 7 Jakobs pellets confirmed the split. Dahl AR: live 21.463 vs card 19.603 (generation lane's card, section 6) |
| Splash: full damage inside the radius when the explode behaviour sets `bDisableRadiusDamageFalloff` | data + native 3.1 | **confirmed live**: Bandit rocket 285.84 at all 21 distances 86-380 uu, gyrojet 28.98 at 86-211 uu |
| Falloff otherwise: the damage type's `MaxDamageRadius` / `MinDamageRadius` / `MinDamagePercent` bands | native 3.1 | **confirmed live** with the explosive type's 0.3 / 0.9 / 0.6: `CalcRadiusDamageScale(107.284, 120)` returned 0.603975, the formula gives 0.603976 |
| Status chance = surface `BaseChance` x weapon modifiers x further factors | native 4.2 | live: Maliwan shock 21/96, corrosive 11/50, slag 18/50 procs against a card (flesh) chance of 11.7 / 11.7 / 23.4 %; fire AR 4/36 against 9 %. The missing factor is taken to be the level-difference scale (row below; player 54 vs target 7 gives x2.0), UNVERIFIED |
| DOT: build up frame time; at 0.33 s deal DPS x built-up time and subtract 0.33; a final partial tick at the end | not read | **confirmed live** on 94 ticks (ratio 1.00000); DPS = the weapon's `StatusEffectDamage`, duration = the definition's |
| Re-procs: shock refreshes its one entry (duration restarts, clock kept); fire and corrosive run parallel entries | not read | observed (a few cases; rule UNVERIFIED) |
| Spin-up: only `BSM_SpinUpToFullFireRate` ramps the interval; `BSM_SpinBarrelWithEachShot` does not gate firing | native 1.5 | read only |
| `AutomaticBurstCount` N: a press fires N shots even if released, then waits for a new press; 1 = one shot per press (`weapon_firing::Burst`, reset when the weapon in hand changes) | native 1.5 + data | **consistent with live**: the slice pistol and rifle (count 1) fired once per hold; the SMG, shotgun (count 0) and Infinity fired repeatedly |
| Tediore reload throw: damage = card damage x rounds left; half to what it strikes, half as a radius-120 splash with falloff | data + observed | **confirmed live** (25.891 x 10 = 258.91; direct 129.455; splash 78.19 at 0.604) |
| The thrown rounds leave with the gun: the reload then takes a full magazine from the reserve | Tediore design | host rule; **not measured** (the live session logged the damage, not the ammo counts) |
| Per-hit momentum = `InstantHitMomentum` x 100 along the shot; from the weapon type, the class default when it has none; splash momentum = the explode behaviour's `MomentumFormula` x the radius scale (2026-11-02, maintainer decision to restore it) | data + NATIVE_WEAPON_TRIGGER_AMMO 3 (x100 once at weapon setup) | the values **match live reads** (R6/R7: pistol 8, shotgun 7, Torgue 10, rocket splash 10,000); the x100 factor is read from native code and the hit's effect on a pawn is **not measured** (UNVERIFIED) |
| A hit changes the target's velocity by momentum / `Mass` (`Default__Pawn.Mass` 100, Engine.upk); the target then slides, slowed by `GroundFriction` 8 (`Default__PhysicsVolume`) | stock UE3 pawn rule + data | UNVERIFIED: the host applies the friction as v x (1 - 8 dt) per frame, a stand-in for the game's walking physics |
| Wide bullets: a firing mode's `TraceExtent` makes the shot a box sweep instead of a line (light bullets each tick and traces); rockets stay lines | native 2.2 + data | read only; none of the 28 local recipes has an extent, so the in-engine check uses a host-chosen 24 uu box |
| A trace or light-bullet firing mode's own `Behavior_Explode` (in `OnAnyImpact`) runs at every impact that is not a pass-through actor, after the bullet's `TakeDamage`; radius, damage formula, momentum and falloff are the behaviour's; `ExplosionOverrideDefinition` (an `ExplosionCollectionDefinition`, first entry whose `WeaponDamage_Is_<element>` condition holds, chosen once at weapon setup) only picks the explosion definition and so the damage type | native read 2026-11-03 (section 8) + data | the Maliwan shock pistol's value **matches live**: 45.30 x 0.8 = 36.24, falloff off, on the same bullymong; every other part UNVERIFIED in game |
| Status chance x the level-difference scale (`GD_Globals.General.Globals.StatusEffectChanceScales`): row = min(difference, 6); shooter higher -> `HigherLevelChanceScale` (x2.0 at 6+), lower -> `LowerLevelChanceScale` (x0 at 6+) | data | **consistent with live counts** (Maya 54 vs bullymong 7: about 2 x the card chance on Maliwan pistols, section 5); a live check against a same-level target is still to do (UNVERIFIED) |

## 4. The host (what it does and where it stands in for the game)

* Damage, bullet speed, pellets, pattern, spread, burst, spin-up/down, rocket speed, splash radius/damage/falloff, the
  status effect and the reload throw come from the recipe's `firing` block; nothing is per gun.
* Light bullets are host actors (the game keeps them in a manager): straight at speed, the AirFriction step, a line trace
  per tick from the camera; drawn as a short additive streak from the muzzle (only for weapon types whose view-model
  data enables a tracer, the lane E rule).
* Rockets launch from the muzzle toward the screen-centre hit (native 2.1) and explode when their definition has a
  Detonate behaviour; bullets whose firing mode carries a `Behavior_Explode` (gyrojets) explode where they hit.
* Momentum: each direct hit, trace or splash passes `momentum` along the shot to the target, which changes its velocity by
  momentum / mass and slides it on the ground plane (none while phaselocked; a Tediore throw's direct half passes none, not
  measured). Wide bullets sweep their `TraceExtent` box (`OpenWillowFiring::SweepShot`).
* Status rolls multiply the card chance by the level-difference scale: Maya's level against the target's
  (`-owtargetlevel=N` sets the host dummy's; without it the dummy counts as Maya's level, factor 1).
* The splash distance is to the target's collision centre (the game measures to a hit-region bone; the live cutoff of
  the radius-250 rocket lay between 380 and 432 uu from the pawn origin). The target's surface is taken as Generic.
* Torgue impact explosions are modelled: they are the firing mode's own `Behavior_Explode` splash (Torgue shotgun 17.755 per
  hit = live 0.85 x 20.888; gyrojet 28.97, live 28.977), section 5.
* Elemental impact explosions are modelled (2026-11-03, section 8): a trace or light-bullet firing mode whose own behaviours
  hold a `Behavior_Explode` (the Maliwan pistol firing mode; not the default pistol, SMG or shotgun modes) explodes at every
  impact after the bullet's own damage. `tools/weapon_firing.py` writes it as `impact_explosion`; the host plays it as the
  gun's splash. Still open (not modelled): element-vs-surface factors (live on flesh: fire x1.5, corrosive x0.9, shock and slag
  x1.0); child projectiles spawned by a projectile's behaviours (the Maliwan launcher's fire area; the firing block lists
  them under `unsupported` and the host logs them); beams (the tool gives them their own kind, `beam`, listed as
  unsupported; the host fires light bullets for them); runtime damage modifiers (skills, relics; the per-pellet split is
  written but unused); slag's damage bonus and its duration (the recipe's slag duration is unresolved; live 8 s); the accuracy pool and kick (the recoil data is in the block, the host keeps lane E's kick); bullet ricochets; homing; the launcher reload throw's own motion (live speed steps not fitted).

## 5. Host versus game

**2026-11-03 run (pc-hummer, branch `feature/weapons-integrity` on main 95e48fb):** `local/firing/run-20261103-014500.log`,
**14 PASS / 0 FAIL**. The recipes were regenerated on this machine (section 2), so cards and kinds differ from the 2026-10-23
table below (for example this machine's `ar_bandit_fire` roll fires bullets, not rockets); the coverage guns have no imported
meshes here and are drawn as the stand-in (firing uses the card and the block). New checks in this run:

* Momentum (`OWFIRING push`): every momentum-carrying hit on all 14 guns changed the dummy's velocity by exactly momentum /
  100 (`pushes` = `momentum_hits`, 4-15 per gun); momentum 500-1,100 per bullet (x100 of the block), 10,000 per rocket splash;
  the dummy slid 1.9-12.7 uu over the five dummy pulls, 84.9 uu for the Bandit launcher.
* Wide bullet (`OWFIRING wide`, the 7 single-straight-bullet guns): the line 45.6 uu beside the dummy's centre missed (0 hits),
  the same pull with a 24 uu half-size box hit (1 hit) on all 7.
* Flight timing (`timed` / `on_time`): 288 of 288 constant-speed bullets arrived within the frame that reaches
  distance / speed (the own-motion and rocket guns are not timed).
* Status rolls ran at level scale 1 (no `-owtargetlevel`: the dummy counts as Maya's level): procs shock 4/26, corrosive
  1/26, slag 8/26, fire AR 3/25. A run with `-owtargetlevel` below Maya's to see the x2 rows was not made.

**Review fixes (PR #62 round 1).** The checker now also checks every splash hit's momentum against the recipe (the explode
behaviour's `MomentumFormula`, or the reload throw's, x the hit's radius scale; the host's `OWFIRING splash` line carries both)
and fails a gun whose shots explode with splash momentum but log no splash line. The push, splash and wide lines are required:
a log without them fails unless `--legacy` is passed (runs from before the momentum checks). The wide check also skips
bullets with an acceleration. A target's knock is cleared when it dies, and the knock and last-hit momentum when it respawns.

The first run of the day (`run-20261103-013858.log`) failed 8 of 14 on shot counts only: the wide check's two pulls were
not counted by the checker, and on the burst fixture the burst's later shots landed during the check. The checker now
counts the two pulls and the wide check skips burst guns; nothing else changed between the runs.

In-engine firing test (`tools/test_weapon_firing.ps1`; Sanctuary_P, the host dummy) after the round-3 fixes:

Run `local/firing/run-20261023-035446.log`: **14 PASS / 0 FAIL** (kind; shots = pulls x burst count; launches = shots x pellets; damage per
direct hit = card; splash <= the block's splash damage, and required when the shots explode; ticks only after procs and
within the duration; the Tediore throw and its full refill; a burst that is interrupted fires nothing more).

| Gun | Kind | Shots | Pellets | Target hits | Direct per hit | Splash per hit | Procs / rolls | Ticks |
|---|---|---|---|---|---|---|---|---|
| slice_pistol | Bullet | 8 | 8 | 5 | 63.63 | - | - | - |
| slice_shotgun | Bullet | 8 | 80 | 12 | 20.65 | - | - | - |
| slice_smg | Bullet | 8 | 8 | 5 | 22.26 | - | - | - |
| infinity_1 | Bullet (pattern) | 8 | 8 | 4 | 739.6 | - | - | - |
| rl_bandit | Rocket | 8 | 24 | 13 | - | 285.84 | - | - |
| ar_torgue_gyrojet | Bullet (own motion) | 8 | 8 | 5 | 32.19 | 28.97 | - | - |
| sg_torgue | Bullet (own motion) | 8 | 40 | 9 | 20.888 | 17.755 | - | - |
| pistol_maliwan_shock | Bullet | 28 | 28 | 25 | 45.30 | - | 3 / 25 | 21 |
| pistol_maliwan_corrosive | Bullet | 28 | 28 | 25 | 45.30 | - | 4 / 25 | 100 |
| pistol_maliwan_slag | Bullet | 28 | 28 | 25 | 45.30 | - | 8 / 25 (applies nothing: duration unresolved) | 0 |
| ar_bandit_fire | Rocket | 28 | 28 | 22 | - | 46.13 | 0 / 22 | 0 |
| sg_jakobs | Bullet | 8 | 56 | 6 | 20.888 | - | - | - |
| pistol_tediore | Bullet + reload throw | 8 | 8 | 5 | 25.89 | throw 64.73 direct + 64.73 splash (5 rounds); refill 12 of 12 | - | - |
| ar_dahl_burst3 (local fixture: burst_count 3) | Bullet | 26 (8 pulls x 3 + 2) | 26 | 17 | 19.60 | - | - | - |

The burst fixture also runs two lifecycle cases: a burst's first shot, then a reload, and the same with a respawn; no shot
followed either (`extra_shots=0`). CTest `weapon-firing-math` covers the same rule (`weapon_firing::Burst`: weapon change
resets it, reload / death / menus interrupt it and a new press is needed).

Direct and splash per hit against the live game, where both were measured: gyrojet 32.19 / 28.97 (live 32.194 /
28.977), Torgue shotgun 20.888 / 17.755 (live 20.888 / 17.755), Bandit rocket splash 285.84 (live 285.84), fire AR 46.13
(live 46.133 before the flesh factor), Jakobs 20.888 (live 20.888), Maliwan pistol bullet 45.30 (live 45.30; the live
gun adds an impact explosion of 36.24 the host does not have). Corrosive: 4 procs x 25 ticks of the 8 s clock = 100. The
host hits are on the host dummy, the live ones on a bullymong; hit counts differ with target size and are not compared.

Same quantities, host versus live:

| Quantity | Game (live) | Host |
|---|---|---|
| Damage per pellet, trace guns | = card (4 of 5 guns) | = card (by construction) |
| Rocket speed | 7,800 / 8,580 uu/s | from the block: 7,800 / 8,580 |
| Gyrojet speed growth | x(1 + 10 dt) per frame | same step (`weapon_firing::step`) |
| Bandit rocket splash | 285.84 flat in the radius | card damage flat in the radius |
| Tediore throw | 258.91 at 10 rounds; 129.455 direct + 0.604 x 129.455 splash | same rule |
| DOT tick | 0.33 s clock, DPS x built-up time, final partial | same clock |
| Status proc rate | 2-3 x the card chance (Maliwan; Maya 54 vs a level-7 target) | the card chance x the level-difference row (x2.0 at 6+ levels above; x1 at equal levels), UNVERIFIED |

## 6. Matched-frame review

Host frames at the real frames' pose (`tools/capture_firing_frames.ps1`, `-owfiringshots`: pitch 0, FOV 77.55, a flat
host wall at 465.9 uu, the distance a rocket measured in the real session) against the real-game frames, eight guns
(ar_bandit_fire, ar_torgue_gyrojet, pistol_bandit, pistol_maliwan_shock, rl_bandit, sg_jakobs, slice_shotgun, slice_smg),
real/host contact sheets from `tools/firing_contact_sheet.py`. Independent critic: Codex gpt-6-luna, effects only
(environment, gun meshes, HUD and muzzle-flash size are other lanes' and were not scored).

| Round | Overall | Change |
|---|---|---|
| 1 | 2.1 / 10 | first host look: bullet streaks drawn next to the camera filled the view; explosions were one white orb |
| 2 | 3.9 / 10 | streaks hidden for the first 150 uu and thin; element colours for flash, streak and impact; sparks; shock arcs; fireball cluster |
| 3 | 4.2 / 10 | fire splash as a compact patch with embers; a fading scorch decal for smoke; longer shock glow with kinked arcs; warm sparks |

Per gun in round 3: pistol_bandit 6.0, pistol_maliwan_shock 4.8, sg_jakobs 4.2, ar_bandit_fire 4.1, slice_shotgun 4.0,
slice_smg 3.8, ar_torgue_gyrojet 3.4, rl_bandit 3.0. **Stalled well short of 9:** the critic's remaining differences
(layered explosions with debris and lingering smoke, broken fading trails, starburst debris, a soft saturated shock
halo, textured impact marks) need BL2's own particle systems (`FX_WEP_*`, the impact and explosion templates) and
translucent materials; the host's effects are additive engine shapes. Reading those templates
(`research/particle_system.py` already reads cooked `ParticleSystem` layouts for Phaselock) is the next step for the look,
not more tuning of the stand-ins. The real frames also show a 2-3 frame capture latency (effects start after the fire
hook); the critic aligned on the first visible effect.

## 7. Found for other lanes

* **Dahl assault rifle card damage** (generation, `tools/weapon_stats.py`): the live gun with the same parts, prefix,
  title, stage and grade deals 21.463 per bullet, the card model 19.603 (+9.5 %); the recipe's "slot WeaponDamage grade
  -3" modifier is the suspect (UNVERIFIED).
* **Generated guns have no firing block yet** (follow-up): `ow.RollGun` and the quest reward build items from the C++
  generator's card (`OpenWillowWeaponGen::ToItem`), which names the firing mode but does not resolve the projectile, splash,
  status, burst and pattern data that `tools/weapon_firing.py` writes for recipes. Those guns fire the host's fallback
  trace (card pellets and spread). Porting the resolution into `src/weapon_gen` (or calling a shared resolver) is the
  next step; recipe guns are unaffected.
* Live skills change weapon stats a lot (a legendary amp shield added a flat 46,940 per shot); real-game measurements
  need skills off.

## 8. Impact explosions of trace and light-bullet guns (2026-11-03, weapons parity 2)

AI-assisted (Claude). Settles the open item of section 4. A live hook was the plan; the game could not be started on
this PC that day (a "Steam must be running" message box), so the rule comes from a clean-room native read (Ghidra,
headless and read-only; the analyst's own-words note stays under ignored `local/p2/wp2-native/`; NATIVE_WEAPON_TRIGGER_AMMO.md
has the outline) plus the cooked data, decoded with our reader. It is `UNVERIFIED` in game except for the one live
number it reproduces.

- **Data.** `GD_Weap_Shared_Effects.Default_Elemental_Explosions` decodes with the array schema line
  `Explosions=StructProperty:ConditionalExplosionData` to four {`Expression`, `Explosion`} entries: the conditions
  `WeaponDamage_Is_Corrosive`, `_Is_Incendiary`, `_Is_Shock`, `_Is_Amp` (each compared with 1) pick
  `GD_Explosions.<element>.Explosion_<Element>Master`. Each `ExplosionDefinition` carries only `DamageTypeDef`
  (`DmgType_<Element>_Impact`, slag `DmgType_Amp_Impact`), `ImpactTraceLength` 512 and an `ExplosionScale` array
  (presentation only). No damage, radius or momentum.
- **Rule (native read).** The bullet-impact native sends the bullet's own `TakeDamage`, then runs the firing mode's
  `OnAnyImpact` behaviours and the surface response's behaviours. A `Behavior_Explode` there explodes at the impact
  point with its own radius, damage formula, momentum and falloff switch, and hurts every damageable actor in range
  (the actor the bullet hit included), with the shooter's controller as instigator. The override collection is
  evaluated once at weapon setup (first matching entry, stored on the weapon as instance data) and only replaces the
  explosion definition, so the damage type. Pass-through actors (non-blocking triggers) get the bullet only.
- **Maliwan pistol** (`Bullet_Pistol_Maliwan`): radius 106, damage `WeaponDamage` x 0.8, momentum 1000, falloff off.
  Live check: 45.30 x 0.8 = 36.24, exactly the earlier live hit. The shock damage type's falloff bands are not used
  (falloff is off), so section 4's band guess was wrong.
- **Torgue** gyrojet bullets use the same mechanism with their own `Behavior_Explode` (no override); the existing
  model (section 3) is unchanged.
- **Which guns.** Of the regenerated recipes only the Maliwan firing mode carries one: `pistol_maliwan_shock`,
  `_corrosive`, `_slag` and `slice_mission_pistol_fire` (radius 106, 0.8 x, damage types shock / corrosive / amp /
  incendiary). The Bandit, Jakobs, Tediore and Dahl pistols, the slice SMG and the fire AR have none (their firing
  modes name the collection but hold no `Behavior_Explode`).
- **Tool and host.** `tools/weapon_firing.py` writes `impact_explosion` (the splash fields plus `explosion_from_override`)
  for trace and light-bullet modes; synthetic tests in `tests/weapon_firing_test.py` (33 OK). The host reads it into the
  gun's splash when the projectile has none (`OpenWillowFiring.cpp`); light bullets already explode on impact when they
  have a splash, and `TracePellet` now does the same after a trace's own damage. Shock, corrosive and slag explosions
  draw a small element-coloured burst (`ExplosionFx`), not the rocket fireball. `tools/check_weapon_firing.py` takes the
  impact explosion as the gun's splash (no wide-bullet check for exploding guns, as before for gyrojets); the firing test
  refills the dummy at each segment start, because the extra damage killed it mid-segment.
- **Firing suite** (this PC, `tools/test_weapon_firing.ps1`): 14 PASS / 0 FAIL after review round 1. The three Maliwan guns
  pass with 24-25 splash hits of 33.94 each per run. The first refill version (health only) made `pistol_tediore` fail: a
  dummy that never dies never respawns at home, so it drifted with the earlier guns' pushes and the thrown gun's arc missed
  it (the same suite on main's host code passed 14/14). The test now also returns the dummy home, at rest, before each gun.
- **Cosmetic budget.** Every shot effect (flashes, sparks, tracers, flash quads, casings) is spawned through one function
  with a cap of 96 per frame and a null check; past the cap the cosmetic actor is skipped. Impact decals (bullet holes,
  scorch marks) have their own cap of 16 per frame. Damage, explosions and their hits are not budgeted (host values).
- **Still open, to check live:** a wall shot next to a bullymong (the wall impact explodes too), a Bandit elemental
  pistol (no second hit expected), a hit at the radius edge (no falloff expected), and the status chance of the
  explosion hit (its status damage and chance attributes are named in the behaviour; how they reach the hit was not read).

## 9. The hit-event API (2026-11-03, slice projectiles lane)

AI-assisted (Claude). One shape for every hit of one of Maya's shots, so that other actors (the range lane's enemies, HUD
numbers, AI reactions) read the same data the host dummy does. Host code only; no new game rule. Files:
`OpenWillowHitEvent.{h,cpp}`.

- **`FOpenWillowHitEvent`**: who (`Instigator` pawn, `Target`, `WeaponId` of the fired item), what kind (`Direct`, `Splash`,
  `StatusTick`, `Throw`), where (`Location`, explosion `Origin`, unit `Direction`, `Bone`), how much (`Damage` before the target's
  own scaling, the stock `DamageType` path, `Element` from that path or the item, the `Momentum` vector, the splash
  `RadiusScale` already in damage and momentum), crit inputs (`bCanCrit`, `CritBonus`) and `Time`. Result fields the target
  writes inside `TakeDamage`: `bCritical`; the shooter stores `Applied` (TakeDamage's return).
- **Delivery.** `AOpenWillowWalker::ApplyShotDamage(Target, Event, Hit)` is the only damage entry. It calls the target's own
  `TakeDamage` with an `FOpenWillowDamageEvent` (an `FPointDamageEvent` subclass: point-damage readers keep working; new readers
  test `IsOfType(FOpenWillowDamageEvent::ClassID)`), then broadcasts the event on `OpenWillowHit::OnHit()`. The old
  `ShotDamageType()` / `ShotMomentum()` readers still work while the call runs.
- **Who gets splash and status.** Explosions used to look only for the host dummy class. They now visit every actor that
  implements `IOpenWillowHitTarget` (`GetHitCentre`: the point the splash distance is measured to and the radius the status
  falloff subtracts, default the simple collision cylinder; `GetHitLevel`: the level for the status level scale, 0 = the
  shooter's; `CanTakeHits`), never the shooter. Bullets, traces and thrown guns count a hit on any such actor; other actors
  are world surfaces (decals, no damage). The host dummy implements it with its existing collision centre, auto-aim radius
  and `-owtargetlevel` level, so its numbers do not change.
- **Crits.** In BL2 the victim decides: a hit region with `bCriticalHit` (from the bone), never for status damage, times the
  instigator's `CurrentInstantHitCriticalHitBonus` (base 2.0, confirmed live, COMBAT_GOLDEN_TRACE.md). The event therefore
  carries the bonus and `bCanCrit` (direct and throw hits; splash taken as non-critical, UNVERIFIED), and the target sets
  `bCritical`. Skills and the weapon's own crit bonus are not applied to `CritBonus` yet. The host dummy has no hit regions and
  never crits.
- **Test.** The firing test's result line gains `hit_events` and one count per kind; `tools/check_weapon_firing.py` requires a
  direct event per bullet or trace hit (none for a rocket body), a splash event per splash hit, a tick event per status tick and
  a throw event per thrown-gun hit (`tests/check_weapon_firing_test.py`). Each event is also logged
  (`OWFIRING hitevent <kind> <weapon> on <target> damage .. applied .. element .. crit .. momentum ..`).
- **Source weapon of a status tick (review fix).** A proc stores the weapon id and the element of the hit that caused it
  on the active status effect, and every tick's event carries those, so a weapon swap before the next tick does not credit it to
  the gun now in hand. Every hit path names its weapon explicitly (bullets and rockets from the launch, traces from the fired
  item, splash from the exploding shot); the walker falls back to the gun in hand only for a hit that names none. The firing
  test checks it: for a status gun whose effect is still running in the final wait, it equips another gun (`OWFIRING swap`) and
  the result line's `tick_weapons` must name only the proc gun, with at least one tick after the swap. The checker requires
  the hit-event fields; `--legacy` grades a log from before the hit-event API without them.
- **Listener safety and a certain swap (review round 2).** Hit listeners run inside `ApplyShotDamage`, so nothing read
  after that call may point into state a listener can change. Status ticks are collected while the effect list is walked
  and sent after the walk. The status roll reads a copy of the gun's status data. A trace's impact explosion uses inputs
  copied before the bullet's hit. Every status gun must now swap during its ticks: with no effect running it pulls once
  more at the dummy (up to 40 extra pulls, `extra_pulls` on the swap line, counted in the expected shots) until one runs.
  The checker fails a status gun with no swap line, a failed swap, no tick after the swap, or `tick_weapons` totals that
  differ from `ticks` / `hit_tick`.
- **Integration batches 2 and 3.** *Slag:* the swap applies only to a status whose effect deals damage over time
  (duration and dps above 0). The slag recipe's status has no resolved duration (section 4) and slag has no damage ticks, so
  the host never runs it (32 procs, 0 ticks in batch 2) and there is nothing to attribute. The test logs
  `OWFIRING swap ... skipped=no_ticks`, which the checker accepts only when the recipe agrees. *Stale recipes:* a gun held
  without a firing block logs `momentum_rule` 0, which batch 3 saw on three slice guns whose local recipes had never been
  given one. `tools/test_weapon_firing.ps1` now runs `tools/weapon_firing_coverage.py --only-firing` when a listed recipe
  has no firing block, and stops if one is still missing. The checker's momentum failure names that cause. `-Extra`
  passes extra game arguments (for example `-Extra '-owaimmodel'`).
