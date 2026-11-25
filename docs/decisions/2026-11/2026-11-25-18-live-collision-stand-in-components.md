## 2026-11-25: A live actor's stand-in component is ruled out of a collision query only by its bounds

AI-assisted (Claude Code)

Lane LIVE-COLLISION-STANDIN, with Claude Opus 5.5, from #681's review (P3, pre-existing on main). A port policy that
tightens a check; every rule here is UNVERIFIED in the running game. No sensitive file, no dependency, no bounds check
changed.

- **Why:** the live collision walks keep an actor's part only when its class derives from
  `Engine.PrimitiveComponent`, and skip the rest before the service bounds or any filter is read. The walks are
  `placement::actor_walk` (the spawn's spot search and the pickup touch trace's line check, `touch_trace.rs`) and
  `Actor.Trace`'s walk (`actor_trace.rs`). A part whose class is an engine-native stand-in (decision 2026-11-15-08:
  no package export, defined directly under `Core.Object`) fails that test although its real chain is not known. The
  query could then report clear where the game might meet the part. A class table with no `PrimitiveComponent` at all
  also ended each walk at once with "clear".
- **The rule** (`trace_filter::part_class`, shared by the walks): the part's class chain is walked up.
  `PrimitiveComponent` is a primitive, and the walk goes on as before. A chain of package classes that does not reach
  it is skipped, as before. A chain that runs through a native stand-in first is uncertain:
  - service bounds that miss the query's region rule it out. `Actor.Trace` also requires the bounds to be finite and
    ordered, as it does for primitives;
  - otherwise the query answers its own `NotYet { NotRead }`: "<query prefix>: a live actor component whose class runs
    through the engine-native stand-in class (decision 2026-11-15-08) <class>, so whether it is a primitive is not
    known (the live actor component <actor>'s <part>)". That is `SPOT_SEARCH` and `NATIVE_SPAWN_PLACEMENT.md` for the
    spawn, the touch trace's prefix and record for the pickup, and `Actor.Trace` with `NATIVE_ACTOR_TRACE.md`.
  - With no `PrimitiveComponent` in the table no package part is a primitive, but the walk still runs, so a stand-in
    part is still refused.
- **Not changed:** the spawn extent's own preload (`placement.rs` `is_primitive`) keeps selecting known primitives, and
  the bounding cylinder's walk is #681's.
- **Tests (synthetic):** the spawn's spot search and `Actor.Trace`, each with a live actor whose collision component
  is a native stand-in. Disjoint bounds make the spawn or the miss; meeting bounds, or none, give `NotRead` naming the
  part, with nothing made and `Actor.Trace`'s counters unchanged. The existing walk tests pass unchanged.
- **Measured** (zuhuPC, Linux, release builds under the machine lock, Aspyr install): installed Off strict
  `--slice-gate` on main 373fe4b and on this branch gives the same result and first stop on all 14 links. Cause A
  (#679) limits that install, so this covers links 2 and 6-9. Not measured on a Windows install.
- **Follow-up:** #681's `bounding_cylinder::primitive_class` and this `part_class` apply the same stand-in rule. Once
  both land, one can call the other.
