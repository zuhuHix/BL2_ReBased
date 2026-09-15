# BL2_ReBased roadmap

The live tracker. Phases, steps, gates and estimates come from
[docs/OPENWILLOW_ENGINE_PLAN.md](docs/OPENWILLOW_ENGINE_PLAN.md); this file
records where each step actually stands, with the verification record that
backs it. Checkbox states are conservative: a step is checked only when its
verification is written down.

Legend: <img src=".github/assets/icons/done.svg" width="18" align="absmiddle" alt=""> done and verified · <img src=".github/assets/icons/now.svg" width="18" align="absmiddle" alt=""> in progress · <img src=".github/assets/icons/todo.svg" width="18" align="absmiddle" alt=""> not started.
Items marked *Caveat:* are done with a recorded limitation.

**Phase 0 gate:** 2026-09-10 · **Now:** Phase 1 · **Last update:** 2026-09-15

---

## Now / next

The concrete open items, roughly in the order they are being taken. Small,
well-bounded ones are marked *good first task*.

- [x] Decode `PF_A8R8G8B8` textures: synthetic pixel/bulk tests pass and the
      real 256x256 Ash sky transition texture extracts successfully. Native sky
      shading remains open. See [verification](docs/verification/A8R8G8B8_TEXTURE.md).
- [x] Diagnose the mirrored Sanctuary shop sign: the host adapter reversed
      winding twice, exposing back faces. Corrected isolated sign renders
      readable; saved UV and winding checks now guard the import path.
- [~] Sky rendering: the bounded Sanctuary slice now imports the accepted
      `Prop_Skybox.Meshes.Sky_Dome` placement with its Unlit material and
      recovered `Sky_TransitionBL2Default_Dif` routed through the host
      Emissive policy. The blue shell and `OpenWillow_SkyAtmosphere` remain
      temporary fallbacks; Ash coverage, outer layers, time-of-day/cloud/mask
      graph connections, Kismet activation and visual parity remain open. See
      the [native skybox record](docs/verification/NATIVE_SKYBOX_VERIFICATION.md)
      and [sky census](docs/verification/SKY_CENSUS.md).
- [ ] Material fallbacks: Sanctuary now has 44 materials lacking diffuse,
      including 12 opaque definitions (42 placed sections). Of those, nine have
      no supported channels (20 sections). Two glacier materials (33 sections)
      have an explicit primary-layer approximation with instance tiling; snow
      blend, reflection/glow and static UV selection remain unresolved. See the
      [glacier record](docs/verification/GLACIER_PRIMARY_LAYER.md) and
      [earlier baseline](docs/verification/SANCTUARY_MATERIAL_BASELINE.md).
      Ash and Southpaw counts not yet re-measured. Any new binary-layout
      interpretation remains subject to the sensitive-area policy.
- [ ] Terrain / BSP geometry: Sanctuary's source census contains eight Terrain
      actors, 15 TerrainComponents and 24 ModelComponents. Missing-floor
      coverage remains unverified until geometry is recovered.
- [~] Sanctuary visual defects observed in the editor fly-through: the native
      dome now uses a two-sided interior policy, and the four known
      `Common_Meshes.Blocking.Blocking_Cube` actors, 94 collision helpers, four
      cloud planes, and the observed start-view blocking box are hidden from
      rendering while source collision is retained where recovered. Ground-floor
      materials now have scoped FrozenLake and regular-concrete/HLS fallbacks;
      native layer blending, HLS UV mapping and matched original screenshots
      remain open. See the [artifact pass](docs/verification/SANCTUARY_ARTIFACT_PASS.md).
- [ ] Unsupported component owners (33 in Sanctuary) and color-stream variants
      (4 in Sanctuary).
- [ ] Complete walking collision: initial Sanctuary convex/box collision and
      placeholder walking are verified; full routes and stairs remain open.
      Ash and Southpaw scenes are not refreshed; sphere/capsule/PhysX shapes
      and blocking volumes remain unsupported; terrain uses triangle collision
      verified by `OpenWillow.TerrainWalking` on Sanctuary only. See
      [walking verification](COLLISION_WALKING_VERIFICATION.md).
- [ ] Broader map coverage. Command-line and in-game selectors exist; the
      in-game list only offers already imported scenes (2026-09-14).
- [ ] Performance: Sanctuary profiled at 12–15 FPS on an Intel Iris Xe
      laptop, GPU-bound with ~72% of the frame in TSR; `-LowEnd` reaches the
      60 FPS cap. Candidate anti-aliasing change recorded, not applied. See
      [performance record](docs/verification/PERFORMANCE.md).
- [ ] Matched-viewpoint screenshots against the original game: the plan's
      per-map verification method. Needs someone with the game and both
      builds open.
- [ ] Cross-check the census against umodel's view of the same packages.
- [ ] Array element types from class reflection instead of hand-written
      schemas (needs cross-package class loading; touches Phase 2).

---

## Phase 0: Foundation and spikes <img src=".github/assets/icons/done.svg" width="22" align="absmiddle" alt="">

*Estimate: 3–6 weeks. Gated 2026-09-10, ahead of estimate; most of the reader already existed as a Python prototype.*

Goal: a repo, a build, a package loader, and a decided host engine.

- [x] Repo, README, `CONTRIBUTING.md`, issue tracker
- [x] License chosen: MIT (2026-09-13; reasoning in [DECISIONS.md](DECISIONS.md))
- [x] Dev environment: Visual Studio 2022, CMake, Python, Git
- [x] Package loader ported to C++ from `research/native_count.py`: LZO1X via
      vendored lzokay (MIT), chunk container, partial-compression chunk
      tables, name/import/export tables. Verified byte-for-byte against the
      Python reader on all nine code packages.
- [x] Census over every package in the install: 2,008 / 2,008 packages read,
      4,751,329 serialized exports; two UHD sidecar files identified and
      skipped. *Caveat:* Counts serialized copies, not unique assets; umodel
      cross-check pending.
- [x] Tagged-property reader: scalars, object/class/component refs, fixed
      structs (`Vector`, `Rotator`, `Guid`, `LinearColor`, `Color`, `Quat`,
      `Vector2D`), nested structs, arrays via explicit element-type schema.
      *Caveat:* Array element types are not serialized in 832/46; reflection-driven
      typing is future work. *Caveat:* The property-start offset must be supplied;
      the observed 4-byte prefix is `UNVERIFIED` as a rule.
- [x] Property dump of a real `WeaponPartDefinition`
      (`AR_Barrel_Jakobs_Sawbar`) compared against BLCMM: all 13 tags match,
      zero trailing bytes in three installed copies. *Caveat:* One
      generated-subobject presentation discrepancy recorded in
      [DECISIONS.md](DECISIONS.md).
- [x] Texture spike: `Texture2D` to PNG, DXT1/DXT5, inline, TFC-streamed and
      LZO-compressed bulk, every resident mip. Viewed; it is the texture.
- [x] Static mesh spike: all render LODs, 16/32-bit indices, all UV sets; one
      LOD to OBJ.
- [x] Host engine decided: **Unreal Engine 5**. Hello-world project builds;
      the probe mesh and its verified diffuse texture render in UE 5.8.2.
      First screenshot user-verified.

**Gate passed.** The census exists; one mesh and one texture from BL2 render in
the host engine. Records: [DECISIONS.md](DECISIONS.md) entries dated
2026-09-10.

---

## Phase 1: World viewer (M1) <img src=".github/assets/icons/now.svg" width="22" align="absmiddle" alt="">

*Estimate: 2–4 months full-time. Started 2026-09-10.*

Goal: walk around any BL2 map in a modern 64-bit renderer. Ship publicly.

- [x] Reusable importer APIs (`PackageStore` with lazy indexing and
      cross-package import resolution; texture and mesh importers behind C++
      APIs instead of CLI-only spikes)
- [ ] Static mesh importer for all meshes
  - [x] All render LODs, all UV sets, section material references
  - [x] Collision hulls from `RB_BodySetup` (box and convex) *Caveat:*
        sphere, capsule and cooked PhysX shapes unsupported; refreshed on
        Sanctuary only
  - [ ] Source mesh data
- [ ] Texture importer with TFC streaming
  - [x] `Textures.tfc`, DXT1/DXT5
  - [x] `PF_A8R8G8B8` (automated extraction verified)
  - [ ] Other pixel formats
  - [ ] `CharTextures.tfc`, `Lighting.tfc`; UHD pack optional
- [ ] Material translation
  - [x] v1: named diffuse/normal/specular/emissive parameters, parent
        inheritance, base-material defaults; opaque lit; fixed roughness
        *Caveat:* an approximation of UE3 shading, not graph translation
  - [x] Diffuse inference from cooked Material resource texture lists when
        no named parameter exists *Caveat:* recorded as inference; graph
        connectivity, tint and masks `UNVERIFIED`
  - [ ] v2: node-graph translation, cel-shade edge, ink lines
        Native resource-tail structure now matches 335 observed Ash/Sanctuary
        materials in diagnostic tooling; field semantics and graphs remain
        `UNVERIFIED`. See [resource census](docs/verification/MATERIAL_RESOURCE_CENSUS.md).
- [ ] Level loader
  - [x] Persistent map + serialized sublevel references (`_P`, `_Px`,
        `_Light`, `_Audio`, `_Combat`, `_Dynamic`, `_FX`)
  - [x] `StaticMeshActor`, `InterpActor`, `StaticMeshCollectionActor`
        transforms (collection tails: observed 84-byte layout, Ash and
        Sanctuary) *Caveat:* observed layout, not a general format guarantee
  - [x] `_Dynamic` props placed as static
    - [~] Skybox: observed `Sky_Dome` placement and Unlit material now import
          through the bounded native-sky v1 path, with a blue host shell keeping
          the inspection background readable; outer layers, time-of-day graph,
          Kismet activation and visual parity remain open
  - [ ] Terrain / BSP
  - [ ] Runtime streaming (all sublevels currently load at once)
- [ ] Lighting
  - [x] Inspection rig: movable sun, neutral skylight, reflection capture,
        auto exposure, AO. For geometry/material checks only
  - [ ] Modern: dynamic GI
  - [ ] Fidelity: parse SM3 lightmaps from `Lighting.tfc`
- [ ] Camera and movement
  - [x] Free-flight spectator pawn, saved start pose and FOV; automated
        movement/camera test passes on Ash and Sanctuary
  - [x] Placeholder character controller with collision (opt-in `-Walk`)
        *Caveat:* UE5 movement defaults, verified at the Sanctuary start only
- [ ] Map coverage: **3 / 82** (`Ash_P`, `Sanctuary_P`, `SouthpawFactory_P`)
  - [x] Command-line base-game map selector (`tools/viewer.py`)
  - [x] Optional DLC package discovery (82 installed map names total)
  - [x] In-game map selector (Tab list, digit keys; imported scenes only)
        *Caveat:* automated level switch verified, physical key press not
  - [ ] Loading times and memory profile *Caveat:* one settled frame-time
        and process-memory sample on Sanctuary recorded; no load-time
        measurement
- [ ] Verification: side-by-side screenshots against the real game per map
      (not yet started; needs matched viewpoints)

**Gate:** 82/82 maps load and are walkable → public release. Kill criterion:
if no map loads by month 6, stop and reassess.

Records: [Material v1 / Ash](docs/verification/MATERIAL_LEVEL_V1_VERIFICATION.md)
· [Phase 1 viewer](docs/verification/PHASE1_VIEWER_VERIFICATION.md)
· [Cooked materials](docs/verification/COOKED_MATERIAL_VERIFICATION.md)
· [Map selection / Southpaw Factory](docs/verification/MAP_SELECTOR_VERIFICATION.md)
· [UV / winding](docs/verification/UV_WINDING_VERIFICATION.md)
· [Collision and walking](COLLISION_WALKING_VERIFICATION.md)
· [Performance / in-game selector](docs/verification/PERFORMANCE.md).

---

## Phase 2: UnrealScript VM (M2) <img src=".github/assets/icons/todo.svg" width="22" align="absmiddle" alt="">

*Estimate: +3–6 months.* Goal: Gearbox's own gameplay code executing.

- [ ] Object model: `UObject` with class/outer/name/property bag; `UClass`
      hierarchy; class default objects from package CDOs; `FName` table;
      cross-package reference resolution
- [ ] Bytecode loader for every `UFunction` / `UState`; opcode table; handle
      the Gearbox local-variable-array quirk flagged by UE Explorer
- [ ] Interpreter: expressions, locals, `out` params, structs, dynamic arrays,
      casts, `foreach`, `switch`, `goto`, delegates, `super`, states and
      transitions, latent functions, timers
- [ ] Native dispatch table: all 7,141 natives registered as stubs that log
      `UNIMPLEMENTED name(args)`
- [ ] The 286 Core builtins (operators, math, string, name, object)
- [ ] Test harness running pure-script classes in isolation (the 268
      script-only `Behavior_*` classes), outputs compared to UDK

**Gate:** VM runs `Behavior_*` chains and the stat-free parts of
`WillowWeapon` without crashing; every unmet native is a logged stub.

---

## Phase 3: Stock UE3 natives (M3a) <img src=".github/assets/icons/todo.svg" width="22" align="absmiddle" alt="">

*Estimate: +6–12 months.* Goal: the 1,914 documented UE3 natives. Ground
truth: UDK.

- [ ] `Actor` (166): spawn/destroy, transforms, timers, traces, movement,
      attachment, tick, `Touch`/`Bump`
- [ ] `PrimitiveComponent` / collision (47) → host physics
- [ ] `Pawn` (126), `Controller` (45), `PlayerController` (73): walking,
      falling, jumping, slopes, input, camera, possession
- [ ] `SkeletalMeshComponent` (127), ~30 `AnimNode*` types,
      `PhysicsAssetInstance` (19)
- [ ] `ParticleSystemComponent` (46) + Cascade modules → Niagara
- [ ] `WorldInfo` (56), `NavigationHandle` (44), `Settings`, `Camera`,
      `Light`, `Sound` stubs

**Gate:** a scripted UE3 pawn moves and animates on a BL2 map with correct
collision, matching UDK-derived golden tests.

---

## Phase 4: Willow natives (M3b) <img src=".github/assets/icons/todo.svg" width="22" align="absmiddle" alt="">: the mountain

*Estimate: +12–24 months.* Goal: a Vault Hunter walks, shoots real guns, uses
real skills, and enemies fight back. Ground truth: the original game
instrumented with unrealsdk, plus community documentation.

Method: the golden-file loop. Hook a native in the real game, log every
call's inputs and outputs during play, implement until our engine reproduces
the log, extend the log on mismatch.

Priority order:
- [ ] Stat core (~120): `AttributeDefinition*`, `SkillDefinition`,
      `WeaponPartDefinition`, `InventoryBalanceDefinition`,
      `WillowDamageTypeDefinition`
- [ ] `WillowPawn` (246), `WillowPlayerPawn` (65): health/shield, FFYL, damage
- [ ] `WillowPlayerController` (298): input, aiming, skills, interaction
- [ ] `WillowWeapon` (121), `WillowProjectile` (95): fire, reload, spread,
      recoil, elemental procs
- [ ] `WillowInventory` (50), `WillowItem` (39), item generation from parts
- [ ] AI: `WillowAIPawn` (121), `WillowMind` (47), `WillowAIComponent` (45),
      GearboxFramework `AIComponent` (48), `GearboxMind` (32),
      `GearboxCoverStateManager` (48)
- [ ] `WillowInteractiveObject` (134): doors, chests, vendors, switches
- [ ] `WillowVehicle` (156)
- [ ] Scaleform bridge (512), `WillowHUDGFxMovie` (98), `WillowUIInteraction`
      (41) via an SWF VM; debug HUD until then
- [ ] Wwise audio (17 + `AkAudio`), Bink video

**Gate:** a full loop on one map: spawn, fight enemies, loot a gun, equip it,
use a skill, die, respawn.

---

## Phase 5: Campaign completable (M4) <img src=".github/assets/icons/todo.svg" width="22" align="absmiddle" alt="">

*Estimate: +12–24 months.*

- [ ] Kismet interpreter (`MissionTracker`, 81 natives)
- [ ] Matinee → Sequencer for cutscenes
- [ ] Mission system, objectives, fast travel, vending, ECHO/dialogue, Bink
      playback
- [ ] Save files: read/write real BL2 saves (Gibbed format)
- [ ] Full Scaleform UI: menus, inventory, skill trees, map
- [ ] All 82 maps' `_Combat` populations, spawn schedules, bosses, raids

**Gate:** Claptrap to the Warrior, single player, with a real save file.

---

## Phase 6: Parity and beyond (M5) <img src=".github/assets/icons/todo.svg" width="22" align="absmiddle" alt="">

- [ ] Co-op netcode (our own)
- [ ] DLC coverage; The Pre-Sequel; standalone Dragon Keep
- [ ] Mod compatibility: BLCMM text mods natively; SDK-mod layer later
- [ ] Editor for new maps; modern lighting toggle; VR; 8-player

---

## Kill criteria

Stated up front so nobody has to guess whether the project is alive:

- Phase 0 not gated in 3 months → tooling loop isn't working. *(Passed.)*
- M1 cannot load a single map by month 6 → the loop isn't holding; fall back
  to the [mod plan](docs/DESIGN_OVERHAUL_MOD.md). *(Three maps already load.)*
- Nobody but the author has contributed by M2 → fine, but plan M3 only.
- The author stops reading the code → pause and fix that.
