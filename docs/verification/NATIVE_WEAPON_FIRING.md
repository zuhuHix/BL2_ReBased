# Native weapon firing: trigger to shot, ammo, accuracy, recoil, reload, shot-side status chance (2026-10-05)

AI-assisted (Claude), analyst lane G13. Read from the game executable with Ghidra (tools/ghidra/) and from the installed
script (`research/script_disasm.py`), written in our own words; no decompiler output, pseudo-code or listing structure is
reproduced here. **Every rule is UNVERIFIED in the running game** unless a line says how it was confirmed. Data values
quoted were decoded from the installed packages with `ow-package --properties` (class defaults and the Maliwan pistol
type); field names were matched to the native code with `tools/ghidra/class_layout.py` (every field used below landed on
a field of the expected type). Raw output is in the lane's Ghidra project out folder and under ignored `local/p2/G13/`.

Scope: what happens when Maya fires one of the slice guns (the Fire mission's lent Maliwan pistol
`GD_Weap_Pistol.A_Weapons_Elemental.Pistol_Maliwan_2_Fire`, and the other pistol, SMG and shotgun types). Card stats and
part generation are in [NATIVE_WEAPON_RULES.md](NATIVE_WEAPON_RULES.md) (not repeated); muzzle flash, glow and first-person
placement are in [NATIVE_WEAPON_VISUALS.md](NATIVE_WEAPON_VISUALS.md); the auto-aim target choice is in
[NATIVE_PHASELOCK_TARGETING.md](NATIVE_PHASELOCK_TARGETING.md); the resource pool tick is in
[NATIVE_SKILLS.md](NATIVE_SKILLS.md) section 5.2. Victim-side damage, shields, death and the status-effect roll are lane
G11's note (NATIVE_DAMAGE_DEATH.md, may still be in progress); inventory, equip and put-down are lane G12's.

## Summary

| Native or script routine | Kind | Slice relevance | Confidence | Status |
|---|---|---|---|---|
| `WillowWeapon` states `Active`, `WeaponFiring`, `WeaponReloading` (`BeginFire`, `FireAmmunition`, `SharedFireAmmunition`, `ShouldRefire`, `AbortRefireShared`, `HandleFinishedFiring`) | script | Fire mission: every shot | high (read) | UNVERIFIED |
| `Weapon.GetFireInterval` (the `WillowWeapon` override) | native, virtual | shot cadence | high | UNVERIFIED |
| `WillowWeapon.GetBurstInterval` | native, virtual | burst pistols/SMGs | high | UNVERIFIED |
| `WillowWeapon.GetFiringModeDefinition` | native, virtual | which fire type a shot uses | high | UNVERIFIED |
| `WillowWeapon.GetAmmoCount`, `GetMaxAmmo`, `ShouldAutoReloadWhileFiring` | native | ammo, reload | high | UNVERIFIED |
| `WillowWeapon.AddAmmo`, `ConsumeAmmo`, `HasAmmo`, `HasActiveAmmo`, `HasSpareAmmo`, `RefillClip` | script | ammo | high (read) | UNVERIFIED |
| `FiringModeDefinition.GetFiringPatternAdjustments`, `NotifyFiringPatternWhenShotComplete` | native | shotgun pellets | medium | UNVERIFIED |
| `WillowWeapon.AddSpread` (Willow override) | script | per-shot cone | high (read) | UNVERIFIED |
| `WillowPlayerController.GetAdjustedAimFor` (accuracy cone) | script | per-shot cone | high (read) | UNVERIFIED |
| `WillowWeapon.GetZoomEffect` | native | zoom removes the pool cone | high | UNVERIFIED |
| `Controller.AddAccuracyImpulse`, `ResourcePool.AddCurrentValueImpulse`, `HasIdleDelayPassed`, `GetTotalRegenRate` | script/native | accuracy bloom and recovery | medium-high | UNVERIFIED |
| `WillowPlayerController.AddWeaponKick`, `ApplyWanderingAndKick` | native | view kick and recovery | medium | UNVERIFIED |
| `WillowWeapon.GetCurrentRecoilAnimScale`, `GetCurrentZoomedRecoilAnimScale` | native, virtual | recoil animation size only | medium | UNVERIFIED |
| `WillowLightProjectileManager.AddProj` and its per-tick update | native | bullet travel and hit timing | medium (update read in outline) | UNVERIFIED |
| `WillowWeapon.ProcessInstantHitBullet` | native | bullet arrival to `TakeDamage` | medium | UNVERIFIED |
| `Weapon.GetTraceRange`, `CalcWeaponFire` (stock) | native/script | hit-scan range | high | UNVERIFIED |
| `WillowWeapon.GetStatusEffectBaseDamage`, `GetStatusEffectChanceModifier`, `GetStatusEffectBaseChanceModifier`, `GetFireIntervalChanceModifier` | native (interface slots) | Maliwan fire pistol: chance and DoT inputs | high | UNVERIFIED |
| `WillowWeapon.GetMultiProjectileDamage` | native, virtual | shotgun pellet damage | medium | UNVERIFIED |
| `Actor.SetTimer` with rate 0 | native | reload interruption | low-medium | UNVERIFIED |
| `WillowWeapon.OnAbortReload` | native | Phaselock aborts a reload | low (body not read) | UNVERIFIED |

## 1. Fire states and the trigger (script, read)

`WillowWeapon` runs one state at a time: `Inactive` (not held), `WeaponEquipping`, `Active` (ready), `WeaponFiring`,
`WeaponReloading`, `WeaponBusy` (a melee or grenade action), `WeaponPuttingDown`. Fire mode 0 is the trigger; mode 1 is
aim/zoom (script keeps it out of the firing state: `SendToFiringState` ignores mode 1, `BeginFire` for mode 1 only starts or
stops zoom, and with `bHoldToZoom` the class default is hold-to-zoom).

**Trigger gate** (`WillowPlayerController.StartFire`): fire is refused while weapons are restricted
(`bWeaponsRestricted`), while the held weapon's `CanPerformAction` is false (true in every state except `Inactive` and
`WeaponPuttingDown`), and, for mode 1 only, while a shared weapon action (melee, grenade) is running. A branch for
vehicle transitions delays the press by 0.1 s through a `PostFireDelay` timer (not traced further). The press reaches the
weapon through `Pawn.StartFire` -> `Weapon.StartFire` -> `BeginFire`, which marks the mode as pending. (The same gate is
the one the Phaselock constraint reads, see NATIVE_PHASELOCK_TARGETING.md section 4.)

**`Active.BeginFire(0)`**, in this priority order:
1. Overheat weapons with an empty clip: dry-fire click only.
2. No ammo at all (`HasAmmo` false: the pool total is 0 and the shot cost is above 0, and not infinite ammo): dry-fire click only.
3. Clip not empty (`ReloadCnt > 0`), the current time is later than `LastAutomaticBurstTime`, the weapon is not blocked
   after a busy action and no burst delay is running: the base behaviour marks the mode pending and, since ammo exists,
   enters `WeaponFiring`.
4. A burst delay is running: only the pending mark is set (the delay timer re-begins fire).
5. Clip empty with spare ammo, nothing else true: **nothing happens on the press** (no click, no reload start); the
   reload comes from the paths in section 8.

**Entering `WeaponFiring`.** `BeginState` sets `bIsFiringWeapon`, clears the refire timer, **fires the first shot
immediately** (`FireAmmunition`), and then, if the state is still `WeaponFiring`, arms the refire timer.

**Refire.** The refire timer is armed as a repeating timer of `GetFireInterval(mode)` seconds (the base weapon routine),
but the Willow tick re-arms it after every shot, so the interval is re-evaluated each shot (this is what lets barrel
spin-up shorten it). On each timer tick: first `AbortRefireShared`; if it did not abort and `ShouldRefire` holds, fire
and re-arm; otherwise `HandleFinishedFiring`.
- `AbortRefireShared`: if the clip is empty (`HasActiveAmmo` false) **and** there is spare ammo and the weapon is not an
  overheat weapon (`ShouldAutoReloadWhileFiring`, native, effectively "not overheat"): start a reload. If the clip is empty
  with no spare ammo: `WeaponEmpty` (back to `Active`). If a put-down was requested: put the weapon down. Each of these
  stops the tick.
- `ShouldRefire`: false when there is no ammo (`HasAmmo`); with `AutomaticBurstCount` 0 it is "trigger still held"
  (pending fire); with a burst count it is true while `CurrentBurstShotCount < AutomaticBurstCount`; when the count is
  reached it records `LastAutomaticBurstTime = now` and returns false.
- `HandleFinishedFiring`: with `AutomaticBurstCount > 1` and the state still firing, read `GetBurstInterval`; if it is
  above 0, clear the pending fire, set `bBurstDelayActive` and start a one-shot `BurstDelayComplete` timer of that length;
  then return to `Active`. `Active.BurstDelayComplete` clears the flag and, if fire is pending again, the weapon has
  ammo and is not being put down, begins fire again. How a still-held trigger sets the pending flag again after the
  burst clears it was not found in script (see Open).
- `WeaponFiring.EndState`: for mode 0 it starts a short `RefireDelayAfterBusy` timer (one fire interval), resets the
  automatic-firing time and, when no trigger is pending and the weapon is not a burst weapon, plays the stopped-firing
  sound and the barrel-spin stop; if the weapon has no ammo at all it plays the dry-fire sound; and it resets
  `CurrentBurstShotCount` to 0.

**One shot, `FireAmmunition` (script, in order):**
1. (Beam weapons only: a running continuous beam is ended first.) `CurrentBurstShotCount` is incremented.
2. `SharedFireAmmunition` (section 2).
3. For a player controller: accuracy impulse and view kick (sections 4 and 5).
4. Extra shot: if the weapon's `ExtraShotChance` is above 0 and a random float in [0,1) is at or below it, a one-shot
   `ExtraFireAmmunition` timer is set for `min(GetFireInterval, ExtraShotDelay)` seconds. When it fires (and the shared
   abort check passes) it runs `SharedFireAmmunition` again: a full extra shot that spends ammo and fires, but adds no
   accuracy impulse and no kick. (This is the two-fang style skill path; `ExtraShotDelay` is data-driven.)
5. If the pawn has weapon-fire skill events enabled (`bEnableWeaponFireSkillEvent` above 0) the skill manager is
   notified with the weapon-fired event.

## 2. Ammo (script, read) and the native count functions

State on the weapon: `ReloadCnt` (rounds in the clip), `ClipSize` and `ShotCost` (integer attributes, so they carry the
modifier stacks of NATIVE_WEAPON_RULES section 1), `AmmoPool` (a resource-pool reference to the owner's pool for the
type's `AmmoResource`, set by `AssociateAmmoPool`), `StoredAmmo` (used only when no pool exists), `AmmoNotInClip`
(server-side bookkeeping, pool count minus clip).

- **`GetAmmoCount` (native): the total ammo available to the weapon, clip included.** If the owner has infinite ammo
  (`InventoryManager.bInfiniteAmmo`) it is `ClipSize`; else if the pool is valid it is the pool's current value truncated
  to an integer; else it is `StoredAmmo` (or `OverheatAmmo` for an overheat weapon).
- **`GetMaxAmmo` (native):** the pool's maximum value if the pool is valid, else the type's `MaxStoredAmmo` (the clip
  size for overheat weapons). The class default `MaxStoredAmmo` is 120 and `StartingAmmoCount` 30; the real caps come from
  the owner's pool.
- **Per shot (`SharedFireAmmunition`, in this order):**
  1. `ReloadCnt = max(min(ReloadCnt, min(ClipSize, GetAmmoCount())) - ShotCost, 0)`.
  2. `ConsumeAmmo` calls `AddAmmo(-ShotCost)` unless the owner has infinite ammo. `AddAmmo` adds the integer delta as a
     pool impulse (so the pool is clamped to its own minimum and maximum), or to `OverheatAmmo` (clamped to 0..ClipSize),
     or to `StoredAmmo` (clamped to 0..`MaxStoredAmmo`).
  3. The shot itself (`InstantFire`, section 3).
  4. The firing sound, the AI mind is told a weapon was fired, and `LastFireTime = now`.
  5. Overheat weapons: `ReloadCnt` follows `OverheatAmmo` and regeneration is delayed (`FireRegenDelay` when the value is
     0, else `OverheatRegenDelay`).
  6. On the server, `AmmoNotInClip = GetAmmoCount() - ReloadCnt`.
  So the pool is charged per shot (it is the real ammo count), and the clip counter is a separate display/limit that is
  refilled from the pool at reload. A shot cost above 1 lowers both by the cost; a clip below the cost is clamped to 0.
- **`HasAmmo(mode, amount)`:** true if infinite ammo or overheat; with no `amount`, `GetAmmoCount() > 0` or `ShotCost <= 0`;
  with an `amount`, `GetAmmoCount() >= amount`. **`HasActiveAmmo`:** `ReloadCnt > 0` or `ShotCost <= 0`.
  **`HasSpareAmmo`:** `AmmoNotInClip > 0`, or `ShotCost > 0` fails, or infinite ammo, or overheat (as read: three
  alternatives joined by or; the first two are "there are rounds outside the clip" and "shots are free").
  **`HasAnyAmmo`:** `GetAmmoCount() > 0` or `ShotCost > 0` fails or infinite ammo or overheat.
- **`RefillClip`** (also used on equip/respawn paths): `ReloadCnt = min(ClipSize, GetAmmoCount())` and the server
  recomputes `AmmoNotInClip`.
- **Empty click:** `PlayDryFireSound` picks a sound from the type's `DryFireSounds` and, for a human player pawn, fires the
  out-of-ammo dialog event and a player event (15); with the argument "trying to fire" true those two run only on a press.
- **Slice values.** Maliwan pistol type: `ClipSize` 6, ammo resource `D_Resources.AmmoResources.Ammo_Repeater_Pistol`,
  a PreAdd of +1 on `WeaponShotCost` in the type's weapon attribute effects (so the cost is 2 rounds per shot before any
  other modifier), plus the part and grade stacks; read the final cost from the card note's evaluator, not from here.

## 3. Shot types, trace, range and bullet travel

**Which fire type.** `GetFiringModeDefinition` (native): the barrel part's `CustomFiringModeDefinition` if it has one,
else the elemental part's, else the weapon type's `DefaultFiringModeDefinition`. `FiringModeDefinition.FireType` is
`Bullet` (0, the class default, used by the pistol, SMG, shotgun and assault-rifle defaults), `Beam` (1), `Rocket` (2) or
`HitScan` (3). `InstantFire` does nothing at all when no definition is found (ammo is still spent), runs
`FiringModeDefinitionFire` for HitScan, Bullet and Beam, and `ProjectileDefinitionFire` for Rocket.

**Start, aim and range.** The start point is the pawn's weapon start trace location (the eye/view point). The aim is
`GetAdjustedAim` = the controller's adjusted aim (section 4). The traced range is the weapon's `WeaponRange` attribute (type
`Range`, class default 16,384 uu). Both `InstantFireStartTrace`/`InstantFireEndTrace` and the Willow shot routine form the
end point as start plus aim direction times that range.

**`FiringModeDefinitionFire` (Bullet, HitScan, Beam), in order:** the fired-stat counter and the muzzle-flash counter are
advanced; then for each of `ProjectilesPerShot` pellets:
1. `FiringModeDefinition.GetFiringPatternAdjustments` (native, below) yields this pellet's direction offset and optional
   wave-motion data from the definition's pattern list.
2. Unless the definition sets `bSuppressWeaponSpread`, the weapon's own random cone `AddSpread` (section 4) is applied on
   top of the pattern direction.
3. A trace of length `WeaponRange` along that direction decides the impact point (`CalcWeaponFire`, stock: it traces with
   the engine's weapon trace flags, records hit actor, location, normal and ray in an impact list, and keeps tracing past
   actors the stock helper says shots pass through and through portal teleporters). It runs for the local human-controlled
   player and for HitScan; for other shooters of a Bullet definition the end point is simply the far end of the line.
4. **HitScan:** every impact in the list goes straight to `ProcessInstantHitBullet` (below), with the weapon's
   `InstantHitDamage`.
5. **Bullet (authority only):** a light projectile is added to the shooter's `WillowLightProjectileManager` from the start
   point to the traced hit point (below). The muzzle-flash location is set to the hit point.
6. **Beam:** a weapon beam is added to the manager (beam weapons are not in the slice; not read further).
After the pellets, if the weapon is not a burst weapon or the burst is complete, the definition is told the shot is
complete (`NotifyFiringPatternWhenShotComplete`: when the definition has `bResetPatternAfterEachShot` the weapon's next
pattern index is reset to 0).

**Firing patterns (`GetFiringPatternAdjustments`, native).** A definition can carry a list of pattern lines (data
`FiringPatternLines`, 0x44 bytes each; the shotgun default has `bFireRandomlyFromPattern`). The routine picks the line:
random when `bFireRandomlyFromPattern`, else the weapon's `NextFiringPatternIndex` (wrapped to the list length); the line
is either a fixed rotator offset or a random point in a rectangle between two offsets (a per-line flag). If the definition has
`bScalePatternByWeaponSpread` and a positive `BasePatternSpread`, the offset is scaled by `max(WeaponSpread /
BasePatternSpread, MinPatternScale)`. The offset is composed onto the aim rotation; the line's wave-motion fields are copied
out; and for sequential patterns the index advances by 1 modulo the count. With an empty list the aim is returned
unchanged. (The shotgun definitions in the data decode with an empty or unreadable pattern list in our reader; the
pattern contents are not known; do not invent them.)

**Light projectile (`WillowLightProjectileManager.AddProj`, native; per-tick update read in outline only).** Bullets are
not instant: the manager keeps one entry per bullet with the start point, the hit point, a velocity of
`FiringModeDefinition.Speed` (uu/s) times the weapon's `ProjectileSpeedMultiplier` along the line, the fire time, the
ricochet budget (`AdditionalRicochets` plus the definition's `NumRicochets`, at least 1 when the weapon says to ricochet
toward enemies), the overcharged flag and a stored damage value. The stored damage is the weapon's `InstantHitDamage`,
or, when the weapon fires several pellets and the shooter passes a pawn-kind check that was not identified, the per-pellet
value of `GetMultiProjectileDamage` (below). A tracer particle from the definition is spawned. On each manager tick the
authoritative side sweeps each bullet's last segment with a trace of the definition's `TraceExtent` and passes every
impact to `ProcessInstantHitBullet` with the stored damage; the bullet is retired when it is done or past its life.
**Consequence for the host:** a Bullet shot is aimed and range-limited at fire time, but damage lands after
`distance / Speed` seconds and a moving target can step out of the swept path. Slice data: default pistol bullet speed
21,500 uu/s, `Bullet_Pistol_Maliwan` (the Fire-mission pistol type default) 12,000 uu/s, default shotgun 20,000.

**`ProcessInstantHitBullet` (native; read as an outline).** Inputs: fire mode, the impact record, the damage amount, the
firing-mode definition, an overcharge flag. It is the single place where a bullet or hit-scan impact becomes damage:
the hit actor receives a `TakeDamage` dispatch with the damage amount, the shooter's controller as instigator, the hit
location, a momentum vector equal to the ray direction times the weapon's `InstantHitMomentum` (Maliwan pistol type 8,
class default 10; then multiplied by 100 once at weapon setup), the damage-type class chosen from the weapon's
`InstantHitDamageTypes` for the fire mode (`WillowDmgSource_Pistol` for the pistol type) and the hit information; the weapon
is the damage causer. Some hit actors forward the hit to an owning object first (a flag on the actor). The impact side
effects pick an entry of the definition's `ImpactResponses` by the surface type of the hit (a lookup of the surface byte
in the response list; `-1` when none matches) and run the definition's `OnAnyImpact` behaviours plus the matched
response's behaviours, and play the impact effects. Everything after `TakeDamage` (health, shields, crit regions,
elemental damage types, status application, death) is lane G11's. Two details were not separated: how the two internal
branches (actor kinds) differ, and where critical-hit and damage-over-distance scaling is applied (not seen on the shot
side: the damage amount is passed through unchanged).

**`GetMultiProjectileDamage` (native, virtual).** Walks the weapon's damage modifier stack with the same arithmetic as
NATIVE_WEAPON_RULES section 1 (scale entries above 0 add to the "up" sum, scale entries at or below 0 to the "down" sum,
PreAdd and PostAdd summed separately; scale factor `S = (1 + up) / (1 - down)`) but spreads the additive parts over the
pellets: `per-pellet = base * S + (PreAdd * S + PostAdd) / ProjectilesPerShot`, with `base` the damage base value. Used
only for multi-pellet shots (see AddProj above).

**Ricochet.** `ShouldBulletRicochetTowardsEnemy`: for a shooter with the right pawn kind, a roll against a chance read from
a skill/attribute path (not identified) decides whether a ricochet budget of at least 1 is granted. Not slice relevant.

## 4. Accuracy and spread (script and data, read; one native checked)

There are two independent cones per shot for a human shooter, applied in this order.

**A. The player accuracy pool cone (`WillowPlayerController.GetAdjustedAimFor`).** For the local player the
controller owns an `AccuracyPool` (and an `OffHandAccuracyPool`, used for the left weapon when dual wielding), a resource
pool of the same kind as health. Its current value is the full cone angle in degrees. For each aim request:
1. Base aim = the pawn's base aim rotation (first person) or the third-person aim rotation.
2. `acc = pool current value` converted to rotator units (`acc * 65536 / 360`).
3. Draw `r1 = 0.5 - random()` and `r2 = 0.5 - random()`. Yaw offset = `r1 * acc`; pitch offset = `r2 *
   sqrt(1 - (2 * r1)^2) * acc`. So the offset lies inside an ellipse whose half-width is `acc / 2` degrees.
4. The offset is multiplied by `1 - ZoomEffect` (`GetZoomEffect`, native): **0 when not zoomed, 1 when fully zoomed (the
   cone vanishes), a linear ramp over `ZoomTime` while zooming in, and `1 - t` while zooming out** (`t = (now -
   ZoomStartTime) / ZoomTime`, clamped to 0..1; the class default `ZoomTime` is 0.2 s).
5. The offset rotates the base aim direction to give the aim passed on.
No other player state (movement, crouch, sprint, jump) appears in the cone arithmetic; a scan of every `Startup.upk` object
for effects on the pool's value, min, max or regeneration found only weapon types, weapon parts, artifacts and class mods
(107 objects): **no movement, crouch or stance modifier exists in the stock data** for the slice character's pool (it is
possible that skills in other packages touch it; the Startup scan covered all Maya skill trees).

**The pool's numbers (`D_Resourcepools.PlayerPools.AccuracyPool`, used by `CharClass_Siren`).** Base minimum 2, base maximum
12, `StartWithMaxValue` true (the first shot after spawn has the widest cone), `BaseOnIdleRegenerationRate` -8 per second and
`BaseOnIdleRegenerationDelay` 0.2 s. The weapon's external attribute effects (slots with `bExternalSlot`, applied to the
owner when the weapon becomes active by `ApplyAllExternalAttributeEffects` in `WeaponEquipping.BeginState`) scale the
pool's min, max and idle regeneration; for the Maliwan pistol type the slots are `AccuracyMin` and `AccuracyMax` at -0.05 per
grade and `AccuracyRegen` at +0.05 per grade (Scale modifiers). How the grades are summed is NATIVE_WEAPON_RULES section 4.

**Bloom and recovery.** After each shot (script, section 1 step 3) the controller adds `PerShotAccuracyImpulse` (Maliwan pistol
type 2.5 degrees, scaled by the `WeaponAccuracyImpulse` slot at -0.05 per grade) to the pool; for a burst weapon the
impulse of the shots before the last of a burst is multiplied by `BurstShotAccuracyImpulseScale`. `AddAccuracyImpulse` runs
on the authoritative side only, for the main-hand or off-hand pool by the weapon's `bOffHand`. The pool update is the
generic one of NATIVE_SKILLS.md section 5.2: the impulse is added to the current value plus the stored remainder and the
sum is clamped to `[min, max]`, and the idle timer restarts; the total rate is `ActiveRegenerationRate +
OnIdleRegenerationRate + PassiveRegenerationRate` once `OnIdleRegenerationDelay` seconds have passed since the last change
(before that only the active and passive rates apply, which are 0 here). With -8 per second and a 0.2 s delay the cone
therefore holds for 0.2 s after a shot, then shrinks by 8 degrees per second down to the minimum. Example with the base
numbers and a 2.5 degree impulse and a 0.4 s fire interval: the value is back to the minimum about 0.6 s after the first shot's
impulse (0.2 s hold plus 0.3 s decay), so single shots never accumulate, while an interval shorter than about 0.5 s
accumulates until the maximum 12 is reached. `InitAccuracyFromWeapon` (equip) refills the pool to its maximum only for a pool
whose idle regeneration is positive; with the player pool's -8 it does nothing.

**B. The weapon's own cone (`WillowWeapon.AddSpread`, script, read).** If the weapon has not set `bDisableWeaponSpread`: `spread =
Weapon.Spread` (degrees; the attribute: the type's `Spread`, Maliwan pistol type 1.5, class default 0.01, then the
`WeaponSpread` slot at -0.05 per grade and part effects); for an AI shooter `spread += AIAimError` first. If `spread <= 0` the
aim is returned unchanged. Otherwise the same ellipse draw as in A with `acc = spread` is rotated onto the incoming aim (so
a shot has the pool cone first, then the spread cone on top of the pattern direction). It is applied per pellet, so the
pellets of one shotgun shot each get an independent cone. The HUD crosshair radius is derived each frame from the held
weapon's `Spread` (`1 / (tan(spread/2 deg) / tan(FOV/2))`, 0 when the spread is not positive).

**Card accuracy** is the remap of `Spread` in NATIVE_WEAPON_RULES section 2; the dynamic pool is separate and does not
appear on the card.

## 5. Recoil: view kick (native, read) and kick animation

Per shot the script (after the accuracy impulse) computes a kick amount and calls the controller's native `AddWeaponKick`:
- Normal weapons: `kick = impulse_used * 182.0444 * lerp(1, WeaponKickZoomMultiplier, ZoomEffect)`, where `impulse_used` is the
  same value just added to the accuracy pool (including the burst scale) and 182.0444 is rotator units per degree. So the
  kick is proportional to the accuracy impulse, and larger when zoomed when the multiplier is above 1 (Maliwan pistol 1.4).
- Weapons with `bAlternativeKickEnabled` (the Hyperion reverse-recoil mechanism): `kick` is the evaluated
  `AlternativeWeaponKick` initialization data instead, with no unit conversion or zoom factor.
- Then `WanderingSmoothInDuration` and its remaining time are set to the type's `ZoomWanderSmoothInTime_OnFire` (view
  wander eases in again after a shot).

**`AddWeaponKick(kick, type)` (native):** the kick is rescaled by `tan(defaultFOV/2) / tan(currentFOV/2)` (a zoomed view
gets the smaller kick), then split into a vertical and a horizontal share with two random draws: vertical sign and weight
come from a random value between `-WeaponKickDown` and `+WeaponKickUp`, horizontal from one between `-WeaponKickLeft` and
`+WeaponKickRight`; the vertical share is `MinimumVerticalPercentage + (1 - MinimumVerticalPercentage -
MinimumHorizontalPercentage) * v / (v + h)` (v, h the absolute draws; 0 when v is 0) and the horizontal share is `1 - the
vertical share` (or `MinimumHorizontalPercentage` when h is 0). The signed amounts `share * kick` are added to the
controller's `TargetVerticalKickAmt` and `TargetHorizontalKickAmt`, and `LastWeaponKickTime = now`.

**Per frame (`ApplyWanderingAndKick`, native):** `CurrentVerticalKickAmt` and `CurrentHorizontalKickAmt` move toward the
targets with a frame-rate-independent interpolation at `WeaponKickSpeed` (class default 4, Maliwan pistol type 15; the
interpolation routine's exact form was not read, a first-order approach is the working assumption), and the current amounts
are added to the view rotation (pitch and yaw) every frame. The targets decay linearly to 0: until `LastWeaponKickTime +
WeaponKickRecoveryTime` (default 0.5 s, pistol type 0.3 s) each target loses the fraction `dt / (end - now)` of its value
per frame, reaching 0 exactly at the end time. A second shot inside the window adds to what is left. The same routine
runs the fractal view wander (`FractalWander*` type fields, scaled by the current accuracy pool value and eased in by the
smooth-in time); that is presentation of the same data and is not detailed here. Whether the kick offsets only the camera or
also the aim used by the next shot's trace (the base aim rotation comes from the pawn's view rotation, which the kick
modifies) is not confirmed (Open).

**Recoil animation scale** (`GetCurrentRecoilAnimScale`, native): the fire/recoil animation's amplitude is read from the
type's `RecoilAnimScaleCurve` at the current accuracy value (minus the pool minimum when `bRemoveBaseAccuracyFromRecoilScale`
is set, which the Maliwan type does), and multiplied by `1 + (ZoomedRecoilAnimScale - 1) * ZoomEffect` when zoomed
(`GetCurrentZoomedRecoilAnimScale`; default `ZoomedRecoilAnimScale` 0.1). This scales the arm/weapon recoil animation only,
not the view kick.

## 6. Fire rate timing (native, read)

`GetFireInterval(mode)` (the Willow override of the base routine; the base returns the `FireInterval` attribute, floored at
0.01 s):
1. Lock-on weapons (`LockOnInProgress`): `max(interval, type.LockCoolDownTime)`.
2. Otherwise, if the barrel part has `bIsSpinningEnabled` and the type's `BarrelSpinMode` is `BSM_SpinUpToFullFireRate` (1,
   the Maliwan and Vladof pattern): let `slow = max(base * barrel.StartingSpinUpFireIntervalMultiplier, base)`; the
   interval is `slow - (slow - base) * BarrelSpinUpPercent`, where `BarrelSpinUpPercent` (0..1) is advanced by the native
   `TickBarrelSpinUp` over `BarrelSpinUpDuration` while firing and back down over `BarrelSpinDownDuration`. So a spinning
   barrel starts slow and reaches the card's fire rate at full spin-up.
3. Otherwise the base interval.
`GetBurstInterval` = `BurstInterval * (GetFireInterval(currentMode) / FireIntervalBaseValue)` (just `BurstInterval` when the
base value is not positive): the gap between bursts follows fire-rate bonuses. Maliwan pistol type `BurstInterval` 0.4.
The class default `FireRate` (the type's interval seed) is 0.4 s; the card's fire rate is its inverse
(NATIVE_WEAPON_RULES section 2). `GetAIFireDelay` and `GetAIBurstLength` are AI-only helpers (random delay from the type's AI
ranges; burst length = burst count times `FireInterval`).

## 7. Reload (script, read; natives checked)

**Starting.** `BeginReload` (script) goes to `WeaponReloading` only when `ReloadCnt < ClipSize`, there is spare ammo and the
weapon is not an overheat weapon. `BeginManualReload` is the same test for the reload key (and sets `bManualReload`).
Manual: the controller's `PerformReload` (server RPC from the client): if the weapon actions are allowed, an empty clip
reloads, and otherwise (key press, not the auto path) a manual reload starts for a non-full clip. Auto-start sources: (1)
the refire tick's `AbortRefireShared` (clip empty mid-fire; the empty clip is noticed on the tick after the last round, one
fire interval later, even if the trigger was released), (2) `WeaponEquipped` when a weapon is equipped with an empty clip
and spare ammo, (3) `CheckReload` after a melee or grenade action completes. Pressing fire with an empty clip does not start
a reload (section 1).

**`WeaponReloading.BeginState`:** the "reload started" skill event (33) is sent; the pawn's `bReloading` is set for the main
hand and the dialog event `Reloading` and a player event (16) are fired; the replicated reload state is 1; the first- and
third-person reload animations are played with the weapon's `ReloadTime` as duration; zoom is forced off; `TimeWeaponReload`
arms two one-shot timers: `ReloadDone` at `ReloadTime` seconds, and `AmmoReloaded` at `ReloadTime *
ReloadCompletePercent` clamped to `[0, ReloadTime]` (a result of 0 or less is replaced by `ReloadTime`). The type fields:
`ReloadTime` class default 2.1 s, `ReloadCompletePercent` default 0.75, Maliwan pistol type 0.6.

**At `AmmoReloaded` (the refill moment):** `bAmmoRefilledDuringReload` = true and `ReloadCnt = min(ClipSize, GetAmmoCount())`,
i.e. the clip is refilled from the pool at that fraction of the reload, not at the end; the pool itself is not changed (it
already holds the count, only the clip counter moves); the server recomputes `AmmoNotInClip`.
**At `ReloadDone`:** the weapon returns to `Active` (replicated state 0).
**`WeaponReloading.EndState`:** the bones are re-hidden, the pawn's `bReloading` cleared, and if the refill happened the
"reload complete" skill event (21) is sent (plus event 35 for a manual reload); the animation references are cleared.
Zoom pressed during a reload only sets a pending fire mark (`StartZoom` in the state).

**Interruptions** call `StopReloading`: the weapon sends replicated state 2, stops the reload animation, re-sets the
`ReloadDone` and `AmmoReloaded` timers with rate 0 (see the next paragraph), records `Active` as the previous state, resets
the visible-ammo state and calls the native `OnAbortReload(Instigator)` (body not read; it notifies the weapon parts'
`OnAbortReload` hooks). Callers found in script: a shared weapon action (melee or grenade, which also forces unzoom and
enters `WeaponBusy`), the start of an `ExecuteActionSkill` (Phaselock is one; this is the in-game confirmation below), and
`TryPutDown` (weapon swap, which also stores `LastReloadCnt`). `Actor.SetTimer` (native) with a rate of exactly 0 on an
existing timer sets that timer's rate to 0 and does not add a new one; I read this as cancelling the timer (UE3's "cleared
timer" form), so an interrupted reload does **not** refill the clip unless `AmmoReloaded` had already fired (after 60 percent
of the reload for the pistol). The timer-update code that would confirm cancel versus fire-next-frame was not read; the
alternative reading would give a free refill on any interruption.
**Confirmed in game on 2026-10-02** for the Phaselock case only (NATIVE_PHASELOCK_TARGETING.md section 4): casting during
a manual reload starts the cast and aborts the reload (`OnAbortReload` observed); swap put-down blocks the cast.
**Tediore throw-reload:** a pointer only; it is a behaviour sequence on the Tediore type triggered from `BeginReload`
(NATIVE_WEAPON_RULES.md section 11); not part of the slice weapons.

## 8. Shot-side status effect inputs (native, read; the roll is G11's)

The weapon exposes four values to the damage pipeline through the damage-causer interface (the same four exist on
projectiles, melee definitions and damage areas):
- `GetStatusEffectBaseDamage` = the weapon's `StatusEffectDamage` attribute (damage per second basis for the damage-over-time
  effect; the type's attribute initialization, Maliwan type uses the weapon-damage curve).
- `GetStatusEffectBaseChanceModifier` = the `BaseStatusEffectChanceModifier` attribute (Maliwan pistol type 0.6; class default 1).
- `GetStatusEffectChanceModifier` = the `StatusEffectChanceModifier` attribute (class default 1; the Maliwan type adds a PreAdd
  0.3 and a Scale on `WeaponStatusEffectDamage` of 0.2 in its weapon attribute effects, then part and grade effects; see the
  card note).
- `GetFireIntervalChanceModifier` = `ratio ^ exponent`, where `ratio = FireInterval / type.FireRate`; the exponent is
  `ShortFireIntervalModPower` when `ratio < 1` (a faster-than-base weapon) and `LongFireIntervalModPower` otherwise; the
  result is 1 when the type is missing, `FireRate` is 0 or the ratio is exactly 1. The class defaults of both powers are 0 (no
  effect) in the data; types may override them (not scanned).
How the victim side combines these (`StatusEffectsComponent.RollChanceForStatusEffect` and the damage-over-time setup in
script `ApplyStatusEffect`): as read, the chance is a product of the status definition's own chance factor, the weapon's
`BaseStatusEffectChanceModifier`, its `StatusEffectChanceModifier`, its `GetFireIntervalChanceModifier`, a per-element modifier
held on the shooter's controller (one each for incendiary, corrosive, shock, amp) and target resistance terms, divided by 100
and clamped to 0..1, then compared with a random float; the DoT per second is the weapon's base damage times the
controller's instigated modifiers (general and per element) and target modifiers. I did not tie each factor in that product
to a named field; treat it as an outline and take the authoritative reading from G11's note. Only `Behavior_AttemptStatusEffect`,
projectiles' `Behavior_CauseDamage` and the weapon itself call the four getters.

## Class defaults and slice data quoted
- Maliwan pistol type (`GD_Weap_Pistol.A_Weapons.WeaponType_Maliwan_Pistol`): `BarrelSpinMode` `BSM_SpinUpToFullFireRate`,
  `ClipSize` 6, `Spread` 1.5, `PerShotAccuracyImpulse` 2.5, `InstantHitMomentum` 8, `BurstInterval` 0.4,
  `ReloadCompletePercent` 0.6, `PutDownTime` 0.45, `WeaponKickSpeed` 15, `WeaponKickRecoveryTime` 0.3,
  `WeaponKickZoomMultiplier` 1.4, `WeaponKickDown` 0.1, `WeaponKickLeft` 1, `WeaponKickRight` 1, `ZoomedRecoilAnimScale` 0.9,
  `ZoomWanderSmoothInTime_OnFire` 0, `DefaultFiringModeDefinition` `GD_Weap_Pistol.FiringModes.Bullet_Pistol_Maliwan`
  (Bullet, speed 12,000).
- Class defaults (`Default__WeaponTypeDefinition`): `Range` 16,384; `FireRate` 0.4; `ReloadTime` 2.1; `ReloadCompletePercent` 0.75;
  `Spread` 0.01; `WeaponKickSpeed` 4; `WeaponKickRecoveryTime` 0.5; `WeaponKickUp` 1; `ZoomTime` 0.2; `ProjectilesPerShot` 1; `ShotCost` 1.
- Player accuracy pool: min 2, max 12, start at max, idle rate -8/s after 0.2 s.

## Implementer checklist (testable statements)
1. A press with weapons not restricted and the weapon in `Active` fires one shot immediately, then repeats every
   `GetFireInterval` while the trigger is held and ammo and clip last; each repeat re-reads the interval.
2. A press with an empty clip and spare ammo does nothing; a press with no ammo plays the dry click.
3. Each shot sets `ReloadCnt = max(min(ReloadCnt, min(ClipSize, ammo)) - cost, 0)` and spends `cost` from the ammo pool.
4. An empty clip with spare ammo starts a reload on the next refire tick (one interval after the last round) and on equip.
5. A reload refills the clip to `min(ClipSize, pool)` at `ReloadTime * ReloadCompletePercent` and ends at `ReloadTime`;
   interrupting it before the refill leaves the clip as it was (UNVERIFIED reading).
6. Player cone per shot: a random point in an ellipse of full width = the accuracy pool value (degrees), times `1 - ZoomEffect`;
   then per pellet an extra ellipse of full width = `Weapon.Spread` degrees (unless suppressed).
7. Pool: min 2, max 12, +`PerShotAccuracyImpulse` per shot (clamped), idle decay -8/s starting 0.2 s after the last change;
   weapon slots scale min/max/regen.
8. View kick per shot = pool impulse times 182.0444 degrees-to-units, FOV-ratio scaled, split by the kick fields; targets decay
   linearly to 0 over `WeaponKickRecoveryTime`; the current kick chases the target at `WeaponKickSpeed`.
9. Bullet shots add a bullet of speed `Speed * ProjectileSpeedMultiplier` along the fire-time line; damage is applied through
   `TakeDamage` with the damage frozen at fire time when the bullet's swept path hits; HitScan applies at once.
10. Burst weapons: `AutomaticBurstCount` shots, then `BurstInterval` (scaled by fire-interval ratio) of no fire.
11. The Phaselock cast, a melee/grenade action and a weapon swap each cancel a reload in progress.

## Open
- How a still-held trigger starts the next burst (the pending flag is cleared at the end of a burst; the re-arm path was not found).
- `CalcWeaponFire`'s "pass through" actor class and whether bullets penetrate pawns; the two branches of `ProcessInstantHitBullet`;
  whether `GetMultiProjectileDamage` or the plain damage is stored for the slice shotgun (the pawn-kind check in `AddProj`).
- The per-frame light projectile update beyond the outline (gravity or acceleration, friction, ricochet, how a bullet that
  misses ends).
- The interpolation function used for the kick; whether the kick moves the aim as well as the camera.
- The `SetTimer` rate-0 reading, `OnAbortReload`'s body, the vehicle-transition branch of `StartFire`, the exact status chance
  factor mapping, `PlayFireEffects` and `ShakeView` (see NATIVE_WEAPON_VISUALS.md section 5), and `FireShake` (the type's view
  shake struct is data; the Maliwan pistol type has a zero offset magnitude).
- Everything is UNVERIFIED in game. Cheapest confirmations: an SDK trace of the player's `AccuracyPool` current value around
  one shot and one burst (expect +2.5, a 0.2 s hold, then -8/s); `ReloadCnt` and the pool before, at 60 percent and at the end of a
  reload; a trace of bullet impact time against distance for a 12,000 uu/s pistol shot at 20 m (about 0.17 s).

## Not read yet
`TickBarrelSpinUp`, `TickMagazineSpinUp`, `GetBarrelRotationsPerSecondAtFullFireRate`, lock-on weapons, beam weapons, the
whole light projectile tick, `ProjectileDefinitionFire`'s projectile flight (`WillowProjectile` natives; damage on hit is
G11's), `PlayFiringSound` and the sound selection, `ShakeView`, `AdjustFOVAngle`/`TickZoom` zoom maths, the `UpdateFiredStats`
native consumers, `GetPhysicalFireStartLoc` (reads cached muzzle location per frame), ammo regeneration of overheat weapons.

## Corrections to earlier notes
- NATIVE_WEAPON_VISUALS.md section 5 lists the per-shot routine's sub-steps as unmatched; the script side of the shot is
  `FireAmmunition` / `SharedFireAmmunition` here, and the muzzle/glow work is a separate native effect call after the shot.
- NATIVE_WEAPON_RULES.md section 11 ("Hyperion reverse recoil is native on data"): confirmed in the script half
  (`bAlternativeKickEnabled` replaces the kick amount); the kick routine itself is described in section 5 above.
