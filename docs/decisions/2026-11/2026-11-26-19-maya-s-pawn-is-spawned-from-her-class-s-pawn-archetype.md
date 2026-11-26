## 2026-11-26: Maya's pawn is spawned from her class's pawn archetype

AI-assisted (Claude Code)

Lane ANALYST-MAYA-PAWN-SPAWN-SOURCE, analyst only, with Claude Opus 5.5. Docs only: no code, no dependency, and no
bounds check changed.

- **Why:** the mission bridge makes Maya's player pawn from `WillowPlayerPawn`'s class defaults. The game spawns it
  from her class's pawn archetype.
- **What:**
  - the new note `docs/verification/NATIVE_MAYA_PAWN_SPAWN_SOURCE.md`;
  - a NATIVE_INDEX row and a GATES row.
- **Evidence:**
  - `research/script_disasm.py`, the port's code on main 457c46e4, and `ow-package --def-dump` of the archetype and the
    class default;
  - a local probe of an installed `--combat-run maya:8` (three temporary log lines, reverted, not committed). It showed
    the class-default pawn is the one maya:8 works on, no template spawn runs, and today's link 1 stop meets an
    unloaded `Default__WillowPlayerPawn` subobject.

  Raw output stays under ignored `local/analysis/pawnsrc/`.
- **Findings** (UNVERIFIED in game):
  - `SwitchPlayerClass` → `RestartPlayer` → `WillowGameInfo.SpawnDefaultPawnFor` spawns the archetype's class with the
    archetype as `ActorTemplate`; `SetPlayerDefaults` then applies the class defaults through
    `ApplyCharacterClassDefaults`;
  - in the port `RestartPlayer` makes no pawn (no start, by reading). The harness's health stand-in calls
    `ApplyCharacterClassDefaults` on the bridge's class-default pawn;
  - 19 properties differ. `Components` decodes the same on both (inherited), and the archetype's instancing remaps it.
- **Proposed:** the bridge spawns from the archetype when the controller has a `PlayerClass` (route B), with a
  write-free and load-free preflight; route A, the game's own `RestartPlayer`, is the later step.
- **Needs a hub ruling:** the old `Mesh` and `CylinderComponent` release in the body composition. The archetype pawn
  has both set but not attached. Without a ruling link 1's stop moves earlier, to "releasing the old Mesh"; with it, the
  stop is #738's physics `LayerAbsent`.
- **Clean room:** a different lane implements this note.
