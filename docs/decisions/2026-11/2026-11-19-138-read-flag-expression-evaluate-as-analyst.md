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
    `PhaseLockOnHold` and `IsPlayer`;
  - the target mind's `MyRuleEngine` picks the branch. With none (branch D, the port's case), `CanPhaseLock`'s
    expression runs: the three inner flags land in the mind's store, and an ordinary enemy answers true, with up to
    three entries written. With one (branch S), `CanPhaseLock` is read from the rule engine's own store, which was not
    read, so no answer is predicted.
- **Proposed Off answer:**
  - eager checks only for the receiver and the `FlagChain` shape; each conditional is checked when reached, earlier
    writes kept;
  - every case of the definition path stated, with `NotYet { NotRead }` for the unread parts: the store path, the
    timed tests, `IsFalse` with neither an expression nor a value resolver, a computed answer's hand-off, and unread
    resolver or store classes;
  - the kinds follow `ow-attr`'s split; any reached handle may load, and a load counts as a write, as do store
    entries, flag updates and expiry.
- **Review r1 of #527, folded in.**
  - every case of the definition path is stated (None context, empty or partial resolver chain, neither expression
    nor resolver);
  - the timed tests are bounded (clock, threshold, held-true time read; the rest refused);
  - the store-path claim is limited to what the evaluator itself does;
  - loads and the `NotYet` kinds are defined, and the checks are made lazy;
  - the prediction and the live check are given per branch.
- Every rule is UNVERIFIED in the running game; the note lists seven open questions and one bl-sdk check.
