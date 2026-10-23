# Native weapon firing: fire modes, pellets and spread, fire interval, kick, projectiles, splash, status effects (2026-10-22)

AI-assisted (Claude). Behaviour notes written from a local reading of `Borderlands2.exe` in Ghidra 12.1.4 (the
existing project, opened read-only through `tools/ghidra/run.ps1` and a small local helper that reads vtable slots) and
of the installed WillowGame script disassembly (`research/script_disasm.py` output under ignored `local/disasm/`), under
the policy in [LEGAL.md](../LEGAL.md) ("Analysing the executable") and [NATIVE_ANALYSIS.md](../NATIVE_ANALYSIS.md).
Nothing below is a listing, pseudo-code or an address. Natives are named by their registered name, their script name,
or by what they do. Field names were matched to the native code with `tools/ghidra/class_layout.py`; where the code
reads a field through an interface sub-object the offset was rebased before naming, and every field quoted landed on a
field of the expected type. Enum orders were read from the installed packages. Raw output is under ignored
`local/analysis/projectiles/` (its `README.txt` lists the files). Companion to
[NATIVE_WEAPON_RULES.md](NATIVE_WEAPON_RULES.md) (stats, card, parts) and
[NATIVE_WEAPON_VISUALS.md](NATIVE_WEAPON_VISUALS.md).

**Every rule here is `UNVERIFIED` in game.** "Read" means read from native code or script, not confirmed by running the
game. No live session was used (another worker owns them). Each rule has a proposed live observation in the table at
the end.

| # | Rule | Source | Confidence |
|---|---|---|---|
| 1 | Fire type picks the path: Bullet and Beam go to the light-projectile manager, Rocket spawns a projectile actor, HitScan damages at once | script + enum | high (read) |
| 1 | One aim per shot, then `ProjectilesPerShot` independent pellets, each: pattern offset, then spread | script | high (read) |
| 1 | Spread: uniform yaw in a disc of diameter `Spread` degrees, pitch uniform on the chord at that yaw | script | high (read) |
| 1 | `FiringPatternLines`: one random point on the current line, optional spread scaling, index advances per pellet | native | high (read) |
| 1 | Fire interval with Vladof spin-up: linear from `StartingSpinUpFireIntervalMultiplier * FireInterval` to `FireInterval` by `BarrelSpinUpPercent` | native | high (read) |
| 1 | Spin-up percent rises at `1/BarrelSpinUpDuration` per second while firing, falls at `1/BarrelSpinDownDuration` | native | high (read) |
| 1 | Burst interval scales with the current fire interval: `BurstInterval * GetFireInterval / FireIntervalBaseValue` | native | high (read) |
| 1 | Kick: impulse in degrees x 182.044 x zoom lerp, split up/down and left/right from the type's kick ranges, FOV scaled | script + native | medium (read; split formula read, sign handling read) |
| 1 | Accuracy is a resource pool per hand; each shot spends `PerShotAccuracyImpulse` (scaled in bursts) | script | medium (read; pool regeneration and the link to `Spread` not read) |
| 1 | Per-pellet damage on multi-pellet guns: `Base * S + (PreAdd * S + PostAdd) / ProjectilesPerShot` | native | high (read; used by explosions, not checked for bullets) |
| 2 | Projectile actor: speed from `SpeedFormula` x `SpeedMultiplier`, damage, radius, fuse, children from the definition; lifespan `MaxLifespan` | native + script | high (read) |
| 2 | Light bullets: speed `FiringModeDefinition.Speed`, ricochets = `NumRicochets + AdditionalRicochets` | native (spawn) | medium (spawn read; flight tick not read) |
| 3 | Radius damage falloff: full inside 20% of the radius, linear to 20% at 80%, 20% to the edge (class defaults) | script + data | high (read) |
| 3 | Explosion damage of a multi-pellet weapon uses the per-pellet damage above | script | high (read) |
| 4 | Status chance = product of ~10 factors in percent, x 0.01, clamped to [0, 1], then one roll | native | high for the product (read); factor identities medium |
| 4 | Fire-interval chance factor: `(FireInterval / FireRate) ^ power`, short or long power by sign | native | high (read) |
| 4 | Level difference: `GlobalsDefinition.StatusEffectChanceScales`, indexed by `min(|diff|, count) - 1` | native | high (read; table values not decoded) |
| 4 | Radius hits scale the chance by `1 - clamp(dist - cylinder radius, 0, R) / R` unless `bNoStatusEffectChanceFalloff` | script | high (read) |
| 5 | Jakobs ricochet: `AdditionalRicochets` adds to the bullet's ricochet budget; ricochet-to-enemy chance is a skill attribute roll | native | medium (read) |
| 5 | Tediore throw: a behavior sequence on the type; child projectile base values set by type name | script | low (outline only) |
| 6 | Impact response: first `ImpactResponses` entry with the hit's surface type; can disable the bullet's damage | native | high (read) |

## 1. What a shot does

### 1.1 Entry and fire types

`FireAmmunition` (script) runs per shot: `SharedFireAmmunition` consumes ammunition (`ShotCost`) and calls
`InstantFire`, which switches on the firing mode's `FireType` (`FiringModeDefinition.EWillowWeaponFireType`, decoded:
0 `EWWFT_Bullet`, 1 `EWWFT_Beam`, 2 `EWWFT_Rocket`, 3 `EWWFT_HitScan`). Bullet, Beam and HitScan go to
`FiringModeDefinitionFire`; Rocket goes to `ProjectileDefinitionFire`. The firing mode comes from
`GetFiringModeDefinition` (the barrel's or the type's `DefaultFiringModeDefinition`).

After the shot, still in `FireAmmunition`:

* **Accuracy impulse.** `Impulse = PerShotAccuracyImpulse`; if the shot is part of an automatic burst
  (`CurrentBurstShotCount < AutomaticBurstCount`) it is multiplied by `BurstShotAccuracyImpulseScale`. The controller
  spends it on the accuracy pool of the weapon's hand (`AccuracyPool`, or `OffHandAccuracyPool` for an off-hand gun;
  both are `ResourcePool`s; their regeneration and how the pool value becomes the aim cone are **not read**).
* **Kick** (player controllers only). `Kick = Impulse`, or for a type with `bAlternativeKickEnabled` (Hyperion) the
  evaluated `AlternativeWeaponKick` instead; then in both cases `Kick *= 182.0444` (degrees to rotator units) and
  `Kick *= lerp(1, WeaponKickZoomMultiplier, zoom)` with `zoom = GetZoomEffect()`. `AddWeaponKick` (section 1.6).
  The controller's wander smooth-in time is set from `ZoomWanderSmoothInTime_OnFire`.
* **Extra shot.** If `ExtraShotChance > 0` and `FRand() <= ExtraShotChance`, an extra `ExtraFireAmmunition` is
  scheduled `GetFireInterval + ExtraShotDelay` later.

### 1.2 Pellets, pattern and spread (`FiringModeDefinitionFire`, script)

Once per shot: start of the trace (`InstantFireStartTrace`), aim (`GetAdjustedAim`, the pawn's aim incl. auto-aim).
Then **for each of `ProjectilesPerShot` pellets**:

1. `GetFiringPatternAdjustments(aim, Spread, ..., NextFiringPatternIndex)` (native, 1.3) gives the pattern aim.
2. Unless the firing mode has `bSuppressWeaponSpread`, `AddSpread(pattern aim)` (1.4).
3. End of trace: start + direction x `GetTraceRange()`.
4. HitScan (or no firing mode): `CalcWeaponFire` collects impacts. Bullet/Beam: the trace is also run for human
   players (to aim the bullet at the hit location), but no damage is applied here.
5. Bullet/Beam with a light-projectile manager: Beam adds a weapon beam once; Bullet calls
   `WillowLightProjectileManager.AddProj(start, hit location, instigator, firing mode, ..., IsOvercharged,
   ShouldBulletRicochetTowardsEnemy)` and stores the pattern index on the bullet when the pattern line has a custom
   wave motion.

After the loop, for HitScan only, `ProcessInstantHitBullet` runs once per collected impact with `InstantHitDamage`
(section 6 for what it does). Then `NotifyFiringPatternWhenShotComplete` (pattern reset bookkeeping, **not read**).

So for **Bullet** guns (most guns) the damage is applied by the light-projectile manager when the simulated bullet
arrives, not by the script; the flight is section 2.2.

### 1.3 Firing pattern (`GetFiringPatternAdjustments`, native)

If the firing mode has no `FiringPatternLines`, the aim is returned unchanged (returned index -1). Otherwise:

1. Index = the weapon's `NextFiringPatternIndex`, or, with `bFireRandomlyFromPattern`, a uniform random index; an
   out-of-range index becomes 0.
2. Offset rotator = the line's `StartPoint` if `bUseStartPointOnly`, else `StartPoint + (EndPoint - StartPoint) * t`
   with **one** uniform `t` in [0, 1] for both pitch and yaw (each component truncated to an integer rotator unit).
3. With `bScalePatternByWeaponSpread` and `BasePatternSpread > 0`: `scale = max(Spread / BasePatternSpread,
   MinPatternScale)`, offset pitch and yaw multiplied by it (class default `BasePatternSpread` 1).
4. The offset is composed with the aim as a local rotation (aim frame times offset), converted back to pitch and yaw;
   roll is 0. The line's custom wave motion is copied out.
5. `NextFiringPatternIndex = (index + 1) mod line count`. The index used is returned.

Because the index advances per pellet, a shotgun with N lines and N pellets puts one pellet on each line per shot.

### 1.4 Spread (`AddSpread`, script)

`S = Spread` (degrees; the card's accuracy is a remap of it, NATIVE_WEAPON_RULES section 2). For a pawn that is not
human controlled, `S += WeaponTypeDefinition.AIAimError`. If `bDisableWeaponSpread` or `S == 0`, the aim is unchanged.
Otherwise, with `A = int(S * 65536 / 360)` rotator units:

* `u = 0.5 - FRand()` (in [-0.5, 0.5]); `yaw = u * A`;
* `pitch = (0.5 - FRand()) * sqrt(1 - (2u)^2) * A`;
* the offset is applied in the aim's local frame.

So the pellet direction is uniform in yaw across a disc of **diameter** `S` degrees, and at each yaw uniform in pitch
over that chord: not uniform over the disc's area (density rises toward the left and right edges). Each pellet draws
its own two random numbers.

### 1.5 Fire interval, spin-up and bursts (native)

**`GetFireInterval`** (the weapon's override): `base` = the `FireInterval` attribute value. Then:

* While a lock-on is in progress (`LockOnTargetState.LockOnInProgress`) and the type exists: the interval is
  `max(base, WeaponTypeDefinition.LockCoolDownTime)`.
* Otherwise, if the barrel part has `bIsSpinningEnabled` and the type's `BarrelSpinMode` is
  `BSM_SpinUpToFullFireRate` (1): `start = max(StartingSpinUpFireIntervalMultiplier * base, base)` (the barrel's
  multiplier), and `interval = start - (start - base) * BarrelSpinUpPercent`. Linear in the percent.
* Otherwise `base`.

**`TickBarrelSpinUp(dt)`** (only with a type, a barrel with `bIsSpinningEnabled` and `BSM_SpinUpToFullFireRate`):
while `bIsFiringWeapon`, `BarrelSpinUpPercent += dt / BarrelSpinUpDuration` (a duration <= 0 snaps it to 1);
otherwise `BarrelSpinUpPercent -= dt / BarrelSpinDownDuration` (<= 0 snaps it to 0); clamped to [0, 1]. Reaching 1 sets
`BarrelSpinState` to `SPIN_STATE_FullySpunUp` (2) and notifies the state change. The durations are attribute values set
from the barrel (`CalculatePartDependentWeaponBaseValues`, NATIVE_WEAPON_RULES section 1).

**`GetBurstInterval`**: if `FireIntervalBaseValue > 0`, `BurstInterval * GetFireInterval(CurrentFireMode) /
FireIntervalBaseValue`; else `BurstInterval`. So burst pacing follows the current (modified, spun-up) fire interval.

**Burst refire** (`ShouldRefire`, script): with `AutomaticBurstCount > 0`, refire while `CurrentBurstShotCount <
AutomaticBurstCount`, then stop and stamp `LastAutomaticBurstTime`; with 0, refire while the trigger is held
(`StillFiring`). The refire timer itself is the stock `TimeWeaponFiring` with the fire interval.

### 1.6 Kick (`AddWeaponKick`, native)

Inputs: the kick from 1.1 (`K`, rotator units) and the weapon type. Fields: `WeaponKickUp`, `WeaponKickDown`,
`MinimumVerticalPercentage`, `WeaponKickLeft`, `WeaponKickRight`, `MinimumHorizontalPercentage`.

1. FOV scale: `f = tan(FOV / 2) / tan(DefaultFOV / 2)` (the current FOV from a virtual getter, assumed to be `GetFOVAngle`, over the controller's `DefaultFOV`, degrees);
   `M = |f * K|`. Zooming in therefore reduces the kick.
2. `v = |Up + Down| * FRand() - Down` (signed vertical pick) and `h = |Right + Left| * FRand() - Left` (signed
   horizontal pick; the left/right naming follows the field order, **sign convention UNVERIFIED**).
3. Shares: vertical share = `MinimumVerticalPercentage + (1 - MinimumVerticalPercentage - MinimumHorizontalPercentage)
   * |v| / (|v| + |h|)` (only the minimum when `v` is 0); horizontal share = `1 - vertical` (only
   `MinimumHorizontalPercentage` when `h` is 0).
4. The controller's pending pitch kick moves by `vertical share * M` in the sign of `v`, the yaw kick by `horizontal
   share * M` in the sign of `h`, and the kick time is stamped. How the pending kick is applied to the view
   (`ApplyWanderingAndKick`) was decompiled but **not worked through**.

### 1.7 Per-pellet damage of multi-pellet weapons (`GetMultiProjectileDamage`, native)

Walks the `InstantHitDamage` modifier stack (the same four sums as NATIVE_WEAPON_RULES section 1: PreAdd, PostAdd,
positive and non-positive Scale) and returns `Base * S + (PreAdd * S + PostAdd) / ProjectilesPerShot`, with `S = (1 +
up) / (1 - down)` and `Base = InstantHitDamageBaseValue`. Flat additions are thus divided among pellets while the base
is not. The script uses it for explosions caused by a weapon (section 3.2). Whether bullets also use it is **not
read** (the HitScan script path passes `InstantHitDamage`).

## 2. Projectiles

### 2.1 Projectile actors (Rocket fire type; grenades and Tediore throws use the same class)

`ProjectileDefinitionFire` (script): per pellet the same pattern and spread as 1.2. For a human player, when the
physical muzzle differs from the trace start, the screen-centre trace is run and the projectile is launched from the
muzzle toward that hit point (only when the corrected direction is still within 90 degrees of the original). Spawn
`ProjectileDefinition.ProjectileClass` at the start; set `SpeedMultiplier` from the weapon's
`ProjectileSpeedMultiplier`, `bOvercharged`; `InitializeFromDefinition(definition, weapon ExpLevel, ..., particle
template, explosion override)`; then the type's `ProjectileBaseValues` set the projectile's attribute bases (this is how
a rocket's damage follows the weapon); `InitWithWaveMotion(direction, the pattern's custom wave)`; `InitStats`.

`InitializeFromDefinition` (native, the projectile's override) on the server:

* `Definition`, `ExpLevel` (at least 1); `CustomGravityScaling = GravityScaling` when not already set; wave motion from
  the definition's `WaveAmp`, `WaveFreq`, `WavePhase` when any amplitude is non-zero.
* `Damage`, `DamageRadius`, `MomentumTransfer`, `FuseTime` and `NumberOfChildProjectiles` attribute bases from the
  definition's initialization data of the same names (evaluated with the projectile as context).
* Physics mode from `PhysicsMode`; `LifeSpan = MaxLifespan` (or a visibility timer with `bDontDestroyIfVisible`).
* `MaxSpeed = Evaluate(SpeedFormula) * SpeedMultiplier`. (The plain `ProjectileDefinition.Speed` field is not read
  here.)
* Damage type from `DamageTypeOverrideContext/Name` or `DefaultDamageTypeDefinition`; explosion from the override
  argument, `ExplosionOverrideContext/Name` or `DefaultExplosionDefinition`; allegiance from `Allegiance` or the
  instigator's.

`Init` / `SetVelocityAndAcceleration` (script): direction (rotated by `CustomAimDirection` when
`bUseCustomAimDirection`); `Velocity = MaxSpeed * direction`, then `Velocity.Z += UpwardVelocityBonus`; `Acceleration =
AccelRate * normal(Velocity)`. `SetSpeed` resets both. Gravity, terminal velocity, wave motion and homing are applied in
native physics (`GetTerminalVelocity`, `GetCircularHomingAcceleration`, `FindHomingTarget`, homing fields
`HomingThresholds`, `TurningRadius`, `HomingRotationRate`, `SearchRadius`, ...): their bodies were decompiled to
`local/` but **not worked through**.

**Impact and explosion** (script): `DoHitWall` raises the definition's `OnHitWall` behavior event and the hit events;
below `AtRestSpeedThreshold` (default 20) the projectile stops bouncing and raises `OnComeToRest`; `Landed` is a wall
hit. `Explode` raises `OnExplode` and shuts the projectile down. What a projectile does on these events (explode,
bounce, split) is **data**: the `BehaviorProviderDefinition` of the projectile definition, typically a
`Behavior_Explode` (section 3).

### 2.2 Light bullets (Bullet fire type)

`AddProj` (native) records a bullet with the firing mode, start, target, overcharge and ricochet flags. Read:
**ricochet budget = weapon `AdditionalRicochets` + firing mode `NumRicochets`**, raised to at least 1 when the
ricochet-toward-enemy roll succeeded. Fields that drive the flight (`Speed` default 2000, `Acceleration`,
`AirFriction`, `RicochetFriction`, `WaveFreq`/`WaveAmp`/`WavePhase`, `Lifetime` default 5 s, `TraceInterval` 0.2,
`TimerEvents`, `RicochetResponse` with `SplitNum`/`SplitAngle`/`NewSpeed`/`bDetonate`) are data on the firing mode; the
manager's per-tick flight, hit and ricochet code is **not read**.

## 3. Splash and explosion damage

### 3.1 Falloff (`WillowDamageTypeDefinition.CalcRadiusDamageScale`, script)

Inputs: distance `d`, radius `R`. Data on the damage type: `MaxDamageRadius`, `MinDamageRadius` (fractions of `R`) and
`MinDamagePercent`; class defaults 0.2, 0.8 and 0.2. With `f = d / R`:

* if `MinDamageRadius - MaxDamageRadius > 0`:
  * `f < MaxDamageRadius`: scale 1;
  * `MaxDamageRadius <= f <= MinDamageRadius`: `MinDamagePercent + (1 - MinDamagePercent) * (1 - (f - MaxDamageRadius) /
    (MinDamageRadius - MaxDamageRadius))` (linear from 1 down to `MinDamagePercent`);
  * `MinDamageRadius < f < 1`: `MinDamagePercent`;
  * otherwise -1 ("not handled").
* otherwise -1.

`WillowPawn.TakeRadiusDamage` (script) uses it: `bFullDamage` gives 1; else the damage type's scale; a negative scale
falls back to the stock linear `1 - d / R`. Damage `scale * BaseDamage` and momentum `scale * Momentum` along the
direction are applied only when the scale is above 0. With the defaults: full damage within 20% of the radius, falling
linearly to 20% at 80% of the radius, 20% from there to the edge, none outside.

**Where `d` is measured.** For pawns with hit regions, Willow's path collects the body's hit regions that allow radius
damage (critical regions excluded under conditions **not fully read**) and measures from the explosion origin to the
first such region's first bone; otherwise the stock `Actor.TakeRadiusDamage` (Engine script, **not read**) is used.
Whether the directly hit target also takes the splash is decided by the projectile's behaviors (data), not by this
code; nothing here excludes it.

### 3.2 The explosion (`Behavior_Explode`, script)

Radius = `DamageRadiusFormula` evaluated in the behavior's context; **0 becomes 256**. Damage: when the context actor
is a weapon with `ProjectilesPerShot > 1` and the damage formula is based on a weapon attribute,
`GetMultiProjectileDamage() * DamageFormula.BaseValueScaleConstant` (so each pellet's explosion carries the per-pellet
damage of 1.7); otherwise the formula evaluated on the context. Momentum from `MomentumFormula`. Hit normal from the
behavior, the event data, the projectile's `BehaviorHitNormal`, or straight up. Then
`ExplosionDefinition.PlayExplosion(..., radius, damage, bDisableRadiusDamageFalloff, momentum, DamageSource, ...)`:
visuals scaled through `ExplosionScale` bands, and the radius damage through the source's `Behavior_CauseRadiusDamage`
and the damage-over-time manager (`DamageOverTimeManager.AddSourceObject`; **not worked through**).

**Torgue.** `EDamageType` (decoded) has `DAMAGE_TYPE_Explosive` (3); no Torgue-specific native code was seen on this
path. Torgue's behaviour is data: the gyrojet firing mode (Rocket type), its projectile definition and an explosive
damage type.

## 4. Status effects

### 4.1 The roll (`StatusEffectsComponent.RollChanceForStatusEffect`, native)

Called from the pawn's damage handlers (`TookDamageFromEnemy`, `TookDamageFromFriendly`, `TakeRadiusDamage`) with the
damage type's `StatusEffect`. It does nothing unless the component may apply effects (`bCanApplyStatusEffects` and two
other state bits), there is an instigator and an owner target, and the owner target accepts effects. Then it computes
the chance (4.2); if the chance is above 0.0001 and `FRand() <= chance` (a debug "guaranteed" switch bypasses both),
the effect is applied (4.3).

### 4.2 The chance (native)

The chance is a product, in percent:

`chance% = BaseChance(surface) * TargetStatusEffectChanceModifier * Friendly * CauserBase * CauserChance *
CauserFireInterval * LevelScale * ExtraScale * InstigatedStatusEffectStatusChanceModifier *
Instigated<Element>StatusChanceModifier`

then `chance = clamp(chance% * 0.01, 0, 1)`. The factors as read:

* **BaseChance(surface)**: the `StatusEffectDefinition.DamageSurfaceChanceModifiers` entry whose `SurfaceType` matches
  the hit's damage surface (`EDamageSurfaceType`: Generic, Flesh, Armor, Shield; looked up from the hit, else the
  target's default surface), its `BaseChance` evaluated on the target; a region-specific override from the target's hit
  region replaces it when present. No matching entry gives 0, so no effect.
* **TargetStatusEffectChanceModifier**: the definition's initialization data of that name, evaluated on the target.
* **Friendly**: `GlobalsDefinition.FriendlyStatusEffectChanceModifier` when the instigator and target are on the same
  side (allegiance test partly read).
* **Causer factors**, through the damage causer's `IDamageCauser` functions (slot order read from the exec natives):
  `GetFireIntervalChanceModifier()`, `GetStatusEffectChanceModifier(def)` and `GetStatusEffectBaseChanceModifier(def)`;
  each is 1 without a causer. For a **weapon** they return its `StatusEffectChanceModifier` and
  `BaseStatusEffectChanceModifier` attribute values and the fire-interval factor below; for a **projectile** they
  return `OverrideStatusEffectChance` / `OverrideStatusEffectBaseChance` when not negative, else its
  `StatusEffectChanceModifier` / `BaseStatusEffectChanceModifier`, and a fire-interval factor of exactly 1.
* **Fire-interval factor (weapon)**: `r = FireInterval / WeaponTypeDefinition.FireRate` (current interval over the
  type's base interval); if the type's `FireRate` is 0 or `r == 1`, 1; else `r ^ ShortFireIntervalModPower` when `r < 1`
  (the gun fires faster than its base), `r ^ LongFireIntervalModPower` otherwise. Both powers are 0 in the class
  defaults (factor 1); per-type values were not decoded.
* **LevelScale**: `GlobalsDefinition.StatusEffectChanceScales`, entries `{LevelDifference, HigherLevelChanceScale,
  LowerLevelChanceScale}`: with `n = min(|level A - level B|, entry count)`, entry `n - 1`; one level above the other
  takes `HigherLevelChanceScale`, below takes `LowerLevelChanceScale`, equal levels 1. A balance-modifier path
  (`BalanceModifierDefinition.GetStatusEffectChanceBasedOnExpLevelDifferences`, game-mode dependent) can supply the
  table instead. Which side is "A" (instigator or target) is **not settled**; the table's values were not decoded.
* **ExtraScale**: a caller argument (default 1). `WillowPawn.TakeRadiusDamage` passes, unless the definition has
  `bNoStatusEffectChanceFalloff`, `1 - clamp(|target - origin| - cylinder radius, 0, R) / R`.
* **Instigator factors**: the instigating controller's `InstigatedStatusEffectStatusChanceModifier` and, by
  `StatusEffectType`, `InstigatedIncendiaryStatusChanceModifier` (Ignite), `InstigatedCorrosiveStatusChanceModifier`,
  `InstigatedShockStatusChanceModifier` or `InstigatedAmpStatusChanceModifier` (skills and relics feed these).

**Consistency with the card.** The card prints the weapon's combined chance remapped from [0, 5] to [0, 100], slope 20
(NATIVE_WEAPON_RULES section 2), and the fire, shock and corrosive definitions' Generic `BaseChance` is 20. That fits
`BaseChance * StatusEffectChanceModifier * BaseStatusEffectChanceModifier` with the other factors at 1; the card's
inputs are **not read**.

### 4.3 Duration, damage over time, stacking

`StatusEffectDefinition` fields: `BaseDuration`, `TargetStatusEffectDurationModifier`, `TargetStatusEffectDamageModifier`,
`bDoesDamageOverTime`, `bIsInfiniteDuration`, `bCanSpread`/`BaseSpreadTimeInterval`/`BaseSpreadDistanceFromSource`/
`BaseSpreadCap`, `bIsRegionSpecific`, `OnApplication`/`OnDurationBegin`/`OnDurationEnd`/`OnSpreadToTarget` behaviors.
The target's component carries per-element chance and duration resistance attributes (`IgniteChanceResistanceModifier`,
`IgniteDurationResistanceModifier`, and the Shock, Corrosive, Slow and Amp equivalents) and the lists
`ActiveStatusEffects` and `OngoingEffects`. The damage comes from the causer's `GetStatusEffectBaseDamage` (a weapon's
`StatusEffectDamage`, a projectile's `OverrideStatusEffectDamage` or `StatusEffectDamage`). The application, the tick
interval, the damage per tick and how a second application of the same effect combines with an active one are **not
read** (the apply function was decompiled to `local/` but not worked through).

## 5. Brand mechanics

* **Jakobs ricochet.** `AdditionalRicochets` (an int attribute on the weapon) is added to the firing mode's
  `NumRicochets` when the light bullet is created (2.2). Separately, `ShouldBulletRicochetTowardsEnemy`: when the
  instigator is a player, a skill-side chance (the pawn's `BulletRicochetTowardsEnemyChance`-family attribute; the
  getter was not opened) is rolled with `rand() / 32767 <= chance`; a success guarantees at least one ricochet. The
  ricochet's direction and damage (`GetReflectionDestination`, `GetReflectionInaccuracyAngle`, `RicochetResponse`) are
  **not worked through**.
* **Tediore throw.** The thrown gun is a projectile spawned by the Tediore types' behavior sequences (census in
  NATIVE_WEAPON_RULES section 11). `Behavior_SpawnProjectile.SpawnChildProjectiles` (script) has a branch keyed on the
  outer type names (`WeaponType_Tediore_Pistol`, `WT_SMG_Tediore`, ...) that applies `ChildProjectileBaseValues` to the
  children; the payload enum has `PPT_TedioreReload`. The damage formula (ammo left in the clip times damage, as
  commonly described) was **not read**; `IncrementPlayerTedioreReloadDamageStat` only counts the stat.

## 6. Impact handling of an instant hit (`ProcessInstantHitBullet`, native, outline)

Inputs: fire mode index, impact, damage, firing mode, overcharge. If the hit actor delegates damage (a flag on the
actor and a proxy interface) the proxy is used. For an actor that takes the direct path, damage is applied with
`InstantHitDamageTypes[fire mode]` and momentum `InstantHitMomentum * hit direction`. With a firing mode:

1. Surface type of the hit (`GetDamageSurfaceTypeFromImpact`).
2. `GetImpactResponseIndex`: the first `ImpactResponses` entry whose `DamageSurfaceType` equals it (-1 when none).
3. If that entry has `bDisableRegularBulletDamage`, no damage; else the damage is applied with the damage type and,
   unless `bSuppressImpactEffects`, `InstantHitImpactDefinitions[fire mode]` as the impact definition.
4. `RunBehaviorsForImpact` runs the entry's `Behaviors` and `OnAnyImpact` (decompiled, not worked through).

The visual impact effect (`PlayImpactEffects`, `WillowImpactDefinition`) per surface is **not read**.

## UNVERIFIED and how to confirm in game

All hooks below are read-only observations in the live SDK session (docs/TOOLING.md); none was run for this note.

| Rule | Live observation |
|---|---|
| Fire type routing | Hook `WillowWeapon.InstantFire`; log `GetFiringModeDefinition().FireType` and whether `ProjectileDefinitionFire` / `ProcessInstantHitBullet` / `WillowLightProjectileManager.AddProj` follow, for one gun per manufacturer. |
| Pellets and pattern | Hook `FiringModeDefinition.GetFiringPatternAdjustments` on a shotgun; log the aim in and out, `NextFiringPatternIndex` before and after, per pellet; expect one call per pellet and the index stepping by 1. |
| Spread disc | Hook `WillowWeapon.AddSpread`; log `Spread`, base aim and returned aim for ~1,000 shots of one gun; expect `|yaw offset| <= A/2` and `(2 yaw/A)^2 + (2 pitch/A)^2 <= 1`, yaw histogram flat. |
| Spin-up interval | On a Vladof spin-barrel gun, poll `BarrelSpinUpPercent` and `GetFireInterval(0)` each tick while firing and after release; compare with the linear formula and the barrel's `StartingSpinUpFireIntervalMultiplier`. |
| Burst interval | On a Dahl gun zoomed, log `GetBurstInterval()`, `GetFireInterval(0)`, `FireIntervalBaseValue`, `BurstInterval`. |
| Kick | Hook `WillowPlayerController.AddWeaponKick`; log the kick argument, FOV, `DefaultFOV`, the type's six kick fields and the controller's kick pitch/yaw before and after. |
| Per-pellet damage | Call `GetMultiProjectileDamage()` on a shotgun with a flat damage modifier (a relic or skill PostAdd); compare with the formula. |
| Projectile init | Hook `WillowProjectile.InitializeFromDefinition` (post); log `MaxSpeed`, `Damage`, `DamageRadius`, `FuseTime`, `LifeSpan`, `CustomGravityScaling` for a rocket and a grenade. |
| Ricochet budget | Hook `WillowLightProjectileManager.AddProj` on a Jakobs gun; read the new entry in `MyProjs` (its ricochet count) against `AdditionalRicochets` and the firing mode's `NumRicochets`. |
| Radius falloff | Hook `WillowDamageTypeDefinition.CalcRadiusDamageScale` and `WillowPawn.TakeRadiusDamage`; log `Dist`, `DamageRadius`, the returned scale and the damage passed to `TakeDamage` for rockets at several distances. |
| Status chance | Hook `StatusEffectsComponent.RollChanceForStatusEffect`; log the arguments and whether an effect starts; plus each factor's source attribute (weapon `StatusEffectChanceModifier`, `BaseStatusEffectChanceModifier`, controller instigated modifiers, levels) to reproduce the chance; dump `GlobalsDefinition.StatusEffectChanceScales` and `FriendlyStatusEffectChanceModifier`. |
| Fire-interval chance factor | Call `GetFireIntervalChanceModifier()` on a weapon before and after a fire-rate buff; log `FireInterval`, type `FireRate`, `ShortFireIntervalModPower`, `LongFireIntervalModPower`. |
| DOT | Hook the status effect's damage events (`StatusEffectsComponent` ticks, `TakeDamage` with a status damage type) on one burning enemy; log time stamps and amounts to get the tick interval and damage per tick; apply twice to see stacking. |
| Impact responses | Hook `WillowWeapon.ProcessInstantHitBullet` and `FiringModeDefinition.RunBehaviorsForImpact` on a gun with `ImpactResponses`; log the surface type and whether damage was applied. |

## What was not read

The light-projectile manager's flight, hit and ricochet tick; projectile physics (gravity, terminal velocity, wave
motion, homing); the accuracy pool's regeneration and how it becomes the aim cone; how the pending kick is applied to
the view; the stock `Actor.TakeRadiusDamage` in Engine script and the damage-over-time manager; the status-effect apply
function, tick interval, damage per tick, duration evaluation and stacking; the status-chance table values and per-type
fire-interval powers (data, not decoded); the Tediore throw damage; impact effect selection per surface; the beam path.
Everything above is `UNVERIFIED` until a live observation confirms it.
