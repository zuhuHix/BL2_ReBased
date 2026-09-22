# Hyperion moon base: inspected surface fallback and moon brightness (2026-09-21)

AI-assisted implementation in `t3code-ec9bc90f`. This record covers two
policies for the moon and its station in Sanctuary's sky: the station's
inspected surface fallback (`INSPECTED_COLOR_FALLBACKS`) and the moon's
inspected Unlit color multiplier (`INSPECTED_UNLIT_COLOR_MULTIPLIERS`, see
"Moon brightness" below). Both are policy tables in `tools/prepare_level.py`
with tests; the multiplier also adds one small graph step to
`host/ue5/import_level.py` and a matching check in `host/ue5/verify_level.py`.
No reader, serialization, container or bounds-check code changed. Neither
policy decodes the stripped `Mat_MoonBase_02a` or `Mat_Moon` graph or compares
against the original game.

## Starting point

The Hyperion station that hangs in front of the moon in Sanctuary's sky is
`Sanctuary_Light:Prop_MoonBase.Mesh.MoonBase02`, one `InterpActor`
(`InterpActor_4.StaticMeshComponent_195`) at (288997, -222027, 249650), scale
480, about 4.4 km from the start camera. Its single section carries the placed
instance `Prop_MoonBase.Materials.Mati_MoonBase_02a`. The moon next to it,
`Prop_MoonBase.Mesh.Moon` with the Unlit additive `Mati_Moon`, already
resolved `Moon_Dif` through the ordinary sole-`_Dif` rule.

The station imported with the neutral gray Lit fallback, which reads as a
black silhouette against the Unlit sky dome. The manifest recorded
`No supported named Material v1 texture parameters; neutral fallback`.

## What the package says

Read with `ow-package --scene-records` on the installed base game:

- `Mati_MoonBase_02a` (`MaterialInstanceConstant`) has `Parent =
  Mat_MoonBase_02a` and no texture, scalar or vector parameter overrides.
- `Mat_MoonBase_02a` (`Material`) is opaque, lit (no `LightingModel` or
  `BlendMode` property), not two-sided. Its surviving expressions are six
  named constants only: vectors `MoonBase_Color` (0.118, 0.339, 0.762),
  `Fog` (0.028, 0.054, 0.061), `RimLight_Color` (1, 1, 1); scalars
  `Brightness` 0.75, `Fog_Intensity` 1, `Emissive_Mult` 10. There is no
  texture sample parameter, so Material v1's named-channel path finds nothing.
- The cooked texture resource list of the base is, in order,
  `MoonBase02a_Nrm` (DXT1 512x512), `MoonBase02a_Dif` (DXT1 1024x1024),
  `MoonBase02a_Emis` (DXT1 1024x1024) and
  `Startup:FX_Shared_Smoke.Textures.Tiling_SmokePanner2_Dif`.

Two `_Dif` textures defeat `cooked_diffuse_candidate`, which requires a
unique `_Dif`/`_Diff` and otherwise gives up; with a `_Dif` present it does
not fall through to the non-auxiliary rule either. This is the situation the
`INSPECTED_COLOR_FALLBACKS` table already covers for `Mat_FrozenLake` and
`Mat_IceRoadSanctuary`.

The extracted textures are the station's hull plating with the large lens
(`_Dif`, mean RGB 71/82/83), cyan window and lens lights on black (`_Emis`,
mean RGB 0.5/2/2, alpha fully opaque) and a tangent-space normal map (`_Nrm`,
mean RGB 127/126/248).

## Policy

`INSPECTED_COLOR_FALLBACKS` gains an entry keyed by the placed instance
`Sanctuary_Light:Prop_MoonBase.Materials.Mati_MoonBase_02a`, method
`moon_base_color_fallback_v1`, status `partial_unverified`:

| Channel | Texture |
|---|---|
| color | `Prop_MoonBase.Textures.MoonBase02a_Dif` |
| normal | `Prop_MoonBase.Textures.MoonBase02a_Nrm` |
| emissive | `Prop_MoonBase.Textures.MoonBase02a_Emis` |

Two small generalizations of the table were needed:

- An entry may be keyed by a placed instance and then names the inspected
  `base`. The lookup is by the placed material's identity; if the resolved
  cooked base differs from the entry's `base` the fallback raises
  `Moon-base fallback requires the inspected base material`, which is
  recorded as an issue and leaves the neutral fallback. Entries without a
  `base` keep their old meaning (they apply to that base material only).
  The previous `key == base` guard is subsumed: an instance of
  `Mat_FrozenLake` is not a table key and still gets nothing.
- An entry may name an `emissive` texture, attached as `p_emissive` only when
  the chain has no explicit emissive parameter, and recorded as
  `emissive_texture` in `surface_approximation`. The host's existing rule
  (Emissive = RGB x A) applies; the texture's alpha is fully opaque.

Recorded as omitted: `MoonBase_Color` tint, `Emissive_Mult` scalar,
`Fog`/`Fog_Intensity` blend, `RimLight_Color`, the
`Tiling_SmokePanner2_Dif` overlay and any UV modulation. The station is
expected to read darker and less blue than the original because the tint and
fog terms are not applied; nothing in the package says how they combine.

## Automated evidence

- `python tests/level_test.py`: 29 tests OK, including the new
  `test_moon_base_color_is_instance_keyed_and_requires_inspected_base`
  (instance-keyed entry resolves diffuse/normal/emissive; the base itself and
  another package's instance get nothing; a mismatched parent and a cooked
  list without the hull diffuse are refused with recorded issues) and
  `test_unlit_color_multiplier_reads_instance_over_base_and_is_scoped`
  (see "Moon brightness").
- `python tests/viewer_test.py`: 12 OK; `tests/sky_census_test.py`: 3 OK;
  `tests/material_audit_test.py`: 2 OK.
- `ctest --test-dir build -C Release --output-on-failure`: 6/6 passed.
- `python tools/verify_packages.py --reader build/Release/ow-package.exe`:
  all nine code packages match.
- `refresh_materials.py --reuse-textures --outer-shell` on the existing
  Sanctuary scene: 381 materials, 176 issues, 90 inferred diffuses (89
  before), 1 sky approximation, 4 replaced hull overrides, 108 hidden
  helpers. A manifest diff against the pre-refresh copy shows exactly one
  changed material record and identical actors, meshes and policies; the
  neutral-fallback issue for `Mati_MoonBase_02a` is replaced by the
  approximation issue.
- `run_ue_level.ps1 -ImportOnly -SkipBuild`: import, saved-scene, collision
  and UV verification passed: 4,888 section actors, 15/20 neutral fallback
  sections (unchanged), 615 collision sections with 0 errors, 545,301 UV
  corners matching the prepared OBJ.

## Runtime evidence (Sanctuary, UE5.8 game viewer)

`test_ue_viewer.ps1 -Inspect` with three station viewpoints
(`local/sanctuary/inspection-views.moonbase.json`; the C++ inspection test
accepts at most 12 views per run, so these are kept beside the standard 12):

| View | Location | Rotation (pitch, yaw) | FOV | Shows |
|---|---|---|---|---|
| 0 | start, z 3100 | 34, -40 | 60 | moon and the station's upper arms above the rooftops |
| 1 | start, z 3100 | 34.4, -36.8 | 25 | plated arms, lens and cyan lights in front of the moon |
| 2 | start, z 14000 | 31, -37 | 30 | the whole H-shaped station against the moon |

All three runs reported `Test Completed. Result={Success}`. The captures are
`Sanctuary_P_inspection_0000000.png`, `_0100000.png` and `_0000001.png`
under the project's ignored `Saved/Screenshots/WindowsEditor/`, with copies
under ignored `local/moonbase/`. The hull shows panel detail, the lens and
the emissive lights; it is no longer a flat dark silhouette.

## Moon brightness (same day, second slice)

With the station textured, the moon behind it was the next thing wrong in
the same captures: `Prop_MoonBase.Mesh.Moon` carries the placed instance
`Mati_Moon`, whose base `Mat_Moon` is `MLM_Unlit` and `BLEND_Additive`. The
sole-`_Dif` rule had already resolved
`Sanctuary_P:Prop_MoonBase.Textures.Moon_Dif` (a gray cratered surface), and
an additive Unlit material that emits that texture at 1x barely rises above
the dome behind it. In `moonbase_view1_fov25.png` (before) the disc is a
pale, nearly featureless smudge.

### What the package says

`Mati_Moon` has `Parent = Mat_Moon` and no parameter overrides of its own.
`Mat_Moon` keeps 65 expression slots, of which nine named parameters survive
the cook:

| Parameter | Kind | Default | Note |
|---|---|---|---|
| `p_moonColor` | vector | (4.02177477, 4.02177477, 4.02177477) | applied by this slice |
| `p_Basecolor2` | vector | (4, 1.138, 0) | orange secondary color, omitted |
| `p_DarkColor` | scalar | -0.05 | omitted |
| `p_moonTime`, `p_moonRotation` | scalar | none | UV motion, omitted |
| `p_moonTimeBaseShadow` | scalar | none | station-shadow phase, omitted |
| `Time_of_Day` | scalar | 170 | omitted |
| `Transition_Track` | texture sample | `Prop_Skybox.Textures.Transition`, desc "Sky color lookup" | omitted |
| `Horizion_track_color_multiplier` | vector | (10, 10, 10), desc "Sky color Mod" | omitted |

Its cooked texture list is `MoonBase02_GRP` (the H-shaped station
silhouette on black, presumably the shadow the station casts on the moon),
`Moon_Dif`, `Prop_Skybox.Textures.Transition` (the time-of-day color strip
the sky master also samples) and `Moon_Comp` (a crater relief/composite
map). None of the connections between these survive; the package does not
say how the shadow mask is placed, rotated or timed.

### Policy

A new table `INSPECTED_UNLIT_COLOR_MULTIPLIERS` in `tools/prepare_level.py`
is keyed by the placed instance
`Sanctuary_Light:Prop_MoonBase.Materials.Mati_Moon`, method
`unlit_color_multiplier_v1`, status `partial_unverified`, required base
`Mat_Moon`, parameter `p_moonColor`. `Scene.unlit_color_multiplier` reads
the named vector through the chain with instance overrides last, refuses a
chain that is not Unlit or does not end at the inspected base, and refuses a
missing, non-finite or negative value; each refusal is recorded as an issue
on `<source>:multiplier` and leaves the material as it was. On success the
manifest material record gains

```
"unlit_color_multiplier": {"method": "unlit_color_multiplier_v1",
  "status": "partial_unverified", "parameter": "p_moonColor",
  "source_material": "Sanctuary_Light:Prop_MoonBase.Materials.Mat_Moon",
  "rgb": [4.02177477, 4.02177477, 4.02177477], "omitted": [...]}
```

and the issue `Approximation: Unlit color scaled by p_moonColor; shadow
mask, relief and secondary color not reconstructed`.

Host side, `import_level.py` handles the record only on the existing Unlit
branch (no explicit emissive parameter): the recovered visible color
(diffuse sample RGB, or the fallback constant) is multiplied by a
`Constant3Vector` holding `rgb` before it reaches `MP_EMISSIVE_COLOR`.
Nothing else in the graph changes, and materials without the record take
the old path unchanged. `verify_level.py` asserts, for every material
carrying the record, that the emissive input is a `Multiply` whose second
input is a `Constant3Vector` equal to `rgb` within 1e-6, then walks the
first input through the existing Unlit checks; it reports them as
`verified_unlit_multiplier_materials`.

Recorded as omitted, in the manifest's `omitted` list: the `MoonBase02_GRP`
station shadow mask, `p_moonTimeBaseShadow`, `Moon_Comp` crater relief,
`p_Basecolor2` secondary color, `p_DarkColor`, the `Transition_Track`
time-of-day tint and `p_moonTime`/`p_moonRotation` UV motion. `Time_of_Day`
and `Horizion_track_color_multiplier` belong to the same omitted
`Transition_Track` term and are not listed separately.

Why a multiplier and not more: `p_moonColor` is the one surviving term whose
meaning is unambiguous for an additive Unlit surface (a color scale on the
emitted value), and 4.02 is far enough from 1 that leaving it out is the
visible defect. Placing the shadow mask needs the station's position on the
moon as the original renders it; that is an in-game reference question, not
something the package answers, and it is left open.

### Automated evidence

- `python tests/level_test.py`: 29 OK. The new
  `test_unlit_color_multiplier_reads_instance_over_base_and_is_scoped`
  checks that the base default is read, that an instance
  `VectorParameterValues` override wins, that the base itself and another
  package's `Mati_Moon` get no record, and that a chain ending elsewhere or a
  negative constant is refused with an issue. `tests/prepare_ue_smoke.py`
  gains a synthetic `UnlitScaled` additive material carrying the record so
  the host importer and verifier exercise the branch in the smoke fixture.
- `python tests/viewer_test.py`: 12 OK; `tests/sky_census_test.py`: 3 OK;
  `tests/material_audit_test.py`: 2 OK.
- `ctest --test-dir build -C Release --output-on-failure`: 6/6 passed.
- `python tools/verify_packages.py --reader build/Release/ow-package.exe`:
  all nine code packages match.
- `refresh_materials.py --reuse-textures --outer-shell` on the Sanctuary
  scene after the station slice: a manifest diff against the retained
  pre-refresh copy (`scene.pre-moon.json`) shows exactly one changed
  material record (`Mati_Moon`, gaining `unlit_color_multiplier`), identical
  actors, meshes and policies, and one added issue (176 to 177).
- `run_ue_level.ps1 -ImportOnly -SkipBuild` (2026-09-21, 16:00-17:01):
  `ue-verify.json` reports 4,888 section actors, 35 Unlit materials, 1
  `verified_unlit_multiplier_materials` (the `Mati_Moon` record), 1 sky
  approximation, 20 neutral fallback sections, 4/4 outer-shell
  replacements; collision (`ue-collision-verify.json`) and UV
  (`ue-uv-verify.json`) verification passed.

### Runtime evidence

`test_ue_viewer.ps1 -Inspect` with the same three station views
(`local/sanctuary/inspection-views.moonbase.json`) re-run after the import,
`Result={Success}`, captures `Sanctuary_P_inspection_0000002.png`,
`_0100001.png` and `_0200000.png` (17:02). Compared with the pre-multiplier
captures of the same views (`local/moonbase/moonbase_view*_*.png`, 16:48),
the moon is now a bright white disc with the crater pattern of `Moon_Dif`
readable through it instead of a pale wash; the station's arms and lens in
front of it are unchanged. Whether 4.02x on the diffuse alone matches how
bright the original's moon reads is not established; the original combines
this with the `Transition_Track` tint and `Horizion_track_color_multiplier`.

## What this does and does not claim

Claimed: the placed station material now carries the hull's own diffuse,
normal and emissive textures on unchanged UV0; the moon's Unlit color is
scaled by the package's own `p_moonColor` value; each change affects one
material record; import and all saved-scene verifiers pass, including the
new multiplier check; the station is visibly textured and the moon visibly
brighter from the start camera. Not claimed: either base graph, the
tint/fog/rim/emissive-multiplier terms on the station, the shadow mask,
relief, secondary color, time-of-day tint or UV motion on the moon, the
smoke overlay, host lighting parity, or any visual parity with the original
game. Those need a matched in-game view; the station-shadow placement in
particular is waiting on an in-game reference screenshot of the moon from
Sanctuary and is not guessed.
