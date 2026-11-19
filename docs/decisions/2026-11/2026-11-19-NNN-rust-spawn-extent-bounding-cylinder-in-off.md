## 2026-11-19: The spawn's extent, the bounding-cylinder rule, in Off

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-LINK9-EXTENT, slice link 9. Coded from
docs/verification/NATIVE_SPAWN_EXTENT.md alone, the note at its review-r1 head 6f842ee8 (PR #488; clean room: lane
LINK-9-NEXT wrote it, decision 2026-11-19-09). The executable and the analysis output were not opened. Synthetic
fixtures only. Based on main d72b96d4 (#477 and #488 landed).

- **What changes (`ow-world` `placement::extent`, used by the spawn's spot search).** The template's extent is now
  answered whatever its collision component is, in the note's section 5 order:
  1. A `CylinderComponent` as the template's `CollisionComponent` gives its radius and height, as before.
  2. Otherwise the bounding-cylinder rule of the template's **exact** class:
     - for `Pawn`, `WillowPawn`, `WillowAIPawn` and `WillowPlayerPawn`: a `CylinderComponent` that is set, is not the
       `CollisionComponent` and is a cylinder component gives its radius and height; otherwise `Actor`'s rule;
     - for `Actor`, its own rule (below);
     - any other class: the named `NotYet` (`NotRead`) "the bounding-cylinder rule of <class>". The note read only
       these five classes, and another class may not be assumed to keep its parent's body.
- **`Actor`'s rule, for each entry of the template's `Components`, in the note's step 5 order:**
  1. a None entry is skipped;
  2. a non-primitive entry is skipped, decided from the class its handle carries, and nothing is loaded for it;
  3. a primitive entry that is a lazy handle is loaded, as the template's own definition;
  4. an entry with `bAttached` clear is skipped;
  5. an entry with `CollideActors` clear is skipped;
  6. an entry that passed steps 2 to 5 meets the skip flag. The port cannot read it (`trace_filter::SKIP_FLAG`), so it
     is the named `NotYet` (`NotRead`).
  7. `Bounds` is not reached while step 6 refuses. `box_cylinder` (the union's horizontal half-diagonal and half
     height) is in place and tested for when the flag is read. No qualifying entry gives (0, 0, 0).
  - A field the classes do not declare is a `VmError::Script` naming it (the actor field reads).
  - A loaded stand-in with no slots is a named `NotYet` (`NotRead`). An entry still unloaded when the read-only
    extent runs is a `VmError::Script`: the caller skipped the loads.
  - A cylinder component the store has not loaded is a named `NotYet` (`NotRead`): the note loads `Components`
    entries only.
- **The loads are writes, and they run in the spawn.**
  - `placement::load_extent_components` runs just before the spot search, which stays read-only. It runs only when
    the world has a collision service, so the no-service refusal comes first and stays clean, and only when the extent
    is `Actor`'s rule.
  - Once a load ran, every `NotYet` up to the actor's insertion is marked "stopped after partial writes: ", its kind
    and record kept, nothing rolled back. That covers the spot search's, the base precheck's, the instancing's and a
    later load's. Other errors propagate unchanged. After the insertion, the spawn's existing rule marks them all.
  - With no load, nothing is marked: the refusals before it are clean.
  - Reference is unchanged, because the spot search is Off only.
- **Tests.**
  - `placement_extent_tests.rs`, eight on invented classes:
    - a cylinder collision component on any class;
    - the pawns' separate cylinder on all four classes;
    - link 9's template (no cylinder, unattached and non-primitive components) with the zero extent;
    - an attached colliding primitive meeting the skip flag;
    - another class;
    - a stand-in entry;
    - an undeclared field as a `Script` error;
    - the box cylinder's arithmetic.
  - World tests, through the spawn, on two invented `Actor` archetype exports. Each lists a lazy primitive, a None
    entry and a lazy non-primitive object.
    - The primitive is loaded and the other entry is not.
    - Attached and colliding: the skip-flag `NotYet` is marked, and the service is never asked.
    - Unattached: the search's first query is a zero-extent box test, and the next `NotYet` (the instancing's, on the
      fixture's non-component primitive class) is marked.
    - Without a service: the clean `LayerAbsent`, nothing loaded.
  - The world test that pinned "the actor's bounding-cylinder rule" now expects "the bounding-cylinder rule of
    Engine.Probe".
- **Installed `--slice-gate`, Off, strict: 0 ok / 5 not yet / 9 error, the same as main d72b96d4.**
  - The 9 errors are main's, row for row (`GameEngine.GetOnlineSubsystem()` unbound on the combat-run links among
    them). They are not this lane's.
  - Only link 9's row changed. It still stops in `spawn`, at 0 of its 2 steps (`spawn`, `cast`), now `NotYet`
    (`NotRead`) "stopped after partial writes: the spawn placement spot search: an axis-aligned box test in template
    mode: the trace filter of WillowGame.WillowPlayerController (the live actor primitive WillowPlayerController_0's
    CollisionCylinder, whose bounds the collision service does not hold)". The base stopped at "the actor's
    bounding-cylinder rule".
  - So Maya's `WillowPlayerPawn` gets an extent, and a later spawn of the cast walks the already-spawned controller.
    `NoLevel` holds no actor bounds, so the walk refuses.
  - The note predicted the trace filter on the `WorldInfo` or the post-spawn encroachment check. The measured stop is
    the trace filter on the live controller's cylinder: the cast has spawned the controller by then.
  - Which earlier write gives the mark (this spawn's component loads, or the cast's earlier spawns) is not
    separated in the row.
  - The other 13 rows are byte-identical.
- **UNVERIFIED.** Nothing here was checked in the running game. Still open, from the note: which other classes
  replace the rule (open question 1), the skip bit's name (2), and whether a class default ever has an attached
  component (3).
