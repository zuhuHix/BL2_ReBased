# Sanctuary scene import gaps: material fallbacks, component owners, colour streams (2026-10-21)

AI-assisted (Claude), lane A, branch `feature/scene-materials-owners`. Evidence comes from the installed base game
through this repository's reader, a census over every base-game package and one live read of the running game.
Every report, texture, scene and capture stays under ignored `local/` (`local/lanea/`, `local/realgame/`).

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
| `Mat_MountainDistant` (5) | no named samplers; `SkyboxMountain_Dif` is a rock-and-snow photo, `SkyboxMountainSnow_Dif` mostly white | `SkyboxMountain_Dif` | snow layer |
| `Mat_DirtySnowDistant` (1) | as PatchySnow_Skybox01 | `DirtySnow_Dif` | snow blend, tints |
| `Mat_ChemTank02` (1) | the tank atlas next to a smoke overlay `_Dif`; cube is `P_SimpleReflect` | `ChemTank02_Dif` | packed `_Comp` mask, six colour vectors, reflection, smoke |
| `Mati_SancSpire_Base` / `_Tile` (6) | `Master_SancSpire` HLS master | already baked by `prepare_sanctuary_pillar.py` (DECISIONS 2026-09-23), which also serves the static spire arm and base placements | as recorded there |

`Mat_WorldMap` (one section, Roland's map table): not changed. Its cooked list is `WorldMap_Comp` and the named `cube`. The live capture of the table shows pale cyan land on a darker sea with red routes and green markers, while `WorldMap_Comp` holds a red landmass, green grid and blue markers: the channels are masks that the stripped graph recolours. Using the texture as RGB would be wrong, and no recolouring is recovered, so the section keeps the neutral fallback. It is the one remaining placed opaque gap.

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
copy is listed. What the game draws for these objects in other mission or challenge states (a vault symbol after it
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

VISUAL_RESULT

## Ash and Southpaw re-measured

Same tools, `main` versus this branch, `prepare_level.py` only (neither map has prepared terrain or BSP):

| Map | Opaque missing diffuse (defs / sections) | Unsupported owners | Excluded previews | Colour-stream issues |
| --- | --- | --- | --- | --- |
| Ash_P before | 13 / 117 | 325 | 0 | 0 |
| Ash_P after | 13 / 117 | 309 | 14 | 0 |
| SouthpawFactory_P before | 0 / 0 | 10 | 0 | 0 |
| SouthpawFactory_P after | 0 / 0 | 5 | 4 | 0 |

The material fallbacks are Sanctuary entries, so Ash's 13 opaque gaps (volcanic ash, lava, alien plaster, cube
windows and others) are unchanged and remain open. Each map gained its cable placement(s). Ash's remaining owners
are `Perch` (128) and `WillowPopulationPoint` (116) AI markers, `WillowInteractiveObject` (38), vehicle-station
platforms and terminals (18), `WillowPropObject` (8) and one `KActor`; they are not classified by this pass.

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
