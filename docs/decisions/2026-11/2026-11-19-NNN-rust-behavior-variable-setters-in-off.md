## 2026-11-19: The behavior-variable setters in Off

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-SET-OBJECT-BEHAVIOR-VARIABLE, slice link 9. Coded from
docs/verification/NATIVE_SET_BEHAVIOR_VARIABLE.md alone, the note's final text (94aa293, PR #538; clean room: lane
ANALYST-SET-OBJECT-BEHAVIOR-VARIABLE wrote it), with the hub's rulings on its review r1. The executable and the
analysis output were not opened. Synthetic fixtures only.

- **What is bound (`ow-behavior` `kernel/variables.rs`, Off only, first-wins through the kernel pack).** The five
  static natives of `GearboxFramework.BehaviorProviderDefinition`: `SetBoolBehaviorVariable`,
  `SetIntBehaviorVariable`, `SetFloatBehaviorVariable`, `SetVectorBehaviorVariable` and `SetObjectBehaviorVariable`.
  Each writes two fields of the caller's `BehaviorVariableValue` (argument 0, by reference) and nothing else:
  - Bool: `IntValue` = 1 for true, 0 for false, and `VariableType` = `BVAR_Bool`;
  - Int: `IntValue`, `BVAR_Int`;
  - Float: `FloatValue`, `BVAR_Float`;
  - Vector: `VectorValue`, `BVAR_Vector`;
  - Object: `ObjectValue`, None included with no check, `BVAR_Object`.

  There is no lookup, no name and no other storage. Under Reference nothing changes.
- **Script callers.** Only `SetObjectBehaviorVariable` has script callers in the installed packages
  (`LiftActionSkill.OnSelectedTarget`, which `PhaseLockTarget` calls as a final function, and the other events the
  note lists). The Bool, Int, Float and Vector setters have none; their rows rest on the note's reading of the
  executable alone.
- **Preflight (write-free, no loads).**
  - Argument 0 is declared `out` as a `BehaviorProviderDefinition.BehaviorVariableValue`. That struct declares the
    setter's value field, of its kind, and `VariableType`, a byte of `EBehaviorVariableType`.
  - The `BVAR_*` value is found **by name** in the loaded enum, never by position. The test enum is ordered unlike the
    game's on purpose.
  - Argument 0 holds None or that struct. **None is the zero struct** (hub ruling on #538 r1): link 9's caller grows
    a function-local array, and the VM fills a local array untyped, so the new elements reach the setter as None.
  - Anything else in argument 0 is `VmError::Script`, "BehaviorProviderDefinition.Set<Kind>BehaviorVariable: <what it
    holds>, not a BehaviorVariableValue". This is the port's own data error, not a `NotYet`: in the game the argument
    is always the typed struct.
  - Argument 1 must be the setter's kind: a bool, an int, a float, the struct's own vector type, or an object (None
    included). A wrong kind, or a missing field, enum or enum name, is `VmError::Script` too.
- **The write** is one, the last step: the struct with the two fields replaced, through the out argument. No refusal
  can follow it, so the setter never needs the "stopped after partial writes" prefix.
- **Tests (`kernel/variables_tests.rs`).** An invented `GearboxFramework` package with the struct (with one field no
  setter writes), a shuffled enum, the five natives and a script caller. They cover:
  - each setter, true and false for Bool, with every other field unchanged;
  - a None object;
  - a None variable, which becomes the zero struct;
  - the script caller: it grows a local array to 2 and fills both new elements through `DynArrayElement` out
    arguments, with `Self` and `None`, as link 9 does;
  - argument 0 that is neither None nor the struct, argument 1 of the wrong kind, and a malformed declaration. Each
    is a Script error, and the heap's committed writes, the RNG draws and the definition load transitions do not
    move;
  - a second registration, which binds nothing new, and Reference, which binds none of the five.
- **Link 9.** With #523 and #535, this removes the `SetObjectBehaviorVariable` stop. The expected next stop is
  `ActivateBehaviorEventFromScript` with `Parameters` (the note's section 4, open question 1).
- **UNVERIFIED.** Every rule is unconfirmed in the running game, the bool's stored value for true included. The note
  gives the live check.
