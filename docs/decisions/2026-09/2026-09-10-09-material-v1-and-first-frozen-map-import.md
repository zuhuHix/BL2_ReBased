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

