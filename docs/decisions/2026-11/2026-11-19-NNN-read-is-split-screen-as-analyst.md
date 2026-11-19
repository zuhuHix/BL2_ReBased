## 2026-11-19: Engine.IsSplitScreen read as analyst

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-LINK1-IS-SPLIT-SCREEN, analyst only (clean room: this thread does not
implement it). The decision number is pending; NNN is a placeholder.

- **What:** `docs/verification/NATIVE_IS_SPLIT_SCREEN.md` and one NATIVE_INDEX row.
- **How:**
  - the installed script, with `research/script_disasm.py`;
  - one read-only Ghidra query under the machine lock;
  - the field named with the project's layout tool.

  Raw output stays under ignored `local/analysis/splitscreen/`. No bl-sdk evidence is on record for it, and no
  real-game session was run.
- **Found:**
  - `Engine.IsSplitScreen()` is a static, final native;
  - it answers whether the engine's `GamePlayers` list holds more than one player, and writes nothing;
  - link 1 reaches it from `WillowGameInfo.ShouldSpawnAtStartSpot`.
- **Off answer** (the hub's pre-ruling): false. The port has one local player. It is a port answer, UNVERIFIED in game.
- **Probe** (local, not committed): with it bound false, links 1, 3, 4, 5, 13 and 14 stop in `maya:8` at the unbound
  iterator `WorldInfo.AllNavigationPoints(class,object)`, from `GameInfo.FindPlayerStart`.
- Every rule is UNVERIFIED in the running game; the note lists two open questions and one bl-sdk check.
