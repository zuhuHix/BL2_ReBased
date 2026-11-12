# Bullymong targeting and melee from the running game (lane L8, 2026-10-09)

AI-assisted (Claude), real-game lane L8. One launch of the original game driven through `tools/real_game/` (level-8 Maya,
`-Character=Save0008.sav`, saves blocked from the first frame, memory-only changes, run lock, closed through its window). This record holds
counts, conclusions and formulas only: no dump, no listing, no game data. The raw rows (target-list dumps, 8 approach runs and 5 swing grids,
hook rows, per-frame cadence rows) stay under the ignored `local/realgame/L8/` (115 files, 22 MB). It answers the **Live confirmation** lines of
[NATIVE_AI_MELEE.md](NATIVE_AI_MELEE.md) (plan step S-h) and the open question of [PROJECTILE_FIRING_PATTERNS_LIVE.md](PROJECTILE_FIRING_PATTERNS_LIVE.md)
section 6 item 5. Every rule below is **OBSERVED** (with how) or **UNVERIFIED**; where the earlier note and the game disagree, the game wins and
section 8 lists the corrections for the note's owner.

## 1. Method

- **Set-up (as lanes L3/L5/L7):** `Install-Driver -BlockSavesAtStart`, run lock `local/ue_run.lock` (taken 08:39:33 when free), 60 files hashed
  before and after (saves, config, SDK settings, Sanity Saver's private copies), `Ice_P` by `WillowGameInfo.InitiateTravel`. Player damage was
  blocked by a `TakeDamage` hook on `Ice_P` (nothing else about the player was changed). Script stand-ins (memory only, all named where used):
  placing the player (`Pawn.Location`, hung with `SetPhysics(0)`), holding the attacker (physics none) for the controlled swings, calling
  `MeleeDefinition.MeleeAttack(pawn)` from script, spawning guns into the backpack.
- **Reads:** `tools/real_game/scripts/ai_melee_probe.py` (new): `am_targeting`, `am_aidef`, `am_snapshot`, `am_summary` (the pawn's
  `AIComponent` is `pawn.MyWillowMind.AIComponent`; the record is a `WillowMindTargetInfo`), per-frame `am_track`, `am_approach` (step the player
  through horizontal distances), `am_melee_grid` (controlled swings), `am_hooks` (PRE and POST on `ActionSequence.TriggerBehavior`,
  `Behavior_MeleeAttack.ApplyBehaviorToContext`, `MeleeDefinition.MeleeAttack`, `AIDefinition.OnTargetAcquired`/`OnTargetLost`,
  `WillowActionSequencePawn.ActivateEvent`, `WillowPlayerPawn.TakeDamage`).
- **Counts:** 8,838 hook rows over three recordings (the long natural fight, 7,098; the 45 s adult swing recording, 1,338, with the pawn
  of each Action; the first sweeps, 402), 2,433 per-frame component rows of one adult, 134 target-record samples across 8 approach runs and 2 sweeps,
  71 clean controlled swings (plus 149 earlier ones that served to find the method's traps and the edges), about 20 spawned guns for the damage question.
- **Traps (new, all in the script's docstring):** (1) **writing `Pawn.Location` does not move the actor's spatial-hash entry**: a melee volume
  query kept finding the player at her old place (hits at the old 30 uu, none at the new 300 uu) until `ForceUpdateComponents(False, False)` was
  called after the write; `Move` and `SetLocation` fail for the player here. (2) The attacker still turns and drifts while "held" (rotation rate
  zero, physics none): judge each swing by the geometry logged **at the call**, not by the commanded one. (3) Hung inside terrain and then
  released, the player falls out of the world (z = -158,983 was reached); the helper now restores her place first. (4) The two
  `AIDefinition` events and the node-by-node tree walk are native-to-native: their hooks were installed and never fired in 8,838 rows.
  (5) The population system despawns AI beyond roughly 10 km and spawns new ones (pawn names 44 to 77 over the run), so a long sweep must pick a
  pawn that still exists (`bDeleteMe`).

## 2. Targeting data: `GD_AI_Targeting.Targeting_ShortRange` (OBSERVED by property read)

The five bullymong `AIDefinition`s (Adult, regular, Baby, Ranged, Badass; the pawn holds a per-pawn transient copy) all name this one
definition and have `TargetSearchRadius` 4096 and `bGetTargetsFromParents` false; the adult's `ReacquireTime` is 0.5 and `ReacquireDistance` 512.

| field | value |
|---|---|
| `bOverrideSearchRadius` / `SearchRadius` | false / 0.0 (so the AI definition's 4096 would apply) |
| `CanTargetIf` / `StopTargetingIf` / `bCanTargetFriendlies` | None / None / false |
| `Knowledge` (in this order) | `WillowTI_CalcThreat`, `WillowTI_CalcCanFire`, `WillowTI_CalcExposure`, `WillowTI_CalcDistances`, **None** (a fifth entry that holds no object: the note's `WillowTI_CalcCombat` has no live object here) |
| `Prioritization` and weights | `WillowTI_PrioritizeIdle` 0.0 (`MaxAICurrentlyTargeting` 0); `WillowTI_PrioritizeAttacking` 0.0 (`DecayRate` 0.0, `FullyEngagedPct` 0.0); `TargetIteratorPrioritizeNearbyTarget` **1.0** (`MaxDistance` **4096**); `TargetIteratorPrioritizeExposedTargets` **1.0**; `TargetIteratorPrioritizePreviousTarget` **0.4**; `TI_PrioritizeBonus` **1.0** |
| iterator types | Knowledge and Idle/Attacking `TARGITER_UpdateState`; the other four `TARGITER_PriorityCalc` |

So Idle and Attacking have weight 0: for this definition their formulas do not matter, and the weight sum is **3.4** (1 + 1 + 0.4 + 1).

The adult's `NodeList` (14 nodes, per-pawn copy of the shared definition) matches the note: root `Tree` (priority; children 1, 2, 13), node 2 `Priority`
(children 3, 4, 5, 6, 12), node 6 `Random` with weights 1.0 / 0.25 / 0.25 over nodes 7 (`Melee`), 8 (`Sequence`: `Roar` then `Leap Attack`) and 11
(`Throw Rock`); the combat-perch node 3 has `TimedStart` 10 to 10 and `TimedReset` 20 to 30; node 5 `Close Melee` (an `Action_AnimAttack`, `Type`
`Attack_InPlace`) and node 7 `Melee` (`Attack_Close`) both play the animation set `AnimRandom_PrimalBeastMelee`; node 4 `Throw Ice Spike` is
`Attack_Far`; each `Action_AnimAttack` carries its pawn in `MyWillowPawn`. The `Limits` (range, line of sight, height) of the attack actions were
listed by path only: their numbers were not read (open).

## 3. The target list, per record and per frame

### 3.1 Cadence (OBSERVED, 2,433 consecutive frames of one adult)

`AIComponent.UpdateRate` is **0.2**. `LastTargetUpdateTime` equalled `LastStateUpdateTime` in **all 2,433 frames**; each changed 214 times with
value spacing min **0.1999**, mean 0.2113, max 0.2606 (the first frame at or after 0.2 s; frames were 18.6 ms), and the stored value equalled the
frame's game clock when it changed. This confirms the note's "both updates run in the same tick, at most once per `UpdateRate`".

### 3.2 Priority formula (OBSERVED, 134 samples, residual < 5e-8)

`Priority = (max(0, 1 - Distance/4096) * 1 + TargetExposureToMe * 1 + (0.4 if the record's target is the current target) + BonusPriority * 1) / 3.4`.
`Distance` is the record's **3D** distance (not `Distance2D`); `BonusPriority` was 0 in every sample. Spot values (adult, current target = player):
`Distance` 517.4, exposure 1.0 gives 0.668731 (predicted 0.668731); 771.8 and 0.5 gives 0.503402; 2807.5 and 0.0 gives 0.210168; 5106.4 and 0.0 gives
0.117647 (the previous-target term alone, 0.4/3.4); 1500 uu above the adult (`DistanceVertical` 1497, `Distance2D` 1292, `Distance` 1977.9) gives
0.563860 (predicted 0.563860). Two further facts: before a target is current the previous term is 0 (priority 0.0 at 5 km with no exposure), and
**`TargetExposureToMe` was -1.0 in one sample** and the priority then carried the -1 unclamped (-0.2163 at 3,012 uu: (0.2648 - 1)/3.4), so the exposed
iterator writes the raw exposure, including that sentinel. UNVERIFIED: what `-1.0` means (a failed visibility query is the guess).

### 3.3 What a record contains (OBSERVED)

Every `WillowMindTargetInfo` field of the adult's record of the player was read at 6 placements (four distances, then two at 1,500 uu above): `Target`
(the player pawn), `Distance`, `Distance2D`, `DistanceVertical`, `DotToTarget`, `DotFromTarget`, `LeftDotToTarget`, `DotTargetFacing`,
`TargetExposureToMe` (observed values 0.0, 0.5, 1.0, -1.0), `Exposure` (`Exposure_None` / `_Half` / `_Full`, i.e. 0 / 0.5 / 1.0 on the 0.4 and 0.9
thresholds of the note: 0.5 was `Exposure_Half`), `ThreatLevel`, `bCanSeeOrHearTarget`, `bCanFireAt`, `Priority`, `OnePriority`,
`BonusPriority`, `TotalDamagePct`, `AggroTime`, `ForgetTime`, `ExposureChangeTime`, `LastKnownLocation`. `OnePriority` read 0.0 in every sample (the field
is overwritten per iterator, as the note says; it cannot be used to verify one iterator). `TotalDamagePct` stayed 0.0 (the player did no damage).

`ThreatLevel` had two values here: `ETL_InThreatRadius` before the pawn had the player as its current target and `ETL_SeenInThreatRadius` while it had.
`bCanSeeOrHearTarget` and `Exposure` followed line of sight, not distance (see 3.4). `Distance`, `Distance2D` and `DistanceVertical` equal the pawns' geometry when both are still (3D 6,931.5 for horizontal 6,925 and vertical 300) and lag it by up
to one update (0.2 s) when they move (1,292 stored against 1,237 live in one moving sample).

### 3.4 Who is in the list and who is the current target (OBSERVED; the radius claim of the note is refuted)

- **The 4096 search radius does not bound the list.** An adult held a record of the player at horizontal distances of **4,100 to 7,000 uu** (and
  every other bullymong at 5 to 8 km at arrival), with live `Distance`, `Priority`, `Exposure` and `Distance2D` updating each 0.2 s. Neither
  `Targeting_ShortRange` (override off) nor the AI definition (4096) fits that. The only 4096 that is visible in the numbers is the nearby
  iterator's `MaxDistance` (priority 0 from the distance term beyond it).
- **In one test geometry a band at about 7.0 km bounded it, with a small hysteresis** (player hung 300 uu above the pawn on bearings 180 to 250 deg, record count
  read 0.6 to 0.8 s after each move): adult, record present at horizontal 6,925 (3D 6,931.5) and absent at 6,950 (3D 6,956.5) coming in; present at 7,053
  (3D 7,059.6) and absent at 7,080.6 (3D 7,087) going out, so added below about **6.94 km** and dropped above about **7.07 km**. Coarser runs agree: a regular
  beast, present at 7,123 and absent at 7,623; an engaged adult, present at 6,976 and absent at 7,526. Horizontal against 3D was not separated (300 uu
  height moves the 3D value by 6 uu). **But the band is not a constant of the pawn**: at 388 s and again at 411 s, with the player on the arrival ledge, six
  bullymongs held live records at 5.1 to 10.2 km (at 411 s: 7,949 and 8,763 uu for a baby and a regular beast) and the rakk drones at 15 to 17.6 km, i.e.
  beyond the band the sweeps measured later. The cause is **UNVERIFIED** (candidates: where the player is relative to the navigation mesh or an
  awareness volume, the pawn's population-relevance state, a distance on another path such as a stored-record lifetime; every pawn of the session showed
  `bDisabledDueToPopulationIrrelevance` true yet its record kept updating). What is solid: the list is **not** cut at 4096, and there is a cut near 7 km
  in the geometry tested.
- **Height is ignored for being in the list**: records existed with the player 1,497 uu above the adult (and 13.8 km above a rakk drone). That part
  of the note stands; the radius part does not.
- **Rakk drones held a record at 20 to 27 km horizontal** while all bullymongs lost theirs at about 7 km, although the rakk's targeting definition
  and `TargetSearchRadius` read the same. Whether those components were still updating was not checked: open.
- **Having a record is not being the current target.** `CurrentTarget` stayed None for about two minutes after arrival with records present (eight pawns at
  5 to 15 km, several of them `Exposure_Half` or `Exposure_Full`). It was set when: a regular beast, hung 300 uu above, came to horizontal 3,084 (3D 3,091) with
  `Exposure_Full` and had no target at 3,320 (3D 3,324), at the same 0.2 s update in which `ThreatLevel` changed to `ETL_SeenInThreatRadius`; an adult and a
  baby at horizontal 4,500 and 4,388 (adult `Exposure_Half`) in the first sweep; rakk drones with `Exposure_Full` at 3.0 to 6.5 km. An adult at the same
  heights with `Exposure_None` (line of sight blocked) **never acquired the player down to 2,925 uu**, although its record's `Priority` was positive.
  So acquisition needs more than the priority list: it followed seeing the player, and the distance at which the first sight happened differed by pawn
  type. The native step that decides this (the Willow override of `ValidCurrentTarget`/`ChooseBestTarget`) is unread (note, "Not read yet").
- **Keeping the target:** an adult that already had the player kept it through `Exposure_None` out to horizontal 6,507 uu, and had lost it (record still
  present, priority 0) at 6,976; the record itself went between 6,976 and 7,526. So loss of the current target comes before loss of the record, and
  consistent with `StopTargetingIf` None there is no other stop condition in the data.
- **Two target events at one moment:** on acquisition **every Action of the pawn's tree** received `ActivateEvent("Target_Acquired")` in one frame
  (the adult's 10, one per action node), in the order: the running action (`Patrol`) first, then the others in node-index order
  (GoToScriptedDestination, CombatPerch, the three AnimAttacks, the Roar, LeapAtTarget, the throw-rock AnimAttack...); the same frame ran
  `TriggerBehavior("Stop")` on the patrol and `("Start")` on the leap, i.e. the state update that follows the target update chose and started an
  action at once. `Target_Lost` went to the same set the same way (the long fight held 20 such frame groups, 318 per-action events: 159 `Target_Acquired` and 159 `Target_Lost`). **`Target_Lost` before
  `Target_Acquired` on a switch was not observed** (there was only one possible target), so the order stays UNVERIFIED. `AIDefinition.OnTargetAcquired`/
  `OnTargetLost` never fired through script.

## 4. From the "Attack" behavior event to the melee call (OBSERVED, 35 swings of one adult, recorded 1483.5 to 1528.2 s)

- For the adult's **Close Melee** node (`Action_AnimAttack`, `Attack_InPlace`, the player adjacent): each cycle is `TriggerBehavior("Stop")` and
  `("Start")` in one frame, then `("Attack")` **0.0105 to 0.1655 s (mean 0.027)** after `Start`, then `Behavior_MeleeAttack.ApplyBehaviorToContext`
  and `MeleeDefinition.MeleeAttack` in the **same frame, 0.400 to 0.425 s (mean 0.4103, sd 0.0073) after the `Attack` trigger**. The animation is the one
  `AnimRandom_PrimalBeastMelee` picks: two different `Behavior_MeleeAttack` objects served the swings (12 and 23 of 35) and **both** pointed at
  `MeleeDef_PrimalBeast_Melee`. The context object was the adult's own pawn.
- **Interval between swings: median 1.249 s** (min 1.221, 33 of 34 within 1.221 to 1.386, one 1.813 after a different action ran in between). The action is restarted
  (`Stop`, `Start`) every swing; the state update runs every 0.2 s, so the interval is the animation and the restart, not a cooldown value that was found.
- The player-side `TakeDamage` followed in the same frame as every call: 35 of 35 swings hit (the player stood within about 300 uu in front), each
  `Damage` 94.104 (= 8 x 1.13^7 x 5 for the level-7 adult), damage type `WillowDmgSource_Melee`, `DamageCauser` the melee definition.
- **Leaps** (the `Leap Attack` and the baby's/regular beast's leap): the `Attack` trigger comes at take-off and the `MeleeDef_PrimalBeast_RadiusLeap` call
  at landing: `Attack` to `Landed` event **1.08 to 3.48 s (mean 1.68, n 157)** for the leaping actions; the call and the `Landed` event were in the same frame in 34
  of the 157 pairs (the others within a few frames; pairing was by action object, without the per-pawn field, so the rest is not resolved). 157 calls of
  `MeleeDef_PrimalBeast_RadiusLeap`, 38 of `MeleeDef_RakkShared_Melee` and 4 of `MeleeDef_PrimalBeast_MeleeGrabBaby` were seen in the long fight,
  and 52, 12 and 3 of them delivered a positive-damage `TakeDamage` to the player (the landing blast also delivers a `Behavior_Explode` hit with damage
  argument 0.0 and type `WillowDmgSource_Grenade`, 80 times). The leap's swing at 24.98 damage and 100,000 momentum came from the regular beast (level 6).

## 5. The melee volume of `MeleeDef_PrimalBeast_Melee`, adult attacker, player victim (OBSERVED, controlled swings)

Definition values read: `DamageRadius` 640, `DamageHeight` 256, `DamageHeightOffset` 0, `DamageConeAngle` 128, `bUseDamageRadius` true, `bUseTargetCone`
false, `bIgnoreWorldGeometry` false, `bAffectEnemy` true, `bAffectNeutral` true, `bAffectFriendly` false. Maya's capsule: radius 42, half-height 80.
The adult's: radius 150, half-height 90. **71 swings** (hung player, `ForceUpdateComponents`, then `MeleeAttack` called from a Tick hook, hits counted only when
the adult's own mind delivered them with that definition; 0 foreign hits in this set; geometry measured at the call). One rule explains all 71:

| test | observed | note |
|---|---|---|
| reach, horizontal distance from a point **100 uu behind** the attacker's `Location` along its facing, at bearing 0 | hit at 639.1 (player 539.1 in front of the attacker), miss at 647.9 | radius 640 with `HitStartOffset` -100: **confirmed** (as a horizontal test; read in 3D from the `Location` height the edge would be 647 to 656, not 640) |
| cone, angle between the attacker's facing and the vector from that same point to the player | hit up to 63.8 deg, miss from 66.2 deg (bearing from the attacker itself reached 78 deg at 300 uu, because of the offset) | half of `DamageConeAngle` 128 = 64: **confirmed**, including that the origin is 100 behind |
| vertical, player's centre relative to the attacker's `Location` (hung, so measured) | hit up to +330.8 and +338.2 (combining runs, edge between 338.2 and 340.7), miss from +340.7; hit down to -141.4, miss from -143.8 (-149.1 in the clean set) | **refuted as written**: the acceptance band is about **-143 to +339** (width about 482, centre about +98 above the pawn's `Location`), not 256 either side of anything plus the victim's half-height (that would be +/- 336) |
| victim damage | `Damage` 94.104 on all 48 hits, one `TakeDamage` per call | 8 x 1.13^7 x 5: **confirmed** (L5b closed form) |
| momentum | magnitude **75,000.0 +/- 0.01**, `Z` exactly 0, direction **from the attacker's `Location`** (not the cone origin) to the player: angle error max 0.003 deg over 48 hits | horizontal unit direction x Knockback x 100,000: **confirmed** (direction and the horizontal rule); the 0.75 is the magnitude over 100,000, `Knockback` itself was not read |

The fitted rule (horizontal 640 from the origin 100 behind, 64 deg half-angle from it, vertical -143 to +339) matches 71 of 71 swings; the vertical edges and the horizontal reading of the radius
are **fitted to this data** (and so UNVERIFIED as the native's own formula); the 640 and the 64 are the note's. Other candidates were not tried: other definitions
(`_RadiusLeap`, `_RadiusBadass`, grab and charge), other victims (the vertical band's dependence on the victim's capsule could not be separated with
one victim), line-of-sight blocking by geometry (not exercised: no blocker was placed), `bMeleeOverPeriodOfTime` definitions.

## 6. Spawned guns' damage: the 0.7589 factor (OBSERVED)

L7 found the five slice guns' live `InstantHitDamage` was a constant **0.7589** of `tools/weapon_stats.py`'s card damage at level 8. The factor is
**the item level times a player-side term, and both were identified**:

- **`ManufacturerGradeIndex` in the definition data is the item level.** The L7 builds passed grade index 0 (the Fire pistol, the others the same),
  which gives the weapon `ExpLevel` 0 (game stage still 8). Rebuilding the five slice guns with grade index 8: base `InstantHitDamageBaseValue` went
  **23.936 to 63.633** (Jakobs pistol), **18.480 to 49.128** (Fire pistol), **8.372 to 22.256** (SMG), **13.585 to 36.115** (rifle), **7.767 to 20.648** (shotgun, per pellet): ratio **2.658444 = 1.13^8** for all five, and the grade-8 values are **exactly the card
  damages L7 compared with** (63.63, 49.13, 22.26, 36.11, 20.65). A weapon the game rolled itself from a balance at game stage 8 had `ExpLevel` 8, and a definition-data twin built
  from its own `DefinitionData` (grade index 8) was identical (ratio 1.0000, three guns); `SetExpLevel(8)` on a grade-0 gun only sets the field and
  leaves the damage unchanged (the damage is computed when the item is built).
- **The player-side term is the Badass Rank modifier.** The equipped grade-0 pistol's `InstantHitDamage` stack held one modifier, an `MT_Scale` from
  `GD_Challenges.BadassSkill.BadassSkill` (the profile's Badass Rank, the same skill L1 found in the health maximum; its size is in the maintainer's private record): `23.936 x (1 + scale)` gives the L7 value. Backpack weapons carry no such modifier (live value equals the base).
- So `0.7589 = (1 + scale) / 1.13^8`. **The equipped grade-8 pistol would read 63.633 x (1 + scale)**: that is arithmetic, not a read (the grade-8 gun was only read in the backpack).
  The weapon card (the base, 63.63) does not include Badass Rank. Also seen: the base is `NormalizedInstantHitDamage` x 8 x 1.13^`ExpLevel` for both kinds of build (2.992 x 8 = 23.936;
  2.2 x 8 x 1.13^8 = 46.789); where the 8 comes from was not read.
- Consequence for L7's recordings: stored damages there are the grade-0 values times Badass Rank; nothing else in L7's record changes (its rules are ratios and formulas).

## 7. Save safety

`Backup-Saves` copied `SaveData` and `Config` first (`local/realgame/save-backup-20261009-083933`, plus the SDK settings and Sanity Saver copies); 60 files
hashed before and after. With saves blocked from the first frame, the 33 save and config files were byte-identical after the session. Two files
differed, as in L3/L5/L5b/L7: Sanity Saver's private `Save0008.json.gz` (rewritten at load) and the SDK's `python-sdk.json` (its update-check stamp).
Both were restored from the backup; re-hash: **0 differences of 60**. The driver folder was removed, the game was closed through its window (2 s), no process was
killed, and the lock was released. No Application Control block, no crash. The player once fell out of the world (at 400 s, see traps); the game put her back.

## 8. Status of the NATIVE_AI_MELEE.md rules

| rule (section of the note) | status | matching number |
|---|---|---|
| Both updates in one tick, gated by `UpdateRate`; target update tests `LastStateUpdateTime` (cadence) | **confirmed** | 2,433 frames, equal in all, spacing min 0.1999, `UpdateRate` 0.2 |
| `Priority` = weighted mean of `OnePriority`; weights from data; Idle and Attacking silent (list of iterators) | **confirmed for this definition** | 134 samples to 5e-8; weights 1, 1, 0.4, 1 (sum 3.4); Idle and Attacking weight 0 |
| Nearby = `1 - Distance/MaxDistance`, clamped; uses `Distance` (iterators) | **confirmed** | MaxDistance 4096, 3D `Distance`, 6 spot values to 6 digits |
| Exposed = `TargetExposureToMe`; Previous = 1 for the current target (iterators) | **confirmed**; the exposed iterator is unclamped (-1.0 seen) | same samples |
| Bonus adds `BonusPriority` plus an AI-interface term | **unchanged**: `BonusPriority` was 0 throughout; no extra term was needed to fit | |
| Search radius 4096 XY, height ignored (target list) | **radius refuted, height confirmed**: records existed at 4,100 to 7,000 uu (and, at the arrival ledge, to 10.2 km); in one geometry they dropped at about 7.07 km; the 4096 is not a filter | present 7,053, absent 7,080.6; 1,497 uu above still present |
| `WillowTI_CalcCombat` is in `Knowledge` (iterators) | **refuted for the live definition**: the fifth entry holds no object | `Knowledge[4]` is None |
| Distance fields and `ThreatLevel` filled by `CalcDistances`/`CalcThreat` (iterators) | **partly confirmed**: the distance fields equal the pawns' geometry; `ThreatLevel` was `InThreatRadius` or `SeenInThreatRadius`, other values not seen | |
| `ChooseBestTarget` takes the best valid record, `CanTargetIf`/`StopTargetingIf` gate | **unchanged for the list; acquisition needs more**: a record with positive priority was not made current without sight | about 2 min without a target at arrival; no target with `Exposure_None` to 2,925 uu |
| Events: `Target_Acquired`/`Target_Lost` go to the running action and every other action in the tree | **confirmed** (running action first, then node order) | 10 of 10 actions of the adult; 159 and 159 per-action events |
| `Target_Lost` raised before `Target_Acquired` on a switch | **UNVERIFIED** (no switch seen) | |
| `AIDefinition.OnTargetAcquired`/`OnTargetLost` observable through script | **refuted for hooks**: never fired | 0 rows in 8,838 |
| `TriggerBehavior("Attack")` then `Behavior_MeleeAttack` then `MeleeAttack` | **confirmed**; delay measured | 0.400 to 0.425 s, mean 0.4103, n 35; same frame for the behavior and the melee call |
| Which `MeleeDefinition` the melee action uses | **confirmed**: `MeleeDef_PrimalBeast_Melee` for the adult's Close Melee, `_RadiusLeap` for the leap at landing | 35 of 35; 157 leap calls |
| Radius volume: radius 640, offset -100 forward, half-angle 64 | **confirmed** (as above) | 71 of 71, edges 639.1/647.9 and 63.8/66.2 deg |
| Vertical: centre +/- `DamageHeight` against the victim's extent | **refuted as written**: band -143 to +339 about the `Location` | edges (-141.4, -143.8) and (+338.2, +340.7) |
| Damage = attribute-initialisation value; momentum = horizontal unit direction x Knockback x 100,000 | **confirmed** (adult: 94.104, momentum 75,000 from the attacker's `Location`) | 48 hits, 0.003 deg |
| Allegiance flags, behavior lists, `bMeleeOverPeriodOfTime` follow-through, grab and charge, line trace blocking | **not exercised** | |
| Selection pass, random node 1.0 : 0.25 : 0.25, perch before 10 s, `Action.CanRun` reasons, `TimedReset` | **not exercised** (the recording was a fight with the player near; the node weights and times were only read as data) | |

## 9. Open items

- Why the list is cut near 7 km in one geometry and not at the arrival ledge, why it is not 4096, whether it is horizontal or 3D, and why rakk drones are not cut (candidates: the navigation mesh or an awareness volume, population relevance, the weapon-range data of `CalcThreat`); repeat the in-and-out sweep on at least two bearings and with the player on the ground.
- What makes a record's pawn make the player its current target (sight distance by type, the hidden part of `ValidCurrentTarget`); a line-of-sight-clear approach of one adult and one baby with finer steps would bracket the acquisition distance per type.
- The vertical band: read the attacker's trace-source height (`GetMeleeTraceSourceLocationAndRotation` has out parameters the SDK would not fill when called without them) and test a second victim size.
- Other melee definitions' volumes (`_RadiusLeap`, `_RadiusBadass`, `_MeleeGrabBaby`, the charge), line-of-sight blocking, and `Action.Limits` values (attack range, height) of the attack actions.
- Random-node outcome frequencies and the perch node's 10 s: a 30 s recording from first sight with the node state (`CurrentState`) read each frame.
- The meaning of `TargetExposureToMe` -1.0 and of `Exposure_Half` (0.5) against the note's three-level table.
- The equipped grade-8 gun's live value (predicted as 63.633 x (1 + the Badass Rank scale)).
