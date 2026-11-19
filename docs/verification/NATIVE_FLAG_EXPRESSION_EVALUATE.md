# Native rules of FlagExpressionEvaluator.Evaluate (draft, 2026-11-19)

AI-assisted (Claude Code, Opus 5.5), analyst lane ANALYST-FLAG-EXPRESSION-EVALUATE (decision 2026-11-19-NNN, number
pending). Analyst only: a different lane implements it (clean room).

**Sources:**
- the installed script, read with `research/script_disasm.py`;
- the class and struct layouts, from the cooked packages through the project's layout tool;
- the installed packages, read through the port's definition store (Maya's skill, a local probe, not committed);
- `Borderlands2.exe`, read locally with the project's Ghidra tooling (read-only queries, under the machine lock).

There is no bl-sdk session and no recorded live evidence on this native. Raw output stays under ignored
`local/analysis/flagexpr/`. This note is in our own words: no decompiler output, pseudo-code, addresses or listing
structure. **Every rule here is UNVERIFIED** in the running game.

**Why it matters.** With #523 (noun replication), slice link 9 passes 1 of its 2 steps: `cast` stops at this native,
unbound. Earlier notes name it: NATIVE_SPECIAL_MOVE_PLAY.md section 0 (Maya's `CanLiftTargetIf`) and
NATIVE_AMBIENT_NPC.md section 3 (the AI definitions' flag evaluators).

## 1. The caller on link 9's path

- `LiftActionSkill.GetPlayerAnimation(NewTarget)` and `LiftActionSkill.SelectTarget` (script) test
  `CanLiftTargetIf == None || CanLiftTargetIf.Evaluate(NewTarget)`. The argument is the target pawn.
- `Evaluate(ContextObject)` is declared native on `Engine.ExpressionEvaluator` and returns a bool. Every evaluator class
  has its own body behind one shared entry: the entry reads the one object argument and calls the evaluator class's
  own rule. So the key the slice reaches is `FlagExpressionEvaluator.Evaluate(object)`, the rule of
  `GearboxFramework.FlagExpressionEvaluator`.
- **Maya's data** (`GD_Siren_Skills.Phaselock.ActionSkill_Phaselock`, read from the installed packages). See section 5
  for the whole tree.

## 2. The shapes (from the packages)

- **`FlagExpressionEvaluator`:**
  - `FlagChain`: an array of `Rule.FlagEvalConditional`, each holding `FlagEvalType`, `FlagDefinition` and
    `TimeSeconds` (initialization data);
  - `FlagChainOperator`: a `CompoundExpressionEvaluator.ECExpressionOperatorType`, 0 `CEOP_Or` or 1 `CEOP_And`.
- **`Rule.EFlagEvalType`:** 0 `FLAG_IsTrue`, 1 `FLAG_IsFalse`, 2 `FLAG_HasBeenTrueForXSeconds`,
  3 `FLAG_HasBeenFalseForXSeconds`.
- **`FlagDefinition`:**
  - `EvaluationExpression` (an `ExpressionEvaluator`: a computed flag);
  - `ContextResolverChain` (attribute context resolvers);
  - `ValueResolver` (a `FlagValueResolver`).
- **`Object.Flag`** (one flag's state): `Value`, `Raised`, `Lowered` (bytes), `RaisedTime`, `LoweredTime`, `Duration`
  (floats).
- **A flag store** (an `IFlagProvider`): `GearboxMind` and `WillowPlayerController` each hold their own:
  - `DynamicFlagDefinitions` (flag definitions);
  - `DynamicFlags` (`RuleEngine.DynamicFlagInstanceData`: a `Flag` and the time it was last evaluated);
  - a native map from a definition to its index in `DynamicFlags`.

  A `RuleEngine` holds the same three fields.

## 3. What Evaluate does, in order

`Evaluate(Context)` on a `FlagExpressionEvaluator`:

1. **The context's rule engine** (`R`), read once (reads only):
   - the context, when its class implements `IRuleEngineOwner`, gives its rule engine (`GearboxMind`: its
     `MyRuleEngine`);
   - otherwise, a `GearboxPawn` context asks its `Controller` (or, with none, its `DrivenVehicle`'s `Controller`), when
     that implements `IRuleEngineOwner`;
   - if that gave none: the context itself when it is a `RuleEngine`; else the `MyRuleEngine` of a context that is a
     `Rule`; else none.
2. **The result starts true.** An empty `FlagChain` answers true.
3. **Each conditional, in order:**
   1. A None `FlagDefinition` is false.
   2. Otherwise one of two paths is chosen:
      - **the store path** when `R` exists and the definition lacks a context resolver or lacks a value resolver;
      - **the definition path** in every other case.
   3. For types 2 and 3, `TimeSeconds` is evaluated as initialization data with the context as source, before the
      path runs.
   4. **The store path:** the flag is looked up in `R`'s own flag store. A flag the store does not hold answers false,
      for both `IsTrue` and `IsFalse`.
   5. **The definition path** (section 4).
   6. **Combine** with the result so far:
      - the first conditional's answer becomes the result;
      - a later one is ORed (`CEOP_Or`) or ANDed (`CEOP_And`); any other operator makes the result false;
      - after each conditional, `CEOP_Or` with a true result, or `CEOP_And` with a false one, returns at once.
4. **Return** the result.

The evaluator itself writes nothing. What a lookup or a value resolver writes is in section 4.

## 4. A flag definition's answer (the definition path)

For a definition `F` and context `C`, `IsTrue` or `IsFalse`:

1. **The context:** `C` is passed through `F.ContextResolverChain`, each resolver in turn (its `GetAttributeContext`).
   A resolver that gives None ends the chain with None (UNVERIFIED in detail). An unresolved context answers false
   for both `IsTrue` and `IsFalse`.
2. **A computed flag** (`F.EvaluationExpression` set):
   - the expression is evaluated on the resolved context, and `IsTrue` is its answer (`IsFalse` its negation);
   - then, when `F` has a value resolver, the answer is handed to it (a write, not on link 9's path; not read);
   - no time check or expiry applies.
3. **Otherwise,** the value resolver's own answer for `IsTrue` or `IsFalse` (below).

**The two value resolver classes on link 9's path:**
- **`RuleEngineFlagValueResolver`** finds the flag store for the resolved context:
  - the context itself when it is a `RuleEngine` or answers `IFlagProvider`;
  - else, a pawn's `Controller`'s rule engine.

  It then reads the flag there.
- **`ObjectFunctionFlagValueResolver`** (`CheckRate`, `FunctionCall`):
  1. it asks the resolved context's `QueryInterface` for `IFlagProvider`. A component asks its `Owner`, and a pawn asks
     its `Controller` (NATIVE_PAWN_AI_INTERFACE.md), so for an AI pawn or its AI component the store is the mind's. No
     store gives false for both;
  2. the flag is looked up there (section 4.1 below);
  3. when the time since the flag last changed (`RaisedTime` while it is true, else `LoweredTime`) is at least
     `CheckRate`, the script function named `FunctionCall` is called on the resolved context with no arguments;
  4. its result, rounded to an integer, sets the flag (nonzero is true), as section 4.2 says.

  Then the flag is read.

### 4.1 The mind's flag store (`GearboxMind`'s `IFlagProvider`)

**A lookup of definition `F`:**
1. When the store's map has no `F`, it is added. `F` is appended to `DynamicFlagDefinitions` when absent, one zeroed
   entry is appended to `DynamicFlags` (`Value` false, every time and `Duration` 0), and the map points `F` at it.
   **A lookup can write.**
2. When `F.EvaluationExpression` is set and the entry's last-evaluated time is not the current time, the expression is
   evaluated with **the mind** as context, the flag is set from its answer, and the entry's last-evaluated time is
   set to now.
3. The entry is returned.

The current time is the level's `WorldInfo.TimeSeconds`.

### 4.2 Setting and reading a flag

- **Setting to true** from false: `Value` true, `Raised` set, `Lowered` cleared, `RaisedTime` = now, `Duration` 0.
- **Setting to false** from true: `Value` false, `Raised` cleared, `Lowered` set, `LoweredTime` = now, `Duration` 0.
- **The same value again:** `Duration` 0, and nothing else unless the setter asks to refresh. The function-call
  resolver does ask: `RaisedTime` (true) or `LoweredTime` (false) becomes now.
- **Reading** `IsTrue` (`Value` set) or `IsFalse` (`Value` clear), after a timed flag's expiry:
  - a true flag with `Duration` above 0.0001 whose time is up (now before `RaisedTime`, or now minus `RaisedTime`
    past `Duration`) is lowered first, as "setting to false" above, and `Duration` becomes 0;
  - **so a read can write.** No flag on link 9's path has a duration.
- **Types 2 and 3** (true or false for X seconds): not read past the evaluated time; not on link 9's path.

## 5. What the slice's phaselock cast passes, and the answer

**Maya's `CanLiftTargetIf`** is one conditional:
- `FLAG_IsTrue` of `Flag_Skills_CanPhaseLock`, operator `CEOP_And`;
- `Flag_Skills_CanPhaseLock` has no context resolvers and no value resolver, and its `EvaluationExpression` is a
  second `FlagExpressionEvaluator`. That second evaluator is `CEOP_And` of three `FLAG_IsFalse`:
  1. `Flag_Skills_DisablePhaseLock`: context `WillowMindAttributeContextResolver` (the pawn's mind), value
     `RuleEngineFlagValueResolver`;
  2. `Flag_Skills_PhaseLockOnHold`: context `AIComponentAttributeContextResolver`, value
     `ObjectFunctionFlagValueResolver` (`FunctionCall` "PhaseLockOnHold", `CheckRate` 0.2);
  3. `Flag_IsPlayer`: context `PawnAttributeContextResolver`, value `ObjectFunctionFlagValueResolver` (`FunctionCall`
     "IsHumanControlled", `CheckRate` 1.0).

**The context** is the cast's target, a `WillowAIPawn` whose `Controller` is its mind.

**The path, in game (UNVERIFIED):**
- **Step 1, `R`:** the pawn's mind's `MyRuleEngine`.
  - With a rule engine, `CanPhaseLock` takes the store path: it is read from the rule engine's own flag store, which
    was not read.
  - With none, it takes the definition path: the second evaluator is evaluated on the pawn.
- **`DisablePhaseLock`:** the mind's store, entry created zeroed on the first lookup. `IsFalse` is true unless
  something set the flag.
- **`PhaseLockOnHold`:** the AI component's query reaches the mind's store, which creates the entry on first use. Once
  0.2 s of world time have passed since its last change, `WillowAIComponent.PhaseLockOnHold()` is called on the
  component (NATIVE_AI_AWARENESS.md) and sets the flag.
- **`IsPlayer`:** the pawn's query reaches the mind's store. `Pawn.IsHumanControlled()` is called when 1.0 s has
  passed, and sets the flag.
- **The answer:** true when the target's mind has no `DisablePhaseLock`, it is not on hold and it is not human
  controlled. So an ordinary enemy can be phaselocked.
- **The writes:** up to three entries in the mind's store, each flag set from its function, and their times.

**A clock subtlety.** A fresh entry's times are 0. At `TimeSeconds` below 0.2, or below 1.0 for `IsPlayer`, the
function is not called and the flag keeps false. That gives the same answer for an ordinary enemy.

## 6. Proposed ParityMode Off answer (for a separate implementer)

**Classes:**
- bind `FlagExpressionEvaluator.Evaluate(object)`, Off only;
- Reference keeps the stub policy;
- the receiver must be a `GearboxFramework.FlagExpressionEvaluator`.

**Preflight** (write-free, non-loading reads only; a load during execution counts as a write):
1. **The receiver.**
   - No receiver: `VmError::Script`, "FlagExpressionEvaluator.Evaluate: called with no evaluator".
   - The wrong class: `VmError::Script`, "<class path> is not a FlagExpressionEvaluator".
   - `FlagChain` not an array of `FlagEvalConditional`, or `FlagChainOperator` not a byte: `VmError::Script`, naming
     the field.
   - An unloaded receiver: `NotYet { NotRead }`, "FlagExpressionEvaluator.Evaluate: the unloaded evaluator <object
     path>".
2. **Each conditional's type.** Types 2 and 3: `NotYet { NotRead }`, "FlagExpressionEvaluator.Evaluate: the timed flag
   tests (HasBeenTrueForXSeconds, HasBeenFalseForXSeconds)".
3. **`R`, by section 3 step 1.** It needs only reads, through the existing `QueryInterface` and the controller field.
   - A non-None `R` is `NotYet { NotRead }`, "FlagExpressionEvaluator.Evaluate: a rule engine's own flag store", for a
     conditional that would take the store path.
   - None (the port's minds have no `MyRuleEngine`) means the definition path for every conditional.
4. **Each definition reached:**
   - A value resolver class other than the two read: `NotYet { NotRead }`, "FlagDefinition: the value resolver class
     <class path> is not read".
   - A context resolver the port's attribute system does not bind: that system's own refusal, unchanged.
   - A store the context answers that is not a `GearboxMind`'s (`WillowPlayerController`'s, say): `NotYet { NotRead }`,
     "FlagDefinition: the flag store of a <class path> is not read".

**Execution, in section 3's order:**
- the definition path of section 4, with the mind's store of section 4.1;
- the function-call resolver calls the named function through the VM, with no arguments; its answer, rounded,
  sets the flag;
- the combine and early-exit rules exactly.

**Writes, in order** (each signalled as a write for the caller's partial-writes mark):
1. per lookup, the store's new definition, entry and map slot;
2. a computed flag's re-evaluation and time stamp;
3. a function call's flag update.

The "stopped after partial writes: " prefix is the caller's job, after a real write.

**The map:** the port may keep it as the index of the definition in `DynamicFlagDefinitions`, which the add keeps in
step with `DynamicFlags`. That is an assumption to check (open question 4).

**The slice's expected answer** (UNVERIFIED): true for the cast's enemy, with three entries in its mind's store.

## Open questions

1. **UNVERIFIED:** a rule engine's own flag store (the store path when `MyRuleEngine` is set) was not read.
2. **UNVERIFIED:** the timed tests (types 2 and 3) beyond the evaluated time.
3. **UNVERIFIED:** the context resolver chain's handling of a None result in the middle of the chain.
4. **UNVERIFIED:** whether `DynamicFlagDefinitions` and `DynamicFlags` always stay index-aligned (the native map is
   authoritative).
5. **UNVERIFIED:** how the function-call resolver converts a non-numeric return (it rounds the call's numeric value).
6. **UNVERIFIED:** whether the port's cast target has a `MyRuleEngine` in game; the port's minds are made without one.

## Live check that would confirm the rule (bl-sdk, the RTSE way)

Phaselock an ordinary enemy, then read its mind's `DynamicFlagDefinitions` and `DynamicFlags`. Expect entries for
`Flag_Skills_DisablePhaseLock` (false), `Flag_Skills_PhaseLockOnHold` (false) and `Flag_IsPlayer` (false), and
`MyRuleEngine` either None or a rule engine whose store holds `Flag_Skills_CanPhaseLock`.
