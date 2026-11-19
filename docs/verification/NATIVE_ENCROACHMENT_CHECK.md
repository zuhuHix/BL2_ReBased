# The post-spawn encroachment check (draft, 2026-11-19)

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-ENCROACHMENT-CHECK (decision number pending from the hub). It reads
the check that NATIVE_SPAWN_PLACEMENT.md section 5 describes from outside, and answers that note's open question 1 in
part.
- **Sources:** the game executable, read with Ghidra (`tools/ghidra/run.ps1`, read-only queries, one headless run at a
  time under the machine lock); field names from `tools/ghidra/class_layout.py`.
- **Own words only:** no decompiler output, pseudo-code, addresses, offsets or listing structure. Raw output stays
  under ignored `local/analysis/spawnspot/`.
- **No live session** was run. **Every rule is UNVERIFIED in the running game.**

**Clean room.** This lane implements nothing. A fresh implementer codes the Off answers from this note.

## 1. Where it runs in the spawn

The spawn routine (NATIVE_ENGINE_CORE.md step 3, NATIVE_SPAWN_PLACEMENT.md section 5), after the actor is made,
`PreBeginPlay` has run and the per-component engine call is done:
1. **`bNoCollisionFail` false:** the encroachment check runs on the new actor at the spawn's location and rotation,
   with touch notification on.
   - If it reports encroachment, the actor is destroyed through the ordinary destroy routine (no forced network
     destroy, the level may be modified), and the spawn returns None.
2. **`bNoCollisionFail` true and the actor's `bCollideActors` set:** the separate engine step of
   NATIVE_SPAWN_PLACEMENT.md section 5 runs instead (not read in this lane).
3. Then the spawn's later steps run. In the routine, `PostBeginPlay` and the rest of begin play follow this point.

**The world lists.** The link into `ControllerList` and `PawnList` (NATIVE_ACTOR_DESTROY_CONTROLLER.md section 4) is a
native begin-play step whose place was not identified. The port links after this check, so a refused actor is never
reachable through **those two lists**. The level's actor list is another matter: the actor joins it when it is made,
before `PreBeginPlay`, so a refused actor is still in it (the destroy of section 3, step 5 takes it out).

## 2. What the check does

1. **The gate.** The whole check runs only when the new actor's own `bCollideActors` is set, read on the live actor
   after `PreBeginPlay`. Otherwise it reports no encroachment, sends no touch, and writes nothing.
2. **The location.** The spawn passes the location and rotation it placed the actor at. For certain physics modes
   (`Physics` of rigid body, one other mode, or one movement flag), the actor is first moved to those values when its
   current `Location` and `Rotation` differ, and its components are updated. A spawn whose `PreBeginPlay` did not move
   the actor passes the values the actor already holds, so nothing changes there.
3. **The shape.** The **live actor's** bounding cylinder: the rule of NATIVE_SPAWN_EXTENT.md (sections 2 and 3),
   asked of the live actor, not of the template. By now the per-component engine call has attached the live actor's
   components, so its colliding primitives can differ from the template's. A radius and a height of 0 make the check a
   zero-extent one.
4. **The candidates.** When the world has its actor collision hash, the hash is asked for every primitive that
   overlaps the new actor at that location and rotation. The query's filter is one flag set, read as:
   - pawns, movers, volumes and other actors;
   - the level bit and one more bit (NATIVE_SPAWN_PLACEMENT.md section 7's bits).

   **A world without the hash** skips the query: the overlap set is empty, so no encroachment and no touch.
   Not read: how the query picks the new actor's own primitives, and whether it tests the level's BSP itself or only
   the actors' primitives the hash holds (open question 5).
5. **The encroachment pass**, for each overlap in the hash's order:
   1. **Skipped:** the actor itself, an actor that is pending delete, and an actor based (directly or through its base
      chain) on the new actor.
   2. **Skipped:** an overlap whose primitive does not block the query's kind (`BlockZeroExtent` for a zero shape,
      else `BlockNonZeroExtent`).
   3. **The per-class test:** the new actor's own engine method decides whether that actor and primitive block it. A
      "no" ends this overlap.
   4. **On a "yes":** a mover that is pushed by encroachers may be pushed out of the way, through its own movement
      calls. Otherwise the other actor's own engine method decides whether it is encroached. When it is:
      - the new actor's location and rotation are put back to what they were before step 2;
      - its components are updated again;
      - the check reports encroachment.
   5. **Otherwise** the other actor is told it was encroached and not blocked (an engine notification on it).
6. **The touch pass** (touch notification is on in a spawn), over the same overlaps, with the same skips:
   - an overlap the new actor's per-class test does not block becomes a touch. The two actors begin touching, through
     the engine's touch routine, with a hit location and normal computed from the overlap or from the direction
     between them;
   - a blocking overlap is not touched here.
7. **The result:** no encroachment.

Not read:
- the per-class test of step 5.3;
- the other actor's "is encroached" method of step 5.4;
- the push of step 5.4;
- the touch routine's own writes of step 6;
- the hash query's own primitive selection and level test (step 4).

## 3. Off answers, step by step (for an implementer)

**Off only. Reference keeps its pre-checks and texts.**

The hub's rulings apply:
- the check returns its refusals **unmarked**. It runs after the actor was made, so the spawn's post-insertion
  boundary marks them: `ow_world::actors::spawn` maps the rest of the spawn through
  `NotYet::after_partial_writes`, which adds "stopped after partial writes: " and keeps the kind and record (it is
  idempotent). Nothing is rolled back;
- grade by the `NotYet`'s `Missing` kind (`LayerAbsent` / `NotRead`), never by text.

**Colliding primitives of the live actor** (the port's definition, from NATIVE_SPAWN_EXTENT.md section 2, applied to
the live actor; UNVERIFIED as the hash query's own selection, open question 5): each non-None entry of the live
actor's `Components`, and its `CollisionComponent`, that is a primitive component, is attached (`bAttached`), has
`CollideActors` set, and has the skip flag clear.

**The port does not model the per-component attach step** that the game runs before this check. So the port's
`bAttached` on a live component does not say what the game's would. An unattached primitive is therefore never taken
to pass: see step 3.2.

1. **`bNoCollisionFail` true:** this check does not run; NATIVE_SPAWN_PLACEMENT.md section 6 keeps its relocation
   `NotYet`.
2. **The gate:** the live actor's `bCollideActors` clear: no encroachment, no touch, **no write**, and the spawn goes
   on. This is now a reading, not the port's choice (NATIVE_SPAWN_PLACEMENT.md section 6 called it "taken to pass").
3. **`bCollideActors` set**, in this order:
   1. **No collision service:** `NotYet { LayerAbsent }`, "the post-spawn encroachment check", as main's
      `ow_world::placement::after_pre_begin_play` answers today.
   2. **The attach step.** If the live actor has any primitive in `Components` or `CollisionComponent` with
      `CollideActors` set: `NotYet { NotRead }`, "the encroachment check: the new actor's per-component attach step
      (which primitives it attaches)". This comes before any `bAttached` read, so "unattached, so pass" is never
      answered.
   3. **The location step.** The two values compared are the spawn's passed location and rotation (the spot it
      found and the asked rotation) and the live actor's `Location` and `Rotation` after `PreBeginPlay`. If they
      differ: `NotYet { NotRead }`, "the encroachment check's move to the spawn location", **whatever the `Physics`
      value**, until open question 3 is read. Equal values write nothing.
   4. **The shape:** NATIVE_SPAWN_EXTENT.md's rule on the live actor, with its own refusals.
   5. **The level part.** Whether the hash query tests the level's BSP is not read (section 2, step 4). Ask the
      service's level part (the box test with the shape's extent at the location) first:
      - **Clear:** both readings agree, so go on;
      - **Blocked or Unsupported:** `NotYet { NotRead }`, "the encroachment check: whether its query tests the
        level's BSP". (`NoLevel` answers Clear.)
   6. **The candidates.** The collision service has no overlap query (`SpawnCollision` in `ow_world::placement`, the
      design of #477). The port walks the live actors itself, as the spot search's actor walk does. The region is
      the new actor's own bounds at its location (the shape's box, or its colliding primitives' bounds when the
      service reports them), so the new actor's own bounds are needed. Then, in this precedence:
      1. **The new actor has no colliding primitive:** the overlap set is empty. (With the attach step of step 3.2 not
         modelled, this is reached only when no primitive has `CollideActors` set.)
      2. **A live primitive of another actor, after the skips of section 2, step 5.1, whose bounds the service does not
         report:** `NotYet { NotRead }`, "the encroachment check's overlap query (the live actor primitive <name>, whose
         bounds the collision service does not hold)". This is the same kind as #477's walk; the service is present
         and only the data is missing, so `NotRead` and not `LayerAbsent`.
      3. **Bounds that miss the region:** that primitive leaves the walk (before its flags are read, as in #477).
      4. **Bounds that meet the region:** `NotYet { NotRead }`, "the encroachment check's per-class blocking test",
         record this note. The touch pass needs the same test.
   7. **An empty overlap set:** no encroachment, no touch, no write.

   All of these come before any write of the check's own.
4. **A world without the hash** (section 2, step 4) is not a case the port has: the port always walks its live list,
   which stands for a world with the hash.
5. **On a reported encroachment** (only once section 2's steps 5.3 and 5.4 are read): the spawn destroys the actor
   through the port's destroy routine and returns None. Every write of that destroy happens
   (NATIVE_ACTOR_DESTROY_CONTROLLER.md).

**The writes, named:**
- the empty path writes nothing;
- a blocked path writes the restored location and rotation, the component update, then the destroy's writes;
- a pushed mover writes its own move;
- a touch writes both actors' touching lists and sends the touch events.
Only the first is in the Off answer today.

**For the implementer, also:**
- update NATIVE_SPAWN_PLACEMENT.md open question 1 (the gate is now read, the rest points here);
- update the doc comments in `crates/ow-world/src/placement.rs` (module documentation, "After `PreBeginPlay`") and
  `crates/ow-world/src/actors.rs` (the spawn's documentation) that say a non-colliding actor is "taken to pass": it is
  now a reading of the gate.

## 4. What link 9 meets

The cast's first spawn is the pawn, from `WillowPlayerPawn`'s class default, `bNoCollisionFail` false:
- **`bCollideActors`** on the live pawn after `PreBeginPlay` decides the gate. The class default's value is read from
  the packages by the implementer, not assumed here.
- **Clear:** the check passes with no write (section 3, step 2).
- **Set:** the cast's world has a service (`NoLevel`), so step 3.1 passes. Then:
  - if any of the live pawn's primitives has `CollideActors` set, the attach-step `NotYet { NotRead }` of step 3.2,
    terminal and prefixed by the spawn's boundary;
  - otherwise the location step and the shape. The template's extent of (0, 0, 0) (NATIVE_SPAWN_EXTENT.md section 4)
    is the port's value for the class default, not the live actor's in the game, whose attached components may give
    another shape. `NoLevel`'s level part answers Clear;
  - the walk: the only other live actor is the `WorldInfo` (the pawn spawns first; the controller and the target come
    later). With no colliding primitive on the new pawn the set is empty. Otherwise a `WorldInfo` primitive that
    survives the skips is the `NotRead` of step 3.6.2, since `NoLevel` reports no bounds.
  - A cylinder of the controller cannot be met by the pawn's check, since the controller is spawned after the pawn.
    It can be met by the controller's own spawn, if the controller has `bCollideActors`. The same rules apply there.

## Open questions

1. The per-class blocking test (section 2, step 5.3), the "is encroached" method and the push (step 5.4).
2. The touch routine's writes (step 6).
3. Which physics modes move the actor in step 2 (one was read as rigid body; the other was not named).
4. The relocation step of a `bNoCollisionFail` spawn (NATIVE_SPAWN_PLACEMENT.md open question 2).
5. The hash query's own primitive selection for the new actor, and whether it tests the level's BSP.
6. The per-component attach step's rule (which of the live actor's primitives it attaches).

## Live checks that would confirm these rules (bl-sdk, the RTSE way)

1. **The gate.** Spawn an actor class with `bCollideActors` false at a spot overlapping a blocking actor. Expect it to
   survive with no `Touch` events on either side.
2. **A blocked spawn.** Spawn a class with `bCollideActors` and `bBlockActors` twice at one free spot. Expect the
   second spawn to return None, with `Destroyed` after `PreBeginPlay` and no `PostBeginPlay`.
3. **A touch.** Spawn a colliding, non-blocking actor overlapping another. Expect `Touch` on both, in the spawn, after
   `PreBeginPlay`.
4. **The cast's pawn.** Read `bCollideActors` on a freshly spawned player pawn after `PreBeginPlay`, its components'
   `bAttached` and `CollideActors`, and the `WorldInfo`'s collision components.
