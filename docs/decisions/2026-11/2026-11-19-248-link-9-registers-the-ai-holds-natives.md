## 2026-11-19: Link 9's world registers the AI holds' natives, GearboxMind.GetAIComponent among them

AI-assisted (Claude Code)

Lane LINK9-GETAICOMPONENT-BINDING, with Claude Opus 5.5. The file number is a placeholder until the maintainer
gives one. Harness set-up only (`crates/ow-cli/src/cmd/slice_gate.rs`). No new rule, no dependency, and no bounds
check changed.

- **Why:** on main 241a6c47 (batch 9, with #602), link 9 errors after `spawn` and `cast` with "unbound native
  GearboxMind.GetAIComponent()". #602's decision predicted it: link 9's slice-gate world never registered
  `ow_combat::holds`, where #601's Off binding of that native lives.
- **What:** `phaselock_setup`'s last step was `ow_combat::pawn_interface::register`. It is now
  `ow_combat::holds::register`, which:
  - registers the same pawn `QueryInterface` rules first, still after the set-up classes are loaded;
  - then binds the AI holds' natives: the four kinds, `PhaseLockOnHold` and `GearboxMind.GetAIComponent`.

  It is the port's own Off registration, the one the combat worlds make through `ow_ai::awareness::holds::Pack`, not
  a harness stand-in, so it carries no harness label. It binds nothing under Reference, and a key bound first keeps
  its binding.
- **Test:** `link_9_s_set_up_binds_get_ai_component` (synthetic). The toy `GearboxFramework` package gains a
  `GearboxMind` declaring the native. Before the set-up, the native is unbound and `Vm::prepare_call` refuses it;
  after, it is bound.
- **Measured** (installed Off `--slice-gate`, strict, release build, under the machine lock): ok 4, notyet 10, error 0
  of 14.
  - Link 9 passes 1 of its 2 driver steps (`spawn`).
  - `cast` now stops with a NotYet, not an error: "AttributeDefinitionBase.ResolveContext: the context resolver
    GD_Siren_Skills.Attributes.IsPhaselocked.PawnAttributeContextResolver_0 is an unloaded handle (a load is a
    write)".
- **UNVERIFIED** in game.
