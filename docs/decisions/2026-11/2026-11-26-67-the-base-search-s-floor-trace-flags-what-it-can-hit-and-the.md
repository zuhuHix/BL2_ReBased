## 2026-11-26: The base search's floor trace: flags, what it can hit, and the source actor

AI-assisted (Claude Code)

Lane ANALYST-FLOOR-TRACE-FLAGS (queue 0ar), analyst only, with Claude Opus 5.5. Docs only: no code, no dependency,
and no bounds check changed.

- **Why:** the base search's floor trace had undecoded flags, and two hub stand-ins rest on that: #798's R2 (the
  actor's own collision skipped) and lane E's geometric clearance of other actors (IMPL-COMBAT-WORLD-COLLISION).
- **What:**
  - the new note `docs/verification/NATIVE_FLOOR_TRACE_FLAGS.md`;
  - one NATIVE_INDEX row and three edited rows;
  - "read since" or "corrected since" pointers in NATIVE_LINK34_AFTER_MIND.md, NATIVE_SPAWN_TRACE_FILTER.md,
    NATIVE_TOUCH_PICKUP_TRACE.md and NATIVE_ACTOR_TRACE.md;
  - a GATES row.
- **Evidence:** a local read-only Ghidra reading under the machine lock (the floor query, the single-line and
  multi-hit routines, both hash walkers, the per-actor filters and the source's blocked-by test in five class tables);
  the layout tool. No probe. Raw output stays under ignored `local/analysis/floor/`.
- **Findings** (UNVERIFIED in game):
  - the flags are 8415 (pawns, movers, the BSP, volumes, other actors, blocking only, world geometry, terrain), plus
    one result; no stop at the first hit;
  - the query is a box swept down 8 units with the actor's collision cylinder as its extent, not a line;
  - world geometry is always tested; pawns, movers, volumes and other actors are tested when the source's blocked-by
    test passes, so pawns can be the floor;
  - the walkers pass over the source's own primitives, its `Owner` chain and the actors it owns. This confirms #798's
    R2, and corrects the earlier notes' "base chain" (and `trace_filter::chain_of`) to the `Owner` chain, both ways;
  - lane E's clearance tests bounds in the right order but with too small a region (a line, not the swept box). A box
    test is proposed; it still clears the combat run's spawns.
- **Hub rulings (review of #809):** all three yes, under the existing `NoLevelBaseSearch` setting, Off only,
  UNVERIFIED, Reference unchanged:
  - the stand-in's skip widens to the `Owner` chains, both ways, as the game's walkers do;
  - `clear_of` takes the swept-box test: the actor's cylinder swept down 8 units;
  - the touch trace gets the same `Owner`-chain correction, and the implementer re-runs links 6 and 7.

  Written into the note (section 6 and test row 8).
- **Review r1 (#809):**
  - rebased onto main c1058de6, both GATES entries kept;
  - the approved harness change keeps today's `holds_no_geometry()` guard (a map-backed service stays `NotRead`);
    the BSP sweep and hit handling move to a separate, unapproved later path;
  - the swept-box test clears a candidate only with finite, non-negative extents and finite bounds; anything else is
    `NotRead`;
  - the volume row gains the `bWorldGeometry` shortcut (bit 128, not `bPawnsOnly`);
  - the predicted effect on main updated: past the floor trace, at `SetBaseValue`.
- **Clean room:** a different lane implements this note.
