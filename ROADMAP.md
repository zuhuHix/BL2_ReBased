# Roadmap — BL2_ReBased

The live tracker. Phases, steps, gates and estimates come from
[docs/OPENWILLOW_ENGINE_PLAN.md](docs/OPENWILLOW_ENGINE_PLAN.md); this file
records where each step actually stands, with the verification record that
backs it. Checkbox states are conservative: a step is checked only when its
verification is written down.

Legend: <img src=".github/assets/icons/done.svg" width="18" align="absmiddle" alt=""> done and verified · <img src=".github/assets/icons/now.svg" width="18" align="absmiddle" alt=""> in progress · <img src=".github/assets/icons/todo.svg" width="18" align="absmiddle" alt=""> not started.
Items marked *Caveat:* are done with a recorded limitation.

**Project start:** 2026-09-09 · **Phase 0 gate:** 2026-09-10 · **Now:** Phase 1

---

## Now / next

The concrete open items, roughly in the order they are being taken. Small,
well-bounded ones are marked *good first task*.

- [ ] Decode `PF_A8R8G8B8` textures — the sky transition texture in Ash uses
      it, which is why the sky is black. *Good first task.*
- [ ] Check UV orientation: the Sanctuary shop sign appears mirrored in the
      viewer. Determine whether it's OBJ handedness, UV V-flip or the source.
- [ ] Sky rendering: translate the skybox sublevel.
- [ ] Material fallbacks: 30 Ash and 67 Sanctuary materials still lack a
      diffuse channel after cooked-resource inference (2026-09-13). Next is
      reading more of the cooked material resource — every extension needs
      explicit approval because the layout is unspecified.
- [ ] Terrain / BSP geometry.
- [ ] Unsupported component owners (33 in Sanctuary) and color-stream variants
      (4 in Sanctuary).
- [ ] Walking collision (currently free-flight with collision disabled).
- [ ] Map selector and the third map.
- [ ] Performance: Sanctuary ran at ~8–9 FPS during automation startup on the
      development machine. Not yet profiled.
- [ ] Matched-viewpoint screenshots against the original game — the plan's
      per-map verification method. Needs someone with the game and both
      builds open.
- [ ] Cross-check the census against umodel's view of the same packages.
- [ ] Array element types from class reflection instead of hand-written
      schemas (needs cross-package class loading; touches Phase 2).

---

## Phase 0 — Foundation and spikes <img src=".github/assets/icons/done.svg" width="22" align="absmiddle" alt="">

*Estimate: 3–6 weeks. Actual: gated 2026-09-10, the day after the project started.*

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
- [x] Texture spike: `Texture2D` to PNG — DXT1/DXT5, inline, TFC-streamed and
      LZO-compressed bulk, every resident mip. Viewed; it is the texture.
- [x] Static mesh spike: all render LODs, 16/32-bit indices, all UV sets; one
      LOD to OBJ.
- [x] Host engine decided: **Unreal Engine 5**. Hello-world project builds;
      the probe mesh and its verified diffuse texture render in UE 5.8.2.
      First screenshot user-verified.

**Gate passed** — the census exists; one mesh and one texture from BL2 render in
the host engine. Records: [DECISIONS.md](DECISIONS.md) entries dated
2026-09-10.

---

## Phase 1 — World viewer (M1) <img src=".github/assets/icons/now.svg" width="22" align="absmiddle" alt="">

*Estimate: 2–4 months full-time. Started 2026-09-10.*

Goal: walk around any BL2 map in a modern 64-bit renderer. Ship publicly.

- [x] Reusable importer APIs (`PackageStore` with lazy indexing and
      cross-package import resolution; texture and mesh importers behind C++
      APIs instead of CLI-only spikes)
- [ ] Static mesh importer for all meshes
  - [x] All render LODs, all UV sets, section material references
  - [ ] Collision hulls from `RB_BodySetup`
  - [ ] Source mesh data
- [ ] Texture importer with TFC streaming
  - [x] `Textures.tfc`, DXT1/DXT5
  - [ ] `PF_A8R8G8B8` and other pixel formats
  - [ ] `CharTextures.tfc`, `Lighting.tfc`; UHD pack optional
- [ ] Material translation
  - [x] v1: named diffuse/normal/specular/emissive parameters, parent
        inheritance, base-material defaults; opaque lit; fixed roughness
        *Caveat:* an approximation of UE3 shading, not graph translation
  - [x] Diffuse inference from cooked Material resource texture lists when
        no named parameter exists *Caveat:* recorded as inference; graph
        connectivity, tint and masks `UNVERIFIED`
  - [ ] v2: node-graph translation, cel-shade edge, ink lines
- [ ] Level loader
  - [x] Persistent map + serialized sublevel references (`_P`, `_Px`,
        `_Light`, `_Audio`, `_Combat`, `_Dynamic`, `_FX`)
  - [x] `StaticMeshActor`, `InterpActor`, `StaticMeshCollectionActor`
        transforms (collection tails: observed 84-byte layout, Ash and
        Sanctuary) *Caveat:* observed layout, not a general format guarantee
  - [x] `_Dynamic` props placed as static
  - [ ] Skybox
  - [ ] Terrain / BSP
  - [ ] Runtime streaming (all sublevels currently load at once)
- [ ] Lighting
  - [x] Inspection rig: movable sun, neutral skylight, reflection capture,
        auto exposure, AO — for geometry/material checks only
  - [ ] Modern: dynamic GI
  - [ ] Fidelity: parse SM3 lightmaps from `Lighting.tfc`
- [ ] Camera and movement
  - [x] Free-flight spectator pawn, saved start pose and FOV; automated
        movement/camera test passes on Ash and Sanctuary
  - [ ] Placeholder character controller with collision
- [ ] Map coverage: **2 / 82** (`Ash_P`, `Sanctuary_P`)
  - [ ] Map selector
  - [ ] Loading times and memory profile
- [ ] Verification: side-by-side screenshots against the real game per map
      (not yet started; needs matched viewpoints)

**Gate:** 82/82 maps load and are walkable → public release. Kill criterion:
if no map loads by month 6, stop and reassess.

Records: [Material v1 / Ash](docs/verification/MATERIAL_LEVEL_V1_VERIFICATION.md)
· [Phase 1 viewer](docs/verification/PHASE1_VIEWER_VERIFICATION.md)
· [Cooked materials](docs/verification/COOKED_MATERIAL_VERIFICATION.md).

---

## Phase 2 — UnrealScript VM (M2) <img src=".github/assets/icons/todo.svg" width="22" align="absmiddle" alt="">

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

## Phase 3 — Stock UE3 natives (M3a) <img src=".github/assets/icons/todo.svg" width="22" align="absmiddle" alt="">

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

## Phase 4 — Willow natives (M3b) <img src=".github/assets/icons/todo.svg" width="22" align="absmiddle" alt=""> — the mountain

*Estimate: +12–24 months.* Goal: a Vault Hunter walks, shoots real guns, uses
real skills, and enemies fight back. Ground truth: the original game
instrumented with unrealsdk, plus community documentation.

Method — the golden-file loop: hook a native in the real game, log every
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

**Gate:** a full loop on one map — spawn, fight enemies, loot a gun, equip it,
use a skill, die, respawn.

---

## Phase 5 — Campaign completable (M4) <img src=".github/assets/icons/todo.svg" width="22" align="absmiddle" alt="">

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

## Phase 6 — Parity and beyond (M5) <img src=".github/assets/icons/todo.svg" width="22" align="absmiddle" alt="">

- [ ] Co-op netcode — our own
- [ ] DLC coverage; The Pre-Sequel; standalone Dragon Keep
- [ ] Mod compatibility: BLCMM text mods natively; SDK-mod layer later
- [ ] Editor for new maps; modern lighting toggle; VR; 8-player

---

## Kill criteria

Stated up front so nobody has to guess whether the project is alive:

- Phase 0 not gated in 3 months → tooling loop isn't working. *(Passed.)*
- M1 cannot load a single map by month 6 → the loop isn't holding; fall back
  to the [mod plan](docs/DESIGN_OVERHAUL_MOD.md). *(Two maps load at week 1.)*
- Nobody but the author has contributed by M2 → fine, but plan M3 only.
- The author stops reading the code → pause and fix that.
