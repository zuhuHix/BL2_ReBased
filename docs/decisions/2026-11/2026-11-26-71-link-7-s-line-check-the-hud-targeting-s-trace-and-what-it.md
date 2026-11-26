## 2026-11-26: Link 7's line check: the HUD targeting's trace and what it passes over

AI-assisted (Claude Code)

Lane ANALYST-LINK7-LINE-CHECK, analyst only, with Claude Opus 5.5. Docs only: no code, no dependency, and no bounds
check changed.

- **Why:** with #810, link 7's `look` stops at ruling 4's line check: the decoy pickup's skeletal-mesh bounds meet the
  line's box, and the stand-in passes over only the owner's pawn, the target and its `Base`.
- **What:**
  - the new note `docs/verification/NATIVE_LINK7_LINE_CHECK.md`;
  - one NATIVE_INDEX row and two edited rows;
  - pointers in NATIVE_PICKUP_TARGETING.md (section 5, the direction) and NATIVE_LINK7_HUD_TARGETING.md (section 8);
  - a GATES row.
- **Evidence:** a local read-only Ghidra reading under the machine lock (the line-check call in the HUD pass, the
  single-line routine and its zero-extent walker, `WillowPickup`'s filter slot, the skeletal mesh's line test outlined);
  the layout tool; `ow-package --def-dump`. No probe. Raw output stays under ignored `local/analysis/lc/` and
  `local/analysis/hud/`.
- **Findings** (UNVERIFIED in game):
  - the check is a zero-extent line from the view point to 25 units short of the pickup, with flags 134357638 and the
    HUD as source;
  - with those flags no pawn and no pickup is ever tested: the `Pawn` row needs bit 1, and `WillowPickup` and
    `DroppedPickup` keep `Actor`'s body, which needs bit 16 without `bWorldGeometry`;
  - only the level, world geometry, movers and terrain can block it; a skeletal mesh's own test is a physics-asset or
    per-polygon test, not a bounds test (outlined).
- **Needs the hub:** whether ruling 4's three-actor list becomes the game's rule (note section 5).
- **Clean room:** a different lane implements this note.
