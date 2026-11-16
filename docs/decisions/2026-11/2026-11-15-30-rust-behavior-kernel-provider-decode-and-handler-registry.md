## 2026-11-15: Rust behavior kernel, provider decode and handler registry

AI-assisted (Claude Code). Lane W5-BEHAVIOR of the Rust port fills `crates/ow-behavior` (`provider.rs`, `kernel.rs`,
`registry.rs`, `handlers/`) and `crates/ow-cli/src/cmd/behavior.rs` (`--behavior-dump`, `--behavior-run`): the port of
`src/behavior.*` and of its two CLI branches in `src/cli.cpp`. It builds on decisions 2026-11-15-26 (the world) and
2026-11-15-27 (`ow_vm::latent::kernel_resume_at`, the kernel's 1/60 s floor). Nothing here was checked in the running
game; every rule keeps the label its record gives it (NATIVE_MISSION_DISPATCH.md A1/A2, NATIVE_BEHAVIOR_POPULATION.md
sections C and G, BEHAVIOR_DATA_DECODE.md), UNVERIFIED.

- **Interface** (contract "ow-behavior handler registry", architecture section 7).
  - `ow_behavior::provider`: `decode(vm, definition) -> ProviderData` (sequences, events with their gates, behaviors,
    links, variables, variable links, `values_decoded`, `diagnostics`), `BehaviorError` (`Vm`, `Package`, `Data`; a
    `NotYet` is `Vm(VmError::NotYet)`), `VALUE_SIZES` / `value_size`, `enum_names`, `provider_path_leaf`, `stock_path`,
    `class_path_of`, `object_name`.
  - `ow_behavior::registry`: `Handler::Run(HandlerFn)` (returns the selected output ids) and `Handler::Boundary {
    describe, outputs }`; `Registry` (exact class paths; `add` refuses a second registration, `set` replaces), the VM
    service `install` / `register` / `lookup`. An unregistered class is `NotYet { NotPorted }` naming the class.
  - `ow_behavior::kernel::Provider`: `new`, `handle` / `report_at_boundary` (the provider's own handlers, consulted
    before the VM's registry), `fire_event(vm, event, outputs, link_id, payload)`, `tick`, `set_sequence_enabled`,
    `register_consumer`, `object_input`, `int_input`, `contexts`, `linked_objects`, `bind_kismet_variable`, `graph`,
    the name-addressed readers, `run_info()` (`RunInfo`: `initial_run`, `has_linked_outputs`, `wait`, `state`), and
    the public `errors`, `not_yet`, `boundary`, `boundary_calls`, `trace`, `on_time`. `behavior`, `contexts`,
    `linked_objects`, `object_input` and `int_input` panic on an id or link that is not the provider's (documented;
    `get_behavior` answers `None` instead), and `from_data` requires the invariants `decode` checks.
  - **What a handler is not given yet.** `HandlerFn` receives no consumer (`SelfObject`) and no current context
    object. The route for W6 is additive: a field on the `#[non_exhaustive]` `RunInfo` or a `Provider` accessor, set
    by the kernel before each call. Changing the `HandlerFn` signature is not that route.
  - `ow_behavior::handlers::Pack` (a `NativePack`) registers the one world-only handler, `Behavior_CompareObject`.
- **The value block is a fit** (AGENTS.md rule 4). The type sizes are the C++ table (BEHAVIOR_DATA_DECODE.md
  "Layout"), read from `DefStore::raw_tail`, and every decode applies the oracles: exact consumption to the export's
  end, every type listed, every non-zero object word inside the package's tables; any failure trusts no value and
  says why. Enum bytes are named from the installed `GearboxFramework` enum exports, never by position.
- **Rules ported** (each UNVERIFIED in the game): the signed link id byte; the A1 event gates (`bEnabled`,
  `MaxTriggerCount`, `ReTriggerDelay`), every matching entry, the link-id filter, output publishing (by property name,
  or by connection index from a payload); A2 (recorded ids, -1 only with `bSupportsDefaultOutputLink`, the first
  selected link continuing the thread, no deduplication, 60 behaviors per thread per call); delayed links and the
  tick's due-time order; latent behaviors with `kernel_resume_at`; G1 (a latent thread of a disabled sequence ends);
  section C (enabled event after the bit, disabled event before, the mutex, consumer registration); G2's context
  objects and the `BehaviorContextData` input binding; named and named-Kismet variable references;
  `Behavior_CompareObject` (output 0 when `ObjectA == ObjectB` by stock path, else 1).
- **Differences from the C++, on purpose.**
  - **Depth-first without recursion.** The C++ runs a started thread by recursion, bounded only by its
    10,000-behavior guard; in Rust that depth overflows the stack (a debug test thread). The Rust runner keeps an
    explicit work stack that performs the same operations in the same order (the same trace, parks and order numbers),
    so package data cannot drive the recursion depth. The guard is kept.
  - **The runner works on the store's behavior definitions** (the C++ instantiated a copy per provider), so the input
    binding writes into the shared definition and clears `ContextObject` after, as the record says the game's runner
    does. The game's per-thread copy of a latent behavior is the handler's `RunInfo::state`. **On an error** (the
    bind, the handler, a boundary description or the clear), the properties the bind wrote get their earlier values
    back before the error is returned, so a failed run leaves the definition as it found it (the C++ exception left
    its copy bound). A sequence whose disabled event fails stays enabled, as the C++ exception skips the clear.
  - **The behavior's `Outer` is not forced to the provider.** The C++ sets each copy's `Outer` to its provider
    (`src/behavior.cpp`, for `BehaviorHelpers`' outer fallback). On shared definitions that would be wrong (one
    provider written as the outer of a definition another may share), so the store's outer stands: the provider, for
    a behavior cooked as its subobject.
  - **An N9 exception** (architecture 5.1). The kernel keeps the C++ provider's own `f64` time, advanced by
    `Provider::tick`, instead of `Vm::clock`, for parity with `--behavior-run`'s `tick:<s>` steps. Moving it onto
    `Vm::clock` is a follow-up; `tick` already takes `&mut Vm`, so the switch changes no signature.
  - **Names compare as FNames** (ASCII case folded) for events and sequences; the C++ compares texts exactly. Name
    texts are the object space's first spelling of each name; behavior and condition names are read from the package
    tables, as the C++ prints them.
  - **Handlers come from a registry**, not built into every provider: a provider made without the world pack has no
    `Behavior_CompareObject` (`NotYet`). The CLI registers the pack, so its output is the C++'s.
  - **`--behavior-*` on a cooked copy.** The definition store refuses to load a file's copy of a path another file
    holds first (identity by path, decision 2026-11-15-16). On that refusal only ("another file holds this path under
    the identity policy", pinned by a test with an invented cooked copy), the command opens a second store whose load
    set is the given file alone and reads the provider from it, as the C++ reads the file it is given. Every other
    refusal is the command's error. References into other
    packages are then stand-ins named by their import paths, which changes what a behavior's own object reference
    reads as in `--behavior-run`, not the dump.
- **Where the C++ tests cannot run on the Rust binary yet.** `tests/behavior_test.py` declares object properties with
  class reference 0, which the Rust reflection refuses (`names no class`), and its last section runs `--mission-run`
  (W6-MISSION). The invented-package parity is therefore pinned in Rust (`cmd/behavior.rs`, 15 commands on invented
  packages whose object properties name a class, compared with the C++ output for the same bytes), and
  `tests/rust_parity.d/behavior.txt` holds only the two CI-safe refusal lines.
- **Verified (automated, local, pc-hummer).**
  - 48 `ow-behavior` unit tests on invented packages (one or more per rule above, the `behavior_test.py` sections 1-3
    and 5 among them; after review r1 also the failed-run restore, the failed disabled event, and the order of a
    latent wait and its own delayed link falling due together) and 5 `ow-cli` tests (the 15 pinned commands, a
    cooked copy among them, the refusals, the step parsers).
  - C++ vs Rust on `behavior_test.py`'s own builders in the typed form (generated under ignored `local/`): 112 of 112
    commands (every dump and run of its sections 1-3 and 5, step and argument errors) equal in stdout and exit code;
    stderr equal too.
  - C++ vs Rust on the installed game (`WillowGame/CookedPCConsole`, one provider of each of the 123 packages,
    seeded pick; outputs under ignored `local/`): `--behavior-dump` 123 of 123 equal; `--behavior-run` with a fixed
    step list 123 of 123 equal (only 2 reached a behavior); `--behavior-run` enabling every sequence of the provider
    and firing each of its own events twice around ticks, 123 of 123 equal, 67 runs with a trace, 1,380 behavior
    executions, 0 error lines. 103 of the 123 paths are held by more than one package (the cooked-copy path above).
    Every behavior those runs reached was a boundary class: `Behavior_CompareObject` was not reached on game data.
  - Inventory: 32,185 provider exports in 123 of 914 packages, the census counts BEHAVIOR_DATA_DECODE.md records.
- **Open items.**
  - `tests/behavior_test.py` on the Rust binary: the reflection's refusal of an untyped object property (W2-REFLECT's
    rule) and `--mission-run` (W6-MISSION).
  - The C++ compares a resolved reference's path (its path in the holder file) with an object variable's path (its
    import path in the provider's file), so one object can compare as two (`Behavior_CompareObject`). The Rust copies
    this for parity; comparing objects by identity is the fix once parity no longer binds.
  - **G1 is applied only in part** (UNVERIFIED). The record ends every thread of a disabled sequence, at its start and
    before each behavior; here, as in the C++, only a resumed latent thread does. A disabled sequence's threads that
    wait on a delayed link, or were parked by the 60-behavior cap, still run, and a running thread is not stopped
    between behaviors. Kept for parity.
  - G2's per-context runs, `FilterObject` (G3), what the game does with a capped thread, and which clock the kernel
    reads (the N9 exception above): as in the C++, not modelled.
  - The consumer and the current context object for handlers (the additive route above), when W6 needs them.
- **UNVERIFIED.** Every rule above, as read from the executable by the records; none checked in the running game.
