# `DroppedPickup.TouchPickupTrace`: what blocks a pickup's touch

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-LINKS-6-7-PAST-TOUCH (decision 2026-11-19-NNN), the analyst half of
the clean room. This lane read the native in `Borderlands2.exe` locally. Ghidra 12.1.4 ran read-only through
`tools/ghidra/run.ps1`, with a local copy of a throwaway constructor-slot helper for the table lookup. The raw output
stays under ignored `local/analysis/touch_pickup_trace/`. No bl-sdk session was granted, so **every rule here is
UNVERIFIED in the running game.** Section 7 gives the live check that would confirm it.

**Why now.** Slice links 6 and 7 pass `make`. They then stop in `touch` at `Actor.GetBoundingCylinder`, which #595
ports and refuses on the live pickup ("the colliding components' bounds"). A throwaway probe used a local stand-in for
that refusal: main b0d9ca08 plus #595, #609 and #617, never pushed. With it, both links stop next at this native, still
in `touch`. The port's Off stub refuses it today: `ow_inventory::pickup`'s `TOUCH_PICKUP_TRACE`, NATIVE_PICKUP.md
section 3.

## 1. Where it sits

- **The caller** (NATIVE_PICKUP.md section 3): the pickup's touch state asks
  `TouchPickupTrace(StartLoc, EndLoc)` before telling the toucher's controller about the touch. `StartLoc` is the
  toucher's location. `EndLoc` is the pickup's touch trace point: its location raised by half its bounding cylinder's
  height, which is where `GetBoundingCylinder` comes in. When the answer is false, the script checks the touch again
  after 0.5 seconds.
- **The native.** The exec function reads the two vectors in declaration order and asks one virtual of the receiver,
  then returns that virtual's answer as the bool result. `DroppedPickup` installs its own implementation in that slot.
  `WillowPickup`'s table has the same implementation, so `WillowPickup` keeps `DroppedPickup`'s rule. No other
  subclass's table was read.

## 2. The rule

The implementation is one line check through the world's single-line routine. That is the routine `Actor.Trace`,
`Actor.FastTrace` and the visible-actor iterators also use.
1. **The segment.** The line runs **from `EndLoc` (the pickup's touch point) to `StartLoc` (the toucher)**. The
   evidence is `Actor.Trace`, which passes its `TraceEnd` and then its `TraceStart` in the same two positions of the
   same routine; an omitted `TraceStart` defaults to the actor's location. `TouchPickupTrace` passes `StartLoc` in the
   end position and `EndLoc` in the start position. This **corrects** NATIVE_PICKUP.md section 3's "direction
   resolved": the parameter names are the script's, and the trace runs the other way. For the answer below the
   direction changes nothing that was read: a level hit ends the query either way, and with no level hit the actors
   are walked over the whole segment. Whether the level's own line test is one-sided was not read.
2. **The shape and the source.** The extent is zero, so it is a point line. The receiver (the pickup) is the query's
   **source actor**.
3. **The flags** (decimal): **134226566**, which is
   - bit 2 (movers);
   - bit 4 (the level's BSP);
   - bit 128 (world-geometry actors);
   - bit 512 (stop at the first hit);
   - bit 8192 (terrain);
   - bit 134217728, a bit no earlier record reads (section 3).

   The single-line routine adds bit 1024 (one result). Bit meanings are NATIVE_SPAWN_PLACEMENT.md section 7's table.
   The set is that note's box-test set 8902 without bit 64 (blocking only), plus bit 134217728.
4. **The routine** is the multi-hit routine NATIVE_SPAWN_PLACEMENT.md section 7 reads ("Which hit a sweep returns"),
   with the flags above:
   - **The level's BSP first** (bit 4). A hit names the world's `WorldInfo` as its actor; it is the same getter
     `Engine.GetCurrentWorldInfo` uses. With bit 512 set, a BSP hit ends the query, and no actor is walked.
   - **Then the actor collision hash**, walked because bits 2, 128 and 8192 are in the walk mask. Each candidate
     primitive passes the walker's tests in NATIVE_SPAWN_TRACE_FILTER.md, for a zero-extent query
     (`BlockZeroExtent`).
     - That walker passes over a primitive whose owner is the source actor or in its base chain. **The pickup's own
       primitives never block its touch.**
     - Then comes the owner's per-class filter, as in NATIVE_SPAWN_PLACEMENT.md section 7, step 3, now with a source
       actor.
   - **With no hit,** the routine reports a fraction of 1 and no actor. With a hit, it reports the first hit and its
     actor.
5. **The answer:** true (a clear touch) exactly when the reported hit names **no actor**. A BSP hit names `WorldInfo`
   and an actor hit names its actor, so any accepted hit makes the answer false.
6. **Effects:** none. The hit record and the trace's scratch memory are local to the call. Nothing in the world is
   written, nothing is loaded, and nothing is drawn.

Applied to the classes whose filters are read (NATIVE_SPAWN_PLACEMENT.md section 7), for a candidate primitive that
blocks a zero-extent query:

| Owner actor | Tested by the touch trace? |
|---|---|
| `Actor` rule (`Actor`, `StaticMeshActor`, `Brush`, `Terrain`) with `bWorldGeometry` | yes (bit 128) |
| `Actor` rule without `bWorldGeometry` | no (needs bit 16) |
| `Volume` with `bWorldGeometry`, not `bPawnsOnly` | yes (bit 128) |
| `Volume` with `bPawnsOnly` | the source actor's test, not read |
| `Volume` without either | no (needs bit 8) |
| `InterpActor`, `KActor` | yes (bit 2; no bit 32 or 64, so no narrowing) |
| `Pawn`, the toucher included | no (needs bit 1, or bit 16 with `bStationary`) |
| any other class | its own filter (section 3) |

So walls, floors and world-geometry meshes block a touch, and so do movers. Pawns, the toucher included, never block
it, and neither does the pickup itself.

## 3. Bit 134217728 and the game's filters

- A whole-program scan for a test of that bit found it on a trace-flags argument in **one** place. The scan covered
  `TEST` with that constant; the `AND` and bit-test forms were not scanned. That place is one game class's override of
  the `Actor` filter. With the bit set, the override rejects the primitive outright. Without it, the override rejects
  when the source actor's class derives from one fixed game class, and otherwise defers to the `Actor` rule.
  - Which class owns that override was not identified: the table that holds it was not found.
  - The class its source-actor test names was not identified either.
  - Read as behaviour: **actors of that class never block a touch.**
- Six more game-class overrides of the `Actor` filter call it in the same way, each first testing other high bits
  (16777216, 33554432, 67108864, 268435456, 536870912, 4194304, 256), and one also reads its own fields first. None of
  those bits is in the touch's set. Their classes were not identified either.
- The filters NATIVE_SPAWN_PLACEMENT.md section 7 reads (`Actor`, `Volume`, `InterpActor`, `KActor`, `Pawn`) do not
  test bit 134217728: the `Actor` rule's own body was re-read here, and that note's table names no such bit for the
  others.
- The level's BSP line test was not in the scan's hits. Its callees were not each read.

None of this changes the port's policy, which already holds: **a candidate whose owner's class has no read filter is
unanswered, naming "the trace filter of <class>"** (NATIVE_SPAWN_PLACEMENT.md section 7). Every game class falls
under it.

## 4. The proposed Off answer

The answer needs a collision service and the existing actor walk (`ow_world::placement` and `ow_world::trace_filter`).
It adds a third query kind next to the box test and the sweep: a **point line with a source actor**. In this order,
each refusal before any write; the native writes nothing anyway.
1. **No receiver:** `VmError::Script`.
2. **The receiver's exact class** is neither `Engine.DroppedPickup` nor `WillowGame.WillowPickup`: a `NotRead` `NotYet`,
   "the touch trace of <class>". Another subclass's table was not read. Look the classes up through the class table's
   `by_path` only; a class absent from the table is "not this class".
3. **The arguments:** two `Vector` structs. Any other shape is `VmError::Script`.
4. **The receiver is an unloaded handle:** a `NotRead` `NotYet`, not loaded. The walk needs the receiver's base chain.
5. **No collision service:** a `LayerAbsent` `NotYet`, "the pickup touch trace". This is the same layer the spawn
   placement refuses on, never "clear".
6. **A zero-length segment** (`StartLoc` equal to `EndLoc`): a `NotRead` `NotYet`, "a zero-length touch trace". The
   routines' handling of it was not read.
7. **The level part:** the service's `point_sweep` from `EndLoc` to `StartLoc`.
   - A hit answers **false**: blocked, and no actor is read (bit 512).
   - `Unsupported` is a `NotRead` `NotYet` naming the level part.
   - `Clear` goes on to the actor walk.
8. **The actor walk** over the live actors, the whole segment's box as the region, zero extent, with the flags of
   section 2 and the pickup as source actor. It follows `trace_filter`'s candidate steps, adding one step: a primitive
   whose owner is the pickup, or in the pickup's `Base` chain, is not tested (NATIVE_SPAWN_TRACE_FILTER.md). The
   owner rows are those of section 2's table; a `bPawnsOnly` volume is a `NotRead` `NotYet`, "the source actor's
   volume test". Every unread point keeps its existing named refusal: the skip flag, terrain handling, hash admission,
   and an owner class with no row. A primitive the filters **test** cannot be answered either, since the port has no
   primitive line test: a `NotRead` `NotYet` naming the primitive's line test.
9. **Otherwise** (the level is clear and every candidate is "not tested"): `Bool(true)`.

Reference keeps the C++ stub policy and binds nothing.

**What the slice gets.** Links 6 and 7 run on the equip run's world, which installs no collision service, so step 5
refuses there. A harness input, the hub's decision and UNVERIFIED, could install the no-level service slice link 9's
world uses. That service answers every level part Clear and holds no actor bounds, so the walk then decides the
answer.

## 5. Tests

These use synthetic classes and an invented service, with `package()` and `tag()` helpers where a package is needed.
Each checks the answer and that #518's three counters (heap committed writes, generator draws, load transitions) are
unchanged:
1. no receiver (`Script`); an `Engine.Actor` receiver and a `WillowPickup` subclass (`NotRead`, naming the class); a
   non-vector argument (`Script`);
2. an unloaded `WillowPickup` handle: `NotRead`, and still unloaded;
3. no service: `LayerAbsent`;
4. a zero-length segment: `NotRead`;
5. a service whose `point_sweep` hits: false. The service records that it was asked from `EndLoc` to `StartLoc`, and
   no actor field is read;
6. a service that answers `Unsupported`: `NotRead` naming the level part;
7. a clear level with no live primitive: true;
8. a clear level with only the toucher pawn's primitive, the pickup's own primitive, and a primitive of an actor in
   the pickup's `Base` chain: true, since none is tested;
9. a clear level with a non-world-geometry `Actor`'s primitive and a `Volume` without `bWorldGeometry`: true;
10. a clear level with a world-geometry `Actor`'s primitive, a mover's primitive, or a `bPawnsOnly` volume: `NotRead`
    (the primitive's line test, or the volume test);
11. a clear level with a primitive of an invented game class with no row: `NotRead`, "the trace filter of <class>";
12. a primitive with `BlockZeroExtent` clear: not tested;
13. Reference: unbound (the stub policy).

## 6. NATIVE_INDEX row

| Native | Note | Slice use | Status | Confidence | Implemented |
|---|---|---|---|---|---|
| DroppedPickup.TouchPickupTrace (the touch's line check: segment, flags, source actor, answer) | [NATIVE_TOUCH_PICKUP_TRACE.md](NATIVE_TOUCH_PICKUP_TRACE.md) | links 6 and 7: `touch`, after `GetBoundingCylinder` | UNVERIFIED | high (segment order, flags, answer, no effects), medium (the game classes' filters, bit 134217728's owner) | |

## 7. Open items and the live check

- **Open:** which game class owns the bit-134217728 override, and which class its source-actor test names; the six
  other game-class overrides; the level line test's one-sidedness and zero-length handling.
- **Live check (bl-sdk, the RTSE way, when a session is granted).** Hook `DroppedPickup.TouchPickupTrace` and log its
  two arguments and its result for:
  - a gun dropped in the open: expect true;
  - the same gun with a wall between the pickup's touch point and the toucher: expect false;
  - with another pawn standing between: expect true.
  Then compare `EndLoc` with the pickup's location plus half its cylinder height.
