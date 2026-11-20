## 2026-11-19: Actor.Trace leaves out primitives the collision hash cannot admit (an analyst amendment)

AI-assisted (Claude Code)

Lane LINK9-NEXT-STOP-2, analyst only, with Claude Opus 5.5. The file number is a placeholder until the maintainer
gives one. Docs only: no code, no dependency, and no bounds check changed.

- **The stop:** a local throwaway merge, never pushed, of main b9dfca8b with #650, #655 and #616's own commits.
  Installed Off `--slice-gate`, strict: ok 4, notyet 10, error 0. Link 9 passes 1 of 2 steps, and `cast` stops at
  `Actor.Trace`'s T5, "the colliding components' bounds".
  - #643 does not answer it: its bounds work covers `GetBoundingCylinder` and the attach record, not the collision
    service's `actor_bounds`.
  - A probe line named the primitive: Maya's `WillowPlayerController`'s `CylinderComponent`, with `CollideActors`,
    the owner's `bCollideActors` and `bAttached` all clear.
- **Not a harness gap or a registration:** it is the Trace rule's walk order. NATIVE_SPAWN_TRACE_FILTER.md section 5
  already reads that such a primitive is never in the collision hash, so no query meets it.
- **What:** NATIVE_ACTOR_TRACE.md section 5 amends section 3.3, step 2. Before the bounds, a primitive whose
  `CollideActors` is clear, or whose owner's `bCollideActors` is clear, is left out. It is read-only, and tests are
  listed. The NATIVE_INDEX row gains the amendment, on the live `Actor.Trace` row (main carries a stale duplicate,
  left as it is).
- **Clean room:** this lane's author wrote NATIVE_ACTOR_TRACE.md and NATIVE_SPAWN_TRACE_FILTER.md, so a different
  lane implements the amendment.
- **Past it, with the change stood in locally:** ok 4, notyet 9, error 1. Trace answers a miss, and the light attach
  runs. `cast` stops with an error at the unbound native
  `WillowPlayerController.UnlockPlayerClassAchievementIfConditionsMet(byte,int,bool,bool)`, which has no record yet.
- **UNVERIFIED** in game.

## Edits: review r1 of #659

1. **Section 5 replaces** section 3.3's "without reading any filter field" and test 5's wording for the two flags. The
   existing fixtures must declare both flags and set them on every obstacle.
2. **The terrain exception is kept:** no skip for a terrain actor or owner, which goes on to T7 naming
   `TERRAIN_HANDLING`. There is a test.
3. **The inference is labelled UNVERIFIED in its own right:** the same hash and per-component test for Trace's line
   check, and removal from the hash, were not read. It is open question 4. Link 9's cylinder was never attached.
4. **Order tests:** flags clear with meeting bounds is a miss, and an unresident owner is T1 first.
5. **`BulletListener` is not read:** a set bit reaches the bounds (the cautious side).
6. **Open question 2** is rephrased.
7. **The index edit** moved to the live row, with its parenthesis closed.
