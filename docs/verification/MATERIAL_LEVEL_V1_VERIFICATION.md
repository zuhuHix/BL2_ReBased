# Material v1 / Ash first-map verification — 2026-09-11

This is an offline UE5 editor import of a frozen scene, not gameplay or runtime
UPK streaming. All extracted assets, generated UE packages and detailed logs
remain under ignored local/ and host output directories.

## Results

- C++ Release build: passed.
- UE5.8 OpenWillowEditor Development build: passed in this isolated checkout.
- CTest: 5/5 suites passed, including bulk scene metadata and payload bounds.
- Scene unit tests: 5 passed (inheritance/cycles, sublevel cycles and missing
  dependencies, frozen dynamic placement, transforms, host handedness/identity).
- Nine installed code packages still match the independent Python reader.
- Ash_P + Ash_Px, Ash_Light, Ash_Audio, Ash_Combat, Ash_Dynamic, Ash_FX:
  5,059 placements, 303 reusable meshes, 225 materials, 5,235 mesh sections.
  Ash_Dynamic contributes 191 frozen placements.
- Full scene saved, reopened and verified: section count, collection positions,
  rotation axes and signed scale, per-section material overrides, each imported
  mesh section's bounds against referenced source OBJ vertices, and material
  graph/color-space settings. Zero verification errors or warnings.
- Full Ash lighting pass saved and reopened: warm directional sun, neutral-cubemap
  skylight fill, runtime reflection capture, automatic-exposure post-process volume, and
  ambient-occlusion override all present and configured. The import manifest
  records sun intensity 1.0, skylight intensity 0.5 with the neutral gray light
  cubemap, capture radius 16,384,
  automatic exposure, and AO 0.35. The rig is anchored at the start camera to
  avoid pathological UE3 sky/environment bounds.
- Interactive UE5.8 visual check completed after the clean reimport: the editor
  Lit viewport shows textured geometry with directional shading and normal depth
  (`local/final-lit.png`), and a separate `-game` run from the saved map shows
  the same view from the inspection camera (`local/final-runtime-lit.png`).
  These screenshots are local ignored evidence, not distributed assets.
- Synthetic four-channel fixture: saved/reopened diffuse, normal, specular and
  masked emissive graph verified; asymmetric geometry and independent expected
  rotated component position verified.

## Check output

```text
100% tests passed out of 5

Ran 5 tests
OK

Core: 234397 bytes, 1621 exports; decoded bytes, counts and export fields match
Engine: 5878264 bytes, 33166 exports; decoded bytes, counts and export fields match
GameFramework: 61714 bytes, 258 exports; decoded bytes, counts and export fields match
GearboxFramework: 1224040 bytes, 7098 exports; decoded bytes, counts and export fields match
WillowGame: 13054200 bytes, 56443 exports; decoded bytes, counts and export fields match
GFxUI: 136680 bytes, 841 exports; decoded bytes, counts and export fields match
IpDrv: 230751 bytes, 1364 exports; decoded bytes, counts and export fields match
OnlineSubsystemSteamworks: 265760 bytes, 1709 exports; decoded bytes, counts and export fields match
AkAudio: 39503 bytes, 176 exports; decoded bytes, counts and export fields match

UE host build: Result: Succeeded
Ash saved-scene verification: Success - 0 error(s), 0 warning(s)
```

## Remaining limits

The scene report lists 325 unsupported interactive-object component owners and
81 materials without supported named texture parameters (neutral fallback).
All 62 p_Specular overrides inspected in Ash_P are null, so the game scene does
not prove the specular-texture path; the synthetic fixture does. The final
full import reports seven UE Interchange console-lookup performance warnings.
Some source meshes also report degenerate tangent bases during initial build.

The lighting rig is an inspection approximation. It does not load UE3 baked
lightmaps or native level light actors. The reflection capture is local to the
start-camera area and runtime-generated; inspect other areas for falloff and
capture coverage.

Native prefix/collection semantics are observed for Ash, not a general UE3
format guarantee. Terrain/BSP, skeletal meshes, physics, scripting, lightmaps,
transparency and complex material graphs are excluded. The captured frame covers
the start-camera view only; wider-map shader appearance, winding/UV acceptance,
performance and free-flight controls still need interactive validation. Headless
success remains separate from visual or in-game proof.

See [docs/TOOLING.md](../TOOLING.md) for preparation, import and fixture commands. The launcher
`tools/run_ue_level.ps1 -ImportOnly` imports and runs saved-scene verification.
