## 2026-11-16: Action-sequence natives and the tree lock from the analyst note

AI-assisted (Claude Code). Lane W7-AI-LOCK: the fresh implementer of the clean-room pair begun by W7-AI part 2
(decision 2026-11-16-66). The only input was `docs/verification/NATIVE_AI_ACTION_SEQUENCE.md`; this lane opened no
analysis output, Ghidra project or REA result for these natives. Code: `crates/ow-ai/src/awareness/actions.rs` and its
tests. No other crate, no dependency, no new crate edge. There was no real-game session, so every rule below is
**UNVERIFIED** in the running game.

- **What is implemented, in `ParityMode::Off`.** `StartAtomic`, `StopAtomic`, `IsAtomic`, `IsRunning`,
  `IsActionReadyToFinish`, `RangeIsValid` and `GetRangeValue`, each from the note's outcome rules:
  - Atomic: with `bInitialized` set, start and stop set and clear `bInAtomicSection` (no nesting, no count), and
    `IsAtomic` is both flags.
  - `IsRunning`: true when the action is the tree's current leaf.
  - `IsActionReadyToFinish`: no latent action pending in the action's state frame, and every `ParentSequence` up the
    chain ready.
  - Ranges: valid when both ends are above zero and `min <= max`; the value is 0 for an invalid range, otherwise
    `min + (max - min) * r / 32767` in single precision, with `r` one 15-bit draw.
- **Reference.** The seven stay unbound, which is the VM's stub policy. There is no C++ AI to follow, and the brief
  keeps today's policy.
- **The tree's lock, in both modes.** The note says the periodic update leaves the running action alone when "both its
  running and its non-interruptible flag" are set, and reads the non-interruptible flag as `bInAtomicSection`.
  - With `bInAtomicSection` clear, the action is not locked, whatever the other flag is: the conjunction is already
    false.
  - With it set, the answer is `NotYet { NotRead }`. A class without the property is `NotYet` too.
  - Both modes run this one rule because there is no C++ reading to differ from.
- **How "running" is seen.** The port keeps the tree's current node in Rust, not in `AIComponent.CurrentState`. The
  brains service records which mind made each action, and what each mind's tree runs. `VmTree::start` and `stop` update
  that record around the action's `Start` and `Stop` events.

**Left `NotYet`, with the question for a later analyst or live lane.** Each is a case the note does not settle, so
this lane does not guess:

1. **Which flag is the update's "running" flag.** The note leaves it open ("Not read"), and names no candidate. The
   action's own flags are `bInitialized`, `bDoWorkFinished` and `bInAtomicSection`; the note does not say the
   "running" flag is one of them. Until it is read, an atomic running action is `NotYet`.
2. **Who sets `bInitialized`** when the AI creates its actions, and the context fields with it (`AI`, `MyGearboxMind`,
   `MyGearboxPawn`, `WillowAI`, `MyWillowMind`, `MyWillowPawn`). The note's "Not read" lists them. This port never sets
   `bInitialized`, so an action the game would have initialised cannot be told from one it would not. The note's
   "uninitialised" answers (no change; `IsAtomic` false) are therefore `NotYet`, not given.
3. **`IsRunning` during the action's own `Start` or `Stop`.** Whether the component's current state already, or still,
   names the action at that point is not read: `NotYet`.
4. **`IsRunning` on an action no brain of this port made.** False when its `AI` is unset (the note's first condition),
   and `NotYet` when it is set, because its tree is not known here.
5. **`WillowActionSequencePawn`'s override of `IsActionReadyToFinish`.** The note says one may exist and was not read.
   Any action of that class or a subclass is `NotYet`.
6. **The random stream.** `GetRangeValue` draws from `CrtStream::AiDecision`, the stream the tree's own draws use.
   Whether the engine's C `rand()` call shares one generator with the tree's draws is UNVERIFIED.
7. **32767 against 32768.** The note reads 32767 for `GetRangeValue`. The tree's draws divide by 32768 (decision 66),
   by a path the note did not re-read. Both are kept as they are.

The `ParentSequence` walk is bounded at 32 links and refuses a loop with an error. That is a Rust-only guard against
bad data, not a rule of the game.

**Proof.**

- Synthetic tests (invented classes, AGENTS.md rule 9), in `actions/tests.rs`:
  - The atomic flags, before and after `bInitialized`.
  - The range natives: five validity cases, no draw when invalid, and exactly one AI-stream draw otherwise.
  - Bound in Off and unbound under Reference.
  - `IsActionReadyToFinish` up a parent chain, with an injected latent answer; the pawn-class `NotYet`; the loop error.
  - `IsRunning` for the current action, the other action, a stray action with and without `AI`, and the mid-`Start`
    case.
  - The lock `NotYet` for an atomic running action, in both modes.
  - The existing brain test now goes past the lock: the non-atomic patrol is not locked, and the attack takes over.
- **The installed headless run** (`installed_actions.rs`, the part 2 setup unchanged: Reference mode, a stock regular
  bullymong, four placements, frames of 1/16 s):
  - Before this lane, every case stopped at 0.5 s on the lock `NotYet`. Now all four run the whole 1.5 s with no
    `NotYet` stop. The patrol is not atomic, so it is not locked.
  - In every case `Patrol` starts at 0.25 s and is still running at 1.5 s, and the dry selection still picks `Patrol`.
  - In the three cases where the player is acquired, `Target_Acquired` reaches every action, the running one first.
    The `CanRun` failures stay `Melee`, `Leap Attack` and `Back Up (Throw Rock)` `NoTarget`, and
    `GoToScriptedDestination` `NotScripted`.
  - The attacks answering `NoTarget` while the component's target is the player is not explained here. One candidate is
    the unread action setup (item 2 and the `InitData` stub the run reaches), but that is a guess, not a finding.
  - Under Reference the seven natives stay stubs, but no `ActionSequence` stub line appears in this run.
  - All of this is **UNVERIFIED**: no live record holds a lone bullymong's choices. The output is under ignored `local/`.
