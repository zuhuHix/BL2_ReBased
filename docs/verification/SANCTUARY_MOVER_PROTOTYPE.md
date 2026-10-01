# Sanctuary Matinee door prototype

2026-10-01. AI-assisted implementation. This is one bounded world-object
connection, not mission/Kismet or original-game behavior parity.

The local candidate is Sanctuary's target-practice door: the installed
`RocksPaperGenocide.SeqAct_Interp_2` binds group `door` through `SeqVar_Object`
to `InterpActor_13`. Preparation follows the action's data/group/track chain,
checks actor/component identity against the existing scene, and resolves the
mesh within that scene's loaded package scope. Shared mesh copies in other
cooked maps are not treated as the canonical scene identity.

## Ownership and scope

No fresh export or import is needed. The existing prepared static mesh,
materials and convex collision are reused; UModel build 1590 remains the
recorded external backend for missing supported asset payloads. No new tool,
dependency or license decision. Zero new mesh/texture payloads, zero new asset
imports; no exporter success or visual-parity claim is made in this pass.

`tools/prepare_mover.py` uses the owned package reader and its existing tagged
array support. It writes one ignored `ow-mover-v1` manifest, with two position
keys, two Euler keys and 1.5-second duration. It lists both omitted Ak-event
tracks explicitly. It requires one component/section with existing convex
collision and rejects ambiguous bindings, dynamic lookup targets, unsupported
frames/modes, invalid key times/numbers and output outside repository `local/`.
The `Points` schema enables tagged decoding; the tool separately validates the
actual movement-point and lookup-point field sets. No serialization offsets
are guessed or scanned. Native action/component tails remain opaque.

The UE component binds the existing source-tagged mesh, checks its initial
placement, promotes that component to movable only at runtime, and restores
its original transform/mobility on failure or shutdown. It evaluates the
installed keys as linear, constant or cubic Hermite segments. Relative-frame
composition uses first-key deltas and the placed actor transform. **That frame
mapping, Euler composition and auto-curve tangent interpretation remain
UNVERIFIED against the original game.** Collision stays on the moving mesh.
This does not implement encroachment/pushing or prove a walk-through route.

The owned VM materialises explicit actor/action exports from class defaults
plus their tagged overrides. Callers supply established prefixes (26 actor,
4 ordinary object, 8 component); it rejects truncated exports and unsupported
prefix requests. Resource references and native tails are not a complete live
object graph, and archetype inheritance is not implemented by this helper.
The selected installed `InterpActor.InterpolationStarted` and
`InterpolationFinished` scripts execute rather than being transcribed.
`GroupInst` is null for the unused start parameter in this bounded call.

The bridge supplies scoped `Actor.ClearTimer` / `SetTimer` with bounded
callbacks, plus stopping an idle ambient component. Actual audio playback is
unsupported. Object-loading or execution diagnostics fail the bridge;
failed notifications/callback batches roll script properties, timers and
clock back. The selected door emits no such diagnostics. Its completion uses
zero-duration timers; positive callback scheduling is covered synthetically.
Timers are a host adapter, not VM latent/state execution or full UE3 parity.

Activation is deliberately a developer interaction: with `-owmover=<manifest>`,
Maya's existing **E** action toggles this door within 220 cm of its bounds.
Repeated input during motion is consumed. No manifest means no door binding.
Mission gating, generated sequence events, mission progress and both audio
tracks are omitted; none is counted as implemented by opening the door.

## Reproduce

Build Release libraries before the UE module:

```powershell
cmake --build build --config Release
& 'C:\Program Files\Epic Games\UE_5.8\Engine\Build\BatchFiles\Build.bat' OpenWillowEditor Win64 Development "-Project=$pwd\host\ue5\OpenWillow\OpenWillow.uproject" -WaitMutex
python tools/prepare_mover.py --game "$env:OPENWILLOW_BL2" --package Sanctuary_Dynamic --action TheWorld.PersistentLevel.Main_Sequence.RocksPaperGenocide.SeqAct_Interp_2 --group door
powershell -NoProfile -ExecutionPolicy Bypass -File tools/test_mover.ps1
```

The runner refuses an existing editor, owns `local/ue_run.lock` and its editor
process, uses the previously verified launch-only cache fallback/D3D11, and
cleans up even on failure. It does not change project cache/renderer settings,
reimport assets or save a map. The test teleports/fixes Maya near the door,
feeds E through player-controller input, checks two open/close cycles, queries
simple component collision at closed and moved positions and saves three UE
screenshots. It does not prove physical keyboard input or original-game parity.

## Evidence

Release and Win64 Development editor builds pass. Synthetic lifecycle tests
use original toy scripts: start sets a flag; completion schedules a callback
using a float override from the placed export; callback clears the flag.
They verify imported placed state, positive timers, both directions, failure
rollback after an unimplemented native, wrong class rejection and truncated
actor-prefix rejection. No game bytecode is stored in fixtures.

Direct installed-script probe: start/finish forward = 57/67 expressions;
reverse = 58/68; checkpoint flag true; zero loading/runtime diagnostics.
No positive timer callback occurs on this installed actor's defaults.

First UE run (`local/doors/run-20261001-082641.log`) reached initialization and
failed explicitly because the existing map component was static. Runtime
promotion was added to the exact bound component, with restoration; no map
was saved. Final UE run (`local/doors/run-20261001-083143.log`) passes **10/10 checks**,
zero errors, two open/close cycles and 500 expressions. Actual player-controller
E input starts both directions. The closed ray hits simple component collision;
after opening it misses the old position and hits the moved component. Both
closures restore the initial transform and blocking collision. Start/finish
expression counts match the direct probe. No mover diagnostics were emitted.
The runner stopped its own editor and removed the lock.

UE captures at `host/ue5/OpenWillow/Saved/Screenshots/WindowsEditor/`:
`OWMover_closed.png`, `OWMover_midpoint.png`, `OWMover_open.png`. Closed and open
captures were visually inspected: the door fills the frame when closed and
swings clear to expose the room when open. This establishes host presentation
only; no paired original-game capture was taken. Texture/lighting fidelity,
original activation, motion and collision-route parity remain UNVERIFIED.

Final automated checks: CTest **8/8**, 51.58 seconds, including the expanded VM
fixture; all **nine** package differential comparisons match decoded bytes,
counts and export fields. Release and UE5 Win64 Development builds pass.
Inventory overlay/HUD reporting were not changed; the inventory action suite
was not rerun and its previously recorded 41/48 overall FAIL remains open.
Packaged builds/other platforms remain UNVERIFIED.
