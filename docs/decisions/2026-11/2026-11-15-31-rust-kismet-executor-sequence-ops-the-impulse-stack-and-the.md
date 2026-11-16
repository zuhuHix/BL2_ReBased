## 2026-11-15: Rust Kismet executor: sequence ops, the impulse stack and the logic ops

AI-assisted (Claude Code). Lane W5-KISMET of the Rust port fills `crates/ow-mission/src/kismet.rs` (with its tests
in `crates/ow-mission/src/kismet/tests.rs`) and `crates/ow-cli/src/cmd/kismet.rs`, the port of `src/kismet.*` and of
the `--kismet-census` / `--kismet-run` branches of `src/cli.cpp`, and adds `tests/rust_parity.d/kismet.txt`. No
parsing behaviour changes. The scheduling rules come from `docs/verification/NATIVE_MISSION_DISPATCH.md` A4 (read from
native code); the logic ops follow the public UE3 sequence model, as the C++ does. None was checked in the running
game; there was no session in this lane.

- **Shipped shapes** (`ow_mission::kismet`; public structs and enums that may grow are `#[non_exhaustive]`).
  - `Kismet::new(&mut ValueCx, PackageId, sequence_path)`: every export whose outer is the sequence and whose class
    derives from `Engine.SequenceOp` is instantiated (`ow_object::cdo::instantiate_export`, 4-byte prefix), in export
    order. The executor works over a decode context (`ValueCx`: the store, the reflection index, the object space,
    the decode state), as the C++ works over the decode `Runtime`; it needs no VM.
  - Entry points `remote_event`, `mission_remote_event`, `events_for_originator` (query), `activate_event`,
    `activate`, `run`, `tick`; handler helpers `input_desc`, `output_index`, `fire`, `fire_named`, `variables`,
    `variable_value`, `prop`; `link_stats` for the census. Ops are addressed by their position in `ops()`.
  - `Handler = Rc<dyn Fn(&mut Kismet, &mut ValueCx, usize, i32) -> Result<(), KismetError>>`, bound by class path
    (any case) and found through the class chain, nearest first; `remote_sink` for remote events no op consumes.
  - `KismetError { SequenceNotFound, Reflect, Decode, Package, Heap }`: what the C++ throws. The wrappers are
    transparent (`Display` is the wrapped error's, `source()` its cause), so the CLI prints the C++ text. What the
    C++ reports as lines (unsupported op, unresolved link, the runaway guard) stays in `Kismet::errors`.
- **Rules ported as the C++ has them** (UNVERIFIED in the game): the stack of queued ops (most recently queued first),
  the links of one output counted in link order and pushed last to first, an op already queued keeps its place, a
  link's delay is the target input's `ActivateDelay` plus the output's, a disabled input or output fires nothing (and
  is traced), an input hit twice runs its op twice, an entered event fires all its outputs, an input on an event
  fires its first output, at most 1,000 ops per run; `SeqAct_ActivateRemoteEvent`, `SeqAct_Gate` (with
  `AutoCloseCount`), `SeqAct_SetBool`, `SeqCond_CompareBool`, `SeqAct_Delay`; remote events of class
  `Engine.SeqEvent_RemoteEvent` exactly, mission remote events by mission path and name, events by `Originator`.
  Not modelled, as in the C++: one input per op per run, `MaxTriggerCount` / `ReTriggerDelay`, latent ops other than
  `Delay`.
- **Where the Rust differs from the C++, on purpose.** A property the C++ dereferences without a check (`bOpen`,
  `AutoCloseCount`, `CurrentCloseCount`, `bResult`, `DefaultValue`, `Duration`) crashes the C++ when the op's class
  lacks it; here it is an error line. A number is written in the slot's own kind (the C++ bag takes any kind). A
  float read as an integer that is NaN or outside 64 bits (undefined in the C++) and a count passing `i32::MAX`
  (wraps in the C++) are error lines. Names compare by the name table's first spelling. None of these arises in the
  invented packages or in the Sanctuary comparison below.
- **The CLI.** Same arguments, order of checks, JSON and exit codes as the C++: the source package is loaded first,
  `--kismet-run`'s entry words and `--tick` pairs are checked after the sequence loads, the ticks' seconds are read as
  `std::stod` reads them, one before each tick; world-acting ops (`Engine.SequenceAction` without a logic handler) go
  to `host_boundary` and fire nothing. The parity mode is passed to the decoder only.
- **Verified (automated, local, pc-hummer).**
  - 36 unit tests in `kismet/tests.rs`: every case of `tests/kismet_test.py` with the same expected values, plus the
    ticks the Python script cannot reach, equal delays in activation order, handler lookup, the remote sink, the
    linked `Duration` variable, a float target variable, and each Rust-only error line above.
  - `tests/rust_parity.d/kismet.txt`: four `diff` lines (the load-first order and its error text), 4/4 pass.
  - `tests/kismet_test.py` passes on the C++ binary and fails on the Rust one, not because of the executor: its toy
    `Engine.upk` declares the object properties `LinkedOp`, `LinkedVariables`, `Originator` and
    `AssociatedMissionDefinition` with no class, which the Rust reflection layer refuses ("names no class"; the
    decoder also checks a reference against the declared class). With a fixture change that only roots the toy
    classes in a toy `Engine.Object` and names it in those properties, the script passes on both binaries and all 43
    of its commands give byte-identical stdout, stderr and exit codes on both, in Off and in Reference mode. The
    change is the lane's proposal to the hub (it is not this lane's file); the manifest line waits for it.
  - The installed game (C++ `ow-package` of this branch, MSVC 14.50): `--kismet-census` of the 26 `Sanctuary*`
    packages is byte-identical, 26/26 (95 sequences, 2,180 ops, 3,150 outputs, 1,650 links, 0 unresolved, 0
    failed). The `--kismet-run` comparison over Sanctuary's event ops is in the lane's handoff. Outputs stay under
    `local/`.
- **Open items.**
  - The `tests/kismet_test.py` fixture change above, then its manifest line.
  - W5-WORLD's `Actor.Destroy` answers `NotYet` for an actor with Kismet `GeneratedEvents`. The destroyed-event class
    is not read, and `ow-world` cannot call `ow-mission`, so the hook needs a service seam in `ow-world`; not done
    here.
  - The executor runs over a decode context, not over the VM: `Vm`/`DefStore` has no `instantiate_export` with the
    decoder's stand-ins, so a world-hosted Kismet (W6-MISSION) needs that seam or its own context.
- **UNVERIFIED.** Every rule above: none was confirmed in the running game.
