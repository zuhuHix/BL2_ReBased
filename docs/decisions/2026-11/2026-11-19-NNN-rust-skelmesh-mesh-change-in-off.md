## 2026-11-19: SetSkeletalMesh's mesh change in Off

AI-assisted (Claude Code). Lane IMPL-SKELMESH-MESH-CHANGE. The decision number is pending (the landing run numbers it).
Coded from docs/verification/NATIVE_SKELMESH_MESH_CHANGE.md alone, the note's final head ec99290a (lane
ANALYST-SKELMESH-MESH-CHANGE, #635 accepted, with the hub's ruling on the transform), copied in unchanged. Clean room:
this lane did not read the native. The executable and the analysis output were not opened. Synthetic fixtures only.

- **What changes:** `ow-world` `world/skeletal_mesh.rs` replaces #599's single mesh-change refusal with the note's
  section 4. The binding is now keep-existing and stays Off-only; Reference is unchanged.
- **The rule: the note's one authoritative Off order.** It is write-free, load-free, RNG-free and intern-free up to
  the writes:
  - **A.** rows 1 to 4, #599's: the receiver, the arguments, the current mesh, and equal meshes as nothing;
  - **B.** rows 5 and 6: the old mesh, then the new mesh, resident (`NotYet { NotRead }`); a clear skips row 6;
  - **C.** rows 7 to 15, in the native's step order:
    - kept transforms over a non-empty buffer;
    - a physics instance;
    - an attached component (its detach and re-attach);
    - an animation tree;
    - the transform: a non-zero `Origin` or `RotOrigin` on the old, then the new mesh is `LayerAbsent`; the kinds are
      checked;
    - a parent animation component;
    - a physics asset;
    - morph sets, or a morph-target map that reads as anything but None;
    - `LODInfo`, for a new mesh only;
  - **D.** the four destinations, each declared with its kind, else a `VmError::Script` naming it;
  - then the four writes, in the native's order: `bAnimTreeInitialised` false, `SkeletalMesh`, `SpaceBases` emptied,
    and `bHasValidBodies` false.
- **Row 15's count:** it comes from #569's skeletal-mesh data layer. `SkeletonSource` gains an optional `lod_count`
  (default: unknown), and `skeletal::lod_count` asks the installed source by the mesh's export identity. No source
  decodes the LOD models today, so on installed data every new mesh refuses `LayerAbsent` (the LOD models). Only a
  known count of zero with an empty `LODInfo` passes; that is a test stand-in.
- **Verified:** host unit tests on invented classes cover the note's eleven tests, #599's kept as they were. Every
  refusal leaves #518's three counters and the name table's length unchanged. The minimal change and the clear make
  four heap writes and no draw or load.
- **Not verified:** nothing is checked against the running game, so every rule is **UNVERIFIED** in game.
