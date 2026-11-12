# The stock death chain on the VM: a bullymong killed by the player (plan step S-c, 2026-10-08)

AI-assisted (Claude), implementer lane Sc. Worked from the own-words notes [NATIVE_DAMAGE_DEATH.md](NATIVE_DAMAGE_DEATH.md) (section 9),
[NATIVE_COMBAT_GROUNDWORK.md](NATIVE_COMBAT_GROUNDWORK.md) (sections 4 and 6), the live record [COMBAT_GOLDEN_TRACE.md](COMBAT_GOLDEN_TRACE.md) (section A step 5) and the
installed script (`research/script_disasm.py`). No decompiler output was read. Every rule below is `UNVERIFIED` as a statement about the native code unless a row says which
live observation it matches; "matches" means the VM's order or number equals the recorded live one, not that the native does exactly this.

## What runs where

`ow-package <any.upk> --combat-run --cooked <CookedPCConsole> spawn:adult:5 player:8 sethealth:0:29.7 hit:0:40:GD_Impact.DamageType.DmgType_Normal:Shotgun` is the live L3
killing pellet with an invented amount (the adult bullymong at 29.7 health loses 40 and reads -10.3 after `SetHealth`). The live pellet was her own
gun's damage, which is in the maintainer's private record, and the runs of 2026-10-08 used it. A rerun on 2026-11-12 with both amounts gave the same
output apart from the amounts and the run times (15 of 434 values differ: the pellet in the step, two rows and six events, `SetHealth`, three times). The pawn's **installed script** does everything between that and the
presentation: `TakeDamage` ... `SetHealth` -> `NotifyTakeHit` -> `TookDamageFromEnemy` -> `WillowPawn.HandleHealthDepleted` -> `MissionTracker.NotifyPawnDied` ->
`Pawn.HandleHealthDepleted` -> `WillowAIPawn.Died` -> (the killer's) `WillowPlayerPawn.KilledEnemy` -> `WillowPawn.Died` -> `DropLootOnDeath` -> `StatusEffectsComponent.OwnerDied` ->
`BodyClassDeathDefinition.OnKilledBy` -> `GearboxPawn.Died` -> `Pawn.Died` (`SetHealth(0)`, `DestroyHealthPool`) -> `WillowGameInfo.Killed` (kill experience, plan step S-d) ->
`GameInfo.Killed` -> `WillowAIPawn.PlayDying` -> `WillowPawn.PlayDying` -> `OnPlayDeathPizazz` -> `CheckInjured`. What this lane adds is the world the script needs and the
natives under it (`src/combat_death.cpp`, `src/combat_run.cpp`). `HitReport.order` (and the `order` array of the hit step in the JSON) is a new call-order trace of the VM
(`Runtime::callTrace`, one pointer, filled where every call already goes through `Interp::invoke`): the earlier hit-chain record could only list scripts as an alphabetical census.

## The world and the stand-ins

| What | Why | Status |
|---|---|---|
| `WorldInfo.Game` = a `WillowGameInfo` instance at its class defaults | the installed `Pawn.Died` begins with a test of `WorldInfo.Game` and returns false without one, and the kill credit is script on it (`WillowGameInfo.Killed`) | the live game has one (the live pellets run `GameInfo.ReduceDamage`); the class is not known to be the exact live one (a coop subclass may be) |
| `WorldInfo.GRI` = a `WillowGameReplicationInfo` with a `MissionTracker` (only when the world has none; the mission bridge's own is kept) | `WillowPawn.HandleHealthDepleted` reaches the tracker through it | UNVERIFIED stand-in |
| an inventory manager on the pawn (an instance of its `InventoryManagerClass`, empty) | `WillowPawn.Died` calls `DropLootOnDeath` only for a pawn that has one | stand-in; the inventory stays empty (the loot of step S-g is records, [COMBAT_LOOT.md](COMBAT_LOOT.md)) |
| the pawn's `StatusEffectComp` and `SMComponent` built from their archetype exports | the archetype holds references; the script calls them (`OwnerDied`, `StopAny`) | as `placeMarcus` builds the dialog component |
| `MyDeathDef` = the body class's `DeathDef`, a consumer handle (`ConsumerHandle.PID`, first value 100) | the stock spawn sets both (`SetDeathDefinition`, `InitializeBehaviorProviders`) | the PID is a counter of this unit, not the game's allocator |
| `Physics` = `PHYS_Walking` when the archetype has 0 | the bullymong class provider's `Behavior_Conditional` tests the physics state; the live pawn was standing on the ground | UNVERIFIED stand-in for the landing that sets it |

`enableDeaths()` is called by the `--combat-run` driver only. The slice (`--slice-run`, the host) does not call it, so its dummy is unchanged. A package set without the classes (a
synthetic one) has no deaths and nothing changes for it.

## Natives bound for the chain (`src/combat_death.cpp`)

| Native | What it does here | Status |
|---|---|---|
| `Pawn.IsHumanControlled` | true when the pawn's `Controller` is a player controller | UNVERIFIED reading of the name |
| `MissionTracker.NotifyPawnDied` | NATIVE_COMBAT_GROUNDWORK 4.2: defend entries, the pawn's own objective, the objectives registered under `KilledStatID`. The VM's tracker registers none of the latter and has no defend entry, so it logs the call and the stat name; a pawn with a `MissionObjectiveToUpdateOnDeath` or a non-empty `DefendMissions` is reported as not implemented | UNVERIFIED; no observable effect in the VM |
| `BodyClassDeathDefinition.OnKilledBy / OnPlayDeathPizazz / OnDeathNonGib / OnDeathGib` | find the pawn by the consumer handle and ask the owner of the providers to raise the event on the death definition's provider | UNVERIFIED |
| `BehaviorKernel.BroadcastBehaviorEventFromScript`, `WillowPawn.GetBehaviorConsumerHandle` | a script behavior (`Behavior_CustomEvent`) raises a named event on every provider of the consumer | UNVERIFIED |
| `BehaviorKernel.ActivateBehaviorOutputLink` | appends the id to the running script behavior's output list | as the mission bridge's own binding |
| `ExpressionTree / CompoundExpressionEvaluator / PhysicsStateExpressionEvaluator .Evaluate` | the data's plain reading: a tree evaluates its root; a compound combines its two expressions by its operator enum (`CEOP_Or` / `CEOP_And`); a physics-state expression compares the context pawn's `Physics` with `PhysicsState` (inverted when `bIsInState` is false) | UNVERIFIED, no note; bodies unread. Needed by `Behavior_Conditional`, which now selects the link to `Behavior_SpecialMove` as the live pellets did |
| `StatusEffectsComponent.OwnerDied`, `PawnAllegiance.RemoveParent / RemoveChildren` | logged no-ops (no status effects or allegiance hierarchy in the VM) | presentation / bookkeeping |
| `WillowPawn.StopSounds / PlayPainSound`, `WillowAIPawn.DropCombatPerch / SetVehicleRBActorEnabled`, `Pawn.PlayDyingSound`, `PrimitiveComponent.SetRBChannel / SetRBCollidesWithChannel`, `SpecialMoveComponent.StopAny` | no-ops | presentation |
| `Pawn.InitRagdoll` | true (the host builds the ragdoll) | presentation |
| `Actor.Destroy` | marks `bDeleteMe`; its `Destroyed` event and the removal belong to the host | UNVERIFIED |
| `ResourcePool.GetCurrentValue` | the pool's `CurrentValue` (`DestroyHealthPool` copies it into `HealthVar` before deleting the pool) | trivial |
| `WillowAIPawn.GetAwesomeLevel / GetGameStageForSpawnedInventory` | field reads | plan step S-g owns what they mean |
| `WillowPawn.DropLootOnDeath` (script, overridden) | the pawn's pools are rolled (plan step S-g, [COMBAT_LOOT.md](COMBAT_LOOT.md)); this lane (S-c) logged the call and rolled nothing | S-g replaced the stub |

The driver (`src/combat_run.cpp`) makes the death definition's provider per pawn like the class provider, runs the installed scripts of `Behavior_Conditional`, `Behavior_CustomEvent`
and `Behavior_CompareBool` on the VM, and lists the behaviors that spawn effects, projectiles, sounds and dialog lines at the boundary (reported, not run).

## The death order next to the live one

Live (COMBAT_GOLDEN_TRACE.md section A step 5, the killing pellet; hook rows in one frame) against the VM (`order` of the hit step, position in the 220 calls of the chain in a `maya:` run with
the kill experience of [COMBAT_KILL_XP.md](COMBAT_KILL_XP.md) bound: without it the calls after `AwardCombatExperienceToParty` are one earlier, as `ExpEarn` is not called; the VM
also runs calls the live hook set does not list, so only the order of the listed ones is compared):

| Live | VM position |
|---|---|
| `TookDamageFromEnemy` | 45 |
| `WillowPawn.HandleHealthDepleted` | 69 |
| `MissionTracker.NotifyPawnDied` | 71 |
| `Pawn.HandleHealthDepleted` | 72 |
| `WillowAIPawn.Died` | 75 |
| the killer's `WillowPlayerPawn.KilledEnemy` | 77 |
| `WillowPawn.Died` | 89 |
| `WillowPawn.DropLootOnDeath` (first thing in `Died`) | 94 |
| `StatusEffectsComponent.OwnerDied` | 95 |
| `BodyClassDeathDefinition.OnKilledBy` | 98 |
| `Pawn.Died` | 108 |
| `Pawn.SetHealth` (0) | 112 |
| `WillowGameInfo.Killed` | 120 |
| `CalculateExperiencePointsForKill` | 123 |
| `WillowGameInfo.AwardCombatExperience` | 124 |
| `AwardCombatExperienceToParty` | 125 |
| `GameInfo.Killed` | 132 |
| `WillowAIPawn.PlayDying` | 149 |
| `WillowPawn.PlayDying` | 151 |
| `BodyClassDeathDefinition.OnPlayDeathPizazz` | 156 |
| `CheckInjured` | 218 |

All 21 listed calls occur in the live order (`local/p2/Sc/death_order.py`, a local check, prints this table from a run). `AIDefinition.OnKilledPawn` does not run for the player's kill (the same
as live). The pre-death part of the chain (`TakeDamage` ... `SetHealth` below 0 ... `NotifyTakeHit` ... `TookDamageFromEnemy`) is the Sb record. Differences from the live record that are
not in the table: `OnKilledBy` ran **three** behaviors as live (`Behavior_CustomEvent_1`, `Behavior_CompareBool_0`, `Behavior_ChangeInstanceDataSwitch_0`); `OnPlayDeathPizazz` ran
**three** here (`Behavior_CustomEvent_0` -> the broadcast `DeathBreathFX` -> `Behavior_ChangeRemoteBehaviorSequenceState_0`, and `Behavior_TriggerDialogEvent_0`) against four live, the last a
dialog event as here; the missing one is not identified (it may sit in the AI definition's provider, whose `FrostBreth_FX` sequence the VM does not load for the death).

Post-state after the chain (state of the step): health 0 (`HealthVar` 0: `DestroyHealthPool` copied the pool's value, then deleted the pool), `HealthPool` the invalid reference (slot 255,
GUID 255, `pool_valid` false), `GetMaxHealth` falls back to `HealthMaxVar`. The live post-state was a hook-row observation of the same order; the pool deletion is the script's
`DestroyHealthPool` over the G24 natives (Sa), unchanged.

## Not modelled, and things to know

- **`WillowAIPawn.CheckInjured`** (the bullymong's crawl at 20 percent health, `InjuredHealthPct` 0.2) is still a logged stub; it runs after the death as live, and its effect on a live
  pawn is unread.
- **Hit region:** the driver sends no bone, so the body class's `DefaultHitRegion` (`HitRegion_Body`) is used; the first hit of a run says so in its events. No head hit, no crit row.
- **Kill-stat objectives:** the tracker has no registry, so a pawn killed while an objective keyed by `STAT_PLAYER_KILLS_BULLYMONG` is active would not progress it here (logged, not silent
  for the pawn's own objective and for defend entries; the stat registry is not read).
- **Loot** (`SetupPawnItemPoolList`, `SpawnBalancedInventoryFromPool`) was plan step S-g and is done: [COMBAT_LOOT.md](COMBAT_LOOT.md). `DropLootOnDeath` is overridden by the unit's roll, not run as script.
- **Skipped iterators** (`foreach` over an engine iterator the VM does not implement; each is logged by the stub line and now named in the call trace): `WillowDamagePipeline.DamageDealtSkillEventsTriggered`
  (in `ProcessDamageDealtSkillEvents`, the player's skill events), `Actor.ComponentList`, `WorldInfo.AllControllers` (in `GameInfo.NotifyKilled`), `InventoryManager.InventoryActors`,
  `WillowInventoryManager.ItemActors`. This answers the plan's "unnamed iterator" for the hit and death chain.
- **Still stubs on the chain** (visible in the run's `stubs`): the Sb list (`AddHitLocation`, `GetParent`, `NotifyAttackedBy`, the pipeline getters, ...), `GameInfo.IsFriendlyFire`,
  `WillowGameInfo.GetSkillManager / UpdateTookDamageFromEnemyStats / TravelPostCountdownInProcess / GetCombatMusicManager`, `Pawn.GetController`, `Actor.SetTimer` (the corpse timers: the VM has no
  timers), `SetLocation / SetOwner / SetCollision`, `PlayTechDeath`, `GetTechDeathIndexForDamageType`, `TryFullBodyGib`, `WillowMind.ActivateEventRule`, `AIComponent.SetTarget`,
  `WillowAIComponent.ClearState`, `StatusEffectsComponent.ClearDenStats`, `RunBehaviors` (empty list in the conditional), `IsChampion`, `IsCharmed`.
- Giving the world a game also lets the earlier hit chain run its game-side scripts (`GameInfo.ReduceDamage`, `IsFriendlyFire`): the Sb L5 sequence (pistol, splash, floor 1.0) and the
  L3 pellets give the same numbers (`local/p2/Sc/handoff.md`).

## Reproduce

```
cooked="$OPENWILLOW_BL2/WillowGame/CookedPCConsole"
build/Release/ow-package.exe "$cooked/Sanctuary_Dynamic.upk" --combat-run --cooked "$cooked" spawn:adult:5 player:8 sethealth:0:29.7 \
    hit:0:40:GD_Impact.DamageType.DmgType_Normal:Shotgun > kill.json
python tests/combat_test.py build/Release/ow-package.exe     # synthetic: the toy killing pawn (call order, tracker native, post-state)
```
