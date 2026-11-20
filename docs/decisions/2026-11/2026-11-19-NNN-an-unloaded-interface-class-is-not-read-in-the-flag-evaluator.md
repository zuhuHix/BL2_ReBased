## 2026-11-19: An unloaded interface class is NotRead in the flag evaluator, not a "no"

AI-assisted (Claude Code, Opus 5.5). Lane FIX-FLAG-IMPLEMENTS-NOTYET, on #535 (`fix/rust-flag-expression-evaluate` at
7f8fc7d). The evaluator exists only there, not on main or on #579's stack, so this change is stacked on #535. Synthetic
fixtures only.

- **What changes (`ow-skills` `action_skill::flag_expression`, Off only; Reference binds nothing, as before).**
  `FlagExpressionEvaluator.Evaluate` asks objects for `GearboxFramework.IRuleEngineOwner` (the context's rule engine)
  and `GearboxFramework.IFlagProvider` (the flag store) by their class tables. Those tables only answer for an
  interface class that is loaded. Before this change, an interface class that was not loaded answered "no" for
  `implements`, so a store or a rule engine could be missed silently. Now:
  - with a context, both interface classes must be loaded. This is checked after the receiver's type check and
    before its load, so before any write. Otherwise the answer is `NotYet { NotRead }`, "the interface class <path>
    is not loaded (a load is a write)";
  - a loaded class that does not implement the interface still answers "no";
  - with no context, nothing is checked: no interface question is reached (the rule engine is None, and the
    definition path's None context answers false);
  - the check is eager. A chain that would never reach an interface question (all None definitions, or a short
    circuit first) also refuses when a class is missing. That is the price of refusing write-free, and it is the
    port's own choice, not the note's.
- **Callers checked.** The module's `implements` has two callers, both inside the evaluator: the rule engine owner
  check, and `RuleEngineFlagValueResolver`'s store check. The function-call path's `QueryInterface` already refused
  an unloaded `IFlagProvider` with `NotRead`, but after writes; it now meets the same eager check. No native outside
  the module calls this helper. The other `implements` users (`Vm::implements` and `Vm::implements_checked` in
  ow-vm, the casts in `interp/eval.rs` and `natives_core.rs`, ow-world `class_route`, ow-attr's
  `implements_interface`, the ow-behavior kernel, ow-combat `pawn_interface`) are unchanged. What changes outside is
  what `Evaluate` returns to its script callers, such as `CanLiftTargetIf` on link 9: a missing interface class now
  stops them with `NotRead` where they used to get false.
- **Tests** (invented classes, rule 9): `an_unloaded_interface_class_is_not_read_before_any_write`, which covers:
  - a loaded class that implements `IFlagProvider` (its store is read);
  - a loaded class that does not (false);
  - each interface class left out of the loaded classes: `NotRead` with that text, no write counter moved, nothing
    marked, and the store untouched;
  - no context: false.
- **Verified:** the host tests only. **UNVERIFIED in the running game**: every rule of the evaluator, as before.
  The slice was not probed in this lane. Link 9's set-up on #579 preloads `IFlagProvider`. Whether
  `IRuleEngineOwner` is loaded there is not checked here; if it is not, link 9 will now stop at this `NotRead`
  instead of reading false.
