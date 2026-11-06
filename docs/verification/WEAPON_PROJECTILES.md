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
| Spread: yaw uniform across a disc of diameter `Spread` degrees, pitch uniform on the chord | native 1.4 | **confirmed live** 2026-11-03 (420 `AddSpread` draws, section 10) |
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
| Status chance x the level-difference scale (`GD_Globals.General.Globals.StatusEffectChanceScales`): row = min(difference, 6); shooter higher -> `HigherLevelChanceScale` (x2.0 at 6+), lower -> `LowerLevelChanceScale` (x0 at 6+) | data | UNVERIFIED. **Equal levels did not give x1** (2026-11-03: 18 of 43 bullet procs against a 23.4 % card chance, section 10). x1 at equal levels is consistent with a skills-off run (8 of 42, small sample, different class, section 13); the mechanism behind the skills-on ~x2 is UNVERIFIED; the higher-level scale is unmeasured |

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
* **Generated guns take their firing block from templates (2026-11-06).** `ow.RollGun`, the quest reward, vendors and loot build items
  from the C++ generator's card (`OpenWillowWeaponGen::ToItem`). `tools/weapon_gen_firing.py` runs the one resolver,
  `tools/weapon_firing.py`, on sampled rolls of every balance and writes a template per weapon type, firing mode and element to ignored
  `local/weapons/gen/`; the host fills it from the card and uses the recipes' parser. The tool merges every roll back and refuses to write if
  anything but the pool differs from the resolver (1494 rolls, 236 templates, 0 differences; a Python check of the rule, not the C++).
  **UNVERIFIED:** the accuracy pool is the type's base because parts are not applied; the spin-down duration is missing; a key the sampled rolls
  never reached fires the plain trace with a warning; nothing has been compared with a rolled gun in the running game. Their refire rule is resolved since 2026-11-06: the card carries the evaluated burst count and
  interval, so a type with count 1 fires once per press. Before that every generated gun fired while held. The firing test now
  rolls one gun of each kind from the manifest and `tools/check_weapon_firing.py` requires one shot from one hold (burst 1) and more than
  one (burst 0), with a hold of at least two fire intervals. A stale manifest fails and names `tools/weapon_gen_manifest.py`; the burst count alone marks a fresh one. This check is about refire only: `from_manifest` is a data check, and a rolled gun with no imported mesh passes it on the host's stand-in, which says nothing about how that gun looks. Porting the resolution into `src/weapon_gen` (or calling a shared resolver) is the
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
- **Damage source (skills lane request).** `FOpenWillowHitEvent::Source` (`EOpenWillowHitSource`: `Unknown` default,
  `Bullet`, `Rocket`, `Explosion`, `Throw`, `Status`, `Skill`) says what dealt the damage, separately from `Kind`. A
  rocket's explosion is `Source` Rocket with `Kind` Splash; a bullet's impact explosion (Maliwan, Torgue gyrojet) is
  `Explosion`, as the running game sends those hits as `WillowDmgSource_Grenade` (R6 and section 10), not as the gun's
  bullet source; a Tediore throw's strike and explosion are `Throw`; status ticks are `Status`. Skills set `Skill`
  themselves. Every firing path sets it: the firing test counts events left `Unknown` (`unknown_sources`) and the checker
  requires 0. Which skills react to which source (Cloud Kill, Immolate) is the skills lane's reading, not checked here.

## 10. Accuracy pool, view kick and the spread draw, live (2026-11-03, slice projectiles lane)

AI-assisted (Claude). One real-game session (`tools/real_game/projectiles_capture.ps1` with `scripts/projectiles_probe.py`;
Save0008 Maya, Sanctuary_P then Ice_P; saving blocked, saves and config restored and hash-compared afterwards). The probe
sampled every frame the controller's `AccuracyPool` value, its four kick fields and its rotation, and logged every
`AddWeaponKick`, `AddSpread` and `FireAmmunition` call with the game clock. Numbers only; the dumps stay under ignored
`local/realgame/pj/`; `tools/analyze_projectiles_live.py` prints the fits. To repeat the status part, run `projectiles_capture.ps1 -StatusOnly`. It needs `local/items/slice/status_maliwan_fire.json`: a copy of `slice_mission_pistol_fire.json` with `balance` set to `GD_Weap_Pistol.A_Weapons_Elemental.Pistol_Maliwan_2_Fire`, made by hand.

**Limits of this session.** The probe's equip step left the save's own Jakobs shotgun (`WT_Jakobs_Shotgun`) in hand for every
run, so all the numbers below come from that one gun. Maya's skills were active: a skill modifier on the weapon's
`PerShotAccuracyImpulse` turned the type's 11 into 5.4524. The rules below are ratios and orders of events, which these
facts do not change; per-gun values come from the data (`accuracy_pool` in the firing block) and are `UNVERIFIED` live.

| Rule | Status |
|---|---|
| The pool rises by the shot's impulse at the shot (5.4524, every one of 8 single shots and the 60 spread shots) | **confirmed live** |
| After a shot the pool holds, then falls linearly: hold 0.176-0.200 s (the data's 0.2 s delay to within a frame), fall -8.9 to -9.1 per second, down to its floor (1.2434 with this gun and Maya's skills) | **confirmed live** (rate and floor include the skill and weapon scales, not separated) |
| The kick passed to `AddWeaponKick` = impulse x 182.0444, x `WeaponKickZoomMultiplier` when fully zoomed (992.58 hip, 1985.2 = x 2.0 zoomed) | **confirmed live** |
| `TargetVertical/HorizontalKickAmt` fall linearly to 0 at `WeaponKickRecoveryTime` (0.35 s) after the kick | **confirmed live** |
| `CurrentVertical/HorizontalKickAmt` chase the targets at exactly `WeaponKickSpeed` per second (first-order: d current = (target - current) x speed x dt; fitted 8.00 on every frame pair, the type's 8) | **confirmed live** |
| The controller rotation never moves (pitch 0 throughout): the kick is a view offset that returns to the aim, not a permanent view change | **confirmed live** |
| The shot's base aim includes the current kick (aim minus view averages the kick amount on shots fired during a kick) | consistent with live (not fitted) |
| `AddSpread`: yaw uniform across the disc, pitch uniform on the chord (420 pellet draws: yaw SD 0.575 and pitch/chord SD 0.567 of the half width, uniform 0.577; none outside the disc) | **confirmed live** |
| Fully zoomed, the pool cone vanishes (`1 - ZoomEffect`) | partly: pitch offsets near 0, but yaw offsets up to 332 units with a pool half-width of 80 (3 shots); the zoomed view wander is the suspect, UNVERIFIED |
| The vertical / horizontal split of a kick and its signs | not fitted (this gun kicks up 3, down 0, left 0.5, right 0.5) |

**Host.** `src/weapon_firing` gains `AccuracyPool` (impulse, hold, idle fall, clamp) and `Kick` (the split, the linear target
decay, the first-order chase), checked by CTest `weapon-firing-math`. `tools/weapon_firing.py` writes the pool the held weapon
gives the player (`accuracy_pool`: the pool definition's base values scaled by the type's activated external slots and the
type's and parts' `ExternalAttributeEffects`) and the type's full kick fields (`recoil`). The walker draws one pool-cone offset
per shot (x 1 - zoom) before pattern and spread, adds the impulse and the kick after the shot, and adds the change of the
current kick to the view each frame (so the view returns when the kick has recovered). The firing suite runs without it unless
`-owaimmodel` is given (its dummy pulls keep their aim); each shot logs `OWFIRING aim`.

*Review round 1 (PR #84).* The chase stays the per-frame linear step `current += (target - current) x speed x dt`, not the
frame-independent `1 - exp(-speed dt)`. The live fit came out at exactly the type's 8.00 on every frame pair at about 8.5 ms,
which is what the linear step gives; the exponential form would read 7.73 there. A frame longer than 1/30 s is cut into 1/30 s
substeps, so a hitch cannot snap the kick onto its target; frames up to 33 ms are one step, as before. The kick keeps its sign
(a negative impulse kicks the other way; the game's sign handling for that case is UNVERIFIED). A non-finite impulse, kick or
pool value is ignored, a negative kick speed counts as 0, and pool bounds given as min above max are read the other way
round. The pool (`accuracy_pool`) and the kick (`recoil`) are gated separately. The model is off in every scripted test or
capture mode, meaning any `-ow...shots`, `-ow...test` or `-owautowalk` switch, unless `-owaimmodel` is given. CTest cases cover
the substepping, the signed kick and the bad inputs. Not modelled: the zoomed view wander,
the Hyperion `AlternativeWeaponKick`, skill modifiers on the pool and the impulse.

**Equal-level status chance (Ice_P, third run).** The Maliwan fire pistol (the slice mission pistol's parts on the elemental
balance `Pistol_Maliwan_2_Fire`, since a mission balance cannot be readied outside its mission), 81 shots at a bullymong whose
level was set in memory to Maya's (6 -> 8, `SetExpLevel`). `RollChanceForStatusEffect` ran 86 times: 43 for bullet hits and 43
for the firing mode's impact explosion. `ApplyStatusEffect` followed 18 bullet rolls (42 %) and 23 explosion rolls (53 %). The
card's flesh chance is 23.4 %. At equal levels the level-difference scale is 1, so if it were the only missing factor (section
3) about 10 bullet procs would be expected; 18 is 2.8 standard deviations above that. **The x1 prediction is not supported with skills on; see section 13 for skills off.**
The proc rate is again about twice the card chance, as it was against a level-7 target. Maya's skills were active in this save,
so a skill's instigated status-chance modifier can't be ruled out. The cause of the x2 stays UNVERIFIED. The host keeps the
data's level-difference scale and the card chance; a skills-off rerun would settle it.

*Likely explanation (UNVERIFIED).* The skill allocation of this save could not be read: the repository has no save reader,
and the probe did not list the skill tree. The run did record the weapon's damage stack. It holds a single modifier,
`GD_Challenges.BadassSkill.BadassSkill`, scale +1.0175, so 45.30 x 2.0175 = 91.39 (the live `InstantHitDamage`). The profile
carries a large Badass Rank, and Badass Rank has an elemental effect chance stat that reaches the roll as an instigated
chance modifier. A level-8 Maya has at most 4 skill points (NATIVE_PROGRESSION.md), so a Cataclysm skill can add at most a few
ranks. Without either, the measured factor is 42 / 23.4 = 1.79; at 18 of 43 its 95 % interval is about 1.2 to 2.5. With a
modifier of that size, the equal-level x1 level scale would fit. The same profile was presumably used for the 2026-10-23 counts
(Maya 54 against level 7), so the "x2 = level scale" reading of section 3 is suspect too. The rerun, with skills and Badass
Rank zeroed in memory, should also read the controller's `InstigatedStatusEffectStatusChanceModifier` and
`InstigatedIncendiaryStatusChanceModifier` before firing.

## 11. Shot visuals from the cooked particle systems (2026-11-04, slice projectiles lane)

AI-assisted (Claude). The section 6 critic stalled at 4.2 / 10 because the host's explosions and trails were hand-made shapes;
the note there said the next step was to read BL2's own particle systems. This section is that first step, for explosions and
rocket trails. It is data and host code; no live session was used, and the look is still a stand-in (UNVERIFIED).

- **Data (cooked Startup.upk).** A firing mode names its tracer (`FiringModeDefinition.PartSysTemplate`), a projectile its
  in-flight effect (`ProjectileDefinition.InFlightEffects`, scaled by `InFlightEffectsScale`), and an explosion definition its
  look in `ExplosionScale`. That field decodes with the array schema line `ExplosionScale=StructProperty:ExplosionScaleData`
  into bands, each with `MinRadius` / `MaxRadius`, `MinParticleScale` / `MaxParticleScale`, `ExplosionPSTemplate`, a sound,
  a camera shake and an `ImpactEffect`. The default explosive, shock, corrosive, slag and incendiary explosions have three
  bands each (0-192, 193-384 and above).
- **Tool.** `tools/weapon_projectile_fx.py` adds an `fx` block to each recipe's firing block, and one to its Tediore reload
  throw. The block holds the tracer, the trail and the explosion bands, each with its particle layers read by
  `tools/weapon_muzzle_fx.py` (the same fitted lookup-table reading, UNVERIFIED against the engine). A splash that names no
  explosion takes the projectile definition's `DefaultExplosionDefinition`; this is how the Tediore throw gets its explosion.
  On the 21 local recipes: a tracer on every bullet gun, a trail on the three launchers, three explosion bands on the
  launchers, the Maliwan guns, the fire mission pistol, both Torgue guns and the Tediore throw. Synthetic tests in
  `tests/weapon_projectile_fx_test.py` (CTest `weapon-projectile-fx-synthetic`).
- **Host.** An explosion plays the band whose radius range holds its radius (else the nearest), at a particle scale lerped
  across the band. The band choice by radius is read from the field names, not from the native: UNVERIFIED. The band's layers
  are drawn by `AOpenWillowShotFx::WorldLayers`: camera-facing soft quads with the cooked size x scale, life, colour and alpha,
  one quad per layer. Burst counts, velocities, size-over-life and textures are not modelled. Recipes without the block keep the
  older look. Rockets leave their trail's layers behind every 0.05 s (host rate, since the emitter spawn rate is not read). All of
  it goes through the shot effects' per-frame budget (96).
- **Not done here.** The tracer's look: the cooked tracer colour is multiplied by a texture the host does not have, so lane E's
  tracer look and gate stay. Per-surface impact effects (`WeaponImpact`, `ImpactEffect`); the element tracer collections
  (`ConditionalPartSysTemplate`); grenades. The matched-frame critic round for this look is pending the hub's frame capture.

## 12. Grenades: the stock grenade mod on the grenade key (2026-11-06, slice projectiles lane)

AI-assisted (Claude; the data tool by a Claude Haiku sub-agent, reviewed). **The stock frag has not been thrown live yet.** One
session (2026-11-06, `tools/real_game/projectiles_capture.ps1 -StatusOnly -NoSkills -Grenades`; saves and config restored and
hash-compared) threw 4 grenades, but the save's equipped mod is the Sky Rocket (`Grenade_SkyRocket`), a different grenade.

- **Measured on the Sky Rocket; UNVERIFIED for the frag.** The projectile is first seen 0.52-0.53 s after `ThrowGrenade` (4 of 4);
  `GrenadeThrowComplete` comes 1.04-1.19 s after it. The launch speed was 1694-1701 uu/s, and the frag's `SpeedFormula` has the
  same 1700. First seen 84-110 uu ahead of the eye (up to a frame of flight included), about 20 uu to its left, -1 to +15 uu
  vertically. Two throws flew along the aim; two show about 16 degrees up, but the aim was tracking a moving target and the
  probe's view read failed, so an upward bonus is not settled. The Sky Rocket's own fuse, damage, radius and children are not
  recorded here because they say nothing about the frag.

- **Script (read, in our own words).** `WillowPlayerController.ThrowGrenade` needs the weapon to be able to act and a
  grenade to spend (`CanThrowGrenade`: not while downed, unless an attribute allows it). It then plays the body's throw special
  move and runs it as a shared weapon action with the completion `GrenadeThrowComplete`, which only clears the throwing flag.
  Without a grenade it notifies "out of grenades". The projectile's release is not in those script functions (an animation
  notify or a native is the suspect), so its point and timing are for the live read.
- **Data.** `tools/grenade_mod.py` resolves `GD_GrenadeMods.A_Item.GrenadeMod_Standard` with one part per list (default: the first
  of each) at a level into a block under ignored `local/items/grenades/`: `Grenade_DefaultFrag`'s speed, gravity scale,
  lifespan and upward bonus (with what its class default fills in) and bounces, the radius (the explosion's formula plus the BlastRadius slot), the damage (`Init_ProjectileDamage` x the projectile's
  scale x the Damage slot), the element and the explosion bands. Its own `unverified` list names each inferred rule.
- **Fuse: settled in section 14.** The default Trigger part gives the FuseTime slot a total of 0; the fuse comes from the
  projectile's behaviour graph, not from that slot. The bounce, fuse and falloff statements below are as first shipped; section 14
  replaces them.
- **The key.** `OWGrenade` is bound to G next to the other gameplay keys. The game's input config files do not hold gameplay keys as
  text, so the evidence is the live read: on this profile each of 4 G presses was followed by a logged `ThrowGrenade`. Whether
  G is the shipped default, rather than this profile's setting, is UNVERIFIED.
- **Host.** `AOpenWillowWalker::ThrowGrenade` (key G, `OWGrenade`) spends one grenade from the Grenade ammo pool and launches
  a falling projectile with the block's values 0.52 s later, from 80 uu ahead of the eye and 20 uu to its left, along the view;
  a 1.05 s lockout covers the throw. Its explosion uses the section 11 bands and the hit source `Grenade`. **UNVERIFIED for the
  frag:** that release delay, lockout and offset (all from the Sky Rocket), the damage, radius and bounces (from the data, not
  seen live), the data's upward velocity bonus added to the launch (the projectile's own field; the rule is read from script, not
  measured), and spending the grenade at the press. A block without positive damage does not load. **Stand-ins:** no throw
  animation; the falloff is the class
  default. A throw needs a closed menu, an active Maya and Maya not down (the script's `CanThrowGrenade`; the attribute that allows
  it while down is not modelled).
- **Test.** After the last gun the firing test presses G through the player controller's input path, so the binding is exercised,
  and logs `OWFIRING grenade`. `tools/check_weapon_firing.py` requires it to have been thrown, to have spent one grenade, to have
  positive loaded damage and to have hit the dummy with positive damage, with every hit counted as Source Grenade, and a second
  press with the pool emptied to have been refused (`--legacy` skips this for older logs). `tools/test_weapon_firing.ps1` generates the block
  when it is missing. Synthetic tests: `tests/grenade_mod_test.py` (CTest `grenade-mod-synthetic`).

## 13. The status chance at equal levels, skills off (2026-11-06, slice projectiles lane)

AI-assisted (Claude). Same session as section 12. The Maliwan fire pistol at a bullymong whose level was set in memory to Maya's
(5 -> 8), after `fp3_no_skills`. **x1 at equal levels is consistent with this run:** fire took on 8 of 42 gun rolls (19 %; bullet
6 of 21, impact explosion 2 of 21) against the card's flesh 23.4 %; at least 8 has probability 0.80 at 23.4 %, at most 8 has
probability 0.0002 at twice that. It is a small sample, against a different class (`CharClass_PrimalBeast_Ranged`; the skills-on
run used `CharClass_PrimalBeast`). The target also stood in environmental damage areas that applied `Status_Shock` 358 times;
those rolls are not counted, and whether they affect the fire rolls is not known.

**The mechanism behind the skills-on ~x2 (41 of 86, section 10) stays UNVERIFIED.** The state did change: the gun's damage stack
held Badass Rank's +1.0175 in that run and is empty in this one. But the controller's five `Instigated*StatusChanceModifier`
fields read 1.0 both before and after skills were switched off, so the factor is not in those fields; `fp3_no_skills` also
listed no active skills before it ran, so its list cannot name the cause. **The higher-level scale (x2 at 6+ levels) is
unmeasured:** the older x2 against level-7 targets is no evidence for it, since that profile's skills were on.

## 14. The frag itself: contact, fuse, falloff (2026-11-06, slice projectiles lane)

AI-assisted (Claude). One real-game session threw `Grenade_DefaultFrag` 4 times by naming it in `ThrowGrenade`
(`tools/real_game/projectiles_capture.ps1 -StatusOnly -NoSkills -Grenades -GrenadeDefinition ...`, a runner mode on branch `feature/slice-projectiles-frag-throw` @ d96b842, not in this change; saves, config and SDK settings
restored and hash-compared). **The Sky Rocket mod was still equipped, so this is the frag's definition without the stock mod's
parts.** Host suite evidence below is pending; a passing suite will mean "the host did this", not parity.

- **The graph (read from the packages, tool: `tools/grenade_mod.py` `contact_rules`).** Hitting the world bounces at elasticity 0.4;
  hitting a damagable object bounces at 0.2. Either then passes a once-only gate and sets a 1.8 s timer. The spawn sets a 6 s
  timer. Either timer running out detonates; no contact does. Which event a timer fires is read from the pairing across the packages'
  projectile graphs (28 read: an omitted `TimerId` with `Timer1`, `BTIMER_2` with `Timer2`, `BTIMER_3` with `Timer3`, no pairing against it;
  not confirmed in the running game), and the tool refuses a graph where either timer's own event does not detonate. The explosion has `bDisableRadiusDamageFalloff`. The earlier block
  was wrong here: it counted the two Bounce behaviours as "2 bounces" and read the Detonate as an impact fuse.
- **Confirmed live against that graph (4 of 4 throws, small sample).** No throw exploded on a contact: each hit a target's collision
  or something else without a `HitWall` at about 0.2 s (speed x 0.2, no `TakeDamage`), then touched the terrain 4 to 5 more times
  (tangential speed x about 0.4 each) and came to rest. The explosion came 2.00-2.06 s after spawn and 1.80-1.88 s after that first
  contact: these frame-sampled ranges agree with the 1.8 s timer but do not separately prove it starts at contact. Falling gravity
  read -800 uu/s^2 (scale 0.8 of 1000) on all 4.
- **Damage and radius in this session.** `TakeRadiusDamage` read base damage 63.28 and radius 500, `bFullDamage` true, and all 7 pawn
  hits took 63.28 at distances from 313 to 523 uu (centre distance; measured to the pawn's location, so past 500 is possible). The
  projectile was initialised with `InExpLevel` 1, and `tools/grenade_mod.py --level 1` gives 63.28: the damage formula is confirmed
  **at level 1 only**. 500 is the `Behavior_Explode` formula without the stock mod's -120 slot.
- **UNVERIFIED.** That a thrown stock mod uses level 8 and the -120 slot (the block's 148.87 and 380): naming a definition applied
  neither, and the equipped mod's attributes did not reach the frag's radius or fuse, so a throw with the stock mod equipped is
  still needed. What the first contact at 0.2 s was (the target is the suspect: about 70-100 uu from its location, no wall event).
  The 6 s timer (no throw went without a contact). The FuseTime slot's effect on these timers. Release delay, lockout and offset stay from the Sky Rocket.
- **Upward bonus: inferred, still UNVERIFIED.** The view read failed again, so the aim is not known. But the launch speeds of the 4
  throws differ (1650-1672 uu/s) and become steady (1705-1708 uu/s) once the data's 270 uu/s is taken off Z. That points to the
  bonus applying in world Z on top of the projectile's speed along the aim, which is what the host does.
- **The firing test's aim.** The first host run (at #115's f6fc289, pc-hummer) threw at the shapes dummy's centre and missed: about 180 uu
  up at the dummy, above its torso and 20 uu beside its head, then an explosion far past the radius. The fault was the test's, not the
  flight: it aimed as if there were no bonus, gravity or start offset. The test's first aim fix never ran on the next host run (its launch direction did not change): `InputKey` only queues the press,
  so the grenade data was not loaded yet when the aim was computed. The test now aims once the data is loaded, where the lob meets the dummy, and logs
  `aimed` and `first_contact`, so a skipped aim fails and a miss names what the grenade touched. It also logs the dummy's health around the throw: the checker requires a
  decrease or a death on that dummy, because an invulnerable target records hits without losing health. That a player must lead a throw follows from this flight; the
  real pawn is larger than the shapes dummy.
- **Host.** `ParseGrenade` takes elasticities, both timers and no-falloff from the block's `projectile.contact` and refuses a block
  without it. `AOpenWillowProjectile::Impact`: every contact bounces, a target takes no direct damage, the first contact starts the
  timer once and the earlier timer wins. **Stand-in:** below 20 uu/s on ground-like normals the grenade stops; the limit is ours.
  The firing test reports `contact_age`, `fuse_age` and `contact_fuse`; `tools/check_weapon_firing.py` requires full damage and the
  explosion within 0.2 s of the contact fuse.
- **Open.** `tools/weapon_firing.py` `projectile_of` still derives `explode_on_impact` and bounce counts for weapon projectiles by
  counting behaviours. That is unchanged and may have the same weakness for a bouncing rocket; no slice gun is known to be affected.
