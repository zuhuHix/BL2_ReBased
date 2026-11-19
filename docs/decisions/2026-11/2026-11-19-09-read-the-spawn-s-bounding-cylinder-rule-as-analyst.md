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
  - For link 9 the template has no cylinder at all. Its components are loaded first (a write), and none is attached,
    so the extent is (0, 0, 0): a point search.
  - On the base, the cast's world has the `NoLevel` service. The zero-extent first test walks the only live actor, the
    `WorldInfo`. So the link then stops either at a trace-filter refusal there, or, after success at L0, at the
    post-spawn encroachment check (terminal, prefixed).
- **#470's cylinder reader is not this rule.** It reads an archetype's `CylinderComponent` for link 2's fight area. It
  does not apply to link 9's class-default template, which has none.
- Not implemented here: another lane codes the Off answers from the note.

**Review r1 of #488 (fixed in the note).**
1. The next stop is rewritten from the base code. The service is checked first (the cast's world has `NoLevel`), then
   the zero-extent walk over the `WorldInfo`, then either a trace-filter refusal or success and the post-spawn
   encroachment check. "#477 checks the extent first" is dropped.
2. The template's components are lazy handles. A primitive entry is loaded as the template's own definition before
   the spot search, and that load is a write, so every later `NotYet` is marked. `bAttached` is read from the loaded
   entry, not assumed.
3. The skip flag is a named `NotYet { NotRead }` (`trace_filter::SKIP_FLAG`), decided after the primitive, `bAttached`
   and `CollideActors` tests.
4. A missing field is `VmError::Script`, as the port's field reads answer it.
