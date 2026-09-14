# Sanctuary performance sample and in-game map selector — 2026-09-14

Checkout `t3code-b88eb790`, UE 5.8, installed Steam game. Game-derived files
stay in ignored local directories. This is a diagnostic sample on one
integrated-GPU laptop, not a benchmark, and nothing in it changes renderer
settings or importer behavior.

## Hardware and inputs

- CPU: 13th Gen Intel Core i5-1345U (10 cores / 12 threads); 32 GB RAM.
- GPU: Intel Iris Xe Graphics (integrated, shared memory), driver 32.0.101.7085.
- Windows 11 Pro; UE 5.8 editor binary running as `-game`, windowed and hidden
  by the test harness (`Start-Process -WindowStyle Hidden`), so presentation
  and vsync behaviour may differ from a visible window.
- Sanctuary scene: the saved `Sanctuary_P.umap` imported in checkout
  `t3code-9935cb53` on 2026-09-13 (collision refresh; 4,768 section actors).
  It predates the 2026-09-14 winding correction and the two newer diffuse
  inference rules, so it is the scene the earlier ~8-9 FPS observation was
  made on, not the current importer output.

## Method

`OpenWillow.Profile` (`host/ue5/OpenWillow/Source/OpenWillow/ProfileTest.cpp`)
waits 45 s after the pawn is possessed, then records 300 frames from the saved
start pose and 300 more after turning the view 180 degrees. Per frame it reads
the same engine counters `stat unit` displays (`GGameThreadTime`,
`GRenderThreadTime`, `GRHIThreadTime`, `RHIGetGPUFrameCycles`) plus process
memory, and finishes with a `ProfileGPU` dump to the log. Values below are
mean / p95 milliseconds. `draw_calls` and `primitives` are read from the RHI
counters mid-frame and varied 3-4x between otherwise identical runs, so they
are not reported; the `ProfileGPU` frame gives a consistent count instead.

```powershell
./tools/test_ue_viewer.ps1 -Engine $engine -Game $game -Scene local/sanctuary -Profile
./tools/test_ue_viewer.ps1 -Engine $engine -Game $game -Scene local/sanctuary -Profile -LowEnd
```

`-LowEnd` on the test harness applies the same runtime-only DX11/SM5 and
scalability arguments as `run_ue_level.ps1 -LowEnd`, including `t.MaxFPS 60`.

## Default path (D3D12, SM5, 1280x720, TSR at `sg.AntiAliasingQuality=3`)

Two runs, both 300 samples per view (logs `viewer-f76e88d0…` and
`viewer-1c5f1c74…` under `local/sanctuary/`):

| View | frame ms | fps | game ms | render ms | rhi ms | gpu ms | memory |
|---|---|---|---|---|---|---|---|
| start, run 1 | 72.9 / 87.5 | 13.7 | 7.2 / 8.2 | 72.7 / 87.3 | 7.1 / 7.8 | 69.7 / 83.5 | 3,530 MB |
| turned, run 1 | 67.7 / 82.0 | 14.8 | 7.1 / 8.3 | 67.6 / 81.9 | 7.2 / 8.1 | 64.1 / 78.0 | 3,547 MB |
| start, run 2 | 84.7 / 107.0 | 11.8 | 7.0 / 9.7 | 84.7 / 106.8 | 6.6 / 8.4 | 80.6 / 101.2 | 3,521 MB |
| turned, run 2 | 74.3 / 98.3 | 13.5 | 6.6 / 8.3 | 74.3 / 98.0 | 6.6 / 7.9 | 70.9 / 94.9 | 3,520 MB |

The game thread is idle-bound at ~7 ms; the render thread time equals the
frame time because it is waiting on the GPU. The frame is GPU-bound.

`ProfileGPU` for one turned-view frame (run 2; 453 draws, 453,784 primitives,
60.1 ms on the GPU timeline):

| Pass | ms | share |
|---|---|---|
| PostProcessing → TemporalSuperResolution 1280x720, history 2560x1440 | 43.2 | 72% |
|   of which TSR RejectShading | 31.0 | 52% |
|   of which TSR UpdateHistory (Quality=Epic, 2560x1440) | 7.0 | 12% |
| RenderDeferredLighting (sun + translucency lighting volume) | 5.0 | 8% |
| ShadowDepths (8192x2048 atlas, `OpenWillow_Sun`) | 3.0 | 5% |
| LightCompositionTasks_PreLighting (SSAO) | 1.8 | 3% |
| ReflectionEnvironmentAndSky | 1.5 | 2% |
| BasePass | 1.5 | 2% |

No Nanite or virtual-shadow-map passes appear in the frame; the imported
static meshes are not Nanite-enabled and the sun uses a conventional shadow
atlas. Geometry itself (prepass + base pass ≈ 2 ms) is a minor cost on this
frame; the dominant stage is UE5's default TSR anti-aliasing, whose Epic
quality tier is sized for discrete GPUs.

## Low-end path (`-LowEnd`: D3D11, SM5, 960x540 at 66%, FXAA, lowest scalability, `t.MaxFPS 60`)

One run, 300 samples per view (log `viewer-c656e4c5…`):

| View | frame ms | fps | game ms | render ms | rhi ms | gpu ms | memory |
|---|---|---|---|---|---|---|---|
| start | 16.9 / 16.7 | 59.1 | 4.5 / 5.3 | 8.7 / 11.2 | 9.0 / 12.1 | 16.9 / 18.4 | 3,695 MB |
| turned | 16.7 / 16.7 | 60.0 | 4.6 / 5.6 | 7.7 / 9.8 | 7.8 / 9.6 | 16.6 / 22.5 | 3,697 MB |

The frame sits on the 60 FPS cap, so the true headroom is not measured; the
`ProfileGPU` frame totals 5.3 ms of scene work (base pass 1.3 ms, reflection
0.7 ms, post-processing 1.2 ms) plus 4.9 ms of buffer uploads and 3.1 ms in
`EndDrawingViewport`, which on D3D11 includes the present wait.

## Reading

- The ~8-9 FPS startup figure in ROADMAP.md is consistent with this scene:
  after settling, the default path runs at 12-15 FPS on this laptop and the
  GPU is the bottleneck.
- About three quarters of the default-path GPU frame is TSR, not the imported
  content. Shadows, lighting and geometry together are under 15 ms.
- The low-end switch reaches the 60 FPS cap on the same scene by dropping to
  FXAA, lower resolution and lowest scalability; it does not isolate which of
  those changes matters most.

## Candidate fix, not applied

Switching the default path's anti-aliasing from TSR to FXAA or TAA
(`r.AntiAliasingMethod 1` or `2`), or lowering `sg.AntiAliasingQuality`,
would remove most of the measured GPU time on integrated graphics. That is a
project rendering-settings decision (`DefaultEngine.ini` or the launch
script), so it is recorded here for review rather than changed in this pass.
A discrete-GPU measurement would show whether TSR is affordable there.

## In-game map selector

`AOpenWillowPlayerController` (`OpenWillowMapSelector.cpp`) is now the
viewer's player controller. Tab toggles an on-screen list built from
`local/*/scene.json` manifests and `Content/OpenWillow/<Map>/<Map>.umap`
files; digit keys 1-9 open an imported entry with `UGameplayStatics::OpenLevel`
using the same `/Game/OpenWillow/<Map>/<Map>` path `run_ue_level.ps1` uses.
Entries without a saved map are listed as "not imported" and refuse to open.
Prepare and import remain command-line only. Console commands `OWMapList` and
`OWMapOpen <n>` expose the same list.

`OpenWillow.MapSelector` (`MapSelectorTest.cpp`, run with
`test_ue_viewer.ps1 -Selector`) launched on Ash, listed Ash, Sanctuary and
Southpaw Factory as prepared and imported, rejected an out-of-range entry,
opened Sanctuary through `OpenEntry` and observed the running world change to
`Sanctuary_P` 5.9 s later; the new world's controller is again the selector
class (log `local/ash/viewer-f9cc7de9…`). The test drives `OpenEntry`
directly: the Tab/digit key bindings themselves were not exercised by a
physical key press in this run and remain a user check.

## Automated baseline

- Release reader build: passed; UE5.8 `OpenWillowEditor` build: passed.
- CTest: `100% tests passed out of 5`.
- `verify_packages.py`: all nine code packages report "decoded bytes, counts
  and export fields match".
- `OpenWillow.MapSelector`: `Test Completed. Result={Success}`.
- `OpenWillow.Profile`: `Result={Success}` for default and `-LowEnd` runs.

Sky, terrain/BSP, remaining fallback materials, matched-viewpoint comparison
and broader map coverage are unchanged by this record.
