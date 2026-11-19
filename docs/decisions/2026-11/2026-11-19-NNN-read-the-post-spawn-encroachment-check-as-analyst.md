## 2026-11-19: Read the post-spawn encroachment check as analyst

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-ENCROACHMENT-CHECK, analyst only (clean room; d2b6 implements). New
note `docs/verification/NATIVE_ENCROACHMENT_CHECK.md` and its NATIVE_INDEX row. The decision number is a placeholder
until the hub assigns one.

- **Sources:** `Borderlands2.exe`, through `tools/ghidra/run.ps1` read-only queries, one headless run at a time under
  the machine lock; field names from `tools/ghidra/class_layout.py`. Raw output stays under ignored
  `local/analysis/spawnspot/`. There is no bl-sdk session, so every rule is **UNVERIFIED**.
- **Read:**
  - the check runs only when the live actor's `bCollideActors` is set (otherwise no encroachment, no touch, no
    write);
  - its shape is the bounding cylinder;
  - the world's actor collision hash gives the overlaps;
  - an encroachment pass with skips (self, pending delete, based on the actor, a primitive that does not block by
    extent), then a per-class blocking test and the other actor's "is encroached" method. On encroachment the location
    is restored and the spawn destroys the actor and returns None;
  - a touch pass for the overlaps that do not block.
- **Off answers** (final after review r1 of #490; Off only, Reference keeps its pre-checks and texts):
  - `bCollideActors` clear passes with no write;
  - with it set: no service is `NotYet { LayerAbsent }` (main's answer); a live primitive with `CollideActors` is
    `NotYet { NotRead }` for the unmodelled per-component attach step; a moved actor, a level part that is not Clear,
    a live primitive with no service bounds and bounds that meet the region are each `NotYet { NotRead }`; an empty
    overlap set passes with no write;
  - the check returns them unmarked; the spawn's post-insertion boundary adds the partial-writes prefix.
- **Link 9:** the pawn spawns with only the `WorldInfo` live. It passes when `bCollideActors` is clear or no live
  primitive collides; otherwise it stops at the attach step or the overlap query, both `NotRead`.

**Review r1 of #490 (eight findings, fixed in the note).**
1. **The shape and the colliding primitives** are the live actor's, not the class default's. The port does not model
   the per-component attach step, so a `CollideActors` primitive gives a named `NotRead`, never "unattached, so pass".
   The (0, 0, 0) extent is labelled the port's template value.
2. **The walk:** the port walks the live list itself (no overlap query in `SpawnCollision`), with a stated precedence;
   unreported bounds are `NotRead`, as in #477.
3. **No service** stays `LayerAbsent`; whether the query tests the BSP is not read, so a level part that is not Clear
   refuses; a world without the hash has an empty set in the game and is not a port case.
4. **Marking:** unmarked in the check, marked at the spawn's boundary.
5. **The location step** compares the passed location and rotation with the live `Location` and `Rotation`, and
   refuses on any difference whatever `Physics` is.
6. **Off only.** 7. **Grading** by the `Missing` kind. 8. **Follow-ups** for the implementer (NATIVE_SPAWN_PLACEMENT.md
   open question 1, the "taken to pass" doc comments); "never reachable" means `ControllerList` and `PawnList` only.
