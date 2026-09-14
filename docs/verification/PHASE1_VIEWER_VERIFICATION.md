# Phase 1 viewer continuation (2026-09-12)

Follow-up: the user approved cooked-material serialization work on 2026-09-13.
See [Cooked-material verification](COOKED_MATERIAL_VERIFICATION.md) for the
bounded resource reader, reduced fallback counts and current verification.

## Verified

- UE5.8 OpenWillowEditor build passed with the pawn/view fix and runtime test.
- CTest: 5/5 suites passed. Nine installed code packages still match the Python reader.
- Scene tests: 7 passed, including unnamed-diffuse ambiguity/null guards and
  class-aware material lookup for duplicate object paths.
- Sanctuary preparation: ten packages, 4,430 placements, 423 reusable meshes,
  391 materials; the first import saved/reopened 4,768 section actors with zero
  verification errors or warnings.
- Standalone `OpenWillow.Viewer` passed on Ash and Sanctuary. The test checks
  possession/view target, actual pawn displacement under engine movement input,
  camera position following and camera/control rotation agreement.
- The original collision-enabled viewer failed the movement test at 0 cm.
  After disabling viewer collision, Ash moved 9,118.81 cm and Sanctuary moved
  10,024.71 cm in the frame-count-based test. These are diagnostic displacements,
  not frame-rate-independent movement benchmarks.
- Settled engine screenshots show textured geometry in both maps. Startup
  HighResShot images were rejected as acceptance evidence because some captured
  placeholder materials or incompletely streamed textures.

## Material iteration

After the refresh, saved-scene verification passed again with 4,768 section
actors. The standalone viewer also passed again (8,262.62 cm diagnostic
displacement). Reviewed `Sanctuary_P_overview00001.png`: textured signs and
architecture are visible, but large white fallback surfaces remain. This is
not visual acceptance.

A section-use audit (including actor material overrides) ranks
`Prop_SancBuildings.Material.Mat_SancBuild1e` at 56 sections lacking diffuse,
and `Common_Materials.Environment.Master_Black` at 53. The former's cooked
Expressions array has null links and surviving vector/cube parameters; the
latter's sole expression reference is null. Do not infer colors or diffuse
textures from these material names. Further cooked-serialization work needs
the explicit approval required by CLAUDE.md.

The refreshed Sanctuary manifest records 14 inferred diffuse materials and 115
remaining neutral fallbacks. Other issues are 33 unsupported component owners
and four invalid color streams. Inference is explicitly labelled in scene.json;
it does not implement masks/tint or claim full cooked-graph reconstruction.

The material-refresh tool preserves meshes/placements and updates the manifest
only after resolving every material by path and class. `--reuse-textures` is for
an unchanged installation; omit it to decode textures again.

## Local evidence

- `local/ash/viewer-9e664a95575d4086820a84034a4db7f4.log`
- `local/sanctuary/viewer-242e1fe663934961be48547243d42ca1.log`
- `local/sanctuary/viewer-71c50810f002411c8646b4965f24650f.log` (after refresh)
- `local/sanctuary/ue-verify.json` and `ue-import.json`
- `local/sanctuary-import.log` and `local/sanctuary-refresh-import.log`
- `host/ue5/OpenWillow/Saved/Screenshots/WindowsEditor/Ash_P_*`
- `host/ue5/OpenWillow/Saved/Screenshots/WindowsEditor/Sanctuary_P_*`

All game-derived images/assets stay ignored locally.

## Open gates

This is a frozen two-map inspection slice, not completed M1. Sky, terrain/BSP,
unsupported material graphs, missing component classes, color-stream variants,
walking collision, map-selector UX and broader map coverage remain open.
The overhead Ash sample is occluded by geometry; it is not evidence of a clean
whole-map overview. Sanctuary also has conspicuous fallback surfaces.

Sanctuary's automation startup observed about 8-9 FPS. The harness lowers its own
readiness threshold to run correctness checks; it does not certify performance.
Windows Computer Use failed to connect, so physical WASD/mouse gestures and
matched viewpoints in the original game remain unverified. No visual-parity or
82-map completion claim follows from these test passes.
