## 2026-11-25: SetSkeletalMesh: the LOD-info refill's constructed entry

AI-assisted (Claude Code)

Lane ANALYST-LODINFO-CONSTRUCTED-ENTRY, analyst only, with Claude Opus 5.5. Docs only: no code, no dependency, and no
bounds check changed.

- **Why:** NATIVE_SKELMESH_MESH_CHANGE.md step 16 says the LOD-info refill adds default entries with index -1. The
  packages' script default of `SkelMeshComponentLODInfo` has index 0, so the native's own values had to be read.
- **What:** a dated amendment to `docs/verification/NATIVE_SKELMESH_MESH_CHANGE.md`, from a local read-only Ghidra
  reading under the machine lock and the layout tool. Raw output stays under ignored `local/analysis/lodinfo/`.
- **Findings** (UNVERIFIED in game): each constructed entry has empty `HiddenMaterials`, both flags false,
  `InstanceWeightUsage` 0 (`IWU_PartialSwap`) and `InstanceWeightIdx` -1 (as read). It differs from the script default
  in the index only. The per-LOD loop then overwrites usage and index for LODs with model data.
- **For lane A:** build the entries explicitly with those values; do not copy the struct default. A unit test of entry
  construction, plus an integration test once a LOD provider exists.
- **Clean room:** lane A implements the amendment.
