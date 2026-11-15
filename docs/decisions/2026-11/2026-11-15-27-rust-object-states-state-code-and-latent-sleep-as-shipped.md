## 2026-11-15: Rust object states, state code and latent Sleep as shipped

AI-assisted (Claude Code). Lane W5-STATES of the Rust port fills `crates/ow-vm/src/states.rs` and
`crates/ow-vm/src/latent.rs`: the object state machine of `docs/verification/NATIVE_ENGINE_CORE.md` ("Object state
machine", "Probe events", "Actor.Sleep (latent) and the latent poll", "State-code step") in place of the C++
name-only approximation, outside `ParityMode::Reference`. It builds on decisions 2026-11-15-14 (interpreter core) and
2026-11-15-20 (aggregates, `foreach`), and replaces the Off-mode `GotoState` `NotYet` that decisions 2026-11-15-07 and
2026-11-15-14 recorded. Nothing here was checked in the running game.

- **Two modes** (architecture 6.1, R3).
  - Under `ParityMode::Reference` nothing changes: `GotoState` keeps the C++ approximation in `natives_core.rs` (the
    name and the two events, a label logged and not run, the 8-level nesting bound), `IsInState` compares names,
    `Enable` and `Disable` do nothing, state code never runs, and `PushState`, `PopState` and `Actor.Sleep` stay
    unbound, as in the C++. `tests/vm_states_test.py` pins this.
  - Otherwise `GotoState`, `IsInState`, `Enable` and `Disable` hand over to `crate::states`, and
    `natives_core::register` also binds `Object.PushState(name,name)`, `Object.PopState(bool)` and
    `Actor.Sleep(float)`.
- **Interface.**
  - `ow_vm::states`: `read_state_body` / `StateBody` / `StateBodyError` (the State export body, below);
    `process_state(vm, object, delta)` (one state-code step, for the actor tick of W5-WORLD);
    `probe_enabled(vm, object, name)` (the probe rule for the world's events such as `Tick` and `Touch`);
    `forget(vm, object)` (drop a frame on destroy); `NO_LABEL_TABLE`, `MAX_STATE_RESTARTS`, `MAX_RUNS_PER_STEP`,
    `MAX_STATE_DEPTH`.
  - `ow_vm::latent`: `LatentAction` (`#[non_exhaustive]`, only `Sleep`), `KERNEL_MIN_WAIT` and
    `kernel_resume_at(now, wait)`: the behavior kernel's latent wait (a negative or NaN wait is finished, otherwise
    `now + max(wait, 1/60)` in double precision, as `src/behavior.cpp:569`), for W5-BEHAVIOR.
  - The state frames live in a VM service (`states::StateMachine`, crate-private), added on first use and borrowed
    only for short scopes, never across a script call. No frozen contract changed.
- **The State export body.** A State export is a plain object's net index and an empty tagged-property block (only
  the `None` terminator), then the struct part and the state part of the Class export body (`ow_object::class_body`),
  ending exactly at the export's serial size. The label table offset is one past the in-memory offset of the code's
  `LabelTable` token (where its entries start); `0xFFFF` is none. This is a fit, checked structurally on the installed
  game (pc-hummer, local, the nine code packages): 166 of 166 State exports read to their exact size and their code
  decodes; 85 of 85 label table offsets are one past a `LabelTable` token; 97 of 97 labels land on a statement; the 81
  states without a label table hold only `Stop` and `EndOfScript`; the 49 parent references all name State exports.
  A State export with tagged properties is refused, not walked. The census also found: `GotoLabel` occurs 45 times,
  only in state code (always with a name constant) and never in a function; `Sleep` is called 73 times and `PopState`
  15 times from state code. The state flags hold four values over the 166 states (0 on 129, 0x2 on 11, 0x4 on 25, 0x6
  on 1); which bit means what is not read, so nothing uses them.
- **Rules ported** (the record's reading, each UNVERIFIED in the running game; this port's own readings are marked).
  - **State frame.** Every object gets one on first use (when the engine makes one is not read; this port's
    reading). The heap's `state` field holds the active state's name and is kept in step, so the interpreter's dispatch
    by state sees it.
  - **States and parents.** A name resolves to the State export of the most derived class of the chain that declares
    it; its parent is the State export its super reference names (at most `MAX_STATE_DEPTH` levels, which also stops a
    cycle). Labels are looked up in the state, then its parents; events in the state's functions, then its parents',
    then the class chain.
  - **GotoState.** An omitted `NewState` stays in the current state. The same state without `bForceEvents` only
    applies the label. Otherwise the state change routine: an unknown name falls back to no state; unless
    `bKeepStack` every pushed state is popped (`PoppedState` each when there is an active state; that each entry is
    restored as the pop routine does, without `ContinuedState`, is this port's reading); `EndState(new name)` when
    forced or the name changes from a state, probe-gated and not from inside another `EndState`; the switch (latent
    action and code position cleared, probe mask rebuilt); `BeginState(old name)` unless unforced to no state, probe-
    gated; a state change inside either event wins (the record's result 2) and the outer label is not applied. The
    label defaults to `Begin`; an explicit label not found logs the record's text `GotoState (<state> <label>): Label
    not found`, a missing `Begin` is silent. An unknown state name logs `GotoState (<name>): state not found in
    <class>` (own wording; the record says only "a not-found style warning"). `GotoState('Auto')` is
    `NotYet { NotRead }`: the auto-state flag bit is not read. That a label not found leaves no code to run, and that
    applying a label clears a pending latent action, are this port's readings.
  - **PushState / PopState.** `PushState` needs a declared state, does nothing for the current state or one already
    pushed, else `PausedState`, push (state and code position), switch, `PushedState`, label (default `Begin`).
    `PopState` sends `PoppedState`, restores the top entry (state and code position, not the label) and sends
    `ContinuedState`, once or until the stack is empty. Non-probe events go by name lookup; a missing one is silent.
  - **IsInState.** The active state or a parent; with `bTestStateStack` also a pushed state; false without a frame.
  - **Probes.** The record names ten probe ids (300-302, 306, 307, 309, 310, 317, 328, 329); those are probes here.
    The 22 other ids carry public UE3 names whose use in this executable is not read: `Enable`, `Disable` and
    `probe_enabled` answer `NotYet { NotRead }` for those names. Any other name is not a probe (`Enable: '<name>' is
    not a probe function`, the record's text). The mask is the set of probes the class chain or the active state chain
    defines a function for, rebuilt at every switch; `Enable` sets a bit only when it is defined. The serialized probe
    mask is read but not used.
  - **State code.** A state's code runs through the interpreter as a frame with no locals (`<state path>.<state
    code>`, a path no lookup finds), from the frame's code position, one top-level call (one step budget) per run. A
    label in the running state's code is a jump inside the run; a label in a parent's code, a state change, a push or
    a pop ends the run and the step continues at the new position. At most five restarts on a state change per step
    (the record), at most `MAX_RUNS_PER_STEP` (64) runs in all (a Rust-only bound, a typed error). A `GotoLabel` not
    found logs `GotoLabel (<label>): Label not found` (own wording) and stops the code. A latent call inside a
    `foreach` of state code is `NotYet { NotRead }`. An actor whose `Role` is not 3 is `NotYet { NotRead }` (which
    state flag lets a state run for a non-authority actor is not read); an object without a `Role` runs.
  - **Sleep.** Only from the object's own running state code (otherwise a typed `VmError::Script`); stores `Seconds`
    in `LatentFloat` and sets the latent action. The poll at the start of each step subtracts the delta (single
    precision) and clears the action when the remainder is below half the delta, then the code continues in the same
    step. The record's `Sleep` has no 1/60 floor; the 1/60 floor of the lane entry is the behavior kernel's latent
    wait, provided as `latent::kernel_resume_at`.
- **Seam edits** (granted: the smallest edits that replace the `GotoState` / label / latent sites), one line per file:
  - `crates/ow-vm/src/interp/mod.rs`: a `state_code` flag on `FuncCode`; `class_at` and `declare_state_code`
    re-exported to the crate; the paragraph on what waits for W5-STATES replaced.
  - `crates/ow-vm/src/interp/load.rs`: `state_code: false` for package functions; `declare_state_code` (a State
    export's code declared as a function with no parameters or locals).
  - `crates/ow-vm/src/interp/eval.rs`: in `run`, state code resumes at the frame's position, records the next
    statement before each one, and checks after each one whether to hand back; `GotoLabel` jumps in state code, is
    stepped over in a function under Reference (the C++), and is `NotYet { NotRead }` in a function otherwise.
  - `crates/ow-vm/src/interp/iter.rs`: `Iterators::is_empty`.
  - `crates/ow-vm/src/interp/tests.rs`: two assertions flipped: `GotoLabel` in a function steps over under
    Reference and is `NotYet` in Off; `GoA` in Off is now a `Decode` error, because that fixture's state has a
    16-byte stand-in body.
  - `crates/ow-vm/src/natives_core.rs`: `GotoState`, `IsInState`, `Enable` and `Disable` call `crate::states`
    outside Reference; `register` calls `states::register` and `latent::register`; the module's "States" paragraph;
    the test that pinned the Off `NotYet` now asserts the state machine on classes defined by hand, and the
    `IsInState` name comparison on a state name poked into the heap moved to a Reference harness (outside Reference
    a name with no state frame is not a state).
- **Verified (automated, local, pc-hummer).** 21 unit tests in `states.rs` and `latent.rs` on an invented Core
  package with real State bodies (code, label tables, a parent state): the body reader and each of its refusals;
  GotoState's event order and arguments, parent-state events and `IsInState`, the forced and None cases, an unknown
  state, a change inside `BeginState` and inside `EndState`, `Auto`, labels found and not found; state code from
  `Begin`, `Sleep` (0.375 s at a 0.25 s delta: not over at exactly half the delta, over on the next step) and a jump
  in the same code; a label in the parent's code; a `GotoLabel` not found; the five-restart bound; `Sleep` outside
  state code; destroyed and non-authority objects; `PushState` / `PopState` from state code and from the host, the
  stack in `IsInState`, `bKeepStack`, pop-all; `Enable` / `Disable` and the probe answers; Reference unchanged; the
  kernel wait. The `ow-vm` suite is 155/155.
  - **`tests/vm_states_test.py`** (`OW_PARITY=reference`) passes on the Rust binary, run on a local merge of this
    lane with W4-VMTOOLS' branch (which brings `--native-census`; the manifest line is W4-VMTOOLS'). In Off mode it
    fails as it should: its fixture's states have 16-byte stand-in bodies, which the state machine refuses with a
    typed `Decode` error.
  - **The installed game, `--run-batch`** (W3-INTERP's comparison: every function of `Core.Object` with no self
    class, and of `Engine.Actor`, `WillowGame.WillowPawn` and `WillowGame.WillowPlayerController` with the class as
    self; C++ `ow-package` of this branch, MSVC 14.50). Under Reference 326/326, 312/312, 513/513 and 1,595/1,596
    identical; the one left is `InitInputSystem`, the C++ tag-reader quirk recorded after W4-INTERP2 (the C++ keeps
    the inherited `InputClass` default as an object, so its `New` gives None). In Off 324/326, 309/312, 513/513 and
    1,593/1,596; each difference is Off-only by design: `FRand` on CrtRand, the `<Class>_<n>` names (4), the
    `Core.Class` stand-in `NotYet` (decision 2026-11-15-08), `PopState` bound where the C++ stubs it, and
    `Actor.SetInitialState`, which reaches `GotoState('Auto')` and is `NotYet` until the auto-state flag bit is read.
    Outputs stay under `local/`.
- **Open items.**
  - The interpreter's dispatch (`Vm::find_method`, W3-INTERP) follows a state by name through the class chain but
    not a parent state of another name (`state B extends A`); events sent by this module do follow it. W3-INTERP's.
  - The auto-state flag bit and the non-authority state flag bit (`NotYet`); the 22 probe names the record does not
    give (`NotYet`); latent natives other than `Sleep` (the AI's `WaitForPath`, `FollowPath` and others are unbound
    natives); a latent call inside a `foreach` of state code.
  - When the engine creates a state frame, and the meaning of the record's two frame flags it leaves open.
- **UNVERIFIED.** Every rule above: none was confirmed in the running game.
