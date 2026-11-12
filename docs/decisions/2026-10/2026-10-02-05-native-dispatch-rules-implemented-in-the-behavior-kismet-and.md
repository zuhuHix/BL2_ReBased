## 2026-10-02: native dispatch rules implemented in the behavior, Kismet and mission executors (UNVERIFIED)

AI-assisted. Executor change; the rules are those of `docs/verification/NATIVE_MISSION_DISPATCH.md` (read from native
code, written in our own words; nothing here comes from a listing). **Every rule below is UNVERIFIED against the running
game**: synthetic tests check that the executors do what the note says, not that the note is right.

- **Behavior kernel** (`src/behavior.*`): the link id byte is read signed; `fireEvent` takes a link-id filter (-1 = all);
  every matching event entry is gated by `bEnabled`, `MaxTriggerCount` and `ReTriggerDelay` with per-process
  `TriggerCount`/`LastTriggerTime`; threads run depth-first (the first selected link continues the thread, the others
  start first); handlers return the recorded output ids (duplicates kept) and -1 is added only when
  `Context.bSupportsDefaultOutputLink` is set (read with class defaults: true on most behaviors, false on
  TriggerDialogEvent/CompareObject); the once-per-event deduplication is gone; at most 60 behaviors per thread per call
  (a capped thread resumes on the next tick: what the game does was not read). Not modelled: `FilterObject`, latent
  behaviors, the "sequence still enabled" condition on running threads.
- **Kismet** (`src/kismet.*`): queued ops form a stack (the first link's target runs next; an op already queued keeps
  its place); link delay = target input's `ActivateDelay` + output's; disabled inputs receive nothing; an input hit
  twice runs its op twice; an activated event fires all its outputs; at most 1,000 ops per `run()` (was 10,000 impulses
  over the instance's life). Not modelled: one op seeing several inputs at once, event `MaxTriggerCount` /
  `ReTriggerDelay`.
- **Mission tracker** (`src/mission.*`): mission events carry the note's link ids (status change `Default` 6 + status:
  accept 7, ReadyToTurnIn 9, Complete 10; kickoff 12/13; set activated 4 / completed 5; objective progress 3 and
  completed 2 after the set check; custom events 0, refused only when Complete). Accept no longer fires every `Default`
  link. `UpdateObjective` is one queued +1 (or a bit OR-ed in; the count of a bit mask is its number of set bits, a
  guess for `TranslateObjectiveCount`); completion at `ObjectiveCount`. Set completion: `bCanCompleteMission` ->
  ReadyToTurnIn, else `bAutoEnableNextSet` -> next set, else wait; `AdvanceObjectiveSet` only to the active set's
  `NextSet` (or the initial set while none is active); `bActivateInitialObjectiveSet`; `ObjectiveDependency` in
  `available()`. Status transitions ReadyToTurnIn only from Active, Complete only from ReadyToTurnIn. Not modelled:
  RequiredObjectivesComplete, Failed, `bRepeatable`, collection/branching sets (an error if activated), blocking sets,
  the level-load replay, the mission weapon at status Active/Complete (still lent with its objective, because the
  host's checks expect that and `IsValidMissionWeapon` was not read).
- **Host stand-ins** (`src/slice.cpp`, `src/mission.cpp`): the slice plays the kickoff (`Default` id 12) right after
  acceptance (what does this in the game is unknown), and a `Behavior_TriggerDialogEvent` selects both its outputs at
  once (Out, then Finished) because no dialog is played. CLI: `--mission-run` gains `kickoff` and `obj:<name>:<bit>`;
  `--behavior-run` gains `fire:<id>:<event>`; `--kismet-run` gains trailing `--tick <s>` steps.
- **Real data (local only)**: the Fire mission now runs as the note predicts in its section B9: no set until the
  kickoff dialog finishes, `RocksPaper_FinalObj` 0.5 s after `RockPaper_GoToRange`, `Targetable` 1 s later, no
  `RocksPaper_TargetBack` after accepting, two `TargetBack` 3 s after `Fire`, Active -> ReadyToTurnIn directly, no
  errors. Its dialog lines are the same set as before. All 28 events of the range's Kismet sequence reach the same
  host ops in the same order as before. Link-id census: 0 links outside the predicted sets.
- Checks: CTest 10/10 (new: behavior acceptance tests 1-6, Kismet test 7; changed expectations follow the stack
  order and the per-frame cap), packages 9/9. The quest suite's step 7 (`OpenWillowQuest.cpp`) now waits for the
  FinalObj set (0.5 s after the touch) instead of the objective's completion; with that change, on a clean rebuild of
  `ow-core` and the UE module: quest 73/73 first run and 10/10 resume, mover 16/16, inventory actions 45 PASS / 0 FAIL /
  2 NOT_RUN / 2 KNOWN_DIVERGENCE (unchanged baseline).

