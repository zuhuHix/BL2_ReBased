## 2026-11-19: Object.GetFuncName() in Off

AI-assisted (Claude Code). Lane IMPL-GET-FUNC-NAME, implementer. `Object.GetFuncName()` is a stock UE3 Core native,
so there is no analyst note: the rule is the public UE3 meaning, and this lane read the executable for nothing. There
was no real-game session, so it stays **UNVERIFIED** in the running game. No dependency or crate edge was added, and
Reference is unchanged (the C++ VM has no binding; Reference leaves it unbound).

- **The call site** (installed script, `research/script_disasm.py`): `LiftActionSkill.OnSelectedTarget` passes
  `GetFuncName()` as the event name of `BehaviorKernel.ActivateBehaviorEventFromScript(ConsumerHandle,
  GetBehaviorProviderDefinition(), GetFuncName(), <filter omitted>, Parameters)`. The declaration is native, static
  and final, with 25 script callers in the five code packages.
- **`ow-vm` `func_name.rs`**, registered from `natives_core` in Off only, first-wins, write-free:
  - the answer is the name of the innermost running script function (the VM's stack of script frames). On link 9 that
    is `OnSelectedTarget`;
  - with no script function running (a call from outside bytecode), `NotYet { NotRead }`: the answer outside a script
    frame is not read.
- **Tests:** 3 (the innermost function's name, back to the outer after the inner returns; the no-frame `NotRead`,
  write-free; Off binds, Reference does not).
- **The slice** (installed, Off, strict, a local merge never pushed): this branch with #535 @ 7f8fc7d7, #548 @
  22b29bfb, #555 @ 0e9092a8, #560 @ 97dc3a43 (newer than the 3bad6013 named), #523 @ d2227301 (its counter names
  mapped onto #518's), and the link 9 spawn branches (`fix/rust-link9-extent`, `fix/rust-encroachment-check`,
  `fix/rust-link9-trace-filter`, `fix/rust-link9-next-stop`, `fix/rust-link9-attr-get-value`).
  - 1 ok / 3 NotYet / 10 errors.
  - Link 9 passes `spawn` and `cast`. The cast now runs through `ActivateBehaviorEventFromScript` and returns, graded
    error only for "no evidence check is chosen yet" (what a lock leaves on its target is not settled).
  - A/B: the same merge without this binding stops link 9's `cast` at the unbound `Object.GetFuncName()`.
- **Live check (bl-sdk, the RTSE way; not run):** hook `BehaviorKernel.ActivateBehaviorEventFromScript` during a
  Phaselock and read its `EventName` argument from `LiftActionSkill.OnSelectedTarget`: expect `OnSelectedTarget`.
  Also call `GetFuncName()` from a script hook inside another function and expect that function's name.
