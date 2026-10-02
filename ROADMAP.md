# BL2_ReBased roadmap

The live tracker. Phases, steps and gates come from
[docs/OPENWILLOW_ENGINE_PLAN.md](docs/OPENWILLOW_ENGINE_PLAN.md); this file
records where each step actually stands, with the verification record that
backs it. Checkbox states are conservative: a step is checked only when its
verification is written down.

Legend: <img src=".github/assets/icons/done.svg" width="18" align="absmiddle" alt=""> done and verified · <img src=".github/assets/icons/now.svg" width="18" align="absmiddle" alt=""> in progress · <img src=".github/assets/icons/todo.svg" width="18" align="absmiddle" alt=""> not started.
Items marked *Caveat:* are done with a recorded limitation.

**Phase 0 gate:** 2026-09-10 · **Now:** Phase 1 · **Last update:** 2026-10-02

---

## Priority: the vertical slice

Porting all 82 maps is the *easier* part of this project; the harder, unproven
part is Phases 2-4 (script VM, stock UE3 natives, Gearbox's undocumented
natives). Rather than finish map breadth first and find out later that a later
phase doesn't hold, the phases below are scoped to prove the hard parts on one
map and one character first. Adopted 2026-09-18 from outside feedback; see
[DECISIONS.md](DECISIONS.md).

Current slice target: **Sanctuary** (closest map to done) + **Maya** (chosen
2026-09-18) + one hand-picked mission + a handful of guns, working
end-to-end: spawn, fight, loot a gun, equip it, use her action skill
(Phaselock), complete the mission, die, respawn. That's Phase 4's gate below.
Broad map coverage (79 more maps) and the remaining Vault Hunters are
deferred until that gate is met; they move to Phase 5/6.

---

## How it's going

The project is worked on actively. There are no completion dates, no time
estimates and no completion percentages here; this section only records what
the repository shows as of 2026-10-02, and each line names the record that
backs it ([DECISIONS.md](DECISIONS.md) entry date or verification file). Where
a number is an automated check it says so; "passes" is never parity with the
original game.

**Elapsed.** First commit 2026-09-10, so 22 calendar days. 191 commits on
`origin/main` across 17 days with at least one commit. Built with heavy AI
assistance (see the README). The first commit imported earlier local
prototypes, so the repository's clock started a little after the work did.

**Dated milestones**

- **2026-09-10:** package reader reads 2,008 / 2,008 packages; Phase 0 gated
  (UE5 chosen as host; one mesh and one texture render in it).
- **2026-09-13:** Ash and Sanctuary load as frozen scenes in UE5 with a
  free-flight camera (Southpaw Factory follows on 2026-09-14).
- **2026-09-14:** placeholder walking with collision on Sanctuary.
- **2026-09-15:** Sanctuary terrain and BSP floors imported with walkable
  collision.
- **2026-09-23 to 2026-09-25:** Maya imported with animated first-person arms;
  Phaselock cast in the host; recipe-driven weapon inventory screen.
- **2026-09-30:** read-only script bytecode disassembler, then a C++ bytecode
  loader, VM runtime and Core natives (prototype).
- **2026-10-01:** the VM drives backpack navigation in UE5; one Sanctuary door
  opens from installed Matinee/Kismet data; the stock Fire mission plays end to
  end in the host with labelled stand-ins; weapon balances, loot pools, world
  placement and Phaselock data decoded for the slice. Policy change: analysing
  `Borderlands2.exe` locally is allowed, output stays out of the repository
  (DECISIONS 2026-10-01).
- **2026-10-02:** Ghidra tooling and the first native-analysis notes (mission,
  behavior and Kismet dispatch, progression, Phaselock targeting); the executors
  in `src/behavior.*`, `src/kismet.*` and `src/mission.*` now follow the
  dispatch notes; weapon paint passes 1 to 3; a `ParticleSystem` template
  reader; inventory open time measured and reduced.

**Measured** (automated checks and timings; one PC unless stated)

- Package reader: 2,008 / 2,008 packages, 4,751,329 serialized exports
  (Phase 0 entries, 2026-09-10). Scripts: 12,968 of 12,978 functions decode
  structurally ([record](docs/verification/SCRIPT_BYTECODE_DISASM.md)). Both are
  structural agreement, not runtime compatibility.
- Suites after the dispatch-rule change (DECISIONS 2026-10-02): CTest 10/10,
  packages 9/9, quest suite 73/73 first run and 10/10 resume, door suite
  16/16, inventory suite 45 PASS / 0 FAIL / 2 NOT_RUN / 2 KNOWN_DIVERGENCE
  (sort order) on a regenerated worktree with a short backpack; 47 PASS / 2
  KNOWN_DIVERGENCE on the fully seeded one (DECISIONS 2026-10-01).
- Inventory open time (DECISIONS 2026-10-02,
  [record](docs/verification/INVENTORY_MOVIE_PROTOTYPE.md)): first open with the
  page loaded 443 ms (358-488, 4 launches) before, 234 ms (213-252, 3 launches)
  after moving one-time VM and mesh work to level start; repeat opens about 60
  ms. An open pressed in the first ~5 s of play still takes about 5.5 s (the
  page's movie player booting). No original-game open time exists.
- Native registration (DECISIONS 2026-10-02, [notes](docs/NATIVE_ANALYSIS.md)):
  the executable's tables hold 6,877 natives in 770 tables and all 199
  numbered script natives resolve by name. A census over 133 missions puts all
  3,420 behavior links in the predicted id ranges (structural only).
- Weapon paint (DECISIONS 2026-10-01 and 2026-10-02): static parameters decode
  exactly in 631 of 631 `Startup.upk` MICs; `tests/weapon_paint_test.py` 29
  passed on invented values. An independent critic agent comparing host stills
  with wiki in-game screenshots scored the Maliwan uncommon pistol 6.5/10 and the
  Jakobs common pistol 5.0/10; that is an agent's judgement, not a real-game
  comparison.
- Particle templates ([record](docs/verification/PHASELOCK_STOCK_DATA.md),
  "Particle template reader"): `tests/particle_system_test.py` 16 passed; all
  17,506 non-empty baked distribution tables in four packages fit the assumed
  layout. Reader and layout only: nothing is rendered from it yet.

**UNVERIFIED** (read or fitted, not confirmed by running the original game)

- Every native rule in the dispatch, progression and Phaselock-targeting notes
  and the executor behaviour built on the dispatch notes: synthetic tests show
  the executors follow the notes, not that the notes are right
  ([dispatch](docs/verification/NATIVE_MISSION_DISPATCH.md),
  [progression](docs/verification/NATIVE_PROGRESSION.md),
  [targeting](docs/verification/NATIVE_PHASELOCK_TARGETING.md)). The
  progression rules are implemented in the tools and host (quest suite 75/75
  and 11/11); the Phaselock targeting, constraint and presentation changes are
  written but have not passed a suite (module DLL blocked by Windows
  Application Control, DECISIONS 2026-10-02).
- The whole weapon paint and decal reading, its display scale and shading
  inputs (`USE_SHADER_SHADING` stays off until the scene lighting is
  calibrated); environment reflection, emissive and in-game lighting are not
  modelled.
- The slice host stand-ins: kickoff played right after acceptance, dialog
  outputs selected together, a save-state fixture for the dependency mission, a
  turn-in loot stand-in (stock data drops nothing for this mission), a lent
  mission weapon ([route record](docs/verification/SANCTUARY_RPG_MISSION.md)).

**Blocked or open**

- No capture of the original game exists for any slice behaviour: it needs the
  game driven interactively with exclusive keyboard and screen
  ([capture blocker](docs/verification/SANCTUARY_RPG_MISSION.md#capture-blocker-needs-the-maintainer)),
  and a launch under the logged-in Steam account failed (DECISIONS 2026-10-02).
- Sanctuary visual parity, the stripped sky graph, native material graphs and
  the 79 untouched maps (see Now / next and Phase 1).
- Audio is looked up and logged, never played; choosing a decoder is a
  maintainer decision.
- Two quest-suite first runs on 2026-10-02 ended silently after check 67; the
  cause is unproven (a mixed build state is likely: a clean rebuild passed and
  exited cleanly)
  ([record](docs/verification/INVENTORY_MOVIE_PROTOTYPE.md), "Suites").

No phase after Phase 0 has passed its gate.

---

## Now / next

The first world-object connection is a bounded Sanctuary Matinee door: installed
movement keys drive the existing mesh/collision, and original mover notification
scripts run in the VM using placed state. Activation is a developer interaction;
mission gating, sequence dispatch, audio and original-game motion parity remain
open. See [the mover record](docs/verification/SANCTUARY_MOVER_PROTOTYPE.md).
Slice progress (2026-10-02): the door is activated through its installed Kismet events, and the stock
"Rock, Paper, Genocide: Fire Weapons!" mission runs in the host through native executors over installed data
(mission, behavior provider, dummy provider) with stock world data: a placed Marcus whose walk the installed Kismet
starts, the stock range cylinder, the stock dummy (standing on the lane floor; the earlier "kneeling" dummy was its
mesh hanging below an origin placed at floor level, fixed by attaching it at the holder's socket from data) and target
Matinee, formula-based health, decoded respawn selection, a lent stock mission pistol, damage type taken from the held
item, candidate XP into the skills component, Phaselock read from stock data (lift rule, valid-target rule and cast
gate from script and data), progression in the quest save, health recomputed on level change, and save/resume. The
executors now follow the native dispatch notes (link-id filters, trigger limits, Kismet op stack, mission set
completion), all `UNVERIFIED`. Recorded checks (DECISIONS 2026-10-02): quest suite 73/73 and resume 10/10, door suite
16/16, inventory suite 45 PASS / 0 FAIL / 2 NOT_RUN / 2 KNOWN_DIVERGENCE, CTest 10/10, packages 9/9.
All of it is host behaviour with labelled stand-ins (native auto-aim selection and constraint evaluation for
Phaselock, a save-state fixture for the dependency mission, a turn-in loot stand-in because stock data drops
nothing for this mission) and `UNVERIFIED` rules; nothing has been compared against the original game, and every
parity claim remains open. Hand play: `tools/run_quest.ps1 -Fresh`. See
[the route record](docs/verification/SANCTUARY_RPG_MISSION.md) and
[the dispatch notes](docs/verification/NATIVE_MISSION_DISPATCH.md).

The concrete open items, roughly in the order they are being taken. Small,
well-bounded ones are marked *good first task*.

- [x] Decode `PF_A8R8G8B8` textures: synthetic pixel/bulk tests pass and the
      real 256x256 Ash sky transition texture extracts successfully. Native sky
      shading remains open. See [verification](docs/verification/A8R8G8B8_TEXTURE.md).
- [x] Diagnose the mirrored Sanctuary shop sign: the host adapter reversed
      winding twice, exposing back faces. Corrected isolated sign renders
      readable; saved UV and winding checks now guard the import path.
- [~] Sky rendering: the accepted Sanctuary `Sky_Dome` now renders a
      daytime gradient with cloud bands built from the instance's own named
      inputs (`Time_of_Day` column of the transition strip over dome V,
      horizon-tinted `Clouds_01.R`), and the blue host shell that hid the
      dome is no longer spawned there. The stripped master graph is not
      decoded: the column reading and the cloud combine are `UNVERIFIED`,
      and the sun spot, masks, cloud motion and time-of-day animation are
      omitted. The `_Outer` city hull imports opt-in (`--outer-shell`) with
      its mesh-default materials in place of the unrecoverable `_Teleported`
      overrides. `OpenWillow_SkyAtmosphere` remains for ambient light; Ash
      coverage, Kismet activation (`_Land` vs `_Outer`) and visual parity
      remain open. See the
      [sky approximation record](docs/verification/NATIVE_SKY_APPROXIMATION.md),
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
- [~] Terrain / BSP geometry: Sanctuary's eight terrains now emit 15 component
      meshes with corroborated topology and host triangle collision, and the
      two persistent-level root Models emit 228 cross-checked polygons (571
      triangles, 105 sections) with native materials, native texture axes
      (field roles confirmed against 15,393 editor FPoly records, 0 differ;
      texel scale still `UNVERIFIED`) and opt-in triangle collision. Native
      terrain blend parity (31 cooked weights now reproduce their PF_G8
      textures and drive eight host weighted-sum materials), the BSP texel
      scale and V orientation, lightmaps / collision flags,
      other maps and original-game alignment remain open. Hole runtime
      assertions include occluded/displaced probes; see the
      [terrain handoff](docs/verification/SANCTUARY_TERRAIN_BSP_HANDOFF.md),
      the [BSP record](docs/verification/SANCTUARY_BSP_POLYGONS.md) and the
      [texture-axis record](docs/verification/BSP_TEXTURE_AXES.md).
- [~] Sanctuary visual defects observed in the editor fly-through: the native
      dome now uses a two-sided interior policy, and the four known
      `Common_Meshes.Blocking.Blocking_Cube` actors, 94 collision helpers, four
      cloud planes, and the observed start-view blocking box are hidden from
      rendering while source collision is retained where recovered. Ground-floor
      materials now have scoped FrozenLake and regular-concrete/HLS fallbacks;
      native layer blending, HLS UV mapping and matched original screenshots
      remain open. See the [artifact pass](docs/verification/SANCTUARY_ARTIFACT_PASS.md).
      The Hyperion moon base in the sky now uses its own inspected
      diffuse/normal/emissive textures instead of the neutral fallback; its
      tint and fog terms remain omitted. The moon's Unlit color is scaled by
      its own `p_moonColor`; the station shadow mask, relief and
      time-of-day tint remain omitted pending an in-game reference. See the
      [moon base record](docs/verification/MOON_BASE_SURFACE.md).
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
      *Lower priority than the vertical slice above until the Phase 4 gate is
      met (2026-09-18).*
- [ ] Performance: Sanctuary profiled at 12–15 FPS on an Intel Iris Xe
      laptop, GPU-bound with ~72% of the frame in TSR; `-LowEnd` reaches the
      60 FPS cap. Candidate anti-aliasing change recorded, not applied. See
      [performance record](docs/verification/PERFORMANCE.md).
- [ ] Matched-viewpoint screenshots against the original game: the plan's
      per-map verification method. Needs someone with the game and both
      builds open.
- [x] Cross-check the census against umodel's view of the same packages:
      4,750,427 exports over 2006 base and DLC packages, no offset, size or
      class disagreement; the 7 name-only differences are umodel-side
      normalization. Sanctuary meshes, textures and material picks were also
      compared with umodel exports and with the game's own object dumps. See
      [umodel record](docs/verification/UMODEL_CROSSCHECK.md) and
      [dump record](docs/verification/BLCMM_DUMP_CROSSCHECK.md).
- [ ] Array element types from class reflection instead of hand-written
      schemas (needs cross-package class loading; touches Phase 2).

---

## Phase 0: Foundation and spikes <img src=".github/assets/icons/done.svg" width="22" align="absmiddle" alt="">

*Gated 2026-09-10; most of the reader already existed as a Python prototype.*

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

## Phase 0.5: External extraction support for the vertical slice (in progress) <img src=".github/assets/icons/now.svg" width="22" align="absmiddle" alt="">

*This supports the Sanctuary/Maya pipeline proof of concept and can speed up
asset preparation; it does not replace the runtime, gameplay or parity
phases.*

Goal: establish whether a mature community exporter can provide repeatable local
visual payloads for the Sanctuary/Maya vertical-slice importer without
replacing OpenWillow's metadata, reference and verification responsibilities.

- [x] Acquire UModel / UE Viewer outside the repository. The current local
      candidate is build 1590 from the upstream `gildor2/UEViewer` checkout;
      source commit, binary hash and provenance are recorded in the
      [external-tool benchmark](docs/verification/EXTERNAL_TOOL_BENCHMARK.md).
- [x] Confirm Borderlands 2 detection and package listing: 920 files scanned,
      `Ash_P.upk` recognized as version `832/46` with 21,834 exports.
- [x] Export representative static and skeletal meshes plus a TFC-streamed
      texture: `Ash_Road01` produced glTF and its binary buffer in 0.1 seconds,
      `MetalRoadConcrete_Dif` produced a 1024x1024 DDS in 0.09 seconds, and
      `Skel_BugMorph` produced glTF in 0.08 seconds. UModel warnings and output
      inventories are recorded; UE5 import and visual parity are not yet
      verified.
- [ ] Export representative animations, materials and sounds.
- [ ] Run a bounded multi-package batch and inventory successes, failures,
      unsupported types, duplicates, warnings, output bytes and elapsed time.
- [ ] Compare the external inventory with the `ow-package` census and prepared
      scene manifests.
- [ ] Import representative outputs into UE5 and choose the most reproducible
      adapter format (glTF, PSK/PSA or another proven path).
- [ ] Decide whether UModel becomes the default visual payload backend. Keep
      `ow-package` authoritative for package identity, object paths, placement
      metadata and verification regardless of the outcome.

**Gate:** representative exports are repeatable, all failures and unsupported
categories are enumerated, at least one output path reaches UE5, and the
external inventory can be related back to package/object identity. Until then,
the full-install export remains an unmeasured experiment.

Record: [external-tool benchmark](docs/verification/EXTERNAL_TOOL_BENCHMARK.md)
and [tooling reference](docs/TOOLING.md#external-tools).

---

## Phase 1: World viewer (M1) <img src=".github/assets/icons/now.svg" width="22" align="absmiddle" alt="">

*Started 2026-09-10.*

Goal: walk around any BL2 map in a modern 64-bit renderer. Ship publicly.

- [x] Reusable importer APIs (`PackageStore` with lazy indexing and
      cross-package import resolution; texture and mesh importers behind C++
      APIs instead of CLI-only spikes)
- [ ] Static mesh importer for all meshes
  - [ ] External visual-payload adapter after the Phase 0.5 gate
  - [x] All render LODs, all UV sets, section material references
  - [x] Collision hulls from `RB_BodySetup` (box and convex) *Caveat:*
        sphere, capsule and cooked PhysX shapes unsupported; refreshed on
        Sanctuary only
  - [ ] Source mesh data
- [ ] Texture importer with TFC streaming
  - [ ] External texture-payload adapter after the Phase 0.5 gate
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
    - [~] Skybox: observed `Sky_Dome` placement renders a labeled
          approximation from its named inputs (gradient column, cloud layer);
          `_Outer` hull opt-in with mesh-default materials; master graph,
          time-of-day animation, Kismet activation and visual parity remain
          open
  - [~] Terrain / BSP *Caveat:* Sanctuary only; single-layer terrain and
        planar-UV BSP approximations, both labeled `UNVERIFIED`
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
      *Caveat: broadening past Sanctuary is deferred until the Phase 4
      vertical-slice gate is met; see the priority note above.*
  - [x] Command-line base-game map selector (`tools/viewer.py`)
  - [x] Optional DLC package discovery (82 installed map names total)
  - [x] In-game map selector (Tab list, digit keys; imported scenes only)
        *Caveat:* automated level switch verified, physical key press not
  - [ ] Loading times and memory profile *Caveat:* one settled frame-time
        and process-memory sample on Sanctuary recorded; no load-time
        measurement
- [ ] Verification: side-by-side screenshots against the real game per map
      (not yet started; needs matched viewpoints)

**Gate (rescoped 2026-09-18):** Sanctuary loads, is walkable, and is visually
verified against the real game → work moves on to Phase 2 for the vertical
slice (Sanctuary + one Vault Hunter). Full 82/82 map coverage remains the
eventual completion target for this phase but is deferred until the Phase 4
vertical-slice gate is met; see Phase 5. Kill criterion unchanged: if no map
loads, stop and reassess. *(Passed: three maps already load.)*

Records: [Material v1 / Ash](docs/verification/MATERIAL_LEVEL_V1_VERIFICATION.md)
· [Phase 1 viewer](docs/verification/PHASE1_VIEWER_VERIFICATION.md)
· [Cooked materials](docs/verification/COOKED_MATERIAL_VERIFICATION.md)
· [Map selection / Southpaw Factory](docs/verification/MAP_SELECTOR_VERIFICATION.md)
· [UV / winding](docs/verification/UV_WINDING_VERIFICATION.md)
· [Collision and walking](COLLISION_WALKING_VERIFICATION.md)
· [Performance / in-game selector](docs/verification/PERFORMANCE.md).

---

## Phase 2: UnrealScript VM (M2) (prototype, in progress) <img src=".github/assets/icons/now.svg" width="22" align="absmiddle" alt="">

Goal: Gearbox's own gameplay code executing, scoped to what Sanctuary and the
chosen first Vault Hunter actually need (see
[vertical-slice priority](#priority-the-vertical-slice)), not full native
coverage.

- [ ] Object model: `UObject` with class/outer/name/property bag; `UClass`
      hierarchy; class default objects from package CDOs; `FName` table;
      cross-package reference resolution
- [ ] Bytecode loader for every `UFunction` / `UState`; opcode table; handle
      the Gearbox local-variable-array quirk flagged by UE Explorer.
      **Python and C++ prototypes present (2026-10-01):** both structurally
      decode 12,968 of 12,978 script functions (10 known failures), see
      [the record](docs/verification/SCRIPT_BYTECODE_DISASM.md). C++ object
      loading, interpretation, Core natives and a default-state sweep exist;
      full states/latent behavior and opcode semantics remain unverified.
      First UE5 inventory connection executes original `MoveDelta` on item-only
      backpack rows; the rest of the menu remains on the host adapter.
- [ ] Interpreter: expressions, locals, `out` params, structs, dynamic arrays,
      casts, `foreach`, `switch`, `goto`, delegates, `super`, states and
      transitions, latent functions, timers
- [ ] Native dispatch table: all 7,141 natives registered as stubs that log
      `UNIMPLEMENTED name(args)`
      *Note (2026-10-02):* a scan of the executable's own registration tables
      finds 6,877 natives in 770 tables, and all 199 numbered script natives
      bind by name ([DECISIONS.md](DECISIONS.md) 2026-10-02, tooling in
      `tools/ghidra/`). The two counts come from different sources and have not
      been reconciled; no stub table exists yet.
- [ ] The 286 Core builtins (operators, math, string, name, object)
- [ ] Test harness running pure-script classes in isolation (the 268
      script-only `Behavior_*` classes), outputs compared to UDK
      **Diagnostic harness present:** synthetic execution tests, `--vm-sweep`
      and scalar UI trace replay. Default-state return agreement does not pass
      this gate; see [VM evidence](docs/verification/SCRIPT_VM_PROTOTYPE.md).

**Gate:** VM runs `Behavior_*` chains and the stat-free parts of
`WillowWeapon` without crashing; every unmet native is a logged stub.

---

## Phase 3: Stock UE3 natives (M3a) <img src=".github/assets/icons/todo.svg" width="22" align="absmiddle" alt="">

Goal: the UE3 natives the vertical slice needs (movement, collision,
animation playback), not all 1,914 up front. Ground truth: UDK.

- [ ] `Actor` (166): spawn/destroy, transforms, timers, traces, movement,
      attachment, tick, `Touch`/`Bump`
- [ ] `PrimitiveComponent` / collision (47) → host physics
- [ ] `Pawn` (126), `Controller` (45), `PlayerController` (73): walking,
      falling, jumping, slopes, input, camera, possession
- [ ] `SkeletalMeshComponent` (127), ~30 `AnimNode*` types,
      `PhysicsAssetInstance` (19)
- [ ] `ParticleSystemComponent` (46) + Cascade modules → Niagara
      *Caveat:* only a read-only template reader exists
      (`research/particle_system.py`, Phaselock templates, layouts checked by
      oracles); nothing is converted or rendered.
- [ ] `WorldInfo` (56), `NavigationHandle` (44), `Settings`, `Camera`,
      `Light`, `Sound` stubs

**Gate:** a scripted UE3 pawn moves and animates on a BL2 map with correct
collision, matching UDK-derived golden tests.

---

## Phase 4: Willow natives (M3b) (slice pieces in the host, in progress) <img src=".github/assets/icons/now.svg" width="22" align="absmiddle" alt="">: the mountain

Goal: **Maya**, the vertical slice's first Vault Hunter, walks, shoots a
handful of real guns, uses Phaselock and her skill trees, and enemies on
Sanctuary fight back. Ground truth: the original game instrumented with
unrealsdk, plus community documentation.

Method (revised 2026-10-01): find each native's behaviour by analysing the game
executable locally ([NATIVE_ANALYSIS.md](docs/NATIVE_ANALYSIS.md)), write the
rule down in our own words, implement from that note, then confirm it with the
golden-file loop: hook the native in the real game, log every call's inputs
and outputs during play, implement until our engine reproduces the log, extend
the log on mismatch. Decompiler output never enters the repository.
Status (2026-10-02): the Ghidra tooling exists (`tools/ghidra/`) and three note sets
have been written (mission/behavior/Kismet dispatch, progression, Phaselock targeting), all `UNVERIFIED`; the dispatch
notes are implemented in the slice executors, the others are not yet. No native rule has been confirmed against the
running game.

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

**Gate:** a full loop on Sanctuary with Maya: spawn, fight enemies, loot a
gun, equip it, use Phaselock, complete one hand-picked mission, die, respawn.
This is the vertical slice; once met, broad map and character coverage
(Phase 5/6) becomes the priority again.

---

## Phase 5: Campaign completable (M4) <img src=".github/assets/icons/todo.svg" width="22" align="absmiddle" alt="">

- [ ] Broad map coverage: the remaining ~79 maps, deferred from Phase 1's
      vertical-slice rescoping (2026-09-18)
- [ ] Remaining Vault Hunters (5 of 6), deferred from Phase 4's
      vertical-slice rescoping (2026-09-18)
- [ ] Kismet interpreter (`MissionTracker`, 81 natives). *Caveat:* slice-scoped
      Kismet, behavior and mission executors already run the Fire mission's
      installed data (rules `UNVERIFIED`; see Now / next), which is not this
      step's general interpreter.
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

- Phase 0 never gets gated → tooling loop isn't working. *(Passed
  2026-09-10.)*
- The Phase 0.5 exporter spike does not produce a repeatable, attributable
  representative export → keep it as an inspection oracle and continue the
  bounded OpenWillow importer path; do not stall the project on a community
  tool.
- M1 cannot load a single map → the loop isn't holding for this approach; stop
  and reassess honestly rather than push on hope. *(Passed: three maps already
  load.)*
- Nobody but the author has contributed by M2 → fine, but plan M3 only.
- The author stops reading the code → pause and fix that.
