## 2026-11-19: Link 9's set-up preloads the phaselock attribute's context resolver

AI-assisted (Claude Code)

Lane LINK9-RESOLVER-PRELOAD, with Claude Opus 5.5. The file number is a placeholder until the maintainer gives one.
Harness set-up only (`crates/ow-cli/src/cmd/slice_gate.rs` and its set-up test). No rule, no dependency, and no
bounds check changed. Cast's preflight is untouched (H6's PREFLIGHT-ORDER-LINK9-CAST owns that).

- **The stop:** with #644, link 9's `cast` stopped at `AttributeDefinitionBase.ResolveContext`'s rule R2: "the
  context resolver GD_Siren_Skills.Attributes.IsPhaselocked.PawnAttributeContextResolver_0 is an unloaded handle (a
  load is a write)". The definition `IsPhaselocked` is loaded. Its `ContextResolverChain` entry, a subobject of it,
  is still a store handle.
- **Is it made resident earlier in the real flow?** Nothing in the port's flow loads it. In the game the skill's
  packages are loaded whole, so a definition's subobjects are resident with it; that package load is not a step the
  port has. So this is a harness input, not a ported rule.
- **What:** `PHASELOCK_SETUP_OBJECTS`, with that one resolver, loaded at link 9's set-up after the set-up classes by a
  new `preload_object`.
  - Its line is "harness input: set-up object <path>: loaded at set-up (a write; UNVERIFIED stand-in for the
    package's load with its definition)", or "ready, nothing loaded", or "not in the packages, nothing loaded".
  - Whether it loaded is judged by the store's load counter.
  - A failed load keeps its error.
- **Tests:** `a_set_up_object_is_loaded_once_and_reported` on a toy export (loaded once, then ready, and a missing
  path). The set-up test counts the object lines after the class lines.
- **Measured** (installed Off `--slice-gate`, strict, release build, under the machine lock):
  - ok 4, notyet 9, error 1 of 14; #644's head alone gives ok 4, notyet 10, error 0;
  - link 9 passes 1 of 2 steps (`spawn`), and `cast` now gets past the resolver;
  - its new first stop is an **error**: the unbound native `AttributeDefinitionBase.AddAttributeModifier(object,
    object,bool)`. Its Off binding for the action-skill pack is IMPL-ADD-ATTRIBUTE-MODIFIER
    (`fix/rust-add-attribute-modifier`), not on main. It is not FlagExpressionEvaluator's lazy flag-definition loads
    (LINK9-FLAGDEF-PRELOAD).
- **UNVERIFIED** in game.
