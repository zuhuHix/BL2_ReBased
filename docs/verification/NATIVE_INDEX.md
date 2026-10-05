# Native function index

One row per native function that has an own-words behaviour note. Notes are written by analyst lanes from local Ghidra
analysis of the executable (`docs/NATIVE_ANALYSIS.md`); implementers work from the notes only. **Status** is UNVERIFIED
unless the row says how the rule was confirmed in the running game. **Implemented** names where our code has it (empty =
not implemented yet). Ranking of what the Fire mission needs: [NATIVE_SLICE_CENSUS.md](NATIVE_SLICE_CENSUS.md).

Earlier note sets (2026-10-01..04) are listed by file, not yet row by row: [NATIVE_MISSION_DISPATCH.md](NATIVE_MISSION_DISPATCH.md)
(behavior kernel, Kismet, mission tracker objectives), [NATIVE_PROGRESSION.md](NATIVE_PROGRESSION.md),
[NATIVE_PHASELOCK_TARGETING.md](NATIVE_PHASELOCK_TARGETING.md), [NATIVE_PHASELOCK_PRESENTATION.md](NATIVE_PHASELOCK_PRESENTATION.md),
[NATIVE_WEAPON_RULES.md](NATIVE_WEAPON_RULES.md), [NATIVE_WEAPON_VISUALS.md](NATIVE_WEAPON_VISUALS.md),
[NATIVE_AMBIENT_NPC.md](NATIVE_AMBIENT_NPC.md), [NATIVE_INVENTORY_SORT.md](NATIVE_INVENTORY_SORT.md).

| Native | Note | Slice use | Status | Confidence | Implemented |
|---|---|---|---|---|---|
| MissionTracker.ActivateMission | [NATIVE_MISSION_SCRIPT_BRIDGE.md](NATIVE_MISSION_SCRIPT_BRIDGE.md) | Fire: accept (entry checks, then the status routine) | UNVERIFIED | high / medium | |
| MissionTracker.CompleteMission | [NATIVE_MISSION_SCRIPT_BRIDGE.md](NATIVE_MISSION_SCRIPT_BRIDGE.md) | Fire: turn-in (status, chain, untrack, unlock queue, fast-forward prompt) | UNVERIFIED | high / medium | |
| MissionTracker.SetMissionStatus | [NATIVE_MISSION_SCRIPT_BRIDGE.md](NATIVE_MISSION_SCRIPT_BRIDGE.md) | shared status routine, script hooks and Default event | UNVERIFIED | medium | |
| MissionTracker.PlayKickoff | [NATIVE_MISSION_SCRIPT_BRIDGE.md](NATIVE_MISSION_SCRIPT_BRIDGE.md) | Fire: fires Default id 12 (called from the tracker tick) | UNVERIFIED | high | |
| MissionTracker.PlayKickoffDialogOnly | [NATIVE_MISSION_SCRIPT_BRIDGE.md](NATIVE_MISSION_SCRIPT_BRIDGE.md) | fires Default id 13 | UNVERIFIED | high | |
| MissionTracker.PlayTurnIn | [NATIVE_MISSION_SCRIPT_BRIDGE.md](NATIVE_MISSION_SCRIPT_BRIDGE.md) | Fire: fires Default id 14 after turn-in | UNVERIFIED | high | |
| MissionTracker.SetActiveMission | [NATIVE_MISSION_SCRIPT_BRIDGE.md](NATIVE_MISSION_SCRIPT_BRIDGE.md) | Fire: writes the pending kickoff record | UNVERIFIED | medium | |
| MissionTracker.SetKickoffHeard | [NATIVE_MISSION_SCRIPT_BRIDGE.md](NATIVE_MISSION_SCRIPT_BRIDGE.md) | sets bHeardKickoff | UNVERIFIED | high | |
| MissionTracker.GetMissionStatus | [NATIVE_MISSION_SCRIPT_BRIDGE.md](NATIVE_MISSION_SCRIPT_BRIDGE.md) | status lookup | UNVERIFIED | high | |
| MissionTracker.MissionDependenciesMet | [NATIVE_MISSION_SCRIPT_BRIDGE.md](NATIVE_MISSION_SCRIPT_BRIDGE.md) | availability | UNVERIFIED | high | |
| MissionTracker.CanStartMission | [NATIVE_MISSION_SCRIPT_BRIDGE.md](NATIVE_MISSION_SCRIPT_BRIDGE.md) | availability incl. blocked test | UNVERIFIED | high | |
| MissionTracker.CanEndMission | [NATIVE_MISSION_SCRIPT_BRIDGE.md](NATIVE_MISSION_SCRIPT_BRIDGE.md) | turn-in availability | UNVERIFIED | high | |
| MissionDefinition.GetCurrencyRewardType | [NATIVE_MISSION_SCRIPT_BRIDGE.md](NATIVE_MISSION_SCRIPT_BRIDGE.md) | Fire: reward currency type | UNVERIFIED | high | |
| MissionDefinition.GetCurrencyReward | [NATIVE_MISSION_SCRIPT_BRIDGE.md](NATIVE_MISSION_SCRIPT_BRIDGE.md) | Fire: credits (0) | UNVERIFIED | medium | |
| MissionDefinition.GetOptionalCreditReward | [NATIVE_MISSION_SCRIPT_BRIDGE.md](NATIVE_MISSION_SCRIPT_BRIDGE.md) | none for Fire | UNVERIFIED | medium | |
| MissionDefinition.GetExperienceReward | [NATIVE_MISSION_SCRIPT_BRIDGE.md](NATIVE_MISSION_SCRIPT_BRIDGE.md) (formula in NATIVE_PROGRESSION.md) | Fire: XP 395 at stage 8 | amount confirmed in game 2026-10-02; call order UNVERIFIED | high | |
| MissionDefinition.ShouldGrantAlternateReward | [NATIVE_MISSION_SCRIPT_BRIDGE.md](NATIVE_MISSION_SCRIPT_BRIDGE.md) | Fire: false, normal reward | UNVERIFIED | low-medium | |
| MissionDefinition.GetItemRewardsForPlayer | [NATIVE_MISSION_SCRIPT_BRIDGE.md](NATIVE_MISSION_SCRIPT_BRIDGE.md) | Fire: empty (pool rolling is lane G1) | UNVERIFIED | medium | |
| WillowPlayerController.ExpEarn | [NATIVE_MISSION_SCRIPT_BRIDGE.md](NATIVE_MISSION_SCRIPT_BRIDGE.md) | Fire: XP into the pool | UNVERIFIED | high | |
| ExperienceResourcePool.ApplyExpPointsToExpLevel | [NATIVE_MISSION_SCRIPT_BRIDGE.md](NATIVE_MISSION_SCRIPT_BRIDGE.md) | Fire: level-up on the pool tick | UNVERIFIED | high | |
| ItemPool.SpawnBalancedInventoryFromPool | [NATIVE_LOOT.md](NATIVE_LOOT.md) | Every enemy drop, chest and mission reward roll; Fire has no stock drop | UNVERIFIED | high | |
| ItemPool.SpawnBalancedInventoryFromInventoryBalanceDefinition | [NATIVE_LOOT.md](NATIVE_LOOT.md) | Lent mission pistol, reward balances | UNVERIFIED | high | |
| (internal) balance candidate expansion (grade window, modifier interpolation) | [NATIVE_LOOT.md](NATIVE_LOOT.md) | Weight and level of every dropped item | UNVERIFIED | high | |
| InventoryBalanceDefinition.GetInventoryDefinitionForManufacturerGrade | [NATIVE_LOOT.md](NATIVE_LOOT.md) | Which weapon type or item definition a balance yields | UNVERIFIED | high | |
| InventoryBalanceDefinition.GetExpLevelFromManufacturerData | [NATIVE_LOOT.md](NATIVE_LOOT.md) | Dropped item level = min(stage, cap) | UNVERIFIED | high | |
| InventoryBalanceDefinition.GetInventoryPartListCollection | [NATIVE_LOOT.md](NATIVE_LOOT.md) | Part list used by a balance | UNVERIFIED | medium | |
| (internal) game-stage level cap | [NATIVE_LOOT.md](NATIVE_LOOT.md) | 50 in the slice | UNVERIFIED | medium | |
| ItemPoolListDefinition.AddToItemPoolList | [NATIVE_LOOT.md](NATIVE_LOOT.md) | Enemy pool list from StandardEnemyGunsAndGear | UNVERIFIED | high | |
| AIPawnBalanceDefinition.SetupPawnItemPoolList | [NATIVE_LOOT.md](NATIVE_LOOT.md) | Which pools a spawned enemy rolls | UNVERIFIED | high | |
| AIPawnBalanceDefinition.GetPlayThroughIndex | [NATIVE_LOOT.md](NATIVE_LOOT.md) | Playthrough entry (0 in the slice) | UNVERIFIED | medium | |
| MissionDefinition.GetItemRewardPools | [NATIVE_LOOT.md](NATIVE_LOOT.md) | Fire mission: empty | UNVERIFIED | high | |
| MissionDefinition.GetItemRewardsForPlayer | [NATIVE_LOOT.md](NATIVE_LOOT.md) | Reward choices: first two rolls; Fire: none | UNVERIFIED | medium | |
| AMissionTracker.GrantMissionWeapon (pool-roll side) | [NATIVE_LOOT.md](NATIVE_LOOT.md) | Level and roll of the lent Maliwan pistol | UNVERIFIED | medium | |
| WillowPawn.GetGameStageForSpawnedInventory / SetGameStageForSpawnedInventory | [NATIVE_LOOT.md](NATIVE_LOOT.md) | Enemy loot level (script rule read, accessors not) | UNVERIFIED | low | |
| BalanceModifierDefinition.GetAmmoDropsPerPlayerMultiplier | [NATIVE_LOOT.md](NATIVE_LOOT.md) | 1.0 in the slice | UNVERIFIED | medium | |
| BalanceModifierDefinition.GetUncommonChestItemPoolWeightMultiplier | [NATIVE_LOOT.md](NATIVE_LOOT.md) | Chests only; no Startup pool sets the flag | UNVERIFIED | medium | |
| BalanceModifierDefinition.GetCommonGearDropWeightBaseValue | [NATIVE_LOOT.md](NATIVE_LOOT.md) | Not in the slice path | UNVERIFIED | low | |
| ItemPool.ToggleAllItemTypesDebug / IsAllItemTypesDebugEnabled | [NATIVE_LOOT.md](NATIVE_LOOT.md) | Debug only | UNVERIFIED | medium | |
| InteractiveObjectBalanceDefinition.SetupInteractiveObjectLoot | [NATIVE_LOOT.md](NATIVE_LOOT.md) | Chests and lockers (summary) | UNVERIFIED | low | |
| WillowItem.ChooseRandomParts (weight rule only) | [NATIVE_LOOT.md](NATIVE_LOOT.md) | Shields, grenade mods, class mods | UNVERIFIED | low-medium | |
| Actor.SetTimer | [NATIVE_ENGINE_CORE.md](NATIVE_ENGINE_CORE.md) | Fire mission: UI/save/HUD timers (41 WillowGame sites), mover | UNVERIFIED | high (data model), medium (firing) | |
| Actor.ClearTimer | [NATIVE_ENGINE_CORE.md](NATIVE_ENGINE_CORE.md) | 30 WillowGame sites | UNVERIFIED | high | |
| Actor.ClearAllTimers | [NATIVE_ENGINE_CORE.md](NATIVE_ENGINE_CORE.md) | 1 site | UNVERIFIED | high | |
| Actor.PauseTimer | [NATIVE_ENGINE_CORE.md](NATIVE_ENGINE_CORE.md) | none seen | UNVERIFIED | high | |
| Actor.IsTimerActive | [NATIVE_ENGINE_CORE.md](NATIVE_ENGINE_CORE.md) | 13 sites | UNVERIFIED | high | |
| Actor.GetTimerCount | [NATIVE_ENGINE_CORE.md](NATIVE_ENGINE_CORE.md) | none seen | UNVERIFIED | high | |
| Actor.GetTimerRate | [NATIVE_ENGINE_CORE.md](NATIVE_ENGINE_CORE.md) | none seen | UNVERIFIED | high | |
| Actor.ModifyTimerTimeDilation | [NATIVE_ENGINE_CORE.md](NATIVE_ENGINE_CORE.md) | none seen | UNVERIFIED | high | |
| Actor.ResetTimerTimeDilation | [NATIVE_ENGINE_CORE.md](NATIVE_ENGINE_CORE.md) | none seen | UNVERIFIED | high | |
| Actor.Spawn | [NATIVE_ENGINE_CORE.md](NATIVE_ENGINE_CORE.md) | every dynamic actor (70 sites); event order PreBeginPlay/PostBeginPlay/SetInitialState | UNVERIFIED | medium | |
| Actor.SpawnForMap | [NATIVE_ENGINE_CORE.md](NATIVE_ENGINE_CORE.md) | ISpawnActor; not seen in script | UNVERIFIED | medium | |
| Actor.Destroy | [NATIVE_ENGINE_CORE.md](NATIVE_ENGINE_CORE.md) | 105 sites; EndState/Destroyed/UnTouch/LostChild order | UNVERIFIED | medium | |
| Actor.AllActors | [NATIVE_ENGINE_CORE.md](NATIVE_ENGINE_CORE.md) | 17 foreach sites | UNVERIFIED | high | |
| Actor.DynamicActors | [NATIVE_ENGINE_CORE.md](NATIVE_ENGINE_CORE.md) | 11 foreach sites | UNVERIFIED | high | |
| Actor.ChildActors | [NATIVE_ENGINE_CORE.md](NATIVE_ENGINE_CORE.md) | none seen | UNVERIFIED | high | |
| Actor.BasedActors | [NATIVE_ENGINE_CORE.md](NATIVE_ENGINE_CORE.md) | none seen | UNVERIFIED | high | |
| Actor.TouchingActors | [NATIVE_ENGINE_CORE.md](NATIVE_ENGINE_CORE.md) | 5 sites | UNVERIFIED | high | |
| Actor.VisibleActors | [NATIVE_ENGINE_CORE.md](NATIVE_ENGINE_CORE.md) | none seen | UNVERIFIED | medium | |
| Actor.VisibleCollidingActors | [NATIVE_ENGINE_CORE.md](NATIVE_ENGINE_CORE.md) | none seen | UNVERIFIED | low | |
| Actor.CollidingActors | [NATIVE_ENGINE_CORE.md](NATIVE_ENGINE_CORE.md) | 6 sites | UNVERIFIED | medium | |
| Actor.OverlappingActors | [NATIVE_ENGINE_CORE.md](NATIVE_ENGINE_CORE.md) | none seen | UNVERIFIED | medium | |
| Actor.LocalPlayerControllers | [NATIVE_ENGINE_CORE.md](NATIVE_ENGINE_CORE.md) | 69 sites; HUD/menu code iterates it | UNVERIFIED | high | |
| Actor.AllOwnedComponents | [NATIVE_ENGINE_CORE.md](NATIVE_ENGINE_CORE.md) | none seen | UNVERIFIED | high | |
| Actor.GetALocalPlayerController | [NATIVE_ENGINE_CORE.md](NATIVE_ENGINE_CORE.md) | 7 sites | UNVERIFIED | high | |
| WorldInfo.GetWorldInfo | [NATIVE_ENGINE_CORE.md](NATIVE_ENGINE_CORE.md) | persistent level WorldInfo; Kismet/behavior lookups | UNVERIFIED | high | |
| SequenceObject.GetWorldInfo | [NATIVE_ENGINE_CORE.md](NATIVE_ENGINE_CORE.md) | Kismet sequence ops; shared with BehaviorBase.GetWorldInfo | UNVERIFIED | high | |
| Object.GotoState | [NATIVE_ENGINE_CORE.md](NATIVE_ENGINE_CORE.md) | 125 sites; state frame, EndState/BeginState, default label Begin | UNVERIFIED | medium | |
| Object.PushState | [NATIVE_ENGINE_CORE.md](NATIVE_ENGINE_CORE.md) | 4 sites; Paused/Pushed events | UNVERIFIED | medium | |
| Object.PopState | [NATIVE_ENGINE_CORE.md](NATIVE_ENGINE_CORE.md) | Popped/Continued events | UNVERIFIED | medium | |
| Object.IsInState | [NATIVE_ENGINE_CORE.md](NATIVE_ENGINE_CORE.md) | 21 sites | UNVERIFIED | high | |
| Object.GetStateName | [NATIVE_ENGINE_CORE.md](NATIVE_ENGINE_CORE.md) | 20 sites | UNVERIFIED | high | |
| Object.Enable | [NATIVE_ENGINE_CORE.md](NATIVE_ENGINE_CORE.md) | probe mask (Tick/Touch/Destroyed gating); VM currently no-op | UNVERIFIED | high | |
| Object.Disable | [NATIVE_ENGINE_CORE.md](NATIVE_ENGINE_CORE.md) | probe mask; VM currently no-op | UNVERIFIED | high | |
| Actor.Sleep | [NATIVE_ENGINE_CORE.md](NATIVE_ENGINE_CORE.md) | latent wait in state code (LatentFloat poll, half-delta wake) | UNVERIFIED | high | |
| MissionTracker.RegisterMissionObserver | [NATIVE_BEHAVIOR_POPULATION.md](NATIVE_BEHAVIOR_POPULATION.md) | Fire: conditions and den aspect observe the Fire mission | UNVERIFIED | medium | |
| MissionTracker.UnregisterMissionObserver | [NATIVE_BEHAVIOR_POPULATION.md](NATIVE_BEHAVIOR_POPULATION.md) | Fire: unlink of the dummy's conditions (not read separately) | UNVERIFIED | low | |
| (tracker) NotifyMissionObservers kinds 0-5 | [NATIVE_BEHAVIOR_POPULATION.md](NATIVE_BEHAVIOR_POPULATION.md) | Fire: when observers are told (status, set, objective updated/cleared/complete, level load) | UNVERIFIED | high | |
| BehaviorSequenceEnableByMission.MissionReactionLevelLoad | [NATIVE_BEHAVIOR_POPULATION.md](NATIVE_BEHAVIOR_POPULATION.md) | Fire: initial enable state of FireDamage on spawn | UNVERIFIED | high | |
| BehaviorSequenceEnableByMission.MissionReactionStatusChanged | [NATIVE_BEHAVIOR_POPULATION.md](NATIVE_BEHAVIOR_POPULATION.md) | Fire: re-evaluation on accept / turn-in | UNVERIFIED | high | |
| BehaviorSequenceEnableByMission.MissionReactionObjectiveSetChanged | [NATIVE_BEHAVIOR_POPULATION.md](NATIVE_BEHAVIOR_POPULATION.md) | Fire: re-evaluation when RocksPaper_FinalObj activates | UNVERIFIED | high | |
| BehaviorSequenceEnableByMission.MissionReactionObjectiveUpdated | [NATIVE_BEHAVIOR_POPULATION.md](NATIVE_BEHAVIOR_POPULATION.md) | Fire: re-evaluation on progress | UNVERIFIED | high | |
| BehaviorSequenceEnableByMission.MissionReactionObjectiveCleared | [NATIVE_BEHAVIOR_POPULATION.md](NATIVE_BEHAVIOR_POPULATION.md) | Fire: re-evaluation on clear | UNVERIFIED | high | |
| BehaviorSequenceEnableByMission.MissionReactionObjectiveComplete | [NATIVE_BEHAVIOR_POPULATION.md](NATIVE_BEHAVIOR_POPULATION.md) | Fire: FireDamage disables when Fire completes | UNVERIFIED | high | |
| (BehaviorSequenceEnableByMission C++ virtuals: link/unlink/verdict/hooks) | [NATIVE_BEHAVIOR_POPULATION.md](NATIVE_BEHAVIOR_POPULATION.md) | Fire: observer registration lifecycle, verdict rule | UNVERIFIED | medium | |
| BehaviorKernel.ChangeBehaviorSequenceActivationStatus | [NATIVE_BEHAVIOR_POPULATION.md](NATIVE_BEHAVIOR_POPULATION.md) | Fire: enable(1)/disable(2)/toggle(0), sequence mutex, enabled/disabled events | UNVERIFIED | medium | |
| BehaviorKernel.IntializeBehaviorProviderForConsumer | [NATIVE_BEHAVIOR_POPULATION.md](NATIVE_BEHAVIOR_POPULATION.md) | Fire: provider registration order (pass 1 enabled-on-spawn, pass 2 conditions) | UNVERIFIED | medium | |
| SequenceEventEnableByMission.MissionReactionLevelLoad | [NATIVE_BEHAVIOR_POPULATION.md](NATIVE_BEHAVIOR_POPULATION.md) | Kismet twin: sets SequenceEvent.bEnabled, calls Toggled | UNVERIFIED | medium | |
| SequenceEventEnableByMission.MissionReactionStatusChanged | [NATIVE_BEHAVIOR_POPULATION.md](NATIVE_BEHAVIOR_POPULATION.md) | Kismet twin | UNVERIFIED | medium | |
| SequenceEventEnableByMission.MissionReactionObjectiveSetChanged | [NATIVE_BEHAVIOR_POPULATION.md](NATIVE_BEHAVIOR_POPULATION.md) | Kismet twin | UNVERIFIED | medium | |
| SequenceEventEnableByMission.MissionReactionObjectiveUpdated | [NATIVE_BEHAVIOR_POPULATION.md](NATIVE_BEHAVIOR_POPULATION.md) | Kismet twin | UNVERIFIED | medium | |
| SequenceEventEnableByMission.MissionReactionObjectiveCleared | [NATIVE_BEHAVIOR_POPULATION.md](NATIVE_BEHAVIOR_POPULATION.md) | Kismet twin | UNVERIFIED | medium | |
| SequenceEventEnableByMission.MissionReactionObjectiveComplete | [NATIVE_BEHAVIOR_POPULATION.md](NATIVE_BEHAVIOR_POPULATION.md) | Kismet twin | UNVERIFIED | medium | |
| MissionPopulationAspect (script: Initialize, SetActivationFromMission, MissionReaction*, OnSpawnActor) | [NATIVE_BEHAVIOR_POPULATION.md](NATIVE_BEHAVIOR_POPULATION.md) | Fire: den enabled while Fire objective active | UNVERIFIED | high | |
| PopulationOpportunity.SetEnabledStatus | [NATIVE_BEHAVIOR_POPULATION.md](NATIVE_BEHAVIOR_POPULATION.md) | Fire: enabling the dummy's den (notifies aspects only) | UNVERIFIED | high | |
| PopulationOpportunity.RespawnKilledActors (Den) | [NATIVE_BEHAVIOR_POPULATION.md](NATIVE_BEHAVIOR_POPULATION.md) | Fire: re-arms the den total on (re)activation | UNVERIFIED | high | |
| PopulationOpportunity.DoSpawning (Den) + master tick gate | [NATIVE_BEHAVIOR_POPULATION.md](NATIVE_BEHAVIOR_POPULATION.md) | Fire: one pawn per tick at WillowPopulationPoint_40 when a player is in range | UNVERIFIED | medium | |
| PopulationMaster.SpawnPopulationControlledActor / SpawnActorFromOpportunity | [NATIVE_BEHAVIOR_POPULATION.md](NATIVE_BEHAVIOR_POPULATION.md) | Fire: pawn creation order (engine spawn -> controller -> providers -> OnSpawned -> setup -> aspect OnSpawnActor) | UNVERIFIED | medium | |
| PopulationFactory.CanSpawn / GetSpawnProbabilityAtThisGameStage (BalancedAIPawn) | [NATIVE_BEHAVIOR_POPULATION.md](NATIVE_BEHAVIOR_POPULATION.md) | Fire: candidate weight for PopDef_TargetDummy (always 1 for AIPawnBalanceDefinition) | UNVERIFIED | medium | |
| WillowAIPawn.InitializeBehaviorProviders | [NATIVE_BEHAVIOR_POPULATION.md](NATIVE_BEHAVIOR_POPULATION.md) | Fire: registers consumer and the dummy's class provider (conditions applied there) | UNVERIFIED | medium | |
| AIClassDefinition.OnSpawned | [NATIVE_BEHAVIOR_POPULATION.md](NATIVE_BEHAVIOR_POPULATION.md) | Fire: OnSpawned event to the dummy's class provider | UNVERIFIED | high | |
| AIDefinition.OnSpawned | [NATIVE_BEHAVIOR_POPULATION.md](NATIVE_BEHAVIOR_POPULATION.md) | Fire: OnSpawned event to the AIDef provider | UNVERIFIED | high | |
| BehaviorKernel thread runner (enabled check, context objects, latent copy) | [NATIVE_BEHAVIOR_POPULATION.md](NATIVE_BEHAVIOR_POPULATION.md) | Fire: sequence-disable stops threads; context rule; latent waits | UNVERIFIED | medium | |
| BehaviorKernel event activation FilterObject | [NATIVE_BEHAVIOR_POPULATION.md](NATIVE_BEHAVIOR_POPULATION.md) | Fire: filters are consulted only with a caller callback; none seen for tracker/OnSpawned | UNVERIFIED | medium | |
