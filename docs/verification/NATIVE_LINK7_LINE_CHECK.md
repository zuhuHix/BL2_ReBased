# Link 7's line check: the HUD targeting's trace, what it tests and what it passes over (2026-11-26)

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-LINK7-LINE-CHECK (decision 2026-11-26-71), the analyst half of the
clean room: **another lane implements this note.** Raw output stays under ignored `local/analysis/lc/` and
`local/analysis/hud/`. This note is in our own words: no decompiler output, pseudo-code, addresses, offsets or listing
structure. Values are decimal. **Every rule here is UNVERIFIED in the running game.**

**Why.** With #810 (lane D), link 7 passes `make` and `touch`, and `look` stops at the line check of hub ruling 4
(NATIVE_LINK7_HUD_TARGETING.md section 8): under `NoLevelLineCheck` the check passes over only the owner's pawn, the
target pickup `W` and `W`'s `Base`, and any other actor holding collision is `NotRead` unless its bounds miss the line's
box. The decoy `WillowPickup`'s collision is its `SkeletalMeshComponent`, whose bounding-cylinder box,
(0, 45, 0) ± (118.5, 118.5, 32.7), meets the line's box from (127.6, 0, 11.2) to (0, 0, 75). This note reads what the
game's check is, so the stand-in can follow the game's rule rather than a list.

## 1. Evidence

- **bl-sdk live reads:** none. No session was granted.
- **The natives:** Ghidra 12.1.4, headless and read-only, under the machine lock. Read:
  - the line-check call in `WillowHUD`'s per-tick pickup targeting pass (NATIVE_PICKUP_TARGETING.md section 5): its
    segment, extent, flags and source actor;
  - the routine it goes through: the world's single-line routine, the one `Actor.Trace`, the touch trace
    (NATIVE_TOUCH_PICKUP_TRACE.md section 2) and the base search (NATIVE_FLOOR_TRACE_FLAGS.md section 2) use, and its
    zero-extent hash walker;
  - the per-actor trace filter slot of `WillowPickup`'s primary table;
  - the line test of `SkeletalMeshComponent` (its primitive class virtual), outlined.
- **Field places:** the local copy of `tools/ghidra/class_layout.py` (`Engine.HUD.PlayerOwner`, `HUDDefinition`,
  `SkeletalMesh`).
- **`ow-package --def-dump`:** `Default__WillowPickup`, its `CollisionCylinder`, and `Default__DroppedPickup`.
- **No probe.** The rule below decides the slice's case by reading alone.

## 2. The check

**One world line check through the single-line routine:**
- **the segment:** **from `V`** (the view point) **to a point 25 units short of `W`** along the direction from `V` to
  `W` (pitch and yaw). By `Actor.Trace`'s argument order (NATIVE_TOUCH_PICKUP_TRACE.md section 2) this is the
  direction: it corrects NATIVE_PICKUP_TARGETING.md section 5, step 3, which has it the other way. Under the rule below
  the direction changes nothing;
- **the extent: zero**, a point line. So a primitive needs `BlockZeroExtent`;
- **the flags: 134357638**, which is bits 2 (movers), 4 (the level's BSP), 128 (world-geometry actors), 512 (stop at the
  first hit), 8192 (terrain), 131072 (complex collision) and 134217728 (NATIVE_TOUCH_PICKUP_TRACE.md section 3). The
  routine adds 1024 (one result). It is the touch trace's word (134226566) plus bit 131072. **Bits 1 (pawns), 8
  (volumes), 16 (other actors), 32 and 64 are not set**;
- **the source actor: the HUD** (the pass's receiver);
- **the answer:** the pass keeps `W` when the check hits nothing, or hits `W` or `W`'s `Base`; any other hit actor
  means no winner (NATIVE_PICKUP_TARGETING.md section 5).

## 3. What it tests

The routine runs as NATIVE_SPAWN_PLACEMENT.md section 7 and NATIVE_TOUCH_PICKUP_TRACE.md section 2 read it:
1. **the BSP** (bit 4). A hit names `WorldInfo` and, with bit 512, ends the query;
2. **the actor collision hash**, with the zero-extent walker. For one primitive (NATIVE_SPAWN_TRACE_FILTER.md section
   3, with NATIVE_FLOOR_TRACE_FLAGS.md section 4 for the source):
   - met before, no owner, or an owner that is the source (the HUD): passed over;
   - the skip flag (`BulletListener`): passed over;
   - `BlockZeroExtent` clear: passed over;
   - the per-component test (`CollideActors`, and the owner's `bCollideActors`) fails: passed over;
   - bounds that miss the line: passed over;
   - the owner in the HUD's `Owner` chain (its controller), or the HUD in the owner's: passed over;
   - the owner's per-class filter (below);
   - the primitive's own line test (with complex collision asked for).

**The owner filters with these flags** (bits 1, 8, 16, 32 and 64 clear):

| Owner actor | Tested by the HUD's line check? |
|---|---|
| `Actor` rule (`Actor`, `StaticMeshActor`, `Brush`, `Terrain`) with `bWorldGeometry` | yes (bit 128) |
| `Actor` rule without `bWorldGeometry` | **no** (needs bit 16) |
| exactly `DroppedPickup` or `WillowPickup` (both keep `Actor`'s body; `WillowPickup`'s table re-read here) | as the `Actor` rule: **no** unless `bWorldGeometry` (then yes, bit 128) |
| a subclass of either | its own filter, not read: `NotRead`, "the trace filter of <class>" |
| `Volume` with `bWorldGeometry`, not `bPawnsOnly` | yes (bit 128) |
| `Volume` with `bPawnsOnly` | the source's volume test, not read |
| `Volume` without either | no (needs bit 8) |
| `InterpActor`, `KActor` | yes (bit 2; no bit 32 or 64, so no narrowing) |
| exactly `Pawn` | **no** (needs bit 1, or bit 16 with `bStationary`) |
| exactly `WillowPawn`, `WillowAIPawn` or `WillowPlayerPawn` (one `WillowPawn` body, NATIVE_FLOOR_TRACE_FLAGS.md section 3) | **no**: its `MeshComponent` and `CylinderComponent` steps only pass a primitive over, and it then defers to the `Pawn` row |
| another `Pawn` subclass | its own filter, not read: `NotRead`, "the trace filter of <class>" (review r1 of #814) |
| any other class | its own filter, not read: `NotRead`, "the trace filter of <class>" |

**So the slice's pickups and pawns never block it**, by the class bodies read (review r1 of #814): an exact
`WillowPickup` or `DroppedPickup` with `bWorldGeometry` false (the class default) is passed over by the owner filter
**whatever its components or bounds**, and so is an exact `Pawn`, `WillowPawn`, `WillowAIPawn` or `WillowPlayerPawn`. A
pickup with `bWorldGeometry` set is tested (bit 128), and a subclass whose filter was not read is `NotRead`. What can
block it is the level, world geometry, movers and terrain: walls between the eye and the gun.

## 4. The pickups' components, and the skeletal mesh's own test

- **`Default__WillowPickup`:** `bCollideActors` true, `bBlockActors` false, `bWorldGeometry` false; its
  `CollisionComponent` is its `CollisionCylinder` (radius 64, height 20, `CollideActors`, `BlockZeroExtent` and
  `BlockNonZeroExtent` true, `BlockActors` false, not a `BulletListener`). The mesh is not in the defaults; the port's
  decoy holds a `SkeletalMeshComponent` as its collision after its set-up.
- So a pickup's primitives would pass the walker's flag steps, but the owner filter rejects them first (section 3).
- **The skeletal mesh's line test** (by fit, outlined; reached only for a tested owner): the mesh asset's
  `bUseSimpleLineCollision` (for a zero extent; `bUseSimpleBoxCollision` for a non-zero one) selects a test against the
  physics asset's bodies; otherwise, or with complex collision asked for and `PerPolyCollisionBones` set, the listed
  bones are tested per polygon. **It is not a bounds test**: bounds are only the walker's pre-filter. Not read further.

## 5. Proposed Off answer (for a separate implementer)

**Hub ruling (decision 2026-11-26-71): yes.** This section's rule replaces ruling 4's
three-actor list, under the same `NoLevelLineCheck` setting and only with a service that holds no geometry. Off only,
logged, UNVERIFIED, Reference unchanged; a service holding geometry keeps the existing refusal.

Replacing ruling 4's list (NATIVE_LINK7_HUD_TARGETING.md section 8) with the game's rule, under the same
`pickup_targeting::NoLevelLineCheck` setting and only with a collision service that holds no geometry. A service
holding geometry keeps the line check's existing refusal. Off only, logged, UNVERIFIED, Reference unchanged.
1. **The query** (review r1 of #814): a point line **starting at `V`** and **ending at the point 25 short of `W`**,
   the flags above, and **the real HUD as source** (the owner's `myHUD`, the pass's receiver). The current call passes
   the pawn and the winner, with the endpoints the other way round; both change. It is `trace_filter`'s point line
   with a source actor (the touch trace's, `Query::PointLine`), with bit 131072 added (it only reaches the primitive's
   own test). The source exclusion reads the **`Owner` chain both ways** (NATIVE_FLOOR_TRACE_FLAGS.md section 4), not
   the touch helper's one-way `Base` walk.
1a. **The candidates** (review r1 of #814): every live actor's primitives, **its `CollisionComponent` and every
   primitive in its `Components`**, as the touch walk enumerates them; each primitive's owner resolved by
   `trace_filter::owner_of` (the component's own `Owner` when set, else the walked actor), and that owner's flags and
   filter row read, not the walked actor's. A mover whose only blocking primitive is in `Components` is a candidate,
   not cleared.
2. **The level part:** none (the service holds no geometry).
3. **Passed over by rule, reading no bounds:** the source and its `Owner` chains (the HUD, its controller); every
   `Pawn`; every `Actor`-rule owner without `bWorldGeometry`, `DroppedPickup` and `WillowPickup` included; every
   `Volume` without `bWorldGeometry` that is not `bPawnsOnly`; and the walker's flag steps (skip flag,
   `BlockZeroExtent`, the per-component test).
4. **Passed over by geometry:** a primitive whose bounds are disjoint from the line's box (the existing bounds step).
   Only well-formed values clear (review r1 of #814): both endpoints and the bounds centre finite; each box's limits
   finite and ordered (minimum ≤ maximum on every axis); the extents (radius, half-height) finite and not negative.
   Anything malformed is `NotRead`, "the pickup targeting's line check: a malformed endpoint or bounds" (a NaN endpoint
   would otherwise collapse an axis and fake a miss).
4a. **Loads and declarations** (review r1 of #814): every read is of resident objects only, through lookup-only names.
   An unloaded object that would decide a candidate (the primitive, its owner, a link of either `Owner` chain) is a
   named `NotRead`; an unloaded `Owner` link is neither loaded nor taken as None. A field the class does not declare,
   or declares with another kind, is `VmError::Script`.
5. **`W` and `W`'s `Base`:** a hit on either keeps `W`. So when every remaining candidate is `W` or `W`'s `Base`, the
   answer is "keep `W`" whatever their own line tests say.
6. **Any other remaining candidate:** `NotRead`, naming the first unread rule on its path, decided before any write.
   In `trace_filter`'s order (review r1 of #814):
   - a `Terrain` owner: `TERRAIN_HANDLING`, ahead of every other test;
   - an owner class with no read row: "the trace filter of <class>";
   - a `bPawnsOnly` volume: `SOURCE_VOLUME_TEST`;
   - a primitive every read test accepts (a world-geometry actor or volume, **a mover**): `HASH_ADMISSION`, which
     refuses before the primitive's own line test. **So an intersecting mover gets `HASH_ADMISSION`**; only with the
     hash's admission established would it reach `PRIMITIVE_LINE_TEST`.
7. **Otherwise:** no hit, `W` kept, with the per-call log line.

**The two other service cases** (review r1 of #814): the setting on and a service **holding geometry**:
`LINE_GEOMETRY`'s `NotRead`; the setting off, or **no collision service**: the headless `LayerAbsent`.

**Where it lands** (main f80b68e7, #810): `ow-inventory` `hud_harness.rs`, `HarnessHost::line_blocker_for`, and its
caller in `pickup_targeting.rs` (which passes the endpoints the other way round). Its skip of the pawn, the target and
the target's `Base` becomes steps 3 and 5; its walk of `CollisionComponent` alone becomes step 1a's walk; its
`not_cleared` bounds test stays as step 4 with step 4's checks; `LINE_GEOMETRY` stays for a service holding geometry;
`LINE_ACTOR` becomes step 6's refusals, named by the rule. The owner rows are `trace_filter`'s, as the touch trace uses
them, with the HUD as the source.

**On the slice:** the decoy and `W` are `WillowPickup`s (passed over, step 3), Maya is a pawn (step 3), the HUD and
its controller are the source's chain (step 3). Nothing remains, so `look` keeps the aimed gun: the answer the probe
of NATIVE_LINK7_HUD_TARGETING.md section 6 got from its stand-in, now by the game's rule.

## 6. Tests (synthetic fixtures)

1. A toy HUD and controller, a toy `WillowPickup` `W`, and a second toy `WillowPickup` whose collision component (a
   cylinder, or a skeletal mesh) has `BlockZeroExtent` and bounds that meet the line: no hit, `W` kept.
2. The same second pickup with `bWorldGeometry` true: tested (bit 128), so `HASH_ADMISSION`'s `NotRead`.
3. A toy pawn on the line with collision: passed over (no bit 1); a pawn with `bStationary`: still passed over (no
   bit 16).
4. A toy `InterpActor` on the line: `HASH_ADMISSION`'s `NotRead`; with bounds disjoint from the line's box: passed
   over; **its only blocking primitive in `Components`** (its `CollisionComponent` None): still `HASH_ADMISSION`, not
   cleared (review r1 of #814).
4a. (review r1 of #814) A NaN endpoint, a NaN bounds centre, a negative radius: each `NotRead`, nothing cleared; an
   unloaded `Owner` link that decides a candidate: `NotRead`, nothing loaded; a mistyped `bWorldGeometry`:
   `VmError::Script`; a toy `Terrain` on the line: `TERRAIN_HANDLING`; a toy subclass of `WillowPickup` or of `Pawn`:
   "the trace filter of <class>"; the line from `V` to the point 25 short of `W`, with the toy HUD as source.
5. A world-geometry `StaticMeshActor` that is `W`'s `Base`, on the line: `W` kept; the same actor not `W`'s `Base`:
   `NotRead`.
6. The HUD's own controller holding a cylinder on the line: passed over (the source's `Owner` chain).
7. A `bPawnsOnly` volume on the line: `NotRead` (the source's volume test).
8. A service holding geometry with the setting on: `LINE_GEOMETRY`'s `NotRead`; the setting off, or no service: the
   headless `LayerAbsent`.
9. Each refusal before any write, with #518's three counters and the class-code load counter unchanged (review r1 of
   #814); Reference: unchanged.

## Open questions

1. **UNVERIFIED:** every rule above.
2. **Ruled by the hub:** section 5's rule replaces ruling 4's three-actor list, under the same setting, only with a
   service that holds no geometry (Off only, logged, UNVERIFIED, Reference unchanged).
3. **Not read:** the skeletal mesh's and the cylinder's own line tests (section 4, outlined); the source's volume
   test; bit 134217728's game class (NATIVE_TOUCH_PICKUP_TRACE.md section 3).
4. **By fit:** the skeletal mesh's choice between the physics asset and the per-polygon bones.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. Drop two guns in a line from the eye, the nearer one in front of the aimed one, and read the HUD's seen pickup:
   the aimed one (pickups do not block the check).
2. Stand an enemy between the eye and a gun beyond a quarter of `PickupRange`: the gun is still reported.
3. Put a wall between: the gun is not reported.
