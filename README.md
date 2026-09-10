# OpenWillow

An AI-assisted Borderlands 2 engine reimplementation experiment. The intended
runtime reads the player's own installed game. No game assets or Gearbox code
are distributed here. **There is no playable engine or renderer yet.**

## Current state (Phase 0, steps 3–7 of 8)

A standalone x64 C++20 tool, `ow-package`, reads version 832/46 packages:

- name/import/export tables, fully and partially LZO-compressed containers;
- object records with package-local outer paths (`--exports`);
- a per-class export count for one package (`--census`), driven over a whole
  installation by `tools/census.py`;
- tagged properties (`--properties`): scalars, object references, nested
  structs, common fixed-layout structs, and arrays whose element type is
  supplied by a schema file;
- one `Texture2D` to PNG (`--texture`, DXT1/DXT5, inline or TFC-streamed mips);
- one `StaticMesh` to OBJ (`--mesh`, LOD 0, 16-bit indices, one UV set kept).

Cross-package object loading, class/default inheritance, map loading, and the
host-engine render are not implemented. Python is used for tests and as an
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

- Four synthetic CTest suites pass (package tables, properties, container,
  assets). They cover truncation, block totals, corrupt LZO streams, partial
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
  784 triangles) and `Prop_Roads.Textures.MetalRoadConcrete_Dif` (1024×1024
  DXT1, streamed from `Textures.tfc`) from `Ash_P.upk`, and confirms through
  the material's `TextureParameterValues` that the texture is that mesh's
  `p_Diffuse`. The PNG has been viewed and is the road texture. The OBJ has
  not yet been opened in Blender or rendered anywhere.

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
means null. Paths describe the current package's outer chain; they do not prove
that an imported object exists in another package.

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
leaving zero trailing bytes. Its values have not yet been compared against
BLCMM, so Phase 0 step 5's external check is still open.

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

Texture extraction picks the largest resident mip and supports only
`PF_DXT1`/`PF_DXT5`. Mesh extraction handles a single LOD with 16-bit indices
and rejects anything else explicitly rather than guessing. Both are spikes for
the Phase 0 gate, not general exporters.

## Host engine probe (not yet run)

`host/ue5/OpenWillow/` is a minimal UE5 C++ project whose module refuses to
start without `OPENWILLOW_BL2` pointing at an installed game.
`tools/run_ue_probe.ps1 -Engine <UE5 root> -Game <BL2 root>` builds it and
launches the editor with `host/ue5/import_probe.py`, which imports the probe
OBJ and PNG, wires the texture into a material, and places the mesh, a camera
and lights in `/Game/Phase0/Phase0`. This has not been executed: UE5 is not
installed on the development machine yet, so the Phase 0 gate (a screenshot
of the mesh and texture rendered in the host engine) remains open and the
host-engine decision is still provisional.

Next: install UE5, run the probe, capture the gate screenshot, then cross-check
the census against umodel and a weapon part against BLCMM.
