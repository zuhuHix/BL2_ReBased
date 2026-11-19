## 2026-11-19: The skeletal mesh data layer, read only

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-SKELETAL-MESH-LAYER. This layer serves the typed refusals that
wait on "a skeletal mesh data reader (sockets and the reference skeleton)":
- #529's socket attach (NATIVE_INITIALIZE_BODY_COMPOSITION.md);
- the per-bone weight builds of Maya's per-bone and feather blend nodes (#566, NATIVE_ANIM_NODE_INIT.md).

No native code was read. Synthetic fixtures only.

- **What it answers (`ow-world` `world::skeletal`, new):**
  - `reference_skeleton(vm, mesh)`: a loaded `SkeletalMesh` export's bones in order, each with its name and its
    parent's index (`None` for the root);
  - `sockets(vm, mesh)`: its `Sockets`, in order, each with its name, bone name, relative location, rotation (pitch,
    yaw, roll) and scale.
- **What each consumer reads:**
  - **#529:** the parent mesh's sockets (name, bone, offset), matched first, then the bone names of its skeleton. The
    `Parent` link's bone map (its ruling 3) would read both meshes' bone names;
  - **#566:** the skeleton's bone count, each bone's parent, and the bone names its branch names are looked up as.
- **Where the data comes from:**
  - **No native decode.** The project does not decode `SkeletalMesh`'s native serialization: its layout has no public
    spec and none is recovered (rule 4; ow-assets `skeletal`). No offset is guessed.
  - **The skeleton comes from a `SkeletonSource` the harness installs** (`SkeletalMeshes::install`). The source in
    `ow-cli` (`cmd::skeletal_source::PskExports`) reads UModel's ActorX `.psk` exports of the player's own install
    with ow-assets' reader, which checks every count, index and parent order. The file is
    `<root>/<file package>/SkeletalMesh3/<object name>.psk`, UModel's layout as decision 2026-11-16-53 used it.
  - **The sockets** are the mesh's tagged `Sockets` property, read from the definition store's objects.
- **The crate edge, the smallest allowed route (the hub's ruling):** `crate_edges_test.py` does not let `ow-world`
  depend on `ow-assets`. So the world owns the trait and plain data types, and `ow-cli` (which may depend on both)
  implements it with ow-assets' reader. No new crate and no new dependency.
- **The rules:**
  - **write-free:** no heap write, no generator draw, no load. Reading an export file is a parse, not a load. An
    object the store has not loaded (a handle, or a slotless stand-in whose class has slots) is
    `NotYet { LayerAbsent }`, the store's load. The layer never loads it;
  - **deterministic and cached:** the skeleton is cached per mesh object in a `DetMap`, so a second ask parses
    nothing;
  - **typed errors:**
    - no source installed, no export for the mesh, or a mesh with no package export: `NotYet { LayerAbsent }` with
      the layer text #529 names;
    - a parse failure, a skeleton out of parent order or empty, or a receiver that is not a `SkeletalMesh`:
      `VmError::Script`;
    - a None `Sockets` entry: `NotYet { NotRead }`;
  - Off-facing; nothing binds a native, so Reference is unchanged.
- **Wiring:** the API is public and the consumers' rules are not changed. #529's refusal site (`body_composition.rs`
  on its branch) and #566's implementer call `reference_skeleton` and `sockets` when they switch over, and their
  drivers install the source then. No driver installs it here, so the slice gate is unchanged.
- **Tests:**
  - `ow-world` `skeletal_tests.rs` (5):
    - no source gives the named layer;
    - the source's skeleton is cached, a second ask reads nothing, and nothing is written;
    - a mesh with no data gives the named layer;
    - a parse failure, an out-of-order and a rootless skeleton, and a non-mesh are script errors;
    - an unloaded socket subobject is refused with nothing loaded, then read after a load;
    - sockets on hand-defined classes are read in order, with their values; a None entry is `NotRead`, and a
      stand-in is `LayerAbsent`.
  - `ow-cli`: an ignored installed probe.
- **Probe (installed data, counts only, under ignored `local/analysis/skel/`):**
  - UModel 1590 exported Maya's body mesh (`GD_Siren_Streaming_SF`, `Skel_SirenBody`) as ActorX.
  - The layer parsed its reference skeleton: **27 bones, parse pass**, the count decision 2026-11-16-53 recorded.
  - Its 13 socket objects are unloaded handles after the mesh's load, so `sockets` refuses them `LayerAbsent` and
    loads nothing.
  - After a harness load of the 13, all 13 read, and all 13 name a bone of the parsed skeleton. So #529's consumer
    needs the socket objects loaded as a harness input, or a ruling.
- **Checks:** `cargo test --workspace -j 2` 2,449 passed, 0 failed, 47 ignored; clippy shows nothing new; crate edges
  OK.
- **Review r1 of #569:**
  - **path safety:** the harness source takes the package and the leaf only as single plain file-name components (no
    separator, `.` or `..`, colon or NUL), and refuses a file that resolves, links and junctions followed, outside
    the canonical export root;
  - **same-named meshes:** a mesh whose package holds another mesh export of the same name (in another group) is
    refused `LayerAbsent` before the source is asked. The flat export layout cannot tell the two apart, so no two
    cache entries can hold one file's bones;
  - **duplicate bone names** under ASCII case folding are a script error before caching. The source's spelling and
    order are kept;
  - **the file lookup** uses the export entry's own spelling, not the name table's first one;
  - **tests are strict:**
    - the socket fixture must arrive unloaded;
    - the installed probe asserts the unloaded-socket refusal and that every socket names a skeleton bone;
    - new synthetic tests: name traversal, a junction or link out of the root, a missing export versus a malformed
      ActorX file, the two same-named meshes, the spelling, and duplicate names;
  - checks: `cargo test --workspace -j 2` 2,454 passed, 0 failed, 47 ignored. The probe is unchanged: 27 bones; 13
    sockets refused while unloaded, all 13 on a skeleton bone after the harness load.
- **Review r2 of #569:**
  - **twins by class identity and ancestry:** the other export's class is looked up by its package-qualified path in
    the loaded class table (read only, no load). It is a twin when that class is `SkeletalMesh` or a subclass. A class
    the table does not hold cannot be settled without a load, so it is refused `LayerAbsent` too, naming the class. An
    unrelated class that shares the leaf name `SkeletalMesh` no longer causes a false refusal once it is known;
  - **tests:** a mesh and a mesh-subclass export of one name are both refused with zero source calls; an unrelated
    same-leaf class refuses while it is unloaded and answers once it is loaded; the link-escape test fails when its
    link cannot be made; the installed probe asserts the store-load layer and the unloaded-socket text;
  - checks: `cargo test --workspace -j 2` 2,455 passed, 0 failed, 47 ignored; the probe is unchanged.
- **UNVERIFIED.** The layer reports what the data holds. That the engine's bone order and parents match UModel's
  export rests on decision 2026-11-16-53's cross-tool bind-pose check, not on the running game.
