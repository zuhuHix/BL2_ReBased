## 2026-11-19: Read EffectCollectionDefinition.GetParticleEffect as analyst

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-GET-PARTICLE-EFFECT, analyst only, for slice link 8. The decision
number is pending from the maintainer (three digits); NNN is a placeholder.

- **What:** a new own-words note, `docs/verification/NATIVE_GET_PARTICLE_EFFECT.md`, and one `NATIVE_INDEX.md` row.
  No code changes.
- **Sources:**
  - the script (`research/script_disasm.py`);
  - the installed packages (`ow-package --def-dump`, `tools/ghidra/class_layout.py`);
  - `Borderlands2.exe`, read locally with the project's Ghidra tooling (read-only, under the machine lock);
  - a local, uncommitted slice-gate probe.

  Raw output stays under ignored `local/analysis/gpe/`. No bl-sdk session took place, so **every rule is UNVERIFIED**
  in the running game.
- **Clean room:** this lane's author implemented #558 (RecomputeAttributeBaseValues), not this native, and will not
  implement this one.
- **Findings:**
  - **The callers:** `ClientInitializeFromWeaponTypeDefinition`, twice: the barrel's collection, then the weapon
    type's, each with the weapon as the argument.
  - **The rule:** the first `ParticleEffects` entry whose `Expression` is None, or whose evaluator answers true for
    the argument, gives its `ParticleEffect`; otherwise None. The native writes and loads nothing of its own.
  - **The slice gun:** 6 entries, 5 attribute tests (corrosive, explosive, incendiary, shock, amp) and a default.
  - **The probe:** the collection and its evaluators arrive as unloaded handles, so a write-free preflight refuses
    `NotYet { NotRead }` until the hub rules on preloading or marked loads. Past that, the next stops are the unbound
    `AttributeExpressionEvaluator.Evaluate(object)` in the equip run, then
    `InventoryBalanceDefinition.GetExpLevelFromManufacturerData(object,int)`.
