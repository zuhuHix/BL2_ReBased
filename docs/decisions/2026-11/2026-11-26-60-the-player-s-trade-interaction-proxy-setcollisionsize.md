## 2026-11-26: The player's trade interaction proxy: SetCollisionSize, SetHardAttach and SetBase

AI-assisted (Claude Code)

Lane ANALYST-PAWN-INTERACTION-PROXY (queue 0aq), analyst only, with Claude Opus 5.5. Docs only: no code, no
dependency, and no bounds check changed.

- **Why:** with lane E's deferred flag (the player's `Pawn` set, from #796), `WillowPlayerPawn.SetUpTradeInteraction`
  spawns a `PawnInteractionProxy` whose `Initialize` calls three natives that refuse today, regressing link 1.
- **What:**
  - the new note `docs/verification/NATIVE_PAWN_INTERACTION_PROXY.md`;
  - one NATIVE_INDEX row and three edited rows;
  - pointers in NATIVE_ACTOR_SETBASE.md and NATIVE_GO_DOWN_PATH.md (3.2 and 4.1);
  - a GATES row.
- **Evidence:**
  - `research/script_disasm.py`;
  - a local read-only Ghidra reading under the machine lock (the three execs, the cylinder setter, four class tables);
  - the layout tool;
  - `ow-package --def-dump`;
  - a local probe: an installed Off `--combat-run` of `maya:8` on main 03a8a09 with the flag emulated and each stop
    stubbed in turn. The temporary code was reverted and never committed.

  Raw output stays under ignored `local/analysis/proxy/`.
- **Findings** (UNVERIFIED in game):
  - the proxy gets `SetCollisionSize(80, 78)` from Maya's trade definition, `SetHardAttach(true)` and `SetBase(Maya)`;
  - `SetCollisionSize` is the cylinder setter, a touch update (a collision query: `LayerAbsent` in the combat world), an
    empty `CollisionChanged` event and `bNetDirty`;
  - `SetHardAttach` is `Actor`'s body on the proxy's table, and a no-op for the proxy (already hard-attached);
  - `SetBase` on the proxy is `Actor`'s body: NATIVE_ACTOR_SETBASE.md's rule with an eighth table, and an exact
    identity case for the hard-attached relative pose.
- **Next** (from the probe): the class-level part of #796 section 3, which lane E is implementing.
- **Hub ruling (review of #802):** the touch update may be a "nothing touched" stand-in behind an Off-only slice
  harness setting, logged and UNVERIFIED; without the setting it stays `LayerAbsent`; Reference is unchanged.
- **Clean room:** a different lane implements this note.
