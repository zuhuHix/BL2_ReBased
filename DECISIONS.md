# Decisions and evidence

## 2026-09-14: First source collision and placeholder walking slice

User explicitly approved collision-parser changes after the sensitive-area rule
was disclosed. The mesh reader now exposes the body reference it already reads;
the scene reader includes Engine.RB_BodySetup tagged properties. Reader limits,
container checks and property-size checks are unchanged.

Installed Ash_P body exports 11404 and 11405 provided initial box and convex
observations. Tagged Box is two XYZ float vectors and one validity byte (25
bytes); tagged Plane stores W,X,Y,Z, and Matrix has four such rows (64 bytes).
Identity boxes and asymmetric rotated/translated synthetic fixtures distinguish
this from XYZW. The observation is limited to BL2 832/46, not general UE3 parity.

Convex VertexData is retained in local centimeters; source FaceTriData and
ElemBox are checked when present. Box TM and dimensions produce eight corners.
Malformed, nonfinite, degenerate and unsupported geometry is rejected. Sphere,
capsule, cooked PhysX blobs, per-poly flags and general class inheritance remain
unsupported. Empty/absent body geometry receives no invented collision.

UE5 cooks these hulls using its installed FKConvexElem/UBodySetup API. Reference:
https://dev.epicgames.com/documentation/en-us/unreal-engine/API/Runtime/Engine/FKConvexElem
and the locally installed UE5.8 headers. No third-party implementation was copied,
no new dependency was added, and no game data is tracked. Full hulls attach only
to the first visual section to prevent duplicate collision for material sections.
Components with explicitly false BlockActors or CollideActors are disabled;
absent flags currently default to enabled, an inspection approximation without
CDO inheritance or original-game collision-channel parity.

The opt-in -owwalk controller uses UE5 CharacterMovement with a 34 cm radius,
88 cm capsule half-height, 450 cm/s walk speed, 420 cm/s jump velocity, 35 cm
steps and a 45 degree floor angle. These are placeholder values. Default free
flight remains available. See COLLISION_WALKING_VERIFICATION.md for test evidence
and the deliberately bounded Sanctuary acceptance claim.

## 2026-09-10: Phase 1 importer foundation

The Phase 0 command-line spikes are now split behind reusable C++ APIs. The
package reader retains decoded package bytes and export/import records;
`PackageStore` lazily indexes `.upk`, `.umap` and `.u` files, caches loaded
packages, and resolves negative imports by package-local outer chain. When a
cooked import's root name is not a disk filename, it falls back to the global
object path across the indexed tree and caches the successful match. This
behavior is based on the installed Ash package: `Common_Materials.Environment.Master_World`
resolved to the `WillowGame` package. Synthetic coverage includes direct and
local-export-nested imports.

The texture importer now returns every available resident mip, including
inline, TFC-streamed and compressed bulk, while retaining the explicit
PF_DXT1/PF_DXT5 and payload-at-end limits. `--mip` selects an output mip and
`--all-mips` writes the available set. The static-mesh importer now retains
all render LODs, all UV sets and either 16- or 32-bit indices; OBJ remains a
diagnostic output for one selected LOD and its first UV set. Source mesh data,
collision hulls, skeletal meshes and material translation are still future
work.

The refactor builds with CMake, all five synthetic CTest suites pass, the nine
code-package differential check remains byte-for-byte identical, and the real
Ash probe now reports 11 resident texture mips and 2 mesh UV sets. These are
still importer/parser checks, not proof of a runtime host-engine asset path.

## 2026-09-10: Phase 0 host gate and independent property comparison

The UE5.8.2 host probe rendered the extracted `Env_Ash.Mesh.Ash_Road01` with
its verified `p_Diffuse` texture in `/Game/Phase0/Phase0`. The first visual
gate screenshot is user-verified. The probe remains a diagnostic import spike:
its material is two-sided and unlit/emissive so asset visibility is independent
of lighting-bake and winding issues.

The installed-package census completed with 2,010 candidate files, including
two identified UHD texture sidecars, and read all 2,008 UE packages. It found
4,751,329 serialized exports. These are serialized copies rather than unique
assets. A UE Viewer/umodel plausibility comparison is still pending because
the reference executable is not installed locally.

The BLCMM Object Explorer dump for
`WeaponPartDefinition GD_Gladiolus_Weapons.AssaultRifle.AR_Barrel_Jakobs_Sawbar`
matches the C++ reader on the decoded property names and values, including the
three attribute effects, `WP_Barrel`, shell-casing settings, gestalt mesh name,
slot upgrades and monetary-value reference. The reader consumes 1,551 bytes
with zero trailing bytes in all three installed copies: `Gladiolus_Startup_SF`
export 1009, `Lobelia_Startup_SF` export 1020, and
`TestingZone_Combat` export 42374.

One discrepancy remains explicitly unresolved: BLCMM prints
`BehaviorProviderDefinition=None`, while the serialized stream contains a
reference to `...AR_Barrel_Jakobs_Sawbar.BehaviorProviderDefinition_13`.
This may reflect BLCMM's reference-dump/default presentation of a generated
subobject, but it is not treated as a confirmed match until that representation
is checked independently.

## 2026-09-10: Start with an engine-independent package reader

C++20 and CMake provide a small working build before a host-engine installation.
UE5 versus Godot remains undecided. The current executable is a research tool,
not the future runtime, and accepts decompressed packages for local verification.

The table layout comes from this workspace's research/native_count.py. The new
C++ reader implements bounds validation independently and does not include its
decompressor. That Python function explicitly describes a minilzo port: its
provenance and applicable license must be established before distribution or
translation into the engine. Project license remains pending.

The initial differential check matches all nine installed code-package table
counts. This does not validate the native/script classification heuristic,
bytecode execution, all content packages, or the plan's effort estimates.

Phase 0 remains incomplete: compressed loading, property parsing, full census,
texture/mesh extraction and host rendering are outstanding. The Sanctuary
weekly gate becomes applicable once a map loader exists.

## 2026-09-10: Direct compressed code-package loading verified

Supersedes the compressed-loading status above. The C++ reader now validates
fully compressed container headers and block sizes, decodes through upstream
`lzo1x_decompress_safe`, and verifies each output length. The dependency is
optional and disabled by default. See THIRD_PARTY.md for the pinned archive,
license, and remaining Python provenance uncertainty. This is not a permanent
host-engine dependency decision or a project-wide license selection.

The nine-package comparison now passes original compressed paths to C++ and
compares the complete decoded buffer to Python's output before reading tables.
All nine match byte-for-byte. No decoded assets are saved in the repository.
The comparison is agreement between two execution paths related to miniLZO,
not an independent proof of compression-format correctness.

Container decoding rejects trailing bytes, inconsistent totals, invalid block
sizes, malformed streams, and decoded lengths over 512 MiB. Partial package
compression and all-package coverage remain unimplemented. Next work is object
records and tagged properties; host rendering and Phase 0's gate remain open.

## 2026-09-10: Object records and first tagged-property reader

The reader retains name/import/export records and exposes `--exports` JSON.
Paths resolve through local outer references, with cycle/depth checks. Import
loading across packages remains future work. Names preserve numbered FNames
and convert serialized Latin-1 or UTF-16 strings to UTF-8 JSON.

`--properties <export-index> --property-offset <bytes>` starts at an explicit
offset relative to an export. A bounded reader prevents property data escaping
that export or a tag's declared size. Scalar values and object references are
decoded. Struct/array/unknown payloads are explicitly unsupported. The offset
is required because prefix serialization differs by object type; no universal
four-byte-prefix assumption is embedded in the parser.

Evidence: the installed WillowGame Default__WeaponPartDefinition (export 36066)
has a stream at offset 4 with 11 tags and an exact end at payload byte 860.
The four-byte prefix's semantics are UNVERIFIED. This is a class default, not
a weapon-part instance. BLCMM/in-game comparison is still required, so Phase 0
step 5 is only partially complete. No game payload fixtures were committed.

Build and all three synthetic CTest suites pass. Differential verification
checks all nine code packages' decoded bytes and export fields. These tests do
not establish struct/array correctness, inheritance/default application, or
arbitrary-object property-start discovery.

## 2026-09-10: Replace miniLZO with vendored lzokay; support partial compression

Supersedes the miniLZO status above. The GPL miniLZO FetchContent path and the
`OPENWILLOW_RESEARCH_LZO` option are removed. The reader now links the MIT
lzokay decoder from `third_party/lzokay`, verified byte-identical to upstream
master `db2df1fc`, and the option is `OPENWILLOW_LZO` (default ON). Reason:
the nine-package differential check produces identical decoded bytes with
either decoder, and removing the GPL dependency removes a licensing blocker
from every enabled build. This does not select a project license and is not a
commitment to link lzokay into a future host engine.

Partially compressed packages (summary flag `0x02000000`, codec 2) are now
decoded: the chunk table is validated for monotonic, contiguous, in-bounds
ranges, each chunk is a full container decoded through the same path, and the
result is assembled at logical offsets. Synthetic tests exercise a one-chunk
package and mutations of every table field. This was required because 1,973
of the 2,008 installed packages use partial compression; only 11 are fully
compressed containers and 24 are stored uncompressed (header survey, 2026-09-10).

## 2026-09-10: Census, struct/array properties, and Phase 0 asset spikes

`--census` counts exports per class path in one package; `tools/census.py`
drives it over every `.upk`/`.umap`/`.u` under the install and refuses to
report success unless every package read. Result: 2,008 of 2,008 packages,
4,751,329 serialized exports (see README). Two UHD files with a `.upk` suffix
are byte-signature-checked and reported as sidecars rather than failures. The
count semantics are serialized copies across packages, not unique assets; the
umodel plausibility check from the plan has not been done.

Struct properties decode either as fixed layouts (`Vector`, `Rotator`, `Guid`,
`LinearColor`, `Color`, `Quat`, `Vector2D`) or as nested tagged streams, with
a depth cap of 32. Array element types are not serialized in 832/46 tags, so
they come from an explicit `--array-schema` file per probe. This is a
deliberate stopgap: the correct source is the class's UProperty reflection
data, which needs cross-package class loading (Phase 1/2). A missing schema
entry leaves the array `unsupported` rather than guessing.

`--texture` and `--mesh` are single-object spikes written against the
serialization order documented in UE Viewer (MIT; read, not copied). Texture:
largest resident mip, DXT1/DXT5 only, inline or TFC-streamed, optional
LZO-compressed bulk. Mesh: first LOD, 16-bit indices, first UV set, section
material references. Anything outside that fails loudly. `tools/prepare_probe.py`
extracts `Ash_Road01` plus its material's actual `p_Diffuse` texture from
`Ash_P.upk` and records hashes in `local/probe/probe.json`. The PNG has been
viewed and is the expected road texture; the OBJ has not been opened in Blender.

Evidence limits: `AR_Barrel_Jakobs_Sawbar` decodes fully (13 tags, zero
trailing bytes) but its values are not yet compared against BLCMM, so step 5
remains externally unverified. The offset-4 prefix observation now spans class
defaults, material instances, weapon parts, textures and meshes but is still
UNVERIFIED as a rule.

## 2026-09-10: UE5 host probe scaffolded; engine decision deferred to the gate

`host/ue5/OpenWillow` is a minimal UE5 C++ project plus an editor Python
import script, following the plan's UE5 recommendation. It has not been built
or run because UE5 is not installed on the development machine. The Phase 0
gate (mesh and texture rendered in the host engine, screenshot captured) and
the UE5-versus-Godot decision therefore remain open. The scaffold is committed
so the gate can be attempted as a self-contained next task; it must not be
read as evidence that the engine choice is made.
## 2026-09-10: Material v1 and first frozen map import

The first target is `Ash_P`. `tools/prepare_level.py` follows the serialized
`LevelStreaming*` PackageName values (including `Ash_Px`) and prepares one
manifest for the UE5 editor host. Sublevel provenance is retained on each
placement and displayed as editor folders. All referenced sublevels are loaded
at once; this is not distance/mission-controlled runtime streaming.

Material v1 uses named diffuse, normal, specular and emissive texture parameters,
material-instance parent chains, and named texture-expression defaults. It is
opaque/default-lit: diffuse -> base color, normal -> tangent normal, specular
red -> scalar specular, emissive -> emissive. Roughness is fixed at 0.65. This is
an explicit approximation of UE3 specular, not graph or lighting equivalence.
Diffuse/emissive are sRGB; normal/specular are linear, with UE normal compression
and normal sampler for the normal channel. Unsupported graphs use neutral gray
and appear in scene.json issues. Multi-section meshes are imported per section
so OBJ group merging cannot reorder component material overrides.

Observed from the installed Ash packages with the existing bounded property
reader: ordinary StaticMeshActor/InterpActor/PlayerStart property offset 26,
StaticMeshComponent offset 8, StaticMeshCollectionActor offset 4. No offset
search is used by the loader. Native prefix semantics remain UNVERIFIED.
Collection tails are exactly 84 bytes per serialized component reference:
16 floats forming an affine matrix, three scale floats, one scale float, and
one integer whose meaning is not assigned. The matrix is rotation/translation;
scale is separate (the first inspected component's Scale3D agrees at 0.5).
Exact lengths, finite numbers, affine last column, and duplicate references are
checked. Collection transforms are not multiplied by component Scale3D again.
This is an observed Ash layout, not a claim of general UE3 serialization parity.

The implementation is original code using existing repository APIs and observed
local game data. No reference implementation was copied; no dependency or
license decision changed. No game-derived files are tracked. Native actors,
physics, script, skeletal meshes, terrain/BSP, transparency, vertex-paint layers,
lightmaps and mission state are outside this slice. `_Dynamic` static meshes and
InterpActors are frozen; unsupported interactive-object owners are reported.

Host validation identified two conversion hazards: Interchange OBJ import
reflects Y even with convert_scene disabled, and positional Python Rotator
arguments do not follow the manifest's pitch/yaw/roll order. The host adapter
reflects OBJ Y, normals and winding together before import; rotations use named
arguments. A synthetic asymmetric triangle and a 90-degree parent transform
verify source bounds and the independent expected world position after saving
and reopening. Stable labels include a hash of the full source path because
collection components can share the same leaf name.

Emissive v1 is RGB multiplied by alpha. The observed base-material default is
white with zero alpha, so direct RGB wiring would incorrectly make ordinary
surfaces emit light. This mask convention remains an approximation for complex
graphs. All 62 p_Specular texture overrides inspected in Ash_P are null; the
four-channel synthetic host fixture exercises the specular path instead.

The first lighting pass adds four saved actors under the `Lighting` folder:
`OpenWillow_Sun` is a movable warm directional light at intensity 1.0 with
four dynamic shadow cascades; `OpenWillow_SkyFill` is a movable cool skylight
at intensity 0.5 using UE's neutral gray light cubemap with a blue
lower-hemisphere fill;
`OpenWillow_ReflectionCapture` is a runtime sphere capture centered on the
player start and clamped to a 16,384-unit influence radius; and
`OpenWillow_Exposure` is an unbound post-process volume with automatic exposure
and ambient-occlusion settings. The rig is anchored at the start camera so a
large UE3 sky/environment bound cannot move the lighting origin out of the
playable scene. This is a stable inspection rig, not UE3 lightmap,
native-light, or reflection-capture parity. The full Ash import saved and
reopened with all four actors and no verification errors. A fresh UE5.8 editor
Lit frame and a separate game-window frame on 2026-09-11 confirmed textured
geometry, directional shading, and the saved inspection camera. Wider-map
brightness, shadow softness, reflection quality, and free-flight coverage
remain user checks.

## 2026-09-12: Second map and runtime viewer regression

Sanctuary_P and nine referenced sublevels were prepared using the same bounded
reader and collection layout: 4,430 placements, 423 meshes, 391 materials and
4,768 imported section actors. The saved scene reopened with zero verification
errors. Four placements still report unsupported color streams; no decoder bounds
checks were relaxed. Terrain, gameplay, streaming and native collision remain open.

The viewer's saved CameraActor is a starting-pose marker. RestartPlayer now puts
the possessed spectator pawn at that pose, applies the recorded FOV, and keeps it
as the view target. Collision is disabled for the free-flight viewer: standalone
automation reproduced zero movement with collision enabled and passed with it
disabled. Walking collision is a separate Phase 1 gate. The same pawn/camera
regression passed in Ash and Sanctuary; physical keyboard/mouse input was not
verified because the Windows Computer Use helper was unavailable.

Sanctuary exposed path collisions between a Material and Texture2D export. Material
refresh filters the export class and accepts both qualified and package-local
class names. Existing scene placement data is kept unchanged. The refresh replaces
the manifest only after all material identities resolve; game-derived output stays
under ignored local directories.

The cooked Mat_SancBuild_Colorized retains an autogenerated texture parameter
pointing to SancBuild1a_Dif_04 while some graph connections are null. Material v1
can infer diffuse only from a unique unnamed sample with an explicit _Dif suffix,
including numbered variants, and only in the absence of any named diffuse channel.
Explicit null overrides, masks, normal maps and ambiguous candidates are protected
by synthetic tests. Fourteen Sanctuary materials use this recorded approximation;
115 still use neutral fallback. This does not restore graph masks or tint.

UE's automation startup waits for 10 FPS by default. Sanctuary remained around
8-9 FPS on this machine during that wait, so the viewer harness overrides only
its own process's readiness threshold to 1 FPS. Its pass means control/camera
correctness, not acceptable frame rate or visual fidelity.

## 2026-09-13: cooked Material resource texture references

User explicitly approved investigating and changing cooked-material serialization.
`prepare_level.py` now reads the native Material resource prefix after the tagged
property terminator. Supported scope is version 832/46 as enforced by ow-package,
with empty compile-error and dependency arrays. Nonempty arrays are rejected;
texture counts are bounded by remaining payload bytes and resolved references
must be Texture2D or TextureCube. Other resource/shader bytes remain opaque.
No package Reader or container bounds checks were changed.

Format provenance: UE Viewer `UMaterial3::Serialize` in
https://github.com/gildor2/UEViewer/blob/master/Unreal/UnrealMaterial/UnTexture3.cpp
(read as a format reference, no code copied or translated). Upstream LICENSE.txt
was checked: MIT, Konstantin Nosov. Independent Python implementation uses the
observed prefix and existing bounded CLI export payloads; no new dependency.

Installed Sanctuary_P export 2000 (Mat_SancBuild1e) has 12 expression slots,
only two surviving references, and a 128-byte native tail. Its resource list
contains SancBuild1e_Comp, Sanctuary_Cube and SancBuild1e_Dif. Export 1882
(Master_Black) has a 68-byte tail and an empty texture list. Null expression
links must not be interpreted as a black constant or reconstructed graph.

When no named diffuse parameter exists (explicit null included), a sole
Texture2D whose path ends in _Dif or _Dif_<digits> may supply diffuse. Multiple
candidates remain unresolved. Scene metadata records source resource, texture
identities, opaque byte count and the inference method. Channel selection is
an approximation: graph connectivity, tint, masks and UV mapping are UNVERIFIED.
Native texture membership is not proof of shader-channel semantics.

Synthetic checks cover truncation, boundary mismatch, negative/oversized counts,
unsupported prefix arrays, ambiguity, explicit null and inference provenance.

## 2026-09-13: Project license MIT; Python decompressor provenance resolved

The project-wide license is MIT (`LICENSE`), chosen by the maintainer. GPL-3
was the alternative and would have absorbed the miniLZO-derived research
decompressor without changes, but the host engine is Unreal Engine 5 and GPL
code cannot be distributed as a binary linked against it under Epic's EULA;
a GPL OpenWillow could never ship a runnable build. MIT is compatible with
UE5 and with the vendored lzokay, is the license used by comparable
reimplementations that sit on a proprietary host (Ship of Harkinian,
OpenGothic), and needs no contributor license agreement: contributions are
accepted under the same MIT terms (inbound = outbound), as stated in
CONTRIBUTING.md and docs/LEGAL.md.

The license does not change the project's exposure to the rights holder;
that is governed by the clean-room rules, the no-redistribution rule and the
original-game requirement, which are unchanged.

To make MIT honest, the miniLZO-derived `lzo1x_decompress` in
`research/native_count.py` was replaced with an independent implementation
(see THIRD_PARTY.md). Verification: `tools/verify_packages.py` reports
decoded bytes, counts and export fields matching the C++ reader on all nine
code packages; `research/native_count.py` reproduces 20,119 / 7,141 / 12,978
/ 2,453. The oracle is now less independent of lzokay than the miniLZO port
was, but the miniLZO-versus-lzokay byte-for-byte agreement was already
recorded on 2026-09-10 and stands as the cross-lineage check.

## 2026-09-14: Base-game selector and near-vertical host rotations

Added a command-line selector over the preparer's base-game package scope.
Installed package names are discoverable without pretending every map loads;
ambiguous names and mismatched saved manifests are rejected. DLC support and
an in-game selection menu remain separate work.

Southpaw Factory exposed a host-only rotation loss: assigning a transform
through UE5's actor API snapped a near-vertical collection pitch to 90 degrees.
The matrix-derived Euler rotation is now assigned to the unattached root
component after the actor transform. A synthetic near-vertical placement with
negative scale passes fresh-process saved-scene verification with the existing
axis tolerances. No serialization layout or bounds checks changed.

Verification and current limitations are recorded in
[map selector verification](docs/verification/MAP_SELECTOR_VERIFICATION.md).

## 2026-09-14: Preserve game winding through the OBJ host adapter

The mirrored Scooter sign was traced to an extra triangle reversal in
`host/ue5/scene_geometry.py`. The extracted sign's 656 face cross products all
oppose its stored outward vertex normals; the former synthetic fixture used
the opposite convention. Reflecting Y positions/normals while retaining game
index order yields the correct OBJ handedness and UE face visibility. The
isolated sign now renders readable with unchanged texture coordinates.

Synthetic fixtures now use the game's winding convention. Saved UV validation
also compares oriented triangle topology; the pre-fix Southpaw scene fails this
new check, while the corrected isolated sign passes. See
[UV/winding verification](docs/verification/UV_WINDING_VERIFICATION.md).

## 2026-09-14: Optional DLC content scope and shared-resource lookup

`--include-dlc` opts preparation and map discovery into installed DLC packages;
base-only behavior remains available. The local install exposes 82 persistent
map names. Named texture caches use a source-package-local file when duplicated,
otherwise require uniqueness. The reader still decides whether an absent cache
is needed by streamed mips; inline mip decoding is not rejected preemptively.
No texture serialization layout changed.

Numeric import references now use the existing CLI import table to check loaded
objects by path and class before an expensive global search. In DLC mode,
unresolved references try the base cooked root before the whole install; only
an absent-target error broadens that search. Validation/ambiguity errors remain
fatal. The current-install reference to `Common_Textures.Stub.StubGray_Gray`
resolves from `WillowGame`, demonstrating why the base-first order matters.
Synthetic tests cover loaded-path class matching, cache reuse, absent-target
fallback and propagation of validation errors. DLC map rendering is pending.

## 2026-09-14: Two more diffuse inference rules and a low-end render switch

Grouping Sanctuary's 64 neutral materials showed that only 31 opaque ones
(157 of 4,768 placed sections) actually rendered as gray; the translucent rest
were already invisible through the zero-opacity path. Two policy rules now
resolve most of the visible ones without reading any new cooked-resource bytes:

- **Sole non-auxiliary texture.** When the cooked texture list has no unique
  `*_Dif`/`*_Diff` Texture2D but exactly one Texture2D whose name does not end
  in a normal/composite/specular/emissive/mask/gray/noise suffix, that texture
  is used as diffuse. Applies to opaque and masked materials only; a translucent
  material's diffuse alpha becomes its opacity, and a guessed opacity is worse
  than the invisible fallback. Recorded as `sole_cooked_resource_texture`.
- **Unconnected DiffuseColor input.** An opaque Material with zero cooked
  textures and a `DiffuseColor` input carrying no `Mask*` flags is rendered as
  the input's `Constant` (default black). Cooked graphs strip the expression
  reference in every case; the mask flags are the observed distinction between
  a stripped connection (`Mask=1`, e.g. `Hanging_Monitor_Arm_Mat`) and an input
  that never had one (`Master_Black`). Observed on one material; not a format
  guarantee. Recorded as `constant_diffuse` on the material.

Sanctuary result after `refresh_materials.py --reuse-textures`: no-supported-channel
materials 64 -> 46, opaque ones 31 -> 13 (157 -> 54 placed sections);
`Master_Black` (53 sections) becomes black. The remaining opaque set is mostly
multi-layer snow/glacier/skybox materials with several `_Dif` candidates, which
this project does not resolve by picking one. The `Numerals` stencil and the sky
transition texture are non-DXT and stay blocked on the texture importer.

Counting clarification from the fresh 2026-09-14
[Sanctuary audit](docs/verification/SANCTUARY_MATERIAL_BASELINE.md): 46 is the
number with no supported channels, not all missing diffuse. Including
normal-only icicles and emissive-only spire instances gives 49 missing diffuse,
16 opaque definitions and 76 placed opaque sections. No material policy changed.

Automated checks: `ctest` 5/5, `verify_packages.py` all match, `level_test.py`
14/14 with new synthetic cases for both rules. Not done: a UE5 import and
viewer run with the refreshed manifest; the in-game appearance of the newly
inferred textures is unverified.

`tools/run_ue_level.ps1 -LowEnd` starts UE with DX11/SM5, lowest scalability
groups, FXAA and a reduced window for machines without a discrete GPU. Runtime
only; imported content and saved scenes are unaffected. No frame rate recorded.
