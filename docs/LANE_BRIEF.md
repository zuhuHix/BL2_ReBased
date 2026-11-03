# Lane brief: the rules every subagent needs, on one page

For subagents and short tasks. This page condenses the binding rules of `CLAUDE.md`, `AGENTS.md`, `docs/LEGAL.md`
and `docs/NATIVE_ANALYSIS.md`; it replaces none of them. Read it first, then open the long documents only for the part
your task touches. If this page and a long document disagree, the long document wins; say so in your report.

## Never
1. Put game-derived data in a tracked file, commit message, PR, issue or doc: no assets, package dumps, extracted
   manifests, script listings, decompiler or disassembler output, pseudo-code, recovered headers, analysis databases.
   Game-derived output goes under ignored `local/` (`local/analysis/` for native analysis). Names of game objects,
   counts, constants and formulas may be written in docs, as `docs/verification/` already does.
2. Paste, translate line by line or auto-convert decompiled code. Use leaked source. Copy code from GPL tools
   (Legendary Explorer, UE Explorer, UPKUtils).
3. Invent bytecode, struct or serialization offsets. Recover them from the packages and check them with structural
   oracles; label what is fitted rather than proven `UNVERIFIED`.
4. Loosen a bounds check to make something work. Fix the cause.
5. Add a tool, dependency or licence (`THIRD_PARTY.md`, `LICENSE`): that is the maintainer's decision.
6. Kill a process you did not start, or delete someone else's lock.
7. Hand an error back unread, or claim more than you checked.

## Clean room (native code)
- An analyst reads disassembly or decompiler output locally and writes an own-words note in
  `docs/verification/NATIVE_<area>.md`: inputs, outputs, side effects, order of effects, constants, formulas, edge
  cases. No pseudo-code and no step-by-step mirror of a listing. Before handing it over, scan it with
  `0x[0-9a-fA-F]{2,}|FUN_|DAT_|param_|this->|LAB_|undefined[0-9]|\{|\}` and fix every hit.
- An implementer works from the note only and never opens `local/analysis/`, the Ghidra projects or REA output.
- Tools: Ghidra 12.1.4 through `tools/ghidra/run.ps1` (one headless run at a time). REA 6.0.0 cannot open
  `Borderlands2.exe` (a fixed 330 s startup limit), so use Ghidra for that binary.

## Evidence
- Live game first: if a value, a native's inputs and outputs, a before/after state or an event order can be seen
  in the running game, get it there (`tools/real_game/`, pyunrealsdk; method in
  `docs/verification/REALGAME_GROUND_TRUTH.md`). Read code only for what cannot be observed.
- A rule read from native code or script, and not seen in the running game, stays `UNVERIFIED` in code and docs.
  A confirmed rule says how it was confirmed (which capture, which matching number).
- "Reproduces a live row" is not "verified in the native". Count agreement is not full compatibility.
- Fixtures in `tests/` are synthetic only: invented values, built with the existing `package()` / `tag()` helpers.

## Files that need care
- `src/package.cpp`, `src/container.*`, `CMakeLists.txt`: edit when the work needs it, then list each one in your
  final message with what changed, why, and what is verified.
- Real-game runs: only when your brief says so. Back up the saves first; afterwards the saves, config and SDK
  settings must be byte-identical (restore from the backup what the game rewrote, and say so).
- One UE editor or game process per machine: `%LOCALAPPDATA%\OpenWillow\ue_run.lock` (`tools/ue_lock.ps1`; `OPENWILLOW_UE_LOCK` overrides it). The suite scripts take it themselves; for a UE build
  use `local/p2/locked.ps1 -Owner <lane> -Command build` when that helper exists in the worktree.

## Gates
Run the commands in `docs/verification/GATES.md` for the parts you touched, paste the output with numbers, and
compare with the baseline there. Report automated checks separately from in-game checks.

## Records
- `DECISIONS.md` holds the current month; older months are in `docs/decisions/YYYY-MM.md`. Read recent entries with
  `python tools/decisions_index.py --tail N` or `--grep WORD`; do not open the file whole.
- Look up natives with `python tools/native_index_find.py <name>`; do not open `docs/verification/NATIVE_INDEX.md` whole.

## How to finish a lane
Unless your brief says otherwise, do not commit, push or edit `DECISIONS.md`, `README.md`, `ROADMAP.md`, `AGENTS.md` or
`docs/TOOLING.md`. Write `local/p2/<lane>/handoff.md` with:
- the changed tracked paths, and the gate output with numbers;
- what is verified and how, and what is `UNVERIFIED`;
- a proposed commit message, and a `DECISIONS.md` entry draft (dated, "AI-assisted (Claude)");
- the open items.

Keep diffs small and explainable: the maintainer must be able to explain the code. Prefer the simpler, obviously
correct version over the clever one.
