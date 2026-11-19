# Native: where a spawn is placed, the spawn's spot search and encroachment check (2026-11-19)

AI-assisted (Claude Code). Lane ANALYST-SPAWN-PLACEMENT, analyst only (clean room, docs/LANE_BRIEF.md; decision
2026-11-19-77). The sources, in the order used:
- the records (NATIVE_ENGINE_CORE.md, "Actor.Spawn" and the spawn routine, steps 3 and 9, which leave the spot-finding
  routine and the encroachment check open);
- the installed packages, through `tools/ghidra/class_layout.py`, to name the flags;
- `Borderlands2.exe` locally, with the project's Ghidra tooling (read-only queries under the machine lock).

Raw output stays under ignored `local/analysis/`; this note is in our own words. No bl-sdk session was granted, and no
recorded live evidence covers it. **Every rule here is UNVERIFIED** until the live checks at the end confirm it. The
lane that wrote this note does not implement it: the port's spawn keeps its `NotYet` until a fresh implementer codes the
rules from this note alone.

**Why it matters.** Slice link 9 (the Phaselock cast, PR #454) spawns through `ow_world::actors::spawn`, which stops at
its own `NotYet`, "the spawn placement spot search".

## Summary

| Question | Answer | Confidence | Status |
|---|---|---|---|
| When the spot search runs | the template's `bCollideWorld`, or its `bCollideWhenPlacing` outside a network client, and `bNoCollisionFail` false | high | UNVERIFIED |
| What it tests | an axis-aligned box of the template's collision cylinder extent against world geometry | high | UNVERIFIED |
| What it changes | only the spawn location (it may move it); nothing else | high | UNVERIFIED |
| When it fails | when no free spot is found by the steps of section 3: the spawn returns None and makes nothing | high | UNVERIFIED |
| The encroachment check | after `PreBeginPlay`, whenever `bNoCollisionFail` is false; a failed check destroys the new actor and the spawn returns None | high for when, low for its internals (not read) | UNVERIFIED |
| `bNoCollisionFail` with `bCollideActors` | a separate relocation step instead of the check | high for when, not read | UNVERIFIED |

## 1. When the spot search runs

The spawn routine (NATIVE_ENGINE_CORE.md, step 3) runs the search only when all of these hold:
- the caller's `bNoCollisionFail` is false;
- the template (the class default object, or the `ActorTemplate` passed) has `bCollideWorld` set, **or** it has
  `bCollideWhenPlacing` set and the world's net mode is not a network client.

Otherwise the requested location is used as it is. The flags are read from the template, before the actor exists.

## 2. What it reads

- **The extent.** The template's collision cylinder, as a box:
  - when the template's `CollisionComponent` is a cylinder component, its `CollisionRadius` in both horizontal axes
    and its `CollisionHeight` vertically (a half height);
  - otherwise the actor's own bounding-cylinder rule gives a radius and a height, used the same way. That rule is a
    per-class engine method, not read here.
- **`bCollideComplex`** on the template chooses complex (per-polygon) collision for the first box test.
- **The requested location.** The search may move it in place; the moved location is where the actor is made.
- Nothing about other actors: the search tests world geometry only. Other actors are the encroachment check's
  business (section 5).

## 3. The search, step by step

"Free" below means a box of the given extent at that point overlaps no world geometry. "A line check" is a trace
between two points against world geometry. The trace kinds are fixed engine flag sets (not identified here).

1. **The box at the requested location is free:** success, the location unchanged.
2. **A zero extent** (a point that is not free): failure.
3. **A horizontal slice.** The test is repeated on a box of the same horizontal size and a height of 1 at the location,
   with simple collision.
   - **If that slice is not free:** go to step 4.
   - **If it is free:** trace down from the location by twice the extent height, then adjust the height:
     - no floor hit: lower the location by one extent height;
     - a hit in the lower half of the trace (hit fraction above 0.5): lower it by the extent height times
       (2 × fraction − 1), then raise it by 1;
     - a hit in the upper half (fraction 0.5 or less): raise it by the extent height times (1 − 2 × fraction), plus 1.

     Then test the box there. If it is free, trace up by one extent height; on a hit, the location becomes that hit
     point. The result is a success. If it is not free, shift the location horizontally by the extent's X size along
     the floor hit's horizontal normal and test once more: success when free, failure otherwise. In this branch, a
     failure here ends the whole search as a failure (step 4 does not run).
4. **Corner nudges** (only when the slice was not free). The location is put back to the requested one.
   - Four small boxes are tested, half the horizontal extent and height 1, at the four corners displaced by ±0.55 × the
     extent's X and ±0.55 × its Y.
   - Each free corner, up to two, moves the location toward itself by 1.1 × its displacement. With exactly one free corner the
     total move is doubled. With none, the search fails.
   - A line check from the requested location to the moved one must not hit; if it hits, the search fails.
   - The box at the moved location must then be free, or the slice step of 3 must succeed there; if neither, the search
     fails.
   - A final swept-box check from a point 20% of the move behind the requested location, to the moved location: with
     no hit, success at the moved location; on a hit, success at the hit point.

Constants: 0.5 (the half-height split), 1 (the slice height and the vertical margin), 0.55 and 1.1 (corner displacements and
moves), 2 (the trace length in extent heights, and the single-corner doubling), 0.2 (the final sweep's start).

## 4. What a failed search does

The spawn returns None before the object is made: no actor, no events, no owner change (NATIVE_ENGINE_CORE.md, step
3). A successful search only changes the location the actor is made at.

## 5. The encroachment check, and the relocation step (step 9 of the record)

- **When `bNoCollisionFail` is false:** after `PreBeginPlay` and the per-component engine call, the world runs an
  encroachment check of the new actor at its final location and rotation, with touch notification on. This happens
  for every such spawn: the spawn routine does not test `bCollideActors` first. Whether the check itself returns at once
  for an actor that does not collide is inside the check, which was not read. If the check reports encroachment, the
  actor is destroyed through the ordinary destroy sequence and the spawn returns None.
- **When `bNoCollisionFail` is true and the actor has `bCollideActors`:** a separate engine step runs instead of the
  check (inferred as a relocation or touch update; not read). If it destroys the actor and the actor is not a "no-fail"
  spawn, the spawn returns None.

## 6. For an implementer: the Off answers, step by step

With `req` the spawn request and `template` the class default object, or the archetype the port uses:

1. `req.no_collision_fail` true: no spot search; the location as requested.
2. Otherwise, if `template.bCollideWorld`, or `template.bCollideWhenPlacing` and the world's net mode is not a client,
   the search runs:
   - **a world with no collision model** (today's port): a `NotYet` of kind `LayerAbsent`, "the spawn placement spot
     search", before anything is made. This is the port's current answer and it stays right: answering "free" without
     collision would be a fitted choice;
   - a world with a collision model that holds no geometry near the location: step 1 of section 3 succeeds, so the
     location is unchanged;
   - a template whose `CollisionComponent` is not a cylinder component, or which has none: a `NotYet` of kind
     `NotRead`, "the actor's bounding-cylinder rule", since its extent comes from a per-class rule not read;
   - with collision: the steps of section 3, then None on failure, or the moved location on success.
3. Otherwise: the location as requested.
4. After `PreBeginPlay` (only after play began), with `req.no_collision_fail` false: the encroachment check.
   - Without collision: a `NotYet` of kind `LayerAbsent`, "the post-spawn encroachment check".
   - The port's present shortcut, passing an actor without `bCollideActors`, is not shown by this read: the spawn
     routine calls the check whatever the flag. Keep the shortcut labelled UNVERIFIED, or refuse with the same
     `NotYet` until the check's internals are read (open question 1).
5. With `req.no_collision_fail` true and `bCollideActors`: a `NotYet` of kind `NotRead`, "the post-spawn relocation
   step".

## 7. Where a later collision test could come from

The host's movement collision (lane H4, `crates/ow-host/src/movement/collide.rs`, decision 2026-11-16-52) is a world of
convex brushes with a swept cylinder or capsule trace that reports a start inside solid. It could supply the tests
above later:
- a zero-length trace that starts in solid, for "the box is not free";
- point and shape traces, for the line checks and the final sweep.

Two differences need settling first, each UNVERIFIED:
- **The shape.** The game's spawn test is an axis-aligned box of the cylinder extent, while H4 sweeps a cylinder or
  capsule.
- **The geometry.** H4 holds static-mesh simple hulls only: no BSP, terrain or per-polygon collision yet, and
  `bCollideComplex` asks for per-polygon collision.

The world's spawn would also need the host's collision world as a VM service, which does not exist yet.

## Open questions

1. The encroachment check's internals: which actors it tests against, whether an actor without `bCollideActors`
   passes at once, and what touch notification sends.
2. The relocation step for a `bNoCollisionFail` spawn of an actor that collides with actors.
3. The bounding-cylinder rule for a template without a cylinder collision component.
4. The trace flag sets of the box tests and the line checks: which geometry they include.
5. Whether the net-mode test compares with the client mode only (as read), so a listen server counts as not a client.

## Live checks that would confirm these rules (bl-sdk, the RTSE way)

- Spawn an invented test actor class with `bCollideWorld` at a point inside a wall, and read the returned actor and its
  `Location`: expect a moved location for a shallow overlap, and None deep inside.
- The same with `bNoCollisionFail`: expect the actor at the requested location.
- Spawn, twice at the same open spot, a class with `bCollideActors` and `bBlockActors`: expect the second spawn to
  return None (the encroachment check), unless `bNoCollisionFail` is passed.
- Spawn the Phaselock cast's actor classes as the slice does, and read their `bCollideWorld` and `bCollideWhenPlacing`
  defaults, to know whether link 9 meets the search at all.
