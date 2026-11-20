## 2026-11-19: The post-spawn encroachment check, in Off

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-ENCROACHMENT-CHECK, slice link 9. Coded from
NATIVE_ENCROACHMENT_CHECK.md alone (PR #490, research/rust-encroachment-check at 488f00f3, with the hub's review
corrections; clean room: lane ANALYST-ENCROACHMENT-CHECK wrote it). The executable and the analysis output were not
opened. Synthetic fixtures only. Based on fix/rust-link9-extent (4328062d, #498 r1). The note's file is not on this branch; it
lands with #490.

- **What changes (`ow-world` `placement::encroachment::check`, called by the spawn after `PreBeginPlay`).** In Off,
  for a spawn with `bNoCollisionFail` false whose live actor has `bCollideActors` set, in the note's section 3 order,
  before any write of its own:
  1. **No collision service:** `NotYet { LayerAbsent }`, "the post-spawn encroachment check", main's answer kept.
  2. **The attach step:** the port does not model the per-component attach step the game runs before the check.
     So a primitive of the live actor (its `CollisionComponent` or a `Components` entry) with `CollideActors` set is
     `NotYet { NotRead }`, whatever its `bAttached` says: an unattached primitive is never taken to pass. A
     primitive the store has not loaded refuses the same way.
  3. **The location step:** the spawn's passed location and rotation against the live actor's `Location` and
     `Rotation` after `PreBeginPlay`. Any difference is `NotYet { NotRead }`, whatever the `Physics` value.
  4. **The shape:** the live actor's bounding cylinder (NATIVE_SPAWN_EXTENT.md's rule, `placement::extent`), with its
     own refusals.
  5. **The level part:** the service's box test with that extent at the location. Clear goes on; Blocked or
     Unsupported is `NotYet { NotRead }`. The mode is the spawn's template mode with the live actor's
     `bCollideComplex`: the note names no mode (UNVERIFIED).
  6. **The candidates:** with no colliding primitive the overlap set is empty and the check passes, with no write.
     Step 2 refuses every primitive with `CollideActors`, so that is the only pass reached today.
     - The port's own walk over the live actors, as the spot search's walk does, is in place for when the attach step
       is read. It skips the actor itself, destroyed or pending-delete actors and actors based on the new one.
     - A primitive whose bounds the service does not report is `NotYet { NotRead }` (the overlap query). Bounds that
       miss the region leave the walk. Bounds that meet it are `NotYet { NotRead }` (the per-class blocking test).
- **The gate.** The live actor's `bCollideActors` clear passes with no write. That is now a reading of the check
  (NATIVE_ENCROACHMENT_CHECK.md section 2, step 1), no longer the port's choice.
  - `bNoCollisionFail` with `bCollideActors` set keeps the relocation step's `NotYet { NotRead }`.
- **Marking.** The check and the relocation step return their refusals unmarked. The spawn's post-insertion boundary
  adds "stopped after partial writes: " and keeps the kind and record. `placement::after_pre_begin_play` no longer adds
  the prefix itself.
  - "the post-spawn encroachment check's rule" (`ENCROACHMENT_RULE`) is retired.
  - Grading goes by the `Missing` kind.
  - Reference is unchanged: its collision refusal stays.
- **Docs.** NATIVE_SPAWN_PLACEMENT.md's encroachment answers, section 5's first bullet (review r1 of #503) and its
  open question 1 now point at the new note. The
  "taken to pass" comments in `placement.rs` and `actors.rs` now describe the gate as read.
- **Tests** (`world/tests.rs`, placement module):
  - the check's order, called directly on a live actor:
    - no service;
    - the attach step for an unattached primitive with `CollideActors`, under the note's record;
    - the move, for a passed location and for a passed rotation;
    - no query before the location step passes;
    - the level part for Blocked and for Unsupported, with the live cylinder's extent in template mode;
    - then `Ok`;
  - the walk, called directly:
    - the actor itself and a rider based on it are skipped;
    - no bounds is the overlap query's `NotRead`;
    - bounds that miss leave the walk;
    - bounds that meet are the per-class test's `NotRead`;
  - through the spawn: the attach step's refusal is marked, kind `NotRead`, and the actor stays made;
  - through the spawn, a passing check (review r1 of #503): a probe colliding with actors and the world, whose cylinder
    lacks `CollideActors`, passes on to `PostBeginPlay` and `SetInitialState` and is made at the spot search's
    location, with two box tests asked (the search's first test and the check's level part);
  - the existing encroachment-and-relocation test: with a service, `Collider` now stops at its shape ("the
    bounding-cylinder rule of Engine.Collider"), marked.
- **Installed `--slice-gate`, Off, strict: 0 ok / 5 not yet / 9 error, every row byte-identical to
  fix/rust-link9-extent.**
  - Link 9 passes 0 of its 2 driver steps (`spawn`, `cast`). It stops in `spawn` at "stopped after partial writes: the
    spawn placement spot search: an axis-aligned box test in template mode: the trace filter of
    WillowGame.WillowPlayerController (the live actor primitive WillowPlayerController_0's CollisionCylinder, whose
    bounds the collision service does not hold)" (`NotRead`).
  - That spawn stops in its spot search, before the actor is made, so it never reaches the encroachment check. The
    cast's earlier spawns get past their post-`PreBeginPlay` step: a refusal there would
    have changed the row. Whether they pass by the gate or by the empty overlap set is not separated in the row.
- **UNVERIFIED.** Nothing here was checked in the running game. Still open, from the note:
  - the per-class blocking test, the "is encroached" method and the push (open question 1);
  - the touch writes (2);
  - the move's physics modes (3);
  - the relocation step (4);
  - the hash query's selection and level test (5);
  - the attach step (6).
