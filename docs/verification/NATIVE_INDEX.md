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
