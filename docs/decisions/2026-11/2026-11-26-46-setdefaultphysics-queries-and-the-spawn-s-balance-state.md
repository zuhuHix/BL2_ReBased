## 2026-11-26: SetDefaultPhysics' queries and the spawn's balance state

AI-assisted (Claude Code)

Lane ANALYST-SETDEFAULTPHYSICS-AND-SPAWN-BALANCE, analyst only, with Claude Opus 5.5. Docs only: no code, no
dependency, and no bounds check changed.

- **Why:** #783 predicts links 3 and 4's spawn stops at `SetDefaultPhysics` past its gates (NATIVE_MIND_ROUTE.md
  section 1's three unnamed queries), and found the port's spawn never fills `BalanceDefinitionState`.
- **What:**
  - the new note `docs/verification/NATIVE_SET_DEFAULT_PHYSICS_AND_SPAWN_BALANCE.md`;
  - four NATIVE_INDEX rows and a GATES row.
- **Evidence:**
  - `research/script_disasm.py`;
  - a local read-only Ghidra reading under the machine lock (class-table slots of four classes, a static flag);
  - the layout tool;
  - a local probe: an installed Off `--combat-run` of links 3 and 4's steps after `player:8`, with a call trace and
    stubs. The temporary code was reverted and never committed.

  Raw output stays under ignored `local/analysis/sdp/`.
- **Findings** (UNVERIFIED in game):
  - the collision copy is skipped only for a cloaked pawn whose cloak's shield requirement is met, behind a
    process-wide can-cloak flag (initially true);
  - the nav handle is "flying" when its `FlightMode` is neither 0 nor 4;
  - the game fills `BalanceDefinitionState` in the population factory's script, after the class step and the attribute
    starting values;
  - on the probe's path:
    - the spawns stop at the unbound `WillowMind.RecalculateAttributeInitializedState()`. Its mind part, now read,
      evaluates `CommerceMarkup` and ten resistances on the mind;
    - five plain reads follow;
    - link 3 then stops at `GameInfo.IsFriendlyFire` (unread), and link 4 at `SetPhysics` on the player pawn.
- **Needs the hub:** the spawn's balance input (the archetype's pawn balance, a labelled harness input).
- **Clean room:** a different lane implements this note.
