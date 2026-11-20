## 2026-11-19: Maya's skeletal mesh and the skeleton source as slice harness inputs

AI-assisted (Claude Code, Opus 5.5). Lane SLICE-HARNESS-MAYA-MESH, from the hub's ruling: "The slice world's set-up
preloads Maya's skeletal mesh and installs ow-cli's skeleton source, each logged as an UNVERIFIED harness input (in
the game her mesh is loaded with her pawn)." The decision number is pending from the hub (three digits); NNN is a
placeholder.

- **Base:** fix/rust-per-bone-weights at 9dfff984 (#585 r1).
- **No real-game session.** Both inputs are **UNVERIFIED** stand-ins. Which exports the game's streaming load makes
  resident is not read, and the engine reads its skeleton from its own mesh data, not from an export file.
- **Unchanged:** the natives, the skeletal mesh data layer (#569) and Reference. No dependency was added and no bounds
  check was loosened. No native code or analysis output was opened.

**What changed** (`crates/ow-cli/src/cmd/combat.rs`, harness only):
- In `apply_class_inputs`, the `maya` step's Off inputs, after the archetype preload and before the class select:
  1. **The skeleton source.** `install_skeleton_source` installs `skeletal_source::PskExports` over the export roots
     as `ow_world::world::skeletal::SkeletalMeshes`, once per VM, and logs "harness input: the skeleton source over
     UModel .psk exports in <roots> ...".
     - The roots are `OPENWILLOW_PSK_EXPORTS` (`;`-separated), else `local/external/psk` under the repository root
       ow-cli was built from (review r1 of #592, item 1). Each root is logged as an absolute path, with "(missing)"
       when it is not a folder; the install goes ahead either way.
     - The install sits in the `maya` step, not in `world()`: the slice gate lists only the harness lines a link's own
       steps log, so a set-up line would not show in the link's record (rule 7).
  2. **The mesh.** `preload_mesh` loads `Char_Siren.Mesh.Skel_SirenBody` through the definition store (`find`, then
     `ensure_loaded`). It logs the attempt first, then "loaded" or "found nothing". A failed load is the step's error.
- `crates/ow-cli/src/cmd/skeletal_source.rs`: its `#![allow(dead_code)]` is gone, now that a driver installs it.

**The export** (game-derived, under ignored `local/` only, never tracked):
- It was made on pc-hummer with UModel build 1590, through the project wrapper, against the installed game:
  `tools/export_with_umodel.ps1 -UModel "<UModel 1590>/umodel.exe" -Cooked "<install>/WillowGame/CookedPCConsole" -Package GD_Siren_Streaming_SF -Object Skel_SirenBody -MeshFormat psk`
  - Result: exit 0, 0.85 s, 11 files, 6,826,949 bytes, written to `local/external/umodel/GD_Siren_Streaming_SF-<stamp>/`.
- UModel lays its output out by the object's own package (`Char_Siren/SkeletalMesh3/`). `PskExports` looks under the
  file package (`<root>/GD_Siren_Streaming_SF/SkeletalMesh3/Skel_SirenBody.psk`, #569), so the `.psk` was copied there
  under `local/external/psk/`. A fresh machine repeats both steps; nothing is distributed.

**Tests** (`combat/mesh_harness_tests.rs`, invented `WillowGame` package, AGENTS.md rule 9):
- the preload loads through the store (loaded, not a handle) and logs the attempt, then the outcome;
- a path no package holds is logged "found nothing" and skipped; an empty path is an error;
- the source is installed once, logged with its roots (single-spaced); a second install is refused;
- the roots come from the variable, else the default.

`combat/harness_tests.rs` now expects the source line and the two mesh lines before the class select, and asserts
that Reference installs no source.

**The installed Off `--slice-gate`** (strict, under the machine lock, pc-hummer), on a throwaway local merge with #570
(2a7cad3f), #500 (c1ed82a9) and #528 (0e8e2312), deleted after:
- ok 1, NotYet 10, error 3 of 14;
- link 1 logs the source line, then "preload Char_Siren.Mesh.Skel_SirenBody: loaded". It runs 0 of its driver's 1 step
  (`maya:8`). Its first stop moved from #569's "the unloaded skeletal mesh Skel_SirenBody" (`LayerAbsent`) to "the
  animation tree initialisation's pass over the component" (`NotRead`, NATIVE_COPY_ANIM_TREE.md section 2 step 5), the
  next unread step. Links 3, 4, 5, 13 and 14 are blocked behind it, with the same first stop.
- **Merge note for landing:** #570 adds its tree preload at the same line of `apply_class_inputs`. On the probe the
  order is the archetype preload (two lines), the tree preload (two), the source (one), the mesh preload (two), then
  the class select (one); `harness_tests.rs` then expects eight lines (review r1 of #592, item 2).
