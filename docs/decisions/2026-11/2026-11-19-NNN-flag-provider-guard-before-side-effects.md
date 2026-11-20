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
- #586's existing tests pass unchanged, the short-circuited And included.

**The installed Off `--slice-gate`** (strict, under the machine lock, pc-hummer), on this branch and on its base
4c4522d7: ok 4, NotYet 9, error 1 of 14 on both, every link the same. Link 9 (the only link that reaches the
evaluator) runs 2 of its 2 steps and stops at "FlagExpressionEvaluator.Evaluate: the interface class
GearboxFramework.IRuleEngineOwner is not loaded (a load is a write)" on both: the base's harness does not preload that
class, and #602's set-up (in the landing run) does.
