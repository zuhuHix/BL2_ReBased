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
