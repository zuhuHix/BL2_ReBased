## 2026-11-16: Rust host scene loading stage 1: Sanctuary as plain data

AI-assisted (Claude Code). Lane H2-SCENE, host track (architecture section 8, "Wave 7 and the host track"), stage 1.
It adds `crates/ow-host/src/scene.rs` and its tests (`crates/ow-host/src/scene/tests.rs`), plus one `mod` line in
`crates/ow-host/src/lib.rs`. `Scene::load(defs, space, assets, map)` builds a map's scene as engine-agnostic data in
the game's units (uu, rotator units, Z-up) and draws nothing. No dependency, no external tool, no bounds check and no
decoder changed. There was no real-game session, so the scene is **UNVERIFIED** as what the game shows. The reference
it was checked against is `tools/prepare_level.py`, the tool that built the manifest the engine benchmark drew.

- **What the scene holds.**
  - Levels: the persistent level, then each `LevelStreaming*` `PackageName`, breadth first, as the tool walks them.
  - Every static mesh placement, keyed by (level, export index) in export order (`OrderedMap`). Each carries:
    - the mesh's object path;
    - its pose as serialized, the actor's and the component's kept apart, or a collection's tail matrix;
    - its hidden flags: component `HiddenGame` and owner `bHidden`;
    - its material overrides.
  - The excluded components, each with its reason.
  - The meshes, decoded by `ow_assets::mesh::read_mesh`.
  - Every material the placements and mesh sections name, with the textures it names. Each texture entry keeps
    the `ParameterName` that named it and whether an instance parameter or a base-material expression did, so a
    consumer can pick a channel by parameter name as the tool's `CHANNELS` table does.
  - The textures. They are only located at load; `Scene::decode_texture` decodes one on demand.
  - The player starts.
  - The light components, by class and count only.
  - The terrain actors, and each level's BSP model counts.
  - A typed `NotYet` gap for each part nothing decodes yet: terrain, BSP, texture classes other than `Texture2D`, and
    meshes or textures no installed package holds.
  - Composing the poses into world transforms is left to the consumer (H3's `frame.rs`, then stage 2).
  - A load that cannot proceed is a `SceneError` (the scene's own error; `ow-host` has no crate-wide one), with
    store `NotYet`s and heap errors as variants of their own. The base classes the scene sorts exports by
    (`LevelStreaming`, `PlayerStart`, `LightComponent`, `Terrain`, `Material`, `MaterialInstance`) are required: a
    failed lookup is an error, never an empty result. So is a level-streaming object whose `PackageName` does not
    read.
- **Which components are placements.** The tool's rules, so the two compare component by component:
  - The class is exactly `Engine.StaticMeshComponent` and the path in its file starts `TheWorld.PersistentLevel.`.
  - A component with no `StaticMesh` reference is not placed.
  - The pose comes from the component's collection actor, if one lists it. Otherwise it comes from its owner, when the
    owner is a `StaticMeshActor`, `InterpActor`, `WiringActor` or `WillowInteractiveObject`.
  - A mission pickup spawner's or failsafe's `PreviewMesh` is excluded.
  - An interactive object's second identical component is excluded.
  - Any other owner is excluded as unsupported.
  - None of the old UE5 host's helper-mesh hiding rules (`hidden_visual_mesh`) is applied: the scene keeps the
    serialized flags only.
- **Read outside the definition store, and why.** Each is a gap in a lower layer, worked around here and named for its
  owner. None needs a guessed value.
  - **`StaticMeshCollectionActor` loads from the wrong prefix.** The prefix comes from `ow_defs`
    `Loader::derive_prefix`, under decision 2026-11-16-36's rule: an `Engine.Actor` export whose outer is a level
    reads from 26, unchecked. This class fails there ("invalid FName"). Its tags start at 4: decision 2026-09-10-09,
    and the per-class prefix table of the C++ and Rust `--scene-records`. `StaticLightCollectionActor` fails the same
    way. The fix belongs to the store's owner.
    - The scene walks the collection's tags with `ow_package::props::read_tagged` from 4, typed by
      `ow_object::decode::ReflectionTyper`.
    - It reads the tail as decision 2026-09-10-09 records it: exactly 84 bytes per component, 16 floats of an affine
      matrix, Scale3D, a scale and an int whose meaning is not assigned. The values must be finite and the matrix
      affine, as in the tool.
    - A collection that fails these checks is a gap, and its components are excluded.
    - A null entry is checked and its 84 bytes dropped, as the tool zips the two arrays and tolerates it.
    - A component two entries list is a gap and is excluded. The tool stops the whole build there ("Duplicate
      collection component", `prepare_level.py` `Scene.build`).
    - The scale rule is decision 2026-09-18-04's: the component's own `Scale3D` and `Scale` when its tags set either.
  - **The `StaticMesh` slot is always empty.** `Engine.StaticMesh` is an engine-native class that no installed package
    exports. In `ParityMode::Off`, the decoder skips a reference to an object of such a class ("defaults: skipped
    StaticMeshComponent.StaticMesh"), so the store's slot is always `None`. The scene therefore reads the reference
    from the component's own tags (from 8), as a package reference. The fix belongs to `ow_object::decode`'s owner.
    - The decoder's approved route was not taken: defining a layout for `Engine.StaticMesh` in the `ClassTable` before
      any decode (`decode.rs`, the engine-native class paragraph). The slot would then hold the store's handle, which
      resolves the path through the store's identity policy: `FirstInScanOrder`, the first holder in scan order,
      possibly another map's copy, and slowly (each new path is a global-index lookup). The tool resolves the
      component's own reference instead, and so does the scene.
    - An export reference is itself.
    - An import is looked up in the scene's levels, in load order: every export at the import's path (folded) is
      tried, and the first whose class has the import's class name wins. This is the tool's `Scene.resolve`
      (`prepare_level.py`), narrowed in two ways. The tool searches every package it has read so far: `Startup` first,
      then the levels, then whatever earlier `--resolve` calls loaded. The scene searches the levels only. And the tool
      accepts any class when the import names none; the scene requires the class name. What is not found goes to
      `PackageStore::resolve`, the C++ resolver the tool calls through `--resolve`. UNVERIFIED as the game's lookup.
    - Searching the levels first was needed for time: through the store's global fallback, the 894 mesh imports of
      Sanctuary's sublevels took 31 s.
  - **`PreviewMesh` is dropped by the decoder.** The cooked spawner class does not declare it (it is editor-only), so
    the decoder drops the tag. The scene reads it from the owner's own tags (from 26), as the tool does.
  - **Archetype values are not applied.** The store applies class defaults, not a component's archetype. 122 Sanctuary
    components tag no `StaticMesh`, and the tool skips the same ones. Whether the game draws an archetype's mesh for
    them is UNVERIFIED.
  - Materials and textures go through the store: `resolve_ref` for a section's material, then `ensure_loaded` along
    the `Parent` chain.
    - A material's textures are the `ParameterValue`s of the chain's `TextureParameterValues`, plus each base-material
      expression's `Texture`. Material function calls are not followed.
    - Their holder is the store's `FirstInScanOrder` choice, so it may be another map's copy of the same path.
    - A section's effective material is the override at its slot (`Materials[i]` for section `i`), else the mesh's.
      The tool differs for a mesh with a triangle-less section: it drops such sections, then indexes `Materials` by
      the position in the shortened list. No Sanctuary mesh has such a section, so no placement differs here.
- **Verified (automated).** Eleven `ow-host` tests on invented packages, built with the testkit: toy `Core`, `Engine`
  and `WillowGame` code packages, and two map packages. They cover:
  - the placement and exclusion set, and the streamed level;
  - attached poses with their class defaults, and both collection scale sources;
  - the hidden filter, and triangle sums;
  - a mesh no file holds giving `NotYet`;
  - an import resolved in a loaded level;
  - material chains naming their textures, and a G8 texture decoded on demand;
  - a `TextureCube` giving `NotYet`;
  - the light, terrain and BSP census and its gaps;
  - a short, a non-finite and a non-affine collection tail, each giving a gap and exclusions, not a pose;
  - a component two collection entries list (a gap, excluded), a null entry, and a collection's own `bHidden`;
  - a spawner whose `PreviewMesh` names another component (an unsupported owner), and a component that does not
    load;
  - two interactive-object components that differ in one flag of `DUPLICATE_FLAGS` (both placed);
  - two loads of the same packages giving an equal `Scene`;
  - a missing map giving an error.
  The ignored driver `sanctuary_on_the_installed_game` reads `OW_BL2_ROOT` and writes only under ignored
  `local/p2/h2-scene/`.
- **Verified (local, installed game, pc-hummer, release build of main 32fc9ae plus this lane; outputs under ignored
  `local/` only).**
  - **The scene.** 10 levels, 4,446 placements. 305 are hidden: 277 by `HiddenGame` and 29 by owner `bHidden`, one
    placement having both. 143 are excluded: 122 with no mesh, 16 mission previews, 5 duplicates. No collection
    lists a component twice or holds a null entry.
  - **Meshes and triangles.** 430 distinct meshes, all decoded; 418 of them are drawn. LOD 0 triangles: 1,348,542
    over every placement and 1,280,100 over the drawn ones.
  - **Materials and textures.** 381 materials, all read. 285 textures are named (280 by the placements' effective
    materials, 276 by the drawn ones). 283 decode. 2 are `TextureCube`, `NotYet`, and both are in the drawn set.
  - **The rest.** One player start (`WillowCoopPlayerStart_0`). Lights: 962 `PointLightComponent` and 61
    `SpotLightComponent`, all in collections; 1 each of `DominantDirectionalLightComponent`, `SkyLightComponent` and
    `LightShaftComponent`. 8 terrain actors and 10 levels with BSP models, all `NotYet`.
  - **Time.** The load takes 24.6 s, most of it the store's first holder lookups. Decoding all 283 textures takes
    1.8 s.
  - **Against `prepare_level.py`, rerun on the same install with the Rust reader.** The tool gives 4,446 placements,
    and they are the same set. For every placement, the mesh and the source-hidden flag agree, and the pose agrees
    exactly: actor locations, component translations and collection matrices as identical f32 bit patterns, and
    rotators as identical ints (the tool's degrees inverted, each an exact integer). Only the scales the tool derives
    (Scale3D times scale, in f64) were compared with a tolerance (1e-6 relative); all agree. The one mesh-name
    difference is the same object named two ways: the scene gives the level-local `WiringMesh_14` its full path.
    Every mesh's LOD 0 triangle count equals the tool's OBJ face count. Excluded: 16 and 5, the same.
  - **The tool's 307 hidden against the scene's 305.** The tool draws 13 hidden-flagged `InterpActor` placements (its
    mover exception, decision 2026-09-23-04). It hides 15 unflagged helpers by mesh rule: 6 `Blocking_Plane`, 3
    `CollisionCube`, 3 `BoxLrg`, 2 `BasePlane_256x128` and 1 `Blocking_Cube`.
  - **The tool's 433 meshes against the scene's 430.** The tool keys meshes by holder file, and three paths are held
    by two files: `SancBuildingGroup01`, `BasePlane_256x128` and `RoadPillar`.
- **Against the benchmark record** (`docs/verification/ENGINE_BENCHMARK.md`: 4,470 placements, 4,163 drawn,
  1,280,463 triangles, 270 textures).
  - **Placements.** Today's tool rerun gives 4,446, not 4,470. Inferred from the record's numbers alone, not from
    B2's manifest: if B2 hid the same 307 as today's rerun (4,470 − 307 = 4,163 drawn, against 4,446 − 307 = 4,139),
    its manifest held 24 more placements, all drawn. That manifest is under `local/` on the zuhu desktop and could not
    be inspected here, so the 24 are not explained.
  - **Triangles.** Over the tool's drawn set, the scene's counts give 1,280,556; over its own drawn set, 1,280,100.
    B2's 1,280,463 presumably comes from a manifest that differs from today's rerun (an inference, not checked).
  - **Textures.** 270 counts something else: the images B2 loaded from the tool's material-v1 channel picks. On
    today's rerun those picks come to 255 diffuse images (290 across the four channels) for the drawn set. The scene
    counts texture objects instead: 276 for the drawn set.
- **Not verified.** Anything in the running game:
  - which placements the game draws (hidden flags under Kismet, Matinee or mission state, archetype meshes);
  - the level-first import lookup;
  - which textures a material really samples.
  Nothing is drawn yet (stage 2 waits for H1).
- **Stage 2** (drawing the scene in H1's window) adds to this file.
