## 2026-11-19: Rust AttributeInitializationDefinition.SetBaseValue in Off

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-SET-BASE-VALUE, for slice link 8 (`ApplyAnimThresholdAttributeValues` in
the weapon's init). The decision number is pending from the maintainer (three digits); NNN is a placeholder.

- **What:** `AttributeInitializationDefinition.SetBaseValue(object,attributeinitializationdata,object,object)` is
  bound in `ParityMode::Off` by the new module `crates/ow-world/src/world/set_base_value.rs`, next to the world's
  `EvaluateInitializationData`. Reference is unchanged: the C++ has no `SetBaseValue`, so the stub policy answers it.
- **Source:** NATIVE_ATTRIBUTES.md section 1 only: the evaluation of `BaseValue` with `ValueContextSource` (no
  override), then `DestAttribute.SetAttributeBaseValue(AttributeContextSource, result)`, false when `DestAttribute`
  is None. The two halves are `ow-attr`'s existing `evaluate_init` and `set_base`.
- **Where the record is silent:**
  - `bSuppressNotify` is taken as the script default, false, so the notification runs (UNVERIFIED);
  - the `out` struct is left unchanged (UNVERIFIED).
- **Write-free preflight:** an unloaded handle among `DestAttribute`, `BaseValueAttribute` and
  `InitializationDefinition` refuses `NotYet { NotRead }`, with nothing loaded.
- **Partial-writes mark:** #518's three counters are snapshotted on entry. A `NotYet` leaving the native after any
  counter moved is marked once, with its kind and record kept; other errors pass through. The counter files are
  copied byte-identical from 1f1a2bcc (#518 is open), as #535 did: `ow-core` rand.rs, `ow-defs` store.rs,
  `ow-object` heap.rs, and `ow-vm`'s interp aggregate.rs and eval.rs.
- **Verified:** 7 synthetic tests (rule 9):
  - the evaluated value becomes the base, and the value is recomputed;
  - the value context and the attribute context are separate;
  - no destination answers false and writes nothing;
  - the notification runs;
  - a `NotYet` after a write is marked once;
  - an unloaded destination refuses without loading;
  - Reference binds nothing.
- **UNVERIFIED in game:** every rule.
- **Review r1 of #572:** the preflight became a walk that loads nothing (`ow-attr`'s `preflight_set_base_value`), run
  before the evaluation. It covers the destination (not multi-context; its resolvers loaded and implemented, the
  value chain up to its first None) and every attribute and definition the initialisation data can reach.
- **Review r2 of #572:**
  - **Reachable branches only:** a conditional's scan stops after an entry with no expressions, which always holds,
    so a later entry and the default are not checked. An expression's second operand is checked only with its
    first.
  - **The harness preload is narrowed to the ruling:** the destination attribute, its context resolvers, and its
    value resolvers up to the chain's first None. **Dropped from the preload:** a `BaseValue`'s `BaseValueAttribute`
    and `InitializationDefinition`, their resolver chains, and destination value resolvers past the first None. An
    unloaded one of those now reaches the native and refuses, so the slice shows it.
  - **Tests:** the lazy-resolver test reaches the resolver (a resolving context chain and a valid target), with a
    control that loads it through `set_base`. A new regression test has an unloaded attribute in an unreachable
    branch.
