## 2026-11-16: Rust definition store: collection actors, engine-native references in Off and archetypes

AI-assisted (Claude Code). Lane FIX-DEFS-COLLECTION closes three gaps H2 stage 1 found in how the definition store
reads placed level content (decision 2026-11-16-46, "Read outside the definition store, and why"). Changed:
`crates/ow-defs/src/store.rs`, new `crates/ow-defs/src/store/collection.rs` and its tests
(`crates/ow-defs/src/store/collection/tests.rs`). No `ow-package`, `ow-object` or `ow-host` file, no dependency and no
bounds check changed. There was no real-game session: every rule below is **UNVERIFIED** as a rule of the game.

- **Fix 1: collection actors.** `Engine.StaticMeshCollectionActor` and `Engine.StaticLightCollectionActor` placed in a
  level were read from the placed-actor prefix 26 (decision 2026-11-16-36's rule) and failed ("invalid FName").
  - **Prefix.** An export whose class path contains `CollectionActor` now reads from 4, unchecked, wherever it is, in
    both modes. This is the C++ rule word for word (`src/cli.cpp` `--scene-records`) and decision 2026-09-10-09's
    observation. It also covers `Engine.BlockingMeshCollectionActor`.
  - **Components.** New `DefStore::collection(w, obj)` returns a loaded collection actor's component array
    (`StaticMeshComponents` or `LightComponents`, as the store's handles) with one tail entry per element, in order.
    The layouts are the records', not new readings:
    - a mesh entry is 84 bytes: 16 floats of an affine matrix, Scale3D, a scale and an unassigned int (decision
      2026-09-10-09; the tool's checks: every float finite, entries 3, 7 and 11 within 1e-5 of 0, entry 15 within 1e-5
      of 1);
    - a light entry is 64 bytes and is handed back raw: its layout was never recovered (presumed a 4x4 transform,
      `docs/verification/SANCTUARY_SCENE_IMPORT_GAPS.md`).
    A tail that is not exactly entries times the entry size, a non-finite value or a non-affine matrix is a new
    `DefError::Collection`: no offset search, no partial result. A null element keeps its entry. A component listed
    twice is left to the caller (the tool stops its build there).
  - `BlockingMeshCollectionActor` derives from `StaticMeshActorBase`, beside `StaticMeshCollectionActor`, not from it.
    It now loads, but `collection` refuses it (`NotADefinition`): no record gives its array or tail. The prefix rule
    goes by the class's name (as the C++ does) and `collection` by inheritance; the module text says so (review r1,
    L2).
- **Fix 2: engine-native references in `ParityMode::Off`.** An object of a class no installed package exports
  (`Engine.StaticMesh`, which every `StaticMeshComponent` names) was a `NotYet` in Off, so the decoder skipped the
  property and the slot stayed `None`.
  - In Off the store now makes the handle itself, before the decoder sees the reference. It is a stand-in named after
    the object, of a class defined once at the path the export's class import (or the import) names, with no
    properties, under `Core.Object`. A code class (`Function`, `State`, `Package`, `Enum`, `ScriptStruct`, the
    property classes) is left to the decoder as before, so no class is defined at `Core.Function` and the like
    (review r1, M1). The class is marked native, so no `IsA` answer through it passes for the game's
    (the attribute resolvers' rule). Its origin is `Origin::Resource` of the object.
  - It is the store's object for the object's path (identity by path), and it stays a stand-in: it never loads, since
    its tags cannot be typed. A class something else defined at that path (an overlay, a native pack) is left to the
    decoder.
  - Why Off and not only Reference: the record is the tool, which resolves a component's `StaticMesh` reference by its
    path (`tools/prepare_level.py` `Scene.resolve`). The C++ stand-in itself stays Reference-only, unchanged.
  - `Engine.Level` is such a class too, so in Off an object inside a level now gets the level's handle as its outer
    (it had none, with a `defs: outer of` log line), as in Reference.
  - An engine-native export looked up directly (`ShadowMap1D`, `Model`, `Polys`, `WiringMesh`) is now
    `NotADefinition` ("an object of an engine-native class") in Off, as in Reference, instead of `NotYet`.
- **Fix 3: archetypes, `ParityMode::Off` only.** The C++ `instantiateExport` never follows the export table's archetype
  reference, so `ParityMode::Reference` keeps class defaults (R3). In Off a loaded definition (and
  `DefStore::instance`) now starts from its archetype's values, then takes its own tags.
  - The archetype is the path the archetype reference names, its holder chosen by the identity policy. Its values are
    its own archetype's values (or its class defaults) with its own tags from its derived prefix. A class's default
    object as archetype gives its class default object.
  - The archetype's class must be the export's or one it derives from (its slots are then the first of the export's).
  - A class default object as archetype is recognised by its name wherever it sits (review r1, L1: not only at the
    top of its package); anything inside one is a template, below.
  - The archetype's values are copied as they are. A reference to one of the archetype's own subobjects (the 17
    Sanctuary NPCs' `DialogComponent`, for one) is therefore shared by every object made from that archetype, not
    made per object as the game presumably does: **UNVERIFIED** (review r1, L4).
  - Each step of a chain is its own decode load, so the decoder's resolve budget counts per step, not per chain (at
    most 17 steps, each within the budget; review r1, L5).
  - An archetype no holder has, of an unrelated class, in a cycle, or more than 16 deep is a `NotYet` for the load,
    never class defaults in its place.
  - **Not followed: templates.** An archetype inside a class default object (the component a class's default actor
    holds, such as `Engine.Default__StaticMeshActor.StaticMeshComponent0`) is the archetype of almost every placed
    component. No record and no C++ read gives such a subobject's property prefix: from 8 the stream does not parse
    ("truncated package"), so applying them failed every component load in a first run. They are therefore not
    applied: the class default object stays the base, as before this lane, and the store logs
    `defs: archetype <path> not applied: a class default object's subobject, its prefix not recorded` once per path.
    A probe of three such Engine templates found their tags parsing only from 16 bytes in; that is an observation, not
    a recovered rule, and `PropertyPrefix` (W3-DECODE) has no 16-byte case. It is left to that owner (open item).
- **Verified (automated).** Seven `ow-defs` tests on invented packages (toy `Core`, `Engine` and a map; five from the
  first round, two from review r1), each in both modes unless named: a collection actor loads from 4 and `load_export`
  at 26 is a `PrefixMismatch`, and its mesh and light entries come back in order with a null element; a short tail, a
  non-affine matrix and a non-collection object are refused; an exported and an imported `StaticMesh` are one
  native-class handle per path that never loads (with the `defs: no holder` line); a placed component takes a two-deep
  archetype chain then its own tags in Off and class defaults in Reference, `instance` too, and a template archetype is
  logged once and not applied; a cyclic archetype and one of an unrelated class, one with no holder and a chain of 17
  are `NotYet` in Off and load in Reference, while a chain of exactly 16 loads in both (the depth boundary); a reference
  to a code object (`Core.Function`) is left to the decoder in Off; a level of an engine-native class is the outer in
  both modes. Each test was checked to fail with its fix disabled.
- **Verified (local, installed game, pc-hummer; outputs under ignored `local/p2/fix-defs-collection/` only).** A local
  driver built against `origin/main` f481008 ("before") and against f481008 with the `ow-defs` files of 79a5e1f, the
  first round ("after"): `find` of every export under `TheWorld.PersistentLevel` of the 26 `Sanctuary*` files
  (32,556 objects), in each mode. Rerun with the `ow-defs` files of 2f5ed03 (review r1: code classes left to the
  decoder, class default archetypes by name): every object's values, every collection, every `StaticMesh` slot, the
  logs and H2's placements are byte-identical to the 79a5e1f run in both modes, so the counts below hold for the
  landed code.
  - **Collection actors.** Before, in both modes: all 154 fail (118 mesh, 30 light, 6 blocking). After: all 154 load.
    `collection` reads 118 mesh collections with 10,223 components and no null, and 30 light collections with 2,197
    components (1,967 point, 226 spot, 4 light shaft). Every tail has the exact size and passes the checks. Oracles:
    the C++ `--scene-records` reads the same 118 mesh collections with 10,223 component references, none null; and in
    both kinds the entries equal the exports whose outer is a collection of that kind (10,223 and 2,197).
  - **`StaticMesh` references.** Placed `StaticMeshComponent`s: 11,195; 10,912 tag a `StaticMesh`. Before, in Off:
    0 typed slots. After, in Off: 10,905 typed, the same as Reference before and after. The 7 left are components
    of `WiringActor` and `WillowInteractiveObject` owners whose mesh is a `GearboxFramework.WiringMesh`, another
    engine-native class: the decoder skips the value in both modes because nothing says that class derives from
    `Engine.StaticMesh` (open item).
  - **Every loaded object's values, before against after** (each reference printed by its package path, so load order
    does not show).
    - Reference: the 154 collection actors now load; the other 31,786 objects are identical.
    - Off: 20,324 identical, and 11,462 with changed values. In 11,444 of them every change is a reference that was
      empty and is now typed (fix 2): `StaticMesh` on 10,905 `StaticMeshComponent`s and 334 `BlockingMeshComponent`s,
      `Brush` (an `Engine.Model`) on 92 brush components and 101 volumes, and an engine-native array on 12
      `WorldInfo`s. 5,046 objects also gain their level
      as outer (fix 2's `Engine.Level`), and the Off log falls from 18,822 lines to 2,717.
    - Fix 3 changes 18 objects: 17 placed `WillowAIPawn` NPCs, whose archetypes are their `GD_*` pawn archetypes
      (`AIClass`, `BodyClass`, `HealthPool`, `Allegiance`, `DialogComponent`, `MissionDirectives` and others now come
      from the archetype), and one `DynamicLightEnvironmentComponent`. The store logs 52 distinct template archetypes
      it does not apply.
    - Off failures: the same 616 before and after (354 `ShadowMap1D`, 127 `Model`, 127 `Polys`, 4 `WiringMesh`, all
      engine-native objects looked up directly and now `NotADefinition`, and 4 components that fail as before).
- **Fix 3 beyond level objects (review r1, M2).** The Sanctuary measurement above covers objects placed in levels. The
  Off consumers that read archetypes outside levels were checked on `origin/main` ce6f681 against ce6f681 with the
  `ow-defs` files of 2f5ed03 (the release `ow-cli` of each, `OW_PARITY` unset, so Off; outputs under ignored
  `local/`):
  - **GD_\* pawn archetypes** (the ignored check `real_pawn_archetypes`, every census path of the game root whose first
    holder derives from `Engine.Pawn`, both modes): 946 paths, 42 loaded at 26 and 904 at 4 in both modes, before and
    after; the per-path files are identical. `GD_Marcus.Character.Pawn_Marcus` (read at 4, decision 36),
    `GD_TargetDummy.Character.Pawn_TargetDummy` and `GD_PrimalBeast.Character.Pawn_PrimalBeast` load.
  - **`--slice-run`** (the Fire mission, its director made with `instance` and the target dummy pawn; both of
    `tools/verify_fire_event_order.py`'s step lists), **`--mission-run`** (the Fire mission's steps of
    `tools/audio_slice_chain.py`) and **`--combat-run`** (`spawn:dummy|regular|adult|ranged|baby:8`, `tick:1`):
    standard output and error byte-identical before and after (the combat output differs only in `elapsed_ms`).
  - **The behavior kernel fixtures:** `tests/behavior_test.py` passes in both modes with this lane's binary, and the
    workspace tests pass.
  - **Not reached.** Every one of those runs stops on main at a gap of its own, the same before and after: the slice
    at `Behavior_AIHold` on the target dummy (not ported), the mission run at the lent weapon's region-stage
    condition (`NotYet`), and every combat spawn at a reference to `Default__WillowAIPawn.SpecialMoveComponent0`
    ("inside a class default object"). The steps after those gaps, where a spawned or placed pawn's
    archetype-derived values would be used, were therefore not exercised: **open**, to be rerun when they are.
- **H2's scene** (`ow-host` `Scene::load` on `Sanctuary_P`, Off, unchanged code, driven from the local driver), on
  `origin/main` f481008 and again on 7f73940 (after H2-DRAW), each against this lane's 79a5e1f, and with 2f5ed03's
  `ow-defs` on f481008. Before and after, on all: 10 levels, 4,446 placements, 305 hidden, 4,141 drawn, 143 excluded
  (122 with no mesh, 16 mission previews, 5 duplicates), 430 meshes (418 drawn), LOD 0 triangles 1,348,542 and 1,280,100
  drawn, the same gaps. The placement and exclusion lists are byte-identical, poses and flags included. This lane
  changes none of them, for three reasons:
  - the scene reads collection actors itself (its tags from 4 and the tail);
  - it reads `StaticMesh` from the component's own tags;
  - every placed `StaticMeshComponent`'s archetype is a template (or none), which fix 3 does not apply, and no placement
    owner (`StaticMeshActor`, `InterpActor`, `WiringActor`, `WillowInteractiveObject`) has another kind of archetype in
    Sanctuary.
  The scene can now switch to `DefStore::collection` and the store's `StaticMesh` slot; that is `ow-host`'s change, not
  made here. Against `tools/prepare_level.py`, rerun on the same install with the Rust reader (whose commands do not go
  through `ow-defs`): 4,446 placements, 21 excluded, 433 meshes, as decision 2026-11-16-46 recorded, and its placement
  set equals the scene's (4,446 in both, none in one only). Against the benchmark record (4,470 placements, 4,163
  drawn): the 24-placement difference decision 2026-11-16-46 could not explain is unchanged, since neither the scene nor
  the tool moved.
- **Gates** (pc-hummer; the first round on `origin/main` 7f73940, review r1 at 2f5ed03): `cargo build --workspace
  --locked` clean; `cargo test --workspace --locked` 1,600 passed, 0 failed, 21 ignored at 2f5ed03 (1,598 in the first
  round); `cargo build --release -p ow-cli --locked` clean;
  `tests/crate_edges_test.py` OK; `tools/decisions_index.py --check --baseline origin/main` OK, 1 added;
  `tools/verify_packages.py --reader target/release/ow-package.exe`: 9 packages, decoded bytes, counts and export fields
  match, 3,339/3,339 class bodies exact; ctest 173/173 with the release binary present.
- **UNVERIFIED.** That collection actors' tags start at 4 and their tails hold what the records say (the light entry
  is not decoded at all); that an engine-native object's identity is its path; archetype inheritance as the game
  applies it, the order (archetype, then own tags) and the template gap.
