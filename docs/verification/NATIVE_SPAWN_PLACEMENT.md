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

**Terms.**
- An *extent* is a half size. The spawn's extent is (radius, radius, half height).
- "A box test" asks whether an axis-aligned box of a given extent at a point overlaps world geometry. Each test runs
  in one of two collision modes. *Template mode* uses complex (per-polygon) collision when the template has
  `bCollideComplex`; *simple mode* always uses simple collision.
- "A sweep" traces a box (or a point) from a start to an end and reports the first hit, its location, its normal and
  its fraction of the way. The sweeps use one fixed engine flag set (not identified here).

**The steps.** E = (Ex, Ey, Ez) is the extent; L is the location, changed in place; L0 is the requested location.

1. **First test.** A box test of E at L0 in template mode. If it is free: success, with L = L0.
2. **A zero extent** (a point that is not free): failure.
3. **The slice step** (it also runs once more inside step 4). The slice extent is (Ex, Ey, 1): the `1` is a Z half
   extent.
   1. A box test of the slice at L, in simple mode. If it is **not** free, the slice step reports "the slice is
      blocked" and fails.
   2. A sweep of the slice downward from L to L − (0, 0, 2 × Ez). Then the height is set:
      - **a hit at fraction t ≤ 0.5:** L.z += Ez × (1 − 2t) + 1, then a box test of E at L in simple mode. If it is
        free, the slice step succeeds at L, with no further sweep;
      - **a hit at fraction t > 0.5:** L.z = L.z − Ez × (2t − 1) + 1;
      - **no hit:** L.z = L.z − Ez.
   3. In the last two cases, and in the first when its test was not free (a common tail):
      - a box test of E at L in simple mode;
      - if free: a sweep of E upward from L to L + (0, 0, Ez). On a hit L becomes the hit location. The slice step
        succeeds;
      - if not free: L moves horizontally by Ex times the floor hit's normal (its X and Y parts), and one more box test
        of E at L in simple mode decides: success when free, failure otherwise.
      - **With no floor hit** there is no normal: the move is zero, the last test repeats the blocked test, and the
        slice step fails.
4. **The search's own steps.** At L0, after step 1 failed:
   1. The slice step at L0.
      - If it succeeds, so does the search, at its L.
      - If it fails with the slice **not** blocked, the search fails (no corner step).
      - If the slice was blocked, go on.
   2. **Corner nudges.** L is set back to L0. Four corner boxes are tested in template mode, each of extent
      (0.5 × Ex, 0.5 × Ey, 1) at L0 + (sx × 0.55 × Ex, sy × 0.55 × Ey, 0). They go in this order:
      (sx, sy) = (−1, −1), (−1, +1), (+1, −1), (+1, +1).
      - The first two **free** corners in that order count, and later free corners are ignored (with three free, the
        last of them in the order is ignored). Each counted corner moves L by 1.1 times its displacement.
      - With exactly one counted corner, the move is doubled (L = 2L − L0). With none, the search fails.
   3. A point sweep from L0 to L. A hit fails the search.
   4. A box test of E at L in template mode. If it is not free, the slice step of 3 runs at L (it may change L). If
      that fails too, the search fails.
   5. **The final sweep** of E from L (after any slice correction in 4.4) to L0 + 0.2 × (L0 − L): a point beyond the
      requested location, on the far side from the move. No hit: success at L. A hit: success at the hit location.

Constants: 0.5 (the fraction split, and the corner box scale), 1 (the slice and corner boxes' Z half extent, and the
vertical margin), 2 (the downward sweep's length in Ez, and the single-corner doubling), 0.55 (corner displacement),
1.1 (corner move), 0.2 (the final sweep's overshoot).

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

`req` is the spawn request. `template` is the object the actor is made from: the supplied archetype, else the class
default object.

**Which object's flags.** The spawn routine reads the template's flags.
- The port's spawn reads both the class default and a supplied archetype today, and refuses when either asks for the
  search (`ow_world::actors`, the pre-checks).
- The intended Off change is to read **the template alone**: an archetype with `bCollideWorld` over a class default
  without it runs the search, and a class default with it under an archetype without it does not. The other flags
  follow the same rule.

**The spot search (before anything is made)**, in this precedence:
1. `req.no_collision_fail` true: no search; L = the requested location.
2. Neither `template.bCollideWorld` nor (`template.bCollideWhenPlacing` and the net mode not a client): no search;
   L = requested.
3. **No collision service** in the world: a `NotYet` of kind `LayerAbsent`, "the spawn placement spot search". This
   answer comes before the extent question.
4. **No cylinder extent** (the template's `CollisionComponent` is not a cylinder component, or there is none): a
   `NotYet` of kind `NotRead`, "the actor's bounding-cylinder rule". So a template without a cylinder in an empty
   collision model answers this, never "unchanged".
5. **The first test (section 3, step 1).** It must be a query the service supports (section 7). Unsupported: a `NotYet`
   of kind `NotRead`, naming the query ("an axis-aligned box test in template mode", for example). Supported and free:
   L = requested.
6. Steps 2 to 4 of section 3 run while every query they make is supported. The first unsupported query is a `NotYet`
   of kind `NotRead` naming it. None of these steps writes before the actor is made: the search only computes L.
7. The search fails: the spawn returns None, nothing made.

**The encroachment check (after `PreBeginPlay`)**, one answer per case. These steps run after the actor is made, so
any `NotYet` here is terminal ("stopped after partial writes", its kind kept):
1. Before play began: not run (the record's step 6 onwards is skipped).
2. **`PreBeginPlay` destroyed the actor:** the spawn returns None. This early return comes first, before any check.
3. `req.no_collision_fail` false, play begun, the actor alive:
   - the **live actor's** `bCollideActors` false: the check is taken to pass. This is the port's Off answer, not a
     reading: the spawn routine calls the check whatever the flag, and whether the check returns at once for such an
     actor is inside it (open question 1). UNVERIFIED;
   - `bCollideActors` true, with no collision service: a `NotYet` of kind `LayerAbsent`, "the post-spawn encroachment
     check";
   - `bCollideActors` true, with a collision service: a `NotYet` of kind `NotRead`, "the post-spawn encroachment
     check's rule" (open question 1).
4. **The relocation step:** only after play began, with `req.no_collision_fail` true, and the **live actor's**
   `bCollideActors` true (read after `PreBeginPlay`): a `NotYet` of kind `NotRead`, "the post-spawn relocation
   step". Otherwise it does not run.

**Replacing today's eager guard.**
- The port refuses before anything is made when the class default or archetype has `bCollideActors`, then checks the
  live actor again after `PreBeginPlay` (`ow_world::actors::spawn`).
- The Off change is to drop the first refusal and decide only on the live actor, after `PreBeginPlay` and after the
  destroyed check. `PreBeginPlay` can then still change the flag.
- The cost is that such a refusal comes after the actor was made, and so is terminal as above.

## 7. The collision service a later implementation could use

**The boundary.** `ow-world` would declare a small collision service as a VM service: a trait and its registration, with no
`ow-assets` or `ow-host` dependency. A host registers an implementation of it. The spawn asks it three kinds of query,
each answered **Clear**, **Hit**, or **Unsupported**:
- a box test: an extent, a point, and the collision mode (template or simple);
- a box sweep: an extent, a start and an end; a hit carries its location, normal and fraction;
- a point sweep: a start and an end.

**Availability is per query.** It is the shape the implementation can test and the geometry it holds near the
points. A query is answered only when both are covered. An empty candidate set inside covered geometry is Clear. An
empty set because the points are outside the covered geometry is Unsupported, never Clear. The location stays
unchanged (section 6, step 5) only after a **supported** first test reports Clear.

**The host's movement collision** (lane H4, `crates/ow-host/src/movement/collide.rs`, decision 2026-11-16-52) is a
world of convex brushes with a swept cylinder or capsule trace that reports a start inside solid.
- **Box queries:** it cannot answer them, whether tests or sweeps, since it has no box shape. They stay Unsupported
  until it gains one.
- **Point sweeps:** a zero-radius trace could answer them within its geometry.
- **Its geometry** is static-mesh simple hulls. It has no BSP, terrain or per-polygon collision, so template-mode
  tests for a `bCollideComplex` template stay Unsupported.
- **Coverage:** the implementation must report where its geometry stops.

## Open questions

1. The encroachment check's internals: which actors it tests against, whether an actor without `bCollideActors`
   passes at once, and what touch notification sends.
2. The relocation step for a `bNoCollisionFail` spawn of an actor that collides with actors.
3. The bounding-cylinder rule for a template without a cylinder collision component.
4. The trace flag sets of the box tests and the line checks: which geometry they include.
5. Whether the net-mode test compares with the client mode only (as read), so a listen server counts as not a client.

## Live checks that would confirm these rules (bl-sdk, the RTSE way)

For every case, capture:
- the returned actor (or None) and its final `Location`;
- the events in order: `GainedChild`, `PreBeginPlay`, `PostBeginPlay`, `SetInitialState`, `Touch`, `Destroyed`.

1. **Client versus server flags.** Spawn a class with `bCollideWhenPlacing` only, at a blocked point. On a listen
   server, expect the search (a moved location or None). On a client, expect no search (the requested location). A
   client session may not be available: an open verification gap.
2. **Class default versus archetype.** With an archetype whose `bCollideWorld` differs from its class default, spawn
   both ways at a blocked point. Expect the archetype's flag to decide.
3. **The no-cylinder fallback.** Spawn a template whose `CollisionComponent` is not a cylinder, at a blocked point.
   Read the extent the move implies from the final location (open question 3).
4. **Floor hit versus floor miss.** At a point a little above a floor, and at a point over a gap, spawn a box that
   does not fit at the requested height. Expect the height set from the floor hit, and failure over the gap when the
   lowered box is blocked.
5. **The corner choice.** Against an inside corner (three of the four corners free), expect the move toward the first
   two free corners in the order of section 3.
6. **Simple versus complex collision.** Spawn the same class with and without `bCollideComplex` next to a mesh whose
   simple and per-polygon collision differ.
7. **The encroachment check.** Spawn, twice at the same open spot, a class with `bCollideActors` and `bBlockActors`.
   Expect the second spawn to return None, with `Destroyed` after `PreBeginPlay`, unless `bNoCollisionFail` is
   passed.
8. **A `PreBeginPlay` flag change.** With a test class whose `PreBeginPlay` sets `bCollideActors`, repeat case 7. Expect
   the check to follow the live flag.
9. **The cast.** Spawn the Phaselock cast's actor classes as the slice does, and read their `bCollideWorld` and
   `bCollideWhenPlacing`, to know whether link 9 meets the search at all.

A case that needs an invented test class depends on the SDK being able to define one. Where it cannot, the case is an
open verification gap.
