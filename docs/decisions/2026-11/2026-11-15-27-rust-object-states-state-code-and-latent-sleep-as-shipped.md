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
  - `ow_vm::states`: `read_state_body` / `StateBody` / `StateBodyError` (the State export body, below) and
    `read_class_state_part` (a Class export's struct and state parts, for its own state flags);
    `process_state(vm, object, delta)` (one state-code step, for the actor tick of W5-WORLD);
    `probe_enabled(vm, object, name)` (the probe rule for the world's events such as `Tick` and `Touch`);
    `find_event(vm, object, name)` and `send_world_event(vm, object, name, args)` (an event through the state chain,
    probe-gated; under Reference the C++ lookup), `latent_of` and `latent_pending` (review r1, for W5-WORLD's
    destroy and tick); `forget(vm, object)` (drop a frame); `NO_LABEL_TABLE`, `MAX_STATE_RESTARTS`,
    `MAX_RUNS_PER_STEP`, `MAX_STATE_DEPTH`.
  - **Every path that frees objects should call `states::forget`**: the world's destroy, and the sweep's
    `release_unreferenced` (W4-VMTOOLS). A frame kept for a freed handle is never read again, but it holds memory.
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
  on 1); which bit means what is not read; only "all 0" is used (`Auto`, below). The census is repeatable: the
  ignored local test `states::tests::installed_state_exports_meet_the_fitted_layout` reads the installed packages
  (`OW_COOKED`, else the default Steam folder) and prints the counts only.
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
    <class>` (own wording; the record says only "a not-found style warning"). That a label not found leaves no code
    to run, and that applying a label clears a pending latent action, are this port's readings.
  - **`GotoState('Auto')`** (review r1). The record: the class itself if it is flagged auto, else the first state of
    the chain flagged auto, else no state. Which flag bit means auto is not read; taking auto to be a bit of the state
    flags (this port's reading, UNVERIFIED), when the class's own state flags (`read_class_state_part`) and those of
    every state its chain declares are all 0, the answer is no state whatever the bit is (129 of the 166 installed
    states have flags 0), and it is silent. Otherwise, or when the class body does not read, `NotYet { NotRead }`.
    **That rule rarely applies to a real class:** the classes' own state flags are `0x2` on 3,317 of the 3,339 class
    exports and 0 on 22 (local census, review r1), so `Engine.Actor`'s `SetInitialState` is still `NotYet` in the
    installed-game comparison below. The same census fits a stronger reading, not adopted here (a maintainer or hub
    call, and still a fit): the 12 classes that own a state with bit `0x2` all have that bit clear on the class and
    own exactly one such state, and the other 10 classes with the bit clear own none. That is what "`0x2` marks auto,
    and a class is auto (no state) unless it declares its auto state" would give.
  - **PushState / PopState.** `PushState` needs a declared state, does nothing for one already pushed and nothing
    for the current state (the record says the steps run "if the new state is not the current one"; that the label
    jump is skipped too is this port's reading of that sentence, UNVERIFIED), else `PausedState`, push (state and
    code position), switch, `PushedState`, label (default `Begin`).
    `PopState` sends `PoppedState`, restores the top entry (state and code position, not the label) and sends
    `ContinuedState`, once or until the stack is empty. Non-probe events go by name lookup; a missing one is silent.
  - **IsInState.** The active state or a parent; with `bTestStateStack` also a pushed state. It makes the frame first,
    as the other state natives do (review r1), so it agrees with `GetStateName` whatever the order of the calls.
  - **Probes.** The record names ten probe ids (300-302, 306, 307, 309, 310, 317, 328, 329); those are probes here.
    The 22 other ids carry public UE3 names whose use in this executable is not read: `Enable`, `Disable` and
    `probe_enabled` answer `NotYet { NotRead }` for those names. Any other name is not a probe (`Enable: '<name>' is
    not a probe function`, the record's text). The mask is the set of probes the class chain or the active state chain
    defines a function for, rebuilt at every switch; `Enable` sets a bit only when it is defined. A declared function
    with no body counts (UNVERIFIED). The serialized probe mask is read but not used. Compared on the installed
    packages (review r1, the ignored local test above; only the ten named probes' bits, at id minus 300): a class's
    serialized mask equals the probe functions it declares itself in 2,929 of 3,339 classes and those with a body in
    2,926; the 410 that differ from both all carry bits outside the ten named probes; 4 classes declare a probe
    function without a body. A state's equals its own declared probe functions in 142 of 166 (with a body: also 142);
    24 differ, 3 of them with bits outside the ten. So the serialized mask counts a class's own functions, a body-less
    one included in 3 of the 4 cases, and the rule here (the chain's functions, body or not) is kept, UNVERIFIED.
  - **State code.** A state's code runs through the interpreter as a frame with no locals (`<state path>.<state
    code>`, a path no lookup finds), from the frame's code position, one top-level call (one step budget) per run. A
    label in the running state's code is a jump inside the run; a label in a parent's code, a state change, a push or
    a pop ends the run and the step continues at the new position. At most five restarts on a state change per step
    (the record), at most `MAX_RUNS_PER_STEP` (64) runs in all (a Rust-only bound, a typed error). A `GotoLabel` not
    found logs `GotoLabel (<label>): Label not found` (own wording) and stops the code. A latent call or a `PushState`
    (review r1) inside a `foreach` of state code is `NotYet { NotRead }`: how the loop survives the wait, or the push
    until the `PopState`, is not read. State changes that never settle (two states whose `BeginState` goes to the
    other) end in the VM's typed call-depth error. An actor whose `Role` is not 3 is `NotYet { NotRead }` (which
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
- **Verified (automated, local, pc-hummer).** 27 unit tests in `states.rs` and `latent.rs` on an invented Core
  package with real State bodies (code, label tables, a parent state): the body reader and each of its refusals;
  GotoState's event order and arguments, parent-state events and `IsInState`, the forced and None cases, an unknown
  state, a change inside `BeginState` and inside `EndState`, `Auto`, labels found and not found; state code from
  `Begin`, `Sleep` (0.375 s at a 0.25 s delta: not over at exactly half the delta, over on the next step) and a jump
  in the same code; a label in the parent's code; a `GotoLabel` not found; the five-restart bound; `Sleep` outside
  state code; destroyed and non-authority objects; `PushState` / `PopState` from state code and from the host, the
  stack in `IsInState`, `bKeepStack`, pop-all; `Enable` / `Disable` and the probe answers; Reference unchanged; the
  kernel wait. Review r1 added: `Auto` with every candidate flag 0 and with one set, `IsInState` on a state set
  before the frame, a `PushState` inside a `foreach`, the never-settling `BeginState` pair on the VM thread, and the
  world functions. The `ow-vm` suite is 161 passed, 1 ignored (the installed-game census).
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
    `Actor.SetInitialState`, which reaches `GotoState('Auto')` and is `NotYet` (`Actor`'s own state flags are
    `0x2`, above).
    Outputs stay under `local/`.
- **Open items.**
  - A native question (this port's reading, UNVERIFIED): `GotoState`'s stack clear restores each pushed entry as the
    pop routine does, so `EndState` reaches the bottom-of-stack state, not the state active when `GotoState` was
    called. Whether the executable's clear restores the entries (or only sends `PoppedState` and empties the stack)
    is to be read.
  - The interpreter's dispatch (`Vm::find_method`, W3-INTERP) follows a state by name through the class chain but
    not a parent state of another name (`state B extends A`); events sent by this module do follow it. W3-INTERP's.
  - The auto-state flag bit (`NotYet` unless every candidate flag is 0; the census above is for the hub) and the
    non-authority state flag bit (`NotYet`); the 22 probe names the record does not give (`NotYet`); latent natives
    other than `Sleep` (the AI's `WaitForPath`, `FollowPath` and others are unbound natives); a latent call or a
    `PushState` inside a `foreach` of state code.
  - When the engine creates a state frame, and the meaning of the record's two frame flags it leaves open.
- **UNVERIFIED.** Every rule above: none was confirmed in the running game.
