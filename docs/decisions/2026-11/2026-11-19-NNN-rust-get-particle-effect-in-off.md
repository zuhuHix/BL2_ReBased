## 2026-11-19: Rust GetParticleEffect in Off

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-GET-PARTICLE-EFFECT, for slice link 8 (equip). The decision
number is pending from the maintainer (three digits); NNN is a placeholder.

- **What:** `EffectCollectionDefinition.GetParticleEffect(object)` is bound in `ParityMode::Off` by the new module
  `crates/ow-inventory/src/equip/particle_effect.rs`, registered in the inventory pack. Reference is unchanged: the
  native stays unbound and the stub policy answers it.
- **Source:** NATIVE_GET_PARTICLE_EFFECT.md (PR #562 at 865ba911, accepted), and only that note. This lane read no
  native code or analysis output for it (clean room).
- **Behaviour:** the native walks `ParticleEffects` in array order. It returns the `ParticleEffect` of the first entry
  that has no `Expression`, or whose evaluator's `Evaluate`, called through the VM with the argument, answers true.
  When no entry passes it returns None. The default entry is never returned without evaluating the entries before
  it. The `ParticleEffect` handle is returned as stored, loaded or not. The native writes, draws and loads nothing.
- **Preflight:** write-free, and it loads nothing (under #518 a lazy load is a write):
  - an unloaded collection is `NotYet { NotRead }`;
  - `ParticleEffects` must be a dynamic array of a struct whose `Expression` and `ParticleEffect` are object
    references, else `VmError::Script` naming the field;
  - entries are checked in array order up to and including the first one with no `Expression`. An unloaded
    evaluator in that range is `NotYet { NotRead }`. Later entries are never reached, so they are not checked. This
    is a **port convention, not game behaviour**;
  - for each of those evaluators that is an `AttributeExpressionEvaluator`, each set operand of its `Expression` goes
    through the new `ow_attr::AttributeSystem::preflight_read_resident` (review r1 of #573, item 2). It is
    `preflight_read`'s check with no load: a cold attribute definition or chain resolver is `NotYet { NotRead }`, and a
    value resolver class nothing implements is refused by its name, all before any load or evaluation. This is a port
    convention too. A refusal that only an evaluation can find stays under the partial-writes rule.
    It asks a value resolver's mode availability through the new `ValueResolver::availability`, which sees only the
    object space and the mode, and never calls `available`, which gets the mutable context (review r2 of #573). The
    black-market resolver's Reference refusal moved there; the loading `preflight_read` keeps asking `available`,
    whose default now delegates to `availability`.
- **Beyond the note:** a handle whose load failed (the collection, or an evaluator in the checked range) is a
  `VmError::Script`. The note does not cover it, and reading the handle as an empty object would hide the failure.
- **Errors:** the three #518 counters are read on entry. When any of them has moved, a `NotYet` from an evaluator takes
  the "stopped after partial writes: " mark once, keeping its kind and record. Every other error passes through
  unchanged, an unbound evaluator included. An `Evaluate` that returns anything other than a bool is a
  `VmError::Script`.
- **#518's counters:** #518 (`fix/rust-483-followup` at 1f1a2bc) supplies them. Its counter files are copied here
  byte-identical, as other lanes have done:
  - `crates/ow-core/src/rand.rs`, `crates/ow-defs/src/store.rs` and `crates/ow-object/src/heap.rs`;
  - since review r1 of #573 (item 1), also its VM instrumentation: `crates/ow-vm/src/api.rs`, `interp/aggregate.rs`,
    `interp/eval.rs`, `interp/mod.rs`, `natives_core.rs` and `states.rs`. Without them a script array change or state
    change does not move the write counter, so a script evaluator's later `NotYet` would lose its mark. The hub records
    #518 as a landing dependency of #573.
- **Wiring (Off only, and only for `--equip-run` and link 8;** review r1 of #573, item 3: the pickup links 6 and 7 and
  the fight-area world keep their packs and preparation unchanged):
  - The equip run's world now also registers `ow_skills::action_skill::Pack`, for its
    `AttributeExpressionEvaluator.Evaluate`, which the slice gun's collection reaches. Before this change the world
    did not register that pack. The pack binds its other natives too: the skill manager, constraint, grade and
    evaluator natives.
  - The equip run's set-up preloads the muzzle-flash collections, as the hub ruled, the way #528 preloads the body
    class and #541 the unlock data. For the barrel part and then the weapon type, a set `MuzzleFlashPSTemplates` is
    loaded. Then come the entries' `Expression`s up to the first entry with none, the native's own boundary (item 4).
    For an `AttributeExpressionEvaluator`, its operands' attribute definitions and their chains' resolver objects are
    loaded too (item 2). Each load is logged as a harness input, and link 8's report now lists them, also when `make`
    fails (item 5). The particle systems are not loaded.
- **Verified:** synthetic tests (rule 9). Two are in `ow-attr`: a resolver whose `available` would write is never
  called by the resident preflight, and the Reference refusal is kept. The other 17 run on an invented package in a
  synthetic store:
  - the order and the context;
  - the default comes only after every earlier entry;
  - None cases;
  - the effect is returned unloaded, with no counter moved;
  - the three preflight rules, including an unloaded evaluator after the default entry going unchecked;
  - the partial-writes mark with and without a write, and for a script evaluator whose only write is an array change
    (it fails without #518's VM files);
  - a cold operand of an attribute evaluator, and a resident one with an unimplemented resolver, both refused before
    any evaluation with no counter moved;
  - a non-bool answer;
  - an unbound evaluator;
  - the call through the VM;
  - Reference binds nothing.
- **Probe (local, not pushed):** this change merged with #534 and #552 and run through the installed Off
  `--slice-gate`. Link 8 passes `GetParticleEffect`'s preflight and reaches the evaluators. It stops at `init` on
  `DamageTypeAttributeValueResolver` (NATIVE_ATTRIBUTES.md section 6), with the partial-writes mark, because the
  evaluator's own evaluation moved a counter. The expected next stop, `GetExpLevelFromManufacturerData`, is not
  reached yet.
- **UNVERIFIED in game:** every rule, including that `Evaluate` here is the evaluator class's own virtual (the note's
  open question 1).
