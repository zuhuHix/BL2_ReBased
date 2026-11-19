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
1. **`bNoCollisionFail` false:** the encroachment check runs on the new actor at its location and rotation, with touch
   notification on.
   - If it reports encroachment, the actor is destroyed through the ordinary destroy routine (no forced network
     destroy, the level may be modified), and the spawn returns None.
2. **`bNoCollisionFail` true and the actor's `bCollideActors` set:** the separate engine step of
   NATIVE_SPAWN_PLACEMENT.md section 5 runs instead (not read in this lane).
3. Then the spawn's later steps run. In the routine, `PostBeginPlay` and the rest of begin play follow this point.

The world lists' link (NATIVE_ACTOR_DESTROY_CONTROLLER.md section 4) is a native begin-play step whose place was not
identified. The port links after this check, so a refused actor is never reachable through a list.

## 2. What the check does

1. **The gate.** The whole check runs only when the new actor's own `bCollideActors` is set, read on the live actor
   after `PreBeginPlay`. Otherwise it reports no encroachment, sends no touch, and writes nothing.
2. **The location.** For certain physics modes (`Physics` of rigid body, one other mode, or one movement flag), the
   actor is first moved to the asked location and rotation when they differ, and its components are updated. A
   spawn's check asks for the location the actor already holds, so nothing changes there.
3. **The shape.** The actor's bounding cylinder (NATIVE_SPAWN_EXTENT.md). A radius and a height of 0 make the check a
   zero-extent one.
4. **The candidates.** When the world has its actor collision hash, the hash is asked for every primitive that overlaps
   the new actor's own colliding primitives at that location and rotation. The query's filter is one flag set, read
   as:
   - pawns, movers, volumes and other actors;
   - the level's BSP and terrain (NATIVE_SPAWN_PLACEMENT.md section 7's bits).
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
- the touch routine's own writes of step 6.

## 3. Off answers, step by step (for an implementer)

The hub's rulings apply:
- the check runs after the actor was made, so every `NotYet` from it is terminal, its text prefixed "stopped after
  partial writes: ", its kind and record kept, and nothing is rolled back;
- grade by the `VmError` variant.

1. **`bNoCollisionFail` true:** this check does not run; NATIVE_SPAWN_PLACEMENT.md section 6 keeps its relocation
   `NotYet`.
2. **The gate:** the live actor's `bCollideActors` clear: no encroachment, no touch, **no write**, and the spawn goes
   on. This is now a reading, not the port's choice (NATIVE_SPAWN_PLACEMENT.md section 6 called it "taken to pass").
3. **`bCollideActors` set:**
   1. **The location step** writes nothing in a spawn (the location asked is the one held). An implementer checks that
      the asked and held values are equal, and refuses `NotYet { NotRead }`, "the encroachment check's move for
      physics <mode>", if they ever differ.
   2. **The shape:** NATIVE_SPAWN_EXTENT.md's rule, with its own refusals.
   3. **The candidates:** the world's collision service must answer which live actors' primitives overlap the new
      actor's colliding primitives, with the filter of step 2.4.
      - A service that cannot answer an actor overlap (the `NoLevel` service holds no actor bounds) answers Unsupported
        for any live candidate primitive. That becomes `NotYet { LayerAbsent }`, "the encroachment check's overlap
        query".
      - A world with no live candidate primitive at all has an empty overlap set: it needs no bounds.
   4. **An empty overlap set:** no encroachment, no touch, no write.
   5. **A non-empty set, after the skips of step 2.5.1:** `NotYet { NotRead }`, "the encroachment check's per-class
      blocking test", record this note. The touch pass needs the same test. Both refusals come before any write of the
      check's own.
4. **On a reported encroachment** (only once steps 5.3 and 5.4 are read): the spawn destroys the actor through the
   port's destroy routine and returns None. Every write of that destroy happens (NATIVE_ACTOR_DESTROY_CONTROLLER.md).

**The writes, named:**
- the empty path writes nothing;
- a blocked path writes the restored location and rotation, the component update, then the destroy's writes;
- a pushed mover writes its own move;
- a touch writes both actors' touching lists and sends the touch events.
Only the first is in the Off answer today.

## 4. What link 9 meets

The cast's first spawn is the pawn, from `WillowPlayerPawn`'s class default, `bNoCollisionFail` false:
- **`bCollideActors`** on the live pawn after `PreBeginPlay` decides the gate. The class default's value is read from
  the packages by the implementer, not assumed here.
- **Clear:** the check passes with no write (step 2).
- **Set:** the shape is (0, 0, 0) (NATIVE_SPAWN_EXTENT.md section 4), and the only other live actor is the `WorldInfo`
  (the pawn spawns first; the controller and the target come later).
  - If the `WorldInfo` has no colliding primitive the filter tests, the overlap set is empty: no encroachment, no
    write, and the spawn goes on to `PostBeginPlay`.
  - If it has one, the `NoLevel` service cannot place it: `NotYet { LayerAbsent }`, "the encroachment check's overlap
    query", terminal.
  - A cylinder of the controller cannot be met by the pawn's check, since the controller is spawned after the pawn.
    It can be met by the controller's own spawn, if the controller has `bCollideActors`. The same rules apply there.

## Open questions

1. The per-class blocking test (step 2.5.3), the "is encroached" method and the push (step 2.5.4).
2. The touch routine's writes (step 2.6).
3. Which physics modes move the actor in step 2.2 (one was read as rigid body; the other was not named).
4. The relocation step of a `bNoCollisionFail` spawn (NATIVE_SPAWN_PLACEMENT.md open question 2).

## Live checks that would confirm these rules (bl-sdk, the RTSE way)

1. **The gate.** Spawn an actor class with `bCollideActors` false at a spot overlapping a blocking actor. Expect it to
   survive with no `Touch` events on either side.
2. **A blocked spawn.** Spawn a class with `bCollideActors` and `bBlockActors` twice at one free spot. Expect the
   second spawn to return None, with `Destroyed` after `PreBeginPlay` and no `PostBeginPlay`.
3. **A touch.** Spawn a colliding, non-blocking actor overlapping another. Expect `Touch` on both, in the spawn, after
   `PreBeginPlay`.
4. **The cast's pawn.** Read `bCollideActors` on a freshly spawned player pawn after `PreBeginPlay`, and the
   `WorldInfo`'s collision components.
