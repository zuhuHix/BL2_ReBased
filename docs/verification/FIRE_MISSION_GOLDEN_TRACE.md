# Fire mission golden trace: the real game against the VM (lane L2, 2026-10-07)

AI-assisted (Claude), real-game lane L2. One full play-through of "Rock, Paper, Genocide: Fire Weapons!"
(`GD_Z1_RockPaperGenocide.M_RockPaperGenocide_Fire`) in the running original game, recorded by Python hooks on script-visible
functions (pyunrealsdk through the `tools/real_game/` driver), compared with our VM's `--slice-run` on the same mission. Method,
save safety and the short summary are in [REALGAME_GROUND_TRUTH.md](REALGAME_GROUND_TRUTH.md) ("Fire mission golden trace (lane
L2)"). The raw rows (about 5,500 per run), screenshots and VM outputs stay under ignored `local/realgame/L2/`; this record holds
rules, counts and classifications only.

Labels: **observed** = seen in the running game by a hook or a read of live state; **VM** = what `--slice-run` printed;
**UNVERIFIED** = a cause or rule inferred, not shown. A hook only sees calls that go through script (`ProcessEvent`); calls the game
makes from C++ to C++ are invisible, so "not seen" never means "did not happen".

## How it was driven

- Level-8 Maya, "Plan B" save, loaded with the startup mod's `-Character=Save0008.sav` and "Continue"; driver installed with
  `Install-Driver -BlockSavesAtStart`; one game process under `local/ue_run.lock`; saves backed up and hashed first.
- Real inputs: the use key at Marcus (the player was placed 110 units in front of him, looking at his chest: `SetRotation` kept
  pitch 0 in this build, `ClientSetRotation` worked, and the use prompt appears only when the camera ray hits him), Enter on the
  offer, Enter on the turn-in screens, the left mouse button for shots, key 3 for the weapon slot.
- Script calls and teleports (memory only): the player's location was set to the range next to Marcus (a property write does not
  raise a touch; one short step with the W key did, so the **waypoint touch, the objective update and every mission event came from
  the game's own path**), the lent pistol was moved from the backpack to slot 3 with `ReadyBackpackInventory`, and the camera was aimed
  at the dummy. The objective was completed by a real shot (four clicks; the first hit counted: one `UpdateObjective(Fire)` call).
- 394 hooks: 97 on non-behavior functions (mission tracker, player controller, mission definition, pawn and interactive-object
  callbacks, dialog component and manager, waypoint, inventory manager, Kismet `SequenceOp.Activated`, behavior kernel) and 297 on the
  `ApplyBehaviorToContext` of every `Behavior_*` class. 56 of the 97 and 37 of the 297 saw a call. Rows carry the QPC clock, the
  game's own time and a frame counter (frames were about 8 ms).
- The Dialog Skipper SDK mod was active (it is installed on this machine). Delays that depend on how long a spoken line lasts
  (the kickoff dialog to the first objective set took 79 ms here; the real line is about 17.7 s) are therefore **not** the unmodded
  timings. Delays inside behavior links (0.5 s, 1.0 s, 2.0 s, 3.0 s below) do not depend on a line and were not shortened.
- A first launch was a rehearsal: a re-load of the probe file reset the recorder, so its accept was not recorded (rows of the
  later part agree with the recorded run). The recorded trace is the second launch.

## The live sequence (observed rules)

Times are offsets from the first call of the step, game time. "Same frame" means one render frame (about 8 ms).

1. **Use key at Marcus (offer).** `WillowAIPawn.UseObject` then `FireOnUsedBehaviors(2)`, `AIClassDefinition.OnUsed`, the nine
   `IsBehaviorSequenceEnabled` checks in the known order (all false), then `Behavior_PlayAIMissionContextDialog` (which calls
   `PlayOnUseDialog`; the tag raised on his dialog component is `VO_NPC_OnUse_MissionsAvailable`), `Behavior_HasMissions`,
   `Behavior_ShowMissionInterface`, `OnPlayerOpenedMissionUI`, `BeginUse`, `FireOnUsedBehaviors(0)`. All in one frame.
2. **Accept (Enter on the offer): one frame.** `WillowPlayerController.AcceptMission` (script; `ServerAcceptMission` is not called
   for the host) calls `ActivateMission`; inside it, in this order: `UpdateMissionStatus(Active)` (whose script calls `AddMission`
   and closes Marcus's screen: `EndUse`, `PlayDismissalDialog` with tag `VO_NPC_GenericDismissal`, `OnPlayerClosedMissionUI`),
   `TriggerMissionStatusChangedDelegates`, **the lent weapon: `AddInventory`, `AddInventoryToBackpack`, `ShowMissionWeaponTraining`
   (a new item in the backpack, not equipped)**, `TriggerActiveMissionChangedDelegates`, the observers (the waypoint's
   `MissionReactionStatusChanged`), one behavior on an unrelated interactive object, nine pawn `MissionStatusChanged` calls. After
   `ActivateMission` returned: `OnPlayerAcceptedMission` on Marcus (no further visible effect). Matches the status-routine order in
   NATIVE_MISSION_SCRIPT_BRIDGE.md for Active (script hook, delegate, weapon, set active mission, observers).
3. **Kickoff, next tick (+9 ms).** `IsMissionMoviePlaying`, then the mission's `Behavior_TriggerDialogEvent_1185` (dialog 01, the
   `Default` link 12). `PlayKickoff` itself is not seen (native from the tick). +79 ms later (the skipped line ended):
   `Behavior_AdvanceObjectiveSet_95`, `TriggerMissionObjectivesChangedDelegates`, the waypoint's set-changed reaction (new set
   `GoToRange_ObjSet`), `Behavior_MissionRemoteEvent_320`, which activates the Kismet event `RocksPaper_MoveMarcusToRange`
   (`WillowSeqEvent_MissionRemoteEvent`) and Marcus's scripted-walk action. +0.505 s after that: `Behavior_TriggerDialogEvent_1188`
   (dialog 02). Marcus's own touch of the waypoint ~9 s later was delivered to `Touch` and caused nothing.
4. **Range (player touch).** `WillowWaypoint.Touch` (player pawn), `ProcessPlayerTouch`, `IsMissionObjectiveActive` (true),
   `UpdateObjective(RockPaper_GoToRange)`, the waypoint's objective-updated reaction, the controller's `UpdateMissionObjective`,
   `TriggerMissionObjectivesChangedDelegates`, then the waypoint's objective-complete reaction: one frame. **+0.505 s later, in one
   frame and in this order:** `Behavior_TriggerDialogEvent_1186` (dialog 03b), `Behavior_AdvanceObjectiveSet_96`, objectives-changed,
   the waypoint's set-changed (`RocksPaper_FinalObj`), a re-check of the touch (objective no longer active).
5. **Dummy spawn (+29 ms after the set).** On the dummy's own provider: `SpecialMove_58`, `AIHold_57`, `Transform_12`, `IntMath_2`,
   `ChangeInstanceDataSwitch_15`, `IntMath_3`, `ChangeInstanceDataSwitch_14`, `RemoteEvent_60` (Kismet `RocksPaper_MoveTargetForward`,
   then the populated-actor and populated-point events, `RegisterWaypoint` of the pawn's objective waypoint component, the target
   Matinee `SeqAct_Interp_0`). **+0.98 s later:** the mission's `Behavior_ChangeRemoteBehaviorSequenceState_171` and the dummy's
   `RegisterTargetable_36`.
6. **Fire hit (real shot), one frame.** `CompareObject_13`, `AIHold_59`, `ChangeRemoteBehaviorSequenceState_84`,
   `UpdateMissionObjective_13` (dummy provider), `UpdateObjective(Fire)`, waypoint objective-updated, controller
   `UpdateMissionObjective`, objectives-changed, waypoint objective-complete, **then** `UpdateMissionStatus(ReadyToTurnIn)`,
   `TriggerMissionStatusChangedDelegates`, the waypoint's status reaction, and the `Default` link 9 behaviors in this order:
   `Behavior_TriggerDialogEvent_1189` (dialog 05), `Behavior_MissionRemoteEvent_321` (`RocksPaper_FireCompleted`). **No weapon
   removal here: the lent pistol stayed in the player's inventory after the objective completed.** No `AttemptStatusEffect` ran
   (the first shot did not proc the weapon's 15.6 % fire chance; the objective completed anyway).
7. **After the hit (timed by behavior links, not by lines).** +2.002 s: the dummy's `RemoteEvent_44` (Kismet
   `RocksPaper_SendTargetBack`), `ChangeRemoteBehaviorSequenceState_64`, the reverse Matinee. +3.002 s: two
   `Behavior_MissionRemoteEvent_319` (`RocksPaper_TargetBack` twice). +4.0 s: Kismet `SetBool`, `Destroy` (the dummy),
   `ResetPopulationCount`, `Gate`.
8. **Use key at Marcus (turn-in).** Same chain as step 1, same answers; the tag is now `VO_NPC_OnUse_MissionComplete`.
9. **Turn-in (Enter on "Collect Reward"): one frame.** `ServerCompleteMission` -> `CompleteMission` -> `UpdateMissionStatus(Complete)`
   -> `ShouldGrantAlternateReward` (false) -> `ServerGrantMissionRewards`: `GetCurrencyRewardType` (0), `GetCurrencyReward` (0),
   `GetExperienceReward` (395), `ExpEarn(395, source 4, type 0)`, `GetItemRewardsForPlayer` (empty, as expected), then
   `ClientSpawnMissionRewardUI`; then `TriggerMissionStatusChangedDelegates`, **`RemoveMissionWeapons`, `RemoveFromInventory`,
   `ClientRemoveMissionWeapons`**, the waypoint's status reaction, `TriggerActiveMissionChangedDelegates` (the tracked mission was
   no longer the Fire mission: snapshot 13 shows the plot mission "Welcome to Sanctuary" tracked again); after `CompleteMission` returned: `OnPlayerTurnedInMission` on Marcus, `PlayMissionTurnedInDialog` (tag
   `VO_NPC_MissionTurnedIn`), `PlayTurnIn` (`Default` link 14). Experience went 26,218 -> 26,613, level stayed 8. Credits before
   XP before items before the reward UI, as the note says.
10. **Reward screen (Enter again, 14.9 s later in this run).** `MissionRewardsReceived`, called by the UI on the player's
    confirmation, not inside the turn-in. `AcceptOrSaveUnclaimedReward` was never called: with Marcus's mission screen open the
    reward page is drawn by the movie, which matches the note's second branch.
11. **Tracked mission (observed, snapshots):** the plot mission "Welcome to Sanctuary" was tracked before; accepting the side mission
    made the Fire mission the tracked one at once (the note's early-exit gate of `SetActiveMission` did not block it for this
    character); after the turn-in the plot mission was tracked again. The VM does not model the tracked mission.
12. **Mission status read after each step (`GetMissionStatus`):** 0 before, 1 after accept (progress 0/0, set `GoToRange_ObjSet`),
    1 with progress 1/0 and set `RocksPaper_FinalObj` after the range, 3 (progress 1/1) after the hit, 4 after the turn-in.

Not visible to script hooks, so **not observed**: the dialog lines themselves (0 calls of `GearboxDialogComponent.Talk`,
`TalkReplicated`, `GearboxDialogManager.TriggerGroupEvent` or `WillowDialogManager.PlayEchoDialog`; the mission's lines are
started natively by `Behavior_TriggerDialogEvent`, whose own application is seen), `SetMissionStatus` and `SetActiveMission` (native to
native), `PlayKickoff`, the native step of Marcus's on-use dialog lookup, and the Kismet receivers of `RocksPaper_FireCompleted` and
`RocksPaper_TargetBack` (the raising behaviors were seen, no `Activated` call followed in the hooked classes).
Dialog events that did reach script: Marcus's four on-use tags above, 8 pain reactions of the dummy, one idle line on the
player's component.

## The VM side

The VM run used a copy of `build/Release/ow-package.exe` (made 01:47, sha256 prefix `89dd649888ef84ee`; the build tree was rebuilt by
another lane at 02:02, so it is not the current binary) in `local/realgame/L2/`:

```
ow-package-copy.exe Sanctuary_Dynamic.upk --slice-run GD_Z1_RockPaperGenocide.M_RockPaperGenocide_Fire --cooked <CookedPCConsole> \
  stage:8 player:8:26218 use accept tick:0.02 tick:0.1 tick:0.5 tick:1 range tick:1 spawn tick:1 hit:fire tick:0.02 tick:2.5 tick:1 use turnin tick:0.1 tick:1
```

`hit:fire` is the VM's own fire-hit step (it needs no damage-type path). Fine-tick variants (0.01 s) were run for the windows
that matter: accept +0.5 s, range +0.5 s, fire hit +2 s and +3 s. All runs ended with no errors.

## Comparison: counts

A comparable item is something the live trace shows and the CLI can show: the host-event stream, the `use` cascade, the dummy
provider trace and the script report (the CLI does not print the VM's internal script-call order, so that is **not compared**).

- **Matched: 38.** Same event, same position relative to its neighbours, same arguments where there are any:
  the nine checks and the three behaviors of both uses, the two on-use tags (`MissionsAvailable`, `MissionComplete`) and the
  screen opening; accept to Active; dialog 01 on the next tick, the first set `GoToRange_ObjSet`, `RocksPaper_MoveMarcusToRange`,
  dialog 02 about 0.5 s later (VM 0.53 s, live 0.59 s after the accept); Marcus's touch ignored; range objective; `RocksPaper_FinalObj`
  0.5 s later (VM +0.51 s, live +0.505 s); the dummy's eight behaviors that both show (`SpecialMove_58`, `Transform_12`, `IntMath_2`,
  `ChangeInstanceDataSwitch_15`, `IntMath_3`, `ChangeInstanceDataSwitch_14`, `RemoteEvent_60`, `RegisterTargetable_36`) in the same
  order; `RocksPaper_MoveTargetForward`; the four hit behaviors (`CompareObject_13`, `AIHold_59`,
  `ChangeRemoteBehaviorSequenceState_84`, `UpdateMissionObjective_13`); Fire objective complete; ReadyToTurnIn after it;
  `RocksPaper_FireCompleted`; `RocksPaper_SendTargetBack` (VM 2.00-2.01 s, live 2.002 s) with the dummy's `RemoteEvent_44` and
  `ChangeRemoteBehaviorSequenceState_64`; `RocksPaper_TargetBack` twice (VM 3.00-3.01 s, live 3.002 s); `ExpEarn` 395 with
  source 4; no currency, no item reward; Complete; experience pool 26,218 -> 26,613 with the level unchanged.
- **Out of order or in the wrong place: 4.** Dialog 03b against the `FinalObj` set; dialog 05 against `RocksPaper_FireCompleted`;
  the mission weapon grant; the mission weapon removal.
- **Missing in the VM (live only): 5.** `AIHold_57` on the dummy's spawn; the mission's `ChangeRemoteBehaviorSequenceState_171`
  and the 1 s before `RegisterTargetable_36`; `OnPlayerTurnedInMission` -> `PlayMissionTurnedInDialog` (`VO_NPC_MissionTurnedIn`);
  the accept-time screen close (`EndUse`, `PlayDismissalDialog` `VO_NPC_GenericDismissal`, `OnPlayerClosedMissionUI`);
  `MissionRewardsReceived` on the player's confirmation (the VM clears the reward flag inside the turn-in).
- **Extra in the VM: 4.** The `dialog` event for `DET_NPC_OnUse_MissionsAvailable`; the lines `VOSQ_RockPaperGeno_03a_LiveHypEng`
  and `VOSQ_RockPaperGeno_04_EchoX_Marcus` after 03b; `Behavior_AttemptStatusEffect_9` with the `status_effect` event.

Live side: 38 + 4 + 5 = 47 items; VM side: 38 + 4 + 4 = 46.

## Differences and what they are

**Out of order**

- Dialog 03b / 05 (2 items): live, the dialog behavior is applied before the behavior that follows it in the same frame
  (`_1186` before `AdvanceObjectiveSet_96`; `_1189` before `MissionRemoteEvent_321`). The VM raises its `dialog` event one kernel wake
  (about 1/60 s) after the behavior, so the set and the remote event come first (03b: set at +0.51 s, line at +0.52 s; 05: line at
  +0.02 s). The two orders are the same behaviors at nearly the same time; what differs is **what the event marks** (live: the
  behavior's application; VM: the line's start). Class: **host-side** (dialog boundary), no rule to change in the script.
  UNVERIFIED that the VM applies the behaviors in the live order (its internal order is not printed).
- Mission weapon grant: live inside `ActivateMission` (backpack, after the status delegate, before `SetActiveMission`), VM with the
  second objective set. Class: **not modelled, already open** in SANCTUARY_RPG_MISSION.md ("the mission weapon at Active/Complete").
  The live trace now gives the position: the native does it, so a VM `ActivateMission` should add the weapon at that point.
- Mission weapon removal: live inside `CompleteMission` (after the status delegate, before the observers), VM at the Fire hit
  (ReadyToTurnIn). Same open item. Live fact: the lent pistol stays usable until the turn-in.

**Missing**

- `AIHold_57`: a behavior of the dummy that ran live between `SpecialMove_58` and `Transform_12` is absent from the VM's dummy trace
  while `AIHold_59` is present. Class: **VM gap** (cause not isolated: a sequence the VM does not enable, or a boundary class).
- Mission `ChangeRemoteBehaviorSequenceState_171` and the 1 s delay: live, `RegisterTargetable_36` ran 0.98 s after the dummy's
  spawn together with the mission's `_171`; in the VM it runs at spawn. Hypothesis (UNVERIFIED): the dummy's `Targetable` sequence is
  enabled by the mission's behavior after a link delay, not by an enable condition at spawn; a hit in the first second would
  then not count (not tested). Class: **VM bug candidate**, the only difference that could change gameplay timing.
- `PlayMissionTurnedInDialog`: live tag `VO_NPC_MissionTurnedIn` right after `CompleteMission`. Class: **not modelled, already
  open** (the turn-in step does not open the screen, so the director callback returns at its first test, as SANCTUARY_RPG_MISSION.md
  says). Now confirmed in game.
- Accept-time screen close and the reward confirmation: both belong to Marcus's mission screen (a movie) which the CLI accept /
  turn-in steps do not open. Class: **host-side**. The note's reward branches are right: with the movie open the page is drawn by
  the movie and `MissionRewardsReceived` waits for the player; without it (VM) the shortcut ends in `MissionRewardsReceived` at once.

**Extra**

- `DET_NPC_OnUse_MissionsAvailable` dialog: the second, native step of the use chain. Not visible live (the same limit as lane L1).
  Class: **unobserved**, UNVERIFIED either way; it was already listed as not observed.
- Lines 03a and 04: the VM plays them one wake after 03b. Live, no `Behavior_TriggerDialogEvent` applied them; they may be chained
  natively inside the dialog event, or not played. Class: **unobserved** (needs a native-level view of the dialog manager).
- `AttemptStatusEffect_9` and the `status_effect` event: the VM's `hit:fire` gives the hit an Incendiary result; live, the first
  shot did not proc and the behavior never ran, yet the objective completed. Class: **host-side** (the VM's hit model); rule:
  objective completion does not wait for the status effect. Whether a procced hit adds `AttemptStatusEffect_9` after
  `UpdateMissionObjective_13` was not observed.

Other notes: the CLI prints its own mission status numbers (ReadyToTurnIn is 2, Complete is 3); the game's `EMissionStatus` has
`RequiredObjectivesComplete` as 2 (ReadyToTurnIn 3, Complete 4). The script report's `player_status` is the game's number (4 at
the end, as live). Host-side naming only.

## What this changes for the notes (observed in game)

- NATIVE_MISSION_SCRIPT_BRIDGE.md: the status routine's order is **observed** for Active, ReadyToTurnIn and Complete (script hook
  first, then the changed-delegate, then the weapon grant or removal, then the active-mission delegate where it applies, then the
  observers and the `Default` event); the lent weapon goes to the **backpack** at Active and is removed by `RemoveMissionWeapons` at
  Complete; the reward order and the empty item result are observed; `ServerAcceptMission` is not used for the host; the reward
  confirmation is the UI's `MissionRewardsReceived`.
- Open item "the early-exit gate in `SetActiveMission`; the tracked-mission choice": for this character (level 8, plot mission tracked) the
  side mission became tracked on accept and the plot mission came back after the turn-in. One case only; the gate's test is still unknown.
- The objective path through the waypoint and the dummy's `UpdateMissionObjective` behavior is observed to match the VM's.
- Delays from behavior links match to within about 10 ms (0.505 s, 2.002 s, 3.002 s live; VM inside 0.01 s windows).

## Next steps the trace points to (ordered)

1. VM / host: grant the lent weapon in `ActivateMission` and remove it in `CompleteMission` (open item, now with the observed place).
2. VM: find out why `AIHold_57` is absent and whether `Targetable` should wait for the mission's `_171` and its delay.
3. Dialog: a native-level look (not a script hook) at whether 03a and 04 play, and at the `DET_` second lookup.
4. Turn-in: when the screen is opened by the VM, run `PlayMissionTurnedInDialog` and the movie-open reward branch.

## Reproduce

`tools/real_game/scripts/fire_mission_probe.py` (hooks, snapshots, stand-in-front) driven through `tools/real_game/realgame.ps1`
as in the lane L2 handoff: backup and hash the saves, `Enter-RunLock`, `Install-Driver -BlockSavesAtStart`, start with
`-Character=Save0008.sav`, press Enter, `fm_start()`, then the steps above with `fm_snap` between them, `fm_dump`, close the game
through its window, compare hashes, `Remove-Driver`, `Exit-RunLock`.
