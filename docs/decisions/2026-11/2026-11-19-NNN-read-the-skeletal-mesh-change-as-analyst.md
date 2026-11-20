## 2026-11-19: A skeletal mesh change empties the bone transforms and rebuilds none

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-SKELMESH-MESH-CHANGE, analyst only (clean room: a different lane
implements it). The decision number is pending from the hub (three digits); NNN is a placeholder.

- **What was decided:** `docs/verification/NATIVE_SKELMESH_MESH_CHANGE.md` records:
  - **the answer to the hub's question.** A mesh change on `SkeletalMeshComponent.SetSkeletalMesh` empties the
    component's transform buffer (`SpaceBases`) and rebuilds it from neither the new mesh's reference pose nor the old
    transforms. Only an attached component's re-attach (or a later attach or tick) refills it. Saved transforms are
    copied back by bone name only into a refilled buffer;
  - **the order** of the change's seventeen steps, and what each reads and writes. Those steps cover the physics
    instance, the origin compensation, the tree flag and per-bone state, the transform, the parent bone map, the
    bodies flag, the morph map, `LODInfo` and a notification;
  - **a correction** to NATIVE_SET_SKELETAL_MESH.md (#583) section 2.2, step 8, which said the transforms are rebuilt
    from the reference pose. That note now points to the new one;
  - **the scope split** with ANALYST-BODYCOMP-SOCKET-BONE-TABLES: the attach's pose update and the bone tables belong
    to that lane's side;
  - **a proposed Off answer** that replaces #599's single refusal:
    - the new and old meshes must be resident (`NotRead`);
    - each path the port has no layer for is refused before any write: keep with a non-empty buffer, a physics
      instance, a tree, an attached component, a parent component, a physics asset, morph sets, and a differing LOD
      count;
    - the minimal case writes `bAnimTreeInitialised`, `SkeletalMesh`, `SpaceBases`, `bHasValidBodies` and the morph
      map;
    - **the native-only transform, by the hub's ruling (05:35Z):** `LayerAbsent` when either mesh has a non-zero
      origin offset or rotation, nothing when both are zero (UNVERIFIED);
    - Reference is unchanged.
- **Sources:** `Borderlands2.exe` through `tools/ghidra/run.ps1 -ReadOnly` under the machine lock. Raw output stays
  under ignored `local/analysis/smmc/`. No addresses, decompiler output or listing structure is in tracked files. No
  bl-sdk session was granted.
- **Verified:** nothing in the running game. Every rule is **UNVERIFIED**, and the field names are by fit.
- **Probe** (local throwaway merge of main 4c4522d7 with #577, #599, #596 and #613, deleted after; installed Off
  `--slice-gate` strict under the lock):
  - without stand-ins: 4 ok, 10 NotYet, 0 errors. Link 8 stops in `init` at `BuildCompositeMesh`'s unloaded
    definition;
  - with a gestalt preload and the mesh-change refusal stubbed: 4 ok, 9 NotYet, 1 error. Link 8 passes both mesh
    changes and stops at the unbound `PrimitiveComponent.SetScale(float)`.

  Both changes link 8 reaches are the minimal case (None to the composite mesh, unattached). The new mesh is an
  unloaded handle, so under the proposal link 8 stops first at its `NotRead`, and then at the `LODInfo` layer row.
- **Index:** a NATIVE_INDEX row was added.
