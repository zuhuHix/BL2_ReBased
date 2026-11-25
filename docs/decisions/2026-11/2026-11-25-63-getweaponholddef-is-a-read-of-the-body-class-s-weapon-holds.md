## 2026-11-25: GetWeaponHoldDef is a read of the body class's weapon holds

AI-assisted (Claude Code)

Lane ANALYST-GETWEAPONHOLDDEF, analyst only, with Claude Opus 5.5. Docs only: no code, no dependency, and no bounds
check changed.

- **Why:** links 1, 3, 4, 5, 13 and 14 now stop at an error, the unbound `BodyClassDefinition.GetWeaponHoldDef`, in
  `maya:8`'s `WillowPawn.UpdateAnimSets`.
- **What:** the new note `docs/verification/NATIVE_GET_WEAPON_HOLD_DEF.md`, plus one NATIVE_INDEX row and one GATES
  row.
- **Evidence:**
  - `research/script_disasm.py`;
  - a local read-only Ghidra reading under the machine lock;
  - the layout tool;
  - `ow-package --def-dump` of Maya's body class, its parent and her hold definitions.

  Raw output stays under ignored `local/analysis/holddef/`.
- **Findings** (UNVERIFIED in game):
  - the lookup scans `WeaponHoldDefs` for the hold name. An exact tag match answers at once, else a name-only
    candidate is kept;
  - it can search `WeaponActionParentClass`, passing the default fallback on only when this class has no default;
  - it falls back to `DefaultWeaponHoldDef` when asked. It writes nothing;
  - for Maya (`Unarmed`, no tag) it answers `WeaponHold_Player_Default` on the shared parent class, then
    `WeaponHold_Siren_Unarmed`.
- **Proposed Off answer:**
  - a load-free read;
  - an unloaded receiver, entry or parent is `NotRead`;
  - malformed declarations are `VmError::Script`;
  - tests listed.
- **Next** (a prediction, not a run): `SetActiveProfileByName`, then the upper-body blend natives.
- **Review r1 (#728):**
  - the parent's exact-match test is named: the caller re-reads the returned hold's `HoldName` and `EnvironmentTag`;
  - so a matching parent default stands over the candidate, and that returned object gets the not-loaded `NotRead`;
  - tests added for the F-passed side, the re-read quirk and the slice's armed-hold shape.
- **Clean room:** a different lane implements this note.
