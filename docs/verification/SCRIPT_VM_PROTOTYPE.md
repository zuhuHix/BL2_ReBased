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

## Next bounded work

Choose a scalar, side-effect-free script on the Sanctuary/Maya path and capture
its required receiver properties alongside inputs/outputs. Add only the state
needed to replay that function faithfully before treating matches as behavioral
evidence. Full object/state/latent semantics, the ten decode failures, native
coverage and UE5 gameplay integration remain open. Do not implement movie
bridge or timer stubs merely because the zero-input sweep ranks them highly.
