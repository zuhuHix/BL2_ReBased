## 2026-11-26: Links 6 and 7 run the pickup gun's init before the drop

AI-assisted (Claude Code)

Lane ANALYST-LINK67-PICKUP-GUN-INIT, analyst only, with Claude Opus 5.5. Docs only: no code, no dependency, and no
bounds check changed.

- **Why:** with option B's template copy, links 6 and 7 are predicted to meet source e's `TEMPLATE` `NotRead` in
  `touch`, because the fixture's gun never runs its init.
- **What:**
  - the new note `docs/verification/NATIVE_LINK67_PICKUP_GUN_INIT.md`;
  - a NATIVE_INDEX row and a GATES row;
  - dated corrections in NATIVE_TOUCH_COMPONENT_TEMPLATE.md section 3.4 and NATIVE_NEW_COMPONENT_INSTANCING.md
    section 6.
- **Evidence:** `research/script_disasm.py`, the port's code on main 8fa9839d, `ow-package --def-dump` of the rifle's
  type and the third-person mesh template, and the property flags of `Engine.upk`. Raw output stays under ignored
  `local/analysis/gunin/`.
- **Findings** (UNVERIFIED in game):
  - a weapon is initialised when it is made (`InitializeInternal` → `ClientInitializeFromWeaponTypeDefinition` →
    `InitMeshes`), long before a drop;
  - `InitMeshes` sets `BuildCompositeMesh`'s answer on both meshes. On the gestalt path that is the shared
    `GestaltSkeletalMesh`, a package export. It sets the third-person scale from the type (0.92 for the slice's rifle);
  - the drop's `AttachMeshToPickup` copies that third-person mesh.
- **Proposed harness:**
  - link 8's preparation for each pickup gun, before `drop_at`: manufacturer inputs, the preloads, the mesh layer,
    the threshold preload and `InitializeInternal(false)`;
  - all covered on main, except the decoy's unowned init;
  - refusals after the weapon's creation keep `make`'s partial-writes mark.
- **Next** (a prediction, not a run): without the mesh layer, source f's stored-bounds `LayerAbsent`. With it, the
  bounding cylinder's `WORLD_TRANSFORM` `NotRead` (the scale 0.92); whether to answer a scaled transform is the hub's
  call.
- **Correction:** two notes said the init's mesh is built at run time and is not an export. For the gestalt path that
  is wrong.
- **Clean room:** a different lane implements this note.
