## 2026-11-19: Link 9's evidence is the Phaselock skill's lift writes

AI-assisted (Claude Code). Lane LINK9-EVIDENCE-CHECK, slice harness. **This is the harness's evidence choice,
UNVERIFIED against the game.** No dependency or crate edge was added, and Reference is unchanged (the gate reads
only).

- **The check** (`ow-cli` `slice_gate.rs`, `phaselock_evident`): after a cast that returned with no stop, the
  Phaselock skill (`GD_Siren_Skills.Phaselock.ActionSkill_Phaselock`, the object the cast runs on) must hold the
  cast's target as `LiftedPawn`, and its `CurrentState` must be `EPLS_Intro`, read by name in the enum the property
  declares (`LiftActionSkill.EPhaselockState`). These are the first writes `LiftActionSkill.PhaseLockTarget` makes, in
  script, before it calls `OnSelectedTarget` (installed script, `research/script_disasm.py`). A cast that is blocked,
  fizzles or picks no target leaves them unset. The check is something the port's run wrote, not the driver's step.
- **Link 9** moves from `Evidence::NotChosen` to `Evidence::Driver`; the unused `NotChosen` variant is removed.
- **Tests:** a test on invented classes (a toy enum that puts `EPLS_Intro` at another position, so a pass proves the
  read by name; unset fields, another target, another state, no target and an enum-less property are no evidence; the
  check reads only). The grader test now checks both outcomes.
- **The slice** (installed, Off, strict, a local merge never pushed: this branch with #564 @ 5eb7e5e8, #535,
  #548, #555, #560, #523 and the link 9 spawn branches, as the GetFuncName probe): 1 ok / 3 NotYet / 10 errors.
  - Link 9 passes `spawn` and `cast` but is **not ok**: "no stop, but no evidence". After the cast the skill's
    `LiftedPawn` is None and `CurrentState` is 0.
  - **Why** (a full VM trace on the probe, corrected after lane FIX-ISPHASELOCKED-UNLOCKED; the first reading named
    `IsPhaselocked`, wrongly): `CanPhaseLockTarget` answers **true**, and `IsPhaselocked`'s attribute
    (`GD_Siren_Skills.Attributes.IsPhaselocked`, a noun with default 0) reads 0 on the target. `SelectTarget`'s second
    test fails: `CanLiftTargetIf` (`FlagExpressionEvaluator.Evaluate` of `Flag_Skills_CanPhaseLock`) answers false,
    so `TargetBlocked` and `OnTargetBlocked` run, not `PhaseLockTarget`. The cast's target is a `WillowAIPawn` the
    world spawned with **no `Controller`**, so it has no mind, and every inner flag of `CanPhaseLock` resolves through
    the mind or its AI component. NATIVE_FLAG_EXPRESSION_EVALUATE.md expects true for an enemy with a mind (its
    branch D). The `GetFuncName` stop of the earlier probes was inside `OnTargetBlocked`.
  - So the evidence check is doing its job. The next fix is the cast's target: a harness input giving it a mind (a
    separate lane), not the attribute.
- **Review r1 of #568:**
  - `tools/check_slice_run_off.py` accepts an ok link 9 only from `phaselock-cast` with exactly the steps `spawn` and
    `cast` (it rejected every ok `phaselock-cast` row before); its test is replaced, the other links' checks unchanged;
  - the cast returns the exact object it ran on, and `drive_phaselock` asks the evidence, read-only, about that
    receiver only after a cast that returned `Ok` (no second lookup, no log drain; never after a cast error, also
    under past-errors grading); a driver-level test catches a dropped assignment or a wrong receiver;
  - `EPLS_Intro` is found with `NameTable::find` and `EnumLayout::index_of`, as the decoder reads enum values; a
    mixed-case fixture covers it.
- **Live check (bl-sdk, the RTSE way; not run):** hook `LiftActionSkill.OnSelectedTarget` during a Phaselock on an
  enemy and read the skill's `LiftedPawn` and `CurrentState` there (expect the target and `EPLS_Intro`). Also read
  `GD_Siren_Skills.Attributes.IsPhaselocked`'s value on an unlocked enemy (expect 0).
