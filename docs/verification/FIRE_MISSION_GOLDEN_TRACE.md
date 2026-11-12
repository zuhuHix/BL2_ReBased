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
   `VO_NPC_MissionTurnedIn`), `PlayTurnIn` (`Default` link 14). Experience rose by 395, level stayed 8. Credits before
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
another lane at 02:02, so it is not the current binary) in `local/realgame/L2/`. The experience total in `player:8:<total>` is shown here as the invented 21,000 that
`tools/verify_fire_event_order.py` uses (level 8 runs from 20,208 to 28,126, so the +395 keeps her there); the run itself used the
traced save's total, which stays in the maintainer's private record:

```
ow-package-copy.exe Sanctuary_Dynamic.upk --slice-run GD_Z1_RockPaperGenocide.M_RockPaperGenocide_Fire --cooked <CookedPCConsole> \
  stage:8 player:8:21000 use accept tick:0.02 tick:0.1 tick:0.5 tick:1 range tick:1 spawn tick:1 hit:fire tick:0.02 tick:2.5 tick:1 use turnin tick:0.1 tick:1
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
  source 4; no currency, no item reward; Complete; experience pool +395 with the level unchanged.
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

## After swap 8 (lane I6, 2026-10-07)

AI-assisted (Claude), implementer lane I6. The comparison was rerun first with the **current** binary (after swaps 7b and the gun-socket change; the CLI's step list is
the one above), then the VM was changed where the trace and the data show a difference, then it was rerun. Method and rules are in
[SANCTUARY_RPG_MISSION.md](SANCTUARY_RPG_MISSION.md) ("Script swap 8"); the order the VM must now keep is stored as rules in `tools/verify_fire_event_order.py` (event
kinds and names in an order, no game values; it needs the installed game; `--self-test` runs it on an invented stream).

**Before (the current binary).** The host-event stream was the one lane L2 compared, to the item: the weapon granted with the second set and removed at the hit, no
`AIHold_57` in the dummy's trace, no turned-in dialog. **38 matched, 4 out of order, 5 missing, 4 extra**, as above, with one correction to the "missing" list: the 1 s
before `RegisterTargetable_36` was already in the VM (see below); it only could not be seen, because the CLI printed no sequence changes and the dummy trace has no times.
The checker's 18 rules held 9 times on that stream.

**After.** 43 matched, 2 out of order, 2 missing, 4 extra (live side 47 items: 43 + 2 + 2; VM side 49: 43 + 2 + 4). The checker's 18 rules hold 18 times on the real data.

| Item | Before | After | How |
|---|---|---|---|
| Mission weapon grant | out of order (with the second set) | **matched** | inside `ActivateMission`: after the status hook and delegates, before the initial set; host: into the backpack, not equipped |
| Mission weapon removal | out of order (at the hit) | **matched** | inside `CompleteMission`: after the status hook (experience) and before the reward; not at ReadyToTurnIn |
| `AIHold_57` after `SpecialMove_58` | missing | **matched** | `Behavior_SpecialMove`'s script selects output 0 on its first run; the link to `AIHold_57` is on that output |
| `ChangeRemoteBehaviorSequenceState_171` and its 1 s | missing | **matched** | data link `AdvanceObjectiveSet_96 -> _171`, default output, `ActivateDelay` 1.0; already run by the VM, now a visible `sequence_change` event |
| `PlayMissionTurnedInDialog` / `VO_NPC_MissionTurnedIn` | missing | **matched** | the screen's open (`BeginUse`) is emulated, so `OnPlayerTurnedInMission` runs its dialog; new `turn_in_dialog` event; the stock event is silent (its switch takes its unlinked last output) |
| Dialog 03b against the `FinalObj` set; dialog 05 against `FireCompleted` | out of order (2) | out of order (2), **explained** | not a VM defect: see below |
| Accept-time screen close; `MissionRewardsReceived` on confirm | missing (2) | missing (2) | host UI, left alone as asked; **matched since swap 11, see "After swap 11" below** |
| Extra: `DET_NPC_OnUse_MissionsAvailable`, lines 03a and 04, `AttemptStatusEffect_9` | 4 | 4 | unchanged |

**The two "out of order" dialog items are what the event marks, not an order the VM got wrong.** The installed script of `Behavior_TriggerDialogEvent` (read with
`--disasm`): on its first run (not forced) it sets `NextExecutionDelayTime` to 0.001 and selects Out (id 0); only the resumed run, one kernel wake later, triggers the
dialog. The live hook sees the behavior's first application (before `AdvanceObjectiveSet_96` in the same frame); the line itself starts one wake later in the real game
too, but the line is native and was not visible. The VM's `dialog` event marks the line's start, so it follows. No change was made. One new observation from the
data: `_1186`'s Finished output (id 1) leads to `_1184 -> _1183` (the lines 03a and 04 in the VM). The live trace never applied `_1184` or `_1183` in the remaining 155 s of the recording, so in the
real game `_1186`'s Finished was never selected in that window: the line 03b did not end, or never started. The VM selects Finished at once ("no talker"), which is the
reason for its two extra lines. Cause not isolated (UNVERIFIED); it needs a native-level view of the dialog manager.

**New things the stream shows.** `sequence_change` (provider, sequence, action) for the behaviors that change the dummy's sequences: the mission's `_171` (Targetable
enabled, in the first tick after the spawn, 1 s after the set), the dummy's own `_84` (ObjectiveComplete enabled at the hit) and `_64` (disabled when the target is
sent back). `turn_in_dialog` after `reward`.

**Newly reached stubs** (real data `--slice-run`): `AIComponent.ActivateEvent` (the `Used` event in `BeginUse`), and from the same call, through `OnNewPrimaryUser ->
StartLingerTimer` (installed script, read with `--disasm`): `Actor.ClearTimer`, `Actor.SetTimer` (now x2) and `EvaluateInitializationData` with a context source other than
the player controller (the pawn's `TimeUntilConsideredLingering`; not implemented). All are presentation or timers for a screen that is not hosted; none changes an event.

## Dialog lines without the dialog skipper (lane L4, 2026-10-07)

AI-assisted (Claude), real-game lane L4. Question (work queue item 3): why did the live L2 run never apply `Behavior_TriggerDialogEvent_1184`
and `_1183` (the lines 03a and 04 behind `_1186`'s Finished output), while the VM plays them? Method as in lanes L1 and L2 (one game process
under `local/ue_run.lock`, saves and the SDK settings folder backed up and hashed first, `Install-Driver -BlockSavesAtStart`, level-8 Maya from
`-Character=Save0008.sav`, memory-only, game closed through its window). Two launches: **launch 1 with the installed Dialog Skipper mod
disabled in memory** (the unmodded dialog), **launch 2 with the mod on** (the L2 condition, replayed with a recorder that can see the dialog).
New tool: `tools/real_game/scripts/dialog_probe.py` (loaded after `fire_mission_probe.py`; adds hooks on `Behavior_TriggerDialogEvent.TriggerDialogEvent`,
`GearboxDialogEventData.IsActive`, `TalkStarted` / `TalkFinished` of the talk acts, and a per-frame poll of the dialog behaviors' event data, the
dialog components' `IsTalking` and the manager's echo actor). Raw rows (29,452 and 15,892) and screenshots stay under ignored `local/realgame/L4/`.
Labels as above: **observed** = a hook row or a live read; **UNVERIFIED** = inferred.

### What the skipper does (read from the mod's own Python, in place; nothing copied)

The installed mod (Dialog Skipper 2.3, GPL3, options "Autoskip Dialog" on and "Skip All Dialog" off) has three hooks. (a) Before
`Behavior_TriggerDialogEvent.TriggerDialogEvent` it blocks the call when the context object is a non-hostile pawn or an echo actor; (b) after a talk act
starts (`TalkStarted`) it calls the talker's component `StopTalking` when the talker is a non-hostile pawn or echo actor; (c) before
`GearboxDialogComponent.TriggerEvent` it blocks the call when the component's owner is such a pawn. The mission's dialog behaviors run with the mission
tracker as context, so (a) does not block them; (b) stops the line at its first instant (observed: `StopTalking`, then `TalkFinished`, then
`TalkStarted` rows in one frame, and the event data's `IsActive` false at once). (c) fits why Marcus's on-use line was never heard in L1 and L2 (launch 2: the `TriggerEvent` row is there, no talk act of his group starts; UNVERIFIED that the block is the only reason).
No other installed mod hooks dialog functions (the other `Dialog` matches in `sdk_mods` are UI boxes).
The mod was switched off with `Mod.disable(dont_update_setting=True)` (no settings write); the settings folder is byte-identical after the session.

### Observed: the unmodded Fire dialog (launch 1, game seconds)

| Step | Line (talk act) | Speaker | Starts | Ends | Length |
|---|---|---|---|---|---|
| kickoff 01 (`_1185`) | `..._01_EchoX_Marcus` (act 1252) | Marcus pawn | 79.854 (accept 79.762) | 97.736 | 17.88 s |
| set `GoToRange_ObjSet`, `RocksPaper_MoveMarcusToRange` | | | 97.746 (+0.010 after the end) | | |
| 02 (`_1188`, +0.510 s link delay) | `..._02_EchoX_Marcus` (act 1253) | Marcus pawn | 98.274 | 104.038 | 5.76 s |
| 03b (`_1186`, range touch) | `..._03b_echoX_Marcus` (act 67) | Marcus pawn | 152.244 (+0.041 after `_1186`) | 158.837 | 6.59 s |
| 03a (`_1184`, applied 0.073 s after 03b's end) | `..._03a_LiveHypEng` (act 108) | the target dummy pawn (`WillowAIPawn_42`) | 158.933 | 160.945 | 2.01 s |
| 04 (`_1183`, applied 0.013 s after 03a's end) | `..._04_EchoX_Marcus` (act 1255) | Marcus pawn | 160.980 | 165.993 | 5.01 s |
| on-use line at the offer | `DET_NPC_OnUse_MissionsAvailable` (act 115 of Marcus's group, event 2 of 4) | Marcus pawn | 76.100 | 79.198 | 3.10 s |

- **The whole chain plays**: `_1186`'s Finished is selected, `_1184` is applied (initial run), plays 03a with the dummy as speaker, `_1183` follows and plays 04 (it
  has no linked outputs, so the chain ends). Lines 03a and 04 are real, not VM artefacts.
- **Poll behaviour (observed)**: the behavior's resumed runs are separate applications of a transient copy of the behavior (`Transient.Behavior_TriggerDialogEvent_N`, not the data
  object `_118x`); L2 filtered on the data names and so saw only the first run of each. The first resumed run is 35 to 90 ms after the first run (it starts the line); then the
  behavior calls `IsActive` about every 0.103 s while the line is live and selects Finished on the first call that
  returns false (kickoff: 0.010 s after the audio ended; 03b: 0.073 s). The talk act's `TalkFinished` row precedes the false `IsActive`.
- **Unmodded delays that L2 compressed**: accept to the first objective set 17.98 s (L2: 0.1 s); first set to line 02 0.510 s; range touch to line 03b 0.041 s and to the end of 04 13.79 s.
  Line lengths are the audio lengths (the host's `--slice-run` stand-in length is not these).
- **`DET_NPC_OnUse_MissionsAvailable` is raised** (observed): in launch 1 the talk act that is event 2 of `DialogGroup_NPC_Marcus` (the group's four events are `DET_NPC_OnUse_AllMissionsInProgress`,
  `DET_NPC_OnUse_MissionsAvailable`, `DET_NPC_OnUse_NoMissions`, `DET_NPC_PlayerLingeringInMenu`; act 115 is the second) started with Marcus as speaker inside the `VO_NPC_OnUse_MissionsAvailable`
  `TriggerEvent` call (between its pre and post rows, same frame as `UseObject`, before `OnPlayerOpenedMissionUI`) and ran 3.10 s. So the VM's extra `dialog` event for it is a
  real event; L1 and L2 did not see it, which fits the skipper blocking that `TriggerEvent` for Marcus.

### Why L2 never saw `_1184` and `_1183` (observed; the mechanism is UNVERIFIED in detail)

Launch 2 (skipper on) reproduces L2: in the 32 s after the range touch `_1184` and `_1183` were never applied (0 rows). What the rows show at the moment of `_1186`'s resumed run:

- the skipper stops 03b at once (`StopTalking` in the same frame; `IsActive` false), so `_1186` selects Finished in its **first resumed run, 35 ms after its first run**, in the same frame in
  which the target dummy spawns: in both launches the dummy's spawn behaviors (`SpecialMove_58`, `AIHold_57`, `Transform_12`, ..., `RemoteEvent_60`) and the Kismet `SeqEvent_PopulatedActor_2` run **after** the dialog
  behavior's resumed run in that frame;
- `_1184` is not a plain mission-context behavior: its `LinkedVariables` hold one entry, the provider's only variable `TargetDummy` (type `BVAR_NamedKismetVariable`), and live it is applied with the **dummy pawn as its context
  object** (unmodded: `ContextObject = WillowAIPawn_42`, the `CharClass_TargetDummy` pawn). The Kismet object variable named `TargetDummy` (`SeqVar_Object_0` in the `RocksPaperGenocide` sequence) holds that pawn after the spawn
  (read live), and `TargetDummy_Bot` is empty;
- so, with the line skipped, Finished is selected before the pawn has been written to that Kismet variable; the link's target has no context object, is not applied, and `_1183` (behind `_1184`'s Finished) never starts.
  UNVERIFIED: that the variable is empty at that instant (not read at that frame; the ordering of the rows fits it) and that an empty context list is what drops the application (the kernel's rule for a context-linked behavior with no
  object was not read in this lane). With a line of normal length (6.59 s) the variable is long set and the chain runs.

So the skipper is the cause, through the line-length dependence of a race with the dummy's spawn, not through a blocked or interrupted event data. The unmodded game has no such gap.

### Consequences for the VM (code pointers in the lane handoff)

- Lines 03a and 04 and the `DET_` dialog are real and in the live order: **extra items 1 to 3 of the L2 comparison move to "matched in kind"**; only `AttemptStatusEffect_9` (hit model) stays extra.
- Differences that remain: the VM runs `_1184` with the mission as its context (no `TargetDummy` resolution), so its talker is not the dummy; in the VM 03b, 03a and 04 take no time when the host plays no audio ("no talker" / no
  audio device selects Finished at once), live they take 6.59, 2.01 and 5.01 s; and the VM has no rule for an empty context ending a chain.
- Observed order inside the frame (both launches): `_1186` first run (Out: `AdvanceObjectiveSet_96`), one kernel wake later `_1186`'s resumed run (the line starts), then the dummy's spawn behaviors.

### Not done / not observed

- A direct test of the race (for example holding the dummy's spawn back a frame) was not made; the mechanism above is the reading that fits every row.
- The skipper-on control was a replay of the whole mission start (launch 2), not a re-trigger of one line: a first attempt to re-trigger line 03b from Python (`WillowDialogManager.TriggerGroupEvent` with `None` for the object
  parameters, to compare the skipper on and off on one line) **crashed the game** (pyunrealsdk fatal error dialog, in launch 1, after the main data had been dumped). Nothing was written (saves blocked, hashes below); the
  call was not tried again. The crash was in the call's arguments (UNVERIFIED which), not in a stale reference.
- 03a's speaker is the dummy pawn by the `TalkStarted` row; the audio itself was not captured. The kickoff, 02 and the on-use lines were only observed as hook rows.
- The mission was not played past 03a / 04 / the skipped replay (the Fire hit and the turn-in are L2's).

### Save safety (this session)

`Backup-Saves` (`local/realgame/save-backup-20261007-125046`) copied `SaveData` and `Config`; 33 files hashed before and after **each** launch (the first ended in the crash above); the SDK mods'
settings folder (19 files) copied and hashed too. With saves blocked from the first frame (`save_blocked` marks in the driver's event log), **all 33 files are byte-identical after both launches and all 19 settings files
are unchanged** (the mod's `enabled` flag was never written: `dont_update_setting=True`); re-checked after the lane's release, while lane L3's game ran (read-only hashing): settings still identical. The driver was removed; the game was closed through its window after launch 2 (after the crash dialog in launch 1: Enter on
its OK button, no kill). The run lock was taken at 12:50:34 and released by this lane after the session; this lane spawned nothing.

## After swap 11 (lane I10b, 2026-10-08): the mission screen's close and the reward page

AI-assisted (Claude), implementer lane I10b. The two items left "missing" after swap 8 are the screen's close at the accept and `MissionRewardsReceived` on the
player's confirmation. Both are now in the VM's event stream (`dismissal_dialog`, `mission_interface_closed` at the accept; `reward_page` at the turn-in, then
`reward_received` on a `confirmreward` step), so the two items become **matched in kind**. **After.** 45 matched, 2 out of order (the explained dialog items), 0 missing,
4 extra (live side 47 items: 45 + 2; VM side 51: 45 + 2 + 4). The checker's rules (24 now: the 18 of swap 8, two of them reworded for the new events, plus six new ones) hold 24 times on the real data.

**What the live trace shows (observed, lane L2 rows and the L4 run; installed script read with the disassembler).** At the accept the order inside `UpdateMissionStatus(Active)` is
`AddMission`, the movie's entry-list calls (`CanEndMission`, `CanStartMission`), `EndUse`, `PlayDismissalDialog` (tag `VO_NPC_GenericDismissal` raised on Marcus's dialog
component), `EndUse` again (the first one comes from `EndFocus`), `OnPlayerClosedMissionUI`; the status delegates and the lent weapon follow. The close is the movie's own script:
`ClientUpdateUIMissionList` -> `QuestAcceptGFxMovie.UpdateMissionList` -> `DetermineQuestEntries` -> `GotoCorrectPartOfMovie(False)`; the entry list is empty (the mission is now in
progress), so it plays the close sound and calls `GFxMoviePlayer.Close`, whose script event `OnClose` ends the use of Marcus. At the turn-in `ClientSpawnMissionRewardUI` shows the reward page
(`DisplayRewardsPage`), `MissionRewardsReceived` came 14.9 s later on the player's Enter (L2), and after the confirmation the screen did not close by itself (a screenshot of that run still lists "Shock Weapons").

**What the VM does now (all UNVERIFIED beyond that observed order).** `MissionScript::bindScreen` keeps a stand-in object of the installed class `QuestAcceptGFxMovie` and a `WillowGFxUIManager`; the
natives `WillowGFxUIManager.GetPlayingMovie`, `GFxMoviePlayer.Close` and `GearboxGFxMovie.PlayUISound` are host bindings (presentation, per NATIVE_GFX_BRIDGE.md), and the script
functions `DisplayRewardsPage` (waits) and `DetermineQuestEntries` (builds the rows in C++) are overridden. Everything else is the installed script: with the screen open the accept
reaches the native `Close`, which runs the emulated `OnClose` (`ServerPlayerEndUseNPC`, which runs `EndUse` and the dismissal dialog, then `OnPlayerClosedMissionUI`) inside the status routine,
so `dismissal_dialog`, `dialog`, `mission_interface_closed` precede the `status` event. Without an open screen (the old path, synthetic packages) nothing changes: the controller finds no movie
and `MissionRewardsReceived` still runs inside the turn-in. With it open the turn-in gives `experience`, `reward_page`, `status Complete`, `mission_weapon_removed`, `reward`, `turn_in_dialog`
and the reward waits; `confirmreward` gives exactly `reward_received`; `close` (Escape) runs the movie's `extChoiceCancelled` and gives the dismissal tag and `mission_interface_closed`.

*Added 2026-11-12:* in the real game with the SDK mods off (session 1, two runs of one path), Escape's release on the list after the reward closed the
screen through `OnClose` with no `extChoiceCancelled`. If the VM's `close` step stands in for that Escape, the game did not do it that way on this page.
That list may be the same state as the "next mission's offer" where one Escape press with the mods on closed the screen (not told apart). Whether
Escape on an offer before any accept calls `extChoiceCancelled` was not observed ([MISSION_SCREEN.md](MISSION_SCREEN.md), "The keys with the SDK mods off").

**Why the rows are built in C++.** The movie's `DetermineQuestEntries` grows arrays of structs from empty with `DynArrayInsert`; the VM keeps no element type for an empty array, so the elements would
be untyped. The override runs the same director calls (`GetRedeemableMissions`, `GetEligibleMissions`, `GetInProgressMissions`: the observed `CanEndMission` / `CanStartMission` rows) and builds
the rows with the rule the script has (redeemable first with status 3, then eligible with 0; in progress apart). The VM was not changed for this. (A first version of the override iterated a temporary and
left the list empty, which is why the close did not happen; fixed.)

**Unverified / differences.** One `EndUse` here against two live (the first comes from the controller's focus object, which the VM does not keep; the second finds the list already empty, so the effect is the
same). The dismissal line's talker: the live L4 run shows the tag starting a talk act with another NPC as talker; the VM's `dialog` event names the same act with `outcome=no talker`. The host's use key
stands in for both Enter (confirm) and the Escape that follows (a second press is not needed); in the real game the screen stayed open after the confirmation. All presentation (the page, the sound, the
post-process override, the focus camera, the preview inventory) is not modelled.
