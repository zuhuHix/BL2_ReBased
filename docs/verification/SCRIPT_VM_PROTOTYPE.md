# Phase 2 VM continuation: 2026-10-01

AI-assisted. Handoff audit found committed C++ loading/runtime/Core natives
(429a5f8), then package indexes, reference caching, dynamic-array terminator
corrections and a default-state sweep (e8ba65f). Both commits were local and
ahead of the remote. The only uncommitted tracked edit was `src/cli.cpp`'s
`--run-batch`; its referenced replay tool did not exist. That source was
backed up under ignored `local/phase2/handoff-backup/cli.cpp` before editing.
No reset, stash, checkout, extraction, dependency or license change was needed.

UModel remains the existing external asset backend. This slice uses the
project's own package reader and bytecode VM; no external tool code was copied.
Package identity, imports, execution and provenance remain project-owned.

## Completed bounded slice

Finished `--run-batch` and added `tools/replay_trace.py`. Batch cases use
tab-separated function, self class and named scalar arguments. Text is UTF-8
hex; malformed hex/numbers, duplicate/unknown/missing parameters, incompatible
self classes and unsupported argument types fail per case. Omitted trailing
optional parameters retain the interpreter's default-expression behavior.
Log clearing happens before lookup, preventing an early failure from inheriting
the previous case's native log. Unsupported result kinds are explicit.

Replay pairs nested SDK call/return records by function and receiver, discards
damaged pairings, and resolves receiver classes from reflected exports. Each
call uses a new receiver initialized from class defaults, with shared package
and runtime caches. It does not reconstruct recorded receiver properties,
object identity graphs, state transitions or side effects. Lossy object/repr
inputs, aggregates, ambiguous classes and out-result records are skipped;
native functions and calls reaching unimplemented stubs are blocked from
return-match counting. Matches and mismatches are both labelled UNVERIFIED.
String/object-repr recognition is conservative and may skip valid text.

Reports are restricted to ignored `local/`. Trace payloads and disassembly
listings are never committed. Fixed `vm_census.py`'s relative executable path
handling on Windows; no bytecode parsing behavior changed in this continuation.

## Automated verification

Release build succeeded. CTest: **8/8 passed**, 32.61 seconds. Existing synthetic
VM suite now also checks named argument order, optional defaults, control
characters in text, per-case log isolation, malformed input, receiver validation,
nested pairing, damaged pairing, matches/mismatches and stub blocking.

`python tools/verify_packages.py --reader build/Release/ow-package.exe`:

```text
Core: 234397 bytes, 1621 exports; decoded bytes, counts and export fields match
Engine: 5878264 bytes, 33166 exports; decoded bytes, counts and export fields match
GameFramework: 61714 bytes, 258 exports; decoded bytes, counts and export fields match
GearboxFramework: 1224040 bytes, 7098 exports; decoded bytes, counts and export fields match
WillowGame: 13054200 bytes, 56443 exports; decoded bytes, counts and export fields match
GFxUI: 136680 bytes, 841 exports; decoded bytes, counts and export fields match
IpDrv: 230751 bytes, 1364 exports; decoded bytes, counts and export fields match
OnlineSubsystemSteamworks: 265760 bytes, 1709 exports; decoded bytes, counts and export fields match
AkAudio: 39503 bytes, 176 exports; decoded bytes, counts and export fields match
```

Re-ran Python `--check` and C++ `--script-check` across all nine code packages:
both decode **12,968 / 12,978**, with ten failures. Count agreement is structural
evidence, not an assertion that their decoded expression trees or semantics match.

Re-ran `tools/vm_census.py`: **12,977 attempted, 12,938 completed, 39 failed,
one skipped as undecodable**, with **1,021 distinct unimplemented native/log
categories reached**. These are executions on default objects with zero inputs,
including stubs. The sweep's lazy decode/cache accounting differs from the
standalone structural check: ten structural failures are not reduced to one
by this result. Completion does not imply correct behavior or compatibility.
Local report: `local/phase2/vm-census-20261001.json`.

## Existing original-game trace: diagnostic comparison

Replayed the first 400 paired calls from the already recorded local UI trace:
**25 scalar return matches, four mismatches, 19 blocked, 352 skipped**.
Pairing across the whole trace found 19,918 pairs, rejected 91 calls and 61
returns. The four mismatches are three mission-name getters returning empty
text and one input handler returning false against recorded true. Their live
state is absent; these are leads, not proven interpreter defects. No new
original-game observation, UE run or gameplay/visual parity test was performed.

```powershell
python tools/replay_trace.py local/ui/traces/uitrace_20260930_164540.jsonl `
  --reader build/Release/ow-package.exe --limit 400 `
  --output local/phase2/trace-replay-20261001.json
```

## Next bounded work before the UE5 connection

Choose a scalar, side-effect-free script on the Sanctuary/Maya path and capture
its required receiver properties alongside inputs/outputs. Add only the state
needed to replay that function faithfully before treating matches as behavioral
evidence. Full object/state/latent semantics, the ten decode failures, native
coverage and UE5 gameplay integration remain open. Do not implement movie
bridge or timer stubs merely because the zero-input sweep ranks them highly.

## First UE5 inventory connection (2026-10-01)

AI-assisted. The first host integration executes the installed
`InventoryListPanelGFxObject.MoveDelta` for ordinary item-only backpack Up/Down.
The page sends a bounded movement request to the HUD's game-thread queue; the
HUD calls the linked C++ VM and returns the selected index. Ruffle and the host
still handle list presentation, cards, equipment navigation and equip actions.
This is execution that changes the page selection, not a shadow comparison.

`src/inventory_navigation.*` instantiates the reflected panel/provider, supplies
the current item-list length through `CachedObjects`, and binds only the native
`GetEntryKindAtIndex` interface. Its item kind is looked up as `EAK_Source` in
the getter's reflected return enum. The existing enum serialization order in
`vm.cpp` was checked against that installed export and is read with a bounded
Reader, validated prefix/count/name references and exact export consumption.
No navigation algorithm, extracted script bytes or listings are embedded.
No original package/container bounds check changed. CMake adds this source.

Each request validates direction, list size and selected index. Execution has
a 20,000-expression limit. Every VM diagnostic (including an unimplemented
native) rejects the result. The page orders repeated keys, discards replies
after selection/list changes, cancels pending work on close, and reports an
error on invalid replies or a six-second timeout; a failed VM does not silently
resume host movement. If initialization is unavailable, the existing host
adapter remains active with `vm.enabled=false` and a HUD initialization error.

The UE module now links the local CMake Release `ow-core.lib` and `ow-lzokay.lib`;
build these first. Win64 Development editor linkage is verified; other platforms
and packaged builds are UNVERIFIED. This reuses the already recorded lzokay
dependency and does not add a dependency or license decision. UModel remains
the external asset backend; this pass extracts no new assets.

Verification: Release CLI and UE5 editor builds succeed; CTest 8/8 (52.57 s),
plus the final updated VM test rerun; all nine package differential checks
match. Navigation tests 22/22, including ordered VM replies, stale selection/list
rejection, cancellation, timeout and failure handling. A synthetic provider fixture deliberately places
the source enum at a different index and uses a toy arithmetic script rather
than stock navigation; it verifies enum lookup, provider length, native dispatch,
invalid input/result rejection and malformed enum counts. Direct installed-script
calls verify forward/backward item movement, both boundaries and a one-item list,
with no VM diagnostics. Full original-game behavior parity remains UNVERIFIED.

In-engine results are recorded in the inventory verification record. The first
run stalled waiting for Zen before gameplay and was stopped; the retry used UE's
built-in `-ddc=InstalledNoZenLocalFallback` cache graph, checked in the installed
BaseEngine.ini, but stalled loading Sanctuary. The third launch added `-d3d11`
and reached the suite. Both `vm_backpack_down` and `vm_backpack_up` pass with
55 expressions each, actual expected selections and no VM errors. No project
cache/renderer configuration or authored editor state changed.

Full action suite reports **41/48 passed, seven failed, zero not run; overall
FAIL**. Wrong selected-item expectations after drag cause marking/drop failures
and a pickup cascade; missing shield data causes gear failures. Some dependent
passes are weak (empty shield ID and stale DropId). No additional VM movement
calls occur after the two passing checks. A VM-disabled baseline was not run;
these are unresolved suite issues, not proven pre-existing failures. See the
inventory record for the precise evidence limits.

Next: equipment navigation and the movie/controller object graph, then equip
script integration. Native sorting, category/empty-entry models, latent/state
semantics, visuals and the Sanctuary/Maya gameplay gate remain open.


## 2026-10-01: first placed world-object connection

A bounded Sanctuary door now loads tagged actor/action overrides and executes
installed InterpActor start/finish notifications through the same VM. Scoped
timer natives are exercised with synthetic callback/rollback checks. The host
evaluates installed Matinee keys and moves the existing component/collision.
Live E-input acceptance passes 10/10 checks over two open/close cycles, 500
expressions, zero diagnostics. CTest 8/8 and nine package comparisons pass.
This proves host plumbing, not mission activation, audio or original-game
movement parity. See [the mover record](SANCTUARY_MOVER_PROTOTYPE.md).
Current world follow-up: compare the original door and recover its stock
activation path before connecting a mission/NPC. Equipment/equip/menu parity
remains a separate open path.
