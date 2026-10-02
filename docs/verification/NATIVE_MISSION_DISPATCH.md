# Native mission and behavior dispatch: first rule batch (2026-10-02)

AI-assisted. Behaviour notes written from a local reading of `Borderlands2.exe` in Ghidra, under the policy in
[LEGAL.md](../LEGAL.md) ("Analysing the executable") and [NATIVE_ANALYSIS.md](../NATIVE_ANALYSIS.md). Nothing below
is listing, pseudo-code or an address; functions are named by their registered native name or, for internal
routines, by what they do and which registered native reaches them. **Every rule here is `UNVERIFIED`**: it was
read from native code and has not been confirmed by running the game. One structural oracle over the installed
data is reported (link-id census, below); it is consistent with the reading and proves nothing more.

Field offsets in the native code were named with `tools/ghidra/class_layout.py` (offsets computed from the script
property chains; its oracle, `Core.Object` = 0x3C bytes with `Outer` at 0x28, matches the engine, and every
offset used below matched a field of the expected type).

Host code cited is the state at commit `877be8d`.

## A. Behavior kernel: event and output-link dispatch

### A1. Firing a behavior event

Reached from `BehaviorKernel.ActivateBehaviorEventFromScript` (registered native) and from every mission event the
tracker raises (section B). The call carries: the consumer, the provider definition (or none = any provider), an
event name, a **link-id filter** (−1 = all links), an optional output-variable payload, and instigator/filter
arguments.

What it does, in order:

1. Nothing happens unless the consumer is registered with the kernel and its process is running.
2. The consumer's process holds one state record per behavior sequence of its providers. Every sequence record
   whose provider matches (all of them when no provider is given) and that is **enabled** is visited, in the
   process's own sequence order.
3. Inside a sequence, **every** `EventData2` entry whose `EventName` equals the requested name is considered (not
   only the first). Each has a per-process runtime state (`TriggerCount`, `LastTriggerTime`, runtime filter).
   The entry is skipped when:
   - `bEnabled` is false;
   - `MaxTriggerCount` > 0 and `TriggerCount` has reached it;
   - `TriggerCount` ≥ 1 and less than `ReTriggerDelay` seconds have passed since `LastTriggerTime`;
   - a runtime filter object or the definition's `FilterObject` rejects the event (through a callback the caller
     supplies; not read further).
4. Otherwise `TriggerCount` is incremented, `LastTriggerTime` set to now, the event's output variables are
   published from the payload, and then the event's output links are walked **in data order**. A link is taken
   when the filter is −1 or the link's id byte equals the filter. Each taken link starts a new behavior thread at
   the linked behavior, due at now + the link's `ActivateDelay`.
5. A thread that is due immediately runs **at once, depth-first**, before the next link is considered. A thread
   that is not due waits in the kernel's waiting list.
6. If `bReplicate` is set and the game is networked, the event is also replicated (irrelevant single-player).

The link id byte is read **signed** (255 means −1). The packed `SubarrayData` word is start in the high 16 bits,
length in the low 16 bits, as the host already assumes.

### A2. Running a behavior thread and following its output links

A thread runs while it is due, its sequence is still enabled and its process is running, at most **60 behaviors
per call** (what happens to a thread cut off by that cap was not read). For each behavior:

1. The behavior's `ApplyBehaviorToContext` runs once per resolved context object (the context resolution loop
   was seen but not read in detail).
2. A behavior can ask to be **latent** by setting a wait time: the thread then waits (minimum 1/60 s) and resumes
   later. Otherwise it is finished.
3. While it runs, a behavior selects outputs by calling `BehaviorKernel.ActivateBehaviorOutputLink(id)`, which
   only **appends the id to a list** (any number of calls, duplicates kept, in call order).
4. When finished, if the behavior's `Context.bSupportsDefaultOutputLink` is set, −1
   (`LINK_ID_RESERVED_FOR_DEFAULT_BEHAVIOR_OUTPUT`) is appended to that list. The installed class defaults: true on
   `BehaviorBase`, overridden to false on `Behavior_TriggerDialogEvent` and `Behavior_CompareObject` (which select
   their outputs explicitly).
5. For each id in the list, in order, every outgoing link of the behavior whose (signed) id byte equals it is
   selected, in data order. Links whose id is not in the list are **not followed**.
6. The first selected link continues on the current thread (due at now + its delay); every other selected link
   starts a new thread immediately (depth-first if due). Net order with zero delays: links 2..n run to
   completion (or their first wait) **before** link 1 continues. No selected link ends the thread. Links selected
   by a behavior that is still latent all start new threads while the current one keeps waiting.
7. **No deduplication**: a behavior reached through two links (or two matching events) runs twice.
   `RecentlyRunBehaviorsForSequence` reads a bounded ring of execution records kept for debugging
   (`BehaviorExecutionRecord`), not a once-per-event set.

### A3. What the host does today, and the difference

| Host | Native reading |
|---|---|
| `src/behavior.cpp:291-309` (`fireEvent`) follows every link of the event, ignoring the id byte | the caller's id filter selects the links (section B lists the ids the tracker uses) |
| `fireEvent` ignores `bEnabled`, `MaxTriggerCount`, `ReTriggerDelay`, `FilterObject` | all four gate the event; `TriggerCount`/`LastTriggerTime` are per-process state |
| `src/behavior.cpp:373-401` (`run`) is one global queue ordered by due time then insertion: breadth-first | depth-first: a due thread runs to completion or a wait before the next link starts |
| `src/behavior.cpp:387` runs a behavior at most once per fired event | no dedup; every link activation runs the target |
| `src/behavior.cpp:399`: a handler returning `nullopt` follows **all** links | only links whose id is in the recorded list; −1 is added only if `bSupportsDefaultOutputLink` |
| `src/behavior.hpp:31-35` reads the id byte as the output it belongs to | consistent; the byte is signed |

### A4. Kismet (`SequenceOp`) impulses

Reached from the registered natives `SequenceOp.ActivateOutputLink`, `SequenceOp.ForceActivateInput`,
`SequenceOp.ForceActivateOutput`, the sequence's per-frame processing of its active ops, and sequence-event
activation.

- `ActivateOutputLink(i)` only sets output `i`'s `bHasImpulse`, unless the output is `bDisabled` (or
  `bDisabledPIE` in the editor); it returns whether it did.
- Activating an event: records originator and instigator, stamps `ActivationTime` and increments
  `TriggerCount`; if the event is not already active it becomes active, its native and script `Activated` run,
  and impulses are set on **all** its outputs (or on the given output indices), disabled outputs excepted; then
  it is queued. An event that is already active records a queued activation in its parent sequence instead.
  Before that, an activation check refuses the event when its `MaxTriggerCount` (> 0) is reached, when it was
  triggered less than `ReTriggerDelay` seconds ago, or when its player/network-side flags do not match.
- Per-frame processing of a sequence, in order:
  1. Delayed activations count down; when one expires its target input gets an impulse (unless the input is
     `bDisabled`), and if that input already had one its `QueuedActivations` is incremented; the op is queued.
  2. The active list is a **stack**: the most recently queued op is taken first; at most the caller's step limit
     and never more than **1000** ops per frame.
  3. An op that is not active becomes active (`ActivateCount` + 1, native and script `Activated`). An active op
     with `bSupportsMultipleActivations` whose first input has an impulse is activated again.
  4. `UpdateOp` runs; when it reports done the op is deactivated (native and script `Deactivated`). A non-latent op
     that is still active after its update does not propagate outputs this step.
  5. Every output with an impulse is walked link by link: delay = **target input's `ActivateDelay` + output's
     `ActivateDelay`**; a positive delay goes to the delayed list, otherwise the link is collected.
  6. Inputs: an input with `QueuedActivations` > 0 (and the op not latent) is decremented and the op is queued again;
     otherwise its impulse and count are cleared. All output impulses are then cleared.
  7. The collected links are applied **last to first**, each pushing its target on top of the stack, so the
     **first link's target runs next** (depth-first, in link order). Queueing is unique: an op already in the list
     keeps its place.

Host today: `src/kismet.cpp:87-112` (`fire`) uses only the output's `ActivateDelay` and ignores the target input's
delay and `bDisabled`; `src/kismet.cpp:173-191` (`run`) executes in due-time-then-FIFO order (breadth-first);
`src/kismet.cpp:119-125` (`activateEvent`) fires only the `Out` output; event `MaxTriggerCount`/`ReTriggerDelay`
are not checked; the runaway guard is 10,000 impulses in total rather than 1,000 ops per frame.

## B. Mission tracker: objectives, objective sets, status

All of `MissionTracker` here is native. A mission's runtime record (`IMission.MissionData`) holds its `Status`
(`EMissionStatus`: NotStarted 0, Active 1, RequiredObjectivesComplete 2, ReadyToTurnIn 3, Complete 4, Failed 5),
`ObjectivesProgress` (one int per entry of the mission's `ObjectiveDefs`), `ActiveObjectiveSet` and
`SubObjectiveSets`.

### B1. `MissionTracker.UpdateObjective(Objective, ObjectiveBit)`

1. The request is appended to `ObjectiveUpdates`. If no drain is running, requests are applied first-in first-out
   until the queue is empty; an update raised while another is being applied (for example by a behavior fired
   from it) waits its turn instead of nesting.
2. An update is ignored unless: the objective's mission has a record; status is Active or
   RequiredObjectivesComplete; there is an active objective set and it contains the objective; the objective is in
   the mission's `ObjectiveDefs`; and its progress is still below `ObjectiveCount`.
3. Progress: normally +1 (the bit argument is ignored). When the objective's `bRememberItemsWithinObjective` is set,
   progress is a bit mask: the bit is OR-ed in (a zero bit does nothing) and the count used everywhere is the
   objective's `TranslateObjectiveCount` of the mask (not read; presumably the number of bits).
4. Order of effects: mission observers are told "objective updated"; waypoints/directors and script hooks are
   notified (`UpdateMissionObjective` on players, `ClientReceiveMissionObjective`); the behavior event named after
   the objective fires with **id 3** and the new count as payload.
5. The objective is **complete when the count equals `ObjectiveCount`** (exact equality). Then: matching
   defend-mission entries are dropped; observers get "objective complete"; the objective's `StatId`, if set, is
   apparently counted as a player stat; the **set-completion evaluation (B2) runs, with notification**; and only then the objective event
   fires with **id 2**; the mission data is then replicated/saved.

### B2. When an objective set completes and what follows

Run after every objective completion, and right after a set becomes active when that set has
`bCanCompleteMission` (so a set whose objectives are already complete completes immediately).

- All objectives of the active set complete → the set is complete. With `bCanCompleteMission`, incomplete
  objectives marked `bObjectiveIsOptional` do not stop the *required* test.
- A branching set (`MissionObjectiveSetBranchingDefinition`) also counts as complete when every objective in
  `BranchedObjectiveDefinitions` is complete.
- A collection set (`MissionObjectiveSetCollectionDefinition`) is complete when each active sub-set is complete
  and has `bCanCompleteMission`. A completed sub-set that contains the objective just finished gets its set event
  with id 5; a completed sub-set with `bAutoEnableNextSet` and a `NextSet` is advanced, and when that next set has
  `bCanCompleteMission` the whole evaluation restarts. When every sub-set is complete the collection's own event
  fires with id 6.
- Set complete: its set event fires with **id 5** (when notifying). Then, if the set has `bCanCompleteMission`
  (class default **true**) the mission goes to **ReadyToTurnIn**; otherwise, if it has `bAutoEnableNextSet`
  (default **false**) the mission advances to `NextSet`; otherwise **nothing happens**: the next set must be
  activated by a behavior.
- If only `bObjectiveIsOptional` objectives remain incomplete in a `bCanCompleteMission` set and the mission is
  Active, it goes to ReadyToTurnIn without the set-completed event (RequiredObjectivesComplete was not seen being
  set by this path).

### B3. `Behavior_AdvanceObjectiveSet` and set activation

The behavior does nothing on a network client or without `ObjectiveSetToAdvanceTo`. It only advances when the
target is exactly the `NextSet` of the active set, or of an active sub-set of an active collection, or is the
mission's `InitialObjectiveSet` while no set is active. Any other target is silently ignored.

Advancing (also used by B2 and by mission activation): refused when the mission is ReadyToTurnIn or Complete.
Activating a set: the previous set is cleaned up, the sub-set list cleared; for a collection with at most four
sub-sets each sub-set becomes active and gets its set event with **id 4**, in order (more than four: the set is
stored and nothing else happens, an edge case as read); then the set's own event fires with **id 4**; observers
are told "objective set changed"; the tracked-mission HUD refreshes; `StartBlockingSet`/`StopBlockingSet`
start or stop mission blocking; finally the B2 evaluation runs if the new set has `bCanCompleteMission`.

### B4. Mission status

- **Active** (`ActivateMission`): allowed from NotStarted or Failed, or from Complete when `bRepeatable`, and only
  when dependencies are met (B6), and a mission with more than 20 objectives (or objectives but no
  `InitialObjectiveSet`) is refused. Progress is reset, the game stage is locked, the mission weapon is granted
  when its `MissionObjective` belongs to this mission and the mission has a `GameStageRegion` (used for the
  weapon's level), the tracked mission may change, and **only if
  `bActivateInitialObjectiveSet`** (class default **true**) the initial set is activated (B3).
- **ReadyToTurnIn** only from Active or RequiredObjectivesComplete; **Complete** only from ReadyToTurnIn or
  RequiredObjectivesComplete (the mission weapon is removed, the active set cleaned up); **Failed** only from
  Active and only when the mission allows it.
- After every accepted change, in order: observers ("status changed"), directors, script hooks
  (`UpdateMissionStatus`, `ClientReceiveMissionStatus`, `TriggerMissionStatusChangedDelegates`), then the
  **`Default` event with id 6 + new status** (7 Active … 11 Failed). The event name is a fixed engine name; that it
  is `Default` is inferred from the data (ids ≥ 7 occur only on events named `Default`).
- `PlayKickoff`, `PlayKickoffDialogOnly` and `PlayTurnIn` fire `Default` with ids **12, 13, 14**.
  `RunMissionCustomEvent(name)` fires `name` with **id 0**, unless the mission is Complete.

### B5. Level load

On level start (server, not the front-end map) every Active or RequiredObjectivesComplete mission without an active
set gets its initial set, and a flag asks the next tick to **replay** every mission's state as events:
`Default` with id = current status (0–5); for a NotStarted mission every objective and set event with id 0; for
others each objective event with id 1 if complete, 0 if not (with its progress), and the sets along
`InitialObjectiveSet` → `NextSet`: id 1 for the active set, 0 for later ones, 2/3 for completed sets and
collections (the 2/3 details are approximate). Missions use these "replay" links to restore world state after loading.

### B6. Dependencies (`MissionDependenciesMet`)

Every `Dependencies` mission must be Complete, and `ObjectiveDependency` must hold: its objective complete, or, when
its status field says Active, the objective currently updatable (B1 step 2).

### B7. Link ids on mission events (summary)

| Event name | id | When |
|---|---|---|
| objective | 0 / 1 | level-load replay: not complete / complete |
| objective | 2 / 3 / 4 / 5 | completed / progress updated / decremented / cleared |
| objective set | 0 / 1 / 2 / 3 | level-load replay (not reached / active / done / collection) |
| objective set | 4 / 5 / 6 | became active / completed / collection completed |
| `Default` | 0–5 | level-load replay: current status |
| `Default` | 7–11 | status changed to Active … Failed (6 + status) |
| `Default` | 12 / 13 / 14 | kickoff / kickoff dialog only / turn-in |
| custom name | 0 | `RunMissionCustomEvent` |

**Structural oracle** (`python research/mission_event_link_ids.py`, all 133 `MissionDefinition`s in `Startup`,
2026-10-02): 3,420 event links; objective events use ids {0,1,2,3,4}, set events {0..6}, `Default`
{0,1,3,4,7,9,10,11,12,13,14}, other events {0}; **0 links outside the predicted sets**. Consistent with the
table, not a proof of it.

### B8. What the host does today, and the difference

| Host (`src/mission.cpp`) | Native reading |
|---|---|
| `accept` (155-161) fires `Default` with all links | only id 7 (status Active); the slice mission has no id-7 link |
| `advanceSet` (142-152) jumps to any set and fires all its links | B3 target rule; set event id 4 only; collections, blocking sets and the immediate B2 check |
| `completeObjective` (163-187) completes in one call | progress +1 per update, `ObjectiveCount`, bit masks (B1) |
| objective event fired with all links (175), before the set bookkeeping (177-185) | id 3 on each update; id 2 on completion, **after** the B2 evaluation |
| set complete → `NextSet` if no behavior advanced, else ReadyToTurnIn (177-185) | `bCanCompleteMission` → ReadyToTurnIn; else `bAutoEnableNextSet` → next; else nothing |
| weapon granted when the set holding its objective activates, removed when that objective completes (150, 171) | granted at status Active, removed at Complete (whether `IsValidMissionWeapon` restricts it in between was not read) |
| `available` (103) checks `Dependencies` only | also `ObjectiveDependency` |
| `customEvent` (207) fires all links while Active | id 0 only, any status but Complete |
| no RequiredObjectivesComplete/Failed, no `bRepeatable` | B4 |

### B9. What this predicts for the slice mission

Read against the installed data of `M_RockPaperGenocide_Fire` (UNVERIFIED as a whole):

- Accepting it sets Active; `bActivateInitialObjectiveSet` is false on this mission and `Default` has no id-7 link,
  so **no set is active yet**. The kickoff (`Default` 12/13) starts a Marcus dialog whose `Finished` output
  (`ETriggerDialogEventOutputLinks` id 1) runs `RocksPaper_MoveMarcusToRange` and the AdvanceObjectiveSet to
  `GoToRange_ObjSet`. `Default` id 1 does the same without dialog on a level load while Active. Which native or
  script call plays the kickoff after acceptance was not identified.
- The host instead runs, at acceptance, the id-1 (load), id-3 (load ReadyToTurnIn), id-9 (ReadyToTurnIn) and
  kickoff links together, which is why it emits `RocksPaper_TargetBack` 3 s after accepting.
- Completing `RockPaper_GoToRange`: id 3 (no links), set evaluation (`GoToRange_ObjSet` has
  `bCanCompleteMission` false and no auto-enable: nothing), then id 2: a dialog after 0.5 s whose `Out` output
  advances to `RocksPaper_FinalObj`; one second later the AdvanceObjectiveSet's default output (class default
  `bSupportsDefaultOutputLink` true) switches the dummy's sequence to `Targetable`. The host
  advances immediately through the id-1 (load) link. `RocksPaper_TargetForward`/`SetFireTargetBool` are id-1 (load)
  links of the FinalObj event and would not run live from this event.
- Completing `Fire`: the FinalObj set completes (`bCanCompleteMission` default true): set event id 5
  (`TargetBack` after 3 s), status ReadyToTurnIn (`Default` id 9: `TargetBack` after 3 s again, no dedup), then
  the `Fire` id-2 links (dialog, `RocksPaper_FireCompleted`).

## Acceptance tests

Synthetic (to add with the implementation, invented data only, `tests/`):

1. An event with links of ids 4 and 5 fired with filter 4 runs only the id-4 target; filter −1 runs both.
2. `MaxTriggerCount` 1 → second fire ignored; `ReTriggerDelay` 2 s → a fire 1 s later ignored, 3 s later runs.
3. Two links to the same behavior from one event → it runs twice.
4. Behavior with links ids {−1, 0, 1}: handler records 1, `bSupportsDefaultOutputLink` false → only the id-1 link;
   true → the −1 and id-1 links (−1 links after the recorded ids).
5. Ordering with zero delays: event links A, B → A's subtree before B; behavior links A, B → B's subtree before A
   continues.
6. Objective with `ObjectiveCount` 3 needs three updates; bit-mask objective counts distinct bits; a set with
   `bCanCompleteMission` false and no `bAutoEnableNextSet` stays put after completion; `AdvanceObjectiveSet` to a
   set that is not the `NextSet` is ignored.
7. Kismet: input `ActivateDelay` adds to the output's; a disabled input receives nothing; links A, B → A's target
   runs first; an input hit twice before its op runs makes the op run twice.

Structural: `python research/mission_event_link_ids.py` must keep reporting 0 links outside the predicted sets.

In the original game (`tools/sdk_trace/openwillow_gametrace`, probe channel; needs the maintainer, see
`SANCTUARY_RPG_MISSION.md` "Capture blocker"), poll `MissionTracker.GetActivePrimaryObjectiveSet`,
`GetMissionStatus` and `GetObjectivesProgress` every probe tick and log the hooked
`WillowSeqEvent_MissionRemoteEvent` activations while playing the slice mission:

- right after `ActivateMission` the active set is None until the kickoff dialog ends;
- `RocksPaper_FinalObj` becomes active about 0.5 s after `RockPaper_GoToRange` completes, not in the same tick;
- `RocksPaper_TargetBack` fires twice about 3 s after `Fire` completes; status goes Active → ReadyToTurnIn
  directly;
- calling `UpdateObjective` twice in one probe on a count-2 objective (any other mission) gives progress 2 and one
  completion.

## Not read yet

The context-object resolution and latent bookkeeping inside the behavior thread runner; `TranslateObjectiveCount`;
the `BehaviorSequenceEnableByMission` reactions (`MissionReaction*` natives) that the dummy's sequences depend on;
`DecrementObjective`/`ClearObjective` internals beyond their event ids; `IsValidMissionWeapon`; what plays the
kickoff after acceptance; the replicated (`Remote*`) paths.
