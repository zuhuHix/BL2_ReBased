## 2026-10-01: analysing the game executable is allowed; transcribed code and decompiler output stay out of the repository

Maintainer decision. AI-assisted drafting. Supersedes the rule, in earlier entries and in LEGAL.md, README and the
plan, that forbade disassembling or decompiling `Borderlands2.exe`.

- **Why:** the slice's hard remainder is native code (mission tracker, behavior kernel, auto-aim, constraint
  evaluators, experience and loot rules, Gearbox's natives, stock UE3 natives). The packages and the script do not
  describe it, and recovering each rule by capturing the running game is slow. The maintainer's priority is to get the
  port done as fast as possible. My rough estimate (not measured) is a week or two saved on the current slice and
  months across Phases 2 to 4; implementation, assets, the host and verification are unaffected.
- **What is allowed:** disassembling and decompiling the executable and its DLLs locally, scripting that analysis, naming
  functions and recovering structures, by anyone working on the project including AI assistants.
- **What is not allowed in the repository, unchanged in kind:** game files and game-derived data (now explicitly including
  decompiler output, recovered headers and analysis databases), leaked Gearbox/2K/Epic/UE3 source, any code transcribed,
  translated or mechanically converted from decompiler output. Project code is written from a behaviour note in our own
  words. Rules read from native code and not confirmed by running the game stay `UNVERIFIED`.
- **Maintainer's assessment and accepted risk:** the maintainer's view is that the rights holder is unlikely to object to a
  non-commercial port that does not distribute the game. That is an assessment, not a legal conclusion. Code derived from
  analysing a binary remains derived from it, and rights holders have removed projects that published such code. The policy
  keeps the published repository to original code and behaviour notes, and the maintainer accepts the remaining risk. The
  decompile-and-publish option was considered and not chosen.
- **Two machines:** the maintainer works on two PCs. Regenerable data is regenerated from the installed game; behaviour
  notes and code are committed; non-regenerable game-derived files (analysis databases, hand patches) move through a store
  outside the repository with `tools/private_sync.ps1` (`docs/NATIVE_ANALYSIS.md`). A private GitHub repository is possible
  but is still a copy of game-derived material on a third party's servers; that is the maintainer's call.
- **Changes:** `docs/LEGAL.md` (rules, new "Analysing the executable", contributor certification), new
  `docs/NATIVE_ANALYSIS.md`, README, CLAUDE.md, AGENTS.md, ROADMAP method note, CONTRIBUTING, CODE_OF_CONDUCT, PR and issue
  templates, the plan documents; `.claude/hooks/sensitive_guard.py` and `.gitignore` now also refuse decompiler/disassembler
  database file types; `tools/private_sync.ps1` added.
- **Follow-up, same day (maintainer decisions):** Ghidra 12.1.4 with a portable Temurin JDK 21 is the chosen tool and has
  a `THIRD_PARTY.md` entry (checksums verified against the published ones); `tools/ghidra_import.ps1` imports and
  analyses the executable headlessly. The private store is a separate private GitHub repository (its name is not published).
  All time-to-completion estimates were removed from ROADMAP.md, README.md and the plan documents; ROADMAP.md now has a
  "How it's going" section stating what was done in what elapsed time, with no forecast.
- **Still open:** no native function has been analysed yet, and no analysis result is confirmed against the game.

