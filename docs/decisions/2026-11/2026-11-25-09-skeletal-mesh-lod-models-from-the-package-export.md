## 2026-11-25: The skeletal mesh data layer reads a mesh's LOD models from its own package export

AI-assisted (Claude Code, Opus 5.5). Lane SKELMESH-LOD-LAYER, on main d3c7a13b.
The lane started on 2026-11-20 (the LOD count) and finished on 2026-11-25 (the LOD
models).

- **What was decided:** `ow-assets` gets `skeletal_lods`. `read_lod_count` reads a `SkeletalMesh` export's native
  prefix after its tagged properties, up to the LOD model count. `read_lod_models` goes on through every LOD model, the
  bone-name map and a short trailer to the export's last byte. The skeletal mesh data layer (`ow-world`
  `world::skeletal`) asks the source through two new defaulted `SkeletonSource` methods that receive the definition
  store's own in-memory package and the mesh's export:
  - `lod_count_in`, asked before the name-based `lod_count`;
  - `lod_trailing_in`, each LOD model's trailing count, read by the new `skeletal::lod_trailing_counts`.

  `ow-cli`'s source implements both with the full read, so a count is only given when the whole export fits.
- **The layout, fitted and UNVERIFIED** (AGENTS.md rule 4: recovered from the packages, not invented). The field
  order is in `crates/ow-assets/src/skeletal_lods.rs`'s module documentation:
  - the prefix: the bounds, the materials, the origin and rotation origin, the reference skeleton (52-byte bones),
    the skeletal depth, the LOD count;
  - each LOD model: the sections, the index buffer, the active bones, the chunks, a word, the vertex count, the
    required bones, a bulk-data block, the vertex buffer, the colour buffer when the mesh is tagged
    `bHasVertexColors`, and a trailing count;
  - after the models: the bone-name map, three zero words, two short arrays.
- **The structural oracles, enforced in code:**
  - every count fits the export;
  - the stored depth equals the depth the parent chain gives;
  - the LOD count equals the mesh's own tagged `LODInfo` array length;
  - in each LOD model:
    - the index count is three times the sections' triangles, at the stated index width;
    - every chunk starts where the ones before it end, and the stated vertex count equals the chunks' total and the
      vertex buffer's length;
    - every bone index names a bone;
    - the bulk-data block records its own package position;
    - the vertex stride is 28 + 4 per UV set;
    - the colour buffer is there exactly when the tag says so;
  - the bone-name map names every bone by its own name;
  - the read ends on the export's last byte.

  Nothing is clamped. A value outside the fitted layout is a typed refusal, and so is a non-empty array whose
  elements are not decoded (the chunks' vertex arrays and the trailing count).
- **What is not checked, and why:** the section material index (a 12-triangle placeholder cube, `CubeForCode`, holds
  255 against one material in four class packages), the child counts (see the prefix), the word before the vertex
  count, and the trailer's array contents.
- **The census** (an ignored installed test in `ow-cli`, `installed_lod_count_census`, counts only; output under
  ignored `local/analysis/skellod/`). It covered twelve packages: Startup, the six class streaming packages,
  Sanctuary, Southern Shelf, Hyperion City, Frost and Ice.
  - **106 SkeletalMesh exports read to their last byte, 0 refused.**
  - 110 LOD models: 104 meshes with one LOD, one with two and one with four.
  - 25 LOD models carry the colour buffer, each on a mesh tagged `bHasVertexColors`.
  - **The trailing count is zero in all 110.**
  - Link 8's `GestaltDef_AssaultRifle_GestaltSkeletalMesh` reads 1 LOD with trailing count 0.
- **The per-LOD usage byte, still open.** NATIVE_SKELMESH_MESH_CHANGE.md step 16 copies a usage byte from each LOD
  model "whose model has data of a kind not identified". In the serialized LOD model, the trailing count is the only
  field left unidentified. It is zero everywhere in the census, so if that field is the native's data, no installed
  LOD needs the byte. That match is a fit from the package layout, not a reading of the native, and it stays
  **UNVERIFIED**. Whether the layer's "trailing count zero" may stand for "no such data" in row 15 is for the hub and
  that row's owner to rule. This lane does not change row 15 (`crates/ow-world/src/world/skeletal_mesh.rs`, lane A's
  file), so link 8 still stops there.
- **The equip world** (the equip run's set-up and the slice gate's link 8) installs the skeletal mesh data layer in
  Off and logs it as a harness input. Then it logs the mesh's LOD count and trailing counts, both read from the
  player's own package.
- **Sensitive files:** none. `crates/ow-package` is unchanged.
- **Verified:** nothing in the running game. The layout is a fit checked by the oracles above, and stays UNVERIFIED.
