# Native rules of FlagExpressionEvaluator.Evaluate (draft, 2026-11-19)

AI-assisted (Claude Code, Opus 5.5), analyst lane ANALYST-FLAG-EXPRESSION-EVALUATE (decision 2026-11-19-138, number
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
   - A None context gives none (each step needs an object; UNVERIFIED for this edge).
2. **The result starts true.** An empty `FlagChain` answers true.
3. **Each conditional, in order:**
   1. A None `FlagDefinition` is false.
   2. Otherwise one of two paths is chosen:
      - **the store path** when `R` exists and the definition has no context resolver, or has no value resolver;
      - **the definition path** in every other case: `R` is none, or the definition has both a context resolver and a
        value resolver.
   3. For types 2 and 3, `TimeSeconds` (initialization data) is evaluated with the context as source, before the path
      runs, giving the threshold `X`.
   4. **The store path** (section 4.3).
   5. **The definition path** (section 4).
   6. **Combine** with the result so far:
      - the first conditional's answer becomes the result;
      - a later one is ORed (`CEOP_Or`) or ANDed (`CEOP_And`); any other operator makes the result false;
      - after each conditional, `CEOP_Or` with a true result, or `CEOP_And` with a false one, returns at once, so the
        later conditionals are not reached.
4. **Return** the result.

The evaluator itself writes nothing. Lookups, value resolvers, flag expiry and computed flags can write (section 4).

## 4. A flag definition's answer

### 4.0 The definition path, case by case

For a definition `F`, the input context `C` and the test asked:

1. **`C` is None:** `IsTrue`, `IsFalse` and both timed tests answer false. Nothing else runs.
2. **The context chain.** `C` is passed through `F.ContextResolverChain`, in order:
   - each resolver is asked for its context (`GetAttributeContext`) from the context so far;
   - a None entry in the chain is skipped;
   - a resolver that answers None ends the chain, and **the context so far stays** (it is not replaced by None);
   - an empty chain leaves `C` as it is.

   So the resolved context `C'` is never None when `C` is not.
3. **`IsTrue`:**
   - with `F.EvaluationExpression` set: the expression evaluated on `C'` (it runs this same native, or another
     evaluator class's). Then, when `F` also has a value resolver, the answer is handed to that resolver (a write; its
     rule is not read);
   - else, with a value resolver: that resolver's `IsTrue` on `C'`;
   - else (neither): **false**.
4. **`IsFalse`:**
   - with `F.EvaluationExpression` set: the negation of the expression's answer on `C'`, then the same hand-off as
     `IsTrue`;
   - else, with a value resolver: that resolver's `IsFalse` on `C'`;
   - else (neither): **not defined.** The native calls through the missing value resolver. UNVERIFIED, and an Off
     refusal (section 6).
5. **Types 2 and 3** (`HasBeenTrueForXSeconds`, `HasBeenFalseForXSeconds`):
   - no value resolver: false;
   - with one: when `F.EvaluationExpression` is set it is evaluated on `C'` first and handed to the resolver; then the
     resolver's own timed test runs with `X`. **The value resolvers' timed tests were not read** (section 6 refuses
     them).

### 4.1 The two value resolver classes on link 9's path

- **`RuleEngineFlagValueResolver`** finds the flag store for `C'`:
  - `C'` itself when it is a `RuleEngine` or answers `IFlagProvider`;
  - else, a pawn's `Controller`'s rule engine.

  It then reads the flag there. No store: false for both `IsTrue` and `IsFalse`.
- **`ObjectFunctionFlagValueResolver`** (`CheckRate`, `FunctionCall`):
  1. it asks `C'`'s `QueryInterface` for `IFlagProvider`. A component asks its `Owner`, and a pawn asks its
     `Controller` (NATIVE_PAWN_AI_INTERFACE.md), so for an AI pawn or its AI component the store is the mind's. No
     store: false for both;
  2. the flag is looked up there (section 4.2);
  3. when the time since the flag last changed (`RaisedTime` while it is true, else `LoweredTime`) is at least
     `CheckRate`, the script function named `FunctionCall` is called on `C'` with no arguments;
  4. its result, rounded to an integer, sets the flag (nonzero is true) as section 4.4 says, with the refresh.

  Then the flag is read (section 4.4).

### 4.2 The mind's flag store (`GearboxMind`'s `IFlagProvider`)

**A lookup of definition `F`:**
1. When the store's map has no `F`, it is added. `F` is appended to `DynamicFlagDefinitions` when absent, one zeroed
   entry is appended to `DynamicFlags` (`Value` false, every time and `Duration` 0), and the map points `F` at it.
   **A lookup can write.**
2. When `F.EvaluationExpression` is set and the entry's last-evaluated time is not the current time, the expression is
   evaluated with **the mind** as context, the flag is set from its answer (section 4.4), and the entry's
   last-evaluated time is set to now.
3. The entry is returned.

### 4.3 The store path (`R`'s own flag store)

- `R`'s flag store is asked for `F`'s entry. **That lookup was not read for a `RuleEngine`**: whether it creates a
  missing entry (as the mind's does) or re-evaluates a computed flag is unknown.
- When the lookup gives no entry, every test answers false (`IsTrue` and `IsFalse` alike). That part is the
  evaluator's own and was read.
- When it gives an entry, `IsTrue` and `IsFalse` read it as section 4.4 says. `HasBeenTrueForXSeconds` is true when
  `X` is at most the time the flag has been true (section 4.4). `HasBeenFalseForXSeconds` was not read.

### 4.4 Setting and reading a flag entry

**The clock.** Every flag-entry operation below reads the current world's `WorldInfo.TimeSeconds` (one engine routine;
the field named by the layout tool, high confidence, UNVERIFIED).

- **Setting to true** from false: `Value` true, `Raised` set, `Lowered` cleared, `RaisedTime` = now, `Duration` 0.
- **Setting to false** from true: `Value` false, `Raised` cleared, `Lowered` set, `LoweredTime` = now, `Duration` 0.
- **The same value again:** `Duration` 0, and nothing else unless the setter asks to refresh. The function-call
  resolver does ask: `RaisedTime` (true) or `LoweredTime` (false) becomes now.
- **Expiry, first in every read:**
  - a true flag with `Duration` above 0.0001 whose time is up (now before `RaisedTime`, or now minus `RaisedTime`
    above `Duration`) is lowered, as "setting to false" above, and `Duration` becomes 0;
  - **so a read can write.** No flag on link 9's path has a duration.
- **`IsTrue`** is `Value` after the expiry; **`IsFalse`** is its negation.
- **The time held true:** after the expiry, when `Value` is true and now is after `RaisedTime`, it is now minus
  `RaisedTime`; else 0. `HasBeenTrueForXSeconds` on an entry is true when `X` is at most that time.
- **The time held false** (for `HasBeenFalseForXSeconds`): not read.

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

**The context** is the cast's target, a `WillowAIPawn` whose `Controller` is its mind. Step 1 asks the mind for
`MyRuleEngine`, and the answer splits the path (in game, UNVERIFIED):

- **Branch S, the mind has a `MyRuleEngine`:** `CanPhaseLock` has no context resolver, so it takes the store path: it is
  read from that rule engine's own store (section 4.3). That lookup was not read, so the answer is not predicted.
  Nothing about the mind's store follows from this branch.
- **Branch D, the mind has none** (the port's minds are made without one): `CanPhaseLock` takes the definition path,
  and its expression is evaluated on the pawn. The three inner definitions each have both a context resolver and a
  value resolver, so they take the definition path in either branch:
  1. **`DisablePhaseLock`:** the mind's store, entry created zeroed on the first lookup. `IsFalse` is true unless
     something set the flag.
  2. **`PhaseLockOnHold`:** the AI component's query reaches the mind's store, which creates the entry on first use.
     Once 0.2 s of world time have passed since its last change, `WillowAIComponent.PhaseLockOnHold()` is called on the
     component (NATIVE_AI_AWARENESS.md) and sets the flag.
  3. **`IsPlayer`:** the pawn's query reaches the mind's store. `Pawn.IsHumanControlled()` is called when 1.0 s has
     passed, and sets the flag.
  - **The answer:** true when the target's mind has no `DisablePhaseLock`, it is not on hold and it is not human
    controlled. So an ordinary enemy can be phaselocked.
  - **The writes:** up to three entries in the mind's store, each flag set from its function, and their times.
  - **A clock subtlety.** A fresh entry's times are 0. At `TimeSeconds` below 0.2, or below 1.0 for `IsPlayer`, the
    function is not called and the flag keeps false. That gives the same answer for an ordinary enemy.
  - CanPhaseLock has no value resolver, so nothing is handed off after its expression.

## 6. Proposed ParityMode Off answer (for a separate implementer)

**Classes:**
- bind `FlagExpressionEvaluator.Evaluate(object)`, Off only;
- Reference keeps the stub policy.

**Errors and kinds.** These follow `ow-attr`'s split in `eval::ensure_loaded`:
- **`NotYet { LayerAbsent }`:** a layer the port does not have, such as a reached handle that needs loading in a world
  with no definition store ("the definition store", `ow-defs`).
- **`NotYet { NotRead }`:** a rule of the game not read, or a reached handle the store leaves a stand-in it does not
  load.
- **`NotYet { NotPorted }`:** an operation that was read but is not ported yet. None is planned here.
- **`VmError::Script`:** malformed data (the shapes of section 2).

**Loads are writes.** Any reached handle may be loaded when it is first used:
- the receiver;
- a conditional's `FlagDefinition`;
- a definition's `EvaluationExpression`, context resolvers and value resolver;
- the context resolvers' answers.

Each load counts as a write for the partial-writes mark, as the hub ruled for the attribute system (review r2 of #523).

**Eager checks, before anything runs** (reads only, and no loads besides the receiver's own):
1. **The receiver.**
   - No receiver: `VmError::Script`, "FlagExpressionEvaluator.Evaluate: called with no evaluator".
   - The wrong class: `VmError::Script`, "<class path> is not a FlagExpressionEvaluator".
2. **`FlagChain`** must be an array of `FlagEvalConditional`, and **`FlagChainOperator`** a byte; otherwise
   `VmError::Script`, naming the field.

Nothing else is checked up front: a conditional the OR or AND never reaches is never checked.

**Per conditional, when it is reached** (in a nested expression too, each at its own turn). Earlier conditionals'
writes stay; a refusal after any write (a store entry, a flag update, an expiry, a load) is marked "stopped after
partial writes" by the caller.
1. **A None `FlagDefinition`** answers false (section 3, step 3.1), before the type check.
2. **Its type.** Types 2 and 3: `NotYet { NotRead }`, "FlagExpressionEvaluator.Evaluate: the timed flag tests
   (HasBeenTrueForXSeconds, HasBeenFalseForXSeconds)". The value resolvers' timed tests and the held-false time were
   not read.
3. **The path, per section 3 step 3.2:**
   - **The store path:** `NotYet { NotRead }`, "FlagExpressionEvaluator.Evaluate: a rule engine's own flag store". A
     non-None `R` is found with reads only, through the existing `QueryInterface` and the controller field.
   - **The definition path:** per section 4.0, with these refusals:
     - `IsFalse` on a definition with neither an expression nor a value resolver: `NotYet { NotRead }`, "FlagDefinition:
       IsFalse with neither an EvaluationExpression nor a ValueResolver";
     - a value resolver class other than the two read: `NotYet { NotRead }`, "FlagDefinition: the value resolver class
       <class path> is not read";
     - a computed flag that also has a value resolver: `NotYet { NotRead }`, "FlagDefinition: handing a computed answer
       to its value resolver" (the hand-off was not read);
     - a store the resolved context answers that is not a `GearboxMind`'s (`WillowPlayerController`'s, say):
       `NotYet { NotRead }`, "FlagDefinition: the flag store of a <class path> is not read";
     - a context resolver the attribute system does not bind: that system's own refusal, unchanged.
4. **The function-call resolver** calls the named function through the VM, with no arguments; its answer, rounded,
   sets the flag. The function's own refusal propagates unchanged.

**Writes, in order** (each counted for the caller's partial-writes mark):
1. per lookup, the mind store's new definition, entry and map slot;
2. a computed flag's re-evaluation, flag update and time stamp;
3. a read's expiry;
4. a function call's flag update and refresh;
5. any load of a reached handle.

The "stopped after partial writes: " prefix is the caller's job, after a real write.

**The map:** the port may keep it as the index of the definition in `DynamicFlagDefinitions`, which the add keeps in
step with `DynamicFlags`. That is an assumption to check (open question 4).

**The slice's expected answer** (UNVERIFIED):
- **Branch D** (the port's minds have no `MyRuleEngine`): true for the cast's enemy, with up to three entries in its
  mind's store.
- **Branch S:** the store path's `NotYet { NotRead }`.

## Open questions

1. **UNVERIFIED:** a rule engine's own flag store (the store path when `MyRuleEngine` is set) was not read.
2. **UNVERIFIED:** the timed tests: the value resolvers' timed tests and the time held false.
3. **UNVERIFIED:** `IsFalse` on a definition with neither an expression nor a value resolver (the native calls through
   the missing resolver), and the hand-off of a computed answer to a value resolver.
4. **UNVERIFIED:** whether `DynamicFlagDefinitions` and `DynamicFlags` always stay index-aligned (the native map is
   authoritative).
5. **UNVERIFIED:** how the function-call resolver converts a non-numeric return (it rounds the call's numeric value).
6. **UNVERIFIED:** whether the cast's target in game has a `MyRuleEngine`, which picks branch S or D.
7. **UNVERIFIED:** the rule-engine lookup with a None context.

## Live check that would confirm the rule (bl-sdk, the RTSE way)

Phaselock an ordinary enemy. First record whether its mind's `MyRuleEngine` is set.
- **None (branch D):** read the mind's `DynamicFlagDefinitions` and `DynamicFlags`. Expect entries for
  `Flag_Skills_DisablePhaseLock`, `Flag_Skills_PhaseLockOnHold` and `Flag_IsPlayer`, all false.
- **Set (branch S):** read that rule engine's `DynamicFlagDefinitions` and `DynamicFlags`, and look for
  `Flag_Skills_CanPhaseLock`'s entry and its value.
