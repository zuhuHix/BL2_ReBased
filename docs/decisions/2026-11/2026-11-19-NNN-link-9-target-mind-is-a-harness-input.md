## 2026-11-19: Link 9's target gets a mind, as a harness input

AI-assisted (Claude Code). Lane IMPL-LINK9-TARGET-MIND, slice harness, based on #568 @ 350fb829 (link 9's evidence
check). **The mind is a harness input, UNVERIFIED against the game.** In the game the population system spawns an
enemy together with its AI controller. The harness has no population system, so it stands in for that step. No
dependency or crate edge was added. The attribute, flag-expression, Phaselock and pawn-interface natives are
unchanged, and Reference is unchanged: these are harness steps of link 9's own Off world.

- **The mind** (`ow-cli` `slice_gate.rs`, `target_mind`, run after the three spawns of `phaselock_actors`):
  - a target whose class declares no `Controller` gets none, and one that already has a controller keeps it;
  - the class is the target's own `ControllerClass` when that names an `Engine.Controller` subclass. Otherwise it is
    `WillowGame.WillowMind`. The installed `WillowAIPawn` takes the fallback, which is the same class its default
    names;
  - the mind is spawned by the world's own spawn (`ow_world::actors::spawn`), so it is a listed actor with its own
    components;
  - it is linked the way the port already possesses an AI pawn (`ow_combat::hit::adopt`): the target's `Controller`,
    plus `MyWillowMind` when declared; the mind's `Pawn`, plus `MyWillowPawn` when declared. The script's `Possess` is
    not run, as in `adopt`;
  - a harness line is logged before the class lookup. The driver copies the spawn's `harness input:` lines into the
    row's `harness`, also when the spawn stopped. A NotYet from the mind is marked "stopped after partial writes",
    because three actors are committed by then.
- **Two more harness steps, found by measuring.** On the installed probe, the mind alone did not change the flag
  answer (`Flag_Skills_CanPhaseLock` still read false). A probe-only debug trace found two causes in the harness:
  - `GearboxFramework.IFlagProvider` was not in the loaded classes. The flag evaluator checks a store by the class
    tables, so it read the mind's class as "not a flag provider", and `Flag_Skills_DisablePhaseLock`'s store was "no
    store". `target_mind` now loads the interface when the packages hold it and it is not loaded yet, and logs a
    second `harness input:` line. A lazy load is a write, so this happens at set-up, before the cast;
  - link 9's world never registered `ow_combat::pawn_interface` (decision 2026-11-19-62), so the pawn gave the plain
    `QueryInterface` answer (None) instead of handing the question to its mind. `phaselock_world` now registers it,
    as `ow-mission`'s slice world already does.
- **For the hub, not changed here:** the evaluator's class check (`flag_expression.rs`, `implements`) answers false,
  not a NotYet, when the interface class is not loaded. A world that never loads the interface gets a false flag with
  no stop. That native belongs to another lane.
- **Tests:** a test on invented classes covers four cases:
  - with no `ControllerClass`, the fallback `WillowGame.WillowMind`;
  - with a `ControllerClass` default naming a toy mind, that class, with `MyWillowMind` and `MyWillowPawn` linked;
  - the mind possesses the pawn, is listed by the world, and its harness line (marked UNVERIFIED) is logged;
    `IFlagProvider`, absent before, is preloaded and logged once;
  - a target class with no `Controller` gets no mind, no preload and no harness line.
- **The slice** (installed, Off, strict, under the machine lock; a local merge never pushed: this branch, which
  holds #568 @ 350fb829, with #535, #564, #548, #555, #560, #523 and the link 9 spawn branches):
  - **B, this branch:** 1 ok / 4 NotYet / 9 errors. Link 9 passes both of the driver's steps (`spawn`, `cast`) and is
    now **NotYet** at the cast: "a pawn's QueryInterface(GearboxFramework.IAIInterface) goes to its Controller, a
    WillowGame.WillowMind: GearboxMind's own QueryInterface answer … is not read" (NATIVE_PAWN_AI_INTERFACE.md).
    `Flag_Skills_DisablePhaseLock` now reads through the mind's store (false, so its `IsFalse` is true). The next flag,
    `Flag_Skills_PhaseLockOnHold`, reaches the AI component resolver, which asks the pawn for `IAIInterface`, and the
    pawn hands that to the mind;
  - **A, the same merge with `target_mind` and the pawn-rule registration switched off:** 1 ok / 3 NotYet / 10 errors.
    Link 9 is an error: "no stop, but no evidence" (the blocked path, as #568 found);
  - the other 13 rows are the same in A and B.
  - So the unbound native that stops the mind is **`GearboxMind`'s own `QueryInterface` answer**, an open item of
    NATIVE_PAWN_AI_INTERFACE.md and NotYet in Off by design. This lane stops there.
- **Live check (bl-sdk, the RTSE way; not run):** on a spawned enemy, read `Controller` (expect a `WillowMind` whose
  `Pawn` is the enemy) and `ControllerClass` on its class default. Then evaluate `Flag_Skills_CanPhaseLock` with the
  enemy as context (expect true for an enemy Phaselock can lift).
