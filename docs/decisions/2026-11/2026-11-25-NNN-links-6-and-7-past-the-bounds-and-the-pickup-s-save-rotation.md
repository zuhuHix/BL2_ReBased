## 2026-11-25: Links 6 and 7 past the bounds, and the pickup's save rotation

AI-assisted (Claude Code)

Lane ANALYST-LINKS67-NEXT, analyst only, with Claude Opus 5.5. Docs only: no code, no dependency, and no bounds
check changed.

- **Why:** a look-ahead for links 6 and 7. Both stop at `GetBoundingCylinder`'s option-B `NotRead`
  (NATIVE_COMPONENT_BOUNDS.md section 9.5).
- **What:** the new note `docs/verification/NATIVE_PICKUP_SAVE_ROTATION.md`, plus one NATIVE_INDEX row and one GATES
  row.
- **Evidence:**
  - `research/script_disasm.py`;
  - `ow-package --def-dump`;
  - a local read-only Ghidra reading under the machine lock (`WillowPickup`'s class table slot, found as in
    NATIVE_COMPONENT_SKIP_FLAG.md section 6);
  - the layout tool.

  Raw output stays under ignored `local/analysis/links67/`.
- **Findings** (UNVERIFIED in game):
  - option B needs two things, neither of them a data layer or a load:
    - the VM's `new` honouring its template operand, an interpreter rule;
    - the pickup fixture's gun running its init, since the weapon's `ThirdPersonMesh` has no mesh until `InitMeshes`;
  - past the bounds, the touch's natives are covered. Link 7 stops at `look`'s HUD;
  - link 6's first unread native is `WillowPickup.GetSaveRotation`: a `PHYS_RigidBody` pickup with a
    `CollisionComponent` answers its body's orientation, else the actor's `Rotation`;
  - the next unread item is `Actor.ComponentList`.
- **Proposed Off answer:**
  - the `Rotation` off the rigid-body path;
  - on it, `LayerAbsent` by default, or the at-rest stand-in (the `Rotation`) under a hub ruling extending #574;
  - tests listed.
- **Clean room:** a different lane implements this note.
