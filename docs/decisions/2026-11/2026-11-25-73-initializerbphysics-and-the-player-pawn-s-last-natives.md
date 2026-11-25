## 2026-11-25: InitializeRBPhysics and the player pawn's last natives

AI-assisted (Claude Code)

Lane ANALYST-LINK1-RBPHYSICS-AND-AFTER, analyst only, with Claude Opus 5.5. Docs only: no code, no dependency, and no
bounds check changed.

- **Why:** the next unread natives on `maya:8` after the collision setters (#734).
- **What:** the new note `docs/verification/NATIVE_LINK1_RBPHYSICS_AND_AFTER.md`, plus two NATIVE_INDEX rows and one
  GATES row.
- **Evidence:**
  - `research/script_disasm.py`;
  - a local read-only Ghidra reading under the machine lock (six execs, their slots in `WillowPlayerPawn`'s table,
    and the routines they reach);
  - the layout tool;
  - `ow-package --def-dump` of Maya's pawn and body classes.

  Raw output stays under ignored `local/analysis/rbphys/`.
- **Findings** (UNVERIFIED in game):
  - `InitializeRBPhysics` sets up rigid bodies for attached components only, then `SetPushesRigidBodies`. For Maya's
    unattached components in the port it builds nothing;
  - `GetFirstPersonWeaponHoldDef` is `GetWeaponHoldDef`'s rule on the first-person fields;
  - `GetLinearColor` divides each byte by 255;
  - the hook sets `ImmuneToFallingDamage`;
  - `InitializeBehaviorProviders` and `GetInstancedMaterialInstanceConstant` are outlined only.
- **Proposed Off answer:**
  - per native;
  - physics creation `LayerAbsent`;
  - the outlined ones `NotRead`;
  - tests listed.
- **Next** (a prediction, not a run): `WillowPawn.InitializeBehaviorProviders`.
- **Clean room:** a different lane implements this note.
