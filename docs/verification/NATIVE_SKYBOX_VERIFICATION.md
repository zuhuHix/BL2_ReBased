# Native Sky_Dome import, 2026-09-15

*2026-09-21: the dome material now uses the recorded sky approximation and the
blue host shell is no longer spawned on Sanctuary; see
[NATIVE_SKY_APPROXIMATION.md](NATIVE_SKY_APPROXIMATION.md). The import
policy and limits below otherwise still apply.*

AI-assisted implementation in `t3code-dc91995f`. This is a bounded Sanctuary
skybox import slice. It does not recover the complete UE3 sky graph.

`tools/sky_census.py` found 31 sky-named placements in Sanctuary_P. Two use
`Prop_Skybox.Meshes.Sky_Dome` in `Sanctuary_Light`; one has the effective
`Mati_Sky_Dynamic_INST` Unlit material and the other is overridden with
`Mati_FloorConcrete01`. Scene preparation marks only the former as
`native_skybox`. The latter remains ordinary geometry so the mesh identity
alone cannot turn a floor surface into sky.

The host importer places accepted native dome sections in the `NativeSkybox/`
folder, assigns the existing Material v1 asset, disables collision and shadow
casting, and records the observed source mesh and `partial_unverified` graph
status in `ue-import.json`. The material's recovered diffuse feeds Emissive
under the previously verified Unlit host policy. The UE5 atmosphere, sky fill,
reflection capture and exposure rig remain as temporary inspection support.

The following are intentionally outside this slice: `SanctuarySky`, outer
building/antenna shells, mountain and distance terrain meshes, Kismet or
streaming activation, cloud/mask/time-of-day graph connections, native lighting,
and original-game visual comparison.

## Automated evidence

- Native mesh classification and the Unlit effective-material guard are covered
  by `tests/level_test.py` (16 tests after this slice).
- The existing material, level, resource, package and container checks remain
  required. Synthetic UE5 material verification already passed all six Unlit
  cases with zero errors/warnings.
- Fresh Sanctuary preparation completed with 4,430 placements, 423 meshes,
  372 materials, 178 bounded recovery issues, and exactly one accepted native
  Sky_Dome placement. The second observed Sky_Dome placement was rejected
  because its effective material is floor concrete.
- UE5 import completed successfully with eight existing Interchange
  performance warnings and no script errors. Saved-scene verification reopened
  4,768 section actors, including one native skybox placement, with zero
  verifier errors or warnings.
- The visual-artifact pass hides 106 observed render helpers while retaining
  recovered source collision: five `Blocking_Cube`, 94 `CollisionCube`, four
  `Mat_CloudLayer_Light` `Blocking_Plane`, two exact `WorldTransition`
  `BasePlane_256x128` helpers, and the exact start-view `InterpActor_34`
  `BoxLrg` placement. The final runtime start frame contains neither the
  yellow/black cloud planes nor the large entrance rectangle.
- Saved UV verification covered 495 sections, 512,418 corners, and 170,806
  triangles with source winding and UV0 binding agreement.
- The final runtime viewer completed `OpenWillow.Viewer` successfully. It
  captured start, moved, and overview frames, moved the pawn 5,262.03 cm, and
  reported 90 samples with 50.67 ms mean and 54.98 ms p95 frame time. The clean
  start frame is `Sanctuary_P_start00031.png`; it is free of the reported plane
  and entrance-box artifacts and shows the blue host background. The separate
  lower `IcePlate` surfaces and native sky parity remain open. A post-diagnostic
  restore import passed and left all `IcePlate` placements visible.

Evidence is retained in ignored `local/sanctuary/` outputs and the UE5 viewer
screenshots under `host/ue5/OpenWillow/Saved/Screenshots/WindowsEditor/`.

The resulting claim is limited to: the observed native dome can enter the
existing UE5 scene pipeline as a non-colliding, non-shadow-casting visual
actor with a conservative Unlit material approximation. It is not a claim of
native sky behavior or visual parity.
