# Native sky census (2026-09-14)

Checkout: `t3code-6bf0ae5d`. AI-assisted `tools/sky_census.py`; installed
Steam base game; outputs under ignored `local/sky/`. This locates the sky and
records its material chain from decoded manifest data. It is not a host
import, a shader reconstruction or an in-game comparison.

## What changed

- `tools/sky_census.py` (new): sky-named `StaticMesh` placements, owners,
  observed transforms, effective and default section materials, parent chains,
  all named sampler/scalar/vector parameters, cooked texture lists, `Texture2D`
  format/size/cache, and a restatement of the Material v1 diffuse outcome with
  the blocking reason. `--extract` writes OBJ/PNG.
- `tools/prepare_level.py`: sublevel traversal factored into `Scene.levels`;
  `build` behaviour unchanged.
- `tests/sky_census_test.py` (new, synthetic): name classification, policy
  precedence (`p_diffuse` over alias, concrete over stub, explicit null,
  unique/ambiguous cooked candidates, translucent block), chain parameter
  collection with undecoded expressions skipped.

No reader, serialization or policy code changed.

## Automated checks

`python tests/sky_census_test.py`: 3 tests OK.
`python tests/level_test.py`: 15 tests OK.
`python tests/material_audit_test.py`: OK.
`ctest --test-dir build -C Release --output-on-failure`: 5/5 passed.
`python tools/verify_packages.py --reader build/Release/ow-package.exe`: all
nine code packages match (output in the commit log).

## Installed-game findings

`Sanctuary_P` (10 levels): 31 sky-named placements, 9 meshes, 13 materials,
31 textures, 0 issues. 14 placed sections resolve a diffuse under the current
policy, 19 do not.

`Ash_P` (7 levels): 26 placements, 6 meshes, 14 materials, 25 textures,
0 issues. 23 placed sections resolve a diffuse, 4 do not.

Neither map streams a `_Skybox` package. The dome in both is
`Prop_Skybox.Meshes.Sky_Dome`: 1 LOD, 265 vertices, 480 triangles, 2 UV sets,
no body setup.

| Map | Placement | Scale | Effective material | Policy diffuse |
|---|---|---|---|---|
| Sanctuary | `Sanctuary_Light` `StaticMeshCollectionActor_33.StaticMeshActor_SMC_20` | 5000, 5000, 6000 | `Prop_Skybox.Materials.Mati_Sky_Dynamic_INST` -> `Common_Materials.Sky.Mat_SkyTimeOfDay_Master` | `Sky_TransitionBL2Default_Dif` (unique cooked `_Dif`) |
| Sanctuary | `Sanctuary_Light` `...StaticMeshActor_SMC_3760` | 900, 900, -500 | `TilingMaterials.Materials.Mati_FloorConcrete01` | `FloorConcrete01_Dif` (named) |
| Ash | `Ash_P` `StaticMeshCollectionActor_244.StaticMeshActor_SMC_1850` | 1000 | `Env_Ash.Materials.Mati_AshSkyTempSunset` -> same master | `Sky_TransitionBL2Default_Dif` |

`Mat_SkyTimeOfDay_Master` is `MLM_Unlit`. Its instance in Sanctuary exposes
samplers `Transition_Track` = `Sky_TransitionBL2Default_Dif` (PF_A8R8G8B8,
256x256, `CharTextures`), `clouds` = `Clouds_01` (DXT1 1024x256, inline),
`Masks` = `Sky_Multi2` (instance override of the base `Sky_Multi`, DXT1
256x256); scalars `Time_of_Day` 170, `sky_brightness` 1, `Sun_spot_brightness`
6, `cloud_cap_opacity` 1, `p_CouldBrightness` 1; vector
`Horizion_track_color_multiplier` 0.201 gray. How the master combines these is
not decoded (the cooked graph is stripped).

The Sanctuary `_Outer` layer, `Prop_Skybox.Meshes.SanctuarySky` (10003
vertices, 7416 triangles), is an `InterpActor` at (501, 222, -124) with
yaw 90. Its overrides `Mat_Sanctuary_Teleported`, `..._Drill_Teleported`,
`..._Rock_Teleported` are masked, two-sided, with `Phased`, `ShieldFailDistort`
and `DomeAlpha` scalars and four `_Dif` textures each, so the policy selects
none. The mesh's own defaults resolve uniquely: `Mat_SancSkyNew` ->
`SanctuarySkybox_Diff` (DXT1 2048x2048, `Textures`), `Mat_SancDrillNew` ->
`SancDrill_Diff`. Which of `_Land` (mountains, arid terrain) and `_Outer`
(sky city) is active is Kismet state and is not interpreted.

`--extract` on Sanctuary wrote 9 OBJ and 29 PNG files with no extraction
errors, including the A8R8G8B8 transition texture through the census path.

## Not verified

The host has not imported the dome; the atmosphere fallback is still in place.
No screenshot exists. The relationship between `Time_of_Day` and the
transition track is a guess and is not recorded as fact. The second `Sky_Dome`
placement with a concrete material and negative Z scale is reported, not
explained. Nothing here is visual-parity evidence.
