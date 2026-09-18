# Decisions and evidence

## 2026-09-14: Sky census locates the native dome without a new policy

The user asked for the native skybox to be located using only already-decoded
data before any material-parameter parsing. AI-assisted `tools/sky_census.py`
reuses `Scene` from `prepare_level.py` (level traversal factored into
`Scene.levels`), the existing `--scene-records`, `--properties` (Texture2D at
the established offset 4), `--payload`, `--mesh` and `--texture` modes, and the
existing cooked-resource reader. No serialization offset, bounds check or
material policy changed; the census only restates what the Material v1 diffuse
rule would select and why.

Findings on the installed game, manifest evidence only:

- Neither `Sanctuary_P` nor `Ash_P` streams a `_Skybox` package. The dome is
  `Prop_Skybox.Meshes.Sky_Dome` (265 vertices, 480 triangles, two UV sets)
  placed by a `StaticMeshCollectionActor` (Sanctuary: `_Light` sublevel,
  scale 5000x5000x6000; Ash: persistent level, scale 1000). Its material instance (`Mati_Sky_Dynamic_INST`
  in Sanctuary, `Mati_AshSkyTempSunset` in Ash) inherits from the unlit
  `Common_Materials.Sky.Mat_SkyTimeOfDay_Master`, whose named samplers are
  `Transition_Track` (`Sky_TransitionBL2Default_Dif`, PF_A8R8G8B8 256x256),
  `clouds` (`Clouds_01`) and `Masks` (`Sky_Multi`/`Sky_Multi2`), with scalars
  such as `Time_of_Day` and `sky_brightness`. The existing policy already
  selects the transition texture as diffuse; the decoder gap closed today was
  the blocker, not the placement.
- Sanctuary's second sky layer, `Prop_Skybox.Meshes.SanctuarySky` in
  `Sanctuary_Outer`, is an `InterpActor` whose three overrides are the
  `*_Teleported` story-state materials with several `_Dif` textures each; the
  policy correctly refuses to pick one. Its mesh defaults resolve to
  `SanctuarySkybox_Diff` (DXT1 2048x2048) uniquely. Which layer is active is a
  Kismet streaming question this project does not interpret.

Not done and not claimed: how the master combines its inputs, the meaning of
`Time_of_Day`, the second `Sky_Dome` placement with a concrete material and
negative Z scale, any host import, and any in-game comparison. See
[verification](docs/verification/SKY_CENSUS.md).

## 2026-09-14: Bounded PF_A8R8G8B8 texture decoding

The user explicitly requested this format addition. AI-assisted implementation
adds little-endian BGRA-to-RGBA conversion after the existing bulk decoding,
without changing Texture2D serialization offsets or bulk flags. Alpha and row
order are preserved; no premultiplication or color-space conversion is applied.
The shared dimension guard retains the DXT limits (16384 per axis, 256 MiB per
mip); exact width * height * 4 bytes are required before channel conversion.
Existing decoded bulk-size and aggregate mip limits remain in force.

Format reference: Microsoft's public
[D3DFORMAT documentation](https://learn.microsoft.com/en-us/windows/win32/direct3d9/d3dformat)
defines A8R8G8B8 channel significance and memory byte order. No reference
implementation code, dependency or game-derived data was added.

All five CTest suites and nine installed code-package comparisons pass.
Ash_P export 21482, Prop_Skybox.Textures.Sky_TransitionBL2Default_Dif,
extracts as 256x256 with one resident mip. Synthetic tests verify exact pixels,
alpha, inline/TFC and LZO paths, small mips and corrupt-input rejection.
Native sky shading and in-game visual parity remain UNVERIFIED.
See [verification](docs/verification/A8R8G8B8_TEXTURE.md).


## 2026-09-14: Glacier primary-layer approximation

The inspected `Mat_Glacier` and `Mati_Glacier2x` now have an explicit, narrowly
scoped Material v1 recipe. It requires the exact four-texture resource set,
Texture2D classes, and a finite retained `P_TexScalar_RGMain_BASnow` vector.
It binds GlacierFront_Dif and GlacierFront_Nrm and applies the vector's RG
tiling to UV0. The base uses (1,1); the inspected instance overrides (3,3).
Explicit diffuse parameters, including null, prevent this fallback. Existing
normal parameters are preserved. Unknown family members and changed/ambiguous
texture sets are not covered.

This is **not reconstruction of the original layered shader**. UV0 and the
primary-layer interpretation are recorded assumptions; native static
permutation data is not decoded. Snow blending, reflection and glow are
omitted. PNG alpha is fully opaque in both diffuse sources, so it does not
provide a snow blend mask. Both affected source meshes have two UV sets;
the current OBJ path still carries only UV0. No native binary-layout parser,
offset, or bounds check changed.

The manifest records `surface_approximation` and per-channel `channel_uv`;
the audit reports partial surfaces separately from missing diffuse. Thirty-three
placed sections receive this partial recipe. Remaining opaque diffuse gaps:
14 definitions / 43 sections; fully untextured opaque gaps: 11 / 21. These
counts do not mean the original glacier appearance is complete.

Host import creates TextureCoordinate nodes, and saved-scene verification
checks their channel and tiling. See the
[glacier verification record](docs/verification/GLACIER_PRIMARY_LAYER.md).

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

## 2026-09-14: diagnostic cooked Material tail structure

The user authorized work on the next high-complexity tasks. Added an
independent diagnostic decoder for the directly observed native tail:
six words, bounded count, 16-byte records and final word, with exact
consumption required. All field semantics remain UNVERIFIED. This does not
change scene material selection or reconstruct stripped graphs. Existing
property, package and container validation is unchanged. Bulk CLI payload
extraction reuses existing export bounds checks and validates every requested
index before output. No dependency or license changes; no external code
copied. See docs/verification/MATERIAL_RESOURCE_CENSUS.md for evidence and limits.

## 2026-09-14: bounded native Sky_Dome import

The user authorized the next Sanctuary visitability slice. Scene preparation
marks only `Prop_Skybox.Meshes.Sky_Dome` placements whose effective material is
Unlit. The observed Sanctuary placement with a floor-material override is not
marked as native sky. UE5 imports the accepted dome as a static visual shell,
with no collision or shadow casting, and records `partial_unverified` graph
status. The temporary UE5 atmosphere remains in the map for unresolved sky
layers. This does not interpret Kismet state, outer sky meshes, time-of-day,
cloud/mask graph connections or visual parity. See
docs/verification/NATIVE_SKYBOX_VERIFICATION.md.

## 2026-09-14: hide observed blocking helpers and render the dome interior

The Sanctuary source contains four `Common_Meshes.Blocking.Blocking_Cube`
placements with no effective material. They are collision helpers, not visible
level geometry; the host keeps their observed collision and hides only their
rendering. The native Sky_Dome faces are outward-oriented while the inspection
camera is inside the shell, so the accepted Unlit sky material is marked
two-sided by the bounded host policy. This does not infer the missing dynamic
sky graph or alter unrelated unassigned slots.

The artifact pass extends the same render-only policy to all five observed
`Common_Meshes.Blocking.Blocking_Cube` placements, all 94
`Common_Meshes.CollisionCube` placements, and the four `Mat_CloudLayer_Light`
`Blocking_Plane` placements. It also hides the exact `Sanctuary_P`
`InterpActor_34.StaticMeshComponent_20` `Prop_Garbage.Meshes.BoxLrg` placement
that blocked the start view. Source collision remains enabled where the
serialized mesh has a recovered collision hull; the policy does not claim
complete collision parity.

## 2026-09-14: visible color for Unlit Material v1

Recovered diffuse or fallback color is also connected to Emissive Color for
Unlit materials when no explicit emissive texture exists. UE Unlit ignores
Base Color for visible shading. Explicit emissive keeps its existing masked
policy and takes precedence independent of channel order. The actual
Sanctuary sky instance uses this fallback path. No native graph or UV mapping
is inferred. See docs/verification/UNLIT_COLOR.md for evidence and limitations.

## 2026-09-15: bounded blue shell for the UE5 inspection sky

The recovered Sanctuary scene still rendered a brown or black upper field when
the temporary atmosphere was the only host fallback. The importer now adds a
centred, two-sided, reverse-culled UE5 sphere with a host-created Unlit blue
constant material, no collision, and no shadow casting. It is labelled
`OpenWillow_SkyFallback`, checked by the saved-scene verifier, and paired with
the existing `OpenWillow_SkyAtmosphere` actor. This keeps the inspection view
readable without claiming recovery of the native sky graph, cloud layers,
time-of-day controls, or lighting parity. The broad lower white regions remain
the separately observed `IcePlate` geometry and were not reclassified as sky.
See docs/verification/SKY_FALLBACK_VERIFICATION.md.

## 2026-09-15: distinguish lower ice geometry from visual helpers

The Sanctuary artifact pass retains all four IcePlate placements and their
recovered collision. WorldTransition remains a narrowly hidden translucent
helper pair; it does not explain the separate omitted terrain/BSP geometry.
Mat_FrozenLake now uses its inspected FrozenLake resource as an explicit UV0
color approximation instead of the generic Snow_Dif selection. Native snow,
noise, reflection, normal, glow and UV modulation remain unverified.
Mat_IceRoadSanctuary, the single SanctuaryRoad_01 placement at the town gate,
follows the same rule with its inspected BrokenRoad_Dif resource; its cooked
list carries three `_Dif` overlays, so the sole-`_Dif` heuristic had left the
road white. Its p_Normal expression survives with a stripped texture and no
normal exists in the cooked list, so no normal is approximated. Both inspected
color fallbacks share one scoped table; the unplaced Env_Ice Mat_IceRoad is
untouched.

The existing HLS regular-diffuse fallback now requires the inspected direct
parent and concrete atlas, records texture/UV provenance, and retains other
supported channels. Material refresh reapplies the placement-derived native
dome interior policy. No binary layout or bounds checks changed. AI-assisted
source inspection and validation are recorded in
`docs/verification/SANCTUARY_ARTIFACT_PASS.md`.

## 2026-09-15: scoped terrain properties and grayscale weightmaps

The user's continued terrain work authorizes a bounded new reader route.
`--terrain-records` uses the observed Terrain actor prefix (26), component
prefix (8), and resource prefix (4), without offset scanning. Its class scope
is separate from `--scene-records`: TerrainLayerSetup.Materials is a struct
array and must not share the mesh Materials object-reference schema.
Individual unsupported objects retain explicit errors.

PF_G8 decoding now requires exactly width*height bytes and expands each value
to opaque grayscale RGBA. TerrainWeightMapTexture is accepted only for PF_G8;
existing dimensions, mip, TFC, LZO and allocation guards remain unchanged.
Synthetic pixel/bounds tests and the five installed Terrain_10 24x28 weightmaps
pass. This proves grayscale extraction, not layer assignment/blending parity.
No terrain triangle/hole semantics or root BSP render buffers are inferred
from this decoder extension.

## 2026-09-15: bounded terrain component geometry and triangle collision

`tools/terrain_decode.py` now walks the remainder of each TerrainComponent
payload after the decoded bounds tree, in Python, without touching the C++
reader: a `(2, N)` u16 record array with an opaque 14-byte stride, an opaque
72-byte block that must end in `(1, own export index)`, an `(8, V)` array of
`<BBHhh>` vertices, two retained words, and a `(2, M)` u16 triangle strip. Every
count is bounded by the payload and by the terrain grid; vertex X/Y/height must
equal the terrain samples; the strip's decoded cells must equal exactly the
non-hole cells with the flag-bit-1 diagonal, and the bounds-tree leaves must
tile the same cells. Any disagreement rejects the component (no scanning, no
retry). The 14-byte records, the 72-byte block, the two int16 vertex words and
the two retained words are kept as opaque data with hashes; their meaning is
**UNVERIFIED**.

What this corroborates: for all 15 installed Sanctuary components the strip
and the leaf tree independently omit the same flag-bit-0 cells and the strip
parity reproduces flag-bit-1 diagonals, so hole and diagonal bits are used as
topology. What it does not establish: the native face orientation for flipped
cells (emitted geometrically, host-visible from above, **UNVERIFIED**), the
meaning of the retained words, and any renderer behaviour.

The Terrain actor tail is additionally probed for `u32 count == len(Layers)`
followed by `count * (u32 vertex_count, bytes)`; 6/8 terrains match. These
arrays are exposed only as a labeled visual approximation
(`terrain_dominant_alpha_layer_v1`: the layer with the largest mean alpha,
indexed by AlphaMapIndex); they do not reproduce the PF_G8 weightmaps and the
native blend is **UNVERIFIED**. Terrains without them use a neutral constant.

`tools/prepare_terrain.py` emits one static mesh per component and, with
`--collision`, marks it `triangle_mesh` (host complex-as-simple, never mixed
with hulls). The saved scene reopened with zero verification errors and the
new `OpenWillow.TerrainWalking` runtime test stood on all 8 terrains, found no
floor in 8 flagged hole cells and crossed the one walkable seam; this is host
behaviour on the decoded topology, not original-game parity. No `src/`
bounds check or layout changed.

## 2026-09-15: explain terrain probe drift without changing acceptance criteria

Codex added per-frame host hole-probe diagnostics (position, velocity, input,
floor and downward trace) without changing pass criteria or parsing behavior.
The Land Terrain_3 probe begins inside StaticMeshActor_SMC_1281, moves about
9.4 m laterally with zero horizontal velocity on its first frame, then slides
along that mesh and lands on neighbouring TerrainComponent_5, 11.5 m away.
This supports penetration correction followed by sliding; internal solver
steps remain uninstrumented. A passing displaced endpoint must not be treated
as runtime verification of the original hole location. Native strip/tree
topology corroboration is independent of this test limitation.

Local-only BSP record and terrain alpha/weightmap diagnostics did not meet
the evidence threshold for new rendering behavior. Candidate BSP normals
match but point association fails; alpha comparisons find only constant-zero
matches. Keep BSP unimplemented and terrain blending explicitly approximate.
Original-game matched views remain outstanding. Detailed evidence and host
runtime results are in the two Sanctuary verification records.

## 2026-09-15: Sanctuary root BSP polygons as a labeled approximation

This supersedes the "keep BSP unimplemented" conclusion above for the two
persistent-level root Models of Sanctuary only. `tools/bsp_decode.py`
consumes the root `Model` native tail as 28 zero bytes, bulk vector, point
and 64-byte node arrays, a self-reference, 60-byte surface and 24-byte vertex
records, and each root `ModelComponent` as material elements with node
membership lists. A polygon is accepted only when its node plane, its
surface plane and its surface normal vector agree, all of its points lie on
that plane within 0.02 cm, it is convex and consistently ordered, and its
component and element memberships back-reference each other and cover every
node exactly once. Unlike the earlier rejected hypothesis, polygon points
come from the node and vertex arrays; the point-like fields of the 60-byte
records are left opaque. Both Sanctuary Models pass every gate (228
polygons, 571 triangles, 24 components, 105 sections).

`tools/prepare_bsp.py` adds those polygons to a frozen Sanctuary scene with
native material assignments, a 128 cm world-planar UV placeholder and opt-in
host triangle collision. Surface texture-axis fields, element lighting
blocks, the 42,068-byte Model remainder and native `PolyFlags` are retained
as digests and remain `UNVERIFIED`; volume-owned Models are still rejected
structurally; other maps are refused. `OpenWillow.BspWalking` stands on and
walks an unobstructed upward-facing polygon per Model; that is host behaviour
on the recovered geometry, not original-game parity, and no matched view has
been produced. No `src/` bounds check or layout changed.

Inspecting the import showed the outdoor start-area floor as UE's default
`WorldGridMaterial` checkerboard. The cause was not BSP: sections the
preparers leave with `material: None` (two terrains labeled
`neutral_constant`, ten static-mesh sections whose native material never
resolved) were skipped by the importer. `import_level.py` now binds a lit
0.5 gray `M_OpenWillowNeutralFallback` to those sections and
`verify_level.py` asserts it (15 mesh sections, 20 placements on Sanctuary).
This makes the gap visible as a labeled flat gray instead of a misleading
pattern; it does not resolve the terrain alpha decode or the missing
materials. Record: docs/verification/SANCTUARY_BSP_POLYGONS.md.

## 2026-09-18: two independent oracles, and a stale cooked collection scale

Two oracles were run against the existing decode, with nothing copied from
either. umodel (UE Viewer, MIT) is a second reader of the same bytes.
OpenBLCMM's Borderlands 2 dumps are the output of the game's own `obj dump`
console command, so they report what the running engine concluded rather than
what another decoder reads. Both stay on the user's machine; reports go to
ignored `local/`.

`tools/crosscheck_umodel.py` compared umodel's `-list` with our `--exports` on
2006 packages (base plus DLC): 4,750,427 exports, no offset, size or class
disagreement. The 7 name-only differences in 5 packages are umodel-side
normalization — it rewrites names containing a control or non-ASCII byte to
`__name_N__` and trims one trailing space, while our reader keeps the raw
bytes — and are bucketed separately so the exit status reflects byte-range
agreement. `tools/crosscheck_umodel_assets.py` compared a prepared Sanctuary
scene with umodel's glTF/PNG exports: 421 of 462 meshes agree on sections,
triangles, vertex counts, positions and UVs with none disagreeing (41 have no
umodel counterpart), and 275 of 288 textures agree, 271 of them within the
maximum per-channel difference of 1 that DXT decoder rounding produces. The
glTF axis mapping and the UV V flip were recovered by search over the data,
not assumed.

`tools/crosscheck_blcmm_dumps.py` compared the same scene with the game's
dumps. All 15 `TerrainComponent`s agree on section base and size, and mapping
our decoded vertices through the reported `_LocalToWorld` reproduces the
reported `Bounds` to 0.003 cm on origin and to the constant one-unit extent
expansion — corroborating the height convention and cell scale against the
engine. All 24 `ModelComponent`s agree on node count, element count and owning
`Model`; the dumps print empty `Nodes(N)=`/`Elements(N)=` values, so contents,
BSP UVs and `PolyFlags` remain untestable and `UNVERIFIED`. 4209 actor
placements reproduce the engine's `_LocalToWorld` within 1e-3 on rotation and
0.05 cm on translation, which confirms the rotator decode; 35 `InterpActor`
placements are reported as movers rather than disagreements, because a dump
shows where a matinee-driven actor had moved to. 353 material texture picks
match the parameters the game reports and none are contradicted, but 348
channels — concentrated in emissive (197) and normal (149) — have no
corresponding parameter, so the oracle is silent on them; 16 parameter names
are listed as unrecognised rather than guessed at.

That pass found one real defect. Our placement of
`Sanctuary_P … StaticMeshCollectionActor_10.StaticMeshActor_SMC_1802` was
unscaled while the engine reports an X row scaled by 0.97. The component export
carries `Scale3D=(0.97,1,1)` as an ordinary property; the collection actor's
cooked per-entry tail records `(1,1,1)`. Of the 3153 collection children in
`Sanctuary_P`, 2037 declare their own `Scale3D`/`Scale` and the tail agrees with
the property in 2036, so the tail is a cache that is stale in exactly this case.
`prepare_level.py` now prefers the component's own property where it exists and
falls back to the tail otherwise (`collection_scale`). No `src/` layout, bounds
check or terminator check changed in this pass; the only parsing behaviour
change is which of two already-decoded scale sources wins.

Records: docs/verification/UMODEL_CROSSCHECK.md and
docs/verification/BLCMM_DUMP_CROSSCHECK.md.

## 2026-09-18: BSP surface texture axes decoded; texel scale left UNVERIFIED

This narrows the "surface texture-axis fields ... remain UNVERIFIED" line of
the 2026-09-15 BSP entry. `tools/bsp_decode.py` now reads three ints of the
60-byte surface record it already unpacked: `s[2]` as the texture base point
index and `s[4]`, `s[5]` as the texture U/V vector indices, each range-checked
against the pools (an out-of-range value rejects the Model, as every other
reference does). No bounds check, array framing or offset changed; the
identification is of fields that were already inside validated bytes.

Two kinds of evidence, both from the installed game through our own reader,
support the roles. First, in-data invariants on the 119 surfaces the Sanctuary
root Models use: `s[4]` is perpendicular to the surface normal on all 119,
`s[5]` on 115 (the four exceptions are 45-degree slopes carrying the
world-axis default `TU = ±X, TV = -Z`), the two axes are mutually
perpendicular throughout, and no other int slot behaves like an index. Second,
volume-owned Models keep an editor `Polys` export whose FPoly records store
`Base`/`TextureU`/`TextureV` as explicit vectors; `tools/crosscheck_bsp_polys.py`
matches each surface to the FPoly on its plane and finds 15,393 agree, 0
differ across 2392 Models in 161 packages, with negative controls showing no
other slot reproduces those vectors. 892 surfaces have no unique coplanar
FPoly (stale editor vertex lists, stale or reversed normals, duplicate polys)
and are reported, not counted.

One planned invariant was dropped on evidence before implementation: the base
point is *not* on the surface plane for most surfaces (volume brushes keep it
in brush space), and the projection formula does not need it to be.

`tools/prepare_bsp.py` now defaults to `--uv surface_axes`:
`((P - Base) . Axis) / texel_scale`, written with the same V flip as the
static-mesh OBJ writer. The divisor is the part no oracle can see — umodel
exports no BSP and the object dumps print empty node arrays — so
`texel_scale = 128` is a prior matching the earlier placeholder's density,
recorded in the manifest as `texel_scale_status: UNVERIFIED`, adjustable with
`--texel-scale`, and waiting on a matched in-game view of a tiled BSP surface.
`--uv planar` keeps the previous placeholder. `PolyFlags`, `iBrushPoly`, the
shadow-map scale, lighting channels and the Model remainder stay opaque.
Record: docs/verification/BSP_TEXTURE_AXES.md.
