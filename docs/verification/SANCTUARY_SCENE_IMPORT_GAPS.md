# Sanctuary scene import gaps: material fallbacks, component owners, colour streams (2026-10-21)

AI-assisted (Claude), lane A, branch `feature/scene-materials-owners`. Evidence comes from the installed base game
through this repository's reader, a census over every base-game package and one live read of the running game.
Every report, texture, scene and capture stays under ignored `local/` (`local/lanea/`, `local/realgame/`).

## Matched-viewpoint round 3 (2026-10-22, lane A, `feature-sanctuary-parity-r2`)

This round uses the installed-package reader and project-owned scene importer for package identity, object references,
scene manifests, materials, decals, lights and verification. UE Viewer/UModel build 1590 (local install; repository:
[Gildor/UEViewer](https://github.com/gildor2/UEViewer)) remains the external extraction path for the selected central
pillar mesh. All extracted outputs and captures stayed under ignored `local/` or
`C:\Users\yorad\Documents\BL2_ReBased\local\hub-gallery\2026-10-22\laneA\round3`. The acceptance checks for this
visual slice are the saved-scene verifier, the touched synthetic/import tests and eight matched captures; no gameplay
or live-game parity was tested here.

| Area | Round-3 change and evidence | Remaining work / confidence |
| --- | --- | --- |
| Decal default-deny | Census: 293 DecalActors/components, 27 unique materials. Kept 33 placements using only `Sanctuary_P:Prop_Decals.DMat_GrafittoBurrito`. Excluded 78 otherwise-supported candidates across five materials, with the reason recorded per actor and material in `scene.json`: no channel/blend recipe validated against the matched reference; round-2 imports produced rainbow noise or magenta/violet projected quads. Exclusions: `DMat_WallConcrete_02` (66), `DMat_StainedWall_03` (4), `Dmat_WaterTank_Burst_Decal` (4), `Decal_Parent_BloodModulate` (3), and `SlaughterArena_Posters` (1). 182 candidates across 21 unresolved materials remain unsupported. The 46 `DecalPlane_LiquidDev` placements remain on the static-mesh path. | The kept graffiti uses inverse Rec.709 luminance (`ceiling=0.55`, `gain=6.0`) and dark ink `[0.001, 0.003, 0.008]`. Its local-X projection uses HitNormal and a 128 cm depth cap, with UE3 NearPlane/FarPlane and HitNormal/Tangent/Binormal to UE5 local X/Y/Z mapping. The tower writing reads darker and stays on its receiver in camera 06; blend, axis mapping and cap remain `UNVERIFIED`, and the paint is less crisp than the game. |
| HQ map table | `WorldMap_Comp` R/G/B remain calibrated as land/contours, grid and markers. The `world_map_mask_colorization_v2` recipe uses a pale-cyan backing and land layer with darker teal grid/marker layers. Cameras 03/08 show the pale-cyan table and darker grid without round-2 blotches. | The route markings remain limited in the static view. The floating map symbol/beam is not present; emitter/particle behavior is outside this importer. Palette and channel mapping remain `UNVERIFIED`. |
| Customization machine | Default-deny removes the violet/magenta projected quads seen in round 2; the cyan `EnergySwirls` layer stays in the machine material recipe. Camera 01 has no purple streaks. | The front display still lacks the game's cyan silhouette. A separate machine overlay/emitter or unreconstructed instance graph is a plausible cause, not verified. |
| HQ static lights | Exported components from `StaticLightCollectionActor.LightComponents`; 128 of 1,023 point/spot components imported, 895 skipped (161 below radius/brightness thresholds, 650 spatial quota, 58 global cap, 26 unsupported). Imported lights preserve decoded radius/color, cast no shadows, and apply 25 candela per source-brightness unit capped at source brightness 3.0 (75 cd maximum); two saturated-orange point lights receive a 0.01 intensity scale. | Unit conversion, general ceiling and orange-point scale are named `UNVERIFIED` host calibrations. The round-3 capture tones down the artificial orange pool; the HQ remains greyer than the game's teal practicals and the projected beam is absent. |
| Outdoor/Moxxi/sky | Retained round-2 outdoor light calibration. Camera 02 remains brighter and more readable than round 1. | Cameras 04/07 still show broad cloud bands; camera 05 string bulbs/sign remain dim or cropped; camera 02 distant mountain contrast remains flatter than the reference. These are lower-priority visual gaps. |

Per-camera review against round 1: 01 better (no purple projection; front cyan panel absent); 02 better (outdoor lighting;
mountain contrast still open); 03 better overall (cyan table, but route/projector detail remains open); 04 better overall
(sky banding remains); 05 better overall (Moxxi lights remain faint); 06 better (dark graffiti); 07 better overall (sky
and bulbs remain); 08 better overall (cyan table and decoded source lights, with HQ teal fill/projector still open).
Round-3 host shots, host-left reference pairs and four-way comparisons are stored in `round3/host`, `round3/pairs`
and `round3/review` beneath the local gallery path above. Visual checks are separate from automation: the eight-view
Inspection test succeeded, while the UE commandlet's Python verification script reported success and wrote
`ue-verify.json` (4,914 section actors, 33 verified decals, 78 default-denied, 182 unsupported, 128 verified lights,
895 skipped; zero script errors). `UnrealEditor-Cmd` then raised a Python 3.11 shutdown access violation and
`run_ue_level.ps1` reported `Saved scene verification failed: 3`; therefore the saved-scene assertions passed, but the
commandlet process exit did not. No Smart App Control block occurred.

The refreshed local pipeline produced 4,446 base placements and 386 base materials; terrain added 15 components,
5,195 cells and 10,390 triangles; BSP preparation read 2 models, 24 components and 105 sections; the UModel pillar has
four sections. Collision refresh found 312 supported, 115 absent and 2 unsupported mesh collision records, 518 hulls,
five blocking volumes and zero live-volume mismatches. Material audit covered 413 materials / 4,914 placed sections
with zero unassigned sections. These are extraction/manifest counts, not visual or gameplay validation.

## Matched-viewpoint round 2 (2026-10-22, lane A)

The eight matched host/game pairs are under `C:\Users\yorad\Documents\BL2_ReBased\local\hub-gallery\2026-10-22\laneA\round2\pairs` (host left); host captures are in the sibling `round2\host`. These are visual checks of the static Sanctuary import, not gameplay or original-game parity tests. No game assets or exports were added to git.

| Gap | Change and evidence | Result and remaining work |
| --- | --- | --- |
| Lighting and grading | The directional component has a serialized lightmap prefix before its UProperties. The previous offset-zero read started on non-property bytes. `prepare_level.py` now locates the exact `LightmassSettings` FName in the component payload and accepts only a complete property stream whose first decoded property is that anchor and which contains the required light fields with no trailing bytes. It also exports WorldInfo defaults and all four PostProcessVolume settings records. | Named, screenshot-calibrated host constants in `scene_import_rules.py`: `SANCTUARY_DIRECTIONAL_LUX_PER_SOURCE_BRIGHTNESS=20.0` (140 UE5 lux for the observed source brightness), `SANCTUARY_SKY_INTENSITY_CALIBRATION=0.75`, and `SANCTUARY_AUTO_EXPOSURE_CALIBRATION_BIAS=-1.5`. These are host calibration, not decoded UE3 units, and remain `UNVERIFIED`. Round-2 shots reduced the clipped highlights, but outdoor views still have hard bright surfaces and dark blue shadows; the HQ remains bluer than the game's green-cyan ambient. The sky-light component has no serialized main Brightness. The WorldInfo mapping (tone scale to exposure stops, midtones to gain, shadows to offset, bloom to intensity) is `UNVERIFIED`. Four local volume settings are recorded but not applied because `bUnbound` and brush bounds were not decoded. |
| Roland's HQ map table | `world_map_mask_colorization_v1` maps `WorldMap_Comp` R artwork to pale cyan, G grid to dark teal, and B markers to dark teal. | Camera 03 now reads cyan instead of the round-1 red/white grid, but the route markings remain faint and the display is flatter than the reference. Palette, reflection, and marker animation remain `UNVERIFIED`. |
| Tower graffiti and decals | Census: 293 DecalActors/components, 27 unique material references, 245 supported candidates across 16 materials, and 48 unsupported candidates across 11 unresolved materials. Supported actors are imported as deferred decals and independently checked for transform, normal, size, material graph and source tag. A further 46 water-effect `DecalPlane_LiquidDev` placements stay on the already-verified static-mesh path. `DMat_GrafittoBurrito` is a single-texture Modulate material; its local exported PNG has fully opaque alpha, so using alpha painted the white background. The host now derives opacity as `1 - Rec.709 RGB luminance` for Modulate decals. | The inverse-luminance rule is a narrow `UNVERIFIED` approximation of the native Modulate graph. Its material graph and all 245 supported decal actors pass the UE import verifier (0 errors); a post-change screenshot is still pending. Other unsupported materials remain enumerated in the ignored scene manifest. |
| Map-table projector | The map-table-near `Engine.Emitter` records resolve to `FX_MysteryMeat.Particles.Props.Part_HeaterSteam`; three nearby emitters share that template. Sanctuary_P has seven Emitter actors in total. | This is an identity candidate from package data, not a verified match for the cyan beam/symbol. No particle importer was added. |
| Moxxi string lights | Three `Prop_Carnival.LightStrip_02` placements and one `WiringMesh` placement are present. The inspected `LightStrip_Comp` recipe drives emissive RGB at `[2,2,2]`. | Geometry/import coverage exists; the host string-light appearance still differs from cameras 05/07. Native bulb palette and bloom are `UNVERIFIED`. |
| Moxxi sign | The scoped `moxxi_neon_masks_v1` recipe uses a named emissive calibration scale of 1.8 against round-1 camera 05. | The host sign is brighter, but its hue, legibility and glow still differ from the reference. The scale is not decoded from the source game. |
| Distant mountains | Five `SkyboxMountains_low` placements use the inspected `Mat_MountainDistant` and the mostly snowy `SkyboxMountainSnow_Dif` fallback. | The snow silhouette is visible in camera 02; its layer blend, size and sky lighting remain different and `UNVERIFIED`. |
| Customization machine | `customization_banner_swirl_v1` and the scoped emissive calibration use a host scale of 1.8. | Camera 01 still lacks the reference's bright cyan display/hologram. Banner, mask and animation are approximations. |
| Sky | No separate cloud tuning was added in this pass. | Camera 04/07 still shows broader, brighter cloud bands than the game. Low priority and `UNVERIFIED`. |

### External-tool benchmark

UE Viewer / UModel was used locally for the selected Sanctuary central-pillar skeletal mesh: `prepare_sanctuary_pillar.py --umodel 'C:/Users/yorad/Tools/UEViewer/umodel.exe' --game 'C:/Program Files (x86)/Steam/steamapps/common/Borderlands 2' --reader build/Release/ow-package.exe --scene local/sanctuary/scene.json --output local/sanctuary`. The installed build is 1590 (compiled 2022-12-23), from the [UE Viewer repository](https://github.com/gildor2/UEViewer), whose source states the MIT license. The end-to-end command took 19.5 seconds. It exported one MD5 mesh and two animation files, plus four material descriptors, five textures and four text sidecars (18 files including two logs); the reconstructed mesh has four sections, 5,811 vertices and 5,280 triangles in ignored `local/sanctuary/center-pillar-umodel-5b97e151`. There were zero `ERROR:` markers and zero duplicate basenames. UModel logged unknown SkeletalMesh3/LODInfo and material-expression editor-coordinate fields and dropped four bytes from each of five texture exports; those fields are not used as parser truth. This is a bounded pillar benchmark only; other asset categories were not benchmarked. Our parser/importer continues to own package/object identity, references, transforms, materials, provenance and verification.

## Matched-viewpoint parity pass (2026-10-21, feature/sanctuary-parity-r2)

This pass uses the five matched pairs in `C:\Users\yorad\Documents\BL2_ReBased\local\hub-gallery\2026-10-21\`
(host left, game right) and reads the local cooked packages. The screenshots stayed outside the repository. The material
recipes below record `partial_unverified`; no post-change UE import, screenshot comparison, or real-game check was run
in this lane.

| Gap | Package finding and change | Remaining work | Confidence |
| --- | --- | --- | --- |
| Customization machine | `CM_Mat` already had named `p_emissive` -> `CM_Comp`, but the importer omitted the chain's `p_EmissiveScale`; it now reads and applies that vector. The second machine material used a generic gradient despite cooked `CM_MachineBanner` and `EnergySwirls_Dif_Tex`; `customization_banner_swirl_v1` uses the banner as color and swirls as cyan emissive. The masked Unlit Hyperion eye had no supported channels, so Material v1 made it transparent; exact eye instances now use their parent `p_EyeColorA` as an opaque Unlit constant. | The runtime side-panel instance reported in the prior live read is not decoded. The panel color, swirls, mask and animation are still approximations. | High for resource identity and missing scale; medium for layer choice; `UNVERIFIED` visually. |
| Moxxi entrance | `Mat_SanctuaryMoxxis` has the diffuse plus two sparse mask atlases; `Mati_LightStrips_01` has `LightStrip_Comp` plus a diffuse. Existing inference treated only diffuse as visible. `moxxi_neon_masks_v1` maps inspected mask channels to emissive colors, and `light_strip_component_emissive_v1` uses the component atlas as emissive RGB. | Native palette, bulb emission strength and bloom are not decoded. These host colors are approximations. | High for texture dependencies; medium for mask-to-color mapping; `UNVERIFIED` visually. |
| Distant mountain | `Mat_MountainDistant` cooked both `SkyboxMountain_Dif` and `SkyboxMountainSnow_Dif`; the previous fallback chose the dark rock-and-snow photo and omitted the mostly snowy layer. `distant_mountain_snow_color_fallback_v1` now selects the snowy texture. | Rock/snow blend and UV modulation are not decoded. | High for texture identity; medium for intended layer; `UNVERIFIED` visually. |
| HQ map table/projector | `Mat_WorldMap` cooks `WorldMap_Comp` and a reflection cube. The texture has separate R/G/B masks, not pre-colored land/route pixels; the former unsupported-channel path left the neutral table face. `world_map_mask_colorization_v1` colorizes R as cyan land, G as red routes/grid and B as green markers, then emits an Unlit display. The ClapTrap lens now has a teal Unlit constant. | The projector avatar is an `Engine.Emitter`/particle component omitted by this mesh-only importer; its scene placement is visible in the records but its system is not imported. The table palette and animated markers remain approximations. | High for texture/channel evidence; medium for palette; projector effect remains open; `UNVERIFIED` visually. |
| Sky and lighting | The scene already carries the observed `Prop_Skybox.Meshes.Sky_Dome` approximation. When an accepted dome is present, the host now omits its temporary UE5 `SkyAtmosphere` and blue shell. `lighting` now exports the placed SkyLight, HeightFog, all decoded post-process settings, the directional actor transform, WorldInfo presence and baked-light component counts. The host reuses `OpenWillow_SkyFill` with the decoded SkyLight colors and adds the decoded HeightFog color, density and start distance. | UE3-to-UE5 intensity/density interpretation is `UNVERIFIED`. The directional component properties fail with `invalid FName`, so only transform is known and the host Sun remains an inspection stand-in. Four PostProcessVolume settings sets are decoded, but brush bounds/unbound flags are not; they are exported and not applied. WorldInfo properties contain no `DefaultPostProcessSettings`. Baked collections are not spawned as dynamic lights. | High for decoded actor/property identity; low-to-medium for host light interpretation; `UNVERIFIED` visually. |

The generated local light census finds one `DominantDirectionalLight`, one `SkyLightToggleable`, one HeightFog and four
PostProcessVolume actors; there are no placed PointLight or SpotLight actors in the loaded levels. The directional
actor's decoded rotation now aims the host Sun, while host intensity/color remain stand-ins. Its component properties
still fail with `invalid FName`. The 13 `StaticLightCollectionActor` records contain 962 point and 61 spot components;
six more point components belong to `InteractiveObjectDefinition` records and one belongs to a `WillowAIPawn`. All
are left out of the static scene import: the collection lights are baked and the other components have definition or
AI owners rather than verified placed light actors. Four PostProcessVolume settings records are retained in
`local/sanctuary/scene.json` with `settings_only_unverified` status.

### External-tool check

UModel / UE Viewer was benchmarked against the customization-machine mesh with
`tools/export_with_umodel.ps1 -UModel 'C:\Users\yorad\Tools\UEViewer\umodel.exe' -Cooked 'C:\Program Files (x86)\Steam\steamapps\common\Borderlands 2\WillowGame\CookedPCConsole' -Package Sanctuary_P -Object CustomizationMachine`.
It was UE Viewer build 1590 (source: [Gildor/UEViewer](https://github.com/gildor2/UEViewer)); the object-only run took
about 0.13 seconds and wrote three files total (run log, glTF and binary payload; 103,047 bytes) under ignored
`local/external/umodel/Sanctuary_P-20261021-231419985-dcdca202`. It emitted no texture files and no duplicate exports.
Warnings included unsupported `RB_BodySetup`, missing `Common_Materials.Master_World` dependency resolution and unknown
material-expression editor fields. The test confirms a useful geometry/material-dependency inspection path, but it did
not replace the package reader for material identity or texture extraction.

### Regeneration and import

The fresh run used `build/Release/ow-package.exe` and produced 4,446 base placements, 386 base materials and 141 issues;
terrain preparation added 15 components and 19 materials, BSP preparation read 2 models / 24 components / 105 sections,
and the pillar exporter added its four local sections. `prepare_sanctuary_pillar.py` required NumPy in addition to the
installed Pillow; NumPy 2.5.4 was installed with `python -m pip install --target local/python-deps numpy` under ignored
`local/python-deps` for that run. Commands:

```powershell
python tools/prepare_level.py --reader build/Release/ow-package.exe --game 'C:/Program Files (x86)/Steam/steamapps/common/Borderlands 2' --map Sanctuary_P --output local/sanctuary
python tools/prepare_terrain.py --reader build/Release/ow-package.exe --game 'C:/Program Files (x86)/Steam/steamapps/common/Borderlands 2' --scene local/sanctuary --collision
python tools/prepare_bsp.py --reader build/Release/ow-package.exe --game 'C:/Program Files (x86)/Steam/steamapps/common/Borderlands 2' --scene local/sanctuary --collision
$env:PYTHONPATH = Join-Path (Get-Location) 'local/python-deps'
python tools/prepare_sanctuary_pillar.py --umodel 'C:/Users/yorad/Tools/UEViewer/umodel.exe' --game 'C:/Program Files (x86)/Steam/steamapps/common/Borderlands 2' --reader build/Release/ow-package.exe --scene local/sanctuary/scene.json --output local/sanctuary
python tools/refresh_collision.py --reader build/Release/ow-package.exe --game 'C:/Program Files (x86)/Steam/steamapps/common/Borderlands 2' --scene local/sanctuary
```

`refresh_collision.py` records three statically blocking volumes and four `dynamic_unobserved` volumes without a live
volume capture. The expected count of five blockers was not verified; a parent-provided `--live-volumes` observation is
needed to determine whether the dynamic volumes raise the count to five. A prepared UModel SkeletalMesh pillar has no StaticMesh collision export,
so collision refresh skips that one exact pose-tagged asset and records `skeletal_meshes_skipped: 1`; ordinary
StaticMesh and WiringMesh provenance checks still run. This invocation printed 312 supported, 115 absent, 2 unsupported,
518 convex hulls, 4 triangle-mesh policies, 3 blocking volumes, 4 dynamic-unobserved volumes, 4 non-blocking volumes,
0 live mismatches and 0 unsupported volume records.

The follow-up manifest audit reports 4,914 placed sections, six partial scene recipes covering 13 sections, zero
unassigned sections, and two remaining opaque missing-diffuse definitions with no placed sections. Those counts describe
manifest coverage; they do not validate rendered appearance.

After this scene is ready, import in the UE editor with
`tools/run_ue_level.ps1 -ImportOnly`; then the parent can compare with `tools/test_ue_viewer.ps1 -Inspect`. The importer
must be rebuilt only if C++ changes; this pass changes only Python import code, so no editor module rebuild is needed.

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
| `Mat_MountainDistant` (5) | no named samplers; `SkyboxMountain_Dif` is a rock-and-snow photo, `SkyboxMountainSnow_Dif` mostly white | previously `SkyboxMountain_Dif`; changed in the matched-viewpoint pass below to `SkyboxMountainSnow_Dif` | rock/snow layer |
| `Mat_DirtySnowDistant` (1) | as PatchySnow_Skybox01 | `DirtySnow_Dif` | snow blend, tints |
| `Mat_ChemTank02` (1) | the tank atlas next to a smoke overlay `_Dif`; cube is `P_SimpleReflect` | `ChemTank02_Dif` | packed `_Comp` mask, six colour vectors, reflection, smoke |
| `Mati_SancSpire_Base` / `_Tile` (6) | `Master_SancSpire` HLS master | already baked by `prepare_sanctuary_pillar.py` (DECISIONS 2026-09-23), which also serves the static spire arm and base placements | as recorded there |

`Mat_WorldMap` (one section, Roland's map table): this was the state before the matched-viewpoint pass. The local importer now uses the three inspected `WorldMap_Comp` channels through `world_map_mask_colorization_v1`; the channel-to-palette mapping is still `UNVERIFIED`, and the map section has not yet been visually rechecked.

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

## Visual check

Matched viewpoints: six real-game frames (Plan B save, 1280x720, camera read back with `so_camera`) and six host
`-Inspect` captures at the same location, rotation and FOV (`local/sanctuary/inspection-views.json`). Host chain on
this machine: `run_ue_level.ps1 -ImportOnly` (4,914 section actors, 0 material compile failures; saved-scene,
collision and UV verifiers passed) then `test_ue_viewer.ps1 -Inspect` (6 views, Success). Views: town entrance,
plaza spire, Moxxi's (star lights, tapestries), Roland's HQ map table, customization machine, bounty board.

Historical critic result, before the matched-viewpoint changes below (Codex GPT-6-Luna, given only the file paths),
verbatim overall: **4.5/10**. Its top five differences, in its order:

1. The host lacks the reference's nighttime lighting and blue, teal and warm practical-light palette across nearly
   all six views.
2. Important glowing details are missing or muted: Roland HQ's map table and doorway hologram, and the plaza's blue
   and turquoise lighting.
3. Several prominent surfaces are blank or the wrong colour: the Roland HQ table, the customization machine's side
   panels, the start-view graffiti wall and the tapestry seating.
4. The start view is missing its snowy mountain skyline and street character; the customization view is missing the
   people visible outside.
5. The tapestry star lights have oversized black rectangular backings, and the spire monument is cropped at the top
   in the host.

Reading for that earlier pass: the object set at all six viewpoints matches (the newly placed piano area, customization
machine, bounty board and wiring are where the game draws them); the scores are dominated by lighting and time of day,
which that pass did not touch. Its open material items were the map table, customization machine's side panels (runtime
instance `MaterialInstanceConstant_113` still not decoded), star-light backings and graffiti decals. The distant
mountain cause is now traced to the selected dark texture fallback, with a snowy texture fallback in the matched-viewpoint
pass below. No "before" host capture was taken, so the score is not a before/after comparison.

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

## Round 2 matched shots (2026-10-22)

The eight same-camera reference frames are under ignored `local/hub-gallery/2026-10-22/laneA/reference/`;
final host captures and side-by-side pairs are under `round2/host/` and `round2/pairs/`. Host is the left image.
`capture_host.ps1` ran directly and acquired/released the shared lock itself. The capture automation completed all
eight views (`Inspection`, Success); visual validation is the matched screenshot comparison, not gameplay validation.

The import gate (`local/laneA/r2-import11.log`) ended `Success - 0 error(s), 0 warning(s)`. It imported 4,914
section actors. `ue-verify.json` independently verifies 111 of 293 source decal components, records 182 unsupported,
confirms four source PostProcessVolume records and the WorldInfo defaults, and verifies the colorized map recipe.
Collision verification reports 635 mesh sections and 3,254 enabled components with zero errors; UV verification
reports 583,965 corners and 194,655 triangles. Collision refresh matched all five blocking volumes.

### Round 2 findings and remaining gaps

- **Lighting and grading:** the directional component's property stream is found by the exact `LightmassSettings`
  FName anchor, fixing the previous invalid-FName read. Brightness and color decode; the SkyLight color decodes, while
  its Brightness is absent from that component. WorldInfo tone scale, midtones, shadows and bloom decode into the
  local manifest. Four volume Settings/Priority records decode, but `bUnbound` is not serialized on those actors and
  brush bounds were not decoded, so their local grading is not applied. **UNVERIFIED host screenshot calibrations:**
  5 UE5 lux per source brightness unit (35 lux); SkyLight intensity 1.5; linear sky tint multipliers (3.0, 2.1, 1.0),
  giving RGB approximately (0.296, 0.510, 0.554); exposure bias calibration -1.5 EV, combined with the local
  WorldInfo tone-scale mapping. HQ highlights and the warm outdoor tone still differ in the matched shots.
- **Roland's map table:** local channel images show `WorldMap_Comp` R is land/contours, G is the grid and B is route
  markers. The `world_map_mask_colorization_v1` recipe now maps R to pale cyan (0.16, 0.58, 0.68), G to dark teal
  (0.035, 0.09, 0.105) and B to teal (0.08, 0.24, 0.27). Camera 03 now shows cyan land with dark markings.
  Palette intensity, reflection and animated markers remain **UNVERIFIED**.
- **Decals:** census: 293 DecalActors/components, 27 unique material references, 111 imported candidates across six
  supported materials, 182 unsupported, and 46 separate planar static-mesh candidates already imported by the mesh
  path. Direct `Texture2D --properties --property-offset 8` reads now supply compression flags omitted by scene
  records; Modulate maps with `CompressionNoAlpha=true` stay unsupported because channel roles are unresolved. The
  33 placements using `DMat_GrafittoBurrito` use the single `GraffitoBurrito_Dif` texture and inverse RGB luminance;
  camera 06 shows the tower graffiti, still fainter than the reference. This opacity and UE3-to-UE5 projection are
  **UNVERIFIED**. The host uses a translucent deferred decal because the native graph/blend is not reconstructed.
- **Projector:** the placed `IO_RolandHQProjector` has five particle components referencing
  `Part_Gen_RaidersRotating`, `Part_Ep10_Objects`, `Part_Gen_AngelRotating`, `Part_Ep13_Objects` and
  `Part_HoloVid_Glows_Looping`. This identifies a nearby projector system set, not which one draws the map-table
  beam; particle rendering remains out of scope and the beam match is **UNVERIFIED**.
- **Moxxi string lights/sign:** three `LightStrip_02` placements use `Mati_LightStrips_01` and its
  `LightStrip_Comp` atlas; one world-space `WiringMesh_14` placement uses `MAT_WireXMasWhite` and `Wire_Dif`. The
  light-strip emissive calibration is 4.0; the Moxxi sign recipe scale is 2.8. The sign is brighter, but the host
  still does not show the reference's dotted bulb line clearly. Bulb mapping and bloom remain **UNVERIFIED**.
- **Distant mountains:** the visible low skybox mesh is `SkyboxMountains_low` (five placements) with
  `Mat_MountainDistant`; its approximation uses `SkyboxMountainSnow_Dif` and omits the `SkyboxMountain_Dif` rock
  blend. The host snow reads smaller/darker in cameras 02/06. Mesh/material identity is inspected; scale and the
  native snow/rock blend remain unresolved.
- **Customization/sky:** the customization display recipe scale is 3.0 and the cyan panel is brighter, but the
  projector hologram remains dim. Broad host cloud bands still differ from the finer reference streaks. These are
  **UNVERIFIED** visual approximations; neither particle effects nor native sky grading was reconstructed.

The local UE Viewer benchmark for the pillar remains bounded: build 1590 (MIT repository, linked above), 19.5 s,
one MD5 mesh, two animations, four material descriptors, five textures, four sidecars and two logs (18 files); no
`ERROR:` markers or duplicate basenames. Its unknown SkeletalMesh3/LODInfo and material-expression fields and four
trailing bytes lost from each texture export remain documented in the benchmark notes above. All outputs stayed in
ignored `local/`.
