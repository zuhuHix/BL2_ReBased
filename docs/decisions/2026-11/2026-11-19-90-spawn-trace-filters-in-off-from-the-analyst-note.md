## 2026-11-19: The spawn's trace filters in Off from the analyst note

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-SPAWN-TRACE-FILTERS, implementer. This lane coded the spot search's
trace filters from `docs/verification/NATIVE_SPAWN_PLACEMENT.md` section 7 alone. That is analyst lane
ANALYST-SPAWN-TRACE-FILTERS, decision 2026-11-19-84, PR #465, read at head 603691f and re-checked before hand-back with
no change since. This lane opened no analysis output, Ghidra or REA. There was no real-game session, so every rule
stays **UNVERIFIED** in the running game. No dependency and no crate edge was added, no bounds check was loosened, and
Reference is unchanged. It stacks on #463 (decision 2026-11-19-79).

- **`ow-world` `trace_filter.rs` (new), Off only.**
  - **The flag sets** as the note gives them: 8902 for every box test, with 131072 added for a template-mode test of
    a `bCollideComplex` template; 8326 for every sweep, with 1024 added by the sweep routine. A box test always walks
    the actor collision hash; a sweep walks it under the mask 8347. The spawn's queries pass no source actor.
  - **Who is tested:** a primitive with the query's blocking flag (`BlockNonZeroExtent` for a non-zero box test and a
    box sweep, `BlockZeroExtent` for a zero-extent box test and a point sweep), on an owner whose class row accepts
    the flag set. The rows are the note's: `Actor` (and the classes that keep its rule), `Volume`, the movers
    (`InterpActor`, `KActor`) and `Pawn`. With no source actor, bit 64 rejects. A class the note did not read takes
    its nearest read ancestor's row (UNVERIFIED: it may override the filter).
  - **The unread points, as the note's Off policy:** the primitive skip flag is unresolved, so every tested candidate
    counts as one that may have it, and the query is Unsupported. The further per-component test is taken as passing,
    and a terrain-specific effect of bit 8192 is ignored.
  - **The sweep's hit choice:** the BSP hit; the actor hits up to 5 units past it along the sweep (the whole sweep
    without one), the smallest fraction kept; then the smaller fraction of the two. An exact tie whose locations or
    normals differ is Unsupported.
- **`placement.rs`:** the filter gate of decision -79 (`TRACE_FILTER`, `FILTER_READ`) is gone. Each query now has two
  parts. The service answers for the geometry it holds (the BSP and whatever else it loaded). This crate answers for
  the VM's live actors: their `CollisionComponent` and the primitives in `Components`, scanned once per search. A
  tested live primitive makes that query `NotYet { NotRead }`, naming the actor and its component, because the world
  holds no geometry for it and its skip flag is unread. A box test the service reports blocked is blocked whatever the
  actors (bit 512, the BSP first). The search's steps of decision -79 now run in Off.
- **The collision service for link 9:** `placement::NoLevel`, installed in the cast's own world
  (`phaselock_world`). That world loads no level (decision 2026-11-16-42), so its service holds no geometry and
  every query of its part is clear. This is a harness stand-in, not a model of a loaded map: the live actors are
  still filtered as above. The hub's ruling (no collision service for link 9 until the filters are read) is met by
  this lane reading them.
- **The partial-write rules of #463 hold:** the search runs before anything is made, so its `NotYet`s carry no prefix.
  The post-spawn encroachment and relocation `NotYet`s keep the "stopped after partial writes" prefix, their kind and
  record. There is no Off rollback, and the gate grades by `VmError` variant.
- **Tests changed:** two spawn tests of decision -79 expected the filter `NotYet`. With the filters read, a clear
  scripted service and no tested live actor, the Off spawn is made where it was asked, and the tests now say so. Reference
  still answers its legacy refusal. The toy package gained a `PrimitiveComponent` parent for `CylinderComponent`, with
  the three blocking flags, and `bWorldGeometry` on `Actor`.
- **Verified (automated, local, desktop):**
  - 8 unit tests of the filters on invented classes and hits: the flag sets, the blocking flags, the note's row table,
    the class rules outside the spawn's sets, nearest-ancestor rows, `tested` on objects, the sweep hit choice and its
    ties;
  - spawn tests on the toy package: a clear service keeps the location after one box test; a live world-geometry
    actor's blocking cylinder makes the box test `NotYet { NotRead }` with nothing made, and the same actor without
    `bWorldGeometry` is not tested; Reference unchanged;
  - the workspace tests and crate edges (numbers in GATES.md).
- **Installed, Off:** `--slice-gate` strict 0/14/0. Link 9 is 0 of 1 driver steps. It no longer stops on the missing
  service (`LayerAbsent`); it stops one check later at `spawn` on "the actor's bounding-cylinder rule" (`NotRead`).
  The spawned pawn's template holds no `CylinderComponent` the port reads (decision -79's extent rule). That is the
  next item for link 9: whether the port's class default or archetype holds its collision cylinder.
- **UNVERIFIED:** every rule above; the nearest-ancestor rule for unread classes; the skip flag; the per-component test.
