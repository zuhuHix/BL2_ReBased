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
| Status chance = surface `BaseChance` x weapon modifiers x further factors | native 4.2 | **not matched**: Maliwan shock 21/96, corrosive 11/50, slag 18/50 procs against a card (flesh) chance of 11.7 / 11.7 / 23.4 %; fire AR 4/36 against 9 %. A factor the host does not apply (level difference: player 54 vs target 7, or the roll's `ChanceModifier` argument) is the suspect (UNVERIFIED) |
| DOT: build up frame time; at 0.33 s deal DPS x built-up time and subtract 0.33; a final partial tick at the end | not read | **confirmed live** on 94 ticks (ratio 1.00000); DPS = the weapon's `StatusEffectDamage`, duration = the definition's |
| Re-procs: shock refreshes its one entry (duration restarts, clock kept); fire and corrosive run parallel entries | not read | observed (a few cases; rule UNVERIFIED) |
| Spin-up: only `BSM_SpinUpToFullFireRate` ramps the interval; `BSM_SpinBarrelWithEachShot` does not gate firing | native 1.5 | read only |
| `AutomaticBurstCount` N: a press fires N shots even if released, then waits for a new press; 1 = one shot per press (`weapon_firing::Burst`, reset when the weapon in hand changes) | native 1.5 + data | **consistent with live**: the slice pistol and rifle (count 1) fired once per hold; the SMG, shotgun (count 0) and Infinity fired repeatedly |
| Tediore reload throw: damage = card damage x rounds left; half to what it strikes, half as a radius-120 splash with falloff | data + observed | **confirmed live** (25.891 x 10 = 258.91; direct 129.455; splash 78.19 at 0.604) |
| The thrown rounds leave with the gun: the reload then takes a full magazine from the reserve | Tediore design | host rule; **not measured** (the live session logged the damage, not the ammo counts) |

## 4. The host (what it does and where it stands in for the game)

* Damage, bullet speed, pellets, pattern, spread, burst, spin-up/down, rocket speed, splash radius/damage/falloff, the
  status effect and the reload throw come from the recipe's `firing` block; nothing is per gun.
* Light bullets are host actors (the game keeps them in a manager): straight at speed, the AirFriction step, a line trace
  per tick from the camera; drawn as a short additive streak from the muzzle (only for weapon types whose view-model
  data enables a tracer, the lane E rule).
* Rockets launch from the muzzle toward the screen-centre hit (native 2.1) and explode when their definition has a
  Detonate behaviour; bullets whose firing mode carries a `Behavior_Explode` (gyrojets) explode where they hit.
* The splash distance is to the target's collision centre (the game measures to a hit-region bone; the live cutoff of
  the radius-250 rocket lay between 380 and 432 uu from the pawn origin). The target's surface is taken as Generic.
* Not modelled (open): the elemental impact explosions of elemental and Torgue bullets (live: Maliwan pistol bullet
  45.30 plus an impact explosion of 0.80 x; Torgue shotgun 0.85 x; from the firing mode's `ExplosionOverrideDefinition`
  collection, not yet decoded); element-vs-surface factors (live on flesh: fire x1.5, corrosive x0.9, shock and slag
  x1.0); child projectiles spawned by a projectile's behaviours (the Maliwan launcher's fire area; the firing block lists
  them under `unsupported` and the host logs them); beams (the tool gives them their own kind, `beam`, listed as
  unsupported; the host fires light bullets for them); runtime damage modifiers (skills, relics; the per-pellet split is
  written but unused); slag's damage bonus and its duration (the recipe's slag duration is unresolved; live 8 s); the status chance
  factor above; the accuracy pool and kick (the recoil data is in the block, the host keeps lane E's kick); bullet ricochets; homing; the launcher reload throw's own motion (live speed steps not fitted).

## 5. Host versus game

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
| Status proc rate | 2-3 x the card chance (Maliwan) | the card chance (open) |

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
