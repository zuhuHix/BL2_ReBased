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
   *Added 2026-11-12:* the "3 each" above is inferred from the sum of three pickups, not read per pickup. In session 2 of 2026-11-12 (Three Horns
   Divide, Maya level 8, the three loot mods off in memory) a pair of level 7 pickups touched in one frame paid 4 together, so the value per pickup at
   one level varies (weakly: one pair; why is UNVERIFIED; the items carry part definitions). That session also saw no `AddCurrencyOnHand` call for a money pickup,
   which fits "the pickup's native money credit" ([COMBAT_RANGE.md](COMBAT_RANGE.md), "Den reads of session 2").

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

## Open-rule checks (lane L5, 2026-10-07/08)

AI-assisted (Claude), lanes L5 (2026-10-07, stopped after question 3) and L5b (2026-10-08, questions 4-6 and the bullymong). Same method as lane L3: one game
process under the run lock, level-8 Maya (`-Character=Save0008.sav`), `Install-Driver -BlockSavesAtStart`, memory-only, closed through its window. New probe:
`tools/real_game/scripts/open_rules_probe.py` (load it after `combat_probe.py`, never after `fire_mission_probe.py`). Raw rows (67,609 hook rows and 107,481 frames on
2026-10-08) stay under ignored `local/realgame/L5/` and `local/realgame/L5b/`. Labels as above: **observed** or **UNVERIFIED**.

**Fire dummy, pool slots, crit base (lane L5, 2026-10-07).**
- The Fire target dummy has `GetHealth` = `GetMaxHealth` = **778.658** (level 8, game stage 8, `ExperiencePointMultiplier` 0.5, class health multiplier 5), not 20000. Real shots took it to
  exactly **1.0** and every later hit (pistol, splash, DoT) applied 0.0; no death rows and no refill until the mission's Kismet destroyed it 4.0 s after the first hit. So it is **not
  killable by damage**; the mechanism of the 1.0 floor is not identified (**UNVERIFIED**).
- Maya's controller pool slots: 0-6 ammo (Combat_Rifle, Combat_Shotgun, Grenade_Protean, Patrol_SMG, Repeater_Pistol, Rocket_Launcher, Sniper_Rifle), 7 accuracy, 8 experience, 9 and 10 the action-skill
  and melee cooldowns, 11 health, 12 shield, GUID byte = slot, 13 used: confirms NATIVE_COMBAT_GROUNDWORK section 1.10 (not re-read after a respawn).
- Crit: `CurrentInstantHitCriticalHitBonus` base 2.0 (confirms section 3.4); the live value 4.9815 = (2.0 + 0.3) x (1 + 1.0159 + 0.15): Scale modifiers add as 1 + sum.

**Kill experience (question 4): `trunc(max(10, mult x 10 x (level + 4)) x table[|d|])` fits 5 of 5 player-only kills on `Ice_P`.**
`or_xp_hook` records one row inside `WillowExperiencePipeline.AwardCombatExperienceToParty` (PRE and POST): the award passed in, whether the killer is the player, the killed pawn's
`GetExpLevel` and `ExperiencePointMultiplier`, the player's `GetExpPoints` before and after (the player's level comes from the `or_enemy` rows taken before each kill: 8, 8, then 10 after the question 5 level-ups).

| victim (class, level, multiplier) | player level | award (`CalculateExperiencePointsForKill`) | pool delta | table entry |
|---|---|---|---|---|
| baby bullymong, 6, 0.5 | 8 | 50.0 | +35 | d=2: lower 0.7 (35.0) |
| adult bullymong, 7, 1.25 | 8 | 137.5 | +123 | d=1: lower 0.9 (123.75 truncated) |
| baby bullymong, 6, 0.5 | 10 | 50.0 | +7 | d=4: lower 0.15 (7.5 truncated) |
| baby bullymong, 6, 0.5 | 10 | 50.0 | +7 | d=4 |
| regular bullymong, 6, 0.75 | 10 | 75.0 | +11 | d=4 (11.25 truncated) |

- **The award fits `max(10, mult x 10 x (level + 4))` exactly in 5 of 5** (50, 137.5, 50, 50, 75), with the multipliers the class data gives (baby Chump 0.5, regular Normal 0.75,
  ranged NormalHigh 1.0, adult Tough 1.25; `GD_AI_Balance.XP.XPMultiplier_01..04`). No crit, no damage-type factor (both lists empty), playthrough 1 so M = 10.
- **The credit fits the level-difference table of NATIVE_COMBAT_GROUNDWORK 5.2, truncated, in 5 of 5** (victim below the player: `LowerLevelEnemyExpScale` at position |d| - 1: 0.9, 0.7, 0.15 for d = 1, 2, 4).
  With lane L3's two kills (112.5 -> +45 at d=3, entry 0.4; 50.0 -> +7 at d=4) the entries d = 1 to 4 are observed. A constant factor does not fit (0.7, 0.9, 0.15). The credit happens inside
  `AwardCombatExperienceToParty` (the pool value changes between its PRE and POST rows; no script `ExpEarn`). Solo: share 100 percent, combat multiplier 1.0.
- **Not observed:** equal level (scale 1), a victim above the player (`HigherLevelEnemyExpScale`), d >= 5, more than one player, playthrough 2 and above. The fit is therefore on lower-level victims with d = 1-4 only.

**Action-skill cooldown and `ExpLevelUp(True)` (question 5).** The function's argument is `bCheated`; `True` runs `OnExpLevelChange(bFeedback true, bNaturalLevelup false)`.
- Valid run: a real Phaselock cast locked a level-5 adult (`PhaseLockTarget` row), the cooldown pool read **13.0 (its maximum) at skill age 2.0 s**, `ExpLevelUp(True)` was called inside that frame and the pool read
  **0.0 in the same call**, stayed 0 for the rest of the lock (no refill), level 9 -> 10, health refilled to full. `ActionSkillCooldownComplete` ran right after `ClientOnExpLevelChange`, as with `False`.
  The lock itself was not cut short (`ReleaseTarget` 5.71 s and skill off 6.72 s after the key, as an uncheated cast). **The cooldown reset does not depend on the flag** (`False`: lane L3).
- **Difference (new):** with `True` only the two `PlayerBehavior_LevelUp` skills ran (health refill, shield recharge boost 90.29/s for 3.99 s, the cooldown reset); `PlayerBehavior_LevelUpNaturally` did not: the equipped
  weapon's damage stayed at 41.398 for the 35 s watched (a natural level-up gives 61.9 for 30 s). So `bNaturalLevelup = not bCheated`.
- Two earlier casts fizzled (no line of sight: the player was placed behind an ice pillar): `StartActionSkill`, the cooldown draining from 13 at 1 per second, `Fizzled` after 1.01 s, `ResetSkillCooldown`, cooldown 0. The first cheat call
  landed on that already-zero pool (level 8 -> 9, same two behaviors), so it does not count as a test.

**Successive injuries and bleed-out (question 6).** Maya was downed by bullymongs (not scripted); a shotgun kill while in the targeted state gave the second wind (health 136.97 = 0.25 x 547.874 in the
kill frame, shield 162.53 = 0.9 x 180.589 the next frame, 3 of 3), then enemies downed her again.

| injury | `NumSuccessiveInjuries` (read after this injury's `BeginState`) | since the previous injury began | `TotalBleedoutTime` | targeted state to dead state |
|---|---|---|---|---|
| first (5 episodes, 3 run to the end) | 0 | over 30 s, or after a respawn | 12.0 | 12.00-12.04 s |
| second | 1 | 11.5 s | 12.0 | 12.04 s |
| third | 2 | 7.45 s | **8.0** | **8.02 s** |

- The shortened bleed-out is observed: the third injury ended 8.0 s after its targeted state, and the attribute itself read 8.0. The second did **not** shorten (12.0 s). The table's counters were read after `BeginState`, which raises the counter at its end; the counters before the three injuries were 0, 0, 1. With N as the counter before the injury, the note's `total / MaxDesiredSuccessiveInjuries x N` fits all three (12, 12, 8). The earlier "contradicted" reading applied N to the after-injury counter, which is wrong. Before-counter N = 2 gives 4 s and N = 3 the 1 s floor; neither was **observed** (no more low-health enemies), and the 30 s window edge is not tested (**UNVERIFIED**).
  VM run ([COMBAT_DOWN_RESPAWN.md](COMBAT_DOWN_RESPAWN.md)): the installed script reproduces 12 / 12 / 8 and gives 12 / 8 / 4 / 1 when the first injury falls in the first 30 s of game time (UNVERIFIED live).
- Unchanged by N: down to the targeted state 2.28 s, dead state to ragdoll 0.84 s, death camera 5.76 s. The counter read 0 again for injuries after a respawn.
- While down the shotgun fired only from the targeted state (clicks in the first 2.28 s did nothing).

**Bullymong (the Phase 4 enemy; files `local/realgame/L5b/bullymong_*.json`).** Classes in the Three Horns Divide group: adult (`CharClass_PrimalBeastAdult`, balance `PawnBalance_PrimalBeastAdult`), regular
(`CharClass_PrimalBeast`, `PawnBalance_PrimalBeast`), baby (`PawnBalance_PrimalBeast_Monglet`) and ranged (`PawnBalance_PrimalBeast_Slinger`), levels 5-7 in this session, game stage 6 for the level-7 adult.
- Health: no shield and no `ShieldArmor` pool (only slot 0 `Pool_Health`, no regeneration), `bIsInvulnerable` false, `bCanBeDamaged` true. Class starting values: health multiplier adult 6, regular 2, ranged 4, baby 0.5;
  base health per level 102.347 (5), 117.756 (6), 135.44 (7), about x1.15 a level (adult 614.1 / 812.6 at levels 5 / 7, ranged 409.4 at 5, baby 58.9 at 6).
- Attack damage on a hit to the player (arrival value; the player was not invulnerable): `8 x 1.13^level x EnemyNonWeaponDamageMultiplier` (adult 5, regular 1.5, baby 0.75, ranged 3): adult melee 73.697 / 83.278 / 94.104 at levels
  5 / 6 / 7 (`MeleeDef_PrimalBeast_Melee` and `_RadiusLeap`), regular 24.983 and baby 12.492 at 6, ranged grenade projectile 44.218 at 5. Applied to the player 0.715-0.741 of the arrival value, as in step 9 (except for 15 s after a second wind, when a "Defense Boost" scales it by 1/1.5; see the lane L6 section).
- AI: `AIDef_PrimalBeastAdult` (targeting `Targeting_ShortRange`, search radius 4096, `bHealOnReset`), controller `WillowMind`, behavior sequences `Default` (with an `OnTakeDamage` event) plus five `M_NameGame_*` mission-name sequences.
- Drops seen in eight AI deaths (five by the player) (the `DropLootOnDeath` post row always shows an empty `SpawnedInventory`; the pools are rolled from inside it): `Pool_Ammo_All_Emergency`, `Pool_Ammo_All_NeedOnly` and `Pool_Eridium_Stick` returned false, `Pool_Health_All` returned true
  twice and made a health pickup; no money or gun pool fired in this session (lane L3 saw `Pool_Money_1or2`). The adult's default pool list is `StandardEnemyGunsAndGear`.

**What this confirms or contradicts.** NATIVE_COMBAT_GROUNDWORK 5.1 and 5.2 (kill XP): confirmed for the cases above. NATIVE_DAMAGE_DEATH 10.x: the successive-injury reduction needs correcting as above. NATIVE_LEVEL_UP_ATTRIBUTES: the cooldown reset is
independent of the flag; the 30 s damage scale is natural-only. The host's Fire dummy health (20000) and its death and respawn are contradicted (first block). Hooks see only script calls: the native credit and the floor mechanism stay invisible.

**Save safety (2026-10-08).** 60 files hashed before and after (33 save and config files, the SDK settings folder, Sanity Saver's private copies). Two differed afterwards: Sanity Saver's own `Save0008.json.gz` (rewritten by that mod at load, as in lanes L3 and L5) and
`sdk_settings/python-sdk.json` (the SDK's own "Next Check Time" stamp, refreshed at startup because the stored one had expired; this lane wrote no setting). Both were restored from the backup taken first and the re-hash showed 0 of 60 different. Saves were blocked from the first frame; the game was closed through its window.

## Applied-damage ratio and the Fire dummy's floor (lane L6, 2026-10-08)

AI-assisted (Claude), lane L6. Question 7 was answered offline from lane L5b's ignored dumps (`local/realgame/L5b/combat_rows_1.json`, `player_hits.json`: 665 hits to the player) plus two live reads of data objects; question 8 needed two
launches (the first showed the floor again but read the wrong flags, the second read the right ones). Same method as lane L5b: one game process under the run lock, level-8 Maya (`-Character=Save0008.sav`), `Install-Driver -BlockSavesAtStart`, memory-only,
closed through the window. Raw rows stay under ignored `local/realgame/L6/`. Labels: **observed** or **UNVERIFIED**.

**Question 7: why some bullymong hits applied about 2/3 (or 1/2) of the formula. Cause found: the 15 s "Defense Boost" after a second wind. The earlier suspects are not it.**
- The formula is `applied = arrival x 0.65 x (1 + 0.02 x attackerLevel)` for melee and grenade hits. The odd rows (applied / arrival: adult level 5 at 0.477, level 6 at 0.364 and 0.485, ranged grenade at 0.477) are **exactly** 2/3 and 1/2 of it: ratio to the formula 0.6667 (12 hits) and 0.5000 (2 hits), every other
  unclipped melee or grenade hit 1.000 (spread 0.9999-1.0001).
- Each reduced hit follows a second wind by a kill: after `GoFromInjuredToHealthy` the game activates two skills of `GD_PlayerShared.injured.PlayerInjuredDefinition` (`SkillEffectManager.ActivateSkill` rows): `Behavior_AttributeEffect_1.SkillDefinition_0` ("Power Boost", `WeaponDamage` Scale +0.5,
  at the second-wind reason) and `Behavior_AttributeEffect_4.SkillDefinition_1` ("Defense Boost", at the frame the injured state is cleared). The Defense Boost definition, read live and in the packages (same values): duration 15 s, Scale modifier **-0.5** on each of `ReceivedBulletDamageModifier`,
  `ReceivedMeleeDamageModifier`, `ReceivedGrenadeDamageModifier` and `ReceivedRocketDamageModifier` (the player controller's attributes, base 1.0, empty modifier stacks and `bIsProtectionTimerActive` false outside the boost). Status-effect damage is not in that list.
- Three Defense Boosts were activated in the dump (game time 763.95, 927.70 and 940.19 s, all after a kill-caused second wind). Per number of boosts active (activation within the previous 15 s):

| boosts active | melee hits (unclipped) | grenade hits | applied / (arrival x 0.65 x (1 + 0.02 x level)) |
|---|---|---|---|
| 0 | 130 | 23 | 1.000 (0.9999-1.0001) |
| 1 | 8 | 4 | 0.6667 |
| 2 | 2 | 0 | 0.5000 |

  A hit is "clipped" when it took health to 0 (not used). The two-boost hits are at 0.3 s and 2.0 s after the 940.19 activation (the 927.70 one still running); the next hits, 3.1 s after 940.19 and 15.6 s after 927.70, fit one boost: the first expired between 14.5 s and 15.6 s after its activation (15 s duration).
  The fit over the three counts is `damage x 1 / (1 + 0.5 n)` for n boosts; `1 - 0.5 n` (0.5, 0.0) and a product of (1 - 0.5) (0.5, 0.25) are contradicted. Why a -0.5 Scale reads as 1/1.5 is **UNVERIFIED** (n = 3 and above not seen; bullet and rocket types not seen, only listed in the data).
- Not the cause (the data contradict them): health fraction (the 130 unboosted melee hits include 41 that began below half of maximum health, 4 that crossed half, and the lowest ended at 1.3 percent of maximum, all at ratio 1.000), a minimum-health or protection-timer cap (damage is reduced, not capped; health kept falling), crit or hit region
  (every melee hit has bone `None`), the time since the previous hit (reduced hits came 0.02-1.5 s after the previous hit, and so did many unboosted ones) and the attacker (adult, regular and baby alike, only the boost count matters). All 140 melee rows lie on the three ratios above, so no averted-death roll or other special case shows up in them (a rare one cannot be excluded).
- Consequence for the comparator (`tools/verify_combat_pawn.py`): its adult level-6 row failed because all three adult level-6 melee hits in the data are boosted hits (0.364, 0.364, 0.485); **no unboosted adult level-6 melee hit exists in the dump**, so 0.728 for that row is the formula (confirmed for regular and baby at level 6, adult at levels 5 and 7), not an observation. The comparator should leave out hits made while a boost is active.
- Not covered: the Power Boost and the other second-wind reasons (a level-up reason did not run these skills in this dump, but the player was not down then), the boost during a second injury, whether the 15 s duration restarts or stacks per activation beyond n = 2 (two overlapping activations did stack).

**Question 8: why the Fire dummy cannot be killed (floor 1.0). The `HoldAIForMatinee` candidate is contradicted; the dummy becomes demigod when its own first-hit behavior `Behavior_AIHold_59` runs.**
- Reproduced in both launches (Fire mission replayed in memory from Marcus, real shots with the lent pistol): health 778.658, 641.6, 420.5, 283.4, 62.3, then **1.0** (second launch; the hit that would have taken 62.33 below 1 set `SetHealth(1.0)` and `NotifyTakeHit` got 61.33), and every later hit arrived and applied 0 (20 of 25 hit rows). As in lane L5 the mission's Kismet destroys the dummy a few seconds after the first hit.
- `Pawn.IsDemigodMode()` on the dummy (WillowAIPawn, class `CharClass_TargetDummy`) was **False** at spawn, after reaching the range and while aiming, and **True** from the first player hit on (read every frame, one row per change: spawn, the flip, then two health changes; health reached 1.0 0.8 s after the flip). The flip is inside the
  PRE and POST rows of one call: the dummy provider's `Behavior_AIHold_59` (reason `Safe`, action 0 (`AIHold_Release` is 1), type 3), which ran in the frame of the first hit, after that hit's `SetHealth` (641.6 health) and before the frame's second hit (the splash). `Behavior_AIHold_57` (reason `Mission`, type 0), run at the dummy's spawn, changed nothing.
- The mind's own answers never changed: `GetAIDemigod`, `GetAIGod`, `GetGodMode` and `InGodMode` False before and after; `bIsInvulnerable` false, `bCanBeDamaged` true, `bUnderMatineeControl` false throughout (full scalar property dumps of the pawn and its mind before the shots and after the floor, first launch, differ only in times, `HealthVar`, pain and hit counters).
- `WillowAIPawn.MAT_BeginAIGroup`, `HoldAIForMatinee`, `MAT_FinishAIGroup`, `ReleaseAIFromMatinee` and the mind's `SetAIDemigod` and `SetAIGod` were hooked in both launches and **never ran from script** (0 rows), so the Matinee path from the plan's section 0 is not what makes this dummy demigod.
  (A native call would be invisible, but the flip is shown in the AIHold call.) That `AdjustDamage` caps a demigod pawn at 1 health (NATIVE_DAMAGE_DEATH) is consistent with the floor.
- The `AIHoldType` enum is `Target` 1, `Movement` 2, `God` 4 (bit values); type 3 here is Target plus Movement, without the `God` bit, yet the demigod answer flipped. **UNVERIFIED:** which part of the hold (the `Safe` reason or the type bits) sets it; whether releasing the hold clears it; anything outside this dummy.

**Save safety (lane L6, 2026-10-08).** 60 files hashed before and after two launches (33 save and config files, the SDK settings folder, Sanity Saver's private copies); saves blocked from the first frame, no setting written by this lane, both launches closed through the window (2 s), driver removed, lock released by this lane.
Two files differed afterwards: Sanity Saver's `Save0008.json.gz` (rewritten at load) and `sdk_settings/python-sdk.json` (only the SDK's own "Next Check Time" stamp, 2026-10-07T17:54Z to 2026-10-09T11:19Z). Both were restored from the backup taken first; the re-hash showed 0 of 60 different. The 33 save and config files were byte-identical without a restore.


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
