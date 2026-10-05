# Native census of the Fire mission's script call graph (2026-10-05)

AI-assisted (Claude). Step 1 of replacing the host stand-ins for "Rock, Paper, Genocide: Fire Weapons!"
(`GD_Z1_RockPaperGenocide.M_RockPaperGenocide_Fire`, giver Marcus in Sanctuary) with the game's own script on the C++ VM:
which natives does that script reach, and how often? **This is a static-plus-VM estimate. It is not a measurement of the
real game**: nothing was run in the game, no trace was taken, and every number below depends on the entry list, on the
objects the VM could build from class defaults and on the fact that every native without an implementation is a zero-result
stub. Nothing in this record is a listing; function names and counts only.

## Method

1. **Counting in the VM.** `vm::Runtime` gained opt-in per-call counters (`countCalls`, `scriptCalls`, `nativeCalls`,
   `stubCalls`, `noneContexts`, `src/vm.hpp`), filled in `Interp::invoke`, the one place every call goes through. Keys are
   function paths (`Package.Class.Function`). Implemented natives and logged stubs are counted separately; `noneContexts`
   counts, per script function, the `Context` expressions whose object was `None` (the interpreter silently yields a zero
   value there, which is how most early bail-outs happen). Off by default; existing callers are unaffected.
2. **`ow-package <package> --native-census <entry-file> --cooked <dir> [--steps n] [--no-static]`** (`src/census.*`). Runs an
   entry file (grammar in `src/census.hpp`): `new`/`export` build objects (class defaults, or an installed export with its
   data), `set`/`add` wire properties (paths like `$pc.MissionPlaythroughs[0].MissionList`), `run` calls one function,
   `runclass` calls every script function a class itself declares with no arguments. Objects persist from entry to entry in
   file order. Unregistered natives are the interpreter's existing logged stub. Prints JSON: per entry completed/stopped (and
   why), script functions entered, native calls (implemented vs stub), `None` contexts and the non-stub log lines; plus the
   merged native table and the static closure.
3. **Static closure** (C++, same file; it reuses the decoder and the VM's reference resolution). Walks the decoded bytecode of
   every function reachable from the entries through `FinalFunction`, `VirtualFunction`, `GlobalFunction`, numbered native
   calls and delegate declarations. Two closures:
   * **A** follows *primary* edges: direct calls, numbered natives, and for a virtual call the method visible at the
     receiver's static class. The receiver class comes from the declared type of variables/struct members/array elements,
     casts, class constants, function results and typed temporaries; when that fails, from the function the compiler resolved
     (a `Context` expression names the called function's return property, whose outer is that function).
   * **B** adds *upper-bound* edges: subclass overrides of the name (also inside states), every function of that name for an
     interface-typed receiver (implementers are not known statically), delegate-bound names, and, for a receiver nobody could
     type, a name that at most 8 classes declare. B is loose by construction (it walks 4,496 script functions; the player
     controller alone drags in most of the UI); A is the closer estimate.
   * Static "call sites" for a native: the number of call expressions to it inside the script functions of the closure
     (each function body counted once, however many entries reach it). Operators (`+`, `==`, `&&` ...) are natives and are
     counted, but listed separately below.
   * Checked: every script function and native the dynamic runs entered lies inside closure B, and all but two inside A (two
     virtual dispatches resolved to an override).
4. **`tools/slice_native_census.py`** (stdlib Python, no game data) drives 2 and 3, merges them, writes
   `local/p2/A/native_census.json` and `native_census_ranked.tsv` (ignored) and prints the ranked tables. Ranking: dynamic calls,
   then closure A sites, then closure B sites. "Implemented" means registered by `Runtime::registerCoreNatives()` (`src/natives_core.cpp`);
   the mover and inventory adapters bind a few more natives in their own scope (`SetTimer`/`ClearTimer`, the entry-kind
   getter) that this census does not register.

Reproduce: `python tools/slice_native_census.py` (needs `OPENWILLOW_BL2` and `build/Release/ow-package.exe`; 1.3 s here).

## Entry list (`tools/slice_native_census_entries.txt`, 319 entries)

Every function name was resolved against the installed packages (the CLI stops on an unknown one). Reasons:

| Section | Entries | Why it is on the path |
|---|---|---|
| Use and accept/turn-in on `WillowPlayerController` | `Use`, `PerformedUseAction`, `ServerUse`, `ServerUseWithoutConfirmation`, `AcceptMission`, `ServerAcceptMission`, `ServerCompleteMission`, `ServerGrantMissionRewards` (both reward flags) | talking to Marcus, accepting and turning in; these call into the native tracker (`ActivateMission`, `CompleteMission`, `PlayTurnIn`) and the giver's director interface |
| Mission state on the controller (about 55) | `UpdateMissionStatus` (status 1, 3, 4), `UpdateMissionObjective`, `UpdateActiveObjectiveSet`, `SubObjectiveSetAdvanced`, `Client*` receivers, `ClientDoMissionStatusFanfare`, `ClientUpdateUIMissionList`, `ClientShowNoRewardScreen`, `SetMissionKickoffHeard`, `RefreshHUDMissionWidget`, `SaveMissionSaveGameData`, `ApplyMissionSaveGameData`, `AddMission`, `UpdateActiveMission`, mission list getters and filters ... | the native tracker calls back into these for every status/objective change (not reachable from script alone, hence entries) |
| Rewards and experience | `ClientSpawnMissionRewardUI`, `AcceptOrSaveUnclaimedReward`, `GetNumRewardChoices`, `ReceiveWeaponReward`, `ReceiveItemReward`, `MissionRewardsReceived`, `WillowInventoryManager.AddBackpackWeaponFromDefinitionData` / `AddBackpackItemFromDefinitionData`, `WillowGameInfo.AwardCombatExperience`, `ExpLevelUp`, `OnExpLevelChange`, `ClientOnExpLevelChange`, `DoLevelUpNotifications`, `OnAllyLevelChange`, PRI and GRI notifiers | turn-in XP, credits, items, level-up |
| Player pawn | `WillowPawn.UseObject`, `NotifyUserCouldNotAffordAttemptedUse`, `GetNextExpLevelPoints`, `WillowPlayerPawn.ServerPlayerBeginUseNPC` / `EndUseNPC` | the use path and the XP bar |
| The giver | 23 `WillowAIPawn` functions: `RegisterMissionDirector`, `AddMissionDirective`, `Get*Missions`, `OnPlayerAcceptedMission`, `OnPlayerTurnedInMission`, `MissionStatusChanged`, `UseObject`, `BeginUse`/`EndUse`, `FireOnUsedBehaviors`, `OnNewPrimaryUser`, dialog and UI notifications | `IMissionDirector` is implemented by `WillowAIPawn`, `WillowInteractiveObject` and `WillowPickup`; the closure data places Marcus as a `WillowAIPawn`, so the other two implementers are reached only through closure B |
| Tracker script, UI, population | every script function of `MissionTracker` (44, delegate triggers and replication), `QuestAcceptGFxMovie` (60, the accept/turn-in screen), `MissionPopulationAspect` (18, `MissionReaction*`, `SetActivationFromMission`), `Action_GoToScriptedDestination` and `WillowDialogAct_Talk` (Marcus's scripted walk and dialog in the Kismet data) | `MissionTracker.*` is native except its delegate and replication glue |
| Behavior and Kismet classes of the data | `runclass` over the 65 `Behavior_*`, `SeqAct_*`, `SeqCond_*`, `SeqEvent_*`, `SeqVar_*`, `GearboxSeq*`, `WillowSeq*` classes in `closure.json` / `kismet_graph.json` (63 entries; most classes declare no script) | script bodies the BehaviorKernel and Kismet executor would run for this mission |

Pruned or not added, with reasons: `MissionDefinition.*`, `MissionObjective*` and the tracker's other functions are native (no
script); `ExpEarn`, `AddCurrencyOnHand`, `GetPawnInventoryManager` are native (they are census rows, not entries);
`WillowInteractiveObject.*` and `WillowPickup.*` directors were left to closure B; whole `WillowAIPawn`, `StatusMenuExGFxMovie`
and `WillowHUD` classes were not run (hundreds of functions, mostly unrelated to the mission).

Script that grants turn-in XP and rewards (read from the bytecode, not confirmed in the running game): `ServerGrantMissionRewards`
asks the mission (native getters) for the currency type and amount and adds it to the player's replication info (native),
adds the optional credit reward, asks the mission for its experience and calls the native `ExpEarn` (amount, type 2 for a
plot-critical mission, else 4), then has the mission fill a pending-reward record (native `GetItemRewardsForPlayer`) and calls
`ClientSpawnMissionRewardUI`. That client function shows the rewards page when the `QuestAcceptGFxMovie` is playing, else
`AcceptOrSaveUnclaimedReward`: with a single choice it calls `ReceiveWeaponReward` or `ReceiveItemReward` (script, which add to
the pawn's backpack through `WillowInventoryManager.AddBackpack*FromDefinitionData`, also script) and then
`MissionRewardsReceived`; with several choices it queues the record in `UnclaimedRewards`. Level-ups come back from native
`ExpEarn` as script `ExpLevelUp` -> `OnExpLevelChange` -> `ClientOnExpLevelChange` / `DoLevelUpNotifications`. For the Fire
mission the data gives XP only (`GD_MissionRewardBalance.XP.XPReward_02_Small`, credit multiplier 0), see `SANCTUARY_RPG_MISSION.md`.

## Coverage honesty

* **All 319 entries completed; none stopped** (no exception, no step limit, no call-depth limit; the longest entry ran 501
  expressions, all 319 together 14,573). That says little about depth: only **90 entries entered a script function other than
  themselves**, **132 evaluated a `Context` on `None`** (365 times in total) and **65 made no native call at all**. Every
  entry that stops early does so quietly through a zero result.
* State that was faked: the object graph in the file header (WorldInfo -> GRI -> MissionTracker, controller, PRI with
  `ExpLevel` 5, pawn, `Marcus` as a default `WillowAIPawn`, one playthrough holding one mission record); struct-typed
  arguments (mission data, rewards, save game) left at zero; `Role` and the other properties at their class defaults. The
  mission and its objectives are the real installed exports. The game's own startup would also create the GFx manager,
  HUD, inventory manager links, player stats and class definition; those stay `None`, which is why UI and reward entries
  bail early.
* Stubs that decide control flow: `GetCurrentPlaythrough` and `NativeGetMissionIndex` return 0, `CanAffordToUseUsableObject`
  returns false (so `Use` takes the "cannot afford" branch), `GetPlayingMovie` and `GetHUDMovie` return `None`, `GetMaxExpLevel`
  returns 0 (`ExpLevelUp` skips its main block), `GetWillowGlobals` is a static call through a class constant (see below).
* `foreach` over an engine iterator is skipped by the interpreter (24 times), so natives inside such loops are reached only
  statically. 28 array reads were out of bounds on empty arrays.
* **VM finding (not changed here): class constants evaluate to a stand-in object, not a class.** `ObjectConst` in
  `src/interp.cpp` tests `object(exportObject.cls).name == "Class"`, but a class export carries class reference 0 (the
  existing `classNameOf` already treats 0 as `Class`), so every `ClassContext(ObjectConst(Class), static call)` logs
  `UNIMPLEMENTED function ?.X` and returns zero (18 `GetWillowGlobals`, 4 `GetFirstInstanceDataObject` here). Treating
  `cls == 0` as a class changed an earlier version of this run by +7 natives reached and +1 script function entered; it is a
  one-line change with wider effects on the host paths, so it was left for the maintainer. The static walker handles the case.
* Not covered: natives that call back into script (the tracker's delegate triggers fire from native code; the entries stand
  in for them), engine-driven events (`PostBeginPlay`, `Tick`, `Touch` ...), `SetTimer` callbacks and other name-based
  dispatch (`ConsoleCommand`, `SetTimer` arguments), UnrealScript states beyond name-based override lookup, objects created
  by natives, replication (`Role`/net mode branches run in whatever way the defaults decide), and anything reachable only
  through the Kismet/BehaviorKernel native executors. Natives reached only inside the four script functions that do not
  decode (`FrontEndPlayerListGFxObject.RefreshPlayerList`, `StatusMenuExGFxMovie.GetHighestChainedPlotMissionCompleted`,
  `WillowGFxUIManager.PlayMovie`, `WillowWeapon.InitMeshAnimation`, all in closure B) are missing; 10 of 12,978 functions fail
  the structural decode in total.
* **Ground truth still missing:** a trace of the real game (`tools/sdk_trace`, `tools/real_game`) for accept, kickoff, objective
  completion and turn-in would give true call counts and the callbacks from native code; it needs exclusive game access and
  was not done. Without it the dynamic column is a floor shaped by stubs, and A/B are structural estimates.

## Results (2026-10-05, build of this branch)

* 319 entries; **360 script functions entered**; closure A **853** script functions, closure B **4,496**.
* **Natives reached: 126 dynamically, 577 in closure A, 1,934 in closure B** (without the 80 Core operators: 95 / 514 / 1,854).
  Dynamic native calls: 1,075, of which 740 ran an implementation (730 of those are Core operators, 10 other Core natives) and
  335 hit a stub. Of the 514 non-operator natives in A, **24 are implemented** (all Core: `GotoState`, `IsA`, `Len`, `Mid`, `Rand`,
  `VSize` ...), 490 are not.
* Per package (distinct natives, operators included; `impl` = implemented in `natives_core.cpp`):

| Package | dynamic (calls, stub calls) | closure A (impl) | closure B (impl) |
|---|---|---|---|
| Core | 38 (745, 5) | 94 (85) | 134 (113) |
| Engine | 18 (60, 60) | 136 (0) | 480 (0) |
| GFxUI | 12 (83, 83) | 30 (0) | 50 (0) |
| GearboxFramework | 9 (36, 36) | 70 (0) | 182 (0) |
| WillowGame | 49 (151, 151) | 247 (0) | 1,000 (0) |
| GameFramework | 0 | 0 | 1 |
| IpDrv | 0 | 0 | 18 |
| OnlineSubsystemSteamworks | 0 | 0 | 69 |
| AkAudio | 0 | 0 | 0 |
| **Total** | **126** | **577** | **1,934** |

By group (non-operator natives in closure A): WillowGame 213, Engine 136, GearboxFramework 70, GFxUI (Scaleform classes in
`GFxUI` and `WillowGame`) 64, Core 31. Owners with most: `MissionTracker` 49, `WillowPlayerController` 34, `Actor` 25,
`GFxMoviePlayer` 18, `AIComponent` 16, `MissionDefinition` 11, `GFxObject` 11, `WillowItem` 11, `GearboxGFxMovie` 10, `WillowGlobals` 10,
`WillowHUDGFxMovie` 10, `BehaviorKernel` 9, `WillowLeviathanService` 9.

Mission-specific natives in closure A (dynamic calls / closure A sites): `MissionTracker` `ActivateMission` (2/1), `CompleteMission`
(1/1), `PlayTurnIn` (1/1), `UpdateObjective` (0/11), `IsDataValid` (6/8), `GetMissionStatus` (2/7), `SetKickoffHeard`, `SetActiveMission`,
`SendMissionData`, `GrantMissionWeaponsToClientPlayer`, the `Remote*` receivers (`RemoteUpdateMissionStatus`, `RemoteUpdateMissionObjective`,
`RemoteUpdateActiveObjectiveSet`, `RemoteSubObjectiveSetAdvanced`, ...), `StartMissionTimer`/`StopMissionTimer`;
`MissionDefinition` `GetExperienceReward` (2/3), `GetCurrencyReward` (2/4), `GetCurrencyRewardType`, `GetItemRewardsForPlayer`,
`GetOptionalCreditReward`, `GetMissionRewardPresentation`, `ShouldGrantAlternateReward`; `WillowPlayerController` `ExpEarn` (0/1),
`GetCurrentPlaythrough` (32/22), `NativeGetMissionIndex` (17/13), `UpdateLcdMissionStatus` (10/8), `GetExpPointsRequiredForLevel`,
`GetMaxExpLevel`; `WillowPlayerReplicationInfo.AddCurrencyOnHand` (0/2); `WillowLeviathanService` `Record*EventForPlayer`;
`BehaviorKernel.ActivateBehaviorOutputLink` (4/18), `BehaviorBase.RunBehaviors`, `GearboxGlobals.GetBehaviorKernel` (6/6).

### Top 40 by dynamic calls (non-operators; "dyn entries" = entries that reached it; sites A/B = closure call sites)

| # | native | package | dyn calls | dyn entries | sites A | sites B | impl | group |
|---|---|---|---:|---:|---:|---:|---|---|
| 1 | WillowPlayerController.GetCurrentPlaythrough | WillowGame | 32 | 28 | 22 | 28 | no | WillowGame |
| 2 | GFxMoviePlayer.ActionScript | GFxUI | 19 | 19 | 13 | 35 | no | GFxUI |
| 3 | WillowPlayerController.NativeGetMissionIndex | WillowGame | 17 | 17 | 13 | 14 | no | WillowGame |
| 4 | GearboxGFxMovie.PlayUISound | GearboxFramework | 16 | 12 | 23 | 138 | no | GearboxFramework |
| 5 | GFxMoviePlayer.GetPC | GFxUI | 16 | 9 | 16 | 47 | no | GFxUI |
| 6 | Actor.SetTimer | Engine | 11 | 11 | 14 | 82 | no | Engine |
| 7 | GFxMoviePlayer.SetVariableString | GFxUI | 10 | 5 | 18 | 53 | no | GFxUI |
| 8 | GFxMoviePlayer.GetVariableObject | GFxUI | 10 | 8 | 10 | 31 | no | GFxUI |
| 9 | WillowPlayerController.UpdateLcdMissionStatus | WillowGame | 10 | 10 | 8 | 8 | no | WillowGame |
| 10 | WorldInfo.IsMenuLevel | Engine | 10 | 10 | 3 | 26 | no | Engine |
| 11 | WillowPlayerController.GetHUDMovie | WillowGame | 9 | 8 | 9 | 18 | no | WillowGame |
| 12 | PlayerController.IsPrimaryPlayer | Engine | 8 | 7 | 13 | 83 | no | Engine |
| 13 | WillowGFxMovie3D.FocusOn | WillowGame | 8 | 6 | 1 | 3 | no | GFxUI |
| 14 | GFxMoviePlayer.Close | GFxUI | 7 | 7 | 11 | 101 | no | GFxUI |
| 15 | MissionTracker.IsDataValid | WillowGame | 6 | 6 | 8 | 12 | no | WillowGame |
| 16 | GearboxGlobals.GetBehaviorKernel | GearboxFramework | 6 | 6 | 6 | 6 | no | GearboxFramework |
| 17 | WillowPlayerController.PlayUIAkEvent | WillowGame | 5 | 5 | 8 | 9 | no | WillowGame |
| 18 | GFxMoviePlayer.SetVariableNumber | GFxUI | 5 | 4 | 6 | 11 | no | GFxUI |
| 19 | WillowAIPawn.IsComponentUsable | WillowGame | 5 | 5 | 1 | 2 | no | WillowGame |
| 20 | QuestAcceptGFxMovie.UpdateMissionTextList | WillowGame | 5 | 5 | 1 | 1 | no | GFxUI |
| 21 | GFxMoviePlayer.ResolveDataStoreMarkup | GFxUI | 4 | 4 | 19 | 61 | no | GFxUI |
| 22 | BehaviorKernel.ActivateBehaviorOutputLink | GearboxFramework | 4 | 4 | 18 | 18 | no | GearboxFramework |
| 23 | GFxMoviePlayer.SetVariableBool | GFxUI | 4 | 4 | 9 | 43 | no | GFxUI |
| 24 | BehaviorBase.GetBehaviorContext | Engine | 4 | 2 | 6 | 6 | no | Engine |
| 25 | Object.GetStateName | Core | 4 | 4 | 2 | 9 | yes | Core |
| 26 | WillowPlayerController.CanAffordToUseUsableObject | WillowGame | 4 | 4 | 2 | 2 | no | WillowGame |
| 27 | Object.Localize | Core | 3 | 2 | 71 | 238 | no | Core |
| 28 | GearboxGFxMovie.SingleArgInvokeS | GearboxFramework | 3 | 3 | 12 | 38 | no | GearboxFramework |
| 29 | AttributeInitializationDefinition.EvaluateInitializationData | Engine | 3 | 3 | 11 | 46 | no | Engine |
| 30 | BehaviorBase.RunBehaviors | Engine | 3 | 3 | 8 | 19 | no | Engine |
| 31 | Actor.ClearTimer | Engine | 3 | 3 | 6 | 57 | no | Engine |
| 32 | GFxMoviePlayer.GetVariableNumber | GFxUI | 3 | 2 | 6 | 25 | no | GFxUI |
| 33 | BehaviorBase.GetBehaviorContextInterface | Engine | 3 | 3 | 5 | 5 | no | Engine |
| 34 | Object.VSizeSq | Core | 3 | 2 | 3 | 5 | yes | Core |
| 35 | BehaviorBase.GetWorldInfo | Engine | 3 | 3 | 3 | 3 | no | Engine |
| 36 | BehaviorBase.StaticGetBehaviorContext | Engine | 3 | 3 | 1 | 1 | no | Engine |
| 37 | Object.IsA | Core | 2 | 2 | 8 | 101 | yes | Core |
| 38 | WillowPlayerController.GetPawnInventoryManager | WillowGame | 2 | 2 | 8 | 32 | no | WillowGame |
| 39 | MissionTracker.GetMissionStatus | WillowGame | 2 | 2 | 7 | 14 | no | WillowGame |
| 40 | GFxMoviePlayer.Advance | GFxUI | 2 | 2 | 4 | 32 | no | GFxUI |

Core operators (80 distinct, 31 reached dynamically, 730 dynamic calls, 63 in A, 80 in B; 77 implemented, the three
vector/rotator operators `>>`, `<<` and `+=` on rotators are not). Leading ones: `NotEqual_ObjectObject`, `EqualEqual_IntInt`, `Not_PreBool`,
`Less_IntInt`, `Greater_IntInt`.

### Not implemented, by closure A sites (first 30 of the work list)

| # | native | package | dyn calls | dyn entries | sites A | sites B | impl | group |
|---|---|---|---:|---:|---:|---:|---|---|
| 1 | Object.Localize | Core | 3 | 2 | 71 | 238 | no | Core |
| 2 | GFxObject.GetObject | GFxUI | 0 | 0 | 50 | 106 | no | GFxUI |
| 3 | Object.QueryInterface | Core | 1 | 1 | 46 | 65 | no | Core |
| 4 | GFxObject.ActionScriptVoid | GFxUI | 0 | 0 | 29 | 112 | no | GFxUI |
| 5 | WillowPlayerStats.IncrementIntStat | WillowGame | 0 | 0 | 26 | 46 | no | WillowGame |
| 6 | GFxObject.SetString | GFxUI | 0 | 0 | 25 | 35 | no | GFxUI |
| 7 | WillowGlobals.GetGlobalsDefinition | WillowGame | 0 | 0 | 24 | 96 | no | WillowGame |
| 8 | GearboxGFxMovie.PlayUISound | GearboxFramework | 16 | 12 | 23 | 138 | no | GearboxFramework |
| 9 | WillowGlobals.GetWillowGlobals | WillowGame | 0 | 0 | 23 | 138 | no | WillowGame |
| 10 | WillowPlayerController.GetCurrentPlaythrough | WillowGame | 32 | 28 | 22 | 28 | no | WillowGame |
| 11 | GFxMoviePlayer.ResolveDataStoreMarkup | GFxUI | 4 | 4 | 19 | 61 | no | GFxUI |
| 12 | GFxObject.SetBool | GFxUI | 0 | 0 | 19 | 33 | no | GFxUI |
| 13 | GFxMoviePlayer.SetVariableString | GFxUI | 10 | 5 | 18 | 53 | no | GFxUI |
| 14 | BehaviorKernel.ActivateBehaviorOutputLink | GearboxFramework | 4 | 4 | 18 | 18 | no | GearboxFramework |
| 15 | WorldInfo.AllControllers | Engine | 0 | 0 | 18 | 57 | no | Engine |
| 16 | GFxMoviePlayer.GetPC | GFxUI | 16 | 9 | 16 | 47 | no | GFxUI |
| 17 | Actor.SetTimer | Engine | 11 | 11 | 14 | 82 | no | Engine |
| 18 | Actor.Destroy | Engine | 0 | 0 | 14 | 60 | no | Engine |
| 19 | GFxMoviePlayer.ActionScript | GFxUI | 19 | 19 | 13 | 35 | no | GFxUI |
| 20 | WillowPlayerController.NativeGetMissionIndex | WillowGame | 17 | 17 | 13 | 14 | no | WillowGame |
| 21 | PlayerController.IsPrimaryPlayer | Engine | 8 | 7 | 13 | 83 | no | Engine |
| 22 | GearboxGFxMovie.SingleArgInvokeS | GearboxFramework | 3 | 3 | 12 | 38 | no | GearboxFramework |
| 23 | Actor.LocalPlayerControllers | Engine | 0 | 0 | 12 | 39 | no | Engine |
| 24 | Actor.SetOwner | Engine | 0 | 0 | 12 | 24 | no | Engine |
| 25 | GFxTextListContainer.GetTextEntryKindAtIndex | WillowGame | 0 | 0 | 12 | 12 | no | GFxUI |
| 26 | GFxMoviePlayer.Close | GFxUI | 7 | 7 | 11 | 101 | no | GFxUI |
| 27 | AttributeInitializationDefinition.EvaluateInitializationData | Engine | 3 | 3 | 11 | 46 | no | Engine |
| 28 | MissionTracker.UpdateObjective | WillowGame | 0 | 0 | 11 | 12 | no | WillowGame |
| 29 | GFxMoviePlayer.GetVariableObject | GFxUI | 10 | 8 | 10 | 31 | no | GFxUI |
| 30 | Actor.Spawn | Engine | 0 | 0 | 10 | 40 | no | Engine |

## Verification

Automated only; no in-game check. `ctest --test-dir build -C Release --output-on-failure`: 10/10 passed (the VM synthetic test now
also covers the counters: implemented operator calls 4 and 6 for a loop of 3, a stub counted once and not as implemented,
a `Context` on `None`, per-entry reset, step-limit stop, closure A vs B on a virtual call, `--no-static`).
`python tools/verify_packages.py --reader build/Release/ow-package.exe`: all nine packages match. Release build without
warnings. Changes touch `CMakeLists.txt` (one source file added to `ow-core`) and the `vm::Runtime` header (new members, so
the UE module must be rebuilt against the new `ow-core.lib`); no change to `package.cpp`, `container.*` or any bounds
check. Everything above that depends on game script stays an estimate until a real-game trace confirms it.
