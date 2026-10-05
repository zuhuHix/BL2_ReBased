# Tooling reference

The working detail behind the BL2_ReBased [README](../README.md): how to
build, what each tool does, what has been checked, and what each check does and does not prove.
Everything here reads the player's own installed game and writes only under
the ignored `local/` directory.

Requirements: CMake, Visual Studio 2022 C++ build tools, Python 3. The UE5
host additionally needs Unreal Engine 5.8 installed.

## External tools

External tools are optional local dependencies. Keep their binaries outside the
repository and write every game-derived export under the ignored `local/`
directory. Do not add a binary, an extracted asset or a generated manifest to
git.

### UModel / UE Viewer

UModel is the primary external visual inspection and extraction candidate. The
current verified local executable is build 1590 from the official
[`gildor2/UEViewer` repository](https://github.com/gildor2/UEViewer). The
[official project page](https://www.gildor.org/en/projects/umodel) documents
the exporter. It recognizes this Borderlands 2 install as the
`border` game tag and package version `832/46`. The source repository is used
as provenance; no source was copied into this project.

The official project page documents package listing and export of static and
skeletal meshes, animations, textures and sounds. It also documents important
limits: some material types are unsupported, exported material files are
heuristic, and exported data does not prove UE5 material or gameplay parity.

The current maintainer checkout is outside this repository at
`C:\Users\zuhu\Documents\BL2_Tools\UEViewer-src`. Other machines must pass
their own path explicitly. The reusable wrapper is
`tools/export_with_umodel.ps1`:

```powershell
./tools/export_with_umodel.ps1 `
  -UModel 'C:/path/to/BL2_Tools/UEViewer-src/umodel.exe' `
  -Cooked 'C:/path/to/Borderlands 2/WillowGame/CookedPCConsole' `
  -Package Ash_P `
  -Object Ash_Road01
```

The wrapper writes a timestamped directory under
`local/external/umodel/`, preserves the run log and reports exit code,
elapsed time, file count and output bytes. It intentionally exports one named
package/object at a time; a whole-install run must wait for the benchmark gate.

For direct inspection, these commands were verified locally:

```powershell
$umodel = 'C:/path/to/BL2_Tools/UEViewer-src/umodel.exe'
$cooked = 'C:/path/to/Borderlands 2/WillowGame/CookedPCConsole'
$out = 'C:/path/to/BL2_ReBased/local/external/umodel-smoke'
& $umodel "-path=$cooked" '-game=border' '-list' 'Ash_P'
& $umodel "-path=$cooked" '-game=border' '-export' '-gltf' '-lods' '-dds' `
  '-nooverwrite' "-out=$out" 'Ash_P' 'Ash_Road01' 'StaticMesh'
```

After representative packages have been tested and an output-size policy is
recorded, a cautious full-install command is:

```powershell
& $umodel "-path=$cooked" '-game=border' '-export' '-gltf' '-lods' '-dds' `
  '-sounds' '-nooverwrite' '-uncook' "-out=$out" '*.upk'
```

This is deliberately not marked as a guaranteed all-asset export. The command
must be accompanied by an inventory of attempted packages, exported classes,
failures, unsupported objects, duplicates, warnings, elapsed time and output
size. See [EXTERNAL_TOOL_BENCHMARK.md](verification/EXTERNAL_TOOL_BENCHMARK.md).

### Secondary tools

| Tool | Use | Current policy |
|---|---|---|
| [UPK Explorer](https://www.nexusmods.com/site/mods/587) | UE2/UE3 GUI inspection, texture/TFC work, package exploration and optional FBX/audio workflows | Optional fallback; current distribution is on Nexus Mods and requires an authenticated download; not required for the first UModel spike |
| `ow-package` | Package identity, census, properties, bounded payloads, scene records and verification | Project-owned and retained even if UModel becomes the visual backend |
| `pyunrealsdk` and community data tools | Future runtime observation and behavioral golden data | Not an asset-extraction replacement; use for observation only, after license review |
| UE Explorer / UPKUtils | Format and behavior references | GPL-licensed references; do not copy code into this MIT project |

The external-tool acquisition and first smoke results are recorded in the
[benchmark record](verification/EXTERNAL_TOOL_BENCHMARK.md). A successful
UModel export is not independent proof that the corresponding custom decoder,
material graph or original-game behavior is correct.

## Build and test

```powershell
cmake -S . -B build -G "Visual Studio 17 2022" -A x64
cmake --build build --config Release
ctest --test-dir build -C Release --output-on-failure
python tests/level_test.py
python tools/verify_packages.py --reader build/Release/ow-package.exe
& ./build/Release/ow-package.exe "C:/Program Files (x86)/Steam/steamapps/common/Borderlands 2/WillowGame/CookedPCConsole/Core.upk"
```

The CTest suites and `tests/level_test.py` use synthetic fixtures only and are
what CI runs. `tools/verify_packages.py` needs an installed game; for a
non-default install add
`--cooked "D:/path/Borderlands 2/WillowGame/CookedPCConsole"`. It only reads
installed packages; temporary decompressed files are removed when the check
exits.

For a decoder-free build add `-DOPENWILLOW_LZO=OFF`; that build accepts only
decompressed packages and skips the container, asset and compressed tests. The
LZO decoder is the vendored MIT-licensed lzokay; see
[THIRD_PARTY.md](../THIRD_PARTY.md).

## `ow-package`: the package reader

A standalone x64 C++20 tool that reads version 832/46 packages:

- name/import/export tables, fully and partially LZO-compressed containers;
- object records with package-local outer paths (`--exports`, `--imports`);
- a reusable `PackageStore` that indexes installed `.upk`, `.umap` and `.u`
  files lazily and resolves negative imports across packages (`--resolve`);
- a per-class export count for one package (`--census`), driven over a whole
  installation by `tools/census.py`;
- tagged properties (`--properties`): scalars, object references, nested
  structs, common fixed-layout structs, and arrays whose element type is
  supplied by a schema file;
- resident `Texture2D` mips to PNG (`--texture`, DXT1/DXT5, inline or
  TFC-streamed, with `--mip` and `--all-mips`);
- all render LODs of a `StaticMesh` in memory and a selected LOD to OBJ
  (`--mesh`, 16/32-bit indices, all UV sets retained by the importer);
- bulk scene metadata (`--scene-records <schema>`) and bounded raw payload
  bytes (`--payload <index>`) used by the level preparer.

General class/default inheritance and runtime package streaming are not
implemented. Python is used for scene preparation, tests and as an independent
execution path for comparison; the executable itself does not require Python.

### Inspect object records and properties

```powershell
$willowPackage = "C:/Program Files (x86)/Steam/steamapps/common/Borderlands 2/WillowGame/CookedPCConsole/WillowGame.upk"
$objects = & ./build/Release/ow-package.exe $willowPackage --exports | ConvertFrom-Json
$partDefault = $objects | Where-Object name -eq "Default__WeaponPartDefinition"
& ./build/Release/ow-package.exe $willowPackage --properties $partDefault.index --property-offset 4
```

Export indices are one-based; negative references identify imports, and zero
means null. `--resolve <reference> --cooked <directory>` follows an import to
the owning package and export path. Paths from `--properties` still describe
the current package's outer chain; resolution is an explicit separate step.

The property offset is relative to the export payload and must be supplied.
Offset 4 has been observed for class defaults, material instances, weapon
parts, textures and static meshes so far. It is not a universal object-prefix
rule and the four bytes' meaning is UNVERIFIED. Wrong offsets, missing
terminators, malformed values and payload overruns fail with an error and no
partial JSON result.

Supported values: int, finite float, bool, name, string, byte/enum, object,
class and component references, and structs. `Vector`, `Vector2D`, `Rotator`,
`Guid`, `LinearColor`, `Color` and `Quat` decode as fixed fields; other struct
types decode as a nested tagged stream. Nesting is capped at 32 levels.

Array tags do not serialize their element type, so arrays need
`--array-schema <file>` with `PropertyName=ElementType` lines
(`tools/phase0-arrays.schema` covers the material and weapon-part probes).
Element types may be `IntProperty`, `FloatProperty`, `NameProperty`,
`StrProperty`, `ObjectProperty`, `ByteProperty` or `StructProperty:<Type>`.
Arrays without a schema entry keep their tag metadata with
`status: "unsupported"` and `value: null`. Resolving element types from the
class's reflection data instead of a hand-written schema is future work.

Local check on 2026-09-10: `GD_Gladiolus_Weapons.AssaultRifle.AR_Barrel_Jakobs_Sawbar`
decodes all 13 top-level tags with the probe schema, consuming 1,551 bytes and
leaving zero trailing bytes. The decoded names and values match the BLCMM
Object Explorer dump, with one generated-subobject presentation discrepancy
recorded in [DECISIONS.md](../DECISIONS.md).

### Census and asset extraction

```powershell
$game = "C:/Program Files (x86)/Steam/steamapps/common/Borderlands 2"
python tools/census.py --reader build/Release/ow-package.exe --game $game --output local/census.json
python tools/prepare_probe.py --reader build/Release/ow-package.exe --game $game
```

Both write only under `local/`, which is ignored. The census exits non-zero
if any package fails to read and lists the error per package. The probe
writes `mesh.obj`, `texture.png` and a `probe.json` manifest with SHA-256
hashes of the package and outputs.

Texture extraction decodes every available resident mip and supports only
`PF_DXT1`/`PF_DXT5`/`PF_A8R8G8B8`/`PF_G8`; payload-at-end mips and other pixel formats still fail
explicitly. Mesh extraction reads every render LOD, 16- or 32-bit indices and
all UV sets; OBJ output intentionally writes one selected LOD and its first UV
set. Source mesh data and skeletal meshes remain future work; tagged convex/box
collision hulls are covered in the walking slice below.

## Verification status of the reader

Local checks on 2026-09-10, Release build, all against the installed game:

- Five synthetic CTest suites pass (package tables, properties, runtime,
  container, assets). They cover truncation, block totals, corrupt LZO streams,
  partial compression tables, malformed tags, nesting limits, DXT pixel
  decoding, PNG CRC/zlib framing, TFC bounds, and mesh buffer bounds.
- All nine code packages decode byte-for-byte identically to the Python
  research reader and match on every export's name, class, outer, super,
  payload size and offset. Core yields 234,397 bytes and 1,621 exports;
  WillowGame yields 56,443 exports.
- `tools/census.py` reads 2,008 of 2,008 packages found under the install
  (base game plus 1,096 DLC packages; two UHD texture sidecar files are
  identified and skipped) for 4,751,329 serialized exports, including 77,958
  `Texture2D`, 40,090 `StaticMesh`, 3,911 `SkeletalMesh`, 35,098 `Material`
  and 100,601 `AnimSequence`. These count serialized copies, not unique
  assets, and have not yet been cross-checked against umodel's view.
- `tools/prepare_probe.py` extracts `Env_Ash.Mesh.Ash_Road01` (473 vertices,
  784 triangles, 2 UV sets) and `Prop_Roads.Textures.MetalRoadConcrete_Dif`
  (1024×1024 DXT1, streamed from `Textures.tfc`, 11 resident mips) from
  `Ash_P.upk`, and confirms through the material's `TextureParameterValues`
  that the texture is that mesh's `p_Diffuse`.
- The UE5.8.2 host probe rendered this mesh and texture in
  `/Game/Phase0/Phase0`; the first Phase 0 screenshot is user-verified.

These checks establish agreement with the research reader and a visually
plausible texture, not independent proof of every format field or gameplay
compatibility.

## Host engine probe (Phase 0)

`host/ue5/OpenWillow/` is a minimal UE5 C++ project whose module refuses to
start without `OPENWILLOW_BL2` pointing at an installed game.
`tools/run_ue_probe.ps1 -Engine <UE5 root> -Game <BL2 root>` builds it and
launches the editor with `host/ue5/import_probe.py`, which imports the probe
OBJ and PNG, wires the texture into a material, and places the mesh, a camera
and lights in `/Game/Phase0/Phase0`. This editor import remains a diagnostic
spike, not the eventual runtime loader.

## Material v1 and the first map loader (Phase 1)

```powershell
python tools/prepare_level.py --reader build/Release/ow-package.exe --game $game --map Ash_P
python tests/level_test.py
./tools/run_ue_level.ps1 -Engine 'C:/Program Files/Epic Games/UE_5.8' -Game $game
```

The preparer follows `Ash_P`'s serialized sublevel references, extracts reusable
LOD0 mesh sections and four-channel materials, and writes `local/ash/scene.json`.
The UE5 importer creates `/Game/OpenWillow/Ash_P/Ash_P` with sublevel folders,
collection and ordinary actor transforms, section material overrides and a
spectator camera. Play controls: WASD/mouse, Q/E down/up. `_Dynamic` placements
remain static; there is no physics or script execution. Use `-ImportOnly` for a
headless editor import; that does not validate rendering or camera gestures.

Material v1 supports named diffuse/normal/specular/emissive texture parameters,
parent inheritance and named base-material defaults. It uses opaque lit shading,
linear normal/specular maps, sRGB diffuse/emissive, specular red, emissive RGB
masked by alpha, and constant roughness 0.65. This approximates UE3 shading.
Unsupported assets, graphs and interactive component owners are listed in
`scene.json` under `issues`. Terrain/BSP, skeletal meshes, lightmaps and full
material graphs are not loaded.

The imported map uses a reproducible inspection lighting rig: a warm movable
directional sun at intensity 1.0, a cool movable skylight at intensity 0.5
using UE's neutral gray light cubemap, a runtime sphere reflection capture
centered at the start camera, automatic exposure, and a 0.35
ambient-occlusion post-process override. The rig is anchored at the start
camera because UE3 sky/environment placements can carry intentionally huge
scales. UE3 lightmaps and native light actors are not translated yet, so this
rig is for geometry and material checks; its visual match to Borderlands 2
remains open.

`--scene-records tools/level-arrays.schema` is bulk CLI metadata for this slice;
`--payload <positive export index>` exposes bounded bytes for observed collection
tails. The Ash-specific prefixes and native collection layout remain subject to
independent visual/in-game validation; see [DECISIONS.md](../DECISIONS.md).

`-ImportOnly` writes the import commandlet log to `<scene>/ue-import.log` and
fails if any material reported `Failed to compile Material`; such a material
would otherwise render as UE's default checkerboard while the graph checks
below still pass. It also reopens and verifies the saved scene: placement counts,
collection transforms, material overrides, imported section bounds against
source OBJ vertices, and material graph/color-space settings. To exercise all
four channels independently of game data, run `python tests/prepare_ue_smoke.py`
then pass `-Scene local/material-smoke -ImportOnly` to the launcher. `-SkipBuild`
uses an already compiled host. All fixture data is synthetic.

Imports also run `host/ue5/verify_uv.py` in a fresh editor process. It checks
saved LOD0 UV0 bindings at every imported vertex instance against the OBJ,
including the inverse OBJ V conversion, and verifies oriented triangle
topology against the original game index order. Results are written to
`ue-uv-verify.json`. This checks the export/import path, not material-graph UV
operations or the original game's appearance. `python tests/prepare_uv_smoke.py`
creates a separate `local/uv-smoke` square: red top-left, green top-right,
blue bottom-left and yellow bottom-right when viewed from its saved camera.

The first Ash import contains 5,059 placements / 5,235 mesh sections and passes
saved-scene verification. A fresh UE5.8 Lit editor frame and a separate game
window confirm the textured start-camera view; wider-map visual coverage and
camera gestures remain open. See
[Material v1 / Ash verification](verification/MATERIAL_LEVEL_V1_VERIFICATION.md).

### Sanctuary and the free-flight viewer

The viewer uses the possessed spectator pawn as its camera after restart. Its
collision is disabled for free flight; this does not implement walking or UE3
collision parity. Open an already imported scene without another import:

```powershell
./tools/run_ue_level.ps1 -Engine 'C:/Program Files/Epic Games/UE_5.8' -Game $game -Scene local/sanctuary -ViewOnly -SkipBuild
```

Preparation defaults to a separate `local/<map-name>` directory, so preparing
`Sanctuary_P` does not overwrite the Ash manifest. To iterate on material
translation without extracting meshes again:

```powershell
python tools/refresh_materials.py --reader build/Release/ow-package.exe --game $game --scene local/sanctuary --reuse-textures --outer-shell
./tools/run_ue_level.ps1 -Engine 'C:/Program Files/Epic Games/UE_5.8' -Game $game -Scene local/sanctuary -ImportOnly -SkipBuild
./tools/test_ue_viewer.ps1 -Engine 'C:/Program Files/Epic Games/UE_5.8' -Game $game -Scene local/sanctuary
```

Use `--reuse-textures` only with the same unchanged game installation. The
refresh preserves placement data and resolves materials by both path and class.
`--outer-shell` is the opt-in hull policy described under the sky notes below;
omit it to keep the placed `_Teleported` overrides.

For the narrow Sanctuary hull placement experiment, rebuild the manifest with

```powershell
python tools/viewer.py --game $game --map Sanctuary_P --action prepare --outer-shell --matinee-first-key
```

(add `--sanctuary-geometry` when preserving the recovered terrain/BSP). This changes only
`InterpActor_29.StaticMeshComponent_393` to the observed first Matinee key;
the default serialized placement remains unchanged. See the
[hull placement note](verification/NATIVE_SKY_APPROXIMATION.md#matinee-first-key-placement-experiment)
for the explicit status and limitations.

On a machine without a discrete GPU, add `-LowEnd` to `run_ue_level.ps1`. It
starts the editor or standalone viewer with DX11/SM5 (no Nanite, no virtual
shadow maps), the lowest scalability groups, FXAA, 66% screen percentage and a
960x540 window. The first launch recompiles shaders for SM5 and is slow; later
launches reuse the cache. The switch changes rendering only; imported content
and saved scenes are identical with or without it. A recorded sample on an
Intel Iris Xe laptop is in [performance](verification/PERFORMANCE.md):
12–15 FPS on the default path (GPU-bound, mostly TSR) and the 60 FPS cap with
`-LowEnd`. To repeat it, `test_ue_viewer.ps1 -Profile` (optionally
`-LowEnd`) runs `OpenWillow.Profile`, which logs `stat unit`-style thread
times and a `ProfileGPU` breakdown; other machines will differ.

```powershell
./tools/run_ue_level.ps1 -Engine 'C:/Program Files/Epic Games/UE_5.8' -Game $game -Scene local/sanctuary -ViewOnly -SkipBuild -LowEnd
```
A single unnamed sample with an explicit `_Dif` texture name can supply
approximate diffuse color when there is no named diffuse parameter; this is
recorded as an inference, not reconstruction of stripped graphs or material
tint. The preparer also reads the cooked Material resource's texture reference
list after the tagged-property terminator; see
[Cooked-material verification](verification/COOKED_MATERIAL_VERIFICATION.md).

The runtime test checks pawn movement and camera following and captures three
settled views in `Saved/Screenshots/WindowsEditor`. It supplies engine movement
input, not physical keyboard/mouse gestures. The test process uses a 1 FPS
startup threshold to allow correctness inspection on slow maps; this is not a
performance pass. See
[Phase 1 viewer verification](verification/PHASE1_VIEWER_VERIFICATION.md).

The runtime test also logs a `Viewer diagnostic` record: 90 engine frame-time
samples during movement (mean and p95 milliseconds), current process physical
memory and peak process physical memory. These are short correctness-run
diagnostics, affected by startup, window state and shader work; they are not
a controlled renderer benchmark or GPU-memory measurement.

To list remaining diffuse gaps, their effective placed section uses, and the
recorded repair buckets:

```powershell
python tools/audit_scene_materials.py --scene local/sanctuary --output local/sanctuary/material-audit.json
```

This report separates absent diffuse, no supported channels, and unassigned
slots. It follows actor overrides and does not change the scene or choose
replacement textures. `gap_status_counts` distinguishes partial channels,
cooked-resource candidates and no-supported-channel fallbacks; `issue_counts`
groups unsupported component owners, invalid color streams, collision gaps and
approximations. Add `--all-gaps` when masked/translucent gaps should be printed
alongside the default opaque priority list. See the [Sanctuary material
baseline](verification/SANCTUARY_MATERIAL_BASELINE.md).

The audit also counts `surface_approximation` recipes separately. The two
inspected glacier materials use a partial primary diffuse/normal layer with
retained instance tiling on UV0; snow blend, glow and reflection remain open.
See [glacier validation and limits](verification/GLACIER_PRIMARY_LAYER.md).

  The generated UE5 inspection map now also receives a temporary
  `OpenWillow_SkyAtmosphere` actor, with the imported sun registered as its
  atmosphere light, plus a centred two-sided blue `OpenWillow_SkyFallback`
  shell for a readable inspection background. This supplies a visible
  non-black background while native `_Skybox` translation remains open;
  `PF_A8R8G8B8` texture extraction is now verified (see
  [record](verification/A8R8G8B8_TEXTURE.md)). Both actors are explicitly
  labelled in `ue-import.json` and `ue-verify.json` as
  `temporary_sky_fallback`; they are not visual-parity evidence.

To locate a map's native sky placements and explain why each one does or does
not get a diffuse under the current policy:

```powershell
python tools/sky_census.py --reader build/Release/ow-package.exe --game "C:/Program Files (x86)/Steam/steamapps/common/Borderlands 2" --map Sanctuary_P --extract
```

The census walks the persistent map and its streamed sublevels, lists every
placed sky-named `StaticMesh` (under `Prop_Skybox` or with `sky` in the object
name) with its owner and observed transform, the effective material per
section, each material's parent chain, all named sampler/scalar/vector
parameters, the cooked texture list and each `Texture2D`'s format, size and
cache. `--extract` writes the meshes as OBJ and the textures as PNG under
`local/sky/<map>/`. It changes no policy and interprets no stripped graph,
Kismet streaming state or lighting. See the
[sky census record](verification/SKY_CENSUS.md).

The normal scene preparation now carries the observed native
`Prop_Skybox.Meshes.Sky_Dome` placement into `scene.json` when its effective
material is Unlit. Its outward-facing source shell is imported two-sided under
`NativeSkybox/`, assigns
the recovered Material v1 approximation, disables collision and shadow
casting, and retains the UE5 atmosphere as a temporary fallback for unresolved
sky layers. A second Sanctuary `Sky_Dome` placement with a floor-material
override is deliberately left as ordinary geometry. See the
[native skybox verification record](verification/NATIVE_SKYBOX_VERIFICATION.md).

When the dome's effective material descends from
`Common_Materials.Sky.Mat_SkyTimeOfDay_Master`, the preparer also writes a
`sky_approximation` record from the named inputs that survive along the
instance chain (transition strip, cloud and mask textures, `Time_of_Day`,
brightness and opacity scalars). The importer builds one fixed graph from it:
the `Time_of_Day` column of the transition strip over dome V, blended toward
the strip's horizon row where `Clouds_01.R` is dense. The stripped master
graph is not decoded; the column reading (`/256`) is recorded as
`UNVERIFIED`, and the sun spot, masks, cloud motion and time-of-day animation
are listed as omitted. When such a dome is accepted, the blue
`OpenWillow_SkyFallback` sphere (which sits inside the dome) is not spawned;
the UE5 atmosphere stays. The verifier walks the saved graph back from
Emissive and reports `verified_sky_approximation_materials`.

The `Sanctuary_Outer` hull (`Prop_Skybox.Meshes.SanctuarySky` and its two
antennas) is placed with masked `_Teleported` overrides that Material v1
cannot recover, so it imports invisible by default. `--outer-shell` (on
`viewer.py --action prepare`, `prepare_level.py` and `refresh_materials.py`)
drops only those overrides whose mesh-default material resolved a diffuse,
records each replacement in the actor's `outer_shell_replaced` list, and
imports the actors under `OuterShell/` without shadow casting. Which of
`_Outer` and `_Land` the running game shows is Kismet state and is not
interpreted. See the
[sky approximation record](verification/NATIVE_SKY_APPROXIMATION.md).

The Hyperion station in front of the moon (`Prop_MoonBase.Mesh.MoonBase02`)
is an ordinary lit placement whose cooked texture list carries two `_Dif`
textures, so the generic rule gave up and it rendered as a dark silhouette.
`INSPECTED_COLOR_FALLBACKS` in `prepare_level.py` names its own
diffuse/normal/emissive textures for that placed instance; the tint, fog and
emissive-multiplier constants are recorded as omitted. The moon itself
(`Mati_Moon`, Unlit additive) is listed in `INSPECTED_UNLIT_COLOR_MULTIPLIERS`,
which records its `p_moonColor` vector as `unlit_color_multiplier`; the host
multiplies the recovered Unlit color by it and `verify_level.py` checks the
pair (`verified_unlit_multiplier_materials`). The station shadow mask,
crater relief and time-of-day tint stay omitted. See the
[moon base record](verification/MOON_BASE_SURFACE.md).

Observed blocking helpers are retained for source collision where recovered and
hidden from rendering: five `Common_Meshes.Blocking.Blocking_Cube` placements,
the 81 of 94 `Common_Meshes.CollisionCube` placements whose component sets
`HiddenGame`, whose owner actor sets `bHidden`, or which carry no material
other than the cube's own `Mat_Collision`, plus three oversized adjacent
`Sanctuary_P` placements (`StaticMeshActor_372`, `StaticMeshActor_690`, and
`StaticMeshCollectionActor_27` instance `SMC_544`), whose concrete-tile
overrides had made collision boxes visible across rooftops. The two actor
placements are roughly 10 x 9 x 4 m; the collection instance is about 5 m per
side. Their source collision remains enabled. Six cloud
`Blocking_Plane` placements (four `Mat_CloudLayer_Light`, two
`Mat_CloudLayer_01`) are hidden as well. The exact `Sanctuary_Outer`
`InterpActor_34` `Prop_Garbage.Meshes.BoxLrg` placement that blocked the start
view is also hidden. Two additional oversized `Sanctuary_Outer`
`Prop_Garbage.Meshes.BoxLrg` placements (`InterpActor_26` and `InterpActor_33`)
were identified in the UE viewport and hidden as visual-only blockers. `_26`
and `_33` retain their source collision. `_34` does not: its box encloses the
game's own `WillowCoopPlayerStart_0`, so it cannot block landed play (see
DECISIONS.md, 2026-09-23). The remaining 10
`CollisionCube` placements, including
the thin concrete street slabs in front of Scooter's garage, render with their
placed materials. Any other static (non-`InterpActor`) placement whose
component sets `HiddenGame` or whose owner sets `bHidden` is hidden the same
way, collision kept. Nearly all of these are `Sanctuary_Px` low-detail shells
and `_Low` sidewalk proxies that otherwise cover the street with roof atlases
(DECISIONS.md, 2026-09-23). This is a bounded visual artifact policy, not
complete collision or material parity.

Unlit Material v1 colors now feed Emissive Color when no explicit emissive
texture exists. The saved-scene verifier reports `verified_unlit_materials`.
See the [Unlit color verification](verification/UNLIT_COLOR.md) for scope
and the synthetic regression fixture.

To investigate the remaining cooked Material resource bytes before extending
serialization support:

```powershell
python tools/material_resource_census.py --reader build/Release/ow-package.exe --game "C:/Program Files (x86)/Steam/steamapps/common/Borderlands 2" --package Sanctuary_P --package Ash_P
python tests/material_resource_test.py
```

The report in `local/material-resources/material_resources.json` contains
validated prefix boundaries, raw texture indices, opaque-tail sizes, hashes
and unsigned words, and surviving expression-slot counts. Words have no
assigned shader semantics. Absent expression arrays remain distinct from
explicitly empty or stripped arrays. Unsupported prefixes are recorded as
errors and cause a nonzero exit status. The observed tail layout requires
exact consumption of six words, a count, 16-byte records and a final word;
unrecognized layouts also cause a nonzero exit status without discarding
the raw observations. The report directory must be under
this checkout's ignored `local/`; these reports must never be committed.

### First collision and walking slice

Prepared scenes now include observed RB_BodySetup convex and box hulls. The
collision refresh/import and walking regression have been verified on Sanctuary.
For a scene already imported with collision, launch the placeholder character:

```powershell
./tools/run_ue_level.ps1 -Engine 'C:/Program Files/Epic Games/UE_5.8' -Game $game -Scene local/sanctuary -ViewOnly -SkipBuild -Walk
```

Use WASD and mouse, with Space to jump. Omit `-Walk` for the free-flight viewer.
This uses UE5 movement defaults tuned for inspection; it does not reproduce BL2
movement physics. See [collision preparation, checks and limits](../COLLISION_WALKING_VERIFICATION.md).

### Selecting an installed map

`tools/viewer.py` lists persistent packages in the base-game cooked directory.
Inside the running viewer, Tab shows an on-screen list of prepared scenes
(`local/*/scene.json`) and imported maps (`Content/OpenWillow/<Map>/`); the
digit keys open an imported one, and entries without a saved map are marked
"not imported". `OWMapList` and `OWMapOpen <n>` do the same from the console.
The list never prepares or imports; `test_ue_viewer.ps1 -Selector` runs the
automated level-switch check.
Package discovery does not imply successful preparation or rendering. Add
`--include-dlc` to discovery and preparation to include the install's `DLC`
directory. The current install has 37 base-game and 45 DLC persistent maps.

```powershell
python tools/viewer.py --game $game --list
python tools/viewer.py --game $game --include-dlc --list
python tools/viewer.py --game $game --map SouthpawFactory_P --action prepare
python tools/viewer.py --game $game --map SouthpawFactory_P --action import --engine 'C:/Program Files/Epic Games/UE_5.8'
python tools/viewer.py --game $game --map SouthpawFactory_P --action view --engine 'C:/Program Files/Epic Games/UE_5.8' --skip-build
```

Omit `--map` for an interactive numbered selection. Use `--list --json` for
machine-readable discovery. Names are matched case-insensitively; duplicate
package names fail explicitly. Preparation writes to this checkout's
`local/<map-name>/`; import and view require a matching prepared manifest.
Import performs the existing saved-scene verification, while view requires an
already imported map. `--skip-build` requires this checkout's compiled host.

DLC preparation indexes packages across the install and locates the named
texture cache. A cache alongside the source package wins; otherwise the cache
name must be unique. Ambiguous cache names fail explicitly. The manifest records
`package_scope`, which material refresh reuses. Base-only preparation retains
the existing base-game search root. DLC discovery alone is not DLC compatibility.

## Where things are

| Path | What |
|---|---|
| `src/` | `ow-core` library (package reader, LZO container, asset importers) and the `ow-package` CLI |
| `tests/` | Synthetic CTest suites and scene-preparation unit tests; no game data |
| `tools/` | Census, probe/level preparation, material refresh, UE launch scripts, array schemas |
| `host/ue5/` | Minimal UE5 C++ project, editor-Python importer/verifier, in-game map selector, viewer/walking/selector/profile automation tests |
| `third_party/lzokay/` | Vendored MIT LZO1X decoder (provenance in `THIRD_PARTY.md`) |
| `research/` | Community-demand corpus, analysis scripts and the original Python package reader used as a comparison oracle |
| `docs/` | Plan, research, verification records; this file |
| `local/` | Ignored. Every game-derived output lands here |

## Sanctuary artifact trace

`tools/audit_sanctuary_artifacts.py --reader build/Release/ow-package.exe --game $env:OPENWILLOW_BL2 --scene local/sanctuary --output local/artifact-pass.json`
records IcePlate, WorldTransition and HLS effective placements plus source
properties and omitted terrain/BSP class counts. Use it after material refresh;
counts do not establish which missing floors terrain/BSP will fill. See
[the verification record](verification/SANCTUARY_ARTIFACT_PASS.md).

## Terrain floors

For a complete Sanctuary preparation, use:

```powershell
python tools/viewer.py --game $game --map Sanctuary_P --action prepare --sanctuary-geometry --outer-shell
```

This runs static-mesh preparation, terrain preparation, then BSP preparation
with triangle collision, stopping on a failed stage. It is scoped to Sanctuary.
Import the resulting scene normally. Native terrain blending, the BSP texel
scale and collision flags retain the limitations below.

`tools/prepare_terrain.py --reader build/Release/ow-package.exe --game $env:OPENWILLOW_BL2 --scene local/sanctuary --collision`
rewrites a prepared scene with one static mesh per TerrainComponent whose
vertex/strip data corroborates the terrain hole/diagonal flags, a labeled
material approximation, and (with `--collision`) triangle-mesh collision.
Cooked weighted materials now drive a host weighted sum when every weight
array matches its paired PF_G8 texture byte for byte (31/31 on Sanctuary).
Failed verification retains the labeled dominant-layer fallback. This does
not establish native shader or original-game visual parity. It
also writes `terrain-runtime.json`; `test_ue_viewer.ps1 -Terrain` then runs
`OpenWillow.TerrainWalking`, which stands on each terrain, drops into a
flagged hole cell and walks one component seam. Full viewer logs now include
`Terrain hole path:` records for every probe frame, with movement, floor and
downward-trace diagnostics. A displaced or occluded endpoint does not directly
verify the original hole location. The runtime summary distinguishes direct
original-point traces from endpoint assertions, obstruction and displacement.
See
[the terrain handoff](verification/SANCTUARY_TERRAIN_BSP_HANDOFF.md).

## Sanctuary root BSP polygons

`tools/prepare_bsp.py --reader build/Release/ow-package.exe --game $env:OPENWILLOW_BL2 --scene local/sanctuary --collision`
adds the persistent-level root `Model`/`ModelComponent` polygons of a frozen
Sanctuary scene as ordinary mesh sections: one section per component material
element, native material assignments, texture coordinates projected onto each
surface's own base point and texture axes (`--uv surface_axes`, the default;
`--uv planar` keeps the earlier 128 cm world-planar placeholder), and (with
`--collision`) host triangle collision. Every node, vertex-pool point, surface
plane and component membership must cross-check before any file is written;
volume-owned Models are rejected structurally. The axis field roles are
confirmed against the editor's Polys exports (below); the divisor that turns
projected distances into repeats is not in the package, so `--texel-scale`
(default 128) is recorded in the manifest as `UNVERIFIED`. Lightmaps and
collision flags are retained only as opaque hashes and remain `UNVERIFIED`.
The script is scoped to `Sanctuary_P`
plus `Sanctuary_Land` and also writes `bsp-runtime.json`;
`test_ue_viewer.ps1 -Bsp` then runs `OpenWillow.BspWalking`, which stands on
and walks 200 cm along an unobstructed upward-facing polygon of each model.
`python tests/bsp_test.py` covers the decoder on synthetic fixtures. See
[the BSP record](verification/SANCTUARY_BSP_POLYGONS.md).

Sections whose preparer could not choose a material (currently two terrains
labeled `neutral_constant`) import with the lit gray
`M_OpenWillowNeutralFallback` host material and are counted in
`ue-import.json` / `ue-verify.json` as `neutral_fallback_sections`. Before
this they fell through to UE's default `WorldGridMaterial` checkerboard.

## Repeatable inspection viewpoints

Create `local/sanctuary/inspection-views.json` with a `views` array containing
1 to 12 objects. Each object requires `location` (world centimeters),
`rotation` (pitch, yaw, roll in degrees), and `fov` (10 to 150 degrees).
For example, a synthetic viewpoint is
`{"location":[0,0,1000],"rotation":[-20,45,0],"fov":75}`.

Run `tools/test_ue_viewer.ps1 -Engine $engine -Game $game -Scene local/sanctuary -Inspect`.
The host waits eight seconds at each viewpoint, asserts camera position,
rotation and FOV, and requests numbered PNGs under the project's ignored
`Saved/Screenshots/WindowsEditor/` directory. Inspect the resulting files;
the camera assertions alone do not verify appearance or original-game parity.
The saved map and its starting pose are not changed. `-Inspect` is exclusive
with the walking, terrain, BSP, selector and profile test modes.

## Maya with the Infinity and Phaselock (host prototype)

The Infinity visual keeps only chosen fragments of UModel's pistol gestalt
glTF. Decode the gestalt part ranges with a local array schema, then filter:

```powershell
# local/infinity/gestalt.schema holds lines such as
#   GestaltInfos=StructProperty:GestaltInfo
#   Parts=StructProperty:GestaltPartInfo
./build/Release/ow-package.exe "$game/WillowGame/CookedPCConsole/Startup.upk" --properties <GestaltDef_Pistol index> `
  --property-offset 4 --array-schema local/infinity/gestalt.schema > local/infinity/gestaltdef.json
python tools/filter_gestalt_gltf.py --gltf <UModel GestaltDef_Pistol_GestaltSkeletalMesh.gltf> `
  --gestalt local/infinity/gestaltdef.json --output local/infinity/Infinity.gltf `
  --parts Pistol_Body_Vladof Pistol_Barrel_Vladof Pistol_Grip_Vladof Pistol_Scope_Vladof
```

To roll a gun from its balance data instead of naming parts by hand:

```powershell
python tools/weapon_recipe.py --reader build/Release/ow-package.exe --package "$game/WillowGame/CookedPCConsole/Startup.upk" `
  --balance GD_Weap_Pistol.A_Weapons_Legendary.Pistol_Vladof_5_Infinity --seed 3 --output local/items/infinity_3.json
python tools/filter_gestalt_gltf.py --gltf <gestalt glTF> --gestalt local/infinity/gestaltdef.json `
  --recipe local/items/infinity_3.json --output local/items/infinity_3.gltf
```

The recipe lists the merge chain, candidates, weights, name parts and the
rules that are still UNVERIFIED.

Import with `OPENWILLOW_PISTOL_GLTF` pointing at the filtered glTF and
`OPENWILLOW_INFINITY_TEXTURES` at UModel's PNG export of
`Mati_VladofLegendaryPistol_Infinity`, running
`host/ue5/import_infinity_proxy.py` as a `pythonscript` commandlet. The pistol
and Phaselock arm clips come from `tools/import_maya_combat_anims.ps1`.

`-owwalk -owmaya -owcombattest -owcombatshots` on `Sanctuary_P` spawns a
training dummy once Maya lands, runs a fixed aim/fire/Phaselock sequence and
writes `OWCombat_1_Idle` to `OWCombat_5_FiringWall` PNGs under
`Saved/Screenshots/WindowsEditor/`, then quits. Controls in play: LMB fire,
F Phaselock, 1 equip, 0 holster. See DECISIONS.md (2026-09-25) for what is
data-derived and what is estimated.

## BL2's HUD movie over UE5 (browser-overlay prototype)

Convert the UI movies into `local/ui/run` first (DECISIONS.md 2026-09-26:
`tools/extract_swfmovie.py`, then `tools/gfx_to_swf.py --localization
<install>/WillowGame/Localization/INT --inline-font-imports`, font library
first, and `tools/hud_harness_swf.py`), with a Ruffle web build in
`local/ui/run/ruffle`. Then:

```powershell
./tools/run_ue_flash_hud.ps1 -Engine $engine -Game $game            # editor; press Play
./tools/run_ue_flash_hud.ps1 -Engine $engine -Game $game -GameWindow
```

The script starts `tools/hud_overlay/serve.py` on port 8767 if it is not
running and launches Sanctuary as Maya with `-owflashhud=<url>` and
`-owflashskills=<url>`. The HUD overlay logs "OpenWillow Flash HUD page loaded"
and "HUD movie ready" in the UE log.
It is a prototype: see DECISIONS.md for what is driven and what is not.
The BL2-style StatusMenu Inventory movie is enabled by default; press **Tab**
(or **I**) for Inventory and **K** for Skills; the header tabs and the K/I keys switch between the two pages. Pass `-NoInventoryMovie` only to use the
fallback host inventory panel.
Enter or E on an equipped item starts a transfer comparison with compatible
backpack items. Move through candidates, then E or Enter to swap; Escape
cancels back to the equipped item. Backpack Enter remains a direct equip
shortcut. Comparison uses recorded movie card positions/scales and panel
tweens; numeric fields are restored after the unsupported native tween callback.
Weapon cells use static mesh previews generated from the locally exported
UModel glTF meshes. If those ignored previews are absent, run
`python tools/render_weapon_previews.py` to create them under
`local/ui/run/previews/`.

**F** opens the native UE weapon mesh preview; hold the left mouse button and
drag to orbit it, then **F** or **Escape** to return to inventory. This uses
the already imported `Weapons/Items/SK_<recipe ID>` asset. The host resolves
the stable inventory ID before loading a mesh, clamps pitch and limits frame
requests to 10 Hz. Its render target is sent to the page as an in-memory PNG;
no game payload enters source control. Unresolved gear meshes show unavailable.
Inspect layout and imported weapon materials remain approximations.

Existing UI Trace SDK observations can populate local gear cards without
guessing rolled parts or package identity:

```powershell
python tools/prepare_inventory_gear.py local/ui/traces/<trace>.jsonl --output local/inventory/observed_gear.json
python tests/inventory_gear_test.py
./tools/test_inventory_actions.ps1 -Port 8791 -Extra @('-owinventoryshots','-NoLiveCoding')
```

The exporter does not equip observations or merge them into a live manifest.
Review the output, preserve stable IDs and copy only reviewed entries into
`local/inventory/gear_manifest.json`. `funStatsMarkup` preserves observed Flash
TextField formatting; the adapter passes it only to the movie, never browser
HTML. `packageResolved` and `visualIdentityResolved` remain false. The capture
flag writes settled open/equip/inspect/close screenshots to the ignored UE
Saved/Screenshots directory. Drag/drop tests dispatch DOM events; key tests
use Slate, so physical mouse/keyboard verification remains separate.

Maya's inventory preview plays the game's own third-person `Idle_Inventory`
clip and draws an ink outline. To build them in a fresh worktree:

```powershell
# 1. UModel 1590 MD5 export of the third-person body and its AnimSets
umodel.exe -path=<CookedPCConsole> -game=border -export -md5 -out=local/external/umodel/maya-body-anims GD_Siren_Streaming_SF Skel_SirenBody SkeletalMesh
umodel.exe -path=<CookedPCConsole> -game=border -export -md5 -out=local/external/umodel/maya-body-anims GD_Siren_Streaming_SF Base_Siren AnimSet
# 2. Body reference pose, then conversion (--anchor none: no first-person camera correction; needs numpy)
$env:OPENWILLOW_CHARACTER_MESH='Skel_SirenBody'; $env:OPENWILLOW_CHARACTER_ANIM_FOLDER='ThirdPerson'
$env:OPENWILLOW_CHARACTER_REFERENCE='local/character/anim/body_ref_pose.json'; $env:OPENWILLOW_CHARACTER_ANIMS=''
UnrealEditor-Cmd.exe <uproject> -run=pythonscript -script=host/ue5/import_character_anims.py -unattended -nullrhi
python tools/prepare_character_anims.py --anchor none --mesh <...>/Skel_SirenBody.md5mesh --reference local/character/anim/body_ref_pose.json --animset <...>/AnimSet/Base_Siren --clips Idle_Inventory Idle_var1 --output local/character/anim/siren_body.json
# 3. Import the clips, then the outline material and matte character material
$env:OPENWILLOW_CHARACTER_REFERENCE=''; $env:OPENWILLOW_CHARACTER_ANIMS='Body=local/character/anim/siren_body.json'
UnrealEditor-Cmd.exe <uproject> -run=pythonscript -script=host/ue5/import_character_anims.py -unattended -nullrhi
# Optional default head colour correction for the inventory display only.
# UModel export includes the local property dump used by the look script.
umodel.exe -path=<CookedPCConsole> -game=border -export -png -out=local/external/umodel/maya-menu-head CD_Siren_Skin_Default_SF Mati_Default_Head MaterialInstanceConstant
$env:OPENWILLOW_MENU_HEAD_PROPS="$pwd/local/external/umodel/maya-menu-head/CD_Siren_Skin_Default_SF/MaterialInstanceConstant/Mati_Default_Head.props.txt"
UnrealEditor-Cmd.exe <uproject> -run=pythonscript -script=host/ue5/import_character_menu_look.py -unattended -nullrhi
# 4. Armed inventory pose, from the already exported Rifle_Siren AnimSet
# Use the Python environment with numpy installed. Paths passed to the UE
# commandlet must be absolute because its working directory differs.
python tools/prepare_character_anims.py --anchor none --mesh <...>/Skel_SirenBody.md5mesh --reference local/character/anim/body_ref_pose.json --animset <...>/AnimSet/Rifle_Siren --clips Idle_Inventory --output local/character/anim/siren_body_inventory_rifle.json
$env:OPENWILLOW_CHARACTER_ANIMS="InventoryRifle=$pwd/local/character/anim/siren_body_inventory_rifle.json"
UnrealEditor-Cmd.exe <uproject> -run=pythonscript -script=host/ue5/import_character_anims.py -unattended -nullrhi
```

Framing, clip and outline width are `Config=Game` properties of
`AOpenWillowInventoryMayaDisplay` (`[/Script/OpenWillow.OpenWillowInventoryMayaDisplay]`
in `DefaultGame.ini`). Without the clip Maya shows in bind pose; without the
outline material she has no ink line.
The selected inventory instance is resolved by the host to its recipe mesh;
weapons use the armed clip and attach to `R_Weapon_Bone`, gear returns to the
unarmed clip. Missing armed clip/mesh leaves the weapon hidden. The armed clip
is currently shared by weapon types: exact hold-definition/launcher differences
remain unverified. The display gun/outline also use stencil 247.

The same menu-look script creates `M_OW_MenuBackdrop`. Renderer setting
`r.CustomDepth=3` enables its stencil mask; the display reserves stencil 247
for Maya's body, head and outline copies. The material dims/desaturates the
world after tonemapping, excluding visible Maya pixels (with a scene-depth
check so occluded pixels are not exempt). Gain, saturation and vignette remain
configurable on the display actor. Without this material, world dimming is
disabled and the host logs a warning. Closing the menu destroys the display
and its post-process component. No new extraction is needed; regenerate this
local material after updating an existing worktree.

For Maya's Skills tab, convert `UI_StatusMenu.StatusMenu` and its shared imports
into `local/ui/run` with the same converter and a StatusMenu harness, then
prepare Maya's tree and icon movies from the installed packages:

```powershell
python tools/prepare_skill_tree.py --reader build/Release/ow-package.exe --game $game --output local/ui/run
python tools/hud_overlay/serve.py --movies local/ui/run
```

Open `http://127.0.0.1:8767/skills.html?points=41&action=1` for a standalone visual
check. In the UE game window, press **K** to open Maya's Skills tab, hover a
skill for its description, use the arrows to rotate branches, and press
**Esc** or the movie's close button to return to play. The standalone `points`
query is only a display check; the page never changes grades itself.

In UE the host owns Maya's level, XP and grades (`UOpenWillowSkills`). The host
earns no XP yet, so pass `-Extra @('-owlevel=45')` to start with points (one per
level from level 5; the default level 1 has none). Phaselock (**F**) works only
after its point is spent in Skills; `-owcombattest` starts at level 5 or more
with Phaselock trained. The HUD's XP bar shows the level number. Click a skill or the action
skill to spend: the page logs `OWSKILL {"branch":B,"tier":T,"cell":C}` to its
console (`-1,-1,-1` for the action skill), the host validates it (points left,
action skill first, tier unlock, max grade) and answers with
`owSkills({points, actionGrade, grades})`, where `grades` maps installed skill
object paths to ranks. Each decision is logged as "OpenWillow Skills spend
(B,T,C) accepted" or "refused: <reason>". Grades live on the walker, so they
survive closing and reopening the menu, but there is no save file yet. The host
reads the same `local/ui/run/skilltree_siren.json` (`-owskilltree=<file>`
overrides). Grade effects are not applied.

The info box and footer are built from the install. `prepare_skill_tree.py`
(via `tools/skill_stats.py`) writes per-grade stat lines from each skill's
SkillEffectDefinitions and AttributePresentationDefinitions, localized from
`<Package>.int`, plus the footer and "Next Level:" strings from
`WillowGame.int` with key names from `DefaultGame.ini` MenuInputMapArray.
`tools/hud_overlay/skill_info.js` arranges them into the info-box HTML.
Hovering selects a skill with the movie's own highlight; clicking or
**Enter** spends. Synthetic tests: `python tests/skill_stats_test.py` and
`node tests/skill_info_test.js`.
`OpenWillow.Skills` is a UE automation test of the spend rules on a synthetic
tree; run it in `-game` mode (see `tools/test_ue_viewer.ps1` for the pattern).

For a repeatable UE capture, launch the same script with
`-GameWindow -Extra @('-owcombattest','-owcombatshots','-owskillshots')`.
After the combat and inventory captures it opens Skills, writes
`OWCombat_8_Skills.png`, requests the page's close route, and writes
`OWCombat_9_AfterSkills.png` under UE `Saved/Screenshots` before quitting.
With `-owcombatshots` (test only) Maya starts at level 36 unless `-owlevel`
is given, so the level-30 recipes and the level-36 gear item are equippable;
the slots get one weapon per ammo type first, and demo currency and ammo
reserves are set (made-up numbers). It also writes `OWCombat_7b_InventoryCompare`
and `OWCombat_7c_InventoryInspect` by sending the page the Down, E and F keys,
then fires slot 2 from a nearly empty magazine to exercise reload. Other
switches: `-owslots=<2..4>` (unlocked weapon slots, default 4),
`-owmoney=<n>`, `-owerid=<n>`, `-owinventoryselftest` (synthetic inventory
round-trip checks, logged), **R** reloads in play.

`-owinvshots` (2026-10-04) is a separate capture that drives the inventory page
with the stock keys:
- open, backpack focus and the five sort modes;
- compare from either side;
- Inspect;
- the Skills tab, one second after the tab.
- Q "Toggle Overview" on the Skills page (`OWCombat_D7_SkillsOverview`).

It writes `OWCombat_D*.png` and is independent of `-owcombatshots`. The Skills
page is now loaded hidden at level start. The page logs `OWINVTIME js_skills_*`
lines with its open and paint times. `tools/test_inventory_actions.ps1` steps
15/16 are now `pagedown_selects_first_item_of_types` and
`pageup_returns_to_all_first_item`.

## Tracing the real game's UI code (golden files for menus)

With the community mod SDK installed in the game (THIRD_PARTY.md), copy
`tools/sdk_trace/openwillow_uitrace/` into `<game>/sdk_mods/` and put the
absolute path of the repository's `local/ui/traces` in a `trace_dir.txt` next
to its `__init__.py`. In game, enable "OpenWillow UI Trace" in the mod menu,
use the menus, then disable it to close the file. Then:

```powershell
python tools/sdk_trace/summarize.py                       # newest trace
python tools/sdk_trace/summarize.py --timeline StatusMenuExGFxMovie
```

Traces are game data and stay under `local/`. See DECISIONS.md 2026-09-26.

Tracer 0.2.1 adds object identity to return records and `out.D` to
`GFxUI.GFxObject:GetDisplayInfo` returns. Earlier traces record the getter's
input struct but omit its completed output; do not use zero-filled inputs as
proof of a flat transform. Native post-hook output timing remains to be checked
in a fresh original-game capture. Synthetic callback checks:
`python tests/ui_trace_returns_test.py`.

## Driving the real game (ground-truth captures)

`tools/real_game/realgame.ps1` (dot-source it) launches the installed game for captures and talks to it through
`tools/real_game/openwillow_realgame/`, our own Library mod for the community mod SDK (THIRD_PARTY.md). The mod runs
`*.py` command files on the game thread and writes `.out` replies; command scripts share one namespace. Needs
`$env:OPENWILLOW_BL2`. Everything it writes goes under ignored `local/realgame/`.

```powershell
. tools/real_game/realgame.ps1
Enter-RunLock; Backup-Saves; Install-Driver          # lock shared with UE runs; copy saves first
Start-Game @('-windowed','-ResX=1280','-ResY=720','-nostartupmovies')
Invoke-GamePy 'print(block_saves(), get_pc())'       # at the main menu, before loading a character
Invoke-GamePyFile tools/real_game/scripts/weapon_cards.py cards   # helpers: spawn, record, card trace
Invoke-Burst 'phaselock/cast' 7 40 { Send-Key F } 0.75            # QPC-stamped frames around a key press
Remove-Driver; Exit-RunLock
python tools/real_game/golden_cards.py               # join records, card trace and screenshots
python tools/real_game/golden_card_compare.py        # evaluator on the exact rolled parts vs the cards
Invoke-GamePyFile tools/real_game/scripts/weapon_dump.py cards     # every weapon part/type/name part value, live
python tools/real_game/golden_card_compare.py --live-data local/realgame/cards/live_weapon_data.json ...  # with the overlay
```

Input is scan-code keys (`Send-Key`, arrows included), `Send-ClickAt`, `Send-Wheel` and `Send-Drag`; it takes the
screen and keyboard, so only run it when nobody is using the machine. Rules learned the hard way: look objects up
again in every command (a stale weapon reference crashed the game), keep spawned items in memory and remove them
before any travel or quit, and compare the save folder with the backup afterwards. `scripts/phaselock.py` samples the
lift skill every frame and marks `StartActionSkill` and the weapon's reload/put-down calls. Results:
`docs/verification/REALGAME_GROUND_TRUTH.md`, DECISIONS 2026-10-02.
`scripts/ambient_npcs.py` (`Invoke-GamePyFile ... ambient_npcs.py ambient`) lists live pawns and dens (`amb_pawns`,
`amb_dens`, `amb_live`), samples positions over time (`amb_sample`) and places the player (`amb_goto`) for the
Sanctuary ambient NPC record. `amb_compose` reads each live citizen's materials and attached meshes (hair, hats, gear);
`amb_view_*`, `amb_cam` and `amb_aim_at` frame a pawn.

## Sanctuary ambient NPCs (host, behind a flag)

Off by default. `-owambient=<manifest>` (or `OPENWILLOW_AMBIENT`) spawns the town's citizens and runs the stock perch
cycle and node walks with stand-in rules (`docs/verification/SANCTUARY_AMBIENT_NPCS.md`, all movement rules UNVERIFIED).

```powershell
python tools/census_ambient_npcs.py                      # dens, points, node graph, perch definitions -> local/slice/
powershell -File tools/seed_ambient_npc_assets.ps1       # UModel extraction, clip conversion, import (needs OPENWILLOW_UMODEL)
powershell -File tools/seed_ambient_npc_assets.ps1 -Steps attach -Compose <amb_compose json>   # hair, hats, gear, head textures
python tools/prepare_ambient_world.py                    # host manifest local/slice/ambient_world.json (--observed copies a capture)
powershell -File tools/test_ambient.ps1 -Seconds 60      # self-test, takes the run lock; -Shots tours the viewpoints
```

## Reading the game's UnrealScript (bytecode disassembler prototype)

`research/script_disasm.py` (Python, read-only) turns every script function in the code
packages into pseudo-code. Needs `OPENWILLOW_BL2` or `--game <CookedPCConsole>`; generated
listings belong under ignored `local/`.

```text
python research/script_disasm.py --check                      # structural validation, ~10 s
python research/script_disasm.py WillowGame.upk StatusMenuInventoryPanelGFxObject.PanelOnInputKey
python tests/script_disasm_test.py                            # synthetic tests, no game needed
```

Native functions print as `native_<n>` and their bodies are not in the packages (the sort
cycle of the inventory is one). Layouts marked UNVERIFIED in the source are fitted, see
`docs/verification/SCRIPT_BYTECODE_DISASM.md`.

The C++ loader and VM now support `--script-check`, `--disasm`, `--run`,
`--run-batch` and `--vm-sweep`. The two diagnostic harnesses are:

```powershell
python tools/vm_census.py --reader build/Release/ow-package.exe `
  --json local/phase2/vm-census.json
python tools/replay_trace.py local/ui/traces/<trace>.jsonl `
  --reader build/Release/ow-package.exe --limit 400 `
  --output local/phase2/trace-replay.json
```

Replay accepts `--function <substring>` and `--cooked <directory>`. It compares
scalar returns on newly instantiated receivers with class defaults, skips lossy
objects/aggregates/out results, and blocks native/stub executions from match
counting. Matches do not establish live-state parity. Pairing rejections,
mismatches and skip/block reasons remain in the local report. Evidence and
remaining work: [VM prototype](verification/SCRIPT_VM_PROTOTYPE.md).

Batch format: one case per line, `Package.Class.Function<TAB>Package.SelfClass`
followed by `<TAB>Parameter=kind:value` fields. `i/f/b/y/s/n/o` are explicitly
typed int/float/bool/byte/string/name/null-object; `d/t` adapt numeric/text input
to the reflected parameter type. String/name/text bodies are UTF-8 hex; bool
is `0` or `1`, null-object has an empty body. Missing trailing optional inputs
retain defaults; other missing inputs fail. Output is JSONL with case index,
result type/value, native flag, per-case error and unimplemented-call list.
The batch command exits zero when individual cases fail: inspect those fields.

The UE5 inventory now links the C++ VM for item-only backpack movement. Build
`cmake --build build --config Release` before the Win64 UE editor target; its
module links `build/Release/ow-core.lib` and `ow-lzokay.lib`. Inventory opens
the VM lazily from `OPENWILLOW_BL2`. `OWINVVM` log entries carry script expression
counts/errors; page reports include `vm.enabled/calls/errors/steps/discarded`.
The in-engine suite includes `vm_backpack_down` and `vm_backpack_up`, which must
observe new successful script calls and the expected actual page selections.
Equipment, transfers and sorting still use the existing host adapter.

For a direct installed-script check:

```powershell
$cooked = Join-Path $env:OPENWILLOW_BL2 'WillowGame/CookedPCConsole'
build/Release/ow-package.exe "$cooked/WillowGame.upk" --inventory-move 1 0 5 --cooked $cooked
```

The provider currently contains item-only rows. Headers and empty-cell kinds
are not modelled by this bridge yet. If UE stalls waiting for the local Zen
server before loading, the inventory runner accepts the installed fallback:
`-Extra @('-ddc=InstalledNoZenLocalFallback','-d3d11')`. This is the verified
launch-time workaround for this run,
not a change to project cache settings or runtime verification by itself.

### Native functions and sharing local files

The bodies of `native_<n>` functions are in `Borderlands2.exe`, not the packages.
Analysing the executable locally is allowed; the workflow, what may be committed and the
tooling notes are in [NATIVE_ANALYSIS.md](NATIVE_ANALYSIS.md) (policy:
[LEGAL.md](LEGAL.md), "Analysing the executable"). `tools/private_sync.ps1` mirrors
non-regenerable game-derived files (such as an analysis database under `local/analysis`)
to a store outside the repository; see "Working on two machines" in that page.

## Tools added 2026-10-01 and 2026-10-02 (one line each)

Everything here writes game-derived output only under ignored `local/` (or, for Ghidra, `%OPENWILLOW_ANALYSIS%`), and
every reading it supports stays `UNVERIFIED` until it is compared with the running game.

Native analysis of the executable (local only; policy in [LEGAL.md](LEGAL.md), workflow in
[NATIVE_ANALYSIS.md](NATIVE_ANALYSIS.md), "Native registration and queries"; DECISIONS 2026-10-02):

| Tool | What it does | Command |
|---|---|---|
| `tools/ghidra/run.ps1` | Drives the imported Ghidra project headlessly; `-Tables` builds the native tables, `-Query` decompiles by name, native number, string, callers or vtable slot | `powershell -File tools/ghidra/run.ps1 -Tables -Apply` / `... -Query MissionTracker.UpdateObjective,native:114 -Callees 1 -Label` |
| `tools/ghidra/OwNativeTables.java` | Ghidra script (run by `-Tables`): reads the registration tables, writes `natives.tsv` and `gnatives.tsv` and labels each native | via `run.ps1 -Tables` |
| `tools/ghidra/OwNativeQuery.java` | Ghidra script (run by `-Query`): resolves queries, writes decompilation under the analysis output folder; its header lists every query form | via `run.ps1 -Query '@<file>'` |
| `tools/ghidra/script_natives.py` | Lists every native `UFunction` of the code packages with its `iNative` number (input to the join above) | `python tools/ghidra/script_natives.py <out.tsv> [--cooked <CookedPCConsole>]` |
| `tools/ghidra/class_layout.py` | Computes 32-bit field offsets of a script class from the packages so a native's field read can be named (oracle: `Core.Object` = 0x3C) | `python tools/ghidra/class_layout.py WillowGame.MissionTracker` |
| `research/mission_event_link_ids.py` | Structural oracle: counts the link ids on mission events and reports any outside the ranges the dispatch note predicts; aggregate counts only | `python research/mission_event_link_ids.py [--packages Startup ...] [--examples]` (needs `OPENWILLOW_BL2`) |

`ow-package` modes added for the slice and the paint work (none changes the install; the executor modes run installed data through our own code, not the game):

| Mode | What it does | Command |
|---|---|---|
| `--payload-file` | Writes an export's raw bytes to a file (a shader cache is too large for `--payload`'s JSON) | `ow-package "$cooked\RefShaderCache-PC-D3D-SM3.upk" --payload-file 1 local/paint_research/refcache.bin` |
| `--names` | Prints the package name table in index order as JSON | `ow-package "$cooked\RefShaderCache-PC-D3D-SM3.upk" --names > local/paint_research/ref_names.json` |
| `--mission-run` | Runs one mission's native executor over installed data; steps `accept`, `kickoff`, `obj:<name>[:<bit>]`, `custom:<name>`, `turnin`, `tick:<s>` | `ow-package "$cooked\Startup.upk" --mission-run GD_Z1_RockPaperGenocide.M_RockPaperGenocide_Fire --cooked $cooked accept kickoff obj:RockPaper_GoToRange turnin` |
| `--behavior-run` | Runs one behavior provider alone; steps `enable:`, `disable:`, `tick:`, `event:<name>[:...]` and `fire:<link id>:<event>` (link-id filter, -1 = all links) | `ow-package "$cooked\Sanctuary_Dynamic.upk" --behavior-run <provider-path> --cooked $cooked fire:-1:OnTakeDamage` |
| `--kismet-run ... --tick` | Runs a Kismet sequence from an entry point (`--remote`, `--mission`, `--op`, `--originator`); trailing `--tick <s>` pairs advance time and run what is due | `ow-package "$cooked\Sanctuary_Dynamic.upk" --kismet-run TheWorld.PersistentLevel.Main_Sequence.RocksPaperGenocide --cooked $cooked --remote RE_Ep14_OpenMarcusDoor --tick 1 --tick 3` |

The step and event names above are examples from the verification records; the rules the executors follow are the
`UNVERIFIED` notes in [NATIVE_MISSION_DISPATCH.md](verification/NATIVE_MISSION_DISPATCH.md).

Weapon paint and effects (detail in "Weapon paint: where the Master_Gun reading comes from" above):

| Tool | What it does | Command |
|---|---|---|
| `tools/material_static_parameters.py` | Decodes the static parameter set a cooked MIC keeps after its properties (631 of 631 in `Startup.upk` consume their bytes exactly) | `python tools/material_static_parameters.py --reader build/Release/ow-package.exe --package "$cooked\Startup.upk" --mic <name>... --output local/paint_research/static_params.json` |
| `tools/weapon_paint_model.py` | Library: our own-words reading of Master_Gun's colour model, shared by the preparer, the thumbnail renderer and the tests (not a command) | `python tests/weapon_paint_test.py` |
| `research/d3d9_bytecode.py` | Library: our own SM3 (vs_3_0 / ps_3_0) token reader, written from Microsoft's public D3D9 bytecode description; `disassemble(code)` prints a plain register listing; listings are game-derived and stay under `local/` | imported by the paint research; checked in `python tests/weapon_paint_test.py` |
| `research/particle_system.py` | Reads cooked `ParticleSystem` templates (emitters, LODs, modules, baked distributions, `BurstList`, `DynamicParams`) to JSON under `local/phaselock/emitters/`; reads only, renders nothing | `python research/particle_system.py [Part_SirenASHandOrb ...] [--package GD_Siren_Streaming_SF] [--oracle]`, tests `python tests/particle_system_test.py` |

Inventory open-time benchmark (DECISIONS 2026-10-02; results in the 2026-10-02 section of
[INVENTORY_MOVIE_PROTOTYPE.md](verification/INVENTORY_MOVIE_PROTOTYPE.md); one PC, no original-game figure):

- `-owinvopenbench=<N>` and `-owinvopenbenchdelay=<s>` (UE command line) open the inventory N times and log `OWINVTIME`
  lines per open; `-owinvnopreload` turns off the level-start preloads for an A/B comparison (it is not a test default).
- `powershell -File tools/test_inventory_actions.ps1 -OpenBench 5 [-OpenBenchDelay 20] [-Items local/items/slice]
  [-NoInventoryMovie]` runs the benchmark instead of the action suite and prints one row per open. Pass
  `-Extra @('-ddc=InstalledNoZenLocalFallback','-d3d11','-owinvnopreload')` for the no-preload run; `-Extra` replaces the
  default list, so repeat the defaults.

## Independent oracles: umodel and the game's own object dumps

Two external oracles are run against the existing decode. Neither is copied
from; both stay on the user's machine and write only to ignored `local/`.
See THIRD_PARTY.md for provenance and docs/LEGAL.md for the rules.

### umodel

umodel (UE Viewer, MIT) is a second reader of the same bytes. Point
`--umodel` at `umodel.exe`; the Borderlands game tag is `border`.

```powershell
python tools/crosscheck_umodel.py --umodel $umodel --reader build/Release/ow-package.exe `
  --game $env:OPENWILLOW_BL2 --dlc --output local/umodel/crosscheck-all.json
```

compares umodel's `-list` with our `--exports` per package: index, serial
offset, serial size, class short name and object name. Name differences caused
by umodel's own normalization (`__name_N__` for names holding a control or
non-ASCII byte, and one trimmed trailing space) are reported in a separate
`name_normalized` bucket and do not fail the run; any other name difference
does. Drop `--dlc` for the 914 base packages only.

Exporting assets for the second tool uses umodel directly:

```
umodel.exe -game=border -export -gltf -png -nolightmap -uncook -groups `
  -path=<CookedPCConsole> -out=local/umodel <Package>
```

```powershell
python tools/crosscheck_umodel_assets.py --scene local/sanctuary/scene.json `
  --umodel-exports local/umodel --reader build/Release/ow-package.exe `
  --game $env:OPENWILLOW_BL2 --output local/umodel/crosscheck-assets.json
```

compares each scene mesh with the matching `<Level>/<Outer>/<Group>/<Name>.gltf`
on section count, triangles, referenced vertex count, positions and UVs, and
each decoded PNG with umodel's. The glTF-to-UE axis mapping is chosen by search
over all 48 permutation/sign combinations and reported in the output rather
than assumed, as is the UV V flip. A maximum per-channel texture difference of
1 is reported as `agree_within_1`: that is DXT decoder rounding, not a decode
disagreement. Meshes umodel does not export (terrain, BSP) are
`oracle_missing`. `tests/crosscheck_umodel_test.py` and
`tests/crosscheck_umodel_assets_test.py` cover both on synthetic fixtures.

### OpenBLCMM object dumps

OpenBLCMM ships the output of the game's own `obj dump` console command: an
SQLite index (`%LOCALAPPDATA%/OpenBLCMM/extracted-data/BL2/data.db`) into
per-class dump files packed in `blcmm_data_BL2-*.jar`. Unlike umodel this is
not a second decoder — it is what the running engine reported, so it is an
oracle for observed behaviour.

`python tools/blcmm_dumps.py "<object name>" [--raw]` prints one object.
Level objects are named `Sanctuary_P.TheWorld:PersistentLevel.Terrain_2`, with
a colon after `TheWorld`.

```powershell
python tools/crosscheck_blcmm_dumps.py --scene local/sanctuary/scene.json `
  --reader build/Release/ow-package.exe --game $env:OPENWILLOW_BL2 `
  --output local/blcmm/crosscheck.json
```

compares four things: `TerrainComponent` section base/size and `Bounds` (our
vertices mapped through the reported `_LocalToWorld`, allowing for the constant
one-unit bounds expansion and for the section base the component matrix already
carries); `ModelComponent` node count, element count and owning `Model`; actor
placement against `_LocalToWorld`; and material texture picks against the
effective `TextureParameterValues` through the `Parent` chain.

`--reader`/`--game` are needed only for the material comparison, which resolves
texture PNG file names back to the texture objects they came from.
`InterpActor` placements that differ are reported as `mover`, not as
disagreements: a dump shows where a matinee-driven actor had moved to, not its
cooked placement. Channels with no corresponding parameter are
`unparameterised` and are likewise not disagreements — the oracle is simply
silent on them. Unrecognised texture parameter names are listed rather than
mapped to a channel by guesswork.
`tests/blcmm_dumps_test.py` and `tests/crosscheck_blcmm_dumps_test.py` cover
the parser and the comparisons on synthetic dump text.

Results for both: [umodel record](verification/UMODEL_CROSSCHECK.md),
[dump record](verification/BLCMM_DUMP_CROSSCHECK.md).

## BSP texture axes against the editor's Polys exports

Neither oracle above sees BSP surfaces, but cooked volume-owned Models keep an
editor `Polys` export whose FPoly records carry explicit `Base`, `TextureU`
and `TextureV` vectors.

```powershell
python tools/crosscheck_bsp_polys.py --reader build/Release/ow-package.exe `
  --game $env:OPENWILLOW_BL2 --output local/bsp/polys-all.json
```

dereferences each surface record's base-point and texture-axis indices through
our reader and compares them with the FPoly on the same plane, across every
non-`_SF` package (`--packages` narrows it). It exits non-zero on a `differ`,
an out-of-range reference or a parse error; `no_unique_poly` (stale editor
vertex lists, duplicate coplanar polys) is reported, not counted. The report
also carries negative controls for the other int slots.
`tests/crosscheck_bsp_polys_test.py` covers it on synthetic fixtures. Results:
[texture-axis record](verification/BSP_TEXTURE_AXES.md).

Inventory backpack scrolling uses seven full rows and a clipped eighth-row
preview. Mouse wheel/chevrons move one row; PageUp sorts forward and PageDown
sorts backward, as resolved in the original-game trace. Drop/Sort are disabled
while swapping. The in-engine action runner verifies ordering, top clamp,
selection retention and native-cell/hit-target alignment. See
[verification](verification/INVENTORY_MOVIE_PROTOTYPE.md) for remaining stock
input and visual parity gaps.

E/Enter on a backpack item starts a transfer: choose an unlocked weapon slot
with its number, Up/Down keys or cell click, then E/Enter to confirm. Gear targets
its matching slot. Escape cancels while keeping the source selected. An empty
destination offers Equip, an occupied destination Swap. These flows reuse the
host's equip validation; exact stock grid traversal remains unverified.

Maya's default inventory ScreenX anchor is .86. The action runner samples her
live Head bone for 13 seconds (a full armed idle loop) and reports normalized
bounds. This checks head-anchor framing at the test viewport; full silhouette,
weapon clipping and exact stock pose remain separately unverified.

An optional paint pass applies the existing thumbnail palette approximation to
one already imported recipe mesh, without reimporting meshes or deleting folders:

```powershell
python tools/prepare_weapon_paint.py --recipe local/items/smg_maliwan_epic_1.json `
  --materials local/external/umodel/weapon-materials-20260929 `
  --output local/items/paint/smg_maliwan_epic_1.json
$env:OPENWILLOW_WEAPON_PAINT = "$pwd/local/items/paint/smg_maliwan_epic_1.json"
& 'C:/Program Files/Epic Games/UE_5.8/Engine/Binaries/Win64/UnrealEditor-Cmd.exe' `
  "$pwd/host/ue5/OpenWillow/OpenWillow.uproject" -run=pythonscript `
  "-script=$pwd/host/ue5/import_weapon_paint.py" -unattended -nullrhi
```

Run this after `import_weapon_items.py`, which replaces item materials. The
prepared JSON and imported textures/materials remain ignored. This is tested
only for the selected Maliwan epic SMG; packed normal/emissive semantics,
pattern placement and original Master_Gun lighting remain unverified.

`--recipe` also accepts several explicit paths, writing a list when there is
more than one. The UE importer accepts either form. Inputs are validated before
preparation writes its output; unsupported pattern inputs fail rather than
silently choosing a substitute. Keep existing Infinity paint imports separate.

Audit Maya's third-person hold references without changing the package decoder:

```powershell
python tools/audit_maya_menu_pose.py --reader build/Release/ow-package.exe `
  --game $env:OPENWILLOW_BL2 --output local/character/anim/hold-reference-audit.json
```

The script checks the reflected AnimSetList inner property type, supplies that
metadata to the existing CLI, and validates local reference identity/class.
It retains unsupported WeaponActions explicitly. Shared Rifle_Siren references
do not by themselves prove inventory action selection or IK behaviour.

`tools/hud_overlay/probe_panel_projection.js` is a developer-only rendering
benchmark, not loaded by the inventory. Evaluate its repository source in the
collaborative preview, then start `owProbePanelProjection()` asynchronously and
poll `window.owPanelProjectionProbeState`. A visual hold argument of at most
10000 ms allows a screenshot before cleanup. It creates a temporary second
Ruffle player, isolates equipment-panel art and tests a synthetic CSS plane
with a matching HTML target. Angles/perspective are synthetic, not stock values.


## Bounded Sanctuary mover bridge

`tools/prepare_mover.py` follows one installed SeqAct_Interp/group/actor binding
and emits an ignored local manifest using the owned reader and existing scene
payloads. Build CMake Release before UE5. Pass `-owmover=<manifest>` to enable
Maya's nearby E interaction; no manifest leaves the bridge inactive.
`tools/test_mover.ps1` owns the shared editor lock and a two-cycle input/movement/
collision test. It refuses an existing editor and never saves the map. Commands,
limitations and evidence: [mover record](verification/SANCTUARY_MOVER_PROTOTYPE.md).


## Sanctuary Fire-mission slice data (stock values instead of stand-ins)

These tools recover the data the slice mission needs from the installed packages with the owned
reader. All output is game-derived and stays under ignored `local/`. Everything they report is
structural: nothing here has been compared against the running game yet, and each record marks
what is fitted or `UNVERIFIED`.

```powershell
$reader = "build\Release\ow-package.exe"
$game   = $env:OPENWILLOW_BL2
$cooked = "$game\WillowGame\CookedPCConsole"

# World placement: range trigger, Marcus and his walk, dummy spawn, target mover, respawn point
python tools/prepare_slice_world.py --reader $reader --game $game      # local/slice/world.json
python tools/slice_values.py --reader $reader --game $game             # XP reward, Maya health

# Phaselock and one upgrade path (Suspension) from skill data
python tools/prepare_action_skill.py --reader $reader --game $game     # local/character/action_skill_siren.json

# Weapons: legal parts of a balance, card stats, the slice guns, drop tables
python tools/weapon_balance.py --reader $reader --package "$cooked\Startup.upk" parts --help
python tools/weapon_slice_gear.py --reader $reader --game $game --level 8 --seed 1   # local/items/slice
python tools/loot_pools.py --reader $reader --package "$cooked\Startup.upk" table --help
python tools/weapon_card_audit.py --reader $reader --package "$cooked\Startup.upk" --trace <ui trace .jsonl>

# Audio identity chain (no decoding): UE3 AkEvent -> Wwise event -> bank -> .wem
python tools/audio_census.py census
python tools/audio_slice_chain.py --reader $reader                     # local/slice/audio.json

# Marcus, the target dummy and the stock Maliwan pistol: UModel export + UE import job
python tools/slice_npc_assets.py all                                   # local/slice/npc_assets.json
powershell -File tools/seed_slice_npc_assets.ps1                       # runs tools/slice_npc_editor.py in the editor

# Player side: turn-in loot stand-in, mission pistol paint, then the UE assets (additive; see below)
python tools/weapon_slice_gear.py --game $game --reward-only --gestalt local/gestalt `
  --gltf local/external/umodel/gestalt/Startup/SkeletalMesh3              # local/items/slice/slice_reward_roll.*
python tools/prepare_weapon_paint.py --recipe local/items/slice/slice_mission_pistol_fire.json `
  --materials local/external/umodel/slice-npc/Pistol --mesh /Game/OpenWillow/Weapons/MaliwanPistol/SK_Pistol_Maliwan_2_Fire_seed1 `
  --reader $reader --package "$cooked\Startup.upk" `
  --output local/items/paint/slice_mission_pistol_fire.json
powershell -File tools/seed_slice_player_assets.ps1                    # steps fx, items, paint

# Slice guns: export each recipe's MaterialInstanceConstant chain, prepare, then paint Weapons/SliceItems
umodel.exe -path=$cooked -game=border -export -png -out=local/external/umodel/slice-guns Startup <Mati name> MaterialInstanceConstant
python tools/prepare_weapon_paint.py --recipe <local/items/slice/slice_*.json ...> `
  --materials local/external/umodel/slice-guns --mesh-folder /Game/OpenWillow/Weapons/SliceItems `
  --reader $reader --package "$cooked\Startup.upk" --output local/items/paint/slice_guns.json
powershell -File tools/seed_slice_player_assets.ps1 -Steps paint -Paint local/items/paint/slice_guns.json
```

`--reader/--package` lets the paint tool fill scalar and vector parameters that no MIC sets from the base
Material's own parameter expressions (they survive in the cooked package although the graph is stripped) and
records each value's source; without them a chain that leaves a zone colour to the base material fails rather
than guessing. With them it also reads the leaf MIC's static parameters, which choose the detail, pattern and
decal channels. The colour model is described in the next section. `tools/slice_npc_assets.py` needs numpy and
Pillow in the Python that runs it.

### Weapon paint: where the Master_Gun reading comes from

Master_Gun's expression graph is stripped from the cooked packages, so the paint model was recovered from compiled
data instead (2026-10-02, AI-assisted, `UNVERIFIED` against the running game):

```powershell
# Static parameter sets of every MaterialInstanceConstant with a static permutation (exact-consumption oracle)
python tools/material_static_parameters.py --reader build/Release/ow-package.exe `
  --package "$cooked\Startup.upk" --mic Mati_MaliwanUncommon Mati_JakobsCommonPistol `
  --output local/paint_research/static_params.json
# Raw shader cache object for local study (220 MB, stays under local/)
build/Release/ow-package.exe "$cooked\RefShaderCache-PC-D3D-SM3.upk" --payload-file 1 local/paint_research/refcache.bin
build/Release/ow-package.exe "$cooked\RefShaderCache-PC-D3D-SM3.upk" --names > local/paint_research/ref_names.json
# Preview the reading on a prepared gun (pure Python thumbnail renderer)
python tools/render_weapon_previews.py slice_pistol --items local/items/slice `
  --paint local/items/paint/slice_guns.json --output local/paint_research/thumbs --jobs 1
```

- `tools/material_static_parameters.py` decodes the bytes a cooked MIC keeps after its properties: its own
  compiled resource block, then its static parameter set. 631 of 631 such MICs in `Startup.upk` consume their
  bytes exactly. A MIC stores its resolved set, so the leaf MIC alone gives the channels.
- In the shader cache each Master_Gun shader map is keyed by such a static set and carries its uniform expression
  set (which parameter feeds which pixel constant and sampler) and the compiled D3D9 SM3 shaders.
  `research/d3d9_bytecode.py` is our own token reader, written from Microsoft's public description of the
  format; it is not a disassembler from a third party. Its output is game-derived and stays under `local/`.
- What the base-pass pixel shader says is written down, in our own words, in `tools/weapon_paint_model.py`. The
  short form: `p_Masks` stacks a light/dark map (upper half) over the zone mask (lower half); zone tones go from
  Midtone towards Hilight and Shadow by those maps; zones are blended over `p_DColor`; pattern and decal multiply
  (or replace) by squared mask weights; the result is multiplied by one detail atlas channel.
- `host/ue5/import_weapon_paint.py` draws the same model in a Custom node, including the `P_SimpleReflect`
  environment term (sampled at the tangent-space reflection vector, as the compiled shader does). Emissive and the
  game's lighting are not reproduced. Shading inputs (pass 3, `UNVERIFIED`): the compiled base and light passes
  multiply the material colour by 0.4 before lighting and take no specular from the material (only
  `pow(R·L, 15)` times the engine's override), so the importer uses base colour 0.4 × colour, metallic 0,
  specular 0, roughness 1. Under the host's current scene lighting this renders much darker than pass 2.
- Colour space (pass 2): each texture follows its installed `SRGB` flag, which `prepare_weapon_paint.py --reader`
  records in `srgb`. `Engine.upk`'s `Default__Texture` serialises SRGB on; only `p_Masks` and the normal maps
  turn it off, so the detail atlas, patterns, decals and environment maps are sRGB. Pass 1 had imported the
  detail atlas as linear, which flattened grime and rust contrast. Vector parameters are linear colours.

`tools/seed_slice_player_assets.ps1` holds `local/ue_run.lock` for each editor launch and never deletes
`Weapons/Items`, Maya's folder or slice NPC content. `fx` runs `host/ue5/import_infinity_proxy.py` only when
`Weapons/InfinityProxy` is absent (it supplies `M_OW_FxAdditive` for tracers, flashes and the Phaselock shell, and
`M_OW_BulletHole`). `items` runs the new additive `host/ue5/import_slice_items.py` (pool-rolled slice guns and the
loot stand-in into `Weapons/SliceItems`, grey stand-in material; existing `SK_<id>` assets are skipped). `paint`
runs `import_weapon_paint.py`, which now accepts a `mesh` target and MIC chains without a pattern texture
(zone colours, plus the decal reading described above). `tools/run_quest.ps1` and `tools/test_quest.ps1` pass
`-owitems=local/items/slice`, `-owactionskill=local/character/action_skill_siren.json` and Maya's level from
`slice_manifest.json` (an UNVERIFIED slice choice; `run_quest.ps1 -Level N` overrides it).

Gun visuals (2026-10-04, `docs/verification/WEAPON_VISUALS.md`):
- `tools/weapon_refresh_fragments.py` rebuilds the fragment list (body variants, no `*_None` parts, hidden-bone
  triangles cut) of existing local recipes; `tools/weapon_refresh_stats.py` re-evaluates their `stats`, optionally on
  the live overlay; `host/ue5/import_gun_meshes.py` re-imports the meshes of named ids only.
- `-owgunshots -owgunids=<id,id>` captures each gun in first person and from the side (`OWGun_<id>.png`,
  `OWGun_<id>_side.png`); `tools/weapon_visual_compare.py` compares a real and a host frame cell by cell.
- The arms play per-type clip sets: convert `Anim_1st_Person.<type>` with `tools/prepare_character_anims.py --clips
  Idle Run_F Sprint Jump_Start Jump_Idle Jump_End Draw ADD_Fire_Recoil` (bones are matched by name, so the rifle set's
  other bone order converts), then import with `OPENWILLOW_CHARACTER_ANIMS=AssaultRifle=...;SMG=...;Shotgun=...`.
- The arms and gun use the game's foreground FOV 45 by default; `-owfpfov=0` restores the old single-FOV view,
  `-owfpfov=<n>` forces one horizontal value for every weapon.
- `python tools/weapon_view_model.py --game <BL2 dir> --dir <items folder...>` writes `weapon_view.json` beside the
  recipes: each recipe's weapon-type `PlayerViewOffset` and `FirstPersonMeshFOV`. The walker places the arms and sets the
  foreground FOV from it when a gun is selected; rerun it when a recipe folder changes (a recipe without an entry gets
  no offset and 45).

- `research/behavior_census.py` and `research/struct_defaults_census.py` are the structural
  oracles behind the behavior variable-data decode and the struct-default reader fix.
- `ow-package --properties-batch` reads many objects from one package in one process; the weapon
  and loot tools use it.
- `tools/weapon_card_audit.py` is the only tool here with a real-game reference: it replays cards
  captured by the UI trace. It reproduces 4 of 6 captured cards; the other two are open.
- `tools/audio_slice_chain.py` stops at raw Wwise Vorbis `.wem` files. No decoder is approved
  yet; picking one is a maintainer decision (license and provenance entry first).
- `tools/slice_npc_editor.py` imports `unreal` and only runs inside the editor.
- `tools/import_phaselock_fx.ps1` waits for `local/ue_run.lock` like the suite scripts. `host/ue5/import_phaselock_fx.py`
  builds the effect materials from own-words notes on their compiled shaders (PHASELOCK_STOCK_DATA.md, Round 6).
  `tools/run_phaselock_shots.ps1 -Extra '-owbubbleradius=<uu>'` overrides the bubble size's mesh-bounds radius, so the
  small engine-shape dummy can stand in for a real enemy. `-owfxscalar=<Template>:<Emitter>:<Parameter>:<Value>[;...]`
  sets one material scalar on every sprite of that Phaselock emitter as it is created (unknown names do nothing), so an
  effect hypothesis can be tested with one capture and no rebuild; write the values as literals, since a PowerShell
  variable inside `-Extra` from a `-File` call is not expanded.

Records: [behavior data](verification/BEHAVIOR_DATA_DECODE.md),
[weapon balances](verification/WEAPON_BALANCE_DECODE.md),
[world placement](verification/SLICE_WORLD_PLACEMENT.md),
[Phaselock stock data](verification/PHASELOCK_STOCK_DATA.md),
[audio chain](verification/SLICE_AUDIO_CHAIN.md),
[NPC assets](verification/SLICE_NPC_ASSETS.md).
