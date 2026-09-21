# Sky approximation from the dome's named inputs, and the opt-in outer hull (2026-09-21)

AI-assisted implementation in `t3code-8311503f`. This slice changes scene
preparation policy, the UE5 host material graph for one material family, the
host's fallback-shell rule, and the saved-scene verifier. No reader,
serialization, container or bounds-check code changed. It does not decode the
stripped `Mat_SkyTimeOfDay_Master` graph, interpret Kismet state, animate time
of day, or compare against the original game.

## Starting point

The [native dome record](NATIVE_SKYBOX_VERIFICATION.md) imported the accepted
`Prop_Skybox.Meshes.Sky_Dome` placement with the ordinary Material v1 path:
the sole cooked `_Dif` texture, `Sky_TransitionBL2Default_Dif`, on UV0. That
texture is a 256×256 time-of-day strip (columns = time, rows = zenith to
horizon, two bright sun columns), so UV0 wrapped every time column once around
the azimuth. The host's blue `OpenWillow_SkyFallback` sphere (radius 5,000 m,
inside the dome's 5000× placement) also hid the dome from the inspection
camera, so no earlier screenshot shows the native dome at all.

## What the package says

`tools/sky_census.py --extract` on the installed base game lists the instance
chain `Mati_Sky_Dynamic_INST -> Mat_SkyTimeOfDay_Master` (`MLM_Unlit`) with
these surviving named inputs. The master's expression graph is stripped; only
parameter expressions and instance overrides remain.

| Input | Kind | Value (instance over master) |
|---|---|---|
| `Transition_Track` | sampler | `Sky_TransitionBL2Default_Dif` (256×256 `PF_A8R8G8B8`) |
| `clouds` | sampler | `Clouds_01` (1024×256 DXT1; R mean 87, zero at the top rows, densest near the bottom) |
| `Masks` | sampler | `Sky_Multi2` (instance override of `Sky_Multi`; R ≈ stars, G ≈ noise, B ≈ radial gradient) |
| `Time_of_Day` | scalar | 170 |
| `sky_brightness` | scalar | 1 |
| `Sun_spot_brightness` | scalar | 6 |
| `cloud_cap_opacity` | scalar | 1 |
| `p_CouldBrightness` | scalar | 1 |
| `Horizion_track_color_multiplier` | vector | 0.201 gray |

The dome mesh's UV0 runs 0–1 around the azimuth and, in UE texture space,
0.02 at the apex to 0.999 at the rim (the OBJ writer stores `1 - v`). The
strip's top row is the dark zenith and its bottom row the bright horizon, so
dome V indexes the strip's vertical axis directly.

## Policy

`tools/prepare_level.py` gains `Scene.named_chain_parameters` (all named
sampler/scalar/vector inputs along a chain, instance overrides last) and
`Scene.native_sky_approximation`, which applies only when the chain's base is
`Common_Materials.Sky.Mat_SkyTimeOfDay_Master`. It requires every input in
the table to be present and well-formed (each sampler a `Texture2D`, each
scalar finite, `Time_of_Day` inside the strip width) and records a
`sky_approximation` block with method `sky_time_of_day_strip_v1`, status
`partial_unverified`, the extracted textures, the constants, the column
lookup and an explicit `omitted` list. A missing or malformed input is
reported as an issue and the material keeps the previous diffuse-only path.

`host/ue5/import_level.py` builds one fixed graph from that record:

```
column_u  = Time_of_Day / 256
sky       = strip(column_u, dome UV0.v) * sky_brightness
cloud_col = strip(column_u, 0.95)       * sky_brightness * p_CouldBrightness
coverage  = saturate(Clouds_01.R(UV0)   * cloud_cap_opacity)
Emissive  = lerp(sky, cloud_col, coverage)
```

The strip is sampled as sRGB color, the cloud texture as linear data. The
ordinary diffuse inference stays connected to Base Color as the recorded
fallback (Unlit ignores it). The material is flagged `Is Sky`: the first
Sanctuary capture without it showed the dome as a brown field with beige
cloud bands, because the host `SkyAtmosphere` applies aerial perspective to
opaque geometry 20,000 km away; UE's sky flag exempts Unlit opaque materials
from fog and aerial perspective. The verifier checks the flag.

When any accepted dome placement carries this record, the importer no longer spawns `OpenWillow_SkyFallback`; the
`OpenWillow_SkyAtmosphere` actor stays as the ambient fallback and
`ue-import.json` records `host_sky_shell_spawned: false` and
`temporary_sky_fallback: UE5_SkyAtmosphere`.

### Assumptions, all UNVERIFIED

- `Time_of_Day` is read as a pixel column (`/256`). Read as degrees (`/360`)
  the value 170 lands in a sun column and the dome turns dusk-grey with an
  orange horizon; read as a column it gives the deep-to-light blue daytime
  gradient. The package does not say which the original shader does; the
  divisor is recorded in the manifest so the reading can be changed.
- `Clouds_01.R` is treated as cloud coverage and tinted with the strip's
  row-0.95 color. The G/B channels, `Masks`, the sun spot, the horizon color
  multiplier, cloud motion and time-of-day animation are omitted and listed
  as such in the record.
- The dome is the only sky layer touched. `SanctuarySky`, the distance
  mountains/terrain in `_Land`, the cloud planes and Kismet activation are
  unchanged.

## Opt-in outer hull

`Sanctuary_Outer` places `Prop_Skybox.Meshes.SanctuarySky` (10,003 vertices,
three sections) and two antenna meshes as `InterpActor`s at (501, 222, -124)
with masked `_Teleported` overrides (`Mat_Sanctuary_Teleported`,
`..._Drill_Teleported`, `..._Rock_Teleported`). Those graphs carry `Phased`,
`ShieldFailDistort` and `DomeAlpha` inputs and several `_Dif` textures, so
Material v1 recovers no diffuse and the hull imported as opacity-0 masked
geometry, i.e. invisible. The mesh-default materials `Mat_SancSkyNew` and
`Mat_SancDrillNew` each resolve a unique cooked `_Dif` texture
(`SanctuarySkybox_Diff`, 2048×2048, the city's exterior plating; `SancDrill_Diff`),
and slot 2 defaults to the already-approximated `Mati_GlacierRocks2X`.

`--outer-shell` (on `prepare_level.py`, `refresh_materials.py` and
`viewer.py --action prepare`) applies `outer_shell_overrides`: for the three
observed meshes only, an override is dropped if and only if its name ends in
`_Teleported` and the slot's mesh default has a diffuse. Each replacement is
recorded on the actor (`outer_shell_replaced`), the placed overrides are kept
in `outer_shell_source_materials` so a later refresh can withdraw the policy,
and `scene.json` records `outer_shell_policy`. The host imports these actors
under `OuterShell/` without shadow casting (the source component records
`CastShadow=False`). Default behaviour without the flag is unchanged.

This substitutes the pre-teleport look for the placed phase-in material. It
does not decide whether the running game shows the `_Outer` hull or the
`_Land` mountains at any story state; both sublevels load together here.

### Matinee first-key placement experiment

The cheap placement test is opt-in and deliberately narrow. On `Sanctuary_P`,
`prepare_level.py --matinee-first-key` matches only
`TheWorld.PersistentLevel.InterpActor_29.StaticMeshComponent_393`, adds the
observed first `RelativeToInitial` translation `(16551, -171794, -164)` to
the serialized actor location, and sets the actor rotation to yaw `78.75°`.
The resulting pose is
approximately `(17052, -171572, -288)`, yaw `78.75°`. The same option is
available through `viewer.py --action prepare --matinee-first-key`; it writes
`matinee_first_key_policy` and `matinee_first_key_applied` into `scene.json`.
It is a visual comparison aid, not a `SeqAct_Interp` or Kismet decoder, and
the default serialized placement is unchanged.

### What the hull is (editor inspection, 2026-09-21)

Status for `StaticMeshComponent_393` / `InterpActor_29`: **decode verified,
in-game position unverified, observed off in editor.** In the editor the
hull's central tower sits visibly off-centre and above the town's own tower.
The serialized actor transform agrees with the game's object dump (translation
delta 0.0) and all 10,003 vertices agree with umodel at 1 cm
([dump record](BLCMM_DUMP_CROSSCHECK.md), [umodel record](UMODEL_CROSSCHECK.md)).
Reading the sublevel's Kismet with our own reader gives a plausible placement
explanation, but does not establish which pose the original game displays:

- `Sanctuary_Outer` contains one Matinee, `SeqAct_Interp_0`, commented
  `SanctuaryLiftoff`, with 84 groups. Its `Sanctuary` group is bound to
  `InterpActor_29` (the hull) through `SeqVar_Object_5`; `Antenna_01` and
  `Antenna_02` bind `InterpActor_23` and `_25`.
- The `Sanctuary` group's `InterpTrackMove_0` is `IMF_RelativeToInitial`
  with position keys at 0.5 s (+16551, −171794, −164), 3.4 s (+16569,
  −171780, +396), 16.8 s (+16551, −171794, +9955) and 39.9 s (+7441,
  −179740, +14684), and yaw keys 78.75° → 84.4° → 95°. Its visibility track
  shows the actor at 0 s and hides it at 36.0 s; the antennas hide at 10.3 s
  and 27.6 s.
- `Sanctuary_Outer` and `Sanctuary_Land` are both `LevelStreamingKismet` in
  `Sanctuary_P`; `Sanctuary_Px` is the only always-loaded sublevel.

So the hull is associated with a liftoff-cutscene prop, and its serialized
transform may be a parking pose coincident with the town; the observed first
key would move it about 1.7 km south and lift it 100–150 m before hiding it.
That interpretation is not an in-game position proof. The normal import keeps
the serialized placement. The opt-in `--matinee-first-key` experiment instead
adds the observed first key to this one component, producing approximately
`(17052, -171572, -288)` at yaw `78.75°` so the editor result can be compared
directly. Full `SeqAct_Interp` start-position decoding and Kismet triggering
remain open; no placement correction is committed by this experiment.

## Automated evidence

- `python tests/level_test.py`: 25 tests OK, including the new sky-record
  tests (instance-over-master precedence, master identity guard, rejection of
  a `TextureCube` input, an out-of-range `Time_of_Day`, a missing scalar and a
  parent cycle) and the outer-shell tests (only `_Teleported` overrides with
  diffuse-bearing defaults are replaced; re-applying is stable; withdrawing
  restores the placed overrides; unrelated meshes are untouched).
- `python tests/viewer_test.py`: 11 tests OK; `--outer-shell` reaches
  `prepare_level.py` only.
- `python tests/sky_census_test.py`: 3 tests OK (unchanged).
- Synthetic UE5 fixture (`tests/prepare_ue_smoke.py`, `-Scene
  local/material-smoke -ImportOnly -SkipBuild`): import and saved-scene
  verification pass. The verifier walks the saved `UnlitSky` graph back from
  Emissive (the `Is Sky` flag, lerp, both multiplies, the append/mask/texcoord
  chain, the constant column and horizon UV, the saturate/coverage chain,
  sRGB vs linear sampler settings) and checks the synthetic hull actor's replaced slot is
  bound to the mesh default while its recorded override ends in
  `_Teleported`. Because the fixture has no native dome the blue shell is
  still spawned there and still verified.
- Sanctuary: see the runtime section below.

## Runtime evidence (Sanctuary)

The existing `local/sanctuary` scene (with its recovered terrain and BSP) was
updated with `refresh_materials.py --reuse-textures --outer-shell`: 381
materials, 176 issues (one more than before: the sky approximation's own
labeled issue), one `sky_approximation`, four replaced hull overrides on two
placements (the second antenna has no override to replace). The pre-refresh
manifest differs only in those two actors' override lists.

`run_ue_level.ps1 -ImportOnly -SkipBuild` then passed import, saved-scene,
collision and UV verification:

- `ue-import.json`: 4,888 section actors, `native_skybox.policy:
  sky_time_of_day_strip_v1`, `host_sky_shell_spawned: false`,
  `outer_shell: {placements: 3, replaced_overrides: 4}`,
  `temporary_sky_fallback: UE5_SkyAtmosphere`.
- `ue-verify.json`: 4,888 section actors, 35 Unlit materials, the dome
  material listed under `verified_sky_approximation_materials`, 3 outer-shell
  placements with 4 of 4 expected replacements bound to the mesh defaults,
  106 hidden helpers, lighting actors without `OpenWillow_SkyFallback`.
- Collision: 615 mesh sections, 3,236 enabled components, 120 triangle
  components, 0 errors. UV: 615 sections, 545,301 corners, 181,767 triangles,
  saved UV0 matches the prepared OBJ.

`test_ue_viewer.ps1 -Inspect` with 12 viewpoints (the four pre-existing ones
plus eight added for this slice) passed, `Inspection captured 12 views`. The
added viewpoints, all `fov 90`:

| View | Location | Rotation (pitch, yaw) | Shows |
|---|---|---|---|
| 4 | start, z 3100 | 40, 45 | dome overhead: deep-to-light blue gradient, cloud bands |
| 5 | start, z 3100 | 8, 225 | horizon through the gate: pale blue with cloud streaks above the rocks |
| 6 | start, z 3100 | 5, 0 | Scooter's front with the dome behind the roofs |
| 7 | start, z 3100 | 5, 135 | town street, dome above |
| 8 | (8650, -26000, 9000) | -12, 90 | the town from the south over the glacier; a yellow/black plane behind the roofs (see below) |
| 9 | (8650, 222, 30000) | -89, 90 | top-down: town, rock, ice, water |
| 10 | (10086, 218, -12000) | 80, 0 | under the city: the hull's drill and rock sections hang below the frozen lake |
| 11 | (32000, 218, 2000) | -3, 180 | from the east: a plated hull block with its round port among the `_Land` rock |

Captures are the numbered `Sanctuary_P_inspection_NN*.png` files under the
project's ignored `Saved/Screenshots/WindowsEditor/`; the first pass
(`_0400001`, `_0500001`) was taken after the `Is Sky` fix, the last pass
(`_1000001`, `_1100001`) after the hull viewpoints were adjusted. The first
capture without `Is Sky` (`_0400000`) shows the brown field described above.

View 8 showed a yellow/black rectangular plane on the horizon behind the
town, and the editor fly-through found a second one. Both are
`Common_Meshes.Blocking.Blocking_Plane` placements in `Sanctuary_Light`
(`StaticMeshActor_SMC_0`, `_SMC_6`) with
`Sanctuary_Light:Env_Ice.Materials.Mat_CloudLayer_01`, a sibling of the
`Mat_CloudLayer_Light` instance the artifact pass already hides. Its sole
cooked texture is `Basic_Dust_Dif`, a dust sprite, which the diffuse
inference tiled across the plane. The hide rule now covers both cloud-layer
instances (`HIDDEN_CLOUD_MATERIALS`); `refresh_materials.py` re-evaluates
the rule so the fix does not need a full rebuild. The refreshed Sanctuary
manifest hides 108 helpers (106 before); the re-import passed scene,
collision and UV verification again (4,888 section actors, 108 hidden, 0
errors) and the recaptured view 8 (`_0800003`) shows the horizon without the
stripes. The one remaining `Blocking_Plane` placement carries
`Mati_BankFloor` and is left visible.

## What this does and does not claim

Claimed: the accepted dome now renders a daytime gradient with cloud bands
built from the instance's own textures and constants; the host's blue shell
is gone from the Sanctuary map; the outer hull's three placements render with
their mesh-default materials when the flag is given; all of it reopens with
zero verifier errors. Not claimed: that the graph matches
`Mat_SkyTimeOfDay_Master`, that `Time_of_Day` is a pixel column, that the
cloud layer is combined the way the original does, that the hull's
pre-teleport materials are what the game shows at any story state, or any
visual parity with the original game. Those need a matched in-game view.
