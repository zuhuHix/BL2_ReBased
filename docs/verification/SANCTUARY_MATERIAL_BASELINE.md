# Sanctuary material baseline and glacier investigation (2026-09-14)

Checkout: `t3code-409c3042`, branch `fix/missing-materials`.
Freshly prepared from the installed base game using this checkout's reader.
No material selection policy or binary-layout interpretation changed in this pass.

## Coverage accounting

Ten packages, 4,430 placements, 423 reusable meshes, 4,768 placed sections,
372 material definitions. The earlier 391-material count predates the current
base-first package resolution. These are manifest counts, not visual acceptance.

`tools/audit_scene_materials.py` follows section slots and non-null actor
overrides, matching the host importer. It distinguishes missing diffuse from
having no supported channels at all, and records affected component identities.

| Category | Material definitions | Placed sections |
|---|---:|---:|
| Textured diffuse | 322 | — |
| Constant diffuse | 1 | — |
| Missing diffuse, all blend modes | 49 | — |
| No supported channels, all blend modes | 46 | — |
| Opaque missing diffuse | 16 | 76 |
| Opaque with no supported channels | 13 | 54 |
| No assigned material | — | 15 |

The previous wording "46 no-diffuse / 13 opaque" conflated the last two
material categories. Icicles have a normal channel but lack diffuse (16 placed
sections); two spire materials have emissive but lack diffuse (six sections).
They explain the 22-section difference between opaque totals.

The unassigned slots belong to resistance banners (six), vending icons (three),
building trim (two), and blocking cubes (four). Assignment and source visibility
need investigation; the count does not establish that all should be visible.

## Priority by placed usage

| Material or family | Sections lacking diffuse |
|---|---:|
| Mat_Glacier and Mati_Glacier2x | 33 |
| Mat_Icicles (normal already present) | 16 |
| Mat_PatchySnow_Skybox01 | 10 |
| Two SancSpire instances (emissive already present) | 6 |
| Mat_MountainDistant | 5 |
| Dirty snow, Sanctuary ice road, moon base, dynamic sky, world map, chemical tank | 1 each |

Three other opaque definitions have zero effective placed uses after overrides:
dry grass rocks, opaque water, and Mat_IceRoad. They should not take priority
over the placed glacier family.

## Glacier findings from existing readers

`Sanctuary_P` export 1987, `Prop_Glacier.Materials.Mat_Glacier`, has 60 expression
slots; 54 are null. Its six surviving parameter expressions retain:

- `P_TexScalar_RGMain_BASnow`: default `(1,1,2,2)`; `Mati_Glacier2x` overrides
  it with `(3,3,2,3)`.
- `p_UseSecondUV`: static switch with stripped A/B expression references.
- `P_SimpleReflect` and `P_SimpleReflectOffset`.
- `p_GlowColor` and `p_GlowRadiusScale`.

The existing bounded resource reader resolves GlacierFront_Nrm, a snow
reflection texture, GlacierFront_Dif, and Snow_Dif. All four decode to PNG with
the current texture importer. GlacierFront_Dif was visually inspected as an
ice-cliff texture. There is no texture-format blocker for these four sources.

These parameters support a layered-material investigation but do not establish
the blend mask, connection order, normal treatment, or glow/reflection formula.
The resource has 32 uninterpreted bytes after its texture list; this is not
evidence that the stripped graph can be recovered from that tail.

The C++ reader retains all UV sets, but `write_obj` in `src/assets.cpp` exports
only `vertex.uvs[0]`. The host currently imports that OBJ. Before implementing
second-UV behavior, establish the actual static-switch value and required source
UV channel, then preserve that channel through the host path. Do not assume
that the existence of the switch means every instance enables it.

## Verification and fresh visual baseline

- C++ Release and UE5 editor module builds succeeded.
- CTest: `100% tests passed out of 5`.
- `verify_packages.py`: all nine installed code packages' decoded bytes,
  counts and export fields match (Core, Engine, GameFramework,
  GearboxFramework, WillowGame, GFxUI, IpDrv, OnlineSubsystemSteamworks, AkAudio).
- `python tests/material_audit_test.py`: one synthetic regression passed;
  covers non-contiguous slots, null overrides, effective replacement, unused
  material definitions, partial channels, constant black, and unassigned slots.
- Fresh import: 4,768 section actors; zero errors, eight Interchange console
  lookup performance warnings. Reopen verification: zero errors/warnings.
- Collision reopen: 495 mesh sections, 3,116 enabled components, zero errors.
- UV/winding: 495 sections, 512,418 corners and 170,806 triangles verified.
  This verifies UV0 transport only, not material graph UV semantics.
- `OpenWillow.Viewer`: passed, 7,307.88 cm displacement. Log:
  `local/sanctuary/viewer-9df6c649edae42b2b5d5ae6fe24abf72.log`.
- `OpenWillow.Walking`: passed on the freshly imported scene. Log:
  `local/sanctuary/viewer-9f59c109d89d4311acea1f0bab438f30.log`.
  This is the existing spawn/fixture regression; manual street and stair
  traversal remains unverified.

The start, moved and overview PNGs were inspected under
`host/ue5/OpenWillow/Saved/Screenshots/WindowsEditor/Sanctuary_P_*00000.png`.
Scooter's sign reads correctly in the full scene, and buildings/roofing carry
texture detail. The sky is black; broad white fallback surfaces remain near
the entrance. Bright green road patches and a large box-shaped surface near
the start require separate material/visibility investigation. Their causes
were not established by these screenshots. The moved view ends partly inside
geometry, so it is a movement check rather than a useful fidelity viewpoint.
No matched original-game comparison was performed. Sanctuary is not visually
accepted and no glacier shader fix is claimed.

The correctness run recorded 90 frame samples: mean 69.66 ms, p95 88.75 ms,
process memory 3,563.7 MiB. These short startup-adjacent diagnostics do not
replace a controlled performance benchmark.

## Reproduce

```powershell
python tools/viewer.py --game 'C:/Program Files (x86)/Steam/steamapps/common/Borderlands 2' --map Sanctuary_P --action prepare
python tools/audit_scene_materials.py --scene local/sanctuary --output local/sanctuary/material-audit.json
python tests/material_audit_test.py
```

Game-derived evidence stays ignored in `local/sanctuary/` and
`local/material-investigation/`. The audit is read-only with respect to the
scene and does not infer a replacement shader.
