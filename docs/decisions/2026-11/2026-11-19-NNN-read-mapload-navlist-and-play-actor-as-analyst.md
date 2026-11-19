## 2026-11-19: The map load's navigation list and play-actor spawn, read as analyst

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-MAPLOAD-NAVLIST-PLAYACTOR (lane 1 of SLICE_MAPLOAD.md section 4),
analyst only (clean room: this thread does not implement it). The decision number is pending; NNN is a placeholder.

- **What:** NATIVE_MAP_LOAD.md sections 5 and 6, corrections to section 4 and the open list, and two NATIVE_INDEX
  rows.
- **How:**
  - the installed script, with `research/script_disasm.py`;
  - the level's untagged tail and the points' tags, through `ow-package` and the definition store (local probes, not
    committed);
  - read-only Ghidra queries under the machine lock, for the join and the map load's player loop.

  Raw output stays under ignored `local/analysis/navlist/`. No bl-sdk evidence is on record for it, and no
  real-game session was run.
- **Found, Q1:**
  - each level serializes its own navigation, cover and pylon start and end in its tail;
  - the chain inside a level is the points' saved `nextNavigationPoint`;
  - each level's chain is joined onto the front of `WorldInfo.NavigationPointList` when the level is added: the
    persistent level at the bring-up's begin-play step, a streamed level in the streaming add;
  - oracles hold on `Sanctuary_P` (one player start) and `Sanctuary_Dynamic` (7 pylons);
  - the order of the persistent join against the streamed joins at load is open.
- **Found, Q3:**
  - `GamePlayers` gets its one local player at engine start-up (`GameViewportClient.Init` → `CreateInitialPlayer` →
    `CreatePlayer` without a play actor);
  - the map load walks `GamePlayers` after the bring-up and calls the local-player spawn for each, which corrects
    section 4's earlier "does not call it directly".
- The auto-state flag bit is not taken here (0cde's lane).
- Every rule is UNVERIFIED in the running game; the note lists five open questions and two bl-sdk checks.
