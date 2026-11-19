## 2026-11-19: The behavior-variable setters, read as analyst

AI-assisted (Claude Code, Claude Opus 5.5). Lane ANALYST-SET-OBJECT-BEHAVIOR-VARIABLE, slice link 9. Analyst note only
(clean room): docs/verification/NATIVE_SET_BEHAVIOR_VARIABLE.md. It is written in own words from the installed script,
the package layouts and local, read-only Ghidra queries under the machine lock. Raw output stays under ignored
`local/analysis/sobv/`. No live session was run.

- **Read:**
  - The five `BehaviorProviderDefinition.Set<Kind>BehaviorVariable` natives write the caller's own
    `BehaviorVariableValue`, passed by reference: one value field (`IntValue` for bool and int, `FloatValue`,
    `VectorValue` or `ObjectValue`), then `VariableType` (`BVAR_Bool` 1 to `BVAR_Object` 5 in the installed enum).
    Nothing else is written. There is no lookup and no None check.
  - On link 9 they run in `LiftActionSkill.OnSelectedTarget`, which fills a two-entry `Parameters` array (the caster,
    the target) and fires the `OnSelectedTarget` behavior event through `BehaviorKernel.ActivateBehaviorEventFromScript`.
- **For the implementer:** bind the five setters in Off, with a write-free type preflight and one write through the
  out argument. They have no refusal of their own.
- **Link 9's expected next stop:** the port's existing `NotRead` for `ActivateBehaviorEventFromScript` with
  `Parameters`. How the kernel passes them to the event is the next question.
- **Review r1 of #538:**
  - argument 0 holding None is taken as the zero `BehaviorVariableValue` and then set. `Parameters` is a local array,
    which the port's VM (and the C++ reference) grows untyped, so link 9's two elements are None on the first call.
    Only a value that is neither None nor that struct is refused, and that refusal is the port's own;
  - only `SetObjectBehaviorVariable` has script callers (34 sites in 21 functions). The Bool, Int, Float and Vector
    rows rest on the executable alone;
  - `OnSelectedTarget` is a final function called directly by `PhaseLockTarget` (with `ConsumerHandle`,
    `MyWillowPawn`, `NewTarget`), not an event; its declared parameters are `TheConsumeHandle`, `SkillInstigator`,
    `SkillTarget`;
  - `SelectTarget` also needs the target's `DrivenVehicle` to be None, and a None `CanLiftTargetIf` passes.
- **UNVERIFIED.** Every rule is unconfirmed in the running game; the note lists the live check.
