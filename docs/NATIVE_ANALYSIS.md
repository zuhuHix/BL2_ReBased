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

Ghidra (Apache 2.0, with a headless mode that can be scripted) is the obvious default; Binary Ninja and
IDA are alternatives. None is installed or recorded yet. Before relying on one, add its version, source,
license and use to [THIRD_PARTY.md](../THIRD_PARTY.md) (maintainer decision, as for any tool), keep the tool
outside the repository, and keep its projects under `local/analysis/`. Scripts that drive it headlessly
(import, auto-analysis, apply the names recovered so far, export notes) belong in `tools/` and contain no
game data, so a fresh machine can rebuild its database from the installed game.

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

A private GitHub repository is still a copy of game-derived material on a third party's servers. It is not
public and not distributed, but that is your risk to take knowingly; a self-hosted or encrypted store avoids it.
Keep that store separate from this repository, never add it as a submodule or a remote of this one, and never
link to it from public text.

```powershell
$env:OPENWILLOW_PRIVATE = 'D:\bl2-private'     # a folder (or clone of a private repo) outside this repository
powershell -File tools/private_sync.ps1 -Direction Push     # worktree -> store
powershell -File tools/private_sync.ps1 -Direction Pull     # store -> worktree
```

The default folders are `local/analysis` and `local/notes`. Pass `-Folders` for others. The script never deletes
anything, never overwrites a newer file with an older one, and refuses a store inside the repository.
