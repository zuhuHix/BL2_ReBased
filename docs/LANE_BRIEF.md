# Lane brief: what a lane or sub-agent needs on top of `AGENTS.md`

For lanes, sub-agents and short tasks. `AGENTS.md` holds the rules and is normally already in your context (Claude
Code loads it through `CLAUDE.md`, Codex directly). If you cannot see its section "Rules that do not bend", read
`AGENTS.md` first: this page repeats only the two lines a brief must carry. `AGENTS.md` also says which work needs
`docs/LEGAL.md` read first; open the other long documents only for the part your task touches. If this page and a
long document disagree, the long document wins; say so in your report.

## Two rules that bind you and go into every brief you write
- **Process safety (rule 10).** Never kill a process by image name (`taskkill /IM`, `Stop-Process -Name`, `pkill`,
  `killall`), or any process you did not start. Stop only a PID your own run started, after checking its command
  line. Never delete a lock you do not hold.
- **Tools, in this order:** bl-sdk live reads and hooks of the running game (the RTSE way), then REA (the `rea` MCP
  server) for decoding, then Ghidra for `Borderlands2.exe`.

## Clean room (native code)
- An analyst reads disassembly or decompiler output locally and writes an own-words note in
  `docs/verification/NATIVE_<area>.md`: inputs, outputs, side effects, order of effects, constants, formulas, edge
  cases. No pseudo-code and no step-by-step mirror of a listing. Before handing it over, scan it with
  `0x[0-9a-fA-F]{2,}|FUN_|DAT_|param_|this->|LAB_|undefined[0-9]|\{|\}` and fix every hit.
- An implementer works from the note only and never opens `local/analysis/`, the Ghidra projects or REA output.
- Tools: Ghidra 12.1.4 through `tools/ghidra/run.ps1` (one headless run at a time). REA 6.0.0 cannot open
  `Borderlands2.exe` (a fixed 330 s startup limit), so use Ghidra for that binary.

## Evidence
- A rule read from native code or script, and not seen in the running game, stays `UNVERIFIED` in code and docs
  (rule 7). A confirmed rule says how it was confirmed: which capture, which matching number.
- "Reproduces a live row" is not "verified in the native".
- A real-game run only when your brief says so (`AGENTS.md`, "Finding out what the game does").

## Gates
Run the commands in `docs/verification/GATES.md` for the parts you touched, paste the output with numbers, and compare
with the baseline there. The general gates, the standing baselines and the newest rows are at its top; the Rust host's
gates and "Tests outside ctest" come after the rows. The archive it names is history. Report automated checks
separately from in-game checks.

## How to finish a lane
Right before you report a branch ready, rebase it on the latest `origin/main` and rerun your light checks, so it lands
without conflicts (`docs/TOOLING.md`, "Landing PRs in a batch"). Unless your brief says otherwise, do not commit, push
or edit `README.md`, `ROADMAP.md`, `AGENTS.md` or `docs/TOOLING.md`, and do not change an existing decision file. Your
decision goes in one new file under `docs/decisions/` (`python tools/decisions_index.py --name "<title>"` prints its
name), and `python tools/decisions_index.py --check --baseline origin/main` is one of your light checks. Write
`local/p2/<lane>/handoff.md` with:
- the changed tracked paths, and the gate output with numbers;
- what is verified and how, and what is `UNVERIFIED`;
- a proposed commit message, and the path of your new decision file (dated, opening "AI-assisted (<tool>)");
- the open items.
