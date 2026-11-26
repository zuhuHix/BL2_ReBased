## 2026-11-26: The combat music manager, the spawn's balance tail, and where links 4, 13 and 14 end

AI-assisted (Claude Code)

Lane ANALYST-COMBAT-MUSIC-AND-AFTER, analyst only, with Claude Opus 5.5. Docs only: no code, no dependency, and no
bounds check changed.

- **Why:** #799 puts the swings' next stop at `GetCombatMusicManager`'s lazy creation; main's new spawn rules meet new
  stops first.
- **What:**
  - the new note `docs/verification/NATIVE_COMBAT_MUSIC_AND_AFTER.md`;
  - two NATIVE_INDEX rows and one edited row;
  - a pointer in NATIVE_SET_DEFAULT_PHYSICS_AND_SPAWN_BALANCE.md section 3;
  - a GATES row.
- **Evidence:**
  - `research/script_disasm.py`;
  - a local read-only Ghidra reading under the machine lock;
  - the layout tool;
  - `ow-package --def-dump`;
  - a local probe: an installed Off `--combat-run` on main 03a8a09 with a call trace, each stop stubbed in turn. The
    temporary code was reverted and never committed.

  Raw output stays under ignored `local/analysis/cmusic/`.
- **Findings** (UNVERIFIED in game):
  - `ApplyGradeCustomizations` uses the balance's own playthrough index, not its argument, and runs that entry's
    behaviours. It is true on the slice, with nothing run;
  - `UpdatePlayThroughMaterial` does nothing on the slice (no playthrough-1 material);
  - `GetCombatMusicManager` makes the manager and sets its parameters from the globals definition;
  - `PlayerDamagedByEnemy`'s gates are read and its threat model outlined;
  - with every earlier answer in, link 4's swing completes; link 13's swings complete without a down; link 14 has
    nothing to bleed.
- **Needs the hub:** whether the music manager's threat model may be a logged no-op stand-in.
- **Clean room:** a different lane implements this note.
