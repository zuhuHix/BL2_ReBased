## 2026-11-25: A package found by name takes the requested spelling when its file name is lower-cased (DRAFT)

AI-assisted (Claude Code)

Lane LINUX-CAUSE-A, with Claude Opus 5.5. **Draft:** the maintainer has not chosen between the two options below
yet; the hub recommended option 1, and this branch implements it so it can be measured. Stacked on #675
(fix/linux-install-paths). It touches the sensitive `crates/ow-package` (`package.rs`, `store.rs`); no bounds,
`limit`, `require`, size or terminator check changes, and no `ow-container` change.

- **Why:** a package is named after its file stem (`Package::load`). Aspyr's Linux build lower-cases every file name,
  so the package part of every object and class path came out lower case (`willowgame.MissionDefinition`,
  `engine.Texture2D`). Exact compares then failed: "not a MissionDefinition" on links 1, 3 to 5 and 10 to 14 of the
  installed slice gate, "not a behavior provider" on link 8, and 201 of the host's Sanctuary textures as "a texture
  class ow-assets does not read". Finding the files was already case-insensitive (the store's folded name index).
- **Option 1 (this branch):**
  - `Package::load_as(path, name)` is `Package::load` with the given name instead of the stem: the same read, decode
    and checks, and `Package::load` now calls it with the stem;
  - `PackageStore::load_path_named(path, name)`, which `load_named` now ends in: the stem, as `load_path` gives it,
    unless the stem has no upper-case letter and `name` is the same name spelled otherwise, in which case the
    package is named `name`. The name comes from the import or code constant that asked, which carries the game's
    spelling. A stem with an upper-case letter (every Windows install's) is kept whatever spelling asks, so Windows
    is unchanged. `load_path` is unchanged and keeps the stem;
  - callers that opened a package by path where the name matters now pass the game's spelling: the slice gate's Fire
    source names its package from `FIRE_SOURCE`, and the host's scene names its persistent level as the map was
    asked (`Scene::load_at` through `load_path_named`).
- **UNVERIFIED and open:**
  - The first load decides a package's name, and a later request returns the package already loaded. That the first
    requester spells it as the game does is not proven. Most requests are imports, whose names come from the game's
    own name tables.
  - A package first opened by `load_path` (a CLI argument, the definition store's holder files) keeps its lower-case
    stem.
  - A Windows install whose file stem is entirely lower case would take the requested spelling too; whether BL2 ships
    such a file is not counted.
- **Option 2, the long-term model:** UE3 compares names without case (FName semantics), so the port's string compares
  of object and class paths should compare without ASCII case, and a package's name should not depend on its file
  name at all. That is a broader change across the crates' compares. Option 1 does not close it.
- **Also on this branch:** the skeleton source (`ow-cli` `skeletal_source.rs`) finds `<file package>/SkeletalMesh3/
  <leaf>.psk` through `ow_core::install::resolve`. The file package there is the holder file's lower-cased stem, while
  UModel's export folder is mixed case. Its path-safety checks are unchanged.
- **Measured** (zuhuPC, Linux, release builds under the machine lock, Aspyr install, `OPENWILLOW_PSK_EXPORTS` set to
  the main checkout's export folder): installed Off strict `--slice-gate` 4 ok / 9 NotYet / 1 error.
  - Links 2 and 10 to 12 are ok. Links 1, 3 to 5, 13 and 14 stop at hook 1's unresolved gear-likeness interface
    (main has no #665 set-up). Links 6, 7 and 9 stop at the stops recorded on pc-hummer.
  - Link 8 is an error that is not about case: Aspyr's `Engine.SkeletalMeshComponent.GestaltDataContainer` declares
    other members than the gestalt data layer reads. `PartMeshNames` is not among them; it has `SkeletalMesh`,
    `LODModelData`, `SocketMappings`, `GestaltAccessoryList`, `GestaltPartMatricesContainer` and
    `ReferencePoseBounds`. The Linux build's packages differ there from the Windows build that
    NATIVE_GESTALT_DATA_LAYER.md
    was read on; that layer is not changed here.
  - Main on pc-hummer's Windows install (d3c7a13) read 4 ok / 10 NotYet / 0 error, so 13 of the 14 links give the same
    kind of answer here. Whether their first stops are identical link by link was compared against the GATES rows
    only, since pc-hummer's per-link log is not on this machine.
- **Not run:** Windows, which only the new tests cover (the exact-case stem keeps the name `load_path` gave), and the
C++
  reader, which is unchanged.
