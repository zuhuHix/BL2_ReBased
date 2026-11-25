## 2026-11-25: SetSkeletalMesh's row 15 refills LODInfo with default entries in Off (link 8)

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-ROW15, slice link 8, stacked on #673 (SKELMESH-LOD-LAYER, decision
2026-11-25-09). Coded from docs/verification/NATIVE_SKELMESH_MESH_CHANGE.md step 16 and row 15, and from #673's data
layer (`skeletal::lod_count`, `skeletal::lod_trailing_counts`), on two **hub rulings** given on this lane's proposal.
Clean room: no executable, Ghidra, REA or analysis output was opened. Synthetic fixtures only. Off only; Reference
binds nothing, as before.

**The hub's rulings** (both UNVERIFIED; the maintainer may overturn either):
1. **"A zero trailing count means no per-LOD data."** Step 16 copies a usage byte from each LOD model "whose model has
   data of a kind not identified". #673 fits that data to the LOD model's trailing count, which is zero in all 110
   installed LOD models. With every trailing count zero, the per-LOD loop does nothing, so no usage byte is needed.
   - That is a fit from the package layout, not a reading of the native.
   - **The alternative condition**, raised in the review of #673: the other unidentified LOD-model field, the word
     before the vertex count. It is also zero in all 110 LODs, so the installed answer would be the same, but #673's
     layer does not expose that word and this row does not check it.
   - Any non-zero trailing count, no count, no trailing list, or a list of another length keeps the existing
     `LayerAbsent`, write-free.
2. **The default entry comes from the struct's script default**, checked in preflight against the note's default
   entry. The note says a default entry has no hidden materials, both flags clear and the index -1. The check is by
   kind: exactly one array (empty), two bools (clear), one int (-1) and one byte. Any other shape or value is
   `NotYet { NotRead }`, "SetSkeletalMesh: the LOD info entry's default".
   - The usage byte is taken as that default holds it. That is UNVERIFIED: the note does not say what it is.

**What changes** (`ow-world` `world/skeletal_mesh.rs`):
- **Row 15** (a new mesh only) reads the LOD count N and the trailing counts. With all of them zero, a `LODInfo`
  length different from N plans the refill; an equal length plans nothing.
- **Row D** checks `LODInfo` with a new mesh: it must be declared as a dynamic array of a struct, else a
  `VmError::Script` naming it. When a refill is planned, the default entry is checked and the planned value passes the
  heap's check. A clear still skips row 15 and does not read `LODInfo`.
- **The writes:** the refill is a fifth write, after `bHasValidBodies`, in the native's step order: `LODInfo` set to N
  default entries (N = 0 empties it). With an equal length there are four writes, as before.

**Effect on link 8** (predicted, not run): `GestaltDef_AssaultRifle_GestaltSkeletalMesh` reads 1 LOD with trailing
count [0] (#673's log), and the component's `LODInfo` is empty by default. So the row now passes with five writes,
provided the installed `SkelMeshComponentLODInfo` default matches the note's shape. Otherwise it stops at the new
`NotRead`. Link 8's next stop needs a runner probe.

**Tests** (`skeletal_mesh_change_tests.rs`, synthetic; every refusal leaves #518's counters, the name table and the
component's slots unchanged):
- the refill: N=1 against an empty `LODInfo` (link 8's shape) gives 5 writes and one default entry with index -1;
  N=2 against 2 entries gives 4 writes with the entries kept; N=2 against 1 entry gives 2 defaults; N=0 against 1
  entry empties it; N=0 against none gives 4 writes;
- the refusals, each `LayerAbsent`: no layer, no count, no trailing list, a non-zero trailing count, and a list of
  another length;
- `LODInfo`'s checks: a struct whose default differs is `NotRead` when a refill is planned and is not asked when none
  is; `LODInfo` declared as ints is a script error with a new mesh and is not read on a clear;
- the fixture's `LODInfo` is now an array of the entry struct, and the clear test keeps a struct entry.

**Not verified:** anything in the running game; both rulings; the installed struct's default; the usage byte.
