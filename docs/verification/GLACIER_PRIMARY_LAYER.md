# Glacier primary-layer approximation (2026-09-14)

Checkout `t3code-409c3042`, following the
[fresh Sanctuary baseline](SANCTUARY_MATERIAL_BASELINE.md).

## Scope and evidence

The two inspected glacier family members now use their main diffuse and
normal textures, with retained RG tiling from `P_TexScalar_RGMain_BASnow`:
base `(1,1)`, instance `(3,3)`. The original RGBA values remain in the manifest
as `(1,1,2,2)` and `(3,3,2,3)`. Selection requires exact family paths, the
four inspected texture paths/classes, and finite scale values. It does not
generalize a texture-name heuristic to other layered materials.

The recipe deliberately remains `partial_unverified`. Snow blend, reflection,
glow and native static UV selection are not reconstructed. It uses UV0 as an
explicit approximation. Both affected reusable meshes (GlacierFlow_Straight and
SlateBoulder_02_SimpleCollision) have two source UV sets. No extra source UV
channel is exported or imported by this change.

Inspection of the native instance tail suggested a false/non-overridden static
switch record matching the surviving expression GUID, but no supported
static-permutation decoder or independent reference establishes that reading.
Do not report the second-UV setting as verified from this observation. The
production recipe does not inspect those bytes.

All four glacier resource textures decode. Both diffuse PNGs have alpha 255
throughout; alpha therefore does not recover the missing snow blend mask.
The exact blend function cannot be established from the surviving parameters
and texture list alone.

## Counts

The 372 material definitions and 4,768 placed sections are unchanged.

| Category | Before | After |
|---|---:|---:|
| Opaque materials lacking diffuse | 16 | 14 |
| Their placed sections | 76 | 43 |
| Opaque materials with no supported channels | 13 | 11 |
| Their placed sections | 54 | 21 |
| Explicit partial glacier recipes | 0 | 2 |
| Sections using those partial recipes | 0 | 33 |

The audit retains the last two counts so a textured partial surface is not
mistaken for completed material fidelity. Fifteen unassigned slots remain.

## Validation

- CTest: `100% tests passed out of 5`.
- All nine installed code packages' decoded bytes, counts and export fields
  still match the independent reader.
- Scene tests: 15 passed. Recipe tests cover inherited/overridden scale,
  changed texture sets, wrong texture class, unknown instances, nonfinite scale,
  explicit diffuse/null precedence and explicit null normal preservation.
- Material audit test: one passed, including partial-surface accounting.
- Before/after manifest comparison confirms only the two intended material
  definitions changed; actor placements, meshes and camera are identical.
  Evidence: `local/material-investigation/glacier-change-scope.json`.
- Three-object comparison scene imported/reopened and passed UV verification.
  In the rendered start view, left: instance tiling. Center: primary recipe.
  Right: old neutral fallback.
  All three use the same original GlacierFlow_Straight mesh at equal scale.
- The host verifies saved TextureCoordinate UV index and RG tiling, as well as
  the existing texture/color-space and mesh UV/winding checks.
- Probe viewer passed. Its start screenshot was visually inspected:
  `GlacierSurfaceProbe_P_start00000.png`. Ice texture detail and the difference
  between 1x and 3x tiling are visible on equal-size meshes, alongside the old
  neutral fallback. Log:
  `local/glacier-probe/viewer-9831239d34744aea822df5bcf5714ed4.log`.
- Full Sanctuary reimport succeeded (zero errors, eight existing Interchange
  console lookup performance warnings). Saved scene, collision and UV/winding
  verification each finished with zero errors/warnings. All 4,768 section
  actors, 495 mesh sections and 3,116 collision-enabled components are retained.
  Log: `local/glacier-import-verified.log`.
- Full Sanctuary viewer passed after the reimport (8,483.24 cm displacement).
  Log: `local/sanctuary/viewer-73bbd0df17c24b1b8bd24f9a25589835.log`.
  `Sanctuary_P_start00001.png` was inspected: the street view retains the
  earlier black sky, green patches and box-shaped surface. Those are separate
  open defects; the controlled glacier probe is the visual evidence for this
  specific material change. No matched original-game comparison was performed.

Game-derived evidence is local only: `local/glacier-probe/`,
`local/material-investigation/`, and UE `Saved/Screenshots/WindowsEditor/`.
The probe is a controlled inspection scene, not an original-game comparison.

## Reproduce Sanctuary

```powershell
python tools/refresh_materials.py --reader build/Release/ow-package.exe --game 'C:/Program Files (x86)/Steam/steamapps/common/Borderlands 2' --scene local/sanctuary --reuse-textures
./tools/run_ue_level.ps1 -Engine 'C:/Program Files/Epic Games/UE_5.8' -Game 'C:/Program Files (x86)/Steam/steamapps/common/Borderlands 2' -Scene local/sanctuary -ImportOnly -SkipBuild
python tools/audit_scene_materials.py --scene local/sanctuary --output local/sanctuary/material-audit.json
```

`--reuse-textures` and `-SkipBuild` require the unchanged local install and the
already-built current UE module. Snow/ice-family fidelity remains open.
