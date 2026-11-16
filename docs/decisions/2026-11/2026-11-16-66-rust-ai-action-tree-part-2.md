## 2026-11-16: Rust AI action tree, part 2

AI-assisted (Claude Code). Lane W7-AI part 2, on top of part 1 (decision 2026-11-16-59). It adds the modules `tree`
and `actions` under `crates/ow-ai/src/awareness/`, with their tests, the ignored installed-game run
`crates/ow-ai/tests/installed_actions.rs`, and the analyst note `docs/verification/NATIVE_AI_ACTION_SEQUENCE.md`.
No other crate changes and no dependency. There was no real-game session. Every rule is **UNVERIFIED** unless its
doc names the live record that confirmed it.

Edits: docs/decisions/2026-11/2026-11-16-59-rust-ai-awareness-and-targeting-part-1.md

Review r2 L1: that entry's step 2 sentence said a controller without a pawn is `NotYet`. NATIVE_AI_AWARENESS.md step 2
replaces the actor only when the pawn is set, so such a controller is now tested as itself, and the sentence says so.

- **Sources.** The tree rules come from `NATIVE_AI_AWARENESS.md` ("AI tree natives", G26) and `NATIVE_AI_MELEE.md`
  ("State selection", "AIComponent: cadence", G25), read by earlier analyst lanes, and the live records
  `AI_MELEE_TARGETING_LIVE.md` (L8) and `AI_AWARENESS_LIVE.md` (L9, section 7). The actions' own logic is
  UnrealScript and runs in the VM. No AI runs in the C++ reference, so there is no stand-in: both parity modes run
  the same rules.
- **The tree** (`tree.rs`, no VM). Node 0 is the root of `AIDefinition.NodeList`; node kinds are action, random,
  priority and sequence.
  - Eligibility, in G26's order: `NextTimedRun`; `CanRunIf` (skipped while the node runs, unless it has
    `bStopIfCannotRun`); the 1.0 s block after `LastFailTime`; the action's script `CanRun`. A node with a `Resource`
    is `NotYet` (the resource registry is not read).
  - Selection: a priority node takes its first eligible child (with its `Conditions` evaluator); a random node draws
    by weight with the engine's integer random over 32768, and counts `LoopCount` against `LoopCountMax` on the same
    child; a sequence node runs its children in turn.
  - Activation arms the timed fail; completion sets the `TimedReset` cooldown, advances a sequence and counts a random
    node's loops; `Finish` sets `LastFailTime` on a failure and re-selects at once.
  - The periodic state update (locked, `CanRun` of the running action, stop when it fails, the timed fail, selection,
    switch); the timed-fail tick; the `TimedStart` re-arm when a target is first acquired.
  - Live (L9 section 7): the re-arm, the reset cooldown, the random node's counters, and a `CanRun` failure name that
    skips a node in the same update. Not exercised live: `LoopCountMax`, the 1.0 s block, a failed `Finish`, the
    timed fail.
- **The adapter** (`actions.rs`). The first tick reads the tree from the AI definition, copies each action node's
  archetype (setting `MyWillowPawn`, live in L8, and `MyWillowMind`, which the actions' own `Init` reads: UNVERIFIED)
  and raises `Init` on each copy. Then each tick, in the recorded order: part 1's tick; on hold, stop the running
  action; the target events (`Target_Lost`, `Target_Acquired`) to the running action first and then the others in
  node order (L8 section 3.4); the running action's `Update`; the timed fail; the state update when due. An action
  event the class does not have is an error, not a silent skip.
  - Natives bound: `ActionSequence.Finish` (queued, applied as soon as the action's script returns: the note reads
    the re-selection as happening inside `Finish`; UNVERIFIED either way), `TriggerBehavior` (through the behavior
    kernel's consumer routing, decision 2026-11-16-50), the component's `Hold` / `Release` / `OnHold` and their
    `Target` forms (name and tag pairs), and the movement natives.
  - **Movement goes to the host** through the `Locomotion` trait (path creation on either navigation handle class,
    `ClearPath`, path validity, invalidation, arrival, `CanMoveTo`, movement speed, nearest nav-mesh point), as
    `Senses` does for perception. `ow-ai` keeps no `ow-host` edge. The latent moves (`FollowPath`, `WaitForPath`,
    `FinishPath`, `WaitForPawnToTurn`, `WaitForPawnToStop`) are `NotYet`: the VM's latent actions are `Sleep` only.
  - The other context fields of an action (`AI`, `MyGearboxMind`, `MyGearboxPawn`, `WillowAI`) and its flags are set
    by native code that was not read; they keep the archetype's values.
- **Clean room.** This lane read `StartAtomic`, `StopAtomic`, `IsAtomic`, `IsRunning`, `IsActionReadyToFinish`,
  `RangeIsValid` and `GetRangeValue` in the executable as analyst and wrote `NATIVE_AI_ACTION_SEQUENCE.md` in its own
  words. It does not implement them: they are `NotYet` in Off and unbound (the VM's stub policy) in Reference, and the
  tree's "is the running action locked" question, which needs the same flags, is `NotYet`. A fresh lane codes them
  from the note alone. Nothing was implemented from `local/analysis/`.
- **Review r2 of part 1, folded in.**
  - L1: a controller without a pawn is tested as itself (`vm.rs`, the toy `Senses`, and decision 59's sentence).
  - L2: `ValidTarget` step 7 asks `Senses::is_registered(pawn)` instead of copying the registry on every call; the
    VM `Host` trait's default answers from `targetables`, and a host that keeps a set can answer directly.
- **Verified.**
  - Synthetic tests (invented values only): 10 on the tree (a beast-shaped invented tree: priority order, timed start
    and reset, random weights and loop counts, sequences, the failure block, the state update, `bStopIfCannotRun`,
    the timed fail, the `NotYet` answers), 3 on the adapter in both parity modes (an invented action class whose
    events are script run by the VM: the brain perceives, chooses and starts, the target event order, `Finish`
    switching actions; holds; movement to the host and the unread latent moves). `ow-ai` lib tests: 43 pass.
  - Installed headless run (ignored test, Reference mode, a stock regular bullymong and the player, the live iterator
    weights as in part 1, an open-field `Locomotion` stand-in, frames of 1/16 s), at four placements: 3,000 uu in
    front, 500 in front, 800 behind with no exposure, 6,000 in front. **All UNVERIFIED.**
    - In every case `Patrol` starts at 0.25 s and the run stops at 0.5 s on the lock `NotYet` (the clean-room gap
      above): the brain does not get past its first running action.
    - The player becomes the target in the three near cases, not at 6,000. `Target_Acquired` is reported at 0.25 s
      to every action in the recorded order at 500 and 800; at 3,000 the target is set by the stop, and its event,
      if raised in the stopped tick, is lost with that tick's report (a reading, not checked).
    - A dry selection would still pick `Patrol`. The attacks' `CanRun` answers `NoTarget` (`Melee`, `Leap Attack`,
      `Back Up (Throw Rock)`), and `GoToScriptedDestination` answers `NotScripted`. The likely cause of `NoTarget`:
      the actions' script reads its target through `GetTargetRecord`, and part 1 keeps target records in Rust, not as
      VM objects, so the script sees none. This is a seam for part 3.
    - Movement requests: only "invalidate path". Stubs reached under Reference: `Action_GenericAttack.InitData`,
      the attribute and flag expression evaluators, `GetCloakBody`, `SetFacingPolicy`, and five `WillowMind` queries
      (`CheckForPerch`, `GetPopOpDen`, `IsInScriptedMovement`, `IsTooFarFromHome`, `ShouldLookAtPlayer`).
  - So the slice shows perception and the first choice; it does not yet show a bullymong moving toward the target or
    meleeing. Not a parity claim.
- **Seams to ask about (not added).**
  - `ow-vm`: a host-driven latent action (the latent moves), and the `vm.find_method` of hand-declared functions.
  - Part 3 in `ow-ai`: the target records as VM objects for `GetTargetRecord`, the lock rule and the analysed
    natives (fresh lane), `InitData`, the evaluators, and the `WillowMind` queries.
  - `ow-world` / `ow-host`: the call from the mind's tick into `tick_brain`, and an implementation of `Locomotion`.
