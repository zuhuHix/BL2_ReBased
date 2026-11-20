## 2026-11-19: Read GetExpLevelFromManufacturerData for link 8 as analyst

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-GET-EXP-LEVEL-FROM-MANUFACTURER-DATA, analyst only, for slice link 8.
The decision number is pending from the maintainer (three digits); NNN is a placeholder.

- **What:** a new own-words note, `docs/verification/NATIVE_GET_EXP_LEVEL_FROM_MANUFACTURER_DATA.md`, and one
  `NATIVE_INDEX.md` row. No code changes.
- **Sources:**
  - the script (`research/script_disasm.py`);
  - the installed packages (`ow-package --def-dump`);
  - `Borderlands2.exe`, read locally with the project's Ghidra tooling (read-only, under the machine lock), to
    re-check NATIVE_LOOT.md's reading;
  - a local, uncommitted slice-gate probe.

  Raw output stays under ignored `local/analysis/explevel/`. No bl-sdk session took place, so **every rule is
  UNVERIFIED** in the running game.
- **Clean room:** this lane's author will not implement this native.
- **Findings:**
  - **The rule** is NATIVE_LOOT.md's, re-checked: the cycle guard answers 1; an interpolating balance answers the
    grade-index word; otherwise the manufacturer's grade gives `GradeModifiers.ExpLevel`, or the word's high half
    capped at 63; then the base; else 1. Only internal loot helpers carry it today; nothing binds the script native.
  - **On link 8:** the slice balance `AR_Bandit` interpolates and is loaded on entry. The equip-run harness leaves
    `ManufacturerGradeIndex` at 0, so the native answers 0 and `ExpLevel` drops from 1 to 0. That is a harness-input
    question for the hub.
  - **The proposed Off answer:** a write-free, load-free preflight (`NotRead` for an unloaded balance on the visited
    chain), then the rule.
  - **The probe:** with stand-ins for this native and `GetParticleEffect`, link 8 is 1 of 5 at `init`, next at the
    unbound `AttributeInitializationDefinition.SetBaseValue(object,attributeinitializationdata,object,object)`
    (`ApplyAnimThresholdAttributeValues`). The slice gate is unchanged at 1 ok, 4 notyet, 9 error.
