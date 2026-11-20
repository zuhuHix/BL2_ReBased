## 2026-11-19: Read WillowWeapon.StaticGetWeaponDamageType for link 8 as analyst

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-STATIC-GET-WEAPON-DAMAGE-TYPE, analyst only, for slice link 8. The
decision number is pending from the maintainer (three digits); NNN is a placeholder.

- **What:** a new own-words note, `docs/verification/NATIVE_STATIC_GET_WEAPON_DAMAGE_TYPE.md`, and one
  `NATIVE_INDEX.md` row. No code changes.
- **Sources:**
  - the script (`research/script_disasm.py`);
  - the installed packages (`ow-package --def-dump`, `tools/ghidra/class_layout.py`);
  - `Borderlands2.exe`, read locally with the project's Ghidra tooling (read-only, under the machine lock);
  - a local, uncommitted slice-gate probe.

  Raw output stays under ignored `local/analysis/dmgtype/`. No bl-sdk session took place, so **every rule is
  UNVERIFIED** in the running game.
- **Clean room:** this lane's author will not implement this native.
- **Findings:**
  - **The rule** (re-checked against the 2026-10-23 reading): None without a weapon type; otherwise the elemental
    part's `CustomDamageTypeDefinition` when both are set, else the type's `DefaultDamageTypeDefinition`. It reads only
    those, and writes and loads nothing.
  - **On link 8:** no elemental part, so the answer is `GD_Impact.DamageType.DmgType_Normal`. The type is loaded on
    entry.
  - **The proposed Off answer:** a write-free, load-free preflight (`NotRead` for an unloaded type or elemental part;
    `VmError::Script` for bad shapes), then the rule. `ow-combat`'s helper is the shared core once its loading is
    taken out.
  - **The probe:** with stand-ins up to and including this native, link 8 is 1 of 5 at `init`, next at the unbound
    `SkeletalMeshComponent.SetSkeletalMesh(object,bool)`. The slice gate is 1 ok, 4 notyet, 9 error.
