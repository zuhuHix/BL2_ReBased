# Combat golden trace: a real fight, a death, a loot drop and a level-up cooldown check in the running game (lane L3, 2026-10-07)

AI-assisted (Claude), real-game lane L3. One launch of the original game (Borderlands 2, Steam, 32-bit DX9, `-windowed` 1280x720), driven
with `tools/real_game/` and recorded by Python hooks on script-visible functions (pyunrealsdk through the `openwillow_realgame` driver), the
way [FIRE_MISSION_GOLDEN_TRACE.md](FIRE_MISSION_GOLDEN_TRACE.md) did for the Fire mission. This is the combat half of the Phase 4 gate: hit,
kill, kill experience, drop, pickup, being hit, down, death, respawn, plus the action-skill cooldown question of NATIVE_LEVEL_UP_ATTRIBUTES.
Raw rows (17,723 hook rows, 46,785 per-frame rows), snapshots and screenshots stay under ignored `local/realgame/L3/`; this record holds rules,
counts, names and formulas only.

Labels: **observed** = seen in the running game by a hook or a read of live state; **UNVERIFIED** = inferred or not shown. A hook only sees
calls that go through script (`ProcessEvent`); native-to-native calls are invisible, so "not seen" never means "did not happen".

## How it was driven

- Level-8 Maya ("Plan B", `Save0008.sav`), loaded with the startup mod's `-Character=Save0008.sav` and Enter on "Continue", driver installed with
  `Install-Driver -BlockSavesAtStart`, one game process under `local/ue_run.lock`, saves backed up and hashed first (see "Save safety").
- **Travel (script, memory only):** `WillowGameInfo.InitiateTravel` with the fast-travel station `GD_FastTravelStations.Zone1.IceEast`
  (Three Horns Divide, `Ice_P`). The game's own `TravelToStation` ran; arrival is `ResurrectPlayer`/`ResurrectAtLocation` on the arrival station's
  exit point. No console `open` command was used.
- **Real inputs:** left mouse button for every shot (Texas Scattergun, a Jakobs shotgun: 2 rounds a clip, 7 pellets a shot), the W key for one
  short step onto money pickups, the use key (E) on a weapon pickup, F for Phaselock. Script stand-ins: the camera was aimed with
  `ClientSetRotation` before each shot (as lane L2), the player was placed 450-900 uu from a chosen enemy to start the fight and 650 uu for the
  casts, and `InventorySlotMax_Misc` was raised from 12 to 20 in memory for the pickup that needed room.
- **Down, death, respawn and second wind were not scripted.** Enemies downed Maya; the bleed-out timer ran out three times by itself and the game
  respawned her; no kill command, no health write. For the second wind the player waited until down, aimed at a 59-health baby bullymong and fired
  once.
- 463 hooks (165 on script functions, 298 on every `Behavior_*` class's `ApplyBehaviorToContext`): all resolved, none missing, none failed to
  install; 106 of the 165 non-behavior hooks saw a call and 91 behavior classes saw 9,944 calls. Rows carry the QPC clock, the game's own time and
  a frame counter; a per-frame sampler (`cb_frames`) records the player's health and shield with their regeneration fields, the injured state and
  timers, the cooldown pool and one focus enemy. State reads ("snapshots") were taken before the fight, after the kill, after respawn, before and
  after the loot pickups, before each cast, after the level-ups and around the second wind.
- Hooks that resolved but **saw no call** (so these happen natively, or did not happen in this session): `AIClassDefinition.OnTakeDamage`,
  `OnVehicleTakeDamage` and `OnUsed`; `BodyClassDeathDefinition.OnDeathNonGib` and `OnDeathGib`; `WillowPawn.OnShieldDepleted`, `OnShieldFull`,
  `ShieldTriggered`, `NotifyDamageTaken`, `HealDamage`; `WillowPickup.Landed`, `FailedPickup`, `BeginShrinking`,
  `AdjustPickupPhysicsAndCollisionForBeingDropped`; `WillowPlayerController.PawnDied`, `ExpEarn`, `ClientOnRevivedPlayer`;
  `WillowGameInfo.ShouldRespawn` and `ResurrectAllPlayers`; `WillowPawn.RevivePawnHealthAndShield`, `RejuvenateFromInjured` and some
  `WillowPawn` copies of the injured-state functions (the `WillowPlayerPawn` copies ran instead); `ItemPool.SpawnBalancedInventoryFromInventoryBalanceDefinition`;
  `StartInjuredPreDeathSequence`, `InjuredSuicide`, `SetBeingRevived`. Net: the OnTakeDamage raise, the shield stage of the pipeline, pool
  regeneration, experience crediting and the `Actor.TakeDamage` chain of `ActorTakeDamageInner` are native.
- **Installed SDK mods (enabled on this machine).** Read from the live mod list at the start: Dialog Skipper (autoskip on), Sanity Saver, Apple's
  Cheats (all Off), Commander, Item Lights, RTSE and others stayed active; none touched combat in the recorded window. **Drop Chance Multiplier
  was listed as not enabled at runtime** (its settings say 2, but this legacy "load on main menu" mod was not active in the session), Loot
  Collector and Be Gone Out Of Bounds Loot were switched off in memory (hooks removed, keybinds disabled, no settings write). The effect on the
  trace is therefore none: each death made exactly one `DropLootOnDeath` call and each pool roll one `SpawnBalancedInventoryFromPool` call (no
  doubling). Better Damage Feedback is not in the loaded mod list. The first `cb_mods()` call failed with a TypeError (a probe bug, fixed before
  any trace row was recorded).
- A world fact that shapes what was seen: on `Ice_P` the enemies also fight each other (burning psychos against bullymongs, a Savage Lee that
  a bullymong killed), so three of the four AI deaths in the recording were not the player's (the 21 `TookDamageFromFriendly` rows).

## The live sequence (observed rules)

Times are game time (`WorldInfo.TimeSeconds`) from the first call of the step unless noted. "Same frame" is one render frame (about 10 ms).

### A. One shot at an AI pawn (a pellet of the player's shotgun on an adult bullymong)

1. **Per pellet, one frame, this order:** `WillowAIPawn.TakeDamage` -> `WillowPawn.TakeDamage` -> `Pawn.TakeDamage` -> `GameInfo.ReduceDamage` ->
   `WillowPawn.AdjustDamage` -> `Pawn.SetHealth(health - damage)` -> `Pawn.NotifyTakeHit` (the `Behavior_*` of the AI class run inside it:
   `Behavior_Conditional`, `Behavior_SpecialMove`) -> `WillowPawn.TookDamageFromEnemy` (with `ProcessDamageTakenSkillEvents` inside it) -> back out of
   `Pawn.TakeDamage` and `WillowPawn.TakeDamage` -> `WillowAIPawn.CheckInjured` -> back out of `WillowAIPawn.TakeDamage`. About 6.5 ms of the
   game's wall clock per pellet with hooks on. Every pellet is its own `TakeDamage` chain (7 and 2 chains for two clicks 1.14 s apart).
2. **Numbers.** Each pellet arrived as 41.4 (the weapon's `InstantHitDamage` read 41.398) and lowered the victim's health by exactly 41.4
   (360.876 -> 319.48 -> 278.08 -> 236.68 ...), no shield, no crit, flesh, a level-5 victim against the level-8 player. `AdjustDamage` showed its
   input only (its output arguments read as unset), so "damage out = damage in" is the health arithmetic, not a read of the output.
3. **Status effect roll** (`StatusEffectsComponent.RollChanceForStatusEffect`) ran for incendiary and shock hits on pawns (88 calls in total, by psychos
   with `Status_Incendiary` and by an area with `Status_Shock`); none for the shotgun's `DmgType_Normal`, which has no status effect.
4. **The OnTakeDamage event** is raised natively (no `AIClassDefinition.OnTakeDamage` call was visible) but its behaviors ran inside
   `NotifyTakeHit`, after `SetHealth` and before `TookDamageFromEnemy`.

### B. The kill, kill experience, the drop (one frame)

5. **Death order (the killing pellet, health read -11.7 after `SetHealth`):** `TookDamageFromEnemy` -> `WillowPawn.HandleHealthDepleted` ->
   `MissionTracker.NotifyPawnDied` -> `Pawn.HandleHealthDepleted` -> `WillowAIPawn.Died` -> the killer's `WillowPlayerPawn.KilledEnemy` ->
   `WillowPawn.Died` -> **`WillowPawn.DropLootOnDeath` first** (pool rolls and pickup creation inside it, below) -> `StatusEffectsComponent.OwnerDied`
   -> `BodyClassDeathDefinition.OnKilledBy` (three behaviors) -> `Pawn.Died` -> `Pawn.SetHealth(0)` -> `WillowGameInfo.Killed` ->
   `WillowExperiencePipeline.CalculateExperiencePointsForKill` -> `WillowGameInfo.AwardCombatExperience` ->
   `WillowExperiencePipeline.AwardCombatExperienceToParty` -> `GameInfo.Killed` -> `WillowAIPawn.PlayDying` -> `WillowPawn.PlayDying` ->
   `BodyClassDeathDefinition.OnPlayDeathPizazz` (four behaviors, the last a dialog event) -> `WillowAIPawn.CheckInjured`. Pellets that landed after
   the kill ran `WillowPawn.Dying.TakeDamage` (a state function). `AIDefinition.OnKilledPawn` ran only for deaths whose killer was an AI mind (3 of the 4),
   not for the player's kill.
6. **Kill experience.** `CalculateExperiencePointsForKill` returned **112.5** for the level-5 adult bullymong against the level-8 player and
   `AwardCombatExperience` was called with 112.5; the player's experience went 26,218 -> 26,263 (**+45, 0.4 of the award**). A level-6 baby
   bullymong killed by the level-10 player: award **50.0**, experience 26,263 -> 26,270 (**+7, 0.14 of the award**). No `ExpEarn` call was seen (the
   crediting is native inside `AwardCombatExperienceToParty`). The scaling between award and pool is read from two cases only; its rule is open.
7. **The pool rolls (what script saw, `SpawnBalancedInventoryFromPool`):** one call per pool that fired, from inside `DropLootOnDeath`, context the
   dying pawn. Adult bullymong (item game stage 7): `Pool_Money_1or2` -> **3** `Money_Little` items (balance `ItemGrade_Currency_Money_Little`,
   manufacturer `Cash_Manufacturer`, grade-or-level 7). Baby bullymong (stage 6): `Pool_Money_1or2` -> 1 item. Regular bullymong (stage 6):
   `Pool_Ammo_All_Emergency` -> returned false, nothing spawned. Savage Lee (a boss-class psycho, stage 6): `Pool_GunsAndGear` -> one Maliwan pistol
   (`GD_Weap_Pistol.A_Weapons.Pistol_Maliwan`, level 6; `RemoveFromInventory` then its pickup in the same frame) and `Pool_VehicleSkins_All` -> a vehicle
   skin item that was initialised and got no pickup (no removal call was seen either: not dropped). Each item that survived got a `WillowPickup`: `PostBeginPlay` ->
   `InitializeFromInventory` -> `InitializeDropLifespan` -> `InventoryAssociated` -> `PickupAtRest`; the pickups had about 490 s of life span
   left when read some 20-120 s later. Pools that failed their chance roll make no call, so they are invisible.
8. **Pickup path.** Money, by touch (one W step): `TouchedPickupable` -> `PickupSomething` -> `WillowPickup.DenyPickupAttempt` (false) ->
   `WillowPickup.GiveTo` -> `HandlePickup` -> `ServerAnnounceItemPickup` -> `WillowInventoryManager.AddInventory` -> `RemoveFromInventory` (the money
   item is consumed) -> `PickedUpBy` -> `WillowPickup.Destroyed` -> `UnTouchedPickupable`; three pickups raised the cash 2,525 -> 2,534 (3 each).
   Weapon, by the use key with the backpack at 13 of 12: `PickupSomething` -> `DenyPickupAttempt`, nothing else ("FULL" on the prompt, no pickup).
   With the slot limit raised in memory: `PickupSomething` -> `DenyPickupAttempt` (false) -> `GiveTo` -> `HandlePickup` ->
   `ServerAnnounceWeaponPickup` -> `AddInventory` -> `AddInventoryToBackpack` -> `PickedUpBy` -> `Destroyed` -> `UnTouchedPickupable`, all in
   one frame; the pistol went to the backpack (13 -> 14), not equipped.

### C. The player is hit

9. **Shield first, then health.** Grenade hits from a ranged bullymong arrived as 44.22; each took **31.616** from the shield (180.589 -> 148.973
   -> ...), a factor of 0.715 on the arrival value (the factor is read, its pipeline stages are not). When the shield had 23.387 left, one hit
   emptied it and the **remainder 8.229 (= 31.616 - 23.387) went to health** (429.066 -> 420.837). Melee arrived as 94.1 and took 69.73 of health
   (0.741); a leap's melee 83.28 took 60.63 (0.728). `WillowPlayerPawn.TakeDamage` and `WillowPlayerPawn.AdjustDamage` wrapped the same chain as A.
10. **Shield recharge.** After the last hit the shield starts to rise **1.613-1.618 s later** (the pool's idle regeneration delay read 1.611 s plus
    one frame) at **about 25.9 per second** (the idle rate read 25.88; per-frame rates 24.7-26.9), reset by each new hit; health did not
    regenerate (idle and active rates 0).

### D. Down (fight for your life), second wind, death, respawn

11. **Down.** The hit that takes health to 0: `WillowPlayerPawn.HandleHealthDepleted` -> `WillowPawn.HandleHealthDepleted` ->
    `ClearSecondWindReason` -> `SetInjuredState(1)` -> `SetupPlayerInjuredState` -> `WillowPlayerController.SetupInjuredState` ->
    `WillowPlayerPawn.injured.HandleHealthDepleted` (the state's copy, which does nothing). Health stays 0 and is clamped; **2.28 s later**
    (2.275-2.284 s in five episodes) `SetInjuredState(2)` -> `GoToInjuredAndTargeted` (the sidearm special move ended).
12. **While down**, enemy damage still reaches `WillowPlayerPawn.injured.TakeDamage` (a state function: 3-12 calls an episode, damage values
    present) and health does not move. The bleed-out timer `flInjuredTargetedTime` counts from state 2.
13. **Bleed-out is 12.0 s** (12.002, 12.005, 11.999 s from state 2 to the dead state in three episodes; `TotalBleedoutTime` read 12.0 in all five;
    the previous injury was over 90 s earlier each time).
14. **Death.** `SetInjuredDeadState(1)` + `StartInjuredDeathSequence`; `SetInjuredDeadState(4)` (ragdoll) **0.84 s** later;
    `WillowPlayerPawn.injured.EndInjuredDeadCamera` / `EndInjuredDeadCamera` **5.76 s after the dead state began** (5.761, 5.765, 5.762).
15. **Respawn, one frame (reason IDied):** `WillowPawn.injured.InjuredRespawn` -> `WillowGameInfo.HandlePlayerDeathResurrection` ->
    `WillowPlayerPawn.ResurrectPlayer(ResurrectReason = 1)` -> `ResurrectAtLocation(holding point, exit point, 1)` ->
    `GoFromInjuredToHealthy` -> `SetInjuredState(0)` / `SetInjuredDeadState(0)` -> `ClearPlayerInjuredState` -> `ClearInjuredState` ->
    `ClearSecondWindReason` -> **five `Behavior_RefillResourcePool` of `PlayerInjuredDefinition`**. Health and shield read full already at the
    start of `ResurrectPlayer` (429.066 / 180.589, later 484.844 / 180.589); no `SetHealth` call was visible, so the refill itself is native.
    The exit point was a different `StationTeleporterExitPoint` each time (_9, _10, _11 after the arrival's _8) and the holding point a
    `TargetPoint` (1, 0, 2), all at the arrival station of `Ice_P` (within about 400 uu of one another); the nearest/active-station choice
    itself is native and not seen.
16. **The fee.** `WillowPlayerPawn.ShowRespawnDialog` ran **11.80 s after `ResurrectPlayer`** (11.803, 11.799, 11.816), then
    `WillowPlayerReplicationInfo.AddCurrencyOnHand(0, -N)` and `ClientShowRespawnDialog(AmountCharged = N, reason 1)`. N was **190 for 2,715 cash,
    177 for 2,534, 165 for 2,357**: 0.07 x cash rounded half up (190.05, 177.38, 164.99), and the cash went 2,715 -> 2,525 -> (+9 loot) 2,534 -> 2,357
    -> 2,192.
17. **Second wind (kill while down).** Bleed timer 7.6 s in, one shotgun shot killed the baby bullymong. Same frame as its `Died`: `injured.KilledEnemy`
    (`WillowPlayerPawn` and `WillowPawn` copies) and health already 25 % of maximum (**136.97 = 0.25 x 547.874**); the next frame
    (65 ms later, a long frame under the hooks) `GoFromInjuredToHealthy`: shield **162.53 = 0.9 x 180.589**, injured state 0,
    `ClearSecondWindReason`. The experience was still awarded. The refill of health in the kill frame shows no script `SetHealth`. She was down
    again 5.4 s later (the enemies were still on her); `NumSuccessiveInjuries` read 1 during that fifth injury (the previous injury was 15.4 s
    earlier, under the 30 s rule). The end of that fifth injury was **not observed** (the game was closed 7 s into it), so the shortened
    bleed-out of the successive-injury rule is not shown (the `TotalBleedoutTime` attribute kept reading 12.0).

### E. Part B: does a level-up reset the action-skill cooldown?

18. **Casts (real F key, Phaselock on a bullymong at 650 uu).** `StartActionSkill` -> `SkillEffectManager.ActivateSkill(Skill_Phaselock)` ->
    `LiftActionSkill.PhaseLockTarget` -> `Skill_Phaselock_CooldownManager` activated -> `ActionSkillCallback(bActivated true)` ->
    `StartActiveSkillCooldown`. The cooldown pool (maximum 13, minimum 0) read 0 before and **13.0 from the next frame, with its net drain 0**
    through the lift and hold; `ReleaseTarget` came 5.708 s after the key, deactivated the cooldown manager (the drain starts: 12.70 at +6.0 s,
    8.70 at +10 s, 1 per second), `Skill_Phaselock` was deactivated and `ActionSkillCallback(false)` ran 6.705 s after the key (the skill's duration
    plus its release buffer). One cast at a ranged bullymong (650 uu) was not a lock: `StartActionSkill` with `AutoAimTarget` None -> `LiftActionSkill.Fizzled`
    **1.01 s later**, cooldown 0, as the action-skill note says; a retry at 500 uu saw no `StartActionSkill` call at all (the player was
    downed within the 11 s of the attempt).
19. **`ExpLevelUp(false)` during the hold (2.0 s after the cast, cooldown 13.0 at its maximum):** the cooldown read **0.0 in the same call**
    and stayed 0 through the release (no later refill). **During the drain (cooldown 7.9915 at 8 s):** 0.0 in the same call. Both level-ups
    (8 -> 9, 9 -> 10) ran, in one frame: `ExpLevelUp` -> `SetSecondWindReason(2)` -> `OnExpLevelChange(true, true)` ->
    `RecalculateAttributeInitializedState` -> `Behavior_AttributeEffect` of `PlayerBehavior_LevelUp` ->
    `ActivateSkill(SkillDefinition_0)` (health refill: 429.07 -> 484.84 and 484.84 -> 547.87, full) -> second `Behavior_AttributeEffect` ->
    `ActivateSkill(SkillDefinition_2)` -> `PlayerBehavior_LevelUpNaturally` -> `ActivateSkill(SkillDefinition_1)` -> `ClientOnExpLevelChange(9)`
    -> **`WillowPlayerController.ActionSkillCooldownComplete`** (script) within 1 ms of `ClientOnExpLevelChange`.
20. **Re-observed from L1, with times:** the shield recharge boost read **90.29 per second (0.5 x the maximum 180.589) from 0.02 s to 3.98 / 4.00 s**
    after the call (4.0 s), and the equipped weapon's damage read **41.398 -> 61.918 from 0.02 s to 29.99 s** (30 s).

## Not visible to hooks

`ActorTakeDamageInner` and the Kismet TakeDamage events, `RecordRecentDamage`, the shield stage of the pipeline (shield value and the factor were read
from the pool, not from a call), the `OnTakeDamage` raise (only its behaviors), pool regeneration, status-effect damage ticks, experience crediting
(`ExpEarn` never ran through script), the refill of health at respawn and at second wind (read, not called), the choice of the respawn station and
`TravelStation.CanResurrectHere`, the pickup's native money credit, dialog lines, kill animations, HUD updates, and every pool whose chance roll failed.

## What this confirms or contradicts in the notes

NATIVE_DAMAGE_DEATH.md:

- **Section 0.1 order: confirmed** (steps 1, 4): reduce, adjust, `SetHealth`, `NotifyTakeHit` (the OnTakeDamage behaviors inside), `TookDamageFromEnemy`,
  then `CheckInjured` after `WillowPawn.TakeDamage` returns. Health is lowered before the event, as the note says (hook rows with the health values).
  The status-effect roll sits inside `TookDamageFromEnemy`'s side (it ran for status-bearing hits only).
- **Section 9 death order: confirmed** (step 5): `NotifyPawnDied` before `Pawn.HandleHealthDepleted`; `KilledEnemy` from `WillowAIPawn.Died`;
  `DropLootOnDeath` first in `WillowPawn.Died`; `OnKilledBy` before `Pawn.Died`; `SetHealth(0)` in `Pawn.Died` (the health read -11.7 before it);
  `Killed` -> experience -> `GameInfo.Killed` -> `PlayDying` -> `OnPlayDeathPizazz`. Refinement: `AIDefinition.OnKilledPawn` runs only for an AI killer.
- **Kill XP "amount rule open": two data points** (112.5 and 50.0 awarded, +45 and +7 credited); the award is computed in script-visible
  `CalculateExperiencePointsForKill` (its return is readable at POST), the pool credit applies a level-based factor natively.
- **Section 1 shield stage: confirmed in shape** (step 9): shield takes first, the remainder goes to health in the same damage units (31.616 absorbed
  of 44.22 incoming; 8.229 overflow). The per-hit factor 0.715 (grenade) and 0.741 (melee) are new data; the stage formulas that produce them stay open.
- **Section 10.1-10.3: confirmed**: down on health 0 (not death), 12.0 s bleed-out from the targeted state (note: `BaseRejuvenateDelay` 12 for one
  player), further damage ignored in the state, the state's `HandleHealthDepleted` does nothing. New numbers: 2.28 s from health 0 to the targeted
  state, 0.84 s to the ragdoll state, **death camera 5.76 s** (answers the note's open "duration of the death camera before respawn").
- **Section 10.4 respawn: confirmed**: `InjuredRespawn` -> `HandlePlayerDeathResurrection` -> `ResurrectPlayer(IDied)` -> `ResurrectAtLocation` -> `GoFromInjuredToHealthy`,
  full health and shield, five refill behaviors, exit point from the station's round-robin (_9, _10, _11 consecutively). **Fee: confirmed 0.07 x cash,
  rounded half up, three cases** (190, 177, 165). **Contradicted:** the fee is charged **11.8 s** after arrival (the note read 2.0 s for the non-station
  path and called the station path untraced); it is charged at `ShowRespawnDialog`, not at death or arrival. Station choice itself is not seen.
- **Second wind: confirmed** (kill while down -> health to 25 % of maximum, shield to 90 %, states cleared by `GoFromInjuredToHealthy` on the next
  tick). Refinement: health is already raised in the kill frame (before `GoFromInjuredToHealthy`, with no `SetHealth` hook call); the shield and the
  state change come with the next frame. A level-up sets reason 2 through `SetSecondWindReason` at the start of `ExpLevelUp` (not tested while down).
- **Successive injuries:** `NumSuccessiveInjuries` read 0 for injuries over 30 s apart and 1 for one 15.4 s after the previous (the grow rule is consistent);
  the shortened bleed-out is **UNVERIFIED** (not observed to its end).
- Shield recharge (open in section 4 "Open"): delay 1.61 s, 25.9 per second for this shield and character, observed per frame.

NATIVE_LOOT.md:

- **Death roll rolls pools through script after `DropLootOnDeath` begins, one `SpawnBalancedInventoryFromPool` call per pool that fires: confirmed**
  (one `DropLootOnDeath` per death, no doubling with the drop mod off). **Correction 3 (vehicle skins): consistent**: `Pool_VehicleSkins_All`
  rolled for a boss-class pawn and its item got no pickup (that it is destroyed rather than dropped is read from the missing pickup, not from a call).
- **Item game stage:** the pools were called with stage 6 or 7; the level-5 adult got stage 7, the level-6 baby stage 6, so the loot stage is not the
  displayed level (population game stage or offset, as the note's overview step 4 allows). Stored item level = stage (Money 7, pistol 6): confirmed
  for these items.
- **`Pool_Money_1or2` gave 3 items in one call** (adult) and 1 (baby): the quantity is not simply 1 or 2; the quantity attribute of this pool is worth
  a re-read (UNVERIFIED, one pool, two calls).
- The optional arguments of the pool call read as 0 / false in the hook (`OuterPoolChance`, `bInventoryMayDropOnDeath`): the hook shows unset optional
  arguments as zero, so those two are **not** an observation of the values used.
- Pickup, auto-touch of money, full-backpack refusal and the backpack pickup are new observed chains (steps 8).

NATIVE_LEVEL_UP_ATTRIBUTES.md / NATIVE_ACTION_SKILL.md:

- **The level-up cooldown reset: confirmed** (answer to lane L1's inconclusive test): the `PlayerBehavior_LevelUp` effect sets the cooldown pool's current
  value to 0 in the same call, whether it is at its maximum during the hold or draining (7.9915 -> 0.0), and it stays 0. Consistent with "Scale by
  `ActiveSkillCooldownMinValue`" (the minimum read 0 for this level-8/9/10 Maya); the scale reading itself is not shown, only the result.
  **New:** `ActionSkillCooldownComplete` runs right after `ClientOnExpLevelChange` on this path (the note listed its native caller as unfound; here a
  script call follows the reset by under 1 ms).
- Shield boost 4.0 s at 0.5 x maximum per second and the 30 s weapon-damage scale of the natural level-up: **a second observation, with times**.
- Cooldown start/drain/release timing: **confirmed** (13.0 at the cast, flat to the release at 5.708 s, 1 per second after; skill off at 6.705 s;
  a fizzle after 1.01 s leaves 0). Order in the cast matches section 5 of NATIVE_ACTION_SKILL: delegate start after `OnSelectedTarget` and the
  cooldown manager.

## Save safety

`Backup-Saves` copied `SaveData` and `Config` first (`local/realgame/save-backup-20261007-130254`), together with the whole `sdk_mods/settings`
folder and `sdk_mods/SanitySaver/Saves`; 60 files were hashed before and after (33 save and config files, the SDK settings folder, Sanity
Saver's own backups). With saves blocked from the first frame (`Install-Driver -BlockSavesAtStart`), **all 33 save and config files and the whole
`sdk_mods/settings` folder were byte-identical after the session** (the SDK's own `python-sdk.json` included). One extra file differed:
Sanity Saver's `Save0008.json.gz`, a private copy that mod rewrites when it loads the character; it was restored from the backup and the comparison
then showed 0 differences in all 60. No mod code was edited, no setting was written by this lane (the loot mods were switched off in memory only), the
driver mod was removed, the game was closed through its window (exited in 2 s) and the run lock was released by this lane (taken 13:02:33). Nothing
spawned or changed in the game (cash, XP, levels, items, the raised backpack limit) survives: they were in the closed process only.

## Reproduce

`tools/real_game/scripts/combat_probe.py` (hooks, per-frame sampler, snapshots, placement and aim helpers, in-memory mod switch, `cb_arm` for the
level-up at a known cooldown state) driven through `tools/real_game/realgame.ps1`: backup and hash, `Enter-RunLock`, `Install-Driver
-BlockSavesAtStart`, start with `-Character=Save0008.sav`, Enter, load the probe, `cb_mods_off()`, `cb_start()`, `cb_frames(True)`, travel with
`WillowGameInfo.InitiateTravel` to the `IceEast` station definition, `cb_place` / `cb_aim` and real clicks, `cb_snap(label)` between steps,
`cb_dump(name)`, close the game through its window, compare hashes, `Remove-Driver`, `Exit-RunLock`.
