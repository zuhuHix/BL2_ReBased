## 2026-11-19: Link 9's target gets a mind, as a harness input

AI-assisted (Claude Code). Lane IMPL-LINK9-TARGET-MIND, slice harness, based on #568 @ 350fb829 (link 9's evidence
check). **The mind is a harness input, UNVERIFIED against the game.** In the game the population system spawns an
enemy together with its AI controller. The harness has no population system, so it stands in for that step. No
dependency or crate edge was added. The attribute, flag-expression, Phaselock and pawn-interface natives are
unchanged, and Reference is unchanged: these are harness steps of link 9's own Off world.

- **The mind** (`ow-cli` `slice_gate.rs`, `target_mind`, run after the three spawns of `phaselock_actors`):
  - a target whose class declares no `Controller` gets none. An already-possessed target keeps its mind: a set
    `Controller`, else a set `MyWillowMind`, which `adopt` also treats as possessed;
  - the class is the target's own `ControllerClass` when that names an `Engine.Controller` subclass. Otherwise it is
    `WillowGame.WillowMind`. The installed `WillowAIPawn` takes the fallback, which is the same class its default
    names;
  - the mind is spawned by the world's own spawn (`ow_world::actors::spawn`), so it is a listed actor with its own
    components;
  - it is linked the way the port already possesses an AI pawn (`ow_combat::hit::adopt`): the target's `Controller`,
    plus `MyWillowMind` when declared; the mind's `Pawn`, plus `MyWillowPawn` when declared. The script's `Possess` is
    not run, as in `adopt`;
  - all four destinations are checked before anything is written: each declared slot must be an object property
    that can hold the other side's class. A slot that cannot refuses the target with nothing spawned or linked, so a
    target never points at a mind whose `Pawn` was never set. Then the mind is spawned and the links are written
    together;
  - a harness line is logged before the class lookup. The driver copies the spawn's `harness input:` lines into the
    row's `harness`, also when the spawn stopped. A NotYet from the mind is marked "stopped after partial writes"
    once, at the start, with its kind and record kept (`NotYet::after_partial_writes`), because three actors are
    committed by then.
- **Two more harness steps, found by measuring.** On the installed probe, the mind alone did not change the flag
  answer (`Flag_Skills_CanPhaseLock` still read false). A probe-only debug trace found two causes in the harness:
  - `GearboxFramework.IFlagProvider` was not in the loaded classes. The flag evaluator checks a store by the class
    tables, so it read the mind's class as "not a flag provider", and `Flag_Skills_DisablePhaseLock`'s store was "no
    store";
  - link 9's world never registered `ow_combat::pawn_interface` (decision 2026-11-19-62), so the pawn gave the plain
    `QueryInterface` answer (None) instead of handing the question to its mind. `ow-mission`'s slice world already
    registers it.
- **Link 9's set-up** (`phaselock_setup`, run by `phaselock_world`; reviews r1 and r2 of #579). It resolves every
  class the harness needs before the driver starts: the target mind's fallback `WillowGame.WillowMind`,
  `IFlagProvider`, the two interfaces the pawn rules compare against (`IAIInterface`, `IPlayerMaster`), and each class
  whose answer `pawn_interface` registers. Then it registers the pawn rules, which find those classes loaded, so the
  cast loads none of them lazily (a load is a write). Each class gets the full class lookup, layout and code, even when
  its layout was already defined by another load. Each gets one `harness input: set-up class` line that names the
  layout and the code separately, each "ready" or "loaded at set-up". A class not in the packages loads nothing, and a
  failed lookup is logged with the state it found before the set-up stops. The row's `harness` carries these lines,
  also when the set-up fails.
- **The fallback is only read at possession** (review r2 of #579). `target_mind` uses the fallback class only when
  set-up made it ready (layout defined and code loaded, a read through `Vm::class_code_loaded`). Otherwise it refuses
  with nothing loaded, spawned or linked, so a lazy load can no longer write before the slot checks refuse.
- **For the hub, not changed here:** the evaluator's class check (`flag_expression.rs`, `implements`) answers false,
  not a NotYet, when the interface class is not loaded. A world that never loads the interface gets a false flag with
  no stop. That native belongs to another lane.
- **Tests on invented classes:**
  - the mind: with no `ControllerClass`, the fallback `WillowGame.WillowMind`; with a `ControllerClass` default naming
    a toy mind, that class, with `MyWillowMind` and `MyWillowPawn` linked. The mind possesses the pawn, the world
    lists it, and its harness line (marked UNVERIFIED) is logged; link 9's set-up runs first. A target class
    with no `Controller` gets no mind and no harness line;
  - a `MyWillowMind` slot narrower than the mind's class: a typed error at the spawn, no mind spawned, `Controller`
    still None;
  - an already-possessed target, by `Controller` and by `MyWillowMind` alone: its mind is returned, nothing is
    spawned, written or logged;
  - set-up classes: absent from the packages, declared only (loaded, reported as a write), already loaded, and a
    layout already defined with no code (the code loaded at set-up, and the line says so); the set-up reports one
    line per class, in order;
  - with no set-up, a cold fallback mind is refused with nothing loaded or spawned, also for a target whose slot is
    too narrow;
  - the partial-writes mark is added once at the start, also when the text already mentions it.
- **The slice** (installed, Off, strict, under the machine lock; a local merge never pushed: this branch, which
  holds #568 @ 350fb829, with #535, #564, #548, #555, #560, #523 and the link 9 spawn branches):
  - **B, this branch:** 1 ok / 4 NotYet / 9 errors. Link 9's `spawn` step completed; the driver records `cast` before
    it runs it, and the cast reached the registered `GearboxMind` rule, which answers **NotYet**: "a pawn's
    QueryInterface(GearboxFramework.IAIInterface) goes to its Controller, a WillowGame.WillowMind: GearboxMind's own
    QueryInterface answer … is not read" (NATIVE_PAWN_AI_INTERFACE.md). A NotYet skips the evidence check, so the
    cast is not shown to pass. Re-run after review r1 of #579: the same counts and the same stop, with the set-up
    lines in the row's `harness`.
    `Flag_Skills_DisablePhaseLock` now reads through the mind's store (false, so its `IsFalse` is true). The next flag,
    `Flag_Skills_PhaseLockOnHold`, reaches the AI component resolver, which asks the pawn for `IAIInterface`, and the
    pawn hands that to the mind;
  - **A, the same merge with `target_mind` and the pawn-rule registration switched off:** 1 ok / 3 NotYet / 10 errors.
    Link 9 is an error: "no stop, but no evidence" (the blocked path, as #568 found);
  - the other 13 rows are the same in A and B.
  - So what stops the mind is **`GearboxMind`'s own `QueryInterface` answer**. The port registers it as a rule that
    answers NotYet, because it is an open item of NATIVE_PAWN_AI_INTERFACE.md. This lane stops there.
- **Live check (bl-sdk, the RTSE way; not run):** on a spawned enemy, read `Controller` (expect a `WillowMind` whose
  `Pawn` is the enemy) and `ControllerClass` on its class default. Then evaluate `Flag_Skills_CanPhaseLock` with the
  enemy as context (expect true for an enemy Phaselock can lift).
