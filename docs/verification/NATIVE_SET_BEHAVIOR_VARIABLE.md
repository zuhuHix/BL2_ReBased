# The behavior-variable setters of BehaviorProviderDefinition (draft, 2026-11-19)

AI-assisted (Claude Code, Opus 5.5). Analyst lane ANALYST-SET-OBJECT-BEHAVIOR-VARIABLE (decision 2026-11-19-149, a
placeholder), for slice link 9. **Analyst only:** a different lane implements it (clean room).

**Sources:**
- the installed script, read with `research/script_disasm.py` and a local scan of every script function for the
  name;
- the struct and enum layouts, from the cooked packages through the project's layout tool and a local read of the
  enum's value names;
- `Borderlands2.exe`, read locally with the project's Ghidra tooling (read-only queries, one headless run at a time
  under the machine lock).

There is no bl-sdk session and no recorded live evidence on these natives. Raw output stays under ignored
`local/analysis/sobv/`. This note is in our own words: no decompiler output, pseudo-code, addresses or listing
structure. **Every rule here is UNVERIFIED** in the running game.

**Why it matters.** With #535 (the flag-expression rule, on #523), link 9's `cast` gets past
`FlagExpressionEvaluator.Evaluate` and stops at the unbound native
`BehaviorProviderDefinition.SetObjectBehaviorVariable(behaviorvariablevalue,object)`.

## 1. The callers on link 9's path

- **`LiftActionSkill.SelectTarget(NewTarget)`:** when `CanPhaseLockTarget` passes, the target's `DrivenVehicle` is
  None, and `CanLiftTargetIf` passes (a None `CanLiftTargetIf` counts as a pass), it calls `PhaseLockTarget(NewTarget)`.
- **`LiftActionSkill.PhaseLockTarget(NewTarget)`** calls `OnSelectedTarget` directly, as a final function, with the
  skill's instance `ConsumerHandle`, its `MyWillowPawn` and `NewTarget`.
- **`LiftActionSkill.OnSelectedTarget(TheConsumeHandle, SkillInstigator, SkillTarget)`** (a final function, not an
  event; it has a local array `Parameters` of `BehaviorVariableValue`, and its body uses the instance `ConsumerHandle`,
  not its first parameter):
  1. sets `Parameters` to length 2;
  2. calls the static `BehaviorProviderDefinition.SetObjectBehaviorVariable(Parameters[0], SkillInstigator)`;
  3. then `SetObjectBehaviorVariable(Parameters[1], SkillTarget)`;
  4. then `BehaviorKernel.ActivateBehaviorEventFromScript(ConsumerHandle, GetBehaviorProviderDefinition(),
     GetFuncName(), <filter omitted>, Parameters)`: the event named after the function (`OnSelectedTarget`), fired on
     the skill's behavior provider with the two values.
- **Other callers** (not on link 9's path): ten other `LiftActionSkill` functions (`OnBeginLiftingTarget`,
  `OnTargetBlocked`, `OnTargetBecomesLocked`, `OnTargetIsAboutToBecomeUnlocked`, `OnReleasedTarget`,
  `OnKilledTarget`, `OnHealedTarget`, `OnLiftFailed`, `OnRuinImpact`, `OnCharmTarget`), the `ExecuteActionSkill`
  events and two `ShieldDefinition` events. All of them follow the same shape: fill a local `Parameters` array, then
  fire an event with it.
- **Only `SetObjectBehaviorVariable` has script callers.** In Core, Engine, GameFramework, GearboxFramework and
  WillowGame, all 34 call sites of the five setters (in 21 functions) are `SetObjectBehaviorVariable`. The Bool, Int,
  Float and Vector setters have none, so their rows in section 3 rest on the executable alone.

## 2. The shapes (from the packages)

- **`BehaviorProviderDefinition.BehaviorVariableValue`:** `IntValue` (int), `FloatValue` (float), `VectorValue`
  (`Object.Vector`), `ObjectValue` (object), `VariableType` (a byte of `EBehaviorVariableType`).
- **`EBehaviorVariableType`**, in the installed order: 0 `BVAR_None`, 1 `BVAR_Bool`, 2 `BVAR_Int`, 3 `BVAR_Float`,
  4 `BVAR_Vector`, 5 `BVAR_Object`, then `BVAR_AllPlayers`, `BVAR_Attribute`, `BVAR_InstanceData`,
  `BVAR_NamedVariable`, `BVAR_NamedKismetVariable`, `BVAR_DirectionVector`, `BVAR_AttachmentLocation`,
  `BVAR_UnaryMath`, `BVAR_BinaryMath`, `BVAR_Flag`, `BVAR_MAX`. (The port's behavior fixtures order their toy enum
  differently on purpose: read the value by name.)

## 3. What the setters do

There are five, one native each on `BehaviorProviderDefinition`, called statically:
`SetBoolBehaviorVariable`, `SetIntBehaviorVariable`, `SetFloatBehaviorVariable`, `SetVectorBehaviorVariable` and
`SetObjectBehaviorVariable`. Each takes:
- **the variable, by reference:** the caller's own `BehaviorVariableValue` (an out parameter; on link 9's path an
  element of the caller's local `Parameters` array). The native writes into it;
- **the value.**

Each one writes exactly two fields of that struct, in this order, and leaves every other field as it was:

| Setter | Value field written | `VariableType` written |
|---|---|---|
| `SetBoolBehaviorVariable` (no script caller) | `IntValue` = the bool (as the engine's 0 or 1) | `BVAR_Bool` (1) |
| `SetIntBehaviorVariable` (no script caller) | `IntValue` = the int | `BVAR_Int` (2) |
| `SetFloatBehaviorVariable` (no script caller) | `FloatValue` = the float | `BVAR_Float` (3) |
| `SetVectorBehaviorVariable` (no script caller) | `VectorValue` = the vector (X, Y, Z) | `BVAR_Vector` (4) |
| `SetObjectBehaviorVariable` | `ObjectValue` = the object | `BVAR_Object` (5) |

Answers to the hub's questions:
- **Which storage:** none of the provider's, the consumer's or the kernel's. The only thing written is the caller's
  struct. The kernel sees the values only when the caller passes the array on (section 1, step 4).
- **How the value struct is matched:** it is not looked up. The struct is the one the caller passes by reference.
- **A None object:** no check. `ObjectValue` becomes None and `VariableType` still becomes `BVAR_Object`.
- **A missing variable:** not a case. There is no name or index, so there is nothing to miss.
- **Return value:** none. No event, no log line, nothing else written.

## 4. What reads the values afterwards

`BehaviorKernel.ActivateBehaviorEventFromScript(..., Parameters)` takes the array (section 1, step 4). How the kernel
hands `Parameters` to the fired event (which event outputs they bind to, by position or by type) **was not read in
this lane**. The port's binding refuses it today: `ow_behavior::kernel::natives` answers a call with `Parameters` as
`NotYet { NotRead }`, "ActivateBehaviorEventFromScript with Parameters (how they reach the event is not read)". So
once the setters are bound, that is link 9's expected next stop (open question 1).

## 5. Proposed ParityMode Off answer (for a separate implementer)

**Bind** the five setters in Off, first-wins, under their package-qualified paths
(`GearboxFramework.BehaviorProviderDefinition.Set<Kind>BehaviorVariable`). Reference keeps its stub policy.

**Write-free preflight** (no loads; nothing is a definition handle here):
1. Argument 0 must be an out argument. **Holding None, it is taken as the zero `BehaviorVariableValue`**, and the two
   fields are then set on that zero struct (`classes.zero_value(&PropKind::Struct(..))`; `natives_core`'s
   `make_struct` is private and takes exactly three fields). This is link 9's own case: `Parameters` is a function-local array, and the port's
   VM grows a local array untyped (`Fill::Untyped` in `ow-vm` `interp::aggregate`, whose fill is None for an empty
   array), so after `Parameters.Length = 2` both elements are None. The C++ reference fills a local array the same
   way. A value that is neither None nor that struct is `VmError::Script`,
   "BehaviorProviderDefinition.Set<Kind>BehaviorVariable: <what it holds>, not a BehaviorVariableValue". **That
   check is the port's own data error, not a refusal:** the native checks nothing, since in the game the argument is always the typed struct.
2. Argument 1 must be the setter's kind: a bool, an int, a float, an `Object.Vector` struct, or an object reference
   (None included). Anything else is `VmError::Script`, naming the argument.
3. The enum value is found **by name** (`BVAR_Bool` ... `BVAR_Object`) in the loaded
   `BehaviorProviderDefinition.EBehaviorVariableType`. A missing enum or name is `VmError::Script`.

**The write** (one, the last step): a copy of the struct with the value field and `VariableType` replaced, written
back through the out argument. No other field changes.

**Refusals:** none of the setters' own. There is no `NotYet` here: the rule is fully read. Since the write is the
setter's last step, no refusal can follow it. The prefix question (#518's counters) does not arise inside the setter.
A caller that refuses after it (the event fire) is marked by that caller.

**Bool encoding:** the engine's bools are 0 or 1 when they reach a native from script. Write 1 for true and 0 for
false (UNVERIFIED for a nonzero value other than 1, which script cannot make).

## Open questions

1. **How `ActivateBehaviorEventFromScript` passes `Parameters` to the fired event**: the next read on link 9's path.
2. **UNVERIFIED:** the bool setter's exact stored value for true (expected 1).

## Live check that would confirm the rule (bl-sdk, the RTSE way)

Hook `LiftActionSkill.OnSelectedTarget` during a Phaselock. After the two setter calls, read the local `Parameters`:
expect two entries with `VariableType` 5, `ObjectValue` the caster and the target, and `IntValue`, `FloatValue` and
`VectorValue` unchanged from the array's new (zeroed) elements.
