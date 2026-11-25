## 2026-11-25: The pickup classes keep the Actor trace filter

AI-assisted (Claude Code)

Lane ANALYST-PICKUP-TOUCH-TRACE-FILTER, analyst only, with Claude Opus 5.5. Docs only: no code, no dependency, and no
bounds check changed.

- **Why:** link 7's `touch` stops at "the trace filter of WillowGame.WillowPickup", the trace filter's refusal for an
  owner class with no read row.
- **What:** section 3.1 of `docs/verification/NATIVE_TOUCH_PICKUP_TRACE.md`, one NATIVE_INDEX row and one GATES row,
  from a local read-only Ghidra reading under the machine lock. Raw output stays under ignored
  `local/analysis/reattach/`.
- **Findings** (UNVERIFIED in game):
  - the per-actor filter's slot is identified by its distinct `Actor`, `Pawn`, `Volume` and `InterpActor` bodies;
  - `DroppedPickup` and `WillowPickup` keep `Actor`'s body;
  - with the touch's flags (no bit 16) and the pickup's `bWorldGeometry` clear, a pickup's own primitives are not
    tested by its touch line;
  - `KActor` has its own body there, with the mover row's tests.
- **Proposed Off answer:** two exact-class `Row::Actor` entries in `trace_filter::ROWS`. Tests listed.
- **Clean room:** a different lane implements this note.
