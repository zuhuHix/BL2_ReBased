## 2026-11-19: Phaselock cast, part A: `Object.VLerp` in ow-vm, the two provider natives read

AI-assisted (Claude Code, Claude Opus 5.5). Lane FIX-PHASELOCK-CAST part A (slice plan step 4, section 3.5).

- **`Object.VLerp` (implemented, both modes, one rule).** `crates/ow-vm/src/natives_core.rs`. It is the stock UE3 Core
  vector lerp, written from the public UnrealScript semantics: `A + Alpha x (B - A)` per axis, alpha not clamped, as
  the float `Lerp` beside it. No record of the executable and no C++ counterpart was used. Each axis is computed in
  double and rounded once, as `Lerp` does here. The native's own single-precision order is UNVERIFIED. Synthetic
  test `vlerp_is_one_rule_in_both_modes` covers alpha 0 and 1, an interior point, extrapolation above 1 and below 0,
  and per-axis bit agreement with `Lerp`, in Off and in Reference.
- **`BeginNondeterministicProviderRegistration` and `ForceUnregisterBehaviorConsumer` (read, not implemented).** No
  record described them (the NATIVE_* notes, docs/verification and `src/` were searched), so this lane was the analyst
  only. Both were read in the saved Ghidra project (read-only, raw output under ignored `local/analysis/`). The
  own-words outcome note is `docs/verification/NATIVE_BEHAVIOR_PROVIDERS.md`, with Off rules for an implementer
  (section 4), and the index has two new rows. In short: Begin writes no script-visible value (it caps the consumer's
  replication slot counter at 32). Force stops the consumer's threads, drops its deferred entries, marks it
  unregistered, and writes -1 into the caller's handle. Both stay unbound in both modes, as decision 2026-11-16-50
  left them, so the VM's stub policy still answers. A separate lane implements them from the note.
- **The installed lift test (ow-skills, ignored).** `GetLiftLocation`'s second part reads `WorldInfo.TimeSeconds`,
  not its argument (read with `research/script_disasm.py`). The test held world time at 0, so once VLerp was bound
  the lift ran below its start (Z 0 to -200). It now sets `TimeSeconds` to the frame time, and the lift runs Z 0 to
  200 without going down, 43 frames at 60 per second, in Off and in Reference.
- **Next stops.** `LiftActionSkill.OnActionSkillStarted` stops at the unbound `ActionSkill.OnActionSkillActivated`
  in both modes. NATIVE_ACTION_SKILL.md has its rule, so it needs an implementer, not an analyst. `Skill.Activate`
  still stops at `BeginNondeterministicProviderRegistration` (unbound, by design of this part).
- **UNVERIFIED.** Every rule above. Nothing was checked in the running game. The lift's shape is the installed
  script on the Rust VM, not a live reading.
