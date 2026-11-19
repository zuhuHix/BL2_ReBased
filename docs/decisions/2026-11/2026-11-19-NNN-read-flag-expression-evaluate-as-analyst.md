## 2026-11-19: FlagExpressionEvaluator.Evaluate read as analyst

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-FLAG-EXPRESSION-EVALUATE, analyst only (clean room: this thread does
not implement it). The decision number is pending; NNN is a placeholder.

- **What:** `docs/verification/NATIVE_FLAG_EXPRESSION_EVALUATE.md` and one NATIVE_INDEX row.
- **How:**
  - the installed script, with `research/script_disasm.py`;
  - read-only Ghidra queries under the machine lock;
  - fields named with the project's layout tool;
  - Maya's skill data read from the installed packages through the definition store (a local probe, not committed).

  Raw output stays under ignored `local/analysis/flagexpr/`. No bl-sdk evidence is on record for it, and no
  real-game session was run.
- **Found:**
  - the evaluator walks its `FlagChain` with OR or AND and early exits;
  - each flag is read either from the context's rule engine (the store path) or through its definition (its computed
    expression, or its context resolvers and value resolver);
  - the mind's flag store creates a zeroed entry on a lookup, and re-evaluates computed flags once per time stamp;
  - the function-call resolver calls a named script function at most once per `CheckRate`.
- **The slice:**
  - Maya's `CanLiftTargetIf` is `IsTrue(Flag_Skills_CanPhaseLock)`, a computed flag: `IsFalse` of `DisablePhaseLock`,
    `PhaseLockOnHold` and `IsPlayer`, all landing in the target mind's store;
  - for an ordinary enemy the answer is true, with up to three entries written.
- **Proposed Off answer:**
  - the definition path in full;
  - `NotYet { NotRead }` for a rule engine's own store, the timed tests and unread resolver or store classes;
  - every store write signalled for the caller's partial-writes mark.
- Every rule is UNVERIFIED in the running game; the note lists six open questions and one bl-sdk check.
