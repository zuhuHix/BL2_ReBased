## 2026-11-19: The flag evaluator's provider guard comes before any resolver side effect

AI-assisted (Claude Code, Opus 5.5). Lane FOLLOWUP-FLAG-PROVIDER-ORDER, from a Sol recheck on main of #586's code. The
decision number is pending from the hub (three digits); NNN is a placeholder.

- **Base:** origin/main at 4c4522d7. This thread did not write #586.
- **No real-game session:** every rule is **UNVERIFIED** in the running game. The rules are NATIVE_FLAG_EXPRESSION_EVALUATE.md's
  (#527); this lane changes only when the port's refusals are raised, not what the evaluator answers.
- **Unchanged:** Reference (the rule is Off only). No dependency was added and no bounds check was loosened.

**The three findings and their fixes** (`crates/ow-skills/src/action_skill/flag_expression.rs`):
1. **The provider guard before any side effect.**
   - The problem: `IFlagProvider` was required per conditional, when it was reached. On an Or chain, a first
     conditional's context resolvers (whose loads are writes) could run before a later conditional's
     missing-interface refusal, which was then marked "stopped after partial writes".
   - The fix: `provider_first` reads the chain before the loop, as the loop will reach it, until the first conditional
     that refuses with no side effect of its own (a timed test, a type past the tests, or a loaded definition on the
     store path). If a conditional before that may need a store, `IFlagProvider` is required at once.
   - While every answer so far is known (None definitions only), it follows the loop's early exits. So a conditional
     the chain cannot reach asks for nothing, as #586 r1 required: an And stops at its first None definition.
   - The rule engine `R` it needs is worked out there, by reads only, and kept for the loop.
2. **A non-loading rule engine search.** The search asked for `IRuleEngineOwner` through `Vm::implements_checked`,
   whose `CodeIndex::implements` resolves a cold interface-table entry, and so loads its target class. It now asks
   through `Vm::implements_read_only` (new in `ow-vm`), which reads only entries already resolved
   (`CodeIndex::implements_resolved`, added here with #602's text so the two branches meet cleanly). An entry not
   resolved yet, with no resolved entry answering yes, is `NotYet { NotRead }` ("the interface table of `<class>` has
   an entry not resolved (resolving it is a load)"), and the entry stays cold. The engine-native stand-in check after
   a "no" is kept.
**Review r1 of #630 (Sol), the scan's two changes:**
- **Item 1, proven early exits only.** The scan treated any loaded definition's answer as unknown, so on an And chain
  whose first conditional answers false with nothing run, an unreachable second conditional's provider need turned
  that `false` into `NotRead`. The scan now follows the loop's early exits on every answer it can prove without
  running anything: a None definition (false), and `IsTrue` on a loaded definition off the store path with no
  `ValueResolver`, no `EvaluationExpression` and no context resolver (false). It stops at the first conditional whose
  answer it cannot prove: a later conditional's reachability then depends on running that one, so the later one is
  left to the per-conditional check.
- **Item 2, the mutable path: a `NotRead` before the first write, not a set-up guarantee.** A context resolver may set
  a definition's `ValueResolver` or `EvaluationExpression`, its own or a later conditional's, and the port cannot prove
  a resolver leaves them alone (resolvers are open to any class the attribute system binds). So when the first
  unproven conditional runs a context resolver and `IFlagProvider` is not loaded, the evaluator refuses
  `NotYet { NotRead }` up front, "a context resolver runs before the chain's store needs are known (it may set a
  ValueResolver or EvaluationExpression), and the interface class GearboxFramework.IFlagProvider is not loaded (a load
  is a write)", before the resolver runs. Every other unproven first conditional either refuses on its own with
  nothing run, or may need a store itself and so requires the interface. Either way, a later conditional is reached
  only after `IFlagProvider` is known to be loaded, and its per-conditional check cannot refuse on the interface after
  writes. The set-up alternative (loading the interface class before the run) was not taken, because it is a harness
  input and not the native's rule. The cost: a lone definition with a context resolver and neither field, which
  answered false with `IFlagProvider` missing, is now that `NotRead`.
- **Item 3:** the ow-object hunk (`CodeIndex::implements_resolved`, its set-up helpers and its test) is now #602's
  byte for byte (3e7c1294), so the two branches merge cleanly. The test that warms the cache through the ordinary
  lookup is `ow-vm`'s `implements_read_only_leaves_a_cold_target_cold` (through `implements_checked`).
- **Item 4:** that test's snapshot also holds the generator's draws and the name table's size.

3. **The store path before the guard.** A loaded definition that takes the store path (a known `R`, and no context chain
   or no value resolver, such as a computed definition) reports "a rule engine's own flag store" before the provider
   guard.

**Tests:**
- an Or chain whose first conditional's resolver would load a cold answer: the provider refusal comes first, no
  counter moves and the answer stays cold;
- a timed first conditional reports the timed refusal;
- a loaded computed definition with a known engine reports the store path with `IFlagProvider` missing;
- `CodeIndex::implements_resolved` and `Vm::implements_read_only` leave a cold entry cold and agree with the caching
  lookup once it resolved the entry;
- review r1 of #630: an And whose first conditional is a proven false answers false with nothing run, and the same
  definition as `IsFalse` gives the loop's own refusal (item 1); an Or chain whose first conditional runs a context
  resolver is refused with the mutable-path `NotRead` before the resolver runs, and answers false with
  `IFlagProvider` loaded (item 2);
- #586's existing tests pass, with one expectation changed by item 2 (the lone definition with a pass-through
  resolver, `IFlagProvider` missing: false before, the mutable-path `NotRead` now).

**The installed Off `--slice-gate`** (strict, under the machine lock, pc-hummer), on this branch and on its base
4c4522d7: ok 4, NotYet 9, error 1 of 14 on both, every link the same. Link 9 (the only link that reaches the
evaluator) runs 2 of its 2 steps and stops at "FlagExpressionEvaluator.Evaluate: the interface class
GearboxFramework.IRuleEngineOwner is not loaded (a load is a write)" on both: the base's harness does not preload that
class, and #602's set-up (in the landing run) does.

After review r1 of #630, the same run on the fixed head gives the same rows: ok 4, NotYet 9, error 1, every link as
before, link 9 still 2 of 2 at the `IRuleEngineOwner` stop.
