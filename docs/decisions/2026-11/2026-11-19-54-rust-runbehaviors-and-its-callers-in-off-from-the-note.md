## 2026-11-19: Rust BehaviorBase.RunBehaviors and its callers in Off, from the analyst note

AI-assisted (Claude Code). Lane FIX-RUN-BEHAVIORS, the fresh implementer of `NATIVE_RUN_BEHAVIORS.md` (analyst lane
RUN-BEHAVIORS, decision 2026-11-19-47; landed with #432). This lane coded from that note and opened no analysis
output. For the context resolver it used `NATIVE_BEHAVIOR_CONTEXT.md`, which the note points at. There was no
real-game session, so **every rule is UNVERIFIED** in the running game. No dependency was added, no bounds check was
loosened and no `NotYet` guard was relaxed.

- **What was added** (`crates/ow-behavior/src/kernel/run.rs`, bound from the behavior kernel pack, **Off only**):
  - `BehaviorBase.RunBehaviors`. One kernel-info record serves the whole call: the zero value of `ApplyBehavior`'s
    record parameter, with only `bIsInitialRunOfThisBehavior` set. Each behavior gets the record the previous one
    left (its `out` value). The walk:
    - goes in index order;
    - reads the list's length again at each step, through the caller's `out` storage;
    - skips `None` entries;
    - runs each behavior's script `ApplyBehavior` with the self object, the record, the instigator, the other
      participant and a copy of the event data;
    - returns nothing.
  - `BehaviorBase.StaticGetBehaviorContext` and `StaticGetAllBehaviorContexts`, which the default `ApplyBehavior`
    and `Behavior_RunBehaviorCollection` reach.
    - The selector picks the base object: 0 self, 1 instigator, 2 other participant, 4 the struct's `ContextObject`.
      Selector 3 and any other value give nothing.
    - With no `InstancedDataContextName` the base object is the result.
    - With a name, a base object without `Engine.IInstanceData` gives nothing.
    - The "all" form writes its `out` array and answers whether it found one.
  - `InstanceDataHelper.GetFirstInstanceDataObject` answers `DefaultValue` when there is no name, no context, or a
    context without `Engine.IInstanceData`. The note does not name the class's package, so the path is tried under
    `Engine` and `GearboxFramework`; a path the package set does not declare binds nothing.
  - `SpecialMoveComponent.RunBehaviors` and `CanPlayBehaviorData`.
    - Without an owner interface (`SMI`), nothing runs and the test answers false.
    - Otherwise each entry that passes the side test has its `Behaviors` run by the same loop, with the owner actor
      as self and the other arguments empty. The entries are read once at the start.
    - The side test: `PlayServer` when `NetMode` is not `NM_Client` (3), `PlayClient` when it is, and `PlayLocal`
      when the owner's `Controller` answers `IsLocalController()` true. That last one reads the note's "locally
      controlled" as the stock check, UNVERIFIED.
  - The defaults of `ApplyBehavior` / `ApplyBehaviorToContext` and `Behavior_RunBehaviorCollection` are script.
    They run as installed and need only the natives above.
- **Output links are not collected.** Each `ApplyBehavior` runs in its own frame of the output-link collector
  (`kernel::natives::collect`), and the ids are dropped.
  - A link a behavior selects leads nowhere, as the note reads it.
  - It never reaches an outer behavior's list. That is the note's section 5 warning about a port that runs
    `ApplyBehavior` inside `ow_behavior`'s collector.
- **What stays `NotYet { NotRead }`** (the note's open questions):
  - a wait request outside a thread: the record's `NextExecutionDelayTime` left non-zero (question 1);
  - a named context or instance-data lookup on an object that has instance data (question 2: which entry kinds
    count; the resolver's entry layout was read coarsely);
  - a `Contexts` array that arrives non-empty (whether it is cleared first was not read);
  - `PlayLocal` on a controller with no `IsLocalController`.
  - `GearboxAnimDefinition.OnStart` / `OnServerStart` stay unbound (question 3).
- **Errors.** A port error inside a behavior (a `NotYet`, an unbound native) stops the walk with that error. The
  note's "a behavior that fails inside its script does not stop the walk" is a script runtime warning, which the VM
  logs and goes past.
- **Reference** binds none of these (R3). `ow-mission`'s bridge keeps its `RunBehaviors` binding (the C++ level-up
  stand-in, still `NotYet { NotPorted }` inside the level-up) in Reference only. It used to bind in both modes and
  answered `NotYet` in Off.
- **Tests** (8, `kernel/run_tests.rs`). They use an invented package for the classes a script dispatch reaches
  (`find_method` reads package code only) and hand-declared natives for the rest:
  - the walk's order, the skipped `None`, the shared record and the event-data copy;
  - an empty or None list;
  - links dropped while an outer frame still collects its own;
  - the wait `NotYet`;
  - Reference binds nothing;
  - the selector table and the non-empty array;
  - the instance-data defaults;
  - the special-move side test and run.
  - Not covered: the length re-read during a walk, which needs a script caller's `out` alias; and the
    instance-data `NotYet` paths, because a hand-defined class implements no interface.
- **`tests/mission_script_test.py`** (this branch's binary): **Off** 0 failures, 17 not yet, exit 3; **Reference**
  0 failures, 8 not yet, exit 3. Both are the same as main.
  - The 8 Reference checks still stop at the bridge's C++ level-up stand-in. It is not this note's rule, and R3
    keeps it in Reference.
  - The 17 Off checks stop first at the player's health pool (`NotYet` by design), before any `RunBehaviors`.
  - So this test does not exercise the new rule yet. Its toy `RunBehaviors` has the C++ shape (3 parameters), and
    its toy behaviors have no `ApplyBehavior`.
- **`--slice-gate`** (installed, pc-hummer; main's binary against this branch's; rows under ignored `local/`): the rows
  are byte-identical, strict 0/11/3 and past-errors 0/14/0. No slice link reaches `RunBehaviors` yet. Link 9 (the
  Phaselock cast, whose special-move start runs `SpecialMoveComponent.RunBehaviors`) has no headless driver, and the
  installed cast stops earlier, at `Pawn.IsFriendly`.
- **Review r1 of #439.**
  - **Item 1, registration.** The mission bridge registers the behavior kernel pack before any script runs. In Off,
    `RunBehaviors` is therefore bound on a `--mission-run` without the director; before this it was bound only once
    Marcus's provider was placed.
  - **Item 2, writes before a refusal** (the hub's ruling: no rollback):
    - (a) Before any entry is dispatched, every entry the list holds is resolved and checked, and the record is made;
      a record without `NextExecutionDelayTime` is refused there. A wait request itself is seen only after the
      behavior that made it ran.
    - (b) A failure after a dispatch is marked "stopped after partial writes"
      (`ow_behavior::kernel::run::PARTIAL_WRITES`). The bridge keeps it as `partial_stop` and runs no more script on
      that VM. `--mission-run` and `--slice-run` end the run at that step, and the step's row says "stopped after
      partial writes".
    - The special-move run makes its entries' side tests and checks before its first dispatch.
  - **Item 3, the instance-data interface.**
    - "No interface" now has to be proven: the class absent from the package set (`Vm::find_class_if_present`,
      new), or `Vm::implements_checked` (new) answering no. That is the public checked form decision 2026-11-15-20
      left open: an unproven "no" through an engine-native stand-in class is `NotYet`.
    - Any other lookup failure is kept.
  - **Item 4, the side test.**
    - `NM_Client`'s value is its position in `NetMode`'s declared enum, no longer the constant 3.
    - `PlayLocal` asks the owner's pawn, which is the owner when it is an `Engine.Pawn`; any other owner is
      `NotYet`.
  - **Item 5, the record** starts from `zero_value`, with no struct defaults.
  - **Item 6, the anim events.** `OnStart` / `OnServerStart` are bound in Off as `NotYet { NotRead }` naming open
    question 3.
  - **Tests:**
    - `ow-behavior`, 5 new: the preflight, the record from zero with a nonzero struct default, the interface
      `NotYet`, `PlayLocal` on a non-pawn owner, the anim events. The wait test now also checks the partial-writes
      mark.
    - `ow-mission`, 2 new: `RunBehaviors` bound without the director in both modes; a partial-write failure stops
      later script runs.
  - `ow-vm` gained the two public helpers above (no sensitive file).
- **Review r2 of #439** (the final round):
  - **Item 1, terminal means terminal.** A nested stop (for example `UpdateMissionStatus` → `RunBehaviors`) now unwinds
    everything at once.
    - `MissionScript::run` returns the "stopped after partial writes" failure and records it once.
    - The tracker's hooks (status, objective, kickoff, notify) return it, and the tracker routine that called them
      stops there: no status-changed effect, no observer, no kickoff record.
    - The native that ran the tracker fails, and the outer script unwinds. A later run on that VM fails the same way
      without running.
  - **Item 2, the preflight's queries.** `PlayLocal` calls `IsLocalController` only when it is a bound native, the
    stock read-only query. A script override, or a native nothing binds, is `NotYet` before it is invoked and before
    any behavior runs.
  - **Item 3, the call-entry check.** The preflight runs the VM's own call-entry check with the dispatch's arguments:
    `Vm::check_call`, new. It shares `interp::check_entry` / `parameter_items` with the call, with no copy of the
    logic. So a static-array parameter given a scalar is refused before the first behavior.
  - **Item 4, `--mission-run`.** The step result is matched, not `?`-propagated. A stop after partial writes, whether
    the step failed with it or recorded it, gives the step its row (`ok` false, `"stopped"` with the text), and no
    later step runs (`step_outcome`).
  - **Item 5, registration timing.** The kernel pack is registered early in Off only. Reference keeps the C++ timing,
    with the director's provider.
  - **Tests:**
    - `ow-behavior`: the call-entry refusal before any run; the side tests with a native query, a later refusal and
      a script override.
    - `ow-mission`: a failing status hook unwinds the routine; a nested stop unwinds the outer run, is recorded once
      and stops later runs; each mode's `RunBehaviors` body runs without the director, with the output-link binding
      timing checked.
    - `ow-cli`: `step_outcome`.
