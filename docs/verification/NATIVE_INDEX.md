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
| MissionTracker.ActivateMission | [NATIVE_MISSION_SCRIPT_BRIDGE.md](NATIVE_MISSION_SCRIPT_BRIDGE.md) | Fire: accept (entry checks, then the status routine) | UNVERIFIED | high / medium | src/mission_script.cpp (calls MissionSystem::accept); Fire case |
| MissionTracker.CompleteMission | [NATIVE_MISSION_SCRIPT_BRIDGE.md](NATIVE_MISSION_SCRIPT_BRIDGE.md) | Fire: turn-in (status, chain, untrack, unlock queue, fast-forward prompt) | UNVERIFIED | high / medium | src/mission_script.cpp (MissionSystem::turnInMission); chain, untrack, unlock queue and fast-forward not modelled |
| MissionTracker.SetMissionStatus | [NATIVE_MISSION_SCRIPT_BRIDGE.md](NATIVE_MISSION_SCRIPT_BRIDGE.md) | shared status routine, script hooks and Default event | UNVERIFIED | medium | src/mission.cpp MissionSystem::setStatus (Active, ReadyToTurnIn, Complete; script hook before the Default event) |
| MissionTracker.PlayKickoff | [NATIVE_MISSION_SCRIPT_BRIDGE.md](NATIVE_MISSION_SCRIPT_BRIDGE.md) | Fire: fires Default id 12 (called from the tracker tick) | UNVERIFIED | high | src/mission.cpp MissionSystem::tick (pending record) |
| MissionTracker.PlayKickoffDialogOnly | [NATIVE_MISSION_SCRIPT_BRIDGE.md](NATIVE_MISSION_SCRIPT_BRIDGE.md) | fires Default id 13 | UNVERIFIED | high | src/mission.cpp MissionSystem::tick (pending record) |
| MissionTracker.PlayTurnIn | [NATIVE_MISSION_SCRIPT_BRIDGE.md](NATIVE_MISSION_SCRIPT_BRIDGE.md) | Fire: fires Default id 14 after turn-in | UNVERIFIED | high | src/mission_script.cpp (MissionSystem::playTurnIn) |
| MissionTracker.SetActiveMission | [NATIVE_MISSION_SCRIPT_BRIDGE.md](NATIVE_MISSION_SCRIPT_BRIDGE.md) | Fire: writes the pending kickoff record | UNVERIFIED | medium | src/mission.cpp (the pending kickoff record only; tracked-mission choice and gate not modelled) |
| MissionTracker.SetKickoffHeard | [NATIVE_MISSION_SCRIPT_BRIDGE.md](NATIVE_MISSION_SCRIPT_BRIDGE.md) | sets bHeardKickoff | UNVERIFIED | high | |
| MissionTracker.GetMissionStatus | [NATIVE_MISSION_SCRIPT_BRIDGE.md](NATIVE_MISSION_SCRIPT_BRIDGE.md) | status lookup | UNVERIFIED | high | src/mission_script.cpp |
| MissionTracker.MissionDependenciesMet | [NATIVE_MISSION_SCRIPT_BRIDGE.md](NATIVE_MISSION_SCRIPT_BRIDGE.md) | availability | UNVERIFIED | high | |
| MissionTracker.CanStartMission | [NATIVE_MISSION_SCRIPT_BRIDGE.md](NATIVE_MISSION_SCRIPT_BRIDGE.md) | availability incl. blocked test | UNVERIFIED | high | |
| MissionTracker.CanEndMission | [NATIVE_MISSION_SCRIPT_BRIDGE.md](NATIVE_MISSION_SCRIPT_BRIDGE.md) | turn-in availability | UNVERIFIED | high | |
| MissionDefinition.GetCurrencyRewardType | [NATIVE_MISSION_SCRIPT_BRIDGE.md](NATIVE_MISSION_SCRIPT_BRIDGE.md) | Fire: reward currency type | UNVERIFIED | high | src/mission_script.cpp |
| MissionDefinition.GetCurrencyReward | [NATIVE_MISSION_SCRIPT_BRIDGE.md](NATIVE_MISSION_SCRIPT_BRIDGE.md) | Fire: credits (0) | UNVERIFIED | medium | src/mission_script.cpp (multiplier 0 and other currencies; credits formula not implemented) |
| MissionDefinition.GetOptionalCreditReward | [NATIVE_MISSION_SCRIPT_BRIDGE.md](NATIVE_MISSION_SCRIPT_BRIDGE.md) | none for Fire | UNVERIFIED | medium | |
| MissionDefinition.GetExperienceReward | [NATIVE_MISSION_SCRIPT_BRIDGE.md](NATIVE_MISSION_SCRIPT_BRIDGE.md) (formula in NATIVE_PROGRESSION.md) | Fire: XP 395 at stage 8 | amount confirmed in game 2026-10-02; call order UNVERIFIED | high | src/mission_script.cpp + src/progression.* (formula, first playthrough below level 50) |
| MissionDefinition.ShouldGrantAlternateReward | [NATIVE_MISSION_SCRIPT_BRIDGE.md](NATIVE_MISSION_SCRIPT_BRIDGE.md) | Fire: false, normal reward | UNVERIFIED | low-medium | src/mission_script.cpp (exercised only with the empty progress the stubbed GetObjectivesProgress gives) |
| MissionDefinition.GetItemRewardsForPlayer | [NATIVE_MISSION_SCRIPT_BRIDGE.md](NATIVE_MISSION_SCRIPT_BRIDGE.md) | Fire: empty (pool rolling is lane G1) | UNVERIFIED | medium | src/mission_script.cpp (empty rewards only; pool rolls not implemented) |
| WillowPlayerController.ExpEarn | [NATIVE_MISSION_SCRIPT_BRIDGE.md](NATIVE_MISSION_SCRIPT_BRIDGE.md) | Fire: XP into the pool | UNVERIFIED | high | src/mission_script.cpp (VM-side pool; scales taken as 1) |
| ExperienceResourcePool.ApplyExpPointsToExpLevel | [NATIVE_MISSION_SCRIPT_BRIDGE.md](NATIVE_MISSION_SCRIPT_BRIDGE.md) | Fire: level-up on the pool tick | UNVERIFIED | high | src/mission_script.cpp MissionScript::updateExperiencePool (C++ state, run from FireMissionSlice::tick) |
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
| MissionDefinition.GetItemRewardsForPlayer | [NATIVE_LOOT.md](NATIVE_LOOT.md) | Reward choices: first two rolls; Fire: none | UNVERIFIED | medium | src/mission_script.cpp (empty rewards only; pool rolls not implemented) |
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
| MissionTracker.RegisterMissionObserver | [NATIVE_BEHAVIOR_POPULATION.md](NATIVE_BEHAVIOR_POPULATION.md) | Fire: conditions and den aspect observe the Fire mission | UNVERIFIED | medium | src/slice.cpp FireMissionSlice::spawnDummy (the dummy only; the immediate LevelLoad verdict) |
| MissionTracker.UnregisterMissionObserver | [NATIVE_BEHAVIOR_POPULATION.md](NATIVE_BEHAVIOR_POPULATION.md) | Fire: unlink of the dummy's conditions (not read separately) | UNVERIFIED | low | |
| (tracker) NotifyMissionObservers kinds 0-5 | [NATIVE_BEHAVIOR_POPULATION.md](NATIVE_BEHAVIOR_POPULATION.md) | Fire: when observers are told (status, set, objective updated/cleared/complete, level load) | UNVERIFIED | high | src/mission.cpp MissionSystem::Notification (kinds 1-3 and 5; cleared not modelled; 0 raised at registration) |
| BehaviorSequenceEnableByMission.MissionReactionLevelLoad | [NATIVE_BEHAVIOR_POPULATION.md](NATIVE_BEHAVIOR_POPULATION.md) | Fire: initial enable state of FireDamage on spawn | UNVERIFIED | high | src/slice.cpp conditionHolds/applyConditions (one shared verdict for all six) |
| BehaviorSequenceEnableByMission.MissionReactionStatusChanged | [NATIVE_BEHAVIOR_POPULATION.md](NATIVE_BEHAVIOR_POPULATION.md) | Fire: re-evaluation on accept / turn-in | UNVERIFIED | high | src/slice.cpp conditionHolds/applyConditions (one shared verdict for all six) |
| BehaviorSequenceEnableByMission.MissionReactionObjectiveSetChanged | [NATIVE_BEHAVIOR_POPULATION.md](NATIVE_BEHAVIOR_POPULATION.md) | Fire: re-evaluation when RocksPaper_FinalObj activates | UNVERIFIED | high | src/slice.cpp conditionHolds/applyConditions (one shared verdict for all six) |
| BehaviorSequenceEnableByMission.MissionReactionObjectiveUpdated | [NATIVE_BEHAVIOR_POPULATION.md](NATIVE_BEHAVIOR_POPULATION.md) | Fire: re-evaluation on progress | UNVERIFIED | high | src/slice.cpp conditionHolds/applyConditions (one shared verdict for all six) |
| BehaviorSequenceEnableByMission.MissionReactionObjectiveCleared | [NATIVE_BEHAVIOR_POPULATION.md](NATIVE_BEHAVIOR_POPULATION.md) | Fire: re-evaluation on clear | UNVERIFIED | high | src/slice.cpp conditionHolds/applyConditions (one shared verdict for all six) |
| BehaviorSequenceEnableByMission.MissionReactionObjectiveComplete | [NATIVE_BEHAVIOR_POPULATION.md](NATIVE_BEHAVIOR_POPULATION.md) | Fire: FireDamage disables when Fire completes | UNVERIFIED | high | src/slice.cpp conditionHolds/applyConditions (one shared verdict for all six) |
| (BehaviorSequenceEnableByMission C++ virtuals: link/unlink/verdict/hooks) | [NATIVE_BEHAVIOR_POPULATION.md](NATIVE_BEHAVIOR_POPULATION.md) | Fire: observer registration lifecycle, verdict rule | UNVERIFIED | medium | src/slice.cpp (verdict, objective states and restrictions; waypoints and bInstanced not modelled) |
| BehaviorKernel.ChangeBehaviorSequenceActivationStatus | [NATIVE_BEHAVIOR_POPULATION.md](NATIVE_BEHAVIOR_POPULATION.md) | Fire: enable(1)/disable(2)/toggle(0), sequence mutex, enabled/disabled events | UNVERIFIED | medium | src/behavior.cpp setSequenceEnabled (transitions only, mutex, event order); the Behavior_ChangeRemoteBehaviorSequenceState handler calls it |
| BehaviorKernel.IntializeBehaviorProviderForConsumer | [NATIVE_BEHAVIOR_POPULATION.md](NATIVE_BEHAVIOR_POPULATION.md) | Fire: provider registration order (pass 1 enabled-on-spawn, pass 2 conditions) | UNVERIFIED | medium | src/behavior.cpp registerConsumer (pass 1) + src/slice.cpp spawnDummy (pass 2, then OnSpawned) |
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
| WillowPlayerController.GetCurrentPlaythrough | [NATIVE_CONTROLLER_HELPERS.md](NATIVE_CONTROLLER_HELPERS.md) | Fire: every mission helper | UNVERIFIED | high | src/mission_script.cpp |
| WillowPlayerController.NativeGetMissionIndex | [NATIVE_CONTROLLER_HELPERS.md](NATIVE_CONTROLLER_HELPERS.md) | Fire: accept/status update | UNVERIFIED | high | src/mission_script.cpp |
| WillowPlayerController.UpdateLcdMissionStatus | [NATIVE_CONTROLLER_HELPERS.md](NATIVE_CONTROLLER_HELPERS.md) | presentation only (LCD), no-op | UNVERIFIED | high | src/mission_script.cpp (no-op) |
| WillowPlayerController.PopulateMissionDataFromStatus | [NATIVE_CONTROLLER_HELPERS.md](NATIVE_CONTROLLER_HELPERS.md) | none (LCD text) | UNVERIFIED | medium | |
| WillowPlayerController.GetHUDMovie | [NATIVE_CONTROLLER_HELPERS.md](NATIVE_CONTROLLER_HELPERS.md) | Fire: HUD guards (None ok) | UNVERIFIED | high | src/mission_script.cpp (None: no HUD in the VM graph) |
| WillowPlayerController.CanAffordToUseUsableObject | [NATIVE_CONTROLLER_HELPERS.md](NATIVE_CONTROLLER_HELPERS.md) | Fire: use of Marcus/objects | UNVERIFIED | medium-high | |
| WillowPlayerController.PayForUsedObject | [NATIVE_CONTROLLER_HELPERS.md](NATIVE_CONTROLLER_HELPERS.md) | Fire: use path | UNVERIFIED | medium-high | |
| WillowPlayerController.DoesObjectCostToUse | [NATIVE_CONTROLLER_HELPERS.md](NATIVE_CONTROLLER_HELPERS.md) | cost query behind Can/Pay | UNVERIFIED | medium-high | |
| WillowPlayerReplicationInfo.GetCurrencyOnHand | [NATIVE_CONTROLLER_HELPERS.md](NATIVE_CONTROLLER_HELPERS.md) | pay path | UNVERIFIED | high | |
| WillowPlayerReplicationInfo.AddCurrencyOnHand | [NATIVE_CONTROLLER_HELPERS.md](NATIVE_CONTROLLER_HELPERS.md) | pay path, rewards | UNVERIFIED | high | |
| WillowPlayerController.GetPawnInventoryManager | [NATIVE_CONTROLLER_HELPERS.md](NATIVE_CONTROLLER_HELPERS.md) | Fire: weapon equip/reward | UNVERIFIED | high | |
| WillowPlayerController.PlayUIAkEvent | [NATIVE_CONTROLLER_HELPERS.md](NATIVE_CONTROLLER_HELPERS.md) | presentation (fanfare sound) | UNVERIFIED | high | src/mission_script.cpp (no-op) |
| WillowPlayerController.GetLevelForMission | [NATIVE_CONTROLLER_HELPERS.md](NATIVE_CONTROLLER_HELPERS.md) | none | UNVERIFIED | low-medium | |
| WillowPlayerController.LocalMissionDependenciesMet | [NATIVE_CONTROLLER_HELPERS.md](NATIVE_CONTROLLER_HELPERS.md) | none in WillowGame script | UNVERIFIED | medium-high | |
| WillowPlayerController.IsMissionObjectiveCompleteLocal | [NATIVE_CONTROLLER_HELPERS.md](NATIVE_CONTROLLER_HELPERS.md) | none in WillowGame script | UNVERIFIED | medium-high | |
| WillowPlayerController.IsMissionObjectiveActiveLocal | [NATIVE_CONTROLLER_HELPERS.md](NATIVE_CONTROLLER_HELPERS.md) | none in WillowGame script | UNVERIFIED | medium-high | |
| WillowPlayerController.GetActiveMissionNumber | [NATIVE_CONTROLLER_HELPERS.md](NATIVE_CONTROLLER_HELPERS.md) | online status only | UNVERIFIED | medium | |
| WillowPlayerController.GetLocalActiveMissionNumber | [NATIVE_CONTROLLER_HELPERS.md](NATIVE_CONTROLLER_HELPERS.md) | save/online status only | UNVERIFIED | medium | |
| PlayerController.IsPrimaryPlayer | [NATIVE_CONTROLLER_HELPERS.md](NATIVE_CONTROLLER_HELPERS.md) | Fire: gates (true standalone) | UNVERIFIED | medium-high | src/mission_script.cpp (true for the sole local controller) |
| MissionTracker.IsDataValid | [NATIVE_CONTROLLER_HELPERS.md](NATIVE_CONTROLLER_HELPERS.md) | Fire: gates AddMission etc. | UNVERIFIED | high | src/mission_script.cpp (bDataValidated; set by ValidateData through the script ClientValidateMissionData run once at graph build: a shortcut for the trigger) |
| WorldInfo.IsMenuLevel | [NATIVE_CONTROLLER_HELPERS.md](NATIVE_CONTROLLER_HELPERS.md) | Fire: false in Sanctuary | UNVERIFIED | high | src/mission_script.cpp (bIsMenuLevel of the VM world, false) |
| GearboxGlobals.GetBehaviorKernel | [NATIVE_CONTROLLER_HELPERS.md](NATIVE_CONTROLLER_HELPERS.md) | Fire: behavior activation | UNVERIFIED | high | src/mission_script.cpp (TheBehaviorKernel of the globals object: None) |
| GearboxGlobals.GetGearboxGlobals | [NATIVE_CONTROLLER_HELPERS.md](NATIVE_CONTROLLER_HELPERS.md) | globals singleton | UNVERIFIED | high | src/mission_script.cpp (one VM globals object) |
| WillowGlobals.GetWillowGlobals | [NATIVE_CONTROLLER_HELPERS.md](NATIVE_CONTROLLER_HELPERS.md) | globals singleton (338 sites) | UNVERIFIED | high | src/mission_script.cpp (one VM globals object) |
| WillowAIPawn.IsComponentUsable | [NATIVE_CONTROLLER_HELPERS.md](NATIVE_CONTROLLER_HELPERS.md) | Fire: NPC use | UNVERIFIED | high | |
| Object.Localize | [NATIVE_CONTROLLER_HELPERS.md](NATIVE_CONTROLLER_HELPERS.md) | all text; .int lookup, ?INT?..? placeholder | UNVERIFIED | high | |
| Object.QueryInterface | [NATIVE_CONTROLLER_HELPERS.md](NATIVE_CONTROLLER_HELPERS.md) | Fire: IMissionDirector, behaviors | UNVERIFIED | medium-high | |
| GFxMoviePlayer.ActionScript / ActionScriptVoid | [NATIVE_GFX_BRIDGE.md](NATIVE_GFX_BRIDGE.md) | Fire: SetQuestTitle, SetPlayerXP, all HUD/reward Set* (sends the calling function's parameters) | UNVERIFIED | high | |
| GFxMoviePlayer.ActionScriptInt / Float / String / Object | [NATIVE_GFX_BRIDGE.md](NATIVE_GFX_BRIDGE.md) | HUD and menu getters (result typed by the caller's return property) | UNVERIFIED | high | |
| GFxObject.ActionScriptVoid / Int / Float / String / Object / Array | [NATIVE_GFX_BRIDGE.md](NATIVE_GFX_BRIDGE.md) | Fire: reward cards, HUD widgets | UNVERIFIED | high | |
| GFxMoviePlayer.ActionScriptSetFunction / GFxObject.ActionScriptSetFunction / ActionScriptSetFunctionOn | [NATIVE_GFX_BRIDGE.md](NATIVE_GFX_BRIDGE.md) | delegate binding into movies | UNVERIFIED | medium | |
| GFxObject.SetFunction | [NATIVE_GFX_BRIDGE.md](NATIVE_GFX_BRIDGE.md) | binds ext* callbacks (95 sites) | UNVERIFIED | high | |
| ExternalInterface callback and function handlers (AS to script, ext* binding) | [NATIVE_GFX_BRIDGE.md](NATIVE_GFX_BRIDGE.md) | Fire: every QuestAcceptGFxMovie ext* function | UNVERIFIED | medium-high | |
| GFxMoviePlayer.Invoke | [NATIVE_GFX_BRIDGE.md](NATIVE_GFX_BRIDGE.md) | callers needing a result | UNVERIFIED | high | |
| GFxObject.Invoke | [NATIVE_GFX_BRIDGE.md](NATIVE_GFX_BRIDGE.md) | rare | UNVERIFIED | medium | |
| GFxMoviePlayer.SetVariable / SetVariableBool / Number / String / Object | [NATIVE_GFX_BRIDGE.md](NATIVE_GFX_BRIDGE.md) | Fire: InitForPC (missions.pcCloseButton._visible) | UNVERIFIED | high | |
| GFxMoviePlayer.GetVariable / GetVariableBool / Number / String | [NATIVE_GFX_BRIDGE.md](NATIVE_GFX_BRIDGE.md) | menus | UNVERIFIED | high | |
| GFxMoviePlayer.GetVariableObject | [NATIVE_GFX_BRIDGE.md](NATIVE_GFX_BRIDGE.md) | Fire: SetFocus | UNVERIFIED | high | |
| GFxMoviePlayer.Get/SetVariableArray (Int, Float, String variants) | [NATIVE_GFX_BRIDGE.md](NATIVE_GFX_BRIDGE.md) | not read | UNVERIFIED | low | |
| GFxMoviePlayer.CreateObject / CreateArray | [NATIVE_GFX_BRIDGE.md](NATIVE_GFX_BRIDGE.md) | list data | UNVERIFIED | high | |
| GFxMoviePlayer.Start / Advance / SetPause / Close | [NATIVE_GFX_BRIDGE.md](NATIVE_GFX_BRIDGE.md) | Fire: open/close of the accept screen | UNVERIFIED | medium-high | |
| GFxMoviePlayer.GetPC / GetLP | [NATIVE_GFX_BRIDGE.md](NATIVE_GFX_BRIDGE.md) | Fire: Start, extAcceptConfirmed | UNVERIFIED | high | |
| GFxMoviePlayer.RegisterGFxObject / UnregisterGFxObject | [NATIVE_GFX_BRIDGE.md](NATIVE_GFX_BRIDGE.md) | wrapper lifetime | UNVERIFIED | medium | |
| GFxMoviePlayer.ResolveDataStoreMarkup | [NATIVE_GFX_BRIDGE.md](NATIVE_GFX_BRIDGE.md) | not seen in mission screens | UNVERIFIED | low | |
| GFxMoviePlayer.SetWidgetPathBinding | [NATIVE_GFX_BRIDGE.md](NATIVE_GFX_BRIDGE.md) | CLIK widget init | UNVERIFIED | low | |
| GFxObject.Get / GetBool / GetFloat / GetString / GetObject | [NATIVE_GFX_BRIDGE.md](NATIVE_GFX_BRIDGE.md) | reward cards | UNVERIFIED | medium-high | |
| GFxObject.Set / SetBool / SetFloat / SetString / SetObject / SetText | [NATIVE_GFX_BRIDGE.md](NATIVE_GFX_BRIDGE.md) | reward cards, list text (htmlText rule) | UNVERIFIED | high | |
| GFxObject.TranslateString (Font/Color markup) | [NATIVE_GFX_BRIDGE.md](NATIVE_GFX_BRIDGE.md) | text markup | UNVERIFIED | medium-high | |
| GearboxGFxMovie.PlayUISound | [NATIVE_GFX_BRIDGE.md](NATIVE_GFX_BRIDGE.md) | Fire: MenuOpen, accept_mission, MenuClose | UNVERIFIED | medium-high | |
| GearboxGFxMovie.SingleArgInvokeS / F / B | [NATIVE_GFX_BRIDGE.md](NATIVE_GFX_BRIDGE.md) | HUD (54 / 8 / 2 sites) | UNVERIFIED | medium | |
| GearboxGFxMovie.GetInstanceContextObject / GetLocalPlayer / InitFromDefinition | [NATIVE_GFX_BRIDGE.md](NATIVE_GFX_BRIDGE.md) | Fire: QuestAccept Start | UNVERIFIED | medium | |
| WillowGFxMovie3D.FocusOn | [NATIVE_GFX_BRIDGE.md](NATIVE_GFX_BRIDGE.md) | Fire: accept screen SetFocus (no-op safe) | UNVERIFIED | low-medium | |
| QuestAcceptGFxMovie.UpdateMissionTextList | [NATIVE_GFX_BRIDGE.md](NATIVE_GFX_BRIDGE.md) | Fire: list category headers (available / turn-in) | UNVERIFIED | medium | |
| Behavior_TriggerDialogEvent.ApplyBehaviorToContext (script) | [NATIVE_DIALOG.md](NATIVE_DIALOG.md) | Fire: Out at first run, Finished at line end (poll 0.1 s) | UNVERIFIED | high | src/dialog.cpp DialogSystem::behavior (first run Out + latent 0.001 s, trigger on the wake, 0.1 s poll, Finished, bForcePlayImmediate) |
| Behavior_TriggerDialogEvent.TriggerDialogEvent | [NATIVE_DIALOG.md](NATIVE_DIALOG.md) | Fire: group event trigger, instigator = context object | UNVERIFIED | medium-high | src/dialog.cpp trigger (group path only; talker-owned events not implemented) |
| GearboxDialogManager.TriggerGroupEvent | [NATIVE_DIALOG.md](NATIVE_DIALOG.md) | Fire: event node lookup (last enabled match), chain runs synchronously, pooled event data | UNVERIFIED | high | src/dialog.cpp trigger/findAct (last enabled entry, link table, event data pool) |
| GearboxDialogGroup.SimpleEvent | [NATIVE_DIALOG.md](NATIVE_DIALOG.md) | sound-effect tags (one-shot play, no live state) | UNVERIFIED | medium | |
| GearboxDialogComponent.TriggerEvent | [NATIVE_DIALOG.md](NATIVE_DIALOG.md) | talker-owned events, client forwarding | UNVERIFIED | medium | |
| GearboxDialogComponent.GetMatchingEvent | [NATIVE_DIALOG.md](NATIVE_DIALOG.md) | group search with ParentGroup fallback | UNVERIFIED | medium | |
| GearboxDialogComponent.Talk | [NATIVE_DIALOG.md](NATIVE_DIALOG.md) | Fire: starts Wwise event, live-line state, TalkStarted | UNVERIFIED | medium-high | src/dialog.cpp talk (device gate, priority gate, live state, group silence, interrupt) |
| GearboxDialogComponent.StopTalking | [NATIVE_DIALOG.md](NATIVE_DIALOG.md) | interrupt / end of line, TalkFinished | UNVERIFIED | medium-high | src/dialog.cpp stopTalking (no TalkFinished script event) |
| GearboxDialogComponent.IsTalking | [NATIVE_DIALOG.md](NATIVE_DIALOG.md) | live-line test | UNVERIFIED | high | src/dialog.cpp (event data live state) |
| GearboxDialogComponent.TalkReplicated | [NATIVE_DIALOG.md](NATIVE_DIALOG.md) | client side (not needed single-player) | UNVERIFIED | low | src/dialog.cpp talk (device gate, priority gate, live state, group silence, interrupt) |
| GearboxDialogComponent.GetDialogInterface | [NATIVE_DIALOG.md](NATIVE_DIALOG.md) | interface accessor | UNVERIFIED | medium | |
| GearboxDialogEventData.IsActive | [NATIVE_DIALOG.md](NATIVE_DIALOG.md) | Fire: true while a talk act is live (what the behavior polls) | UNVERIFIED | high | src/dialog.cpp active() |
| GearboxDialogNode.ActivateOutput | [NATIVE_DIALOG.md](NATIVE_DIALOG.md) | direct link first, then group link table; shared template nodes | UNVERIFIED | medium-high | |
| GearboxDialogAct_Talk.Activate / WillowDialogAct_Talk.Activate | [NATIVE_DIALOG.md](NATIVE_DIALOG.md) | Fire: no-audio pass-through, talker choice, Talk call, no-match output | UNVERIFIED | medium-high | src/dialog.cpp talk (talker choice: instigator -> none, random TalkData by exact name tag, echo caller) |
| GearboxDialogAct_Chance.Activate | [NATIVE_DIALOG.md](NATIVE_DIALOG.md) | chance + quiet time | UNVERIFIED | high | |
| GearboxDialogAct_Compare.Activate | [NATIVE_DIALOG.md](NATIVE_DIALOG.md) | talker-set intersection | UNVERIFIED | medium | |
| GearboxDialogAct_ObjectParameterSwitch.Activate | [NATIVE_DIALOG.md](NATIVE_DIALOG.md) | switch on event ObjectParameter | UNVERIFIED | medium-high | |
| GearboxDialogAct_Trigger.Activate / ActivateOutput | [NATIVE_DIALOG.md](NATIVE_DIALOG.md) | template call, continues on line end | UNVERIFIED | medium | |
| WillowDialogAct_MissionSwitch.Activate | [NATIVE_DIALOG.md](NATIVE_DIALOG.md) | output = mission status | UNVERIFIED | low-medium | |
| WillowDialogAct_RandomBranch.Activate | [NATIVE_DIALOG.md](NATIVE_DIALOG.md) | weighted pick, quiet time, repeat avoidance (weights not decoded) | UNVERIFIED | low-medium | |
| GearboxDialogManager.RegisterTalker / UnregisterTalker / EnableTalker / DisableTalker | [NATIVE_DIALOG.md](NATIVE_DIALOG.md) | talker registry | UNVERIFIED | medium | src/dialog.cpp registerTalker (register only) |
| GearboxDialogManager.AddGroup / SilenceGroup / GetGroupEventTag / SetGroupEventTag | [NATIVE_DIALOG.md](NATIVE_DIALOG.md) | group state keyed by root group | UNVERIFIED | medium | |
| GearboxDialogManager.GetPriority / GetEventTagForEventInfo / Cleanup | [NATIVE_DIALOG.md](NATIVE_DIALOG.md) | priority index = position in globals Priorities array | UNVERIFIED | high | src/dialog.cpp indexOf/floor (the priority index and the tracked-mission floor) |
| WillowDialogManager.PlayEchoDialog | [NATIVE_DIALOG.md](NATIVE_DIALOG.md) | echo caller creation and trigger | UNVERIFIED | medium | |
| WillowDialogManager.IsMissionKickoffPlaying / GetPriorityForEchoActor | [NATIVE_DIALOG.md](NATIVE_DIALOG.md) | kickoff priority comparison | UNVERIFIED | medium | |
| WillowDialogGlobalsDefinition.Get / TriggerTemplateEvent / StaticTriggerTemplateEvent | [NATIVE_DIALOG.md](NATIVE_DIALOG.md) | generic template events | UNVERIFIED | low-medium | |
| GearboxSeqAct_TriggerDialogName (Kismet action) | [NATIVE_DIALOG.md](NATIVE_DIALOG.md) | Fire: dummy-reset line, latent until line end | UNVERIFIED | medium | |
| BehaviorHelpers.IsBehaviorsV2 | [NATIVE_DIALOG.md](NATIVE_DIALOG.md) | true when KernelInfo carries a live kernel | UNVERIFIED | high | src/behavior.cpp (always true: every behavior runs under the thread kernel here) |
| PlayerSkillTree.UpgradeSkill | [NATIVE_SKILLS.md](NATIVE_SKILLS.md) | Fire mission: Maya spends a point (host TrySpend) | UNVERIFIED | high | |
| PlayerSkillTree.SetSkillGrade | [NATIVE_SKILLS.md](NATIVE_SKILLS.md) | Load / client mirror, tier unlock bookkeeping | UNVERIFIED | high | |
| PlayerSkillTree.GetSkillState | [NATIVE_SKILLS.md](NATIVE_SKILLS.md) | Grade/unlocked read by UI and activation | UNVERIFIED | high | |
| PlayerSkillTree.GetBranchState | [NATIVE_SKILLS.md](NATIVE_SKILLS.md) | Branch counters in the skill UI | UNVERIFIED | medium | |
| PlayerSkillTree.GetTierState | [NATIVE_SKILLS.md](NATIVE_SKILLS.md) | Tier unlocked / points in tier | UNVERIFIED | medium | |
| PlayerSkillTree.GetSkillPointsSpentInTree | [NATIVE_SKILLS.md](NATIVE_SKILLS.md) | Respec refund, earned-any-points | UNVERIFIED | high | |
| PlayerSkillTree.GetActionSkill | [NATIVE_SKILLS.md](NATIVE_SKILLS.md) | Phaselock lookup | UNVERIFIED | high | |
| PlayerSkillTree.HasTrainedASkillOfType | [NATIVE_SKILLS.md](NATIVE_SKILLS.md) | Skill-type queries | UNVERIFIED | medium | |
| PlayerSkillTree.AllSkills | [NATIVE_SKILLS.md](NATIVE_SKILLS.md) | Activation loop after reset | UNVERIFIED | medium | |
| PlayerSkillTree.AllSkillsOfType | [NATIVE_SKILLS.md](NATIVE_SKILLS.md) | Action-augment activation | UNVERIFIED | medium | |
| PlayerSkillTree.Initialize | [NATIVE_SKILLS.md](NATIVE_SKILLS.md) | Builds tree from SkillTreeDefinition | UNVERIFIED | medium | |
| PlayerSkillTree.UpdateBranchProgression | [NATIVE_SKILLS.md](NATIVE_SKILLS.md) | UI progress bars only | UNVERIFIED | medium | |
| PlayerSkillTree.SaveSkillSaveGameData | [NATIVE_SKILLS.md](NATIVE_SKILLS.md) | Quest save of grades | UNVERIFIED | medium | |
| PlayerSkillTree.ApplySkillSaveGameData | [NATIVE_SKILLS.md](NATIVE_SKILLS.md) | Save replay (script loads per skill instead) | UNVERIFIED | medium | |
| WillowPlayerController.InitPlayerSkillTree | [NATIVE_SKILLS.md](NATIVE_SKILLS.md) | Tree creation at class change | UNVERIFIED | medium | |
| WillowPlayerController.ResetSkillTree | [NATIVE_SKILLS.md](NATIVE_SKILLS.md) | Respec / load reset, refund | UNVERIFIED | high | |
| WillowPlayerController.HasPlayerEarnedAnySkillPoints | [NATIVE_SKILLS.md](NATIVE_SKILLS.md) | Skill UI gating | UNVERIFIED | high | |
| WillowPlayerController.GetActionSkillDuration | [NATIVE_SKILLS.md](NATIVE_SKILLS.md) | Action skill active-ability time | UNVERIFIED | medium | |
| SkillDefinition.DoesSkillPassMinGradeTest | [NATIVE_SKILLS.md](NATIVE_SKILLS.md) | Passive needs grade >= 1 | UNVERIFIED | high | |
| Skill.CalculateModifierValue | [NATIVE_SKILLS.md](NATIVE_SKILLS.md) | Grade -> effect value formula | UNVERIFIED | high | |
| Skill.CalculateModifierValueFromDefinitionEffectArray | [NATIVE_SKILLS.md](NATIVE_SKILLS.md) | Same, by effect index | UNVERIFIED | high | |
| Skill.AddSkillEffect | [NATIVE_SKILLS.md](NATIVE_SKILLS.md) | Effect -> modifier object | UNVERIFIED | high | |
| Skill.AdjustModifiers | [NATIVE_SKILLS.md](NATIVE_SKILLS.md) | Apply/remove modifiers on attributes | UNVERIFIED | high | |
| Skill.ForceRefresh | [NATIVE_SKILLS.md](NATIVE_SKILLS.md) | Re-evaluate after grade change | UNVERIFIED | high | |
| SkillEffectManager.RefreshSkillsForInstigator | [NATIVE_SKILLS.md](NATIVE_SKILLS.md) | Re-evaluate a player's skills | UNVERIFIED | medium | |
| SkillEffectManager.RefreshSkillsAffectingInstigator | [NATIVE_SKILLS.md](NATIVE_SKILLS.md) | Re-evaluate skills affecting a player | UNVERIFIED | medium | |
| WillowPlayerController per-frame usable evaluation (no script name; sets CurrentUsableObject) | [NATIVE_USE_INTERACTION.md](NATIVE_USE_INTERACTION.md) | Fire: choosing Marcus (camera ray 350 uu) | UNVERIFIED | medium-high | |
| WillowPlayerController.UpdateInteractionIcon | [NATIVE_USE_INTERACTION.md](NATIVE_USE_INTERACTION.md) | prompt presentation | UNVERIFIED | high | |
| IUsable native virtuals of WillowAIPawn (icon, can-be-used, prompt, usable-by-user) | [NATIVE_USE_INTERACTION.md](NATIVE_USE_INTERACTION.md) | Fire: Marcus is usable | UNVERIFIED | medium-high | |
| WillowAIPawn.SetUsable | [NATIVE_USE_INTERACTION.md](NATIVE_USE_INTERACTION.md) | Fire: gates the talk prompt | UNVERIFIED | high | |
| WillowAIPawn.SetInteractionIcon | [NATIVE_USE_INTERACTION.md](NATIVE_USE_INTERACTION.md) | prompt icon | UNVERIFIED | high | |
| WillowAIPawn.GetPrimaryUser | [NATIVE_USE_INTERACTION.md](NATIVE_USE_INTERACTION.md) | talk state | UNVERIFIED | high | |
| WillowAIPawn.HasAnyMissionsForPlayer | [NATIVE_USE_INTERACTION.md](NATIVE_USE_INTERACTION.md) | director table, compass icon | UNVERIFIED | high | |
| WillowAIPawn.GetMissionDirectorLocation | [NATIVE_USE_INTERACTION.md](NATIVE_USE_INTERACTION.md) | director table | UNVERIFIED | high | |
| WillowAIPawn.GetAllDirectorData | [NATIVE_USE_INTERACTION.md](NATIVE_USE_INTERACTION.md) | director data read | UNVERIFIED | high | |
| MissionTracker.RegisterMissionDirector | [NATIVE_USE_INTERACTION.md](NATIVE_USE_INTERACTION.md) | director table (presentation) | UNVERIFIED | medium | |
| MissionTracker.UnregisterMissionDirector | [NATIVE_USE_INTERACTION.md](NATIVE_USE_INTERACTION.md) | director table (presentation) | UNVERIFIED | medium | |
| MissionTracker.ProcessDynamicMissionDirectives | [NATIVE_USE_INTERACTION.md](NATIVE_USE_INTERACTION.md) | client refresh of the director table | UNVERIFIED | medium-low | |
| MissionTracker.GetCompletedBranch | [NATIVE_USE_INTERACTION.md](NATIVE_USE_INTERACTION.md) | redeemable filter (Fire: branch None) | UNVERIFIED | medium-high | |
| AIClassDefinition.OnUsed (and OnSecondaryUsed, OnUserCouldNotAfford, OnUserCouldNotAffordSecondary) | [NATIVE_USE_INTERACTION.md](NATIVE_USE_INTERACTION.md) | Fire: starts Marcus's behavior chain | UNVERIFIED | high | |
| AIDefinition.OnUsed (and the three twins) | [NATIVE_USE_INTERACTION.md](NATIVE_USE_INTERACTION.md) | Fire: starts Marcus's behavior chain | UNVERIFIED | high | |
| WillowAIPawn.UpdateLookAtTarget | [NATIVE_USE_INTERACTION.md](NATIVE_USE_INTERACTION.md) | talk state (look-at target) | UNVERIFIED | medium-low | |
| WillowAIPawn.GetFocusLocation / GetFocusRadius / GetFocusScreenOffset | [NATIVE_USE_INTERACTION.md](NATIVE_USE_INTERACTION.md) | talk camera | UNVERIFIED | medium-high | |
| WillowAIPawn.CanTalk / WillowMind.ShouldLookAtPlayer | [NATIVE_USE_INTERACTION.md](NATIVE_USE_INTERACTION.md) | thin front ends, not on the use path | UNVERIFIED | low | |
| Engine touch dispatch (actor overlap update, begin/end touch; no script name) | [NATIVE_OBJECTIVE_TRIGGERS.md](NATIVE_OBJECTIVE_TRIGGERS.md) | Fire mission: GoToRange (Touch raised on the waypoint/player pair) | UNVERIFIED | medium (dispatch), low (shape test, pair order) | |
| WillowWaypoint.PostBeginPlay/Touch/ProcessPlayerTouch/MissionReactionObjectiveSetChanged (script) | [NATIVE_OBJECTIVE_TRIGGERS.md](NATIVE_OBJECTIVE_TRIGGERS.md) | Fire mission: GoToRange completion rule | UNVERIFIED | high | |
| Actor.IsPlayerOwned | [NATIVE_OBJECTIVE_TRIGGERS.md](NATIVE_OBJECTIVE_TRIGGERS.md) | Fire mission: filters Marcus out of GoToRange | UNVERIFIED | medium | |
| MissionTracker.IsMissionObjectiveActive | [NATIVE_OBJECTIVE_TRIGGERS.md](NATIVE_OBJECTIVE_TRIGGERS.md) | Fire mission: waypoint gate, update gate, enable condition | UNVERIFIED | high | |
| MissionTracker.IsObjectiveSetActive | [NATIVE_OBJECTIVE_TRIGGERS.md](NATIVE_OBJECTIVE_TRIGGERS.md) | Fire mission: waypoint set restrictions (empty here) | UNVERIFIED | high | |
| MissionTracker.IsMissionObjectiveComplete | [NATIVE_OBJECTIVE_TRIGGERS.md](NATIVE_OBJECTIVE_TRIGGERS.md) | Fire mission: enable conditions, HUD | UNVERIFIED | high | |
| MissionTracker.UpdateObjective (fan-out to players, additions to B1) | [NATIVE_OBJECTIVE_TRIGGERS.md](NATIVE_OBJECTIVE_TRIGGERS.md) | Fire mission: GoToRange and Fire progress | UNVERIFIED | high | |
| Behavior_UpdateMissionObjective.ApplyBehaviorToContext (script) | [NATIVE_OBJECTIVE_TRIGGERS.md](NATIVE_OBJECTIVE_TRIGGERS.md) | Fire mission: Fire objective from the dummy provider | UNVERIFIED | high | |
| Behavior_AdvanceObjectiveSet.ApplyBehaviorToContext (self object must be the tracker) | [NATIVE_OBJECTIVE_TRIGGERS.md](NATIVE_OBJECTIVE_TRIGGERS.md) | Fire mission: set transitions | UNVERIFIED | high | |
| Behavior_MissionRemoteEvent.ApplyBehaviorToContext | [NATIVE_OBJECTIVE_TRIGGERS.md](NATIVE_OBJECTIVE_TRIGGERS.md) | Fire mission: Marcus walk, dummy moves (mission-matched Kismet events) | UNVERIFIED | high | |
| Behavior_ClearObjective.ApplyBehaviorToContext | [NATIVE_OBJECTIVE_TRIGGERS.md](NATIVE_OBJECTIVE_TRIGGERS.md) | not on the Fire route | UNVERIFIED | medium | |
| Behavior_DecrementObjective.ApplyBehaviorToContext / MissionTracker.DecrementObjective | [NATIVE_OBJECTIVE_TRIGGERS.md](NATIVE_OBJECTIVE_TRIGGERS.md) | not on the Fire route | UNVERIFIED | medium | |
| MissionTracker.RegisterWaypoint / UnregisterWaypoint | [NATIVE_OBJECTIVE_TRIGGERS.md](NATIVE_OBJECTIVE_TRIGGERS.md) | Fire mission: objective markers (presentation) | UNVERIFIED | medium | |
| MissionObjectiveWaypointComponent.RemoveWaypoint (and its refresh) | [NATIVE_OBJECTIVE_TRIGGERS.md](NATIVE_OBJECTIVE_TRIGGERS.md) | Fire mission: objective markers (presentation) | UNVERIFIED | medium | |
| WillowPlayerController.UpdateMissionObjective (script hook) | [NATIVE_OBJECTIVE_TRIGGERS.md](NATIVE_OBJECTIVE_TRIGGERS.md) | Fire mission: HUD progress text and fanfare | UNVERIFIED | high | |
| AIClassDefinition.OnTakeDamage (and the Pawn.NotifyTakeHit / WillowMind.NotifyTakeHit chain) | [NATIVE_OBJECTIVE_TRIGGERS.md](NATIVE_OBJECTIVE_TRIGGERS.md) | Fire mission: Fire objective trigger | UNVERIFIED | medium | |
| AIDefinition.OnTakeDamage (read only as the first of the event pair) | [NATIVE_OBJECTIVE_TRIGGERS.md](NATIVE_OBJECTIVE_TRIGGERS.md) | Fire mission: not used by the dummy | UNVERIFIED | low | |
