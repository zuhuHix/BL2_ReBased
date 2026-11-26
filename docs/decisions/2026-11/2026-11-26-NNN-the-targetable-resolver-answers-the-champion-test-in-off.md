## 2026-11-26: The targetable resolver answers the champion test in Off

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-TARGETABLE-RESOLVER, on main 785c2a4. Clean room: implemented
from docs/verification/NATIVE_TARGETABLE_RESOLVER.md (origin/docs/native-targetable-resolver ac1e95e, #783) section 5
alone; this lane did not read the executable. No session was run. Every rule is UNVERIFIED in the running game.

- **What was decided**, in `ow-attr`'s `value_resolvers::targetable`, Off only (Reference keeps the C++ stand-in's 0.0
  for `Targetable_Is_Champion`). The get and the get-base answer the same, the first answer met:
  1. `ValueToResolve` declared as a byte, else `BadData` (the port's `VmError::Script` at this layer); not 0: 0.0;
  2. no context: 0.0; a context not resident (a store handle not loaded or failed, or a stand-in without its slots):
     `NotRead`, "a context the store has not loaded";
  3. the `ITargetable` cast by the class chain, the class table only: no `Engine.Pawn`,
     `WillowGame.WillowInteractiveObject` or `WillowGame.WillowProjectile` on it: 0.0. **Added here:** an ancestry
     that ends, short of `Core.Object`, at a class with no super class (an import-only stand-in) cannot prove "no",
     so it is `NotRead` rather than 0.0;
  4. `IsChampion` by the first native class: `Engine.Pawn`, `WillowPawn` and `WillowPlayerPawn` 0.0; `WillowAIPawn`
     its `BalanceDefinitionState.BalanceDefinition` (None 0.0; not resident `NotRead`; an
     `AIPawnBalanceDefinition` its `Champion`, 1.0 or 0.0; another class the `DoesSpawnChampion` `NotRead` naming
     it); any other class the `IsChampion` `NotRead` naming it.
  - **The first native class:** a class with code in the store by its decoded body's native bit (an undecoded body
    counts as native, as `ow-world`'s `first_native` does); a class without code by the class table's native mark.
    `ow-attr` cannot depend on `ow-world`, so this is a second copy of that walk.
  - **A resolver the store has not loaded is `NotRead`** (step 0, the hub's ruling on review of #783): the get does
    not load it, since the note did not read whether the evaluator loads the resolver first. The installed probe met
    that case: the resolver is an unloaded handle (a subobject of `Targetable_Is_Champion`), so links 3 and 4 now
    stop at this refusal. The rule writes, loads and interns nothing. Add, remove and set-base keep their `NotRead`.
- **Tests** (`targetable_tests.rs`, invented classes): the note's tests 1 to 7 (8, Reference, is the existing test).
  Each call checks that the value and the base value agree and that the heap's writes, the store's load transitions
  and the name table are unchanged. `class_code_loads` is a VM counter: `ow-attr` has no VM and loads no class code,
  so there is nothing to count at this layer.
- **Measured** (installed, Off, release build, under the machine lock):
  - **the slice gate** (strict), main 785c2a4 and this branch: 6 ok / 8 not ok, 51.2% on both, every row unchanged.
    Link 1 stops at pass 2 of `InitializeBehaviorProviders` on a `CustomEnableCondition` the store has not loaded, and
    links 3, 4, 5, 13 and 14 run on link 1's world, so the resolver is not reached in the gate's own run yet;
  - **a `--combat-run` probe** with link 3's and link 4's spawns after `player:8` (`spawn:baby:6`, `spawn:adult:7`):
    both pass the resolver and stop at `WillowPawn.SetDefaultPhysics` "past its gates" (NATIVE_MIND_ROUTE.md section
    1), as the note's section 7 predicts. **One difference:** that refusal carries no "stopped after partial writes"
    prefix, where the note expected one.
- **Verified:** the tests on invented data. **Not verified:** everything in the running game.
- **Sensitive files:** none.
