# The base search's floor trace: its flags, what it can hit, and its source actor (2026-11-26)

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-FLOOR-TRACE-FLAGS (queue 0ar, decision 2026-11-26-NNN), the analyst
half of the clean room: **another lane implements this note.** Raw output stays under ignored `local/analysis/floor/`.
This note is in our own words: no decompiler output, pseudo-code, addresses, offsets or listing structure. Values are
decimal. **Every rule here is UNVERIFIED in the running game.**

**Why.** NATIVE_LINK34_AFTER_MIND.md section 2 read the base search (the `Actor` step of `SetPhysics` into
`PHYS_None`, `PHYS_Walking`, `PHYS_Rotating` or `PHYS_Spider`) as an 8-unit floor trace whose flags were not decoded.
Two hub stand-ins rest on that gap:
- #798's R2: the stand-in skips the actor's own collision, "on the common convention that an engine trace ignores its
  source";
- lane E's geometric clearance (IMPL-COMBAT-WORLD-COLLISION, `set_physics::clear_of`): another actor's cylinder is
  passed over when the downward line runs wholly outside it, "whatever the trace's flags".

This note reads the flags, what kinds of objects the trace can hit, and how the source actor is treated, and checks
both stand-ins against them.

## 1. Evidence

- **bl-sdk live reads:** none. No session was granted.
- **The script:** not involved; the base search is native.
- **The natives:** Ghidra 12.1.4, headless and read-only, under the machine lock. Read:
  - the floor query (the actor virtual the base search calls), in the primary tables of `Actor`, `Pawn`,
    `WillowPawn`, `WillowAIPawn` and `WillowPlayerPawn`: one shared body;
  - the extent it passes, and the bounding-cylinder virtual it falls back on (`Actor`'s body, and the pawn tables'
    shared one);
  - the world's single-line routine and its multi-hit routine (the ones `Actor.Trace` uses: NATIVE_ACTOR_TRACE.md,
    NATIVE_TOUCH_PICKUP_TRACE.md section 2);
  - the collision hash's two line walkers (zero extent, and with an extent);
  - the per-actor trace filter of `Actor`, `Pawn` and the shared `WillowPawn` body (also `WillowAIPawn`'s and
    `WillowPlayerPawn`'s);
  - the source actor's blocked-by test that the filters' blocking-only bit calls, and the two helpers it uses (the
    ignore-blocking test and the encroacher test), in the same five tables;
  - the class virtual that tells a cylinder component, in the `PrimitiveComponent` and `CylinderComponent` tables.
- **Field places:** the local copy of `tools/ghidra/class_layout.py`. The `Actor`, `Pawn` and `PrimitiveComponent`
  fields read here land exactly; `WillowPawn`'s and the two subclasses' are named by fit where noted.
- **No probe:** nothing on main reaches the trace with these rules yet (section 7).

## 2. The query

**One world line check through the single-line routine**, the routine `Actor.Trace`, `Actor.FastTrace` and the
visible-actor iterators use:
- **the segment:** from the actor's `Location`, offset by its `CollisionComponent`'s translation when it has one (by
  fit, as section 2 of NATIVE_LINK34_AFTER_MIND.md already says), straight down by 8 units;
- **the extent: the actor's collision cylinder, not zero.** This corrects NATIVE_LINK34_AFTER_MIND.md section 2's
  "single line trace". The query is a box swept down by 8 units:
  - when the `CollisionComponent` is a `CylinderComponent` (its class virtual answers so; the base primitive's answers
    no): the half-extent is (`CollisionRadius`, `CollisionRadius`, `CollisionHeight`). This also confirms the fit of
    the two cylinder fields in NATIVE_PAWN_INTERACTION_PROXY.md section 1: the field used for both X and Y is the
    radius;
  - otherwise: the actor's bounding-cylinder virtual. On the pawn tables it answers the pawn's `CylinderComponent`
    when that differs from the `CollisionComponent` and is a cylinder; else (and on `Actor`) a cylinder fitted to the
    components' bounding box (radius from the box's horizontal half-diagonal, height its vertical half-size);
- **the flags:** **8415**, which is bits 1 (pawns), 2 (movers), 4 (the level's BSP), 8 (volumes), 16 (other actors), 64
  (blocking only), 128 (world-geometry actors) and 8192 (terrain). The single-line routine adds 1024 (one result).
  Bit 512 (stop at the first hit) is **not** set. Bit names are NATIVE_SPAWN_PLACEMENT.md section 7's table. This is
  the same word NATIVE_BOUNDING_CYLINDER_AND_AFTER.md section 3 found on `TraceComponent`'s line check;
- **the source actor:** the actor itself;
- **the answer:** the hit's actor and normal, or no actor. The base search then compares that actor with `Base`
  (NATIVE_LINK34_AFTER_MIND.md section 2, step 2).

## 3. What the trace can hit

The multi-hit routine runs as NATIVE_SPAWN_PLACEMENT.md section 7 reads it ("Who is tested" and "Which hit a sweep
returns"), with these flags:
1. **The level's BSP** (bit 4), swept with the extent. A BSP hit names the world's `WorldInfo` as its actor
   (NATIVE_TOUCH_PICKUP_TRACE.md section 2). Without bit 512 it does not end the query: the actors are then walked up
   to the hit plus 5 units.
2. **The actor collision hash** (the walk mask holds bits 1, 2, 8, 16, 128 and 8192), with the extent walker. For one
   candidate primitive, in order (NATIVE_SPAWN_TRACE_FILTER.md section 3, with section 4 below):
   - met before in this query, no owner, or **an owner that is the source actor**: passed over;
   - the skip flag (`BulletListener`), unless bit 30: passed over;
   - `BlockNonZeroExtent` clear (the extent is not zero): passed over;
   - the per-component test (`CollideActors`, and the owner's `bCollideActors`) fails: passed over;
   - **bounds disjoint from the swept box:** passed over;
   - **the owner chains** (section 4): passed over;
   - the owner's per-class filter (below);
   - the primitive's own swept-box test.
3. **One result** (bit 1024): the nearest, with the comparator between a BSP hit and an actor hit not read
   (`trace_filter::BSP_ACTOR_COMPARATOR`).

**The owner filters with these flags** (bit 32 is clear, bit 64 is set, and there is a source actor):

| Owner actor | The floor trace tests it when |
|---|---|
| `Actor` rule (`Actor`, `StaticMeshActor`, `Brush`, `Terrain`) with `bWorldGeometry` | always (bit 128), no blocking test |
| `Actor` rule without `bWorldGeometry` | the source's blocked-by test passes (bit 16, then bit 64) |
| `Volume` | as NATIVE_SPAWN_PLACEMENT.md section 7 (bit 8, then the bit 64 narrowing; `bPawnsOnly`: the source's volume test, not read) |
| `InterpActor`, `KActor` | the source's blocked-by test passes (bit 2, then bit 64) |
| `Pawn` | not `bIgnoreAllTraces`, and the source's blocked-by test passes (bit 1, then bit 64) |
| `WillowPawn`, `WillowAIPawn`, `WillowPlayerPawn` | the `WillowPawn` step below, then the `Pawn` row |
| any other class | its own filter, not read |

**So pawns can be the floor.** A pawn under another pawn is tested, and blocks when the blocked-by test passes. World
geometry always can. Movers, volumes and other actors can, through the same test.

**The `WillowPawn` filter step** (one body for the three Willow pawn tables), before the `Pawn` row:
- a primitive that is a `MeshComponent` is passed over when the query sets bit 2147483648, or when the candidate's
  `bAllowProjectilesToCollideWithSkeletalMesh` (by fit) is clear and the source's `bTraceIgnoreRigidBodyForPawns` is
  set;
- a `CylinderComponent` primitive that is not a `BulletListener` is passed over when the query sets bit 67108864. The
  floor trace does not set it.

**The source's blocked-by test** (the source actor's virtual, given the candidate's owner and the primitive). `Actor`'s
body, which `Pawn` keeps:
1. a primitive whose `BlockActors` is clear: no;
2. the owner's `bIsBlockingMesh` (by fit) set: no;
3. the owner `bWorldGeometry`: yes when the source's `bCollideWorld` and the owner's `bBlockActors` are set;
4. the owner ignores the source, or the source ignores the owner (the ignore-blocking test below): no;
5. the owner is an encroacher (the brush test is a stub that answers no on all five tables): yes when the source's
   `bCollideWorld` and the owner's `bBlockActors` are set;
6. the source is an encroacher: yes when the owner's `bCollideWorld` and the source's `bBlockActors` are set;
7. otherwise: yes when both have `bBlockActors`.

- **The encroacher test:** `bCollideActors`, and `Physics` `PHYS_RigidBody` or `PHYS_Interpolating`, or
  `bCollideAsEncroacher`.
- **The ignore-blocking test:**
  - `Actor`: its own `bIgnoreEncroachers` and the other is an encroacher;
  - `Pawn`: also when its own `Physics` is `PHYS_RigidBody` and the other has `bIgnoreRigidBodyPawns`;
  - `WillowPawn` (also `WillowPlayerPawn`'s): first, an other with `bIsWillowInteractiveObject` whose interactive
    definition holds one flag (not named) is ignored;
  - `WillowAIPawn`: first, the other is ignored when it is the actor one of its own fields holds (inside `PerchState`
    by the layout tool, not named).
- **The Willow pawn overrides of the blocked-by test itself:**
  - `WillowPawn`: an owner with `bIsWillowInteractiveObject` whose interactive definition holds another flag blocks,
    unless one more condition holds (not read);
  - `WillowAIPawn`: an owner with `bIsBlockingMesh`, when a world list is non-empty, is decided by two bits of the
    primitive's own flag word (not named);
  - `WillowPlayerPawn`: an owner with `bIsBlockingMesh` is decided by one bit of the primitive's flag word (not named);
    an owner equal to its `TemporaryNonBlockedBy` (by fit) does not block;
  - each then falls back to the class above it, and finally to `Actor`'s body.

## 4. The source actor: what is passed over

Both line walkers (zero extent and with an extent) pass over a candidate primitive when:
1. its owner **is** the source actor (checked first, before its bounds); or
2. its owner is one of the source's **owners**: the walk follows the source's `Owner`, then that actor's `Owner`, and
   so on; or
3. the source is one of the owner's **owners**: the same walk from the owner.

**The chain is `Owner`, not `Base`.** The walk follows the actor field the layout tool places as `Actor.Owner`, and the
base search's own comparison uses the field after it for `Base`. This **corrects** NATIVE_SPAWN_TRACE_FILTER.md
section 3 ("in its base chain") and NATIVE_TOUCH_PICKUP_TRACE.md sections 2 and 4, step 8. The port's
`trace_filter::chain_of`, which the touch trace uses (`touch_trace.rs`), walks `Base` from the source only. Both
directions of the `Owner` walk apply there too.

**So #798's R2 holds, and is wider in the game.** The floor trace never hits the actor's own primitives, and it never
hits an actor in its owner chain or an actor it owns. For a possessed pawn, its `Owner` is its controller, so the
controller's collision never blocks its floor trace either.

## 5. Lane E's geometric clearance

`set_physics::clear_of` (IMPL-COMBAT-WORLD-COLLISION) passes over another actor's cylinder when the **line** from
`Location` down by 8 units runs more than R from the cylinder's axis, or wholly above or below it.
- **The order is right.** The extent walker tests bounds before the owner's flags or filter, so a primitive whose
  bounds miss the swept region is never tested, whatever the flags.
- **The region is too small.** The game sweeps the source's cylinder box (section 2), so its region is the box of
  half-extent (Rs, Rs, Hs) swept down by 8. A cylinder that `clear_of` clears can still meet that box: one standing
  within R + Rs of the line horizontally, or within Hs below or above it. For those the game tests the primitive,
  and the stand-in would wrongly answer "no floor".

**Proposed correction** (Off only, under the same `NoLevelBaseSearch` setting, UNVERIFIED): another actor's cylinder
is passed over only when its bounds box (the bounding-cylinder rule: centre C, half-extent (R, R, H)) is disjoint from
the swept box, with `S` the trace start:
- |S.x − C.x| > R + Rs, or |S.y − C.y| > R + Rs (box against box, so per axis, not a radius); or
- S.z − 8 − Hs > C.z + H (the swept box wholly above it); or
- S.z + Hs < C.z − H (wholly below it).

Touching counts as meeting. Rs and Hs come from section 2's extent, read with the same load-free reads; a source
whose extent is not the cylinder case (no cylinder `CollisionComponent`) keeps the `NotRead`. On the combat run the
spawns stand 1000 units apart on X, far beyond R + Rs, so the corrected test clears them as `clear_of` does.

## 6. Proposed Off answer (for a separate implementer)

In `Actor.SetPhysics`' preflight, replacing `BASE_TRACE_FLAGS`' "its trace flags are not decoded". All decided before
the mode write, so nothing carries the partial-writes prefix.
1. **No collision service:** `LayerAbsent`, as today.
2. **The extent:** the cylinder case of section 2, else `NotRead`, "Actor.SetPhysics: the base search's extent (the
   bounding-cylinder fallback)". Declared, else `VmError::Script`: `CollisionComponent`, `CollisionRadius`,
   `CollisionHeight`, `Location`.
3. **The level part:** the service's swept box (`box_sweep` with the extent) from the start down by 8.
   - `Unsupported`: `NotRead`, "Actor.SetPhysics: the base search's level sweep";
   - `Hit`: the floor is `WorldInfo`, unless an actor candidate lies within the hit plus 5 units, which needs the
     unread comparator: `NotRead` naming `trace_filter::BSP_ACTOR_COMPARATOR`. A `WorldInfo` floor that differs from
     `Base` reaches the base setter, which keeps `BASE_SETTER`'s `NotRead` for classes outside NATIVE_ACTOR_SETBASE.md's
     list;
   - `Clear`: go on.
4. **The actor part**, over every live actor's primitives, in the placement walk's order:
   - passed over without reading any filter field: the owner is the source; the owner is in the source's `Owner`
     chain or the source is in the owner's (section 4; each chain read load-free, bounded by the links seen; an
     unloaded link that decides it is `NotRead`, "a source Owner-chain link the store has not loaded"); bounds
     disjoint from the swept box (section 5's test);
   - passed over by a read flag: `CollideActors` clear or the owner's `bCollideActors` clear (the per-component test);
     `BlockNonZeroExtent` clear; the skip flag;
   - **any other candidate**: `NotRead`, "Actor.SetPhysics: the base search's floor trace (a candidate the walker
     tests: the owner filter's blocked-by test and the primitive's swept-box test are not ported)". The filters are read
     (section 3), but several Willow steps read unnamed fields, and the primitive's own test is unread.
5. **No candidate left and the level clear:** no floor. With `Base` None nothing changes (the stand-in's log line);
   with a `Base` set, `BASE_SETTER`'s `NotRead`.

The `NoLevelBaseSearch` stand-in becomes this rule on a `NoLevel` service (level part Clear): its own-collision skip
widens to section 4's chains, and `clear_of` takes section 5's box. Its label changes from "the common convention"
to "read (NATIVE_FLOOR_TRACE_FLAGS.md section 4), UNVERIFIED in game".

**The touch trace** (`touch_trace.rs`, `trace_filter::chain_of`): replace the `Base` walk with section 4's two `Owner`
walks. Links 6 and 7 are re-run after it.

## 7. Predicted effect

- **On main today:** none. The combat world has no collision service, so links 3, 4, 5, 13 and 14 stop at the floor
  trace's `LayerAbsent`.
- **With #807** (the combat run's spawn positions and `clear_of`): those links pass the floor trace. With section 5's
  correction they still do: the spawns stand 1000 units apart. With section 4's chains, Maya's controller is passed
  over by the rule rather than by geometry.
- **The touch trace's correction** may change which actors links 6 and 7 pass over (the pickup's owner chain, and the
  actors it owns, in place of its `Base` chain). Not probed.

## 8. Tests (synthetic fixtures)

1. **The extent:** a toy pawn with a cylinder `CollisionComponent` (radius 30, height 40): the swept box (30, 30, 40)
   from `Location` down by 8; a non-cylinder `CollisionComponent`: the extent `NotRead`; a mistyped radius:
   `VmError::Script`.
2. **The source exclusion:** the source's own cylinder on the segment: passed over; its `Owner` (a toy controller with
   a cylinder at the same place): passed over; an actor whose `Owner` is the source: passed over; an actor that is
   only the source's `Base`: **not** passed over (it meets the box: `NotRead`).
3. **Section 5's box:** another cylinder with its axis 35 units from the line, radius 10, the source's radius 30:
   meets the box (35 < 40), so `NotRead` (where `clear_of` would clear it); at 45 units: passed over; a cylinder
   wholly below the swept box: passed over; touching: `NotRead`.
4. **The read flags:** a candidate with `CollideActors` clear, its owner's `bCollideActors` clear, or
   `BlockNonZeroExtent` clear: passed over without its filter fields loaded.
5. **The level part:** `Unsupported`: its `NotRead`; `Hit` with no actor candidate and `Base` None: the base setter's
   `NotRead` (a toy pawn class outside SetBase's list); `Clear` and nothing else: no floor, the log line.
6. **Each refusal before the mode write,** with #518's three counters unchanged.
7. **The touch trace:** a candidate owned by the source pickup, and the pickup's own `Owner`: passed over; the
   pickup's `Base` alone: no longer passed over.
8. Reference: unchanged.

## Open questions

1. **UNVERIFIED:** every rule above.
2. **Not read:** the primitive's own swept-box test (the cylinder's); the comparator between a BSP hit and an actor hit;
   the `Volume` filter's `bPawnsOnly` source test; the unnamed fields and flags of the Willow steps in section 3.
3. **By fit:** the trace start's translation field; `bAllowProjectilesToCollideWithSkeletalMesh`, `bIsBlockingMesh`
   and `TemporaryNonBlockedBy` (the last one field later than the layout tool places it, as for `WillowAIPawn`).
4. **Whether a terrain owner** is tested by its own filter: bit 8192 is in the walk mask, but no per-actor test of it
   was found (NATIVE_SPAWN_PLACEMENT.md section 7).

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. With Maya standing on another pawn, read her `Base` after a jump lands: the pawn (pawns can be the floor).
2. Stand Maya with an AI pawn's cylinder within her radius plus its radius but outside its own radius from her
   centre, then call `SetPhysics(PHYS_Walking)` from `PHYS_Falling` and read `Base`.
3. Read a possessed pawn's `Owner` (its controller) and confirm the controller's collision does not become its `Base`.
