## 2026-11-19: SetSkeletalMesh's mesh change in Off

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-SKELMESH-MESH-CHANGE, branch
`fix/rust-skelmesh-mesh-change-b6d8` from main 241a6c47. It is coded from docs/verification/NATIVE_SKELMESH_MESH_CHANGE.md
(#635, on main) and #599's code alone (clean room: lane ANALYST-SKELMESH-MESH-CHANGE read the native, this lane did
not). It was written apart from the earlier `fix/rust-skelmesh-mesh-change` branch, whose content this lane did not
read (the hub's ruling). No real-game session was run. **Every rule is UNVERIFIED in the running game.** Off only;
Reference is unchanged and binds nothing.

**What changed.** `ow-world` `skeletal_mesh.rs`: #599's single mesh-change `LayerAbsent` is replaced by the note's
section 4 rows, in its order:
- **A, #599's rows, unchanged:** the receiver, the arguments, the current mesh, and equal meshes as a no-op;
- **B, the meshes:** each set mesh must be resident, the old one first, or the answer is `NotRead`. An unloaded, a
  failed or a slotless stand-in handle refuses alike, and a clear skips the new mesh's row;
- **C, the unported paths (rows 7 to 15):** a non-empty buffer kept, a physics instance, an attached component, an
  animation tree, the transform, a parent component, a physics asset, morph sets or a non-None index map, and the LOD
  row;
- **D, the four destinations** checked;
- then **four writes:** `bAnimTreeInitialised` false, `SkeletalMesh`, `SpaceBases` emptied, `bHasValidBodies` false.

Every row reads only: no write, no load, no draw, and no name interned (lookups through `names.find`).

**The hub's rulings and the port's choices:**
- **The transform** (row 11, the hub's ruling): `LayerAbsent` when either mesh has a non-zero `Origin` or `RotOrigin`.
  When both are zero, nothing is answered for the transform (UNVERIFIED). The members are checked by name and kind.
- **Four writes, not five.** The hub's brief said "five writes in the minimal case". The note's section 4 (on main) lists
  four, and leaves the transform, step 12's field and step 17's notification unwritten. This lane follows the note.
  **For the hub.**
- **The LOD count** (row 15) comes from a new, read-only `SkeletonSource::lod_count` in the skeletal mesh data layer.
  Its default, and so every installed source's answer today, is "no answer", so row 15 refuses `LayerAbsent` on the
  installed game. `ow-cli`'s source is unchanged. A mesh the layer cannot place in a package export also gets no answer.
- **A slotless stand-in mesh** counts as not resident (rows 5 and 6). The note names unloaded and failed handles; the
  stand-in case is the port's, UNVERIFIED.

**Tests** (`skeletal_mesh_change_tests.rs`, 10, on invented packages through the testkit; #599's tests kept, its
mesh-change test replaced): the note's tests 1 to 11, each refusal with #518's counters, the name table's length and
the component's slots unchanged.

**Gates.**
- `cargo test --workspace -j 2`: 3,209 passed / 0 failed / 51 ignored. `crate_edges_test.py` OK. Clippy has no hits in
  the touched files.
- Installed Off strict `--slice-gate` (release, under the machine lock): 4 ok / 9 NotYet / 1 error, the same counts as
  main 241a6c47, run the same way. The one error is link 9's unbound `GearboxMind.GetAIComponent()`, which main has too.
- Link 8 passes `make` (1 of 5). It stops in `init` at row 6, "stopped after partial writes: SetSkeletalMesh: the new
  mesh is an unloaded handle (a load is a write)" (`NotRead`), as the note's section 5 predicts. On main it stopped at
  #599's "a mesh change" `LayerAbsent`.
- This is what the host did, not parity.
