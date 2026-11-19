## 2026-11-19: Read the spawn's bounding-cylinder rule as analyst

AI-assisted (Claude Code, Opus 5.5). Lane LINK-9-NEXT, which turned into an analyst lane (clean room). New note
`docs/verification/NATIVE_SPAWN_EXTENT.md`, with NATIVE_INDEX rows. No code changes. The hub's lane id was 2026-11-19-101; decision file numbers have two digits, so the hub assigned 2026-11-19-09.

- **The question.** On #477 (ab5288d), slice link 9 (`phaselock_enemy`) stops at `spawn` on "the actor's
  bounding-cylinder rule". The cast spawns `WillowPlayerPawn` from its class default, whose `CollisionComponent` is
  None.
- **Sources:**
  - `Borderlands2.exe`, through `tools/ghidra/run.ps1` read-only queries and the local table reader, one headless run
    at a time under the machine lock;
  - field names from `tools/ghidra/class_layout.py`;
  - one local diagnostic run of the slice gate (an uncommitted print, reverted).
- Raw output stays under ignored `local/analysis/link9/`. Every rule is **UNVERIFIED**.
- **What the note says:**
  - Without a cylinder `CollisionComponent`, the extent comes from the class's bounding-cylinder rule.
  - `Actor`'s rule takes the union of the bounds of the attached, colliding primitive components without the skip
    bit. The radius is the horizontal half-diagonal, and the half height is half the box's height. With none, it is
    all zeros.
  - The four pawn classes share `Pawn`'s rule: their own `CylinderComponent` when it is a separate cylinder, else
    `Actor`'s.
  - For link 9 the template has no cylinder at all and no attached component, so the extent is (0, 0, 0): a point
    search. The link then stops at the missing collision service.
- **#470's cylinder reader is not this rule.** It reads an archetype's `CylinderComponent` for link 2's fight area. It
  does not apply to link 9's class-default template, which has none.
- Not implemented here: another lane codes the Off answers from the note.
