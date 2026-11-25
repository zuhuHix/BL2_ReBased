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

**Review r1** (Sol high, on #678):
1. **An unsupported trailing count is data, not an error.** On installed data, `ow-cli`'s source answers both
   in-package questions with #673's full reader, and the reader refuses a non-zero trailing count as
   `LodCountError::TrailingArray`. That became a string error, and a `VmError::Script` before row 15 could refuse.
   - **What changed in #673's source** (`crates/ow-cli/src/cmd/skeletal_source.rs` only): one helper, `known`,
     matches `TrailingArray` and answers "not known" (`Ok(None)`) for `lod_count_in` and `lod_trailing_in`. With no
     name-based count either, row 15 then refuses `LayerAbsent`, write-free.
   - The reader, its bounds checks, its error type and the `SkeletonSource` trait are unchanged. Every other reader
     refusal is still an error with its text.
   - Tests: the helper's three cases in `ow-cli`, and in `ow-world` a source that answers like the CLI one.
2. **The named members by name.** The default check resolves `InstanceWeightUsage` (a byte) and `InstanceWeightIdx`
   (an int holding -1) by name through the struct's layout, in any order. A struct with the same kinds and values under
   other names is `NotRead`. The note does not name the other three members, so they are still checked by kind and
   value (one empty array, two clear bools). Their names can be pinned once a names-only read of the installed struct
   is recorded.
3. **The order and the byte.** A test asserts the planned writes in order, with `LODInfo` after `bHasValidBodies`.
   The fixture's script-default usage byte is non-zero (3), and the refill carries it.

**Review r2** (Sol high, on #678), and the hub's amendment to ruling 1:
- **The `known` mapping of review r1, item 1, is reverted.** The reader emits `TrailingArray` for any non-zero
  trailing count, negative ones included, and a misread export already yields it: the reader's own test with an
  untagged colour buffer reads a trailing count of 4. So the source cannot tell a real trailing array from a misread,
  and answering "not known" would hide malformed exports.
- **Ruling 1, amended (UNVERIFIED; the maintainer may overturn it):** on installed data, a non-zero trailing count
  surfaces the reader's error. It is a `VmError::Script` raised from row 15, loud and still write-free, because the
  preflight has written nothing by then. "Non-zero -> `LayerAbsent`" still holds where a source reports the counts as
  data (the synthetic sources, and the `ow-world` test with a source answering like the CLI one, which stays).
- `crates/ow-cli/src/cmd/skeletal_source.rs` is back as #673 left it, and its r1 test is removed with the mapping. All
  110 installed LOD models read a trailing count of 0, so the slice is unaffected.

**Ruling 2, corrected by the hub** (after the runner's read; UNVERIFIED, and the maintainer may overturn it). The
installed `Engine.SkelMeshComponentLODInfo`, read by the runner from the installed Windows packages (names, kinds and
script defaults, in declaration order):

| Member | Kind | Script default |
|---|---|---|
| `HiddenMaterials` | array of bool | empty |
| `bNeedsInstanceWeightUpdate` | bool | false |
| `bAlwaysUseInstanceWeights` | bool | false |
| `InstanceWeightUsage` | byte (enum `SkeletalMeshComponent.EInstanceWeightUsage`) | 0, `IWU_PartialSwap` |
| `InstanceWeightIdx` | int | **0** |

The package's own default stream for the struct holds 5 entries. The port reads that stream through a fitted layout.

- **Why it changed:** the note says a refilled entry has index -1, and the script default holds 0. So the native most
  likely builds new entries with its own constructor rather than the script default, and the earlier check correctly
  refused link 8 on installed data (the runner's JOB 10).
- **The corrected ruling:** a refilled entry is the natively constructed entry as the note states it (empty
  `HiddenMaterials`, both bools false, `InstanceWeightIdx` -1), not the struct's script default. Its
  `InstanceWeightUsage` byte is unread, the note's gap, and no value is guessed (R3).
- **The code:**
  - when `LODInfo`'s length equals the LOD count, there is no `LODInfo` write: the four writes, as before;
  - when a refill is needed, the entry struct must declare exactly the five members above by name and kind (else
    `NotYet { NotRead }`, "SetSkeletalMesh: the LOD info entry's declaration");
  - then the row refuses `NotYet { NotRead }`, "SetSkeletalMesh: the LOD info entry's usage byte (a natively
    constructed entry; not read yet)", write-free;
  - the script-default comparison and the planned fifth write are gone.
- **The tests:** an equal length gives the four writes in order with `LODInfo` kept; every refill case (link 8's 1 LOD
  against an empty `LODInfo`, 2 against 1, 0 against 1) gives that `NotRead`, write-free; a byte `InstanceWeightIdx`
  or renamed members give the declaration `NotRead`; `LODInfo` declared as ints is still a script error.
- **What remains:** the natively constructed entry's usage byte, queued to an analyst (lane F). Until it is read,
  link 8 stops at row 15's refill, now at this typed `NotRead`.

**The refill, from #687's amendment (2026-11-25).** The usage byte is no longer a gap. NATIVE_SKELMESH_MESH_CHANGE.md's
amendment (#687 at de235f7, decision 2026-11-25-22; analyst-read, UNVERIFIED in game) gives the natively constructed
entry member by member:
- `HiddenMaterials` empty;
- `bNeedsInstanceWeightUpdate` and `bAlwaysUseInstanceWeights` false;
- `InstanceWeightUsage` the byte 0 (`IWU_PartialSwap`);
- `InstanceWeightIdx` -1.

It differs from the script default only in the index.
- **The code:** when `LODInfo`'s length differs from the LOD count, the row now plans the refill as the fifth write,
  after `bHasValidBodies`: `LODInfo` = N such entries, each built explicitly by name (`lod_entry`), never the struct's
  script default. N = 0 empties it. The by-name shape check of the five members stays, as `NotRead` "the LOD info
  entry's declaration". An equal length still writes no `LODInfo`. The usage-byte refusal is removed.
- **The per-LOD loop** stays a no-op by ruling 1 (all trailing counts zero, #673's fit). So every refilled entry keeps
  -1 and 0, as the amendment says a LOD without the per-LOD data does.
- **The tests:**
  - link 8's shape, 1 LOD against an empty `LODInfo`, gives five writes in order and one entry with index -1 and
    byte 0;
  - 2 against 1 gives two such entries, and 0 against 1 empties it;
  - equal lengths give four writes with the entries kept;
  - the amendment's unit test: an entry built while the struct default holds index 0 and byte 1 still has -1 and 0.
- **Rebased** onto main 84b6d99 with #673 landed, without its earlier cherry-picks.
- **Predicted, not run:** link 8 passes row 15 and stops at its next unread step.
