# OpenWillow

An AI-assisted Borderlands 2 engine reimplementation experiment. The intended
runtime reads the player's own installed game. No game assets or Gearbox code
are distributed here. **The UE5 editor host can inspect imported assets and a
frozen map; there is no playable BL2 reimplementation yet.**

## Current state (Phase 1, importer foundation)

A standalone x64 C++20 tool, `ow-package`, reads version 832/46 packages:

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
  (`--mesh`, 16/32-bit indices, all UV sets retained by the importer).

General class/default inheritance and runtime package streaming are not
implemented. An offline first-map loader and Material v1 feed the UE5 editor
host (see below). Python is used for scene preparation, tests and as an
independent execution path for comparison; the executable itself does not
require Python. The LZO decoder is the vendored MIT-licensed lzokay; see
[THIRD_PARTY.md](THIRD_PARTY.md).

Requirements: CMake, Visual Studio 2022 C++ build tools, Python 3.

```powershell
cmake -S . -B build -G "Visual Studio 17 2022" -A x64
cmake --build build --config Release
ctest --test-dir build -C Release --output-on-failure
python tools/verify_packages.py --reader build/Release/ow-package.exe
& ./build/Release/ow-package.exe "C:/Program Files (x86)/Steam/steamapps/common/Borderlands 2/WillowGame/CookedPCConsole/Core.upk"
```

For a decoder-free build add `-DOPENWILLOW_LZO=OFF`; that build accepts only
decompressed packages and skips the container, asset and compressed tests.
For a different install, add `--cooked "D:/path/Borderlands 2/WillowGame/CookedPCConsole"`
to the verification command. It only reads installed packages; temporary
decompressed files are removed when the check exits.

## Project rules

- Never distribute game files, asset dumps, or proprietary code. Fixtures must be synthetic.
- Do not use leaked source or transcribe decompiled executable code.
- Use observed behavior and documented formats; record reference provenance.
- Check licenses before incorporating reference implementations.
- The intended runtime requires the original installed game.
- No paid builds or premium features; any donations support engine development.
- Disclose AI assistance and keep verification evidence honest.

License selection is pending a provenance review; no project-wide open-source
license is granted yet. See [DECISIONS.md](DECISIONS.md) and
[OPENWILLOW_ENGINE_PLAN.md](OPENWILLOW_ENGINE_PLAN.md).

## Verification status

Local checks on 2026-09-10, Release build, all against the installed game:

- Five synthetic CTest suites pass (package tables, properties, runtime,
  container, assets). They cover truncation, block totals, corrupt LZO streams, partial
  compression tables, malformed tags, nesting limits, DXT pixel decoding, PNG
  CRC/zlib framing, TFC bounds, and mesh buffer bounds.
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
  `Ash_P.upk`, and confirms through
  the material's `TextureParameterValues` that the texture is that mesh's
  `p_Diffuse`.
- The UE5.8.2 host probe rendered this mesh and texture in
  `/Game/Phase0/Phase0`; the first Phase 0 screenshot is user-verified.

These checks establish agreement with the research reader and a visually
plausible texture, not independent proof of every format field or gameplay
compatibility.

## Inspect object records and properties

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
recorded in `DECISIONS.md`.

## Census and asset extraction

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
`PF_DXT1`/`PF_DXT5`; payload-at-end mips and other pixel formats still fail
explicitly. Mesh extraction reads every render LOD, 16- or 32-bit indices and
all UV sets; OBJ output intentionally writes one selected LOD and its first UV
set. Source mesh data and skeletal meshes remain future work. Tagged convex/box
collision is available in the first walking slice described below.

## Host engine probe (Phase 0 complete)

`host/ue5/OpenWillow/` is a minimal UE5 C++ project whose module refuses to
start without `OPENWILLOW_BL2` pointing at an installed game.
`tools/run_ue_probe.ps1 -Engine <UE5 root> -Game <BL2 root>` builds it and
launches the editor with `host/ue5/import_probe.py`, which imports the probe
OBJ and PNG, wires the texture into a material, and places the mesh, a camera
and lights in `/Game/Phase0/Phase0`. This editor import remains a diagnostic
spike, not the eventual runtime loader.

The next slice below adds Material v1 and one persistent map with its sublevels
using the package and asset APIs.

## Material v1 and first map loader

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
masked by alpha, and constant roughness 0.65. This approximates UE3 shading. Unsupported assets, graphs and
interactive component owners are listed in `scene.json` under `issues`.
Terrain/BSP, skeletal meshes, lightmaps and full material graphs are not loaded.

The imported map uses a reproducible inspection lighting rig: a warm movable
directional sun at intensity 1.0, a cool movable skylight at intensity 0.5
using UE's neutral gray light cubemap, a runtime sphere reflection capture centered at the start camera, automatic
exposure, and a 0.35 ambient-occlusion post-process override. The rig is
anchored at the start camera because UE3 sky/environment placements can carry
intentionally huge scales.
UE3 lightmaps and native light actors are not translated yet, so this rig is for
geometry and material checks; its visual match to Borderlands 2 remains open.

`--scene-records tools/level-arrays.schema` is bulk CLI metadata for this slice;
`--payload <positive export index>` exposes bounded bytes for observed collection
tails. The Ash-specific prefixes and native collection layout remain subject to
independent visual/in-game validation; see DECISIONS.md.

`-ImportOnly` also reopens and verifies the saved scene: placement counts,
collection transforms, material overrides, imported section bounds against
source OBJ vertices, and material graph/color-space settings. To exercise all
four channels independently of game data, run `python tests/prepare_ue_smoke.py`
then pass `-Scene local/material-smoke -ImportOnly` to the launcher. `-SkipBuild`
uses an already compiled host. All fixture data is synthetic.

The first Ash import contains 5,059 placements / 5,235 mesh sections and passes
saved-scene verification. A fresh UE5.8 Lit editor frame and a separate game
window now confirm the textured start-camera view; wider-map visual coverage
and camera gestures remain open. See [verification evidence and remaining limits](MATERIAL_LEVEL_V1_VERIFICATION.md).

### Continue Phase 1: Sanctuary and the free-flight viewer

The viewer now uses the possessed spectator pawn as its camera after restart.
Its collision is disabled for free flight; this does not implement walking or
UE3 collision parity. Open an already imported scene without another import:

```powershell
./tools/run_ue_level.ps1 -Engine 'C:/Program Files/Epic Games/UE_5.8' -Game $game -Scene local/sanctuary -ViewOnly -SkipBuild
```

Preparation defaults to a separate `local/<map-name>` directory, so preparing
`Sanctuary_P` no longer writes the Ash manifest by default. To iterate on material
translation without extracting meshes again:

```powershell
python tools/refresh_materials.py --reader build/Release/ow-package.exe --game $game --scene local/sanctuary --reuse-textures
./tools/run_ue_level.ps1 -Engine 'C:/Program Files/Epic Games/UE_5.8' -Game $game -Scene local/sanctuary -ImportOnly -SkipBuild
./tools/test_ue_viewer.ps1 -Engine 'C:/Program Files/Epic Games/UE_5.8' -Game $game -Scene local/sanctuary
```

Use `--reuse-textures` only with the same unchanged game installation. The refresh
preserves placement data and resolves materials by both path and class. A single
unnamed sample with an explicit `_Dif` texture name can supply approximate diffuse
color when there is no named diffuse parameter; this is recorded as an inference,
not reconstruction of stripped graphs or material tint.

The runtime test checks pawn movement and camera following and captures three
settled views in `Saved/Screenshots/WindowsEditor`. It supplies engine movement
input, not physical keyboard/mouse gestures. The test process uses a 1 FPS startup
threshold to allow correctness inspection on slow maps; this is not a performance
pass. See [Phase 1 viewer verification](PHASE1_VIEWER_VERIFICATION.md).

### First collision and walking slice

Prepared scenes now include observed RB_BodySetup convex and box hulls. The
collision refresh/import and walking regression have been verified on Sanctuary.
For a scene already imported with collision, launch the placeholder character:

```powershell
./tools/run_ue_level.ps1 -Engine 'C:/Program Files/Epic Games/UE_5.8' -Game $game -Scene local/sanctuary -ViewOnly -SkipBuild -Walk
```

Use WASD and mouse, with Space to jump. Omit `-Walk` for the free-flight viewer.
This uses UE5 movement defaults tuned for inspection; it does not reproduce BL2
movement physics. See [collision preparation, checks and limits](COLLISION_WALKING_VERIFICATION.md).
