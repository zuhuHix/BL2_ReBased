## 2026-11-25: GetParticleEffect loads no class code

AI-assisted (Claude Code, Opus 5.5). Lane PARTICLE-EFFECT-CODE-LOAD, stacked on #685 (decision 2026-11-25-20).
A loading-discipline fix, not a rule change: the rule
is docs/verification/NATIVE_GET_PARTICLE_EFFECT.md's, unchanged ("the native writes nothing, draws nothing and loads
nothing of its own"). Synthetic fixtures only.

- **The finding (#685):** `ow-inventory` `equip::particle_effect` dispatched each entry's `Evaluate` with
  `Vm::find_method`. On a cold evaluator class that lookup loads the class's code (it registers functions and can
  intern names, a write), and #518's counters did not see it. #685's fixture showed three code loads
  (`GearboxFramework.ExpressionEvaluator` and `GearboxFramework.Object`).
- **What changes:**
  - **Preflight rule 5:** after rule 4, each reachable evaluator's `Evaluate` must resolve with its class code
    already loaded (`Vm::find_loaded_method`, as in #668 and #682). Otherwise the refusal is `NotYet { NotRead }`,
    "GetParticleEffect: an entry's evaluator class code is not loaded (a load is a write)". This comes before any
    evaluator runs, so before any write.
  - **Composites (review r1 of #686):** rule 5 also walks down the composite evaluators: an `ExpressionTree`'s
    `RootChild`, and a `CompoundExpressionEvaluator`'s `Expression1` and `Expression2`. Their `Evaluate` bindings, in
    `ow-combat`, evaluate those children through `find_method`, which would load a cold child's code after the
    preflight. Every node it reaches must be resident (else rule 3's refusal) and have its `Evaluate` code loaded.
    With that checked, the composites' own `find_method` lookups load nothing, so `ow-combat`'s shared dispatch
    (`evaluate_child` / `call_method`) is left unchanged for its other callers. At most 1024 nodes are walked.
    Other composite classes are not walked.
  - **Only the cold outcome is refused in the preflight (review r1 of #686):** a class with no `Evaluate` stays the
    walk's script error, raised only when the walk reaches that entry. So an earlier entry that passes still
    answers, as before rule 5.
  - **The walk:** it dispatches through the same resident-only lookup.
  - **Warm code:** in the fixtures where the evaluator code is loaded, the answers are unchanged. Whether the game's
    world has it loaded when this native runs is not observed (see "Not verified").
- **Tests:**
  - **The fixture:** `fx()` now looks up `ExpressionEvaluator` and `AttributeExpressionEvaluator`, so their code is
    loaded (a fixture condition, not observed game state), and the existing tests keep their answers. A `fx_cold()`
    leaves it cold. The entry's `Expression` field is typed as a plain object, so a test can put a class with no
    `Evaluate` there: every evaluator subclass inherits one.
  - **#685's pinned test,** un-ignored: on `fx_cold()` the lookup refuses with rule 5's text. It writes, draws and
    loads nothing, loads no code, the class stays cold, and no evaluator runs.
  - **A warm companion:** the same lookup with the code loaded answers as before and loads no code.
  - **The original "nothing is written or loaded" test** is back to the full counters, code loads included.
  - **Review r1 of #686:**
    - a warm compound with a cold child: refused before any evaluator runs, with every counter unchanged and the
      child still cold;
    - a warm compound with a warm child: it answers through the child and loads no code;
    - a warm first entry before a cold second one: zero evaluator calls, so the preflight runs before the walk;
    - an entry that passes before a later, warm entry with no `Evaluate`: it answers, and reaching that entry gives
      the walk's script error.
  - **Mutation check:** going back to `find_method`, with no rule 5, fails the pinned test.
- **Not verified:** nothing in the running game (**UNVERIFIED**). Whether the equip run's world has the evaluator
  code loaded when this native runs is not measured. If it does not, rule 5 is where that run would now stop, which
  is a typed refusal instead of a silent load. The installed slice gate was not run (no install yet).
