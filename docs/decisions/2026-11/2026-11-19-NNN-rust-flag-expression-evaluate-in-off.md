## 2026-11-19: FlagExpressionEvaluator.Evaluate, in Off

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-FLAG-EXPRESSION-EVALUATE, slice link 9. Coded from
docs/verification/NATIVE_FLAG_EXPRESSION_EVALUATE.md alone (PR #527 at f28a8d1d). Clean room: lane
ANALYST-FLAG-EXPRESSION-EVALUATE read the native, and this lane did not open the executable or the analysis output.
Synthetic fixtures only.

- **What changes (`ow-skills` `action_skill::flag_expression`, bound first-wins under
  `GearboxFramework.FlagExpressionEvaluator.Evaluate` by the action-skill pack, Off only).** The note's sections 3, 4
  and 6:
  - **the context's rule engine `R`:** an `IRuleEngineOwner` context (only `GearboxMind`'s `MyRuleEngine` is read);
    else a `GearboxPawn`'s controller, or its driven vehicle's; else a `RuleEngine` or `Rule` context;
  - **the chain:** an empty chain is true; a None definition is false; the conditionals are combined by `CEOP_Or` or
    `CEOP_And` (any other operator gives false), with the early exits;
  - **the definition path:**
    - the context passes through `ContextResolverChain`, each resolver through ow-attr's registered context
      resolvers. A resolver that answers None ends it with None, and an empty chain keeps the context;
    - then either a computed flag, evaluated on the resolved context (a nested flag evaluator goes through this rule
      directly), or the value resolver;
  - **the mind's flag store:** a lookup adds the definition and a zeroed entry, and re-evaluates a computed flag once
    per world time. `RuleEngineFlagValueResolver` reads the context's own store when its class answers
    `IFlagProvider`. `ObjectFunctionFlagValueResolver` finds the store through `QueryInterface(IFlagProvider)`, and
    once `CheckRate` has passed it calls `FunctionCall` on the context and sets the flag, refreshing its time;
  - **the flag set, read and expiry rules** of section 4.2;
  - the time is the level's `WorldInfo.TimeSeconds`. With no `WorldInfo` it is `LayerAbsent`.
- **The hub's interim ruling on #527's review** (until the revised note):
  - only the receiver and the `FlagChain` shape are checked up front;
  - each conditional is checked when the chain reaches it, so a short-circuited branch never refuses, and a
    refusal after an earlier write is marked;
  - a reached timed test is `NotRead`;
  - the unanswered paths refuse `NotRead`: no context object, an empty `ContextResolverChain`, a context the chain
    does not resolve, and a definition with neither an expression nor a value resolver.
- **Refused with `NotYet { NotRead }` when reached:**
  - an unloaded receiver, flag definition, resolver or expression (nothing is loaded);
  - the timed tests (types 2 and 3), when reached;
  - a rule engine's own flag store (the store path, a `RuleEngine` context, a pawn's controller's rule engine);
  - a rule engine owner other than a `GearboxMind`;
  - a value resolver class other than the two read;
  - a computed flag that also has a value resolver;
  - a flag store that is not a `GearboxMind`'s;
  - a flag store whose two arrays are out of step (open question 4).
- **Refused after writes, marked:** a fractional function-call result, whose rounding is not read (open question 5).
  Only a bool or an integral number is taken. The native marks its own `NotYet` "stopped after partial writes: " once
  it has written, keeping the kind and record. Reference is unchanged.
- **Tests** (`ow-skills` `flag_expression_tests.rs`, invented classes):
  - an empty chain;
  - `Or` and `And` early exits, the second conditional's function never called;
  - a None definition;
  - `IsTrue` and `IsFalse` through the mind's store: the added entry, `CheckRate`, the function setting the flag;
  - types 2 and 3 refused;
  - an unloaded definition: `NotRead`, unmarked, nothing loaded;
  - an unread value resolver class;
  - the marked refusal after a write.
- **Checks.** `cargo test -p ow-skills` flag-expression tests 6 passed (a skipped timed branch and the unanswered paths added); `cargo test --workspace` 2,450 passed, 0
  failed, 46 ignored; crate edges OK.
- **Installed `--slice-gate`, Off, strict, under the machine lock,** on a throwaway local merge of this commit with
  #523 (d2227301, which carries #514 and #515). The `ow-skills` pack registration conflict was joined, and the merge
  was never pushed. Result: 1 ok / 4 not yet / 9 error.
  - Link 9 passes `spawn` (1 of its 2 steps) and stops in `cast` at this rule's own refusal: "FlagExpressionEvaluator.
    Evaluate: the unloaded evaluator GD_Siren_Skills.Phaselock.ActionSkill_Phaselock:FlagExpressionEvaluator_0"
    (`NotRead`).
  - Maya's `CanLiftTargetIf` evaluator is still an unloaded definition handle when the cast's script calls it. Who
    loads the skill definition's sub-objects before the cast is a question for the hub (a load is a write; this
    native loads nothing).
- **Aligned to the accepted note, e2724bc9** (#527, the hub's low fix included). This supersedes the interim and
  draft rules above where they differ:
  - **eager checks:** only the receiver and the `FlagChain` shape; each conditional is checked when reached;
  - **step 1:** a None `FlagDefinition` answers false, before the type check;
  - **loads are writes:** a reached handle (the receiver, a definition, its expression, its resolvers) is loaded when
    first used, through ow-attr's `ensure_loaded` (`LayerAbsent` with no definition store, `NotRead` for a stand-in),
    and counts for the mark. No unloaded handle is refused any more;
  - **section 4.0:**
    - a None context answers false;
    - a None chain entry is skipped;
    - a resolver that answers None ends the chain, and the context so far stays;
    - an empty chain keeps the context;
    - with neither an expression nor a value resolver, `IsTrue` is false and `IsFalse` is `NotRead`;
    - the hand-off of a computed answer to a value resolver is `NotRead` ("FlagDefinition: handing a computed answer
      to its value resolver"), after the expression is evaluated.
  - **Tests:** a None definition of a timed type is false; a reached handle is loaded; the definition path's cases.
    Flag-expression tests 6 passed; workspace 2,450 passed, 0 failed.
  - **Installed gate, rerun** on a throwaway merge with #523 (d2227301): 1 ok / 3 not yet / 10 error. Link 9 still
    passes `spawn` (1 of 2), and its `cast` now gets past `Evaluate`. It stops at the unbound native
    `BehaviorProviderDefinition.SetObjectBehaviorVariable(behaviorvariablevalue,object)`, a VM error.
- **UNVERIFIED.** Nothing here was checked in the running game. The note's open questions 1 to 6 stand.
