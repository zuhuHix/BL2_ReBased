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
  - **The same mesh does nothing.** On link 8 the probe observed two resets (first- and third-person meshes) to
    None, on components that already held None. The weapon had no extra slots, so the extra-slot loop after
    `InitExtraMeshes` made no call.
  - **A different mesh** re-initialises the component: physics instance, tree-initialised flag, bone transforms
    from the reference pose, the tree's per-bone state, and the render registration. One `LayerAbsent` refusal is
    proposed for the whole change.
  - **Overlap:** #569 reads the reference skeleton but binds no component native; #570 and #575 cover other steps.
  - **The first stop on link 8 is an instancing question:** the weapon's `FirstPersonMesh` is the class default's
    unloaded component handle. The hub ruled (on #583) that the harness instances the weapon's components through
    #435's spawn-path instancing, as an UNVERIFIED harness input.
  - **Review r1 of #583:** the preflight order is specified as receiver, the current mesh read from the resident
    slot, the arguments, then identity before residency. Equal handles succeed even when unloaded; unequal handles,
    a clear to None included, refuse `LayerAbsent` before any write.
  - **The probe:** with the handles loaded, both observed resets are no-ops, and link 8 stopped at the unbound
    `WillowWeapon.BuildCompositeMesh(object,weapondefinitiondata)`. A refusal in this native is predicted only if the
    run later reaches the setter with a different mesh.
