## 2026-11-19: Link 9's harness set-up resolves IRuleEngineOwner and installs the empty scene

AI-assisted (Claude Code)

Lane HARNESS-LINK9-SETUP, with Claude Opus 5.5. The file number is a placeholder until the maintainer gives one.
Harness set-up only (`crates/ow-cli/src/cmd/slice_gate.rs`). No native rule, no dependency, and no bounds check
changed.

- **Why:** on main 4c4522d7, link 9's `cast` stopped at "FlagExpressionEvaluator.Evaluate: the interface class
  GearboxFramework.IRuleEngineOwner is not loaded (a load is a write)". That is the same case as `IFlagProvider` and
  `IAIInterface`, which the set-up already resolves.
- **What:**
  - `GearboxFramework.IRuleEngineOwner` joins `phaselock_setup_classes`, after the flag store's interface. Its line
    is the existing set-up-class harness input, marked a write and an UNVERIFIED stand-in for the game having it
    loaded.
  - `phaselock_world` installs the pickup worlds' empty scene layer (`ow_inventory::attach_mesh::add_empty_scene`),
    following the hub's ruling on #623. It is logged as the harness input "the scene layer: the pickup worlds' empty
    scene", an UNVERIFIED stand-in for the game's render, collision and physics registration.
- **Measured** (installed Off `--slice-gate`, strict, release builds, under the machine lock):
  - main 4c4522d7 and this branch give the same totals, ok 4, notyet 9, error 1 of 14;
  - only link 9's row differs. It passes 1 of 2 steps (`spawn`), and `cast` now stops later: "stopped after partial
    writes: a pawn's QueryInterface(GearboxFramework.IAIInterface) goes to its Controller, a WillowGame.WillowMind:
    GearboxMind's own QueryInterface answer ... is not read";
  - #623 (AttachComponent in the skill worlds) is not reached yet, so no merge with it was run.
- **Test:** the set-up test checks the new class's place and line on the toy packages.
- **UNVERIFIED** in game, like every harness input.
