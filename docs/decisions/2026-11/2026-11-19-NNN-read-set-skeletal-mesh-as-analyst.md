## 2026-11-19: Read SkeletalMeshComponent.SetSkeletalMesh for link 8 as analyst

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-SET-SKELETAL-MESH, analyst only, for slice link 8. The decision number
is pending from the maintainer (three digits); NNN is a placeholder.

- **What:** a new own-words note, `docs/verification/NATIVE_SET_SKELETAL_MESH.md`, and one `NATIVE_INDEX.md` row. No
  code changes.
- **Sources:** the script (`research/script_disasm.py`); `Borderlands2.exe`, read locally with the project's Ghidra
  tooling (read-only, under the machine lock); a local, uncommitted slice-gate probe. Raw output stays under ignored
  `local/analysis/ssm/`. No bl-sdk session took place, so **every rule is UNVERIFIED** in the running game.
- **Clean room:** this lane's author will not implement this native.
- **Findings:**
  - **The same mesh does nothing.** On link 8 the first three calls reset the weapon's meshes to None, and the
    components already hold None.
  - **A different mesh** re-initialises the component: physics instance, tree-initialised flag, bone transforms
    from the reference pose, the tree's per-bone state, and the render registration. One `LayerAbsent` refusal is
    proposed for the whole change.
  - **Overlap:** #569 reads the reference skeleton but binds no component native; #570 and #575 cover other steps.
  - **The first stop on link 8 is an instancing question:** the weapon's `FirstPersonMesh` is the class default's
    unloaded component handle, so the preflight refuses `NotRead`. The hub chooses between a harness instancing of
    the weapon's components and keeping the stop.
  - **The probe:** with that handle loaded, the resets are no-ops, and link 8 next stops at the unbound
    `WillowWeapon.BuildCompositeMesh(object,weapondefinitiondata)`.
