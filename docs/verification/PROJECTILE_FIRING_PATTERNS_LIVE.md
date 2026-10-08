# Projectiles and firing patterns from the running game (lane L7, 2026-10-08)

AI-assisted (Claude), real-game lane L7. One launch of the original game driven through `tools/real_game/` (level-8 Maya,
`-Character=Save0008.sav`, saves blocked from the first frame, memory-only changes, run lock, closed through its window). This record holds
counts, conclusions and formulas only: no dump, no listing, no game data. The raw rows (firing modes, projectile definitions, barrel and type
tables, 27 shot recordings) stay under the ignored `local/realgame/L7/`. Every rule is labelled **OBSERVED** (seen in this run, with how) or
**UNVERIFIED**. This record answers the questions left open at the end of [NATIVE_WEAPON_FIRING.md](NATIVE_WEAPON_FIRING.md) (sections 3 and 4)
and extends [RTSE_GUN_DATA_CROSSCHECK.md](RTSE_GUN_DATA_CROSSCHECK.md) to the firing side; it changes no earlier record.

## 1. Method

- **Data (parts A and B):** the SDK's `find_all` over `FiringModeDefinition`, `ProjectileDefinition`, `WeaponPartDefinition`,
  `WeaponTypeDefinition`, `WeaponPartListCollectionDefinition` and `WeaponBalanceDefinition`, every property read through the class's own field
  list (`tools/real_game/scripts/firing_probe.py`, `fp_enumerate`). The cooked side is `Startup.upk` decoded by `tools/weapon_recipe.Package`
  with the weapon-stats schema. Counts are of what the running game had loaded in Sanctuary (DLC packages included).
- **Shots (part C):** hooks (PRE and POST) on the script-visible functions of one shot: `WillowWeapon.FireAmmunition`, `InstantFire`,
  `GetFiringModeDefinition`, `FiringModeDefinitionFire`, `ProjectileDefinitionFire`, `GetAdjustedAim`, `WillowPlayerController.GetAdjustedAimFor`,
  `FiringModeDefinition.GetFiringPatternAdjustments`, `NotifyFiringPatternWhenShotComplete`, `WillowWeapon.AddSpread`,
  `WillowLightProjectileManager.AddProj` and `TakeDamage` on victims, plus a per-frame read of every entry of the light projectile manager's
  `MyProjs` array and of every live `WillowProjectile`. All shots were real left-mouse presses. 27 recordings, 168 shots, 485 pellet rows
  (about 26 gun builds: the five slice guns with their exact parts, the same guns with a replaced barrel, rolled launchers, snipers and shotguns,
  and two guns the save carried). Guns were spawned into the backpack (saves blocked, backpack size raised in memory) and equipped with
  `ReadyBackpackInventory`; `bInfiniteAmmo` was set in memory. Shots were fired at a wall 1008 uu away in Sanctuary (pattern, spread, speed) and
  at live bullymongs on `Ice_P` (hits; the player's `TakeDamage` was blocked by a hook there).
- **Traps worth knowing (new):** (1) a native function's out parameter is **not** filled in the POST hook's argument struct
  (`GetFiringPatternAdjustments`' `OutputAimAdjustment` showed the previous call's value); the pattern offset is read instead as
  `AddSpread.BaseAim - GetFiringPatternAdjustments.InputAimAdjustment`, or, when the mode suppresses the spread cone (no `AddSpread` call), from the
  ray of `AddProj` (`EndTrace - StartTrace`). (2) `ReadyBackpackInventory` equips a **copy**: the weapon object after equipping has a new name.
  (3) A weapon built from a `MissionWeaponBalanceDefinition` (the Fire mission pistol's balance) vanishes when readied outside its mission; the
  elemental-balance twin `Pistol_Maliwan_2_Fire` with the same parts was used. (4) Native-to-native calls are invisible: `ProcessInstantHitBullet`
  (called from the manager's tick) never showed; hits were seen through `TakeDamage`.

## 2. What exists (counts; OBSERVED by enumeration)

| thing | live | cooked `Startup.upk` |
|---|---|---|
| `FiringModeDefinition` objects | 150 (plus the class default) | 88 |
| of them FireType Bullet / Rocket / Beam / HitScan | 101 / 43 / 6 / **0** | |
| modes with `FiringPatternLines` | 37 (113 have none; 1 line: 14 modes, 9 lines: 4, 16 lines: 3, up to 30) | 23 (the lines themselves are undecoded in the cooked reader) |
| modes with random pattern line / scale by spread / suppress spread / reset each shot / penetrate pawns | 14 / 13 / 11 / 6 / 9 | |
| modes with ricochets above 0 / non-zero trace extent / non-zero air friction | 8 / 7 / 16 (12 have an `Acceleration` vector) | |
| pattern lines: single point (`bUseStartPointOnly`) / rectangle between two offsets / with custom wave motion | 162 / 71 / 53 | |
| `ProjectileDefinition` objects | 159 (plus default); 41 referenced by a mode | 79 |
| of those referenced: type Rocket / Unknown / Frag grenade / Protean grenade | 18 / 15 / 4 / 4 | |
| projectile classes | `WillowProjectile` 119, `WillowServerSideProjectile` 27, `WillowLocalOnlyProjectile` 13 | |
| `WeaponTypeDefinition` objects | 35 | 33 |
| type defaults: Bullet / Rocket | 30 / 5 (the five launcher types) | same |
| barrel parts (`WP_Barrel`) | 202 | 129 |
| barrels with a `CustomFiringModeDefinition` | **110** (Bullet 70, Rocket 39, Beam 1) | **62** (Bullet 37, Rocket 25) |
| non-barrel parts with a custom mode | 1 (a DLC launcher barrel typed `WP_Accessory2`) | 0 |
| elemental parts with a custom mode | **0** | 0 |

- 121 of the 150 modes are reachable as a barrel's custom mode or a type default; the other 29 (24 Bullet, 5 Beam) are children or helpers spawned
  by behaviours (Flakker child bullets, grenade-mod beams) and are never a weapon's own mode.
- No mode has FireType HitScan, so the native note's `HitScan` branch is unused by the installed data. Beam modes: 6 (five grenade-mod beams and
  one launcher barrel, the DLC `Pyrophobia`).
- Tracer particle (`PartSysTemplate`) over the 150 modes: 21 distinct values; `Part_Wep_Tracer_Default` 49 modes, none 40 (the rocket modes: the
  projectile carries its own in-flight effect), `Part_Wep_Tracer_Wavy` 20, `Part_Wep_Tracer_Ricochet_Bounce` 10, `Part_Gyrojet_Tracer_Default` 6.

## 3. Barrel to firing mode (OBSERVED)

- **Resolution:** `GetFiringModeDefinition` returned the barrel's `CustomFiringModeDefinition` when the barrel has one and the weapon type's
  `DefaultFiringModeDefinition` otherwise, in every gun build (e.g. a Bandit-type shotgun with `SG_Barrel_Tediore_Blockhead` returned
  `Bullet_Shotgun_Blockhead`; the same shotgun with `SG_Barrel_Jakobs`, which has no custom mode, returned the type's `Bullet_Shotgun_Default`;
  a Jakobs assault rifle with `AR_Barrel_Torgue_Torgue` returned the rocket mode). No elemental part carries a mode, so the third rule of the
  native note (elemental part) is never exercised by this install's data.
- **The barrel part decides the projectile kind:** an assault rifle fired a rocket (3,400 uu/s base) with the Torgue-Torgue barrel; a shotgun
  fired a rocket with `SG_Barrel_Alien` (the Splatgun) and with `SG_Barrel_Torgue_Flakker`; a launcher barrel `L_Barrel_Maliwan_Pyrophobia` made
  `FiringModeDefinitionFire` take the Beam path (no `AddProj`, no projectile).
- **Cooked against live (Startup parts only):** the 62 barrels with a custom mode in the cooked data are exactly the 62 Startup barrels that carry
  one live, with the same mode in 62 of 62; the 49 further live parts are in DLC packages the cooked file does not contain. All 33 type defaults agree
  (33 of 33). So `tools/weapon_stats.py`'s rule (`barrel.CustomFiringModeDefinition or type.DefaultFiringModeDefinition`) is right; its count of 62 is
  the Startup count, and a full install has 111 parts (110 barrels).
- **Disagreements:** over 88 modes x 16 scalar fields: 2 differences, `Bullet_Shotgun_TidalWave.NumRicochets` (cooked 2, live 3) and
  `Bullet_Shotgun_ConferenceCall_Child.bPenetratePawn` (cooked true, live false); `Speed` agrees on all 88. Over 79 projectile definitions
  (speed formula, gravity, lifetime, type, physics, default damage type and explosion): 1 difference, `Grenade_SkyRocket.GravityScaling` (cooked -2,
  live -1). The same kind of load-time difference NATIVE_WEAPON_RULES section 7 and REALGAME_GROUND_TRUTH describe for weapon types; its source is still
  open (UNVERIFIED).

## 4. Per gun (OBSERVED; speeds read from the manager's live bullet entries or the projectile's velocity)

`speed` = mode `Speed` (bullets) or `ProjectileDefinition.SpeedFormula` (rockets) times the weapon's `ProjectileSpeedMultiplier`, which the
weapon's own part effects set (1.0 to 2.6 here). "Observed" is the first sampled velocity, after at most one frame of flight.

| gun (parts as built) | mode | FireType | pellets | speed formula = observed | spread cone | pattern |
|---|---|---|---|---|---|---|
| Fire mission pistol (Maliwan type, `Pistol_Barrel_Jakobs`, Fire element) | `Bullet_Pistol_Maliwan` | Bullet | 1 | 12,000 x 1.0 = 12,000 | Spread 0.968 deg | none |
| slice Jakobs pistol (`Pistol_Barrel_Torgue`) | `Bullet_Pistol_Default` | Bullet | 1 | 21,500 x 1.3 = 27,950 | 0.90 deg | none |
| slice Hyperion SMG (`SMG_Barrel_Maliwan`) | `Bullet_SMG_Default` | Bullet | 1 | 20,000 x 1.6 = 32,000 | 0.714 deg | none |
| slice Jakobs assault rifle (`AR_Barrel_Vladof`) | `Bullet_Assault_Default` | Bullet | 1 | 25,000 x 1.3 = 32,500 | 1.5 deg | none |
| slice Bandit shotgun (`SG_Barrel_Jakobs`) | `Bullet_Shotgun_Default` | Bullet | 10 | 20,000 x 1.0 = 20,000 | 10.8 deg per pellet | 1 rectangle line, yaw -455..+455 |
| Jakobs shotgun the save carries | same mode | Bullet | 7 | multiplier 1.5 read, bullets not sampled | 5.69 deg per pellet | same line |
| Hyperion shotgun (type default) | `Bullet_Shotgun_Hyperion` | Bullet | 6 | 20,000 x 1.6 = 32,000 | 6.9 deg per pellet | 1 line, **pitch** -455..+455 |
| Torgue shotgun (type default) | `Bullets_Shotgun_Torgue_GyroJet` | Bullet | 5 to 9 | 600 start, accelerating (section 5) | 5.9 to 9.6 deg | 1 line, yaw -546..+546 |
| Jakobs sniper (`SR_Barrel_Hyperion`) | `Bullet_Sniper_Default` | Bullet | 1 | 45,000 x 1.3 = 58,500 | 0.96 deg | none |
| shotgun + `SG_Barrel_Tediore_Blockhead` | `Bullet_Shotgun_Blockhead` | Bullet | 9 | 2,000 x 1.0 | **suppressed** | 9 start-point lines, sequential, scaled by spread, reset each shot |
| shotgun + `SG_Barrel_Torgue_Teeth` | `Bullet_Shotgun_Teeth` | Bullet | 12 | 3,200 x 1.0 | suppressed | 10 start-point lines, sequential, continues across shots |
| Jakobs pistol + `Pistol_Barrel_Vladof_Infinity` | `Bullet_Pistol_Infinity` | Bullet | 1 | 21,500 x 1.3 = 27,950 | suppressed | 16 start-point lines (a loop), sequential, scaled by spread |
| Jakobs pistol + `Pistol_Barrel_Dahl_Tinderbox` | `Bullets_Pistol_Bandit_Tinderbox` | Bullet | 1 | 2,500 x 1.3 = 3,250 (3,249.8) | 1.08 deg | 1 start-point line, pitch +364 (a fixed aim offset of 2 deg) |
| assault rifle + `AR_Barrel_Bandit_Madhouse` | `Bullet_Assault_Bandit_Madhouse` | Bullet | 1 | 3,500 x 1.3 = 4,550 | 1.2 deg | 4 zero-length lines picked at random; 2 ricochets |
| Hyperion SMG + `SMG_Barrel_Bandit_Lascaux` | `Bullet_SMG_Lascaux` | Bullet | 2 | 20,000 x 1.6 = 32,000 | suppressed | 16 rectangle lines, random line, scaled by spread |
| assault rifle + `AR_Barrel_Torgue_Torgue` | `FM_Rocket_Torgue` (assault-rifle copy) | Rocket | 1 | 3,400 x 1.3 = 4,420 | 1.2 deg | none |
| Torgue launcher (rolled) | `FM_Rocket_Torgue` | Rocket | 1 | 2,600 x 1.0 = 2,600 | 5.0 deg | none |
| Vladof launcher | `FM_Rocket_Vladof` | Rocket | 1 | 5,200 x 1.0 = 5,200 | 4.0 deg | none |
| Maliwan launcher | `FM_Rocket_Maliwan` | Rocket | 1 | 3,000 x 2.6 = 7,800 | 2.45 deg | none |
| Bandit launcher | `FM_Rocket_Bandit` | Rocket | 3 rockets | 3,000 x 1.0 = 3,000 | 4.0 deg per rocket | none |
| shotgun + `SG_Barrel_Alien` (Splatgun) | `Bullets_Splatgun` | Rocket | 3 | not sampled | 10.4 deg per rocket | none |
| shotgun + `SG_Barrel_Torgue_Flakker` | `Bullets_Shotgun_Torgue_Flakker` | Rocket | 4 | not sampled | 15.7 deg | 3 start-point lines (0, +2730, -2730 yaw), sequential |
| launcher + `L_Barrel_Maliwan_Pyrophobia` (DLC) | `FM_Rocket_Maliwan_Pyrophobia` | **Beam** | 1 | no projectile | 2.14 deg | none |

## 5. Rules (each with how it was observed)

**R1. Pellet loop (OBSERVED, 168 shots).** One `FireAmmunition` makes one `InstantFire`, one `GetFiringModeDefinition`, one
`GetAdjustedAimFor` and one `FiringModeDefinitionFire` (Bullet, Beam) or `ProjectileDefinitionFire` per projectile (Rocket); inside
`FiringModeDefinitionFire` each pellet runs `GetFiringPatternAdjustments`, then `AddSpread` unless the mode suppresses it, then `AddProj`, and
the shot ends with one `NotifyFiringPatternWhenShotComplete`. Pellet counts per shot equalled the weapon's `ProjectilesPerShot` in every
recording (1, 2, 3, 4, 5, 6, 7, 9, 10, 12), clip and ammo handled once per shot, not per pellet. A beam shot called `FiringModeDefinitionFire` and
never `AddProj`.

**R2. Aim before the pellet (OBSERVED).** `GetAdjustedAimFor` returns the view rotation plus a random offset inside a disc of radius
`AccuracyPool/2` degrees (observed ratio offset / bound at most 1.022 on 20 of the 23 analysed builds; the excess is rounding of the integer rotator), the same for
all pellets of a shot (it is called once per shot). Exceptions: the SMG and Lascaux bursts (ratio 4.0 and 4.8: the view kick moves the base aim,
consistent with NATIVE_WEAPON_FIRING section 5, not separated) and, unexplained, the four shots of the Infinity pistol (ratio up to 1.8 of a 0.40 deg
cone; **UNVERIFIED**). The pool value at the moment of each shot was read: it sat at its minimum between
single shots 1.3 s apart (1.39, 0.88, 2.19, 0.40 for the pistols, the shotgun and the rifle) and **fell** during a Hyperion SMG burst
(8.57 to 4.76 over 9 shots, about 6.7 per second): the reverse-recoil gun gets more accurate while firing.

**R3. Pattern offset (OBSERVED).** `GetFiringPatternAdjustments` composes one line's offset onto the incoming aim as a rotator addition
(pitch and yaw units, 65,536 per turn, 182.04 per degree). Line choice and shape:
- `bFireRandomlyFromPattern` false: the line is `NextFiringPatternIndex` modulo the line count and the index then advances by one **per pellet**
  (Blockhead 0..8; Teeth 0, 2, 4, 6 at the start of four 12-pellet shots with 10 lines; Infinity 0, 1, 2, 3 over four single shots; Flakker 0, 1, 2).
- `bResetPatternAfterEachShot` true: the index is 0 at the start of every shot (Blockhead, four of four shots). False: the index continues across
  shots, 1.5 s apart (Teeth, Infinity). Initial value 0 on a new weapon.
- `bFireRandomlyFromPattern` true: the observed line index was random (Madhouse: 0, 2, 2 of four lines; Lascaux: 0, 6, 2, 8, 8, 3, 13, 10, 1, 1 of 16).
  With one line (the shotgun defaults) the index is always 0. How the random line is drawn (uniform over the count) is **UNVERIFIED** beyond these.
- A line with `bUseStartPointOnly` gives its `StartPoint` as the offset (Tinderbox pitch 364 on every shot; Blockhead, Teeth, Infinity, Flakker
  offsets equal their lines). A line without it gives a **random point on the segment StartPoint..EndPoint**: shotgun yaw offsets over 80 pellets
  ranged -448..448 for a line of -455..455 with standard deviation 282 (uniform: 263); the Hyperion shotgun's offsets were pitch only
  (-428..400, 24 pellets, deviation 236) and the Torgue shotgun's yaw only (-508..456, 36 pellets, deviation 305, line +-546).
- `bScalePatternByWeaponSpread` true (Blockhead, Teeth, Infinity, Lascaux): the offset is multiplied by `max(WeaponSpread / BasePatternSpread,
  MinPatternScale)`. Blockhead 6.957 / 7 = 0.994: observed 540 and -544 for lines of +-546; Teeth 0.994: 720 and 1444 for 728 and 1456;
  Infinity 0.9 / 2.1 = 0.43, below the floor 0.5: observed -48, 88 / -92, 180 / -92, 292 for lines (-91, 182) / (-182, 364) / (-182, 591), i.e. the
  floor was applied. Ratios above 1 were not tested (the formula's upper end is **UNVERIFIED**; the lower floor is observed).

**R4. Weapon spread cone (OBSERVED).** Unless `bSuppressWeaponSpread` is true, `AddSpread` runs once per pellet on the pattern-adjusted aim and
returns it rotated by a random offset whose magnitude never exceeded `Spread/2` degrees (271 pellet rows over 19 guns, 0 outside; the largest
radius was 1.018 of the bound on the 0.714 deg SMG, rotator rounding of a 65-unit radius, and 0.997 on the others). The draw shape (an ellipse per NATIVE_WEAPON_FIRING section 4, formula `yaw = r1*acc`, `pitch = r2*sqrt(1-(2 r1)^2)*acc`
with r in (-0.5, 0.5)) is consistent with the bound but its density was not fitted (**UNVERIFIED**). With `bSuppressWeaponSpread` true (Blockhead,
Teeth, Infinity, Lascaux, 11 of 150 modes) `AddSpread` was not called at all and the pellet direction was exactly aim plus pattern (the pattern
offsets above were read from the `AddProj` ray, which equalled `aim + offset * scale` to within a few rotator units).

**R5. The bullet (OBSERVED).** `AddProj` is called with the start (the eye, `StartTrace`), the end (the ray traced `WeaponRange` = 16,384 uu to the
first hit, 1,008 uu at the wall), the shooter, the mode, `bStartParticleTracerAtWeapon` (true for every player shot) and `bRicochetTowardsEnemy`
(false for all). The manager keeps an entry per bullet (`MyProjs`) with id, start time, location, velocity, **stored damage**, ricochet and
penetration counters and the mode; read each frame:
- Initial speed = mode `Speed` x weapon `ProjectileSpeedMultiplier`: 13 non-accelerating modes, ratio 1.0000 on every bullet (0.9999 for the Tinderbox) (e.g. 20,000 x 1.6 = 32,000;
  25,000 x 1.3 = 32,500; 45,000 x 1.3 = 58,500).
- Stored damage = the weapon's `InstantHitDamage` per pellet, unchanged, for all 559 sampled bullet entries of 19 guns (10 pellets of 15.67; 5 to 9
  pellets of 40 to 46); the bullet array never held a divided value.
- A negative `AirFriction` accelerates the bullet: per frame `v = v * (1 + |AirFriction| * dt)` (648 frame pairs of the Torgue shotgun, AirFriction -6:
  mean `(v1/v0 - 1) / dt` = 6.002; a pure exponential `exp(6 dt)` would give 5.64). Speed 600 became 683 after 0.02 s, 1,505 after 0.17 s, 2,889 after
  0.29 s and 6,347 after 0.43 s. Positive friction and the 12 modes with an `Acceleration` vector were not observed (**UNVERIFIED**).
- A bullet is swept each manager tick and leaves the array on the frame after its last segment; damage arrives at about `distance / speed` plus
  up to one frame. Measured on live bullymongs: a pistol shot 2,146 uu away (27,950 uu/s, 0.077 s of flight) landed 0.096 s after the shot; a shotgun
  pellet 2,800 uu away (20,000 uu/s, 0.14 s) landed 0.154 s after.
- One bullet per pellet, one `TakeDamage` per pellet that hits, several in one frame when they land together.

**R6. What the victim receives (OBSERVED, `Ice_P` bullymongs, 59 hit rows of the victim's `TakeDamage`: pellets, area hits and rockets).**
`TakeDamage.Damage` equals the stored bullet damage to the last digit (48.29008 from a 48.29008 pistol; 15.67 per shotgun pellet, up to four pellets
in one frame as separate calls); `Momentum` is the ray direction times
`InstantHitMomentum x 100` (length 800 for the pistol type's 8, 700 for the shotgun's 7, 1,000 for the Torgue shotgun's 10); the damage type is the
weapon's source class (`WillowDmgSource_Pistol`, `_Shotgun`); `HitInfo` carries the bone and the physical material (`PhysMat_Flesh`). The
shotgun's `Behavior_Explode` of the GyroJet mode added an area hit per shot (`WillowDmgSource_Grenade`, 33.998 against 39.998 per pellet, 0.85 of it;
the ratio is an observation on one roll, **UNVERIFIED** as a rule).

**R7. Rockets (OBSERVED, 14 launcher shots, 5 designs).** `ProjectileDefinitionFire` spawns one `WillowProjectile` per rocket (the Bandit launcher
three per shot, each with its own spread). Speed from the first sampled velocity: `ProjectileDefinition.SpeedFormula.BaseValueConstant` x the
weapon multiplier (2,600; 5,200; 3,000 x 2.6 = 7,800; 3,000; 3,400 x 1.3 = 4,420), constant in flight (no acceleration), **not** the mode's `Speed`
(0 on every rocket mode). Spawn point: 122 to 134 uu ahead of the eye, 17 to 31 uu to the right, 8 to 13 uu below (per gun; consistent with the
muzzle, **UNVERIFIED** against the decoded socket). The rocket stops at the wall and explodes; direct damage at the victim was `1416.7266845703125`
for a weapon whose `InstantHitDamage` read `1416.7266845703125` (two victims, source `WillowDmgSource_Rocket`, causer the projectile, momentum
10,000). In the data 40 of 41 referenced projectile definitions explode through `Behavior_Explode` with the attribute `D_Attributes.Projectile.ProjectileDamage`
(the weapon's damage); 2 use `Behavior_CauseDamage` with the same attribute.

**R8. Game clock.** Shots, hooks, bullet entries and hits were lined up on `WorldInfo.TimeSeconds`; frames were 13 to 25 ms, so every arrival time
above has about one frame of resolution.

## 6. Where the live values differ from the cooked-data decode (`tools/weapon_stats.py`)

1. **Rocket speed is lost.** The card computes `projectile_speed` as the mode's `Speed` times the multiplier; that is correct for bullets (live
   16 of 16) and gives nothing for the 43 rocket modes whose `Speed` is 0. The speed lives in the projectile definition's `SpeedFormula`.
2. **The firing mode of a DLC barrel is not available from `Startup.upk`** (49 of the 111 custom-mode parts are DLC parts); the card's rule is right
   for what it can see (62 of 62).
3. **Mode scalars:** 2 of 88 differ (section 3); the cooked reader does not decode `FiringPatternLines`, which live carries on 37 modes (23 in
   `Startup.upk`).
4. **`ProjectilesPerShot`** and **spread** of the five slice guns equal the card's (10, 1, 1, 1, 1 and the same spread values).
5. **Damage:** the live `InstantHitDamage` of the five slice guns, built with `AddBackpackWeaponFromDefinitionData` at game stage 8 and grade index 0,
   was **0.7589 times** the card damage `weapon_stats` evaluates at level 8 (63.63 / 48.29, 49.13 / 37.28, 22.26 / 16.89, 36.11 / 27.41, 20.65 / 15.67: the
   same ratio 1.3177 for all five). The same evaluator at level 6 gives 38.47 for the fire pistol, so this is not an integer level shift. The cause
   is **UNVERIFIED** (item level from the spawn path or a player-side damage term); it does not touch any rule above, which are in ratios and formulas,
   but the absolute stored damages in the recordings should not be compared with cards.

## 7. What the host must change (`host/ue5/OpenWillow/Source/OpenWillow/OpenWillowWalker.cpp`, `AOpenWillowWalker::FireWeapon`)

Current behaviour (read, lines 864 to 916): every gun fires one invisible-speed trace along a figure-eight direction (2.1 deg amplitude,
`ShotCount * pi/8` phase) for a fixed 10,000 uu, hits at once and calls `ApplyPointDamage` once with the card damage; the tracer is a visual
only. Nothing reads the weapon's mode, speed, pellet count or spread. In order of how many slice guns each change affects:

1. **Resolve the mode per weapon and carry it in the item** (all five slice guns): `mode = barrel.CustomFiringModeDefinition` else
   `type.DefaultFiringModeDefinition`; `tools/weapon_stats.py` already returns it (`card.firing_mode`). Add to the manifest the mode's FireType, `Speed`,
   `AirFriction`, `NumRicochets`, `TraceExtent`, the four pattern flags, `BasePatternSpread`, `MinPatternScale` and the lines (or the rocket's
   projectile definition and its `SpeedFormula`). Decoding `FiringPatternLines` is now checkable: 37 live modes give the expected values (the
   cooked reader must be extended; live lines are the oracle; no invented offsets).
2. **Fire `ProjectilesPerShot` pellets per press** (shotguns 10, 7, 6, 9, 12; the host currently fires 1): per pellet, pattern offset, then the
   spread cone unless suppressed, then one projectile. Ammo and clip once per shot.
3. **Replace the figure-eight by the observed pattern**: R3 (line choice, per-pellet index advance, reset flag, start-point or random point on the
   segment, scale `max(Spread/BasePatternSpread, MinPatternScale)`), keeping `NextFiringPatternIndex` per weapon (0 on a new weapon).
4. **Per-pellet weapon spread cone** from the card's `Spread` (degrees, disc radius `Spread/2`) except for `bSuppressWeaponSpread` modes; replace the
   fixed 2.1 deg. The accuracy-pool cone (R2) stays a separate, earlier offset shared by all pellets of the shot.
5. **Bullets travel**: speed = mode `Speed` x `WeaponProjectileSpeedMultiplier` (card `projectile_speed` for bullets); sweep the last segment each tick
   (mode `TraceExtent` for the thick ones) instead of an instant trace; land the hit about `distance / speed` later (slow guns: Tinderbox 3,250,
   Blockhead 2,000, GyroJets from 600); apply negative `AirFriction` per frame as `v *= 1 + |f| dt`; range `WeaponRange` 16,384, not 10,000.
6. **Damage per pellet**: each pellet that hits applies the weapon damage unchanged (not once per press); momentum = direction x type
   `InstantHitMomentum` x 100. Crit and element handling stay with the damage pipeline.
7. **Rockets and beams** (launcher types, Torgue barrels, Splatgun, Flakker): spawn a projectile at the muzzle (about 125 to 135 uu ahead of the eye,
   right and below), speed from the projectile definition, explode on contact with the weapon's damage; Beam modes are a separate beam path (no
   projectile, no `AddProj`). Not needed for the five slice guns; needed before any launcher or Torgue barrel is a slice item.
8. **Tracer**: a bullet's tracer starts at the weapon (`bStartParticleTracerAtWeapon` true) and follows the bullet; the host's instant tracer
   should travel at the bullet's speed once item 5 exists; the template per mode is `PartSysTemplate` (`Part_Wep_Tracer_Default` for 49 modes).
9. **Order of the muzzle flash and the shot**: unchanged by this record.

## 8. Open or UNVERIFIED

- Selection of a random line when a mode has more than one and `bFireRandomlyFromPattern` is true (only observed to vary); the upper end of the
  pattern scale (ratio above 1); the exact density of the spread draw; positive air friction and `Acceleration`; ricochet and penetration
  behaviour (counters stayed 0; `NumRicochets` 2 and 3 modes were fired at a wall but a bounce was not followed); homing projectiles; the
  Flakker's and Splatgun's projectile speeds and children; the muzzle socket against the rocket spawn offset; `NextFiringPatternIndex` on weapon
  swap or reload (not exercised); crit, element and distance falloff after `TakeDamage`; the 0.7589 damage factor of section 6 item 5; whether
  `GetMultiProjectileDamage` differs from the plain damage when the damage stack has PreAdd/PostAdd terms (the stored value equalled
  `InstantHitDamage` for every multi-pellet gun, which is what the formula gives with empty add terms).
- The in-flight bullet per-frame update beyond velocity (gravity on bullets was not looked for: 600 to 6,000 uu/s bullets showed straight paths over
  1,000 uu, not a precise test).
- Everything above is one launch on one install (DLC present), one character, one map pair.

## 9. Save safety and checks

`Backup-Saves` first; 60 files (SaveData, Config, SDK settings, Sanity Saver's private copies) hashed before and after. Two differed after the run
as in earlier lanes (Sanity Saver's `Save0008.json.gz`, rewritten at load, and `python-sdk.json`'s "Next Check Time" stamp) and were restored from the
backup; the re-hash shows 0 differences of 60. The 33 save and config files were byte-identical without a restore. Driver removed, the game closed
through its window, run lock released. No Application Control block. Automated checks: none (no code under `src/`, `host/` or `tests/` was
touched); `tools/real_game/scripts/firing_probe.py` compiles (`py_compile`). The tracked additions are this record and that probe.
