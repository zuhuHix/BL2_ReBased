# Analysing the game executable, and working on two machines

Policy: [LEGAL.md](LEGAL.md), "Analysing the executable" (maintainer decision, 2026-10-01). This page is
the practical side. Nothing here puts game-derived data in the repository.

## Why

The slice's hard remainder is native: the mission tracker, the behavior kernel, auto-aim target choice,
constraint evaluators, experience and loot rules, Gearbox's own natives and the stock UE3 natives the
script VM needs. The packages hold the data and the UnrealScript; they do not hold these functions.
Capturing the running game to recover each rule is slow. Reading the native function is usually much
faster, and a capture then only has to confirm it.

## Workflow for one native rule

1. **Find it.** UE3 registers native functions by name, and the disassembled script already names every
   native the VM needs (`research/script_disasm.py`, `docs/verification/SCRIPT_BYTECODE_DISASM.md`).
   Start from the script call site and the class layout the packages define, then locate the native in the
   analysis tool by its registered name or by the strings and object references it touches.
2. **Understand it.** Name the function and its structures in your analysis database. Work out inputs,
   outputs, side effects, floating-point constants and the order of effects.
3. **Write it down in your own words** as a short behaviour note: what it reads, what it computes, what it
   changes, edge cases. This note is the hand-off between analysis and implementation, and it is the only
   artefact that may be committed (see below).
4. **Implement from the note** in the owned executors (`src/`) or the host. Match its structure to the project,
   not to the listing.
5. **Test it.** Synthetic tests with invented values, plus a differential check against the running game where
   one is possible (a capture through `tools/sdk_trace`, a number visible in the game's UI). Until a check
   confirms it, label the rule `UNVERIFIED`.
6. **Record it.** A `DECISIONS.md` entry names the function and what was verified, in the usual evidence-first
   tone.

## What may be committed

| Artefact | Where |
|---|---|
| Raw decompiler or disassembler output, pseudo-code, recovered headers | never; ignored `local/analysis/` or the private store |
| Analysis databases (Ghidra projects, `.gzf`, IDA/Binary Ninja files) | never; same |
| Behaviour notes in your own words (formulas, constants, order of effects, names of game objects as already used in `docs/verification/`) | yes, in `docs/verification/` |
| Original code written from a note | yes |
| Tests with invented values | yes |

The repository hook refuses writes of the database and asset file types it knows about
(`.claude/hooks/sensitive_guard.py`). It cannot recognise pasted pseudo-code, so review diffs for it.

## Tooling

Ghidra 12.1.4 (Apache 2.0) with Temurin JDK 21 is the chosen tool (maintainer decision 2026-10-01; versions,
sources, checksums and license in [THIRD_PARTY.md](../THIRD_PARTY.md)). Both live outside the repository, for
example `C:\Users\<you>\Tools\ghidra_12.1.4_PUBLIC` and `...\jdk-21.0.12.1+1` (set `OPENWILLOW_GHIDRA` and
`OPENWILLOW_JDK` if yours differ; the JDK is only put on `PATH` by the script, not installed system-wide).

`tools/ghidra_import.ps1` copies `Borderlands2.exe` to a work folder, imports it into a Ghidra project and runs
auto-analysis headlessly. Two practical points it handles: `analyzeHeadless.bat` breaks on the usual Steam path
(`Program Files (x86)`), hence the copy; and Ghidra rejects project paths containing a folder that starts with
`.`, which includes worktrees under `.t3`, so the project lives in `%OPENWILLOW_ANALYSIS%` or
`%USERPROFILE%\bl2-analysis`, outside every worktree. Further scripts that drive Ghidra headlessly belong in
`tools/ghidra/` (next section) and contain no game data.

## Native registration and queries

`tools/ghidra/` drives the imported project headlessly (Java GhidraScripts; this Ghidra has no Python feature). The
scripts contain no game data; everything they write is game-derived and goes to `%OPENWILLOW_ANALYSIS%\out`
(default `%USERPROFILE%\bl2-analysis\out`) or, with `-Out`, under the ignored `local/`. Only one `analyzeHeadless`
may use the project at a time. Without `-ReadOnly` the project is saved after each run, which is how names persist.

**How registration was found.** In this build every native is registered by name: each native C++ class has a static
table of pairs, an ANSI string `<CppClass>exec<Function>` (for example `UBehaviorKernelexec...`, `AMissionTracker...`)
and the function's address, ended by a null pair. No static `GNatives` array exists; natives with a fixed script
index (`native_<n>` in the disassembler) are bound by name too, so `native_<n>` maps to an address by joining the
`iNative` stored in each package UFunction's tail with the name tables. Found by searching the executable's strings
for known native names and following the data references to them. Virtual natives (for example every
`Behavior_*.ApplyBehaviorToContext`) share one folded exec thunk that calls through the object's vtable; the class's
vtable is found from its registration (the class name as a UTF-16 string next to an in-place constructor that stores
the vtable).

```powershell
# once (and after re-importing): name -> address tables, iNative join, label every native in the database
powershell -File tools/ghidra/run.ps1 -Tables -Apply
# decompile natives/addresses into <out>\decomp\*.c, list them in <out>\query.tsv; -Label names resolved natives
powershell -File tools/ghidra/run.ps1 -Query MissionTracker.UpdateObjective,native:114 -Callees 1 -Label
powershell -File tools/ghidra/run.ps1 -Query '@C:\path\queries.txt'      # one query per line, '#' comments
python tools/ghidra/class_layout.py WillowGame.MissionTracker           # name the offsets a native reads
```

`-Tables` runs `script_natives.py` (package UFunction `iNative` list) and `OwNativeTables.java`, writing
`natives.tsv` (raw name, C++ class, function, address, table) and `gnatives.tsv` (iNative, script function,
address). On this build: 6,877 natives in 770 tables, every table naming a single class, all 199 script `iNative`
values bound by name. Query forms (`OwNativeQuery.java` header has the full list): `Class.Function`,
`execFunction`, the raw registered name, `native:<n>`, `0x<address>`, `str:<text>` (functions using a string),
`refs:<address>` (functions referencing a global), `insn:<regex>` / `insn@<lo>-<hi>:<regex>` (functions containing a
matching instruction), `callers:<query>`, `virtual:<Class>:<hexOffset>[:<Name>]` (a slot of a C++ class's vtable;
refuses script-only classes) and `name:<address>:<Ns::Name>` (persist your own name; empty resets it). Pass query
files rather than inline arguments when a query contains `|`, quotes or `=`: `analyzeHeadless.bat` splits arguments
at `=`, which is why script options use `key:value`.

`class_layout.py` computes 32-bit field offsets from the packages: fields follow the reverse-declaration `Next`
chain of the cooked packages, bools share 32-bit words, structs align to their largest member. Its oracle is
`Core.Object` = 0x3C bytes with `Outer` at 0x28; the `SequenceOp` link arrays and `MissionTracker`/`IMission`
layouts it computes match every offset seen in the native code read so far. It is still a computed layout:
treat an offset that does not fit the code as a reason to re-check the rule, not the code.

Timing on the analysis machine: about 15 s per run (Ghidra start-up and project open dominate); `-Tables -Apply`
15 s; a 50-function query 20 to 60 s with decompilation; a whole-program `insn:` scan 30 to 100 s. First results
are recorded in [NATIVE_MISSION_DISPATCH.md](verification/NATIVE_MISSION_DISPATCH.md); later note sets (all
`UNVERIFIED` in game) are the other `verification/NATIVE_*.md` files, including Phaselock presentation, weapon visuals,
ambient NPC movement and the backpack sort (2026-10-04).

## Working on two machines

Everything the tools generate from the installed game is regenerable and stays machine-local. The files
that are worth sharing are the ones that cost human time and cannot be regenerated: your analysis
database and any hand-made patches to imported content.

1. Regenerate what is regenerable. The seed and prepare scripts rebuild `local/` and the UE `Content/`
   folder from the installed game (`tools/worktree-assets.md`, `docs/TOOLING.md`). Install the game and
   UModel on each machine; set `OPENWILLOW_BL2` and `OPENWILLOW_UMODEL`.
2. Commit your own-words behaviour notes and code. They are the part of the analysis that other machines
   need, and they are not game data.
3. Share the non-regenerable game-derived files through a store outside this repository, with
   `tools/private_sync.ps1`. It mirrors named folders between this worktree and a folder you point
   `OPENWILLOW_PRIVATE` at. Use a private repository, a cloud-synced folder or an external drive, as you prefer.
   Large imported content is better regenerated than synced; GitHub rejects files over 100 MB.

The maintainer's store is the private GitHub repository `zuhuHix/BL2_ReBased-private`, cloned to
`C:\Users\<you>\bl2-private` with `OPENWILLOW_PRIVATE` pointing at it. It is still a copy of game-derived material
on a third party's servers: not public and not distributed, but a risk taken knowingly; a self-hosted or encrypted
store avoids it. Keep that repository separate from this one, never add it as a submodule or a remote of this one,
never make it public, and never link to it from public text. Its README states these rules.

A Ghidra project is a database of many files that changes constantly and can exceed GitHub's 100 MB file limit, so
do not copy the live project folder in. Export the program (`File > Export Program > Ghidra Zip File`, `.gzf`, or
a scripted export) into `local/analysis/` and sync that; the other machine imports it instead of re-analysing.
Rebuilding the analysis from the executable with `tools/ghidra_import.ps1` is the fallback and needs no sync.

```powershell
$env:OPENWILLOW_PRIVATE = 'D:\bl2-private'     # a folder (or clone of a private repo) outside this repository
powershell -File tools/private_sync.ps1 -Direction Push     # worktree -> store
powershell -File tools/private_sync.ps1 -Direction Pull     # store -> worktree
```

The default folders are `local/analysis` and `local/notes`. Pass `-Folders` for others. The script never deletes
anything, never overwrites a newer file with an older one, and refuses a store inside the repository.
