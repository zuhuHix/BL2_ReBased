## 2026-11-19: The spawn's trace filters in Off from the analyst note

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-SPAWN-TRACE-FILTERS, implementer. This lane coded the spot search's
trace filters from `docs/verification/NATIVE_SPAWN_PLACEMENT.md` section 7 alone. That is analyst lane
ANALYST-SPAWN-TRACE-FILTERS, decision 2026-11-19-84, PR #465. It was first read at 603691f, then implemented again
from its review-r3 head 4ee2c09. This lane opened no analysis output, Ghidra or REA. There was no real-game session,
so every rule stays **UNVERIFIED** in the running game. No dependency and no crate edge was added, no bounds check was
loosened, and Reference is unchanged. It stacks on #463 (decision 2026-11-19-79).

- **`ow-world` `trace_filter.rs` (new), Off only.**
  - **The flag sets** as the note gives them: 8902 for every box test, with 131072 added for a template-mode test of
    a `bCollideComplex` template; 8326 for every sweep, with 1024 added by the sweep routine. A box test always walks
    the actor collision hash; a sweep walks it under the mask 8347. The spawn's queries pass no source actor.
  - **Who is a candidate:** a primitive with the query's blocking flag (`BlockZeroExtent` for a box test of extent
    (0, 0, 0) and a point sweep, `BlockNonZeroExtent` otherwise), on an owner whose class row accepts the flag set.
    Only the classes the note read have a row: `Actor`, `StaticMeshActor`, `Brush` and `Terrain` (the actor rule),
    `Volume`, the movers `InterpActor` and `KActor`, and `Pawn`. No row is inherited. With no source actor, bit 64
    rejects.
  - **The unread rules (note's Off policy, r3):** where one decides the answer, the query is Unsupported, naming it.
    - Every candidate is Unsupported, "the trace filter's per-component test": that test runs for every candidate and
      was not read. This also covers the unresolved skip flag.
    - A blocking primitive whose owner's class has no row is Unsupported, "the trace filter of <class>".
    - Any other blocking primitive is Unsupported, "the trace filter's skip flag" (review r1 of #477). The note makes
      a query with a blocking primitive that has the flag set Unsupported, and the flag cannot be read, so the owner
      row may not drop a blocking primitive first. Only a primitive that does not block the query's kind is answered.
    - A `Terrain` actor's primitive is Unsupported, "the trace filter's terrain handling".
  - **The sweep's hit choice** (`choose_sweep_hit`, a helper): a BSP hit alone is the answer, and a BSP hit together
    with an actor hit is Unsupported (the unread comparator). Among the actors every hit at the minimum fraction is
    kept: tied hits that agree give that hit, and a tie whose locations or normals differ is Unsupported. The
    comparator and tie cases are **helper coverage only**: placement never hands the helper an actor hit (below).
- **`placement.rs`:** the filter gate of decision -79 (`TRACE_FILTER`, `FILTER_READ`) is gone. Each query has two
  parts, kept apart (review r1 of #477):
  - **the level part**, from the service first. The `SpawnCollision` contract now says its three answers cover the
    level's own geometry only, never a live VM actor;
  - **the actor walk**, by this crate, only when the query reaches it. A box test the level blocks is blocked, and no
    actor is read for it (bit 512, the BSP first). The walk covers the query's region: the box, or for a sweep the
    swept box from the start to the BSP hit plus 5 units, capped at the end point (the whole sweep without a BSP hit).
    It visits the live actors' `CollisionComponent` and the primitives in `Components`;
  - a primitive under an unread rule leaves the walk only when the service reports its bounds (the new
    `actor_bounds`, default none) and they miss the region. Otherwise the query is `NotYet { NotRead }`, naming the
    rule, the actor and the component (and, with no bounds, that the service does not hold them);
  - missing data on a live primitive (a property not loaded) is a named `NotRead` too, not a heap error;
  - a sweep's result goes through `choose_sweep_hit`, with walk hits rescaled to the whole sweep. The walk refuses
    before it could record an actor hit, so **placement behaviour** is a level-only hit or no hit; the capped segment
    decides which primitives the walk visits;
  - the search's steps of decision -79 now run in Off.
- **The collision service for link 9:** `placement::NoLevel`, installed in the cast's own world
  (`phaselock_world`).
  - That world loads no level (decision 2026-11-16-42), so its service holds no geometry and every query of its part
    is clear.
  - This is a harness stand-in, not a model of a loaded map: the live actors are still filtered as above.
  - The hub's ruling (no collision service for link 9 until the filters are read) is met by this lane reading them.
- **The partial-write rules hold (note r3, #463):**
  - the search runs before anything is made, so its `NotYet`s carry no prefix;
  - from the made actor on, every `NotYet` that escapes the spawn is terminal, from `PreBeginPlay`, the checks or a
    later event: prefixed "stopped after partial writes", with its kind and record kept;
  - there is no Off rollback, and the gate grades by `VmError` variant.
- **Tests changed:**
  - Two spawn tests of decision -79 expected the filter `NotYet`. With a clear scripted service and no live primitive
    under an unread rule, the Off spawn is now made where it was asked, and the tests say so.
  - Reference still answers its legacy refusal.
  - The toy package gained a `PrimitiveComponent` parent for `CylinderComponent`, with the three blocking flags, and
    `bWorldGeometry` on `Actor`.
- **Verified (automated, local, desktop):**
  - unit tests of the filters on invented classes and hits: the flag sets, the blocking flags, the note's row table,
    the class rules outside the spawn's sets, rows for the read classes only, a candidate's per-component answer, an
    unread owner class and terrain, the BSP-only and actors-only sweep answers, the comparator case, the capped reach,
    and kept ties;
  - spawn tests on the toy package:
    - a clear service keeps the location after one box test;
    - an `Engine.Actor` blocker with `bWorldGeometry` gives the per-component `NotRead`, and without it the skip-flag `NotRead`; a
      primitive that does not block the query's kind is not tested;
    - a `Probe` blocker gives "the trace filter of Engine.Probe";
    - in each stop nothing is made, and Reference is unchanged;
  - review r1 of #477: the skip-flag refusal; with service bounds, a box missed or met; a box the level blocks reads
    no actor (a stand-in primitive whose properties are not loaded stays unread), while a clear one refuses on it by
    name; the capped reach and the walked region; a level-only sweep hit;
  - the workspace tests and crate edges: the counts are in PR #477's body (the gate record of this lane), not in
    GATES.md.
- **Installed, Off:** the `--slice-gate` summary is in the PR.
- **UNVERIFIED:** every rule above; the per-component test, the skip flag, unread owner classes, terrain handling, the
  BSP and actor comparator and the actor tie order stay unread, so Off refuses each.
