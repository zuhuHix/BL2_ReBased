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
