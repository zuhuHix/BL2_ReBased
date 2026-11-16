## 2026-11-15: Rust dialog and movers: the mover adapter, the dialog system and the Kismet seam

AI-assisted (Claude Code). Lane W5-DIALOG of the Rust port fills `crates/ow-mission/src/{mover,dialog}.rs` (with
their `tests.rs`) and `crates/ow-cli/src/cmd/mover.rs` (`--mover-probe`, `--mover-event`), adds
`tests/rust_parity.d/mover.txt`, `tests/mover_parity_test.py` and its ctest entry `mover-rust`, and switches
`crates/ow-cli/tests/not_ported.rs` to `--slice-run` (all granted). No parsing behaviour changes. The rules are the C++ ones of `src/mover.*` and `src/dialog.*`, which come from
`docs/verification/SANCTUARY_MOVER_PROTOTYPE.md`, NATIVE_DIALOG.md, NATIVE_DIALOG_GROUPS.md and
NATIVE_BEHAVIOR_CONTEXT.md. None was checked in the running game; there was no session in this lane, so everything
stays UNVERIFIED as a game rule.

- **Shipped shapes** (public structs and enums that may grow are `#[non_exhaustive]`).
  - `mover::Mover::new(cooked, package, actor, action, ParityMode, StubPolicy)` and `Mover::bind(vm, ..)`;
    `notify(finished, reverse)` and `advance(seconds)` give `MoverResult { steps, checkpoint, error }`;
    `loading_diagnostics()`, `store_notes()`, `timers()`, `clock()`, `vm()`, `actor()`. The Kismet entry points
    (`remote_event`, `mission_event`, `motion_finished`, `sequence_event`, `originator_event`, `output`,
    `advance_sequence`) return the C++ `Dispatch` (with its `Request`s), the C++ catching inside the call;
    `variables` returns `Result<Vec<Variable>, MoverError>`, as the C++ does not catch there.
    `MoverError { Binding, Vm, Def, Logged, Kismet, NotYet }` has `source()` and `cpp_text()` (the C++ `what()`);
    `Kismet` holds the executor error's text, the C++ text, because `KismetError` is neither `Clone` nor `PartialEq`.
  - `dialog::DialogSystem::new(package, ParityMode)`; every call takes the `&mut Vm` the system works with.
    `set_line_player`, `set_timed_line_player`, `set_test_line_length`, `line_ended`, `register_talker`,
    `pawn_dialog`, `trigger_on_component`, `set_tracked_mission`, `tick`, `advance_to`, `not_run`, `last_search`,
    `object_path`; the callbacks `on_line` (`LineSink`) and `on_line_stopped`. `DialogError { NotImplemented,
    Unresolved, Malformed, Vm, Def, Package }` has `source()` and `cpp_text()`.
  - **The behavior seam.** `DialogSystem::behavior(vm, &mut dyn BehaviorRun)`: the trait is what the C++ body reads
    from the `BehaviorProvider` and its run record (behavior object and name, initial run, linked outputs, the Context
    link's objects, the wait, the per-thread state as `DialogBehaviorState`). Lane W5-BEHAVIOR's kernel is not on
    main, so the caller that runs a provider (W6-MISSION) adapts its run to the trait; no `ow-behavior` type is named.
- **The mover binding** loads the actor and the action from the definition store (`DefStore::load_export` with the
  C++ prefixes, 26 and 4), one object per path as decision 2026-11-15-23 requires, not a copy as the C++ makes. The
  C++ prefix size check runs first with the C++ prefix ("object property prefix outside export"); a prefix the store
  derives differently is its `PrefixMismatch`. The ambient sound component is loaded by the store (its derived prefix,
  8 for an `Engine.ActorComponent` subclass; the C++ forces 8). Every C++ check is kept, in the C++ order. Two
  differences: the notification bytecode is decoded at the first call, not at binding (a function that does not
  decode fails its first notification instead of the binding); and the store's own `defs: no holder for <path>`
  lines, which have no C++ counterpart (its resource stand-ins are silent), are kept apart in `store_notes()` and do
  not count as a loading diagnostic. Every other line the load logs does.
- **Stub policy.** `--mover-event` is a gameplay command and refuses an unbound native (`StubPolicy::Refuse`, empty
  allow list), as architecture section 3.8 lists it. `--mover-probe` is not in that table; it keeps `LogAndZero`,
  the C++ `Runtime` behaviour, because `tests/vm_test.py` pins its broken-mover case by the logged `UNIMPLEMENTED`
  text.
- **The Kismet sequence** runs on lane W5-KISMET's executor (`ow_mission::kismet`, decision 2026-11-15-31). The
  sequence is the action path up to its last `.` (an action with none is the C++ "mover action path has no owning
  sequence"). The executor works over a decode context, not the VM, and its handlers only record requests, so the
  mover keeps a context of its own, as the hub ruled: a package store opened on the VM store's cooked root
  (`PackageStore::root`), a reflection index, an object space and a decode state, built with the `Kismet` at the first
  sequence call and kept (a failed build is retried at the next call, as the C++ leaves its `Kismet` unset). Two
  handlers, as `src/mover.cpp`: `Engine.SeqAct_Interp` records `Play` (+1) or `Reverse` (-1) on this mover's action
  and lists any other input at the host boundary; `Engine.SequenceAction` lists every other action
  (`<Class>:<op> <- <input>`, with the structured `Request`). One call clears the per-call state and the context's
  decode log, runs its entry and then every impulse due; what the executor or the entry throws goes to
  `Dispatch::errors` (and the trace and the executor's error lines are dropped for that call, as the C++ assigns them
  only after `run`), followed by the call's decode log lines as `runtime: <line>`. The sequence's objects are that
  context's: the notifications run on the VM's actor and action, and no state crosses between the two (the C++ shares
  one runtime; the handlers read only the executor's own objects, so no output depends on it). No Kismet shape blocked
  the wiring: `Handler`'s `&mut ValueCx` is what these handlers need.
- **The dialog**, where the Rust port differs from the C++ one (representation, not game rules):
  - objects are the store's, one per path; the C++ loads a copy per `path@package`. The parent search's identity is
    the object; the C++ key differs only where two packages export one path, which the identity policy makes one;
  - an object path is the C++ `pathOf`: the path of the export the reference resolves to (an import the store has not
    loaded is resolved with `CodeIndex::resolve_ref`, the C++ `resolveRef`), or the import's own path when it resolves
    to nothing;
  - the random `TalkData` pick is a new reference stand-in: under `ParityMode::Reference` the C++ `std::mt19937`
    seeded 12345 (`rng() % n`; checked against the C++ standard's value for the 10000th output); under
    `ParityMode::Off` the world's `CrtStream::Dialog` (`rand15 % n`). How the game picks is not read. No tracked
    test pins the C++ pick yet (every Fire act it would reach has one entry); it needs a row in architecture
    section 6.1, which this lane does not own;
  - a chain of more than 16 Trigger acts is `DialogError::Malformed` (the C++ has no bound), and a link entry without
    `LinkNumber` or `ToNodeID` is `Malformed` where the C++ reads through a null pointer. The C++ reads `LinkNumber`
    only once `FromNodeID` matched, and so does the port.
- **Checks.** Rust unit tests on invented packages: 15 for the mover (`mover/tests.rs`: the probe sequence, rollback
  after a logged or refused call, the binding refusals with the C++ texts, the timer rate and name checks, the
  callback limit, the delta bounds, the three natives' target checks, no owning sequence, a remote event's motion and
  host boundary, the motion's end, the event, output and time entry points, the action's variables) and 22 for the
  dialog
  (`dialog/tests.rs`: the behavior's waits and outputs, the empty context list, `bForcePlayImmediate`, priority
  blocking and interruption, the tracked-mission floor, echo and missing talkers, once per session, the last enabled
  entry, the template and node links, the no-match Trigger route, the not-implemented nodes, `GetDialogGroups`, the
  globals' indices, the three players, `mt19937`). Against the C++ binary on the invented packages of
  `tests/vm_test.py` (its `make()`: the mover, the broken mover and the truncated mover, with right, swapped and
  missing exports, a short command line and a wrong option, both commands): 25 of 25 identical in exit code, stdout
  and stderr. `tests/vm_test.py`'s `--mover-probe` site passes on the Rust binary. `tests/mover_parity_test.py` (in
  `mover.txt` and ctest `mover-rust`) builds an invented install, a toy mover with a toy Kismet sequence, and diffs both
  commands on both binaries: 13 of 13 cases as expected, 11 byte-identical and 2 checked differences. One is the stub
  policy (a finish notification reaching an unbound native: both fail it; the C++ text is the logged `UNIMPLEMENTED`,
  the Rust one `unbound native`, and the steps differ because the C++ runs on). The other is an export that is not an
  actor bound as the placed actor: both fail the binding with nothing on stdout; the C++ reads it from the 26-byte prefix
  and fails on what it finds ("invalid FName"), the store derives the export's own prefix and refuses the mismatch.
  `mover.txt` also holds three package-free diff lines (a missing source package).
- **The installed game's movers** (output under `local/` only). The installed packages hold 3,249 `InterpActor` and
  1,006 `SeqAct_Interp` exports, 120 packages with both. Bounded run: every one in the Sanctuary packages, up to 3 of
  each elsewhere; a probe pairs an actor with its package's first action, an event pairs each action with the first
  actor. Under `ParityMode::Reference`: `--mover-probe` 498 of 498 identical to the C++ (exit code, stdout, stderr).
  `--mover-event` on the Sanctuary packages after the wiring: for each of the 113 `SeqAct_Interp` exports of the
  Sanctuary packages that share a package with an `InterpActor` (2 more have none), every remote-event name of its own
  sequence (768) plus one unmatched name, with the package's first `InterpActor`: 881 of 881 identical to the C++ in
  exit code, stdout and stderr (Rust under `OW_PARITY=reference`). 29 of them request motion (22 play, 7 reverse) and
  so also run both notifications and the motion's end; 134 carry error lines (unsupported ops, unresolved links), the
  same on both. Under `ParityMode::Off` (a partial run, the first 19
  packages by name): 504 of 504 differ, all for one cause. Building `Engine.Actor`'s class defaults logs
  `defaults: skipped Actor.ReplicatedCollisionType (ByteProperty)` (an enum value its declared enum lacks, the Off
  rule of decision 2026-11-15-08), and the mover's C++ check "loading emitted a diagnostic" refuses the binding. The
  check is kept, not loosened; the open question belongs to the decoder.
