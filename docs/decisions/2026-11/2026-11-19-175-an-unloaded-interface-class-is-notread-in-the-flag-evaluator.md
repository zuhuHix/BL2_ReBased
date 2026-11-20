## 2026-11-19: An unloaded interface class is NotRead in the flag evaluator, not a "no"

AI-assisted (Claude Code, Opus 5.5). Lane FIX-FLAG-IMPLEMENTS-NOTYET, on #535 (`fix/rust-flag-expression-evaluate` at
7f8fc7d). The evaluator exists only there, not on main or on #579's stack, so this change is stacked on #535. Synthetic
fixtures only.

- **What changes (`ow-skills` `action_skill::flag_expression`, Off only; Reference binds nothing, as before).**
  `FlagExpressionEvaluator.Evaluate` asks objects for `GearboxFramework.IRuleEngineOwner` (the context's rule engine)
  and `GearboxFramework.IFlagProvider` (the flag store) by their class tables. Those tables only answer for an
  interface class that is loaded. Before this change, an interface class that was not loaded answered "no" for
  `implements`, so a store or a rule engine could be missed silently. Now:
  - an interface class that is not loaded answers `NotYet { NotRead }`, "the interface class <path> is not loaded
    (a load is a write)", never "no". A loaded class that does not implement the interface still answers "no";
  - each class is required only where the remaining path may still ask for it, and before the next load, write or
    draw (review r1 of #586):
    - `IRuleEngineOwner` where the rule engine search asks for it. That search reads only and comes before the
      conditional's first load;
    - `IFlagProvider` before a reached definition is loaded, when the definition may still need a store: it is not
      loaded yet, or it has a `ValueResolver` or an `EvaluationExpression`. A nested evaluator may ask either
      interface, which is the port's conservative choice;
    - both before a receiver handle is loaded, because its chain is unknown until then;
  - so an empty chain, a short-circuited branch, a None definition and a definition with neither field answer as
    before, even when an interface class is missing. With no context, nothing is required: no interface question is
    reached.
- **Callers checked.** The module's `implements` has two callers, both inside the evaluator: the rule engine owner
  check, and `RuleEngineFlagValueResolver`'s store check. The function-call path's `QueryInterface` already refused
  an unloaded `IFlagProvider` with `NotRead`, but after writes; it now refuses before the definition's load,
  because the definition's `ValueResolver` makes `may_need_store` true. Not covered by a test yet (review r2 of #586,
  low): a cold definition handle with `IFlagProvider` missing, which needs a second invented package export. No native outside
  the module calls this helper. The other `implements` users (`Vm::implements` and `Vm::implements_checked` in
  ow-vm, the casts in `interp/eval.rs` and `natives_core.rs`, ow-world `class_route`, ow-attr's
  `implements_interface`, the ow-behavior kernel, ow-combat `pawn_interface`) are unchanged. What changes outside is
  what `Evaluate` returns to its script callers, such as `CanLiftTargetIf` on link 9: a missing interface class now
  stops them with `NotRead` where they used to get false.
- **Tests** (invented classes, rule 9): `a_cold_receiver_stays_unloaded_on_an_interface_refusal` (an unloaded
  receiver handle stays unloaded and moves no counter on the refusal, and loads once the interfaces are there), and
  `an_unloaded_interface_class_is_not_read_before_any_write`, which covers:
  - a loaded class that implements `IFlagProvider` (its store is read);
  - a loaded class that does not (false);
  - each interface class left out of the loaded classes: `NotRead` with that text, no write counter moved, nothing
    marked, and the store untouched;
  - no context: false;
  - with a missing class and a loaded receiver: an empty chain (true), an And that stops at a None definition
    (false), and a definition with neither field (false without `IFlagProvider`; with `IRuleEngineOwner` missing,
    the rule engine search still asks for it, so `NotRead`).
- **Verified:** the host tests only. **UNVERIFIED in the running game**: every rule of the evaluator, as before.
  The slice was not probed in this lane.
- **Link 9 (#579, not changed here).** The ordinary Phaselock path reaches the rule engine search, which asks for
  `IRuleEngineOwner`. #579's set-up preloads only `IFlagProvider`. In the combined stack, if nothing else loads
  `IRuleEngineOwner`, link 9 stops at this `NotRead` where it used to read "no". Recommendation: link 9's set-up
  should preload `GearboxFramework.IRuleEngineOwner` alongside `IFlagProvider`, logged as the same kind of harness
  input. In the game both are classes of the loaded `GearboxFramework` package.
