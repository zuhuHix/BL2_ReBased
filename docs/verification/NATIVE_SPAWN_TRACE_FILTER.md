# Native rules of the spawn's trace filter on a controller's cylinder (draft, 2026-11-19)

AI-assisted (Claude Code, Opus 5.5), analyst lane ANALYST-LINK9-TRACE-FILTER (decision 2026-11-19-NNN, a placeholder),
for slice link 9 (the Phaselock cast's spawn).

**The question.** On `fix/rust-link9-extent` (#498, 40982583), link 9 of `ow-package --slice-gate` stops in `spawn`
(0 of 2 steps). The stop is the spot search's first box test, at "the trace filter of
WillowGame.WillowPlayerController": the live `WillowPlayerController_0`'s `CollisionCylinder`, a class whose filter
was not read (NATIVE_SPAWN_PLACEMENT.md section 7). This note says what decides whether the spawn's traces test that
cylinder or skip it, and what the spawn then does with the answer.

**How it was read.**
- **Recorded evidence first.** NATIVE_SPAWN_PLACEMENT.md section 7 (lane ANALYST-SPAWN-TRACE-FILTERS, decision
  2026-11-19-84) already reads the flag bits, the per-actor filters of `Actor`, `Pawn`, `Volume`, `InterpActor` and
  `KActor`, and the walkers' order. It leaves three points open: the primitive skip flag, one per-component test, and
  the terrain bit. No recorded live capture covers a controller's collision flags.
- **The script.** No UnrealScript decides this: the filter and the walkers are native.
- **The native**, read locally with Ghidra (read-only, under the machine lock), per docs/NATIVE_ANALYSIS.md:
  - the per-actor filter method of the four controller classes;
  - the per-component test;
  - the component attach routine's admission to the collision hash;
  - the box and sweep walkers.
- **The class defaults** come from the cooked packages (`ow-package --def-dump`), and field positions from
  `tools/ghidra/class_layout.py`.
- No bl-sdk session was granted.

Written in our own words, as behaviour only: no decompiler output, pseudo-code, addresses, offsets or listing
structure. **Every rule is UNVERIFIED in the running game.** The last section lists the live checks that would
confirm them.

**Clean room.** This lane implements nothing; a fresh implementer codes the Off answers from this note alone.

## Summary

| Piece | Kind | Confidence | Status |
|---|---|---|---|
| The per-actor trace filter of `Controller`, `PlayerController`, `GamePlayerController` and `WillowPlayerController` | native (an actor virtual) | high | UNVERIFIED |
| The walkers' per-component test (NATIVE_SPAWN_PLACEMENT.md open question 4) | native | high | UNVERIFIED |
| The collision hash's admission at attach | native | medium (one condition not read) | UNVERIFIED |
| The skip flag's name (open question 4) | native, inferred from the field layout | medium | UNVERIFIED |
| What link 9's spawn does with the controller's cylinder | follows from the above | high | UNVERIFIED |

## 1. The per-actor filter of the controller classes

The four classes on `WillowPlayerController`'s chain (`Controller`, `PlayerController`, `GamePlayerController`,
`WillowPlayerController`) do **not** override the per-actor filter. Each one runs exactly the `Actor` body, the
`Actor` row of NATIVE_SPAWN_PLACEMENT.md section 7:
- with `bWorldGeometry`, it is accepted when the query sets bit 128;
- otherwise it needs bit 16, then the narrowing.

The spawn's queries set neither bit 16 nor, for a controller (`bWorldGeometry` false in every class default), any
accepting path. So **the owner filter rejects a controller's primitive** for every box test (8902) and every sweep
(8326) the spawn makes.

Only these four exact classes were read. A further subclass may override the filter, as before.

## 2. The per-component test (NATIVE_SPAWN_PLACEMENT.md open question 4, now read)

This is the test the walkers run on each candidate before the owner filter. It passes when:
- the primitive's `CollideActors` is set, **and**
- the primitive has no `Owner`, or its `Owner`'s `bCollideActors` is set.

Both the box walker and the sweep walker call it, in the same place in their order. Here `Owner` is the component's own
owner field, which the game sets when the component is attached to its actor.
- **In the port** the spawn sets only the actor's `Owner`, never a component's owner field, so that field reads None
  on every live primitive. Read literally, every primitive would then be passed over (section 3, step 3). The port's
  owner is therefore **the walked actor**: the actor whose `CollisionComponent` or `Components` holds the primitive
  (section 7, step 2).

## 3. The order of one candidate's tests

Each test can only reject, and a rejected primitive is simply not tested (no hit, no side effect).

**The box walker** (the spawn's box tests). For each primitive in the hash cells the box meets:
1. a primitive already met in this query is passed over;
2. the skip flag set (section 4): passed over;
3. no `Owner`: passed over (in the port, the walked actor stands for it: section 2);
4. its `Bounds` do not meet the box: passed over;
5. the blocking flag for the query's extent (`BlockZeroExtent` for an extent of (0, 0, 0), else
   `BlockNonZeroExtent`) clear: passed over;
6. the per-component test (section 2) fails: passed over;
7. the owner's filter (section 1 and NATIVE_SPAWN_PLACEMENT.md section 7) rejects: passed over;
8. the primitive's own box test.

**The sweep walker** (the spawn's sweeps) runs the same chain, with three differences:
- the skip flag is ignored when the query sets bit 30 (1073741824). The spawn's sweeps (8326) do not set it;
- the walker also passes over a primitive whose owner is the query's source actor, or in its base chain. The spawn
  passes no source actor;
- the blocking-flag step follows the sweep's own rules (NATIVE_SPAWN_PLACEMENT.md section 7).

So the order of the unread and read tests no longer matters for a primitive that any one of them rejects.

## 4. The skip flag: `BulletListener` (inferred)

The skip flag is bit 10 of the primitive's collision bool word. That word's bits 2, 5 and 6 are the ones the
walkers read as `CollideActors`, `BlockZeroExtent` and `BlockNonZeroExtent`. The cooked declaration order puts
`BulletListener` at bit 10 of the same word.
- So the skip flag reads as `BulletListener`: a primitive with it set is never a candidate for the spawn's queries.
- The match rests on the declaration order of one bool word, not on a name in the executable: medium confidence,
  UNVERIFIED.
- `tools/ghidra/class_layout.py` places this word 8 bytes early. Its bit order agrees, and the field position does
  not change the reading.

## 5. The collision hash's admission

A primitive enters the world's collision hash when it is attached, and only if the per-component test of section 2
passes and one further condition holds. That condition is a query on the component's scene, which was not read.
- So a primitive with `CollideActors` clear, or whose owner has `bCollideActors` clear, is not in the hash at all, and
  no query meets it.
- Removal from the hash (at detach, or when the flags change later) was not read.

## 6. The controller's cylinder in link 9

**The packages' class defaults** (`WillowPlayerController`, its parents, and their `CollisionCylinder`):
- the actor: `bCollideActors` false, `bBlockActors` false, `bWorldGeometry` false, `bProjTarget` false;
- the cylinder: `CollideActors` false, `BlockActors` false, `BlockZeroExtent` true, `BlockNonZeroExtent` true,
  `bAttached` false, radius and height 22.

**In the game**, from sections 1 to 5:
- the cylinder is not admitted to the hash (section 5);
- if it were met, the per-component test would reject it (section 2);
- and the owner filter would reject it too (section 1).

So **the spawn's traces skip it**: it adds no hit and does not change the answer. Two independent rules give the same
outcome: the collide-actors test (which decides both the hash admission and the per-component test) and the owner
filter. The live controller's flags at the moment of the cast are UNVERIFIED (live check 1); a script that
turns on the controller's collision would change the first two.

**Which spawn, and what the walk visits.** Link 9's driver spawns, in order, a `WillowPlayerPawn`, a
`WillowPlayerController`, then a `WillowAIPawn`. The stop is in the **third** spawn, the `WillowAIPawn`'s, whose spot
search walks the live actors in the port's list order:
1. **`WorldInfo`:** no `CollisionComponent` and an empty `Components` in its class default, so it holds no primitive
   to test.
2. **`WillowPlayerPawn_0`:** its class default has no `CollisionComponent`. Its `Components` entries are a None
   entry, a special-move component, a dialog component, a stat component and two light environments. None of those is
   a primitive component, so it holds nothing to test either. In the game a pawn usually carries a cylinder and a mesh
   from its archetype; the port's spawn from the class default does not.
3. **`WillowPlayerController_0`:** its `CollisionCylinder`, the case above: not tested.

**What the spawn then does.** With nothing tested, the first box test is decided by the BSP alone
(NATIVE_SPAWN_PLACEMENT.md section 6, step 5). In link 9's world, the `NoLevel` service, that is Clear, so the
location stays the requested one. The actor is then made, its components instanced and `PreBeginPlay` sent.
- **The encroachment stop.** The post-spawn encroachment check (NATIVE_SPAWN_PLACEMENT.md section 6) answers its
  named `NotYet` when the **live** `WillowAIPawn` has `bCollideActors` set after `PreBeginPlay`. Its class default has
  it set, so that is the expected next stop. With it clear, the check is taken to pass.

## 7. Off answers, step by step (for an implementer)

The filter itself reads only and writes nothing, so it never marks its own `NotYet`. Whether its refusal reaches the
row marked depends on what ran before it:
- the spawn's extent loads (`load_extent_components`) write the store before the spot search, so a refusal after them
  is marked "stopped after partial writes: ";
- the slice driver marks every `NotYet` after the first committed spawn of its run.

Link 9 meets both: its third spawn comes after two committed spawns, and the extent loads run first. So link 9's
refusal is marked. A refusal is unmarked only in a first spawn that loads nothing.
1. **The controller rows.** Add `Engine.Controller`, `Engine.PlayerController`, `GameFramework.GamePlayerController` (the installed packages' path) and
   `WillowGame.WillowPlayerController`, exact classes only, to the read rows as the `Actor` row. Any other class still
   needs its own reading.
2. **The per-component test** (section 2) replaces the per-component `NotYet` (not read).
   - It reads the **live** primitive's `CollideActors` and the **live** owner's `bCollideActors` at query time. It
     does not read class defaults, because `PreBeginPlay` and script can change them.
   - **The owner** is the component's owner field when it is set, and otherwise the walked actor (section 2). A
     primitive held by no walked actor is never met by the port's walk.
   - Its failure is "not tested".
3. **The order.** Every test of section 3 only rejects, so an implementation may evaluate them in any order. The
   candidate is "not tested" when any read test rejects it. An unread rule (for example an owner class not read) still
   makes the query Unsupported only when every read test passes.
   - **Terrain is the exception.** What bit 8192 does beyond entering the actor hash was not read, so whether it
     only rejects is not known (inferred at most, UNVERIFIED). A primitive of a `Terrain` actor stays Unsupported
     ahead of every other test, as `ow_world::trace_filter::tested` does today.
4. **The skip flag** (section 4): read `BulletListener` from the live primitive.
   - Set: not tested, except in a sweep that sets bit 30, which the spawn never does.
   - It is the inferred name, UNVERIFIED (live check 3).
5. **Hash admission** (section 5). The port walks the live actors' primitives, not a hash.
   - **A primitive that was never attached** is never in the game's hash, so it is not tested. The port can answer
     that only where it knows the primitive was never attached. A `bAttached` that reads clear does not show this on
     its own: a primitive attached and later detached reads clear too, and whether detach removes it from the hash
     was not read (open question 1). So `bAttached` clear alone is not an answer here.
   - A primitive that fails the per-component test is not tested whether or not it is in a hash, so that case is
     answered.
   - A primitive that passes it, blocks the query's kind, and whose owner the filter accepts, may still be outside the
     game's hash, through the unread scene condition or the unread removal rules. For it, the query stays Unsupported,
     `NotYet` (not read), "the collision hash's admission beyond the per-component test".
6. **What this note supersedes** in the neighbouring records (pointers were added in place):
   - **NATIVE_SPAWN_PLACEMENT.md section 7:**
     - "Who is tested", step 2: the skip flag's name (now `BulletListener`, section 4) and the unread per-component
       test (now section 2);
     - "The Off policy for what is unread": the per-component test and skip-flag entries, which give way to steps 2
       and 4 here;
     - open question 4, except the terrain bit.
   - **NATIVE_SPAWN_EXTENT.md:**
     - section 2, step 1's last condition, with its rejection of the name `BulletListener`;
     - its Off step 6: the skip flag now reads as `BulletListener`. An entry with it clear passes step 6 instead of
       refusing, and one with it set is skipped. UNVERIFIED, as section 4.
   - **The placement row of NATIVE_INDEX.md:** its two unread component conditions are now read here.
   - **The "does not fit its use" objection** (NATIVE_SPAWN_EXTENT.md):
     - a bullet listener is a primitive that notices bullets passing, not one that blocks;
     - leaving it out of ordinary collision queries, and out of the extent box, fits that;
     - the sweep walker's exception says the same: a query that sets bit 30, a dedicated bullet query, is the one that
       includes these primitives.
7. **Link 9's case.** `WillowPlayerController_0`'s cylinder fails step 2 (and step 1), so it is not tested. The first
   box test is then Clear in the `NoLevel` world, and the spawn goes on to make the actor. The next stop is the
   post-spawn encroachment check, already a named `NotYet` (NATIVE_SPAWN_PLACEMENT.md section 6), terminal and
   prefixed because the actor was made. It is reached only when the live `WillowAIPawn` has `bCollideActors` set after
   `PreBeginPlay`.
   - This assumes no other live primitive passes the tests above. The walk above, over `WorldInfo`,
     `WillowPlayerPawn_0` and the controller, finds none.

## Open questions

1. The hash admission's scene condition, and removal from the hash at detach or on a flag change.
2. The skip flag's name, `BulletListener`, inferred from the field layout (section 4).
3. Whether the live `WillowPlayerController` at the time of the cast has `bCollideActors` and its cylinder's
   `CollideActors` clear, as its class defaults have (script could set them).
4. The terrain bit (8192) beyond entering the actor hash: unchanged from NATIVE_SPAWN_PLACEMENT.md.

## Live checks that would confirm these rules (bl-sdk, the RTSE way)

1. **The controller's flags.** In Sanctuary, before and right after casting Phaselock, read on Maya's
   `WillowPlayerController`: `bCollideActors`, `bBlockActors`, `bWorldGeometry`, `CollisionComponent`; and on its
   `CollisionCylinder`: `CollideActors`, `BlockZeroExtent`, `BlockNonZeroExtent`, `BulletListener`, `bAttached`.
   Expect the collision flags false, as the class defaults are.
2. **The per-component test.** Spawn a test actor (with `bCollideWorld`) at a point inside a placed actor's blocking
   primitive. First leave that actor's `bCollideActors` set, then turn it off with `SetCollision`. Expect a moved
   location (or None) in the first case and the requested location in the second, so the owner flag decides it. Read
   the primitive's `bAttached` around the `SetCollision` call too, since it may reattach the component.
3. **The skip flag.** The same, with a primitive whose `BulletListener` is set. Expect it to be ignored by the spawn's
   search.
4. **The cast itself.** Hook the spawn of the Phaselock actor and read its final location against the requested one.
   Expect them equal when nothing but controllers and pawns stand there.
