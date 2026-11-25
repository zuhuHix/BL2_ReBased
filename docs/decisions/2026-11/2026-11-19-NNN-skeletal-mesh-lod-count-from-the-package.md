## 2026-11-19: The skeletal mesh data layer reads a mesh's LOD model count from its own package export

AI-assisted (Claude Code, Opus 5.5). Lane SKELMESH-LOD-LAYER, stacked on #651 (fix/rust-link8-newmesh-preload,
da91849b, which carries #645). The decision number is pending from the hub (three digits); NNN is a placeholder.

- **What was decided:** `ow-assets` gets `skeletal_lods::read_lod_count`. It reads a `SkeletalMesh` export's native
  prefix after its tagged properties, up to the LOD model count, and nothing past it. The skeletal mesh data layer
  asks it first, through a new defaulted `SkeletonSource::lod_count_in` method that receives the definition store's
  own in-memory package and the mesh's export. `ow-cli`'s source implements it.
- **The layout, fitted and UNVERIFIED** (AGENTS.md rule 4: recovered from the packages, not invented). In order:
  - the bounds (28 bytes);
  - the materials (a count, then 4-byte references);
  - the origin and the rotation origin (12 bytes each);
  - the reference skeleton (a count, then 52-byte bones: a name, flags, a rotation, a position, the child count, the
    parent index, a colour word);
  - the skeletal depth;
  - the LOD count.
- **The structural oracles, enforced in code:**
  - every count fits the export;
  - every material reference resolves;
  - every bone name is a valid name-table entry, and every number is finite;
  - the parents are in order;
  - the stored depth equals the depth the parent chain gives;
  - the LOD count is in 1..=32 and equals the mesh's own tagged `LODInfo` array length, read independently from the
    tag stream.

  The child counts are not enforced: they sum to the bone count less one for every mesh but one gestalt mesh.
- **The census** (an ignored installed test in `ow-cli`, `installed_lod_count_census`, counts only). It covered nine
  packages: Startup, the five class streaming packages, Sanctuary, Southern Shelf, Hyperion City and Frost. **90
  SkeletalMesh exports read, 0 refused**: 88 with one LOD, one with two and one with four, each equal to its tagged
  `LODInfo` length. For Maya's `Skel_SirenBody`, the 27 bone names also equal UModel's `.psk` export bone for bone
  (UModel as an external reference only, the local probe). Link 8's `GestaltDef_AssaultRifle_GestaltSkeletalMesh`
  reads 1 LOD.
- **The equip world** (the equip run's set-up and the slice gate's link 8) now installs the skeletal mesh data layer
  in Off, as the combat run does. That install is a logged harness input. The count itself is read from the player's
  own package, not a stand-in, and is logged per mesh.
- **What this does not change:** `SetSkeletalMesh`'s LOD row (NATIVE_SKELMESH_MESH_CHANGE.md row 15, #645) refuses
  every known positive count by its own rule. The native copies a per-LOD usage byte from each LOD model, and the
  port does not decode LOD models. So link 8 still stops there, now with the count known. This lane does not change
  that rule (its author analysed that note: clean room). The next step is decoding that per-LOD byte, which needs the
  LOD model layout.
- **Sensitive files:** none; `crates/ow-package` is unchanged.
- **Verified:** nothing in the running game. The layout is a fit checked by the oracles above, and stays UNVERIFIED.
