# Sanctuary scene import gaps: material fallbacks, component owners, colour streams (2026-10-21)

AI-assisted (Claude), lane A, branch `feature/sanctuary-parity-r2`. Evidence comes from the installed base game
through this repository's reader, a census over every base-game package and live reads of the running game.
Every report, texture, scene and capture stays under ignored `local/` (`local/lanea/`, `local/realgame/`,
`local/sanctuary/`) and the local gallery named below. No game-derived file is committed.

## Matched-viewpoint pass, rounds 1-4 (2026-10-21/22)

Lane A matched the host import against eight real-game cameras (`reference/`, same location, rotation and FOV) over
four rounds. Each host capture was checked with the Inspection test and an independent visual critic. The saved-scene
verifier, the touched synthetic tests and the matched captures are the acceptance checks; no gameplay or live-game
parity was tested in these rounds. Every visual statement below is an **UNVERIFIED host approximation** unless it says
otherwise.

### Critic scores

The critic is an independent Codex GPT-6-Luna reviewer, given only the host and reference image paths for the eight
matched cameras. It scores out of 10.

| Pass | Score | What the critic saw |
| --- | ---: | --- |
| Baseline (2026-10-21, before round 1) | 4.5 | Six views. Top gaps: lighting and time of day, the map table and doorway glow, blank surfaces (map table, machine side panels, graffiti wall), missing mountains and street life, oversized tapestry light backings. Scored before any change, so not a before/after pair. |
| Round 1 | 4.7 | Matched-viewpoint parity pass (below). |
| Round 2 | 4.1 | Lighting, map table and decal changes. Decal noise regression: round-2 projected decals produced rainbow noise and magenta/violet quads. |
| Round 3 | 4.3 | Decal default-deny, calibrated map table and bounded static lights. |
| Round 4 | 4.4 | Warm host grade, thinner cloud cap, bulb emissive and `p_diffmod` diffuse tint. |
| PR #51 default state | 5.0 | After review 1: the round-4 grade, `p_diffmod` tint, bulb emissive, calibrated static lights and graffiti decals are opt-in and off; the 0.14 cloud cap, decoded source lighting, round-2 sun/sky calibration and the v2 map palette stay on. Per camera 4.2-6.1; top gaps: interior teal light, machine/Moxxi/screen emissive, graffiti and posters, sky bands, exterior grime and contrast. |

The gate of 8/10 was **not reached**. The lane stopped after four rounds, as briefed. One critic on eight cameras
moves by a few tenths between rounds; these scores are not a measured improvement.

### Current state per gap

- **Lighting and grading.** The directional light component now decodes: its property stream is found by the exact
  `LightmassSettings` anchor (round 2). Brightness and colour are read, and the host applies 5.0 UE5 lux per source
  brightness unit, a screenshot calibration. The SkyLight colour decodes with its UE3 FColor values converted to linear
  through a 2.2 power curve (`UNVERIFIED`); its main Brightness is absent from that component, so the host uses the
  calibration 1.5 and tint (3.0, 2.1, 1.0). HeightFog colour, density and start distance decode and feed the host fog.
  WorldInfo tone scale, midtones, shadows and bloom decode into `scene.json`; the mapping to exposure, gain, lift and
  bloom is `UNVERIFIED`. Host grade constants (exposure bias -1.5 EV, white balance 8200 K, saturation
  (1.12, 1.02, 0.92), contrast 1.10, shadow lift (0.03, 0.03, 0.035)) are screenshot calibrations, **not decoded**.
  The four PostProcessVolume settings records decode, but brush bounds and `bUnbound` do not, so they are exported
  with `settings_only_unverified` and **not applied**. The HQ interior is still greyer and less teal than the game.
- **Sky and clouds.** The native `Prop_Skybox.Meshes.Sky_Dome` is the approximation; the host adds a dome, not the
  game's sky shader. Cloud-cap opacity is scaled by the 0.14 calibration. Broad cloud bands and fine streaks remain
  different (cams 04 and 07).
- **Distant mountain.** Five `SkyboxMountains_low` placements use `Mat_MountainDistant`, approximated by the mostly
  snowy `SkyboxMountainSnow_Dif`. The rock/snow blend (`SkyboxMountain_Dif`) and its UV modulation are not
  reconstructed; the snow reads smaller and flatter than the reference (cams 02, 06).
- **Customization machine.** `CM_Mat` carries `p_emissive` with `p_EmissiveScale`, applied on the host. The
  banner-and-swirl recipe (`customization_banner_swirl_v1`) uses the cooked `CM_MachineBanner` as colour and
  `EnergySwirls_Dif_Tex` as cyan emissive, at the scale 3.0 recorded in round 2. The masked Hyperion eye is an opaque
  Unlit constant from its parent `p_EyeColorA`. The cyan hologram silhouette in the front view is **not in any cooked
  texture of the machine's materials**; the likely cause is an animated overlay or particle, which is not imported and
  is `UNVERIFIED`.
- **Moxxi sign.** The `moxxi_neon_masks_v1` recipe maps the two sparse mask atlases to emissive colours at a named
  calibration scale of 2.8 (round 1 used 1.8 and round 2 changed it). Hue, legibility and glow still differ.
- **Moxxi string lights and bulbs.** Three `LightStrip_02` placements use `light_strip_component_emissive_v1` from
  `LightStrip_Comp`. The calibration is 4.0; round 4a records an effective scale of 8.0, which is consistent with that
  4.0 applied to the recipe's [2, 2, 2] drive (an inference, not checked). The host strips still do not show clearly in
  cams 05 and 07. `WiringMesh_14` (`MAT_WireXMasWhite`) uses `xmas_bulb_emissive_v1`: warm `[1.0, 0.62, 0.28]` times
  4.0, from its `XmasLightBulbs_Dif` atlas. The bulbs are still faint on the host. Bulb palette, strength and bloom
  are `UNVERIFIED`.
- **HQ map table (Roland's).** `world_map_mask_colorization_v2` uses the three inspected `WorldMap_Comp` channels
  (R land/contours, G grid, B markers) over a pale-cyan backing with darker teal grid and marker layers. Cams 03 and
  08 show the pale-cyan table without the round-2 blotches. The palette comes from the `WorldMap_Comp` channel values
  and is `UNVERIFIED`. Route markings are still limited in the static view, and the map's animated markers are not
  imported.
- **Projector beam.** The beam is an `Engine.Emitter` particle component (`IO_RolandHQProjector`), not a mesh, so it is
  **not imported**. The package records name several candidate templates (round 1 pointed at a heater-steam template
  for the nearby emitters; round 2 listed five projector particle components). None is matched to the beam, so the
  identity is open.
- **HQ static lights.** Exported components come from `StaticLightCollectionActor.LightComponents`. The object array is
  declared in `tools/level-arrays.schema` as bulk decode metadata; the resolved elements are point and spot light
  components. Of 1,023 point/spot components, 128 are spawned: 895 are skipped (161 below thresholds, 650 by spatial
  quota, 58 by global cap, 26 unsupported). The imported lights are **shadowless**, use 25 candela per source
  brightness unit with a brightness ceiling of 3.0 (75 cd maximum), and two saturated-orange point lights get a 0.01
  intensity scale. All of these are host calibrations (`UNVERIFIED`). The orange pool is toned down, but the HQ is still
  greyer than the game's teal practicals.
  **Serialized tail (schema condition, `UNVERIFIED`).** After a `StaticLightCollectionActor`'s properties the reader finds
  an N x 64-byte tail, one 64-byte entry per `LightComponents` element. The reader size-checks it and skips it; it is not
  decoded. Its per-component layout is presumed to be a 4x4 transform, as for `StaticMeshCollectionActor`, and that is
  **UNVERIFIED**. Placement uses each component's decoded `CachedParentToWorld`. Synthetic reader coverage:
  `tests/properties_test.py` (N = 2 array, tail = 128 bytes, and both size-mismatch refusals).
  **Schema census (2026-10-22).** Declarations, over all 914 base-game `CookedPCConsole` packages (0 reader failures):
  129 packages have `LightComponents` in their name table, and the only exports of that name are
  `StaticLightCollectionActor.LightComponents` (`Core.ArrayProperty`) and its inner (`Core.ComponentProperty`, an
  object-reference property). Since a tagged property takes its type from the declaring class, every serialized
  `LightComponents` in the install is an array of object references, which is what the schema line declares. DLC, same method over all 1,096 `.upk` files under the install's `DLC` directory (the `--include-dlc` scope): 151 carry
  the name (instances of the base-game class) and none declares a property named `LightComponents`; 2 files failed with
  "invalid package magic" (`Mancana_Textures_Data.upk`, `Mancana_Textures_Meta.upk`, texture payload files with a `.upk`
  extension, which the reader cannot open at all). Instances, as a
  cross-check: a decode pass batched per export class with `--properties-batch --array-schema` stopped at its 30-minute
  budget after 666 packages; within those, 48 packages carry 119 occurrences, all owned by `StaticLightCollectionActor` and all
  decoded as `ObjectProperty` arrays, with no reader failure.
- **Decals and graffiti.** The census has 293 DecalActor/component records over 27 materials. **Default-deny** keeps 33
  placements that use only `Sanctuary_P:Prop_Decals.DMat_GrafittoBurrito`. The other 78 otherwise-supported candidates
  are excluded with a reason recorded per actor and material in `scene.json`: `DMat_WallConcrete_02` (66),
  `DMat_StainedWall_03` (4), `Dmat_WaterTank_Burst_Decal` (4), `Decal_Parent_BloodModulate` (3) and
  `SlaughterArena_Posters` (1). The reason in each case is that no channel/blend recipe was validated against the
  reference, and round-2 imports produced noise or quads. 182 candidates across 21 materials remain unsupported. The 46
  `DecalPlane_LiquidDev` placements stay on the static-mesh path.
  For the kept graffiti: the opacity is `1 - Rec.709 luminance`, capped at 0.55 and gained by 6.0, with dark ink
  `[0.001, 0.003, 0.008]`. Projection uses HitNormal and a 128 cm depth cap, with the UE3-to-UE5 axis mapping of local
  X/Y/Z. Blend, axis mapping and cap are `UNVERIFIED`. The tower writing reads darker and stays on its wall in cam 06,
  but it is less crisp than the game.
- **Building colour (facades).** The 18 Sanctuary building materials carry a `p_diffmod` diffuse tint read from
  `Master_World` instances (`diffuse_tint_v1`, `partial_unverified`). The host multiplies it into Base Color through a
  Multiply with the diffuse texture, and `verify_level.py` checks 18 of 18. The rust and orange facades still read pale
  in cams 02, 06 and 07. The likely cause is the omitted `Master_World` colour ramps and vertex paint (recorded in each
  material's `omitted` list). Game-side behaviour is `UNVERIFIED`.

### Host opt-in flags (current state)

Every round-4 host visual is behind an opt-in flag, default OFF, read from the environment variable
`OPENWILLOW_SANCTUARY_OPT_IN`: a comma-separated list of `grade` (round-4 warm white balance 8200 K, saturation, contrast
and shadow lift), `diffuse_tint` (the `p_diffmod` Base Color multiplier), `xmas_bulbs` (the `xmas_bulb_emissive_v1`
emissive hue and scale), `static_lights` (spawning the `StaticLightCollectionActor` point and spot lights with the
calibrated intensities) and `graffiti_decals` (the allowlisted `DMat_GrafittoBurrito` decal projectors). The 0.14
cloud-cap scale stays on. All of this applies only when `scene.json`'s map is `Sanctuary_P`. The source inputs are still
decoded and recorded in `scene.json` whatever the flags are. Only the Sanctuary persistent map gets the `lighting` key;
other maps have none. (Host side: `host/ue5/*.py`, implemented by the other lane-A agent.)

### Ranked remaining gaps (round-4 critic)

1. HQ interior grading and teal spill.
2. Map table detail and the projector beam.
3. Customization machine hologram (silhouette absent).
4. Moxxi neon and string-light bulbs weak.
5. Sky streaks and cloud bands.
6. Mountain snow brightness.
7. Graffiti pattern and colour.
8. Facade rust colour.
9. A dark patch under the Moxxi landing (cam 05).

### Round history

- **Round 1 (2026-10-21, matched-viewpoint parity pass).** Customization machine: the `p_EmissiveScale` that the importer
  omitted is now read, and the banner recipe replaces a generic gradient; the Hyperion eye becomes an opaque Unlit
  constant. Moxxi: `moxxi_neon_masks_v1` and `light_strip_component_emissive_v1` added. Distant mountain: the snowy
  texture replaces the rock-and-snow photo. Map table: `world_map_mask_colorization_v1` (R cyan, G red, B green) and a
  teal ClapTrap lens constant. Sky and lighting: `lighting` exported for the SkyLight, HeightFog, post-process settings
  and directional transform; the host omits its temporary `SkyAtmosphere` and blue shell when the dome is present. At
  the time the directional component's properties were reported as `invalid FName`; round 2 fixed that. Six real-game
  frames (Plan B save, 1280x720) were compared with host `-Inspect` captures at the same viewpoints. Critic 4.7.
- **Round 2 (2026-10-22).** The directional property stream is anchored on `LightmassSettings`; the sky light colour
  and WorldInfo tone decode. Host calibrations are named in `host/ue5/scene_import_rules.py`. The map table moves to
  pale-cyan and dark-teal palette (still `v1`). Decals: 111 candidates were imported, including 78 other materials
  whose projections produced noise and were excluded in round 3; the graffiti inverse-luminance rule was kept. The
  mountain snow was kept. Critic 4.1 (decal noise regression). The import
  gate ended `Success - 0 error(s)`; `ue-verify.json` then verified 111 of 293 decals, 182 unsupported, four
  PostProcessVolume settings and the colorized map recipe. Collision refresh matched all five blocking volumes.
- **Round 3 (2026-10-22).** Decal default-deny (33 kept, 78 excluded with reasons, 182 unsupported). Map table moved to
  the calibrated pale-cyan palette (`v2`). Static lights bounded (128 spawned, shadowless). Customization machine
  violet/magenta quads gone. Critic 4.3. The verifier's saved-scene assertions passed, but `UnrealEditor-Cmd` raised a
  Python 3.11 shutdown access violation after it, so `run_ue_level.ps1` reported `Saved scene verification failed: 3`.
  That was fixed in round 4d.
- **Round 4 (2026-10-22).** Warm host grade, thinner cloud cap (0.14), bulb emissive (`xmas_bulb_emissive_v1`), and
  `p_diffmod` diffuse tint applied on the host (4a and 4d). Shutdown fix: the decal-check loop in `verify_level.py` kept a
  bound method of a UE actor (`actor.get_actor_right_vector`) in a module global; a run with that global deleted
  exits 0, a run that keeps it exits 3. The loop now reads the axis vectors as values, and the full gate exits 0. The
  root cause inside the UE Python plugin's teardown is not analysed. Also fixed: an Unlit material whose visible colour is
  an `emissive_layers` recipe was checked with the plain-diffuse assertion; it is now covered only by
  `verify_texture_layers`. Critic 4.4.

### Where the matched shots live

Local gallery, ignored and outside the repository: `C:\Users\yorad\Documents\BL2_ReBased\local\hub-gallery\2026-10-22\laneA\`.
References are in `reference/`; each round has its own `host/` and `pairs/` folders (host left, reference right), and
round 3 also has `review/` for the four-way comparisons. Capture scripts are under ignored `local/laneA-capture/`. Automation results: the
eight-view Inspection test succeeded in each round. The import verifier (`ue-verify.json`) reports the decal, light
and map counts above, and the UE commandlet exits 0 since round 4d.

## Counts before and after

Both columns are the full Sanctuary pipeline (`prepare_level.py`, `prepare_terrain.py --collision`,
`prepare_bsp.py --collision`, then `prepare_sanctuary_pillar.py --umodel`), audited with
`tools/audit_scene_materials.py`. "Before" is `main` at 664b5fd with its own reader.

| Measure | Before | After |
| --- | ---: | ---: |
| Material definitions | 399 | 413 |
| Missing diffuse, all blend modes | 32 | 30 |
| Opaque missing diffuse: definitions / placed sections | 8 / 34 | 3 / 1 |
| Opaque with no supported channel: definitions / placed sections | 7 / 18 | 3 / 1 |
| Issues: unsupported component owner | 33 | 0 |
| Issues: invalid colour stream | 4 | 0 |
| Excluded components (listed with a reason) | 0 | 21 |
| Placed actors (all kinds) | 4,470 | 4,486 |

The audit now counts the eight terrain weighted-sum materials (15 placed sections) as `terrain_weighted_blend`. They
are host blends of the terrain's layer diffuses built by `prepare_terrain.py` and never had a single diffuse
channel; the earlier audit listed them as "no supported channels". Under the earlier rule the "before" row reads 40
missing, 16 opaque definitions / 49 sections, 15 with no channel / 33 sections. The ROADMAP figures (44 / 12 / 42 /
9 / 20) predate the terrain, moon-base and snow-terrain work and are superseded by this table.

## Material fallbacks

Each remaining opaque gap was read from its parameter chain (instance overrides over base defaults) and its cooked
texture list, and the candidate textures were decoded and looked at. Every new rule is an exact entry in
`INSPECTED_COLOR_FALLBACKS` (`tools/prepare_level.py`): it names the material, the texture it uses as colour and
the inputs it omits, refuses to apply when the named texture is missing from the cooked list, and records a
`partial_unverified` surface approximation. None of them reconstructs the stripped graph.

| Material (placed sections) | Evidence | Colour used | Omitted |
| --- | --- | --- | --- |
| `Mat_Icicles` (16) | `Ice_Dif` is the named `P_InternalGlowTexture`, tinted by `P_InternalGlowColor`; the cube is `P_SimpleReflect` | `SnowDrift_01_Dif`, the one unnamed `_Dif` left; the named `IcicleMultiple_Nrm` stays the normal | internal glow, reflection |
| `Mat_PatchySnow_Skybox01` (10) | no named samplers; `C1`/`C2` tint and fresnel vectors | `DirtySnow_Dif` (same reading as `Mat_PatchySnow`) | noise-masked snow/gravel blend, tints |
| `Mat_MountainDistant` (5) | no named samplers; `SkyboxMountain_Dif` is a rock-and-snow photo, `SkyboxMountainSnow_Dif` mostly white | previously `SkyboxMountain_Dif`; changed in round 1 to `SkyboxMountainSnow_Dif` | rock/snow layer |
| `Mat_DirtySnowDistant` (1) | as PatchySnow_Skybox01 | `DirtySnow_Dif` | snow blend, tints |
| `Mat_ChemTank02` (1) | the tank atlas next to a smoke overlay `_Dif`; cube is `P_SimpleReflect` | `ChemTank02_Dif` | packed `_Comp` mask, six colour vectors, reflection, smoke |
| `Mati_SancSpire_Base` / `_Tile` (6) | `Master_SancSpire` HLS master | already baked by `prepare_sanctuary_pillar.py` (DECISIONS 2026-09-23), which also serves the static spire arm and base placements | as recorded there |

`Mat_WorldMap` (one section, Roland's map table) is no longer a fallback of this kind. It uses the three inspected
`WorldMap_Comp` channels through `world_map_mask_colorization_v2` (see the matched-viewpoint section above); the
channel-to-palette mapping is `UNVERIFIED`.

Not changed: `Mati_DryGrassRocks` and `Mat_IceRoad` have no effective placed section; the 13 non-opaque gaps
(collision, water, fog sheets, title cards, transition planes, the masked `_Teleported` hull) keep their existing
policies.

## Component owners

`prepare_level.py` placed only components owned by `StaticMeshActor` and `InterpActor` (or a collection). The 33
others in Sanctuary, by owner class:

| Owner class | Components | Evidence | Disposition |
| --- | ---: | --- | --- |
| `WillowMissionPickupSpawner` | 14 | each actor serializes the component as its `PreviewMesh`, next to a `MissionItemDef` | excluded, `mission_item_preview_mesh` |
| `WillowMissionPickupFailsafe` | 2 | same | excluded, `mission_item_preview_mesh` |
| `GearboxFramework.WiringActor` | 1 | a `WiringMesh` (generated cables, world-space vertices, actor at (1, 1, 1)) | placed |
| `WillowInteractiveObject` | 16 | live: attached, not hidden, package mesh and relative pose (5 vault-symbol actors serialize the same plane twice; the game attaches one) | 11 placed, 5 exact duplicates excluded (`duplicate_component`) |

Owner properties are not in the bulk scene records (their class filter covers only engine placement classes), so
`Scene.owner_properties` reads them per actor with the same 26-byte actor prefix and requires the decode to end at the
object's end (all 25 Sanctuary owners did, 0 trailing bytes). Excluded components are written to the manifest's
`excluded_components` with their reason and counted by the audit; they are not issues.

Live read (2026-10-21, `tools/real_game/scripts/scene_owners.py`, level-8 Maya at Plan B, saves blocked from the
first frame and byte-identical afterwards, driver removed): the 14 spawners and 2 failsafes have no primitive component
at all in the running game, which supports the preview reading. Every interactive-object component the importer now
places exists live with `HiddenGame` false, attached, with the package's mesh, materials and relative translation,
rotation and scale (vault symbols, Moxxi's piano, the customization machine, Claptrap's stash, Roland's projector
eye, the bounty board's transporter and board). Each of the five vault-symbol actors attaches one
`BasePlane_256x128_Col` plane while the package serializes two identical components; the first is placed and the
copy is listed. A copy only counts as a duplicate when it also matches on `HiddenGame`, shadow casting and the collision flags, so a hidden and a visible component are never merged (review round 1); the five Sanctuary pairs match on all of them, and the counts did not change. What the game draws for these objects in other mission or challenge states (a vault symbol after it
is found, the projector during Episode 10) is not interpreted and stays `UNVERIFIED`. The live game also shows
interactive objects whose meshes come only from their definitions at runtime (for example Roland's HQ TV, the fuel-cell
pylons, the cardboard boxes); the package has no placed component for them, so they are outside this count.

The newly placed vault symbols (`VaultRoy_INST`) and the blood decal are Modulate materials with no recovered colour.
The host now gives such a material a white neutral, which leaves the scene unchanged, instead of 0.5 grey, which would
have darkened the wall behind it. Three new non-opaque gaps follow from the new placements (`VaultRoy_INST`, 5
sections; two masked Claptrap/Hyperion lens materials, 2 sections each).

## Colour streams and WiringMesh (reader change, `src/assets.cpp`)

The four "invalid color stream" placements are two meshes: `Prop_Carnival.Meshes.Tapestry_01` (three placements,
195 vertices) and `Common_Meshes.BasePlane_256x256` (one blood-decal placement, 4 vertices). Both store a colour
buffer of stride 4 whose count and bulk count are exactly twice the vertex count. A throwaway diagnostic build
(local only) read the first `vertexCount` entries, skipped the rest and ran a census over every StaticMesh and
WiringMesh export in the 914 base-game packages:

| Result | Exports |
| --- | ---: |
| StaticMesh, ordinary colour buffer | 17,526 |
| StaticMesh, doubled colour buffer | 27 |
| WiringMesh | 52 |
| Failed | 0 |

Every one of the 17,605 exports left exactly 73 bytes after its LODs, the same trailer as the ordinary meshes; that
is the structural oracle (sizes meet). The reader now accepts a colour count of `vertexCount` or `2 × vertexCount`
(stride 4, bulk count equal to the count), reads the first `vertexCount` entries and skips the remainder. Indices
are already checked to stay below `vertexCount`, so the skipped entries cannot be referenced by our geometry; which
entries the original renderer binds is `UNVERIFIED`, and vertex colours are not exported by the OBJ writer. It also
accepts the export class `WiringMesh` alongside `StaticMesh`; the census is the evidence that its layout matches.
No bounds check was relaxed: counts are still bounded by the payload, and other colour counts still fail.

## Ash and Southpaw re-measured

Same tools, `main` (664b5fd) versus this branch at 0dfa2c2, `prepare_level.py` only (neither map has prepared
terrain or BSP). An independent review run at 0dfa2c2 reported the same owner numbers.

| Map | Placements | Opaque missing diffuse (defs / sections) | Unsupported owners | Interactive objects placed | Excluded (previews / duplicates) | Colour-stream issues |
| --- | ---: | --- | ---: | ---: | --- | ---: |
| Ash_P before | 5,059 | 13 / 117 | 325 | 0 | 0 / 0 | 0 |
| Ash_P after | 5,082 | 13 / 117 | 271 | 21 | 14 / 17 | 0 |
| SouthpawFactory_P before | 3,987 | 0 / 0 | 10 | 0 | 0 / 0 | 0 |
| SouthpawFactory_P after | 3,992 | 0 / 0 | 1 | 4 | 4 / 0 | 0 |

The owner rules are not Sanctuary-specific, so they apply on every map: Ash places 21 interactive-object components
and lists 17 exact duplicates (21 + 17 = its 38 interactive-object components), plus two cables; Southpaw places 4
and one cable. **Interactive-object placement outside Sanctuary is `UNVERIFIED`:** only Sanctuary was compared with
the running game, so whether every Ash and Southpaw component is attached and visible in game, and whether each
listed duplicate is really the copy the game drops, has not been checked. The material fallbacks are Sanctuary
entries, so Ash's 13 opaque gaps (volcanic ash, lava, alien plaster, cube windows and others) are unchanged and remain
open. Ash's 271 remaining owners are `Perch` (128) and `WillowPopulationPoint` (116) AI markers, vehicle-station
platforms and terminals (18), `WillowPropObject` (8) and one `KActor`; Southpaw's one is a `KActor`. They are not
classified by this pass.

The duplicate test compares raw serialized values, so an omitted flag (`None`) and an explicitly serialized default
differ and such a pair would both be placed. That errs towards drawing both; no such pair was found on Sanctuary, Ash
or Southpaw.

## External-tool benchmarks (UE Viewer / UModel)

Both benchmarks used UE Viewer build 1590 (compiled 2022-12-23), from the [UE Viewer repository](https://github.com/gildor2/UEViewer),
whose source states the MIT license. The binary is local and outside the repository. Neither run replaced the package
reader for object identity, references, materials, provenance or verification.

- **Customization-machine mesh (2026-10-21).** `tools/export_with_umodel.ps1 -UModel <umodel.exe> -Cooked <CookedPCConsole> -Package Sanctuary_P -Object CustomizationMachine`.
  The object-only run took about 0.13 seconds and wrote three files (run log, glTF and binary payload; 103,047 bytes)
  under ignored `local/external/umodel/`. It emitted no texture files and no duplicate exports. Warnings included
  unsupported `RB_BodySetup`, missing `Common_Materials.Master_World` dependency resolution and unknown material-expression
  editor fields. The check confirms a useful geometry and material-dependency inspection path only.
- **Central pillar (round 2).** `prepare_sanctuary_pillar.py --umodel <umodel.exe> --game <BL2> --reader build/Release/ow-package.exe --scene local/sanctuary/scene.json --output local/sanctuary`.
  The end-to-end command took 19.5 seconds and exported one MD5 mesh and two animation files, plus four material
  descriptors, five textures and four text sidecars (18 files including two logs). The reconstructed mesh has four
  sections, 5,811 vertices and 5,280 triangles in ignored `local/sanctuary/center-pillar-umodel-5b97e151`. There were
  zero `ERROR:` markers and zero duplicate basenames. UModel logged unknown SkeletalMesh3/LODInfo and
  material-expression editor-coordinate fields and dropped four bytes from each of five texture exports; those fields
  are not used as parser truth. This is a bounded pillar benchmark only.

## Regeneration and import (first pass, 2026-10-21)

The first pass ran the pipeline below. Its collision counts are superseded by the round 2 and round 3 numbers above.

```powershell
python tools/prepare_level.py --reader build/Release/ow-package.exe --game 'C:/Program Files (x86)/Steam/steamapps/common/Borderlands 2' --map Sanctuary_P --output local/sanctuary
python tools/prepare_terrain.py --reader build/Release/ow-package.exe --game 'C:/Program Files (x86)/Steam/steamapps/common/Borderlands 2' --scene local/sanctuary --collision
python tools/prepare_bsp.py --reader build/Release/ow-package.exe --game 'C:/Program Files (x86)/Steam/steamapps/common/Borderlands 2' --scene local/sanctuary --collision
$env:PYTHONPATH = Join-Path (Get-Location) 'local/python-deps'
python tools/prepare_sanctuary_pillar.py --umodel 'C:/Users/yorad/Tools/UEViewer/umodel.exe' --game 'C:/Program Files (x86)/Steam/steamapps/common/Borderlands 2' --reader build/Release/ow-package.exe --scene local/sanctuary/scene.json --output local/sanctuary
python tools/refresh_collision.py --reader build/Release/ow-package.exe --game 'C:/Program Files (x86)/Steam/steamapps/common/Borderlands 2' --scene local/sanctuary
```

The run produced 4,446 base placements, 386 base materials and 141 issues; terrain preparation added 15 components and
19 materials, BSP preparation read 2 models / 24 components / 105 sections, and the pillar exporter added its four local
sections. `prepare_sanctuary_pillar.py` required NumPy in addition to the installed Pillow; NumPy was installed with
`python -m pip install --target local/python-deps numpy` under ignored `local/python-deps`.

`refresh_collision.py` in this first pass printed 312 supported, 115 absent, 2 unsupported, 518 convex hulls, 4
triangle-mesh policies, 3 blocking volumes, 4 dynamic-unobserved volumes, 4 non-blocking volumes, 0 live mismatches and
0 unsupported volume records. The prepared UModel SkeletalMesh pillar has no StaticMesh collision export, so collision
refresh skips that one exact pose-tagged asset (`skeletal_meshes_skipped: 1`). The round 2 and round 3 refreshes count
five blocking volumes; the 2026-10-22 record is in `COLLISION_WALKING_VERIFICATION.md`.

The follow-up manifest audit reports 4,914 placed sections, six partial scene recipes covering 13 sections, zero
unassigned sections, and two remaining opaque missing-diffuse definitions with no placed sections. Those counts describe
manifest coverage; they do not validate rendered appearance.

Import in the UE editor with `tools/run_ue_level.ps1 -ImportOnly`; then `tools/test_ue_viewer.ps1 -Inspect` gives the
matched captures. Since round 4d the commandlet exits 0 after the saved-scene verification passes.

## Reproduce

```powershell
python tools/prepare_level.py --reader build/Release/ow-package.exe --game $env:OPENWILLOW_BL2 --map Sanctuary_P --output local/sanctuary
python tools/prepare_terrain.py --reader build/Release/ow-package.exe --game $env:OPENWILLOW_BL2 --scene local/sanctuary --collision
python tools/prepare_bsp.py --reader build/Release/ow-package.exe --game $env:OPENWILLOW_BL2 --scene local/sanctuary --collision
python tools/prepare_sanctuary_pillar.py --umodel $env:OPENWILLOW_UMODEL --game $env:OPENWILLOW_BL2 --reader build/Release/ow-package.exe --scene local/sanctuary/scene.json --output local/sanctuary
python tools/audit_scene_materials.py --scene local/sanctuary --output local/sanctuary/material-audit.json
python tests/level_test.py; python tests/material_audit_test.py
```

The live read is `tools/real_game/scripts/scene_owners.py` through `Invoke-GamePyFile` (docs/TOOLING.md, "Driving
the real game").
