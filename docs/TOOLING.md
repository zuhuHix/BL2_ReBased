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
| `pyunrealsdk` and community data tools | Future runtime observation and behavioral golden data | Not an asset-extraction replacement; use only with clean-room and license review |
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

`-ImportOnly` also reopens and verifies the saved scene: placement counts,
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
94 `Common_Meshes.CollisionCube` placements, and six cloud `Blocking_Plane`
placements (four `Mat_CloudLayer_Light`, two `Mat_CloudLayer_01`). The exact `Sanctuary_P` `InterpActor_34` `Prop_Garbage.Meshes.BoxLrg`
placement that blocked the start view is also hidden. This is a bounded visual
artifact policy, not complete collision or material parity.

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
material approximation, and (with `--collision`) triangle-mesh collision. It
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
