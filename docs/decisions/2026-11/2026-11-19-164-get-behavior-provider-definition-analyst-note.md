## 2026-11-19: ActionSkill.GetBehaviorProviderDefinition: analyst note

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-GET-BEHAVIOR-PROVIDER-DEFINITION, 2026-11-19. Number pending (NNN).

**What changed.** A new own-words note, `docs/verification/NATIVE_GET_BEHAVIOR_PROVIDER_DEFINITION.md`, and a
NATIVE_INDEX row. No code. Analyst only: a different lane implements it (clean room).

**Why.** Slice link 9's `cast` stops at this native, unbound, on a merge with #523, #535 and #548. It is the definition
argument of the behavior event `LiftActionSkill.OnSelectedTarget` fires.

**What the note says:**
- **The native forwards (high):** it is the behavior-provider interface's method, native in 22 classes and bound by
  name. `ActionSkill`'s asks the receiver's interface method and returns its answer, writing nothing.
- **The answer (low, fitted):** `ActionSkill`'s interface method answers the receiver's own
  `BehaviorProviderDefinition` field, with no fallback. The structural step is consistent only with a range of struct
  sizes, so the rule leans on the script's call pattern and, if a spawned skill keeps the archetype's subobject, on
  NATIVE_ACTION_SKILL.md's consumer registration.
- **Link 9's receiver:** the installed Phaselock archetype holds its own subobject there.
- **Proposed Off answer:** the field read straight from the heap, as stored, unloaded and None included. An
  unloaded receiver is a named `NotYet` with nothing loaded; a class without the field is a data error. The other 21
  classes stay unbound, and there is no hold-back (#553 r1 ruling).

**Sources:** the installed script (`research/script_disasm.py`), the definition store, and local read-only Ghidra
queries under the machine lock. Raw output is under ignored `local/analysis/gbpd/`.

**Not done:** the local probe of link 9's next stop. Merging #535 onto main with #523 conflicts in code, so it was not
cheap.

**UNVERIFIED:** every rule in the running game. The bl-sdk checks are in the note.
