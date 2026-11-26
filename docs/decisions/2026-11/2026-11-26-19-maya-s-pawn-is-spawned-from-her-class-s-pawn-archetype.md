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
    `ApplyCharacterClassDefaults` on the bridge's class-default pawn (on e947919, `player_pawn_with_components`);
  - 19 properties differ. `Components` decodes the same on both (inherited), and the archetype's instancing remaps it.
- **Proposed:** the bridge spawns from the archetype when the controller has a `PlayerClass` (route B), with a
  write-free and load-free preflight; route A, the game's own `RestartPlayer`, is the later step.
- **The release** of the old `Mesh` and `CylinderComponent` in the body composition: the archetype pawn has both set
  but not attached. The hub ruled (on #762) that the attach record decides, as the body-composition note already
  says. Until the code follows, link 1's stop moves earlier, to "releasing the old Mesh"; after, it is #738's physics
  `LayerAbsent`.
- **Review r1 (#762):**
  - "today" re-measured on main e947919: links 1, 3, 4, 5, 13 and 14 stop at `GetBoundingCylinder`'s colliding-bounds
    `LayerAbsent` (6 ok / 8 NotYet); the 457c46e4 probe is marked as the older base; #760 moves that stop to 2.A;
  - route B replaces `player_pawn_with_components` with `actors::spawn` and the archetype as template; the spawn
    publishes the attach;
  - the release step brings the code in line with NATIVE_INITIALIZE_BODY_COMPOSITION.md's own rule; the hub ruled yes;
  - `RestartPlayer`'s and `SwitchPlayerClass`' missing steps added to route A's stops; route B skips
    `ApplyGlobalPlayerMovementSettings`; which steps are script; the cylinder conflict flagged.
- **Clean room:** a different lane implements this note.
