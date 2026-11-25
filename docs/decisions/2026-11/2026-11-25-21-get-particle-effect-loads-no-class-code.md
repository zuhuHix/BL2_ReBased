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
  - **The walk:** it dispatches through the same resident-only lookup.
  - **Warm code:** with the code loaded, which is the game's case, the answers are unchanged.
- **Tests:**
  - **The fixture:** `fx()` now looks up `ExpressionEvaluator` and `AttributeExpressionEvaluator`, so their code is
    loaded as in the game, and the existing tests keep their answers. A `fx_cold()` leaves it cold.
  - **#685's pinned test,** un-ignored: on `fx_cold()` the lookup refuses with rule 5's text. It writes, draws and
    loads nothing, loads no code, the class stays cold, and no evaluator runs.
  - **A warm companion:** the same lookup with the code loaded answers as before and loads no code.
  - **The original "nothing is written or loaded" test** is back to the full counters, code loads included.
  - **Mutation check:** going back to `find_method`, with no rule 5, fails the pinned test.
- **Not verified:** nothing in the running game (**UNVERIFIED**). Whether the equip run's world has the evaluator
  code loaded when this native runs is not measured. If it does not, rule 5 is where that run would now stop, which
  is a typed refusal instead of a silent load. The installed slice gate was not run (no install yet).
