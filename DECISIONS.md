# Decisions and evidence

## 2026-09-23: Sanctuary central-pillar shell hierarchy

The Sanctuary pillar's UModel build 1590 MD5 mesh names the actual 17-bone
hierarchy, but its `Open.md5anim` hierarchy lists the child tracks under Root.
The animation frame values for those tracks are local to the **mesh** parents:
composing them through the mesh hierarchy returns all 17 frame-0 joints to
their mesh bind positions within 0.11 cm, with quaternion alignment above
0.999. Composing through the animation header instead placed the front and
upper TopShell plates thousands of centimetres below the cap.

`prepare_sanctuary_pillar.py` now uses the mesh hierarchy for the bounded
17-joint `Open` bake and rejects a changed hierarchy or a frame-0 bind-pose
mismatch. The lower Base and Tile OBJ sections remain byte-identical; only
the Top and Topinner sections change. UModel remains the external mesh and
animation payload source. The project still owns package identity, placement,
material selection and the frame-0 verification. Corrected UE5 host captures
are under ignored `local/center-pillar/`; original-game visual parity and
runtime animation remain **UNVERIFIED** pending paired capture review.

## 2026-09-22: spaced hole probes and target-aware inspection candidates

`prepare_terrain.py` now emits three spaced flagged-hole candidates per
terrain (`hole_probe_policy=three_spaced_adjacent_cells_v1`,
`hole_candidates`) instead of a single one, so one prop or building floor
sitting over the chosen cell cannot hide the whole runtime check. The legacy
`hole` object is still emitted as the first candidate, and regenerating
Sanctuary's `terrain-runtime.json` left all eight first candidates identical
to the previously recorded cells, so this is a strict superset rather than a
change to existing evidence. `TerrainWalkingTest.cpp` walks the candidate
list and falls back to the single `hole` object for scenes prepared before
this change. Endpoint assertions are still reported separately from the
original-point band trace and are explicitly not treated as original-hole
proof; whether any of these cells is a hole in the original game remains
UNVERIFIED pending matched screenshots.

`prepare_inspection_views.py` adds host-side candidate poses that look back
at a recorded terrain stand point for the obstructed Terrain_10 view
(`target_trace_candidate_v1`), and `InspectionTest.cpp` selects the first
candidate whose target trace reaches the requested actor, warning and
advancing when one is obstructed. The offsets are deliberately broad and
symmetric host inspection candidates, not recovered original-game camera
coordinates.

`tools/diagnose_sanctuary_geometry.py` is a new read-only diagnostic that
resolves scene ownership, per-section effective material, terrain layer
provenance and camera-ray coverage gaps from the prepared manifest. It
reports; it does not modify host geometry. It has not yet produced a
confirmed cause for the Scooter-street opening or the bright snow-view
surface.

Verification: UE5 `OpenWillowEditor` compiles with both test changes; the
runtime automation tests could not be run in this worktree because the
Sanctuary map is not imported here. ctest 6/6 and 154 focused Python tests
passed.

## 2026-09-22: bounded dispositions for Sanctuary null mesh sections

The installed Sanctuary payload assigns material index zero (`None`) to 15
placed sections: six `ResistanceBanner_03` sections, four `Blocking_Cube`
sections, three `VendingIcon` sections and two `SancBuild1_Trim` sections.
`tools/prepare_level.py` now records exact, package-scoped dispositions rather
than treating these as unresolved path lookups. The six banner companion
sections use the observed `Mati_ResistanceBanners_Static` from the sibling
`ResistanceBannerFrame_02` mesh; the two trim sections reuse the observed
`Mati_SancBuild1a_04` slot-0 material, corroborated by `SancBuild1Base_Trim`.
The three vending icons retain a named host neutral fallback. The four
unassigned blocking helpers retain their observed collision body and the
existing render-only hide policy for the exact `InterpActor_19/55/56/57`
placements. `refresh_materials.py` reapplies these rules to older manifests.

These are host-side material/visibility dispositions, not native bindings or
shader reconstruction. `audit_scene_materials.py` reports all 15 as explicit
policy rows with `visual_status=UNVERIFIED` and zero unresolved null sections;
matched original-game screenshots remain required for visual acceptance. See
`docs/verification/SANCTUARY_SECTION_MATERIAL_ASSIGNMENTS.md` and
`tests/material_assignment_test.py`.

## 2026-09-18: Material inference honesty fixes (stale metadata, auxiliary suffixes)

Two game-free fixes from a code-only pipeline review of zuhu's Sanctuary
screenshots (view-dependent shading, untextured town center). No new
serialization, offsets, or bounds checks; `src/` untouched.

- A failed diffuse texture decode no longer leaves `diffuse_inference` /
  `diffuse_inference_method` / `surface_approximation` metadata claiming a
  diffuse that never decoded (`tools/prepare_level.py`, channel loop). The
  material renders the neutral fallback, and the manifest now says so too;
  refresh previously counted these as fixed while the audit counted them as
  gaps. Synthetic test: `test_failed_diffuse_decode_clears_stale_inference`.
- `AUXILIARY_TEXTURE` now also excludes `_detail`, `_rough(ness)`,
  `_height`, `_bump`, `_opacity`, `_illum`, `_lightmap`, `_gloss`,
  `_metal(lic)`, `_ao`, `_cavity`, `_displacement`, `_reflection` and `_env`
  suffixes (plus `_\d+` variants). A lone utility map previously became
  BaseColor with fixed roughness 0.65, producing wrong albedo with
  view-dependent shading. Synthetic assertions extend the existing
  `sole_cooked_resource_texture` cases.

Evidence: `ctest` 6/6 plus all pure-Python suites pass on a game-less PC;
`tools/verify_packages.py` not run (needs the install). Sanctuary/Ash
re-measurement against the real game remains open, as do the documented
approximations (fixed 0.65 roughness, planar/UV0 mappings, sky graph).

## 2026-09-18: First Vault Hunter chosen: Maya

zuhu chose Maya as the vertical slice's first Vault Hunter, resolving the
open question left in the entry below. Phase 4's gate is unchanged in scope:
one action skill, a handful of guns, one hand-picked mission, on Sanctuary,
now specifically Maya's Phaselock and her skill trees (Motion, Harmony,
Cataclysm) rather than a generic placeholder character. ROADMAP.md,
`docs/OPENWILLOW_ENGINE_PLAN.md` and `CLAUDE.md` are updated to name her.

Not done and not claimed: no Maya-specific native, skill or animation work
has started yet; this only fixes which character the vertical slice targets.

## 2026-09-18: Vertical slice adopted as the development priority

An outside software engineer reviewed the project's public roadmap and left
feedback recommending, in essence, that map porting (Phase 1's breadth) not
be finished before proving the harder, unproven phases: the script VM
(Phase 2), stock UE3 natives (Phase 3) and Gearbox's undocumented natives
(Phase 4). Their argument: porting maps is likely the easiest part of this
project, and discovering in Phase 2-4 that the approach doesn't hold after 80
maps are already ported would waste most of the project's calendar time on
the easy part. zuhu agreed and adopted a vertical-slice-first priority.

What changed: no code, no serialization, no parsing behavior. ROADMAP.md and
`docs/OPENWILLOW_ENGINE_PLAN.md` now scope Phases 1-4's gates to one map
(Sanctuary, already the most complete) and one Vault Hunter, plus one
hand-picked simple mission and a small set of guns, working end-to-end
(spawn, fight, loot, equip a gun, use a skill, complete the mission, die,
respawn), instead of requiring all 82 maps before gameplay work starts. Broad
map coverage (the remaining ~79 maps) and the remaining Vault Hunters are
deferred to Phase 5, after the slice gate is met. `CLAUDE.md` now records
this priority order for AI assistants so map or character breadth work isn't
picked up out of order without asking first.

Also adopted from the same feedback: an AI-usage norm recorded in
`CLAUDE.md` and `docs/OPENWILLOW_ENGINE_PLAN.md` §7, debug errors yourself
first (read the message, check documentation, Stack Overflow, GitHub issues)
before handing them to an AI assistant, and be able to explain generated code
before merging it. This is a working-norm change, not a parsing or
verification claim.

Not done and not claimed: which mission is still an open question (the
Vault Hunter was chosen the same day; see the entry above). No phase gate has
actually been met differently than before this entry. This is a
reprioritization of already-planned work, not new verified functionality.

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

## 2026-09-21: sky approximation from the dome's named inputs, and an opt-in outer hull

The accepted `Sky_Dome` placement previously painted the raw
`Sky_TransitionBL2Default_Dif` strip across the dome with UV0: every
time-of-day column wrapped once around the azimuth. The master graph
`Mat_SkyTimeOfDay_Master` is stripped from the cooked package, so this entry
does not decode it. Instead `prepare_level.py` reads the named inputs that
survive along the instance chain (`Transition_Track`, `clouds`, `Masks`,
`Time_of_Day`, `sky_brightness`, `Sun_spot_brightness`, `cloud_cap_opacity`,
`p_CouldBrightness`, `Horizion_track_color_multiplier`) into a
`sky_approximation` record, and the UE5 importer builds one fixed graph from
them: visible = lerp(strip(column, dome V) × sky_brightness,
strip(column, 0.95) × sky_brightness × cloud_brightness,
saturate(clouds.R × cloud_cap_opacity)). Method name
`sky_time_of_day_strip_v1`, status `partial_unverified`.

Two readings of `Time_of_Day = 170` were compared on the extracted strip:
as a pixel column (/256) it selects the blue daytime gradient; as degrees
(/360) it lands in a sun column and gives dusk hues. The pixel-column reading
is used and recorded as `UNVERIFIED` in the manifest; nothing in the package
says which the original shader does. The sun spot, `Masks` (stars and cap
gradient), the horizon color multiplier, cloud channels G/B, cloud motion,
time-of-day animation and Kismet control are listed as omitted. The ordinary
diffuse inference stays on Base Color as the fallback, and the host's blue
`OpenWillow_SkyFallback` sphere, which sits inside the dome and hid it, is
now only spawned when no accepted dome carries this record.

Separately, the `Sanctuary_Outer` hull (`Prop_Skybox.Meshes.SanctuarySky` and
its two antennas) is placed with masked `_Teleported` phase-in overrides whose
graphs Material v1 cannot recover, so it imported invisible. With the new
opt-in `--outer-shell` flag the preparer drops only those `_Teleported`
overrides whose mesh-default material resolved a diffuse and records each
replacement; every other override is kept. This is a substitution, not the
placed material, and whether the running game shows `_Outer` or `_Land` is
Kismet state that is still not interpreted. Default behaviour is unchanged.
Record: docs/verification/NATIVE_SKY_APPROXIMATION.md.

The editor fly-through after this slice added two facts. First, the hull's
tower sits off-centre and above the town's own; the actor transform matches
the game dump and the vertices match umodel, and the sublevel's Kismet shows
why: `SeqAct_Interp_0` (`SanctuaryLiftoff`) binds the hull to a
`RelativeToInitial` move track that jumps it 1.7 km south and lifts it
100–150 m before hiding it at 36 s. The placed transform may be a parking pose
for a cutscene prop, but its in-game position remains unverified; the mismatch
is left as-is and recorded. Status for
`StaticMeshComponent_393` / `InterpActor_29` is **decode verified, in-game
position unverified, observed off in editor**. The first-key import is an
opt-in experiment recorded in the verification note; the serialized placement
remains the default and no correction is committed. Second, two
`Blocking_Plane` placements on the horizon carried
`Sanctuary_Light:Env_Ice.Materials.Mat_CloudLayer_01`, a sibling of the
already-hidden `Mat_CloudLayer_Light`, and tiled a dust sprite as yellow/black
stripes; the hide rule now names both instances and `refresh_materials.py`
re-evaluates it.

## 2026-09-21: inspected surface fallback for the Hyperion moon base

The station in front of the moon (`Prop_MoonBase.Mesh.MoonBase02`, placed
material `Mati_MoonBase_02a`) imported as the neutral gray Lit fallback and
read as a black silhouette against the Unlit sky dome. The package explains
why: the base `Mat_MoonBase_02a` keeps no texture parameters, only tint,
fog, rim and emissive-multiplier constants, and its cooked texture list
holds `MoonBase02a_Dif/_Nrm/_Emis` plus a `Tiling_SmokePanner2_Dif` overlay.
The second `_Dif` defeats the sole-`_Dif` rule, which then refuses to guess.

`INSPECTED_COLOR_FALLBACKS` gains an entry for that placed instance naming
the hull's own `_Dif` as color, `_Nrm` as normal and `_Emis` as emissive
(`moon_base_color_fallback_v1`, `partial_unverified`). The table now allows
an entry keyed by a placed instance with an explicit required `base`
(refused with a recorded issue when the parent differs) and an optional
emissive texture. `MoonBase_Color`, `Emissive_Mult`, `Fog`/`Fog_Intensity`,
`RimLight_Color`, the smoke overlay and UV modulation are recorded as
omitted; the station is expected to read darker and less blue than the
original. Verified: exactly one manifest material changed; import, saved
scene, collision and UV verification pass; three game-viewer captures show
the plated hull, lens and lights. Not verified: the stripped graph, the
tint/fog combine, or parity with the original.

Same day, the moon behind it: `Mati_Moon` over the Unlit additive `Mat_Moon`
had already resolved `Moon_Dif`, but emitting that gray texture at 1x reads
as a pale smudge against the dome. The package keeps `p_moonColor`
(4.02177477 gray) among `Mat_Moon`'s nine surviving named parameters, and
for an additive Unlit surface a color scale on the emitted value is the one
term whose meaning is not in doubt. A second table,
`INSPECTED_UNLIT_COLOR_MULTIPLIERS` (`unlit_color_multiplier_v1`,
`partial_unverified`), keyed by that placed instance with a required base,
reads the named vector through the chain (instance overrides last), refuses
a non-Unlit chain, a different base, or a missing/non-finite/negative value
with a recorded issue, and writes `unlit_color_multiplier` into the material
record. The host multiplies the recovered Unlit color by that constant before
the emissive input, on the existing Unlit branch only; `verify_level.py`
checks the `Multiply`/`Constant3Vector` pair against the recorded value and
reports `verified_unlit_multiplier_materials`. Recorded as omitted: the
`MoonBase02_GRP` H-shaped station shadow mask, `p_moonTimeBaseShadow`,
`Moon_Comp` relief, `p_Basecolor2`, `p_DarkColor`, the `Transition_Track`
time-of-day tint (with `Time_of_Day` and
`Horizion_track_color_multiplier`) and `p_moonTime`/`p_moonRotation` UV
motion. Verified: one manifest material changed; import and all saved-scene
verifiers pass with the new check; the same three captures show a bright
cratered disc instead of the wash. Not verified: the moon's absolute
brightness against the original, and the station shadow's placement on the
moon, which is deliberately not guessed until an in-game reference
screenshot exists. See
[MOON_BASE_SURFACE.md](docs/verification/MOON_BASE_SURFACE.md).

## 2026-09-22: terrain layer weights read from the cooked WeightedMaterials block

`tools/prepare_terrain.py` draws each Sanctuary terrain as a host weighted
sum of its layers (`terrain_weighted_sum_v2`) when, and only when, the
weightmap/layer pairing can be read from the package itself. The terrain
actor's native tail, after the per-layer `AlphaMaps`, carries a
`WeightedMaterials` array (per entry: `Data[grid]` bytes, `SizeX`, `SizeY`,
a reference back to the terrain, a `TerrainMaterial` reference) followed by
a `WeightedTextureMaps` array of `TerrainWeightMapTexture` references, entry
`i` pairing with weight `i`. `grid` is `NumPatches * WeightmapTesselationLevel
+ 1` per axis; that property, not a decoding fault, is why `Terrain_2` and
`Sanctuary_Land:Terrain_3` looked undecodable at level 2. Decoded by
`decode_weighted_materials` in `tools/terrain_decode.py` and bounded like
the alpha-map decoder.

Verification: on all eight installed Sanctuary terrains, every cooked weight
array equals its paired `PF_G8` texture texel for texel (31 of 31), and
every `TerrainMaterial` reference belongs to a layer of that terrain. The
preparer repeats that byte comparison per terrain and falls back to the
single-layer approximation below if it fails. Weight sampling is
vertex-centred bilinear over patch coordinates (`scale = tessellation /
texture size`, `offset = 0.5 / texture size`); each layer's colour tiles by
its `TerrainMaterial`'s diagonal `LocalToMapping`. UE3 stores these weights
already stacked, so the host sums `weight_i * colour_i` without further
normalisation. Slope/noise filters are baked into the stored weights;
lightmaps, decorations and foliage are not reconstructed.

`Sanctuary_Land` layers reference setups and `TerrainMaterial`s imported
from `Sanctuary_P`; the preparer now resolves those through the scene
(`--properties ... --array-schema tools/terrain-arrays.schema`), which is
what made `Sanctuary_Land:Terrain_3` recoverable at all.

Codex's earlier revision of this change gated the weighted graph behind a
local evidence file that was never produced and replaced the previous
fallback with an untextured neutral material, regressing all 15 components
to white. When the cooked pairing cannot be trusted the preparer keeps the
2026-09-15 `terrain_dominant_alpha_layer_v1` approximation, or
`terrain_first_material_layer_v1` when no alpha ranking exists.

Five layer materials resolved with no texture channels because their cooked
lists carry several `_Dif` textures: `Mat_PatchySnow`, `Mat_SolidSnow`,
`Mat_DirtySnow`, `Mat_InterludeSandTracks`, `Mat_ColdGrass`. Their
extracted textures were inspected side by side and entries added to
`INSPECTED_COLOR_FALLBACKS` in `tools/prepare_level.py` naming the texture
that carries the surface colour (plus a native normal where one survives).
These are single-texture stand-ins for masked or macro-tinted blends and are
labeled as such. `tools/refresh_materials.py` now carries host terrain blend
materials across a refresh; re-run `prepare_terrain.py` afterwards.

Not established: visual match against the original game (pending in-game
comparison), the BSP texel scale and V orientation (`tools/calibrate_bsp_uv.py`
has no matched original-game/UE5 measurements; see
[BSP_TEXTURE_CALIBRATION.md](docs/verification/BSP_TEXTURE_CALIBRATION.md)).

## 2026-09-22: Refresh host evidence without claiming original-game parity

The BSP calibration fixtures and example now supply both per-view pixel
anchors required by the existing tool. Six synthetic tests pass, including
rejection of legacy or incomplete anchors; no production acceptance was
weakened. Fresh TerrainWalking, BspWalking and seven-view Inspection runs
pass against the final imported map. Direct hole evidence remains 3/8 and
visual review still finds unresolved artifacts. The maintainer deferred
matched original-game captures and item 6 calibration to a later pass.
See [the refresh record](docs/verification/SANCTUARY_TERRAIN_BSP_REFRESH.md).

## 2026-09-22: hide CollisionCube placements by their observed hidden flags

The 2026-09-14 render-only policy hid all 94 `Common_Meshes.CollisionCube`
placements by mesh name. That removed the street in front of Scooter's
garage: six `Sanctuary_Land` `StaticMeshCollectionActor_38` placements
(`SMC_544/603/608/615/617/618`) are 1536x1536 uu `Mati_FloorConcrete01`
slabs with top z = 2720, and 8 of the 11 floating liquid decals in the
scene sit 8 uu above them. Nothing else in the manifest covers that area.

`prepare_level.py` now records `source_hidden` for each placement: whether
the component serializes `HiddenGame`, or its owner actor serializes
`bHidden`. A CollisionCube is hidden when `source_hidden` is set, or when none
of its effective materials resolves to a known source other than the cube's
own `Common_Meshes.Collision.Mat_Collision`. In the regenerated Sanctuary
manifest, 81 cubes stay hidden: 65 set `HiddenGame` (one also has
`Mati_FogsheetBlack`) and 16 belong to `bHidden` `InterpActor`s. Those
`InterpActor`s include the 13 `Mati_SlateRock8xTileWarm` cubes that are
parked about 350,000 uu away. 13 cubes render: eight `Mati_FloorConcrete01`,
three `Mati_SancBaseConcrete_tile02`, one `Master_Black` sheet
(1000x70x0.05 scale, shadows and lighting disabled) and one
`Mat_RoadIceSkybox` piece. `refresh_materials.py` treats a manifest without
`source_hidden` as hidden, which keeps the earlier result for older
manifests.

Verified: decoded flags and resolved materials for all 94 placements, and
synthetic tests of the rule. Comparing the regenerated manifest with the
previous one: exactly 13 placements change, all CollisionCube, from hidden
to rendered. The placement count (4469) and issue count (185) are unchanged.
The UE5 import and its saved-scene, collision and UV verifiers pass. An
ad-hoc Inspection capture from (1500, -6400, 2900) toward the
`ScootersGarageSign` shows a continuous concrete street where the sky showed
through before. The run reported one camera-rotation tolerance error for the
fractional ad-hoc pitch; the screenshots were still captured. The layout
matches a maintainer's original-game capture of the same spot. The dark puddle
stain visible in that capture does not appear in the host; the liquid decal
planes above the slabs were not investigated in this pass. Not verified: in UE3, `bHidden` on an owner
that Kismet or Matinee toggles at runtime is only the saved state. The owner
flag is applied only to CollisionCube; other meshes with a `bHidden` owner
are unchanged by this entry. Whether `Master_Black` and `Mat_RoadIceSkybox`
look right in the original game has not been checked against a capture.

## 2026-09-23: landed Sanctuary centre pillar with its pre-takeoff surface

The centre of the Sanctuary plaza is `SkeletalMeshActor_2` in
`Sanctuary_Dynamic`, using `Skel_SanctuaryCentralPillar`. The full mesh is the
tall spire seen after Sanctuary takes flight. Before that, the takeoff Matinee
has not run, and only the top of the spire shows above the plinth as an angular
monolith. `tools/prepare_sanctuary_pillar.py` bakes one frame of UModel build
1590's MD5 export of the mesh and its `Open` animation to OBJ. It places the
actor at the first world key of the `Spire` group's move track in
`Episode_8`'s `InterpData_0`, (8424, 632, 544), at the actor's 0.67 scale.
That key was read in an earlier session and is recorded in the script; it was
not re-derived for this entry. The reference state is a maintainer's own save
at *Welcome to Sanctuary* (Plan B, "Install first fuel cell") and its in-game
capture of the plaza. Neither is tracked.

The surface comes from `Master_SancSpire`. Its instances pass two textures:
`Color` (`SancSpire_Col`, a low-contrast tint in three horizontal-UV strips,
selected by `Color_UV_Scale & Offset`) and `Luminosity` (`SancSpire_Lum`,
three unrelated grayscale detail maps packed in R, G and B). The master's
`Luminosity_Channel` static component mask defaults to R. Each instance
overrides it, and the value is not in the tagged properties. We located it by
searching each instance's trailing static-parameter bytes for the mask node's
`ExpressionGUID` and reading the four flags before it: Top = G, Base = B,
Tile = R. `Emissive_Channel` uses the same pattern. Two independent checks
agree with that reading. First, Base's rasterized UV islands land on B-channel
detail, while the area outside them is B's flat filler (mean 138 outside vs 59
inside). Second, the coarse Col-strip/Lum-channel correlation is highest on the
same diagonal. Topinner samples `StubWhite_Gray` as Luminosity and carries its
detail in `SancPillarTopInner_Dif`. Our decode of `SancSpire_Lum` matches
UModel's DDS within 1/255.

What is approximated and UNVERIFIED: the cook stripped every non-parameter
node of `Master_SancSpire`, so how Color and Luminosity are combined is not
known. The tool bakes `2 x Luminosity[channel] x Color strip` in 8-bit sRGB
space per section, chosen so the roughly mid-grey tint (mean about 119) stays
near neutral. The baked sections then use the mesh's raw UVs. The emissive
channel is dropped: `p_emissive` defaults to 0 and no instance raises it (Topinner
sets it to 0 explicitly). The glow is assumed to belong to the takeoff
sequence; that is not checked. The pose is a frozen frame 0, collision is
absent, and the placement is compared by eye only. Before this change, the
host rendered the pillar with the raw Col atlas as diffuse and
`SancPillar_Emm` as emissive, giving an iridescent surface with coloured
streaks. The master's default `SancPillar_Emm` was used even though the
instances override Emissive with `SancSpire_Ems`.

Host result: the UE5 import passes its saved-scene, collision and UV
verifiers with no material compile failures. The four plaza Inspection
captures show a weathered grey-blue panelled monolith with no iridescence or
glow. Compared by eye with the maintainer's capture: the silhouette and
overall tone agree. The host shows more high-contrast grating and light panels
on the faces than the original, which could come from the unverified combine
step, the missing lighting and specular behaviour, or UV placement. That
difference has not been resolved.

UV fix (same day): the tool wrote UModel's MD5 V as-is, but OBJ V is bottom-up and
ow-package writes `1 - v` (`src/assets.cpp`). The earlier Base-channel UV-island
test only matched top-down V, which confirms the MD5 V is top-down. After flipping V,
close-up host captures show the vertical "SANCTUARY" face and the single grated
channel seen in the game close-ups; the diamond-grating "shard" look came from
sampling the texture upside down. Compared by eye only.

Open (pose not solved): close-up game captures show the same `Top` texture details
("SANCTUARY" lettering, screw hatch, grated channel), so the plaza
monolith is this pillar. But in the game it stands much taller through the
plinth hole, and its top block is tilted. No frame of the UModel-exported
`Open` / `Open_Idle` clips matches; frame 0 equals the bind pose. What was
checked: our skinning matches UModel's PSK bind pose to within 0.15 units
across all 5,811 vertices. The Y flip matches UModel's PSKX of
`SanctuaryPlatformInner` against our reader. Nothing else occupies the space
above the plinth. What is left to check: the Matinee movement and
anim-control tracks for `SkeletalMeshActor_2` (the actor is
`PHYS_Interpolating`), and whether UModel decodes this compressed animation
correctly.

## 2026-09-23: no walker collision for the BoxLrg on the Sanctuary player start

A maintainer walking the host scene was stuck at spawn and identified the
blocker in the UE editor as `Sanctuary_Outer` `InterpActor_34`
`StaticMeshComponent_20`. It is a `Prop_Garbage.Meshes.BoxLrg` at
(2251, -4296, 2733), yaw -61.875 degrees, with DrawScale 20 and DrawScale3D
(1.5, 4, 0.5). That makes a box roughly 19 x 34 x 4.6 m spanning z 2503-2963.
It was already hidden visually on 2026-09-22, with its collision kept.
`Sanctuary_P`'s own `WillowCoopPlayerStart_0` is at (655.6, -6348.9, 2800),
inside that box, and the host spawns 100 units above it. A blocking volume on
the game's own player start cannot be active when the player is in the landed
town. The component serializes no collision overrides, so it blocks under
UE3 defaults. So something the host does not run (Kismet level visibility of
`_Outer` versus `_Land`, or `_Outer`'s `Main_Sequence` Matinee) must remove or
move it in the original. Which one is not known; we found no record-level
reference to the actor in `_Outer`'s scene records, but the record schema does
not cover Kismet variables.

`tools/prepare_level.py` now leaves exactly this (level, source) placement
without walker collision. The other two hidden BoxLrg placements (`_26`,
`_33`) keep their collision, since there is no comparable evidence for them.
`docs/TOOLING.md` previously placed `_34` in `Sanctuary_P`; the manifest records
it in `Sanctuary_Outer`, and the doc is corrected. Not verified: whether other
`_Outer` content should be inactive in landed play.

## 2026-09-23: hide every static placement the source marks hidden

A maintainer reported roof textures on the street in front of Scooter's
garage and the gate square beside it. The original game shows plain concrete
there. A triangle probe of the manifest at the bollards (around
(3500, -3800)) found two stacked floors: `Sanctuary_Land`
`SanctuarySidewalk_ParkingLot` at z 2783 with its concrete materials, and 8 uu
above it `Sanctuary_P` `StaticMeshCollectionActor_24` `SMC_288`,
`SanctuarySidewalk_ParkingLot_Low`, drawn with
`Optimization.Mati_SancBuild4a`. That material's regular-atlas fallback
(2026-09-14) samples a building atlas that includes roof tiles. The component
serializes `HiddenGame = true`, so the game never draws it. The 2026-09-22
rule applied `HiddenGame`/`bHidden` only to `CollisionCube`.

In the manifest, 219 placements had `source_hidden` set but still rendered.
204 of them are in `Sanctuary_Px`, an always-loaded sublevel where 205 of 206
placements are `HiddenGame`. They include merged low-detail building shells
such as `SancBuildingGroup01`, whose bounds reach down to street level
(z 2780) around Scooter's, plus roof pieces and `_Low` sidewalks.
`hidden_visual_mesh` now hides any placement with `source_hidden` unless its
source is an `InterpActor`. A mover's flag is only its saved state because
Kismet or Matinee can toggle it, so the 12 hidden-flagged `InterpActor`
placements keep the earlier rules. Collision is unchanged: UE3 `HiddenGame`
does not disable collision, and the host keeps each placement's
`collision_enabled`. `refresh_materials.py` now presumes an unrecorded flag
hidden only for `CollisionCube`, so older manifests keep their earlier result.
`verify_level.py` accepts any mesh for a hidden-flagged static placement.

Verified: the decoded `HiddenGame` on `SMC_288`, and synthetic tests of the
rule and the legacy-manifest default. Re-applying the rule to the local
manifest changes exactly 207 placements from rendered to hidden (204
`Sanctuary_Px`, 2 `Sanctuary_P`, 1 `Sanctuary_Land`) plus one entry that had
no flag (the skeletal centre pillar; it stays rendered). The UE5 reimport
hides 307 placements and passes the saved-scene, collision and UV verifiers.
`verify_level.py`'s exact `BoxLrg` source list now applies only to placements
without a source hidden flag, because two hidden-flagged `Sanctuary_Px`
`BoxLrg` placements are also hidden. A maintainer confirmed in the editor that
the roof textures are gone from the street. Not verified: a matched
original-game capture comparison, and which earlier commit first made the
overlap visible.

## 2026-09-24: Scooter frontage collision and Maya's empty-hand clips

The Sanctuary scene records collision enabled on the placed
`SancScooterStairs` and `SanctuarySidewalk_ParkingLot_Low`, but their source
meshes have no `RB_BodySetup`. The latter is a source-hidden proxy that
overlaps the visible `Sanctuary_Land` parking-lot mesh about 8 cm higher.
For these two exact mesh identities only, when the body setup is absent, the
preparer records a render-triangle collision fallback. The placement's source
collision flag still controls whether it blocks the pawn. This is a host
walking approximation; native UE3 collision parity is `UNVERIFIED`. The
visible `Sanctuary_Land` parking-lot mesh is not changed.

The current Maya walker has no weapon. It now selects the UModel build 1590
`1st_Person_Unarmed` clips from `GD_Siren_Streaming_SF` instead of the pistol
clips. The existing converter and UE skeleton still own track conversion and
asset identity. Clip import and the UE host build are automated checks;
original-game pose and blending remain `UNVERIFIED` until a paired capture
is inspected.

Fresh checks in the isolated checkout: Release C++ build, 6/6 CTest, nine
package comparisons, UE5.8 host build, 31 level tests, the saved scene check
(4,888 section actors) and the saved collision check (615 sections, 123 active
triangle components, zero errors) passed. The imported unarmed idle also
logged active in a game capture. The saved UV0 check passed on 615 sections,
181,767 triangles and 545,301 corners. Live `-owspawnprobe` walking checks grounded
the pawn on the exact parking-lot proxy `StaticMeshActor_SMC_288` and the
Scooter stairs `StaticMeshComponent_970`; both captures show the player above
the floor. An earlier lower-stairs probe rested on an overlapping building
floor, so that probe was not used as stairs evidence. Commandlet editor-world
line traces returned no hits and were not used as runtime evidence. Native
BL2 collision shapes, animation blending and original-game visual parity
remain `UNVERIFIED`.

## 2026-09-23: Maya's body, default head and first-person arms reach UE5

First Maya asset slice for the vertical slice: meshes and a default-skin
surface only, with no animation, skeleton merge or gameplay.
UModel build 1590 exported `GD_Siren_Streaming_SF` `Skel_SirenBody`
(5,368 vertices, 27 joints) and `Hands_Siren` (2,204 vertices, 47 joints),
plus `CD_Siren_Head_Default_SF` `Skel_Siren000` (2,803 vertices, 7 joints),
as glTF. Each mesh run exited 255 after writing the mesh but before any
texture, following missing `Common_Textures` stub imports. Exporting each
`Texture2D` alone as PNG succeeded. UE 5.8 rejects UModel's BC1 DDS
(`DXGIFormat not supported : 71`), so PNG is the texture path.
`host/ue5/import_character.py` imports them as UE5 skeletal meshes, and
`tools/run_ue_character.ps1` imports and captures a `-game` preview. Height
comes out at about 172 cm (glTF metres, Y-up converted by UE). The head's
lowest point meets the body's neck in bind pose.

Surface: the default skin (`CD_Siren_Skin_Default` ->
`CD_Skins_Siren_MainGame.Mati_Default_Body` / `Mati_Default_Head`, parent
`Common_Materials.Player.Master_Player`) passes `p_Diffuse`, `p_Normal`,
`p_Masks` and Shadow/Midtone/Hilight colours for zones A, B and C, read with
`ow-package --properties` and `level-arrays.schema`. `Master_Player`'s graph is
stripped, so the combine is not known. Observed: each `_Msk` holds two
half-width copies of the UV layout. In the right half, the head's hair is R
(zone A, whose midtone is blue), the face is B and the collar is G. The host
therefore samples the right half at `u/2 + 0.5`, takes A = R, B = G, C = B, and
multiplies the diffuse by `2 x Midtone` inside each zone.

UNVERIFIED: that combine, the factor 2, the unused Shadow/Hilight colours and
intensity scalars, the `_Msk` left half (it may drive the tattoo glow; the
emissive colours are not used), the hands reusing the body colours, and
left/right handedness. Result compared by eye only, with no original-game
capture: the preview shows blue hair, a yellow-orange top with grey panels,
black trousers, and a tattooed arm opposite a yellow sleeve, the arms matching
the body. The preview's fixed exposure and 2.5 lux sun are an inspection aid.
No C++ or parser code changed.

## 2026-09-23: Maya's animated first-person arms and eye height on the Sanctuary walker

UModel can export `AnimSet` animation as glTF only from its viewer
(`glTF animation could be exported from mesh viewer only`), so `Hands_Siren`
and `1st_Person_Pistol` were exported as MD5.
`tools/prepare_character_anims.py` converts six clips (Idle, Run_F, Sprint,
Jump_Start, Jump_Idle, Jump_End; 30 fps) into bone tracks for the UE skeleton
imported from the same mesh's glTF. The MD5-to-UE map is fitted from the 47
bind positions and must be a pure Y mirror (it is). Each bone's axis
convention is absorbed by comparing the MD5 and UE bind poses.
`host/ue5/import_character_anims.py` writes the tracks as AnimSequences through
the editor's animation data controller.

A per-frame root correction keeps the arms skeleton's `Camera` bone at the
player camera. In all six clips that bone does not move relative to the root
(range 0.1 cm or less), so this is equivalent to a fixed attachment. That BL2
views from this bone is UNVERIFIED. Check: the right hand's camera-space
position from forward kinematics of the written tracks equals a direct MD5
computation (47, 17, -21.5 cm, frames 0/30/60 of Idle). At the converted FOV
this projects to about 64% across the screen, which matches the pistol hand
in a maintainer capture outside Scooter's. The earlier static OBJ bake of the
same frame rendered the hand near 85-90% across, so that render was wrong and
the static path has been removed; `prepare_character_pose.py` remains as the
MD5 reader and a single-frame OBJ baker.

The walker's `-owmaya` mode plays Idle, Run_F (horizontal speed over 50),
Jump_Idle (falling) and Jump_End (landing) with hard cuts. BL2's AnimTree
blending is not reproduced. The clips themselves are subtle: the hand moves
about 2 cm running and 5 cm jumping. The strong running gun bob seen in the
game is assumed to come from native weapon code, which is not implemented.

Eye height and collision come from `GD_Siren_Streaming.Pawn_Siren`: the
`CylinderComponent` (properties at offset 8, not 4) gives CollisionRadius 42
and CollisionHeight 80. The pawn gives BaseEyeHeight 70 and a serialized
EyeHeight of 77. `-owmaya` uses a 42/80 capsule and puts the camera 70 above
its centre (150 cm standing eye). Choosing BaseEyeHeight over EyeHeight is
UE3 behaviour assumed, not checked in the game.

FOV: `DefaultEngine.ini` sets `[Engine.LocalPlayer]
AspectRatioAxisConstraint=AspectRatio_MaintainYFOV`. A BL2 FOV value is
treated as horizontal at 4:3 and converted to UE's horizontal FOV at the
actual aspect. `-owfov=` defaults to 90 (106 degrees at 16:9). This matched
the Scooter's capture by eye; the player's actual setting was not read.

Automated: host build; import commandlets exit 0; `OpenWillow.Walking` passes
(without `-owmaya`). Runtime: an unattended `-owautowalk` run logged the state
sequence Idle, Jump_Idle, Jump_End, Run_F, then Run_F/Idle alternating once
per 6 s lap as the pawn bumped geometry. Visual: idle and walking captures
compared by eye only. No gun is held.

## 2026-09-16: External extraction is an accelerator, not a replacement

The project will evaluate mature community exporters before expanding every
custom visual decoder. UModel / UE Viewer is the first candidate because its
official compatibility data includes Borderlands 2 and it recognizes this
installation as package version `832/46`.

The verified local candidate is UModel build 1590 from the upstream
`gildor2/UEViewer` checkout at commit
`a0bfb468d42be831b126632fd8a0ae6b3614f981`. The executable SHA-256 is
`13502E5A4D8F6B5F32252AFEBD6360F7302CCFACCF6B8DDA65BEFF0BE2D364A0`.
It scanned 920 files, listed `Ash_P.upk` with 21,834 exports and exported
`Ash_Road01` as glTF in 0.1 seconds. Follow-up smoke runs exported the
TFC-streamed `MetalRoadConcrete_Dif` texture as a 1024x1024 DDS in 0.09
seconds and `Skel_BugMorph` as glTF in 0.08 seconds. The skeletal run emitted
unknown-field warnings that remain recorded as benchmark limitations. All
output was written only under ignored `local/external/`.

This is an acquisition and smoke result, not an importer or compatibility
decision. The Phase 0.5 gate in `ROADMAP.md` must test textures, static and
skeletal meshes, animations, sounds, materials, batch failures, duplicates,
output size and UE5 importability. Until that gate passes, UModel remains an
external visual oracle and optional payload source; `ow-package` remains the
project’s metadata, reference and verification path. No UModel source was
copied, and no game-derived output is tracked.

## 2026-09-25: landed walk excludes Sanctuary_Outer

`Sanctuary_Outer` and `Sanctuary_Land` are separate Kismet-streamed sublevels.
The landed walking view now tags all 29 `_Outer` placements (31 rendered mesh
sections) on import and hides them with collision disabled at `-owwalk` startup.
The saved inspection map retains them for scene-provenance work. This includes
the liftoff hull and antennas, skybox buildings, and invisible collision boxes;
it does not suppress the landed road or plaza geometry. The alternative
`Sanctuary_LandedComparison` map and captures are ignored local outputs.

Automated: UE 5.8.3 reimport and saved-scene, collision and UV verifiers passed;
the runtime log reports 31 hidden sections; 6 CTest cases and the nine-package
comparison passed. Visual: a capture from (8424, -2600, 3900) facing north
shows a clear road into the plaza. The maintainer identified the center object
in an overhead host capture as the intended monolith and accepted its current
appearance in that view. Its UModel 1590 `Open` frame-0 bake is present; exact
pose parity against the original-game reference has not been measured.
The maintainer's side-street framing was matched at approximately
(8150, -700, 3800), yaw -90 degrees, 106-degree FOV: one Resistance poster on
the left, two on the right and the road cover align with the report image.
In the landed comparison capture the tall slab and blocky wall are absent,
exposing Dr. Zed's building beyond the street. This is visual validation of
that view, not an original-game placement comparison. Runtime: the walking
pawn and Maya's first-person arms initialize; the reported street has not yet
been traversed after this change.

## 2026-09-25: Archives floor collision for the Sanctuary walker

The visible `Sanctuary_P` placements of
`Env_Sanctuary.Meshes.RolandsArchivesFloors` (`StaticMeshActor_44` component
228) and `RolandsArchivesCrushRoom` (`StaticMeshActor_222` component 439) have
host collision enabled but neither mesh has an `RB_BodySetup`. The first has
three render sections with horizontal triangles at z 3680 and 4256; the
adjoining room has floor triangles at z 4256. Their actor placement translates
Y by -384. The separate `Sanctuary_Px` copies of the main floor remain
source-hidden with collision disabled.

For these two exact mesh identities, `prepare_level.py` now records a
`triangle_mesh` fallback when the source body is absent. UE5 uses each
section's render triangles as complex collision and retains each placement's
source collision switch. This is a host walking approximation, not a claim
about the original game's blocking volumes or collision parity. UModel build
1590 is available at `C:/Users/yorad/Tools/UEViewer/umodel.exe` (SHA-256
`13502E5A4D8F6B5F32252AFEBD6360F7302CCFACCF6B8DDA65BEFF0BE2D364A0`),
but this collision decision uses the project's package identities and local
render geometry rather than an external collision export.

Fresh Sanctuary static-mesh, terrain and BSP preparation completed: 4,430
base placements, eight terrains with 15 components, and 228 root BSP polygons.
The previous local scene included the baked centre pillar. Its preparation
step could not rerun because this Python 3.14 installation lacks NumPy, so
the prior ignored scene was restored with only the two freshly prepared
collision records inserted; a comparison found those were the only changed
shared mesh records. UE5 reimport and saved scene, collision and UV checks
passed. The collision check reports 619 mesh sections, 3,246 enabled
components, 131 enabled triangle components and zero errors. A live
`-owwalk -owmaya -owspawnprobe` drop at (12357.3, 857.3, 4456) settled at
z 4338.15, `grounded=1`, directly on `StaticMeshActor_44` component 228.
Two adjoining-room drops grounded on overlapping props, so direct runtime
ground contact on the CrushRoom triangles and a walked route through the
building remain unverified. CTest passed 6/6, `tests/level_test.py` passed
31/31 and installed-package verification matched all nine code packages.

## 2026-09-25: Maya holds a part-filtered Infinity and casts Phaselock (host prototype)

The Infinity visual is no longer the whole pistol gestalt. `ow-package
--properties` with a local array schema (element types only, no parser change)
decodes `Weap_Pistol.GestaltDef_Pistol.GestaltInfos[0].Parts`: 59 fragments,
each a `SkeletalMeshFragmentName`, `MaterialIndex`, `FirstIndex` and
`NumPrimitives`. Their per-material triangle totals (22,316 and 538) equal the
two UModel glTF primitives exactly, which supports the assumption that UModel
writes sections in material order. `tools/filter_gestalt_gltf.py` checks that
equality and keeps only named fragments.

Part choice: `Pistol_Vladof_5_Infinity` uses `EPRM_Selective` over base
`Pistol_Vladof_4_VeryRare`. Its barrel `Pistol_Barrel_Vladof_Infinity` maps to
gestalt fragment `Pistol_Barrel_Vladof`; the base body `Pistol_Body_Vladof_4`
maps to `Pistol_Body_Vladof`. The base sight list spans eight manufacturers and
the grip comes from data not traced here, so `Pistol_Scope_Vladof` and
`Pistol_Grip_Vladof` are one plausible roll, not the only one. Kept: 1,505
triangles. The UE mesh bounds still count the unused gestalt vertices.

Material: `M_OW_InfinityApprox` is an UNVERIFIED stand-in for the stripped
`Master_Gun` graph. `p_Masks` R/G/B select regions A/B/C. A `p_Diffuse` channel
lerps each region's shadow/midtone/highlight colors from the MIC. The pistol
detail is read from blue, inferred from the texture name
`Weap_LauncherShotgunPistol_Comp`. `p_HighlightsIntensity`,
`p_ShadowsIntensity`, the pattern and the decal are not used. The white body
with navy detail follows the MIC colors; it is not compared with a game capture.

Pose: the mesh attaches to `R_Weapon_Bone` with a 90 degree yaw, an observed
fit. A logged barrel axis (`WeaponOffset` to `Barrel` bone) is 0.89 forward
in view; its 0.45 up component matches the bones' 6.5 cm height difference.
`ADD_Fire_Recoil` has identity tracks at frame 0 on all 47 bones, so it is a
UE3 additive clip. The arms instance now layers it as clip(t) relative to
clip(0) instead of blending it as a full pose, which had collapsed the arm.
Its 29.1429 fps source imports at 30 fps (logged). A broken earlier import of
that clip crashed any load in animation compression; the generated asset was
deleted and re-imported.

Gameplay, all host prototypes: hitscan at 10 Hz after a 0.8 s spin-up, the
installed `SpinUpDuration`. The fire rate is not evaluated. A deterministic
figure-eight stands in for the undecoded `FiringPatternLines`. Damage is a
placeholder 87 per shot. Tracers, the muzzle flash, impact sparks and bullet
decals are engine shapes with host materials, not `FX_WEP_Pistol` particles.
The muzzle point is estimated from the barrel's gestalt bounds, 27 cm ahead
of the `Barrel` bone. Phaselock sweeps a 30 cm sphere. The host-made training
dummy lifts in the installed 0.7 s over an estimated 170 cm, hovers in a
violet shell and falls back at -500 cm/s^2. A violet beam runs from Maya's
`L_Hand` bone during `Phase_Lock_Lift`. A point light on the shell pooled
violet on the road under Lumen and was removed. The duration (5.5 s) and
cooldown (13 s) remain unevaluated. The HUD follows BL2's layout with canvas
shapes; it is not BL2's Scaleform.

`-owcombatshots` (with `-owwalk -owmaya -owcombattest`) runs an unattended
aim/fire/Phaselock sequence and writes five `OWCombat_*.png` captures.

Review: an independent reviewer agent scored the five captures from BL2
feel, not from data: 1/10 before this pass, then 4/10 and 4/10. Its open
items: the gun sits high and tilted in the view, the material is flat
white, the tracer and figure-eight spread are hard to see in stills, and the
Phaselock shell and beam look generic. It also found the dummy and HUD
placeholder-grade and saw arcs in the sky. The gun pose follows BL2's pistol
Idle clip on `R_Weapon_Bone`; the view has not been compared with a matched
game capture, so a socket or axis error is not ruled out. This is a working
prototype, not visual parity.

Automated: host build; import commandlets exit 0. Runtime: the unattended
sequence logs 4 target hits, Phaselock activation and release about 5.5 s
later. Visual: the captures were reviewed by eye and by the reviewer agent.
None was compared with the original game.

Follow-up after a maintainer play test (same day):

- Key 1 did not re-equip the Infinity. The map selector's controller
  bindings for 1-9 consumed the key before the pawn's `OWEquipInfinity`
  action. Those bindings no longer consume input. Pressing 1 with the map
  menu open now does both, which is harmless because the map changes.
- Phaselock timing is now read from `GD_Siren_Streaming_SF.upk`.
  `ActionSkill_Phaselock` (`LiftActionSkill`) has LiftDuration 0.7,
  LockFadeOutTime 1.1 and LiftSnapTimePct/HeightPct 0.5. Its
  LockDurationFormula is `Att_Phaselock_Duration`, base 5, scaled by
  `PhaselockTimeScale`, default 1. The host lock is now 5 s instead of the
  5.5 s placeholder, and the shell fades over the last 1.1 s. `Startup.upk`
  `Cooldown_Phaselock` resolves to a constant 13, which confirms the host
  cooldown. Skill-tree and class-mod modifiers are not applied. The lift
  snap is not modelled and the 170 cm lift height is still an estimate. The
  skill also names BL2's hand-orb, enemy-bubble and point-light effects; those
  are not hosted.
- Infinity material: `Pattern_Infiniti` is a 256x4 color ramp (black, navy,
  purple, pink, cream) that the first version ignored. It is now sampled
  through UV1. `p_PatternScalePosition` is read as UV1 scale (-1.4429, 30)
  and offset (0.3671, 0.03). It is weighted into regions A and B by
  `p_PatternChannelScale` (0.85, 1), tinted by `p_PatternColor` and shaded
  by the detail channel. Every one of those parameter meanings is an
  UNVERIFIED guess at the stripped `Master_Gun` graph. The gun now shows a
  multicolor gradient instead of flat white; it has not been compared with
  the game.

## 2026-09-25: Weapon part rolls from installed balance data (items before UI)

The maintainer chose to build the item/part layer before the inventory and
HUD. Item cards need a gun's name, rarity, parts and stats, and a UMG
rebuild of BL2's UI was chosen over running its Scaleform movies.
`UI_HUD.HUD` and `SharedWillowInventory` `SwfMovie`s exist in `Startup.upk`.
SWF playback remains an optional later benchmark.

`tools/weapon_recipe.py` reads a `WeaponBalanceDefinition` through
`ow-package --properties` with an array schema; parser code is unchanged. It
follows `BaseDefinition` to the root, whose `InventoryDefinition` is the
`WeaponTypeDefinition` and whose `Manufacturers[0]` is the manufacturer. It
merges each balance's `WeaponPartListCollection` per slot, filters by game
stage, weights, rolls from a seed and writes a JSON recipe: parts, gestalt
fragments, material instance and name. `filter_gestalt_gltf.py --recipe`
builds that roll's mesh.

Decoded and used:
- `WeightedParts` entries index `ConsolidatedAttributeInitData` for min/max
  game stage and weight.
- Rarity weights such as `GD_Balance.Weighting.Weight_1_Common` are
  `ValueFormula` Multiplier 100 x Level 1 ^ Power 1, clamped at a 100 minimum.
- Name parts carry `Priority`. Manufacturer variants carry an `Expressions`
  entry `Weapon_Is_<Maker> == 1`. For example, "Xtra Fast" is the Bandit
  spelling of Vladof's "Rapid".

For `Pistol_Vladof_5_Infinity` the chain is `Pistol_Vladof` (Additive), then
`_2_Uncommon`, `_3_Rare`, `_4_VeryRare` and `_5_Infinity`, all Selective.
Barrel and material are fixed, so the Infinity's paint does not roll. Grip,
sight, element and accessory roll across manufacturers. Seeds 1-5 gave
Burning, Caustic, Discharge, Angry and Discharge Infinity. Every rolled
fragment exists in the pistol gestalt.

UNVERIFIED, flagged in each recipe:
- The replacement-mode semantics (Selective replaces enabled slots,
  Additive appends, Complete replaces all).
- The uniform pick when every candidate weighs 0. The Infinity barrel and the
  root body and grip entries do weigh 0.
- Manufacturer grade restrictions are ignored.
- The name rule: highest priority, ties broken by the seed.
Recipes have not been compared with in-game drops.

Also recorded for the weapon pass: `WeaponType_Vladof_Pistol` gives
FirstPersonMeshFOV 45, PlayerViewOffset (20, 4, 2), FireRate 0.125,
ClipSize 20, Spread 2.1 and the WeaponKick values. The separate 45 degree
weapon FOV likely explains the oversized gun in the host view.

Automated: `python tests/weapon_recipe_test.py` 5/5 on synthetic data.

Follow-up, part weights and item stats (same day):

- Weights: the maintainer expected every part to carry a drop weight. In the
  cooked `Pistol_Vladof.PartList`, `ConsolidatedAttributeInitData` is
  [1, 100, 0]. Every grip, sight and body entry points at the 0 with no
  InitializationDefinition. Elements and accessories do use rarity formulas
  (`Weight_1_Common` = 100 and so on). What BL2 does with an all-zero slot is
  native code; the host picks uniformly, still UNVERIFIED. The OpenBLCMM dump
  oracle is not installed on this machine, so no in-engine cross-check was
  made.
- `tools/weapon_stats.py` evaluates a recipe into item-card numbers. Base
  values come from the weapon type (`InstantHitDamage`, `FireRate`,
  `ClipSize`, `ReloadTime`, `Spread`). Damage is `Init_WeaponDamage` = 8 x
  `Att_UniversalBalanceScaler` ^ `WeaponLevel`, minimum 5, times the type's
  1.45. The scaler resolves through a ConstantAttributeValueResolver to 1.13.
  Part `WeaponAttributeEffects` are added, along with the type's
  `AttributeSlotEffects` at the summed `AttributeSlotUpgrades` grade.
  `Weapon_Is_<Maker>` operands are 1 only for the weapon's own manufacturer.
  Infinity seed 1 at level 30: damage 649, fire rate 8/s, magazine 1, shot
  cost 0, spin-up 0.55 s. The combination (base + PreAdd) x (1 + Scale) +
  PostAdd with a 0 clamp, the grade sum, the unapplied balance manufacturer
  grades and the missing accuracy-percentage conversion are all UNVERIFIED.
  No value has been compared with an in-game item card.
- `attribute_value` now applies BaseValueScaleConstant to formula and
  attribute bases too, and resolves attribute operands from a supplied map.

Automated: `tests/weapon_recipe_test.py` 5/5 and `tests/weapon_stats_test.py`
2/2, synthetic.

Follow-up, cross-checked against OpenBLCMM (same day):

The maintainer approved downloading OpenBLCMM. Installed outside the repo
at `C:/Users/yorad/Tools/OpenBLCMM/`:
- OpenBLCMM v1.4.1 (`OpenBLCMM-1.4.1-Windows.zip`, SHA-256
  `bbe9d09a3373de7f20b2f138b865baed762f2a8e6ef9b50738966f4095bc4000`) from the
  official BLCM/OpenBLCMM release.
- Datapack `blcmm_data_BL2-2023-04-20-01.jar` (SHA-256
  `8bf07971904ed9d511586e11adcc4e676fbeda546994b439456860fcce2457bc`) from
  BLCM/OpenBLCMM-Data.
- Its `data.db` matched the shipped `.sha256sum` and was extracted to
  `%LOCALAPPDATA%/OpenBLCMM/extracted-data/BL2/`, where `tools/blcmm_dumps.py`
  looks for it.
No OpenBLCMM code is used and no dump text enters the repository
(THIRD_PARTY.md already records this relationship). Subobject names in the
dumps use a colon (`Pistol_Vladof:PartList`).

Results:
- Part lists agree. All 45 part-list slots across the five Infinity-chain
  balances match our decode exactly: part order, per-part `Manufacturers`
  overrides, stage and weight indices, `ConsolidatedAttributeInitData`
  constants and `PartReplacementMode`. The game itself holds weight 0 for
  every grip and body.
- A clamp bug in our evaluator was fixed. Cooked data omits false booleans;
  the dumps show `RangeRestriction.bEnableMinValueRestriction=False` on
  `Weight_*` and `Init_WeaponDamage`. We had applied those minima anyway,
  which flattened `Weight_2_Uncommon` (10) and `Weight_4_Rare` (1) to 100.
  Restrictions now apply only when enabled; the formula also adds `Offset`
  and honours `ValueFormula.bEnabled`. A definition with another
  `BaseValueMode` or an enabled `ConditionalInitialization` is reported as
  unresolved instead of guessed.
- Manufacturer weight overrides now apply only when an entry names the
  weapon's manufacturer. Entries with `Manufacturer=None` are not wildcards;
  as wildcards they would make every `DefaultWeight` formula unused, whereas
  BL2 elemental-chance mods work by editing those formulas. UNVERIFIED.
- Resulting Infinity odds: element None 76.9%, Fire, Shock and Corrosive
  7.7% each; accessory None 74.1%, the other seven 3.7% each; sight 12.5%
  each of eight. Level-30 damage over seeds 1-8 is 649-835. No value has
  been compared with an in-game card.

## 2026-09-25: Inventory component uses rolled weapon recipes (host)

`UOpenWillowInventory` loads every recipe under `local/items` (or
`-owitems=<dir>`) that `tools/weapon_stats.py` has evaluated. It keeps a
backpack plus BL2's four weapon slots, and the first four items are equipped
in order. Keys 1-4 select a slot and 0 holsters; an empty slot keeps the
current weapon. The walker fires the active item's evaluated values:
- interval 1 / fire rate
- spin-up delay
- per-shot damage (no criticals, element or resistance yet)
Each item's mesh is `SK_<recipe id>`, imported by
`host/ue5/import_weapon_items.py` from its `filter_gestalt_gltf.py --recipe`
output. Only the Infinity MIC has a material approximation; other materials
get a neutral grey stand-in with a logged warning. Item rarity is the highest
part `Rarity` (`ItemRarity5_Legendary` = 5, resolved through its constant
attribute), which gives the HUD's rarity colour. Taking the max is
UNVERIFIED.

Runtime check: 8 Infinity recipes (seeds 1-8, level 30) loaded. Slot 1 fired
"Despair Infinity" at 740 per hit and 8/s; switching to slot 4 loaded "Angry
Infinity" (753, 9.1/s) with its own rolled Tediore sight on screen. The host
dummy's health was raised to 20,000 so the capture sequence survives level-30
damage.

Inventory screen (same day): `UOpenWillowInventoryWidget`, UMG built in C++
so the layout is reviewable text rather than a Blueprint asset. It opens with
I; Tab stays the dev map selector. The layout follows BL2's inventory:
equipped slots and backpack on the left in rarity colours, and an item card
with level, manufacturer, damage, fire rate, reload, magazine, spread,
element, "Consumes no ammo" and spin-up. Stats are compared against the
targeted slot, green up and red down, with lower reload and spread counted as
better. Clicking a slot targets it; clicking a backpack item equips it there.
Fonts, frames and icons are host stand-ins, not BL2's Scaleform art. The
module now depends on UMG, Slate and SlateCore. Captures now request the UI
layer so screenshots include UMG widgets. The unattended sequence adds
`OWCombat_6_Slot4` and `OWCombat_7_Inventory`; the latter was inspected and
shows the card for "Extended Infinity" (835, 7.3/s) compared with slot 1.

Item level (same day): `weapon_stats.py --level` now defaults to the recipe's
`game_stage`, the level it was rolled at. `Init_WeaponDamage` scales by
`1.13 ^ WeaponLevel`; for Extended Infinity (seed 5) the tool gives 24, 246,
835, 9,621 and 141,555 damage at levels 1, 20, 30, 50 and 72. The level-50
value looks high for a BL2 pistol. The likely suspect is the summed slot-grade
bonus (+3% per WeaponDamage grade), whose combination rule is UNVERIFIED. A
real item card at a known level is needed to calibrate.

## 2026-09-26: Run BL2's real HUD movie (option 1), benchmark players first

The maintainer chose to run the game's own HUD movie over rebuilding it,
"as true to game as possible even if harder". Observed in `Startup.upk`
(export 46064, `GFxUI.SwfMovie UI_HUD.HUD`):
- `SourceFile` `..\..\WillowGame\Flash\UI_HUD\HUD.swf`, timestamp 2012-08-01.
  `RawData` holds a 131,240-byte `CFX` file: zlib-compressed Scaleform SWF,
  version 9, 336,607 bytes uncompressed.
- The stage is 1280x720 at 24 fps.
- Tag census: 353 sprites, 13,446 PlaceObject2, 389 frame labels, 314 shapes,
  81 edit texts, 737 DoAction plus 22 DoInitAction and no DoABC, so the
  scripts are ActionScript 2. It also has 25 Scaleform DefineExternalImage2,
  164 DefineSubImage and one DefineCompactedFont, and 12 ImportAssets2 tags
  (`gfxfontlib.swf` fonts `$WillowBody`, `$WillowHead`, `$WillowCompact`, and
  `SharedWillowComponents.swf`).
- Its art is 17 separate `UI_HUD` `Texture2D`s: DXT1/DXT5, power-of-two
  padded, including the 1024x1024 atlas `texture1`. All 17 decode with
  `ow-package --texture` to ignored `local/ui/tex`.

Observed tag layouts (from the bytes; no Scaleform code consulted):
- DefineExternalImage2 (1009): u32 character id, u16 format (13 in every
  record), u16 target width, u16 target height, u8-length export name, then
  u8-length file name. Example: id 0xB8 -> `HUD_IB8.tga`, 389x14; the
  matching texture is `UI_HUD.HUD_IB8`, 512x16.
- DefineSubImage (1008): u16 id, u16 image id, then u16 x1, y1, x2, y2.
- UNVERIFIED: the first 1009 record, the atlas `texture1.tga`, reads
  `01 00 09 00` where the others hold a u32 id. The sub-images reference
  image 1. How that id relates to the `-nopack` weapon placeholders, which
  also start at id 1, is unresolved.

Policy recorded in docs/LEGAL.md, "UI movies", with maintainer approval: the
movies run from the install; disassembled script listings stay local; no
transcription into project code; no Scaleform SDK or source. Next, a
timeboxed benchmark of Ruffle (MIT/Apache-2.0) and of public-domain gameswf
on a standard SWF converted locally from the installed movie. Integration
into UE5 waits for those results.

Benchmark progress (same day):
- `tools/extract_swfmovie.py` writes a `SwfMovie`'s RawData. The value starts
  24 bytes (the UE3 tag header) plus a u32 count after the property's
  reported offset; checked against the CFX signature at byte 256 of
  `UI_HUD.HUD`.
- `tools/gfx_to_swf.py` turned the installed HUD into a 1,067,262-byte
  standard SWF under `local/ui`. It converts 16 external images and 164
  sub-images to `DefineBitsLossless2`; the 8 `-nopack` weapon-icon slots,
  filled by the game at runtime, become transparent. It drops
  `ExporterInfo` and `DefineCompactedFont`.
- Ruffle nightly-2026-09-26 loads it (`Loaded SWF version 9, resolution
  1280x720 @ 24 FPS`). It then fails to fetch `../gfxfontlib.swf`: the
  `--base` argument must be a `file:///` directory URL.
- The font library is `UI_FontsEn.FontsEn` (`FontsEn.swf`, CFX version 8). It
  holds three `DefineCompactedFont` tags only: WillowBody, "Compacta Bd BT"
  and "Chintzy CPU BRK". Those fonts are licensed to the game and load from
  the install only. Text needs this Scaleform font format decoded into
  standard `DefineFont3`.
- `SharedWillowComponents.swf`, which the HUD imports, is not in
  `Startup.upk`; its package is not yet found.
- Nothing rendered yet. Window capture of Ruffle's Vulkan surface via
  PrintWindow came back blank, and a screen-copy capture was discarded
  because it caught other desktop windows. The HUD's clips are also expected
  to stay hidden until game code drives them, so a visual check needs a host
  harness that calls into the movie.

## 2026-09-26: HUD movie renders in Ruffle with game fonts, library imports and localized text

Continues the HUD benchmark above. All of this is in the locally converted
copies under ignored `local/ui/run`; the install is untouched.

Observed and implemented (layouts read from the bytes; no Scaleform code or
SDK consulted):
- `SharedWillowComponents.SharedWillowComponents` and
  `SharedComponents.ConsoleComponents` are in `WillowGame.upk`, not
  `Startup.upk`.
- Packed atlases: a DefineExternalImage2 whose bytes 2-3 are `09 00` is read as
  an atlas, with bytes 0-1 a u16 atlas index. DefineSubImage's second u16 is
  that index. UNVERIFIED reading, but consistent across UI_HUD (index 1),
  SharedWillowComponents and ConsoleComponents (0 and 1). This replaces the
  earlier `--pack-texture` guess.
- DefineCompactedFont (1005) is decoded by `tools/gfx_compacted_font.py` and
  written as DefineFont3. The layout is documented in that file's docstring.
  Glyphs were checked by contour closure and bounds on every ASCII glyph, and
  by rendering. From `FontsEn`: WillowBody 293 glyphs, 28 skipped; "Compacta
  Bd BT" 232, 10 skipped; "Chintzy CPU BRK" 40, none skipped. Skipped glyphs
  use edge-word bit 0 (accented Latin Extended and some quotes). That encoding
  is not understood, so they are emitted empty and listed in the report.
- Font aliases: the font library's sample texts read `$Alias = Font Name`, and
  each font is also exported under its alias (`$WillowBody`, `$WillowCompact`,
  `$WillowTechNumbers`).
- `--localization`: `$File.Section.Key` tokens are replaced from the install's
  `.int` files, with `Patched*.int` overriding its base file. None of the HUD's
  static DefineEditText strings use tokens. All 40 HUD tokens are ActionScript
  ConstantPool/Push strings (for example
  `$WillowMenu.HUD.EnemyLevelAbbreviation`), which Scaleform translates when
  script assigns them. They sit in straight-line frame scripts with no
  branches, functions, `with` or `try`, so each string is rewritten in place
  and only its action's length changes. Streams containing any of those actions
  are left alone and counted as `script_skipped` (0 in the three movies). In
  Ruffle, the XP bar's level label now reads `LV` instead of a clipped `$Willo`.
- `--inline-font-imports` works around a Ruffle limitation read from its
  source (`core/src/loader.rs` `load_asset_movie`,
  `core/src/display_object/movie_clip.rs` `preload`, nightly 2026-09-26). An
  imported movie is preloaded once. If it has its own ImportAssets, preload
  stops there and never resumes, so its later exports never register.
  SharedWillowComponents imports its fonts from gfxfontlib, so every HUD import
  from it (value clip, eridium counter, item cards, manufacturer logos) failed
  with "non-registered character ID". The option replaces a font-only import
  with the DefineFont3 from the already converted library, under the
  importing id, plus an ExportAssets under the import name so HTML
  `<font face="$WillowBody">` still resolves. Scaleform resolves nested imports
  itself; this changes only our converted copies. After this change the HUD
  loads with no missing characters or unknown-font warnings. `$WillowHead` is
  not exported by gfxfontlib and remains a plain import.
- `tools/hud_harness_swf.py` writes a small AVM1 wrapper, our own bytecode
  assembled from the public SWF spec. It loads a movie into `_level1` and
  exposes `ow(target, op, a, b)` to JavaScript through ExternalInterface.
  `gotoAndStop("16_9")` on the root shows the 16:9 layout: vitals, XP bar,
  ammo and grenade bars, minimap and crosshair.

Not yet verified or still open:
- Nothing has been compared to the game beyond eyeballing the layout against
  `local/ui/ref`. Bars show authoring-time fill; no host data is driven yet.
  103 "Stack underflow" warnings during the first frames are unexplained.
- About 10 "Character ID collision" errors remain. Ruffle fetches
  SharedWillowComponents once per import tag (5 times) and registers the
  exports each time. This looks harmless but is UNVERIFIED.
- No count or render agreement here claims full Scaleform compatibility.

## 2026-09-26: Prototype: BL2's HUD movie over UE5 through the engine's web browser

The maintainer chose to prototype the quick path before any native Ruffle
embedding: UE's built-in CEF browser (the `WebBrowser` engine module, shipped
with UE 5.8; no new repository dependency) shows a transparent local page that
runs Ruffle with the converted HUD, and the C++ HUD pushes Maya's state into it.
This is a stopgap to test the real movie with live game data, not the final
integration.

- `-owflashhud=<url>` on the Maya HUD (`AOpenWillowMayaHUD`) adds a
  transparent, hit-test-invisible `SWebBrowser` over the viewport. The canvas
  bars, crosshair and weapon panel are then skipped; damage numbers are still
  drawn by the host. The module must be loaded explicitly
  (`IWebBrowserModule::Get()`); `SWebBrowserView` creates no window otherwise,
  which surfaced as an immediate load error.
- `tools/hud_overlay/index.html` (our code) runs Ruffle with
  `wmode: 'transparent'`, waits until the HUD has loaded all frames before
  jumping to `16_9` (jumping earlier left `p1` missing), hides `bossModule`,
  and exposes `owHud(state)`. `tools/hud_overlay/serve.py` serves it with the
  converted movies from `local/ui/run`; `tools/run_ue_flash_hud.ps1` starts
  both.
- Clip mapping, observed in the Ruffle bench by setting frames and reading
  them back, not read from the game's scripts: `p1.health`, `p1.shield`,
  `p1.grenades` and `p1.bullets` have 100 bar frames with frame 1 full;
  `shield`/`grenades` frame 101 is `none`, `bullets` 102/103 are
  `weaponSwitch`/`noWeapon`; `p1.xpbar` frame N is N% full; `p1.character`
  frame `siren` shows the action-skill icon. Whether the game uses exactly
  these frames for a given value is UNVERIFIED.
- Only real host state is sent: Maya takes no damage and has no XP, grenade
  or magazine tracking yet, so vitals are full with empty number fields,
  grenades are hidden, and the ammo text is the recipe's magazine size.

Checked: a `-game` run with `-owcombattest -owcombatshots` logged "page
loaded" and "HUD movie ready" from inside UE, and the captures show the movie's
bars, action-skill icon, XP bar, ammo panel, minimap and crosshair over
Sanctuary, under the inventory screen. Not checked: frame cost of the
browser, input focus in a long play session, behaviour at other window aspect
ratios, and anything against the real game's HUD beyond layout by eye.

## 2026-09-26: What BL2's menus depend on; pause menu renders in Ruffle

Asked "how could we mass import the menu logic", measured the dependency
first. Counts come from `ow-package --exports` over the install and from
string scans of decompressed movies. The movies' script listings stay local
(docs/LEGAL.md, "UI movies").

- Movies: 914 packages scanned, 0 failures, 64 hold `GFxUI.SwfMovie` exports,
  280 distinct movies. 137 are skill/action-skill icons, about 40 are ECHO
  portraits and small icons, one is a tactical map per level, and about 30 are
  real screens. Examples: `UI_StatusMenu.StatusMenu` (inventory, skills,
  missions and map), `UI_FrontEnd_TitleMenusClik`, `UI_Options`,
  `UI_VendingMachine`, `UI_FastTravelStation`, `UI_Mission`, `UI_Trading`.
- The logic that drives them is UnrealScript. Classes whose names match UI
  patterns own 3,205 of WillowGame's 11,945 functions, including
  `StatusMenuExGFxMovie` 196, `FrontendGFxMovie` 192 and `WillowHUDGFxMovie`
  127. Add 162 in GearboxFramework, 73 in Engine, and GFxUI's 196: the
  `GFxMoviePlayer`/`GFxObject` bridge, mostly natives. This is name-pattern
  counting, so it is approximate in both directions. The functions read game
  state (inventory, item definitions, skills, missions, player controller),
  which is why ROADMAP.md places the menus in Phase 4 on top of the Phase 2 VM.
- Movie to game: the movies call named ExternalInterface functions. The HUD
  uses 13 `ext*` names; StatusMenu uses 41 in script constant pools and 55
  `ext*` byte strings overall. In Ruffle, clicking StatusMenu's Skills tab
  called `extGenericButtonClicked("skills")` on the page. No call fired at
  load, so the movie appears to wait for the game to drive it first
  (UNVERIFIED).

Converter changes (`tools/gfx_to_swf.py`):
- Import URLs have `\` replaced by `/`. StatusMenu imports
  `..\SharedWillowInventory\...`, which Scaleform on Windows accepts and a web
  player does not.
- Script localization no longer skips streams with branches. After strings
  change length, every Jump/If offset and every DefineFunction(2), With and
  Try size is recomputed from old-to-new action positions. A stream is left
  unchanged, and counted as `script_skipped`, if any distance does not end on
  an action boundary. `tests/gfx_to_swf_test.py` (synthetic AVM1) covers
  forward and backward branches, non-spanning jumps, function and Try sizes,
  the misaligned fallback, and URL normalization. It is not yet registered
  with ctest because that needs a CMakeLists.txt change. On the real outputs:
  HUD 40, StatusMenu 30 and SharedWillowInventory 4 tokens translated, 0
  skipped; 2,168 branches checked, 0 off an action boundary.

StatusMenu (with `SharedWillowInventory`, textures decoded with `ow-package
--texture`) now renders in the Ruffle bench: tab bar, localized title
"INVENTORY", close button, background. Its panels stay empty because nothing
plays the game side.

## 2026-09-26: Menus for Maya: trace Gearbox's UI code in the real game, then build to the trace

The maintainer asked to "run Gearbox's code and base off that" to get Maya's
menus working faster. Running it inside OpenWillow needs the Phase 2 VM and
the natives the menu code reaches, which is months away. Reading the scripts
and porting them is forbidden (docs/LEGAL.md, "UI movies" and clean-room rule
3). The approved route is observation: run the real game with the community
mod SDK the player already has installed, record everything the UI code does,
and build host controllers that reproduce the recording against the same
movies. The traces also become golden files for the VM later.

- The SDK is in the player's install: unrealsdk v3.2.0, pyunrealsdk v1.10.0,
  mod manager 3.8, recorded in THIRD_PARTY.md with maintainer approval. Its
  log shows ProcessEvent and CallFunction detoured, so script calls to native
  functions are hookable too.
- `tools/sdk_trace/openwillow_uitrace` is our own logging-only mod, installed
  with maintainer approval into the game's `sdk_mods` folder. When enabled
  from the mod menu, it hooks every function declared on classes inheriting
  `GFxMoviePlayer` or `GFxObject`. That covers the controllers, the Scaleform
  bridge and the `ext*` callbacks. It writes JSONL to `local/ui/traces`, the
  path taken from `trace_dir.txt` beside the installed mod. After 200 detailed
  records per function it only counts, so per-frame HUD traffic stays bounded.
- `tools/sdk_trace/summarize.py` reports classes, bridge calls and callbacks,
  and prints a per-class timeline. Checked on a synthetic trace only.

Subsequent real-game runs produced two local traces: 2,919 functions hooked,
60,200 and 37,502 JSONL records over 90.4 and 40.2 seconds, respectively,
with zero trace errors. Performance cost while enabled is not measured.
Controllers built from a trace must still be checked side by side with the
real game.

## 2026-09-26: Maya's Skills tab populated through the StatusMenu movie

The real-game UI trace under ignored `local/ui/traces` showed the host's
`SetupSkillTree` opening `skills`, then calling movie methods to set the class
portrait, points and branch names. The movie called `extInitTree` for its three
branches. A browser probe of the converted StatusMenu verified that these
methods, `SetCellVisible`, `SetInfo`, and `loadMovie` on each icon container
render Maya's data from `tools/prepare_skill_tree.py` without copying menu
script logic into the project.

`tools/hud_overlay/skills.js` waits for all three movie callbacks, then fills
Maya's 30 skill cells, action-skill art, portrait, labels and descriptions.
Its transparent HTML hit targets follow the movie clips' `getBounds` after the
opening tween so hover and branch arrows work in Ruffle. The UE host opens the
page with **K**, sends its current zero skill points and restores game input on
close. The standalone `?points=N` value is only for visual checks.

Automated extraction check: the local JSON has three branches and 30 skills;
all 32 distinct referenced movies have SWFs and converter reports, with zero
missing external textures and zero dropped tags in those reports. Visual check
in a local browser at 1280x720: all 30 icons render; hovering Mind's Eye
updates the info panel; the right arrow centres Cataclysm. The movie still
logs Ruffle character-ID collisions and AVM1 stack underflows. A UE 5.8
Sanctuary game-window capture at 1280x720 shows the populated menu with Maya's
portrait, action-skill description, three branches and icons. The first
capture at 3 seconds after opening was blank because the imported movie had
not initialized; the menu logged ready about 4 seconds after opening and
rendered in a later capture. The automated UE run also observed the page's
close route restoring the game view. Manual pointer and keyboard interaction
in the UE window, skill spending, earned points and skill effects remain
UNVERIFIED.

## 2026-09-27: Skills visual states and motion from the installed movie

The maintainer's original-game reference shows rank badges, stronger depth
between the selected and side branches, a green action-skill frame, class-mod
text when equipped, richer grade descriptions, and contextual footer text.
The converted movie already contains `SkillTreeCellController.SetState` frames
for disabled, enabled, partly invested and maxed skills (with separate kill
skill frames), plus `SetCharacter`, `TweenBranch`, the sway clip and tooltip
text. Browser probes against the local movie confirmed these methods and the
resulting colours and rank badges. No movie art was copied into the repo.

The overlay now drives those frames from a grade map, displays Phaselock as
1/1 in the current UE slice, uses the movie's imported full WillowBody font
for the badge slash, starts sway, tweens the initial branch layout, and shows
the controls that actually work. Ruffle did not apply the traced Z depth to
the side branches in the browser check, so their 2D scale and positions are
adjusted in the page. A host supplied class-mod label can appear through the
movie, but the current prototype has no class mod equipped or represented.
Current branch grades and available points remain zero. The original-game
capture's invested grades and calculated current/next grade stats cannot be
claimed for this host yet; they require actual skill state and attribute
evaluation. Pointer hover animation is an overlay effect because native
Ruffle rollover callbacks did not fire in the browser probe.

Runtime check: UE 5.8 opened the updated menu in a Sanctuary game window and
captured `OWCombat_8_Skills.png` under ignored UE `Saved/Screenshots`. The
image shows the 1/1 action badge, 0/5 first-tier badges, dimmed deeper tiers,
the receded side branches and corrected footer. The movie logged ready about
9.6 seconds after the overlay opened in this run; the scripted page close
restored the game view. This checks rendering and the close route, not manual
mouse interaction or skill spending.

The first runtime capture still showed gameplay health, ammo, level and
minimap HUD behind Skills. The host now hides both its Flash HUD viewport
widget and native HUD drawing while Skills is open and restores them on close;
the subsequent `OWCombat_8_Skills.png` capture shows none of those HUD elements,
and `OWCombat_9_AfterSkills.png` shows them restored after the page close route.

How the interface was learned, and what it rests on:
- `tools/hud_harness_swf.py` gained four ops besides get/set/call: `apply`
  (call with an argument array), `keys` (member names), `unhide`
  (`ASSetPropFlags(target, null, 0, 1)`, bench inspection only, to list class
  methods) and `forward` (installs a clip function that relays to the page
  through ExternalInterface, the page's stand-in for Scaleform's
  `SetFunction`). Ruffle ends `Enumerate2` with undefined rather than the
  spec's null, so `keys` compares with `==`.
- Method names and parameter lists (for example `SetCellVisible(BranchNum,
  TierNum, CellNum)`, `TweenBranch(BranchNum, bImmediate, TweenDuration, XPos,
  YPos, ZPos, XScale, YScale, Alpha)`) come from a local signature listing of
  the converted StatusMenu (`local/ui/as2_signatures.py`, output kept under
  `local/` per docs/LEGAL.md). No function bodies were transcribed. Cell
  states are the cell sprite's frame labels.
- The movie calls `extCellClicked(branch, tier, cell)` itself on mouse
  release (trace seq 25355), so the game does not hit-test cells.
- `tools/prepare_skill_tree.py` places cells by tier size (1 skill: column 1;
  2: columns 0 and 2; 3: all). All 28 tree cells hovered in the two traces
  match; `bCellIsOccupied` is a bool array that `ow-package` does not decode
  yet, and extending the array decoder is a sensitive-area change not made
  here. Traced branch layouts fit X = 15 + 330d, Z = -5500|d|, alpha =
  100 - 15|d| for branch offset d (two observations; UNVERIFIED beyond them).

## 2026-09-27: Host-owned skill points for Maya (level, XP, spending)

Maya's level, experience, skill points and grades now live in the UE host
(`UOpenWillowSkills`, a component on `AOpenWillowWalker`). The Skills page only
reports clicks and presents what the host sends. Skill gameplay effects are out
of scope and not applied.

Rules, and what each rests on:
- Earned points = level - 4, i.e. one per level from level 5. The real-game
  trace under ignored `local/ui/traces` (the respec run) shows a level 45
  character (`SetPlayerLevel "Level 45"`, XP `2619964 / 2715586` in both
  traces) with 41 unspent points and no grades, which is 45 - 4. That checks
  one level only; other levels and any DLC or level-cap rules are UNVERIFIED.
- XP threshold for level L: floor(60 * L^2.8 - 60). The formula is a
  community-known curve, not read from the install. It reproduces the traced
  next-level value exactly (2,715,586 for level 46) and nothing else has been
  checked, so it is UNVERIFIED. No level cap is modelled and the host awards no
  XP yet, so `-owlevel=<N>` sets the starting level (default 1, no points).
- The action skill costs one point per grade and is spent first. The trace's
  first spend was Phaselock through `extCellClicked(-1, -1, -1)`, going from
  grade 0 to 1 and from 41 to 40 points. The three trees also require it,
  because the installed root branch's first tier has PointsToUnlockNextTier 1
  (`actionSkillPoints` from `tools/prepare_skill_tree.py`). That gate is read
  from data. The trace spent the action point first, so what the game does
  with a tree click before it is not observed (UNVERIFIED).
- A tier opens once its branch holds the sum of `pointsToUnlockNext` of every
  lower tier; a skill cannot go past its max grade; every spend needs a point.

Channel: the page writes `console.log('OWSKILL {"branch":B,"tier":T,"cell":C}')`
from its own cell hit targets. It also forwards the movie's `extCellClicked` in
case the movie sees a release itself (it did not in the checks below, because
the page's targets cover the cells). The HUD's existing `OnConsoleMessage`
handler parses the line, queues the spend, applies it in `DrawHUD`, and pushes
`owSkills({points, actionGrade, grades})` back. No new binding API was needed.
The page now shows a tier as enabled only when the host rule would accept a
spend there. It also adds the action-skill hit target before the cells: the
`ActiveAbility` clip's bounds reach y=200 on the stage (probably an invisible
child), so it had covered the first tier and taken its clicks and hovers.

Automated checks:
- `OpenWillow.Skills` (new UE automation test, synthetic tree, no game data):
  points by level, action-skill gate, tier lock, max grade, no points, bad
  cells, the owSkills JSON and level-up at an XP threshold. It passed in a
  `-game -nullrhi` run.
- Replay against the real game (one-off, not committed, because it reads the
  local trace): the trace's 55 `extCellClicked` calls, replayed through
  `TrySpend` on the local Siren tree at level 45, gave the game's outcome for
  all 55. That is 41 accepted, plus 4 refusals for locked tiers, 8 for max
  grade and 2 for no points (the game spent on the action skill before any
  tree click).
- ctest 6/6, `tools/verify_packages.py` (9 packages match) and
  `tests/gfx_to_swf_test.py` (8 tests) pass. Windows Application Control
  blocked the freshly built unsigned `ow-package.exe` on this machine. So the
  ctest scripts and verify_packages ran against the main checkout's
  `ow-package.exe`, built 2026-09-22; `src/` and `CMakeLists.txt` have no commits
  since then and this change does not touch them. The UE 5.8 editor module
  build succeeded.

In-game check (UE 5.8, Sanctuary `-game` window at 1280x720, `-owlevel=45`).
Input was real OS keyboard and mouse events sent to the UE window by a local
helper script, not a person's hand, and no enemy XP was involved:
- K opened Skills, and the host logged 41 points. Ten clicks gave, in order:
  Mind's Eye refused (action skill not unlocked); Phaselock accepted (40);
  Sweet Release accepted five times (35), then refused at max grade;
  Restoration accepted (34); Elated refused (6 of 10 points in branch). There
  was one page message per click.
- The captured frame shows 34 skill points, Phaselock 1/1, Sweet Release 5/5
  in the maxed frame, Restoration 1/5, and tier 3 dimmed. After K to close
  (gameplay HUD back) and K to reopen, the menu showed the same grades, and
  another Sweet Release click was refused at max grade.
- The scripted `-owcombattest -owcombatshots -owskillshots` run still completes
  at level 1 with 0 points.

Not done or not verified: grades are lost when the session ends (no save).
Phaselock can still be cast at action grade 0, because gating it is a gameplay
effect left for the next step. The HUD shows the XP fraction but no level
number (the page does not wire `levelText`). Nothing here is compared with the
real game's menu beyond the trace replay and the by-eye capture.

## 2026-09-27: Skill-tree branches slide in a row, as traced (no wrap-around)

Maintainer report: the Skills trees "rotate fully" instead of the selected
tree popping forward. The page had cycled the three trees modulo 3. The
real-game UI trace (local, ignored) shows what the game does on each arrow:
`extGenericButtonClicked("arrowright"|"arrowleft")`, then the movie's own
`BubbleSortBranchDepths(n)` with n = the new front branch (1-based), then
`TweenBranch` for all three branches. For offset d from the selected branch:
X = 15 + 330d, Y = 17, Z = -5500|d|, scale 100, alpha 100 - 15|d|, duration
0.3 s. Observed for Harmony and Cataclysm in front (Motion at X -645,
Z -11000, alpha 70 in the latter) and for Motion in front. So the trees keep
one row order and slide as a unit; the front tree comes forward in depth.

`tools/hud_overlay/skills.js` now sends those values and calls
`BubbleSortBranchDepths`. Selection is clamped at the ends. The trace never
pressed an arrow at an end, so wrapping there is unobserved (UNVERIFIED).

Ruffle ignores the Z coordinate, so the page projects it in 2D: scale by
f = 13200 / (13200 + |Z|) about a parent-local centre (-209, 12). Both
constants were fitted to the maintainer's real-game capture, after removing
the global offset and 1.087x scale of the game's 3D menu plane (fitted from
two Harmony cells at Z = 0, residual under 2 px). That gives Cataclysm at
70.6%, tucked partly behind Harmony as in the capture. The fitted Cataclysm
cells land within 4 and 9 px of the capture, but three samples of the
horizontal centre scatter by about +/-20 px. The game's 3D plane may also
rotate the side trees, which a 2D scale cannot show. Two steps back (f = 0.545)
has no reference capture.

Checked in headless Edge with Playwright (local bench under ignored `local/`)
at 1280x720: arrow keys slide the row, the front tree has the highest depth
after each move, and a second right arrow at Cataclysm changes nothing. The UE
5.8 check was attempted but not completed: the OS-input helper could not bring
the UE window forward, so its keys went to another application. That run was
stopped and its captures deleted. The UE view of this change is UNVERIFIED
until the maintainer checks it by hand.

## 2026-09-27: Skill info box, footer and progress band from install data

Toward a 1:1 Skills tab. Everything below is decoded from the install or
observed in the local real-game UI trace; no Gearbox script logic was read.

Stat text. `SkillDefinition.SkillEffectDefinitions` (struct array) and
`SkillEffectPresentations` (object array) decode with the reader's existing
`--array-schema` option; no reader change was needed. An
`AttributePresentationDefinition` holds the label ("Melee Damage: $NUMBER$")
and number flags. The install's `GD_Siren_Skills.int` overrides the English
strings, under keys like `[Cataclysm.Immolate:AttributePresentationDefinition_0
AttributePresentationDefinition]`. `tools/skill_stats.py` implements the rules
fitted to the traced text. Effects scale as base + per-grade *
floor((grade - start) / interval). SignStyle, bDontDisplayPlusSign,
bDisplayAsPercentage (default true) and float rounding are handled. A line
without custom placement is "<number> <text>". Designer attributes resolve to
their own BaseValue (Recompense 10%), and constant resolvers are looked up
across packages (Phaselock cooldown 13 s from Startup.upk).
`tools/hud_overlay/skill_info.js` builds the info HTML as traced: description,
current block (numbers #cc6600, #00cc00 once trained to max), a "Next Level:"
block while below max, everything wrapped in #a3a3b0 while the tier is
locked, and the cyan class-mod note.

Checks against the real game (local, not committed, because they read the
trace): all 63 distinct SetInfo texts in the two traces are reproduced byte
for byte for some (grade, locked, bonus). That includes eight with class-mod
notes and the lock and max colours. Only 16 of Maya's 31 skills appear in the
traces. The stats of the other 15 follow the same rules but are UNVERIFIED,
notably Converge (custom placement without $NUMBER$ shows no number) and
Thoughtlock. The trace's grey Mind's Eye text before Phaselock was bought,
with 41 points available, is direct evidence for the host's
action-skill-first gate. From the maintainer's capture (Ward +30% at 2 + 4
class-mod grades, shown orange), the maxed colour follows the trained grade;
no traced text covered that. Class-mod bonus grades are supported in the page
(`owSkills({bonuses})`), but the host has no class mods, so they are always 0.

Footer. `[SkillTreeGFxObject]` in `WillowGame.int` has Tooltips_SpendPoints,
Tooltips_Overview and Tooltips_Cancel. `<StringAliasMap:GFx_*>` tokens resolve
through `DefaultGame.ini` MenuInputMapArray (Set="PC") to
`GameMappedStrings`: "[Enter] Spend Point", "[Q] Toggle Overview", "[Escape]
Close". The page shows Spend Point only when the selected skill can take a
point. Toggle Overview is left out until the overview mode exists; the traces
contain no overview use. `Action.ActionSkill`/`Action.Melee` come from the
player's bindings, not the install. F matches the traced Phaselock text; V is
the default melee key (UNVERIFIED).

Selection. In the trace, rolling over a cell sends its highlight clip to
"over" ("over_KillSkill" for kill skills) and tweens the cell to Z 200 over
0.2 s; the previous one returns to "up" and Z 0. The screen opens with
Phaselock selected. The highlight clips are `SkillRowT.HighlightC` and
`ActiveAbility.highlight`. The page now does the same and drops its CSS hover
glow. Ruffle ignores Z, so the lift itself is not visible.

Progress band. The movie's `SetBranchProgression(branch, frame)` takes a
0-based branch and sends that tree's ProgressBackground (1029 frames; 1029
wraps to 1) to the frame. The traced calls were not recorded in detail
(per-function record cap), so the mapping is measured. The lit band grows
about 0.43 px per frame, and tier-row bottoms fall at frames 173, 333, 490,
646, 815, 962. The maintainer's capture has exactly 10 trained points in
Harmony and the band at the bottom of tier 3. The page therefore puts the band
at the bottom of the deepest open tier (frame 1 while the trees are closed)
and fills part of the way toward the next tier in between (UNVERIFIED).

Rendering. The info box's embedded font is a subset without ' : + %; Scaleform
falls back to the imported font library and Ruffle does not. The page wraps
the info HTML, and re-sets the SkillName field, with the imported $WillowBody
alias. Ruffle drops an italic capital at one line wrap ("ife Orbs" in Sweet
Release); cause not investigated.

Automated: `tests/skill_stats_test.py` (4) and `tests/skill_info_test.js` (9
cases), synthetic, not registered with ctest (that needs a CMakeLists.txt
change). ctest's six scripts, `tests/gfx_to_swf_test.py`,
`tests/weapon_stats_test.py` and `tools/verify_packages.py` pass, run with the
main checkout's 2026-09-22 `ow-package.exe` as before (unchanged `src/`).
Visual: headless Edge at 1280x720 in the maintainer's capture state (Phaselock
1/1, Mind's Eye 5/5, Wreck 5/5) shows the movie highlight, green stats and
the band at tier 3; the locked and 2/5 states render as traced. Not checked in
UE 5.8 (no C++ change in this step).

## 2026-09-27: Phaselock needs its skill point; HUD shows the level

Two host gaps from the Skills work:
- `AOpenWillowWalker::UsePhaselock` now returns unless the action skill has a
  grade. The trace shows the game selling Phaselock for one point in the
  Skills tab, and its tree skills stay locked until then. What the game does
  when the key is pressed before that is UNVERIFIED; the host does nothing.
  `-owcombattest` starts at level 5 or higher and spends that point, so the
  scripted combat run still casts.
- The HUD's XP-bar caption fields are `p1.levelClassMod.level` and `.comm`
  (next to the movie's localized "LV"), found by listing the clip in Ruffle.
  The game's writes to them were not recorded in the trace. `owHud` now takes
  `levelText` and `classModText`, and the host sends its level. A headless Edge
  capture shows "LV 45" beside a 42% bar, in the style of the public BL2
  screenshot "LV 17 Hoarding War Dog". No class mod exists in the host, so the
  title stays empty.

Checked: UE 5.8 module build; `OpenWillow.Skills` passes; headless `-nullrhi`
runs log "level 5, 0 skill points, action grade 1" plus a Phaselock cast for
`-owcombattest`, and "level 45, 41 skill points, action grade 0" for
`-owlevel=45`. A key press on F before spending was not tested (no input was
sent to the machine); the maintainer should confirm it by hand.

## 2026-09-27: UI trace budget per movie member (trace mod 0.2.0)

The two existing traces lost the skill tree's `SetBranchProgression` values
and its spend flourish. `GFxObject:Invoke` had one 200-record budget for every
movie method, and 99 SetInfo calls plus hover calls spent it. The trace mod
now keeps a separate budget for each member a bridge call names (its
`Member`, `Method` or `Path` argument), and a return follows its call's
decision. Not run yet: it needs the game with the mod SDK. The installed copy
in the game's `sdk_mods` is still 0.1.0 until the maintainer replaces it. The
next recording should cover the Skills tab's Q overview, which no trace has
used, plus spends in every branch, a respec, and a class-mod equip.

## 2026-09-27: Vladof spin-up no longer delays the first shot (host)

The maintainer found the Infinity's wait before firing wrong in play. The
host held every first shot for the evaluated spin-up (0.55 s). Installed data:
`WeaponType_Vladof_Pistol.BarrelSpinMode` is `BSM_SpinUpToFullFireRate`, and
`WeaponPartDefinition` has `StartingSpinUpFireIntervalMultiplier`, 1 on
`Default__WeaponPartDefinition` and not overridden by
`Pistol_Barrel_Vladof_Infinity`. Read together, the names suggest the gun
fires at once and the barrel's spin ramps the fire interval from
multiplier x interval down to the interval, which is flat at 1. The host now
skips the wait in that mode; other modes keep the old wait. `weapon_stats.py`
adds `spin_mode` and `spin_start_interval_scale` to the card. The native
behavior, the other `EBarrelSpinMode` values and the ramp shape are
UNVERIFIED; no real-game timing was measured.

Automated: `tests/weapon_stats_test.py` 3/3 (synthetic). The 8 local Infinity
recipes re-evaluate to `BSM_SpinUpToFullFireRate`, scale 1.

## 2026-09-28: Opt-in inventory movie adapter and bounded Ruffle workaround

The inventory prototype now drives the installed StatusMenu movie using
host recipe IDs and snapshots. `prepare_inventory_movie.py` validates a
converted single-frame library and defers its import tags, preserving their
bytes. In the local SharedWillowInventory benchmark this restored the
backpack, small equipment cells and ammo panel; remaining Ruffle warnings
mean this is not general import compatibility. No package parser changes.

The launcher enables it with `-InventoryMovie`. Names still stand in for
weapon thumbnails; no Maya menu preview or full original inventory behavior
is claimed. Skills keyboard focus and spend guards are corrected. Equip
requests now have a level guard in source; moving the active weapon into
another slot selects that destination instead of leaving the mesh stale.

The independent critic rates the combined result 5/10, below the requested
8/10. Browser interaction checks and package checks pass. Windows Application
Control blocked the fresh UE module load and then the final rebuild, so the
latest host level guard is not compiled or runtime-verified. Full evidence,
benchmark command, local reference provenance and limitations are in
`docs/verification/INVENTORY_MOVIE_PROTOTYPE.md`. AI-assisted implementation;
all game-derived outputs remain local and ignored.

## 2026-09-29: Weapon card accuracy, sale value and red text from install data

The inventory card lacked the Accuracy row, the price and the red flavour line.
`tools/weapon_stats.py` now derives them from the installed data, and the host
forwards them (`accuracy`, `accuracyKnown`, `value`, `valueKnown`, `funStats`).
No package parser changes; the properties are read with the existing
`ow-package --properties`.

- **Accuracy**: `AttrPresent_WeaponSpread` remaps spread 0..15 to 100..0, so
  accuracy is `100 * (1 - spread / 15)` (orientation inferred, clamp
  UNVERIFIED). The spread input is the existing UNVERIFIED model, which does not
  reproduce the spread of any of seven real cards (Conference Call: model 3.90,
  real 4.44; Striker: 1.02 vs 1.995). Emitted with `accuracy_known = false`.
- **Sale value**: the type's `MonetaryValue` price calculator with the product of
  the parts' `MonetaryValueMod` and the level, rounded down. Integer-exact for
  5 of 7 real cards (shotgun, AR, SMG); the two launchers do not reproduce.
  `sale_value_known` is true only for the shotgun, assault rifle and SMG
  calculators; others carry the number flagged false.
- **Red text**: the title part's `CustomPresentations` line with
  `TextColor` (220, 70, 70), overridden by the installed `.int` when present. All
  nine red lines on real cards exist verbatim in the data. White stat lines are
  not derived.
- Host: items with no recipe `type` (the Infinity recipes) get a type label from
  the resolved ammo type.

Automated: `tests/weapon_stats_test.py` 11/11 (8 new, synthetic), CTest 6/6,
`verify_packages.py` counts match, UE module build succeeded. No in-game check
yet. Evidence and limits: `docs/verification/INVENTORY_CARD_STATS.md`.
AI-assisted; the real-card observations are local ignored traces.

## 2026-09-29: Inventory on Tab, header tabs, varied demo weapons, shield preview

AI-assisted. The maintainer asked for the whole inventory menu to work, opened
with **Tab**, and said local decoding of anything the menu needs is fine for
speed. That widens what is decoded (more weapon meshes and a shield mesh from
the installed game through the existing UModel path) but not where it goes: all
game-derived output (recipes, glTF, textures, preview PNGs, the gear manifest)
stays under ignored `local/`; no game file, decompiled source or third-party
code was added to the repository, and no GPL tool was used.

- **Tab.** `AOpenWillowPlayerController::ToggleMenu` (the map list's Tab) now
  hands Tab to `AOpenWillowWalker::ToggleInventory` while Maya is the pawn; the
  map list stays on other pawns and via `OWMapList`. The page closes on Tab,
  Escape or I; category cycling moved to `[` / `]` (Tab used to cycle it).
- **Header tabs.** The five StatusMenu header tabs (movie clips nav1..nav5) have
  hit boxes on both pages. Inventory and Skills switch through the intercepted
  routes `/__ow_tab_skills` and `/__ow_tab_inventory` (host: `PendingTabSwitch`
  in `AOpenWillowMayaHUD`); K / I do the same from the keyboard. Missions, Map
  and Challenges have no host data, so they are disabled with an "unavailable"
  label instead of being faked.
- **Backpack panel.** The converted movie leaves it full size and overlapping the
  INVENTORY title. The page now scales it to 0.74 and places it at
  (682, 134) in the 1280x720 stage; row positions are computed in stage pixels
  and converted to panel units. Numbers are read from the reference captures
  (host choices, not movie values).
- **Type icons.** Probed in the bench: the movie draws frames `Sniper` and
  `Rocket`; `Sniper Rifle` / `Rocket Launcher` draw nothing (the launcher card
  used to show a pistol).
- **Demo weapons.** 18 new rolled recipes (pistol, SMG, assault rifle, shotgun,
  sniper, launcher; seven manufacturers; common to legendary) rolled with
  `tools/weapon_recipe.py` / `weapon_stats.py`, filtered with
  `filter_gestalt_gltf.py`, previews from `render_weapon_previews.py`, meshes
  imported by `import_weapon_items.py` (28 items). UNVERIFIED: rarity is the
  maximum over parts; 11 recipes drop fragments supplied only by `*_None` parts
  because those decode with meaningless mesh names; preview paint is the same
  approximation as before (colours are not faithful); rolled part mixes can be
  cross-manufacturer. In-hand meshes still use a neutral grey material except
  the Infinity.
- **Backpack size.** The demo library exceeds the 12-slot base, so the host sets
  the class maximum (39) unless `-owbackpack=<12..39>`; an optional local
  `load_order.txt` in the recipe folder orders the first items so the list is
  not eight identical guns in a row.
- **Shield preview.** "The Bee" now has a rendered preview from the shared
  Hyperion shield gestalt (four Hyperion fragments, section totals agree); the
  fragment choice for this specific roll and the paint are UNVERIFIED. Relic,
  grenade mod and class mod previews exist locally but no such items are in the
  manifest, so no cell shows them.
- **Maya display.** Her two lights now carry distinct forward-shading
  priorities, which removes the editor's "Multiple directional lights" on-screen
  warning; the leftover bone-name debug logging was removed.

Automated: UE 5.8 build succeeded; in-engine `tools/test_inventory_actions.ps1`
26/26 (adds Tab open/close through the controller path, K / I tab switching, `]`
category cycling); `ctest` 6/6; `verify_packages.py` all nine packages match;
`tests/weapon_stats_test.py`, `skill_stats_test.py`, `skill_info_test.js` pass.
A local browser check (`tab_check.py`, synthetic snapshot) covers Tab, K, `]`,
the disabled tabs and the Skills-tab click. Visual: fresh 1280x720 engine
captures `OWCombat_7*`; the independent critic's score is recorded in
`docs/verification/INVENTORY_MOVIE_PROTOTYPE.md`. Not verified: a physical
keyboard Tab press (the test injects the key through the player controller),
drag and drop, the Missions / Map / Challenges tabs, the in-hand materials, and
any claim of parity with the original inventory.

## 2026-09-29: Maya's inventory preview uses the game's Idle_Inventory clip and an ink outline

AI-assisted. UModel 1590 MD5 export of `GD_Siren_Streaming_SF` `Skel_SirenBody`
and AnimSets `Base_Siren`, `Unarmed_Siren`, `Rifle_Siren` (0.9 s, 68 clips, no
failures). `Base_Siren.Idle_Inventory` (271 frames, 30 fps) is converted by
`tools/prepare_character_anims.py --anchor none` (new option: skips the
first-person camera correction; the MD5-to-UE Y-mirror check still passes) and
imported by `import_character_anims.py` onto the body skeleton
(`OPENWILLOW_CHARACTER_MESH` / `_ANIM_FOLDER`). The head follows through leader
pose (same bone names). `host/ue5/import_character_menu_look.py` adds an
inverted-hull outline material and Specular 0.15 / Roughness 0.85 on
`M_OW_Character`; both are art-direction approximations, UNVERIFIED against
BL2's stripped `Master_Player` shaders.

Visual (engine capture at DistanceCm 345): hand-on-hip idle on the same side as
the 1920x1080 reference, which also supports the earlier UNVERIFIED body
handedness. The final framing (DistanceCm 300, ScreenX 0.79) is built but its
capture run stalled at editor start-up, so it is not visually verified. The
backdrop post-process still darkens Maya with the world (suit reads brown);
that belongs to the backdrop pass. No critic re-score of this change.

## 2026-09-30: Inventory backdrop grading excludes Maya

AI-assisted bounded host rendering change; existing UModel 1590 payloads and
object identity/import paths remain unchanged. No new extraction or third-party
code. `import_character_menu_look.py` now also generates `M_OW_MenuBackdrop`
locally. After-tonemap gain, desaturation and vignette use custom stencil 247
on the menu body, head and ink hulls, with a scene-depth visibility check. The
renderer enables depth/stencil (`r.CustomDepth=3`). The display owns the
blendable; destroying it on close removes the effect. Missing material logs a
warning and leaves world grading disabled. Depth of field remains focused on
Maya. Lighting reduced from 14/18 to 4/5 lux because Maya no longer needs to
compensate for global dimming.

Visual: fresh 1280x720 engine captures show yellow suit panels rather than
brown, world dimming and the inventory idle/outline. The 300 cm capture clipped
her elbow; default distance restored to 345 cm and visually checked again.
`OWCombat_9_AfterSkills` shows gameplay colour/FOV restored after menu close.
Local before/after evidence: `local/inventory/{before,after}-backdrop.png`;
captures remain ignored. Material generation and UE 5.8 module build passed.
CTest: 6/6; package verification: all nine decoded byte/count/export-field
checks match; in-engine inventory action regression: 26/26 with Slate inputs.
Detailed runtime results and limits are recorded in
`docs/verification/INVENTORY_MOVIE_PROTOTYPE.md`.

UNVERIFIED: exact BL2 shader/lighting parity, stencil occlusion under arbitrary
camera/world geometry, temporal edge stability, and an independent critic
re-score. Self-review against the local real-game reference still finds a
brighter face, different outfit and approximate blurred backdrop/glass; the
previous independent 5.5/10 score is not updated by this visual check.

## 2026-09-30: Stock inventory target, camera-space framing and native item inspection

AI-assisted. The maintainer selected stock inventory and default Maya as the
target; the custom appearance in the older screenshot is not the appearance
target. Existing UModel 1590 exports, local SWF conversion and Ruffle
0.7.0-nightly.2026.9.26 remain the external payload path. Project code owns
stable inventory identity, host validation, layout and input adaptation.

The menu display now rotates with the camera, including pitch, so looking
up/down before opening does not tilt the display copy out of its framing.
Distance 300 cm / ScreenX 0.79 fits in fresh captures after this correction;
key/rim 1.5/3, f-stop 16 and lighter backdrop grading expose the world through
the curved glass. Affine panel layout and native glass alpha are adjusted
against the stock screenshot. Native movie favorite/trash icons replace HTML
symbols and have explicit hit targets. Drag/drop uses stable IDs and the
existing authoritative equip/unequip path, rejecting wrong gear types and
locked slots. The host starts its new-session purse at zero; this is host
state, not a decoded BL2 save or economy implementation.

`prepare_inventory_gear.py` consumes existing SDK callback observations,
keeping interleaved card transactions separate and ending at SetHeight.
Benchmark: two ignored traces, 87 gear observations, 11 unique cards,
76 duplicates; four shields, four class mods, three relics, zero grenade mods;
0.287 s. Output: `local/inventory/observed_gear.json`. No failed input reads.
Unknown types and unfinished transactions are omitted (synthetic coverage).
No package identity, rolled parts or mesh is inferred. Observed stat icons and
Flash flavour formatting are preserved, including red text among white bonuses.
Formatting is passed only to the movie TextField, never browser HTML.

Inspect reuses the existing UE preview actor and imported recipe mesh rather
than a static PNG. The host resolves the current inventory instance, clamps
orbit pitch and rate-limits capture to 10 Hz. PNG frames stay in memory; close
destroys the actor. Gear without resolved visual identity is unavailable.
The render target uses isolated channel-2 lights and manual exposure.
[Epic's image utility API](https://dev.epicgames.com/documentation/unreal-engine/API/Runtime/Engine/FImageUtils)
supports render-target readback and PNG encoding; no third-party code was added.

Verified: UE 5.8 build; 32/32 in-engine actions with Slate keys and synthetic DOM
drag/pointer events; rotated Inspect captures; currency zero; yellow suit and
unclipped elbow. CTest 6/6, all nine package byte/count/export comparisons match,
weapon-stat tests 11/11 and gear-observation tests 4/4. The initial Inspect test
used an unmapped uppercase helper key and was corrected to lowercase before
the passing run. Lighting refinement is checked separately in the verification
record. All game-derived payloads/captures remain ignored.

UNVERIFIED: 1:1 parity, physical drag input, continuous Inspect latency, original
Inspect composition, BL2 material graphs, exact 3D panel projection, stock
focus/swap and sorting behaviour, compare-card placement, unresolved gear art,
and an independent critic re-score. The old 5.5/10 score remains historical.

## 2026-09-30: Equipped transfer state and stock-sized comparison cards

AI-assisted, same local SWF/Ruffle payload path. Recorded callbacks show
equipped-to-backpack transfer, `TweenPanel` compare positions and left-origin
`TweenCards`. Enter/E on equipped now pins that source, filters compatible
backpack candidates and preserves the comparison while moving selection.
E/Enter confirms through the existing validated host equip path; Escape
restores source selection/category without closing inventory. Backpack Enter
remains a host direct-equip shortcut, not a verified stock focus sequence.

A newly consulted [stock comparison screenshot](https://www.thatgamesux.com/borderlands-2-can-there-be-too-much-loot)
confirms that large cards intentionally overlap the upper equipment/backpack
panels. The previous host-chosen 55% cards were incorrect. Recorded
75/81/81/75 scales and positions are restored; equipped source is the left
highlight card and the candidate is the right comparison card. Numeric fields
must be reasserted after the movie tween because its native completion callback
is absent. Host overlays covered by cards are hidden, including category arrows.

Verified: UE build, 35/35 in-engine actions, source preserved on selection,
268.1/289.6 card widths and both first-stat fields visible after tween; settled
capture reviewed. Browser E produces the expected candidate-ID/slot request.
CTest 6/6 (8.47 s); all nine package byte/count/export checks match. Details in
the inventory verification record. No extracted payload or reference image is
tracked. Not verified: exact 3D perspective, full stock focus behaviour,
physical mouse interaction, other sort/inspect states or independent critic.

## 2026-09-30: Inventory Maya displays the selected weapon

AI-assisted bounded host presentation change. Reuses UModel 1590's already
exported `GD_Siren_Streaming_SF/Rifle_Siren.Idle_Inventory`: 381 frames at 30 fps,
converted with the existing MD5-to-UE body reference and `--anchor none`, imported
as `Anim_InventoryRifle_Idle_Inventory`. No animation data is tracked. The
first commandlet attempt used a relative input path and failed; absolute input
import succeeded, zero errors/one reference-gathering warning. A representative
request for `Pistol_Siren` returned no matching export (0.274 s); package listing
contains Base, Rifle, RocketLauncher and Unarmed third-person sets, not that
requested name. No serialization or architecture change followed that failure.

The page reports selection through a dedicated preview message. UE resolves
the current stable inventory instance to its recipe ID before loading the
mesh; arbitrary page asset paths are not accepted. The display uses the armed
clip and existing `R_Weapon_Bone` attachment, with a matching ink hull/stencil
247. Gear selection clears the gun and restores unarmed idle. Cached-menu
reopen resets transient inspect/transfer state and reissues selection to the
new display actor; empty selection clears an old display weapon.

Verified: UE build (13.85 s); in-engine 35/35 actions, log
`run-20260930-012557.log`; weapon IDs/mesh/armed-idle load logged for pistol,
shotgun and SMG; gear logs mesh=0 / armedIdle=0. Open and comparison captures
show the gun attached and following the armed pose. CTest 6/6 (8.54 s), all
nine package checks match, JS syntax/diff checks pass; browser reopen callback
clears Inspect and reissues current selection. No new independent critic.

UNVERIFIED: exact weapon-specific hold definitions/launcher pose, material
paint for most display guns, all animation phases/long-gun clipping and parity
against a matched default-Maya original-game capture. Stock shader/3D-panel
projection gaps remain; 1:1 goal is not complete.

## 2026-09-30: Inventory continuous backpack window

AI-assisted presentation change reuses the existing UModel/Ruffle local payload
path; the project owns row selection, input routing and host validation. Seven
full backpack rows now scroll by one item instead of seven-item pages. Selection
crossing the window edge reveals the next item. Wheel input accumulates fractional
steps, reversals reset accumulation, and offsets clamp to the available list.
Page keys advance seven rows. Eight native cells are rendered under an AS2
scrollRect, leaving a clipped eighth-row preview; its HTML hit target is clipped
as well. Selecting the preview reveals that row fully.

Ruffle's existing scrollRect was checked with a small drawn-rectangle probe
before using it; no new tool or extraction architecture. Stock reference shows
seven rows and the next-row sliver. Browser measurement confirms about 9.5 px
of the eighth hit target remains visible. UE build succeeded (7.75 s), runtime
37/37 PASS (run-20260930-014550.log), including overlap ordering and top clamp.
CTest 6/6 (20.39 s); all nine decoded-package checks match. No extracted output
is tracked. UNVERIFIED: original-game wheel acceleration, bottom-edge physical
mouse interaction, stock horizontal focus navigation and independent critic.
The full 1:1 goal remains active.

## 2026-09-30: Default Maya head preview correction and crop anchors

AI-assisted inventory-only material adjustment. Existing UModel 1590 external
binary (official gildor2/UEViewer source) exports `CD_Siren_Skin_Default_SF
Mati_Default_Head MaterialInstanceConstant` with `-game=border -export -png`.
Fresh representative export: 0.773 s, exit 0, eight files / 6,508,229 bytes:
two material descriptions, two property dumps, four PNGs (head diffuse/normal/
mask plus referenced Fire_Tile). Zero failed exports or duplicates in the fresh
output; Master_Player's cooked graph remains unsupported, not reconstructed.
Output: ignored `local/external/umodel/maya-menu-head-palette-20260930`;
log `local/ue-import/inventory-head-export.log`. Parent/texture references agree
with the earlier default head export; the head property dump SHA256 matches.

The look script reads that local palette and creates MI_InventorySirenHead,
parented to the imported head instance. The existing factor-two shader is
compensated for the face, while hair uses the default dark-blue shadow colour
at half gain. Only the inventory display actor applies this material; original
mesh assets retain their materials. This is an explicitly UNVERIFIED visual
translation of the missing stock shader, not proof of original ramp math.
Engine captures show dark-blue hair instead of violet and less pale face;
body colour/ramp and matched original-game lighting still need work.

A regression from the crop change was found in fresh captures: immediate
getBounds after setting scrollRect yielded transient displaced header anchors.
The page now captures row/header anchors before setting the crop. Browser
controls sit above row one (~169-193 px), rather than beside the tabs (~52-76).
Engine open/reopen verification now asserts that relationship. No independent
critic; 1:1 menu remains incomplete.

Verified final follow-up: UE build 7.95 s; runtime 37/37 PASS,
`run-20260930-015423.log`, including header placement on open/reopen/Skills return.
CTest 6/6 (15.60 s), all nine package checks match, Python/JS syntax/diff checks
pass. Fresh capture confirms corrected header and preview palette load.

## 2026-09-30: Observed sort bindings and swap action guards

AI-assisted UI correction based on existing local UI Trace SDK observations;
no new extraction backend. In `uitrace_20260926_221202.jsonl`, seq 16508-16512
resolves GBA_SortInvForward/GBA_SortInvBackward to Page Up/Page Down and marks
Drop/Sort disabled during transfer. The page now uses those directional sort
keys, preserves the selected stable instance, shows the contextual Sort hint,
and blocks Drop/Sort while swapping. Slate test routing now includes both page
keys. The sort-mode list itself remains a host approximation: the traces do
not exercise the full stock cycle, so binding agreement is not full sort parity.

Repeated browser renders revealed an additional nonzero-scrollRect-origin
regression: native cells drifted from their HTML hit targets after sort/filter/
transfer renders. Backpack rows now use panel-local coordinates under a fixed
zero-origin mask. Twelve repeated renders leave the first native hit clip at
803.75/194.95 and HTML target at 803.75/194.9375 in 1280x720; no accumulating
offset. Engine comparison/sort checks now assert native/HTML alignment.
The first action run used reversed sort directions; the original alias return
was then inspected, directions corrected, and checks rerun. Final validation
is recorded in the inventory verification record. Full focus navigation, exact
stock sort cycle, Inspect/perspective/shader parity remain incomplete.

Final: UE build 8.06 s; in-engine 40/40 PASS (`run-20260930-020452.log`),
comparison capture reviewed; CTest 6/6 (22.16 s), all nine package checks match,
JS syntax/diff checks pass. Game-derived output remains ignored; 1:1 active.

## 2026-09-30: Backpack-origin inventory transfer

AI-assisted behavioural correction using existing original-game UI Trace SDK
observations. `_220818` seq 23550 starts equip from InventoryListPanel, seq
24230 passes the backpack item to equipped-panel StartEquip with type-cell
selection, and subsequent TweenCards records bStartedFromLeftPanel=false.
This complements the already implemented equipped-origin transfer. No new
extraction tool or asset export; UModel/Ruffle payloads remain local.

E/Enter on a backpack item now starts a pending transfer rather than equipping
immediately. The source stable ID stays pinned while choosing an unlocked
weapon slot; comparison follows its current occupant. Gear can only target its
matching slot. Confirm submits the existing host-validated equip; Escape keeps
the source in the backpack and leaves equipment unchanged. Empty slots remain
valid pending targets and show Equip rather than Swap, with no invented
comparison item. Selected previews remain on the backpack source. Clicks on
equipped slots during left-origin transfer no longer silently change its
pinned destination. Snapshot refresh only cancels a transfer if its source
vanishes, rather than cancelling every backpack-origin transfer.

Panel/card tweens now distinguish transfer direction and keep the transfer
layout for empty destinations. Both panel focus arguments follow observed
comparison calls. Existing action checks were updated to start then confirm;
new checks cover pinned source, destination comparison, cancel and empty slot.
Stock initial analogue selection, exact keyboard grid traversal and click vs
hover timing remain unverified. Full 1:1 goal remains active.

Final checks: UE build 7.84 s; runtime 45/45 PASS
(`run-20260930-021213.log`), occupied/empty right-origin captures reviewed;
CTest 6/6 (21.64 s), all nine package checks match, JS syntax/diff pass.
Animated preview can overlap the backpack; full-cycle pose/framing remains
unverified, alongside the other full-parity gaps.

## 2026-09-30: Full-loop Maya preview framing

AI-assisted presentation refinement reuses existing UModel 1590 body/animation
payloads and the project-owned MD5-to-UE converter. No extraction/import or
animation-track edit. Forward kinematics on the local converted Rifle idle
(381 frames / 30 fps) finds a stationary Root, authored Hips ranges
x -4.70..8.21 / y -11.21..16.65 cm and Head x -5.12..5.84 /
y -10.16..11.69 cm. Thus the earlier sideways movement is authored animation,
not accumulating root drift. The base unarmed idle's head y range is
-10.19..2.14 cm; these measurements do not prove exact original-game playback.

Default horizontal preview anchor moves from .79 to .86 to fit the stock
(default outfit) reference's right-side composition over the whole armed loop.
The authored motion is retained. The action runner now projects the live Head
bone throughout 13 seconds, slightly longer than the 12.7-second source clip,
and checks its normalized position stays to the right of the backpack region
and on screen. This measures the head anchor, not all silhouette pixels or
weapon-specific hold definitions.

Verified: UE build 11.49 s; runtime 46/46 PASS,
`run-20260930-021751.log`: 2,110 head samples, x .806.. .906 / y .319.. .360.
Fresh open and empty-target captures show the face clear of the backpack at
the sampled poses. CTest 6/6 (22.01 s), all nine package checks match, diff
check passes. No independent critic or matched original full-loop capture;
exact body/weapon clipping, stock initial pose/hold selection and full visual
parity remain incomplete. All local payloads/screenshots remain ignored.

## 2026-09-30: Scaleform 3D projection probe

AI-assisted developer-only benchmark, no menu runtime change. Autodesk's
primary 3D guide documents AS2 _z/_xrotation/_yrotation/_matrix3d/_perspfov as
Scaleform extensions (https://help.autodesk.com/cloudhelp/ENU/Scaleform-Help/scaleform_help/3di.html).
The current Ruffle 0.7.0-nightly.2026.9.26 bridge was tested with original
synthetic 100x100 geometry, not game art. `probe_scaleform_3d.js` is never loaded
by the inventory page; paste/run it in the ready browser, then await
`owProbeScaleform3D()`. Four cases / 405.5 ms including four 100 ms waits,
with gfxExtensions=true. Flat, _yrotation=45 and _z=-300 all remain 100x100;
ordinary _rotation=45 produces 141.4x141.4 bounds. Three cases execute but two
3D mutations have no rendered effect; ordinary rotation is the positive
control. The temporary clip is removed in finally. No output duplicates,
exports or new external tools. A first attempt to load the probe by HTTP failed;
executing its repository source through preview_evaluate succeeded.

Both existing SDK traces contain Get/SetDisplayInfo observations, but zero
nonzero Z/XRotation/YRotation SetDisplayInfo calls. Thus they do not establish
the stock projection values. This confirms a renderer support gap rather than
proving an intended transform. Do not compensate by inventing 3D parameter
values or claim the current affine presentation is 1:1. A projection adapter
requires a separate rendering/interaction benchmark and original transform
observations before replacing the current path. Other parity work can continue.

Verification: developer probe above; CTest 6/6 (8.64 s), all nine package checks
match, JS syntax/diff checks pass. No new engine run (runtime unchanged);
previous 46-action run is retained as earlier evidence, not this probe's scope.

## 2026-09-30: Preserve display-info getter output in UI observations

AI-assisted trace instrumentation fix. Existing UI Trace SDK 0.2.0 records
GetDisplayInfo's input D but omits its post-call out parameter; its return value
alone is unset. Thus earlier zero-valued getter input records do not prove a
flat original transform. Version 0.2.1 adds return object identity and out.D for
that getter only, using the existing WrappedStruct serializer, with no extra
getter invocation/property writes or new extraction tool. Normal return format
remains compatible; exhausted budgets and callback errors retain isolation.
The SDK API is referenced from primary bl-sdk/pyunrealsdk documentation; no
external source copied. Native post-hook output timing remains UNVERIFIED
until a fresh game run, not established by synthetic mocks.

Three synthetic callback-contract checks pass (completed output/identity,
unrelated return, budget/error isolation). CTest 6/6 (8.53 s); all nine package
checks match, Python syntax/diff pass. Updated the already installed local
OpenWillow tracer after checking its identity, preserving trace_dir.txt and
backing up the prior own script under ignored local/ui/tool-backups. Installed
and repository script SHA256 match. No original game is running and no new
native trace was captured. No UE runtime change or new engine run. The
projection gap and full 1:1 goal remain unresolved; other work can continue.

## 2026-09-30: Local paint pass for the selected Maliwan SMG

AI-assisted bounded visual improvement using the already benchmarked UModel
1590 exports, not another extraction backend. prepare_weapon_paint.py resolves
the known recipe's two-MIC parent chain and four existing textures; the UE
importer assigns an approximate material to its existing mesh without mesh
reimport or directory deletion. One glTF primitive has UV1. No new exports,
duplicates or external binaries. Preparation succeeded; UE commandlet completed
with zero reported errors/warnings (script execution 1.00 s). Output remains
under ignored local/ and UE Content. Leaf-name resolution is bounded to this
known export set, not proof of general cross-package material identity.

The in-engine screenshot shows blue/pale metal paint replacing neutral grey.
The missing Master_Gun graph, inferred detail channel, HDR palette compression,
packed normal/emissive semantics, pattern placement, roughness and metallic
response remain UNVERIFIED. Other weapons still use their previous materials;
this does not establish stock shader parity.

Fresh runtime run local/inventory-actions/run-20260930-023458.log: 46/46 PASS,
including 2049 live head samples across the full 13-second armed idle. CTest
6/6 (17.39 s); all nine package checks match decoded bytes/counts/export fields.
Python syntax checks pass. Native computer-use pipe was unavailable, so no fresh
original-game trace or physical input validation occurred. Self-review only;
no independent critic rerun. Full menu projection, Inspect and appearance parity
remain open.

## 2026-09-30: Explicit batch coverage for weapon paint

AI-assisted extension of the same local paint pass to explicit recipe lists.
Preparation validates all required texture/vector inputs and rejects duplicate
IDs before writing a batch. No backend or architecture change. Of 28 existing
mesh recipes, 14 support this four-texture approximation, 6 have missing/null
pattern inputs and remain unchanged, and 8 Infinity variants are excluded to
retain their previous paint. Coverage report remains ignored at
local/items/paint/coverage.json. Preparation took .055 s; 56 texture references
use 24 unique existing source PNGs (32 repeated source references, no new
exports). All 14 glTF primitives contain UV1. Batch UE commandlet completed
with zero reported errors/warnings, script execution 11.67 s. Separate per-item
texture assets currently duplicate shared source payloads locally; not a
deduplicated material library. No claim that all 14 paints visually match.

Fresh engine run local/inventory-actions/run-20260930-023958.log: 46/46 PASS,
1802 full-loop head samples, same framing bounds. Early comparison capture
still displays Preparing Shaders (2), so that image is not final shader
appearance evidence. The prior completed SMG capture remains the bounded
visual result. CTest 6/6 (20.95 s), all nine package checks match, Python syntax
and diff checks pass; 14/14 complete input validations pass, null pattern
negative check rejected explicitly. Other stock visual/input parity gaps
remain open; no fresh original-game capture or independent critic.

## 2026-09-30: Audit Maya's hold references before changing menu animation

AI-assisted read-only audit using the existing bounded package CLI. No parser,
serialization layout or engine change. WillowGame's reflected
BodyWeaponHoldDefinition.AnimSetList inner property is Core.ObjectProperty;
the audit supplies that metadata through the existing --array-schema option
and checks each reference against the actual local Engine.AnimSet export.
Nine third-person Maya holds found, zero trailing bytes in their property
streams, eight unsupported WeaponActions retained explicitly. Pistol, rifle,
shotgun, SMG and sniper reference Rifle_Siren. Launcher references
RocketLauncher_Siren; unarmed references Unarmed_Siren. Default and Blizzard
contain no own AnimSetList. This supports sharing the Rifle set across five
classes, not the complete menu action/clip selection, inheritance or IK.

Representative existing-backend benchmark: UModel 1590 from official
https://github.com/gildor2/UEViewer, local external binary. Command
`umodel -path=<CookedPCConsole> -game=border -export -md5
-out=local/external/umodel/maya-launcher-anims-20260930 GD_Siren_Streaming_SF
RocketLauncher_Siren AnimSet`: .086 s, exit 0, 9 MD5 clips / 357936 bytes,
zero duplicate files or export failures, no Idle_Inventory. A separate
`-dump ... WeaponHold_Siren_Pistol BodyWeaponHoldDefinition` benchmark took
.114 s and reported Unknown class/no supported objects despite exit 0;
that class is unsupported by UModel. No new tool or architecture change.
Do not substitute a launcher Draw or additive clip for the inventory idle.

Reproducible own audit tool writes ignored local output, .914 s for nine holds.
CTest 6/6 (8.57 s), all nine package checks match, Python syntax/diff pass.
No runtime changes, new engine run or independent critic. Native computer-use
pipe remains explicitly unavailable on recheck, so no fresh game observation.
Full projection, action selection, Inspect and visual parity remain open.

## 2026-09-30: Benchmark an independent native-art panel projection plane

AI-assisted developer-only benchmark using existing Ruffle
0.7.0-nightly.2026.9.26 and local StatusMenu payloads; no external backend,
new dependency, game-derived code or production rendering change. A temporary
second player exposes native equipment-panel art inside a CSS plane. Synthetic
rotateY(20deg), perspective 1200 px; HTML hit target shares that plane.
Native cell bounds 429.55..597.05 x / 119.8..189.3 y, transformed target
169.52 x 82.71 px. Center elementFromPoint hits the target. Snapshot confirms
the panel art and green target transform together. Benchmark completes in
2990.5 ms; 60 browser RAF intervals mean 5.97 ms/max 8 ms. This is browser
cadence, not UE render throughput or a full interaction/performance gate.
Cleanup verified: one remaining main player, main inventory ready.

The first awaited preview call timed out at 15 s. Its temporary DOM was then
observed removed; a subsequently instrumented probe reported initialization
failure. Corrected readiness to wait for all movie frames, as the main adapter
already does, and used _level1-relative bounds. Corrected probe completes with
explicit state/results and removes its temporary player in finally. Two
successful runs (3018.1 and 2990.5 ms), zero output assets/duplicates. No memory,
drag, all-cell alignment, multi-panel overlap or tween-synchronization claim.

This establishes a bounded possible rendering path, not original projection
values or 1:1 stock layout. The production affine adapter remains until real
transforms and broader interaction/performance checks support replacing it.
CTest 6/6 (8.69 s); all nine package checks match; JS syntax/diff pass. No new
engine run or independent critic because runtime unchanged. Native capture
connection is still unavailable. Full menu parity remains open.

## 2026-09-30: Phase 2 starts with a read-only bytecode disassembler (Python prototype)

AI-assisted. Prompted by the inventory-parity work: the original menu's input
code could not be read, only observed. Added `research/script_disasm.py` (prototype, in the
manner of `native_count.py`) and `tests/script_disasm_test.py` (synthetic, 11 checks). It
reads the nine code packages with the existing Python reader and decodes every script
`UFunction` without executing anything.

Established from the data: the function header layout (a `u16` local-variable array, ten
`i32`, the in-memory size, the file size), `0x53` as end of script in this build, and that
jump/skip operands are measured in in-memory bytes where each object reference is 8 bytes
(a least-squares fit gave exactly 4 extra bytes per reference). Result: 12,968 of 12,978
script functions decode exactly under structural checks (header size meets the function tail,
grammar consumes exactly the script, in-memory size equals the header, jump targets are
statement starts); the 12,978 total matches the native census. Ten functions still fail
and some operand layouts are fitted, not proven: they are marked UNVERIFIED in the source.
Full record: `docs/verification/SCRIPT_BYTECODE_DISASM.md`.

Finding that changes the approach: the backpack sort logic (`extOnChangeSort`,
`ApplySortConfiguration`) is native C++, not bytecode, so its ordering still comes from
observing the game. Script-side menu navigation (`NormalMove`, `MoveDelta`, `StartEquip`,
`IsComparing`) is readable. Not done: C++ port (touches `CMakeLists.txt` and possibly the
`Reader`, both sensitive areas, awaiting confirmation), object model, interpreter.
No game bytes committed; listings stay under ignored `local/`.

## 2026-10-01: Resume the VM handoff through diagnostic trace replay

AI-assisted. Preserve the two local Phase 2 commits and the existing uncommitted
batch CLI before continuing. Complete that batch/replay path rather than replacing
the interpreter architecture. No parser layout, bounds check, dependency or
license changes. Input validation rejects malformed scalar payloads, missing or
duplicate arguments and incompatible receiver classes; omitted trailing optional
parameters retain script defaults. Clear native logs before every case, including
cases that fail during lookup. Native/stub execution cannot count as a return match.

Replay uses fresh class-default receivers, not recorded live object state. Results
remain UNVERIFIED: the first 400 existing trace pairs yielded 25 return matches,
four mismatches, 19 blocked and 352 skipped. CTest 8/8; all nine package differential
checks match; both disassemblers remain at 12,968/12,978 structurally decoded.
Full evidence and the next state-faithful comparison are documented in
`docs/verification/SCRIPT_VM_PROTOTYPE.md`. No fresh in-game or UE validation.

## 2026-10-01: Connect item-only backpack movement to the original script VM

AI-assisted. Prefer the smallest live menu connection over replacing the entire
adapter at once: execute installed `InventoryListPanelGFxObject.MoveDelta` for
ordinary backpack Up/Down, supplying the list length and binding the provider's
native entry-kind interface. Resolve the source kind through reflected enum
identity instead of hardcoding a numeric enum value. Reuse the enum serialization
order already decoded in vm.cpp, with a bounded Reader, checked prefix/count/name
references and exact consumption. No existing package/container bounds checks
are loosened. CMake adds the independent navigation adapter to ow-core; the UE
module links the local Release libraries. No dependency/license change.

Fail on VM diagnostics, malformed input or invalid results. Serialize repeated
keys, reject obsolete selection/list replies and cancel on menu close. Synthetic
tests and direct installed-script checks establish the bridge plumbing and
bounded navigation behavior. Full original-game state, equipment/equip scripts,
sorting, empty/category entries and visual parity remain UNVERIFIED. Verification
details: `docs/verification/SCRIPT_VM_PROTOTYPE.md` and the inventory record.

Runtime acceptance: two new Slate-key checks pass through original MoveDelta
and update the actual page selection, 55 expressions each, zero diagnostics.
Full runner remains FAIL (41/48, seven failures from item-selection expectations,
a pickup cascade and missing shield data; no VM-disabled baseline). Dependent
gear/pickup passes are not parity evidence. CTest 8/8 and navigation 22/22 pass;
nine package differential checks match. Engine test used launch-only cache
fallback/D3D11 after two startup stalls; no project renderer/cache changes.


## 2026-10-01: one placed Sanctuary mover before broad world behavior

Reuse one prepared door mesh/material/convex-collision chain. Follow the installed
Matinee action's variable/data/group/track references using the owned reader,
resolve shared resources in the existing scene scope, and keep curves/bindings
under ignored local output. No new extraction backend or license decision.

Extend VM object materialisation to explicit placed exports: class defaults plus
tagged overrides at caller-supplied, established 4/8/26 prefixes. Validate the
prefix fits the export rather than scanning offsets or loosening bounds. Native
tails, resource object graphs and archetype inheritance stay outside this helper.
Use installed InterpActor lifecycle scripts and scoped timer natives; reject
loading/execution diagnostics and restore script state on failure. No script
listing or original game logic is transcribed into project code.

The host evaluates the installed movement keys and promotes only the bound
component to movable at runtime, restoring its pose/mobility on failure or
shutdown. E input is a developer activation path. Mission/Kismet activation,
Ak-event tracks, encroachment, checkpoint persistence and original-game relative
frame/Euler/auto-curve parity remain UNVERIFIED. Test synthetic state/timers and
real host collision separately; see the mover verification record.

## 2026-10-01: native mission/Kismet executors over installed data; struct-embedded arrays by reflection

AI-assisted. Context: the Sanctuary + Maya slice needs mission, behavior and Kismet logic, and
`MissionTracker`, `BehaviorKernel`, `Behavior_AdvanceObjectiveSet/MissionRemoteEvent/ActivateMission/
CompleteMission` and `SequenceOp` activation are native in this build (no script), while their definitions
are readable data. Decision: implement small native executors in `src/kismet.*` and `src/mission.*` that
read the installed definitions through the VM's reflection-typed property reader, report world-acting ops
at a host boundary instead of running them, and record every guess as `UNVERIFIED` (see
`docs/verification/SANCTUARY_RPG_MISSION.md`). Do not recreate recoverable content.

Parsing behaviour change (`src/vm.cpp`): arrays inside structs are decoded using the struct field's own
reflection, and struct declarations are taken from the property declaration rather than looked up by name in
the object's package (which only worked for structs the package happened to import; `SeqOpOutputLink.Links`
had silently decoded as empty). No bounds check was loosened: element decoding still requires exact
consumption of the tagged size and fails the whole property otherwise. `CMakeLists.txt` gained `kismet.cpp`,
`mission.cpp` and a synthetic test; no dependency or license change. Verified: CTest 9/9, nine package
comparisons, Kismet census 0 unresolved links over `Sanctuary_Dynamic`, in-engine door suite 16/16.
Unverified: all native semantics against the original game (no paired capture yet).

## 2026-10-01: behavior variable data and two VM reader fixes

AI-assisted. `BehaviorProviderDefinition` variable values are an untagged block after the tagged properties:
one entry per `VariableData` element, in sequence then variable order, with a size per `EBehaviorVariableType`
name (Bool/Int/Float/Object 4, Vector 12, DirectionVector 48, InstanceData 12, Attribute 20, UnaryMath 8,
BinaryMath 12, AttachmentLocation 32, Flag 8, Named*/AllPlayers 0). The table is FITTED: checked by exact
consumption on 32,185 provider exports (0 mismatches), reference/class checks and cross-copy consistency
(0 inconsistent), and trusted only when consumed exactly. `Behavior_CompareObject` now runs from data (its
script: equal objects follow link 0, otherwise link 1); the Fire dummy's sequence is chosen by its
`BehaviorSequenceEnableByMission` condition, whose evaluation is native, so the rule used is UNVERIFIED.

Parsing behaviour change (`src/vm.cpp`): enum bytes declared in another package now resolve through the
declaring `ByteProperty` (previously read as 0); struct values start from the `ScriptStruct`'s own default
tags (header 52 bytes, fitted; 1,275 of 1,275 structs in the code packages end exactly at the export end) and
only when that stream is consumed exactly. No bounds check was loosened. `CMakeLists.txt` gained the
`behavior-synthetic` test. Verified: CTest 10/10, nine package comparisons, unchanged mission/Kismet/door
runs apart from the new `SetSequence` action field. Not verified against the game: link-id semantics, the
once-per-event rule (duplicate links always differ by id byte; the Fire mission's `Default` event has two such
pairs, so this rule may drop real activations), and the enable-condition rule. Record:
`docs/verification/BEHAVIOR_DATA_DECODE.md`.

## 2026-10-01: weapon balances, card stats and loot pools decoded for the slice

AI-assisted. The mission pistol `MW_RockPaper_Fire` resolves through `Pistol_Maliwan` -> `_2_Uncommon` ->
`Pistol_Maliwan_2_Fire`; body, Fire element and material are fixed, grip/barrel/sight/accessory roll (4,608
combinations). Our part-list merge equals the game's `RuntimePartListCollection` (OpenBLCMM dumps) for 243 of
249 Startup balances; the 6 others differ in data the running game changed. `tools/weapon_stats.py` now
applies the weapon type's own effects and class defaults, divides by negative Scales instead of subtracting
them (FITTED: 4 of 6 observed cards fully reproduced, was 1), and emits projectiles, status chance/damage,
firing mode and card rounding. Still failing: launcher sale value, one Dahl SMG (1 damage point) and the
Bandit slag SMG (damage, magazine). `tools/loot_pools.py` reproduces 2,505 of 2,554 cooked
`ProbabilityDisplayString` shares; the target dummy has no loot in stock data, so the slice uses
`StandardEnemyGunsAndGear`. Selection/roll rules, the scale rule and rounding are native and stay UNVERIFIED.
Reader: additive `--properties-batch` CLI mode. Record: `docs/verification/WEAPON_BALANCE_DECODE.md`.

## 2026-10-01: stock slice world placement, values, Phaselock data, audio chain and NPC assets

AI-assisted. Tooling only; no parsing behaviour change. None of this is checked against the original game.

- World (`tools/prepare_slice_world.py`, record `docs/verification/SLICE_WORLD_PLACEMENT.md`): the GoToRange
  trigger is `WillowWaypoint_9`, a cylinder (357.81 / 145.31) completed by readable `WillowWaypoint.Touch`
  script; Marcus is the placed `WillowAIPawn_13` (not population-spawned, correcting the earlier census) with a
  scripted move-node walk 12 -> 26 -> 35 -> 40 and door Play/Reverse arrival events; the dummy comes from
  `PopulationOpportunityDen_13` at `WillowPopulationPoint_40`; the target Matinee `SeqAct_Interp_0` is driven
  by the dummy's own `MoveTargetForward`/`SendTargetBack` behaviours (the mission's `TargetForward`/`TargetBack`
  events have no Kismet listener); respawn follows the script rule in `GetBestPlayerPlacementPoint`. 20
  structural oracles pass. UNVERIFIED: native navigation, population spawning, Matinee frame, station activation.
- Values (`tools/slice_values.py`): XP reward percentage 0.05 (the amount is native; the candidate formula is
  UNVERIFIED); health 80 x 1.13^L (which of two constants applies is UNVERIFIED).
- Phaselock (`tools/prepare_action_skill.py`, record `docs/verification/PHASELOCK_STOCK_DATA.md`): lift 0.7 s,
  lock attribute 5 x target time scale, fade 1.1 s, cooldown pool 13 s paused while the target is held,
  Suspension +0.5 s per grade. The script was read, never run; the timeline, modifier rule and cooldown
  semantics are UNVERIFIED.
- Audio (`tools/audio_census.py`, `tools/audio_slice_chain.py`, record `docs/verification/SLICE_AUDIO_CHAIN.md`):
  16 slice events resolve to Wwise Vorbis media; containers tile exactly; ids are FNV-1 of the lower-cased
  name (16/16 events, 99/99 banks). Nothing installed decodes the media; a decoder is a new external tool and
  therefore a maintainer decision. No tool was downloaded.
- Assets (`tools/seed_slice_npc_assets.ps1`, record `docs/verification/SLICE_NPC_ASSETS.md`): Marcus, the target
  dummy and the Maliwan pistol candidate fragments extracted with UModel build 1590 and imported locally
  (12 jobs, 0 failed). Textures cross-check against our decoder; meshes are identity-only; materials are
  UModel's texture guess, not verified graphs.

## 2026-10-01: slice host loop uses stock world data

AI-assisted. Host and executor work; no package parsing change. Nothing here is compared against the original game.

- `-owquest` now reads world, NPC and audio data from ignored manifests (`-owslice`, `-ownpcs`, `-owaudio`).
  Marcus is a placed NPC whose walk is started by the installed Kismet; each move-node arrival re-enters the
  same sequence, so the door opens and closes through the installed links rather than host calls. The range
  objective uses the stock waypoint cylinder. The stock dummy spawns at its population point, the map's
  populated events attach it to the target carrier, its own behaviour events play and reverse the target
  Matinee, and the installed Destroy op removes it. Maya's health uses the recovered formula and respawn the
  decoded station selection. Dialog is looked up and logged, never played.
- Executor additions: `Kismet::eventsForOriginator`, `Mover::sequenceEvent` / `originatorEvent` / `output` /
  `advanceSequence` / `variables`, CLI `--kismet-run ... --originator <object-path>` (synthetic test case 13).
- Two host conventions, both UNVERIFIED: Matinee pose = Key(t) x Key(0)^-1 x placed pose (identical to the
  door's formula when the first rotation key is zero; chosen for the target on screenshot evidence only), and
  a door that receives the opposite request while moving turns around from where it is.
- Checks: quest suite 37/37 and resume 7/7, door suite 16/16, CTest 10/10, packages 9/9. Two earlier quest runs
  failed (35/37, 36/37) and are kept in the record. Not verified against the original game: the navmesh first
  leg of Marcus's walk (a straight line here), the dummy's spawn trigger and event order, touch semantics,
  station activation, mesh hit volumes, the dummy's health. Hand play is launched with `tools/run_quest.ps1`.
  Record: `docs/verification/SANCTUARY_RPG_MISSION.md`, "Host loop with stock world data".

## 2026-10-01: slice player side on stock data

AI-assisted. Host work; no package parsing change; nothing compared against the original game.

- The lent mission weapon is the recipe matching the mission's own `MissionWeapon` (Maliwan fire pistol), given
  at the Fire objective and shown with the imported Maliwan sample mesh. Each shot hands the held item's
  damage type path to the dummy's `OnTakeDamage` check; the host fire-damage class is removed. A normal-damage
  pistol takes the wrong-element path and has its own check.
- Hand play (`tools/run_quest.ps1`) loads `local/items/slice` with Maya at the slice gear level (8, an
  UNVERIFIED slice choice). First-person arms stay hidden until a weapon is drawn; whether the original shows
  arms when unarmed is UNVERIFIED.
- Turn-in adds the candidate XP amount (0.05 x the experience span at mission level 8 = 396; the native rule
  is UNVERIFIED) to the skills component.
- Phaselock reads `action_skill_siren.json`: lift, lock length, fade, cooldown paused while a target is held,
  miss reset, re-lock penalty, Suspension's bonus. Targeting, the cast gate and target state stay host
  stand-ins; the cooldown model and curve shapes are UNVERIFIED.
- Stock data drops no item for this mission (the dummy has no pools; the reward is XP only). A labelled
  turn-in loot stand-in (first fallback-pool seed that drops a weapon) exercises the pickup path; it is not
  stock behaviour.
- `tools/prepare_weapon_paint.py` accepts material chains without a pattern texture; the pistol decal and the
  shader channel reading are UNVERIFIED.
- Checks: quest suite 57/57 and resume 7/7, door suite 16/16, inventory suite 47 PASS / 2 KNOWN_DIVERGENCE,
  CTest 10/10, packages 9/9. Two earlier failing quest runs are kept in the record. Known gaps: Phaselock can
  lift the target into ceiling beams, XP and skill grades are not saved, health is not recalculated on
  level-up, slice guns have no inventory 3D preview. Record: `docs/verification/SANCTUARY_RPG_MISSION.md`,
  "Player side with stock data".

## 2026-10-01: progression in the quest save; health follows level

AI-assisted. Host work; no package parsing change; nothing compared against the original game.

- The quest save (`-owquestsave=`) gains an optional `progression` block: level, experience, action grade and skill
  grades (available points are written for checking only; they are derived from level and grades). A save that has
  the block replaces the `-owlevel` start level before weapons are equipped; saves without it load as before. A block
  that fails validation (experience outside its level, unknown skill, grade above its maximum, more points spent than
  earned) is rejected and the quest fails, as for a rejected mission state.
- Maximum health is recomputed with the same formula whenever Maya's level changes. Current health on a level-up is
  read from installed data: `WillowPlayerController.OnExpLevelChange` (script) calls the native
  `RecalculateAttributeInitializedState` and then runs `CharClass_Siren.OnLevelUp` =
  `GD_PlayerShared.Behaviors.PlayerBehavior_LevelUp`, whose skill definition adds `HealthMaxValue` x 1 to
  `HealthCurrentValue` (`MT_PostAdd`). The host refills to the new maximum.
- UNVERIFIED: that the pool caps the sum at the maximum; that the timed effect acts once as a heal; the 1 s guard
  between level-ups (not modelled); the argument order of `OnExpLevelChange` (taken from export order); level
  decreases (test fixtures only, treated the same). The health formula's own open constant is unchanged. The same
  skill definition also scales the action-skill cooldown; the host does not model that.
- Checks: four new first-run checks and three new resume checks (names in the record), a synthetic round trip in
  `OpenWillow.Skills` (not run in this pass). Record: `docs/verification/SANCTUARY_RPG_MISSION.md`, "Progression,
  Phaselock rules and dummy behaviours".

## 2026-10-01: Phaselock lift, target rule and cast gate from script and data

AI-assisted. Host and tooling; no package parsing change; nothing compared against the original game.

- `LiftActionSkill.BeginLifting` (read, not run) computes the lift end from the pawn's centre: a trace down
  `HeightFromGround`; on ground the end is ground + collision half height + `HeightFromGround`; a trace up the
  centre's path lowers the end to a surface minus the half height. The host now does the same with a 1 uu box on
  `ECC_Visibility` and its targets' collision bounds standing in for the cylinder (UNVERIFIED). The rule only traces
  the centre's path, and the bob is not clamped.
- Correction to the earlier record: "Phaselock can lift the target into ceiling beams" was a misreading of the
  screenshot. Measured in the lane, the nearest surface is about 460 uu above the target's centre and the lifted
  target's top stays about 160 uu below it; a beam nearer the camera hides the upper part of the view. The suite
  exercises the clamp with a labelled test fixture (an invisible blocking box), because the lane has no low surface.
- `SelectTarget`/`CanPhaseLockTarget` are followed as far as the host has state: alive, not already locked, and a
  host property standing for `Flag_Skills_CanPhaseLock`. A blocked target is not lifted; its damage is not applied
  because the amount is not recovered. The targeting range is the auto-aim data's `MaxTargetDistance`
  (`GD_Autoaim.Default`); the native `GetPreferredTarget` is not reproduced and the view ray plus 30 cm sweep stay a
  host stand-in (UNVERIFIED).
- `Skill_Phaselock.SkillConstraints` evaluators are native. The host maps weapon action to "not reloading", healthy
  to "health above 0" and treats on-foot as always met (UNVERIFIED). While-active constraints, ladders, rider seats,
  friendliness, vehicles and Resurrect are not modelled.
- `tools/prepare_action_skill.py` writes format `openwillow.action_skill/2` (constraint evaluators, the
  `CanLiftTargetIf` flag chain, auto-aim data). A /1 manifest makes Phaselock unavailable with a message; regenerate.

## 2026-10-01: dummy world behaviours run in the host; attach socket from data; dependency fixture kept

AI-assisted. Host and tooling; no package parsing change; nothing compared against the original game.

- `Behavior_Transform` and `Behavior_RegisterTargetable` are readable script. The first sets
  `WillowAIPawn.TransformType`, whose readable consumer is `GetTargetName` (the balance's per-playthrough transformed
  display name; the lookup itself is native). The second registers the pawn in the native global `TargetableList`.
  The host now runs both for the target dummy: a transform type with its target name, and a targetable flag that no
  host targeting reads yet. IntMath and ChangeInstanceDataSwitch stay logged.
- The `AttachToActor` "bone" `Target` is the holder's `SocketComponent` of that name, not a skeletal bone, so no
  carrier mesh import is needed. `tools/prepare_slice_world.py` emits its pose; the same composition convention
  reproduces the holder's own Base attachment to 0.0 uu (new oracle, 21 of 21 pass). The host puts the dummy's origin
  on the socket at attach. By eye the dummy now stands on the lane floor instead of being sunk to its knees.
- UNVERIFIED: native `SeqAct_AttachToActor.Activated` and `bUseConstructAttachment`; the transformed-name lookup rule
  and the playthrough (1 assumed); which native code reads the targetable list.
- The `GD_Episode03.M_Ep3_CatchARide` dependency cannot be satisfied from installed data: the mission lists it as a
  plain dependency, Sanctuary is reachable while that mission is still active, and the only bulk-completion path
  (the mission fast-forward) is native with a trigger set at run time. It stays a fixture standing in for save state,
  now labelled as such, logged at start and filled from the mission's declared `Dependencies` instead of a
  compiled-in name.
- Checks for the three entries above (one build, this machine): quest suite 73/73 first run and 10/10 resume
  (`run-first-20261001-223529`, `run-resume-20261001-223643`), door suite 16/16, CTest 10/10, packages 9/9. One
  earlier run failed 67/68 (`run-first-20261001-222312`): the new ceiling check found no low surface in the lane.
  Inventory suite on this machine: 45 PASS, 0 FAIL, 2 NOT_RUN (the locally seeded backpack has fewer than nine
  rows), 2 KNOWN_DIVERGENCE.

## 2026-10-01: weapon paint reads Master_Gun's parameter defaults and draws a decal layer

AI-assisted. Tooling and editor importer; no package parsing change; nothing compared against the original game.

- Master_Gun's graph is stripped, but its 43 parameter expressions survive in `Startup.upk` and decode completely
  with the owned reader (`ParameterName`, and `DefaultValue` where it differs from the class default; an absent value
  is taken from the expression class default in `Engine.upk`, and the scalar default, which serializes nothing, is
  read as zero). `tools/prepare_weapon_paint.py --reader/--package` uses them only for scalar and vector parameters
  that no MIC sets and records each value's source. Reader and UModel agree on all 116 MIC parameter values of the
  nine slice MICs; UModel exports nothing for the Material itself. This makes the two Jakobs common chains
  preparable (their B and C zone colours are the base defaults), so all five slice guns and the mission pistol are
  now painted instead of grey.
- The decal parameters are emitted and the importer draws a decal by this reading: placement UV1 x scale + offset
  with the installed texture address modes; weight = zone weights dotted with the mask channels, times decal alpha;
  colour multiplies the zone colours, or replaces them as `p_ReplaceDecal` goes to 1. Evidence is structural only:
  of six candidate placements only this one puts the clamped Hyperion stripe along the gun; the Jakobs decal and
  pattern zone weights are exact complements; two chains' decal colours normalise their decal's mean to about 1.
- UNVERIFIED: the whole paint and decal reading. `p_DecalRotate` and the flip switch are not applied; static
  parameter overrides (undecoded trailing bytes on the MICs) are unknown; the replace mode is inferred from its name.
- Checks: `tests/weapon_paint_test.py` 9 passed; imports ran for six guns ("decal used" on each); quest suite 73/73
  and resume 10/10 afterwards (`run-first-20261001-224649`). Visual, by an independent critic agent that could not
  fetch real first-person images and judged from text descriptions of the skins plus one unrelated local reference
  (so low to medium confidence): Jakobs common pistol 2/10, Maliwan fire pistol 4/10 (2/10 before the decal). Its
  main finding is that both guns show irregular blotchy patches where the real skins have clean zones, so the zone
  mask reading itself is suspect. That is open work, not a result.

## 2026-10-01: analysing the game executable is allowed; transcribed code and decompiler output stay out of the repository

Maintainer decision. AI-assisted drafting. Supersedes the rule, in earlier entries and in LEGAL.md, README and the
plan, that forbade disassembling or decompiling `Borderlands2.exe`.

- **Why:** the slice's hard remainder is native code (mission tracker, behavior kernel, auto-aim, constraint
  evaluators, experience and loot rules, Gearbox's natives, stock UE3 natives). The packages and the script do not
  describe it, and recovering each rule by capturing the running game is slow. The maintainer's priority is to get the
  port done as fast as possible. My rough estimate (not measured) is a week or two saved on the current slice and
  months across Phases 2 to 4; implementation, assets, the host and verification are unaffected.
- **What is allowed:** disassembling and decompiling the executable and its DLLs locally, scripting that analysis, naming
  functions and recovering structures, by anyone working on the project including AI assistants.
- **What is not allowed in the repository, unchanged in kind:** game files and game-derived data (now explicitly including
  decompiler output, recovered headers and analysis databases), leaked Gearbox/2K/Epic/UE3 source, any code transcribed,
  translated or mechanically converted from decompiler output. Project code is written from a behaviour note in our own
  words. Rules read from native code and not confirmed by running the game stay `UNVERIFIED`.
- **Maintainer's assessment and accepted risk:** the maintainer's view is that the rights holder is unlikely to object to a
  non-commercial port that does not distribute the game. That is an assessment, not a legal conclusion. Code derived from
  analysing a binary remains derived from it, and rights holders have removed projects that published such code. The policy
  keeps the published repository to original code and behaviour notes, and the maintainer accepts the remaining risk. The
  decompile-and-publish option was considered and not chosen.
- **Two machines:** the maintainer works on two PCs. Regenerable data is regenerated from the installed game; behaviour
  notes and code are committed; non-regenerable game-derived files (analysis databases, hand patches) move through a store
  outside the repository with `tools/private_sync.ps1` (`docs/NATIVE_ANALYSIS.md`). A private GitHub repository is possible
  but is still a copy of game-derived material on a third party's servers; that is the maintainer's call.
- **Changes:** `docs/LEGAL.md` (rules, new "Analysing the executable", contributor certification), new
  `docs/NATIVE_ANALYSIS.md`, README, CLAUDE.md, AGENTS.md, ROADMAP method note, CONTRIBUTING, CODE_OF_CONDUCT, PR and issue
  templates, the plan documents; `.claude/hooks/sensitive_guard.py` and `.gitignore` now also refuse decompiler/disassembler
  database file types; `tools/private_sync.ps1` added.
- **Follow-up, same day (maintainer decisions):** Ghidra 12.1.4 with a portable Temurin JDK 21 is the chosen tool and has
  a `THIRD_PARTY.md` entry (checksums verified against the published ones); `tools/ghidra_import.ps1` imports and
  analyses the executable headlessly. The private store is the private GitHub repository `zuhuHix/BL2_ReBased-private`.
  All time-to-completion estimates were removed from ROADMAP.md, README.md and the plan documents; ROADMAP.md now has a
  "How it's going" section stating what was done in what elapsed time, with no forecast.
- **Still open:** no native function has been analysed yet, and no analysis result is confirmed against the game.

## 2026-10-02: weapon paint from Master_Gun's compiled shader data; MIC static parameters decoded

AI-assisted. Tooling, editor importer and two read-only CLI modes (`src/cli.cpp`: `--payload-file <index> <out>`,
`--names`); no change to `src/package.cpp`, the container code or `CMakeLists.txt`, no bounds check changed. Nothing
compared against the running game.

- **Static parameters.** The bytes after a MaterialInstanceConstant's properties are its fully resolved static
  parameter set; `tools/material_static_parameters.py` decodes them exactly (sizes meet) in 631 of 631 MICs in
  `Startup.upk`. On the slice chains they only pick channels: `p_WeapClassSelect` the detail atlas channel (B for
  pistols, matching the earlier guess), `p_PatternChannel` and `p_DecalChannel` the pattern and decal channels;
  `sw_FlipDecalOnRightSide` is off. The hypothesis that undecoded static overrides caused the blotches is refuted.
- **Shader data.** `RefShaderCache-PC-D3D-SM3.upk` keeps, per static permutation of Master_Gun, the uniform expression
  set (which parameter feeds which constant and sampler) and the compiled ps_3_0 shaders. A small token reader written
  from Microsoft's public D3D9 bytecode description (`research/d3d9_bytecode.py`; its listings are game-derived and stay
  under `local/`) and a diff of three permutations give the colour model described in our own words in
  `tools/weapon_paint_model.py`: `p_Masks` holds two stacked maps (lower half zone mask, upper half highlight/shadow
  map); zone tones go Midtone -> Hilight -> Shadow and are blended over `p_DColor`; pattern and decal multiply or
  replace by squared mask weights; the decal UV is shifted, rotated by `p_DecalRotate` x pi and scaled about the
  centre; the result is multiplied by the selected detail channel.
- **The bug:** the previous reading sampled `p_Masks` over its full height, so the light/dark map became zone weights
  (the camouflage blotches). Also fixed: detail used as a tone selector, `p_DColor` and the two intensities ignored,
  decal placement and rotation, the single-channel decal path.
- UNVERIFIED: the whole reading until compared with the running game; `DISPLAY_SCALE` (0.4) chosen by eye; the
  environment reflection (`P_SimpleReflect`), emissive, digistruct and lighting are not modelled; the material
  resource words before the static parameters are not interpreted.
- Checks: `tests/weapon_paint_test.py` 25 passed (invented values), CTest 10/10, packages 9/9, six guns re-imported
  with 0 errors, quest suite 73/73 and resume 10/10 afterwards (`run-first-20261002-005221`). Visual: an independent
  critic agent compared host stills with in-game inspect screenshots from the Borderlands wiki (kept under ignored
  `local/paint_research/ref_online/` with sources): Maliwan uncommon pistol **6.5/10** (was 4), Jakobs common pistol
  **4.5/10** (was 2). Its remaining findings: a cool blue cast on bare metal and on the Jakobs wood (wood reads grey,
  not brown), the Maliwan barrel looks painted rather than chrome (no reflection term), orange slightly too wide on
  the Maliwan grip. No real-game capture yet (the game would not launch under the logged-in Steam account).

## 2026-10-02: weapon paint pass 2: texture colour space from data, reflection term; albedo checked against screenshots

AI-assisted. Tooling and editor importer only; nothing compared against the running game.

- Textures default to sRGB (`Default__Texture`); only `p_Masks` and the normal maps switch it off. The detail atlas was
  imported as linear, which washed out grime and rust. The preparer now reads each texture's SRGB flag and the
  importer and thumbnail renderer follow it. The by-eye `DISPLAY_SCALE` is removed; the shader colour is used unscaled
  (clamped to 1, keeping hue).
- The environment term (`P_SimpleReflect`, `p_ReflectColor`, `p_ReflectionChannelScale`, `p_ReflectColorScale`) is
  drawn as the compiled shader combines it: it brightens surfaces that are already bright and cannot turn a dark base
  silver.
- Re-checked from the compiled shader: the shadow amount is the clamped product of the light/dark map's green and
  `p_ShadowsIntensity`, moving the tone from Midtone toward Shadow; the zone mask is the lower half of `p_Masks`
  (97% of its texels are flat, against 74% in the upper half).
- Numeric check (unlit model albedo against in-game wiki inspect screenshots, median sRGB of matching regions; the
  references include the game's lighting): Jakobs wood (97, 90, 74) vs (91, 82, 67), Jakobs frame (173, 175, 179) vs
  (168, 166, 167), Maliwan barrel (88, 99, 109) vs (148, 160, 171) (same tint, about 40% darker; reflection and
  lighting omitted in the render). So the blue cast seen in first person most likely comes from the host's lighting
  and its fixed metallic 0.35 / roughness 0.55, not from the paint formula (UNVERIFIED).
- Checks: `tests/weapon_paint_test.py` 29 passed, six guns re-imported with 0 errors, quest suite 73/73 and resume 10/10
  (`run-first-20261002-010701`). Critic (in-game stills and thumbnails vs wiki screenshots): Maliwan 6.5/10 (same),
  Jakobs 5.0/10 (was 4.5).

## 2026-10-02: first native analysis: registration tables, query tooling, mission and behavior dispatch notes

AI-assisted. Tooling and behaviour notes only; no executor or parsing change yet. Policy: LEGAL.md "Analysing the
executable". Raw output stays in the ignored analysis folder; nothing here is listing or address.

- **Machinery** (`tools/ghidra/`, method in `docs/NATIVE_ANALYSIS.md` "Native registration and queries"): each native
  class has a table of name/function pairs (`<Class>exec<Function>`); fixed script native numbers bind by name. The
  table scan finds 6,877 natives in 770 tables, and all 199 numbered script natives resolve. A batch query names and
  decompiles functions by registered name, native number, string, callers or virtual slot (virtual natives such as
  `Behavior_*.ApplyBehaviorToContext` share one exec function and are reached through the class's virtual table).
  `tools/ghidra/class_layout.py` computes 32-bit field offsets from the packages (oracle: `Core.Object` 0x3C).
  About 15 s per run.
- **Behaviour notes** (`docs/verification/NATIVE_MISSION_DISPATCH.md`, all UNVERIFIED, read from native code, not
  confirmed in the game):
  - Behavior link id byte: a signed occasion selector; the event caller passes a filter (-1 = all links); a behavior
    chooses outputs by id, and the default output is followed only when `bSupportsDefaultOutputLink` is set. There
    is no once-per-event deduplication (a behavior reached by two links runs twice); events honour `bEnabled`,
    `MaxTriggerCount` and `ReTriggerDelay`; threads run depth-first.
  - Kismet: ops run from a stack, at most 1,000 per frame; a link's delay is the input's plus the output's; an input
    hit twice runs the op twice; an activated event fires all its outputs.
  - MissionTracker: updates are queued; an objective completes when progress reaches `ObjectiveCount`; a completed
    set makes the mission ready to turn in when `bCanCompleteMission`, else activates the next set when
    `bAutoEnableNextSet`, else waits for a behavior; `AdvanceObjectiveSet` only moves to `NextSet`. Mission event link
    ids: a census over 133 missions puts all 3,420 links in the predicted id ranges (structural only).
  - Consequence for the slice: the host fires every `Default` link on accept, which is why `TargetBack` appears 3 s
    after accepting; natively the first set is activated by the kickoff dialog's Finished output.
- Not yet implemented in `src/`; the host behaviour is unchanged. Checks: CTest 10/10, packages 9/9.

## 2026-10-02: weapon paint pass 3: shading inputs read from the shader, kept behind a flag

AI-assisted. Importer and notes only. The compiled base and light passes of Master_Gun multiply the material colour
by 0.4 (a factor most other shaders in the cache do not have) and take no specular from the material (the light
pass's specular is the engine override only). Mapped to UE5 that is base colour 0.4 x colour, metallic 0, specular
0, roughness 1 (`USE_SHADER_SHADING` in `host/ue5/import_weapon_paint.py`, UNVERIFIED). With the host's uncalibrated
Sanctuary lighting those inputs render the guns 4-7x darker than the reference screenshots, and the red-down/blue-up
tint measured on the stills (about x0.8-0.9 red, x1.1-1.27 blue against the unlit albedo) is unchanged by them, so the
tint comes from the host scene lighting. Maintainer-facing choice made by the orchestrator: the importer keeps the
pass-2 host stand-in (scale 1, metallic 0.35, roughness 0.55) until the scene lighting is calibrated; the six guns were
re-imported with it (0 errors).

## 2026-10-02: native dispatch rules implemented in the behavior, Kismet and mission executors (UNVERIFIED)

AI-assisted. Executor change; the rules are those of `docs/verification/NATIVE_MISSION_DISPATCH.md` (read from native
code, written in our own words; nothing here comes from a listing). **Every rule below is UNVERIFIED against the running
game**: synthetic tests check that the executors do what the note says, not that the note is right.

- **Behavior kernel** (`src/behavior.*`): the link id byte is read signed; `fireEvent` takes a link-id filter (-1 = all);
  every matching event entry is gated by `bEnabled`, `MaxTriggerCount` and `ReTriggerDelay` with per-process
  `TriggerCount`/`LastTriggerTime`; threads run depth-first (the first selected link continues the thread, the others
  start first); handlers return the recorded output ids (duplicates kept) and -1 is added only when
  `Context.bSupportsDefaultOutputLink` is set (read with class defaults: true on most behaviors, false on
  TriggerDialogEvent/CompareObject); the once-per-event deduplication is gone; at most 60 behaviors per thread per call
  (a capped thread resumes on the next tick: what the game does was not read). Not modelled: `FilterObject`, latent
  behaviors, the "sequence still enabled" condition on running threads.
- **Kismet** (`src/kismet.*`): queued ops form a stack (the first link's target runs next; an op already queued keeps
  its place); link delay = target input's `ActivateDelay` + output's; disabled inputs receive nothing; an input hit
  twice runs its op twice; an activated event fires all its outputs; at most 1,000 ops per `run()` (was 10,000 impulses
  over the instance's life). Not modelled: one op seeing several inputs at once, event `MaxTriggerCount` /
  `ReTriggerDelay`.
- **Mission tracker** (`src/mission.*`): mission events carry the note's link ids (status change `Default` 6 + status:
  accept 7, ReadyToTurnIn 9, Complete 10; kickoff 12/13; set activated 4 / completed 5; objective progress 3 and
  completed 2 after the set check; custom events 0, refused only when Complete). Accept no longer fires every `Default`
  link. `UpdateObjective` is one queued +1 (or a bit OR-ed in; the count of a bit mask is its number of set bits, a
  guess for `TranslateObjectiveCount`); completion at `ObjectiveCount`. Set completion: `bCanCompleteMission` ->
  ReadyToTurnIn, else `bAutoEnableNextSet` -> next set, else wait; `AdvanceObjectiveSet` only to the active set's
  `NextSet` (or the initial set while none is active); `bActivateInitialObjectiveSet`; `ObjectiveDependency` in
  `available()`. Status transitions ReadyToTurnIn only from Active, Complete only from ReadyToTurnIn. Not modelled:
  RequiredObjectivesComplete, Failed, `bRepeatable`, collection/branching sets (an error if activated), blocking sets,
  the level-load replay, the mission weapon at status Active/Complete (still lent with its objective, because the
  host's checks expect that and `IsValidMissionWeapon` was not read).
- **Host stand-ins** (`src/slice.cpp`, `src/mission.cpp`): the slice plays the kickoff (`Default` id 12) right after
  acceptance (what does this in the game is unknown), and a `Behavior_TriggerDialogEvent` selects both its outputs at
  once (Out, then Finished) because no dialog is played. CLI: `--mission-run` gains `kickoff` and `obj:<name>:<bit>`;
  `--behavior-run` gains `fire:<id>:<event>`; `--kismet-run` gains trailing `--tick <s>` steps.
- **Real data (local only)**: the Fire mission now runs as the note predicts in its section B9: no set until the
  kickoff dialog finishes, `RocksPaper_FinalObj` 0.5 s after `RockPaper_GoToRange`, `Targetable` 1 s later, no
  `RocksPaper_TargetBack` after accepting, two `TargetBack` 3 s after `Fire`, Active -> ReadyToTurnIn directly, no
  errors. Its dialog lines are the same set as before. All 28 events of the range's Kismet sequence reach the same
  host ops in the same order as before. Link-id census: 0 links outside the predicted sets.
- Checks: CTest 10/10 (new: behavior acceptance tests 1-6, Kismet test 7; changed expectations follow the stack
  order and the per-frame cap), packages 9/9. The quest suite's step 7 (`OpenWillowQuest.cpp`) now waits for the
  FinalObj set (0.5 s after the touch) instead of the objective's completion; with that change, on a clean rebuild of
  `ow-core` and the UE module: quest 73/73 first run and 10/10 resume, mover 16/16, inventory actions 45 PASS / 0 FAIL /
  2 NOT_RUN / 2 KNOWN_DIVERGENCE (unchanged baseline).

## 2026-10-02: inventory open time measured and the one-time work moved to level start

AI-assisted. Host change, no parsing change. Measured on one PC (1280x720, `-d3d11`), key press to the page showing
the open inventory: first open with the page loaded 443 ms (358-488, 4 launches) -> 234 ms (213-252, 3 launches);
repeat opens about 60 ms (unchanged); first preview of each starting weapon about 32 ms with spikes up to 1036 ms ->
about 1 ms. The cost was the inventory VM build (about 120 ms) and Maya's menu meshes (45-115 ms) in the open frame,
plus each weapon mesh loading on first preview; these now preload at level start (about 140-270 ms added there;
`-owinvnopreload` turns it off for comparisons; `-owinvopenbench` and `OWINVTIME` log lines measure it). Not fixed:
pressing open in the first ~5 s of play still takes about 5.5 s, which is Ruffle booting the converted movie and its
shared libraries (two fetched more than once); a 60 Hz page frame rate and HTTP caching made no measurable difference
and were reverted. Also: page hover selects a cell only when the pointer actually moves (UE resent an unchanged position
about 500 times per open, which re-selected an equipped cell and broke the suite), and the quest save is written only
when its text changes. No real-game open time exists yet, so nothing here is compared with the game. Details:
`docs/verification/INVENTORY_MOVIE_PROTOTYPE.md` (2026-10-02 section).

## 2026-10-02: native progression and Phaselock targeting notes (UNVERIFIED)

AI-assisted. Notes only, no code change apart from `tools/ghidra/class_layout.py` sizing Gearbox attribute properties
(it stopped at the first one before). Read locally from `Borderlands2.exe` in Ghidra and from script, written in our
own words; every rule is UNVERIFIED and each section of the notes names the in-game check that would confirm it.

- `docs/verification/NATIVE_PROGRESSION.md`: balance formulas evaluate `Multiplier x (Level^Power + Offset)`
  (`tools/weapon_recipe.py` and `tools/weapon_stats.py` put the offset outside; no slice number changes, a census is
  pending); mission XP is truncated, not rounded (395 at level 8 where the host gives 396) and the mission level is
  Sanctuary's region game stage (clamp(player level, 7, 9) on playthrough 1); the level curve is
  `max(0, trunc(60 x (n^2.8 + 7.33)) - 499)` (it reproduces the one real-game threshold on record, 2,715,586 at level
  46; the host is one point low at most levels), level cap 50; skill points `max(0, L - 4)` match the host; max health
  `max(20, 80 x 1.13^L)` matches the host (the 94 constant is never used).
- `docs/verification/NATIVE_PHASELOCK_TARGETING.md`: Phaselock takes the auto-aim strategy's instantaneous best
  target (screen-space magnetism cone, score favouring the crosshair then distance, line of sight to the aim point),
  where the host uses a view ray and a 30 cm sweep; reloading does not block the cast but putting a weapon away does;
  an injured Maya cannot cast and going down ends the lock; the lift bob is timed from the cast and smoothed by
  `VInterpTo` at speed 1 (about 16 units visible, not 30).

## 2026-10-02: progression rules from the native notes in the tools and the host (UNVERIFIED in game)

AI-assisted. Implements `docs/verification/NATIVE_PROGRESSION.md` sections 1-4. Those rules were read from native code,
and none has been confirmed by running the game. Package parsing is unchanged: `src/` and `CMakeLists.txt` were not
touched.

- **Formula order.** `tools/weapon_recipe.py` (`formula_value`), `tools/weapon_stats.py` and `tools/loot_pools.py` now
  evaluate `Multiplier x (Level^Power + Offset)`. Before, they added the offset outside the multiplier.
  - A census of the base-game packages (local only) found 228 distinct enabled formulas. 19 of them have both a
    non-zero Offset and a Multiplier other than 1: enemy health and damage, enemy and world-discovery XP, melee damage,
    class-mod bonuses, three Soldier/Mercenary skill formulas, vehicle damage and the XP curve. DLC packages were not
    included.
  - No slice number changes. The slice gear recipes and manifest are byte-identical before and after. The loot display
    check is unchanged (2,505 of 2,554 agree). Maya's health formula has no offset, and the XP curve's offset cancels
    in the reward span.
- **Level curve and cap.** `UOpenWillowSkills::ExperienceForLevel` is now `max(0, trunc(60 x (n^2.8 + 7.33)) - 499)`,
  evaluated in single precision. It was `floor(60 n^2.8 - 60)`.
  - Changed thresholds: level 2 357 -> 358, level 5 5,375 -> 5,376, level 8 20,207 -> 20,208, level 9 28,125 -> 28,126.
    Level 46 stays 2,715,586, the one real-game threshold on record.
  - The level cap of 50 applies in `AddExperience`, `SetLevel` and save restore. DLC cap increments are a TODO.
  - Float and double evaluation differ by one point at levels 17, 22, 33, 42, 45, 47 and 49. Which value the game
    gives at those levels is not known.
- **Mission XP.** `MissionXp` is now `trunc(span x percentage)` on the integer curve. Before, it rounded a span computed
  on unrounded doubles.
  - Changed amounts: stage 8 396 -> 395, 9 484 -> 483, 10 579 -> 578, 11 682 -> 681. Stage 7 stays 316.
  - The mission level is now Sanctuary's region game stage, not the slice gear level. `tools/slice_values.py` decodes
    the playthrough-1 `RegionBalanceData` entry into `world.json` `values.xp.region_stage` (default 7..9,
    WelcomeToSanctuary 8..11, later overrides).
  - The host fixes the stage as `clamp(level + boost, min, max)`, using the largest completed override or else the
    default. It does this when the walker sets Maya's level at session start and keeps the value in the quest save.
  - A manifest without the block gets a logged STAND-IN with the note's bounds.
  - The suite's Maya starts at level 8, so the stage is 8 and the reward is 395.
- **Max health** already matched the note. `SLICE_WORLD_PLACEMENT.md` 2b now records that the 94 constant is never
  used (native reading).
- **Checks.**
  - CTest 10/10 and packages 9/9 pass.
  - Synthetic Python tests pass: weapon_recipe 8, weapon_stats 19, loot_pools 9, slice_world 17.
  - The `OpenWillow.Skills` automation test passes (curve values, synthetic order and truncation cases, cap).
  - Quest suite: 75/75 first run and 11/11 resume. New checks are `mission_level_is_region_stage_fixed_at_start`,
    `level_curve_matches_tool_integer_curve` and `resume_region_stage_from_save` (the stored stage 8 is kept where a
    fresh computation at the resumed level 11 would give 9). `xp_amount_is_candidate_formula_at_mission_level` is
    renamed `xp_amount_is_truncated_rule_at_region_stage`.
  - Door suite 16/16.
  - Every rule above is still unverified in game. The note lists the confirmations: turn-in XP 395 at stage 8,
    "next level at" 358 at level 2, and the first skill point at level 5.

## 2026-10-02: ParticleSystem template reader (research)

AI-assisted. Research prototype only (`research/particle_system.py`, `tests/particle_system_test.py`, 16 synthetic
tests; not registered in CTest). Templates are delta-serialized against their archetype chain; baked distribution
tables are read as two range values plus entries of `ChunkSize` floats; `BurstList` and dynamic parameters are tagged
structs. All 17,506 non-empty baked tables in four packages fit that layout (structural oracle). Curve sampling between
table entries is fitted, UNVERIFIED. Record: `docs/verification/PHASELOCK_STOCK_DATA.md`, "Particle template reader".

## 2026-10-02: Phaselock stock presentation and targeting in the host (work in progress, no suite run)

AI-assisted. Host and tooling; no parsing change. Written but **not verified by any suite**: after the module was
rebuilt at 09:18, Windows Application Control (Smart App Control) blocked `UnrealEditor-OpenWillow.dll`
(`GetLastError=4551`, Code Integrity events 3033/3077). Per project rules nothing was done to get around it; UE work
stops until the maintainer clears it.

- Presentation (`OpenWillowPhaselockFx.*`, `OpenWillowCombatTarget.*`, `OpenWillowWalker.*`): the decoded emitter
  templates are played with plane/mesh components (no Niagara, no new module dependency). From data: hand orb at the
  0.25 s notify on `L_Weapon_Bone` with the socket offset and scale 0.35; bubble scaled by bounds radius / 66.7, 0.2 s
  intro, collapse 0 -> 0.75 over the last 2 s; point light radius 500, brightness 4, colour (96,128,255); tattoo glow
  curve over 1 s. Host stand-ins (UNVERIFIED): what each stripped material does with its textures, the tattoo mask
  channel, the screen effect as a full-view quad, UE3 brightness -> UE5 intensity, burst timing, the dummy's auto-aim
  radius/aim point. The dummy keeps its idle (it has no PhaseLock clips).
- Rules from `NATIVE_PHASELOCK_TARGETING.md` (UNVERIFIED): screen-space magnetism target choice replaces the view
  ray and 30 cm sweep; reloading no longer refuses the cast, a holstered weapon does; going down ends the lock; the
  bob is timed from the cast and smoothed.
- Tooling: `tools/prepare_phaselock_fx.py` (manifest from installed data), `host/ue5/import_phaselock_fx.py` with
  `tools/import_phaselock_fx.ps1` (textures and meshes from UModel output under `local/`), `tools/run_phaselock_shots.ps1`.
- One in-engine capture run (before the block) showed the lift, the smoothed bob (about +/-16 uu), the collapse
  reaching 0.749 and the light ramp; defects seen: an overexposed white-pink core where web screenshots show a
  violet sphere with a dark core, a large violet light pool, loop sprites not showing, and first-use texture
  compilation delaying the hand orb to +0.53 s. Fixes for some of these are written and not run.
- Quest suite: check 65's reload refusal is replaced by "reload does not refuse" and "holstered refuses"; new checks
  for the presentation, an off-crosshair target and going down. Not run. Expected totals once it can run: 79 first-run
  checks (75 baseline + 4) and 11 resume checks (unchanged).
- Resumed later the same day under the maintainer's one-rebuild rule. A normal rebuild with no source change did not
  relink (the 09:18 DLL stayed, still blocked). After the one requested source change (the stale targeting comment in
  `OpenWillowQuest.h`), the module was relinked at 10:01 and Smart App Control blocked that DLL as well
  (`GetLastError=4551`, Code Integrity events 3033/3077/3118 at 10:01:40). UE work stopped again; nothing was done to
  get around the block.

## 2026-10-02: weapon generation rules read from native code; card audit 9 of 9 with runtime data (UNVERIFIED in game)

AI-assisted. Tools and notes only: no host C++, no `src/`, no `CMakeLists.txt` change; package parsing is unchanged.
Read locally from `Borderlands2.exe` in Ghidra and from script, written in our own words in
`docs/verification/NATIVE_WEAPON_RULES.md`; every rule is UNVERIFIED in game and each section names its confirmation.

- **Attribute stack (settles the fitted rule).** The game sums PreAdd, PostAdd, positive and non-positive Scales in
  single precision and computes `(base + PreAdd) * (1 + up) / (1 - down) + PostAdd`, with no clamp; integer stats (clip,
  projectiles, shot cost, burst count) truncate. The 2026-10-01 "split" rule was right; its clamp at 0 was not. Enum
  orders and class defaults were decoded from the packages.
- **Effect order** (script): type, parts in slot order, attribute slots (activated ones; grades count every increase),
  then prefix and title. **Card rounding** comes from the presentation data (damage up, clip down, the rest half up to
  `FloatPrecision`); single precision decides the damage ceiling on one observed launcher.
- **Part pick.** An entry without a `Manufacturers` list weighs a flat 100 (1,572 of 2,473 weapon entries); stage
  windows use truncated bounds; zero weights are dropped, a duplicate keeps its later weight, and a slot with nothing
  left stays empty (not a uniform pick). **Names** are deterministic (type lists first, then parts in slot order, highest
  priority, later wins ties; class defaults priority 1, level window 1..100). **Level** = the spawn game stage
  (`bInterpolateExpLevel` default true). **Rarity** = sum of truncated part rarities looked up in
  `RarityLevelColors` (was: max). **Value**: the prefix's `MonetaryValueMod` is in the part product; this was the
  launcher value gap (inferred from script order and data; the value function itself was not resolved).
- **Runtime data.** The observed cards were captured with Gearbox hotfixes active; OpenBLCMM's dumps of the running game
  show 39 weapon objects with changed stat data, which explain the four hotfixed legendaries. `weapon_card_audit.py
  --runtime-overlay` reads them locally as an oracle input; whether the port applies hotfix data is a maintainer decision.
- **Card audit** (9 distinct cards in this machine's 2026-09-26 traces; the record's 6-card set is not on this PC):
  before (HEAD) 4 of 9 cards on the main four stats and on every printed field; HEAD rules with runtime data 8 / 7 of 9;
  read rules on cooked data 5 of 9; read rules with runtime data **9 of 9 on every printed field, name included**.
  Ablation: without single precision 8 of 9; without name parts 7 of 9 numerically; adding the slot base grade 0 of 9.
- **Changes:** `tools/weapon_recipe.py` (evaluator order and precision, `entry_weight`, `pick`,
  `choose_name_parts`), `tools/weapon_stats.py` (`combine`, effect order, slots, `rarity_of`, `present`/`display`, name
  parts in the value, launchers' calculator checked; new card fields `rarity_level`, `rarity_rating`, `rarity_color`),
  `tools/weapon_card_audit.py` (name parts and `name` check, `--runtime-overlay`, every-field counts),
  `tools/weapon_balance.py` (docstring), tests, `WEAPON_BALANCE_DECODE.md` pointer. Regenerating `local/items/slice`
  changes the slice guns' parts and names (new pick sampler and name rule); it was not regenerated in place.
- **Checks:** weapon_recipe 14, weapon_stats 25, weapon_balance 7, loot_pools 9, weapon_paint, skill_stats, slice_world
  and golden_card_compare tests pass; CTest and packages in the lane report. Not done: any in-game check, the host
  changes listed in the lane report (magazine and shot-cost truncation, HUD card rounding, E-tech colour).

## 2026-10-02: real-game ground truth: driver tooling and the first capture session

AI-assisted. Tooling (`tools/real_game/`) and records; no change to `src/`, `CMakeLists.txt`, the reader or the
host. The maintainer allowed unattended launches of the installed game for captures (orchestrator brief, 2026-10-02).
Details and method: `docs/verification/REALGAME_GROUND_TRUTH.md`; everything recorded stays under ignored
`local/realgame/`.

- **Tooling.** `tools/real_game/realgame.ps1` (run lock shared with UE runs, save backup, launch, window capture,
  scan-code keys including arrows, click/wheel/drag, QPC-stamped burst capture, `Invoke-GamePy[File]`) and
  `openwillow_realgame`, our own Library mod for the community SDK that runs command files on the game thread, with
  `block_saves()`. Command scripts: `scripts/weapon_cards.py` (spawn by balance or exact definition, weapon record,
  card trace in the uitrace row format), `scripts/phaselock.py` (per-frame lift-skill sampler, cast and weapon-call
  marks, damage immunity, `face`). `golden_cards.py` joins records, card trace and screenshots;
  `golden_card_compare.py` (written by a subagent, reviewed) evaluates `tools/weapon_stats.py` on the exact rolled
  parts; `tests/golden_card_compare_test.py` 12 synthetic tests. Smoke test: the channel answered at the main menu
  15 s after launch and the driver was removed afterwards.
- **Saves.** Backed up first; all 22 save files are byte-identical to the backup after the session (four files the
  game had written at start, character selection and one load, while the first hook version was broken, were restored
  from it). One game crash came from a command reusing an invalidated weapon reference; nothing was written.
- **Confirmed in game** (each with how; the notes' other rules stay `UNVERIFIED`):
  - level curve: `GetExpPointsRequiredForLevel` for 1-80 matches the single-precision formula at every level 1-59;
  - "next level at" and skill points `max(0, L − 4)` at levels 2, 8, 17, 70;
  - max health base `80 × 1.13^L` (health pool base value) at levels 2, 8, 17, 70; the HUD adds the Badass Rank;
  - Fire mission XP 395 at stage 8 (`MissionDefinition.GetExperienceReward`);
  - Phaselock bob: the note's sine-from-cast plus `VInterpTo` speed 1 reproduces a lifted enemy over 435 frames to
    0.001 units RMS; a cast during a manual reload starts and aborts the reload; a cast during a swap put-down is
    refused (2/2, with controls).
- **Observed, for the lanes:**
  - Phaselock presentation timings (hand orb ≈ 0.45 s after the key, not 0.25 s; dark-cored violet bubble; cast
    vignette, target burst and cyan release ring that the host lacks); skill 5.7 s = 0.7 + 3.9 + 1.1 at level 8.
  - Weapon cards (69, exact parts): every printed stat matches on 53/69 with the evaluator before `bb2a444` and 52/69 after it (fire rate fixed; one 1.75 reload now prints 1.8, the game 1.7); damage prints rounded up;
    the live weapon-type objects differ from the cooked `Startup.upk` decode (Bandit pistol magazine 36 vs 30, Dahl
    pistol 16 vs 12, Bandit shotgun 10 vs 9 and reload 4.1 vs 4.4) and no live hotfix entry touches them (source
    open); single precision fixes the 1.25 fire-rate display; the game falls back to the type's title; no level line
    at stage 1 or on mission weapons.
  - Paint: an independent critic agent scored the host 4.5/10 (Maliwan) and 3/10 (Jakobs) against real inspect
    captures (earlier 6.5 and 5.0 were against wiki screenshots): too dark even unlit, wrong Maliwan orange hue,
    no element glow.
  - Inventory open in the original game: ~126-156 ms to the first page frame (screen capture, upper bound).
- **Not done:** downed/injured Phaselock checks, the auto-aim radius thresholds, a real mission turn-in, the
  level-54 save. No UE suite was run (Smart App Control blocks the module DLL; no rebuild attempted).

## 2026-10-03: Phaselock presentation compared with the game: shader warm-up, capture clock, dark bubble core

AI-assisted (Claude). Host and tooling; no change to `src/`, `CMakeLists.txt` or package parsing. Smart App Control was
off and the module built and loaded. The host's Sanctuary dummy was compared with the 2026-10-02 game captures (a
bullymong in Three Horns). Details, causes and the remaining differences are in `docs/verification/PHASELOCK_STOCK_DATA.md`,
"Host presentation pass, second round". Frames and the side-by-side stay under ignored `local/phaselock/`.

- **Invisible hold bubble: a first-draw shader compile, not the effect data.** Every import recreates the host
  materials and compiles nothing (`-nullrhi`), so the next game run compiled them on first draw and skipped the
  translucent quads for about 2.5 s. An unchanged rerun drew the bubble on time. `FOwFxTemplate::Preload` now loads all
  Phaselock materials and meshes when the manifest loads and compiles the parent materials synchronously in editor
  builds. The first run after a fresh import then drew every effect on time (checked once, 17:55 run).
- **Capture clock.** Screenshot stalls (0.36 s, then about 0.13 s per shot) had moved the labelled shots late (the
  "0.25 s" frame was at +0.52 s). That made the arm look twice as fast as in the game and the hand orb look absent.
  `-owphaselockshots` now caps game time at 1/60 s per frame and logs each shot's actual time. With that, the arm
  timing matches the game to the eye. The stock data gives the cast clip `PlayRate` 1 (no `RateScale`); the caller's
  `SpecialMoveData` was not resolved.
- **Emitter playback (UE3 conventions, UNVERIFIED here):** spawn-time distributions are read at the emitter's time in its
  loop, and a linear sub-image layout follows the SubUV module's `SubImageIndex`. The second change removed the black
  wedge at the collapse: the end smoke now runs its frames 15 -> 0 instead of 0 -> 15.
- **Host stand-ins, chosen against the capture (UNVERIFIED):** all host FX parents render before DOF. The darkening
  modulates (`Mat_SirenOrbBlackMOD`, `_NoBias`, `Mat_SirenOrbEnergySpikesMOD`) are drawn as translucent black, because
  UE5 applies modulate apart from the additive layer. The textureless black orb uses a disc mask. `Mat_SirenGlowMOD`
  weights by alpha. The screen particle is a modulate by the colour's hue. The lock light reaches only the target,
  although its data (`LAC_DYNAMIC_AND_STATIC_AFFECTING`) says it lights the floor; the capture shows no pool. The
  hold now reads as a near-black core with a violet rim, the release as a cyan-white ring that shrinks, and
  1.2-1.5 s as a blue screen tint.
- **Open:** the game's dark blob around the raised hand at about 0.27 s and its solid blue palm orb (the host shows
  flashes and swirls); the 0.8 s intro burst draws as a white band; the screen tint starts at 1.05 s, against about
  0.82 s in game (a burst `Time` in seconds would fit; not changed); what `SphereCollapse` drives in the stripped graph.
- Also: the `OpenWillowPhaselock.h` lock-duration comment now cites the modifier stack read from native code instead
  of the old fitted rule; FX logs print mesh-particle scales with decimals and each quad's blend, sort priority and
  visibility.
- **Checks (automated):** quest suite first run PASS 79 checks / 0 errors and resume PASS 11 / 0; mover PASS 16 / 0;
  inventory actions PASS 45, FAIL 0, NOT_RUN 2, KNOWN_DIVERGENCE 2 (exit 1 from the NOT_RUN rows, as before); CTest
  10/10; `verify_packages.py` 9/9. **In-game check:** host frames compared by eye with the game captures only; no new
  game capture was made.

## 2026-10-03: host card rounding and integer truncation follow the native weapon rules (UNVERIFIED in game)

AI-assisted (Claude). Host and page only; no change to `src/`, `CMakeLists.txt`, package parsing or `tools/weapon_stats.py`.
This closes the host items the 2026-10-02 weapon-rules entry left open. The rules come from
`docs/verification/NATIVE_WEAPON_RULES.md` sections 1 and 2, which were read from native code and presentation data. The
only in-game evidence is the golden-card set in `REALGAME_GROUND_TRUTH.md`. Those cards show damage rounded up and
accuracy printed with one decimal and no `%`. They do not test this host code.

- **Truncation.** `UOpenWillowInventory::MagazineSize` and `ShotCostRounds` now truncate toward zero instead of rounding to
  nearest, because `ClipSize` and `ShotCost` are integer attributes. The slice recipes were evaluated before that rule
  and still carry fractional magazines (10.5 and 13.8 now give 10 and 13, not 11 and 14). The magazine's floor of one
  round is a host guard, not a game rule. A shot cost below 1 now truncates to 0, meaning no ammo cost; no local recipe
  has one.
- **Card numbers.** The host still sends raw stats. `inventory.js` `cardRound` rounds the stored single-precision value
  the way `present()` does: damage up, magazine down, and fire rate, reload and accuracy half up to one decimal. The
  accuracy line drops its `%`. Compare deltas are now differences of the printed numbers. The page does not print the
  recipes' `stats.card.display`, because the local slice's copy predates single precision (it holds 1.2 for a 1.25 fire
  rate that the game prints as 1.3).
- **Rarity colour.** Weapons now forward the card's `rarity_color` as `rarityColor` when the recipe has it. The page
  already prefers it, so E-tech gets its own colour. The local slice predates the field, so nothing changes until it is
  regenerated; that regeneration is still a separate decision.
- **Tests.** `tests/card_rounding_cases.json` holds invented cases. Both `tests/weapon_stats_test.py` (Python
  `display`) and `tests/inventory_navigation_test.js` (the page) check them, so the two implementations must agree.
  The navigation test was already failing at HEAD because its `document` stub lacked `addEventListener` (since
  `988b0b9`); the stub now has it. The inventory self-test checks truncation on invented values.
- **Changed card text in the suite:** the demo Infinity pistol with damage 753.22 now prints 754 (was 753), and the
  compare delta follows (+105 against 649, was +104). This was seen in the `backpack_transfer_changes_destination`
  frame. The 780.44 pistol would print 781 (was 780), but no captured frame shows it. Its other stats print as before.
  Suite frames from before and after are under ignored `local/card_rounding/`.
- **Checks (automated):** inventory actions PASS 45, FAIL 0, NOT_RUN 2, KNOWN_DIVERGENCE 2 (exit 1 from the NOT_RUN
  rows, as before); inventory self-test passed; quest first run PASS 79 / 0 errors and resume PASS 11 / 0; CTest 10/10;
  `verify_packages.py` 9/9; navigation test 23/23; weapon_stats 26, weapon_recipe 14, weapon_balance 7 and weapon_paint
  29 tests OK. **In-game:** none. The host widget fallback (`OpenWillowInventoryWidget.cpp`) still formats unrounded
  values.

## 2026-10-03: Phaselock presentation round 3: LDR-like clip, brightening screen tint, burst time in seconds

AI-assisted (Claude). Host and tooling; no change to `src/`, `CMakeLists.txt` or package parsing. This round follows an
independent critic's score of 4.5/10 for the round-2 side-by-side. Details are in `docs/verification/PHASELOCK_STOCK_DATA.md`,
"Round 3". Frames and side-by-sides stay under ignored `local/phaselock/`.

- **Common cause checked first.** Auto-exposure is already off project-wide; the difference is UE5's filmic tone curve
  against UE3's per-channel clip. Host stand-in (UNVERIFIED): additive FX layers cap each channel at 1, translucent ones
  at 1 / opacity, so that HDR particle colours stay saturated instead of turning white. The global tone mapper is
  unchanged.
- **Screen effect.** Its tint now divides the colour by its luminance, so it brightens toward blue instead of darkening
  to grey-brown (stand-in). Burst `Time` is now read as seconds of emitter time (UNVERIFIED). Only the screen burst
  moves (1.05 s -> 0.70 s), matching the game's tint onset of about 0.82 s.
- **Modulate readings (stand-ins).** `Mat_SirenGlowMOD` reads its colour as a brightness-keeping tint (cobalt cast
  flashes, as in the game frames). `Mat_SirenOrbBlackMOD` reads darkness as mask x (1 - alpha), the one reading that
  fits its three emitters. It adds the game's dark blob around the raised hand at about 0.27 s and keeps the bubble's
  dark core. Its host disc is softer (full to half the radius), so the hold rim reads violet-magenta.
- **Not changed:** the bubble size (stock draw-scale rule; a matched-distance game capture or the bullymong's bounds
  radius is needed to compare) and the palm orb size (it follows the hand, which is about 2.5x smaller on screen in
  the host: arms placement or FOV). The shots add a 5.0 s capture.
- **Open:** the release ring is blue-violet where the game's is cyan-white (the same `Mat_SirenGlowMOD` reading that
  fixes the cast flashes weakens it); the 0.6 s white starburst; the dummy's lifted pose (no stock clips);
  `SphereCollapse`.
- **Checks (automated):** quest suite first run PASS 79 checks / 0 errors and resume PASS 11 / 0; mover PASS 16 / 0;
  inventory actions PASS 45, FAIL 0, NOT_RUN 2, KNOWN_DIVERGENCE 2 (exit 1 from the NOT_RUN rows, as before); CTest
  10/10; `verify_packages.py` 9/9. **In-game check:** host frames compared by eye with the 2026-10-02 game captures
  only; no new game capture.

## 2026-10-03: Live weapon data read from the running game (real-game lane)

- **What.** `tools/real_game/scripts/weapon_dump.py` reads the stat properties of every weapon part, name part and type from
  the running game (SDK, main menu, saving blocked, saves restored byte for byte) into ignored `local/realgame/cards/`.
  `tools/real_game/live_overlay.py` and `golden_card_compare.py --live-data` use it as an overlay;
  `tools/real_game/openwillow_valuewatch/` logs chosen values from SDK load (local developer tools).
- **Confirmed in game (2026-10-03, method above).** The live values equal OpenBLCMM's static dump on all 78 values of W's
  61 changed objects; 37 objects carry real value differences against the cooked decode (ClipSize 7 types, ReloadTime 5,
  InstantHitDamage 2, 24 part effect lists), all in W's list. On the 69 golden weapons with exact parts the main four
  stats match 52/69 on cooked data and 69/69 with the live overlay.
- **Correction.** The source is not an online hotfix: the values exist 0.01 s after SDK load, before the `Micropatch`
  configuration, and its 23 entries touch no weapon. Not a package override (only `Startup.upk` defines the objects among 2,010
  packages) and not an installed mod. The origin is UNVERIFIED (load-time change by the game, or a decode gap). The wording in
  NATIVE_WEAPON_RULES section 7 still says "hotfix"; the weapon lane's owner should change it.
- **Open after the overlay.** Rocket launcher sale value about 4 % high (4 weapons), one status chance 33.3 vs 33.4, the
  level line rule, the stage-15 Maliwan pistol reload rounding (1.8 vs 1.7).
- **Not done / needs the maintainer.** The dump is game data and stays in `local/`; whether the port may read live values or
  must carry them some other way is the maintainer's call. No sensitive files touched; no tests rerun (no code under src/ or host/).

## 2026-10-03: Phaselock at a matched 650 uu in the real game (real-game lane)

- **What.** A requested capture for the Phaselock lane: the real game (Ice_P, level 8 Maya, saving blocked, saves restored byte for
  byte afterwards) casting Phaselock at one Adult Bullymong placed 650.0 uu away (horizontal; 652.9 in 3-D), every other enemy
  held still at least 2,500 uu off. New `tools/real_game/scripts/phaselock_matched.py` (list, bounds, isolate, place, measure,
  mark_lock); `phaselock.py` now samples the skill instance that moves (see below). Frames, sampler lines and a notes file stay in
  ignored `local/realgame/phaselock/matched_650/`.
- **Measured in game (one enemy type, one level, one cast pair; not a rule).** The bubble is 404-427 px across on the 1280x720
  frame, about 0.32 of the width: radius 167-174 uu at 650 uu depth (horizontal FOV 77.55 degrees, focal length 796.7 px). That is
  about 1.1 times the target's collision radius (150) and 0.55-0.58 times its mesh bounding sphere (300.8; the sphere moves with
  the animation: 241.9 and 315.3 on other reads). Inside the rim the view is deep violet-blue (mean about RGB 50/52/98, darkest tenth
  about 8 of 255); the held target stays visible at roughly a third of its normal brightness. Timeline from the per-frame
  sampler, measured from the skill start (15 ms after the key): lift to 0.70 s (target 190 uu higher), hold to 4.77 s, release to
  5.88 s; the screen vignette is up at 0.25 s, the hand orb at 0.5 s, a cyan-white burst at 0.8 s, the bubble from about 1.5 s,
  gone by 5.0 s. A second cast of the same setup (run1) agrees by eye.
- **Method findings.** `SetLocation()` returned False for AI pawns and for Maya and moved nothing; assigning `Location` moves
  her (the capture uses that) and `Destroy()` removed nothing. A target whose AI controller was detached was not locked: the
  skill ended at once and the target took damage (cause not read from script). Freezing pawns with `CustomTimeDilation = 0`
  held them only for a while. What worked: detach the controllers of every other pawn, keep the target's controller and set its
  `GroundSpeed` to 0. The sampler's `lift_skill()` took the last listed `LiftActionSkill`, which was the idle data object; it now
  takes the instance whose `SkillStartTime` is largest (the world instance moves; the `GD_Siren_Skills` object stays at 0).
- **Not done.** No host run compared with these frames; other enemy sizes and distances; a cast at other levels.
- **Checks.** None automated (game captures only; no code under src/ or host/). No sensitive files touched.

## 2026-10-03: Phaselock presentation round 4: matched-distance comparison, depth-biased darkening, release override

AI-assisted (Claude). Host and tooling; no change to `src/`, `CMakeLists.txt` or package parsing. The host is compared
with the matched-distance game capture (an Adult Bullymong at 650 uu, horizontal FOV 77.55 degrees) at the same
distance and FOV (`-owfov=62.15`). Details are in `docs/verification/PHASELOCK_STOCK_DATA.md`, "Round 4".

- **Stock data:** `Mat_SirenOrbBlackMOD` has a `DepthBias` parameter (-20), `Mat_SirenGlowMOD` a `Bias` (-15) and the
  smoke a `DepthBias` (-18). The host reads a negative bias as a camera-ward offset of the sprite plane, with the size
  scaled to keep the outline (UNVERIFIED). The black orb now covers the lifted target's front.
- **Darkness, calibrated against the one capture (stand-in):** the black orb is fully dark to 0.6 of its radius (thin,
  dim rim), with darkness capped at 0.9 on opaque geometry just behind it (the game keeps the target at about a third of
  its brightness) and at 0.96 over the background. The second cap compensates for UE5's float target, where the
  additive layers sum to about 2.5 before the black, against UE3's clamped 8-bit target.
- **Release:** a labelled per-emitter override gives the end template's `Brighten` the plain multiply. The release is a
  cyan-blue ring; the cast flashes keep the brightness-keeping tint.
- **Size: not changed, not confirmed.** The stock rule (mesh bounds sphere radius / 66.7, full-width sprites) puts this
  bullymong's rim at about 356 uu, while the capture measures 167-174 uu. Two readings fit and cannot be separated with
  this enemy, so the report carries a capture request for a second enemy type with an SDK read of the bubble emitter's
  draw scale.
- **Open:** the release timing (game 4.77 s in this capture, host 4.60 s; not changed); the floor glow seen in the
  matched frames against the light-channel stand-in; the magenta-leaning interior; the 0.6 s starburst; the host's
  default capture FOV (106 degrees against the game's 77.55).
- **Checks (automated):** quest suite first run PASS 79 checks / 0 errors and resume PASS 11 / 0; mover PASS 16 / 0;
  inventory actions PASS 45, FAIL 0, NOT_RUN 2, KNOWN_DIVERGENCE 2 (exit 1 from the NOT_RUN rows, as before); CTest
  10/10; `verify_packages.py` 9/9. **In-game check:** host frames compared by eye with the matched-distance game capture
  only.

## 2026-10-03: Phaselock bubble size rule read from the game's emitters (real-game lane)

- **What.** A follow-up to the matched 650 uu capture, requested by the Phaselock lane: read the spawned bubble emitters at lock
  time on three enemies (Baby, Adult and Ranged Bullymong, Ice_P, level 8 Maya, saving blocked, saves restored byte for byte) and
  measure the rim in frames. New `tools/real_game/scripts/phaselock_size_rule.py` (probe), `tools/real_game/bubble_frames.py`
  (frames at chosen times and a rim width), `aim()` in `phaselock_matched.py`. Frames, probe lines and notes stay in ignored
  `local/realgame/phaselock/size_rule/`.
- **Measured in game (one map, level and skill build).** The two bubble emitters of one pawn (`Part_SirenASEnemyOrbBegin` at lock,
  `Part_SirenASEnemyOrb` 0.2 s later) have different `DrawScale` values (adult 4.2825 and 3.9026, ranged 4.3144 and 3.7926, baby
  1.8181 and 1.9218) while the collision radius never changes (150, 150, 64). The intro value times `BubbleFXScale` (66.7) is within
  1 to 2.3 percent of the pawn's mesh bounds sphere radius read at +0.72 s (285.6 against 290.3, 287.8 against 290.0, 121.3 against
  124.1). So the size input is the mesh bounds sphere at spawn time, not the collision radius. `DrawScale3D` and the particle
  component's scale are 1 on every emitter. The visible rim radius per unit of the loop emitter's `DrawScale` is 47.6, 48.6 and 49.5
  uu on the baby, adult and ranged runs (about 0.73 times 66.7): a constant of the particle template, not of the enemy.
- **Limit.** All three pawns have mesh sphere over collision radius of 1.93 to 1.94 at the lock pose (the same animation), and no
  enemy with a very different ratio (a Skag, a Brut) was on the map, so size ratios alone do not separate the readings; the
  emitters changing with the pose and the baby's half-size bubble following its 0.5 mesh scale do.
- **Timing, game clock.** Lift 0.70 s, hold 3.9 s, release 1.1 s (`LockFadeOutTime`), duration 5.7 s; release began 4.608 s after the
  skill start in all three casts. The earlier matched capture read 4.786 s (hold 0.175 s longer, on a second lock of the same pawn;
  cause not read, the skill's tick rate is 0). `LockDurationFormula` is a 7.0 constant plus the designer attribute
  `Att_Phaselock_Duration` (its value was not read).
- **Method findings.** A cast at pitch 0 did not lock a baby at 650 uu (no target within the auto-aim); aiming the crosshair at the
  target's origin did. Several pawns were found dead or gone after their neighbours' controllers had been detached (cause not
  read), so one capture needs its setup and cast within seconds.
- **Not done.** No host comparison; a Skag or Brut; other levels; the duration attribute's value.
- **Checks.** None automated (game captures only; no code under src/ or host/). No sensitive files touched.

## 2026-10-03: weapon card rules: level line, single-precision rounding, float bases; golden cards 69/69 (weapon lane)

AI-assisted (Claude). Tools, tests and notes only: no `src/`, no `CMakeLists.txt`, no host or page code; package
parsing is unchanged. The rules are written in our own words in `docs/verification/NATIVE_WEAPON_RULES.md`. Raw
decompiler output stayed in the analysis store. Evidence: the 69 golden cards (exact rolled parts) recorded by the
real-game lane.

- **Level line** (read from script and data, section 4). A mission-balance weapon requires level 0. Any other weapon
  requires its item level minus the floor of the player's level-requirement bonus (0 in this data unless the player has
  something that raises it), at least 1. The card prints the line only when the requirement is above 1, so every
  mission weapon and every level-1 weapon prints none. New `weapon_stats.level_requirement` and the card fields
  `level_requirement` and `level_line`. Not modelled: the over-level text and the DLC message.
- **Rounding precision** (section 2). The Float rounding scales, adds the half and floors on the x87 unit. A golden
  status chance that is exactly a float tie (stored 33.349998, printed 33.4) shows the unit runs at single precision.
  `half_up` now rounds `f32(value * 10^p)` before adding the half. How the game sets the x87 precision was not read
  (UNVERIFIED; Direct3D 9's default). Consequence: an invented 87.35 accuracy now prints 87.4, not 87.3.
- **Float bases** (section 1). Plain weapon-type fields enter the stack as floats. The class-default 2.1 reload is
  2.0999999, so a -20 % scale gives 1.7499998, which prints 1.7 as in the game, not exactly 1.75 printed as 1.8.
- **Launcher sale value** (section 6). The evaluator was right. The golden comparison tool does not pass the recorded
  prefix and title, so the launcher prefixes' price multiplier was missing. The value function itself is still not
  read: its vtable could not be resolved, a second time.
- **Status rows** come from their own presentation data: Float, one decimal. The chance row is a remap whose slope equals
  the Generic BaseChance, except for slag (30.03 against 30; no slag card, UNVERIFIED).
- **Golden counts**, tracked comparison tool, main four / every printed stat / every field:

  | data | before | after |
  |---|---|---|
  | live | 69 / 68 / 54 | 69 / 69 / 54 |
  | cooked | 52 / 52 / 40 | 53 / 53 / 41 |

  Per field (live), status chance went from 15/16 to 16/16. On cooked data reload went from 63 to 64/69.
  A local copy of the tool that passes prefix, title and balance and uses `card['level_line']` gives every field on
  68/69 live (sale value 69/69, level line 69/69) and 52/69 on cooked data. The one remaining live miss is the host's
  own slice name. That tool change belongs to the real-game lane.
- **Tests and shared cases.** `tests/card_rounding_cases.json` gains two reload cases at the half (1.7499998 prints
  1.7; 1.75 prints 1.8). Its 87.35 accuracy case moves to `pending_page_change` with the new text 87.4, which only the
  Python test reads. `inventory.js` `cardRound` still prints 87.3 for it. To match, the page would round
  `Math.fround(Math.fround(stored * scale) + 0.5)` before the floor, with `scale = Math.fround(10 ** decimals)`. That
  change needs the in-engine suite and was not made. `weapon_card_audit.py` audits cards without a level line at level 1
  instead of failing.
- **Checks:** weapon_stats 28, weapon_recipe 14, weapon_balance 7, weapon_paint 29, golden_card_compare 12 tests OK;
  navigation 23/23; CTest 10/10; `verify_packages.py` 9/9. **In-game:** none beyond the golden cards above. No
  sensitive files touched.

## 2026-10-03: Golden card compare passes the balance and name parts (real-game lane)

- **What.** `tools/real_game/golden_card_compare.py` now gives the evaluator the record's balance and its prefix and title name
  parts, renders the level line from the evaluated `level_requirement` instead of the spawn stage, and blanks it when the model
  says there is no level line. Requested by the weapon lane, whose commit `13d9e87` made the rules need these inputs for sale
  value, the name and the level line.
- **Result (automated, golden-card evidence; the rules stay UNVERIFIED).** On the 69 golden weapons with exact parts: cooked data
  alone 53 matching the main four stats, 53 every printed stat, 52 every field; with the live-data overlay 69 / 69 / 68. The one
  miss is the host slice's own display name (`host_name`, 5 of 6). `tests/golden_card_compare_test.py` passes (12 tests).
- **Not done.** No new in-game capture; the rules behind the level line and rounding are still read from native code and
  confirmed only against the golden cards.
- **Checks.** Compare tool run twice as above; no sensitive files touched.

## 2026-10-03: Phaselock presentation round 5: bubble size from the confirmed rule, floor light, interior colour

AI-assisted (Claude). Host and tooling; no change to `src/`, `CMakeLists.txt` or package parsing. Details are in
`docs/verification/PHASELOCK_STOCK_DATA.md`, "Round 5".

- **Size rule, confirmed in game on 2026-10-03** (the real-game lane's SDK reads of the bubble emitters' `DrawScale` plus
  frames, three bullymong variants):
  - each bubble emitter's `DrawScale` = the lifted pawn's mesh bounds sphere radius at its own spawn / 66.7;
  - the loop's visible rim sits 48.6 uu per `DrawScale` unit.
- **Host calibration (UNVERIFIED cause):**
  - each template now takes its own draw scale at spawn;
  - all three bubble templates are drawn at 48.6 / (0.44 x the `Sphere` StartSize) of it, because the host drew the rim
    ridge at 0.88 of the `Sphere` half-width (about 1.8 times the game's size);
  - size check on host frames at the matched FOV: rim radius 79.7 uu at 1.5, 3.0 and 4.5 s against the expected
    48.6 x 1.640 = 79.7 uu.
- **Floor light:** it reaches the floor again, as its data says. The 10-03 game frames show a pale blue pool, which
  undoes the round-1 stand-in.
- **Interior (stand-in, calibrated against one capture):** the bubble's black orb blends toward a deep blue-violet
  instead of black, standing in for UE3's clamp after every blend. Host interior (38, 37, 84) against the game's
  (39-48, 38-45, 88-100). The hand orb stays black.
- **Timing:** first locks in game release at 4.608 s, which the host matches; no change.
- **Open:** the dummy stays lit in front of the smaller bubble; the floor pool is fainter on dark asphalt; the 0.6 s
  starburst.
- **Checks (automated):** quest suite first run PASS 79 checks / 0 errors and resume PASS 11 / 0; mover PASS 16 / 0;
  inventory actions PASS 45, FAIL 0, NOT_RUN 2, KNOWN_DIVERGENCE 2 (exit 1 from the NOT_RUN rows, as before); CTest
  10/10; `verify_packages.py` 9/9. **In-game check:** host frames compared with the matched-distance and size-rule game
  captures only.

## 2026-10-03: Maintainer README/plan edits, written stop conditions dropped, worktree provisioning from a source worktree

Maintainer decision. Docs and tooling only; no change to `src/`, `CMakeLists.txt`, the reader or package parsing.

- **Maintainer edits kept as written:** the README intro, status-table notes for Phases 1, 3 and 4, the FAQ and
  "Who's making this" wording, and the plan's opening/credit lines. The Phase 3 and Phase 4 notes ("working well",
  "roughly 6/10 parity") are the maintainer's own testing and have no verification record; they are worded as such.
- **Stop conditions dropped:** the plan's section 9 (now "Commitment") and ROADMAP's "Kill criteria" section no longer
  list stop conditions. Links and descriptions that pointed at them were updated (README FAQ, document table, the
  Phase 1 gate sentence in ROADMAP). Verification rules are unchanged.
- **Worktree provisioning:** `tools/provision_worktree_assets.ps1 -SourceWorktree <path>` copies missing files from
  a populated worktree's Content and every `local/` folder (the shared seed only holds four folders and goes stale).
  Documented in AGENTS.md and `tools/worktree-assets.md`. Parse-checked only; not run in a new worktree.

## 2026-10-04: native analysis notes: Phaselock presentation, weapon visuals, ambient NPC movement, backpack sort

AI-assisted (Claude). Docs only; no change to `src/`, `CMakeLists.txt`, the reader or package parsing. Four own-words
notes under `docs/verification/`, read from the installed script (`research/script_disasm.py`), installed class
defaults and a local Ghidra 12.1.4 reading of the executable (`tools/ghidra/`). Raw output stays under ignored
`local/analysis/E/`; the notes contain no listings or pseudo-code.

- **Phaselock presentation** (`NATIVE_PHASELOCK_PRESENTATION.md`): the lifted target plays four stock special moves
  (lift, loop, fall, land) from its own AnimSets, starting at the cast; the fall clip is stretched to `DropTime`
  (0.5 s); the first-person hand effect comes from a notify 0.25 s into the cast clip, attached at an arms socket; the
  only bubble instance parameters are `PhaselockLifeTime` (once) and `SphereCollapse` (every tick).
- **Weapon visuals** (`NATIVE_WEAPON_VISUALS.md`): the weapon material is a new instance over the Material part's MIC
  with the parts' vector parameters applied as linear colours (only elemental parts carry any); a per-shot impulse drives
  the emissive scale; the first-person mesh attaches to the arms' weapon socket; foreground FOV 45 with a weapon, 60
  without.
- **Ambient NPC movement** (`NATIVE_AMBIENT_NPC.md`): script and data driven (scripted-NPC actions walking move-node
  chains, perches with weighted idle variants); a native load balancer admits at most 7 walkers, one per 0.5 s. Next-node
  choice and speed rule not read.
- **Backpack sort** (`NATIVE_INVENTORY_SORT.md`): the five modes are data; comparator chains, filters and header rules;
  reproduces every 2026-09-30 observation. Ties are unordered in the engine's sort.
- **All rules UNVERIFIED in game;** each note names the capture that would confirm it.
- **Checks:** none needed (docs only).

## 2026-10-04: Phaselock presentation rounds 6-7: effect materials from own-words notes on the compiled shaders

AI-assisted (Claude). Host and tooling only; no change to `src/`, `CMakeLists.txt`, the reader or package parsing.
Details: `docs/verification/PHASELOCK_STOCK_DATA.md`, "Round 6" and "Round 7".

- **Method:** the cooked effect-material graphs are stripped. Their compiled pixel shaders in
  `RefShaderCache-PC-D3D-SM3.upk` were read with `research/d3d9_bytecode.py`, the same method as the weapon paint model
  (2026-10-02). What each one computes was written down in our own words, and the 17 effect materials in
  `host/ue5/import_phaselock_fx.py` were written from those notes as UE Custom nodes, with plain constants and prose
  comments. The listings stay under ignored `local/`.
- **Findings that replace earlier rounds** (UNVERIFIED in game unless compared below):
  - `DepthBias` is a soft-particle fade distance, not a camera-ward shift.
  - The bubble sphere is a warped read of the bubble texture.
  - The tattoo mask is the B channel of one quadrant of the mask texture; rounds 1-5 lit the whole sleeve.
  - The bubble ring sits at 0.78 of the sprite half-width.
  - The sigil under the target is HUD, not an effect.
- **Host calibrations (UNVERIFIED):**
  - bubble warp strength driven by the collapse value;
  - core haze ×3;
  - layer colours above 1 normalised;
  - blue tints on the rim and haze;
  - a per-emitter darkening cap and fade floor;
  - floor light gain ×4 (the data's brightness is still what `PhaselockLightIntensity()` reports);
  - lighter swirl, star-burst and brighten.
- **Visual review:** an independent critic agent compared matched host and real-game frames. It scored round 6 at 5/10
  and round 7 at 6/10 (hand 6, bubble 7, release 5). Largest remaining gaps:
  - the release does not collapse the sphere;
  - the palm orb is lost around 0.55 s;
  - the interior is more see-through than the game's.
- **Target animation:** the stock clips exist only on enemy AnimSets and the dummy has none, so it is still not
  exercised.
- **Checks:**
  - CTest 10/10; `verify_packages` 9/9 match; `particle_system_test` OK.
  - Quest suite first run PASS 79/0, resume PASS 11/0 on the round-7 build.
  - Mover and inventory suites not run for this lane (untouched areas); the end-of-session run covers them.

## 2026-10-04: inventory and Skills pages: Skills preload, stock sort list, backpack focus layout, compare frames from real-game captures

AI-assisted (Claude). Host page, host forwarding and the suite only; no change to `src/`, `CMakeLists.txt` or package
parsing. Details: `docs/verification/INVENTORY_MOVIE_PROTOTYPE.md` ("Skills preload, real-game comparison and the stock
sort list: 2026-10-04") and `docs/verification/INVENTORY_CARD_STATS.md` ("What the page prints on a weapon card").

- **Skills page:** in the baseline frame it still said "Loading Maya's skill tree...". The page was created on the key
  press and took about 3.4 s. It is now loaded hidden at level start, like the inventory page:
  - key to a populated page: 49 ms (page log lines);
  - cost: the inventory page's own boot grows from 4.4 s to 6.1 s;
  - real game: page visible about 0.28 s after K, settled by 0.65-0.85 s. No timing parity is claimed.
- **Real-game session (Maya L8):**
  - Saves were backed up and compared byte-for-byte afterwards, and the driver was removed.
  - The install runs community mods: part-name lines on cards and instant gear equip are theirs, so gear compare was
    not observed.
  - Confirmed in these captures:
    - compare frames: the moved item is green, the other yellow (the host had them inverted);
    - compare rows show arrows only;
    - red gear cells appear in a weapon compare;
    - backpack focus layout: enlarged centred panel, receded equipped panel, "BACKPACK used/capacity" plate;
    - the lists of all five sort modes;
    - the full-screen Inspect.
- **Stock sort list:**
  - ALL/TYPES/BRANDS/ITEMS/VALUE with sub-headers replace the host's own modes and category filter, following the
    comparators of `NATIVE_INVENTORY_SORT.md`. Those rules stay UNVERIFIED; every point the capture could test agreed.
  - PageDown and PageUp step through the modes, and the first item is selected on each change.
  - Ties keep pickup order. This is a host choice: the game's sort is unstable.
- **Cards:**
  - They now show the projectile count, the status rows for elemental guns, and values in the label colour.
  - For the six slice recipes the page text matches the golden cards, except the two shotgun recipes (reload and
    magazine one step off). That is a recipe/evaluator question, not a page one.
- **Inspect:** now full screen. The picture is the 3D preview keyed against black, so black gun parts can show as holes.
- **Slot art:** the slots now show item art, from previews rendered locally for the slice guns (local data only).
- **Visual review:** an independent critic agent scored 11 matched host/real-game pairs at 5.6/10 overall. Main gaps:
  - flat panels against the movie's tilted glass (Ruffle ignores the movie's 3D transforms);
  - card text about 20% smaller;
  - sort sub-headers overlapping cards;
  - the inset selection fill;
  - the compare layout;
  - the Skills tiles.
- **Open:** selectable empty backpack cells, gear compare in the real game, white flavour lines on cards, Q on the
  Skills page, and the Phaselock eye sigil on the HUD.
- **Checks:**
  - `tools/test_inventory_actions.ps1`: 49 PASS / 0 FAIL / 0 NOT_RUN / 0 KNOWN_DIVERGENCE (baseline 45/0/2/2).
    - Steps 15/16 now assert the stock sort.
    - The two wheel steps run because the test adds four synthetic filler weapons.
    - Slate keys only; the suite does not judge visuals.
  - `node tests/inventory_navigation_test.js`: 27/27.
  - CTest 10/10 and `verify_packages` 9/9 on the same tree.

## 2026-10-04: Phaselock presentation round 8: release size, hand timing, interior measured against the game

AI-assisted (Claude). Host only; no change to `src/`, `CMakeLists.txt` or package parsing. Details:
`docs/verification/PHASELOCK_STOCK_DATA.md`, "Round 8".

- **Release "does not collapse":** this was a test-aid error, not an effect rule.
  - No bubble shader reads the collapse value. The game sizes each bubble template from the pawn's mesh bounds at that
    template's spawn, and the bounds shrink as the pawn is lifted: 290 uu at the lock and 193 uu lifted, read with the
    SDK driver in the real game.
  - `-owbubbleradius` now takes one value per template (`290,260,185` stands in for an adult bullymong).
- **Hand (host calibrations, UNVERIFIED):**
  - The effect starts 0.08 s before the clip's notify.
  - The cast clip plays at 0.85 speed (`PlayAction` gained a rate argument).
  - Orb texture ×0.6 and star-burst alpha ×0.2.
  - Basis: the game frames show an opaque palm orb at 0.44 s and the arm dropping about 0.05 s later than the host's.
- **Interior opacity, measured on the game frames:** about a quarter of the background's contrast survives inside the
  bubble (regression slope 0.26 at 1.5 s, 0.21 at 3.0 s).
  - The host darkening caps (0.72 loop, 0.45 end) bring the host to 0.11 / 0.26.
  - The host bubble centre in the measurement is approximate.
  - The method and its script are kept local.
- **Floor light:** wider, softer and paler (UNVERIFIED calibration).
- **Visual review:**
  - An independent critic gave an absolute score of 5.5/10, against 6/10 for round 7.
  - A blind A/B critic, judging against the same game frames, preferred round 8 on the hand and release sheets and called
    the bubble sheet a tie.
  - Absolute scores from separate critic runs vary by about a point, so the A/B result decided the commit.
  - Remaining gaps:
    - the bubble is about 20% large and sits high-left;
    - the floor glow is now too faint;
    - the 0.30-0.40 s black sphere and swirl are small;
    - the tattoos are overexposed after 0.55 s;
    - no ice-shard flash facets;
    - straight release shards;
    - no fist clench;
    - no target animation.
- **Checks:** quest suite first run PASS 79/0 and resume PASS 11/0 on the round-8 build; CTest and `verify_packages`
  were not rerun for this commit (no `src/` change).

## 2026-10-04: Sanctuary ambient citizens: census, assets, host perch cycle and node walks behind a flag

AI-assisted (Claude). Host and tools only; no change to `src/`, `CMakeLists.txt` or package parsing. Details:
`docs/verification/SANCTUARY_AMBIENT_NPCS.md`; native reading in `NATIVE_AMBIENT_NPC.md` (UNVERIFIED).

- **Census (our reader):** the civilians come from four population definitions in `Sanctuary_Combat` and two pawn
  archetypes, male and female Sanctuary Citizen. Their routes are decoded data: initial destinations, weighted next
  nodes, perch start/idle/stop clips, loop and lerp times, and Kismet scripted moves. Counts are in the record.
- **Real-game observations (SDK driver; Maya level 8 at mission Plan B):**
  - Saves were backed up and blocked in game. The game rewrote two files, which were restored, and the folders then
    compared identical.
  - 33 live citizens; 36 of 52 dens and encounters enabled, none of the 7 crowd dens.
  - A walking citizen's velocity read 150.
  - The town-wide patrols are Resistance fighters, not citizens: 94-95 uu/s by displacement, along the data's node
    circuits for 3 of 7 sampled.
- **Host:**
  - Both kinds are imported through the Marcus pipeline pattern: 112 bones and 42 clips each. Textures are bound by
    UModel's guess and the clip mapping is UNVERIFIED.
  - `-owambient=<manifest>` spawns them. Otherwise nothing spawns.
  - Stand-ins, all UNVERIFIED:
    - The live set is copied from one real-game moment.
    - Walking is in straight lines at 150 uu/s, with the floor taken from a trace.
    - A load balancer follows lane E's note.
    - There are no hats, hair or outfit variants, no Resistance fighters, and no talking or look-at.
- **Visual review:** an independent critic scored the host citizens 5/10 against real-game frames (mesh/outfit 4,
  poses 5, walk 6, scale 7). Main gaps:
  - bald identical heads;
  - no ink outline and washed-out colours;
  - the female reads as male;
  - a duplicated pawn at one perch;
  - several perch poses misaligned with their props.

  The real frames are matched by activity, not position: the population could not be held for close-ups.
- **Checks:**
  - `tools/test_ambient.ps1`: PASS (33 pawns, 33 reached a node, 3 walked, 24 at a perch).
  - Quest suite with ambient on: 79/0 and 11/0. One earlier first run failed check 61 on a Phaselock light reading; the
    repeat passed and the flake is with the Phaselock lane.
  - Quest suite without ambient: 79/0 and 11/0.
  - CTest 10/10; `verify_packages` 9/9.

## 2026-10-04: Inventory and Skills pages round 10: sub-header rows, compare layout, selection band, card fit, Inspect clip, Q overview

AI-assisted (Claude). Page and HUD capture sequence only; no change to `src/`, `CMakeLists.txt` or package parsing.
Details: `docs/verification/INVENTORY_MOVIE_PROTOTYPE.md` ("Round 10").

- **Sort sub-headers:** each sits on its own row between groups. They were centred on the header clip's bounds, which
  are taller than the text, so every header sat about half a row high over the previous card.
- **Compare view:** the Equipped panel is narrowed into the gap between the cards with slots 1-4 drawn, the Backpack
  panel moves right so "(COMPARE)" stays readable, and both cards carry the price chip and Accuracy row (the synthetic
  test variants lacked those fields).
- **Selection:** the movie's own highlight symbol is stretched to the panel width behind the selected row (width and
  shift set by eye).
- **Card size:** cards are rescaled to visible widths measured on the real captures. A fit, UNVERIFIED.
- **Inspect:** the movie is clipped to the card frame and hint strip, so the backdrop box is gone.
- **Skills:** Q toggles an overview of the three trees. The layout numbers start from the installed `Gfx_SkillTree`
  defaults and are enlarged by eye; how the game combines them was not read, so UNVERIFIED.
- **Visual review:** an independent critic scored round 10 6.6/10 against real-game frames (round 9: 5.6) and judged
  round 10 closer than round 9 on all ten inventory pairs. Main gaps: the perspective tilt and curved glass (3D
  transforms, which Ruffle ignores), backpack-focus list geometry, compare panel positions and padlocks, Inspect card
  level strip, Skills page layout.
- **Checks:** `node tests/inventory_navigation_test.js` 27/27; `tools/test_inventory_actions.ps1` 49 PASS / 0 FAIL /
  0 NOT_RUN / 0 KNOWN_DIVERGENCE, rerun after the weapon lane's preview and stat changes with the same result.

## 2026-10-04: Sanctuary ambient citizens round 2: heads, hair and hats from live pawns, ink line, perch root motion

AI-assisted (Claude). Host and tools only; no change to `src/`, `CMakeLists.txt` or package parsing. Details:
`docs/verification/SANCTUARY_AMBIENT_NPCS.md` section 8.

- **Causes of the round-1 defects:**
  - The "duplicate pawn" was two real neighbouring citizens (`Perch_66` and `Perch_140`, 240 uu apart) on the camera's
    line. The capture camera now rejects lines that pass within 130 uu of another pawn.
  - Lean, legs and squat: the stock perch clips carry root motion (the observed pawn-to-node offset matches the start
    clip's travel, e.g. 24.1 vs 25.8 uu at `Perch_66`), and the floor trace missed the real pawn height by more than
    10 uu on 19 of 33 pawns. The host now applies each clip's root travel when it ends and keeps the observed height
    for idle and held pawns.
- **Observed in the real game** (third SDK session, saves blocked, byte-identical afterwards): each live citizen's
  materials and attached static meshes are readable. 33 pawns carried 88 attachments (20 meshes, 8 head textures).
- **Host:** those heads and attachments, bone-attached; Maya's inverted-hull ink line and matte constants for the
  citizens. UNVERIFIED: the attachment transform (judged by eye), the hair tint stand-in, the outline thickness and the
  shader. Not done: `Master_NPC` zone colours, body garment variants, Resistance patrols, matched real-game close-ups.
- **Visual review:** an independent critic scored round 2 5.5/10 (round 1: 5), judged better than before, no
  overlapping pawns. Main gaps: the female still reads as male (face patch, hair), a mis-parented hat at one stop, scalp
  showing through blonde hair, one bald untextured head, the bang-on-wall fist not meeting the wall, a squat clipping
  a pipe.
- **Checks:** quest suite with ambient on 79/0 and 11/0, with ambient off 79/0 and 11/0; `test_ambient.ps1 -Shots`
  PASS (36 pawns, 36 reached a node); CTest 10/10; `verify_packages` 9/9.

## 2026-10-04: Phaselock presentation rounds 9-11: ring factor, release size, soft streaks and ground wash, hand spikes, check-61 fix

AI-assisted (Claude). Host and tooling; no change to `src/`, `CMakeLists.txt` or package parsing. Details:
`docs/verification/PHASELOCK_STOCK_DATA.md` ("Round 9" to "Round 11"). All calibrations UNVERIFIED.

- **Bubble too large (cause):** round 6 had replaced the ridge detector's ring factor (0.88) with the bubble texture's
  own ring (0.78), which drew the bubble about 13% too large. Restored; the per-template stand-in radii are 290, 233
  and 210. The release shell was re-measured on the game frames (112 px radius at 4.80 s, 176 px at 5.00 s); round 9's
  smaller figure was a mis-measure.
- **Hand timing:** round 8's whole-effect early start shrank the swirl and disc. Now only the palm orb (0.03 s) and the
  disc's alpha (0.1 s) run ahead; the disc's size curve is read at its unshifted age. The hand effect starts at the
  cast clip's notify; the clip still plays at 0.85.
- **0.65 s spike rays:** they come from the hand template's star-burst emitter; the game frames show none, and why was
  not found. The host scales that emitter to 0.15 (a new per-emitter gain).
- **Look:** soft pale-blue ground wash (light gain 6.5, radius 0.85x, colour 20% toward white), wider and softer
  streaks, release spikes and ribbons at reduced strength, tattoo glow x0.3.
- **Quest check 61 flake** (light 32 vs 4 once): traced to an intermediate build that reported UE5's intensity rather
  than the data value (32 = 4 x a gain of 8). The light's data-unit brightness is now stored when it is set, so the
  check no longer depends on the host gain. The check itself is unchanged.
- **Visual review:** independent critics scored rounds 9, 10 and 11 at 5.5, 5.5 and 6.3. Blind A/B against round 8:
  round 9 and round 10 each won one sheet of three; round 11 won bubble and release and tied hand, so it is committed.
- **Open:** the 0.30 s dark void (about 200 px against the game's 330; two enlargements made it fade, cause not found),
  the 0.80 s whiteout, interior opacity, rim weight, ice-shard flash facets, straight release shards, fist clench, target
  animation (no stock clips on the dummy), slight egg shape.
- **Checks:** quest suite first run 79/0, resume 11/0 on the final build. CTest and `verify_packages` in the final pass.

## 2026-10-04: Inventory and Skills pages round 11: list scroll origin, compare and Skills layout, Inspect strip

AI-assisted (Claude). Page code only (`inventory.js`, `skills.js`); no change to `src/`, `CMakeLists.txt` or package
parsing. Details: `docs/verification/INVENTORY_MOVIE_PROTOTYPE.md` (round 11).

- **Cause of the off-centre selection band:** round 10 gave the row list a scroll rectangle with a negative x origin.
  Ruffle shifts the content right by that amount instead of revealing it, so every row sat about 23 px right of the
  panel centre and a band shift constant hid it. The origin is zero now and the shift constant is gone; the band is
  sized from the panel frame.
- **Compare:** the Equipped panel is wider and the Backpack panel moved left; padlocks show on locked slots (checked
  with `-owslots=2`, the real session's count, only). Compare from equipped keeps the highlight on the chosen
  equipped slot (drawing only; behaviour unchanged).
- **Skills:** tab group and trees moved right, Phaselock card refitted after each tree tween, overview footer below
  the panels and locked tiers dimmed. **Inspect:** level strip and price chip restored, thinner hint strip.
- All placements are fits read off the captured frames (UNVERIFIED).
- **Visual review:** an independent critic scored round 11 7.3/10 (round 10: 6.6) and judged it closer than round 10
  on every pair, except that compare-from-equipped lost the highlight on the focused backpack tile. Main gaps: the
  perspective tilt and glass (not reproduced; Ruffle ignores the movie's 3D transforms), band overrun at the panel's
  right edge, card text about 15 px against 17 px, Skills plate colliding with the footer hint.
- **Checks:** `node tests/inventory_navigation_test.js` 27/27; `tools/test_inventory_actions.ps1` 49 PASS / 0 FAIL /
  0 NOT_RUN / 0 KNOWN_DIVERGENCE.

## 2026-10-04: Phaselock presentation rounds 12-13: hand effects in first-person space, saturated swirl, deeper orb

AI-assisted (Claude). Host and tooling; no change to `src/`, `CMakeLists.txt` or package parsing. Details:
`docs/verification/PHASELOCK_STOCK_DATA.md` ("Round 12", "Round 13"). All calibrations UNVERIFIED.

- **First-person space:** with the weapon lane's `-owfpfov` (the game's separate foreground FOV for arms and gun) the
  hand sprites render as first-person primitives, so they stay on the hand; the bubble, screen effect and light stay in
  world space. With it the swirl, disc and orb are about game-sized.
- **Round 12's "enlarging the void fades it"** was a misreading: the wider core did grow, but its soft gradient let
  the street show through. The normal-FOV widening and a thick rim halo were tried and reverted after a blind A/B.
- **Hand look:** energy swirl alpha 0.8 with a more saturated cyan; palm orb a deeper blue with its texture veins.
- **Diagnostic:** `-owfxscalar` sets one effect material scalar per emitter at run time (TOOLING).
- **Visual review:** round 12 6/10, round 13 6/10. Blind A/B: round 13's hand beat round 12's (both close); the bubble
  and release were close to ties with rounds 11 and 12. Scores have stayed between 5.5 and 6.3 since round 7.
- **Open:** near-black violet void and interior, a compact ground disc, side streaks, thick release ribbons, orb vein
  cracks, the forward fist, the 0.80 s whiteout, ice-shard facets, target animation.
- **Checks:** quest suite with `-owfpfov=45` 79/0 and 11/0 (via a local copy of `tools/test_quest.ps1` that adds the
  flag).

## 2026-10-04: Guns: parts and card stats from the running game, a tone-mapper-aware gun material, first-person foreground FOV

AI-assisted (Claude). Host, tools and tests; no change to `src/`, `CMakeLists.txt` or package parsing. Numbers and
paths: `docs/verification/WEAPON_VISUALS.md`.

- **Real-game session** (SDK driver, saves backed up, blocked and byte-identical afterwards, guns in memory only):
  six exact-part guns captured in first person and in the Inspect view; live part mesh lists and material instances read.
- **Part assembly (confirmed in game):** the host used only the main gestalt mesh. Adding the body-variant meshes and
  drawing nothing for `*_None` parts makes all six fragment lists equal the live part mesh names, with exactly equal
  triangle totals. The Jakobs pistol's hidden-bone triangles (moon clip, bullet) are cut. Not confirmed: that the
  `*_None` name is the game's own test.
- **Colour, round 1:** with the right meshes the existing paint model was largely right (Maliwan colours within 0.82x
  median and 2.7 degrees of hue of the real Inspect view). The material became an Unlit evaluation of the base pass's
  structure with stand-in lights (UNVERIFIED) and the part's elemental emissive vectors.
- **Colour, round 2 (cause):** the oversaturation, crushed blacks, red-for-orange accents and hot Infinity shroud came
  from UE5's film tone mapper acting on the Unlit output (scene-linear 0.18 displays 0.03, 1.0 displays 0.51; measured
  with a grey ramp in the material). The material now applies the inverse of the measured curve (a 28-point table used
  per channel; an approximation) and clips per channel. Median linear ratio host/real against the Inspect view:
  Maliwan 1.06, Jakobs 0.69, Infinity 1.01, SMG 1.16, rifle 1.05, shotgun 0.86 (round 1: 0.77, 0.72, 1.06, 0.93,
  1.08, 0.64).
- **First person:** the game draws arms and gun with a foreground FOV of 45 (read through the SDK; world FOV 77.55
  with the config's 90). UE 5.8's first-person FOV now does the same by default; making it the default is a host
  choice, `-owfpfov=0` opts out. The Phaselock hand effects follow it (rounds 12-13). The SMG, rifle and shotgun were
  held in the pistol clips; their own clip sets are imported and chosen by weapon type. The weapon socket and its
  90-degree yaw, and the arms mesh and material, are the game's own data.
- **Shotgun cards (lane D's report):** 4.4/9 and 3.7/13 came from stats stored by an older evaluator and from the
  cooked weapon type differing from the running game's. Re-evaluated on the live overlay: 4.1/10 and 3.5/14, matching
  the game's Inspect card for the exact parts. Where the live values come from is still unexplained.
- **Mission pistol:** the quest lends the recipe's own corrected mesh where it is imported, with the earlier rolled
  sample as the fallback (one block in `OpenWillowQuest.cpp`).
- **Visual review:** independent critics scored round 1 7.2/10 and round 2 6.8/10; round 2 was judged closer than round
  1 on all six Inspect pairs and on five of six first-person pairs (one tie). Main gaps: guns about 20% large with too
  little cant in first person, the Infinity shroud still too pink, the fire pistol's barrel too light, no ink outlines.
- **Checks:** CTest 10/10; `tests/weapon_paint_test.py` 34 OK, `tests/weapon_recipe_test.py` 14 OK,
  `tests/weapon_stats_test.py` 28 OK; quest suite with the foreground FOV default 79/0 and 11/0. Inventory suite in the
  final pass.

## 2026-10-05: Inventory and Skills pages round 12: flat selection band, focus-view extents, compare highlight, Skills chrome

AI-assisted (Claude). Page code only (`inventory.js`, `skills.js`); no change to `src/`, `CMakeLists.txt` or package
parsing. Details: `docs/verification/INVENTORY_MOVIE_PROTOTYPE.md` (round 12).

- **Band overrun:** the movie's highlight symbol has a glow tail that ran about 20 px past the panel's right edge. The
  focus view now draws a plain filled band (colour sampled from a real capture) inside the panel frame.
- **Extents:** the focus panel is 15 px shorter, the list starts 7 px higher, the hint line is lifted 9 px and the
  Inventory tab group shrinks 7%. Compare from equipped keeps the backpack tile highlight (clipped at the tile) and
  the compare hint has no Sort entry. Skills tab group and footer re-placed so the hint clears the Siren plate. Card
  text is 16 with bold values; the Inspect card is narrower with a hint strip clipped to its text; the mini equipped
  column hides its clipped title.
- All placements are fits read off the 2026-10-04 captures (UNVERIFIED).
- **Visual review:** a blind A/B critic preferred round 12 over round 11 on 11 of 12 screens (one could not be judged),
  means 7.0 against 5.5 (absolute scores drift about a point between critics). Main gaps: the list clips about one row
  early, compare from equipped should still show the Sort hint, the selected tile in compare from backpack should be
  green, Skills hint about 38 px right of the game's, perspective tilt and glass not reproduced.
- **Checks:** `node tests/inventory_navigation_test.js` 27/27; `tools/test_inventory_actions.ps1` 49 PASS / 0 FAIL /
  0 NOT_RUN / 0 KNOWN_DIVERGENCE (run with the gun lane's preview and paint changes in place).

## 2026-10-05: Phaselock presentation rounds 14-15: why the bubble interior read navy, violet interior

AI-assisted (Claude). Host and import script only (`OpenWillowPhaselockFx.cpp`, `import_phaselock_fx.py`); no change to
`src/`, `CMakeLists.txt` or package parsing. Details: `docs/verification/PHASELOCK_STOCK_DATA.md` (Round 14).

- **Cause:** blend mode, draw order and the colour the template feeds the dark layer all match the game (checked
  with `-owfxscalar` and against the own-words shader notes). The navy, see-through interior came from the host's own
  round 5-8 compensations (dark-layer cap and fade floor, a blue-tinted haze, a blue ring tint).
- **Changes (UNVERIFIED calibrations):** the ring texture's dim disc is tinted violet while its bright rim keeps the
  white-blue tint; dark-layer cap 0.8 and fade floor 0.65; the purple wisps at alpha x0.6 and colour x5. A compact
  ground light (round 14) made the ground disc disappear and was reverted in round 15.
- **Visual review:** a blind A/B critic preferred round 14's interior but not its missing ground disc; round 15 against
  round 13 was a tie on the bubble and release sheets and narrowly better on the hand sheet (about 6/10). This lane
  stops here. Open: interior still too translucent, side streaks (the streak sprites take a random rotation whose
  rule was not read), the 0.80 s whiteout, straight release shards, ice-shard flash, fist clench, target animation.
- **Checks:** quest suite with the foreground FOV default 79/0 and 11/0; CTest and `verify_packages` unaffected
  (no `src/` change) and green in this session.

## 2026-10-05: Sanctuary ambient citizens round 3: attachment transforms, zone colours, the grey stand-ins

AI-assisted (Claude). Host and tools (`OpenWillowAmbient.cpp/.h`, `ambient_npc_assets.py`, `ambient_npc_attach_editor.py`,
`prepare_ambient_world.py`, the real-game `ambient_npcs.py` script); no change to `src/`, `CMakeLists.txt` or package
parsing. Details: `docs/verification/SANCTUARY_AMBIENT_NPCS.md` section 9.

- **Hats, hair and gear:** their placement lives on the attached mesh component itself (translation, rotation, scale),
  not on the attachment entry; rounds 1-2 ignored it. Observed on live pawns through the SDK script, now applied.
- **Zone colours:** a host material mixes the diffuse with the pawn's own zone colours through its light map and zone
  mask. The mixing formula is applied by analogy with the weapon master reading and is `UNVERIFIED` for the NPC master.
- **Grey citizens in the 2026-10-04 showcase:** the first version of that material failed to compile (a linear sampler
  given an sRGB default), so UE drew the default material. Fixed; the shot tour log has no compile failure and 35 of
  35 pawns render textured.
- **Visual review:** a blind A/B critic judged round 3 a little closer than round 2 (about 5.9 against 5.6 per stop;
  the female now reads as female). Main gaps: packs float beside the shoulder, some heads read as flat discs, the
  palette is warmer than the game's night-lit blue-grey, the wall-bang and kick idles do not read, weak face cues.
- **Checks:** `tools/test_ambient.ps1 -Seconds 60` PASS (35 pawns, 35 reached a node, 24 at a perch); quest suite with
  ambient on and off 79/0 and 11/0; CTest 10/10.

## 2026-10-05: Inventory and Skills pages round 13: list height, compare hints, focus spacing, Skills footer

AI-assisted (Claude). Page code only (`inventory.js`, `skills.js`); no change to `src/`, `CMakeLists.txt` or package
parsing. Details: `docs/verification/INVENTORY_MOVIE_PROTOTYPE.md` (round 13).

- The focus view shows 7.6 rows, ending near y 625 like the game (was 6.5); the panel title sits 8 px higher and the
  sub-header-to-tile gap is about 12 px. The Sort hint shows in compare from equipped and not in compare from backpack,
  as observed. Skills footer moved 38 px left, tab group 5 px down, description text one size smaller.
- The moved tile in compare from backpack gets a host-drawn green frame (colour chosen by eye); the critic did not see
  it in the frames, so it is an open item.
- All placements are fits read off the 2026-10-04 captures (UNVERIFIED).
- **Visual review:** a blind A/B critic preferred round 13 over round 12 on 9 of 12 screens (3 ties), means 6.8 against
  5.8. Main gaps: compare-from-backpack tile frame and list position, Skills card height and line wrap, Inspect card
  height (no part or flavour lines), perspective tilt and glass.
- **Checks:** `node tests/inventory_navigation_test.js` 27/27; `tools/test_inventory_actions.ps1` 49 PASS / 0 FAIL /
  0 NOT_RUN / 0 KNOWN_DIVERGENCE.

## 2026-10-05: Inventory and Skills pages round 14: compare tile frame, list positions, Skills tree height

AI-assisted (Claude). Page code only (`inventory.js`, `inventory.html`, `skills.js`); no change to `src/`,
`CMakeLists.txt` or package parsing. Details: `docs/verification/INVENTORY_MOVIE_PROTOTYPE.md` (round 14).

- The stray yellow rectangle in compare view was the HTML hit box's browser focus ring, drawn around the larger hit
  box; it is off (the movie draws the selection). The moved tile's green frame is thicker with a light tint.
- Compare and equipped lists start about 30-34 px higher ("WEAPONS" near y 170 as in the game); Skills trees, action
  bar and HARMONY label 20 px lower, the card 2.5% taller; focus hint and backpack plate nudged.
- All placements are fits read off the 2026-10-04 captures (UNVERIFIED).
- **Visual review:** a blind A/B critic preferred round 14 over round 13 on 10 of 11 scored screens (1 tie), means 7.2
  against 7.0. Main gaps: Skills vertical layout (icon pitch 67 against 72 px, plate low), Skills card wraps to five
  lines against six, equipped-view header collision, compare lists one row short, the game's solid lime fill on the
  moved tile, perspective tilt and glass.
- **Checks:** `node tests/inventory_navigation_test.js` 27/27; `tools/test_inventory_actions.ps1` 49 PASS / 0 FAIL /
  0 NOT_RUN / 0 KNOWN_DIVERGENCE.

## 2026-10-05: Sanctuary ambient citizens round 4: the attachment frame, read from a live sample

AI-assisted (Claude). Tools and a synthetic test (`prepare_ambient_world.py`, the real-game `ambient_npcs.py` script,
`tests/ambient_transform_test.py`); no change to `src/`, `CMakeLists.txt`, package parsing or host C++. Details:
`docs/verification/SANCTUARY_AMBIENT_NPCS.md` section 10.

- **Cause of floating packs, disc heads and sideways hats:** the bone frame and the worn piece's own transform were
  combined in the wrong convention. One live sample (bone world matrices of two bones on 33 pawns, saves backed up
  and restored, driver removed) showed that the extracted bone frames are the live ones turned 180 degrees about the
  bone's X axis, and that imported static meshes keep the original coordinates while the imported skeleton is
  mirrored. Only the new composition puts the pack behind the middle of the back with the live matrices. The result
  is a reflection, carried as one negative scale; the synthetic test checks the decomposition (4 OK, needs numpy).
  `UNVERIFIED` beyond that one capture and two bones; the helper's attempt to read the pieces' own world matrices
  failed, so the live bounds were not captured.
- Palette and idles unchanged (palette still `UNVERIFIED`; the night-lit blue-grey in some real frames is lighting).
- **Visual review:** a blind A/B critic preferred round 4 over round 3 on six of seven stops (one tie), about 6.6
  against 4.4 per stop. Main gaps: a hair bun floating behind one head, a background pawn's mask, flat faces from
  above, weak ink outline, packs unconfirmed on front-facing pawns.
- **Checks:** `tools/test_ambient.ps1 -Seconds 60` PASS (35 pawns); quest suite with ambient on and off 79/0 and
  11/0; CTest 10/10; `verify_packages` all packages matched.

## 2026-10-05: Inventory and Skills pages round 15: equipped header, compare rows, Skills plate and wrap, Inspect hint

AI-assisted (Claude). Page code only (`inventory.js`, `inventory.html`, `skills.js`); no change to `src/`,
`CMakeLists.txt` or package parsing. Details: `docs/verification/INVENTORY_MOVIE_PROTOTYPE.md` (round 15).

- Equipped view: the panel title sits 13 px higher, so it no longer collides with the first sub-header. Compare
  views show 7.6 rows like the focus view; the moved tile has a translucent lime fill (solid would hide the gun art).
- Skills: trees 7 px higher, the SIREN plate 22 px higher with the card kept in place, description text one size
  larger so it wraps to six lines like the game, MOTION name label hidden as in the game. Inspect: hint strip moved
  right and the right-edge fade is off while inspecting.
- All placements are fits read off the 2026-10-04 captures (UNVERIFIED). This lane stops here.
- **Visual review:** a blind A/B critic preferred round 15 over round 14 on 5 of 12 screens and tied the rest, means
  7.4 against 6.9. Open: perspective tilt and glass (Ruffle ignores the movie's 3D transforms), Skills action plate
  about 25 px low and line pitch 25 against 22 px, Inspect card height (no part or flavour lines) and watermark,
  black gun parts keyed out as holes in Inspect, selectable empty cells, gear compare not observed in the real game,
  the Phaselock HUD sigil.
- **Checks:** `node tests/inventory_navigation_test.js` 27/27; `tools/test_inventory_actions.ps1` 49 PASS / 0 FAIL /
  0 NOT_RUN / 0 KNOWN_DIVERGENCE.

## 2026-10-05: Guns rounds 3-4: first-person placement from the weapon type's view offset and mesh FOV

AI-assisted (Claude). Host and tools (`OpenWillowWalker.cpp/.h`, new `tools/weapon_view_model.py`, a debug mode in
`import_weapon_paint.py`); no change to `src/`, `CMakeLists.txt` or package parsing. Details:
`docs/verification/WEAPON_VISUALS.md` sections 9-10.

- **Round 3** read the foreground FOV 45 as a vertical angle. A blind A/B split by type: better pistols, worse long
  guns. Not committed on its own.
- **Round 4, cause:** the host never placed the arms as the game does. Read through the SDK on five held weapons:
  the arms origin is the view point plus the weapon type's `PlayerViewOffset`, and the controller's foreground FOV is
  the type's `FirstPersonMeshFOV` (45; SMG 50). The cooked values are read from the weapon types by the new script
  into an ignored `weapon_view.json`; the walker applies them on weapon select, plus the idle clip's Camera-bone
  offset. Host bone positions then match the live ones to about 0.1 cm. With that placement the 45 fits as a
  horizontal angle on all six guns (silhouette widths within 3-12% of the real frames). The data values and the live
  equality are read facts; how the engine applies them and the angle's axis are `UNVERIFIED`.
- **Exposure:** the maintainer saw all-white guns. Those were the lane's solid-white silhouette runs
  (`OW_Debug` 4, measurement only). In normal runs the in-silhouette brightness is within about 25 levels of the real
  frames; the real Jakobs metal is near-white too and the real frames are lit blue. No exposure change.
- **Visual review:** a blind A/B critic preferred round 4 over the committed round 2 on five of six guns (one tie),
  about 7.0 against 5.3 per gun. Main gaps: paint too warm (Infinity, rifle and shotgun barrels), the plain pistol
  slightly low and large, long-gun bodies running a little far right, the rifles' forearm tint.
- **Checks:** CTest 10/10; `tests/weapon_paint_test.py` 34 OK, `weapon_recipe_test` 14 OK, `weapon_stats_test` 28 OK;
  `verify_packages` OK; quest suite 79/0 and 11/0. The Phaselock cast hand moves with the arms; re-checked separately.

## 2026-10-05: Sanctuary ambient citizens round 5: ink line sized in pixels and on worn pieces

AI-assisted (Claude). Host and tools (`OpenWillowAmbient.cpp/.h`, `prepare_ambient_world.py`,
`ambient_npc_attach_editor.py`, `seed_ambient_npc_assets.ps1`); no change to `src/`, `CMakeLists.txt` or package
parsing. Details: `docs/verification/SANCTUARY_AMBIENT_NPCS.md` section 11.

- The ink hull was a fixed 0.5 cm, about 1 px at tour distance; the real game's line looks 2-3 px at any distance
  (by eye on one frame). The hull thickness now follows the camera distance for a target of 3 px (`UNVERIFIED`), and
  hats, hair and packs get their own hull. `-Steps outline` rebuilds only the ink material (a failed editor run had
  deleted it during this round; rebuilt).
- The floating "hair bun" at stop 00 is the raised fist of the wall-bang clip seen behind the head, not a worn piece.
  Whether the fist meets the wall depends on the real perch's wall distance (`UNVERIFIED`). Packs confirmed on the
  back from behind.
- **Visual review:** a blind A/B critic preferred round 5 over round 4 on every stop, about 6.0 against 3.4 (absolute
  scores drift between critics). This lane stops here. Open: line width still uneven (1.3-5 px) and navy rather than
  black, a gap between the line and the body on thick lines, creases drawn inside the silhouette, a flat untextured
  cap and a featureless face from above, body garment variants, Resistance patrols, matched real close-ups.
- **Checks:** `tools/test_ambient.ps1 -Seconds 60` PASS (35 pawns); quest suite with ambient on and off 79/0 and
  11/0; CTest 10/10; `verify_packages` exit 0; `tests/ambient_transform_test.py` OK; module build exit 0.

## 2026-10-05: native census of the Fire mission's script call graph

AI-assisted (Claude). Phase 2 step A. Added opt-in call counters to `vm::Runtime` (filled in `Interp::invoke`; off by
default, existing callers unaffected), `ow-package --native-census` (`src/census.*`; entry file
`tools/slice_native_census_entries.txt`, 319 entries) and `tools/slice_native_census.py`; record in
`docs/verification/NATIVE_SLICE_CENSUS.md` (names and counts only).

- Result: 126 natives reached dynamically, 577 in closure A (primary static edges), 1,934 in closure B (upper bound with
  subclass overrides and interface implementers); 24 of the 514 non-operator natives in A are implemented. All 319 entries
  complete on default objects with stubbed natives, but only 90 enter a second script function: the dynamic numbers are a
  floor.
- Verified (automated): counters and closures on a synthetic package (`tests/vm_test.py`), every dynamically entered function
  inside the static closure B, CTest 10/10, packages 9/9.
- UNVERIFIED: every number as a description of the real game (no trace was taken); the static resolution of virtual and
  interface calls; calls made by native code back into script (the entry list stands in for them).
- Finding, left unchanged here: `ObjectConst` of a class export evaluates to a stand-in object because class exports have
  class reference 0 (`src/interp.cpp`), so static calls through class constants (e.g. `GetWillowGlobals`) do nothing in the VM.
- Sensitive file: `CMakeLists.txt`, one source (`src/census.cpp`) added to `ow-core`; nothing else changed.

## 2026-10-05: native notes for the mission script bridge: accept, complete, kickoff, rewards (UNVERIFIED)

AI-assisted (Claude), analyst lane C1, Ghidra; own-words note `docs/verification/NATIVE_MISSION_SCRIPT_BRIDGE.md`, no
listing text. Accept and turn-in are script (`WillowPlayerController.AcceptMission` / `ServerCompleteMission`) driving the
natives `MissionTracker.ActivateMission` / `CompleteMission`, which share one status routine. That routine calls back into
script (`WillowPlayerController.UpdateMissionStatus` on each local controller) before observers and the `Default` event (id
6 + status). Rewards are granted by script: `UpdateMissionStatus` → `ServerGrantMissionRewards` → credits, `ExpEarn`,
items, reward UI. The kickoff after acceptance is a pending record the tracker's tick consumes (`Default` id 12 one tick
after id 7), answering an open item of NATIVE_MISSION_DISPATCH. `ExpEarn` only raises the pool; the level-up comes from
the pool's update. Corrections to NATIVE_MISSION_DISPATCH B4 (hook order) and NATIVE_PROGRESSION (`ExpLevelUp(bCheated)`)
are recorded in the new note with pointers from both. Every rule UNVERIFIED in game except the 395 XP amount (2026-10-02).

## 2026-10-05: native notes on loot rolls (UNVERIFIED) and a native index

AI-assisted (Claude), analyst lane G1, Ghidra; own-words note `docs/verification/NATIVE_LOOT.md`, no listing text. How an
item pool picks entries (one flat weighted list per pool, stage window, `Quantity` draws with replacement, nested pools
recurse), how a balance becomes an item (first grade window containing the capped stage, spawn-modifier interpolation,
level = capped stage), enemy drop lists and the death roll, mission reward items. Confirms that the Fire mission's stock
data carries no reward item, so the turn-in loot remains a labelled host stand-in. The note lists corrections to
`tools/loot_pools.py` and NATIVE_WEAPON_RULES section 4 (not applied yet). All UNVERIFIED in game.
`docs/verification/NATIVE_INDEX.md` starts a one-row-per-native index over the note files (mission script bridge and loot).

## 2026-10-05: native notes on stock Engine/Core natives: timers, spawn, iterators, states (UNVERIFIED)

AI-assisted (Claude), analyst lane G3, Ghidra; own-words note `docs/verification/NATIVE_ENGINE_CORE.md`, no listing text.
Actor timers (replacement, clear-by-zero-rate removed on the next update, strict greater-than firing, loops firing
`floor(count/rate)` times per pass, tick order Tick → state code → timers → LifeSpan → physics), `Spawn` (the tag is not
used; event order GainedChild → PreBeginPlay → PostBeginPlay → SetInitialState) and `Destroy`, the actor iterators (lazy
walks, start indices, filters), `GetALocalPlayerController` / `GetWorldInfo`, and the state machine (EndState/BeginState
order, default label `Begin`, push/pop events, probe-mask gating by `Enable`/`Disable`, latent `Sleep`). Much of the
engine-side machinery has no registered name and was identified by call structure; the note marks those identifications.
Differences from our code are listed in the note (`src/mover.cpp` timer validation, `src/natives_core.cpp` state model)
and are not changed yet. All UNVERIFIED in game.

## 2026-10-05: VM: a class constant evaluates to its class

AI-assisted (Claude), implementer lane I1. `EX_ObjectConst` in `src/interp.cpp` decided "is a class" by the name of the
export's class reference; class exports have class reference 0, so class constants became stand-in objects and static
calls through them (e.g. `GetWillowGlobals`) did nothing. Class reference 0 now means a UClass (the rule `vm.cpp`'s
`classNameOf` already used). Verified: new synthetic case in `tests/vm_test.py` (fails before, passes after); the
real-data `--slice-run` and `--inventory-move` outputs are byte-identical before and after; the mover scripts' only class
constant sits in a `foreach` header the interpreter skips. Door suite not rerun for this step.

## 2026-10-05: script swap 1: Fire mission accept and turn-in run the installed controller script (UNVERIFIED rules)

AI-assisted (Claude), implementer lane I1, written from `docs/verification/NATIVE_MISSION_SCRIPT_BRIDGE.md` only (no
analysis output opened). Phase 2 step D, first stand-in replaced.

- `FireMissionSlice::accept` runs `WillowPlayerController.AcceptMission` and `turnIn` runs `ServerCompleteMission` on a VM
  controller (authority role, `WorldInfo.GRI.MissionTracker` → a VM `MissionTracker`) with the installed mission object.
  New `src/mission_script.*` binds, to the bridge's own objects only, `MissionTracker.ActivateMission`, `CompleteMission`,
  `PlayTurnIn`, `GetMissionStatus` per the note, driving `MissionSystem`, which stays the single owner of mission state.
  Each status change calls the script `UpdateMissionStatus` and `TriggerMissionStatusChangedDelegates` before observers and
  the `Default` event. The host stand-in "kickoff at once" is replaced by the note's pending record consumed on the next
  tick (the 22 host events on real data are unchanged; the three kickoff events move from accept to the next tick).
- Inferred without a note (labelled in code): `MissionTracker.IsDataValid` (true), `NativeGetMissionIndex` (list lookup).
  Stand-ins kept: `ExpEarn` only records its call (the host still grants XP); `GetExperienceReward` returns the host's
  amount. On real data the script reaches `ExpEarn(395, SideMissionAward)`, equal to the host's 395 by construction.
  Not modelled: `CompleteMission`'s chain, untracking, unlock queue and fast-forward prompt. The slice refuses a turn-in
  that is not ReadyToTurnIn (the real screen offers it only when `CanEndMission` holds).
- 19 natives remain logged stubs on this path (listed in `SANCTUARY_RPG_MISSION.md`, "Script swap 1"); their zero results
  match the Fire mission's data. Known VM gap: static-array fields inside a zero struct are not materialised (reward
  struct reads log out-of-bounds; harmless for this mission).
- Checks: CTest 11/11 (new `mission-script-synthetic`), packages 9/9, UE module build Succeeded, quest suite first run
  PASS 80/80 (79 before, one new check) and resume PASS 11/11. Sensitive file: `CMakeLists.txt` (one source and one test
  added). All native rules UNVERIFIED in game.

## 2026-10-05: native notes on mission-linked behavior conditions, population spawning and kernel leftovers (UNVERIFIED)

AI-assisted (Claude), analyst lane G2, Ghidra; own-words note `docs/verification/NATIVE_BEHAVIOR_POPULATION.md`, no listing
text. `BehaviorSequenceEnableByMission`: every `MissionReaction*` recomputes the verdict from the tracker's state and
re-applies Enable/Disable to all linked sequence records (events fire only on a real change); objective states
NotStarted/Active/Complete are derived with mission-status gating; `ObjectiveSetRestrictions` apply; the condition
registers as an observer at provider registration and gets an immediate level-load call (before `OnSpawned`).
`SequenceEventEnableByMission` uses the same verdict. Population: `MissionPopulationAspect` is script; the Fire den is
enabled only while its objective is active and spawns on a later master tick under radius/height/capacity/time gates;
the spawn order up to `OnSpawned` is recorded. Kernel: a thread stops at the next behavior boundary when its sequence is
disabled; context-list resolution; latent copies with a 1/60 s minimum wait; `FilterObject` only with a caller filter.
Corrections recorded in the note: the enable rule in `src/slice.hpp`/BEHAVIOR_DATA_DECODE (not applied yet), the
set-changed notification source (NATIVE_MISSION_DISPATCH B3), and `tools/ghidra/class_layout.py` offsets on
`WillowPawn`-derived classes (4 bytes lower than the executable from `ConsumerHandle` on; cause not investigated). All
UNVERIFIED in game.

## 2026-10-05: native notes on controller and helper natives the mission script calls (UNVERIFIED)

AI-assisted (Claude), analyst lane G4, Ghidra; own-words note `docs/verification/NATIVE_CONTROLLER_HELPERS.md`, no listing
text. Covers the census's top helpers: `GetCurrentPlaythrough` (GRI's `CurrentPlaythrough`, else 0),
`NativeGetMissionIndex` (first `MissionList` entry with the same `MissionDef`, else -1; matches the provisional
implementation of script swap 1), `MissionTracker.IsDataValid` (the tracker's `bDataValidated` bit set by `ValidateData`,
not a constant: the provisional `true` is a stand-in), `GetHUDMovie`, `CanAffordToUseUsableObject` / `PayForUsedObject`
and the currency caps, `GetPawnInventoryManager`, `IsPrimaryPlayer`, `WorldInfo.IsMenuLevel`, the globals getters,
`WillowAIPawn.IsComponentUsable`, `Object.Localize` (search roots, `INT` fallback, `?INT?Package.Section.Key?` for a
missing entry) and `Object.QueryInterface`; presentation-only natives are marked as such. All UNVERIFIED in game.

## 2026-10-05: native notes on the UnrealScript–Scaleform bridge (UNVERIFIED)

AI-assisted (Claude), analyst lane G5, Ghidra; own-words note `docs/verification/NATIVE_GFX_BRIDGE.md`, no listing text.
The contract the host's Ruffle adapter must meet: the `ActionScript*` family forwards the calling script function's own
parameters and converts the result to its return type (conversion tables in the note; missing movie or path is a silent
no-op); `Invoke`/`SetVariable*`/`GetVariable*` conversions by `ASType`; ActionScript→script calls through
`ExternalInterface.call` (named script function on the player's `ExternalInterface` object), `SetFunction` bindings,
`fscommand` and the CLIK widget hooks (the VM must be re-entrant there); wrapper lifetime and `Close` event order; markup
translation in `SetText`; `PlayUISound`; `FocusOn` (presentation only); `QuestAcceptGFxMovie.UpdateMissionTextList` (the
accept screen's category headers). Scaleform's own semantics (paths, sticky variables, conversions) were not read. All
UNVERIFIED in game.

## 2026-10-05: script swap 2: Fire mission experience through the script path (UNVERIFIED rules)

AI-assisted (Claude), implementer lane I1, from NATIVE_PROGRESSION.md and NATIVE_MISSION_SCRIPT_BRIDGE.md only. The host's
own XP computation is gone: `UpdateMissionStatus(Complete)` → `ServerGrantMissionRewards` → `GetExperienceReward` +
`ExpEarn(amount, 4)` on the VM, then the pool update runs the script `ExpLevelUp` → `OnExpLevelChange`.

- New `src/progression.*`: a bounded attribute evaluator (constant, simple-math and global-level resolvers, value formula,
  conditional on `PlayThroughCount`, range and rounding; any other shape throws) and the experience curve.
  `GetExperienceReward` = `trunc(float(span × percentage × m))` at the mission's locked game stage (m = 1 first playthrough
  below 50; other cases throw "not implemented"). `ExpEarn` scales (taken as 1: not decoded), clamps to the maximum level's
  XP and never decreases a VM-side pool. Also from the bridge note: `GetGameStage`, `GetMaxExpLevel`,
  `GetExpPointsRequiredForLevel`, `GetCurrencyRewardType`, `GetCurrencyReward` (multiplier 0 only),
  `ShouldGrantAlternateReward`, `GetItemRewardsForPlayer` (empty rewards only; pool rolls logged as not implemented).
- Design: the VM owns the pool and level; the host keeps Maya's display, skill points, health and save, pushes its inputs
  (region stage, level, experience) before accept and turn-in, applies the `Experience` event and compares the `Level`
  event with its own level. The host's earlier formula remains only as the suite's oracle.
- VM fix: struct default tags fill a static array (`ArrayDim` > 1) element by element (before, the last tag replaced the
  whole field). `ProviderDefinitionPathName.PathComponentNames` became an array as a result; `providerPathLeaf`
  (`src/behavior.*`) reads its last name for the two users. The slice's 22 host events are unchanged.
- Real data: 395 XP at stage 8 (the amount confirmed in game on 2026-10-02), 316 at stage 7; level 8 → 9 by the script from
  27,900 XP. Turn-in stubs 19 → 14.
- Checks: CTest 11/11 (synthetic invented curve, locked stage, level-up, clamps; static-array struct case), packages 9/9,
  UE build Succeeded, quest suite first run PASS 81/81 (new check `script_pool_update_levels_up_to_host_level`) and resume
  PASS 11/11, door suite PASS 16/16, inventory suite PASS 49 / FAIL 0 / NOT_RUN 0 / KNOWN_DIVERGENCE 0. Sensitive file:
  `CMakeLists.txt` (`src/progression.cpp` added). UNVERIFIED: XP scales = 1, the conditional "all expressions hold",
  the BaseValueMode numbering, everything but the 395 amount.

## 2026-10-05: native notes on the dialog system: event selection, talker, priority, line end (UNVERIFIED)

AI-assisted (Claude), analyst lane G6, Ghidra; own-words note `docs/verification/NATIVE_DIALOG.md`, no listing text.
`Behavior_TriggerDialogEvent` selects Out (id 0) on its first run and Finished (id 1) only once the event's talk act is no
longer live (polled every 0.1 s; at once when no line starts; immediate mode selects both), so the host stand-in that
selects both at once is wrong in timing. Event selection within a group, template-event wiring (a data-layout check on
the Fire group agrees with SLICE_AUDIO_CHAIN's pairing), talker resolution (talker variable, instigator, random
`TalkData` by name tag; echo callers), the priority rule (index in the globals' `Priorities`, tracked-mission floors), and
the line end (Wwise playing id stops, then `OutputDelay`). Predicts that the Fire mission's first objective set starts
after Marcus's first line ends, not at the kickoff. All UNVERIFIED in game.

## 2026-10-05: native notes on skills: points, upgrades, grade effects, cooldown (UNVERIFIED)

AI-assisted (Claude), analyst lane G7, Ghidra; own-words note `docs/verification/NATIVE_SKILLS.md`, no listing text. Skill
points are awarded by the script `ExpLevelUp` (`GeneralSkillPoints += PointsPerLevelUp` after the level rises; the
confirmed `max(0, L − 4)` total follows) and announced through `FireSkillPointsChangedDelegates`. Spending: UI
`RequestSkillUpgrade` → `CanUpgradeSkill` → `ServerUpgradeSkill` → native `PlayerSkillTree.UpgradeSkill` (refusals, tier
unlock by summed `PointsToUnlockNextTier`, child branches; the level-5 gate is UI-only). Grade to effect:
`Base + PerGradeUpgrade × ((g − start) div max(interval, 1)) + bonus` (bonus = the single best `BonusUpgradeList` entry),
modifiers applied through the attribute stack and refreshed on the next tick after a grade change. Cooldown pool refill and
drain (the Phaselock pause reading is consistent). `tools/skill_stats.py` lacks the bonus rule and defaults
`GradeToStartApplyingEffect` differently (not changed yet). All UNVERIFIED in game.

## 2026-10-05: script swap 3: mission-linked sequence conditions and the controller helpers (UNVERIFIED rules)

AI-assisted (Claude), implementer lane I1, from NATIVE_BEHAVIOR_POPULATION.md section 1 and NATIVE_CONTROLLER_HELPERS.md
only.

- The dummy's `BehaviorSequenceEnableByMission` conditions replace the earlier UNVERIFIED rule in `src/slice.*`:
  `MissionSystem` raises the tracker's observer notifications (status change after the script hook and before `Default`,
  active-set switch, objective progress, objective complete), and every notification recomputes each condition (objective
  state with mission-status gating, objective-only bit for objective-specific conditions, `ObjectiveSetRestrictions`,
  mission-level bits) and applies Enable/Disable, with events only on a real transition. The provider registers as a
  consumer at `spawnDummy` (bEnabledOnSpawn pass, then the immediate level-load verdict, then `OnSpawned`); nothing observes
  the mission before the spawn. `bSequenceEnabledMutex` and the enabled/disabled event order follow the note (no dummy
  sequence sets the flag; synthetic test only).
- Real data: the 22 host events are identical and in the same order, the final enabled sequences are the same. New:
  `Idle`'s enabled event now fires at registration and reaches `Behavior_SpecialMove` (an animation request), listed as a
  host-boundary call that the host does not run yet.
- Helpers: `IsDataValid` reports the tracker's `bDataValidated` flag, set by running the script `ClientValidateMissionData`
  once when the VM graph is built (a slice shortcut: the standalone trigger is not known); `GetCurrentPlaythrough`,
  `NativeGetMissionIndex` (range-checked), `IsPrimaryPlayer`, `GetHUDMovie` (None), `IsMenuLevel`, the globals getters and
  `GetGlobalsDefinition`; `UpdateLcdMissionStatus` and `PlayUIAkEvent` as documented no-ops. Turn-in stubs 14 → 12 (7 gone,
  5 newly reached further down the script: `Localize`, `AllExpansionSideMissionsComplete`, `IsLocalPlayerController`, a
  `SpawnPlayerMovie` call on a data-object stand-in, an engine iterator).
- Not modelled: per-instance objectives, `bInstanced`, the Kismet twin `SequenceEventEnableByMission`, the
  RequiredObjectivesComplete / Failed statuses, `ObjectiveCleared`.
- Checks: CTest 11/11 (new synthetic slice scenario on invented packages), packages 9/9, UE build Succeeded, quest suite
  PASS 81/81 and resume PASS 11/11, door suite PASS 16/16. No sensitive file touched.

## 2026-10-05: native notes on the use key: usable choice, mission giver, accept screen (UNVERIFIED)

AI-assisted (Claude), analyst lane G8, Ghidra; own-words note `docs/verification/NATIVE_USE_INTERACTION.md`, no listing text.
The current usable object comes from one view ray of `PlayerInteractionDistance` (350 uu in the stock globals), first
usable hit nearest-first, a blocking hit ends the search, no angle cone or type priority, at most 30 updates per second
(so the host's 250 cm reach and sweep are not the stock rule). The use key runs script (`Use` → `ServerUse` →
`PerformedUseAction` → `CanAffordToUseUsableObject` → `UseObject` → `PayForUsedObject`); `WillowAIPawn.UseObject` fires
the AI `OnUsed` events (Generic, then HasMissions/NoMissions); Marcus's `Brain` behavior falls through his story checks to
`Behavior_ShowMissionInterface`, which opens the mission screen with Marcus as context; the screen's buttons call the
`AcceptMission` / `ServerCompleteMission` scripts already running on the VM (script swap 1). Talk state: `BeginUse` /
`EndUse` at screen open/close, 30 s linger, look-at, focus camera. The trace flag word is recorded as a number with a
labelled guess at its bits. All UNVERIFIED in game.
