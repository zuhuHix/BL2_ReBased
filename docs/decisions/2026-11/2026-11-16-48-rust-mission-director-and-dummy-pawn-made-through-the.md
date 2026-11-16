## 2026-11-16: Rust mission director and dummy pawn made through the definition store

AI-assisted (Claude Code). Lane FIX-DIRECTOR-STORE removes the mission bridge's workaround that made the director, and
the other archetype objects the bridge and the Fire slice need, outside the definition store
(`crates/ow-mission/src/bridge.rs` `instantiate_copy` / `copy_of` / `store_object`, decisions 2026-11-15-34 and
2026-11-16-38). It adds one store call, `DefStore::instance` (`crates/ow-defs/src/store.rs`; the additive helper
granted in the lane brief), and one Reference-only stub key in `crates/ow-cli/src/cmd/slice.rs` (granted by the hub,
below). No parsing behaviour changes: no decode, prefix or bounds check was touched. There was no
real-game session; every rule keeps the label its record gives it (UNVERIFIED unless that record says it was seen
live).

- **The cause.** The workaround decoded the archetype with `ow_object::cdo::instantiate_export` over a decode context
  of its own, outside the store, because the store used to refuse actor archetypes (fixed by decision 2026-11-16-36).
  That context has no store resolver, so every reference decoded inside the copy was a decoder stand-in: a slotless
  object that is not one of the store's handles, which the VM does not load before reading (`loaded_for_read` loads
  only the store's unloaded handles). On the installed Fire run, `EvaluateInitializationData` reached one of the
  director's references at the use key and the heap refused the read ("has no slot 15"). The slice's `store_object`
  existed only to swap the copy's body class for the store's object before the dialog read it.
- **What the C++ does.** `Runtime::instantiateExport` (`src/vm.cpp:1100`) builds a new object on every call: the
  export's class defaults, then its tags from the prefix the caller gives. `MissionScript::placeMarcus`
  (`src/mission_script.cpp:484-512`) makes the director from his archetype at 4 (line 487), his `MissionDirectives`
  through `loadRef` (lines 491-492, `instantiateExport` at 4 of the reference's export, lines 956-959), and his
  `AIClass` (lines 498-504) and `DialogComponent` (lines 508-511, at 8) only when the reference names an export
  (`resourceIndex > 0`). `FireMissionSlice::placeDummyPawn` (`src/slice.cpp:362-370`) makes the dummy's pawn from
  `GD_TargetDummy.Character.Pawn_TargetDummy` at 4 (line 366), once.
- **The store call.** `DefStore::instance(space, package, index, prefix)`: a new object made from that export on
  every call, decoded again (as the C++), inside the store's load context, so every reference it decodes is the
  store's object for its path, an unloaded handle that loads when read, as a definition's references are. Its origin
  is `Origin::Export` of the given package and index, its outer `None`. It is not one of the store's objects: the
  identity map does not hold it, `find` never returns it, two calls give two objects, and a write to it changes no
  definition. It decodes the export it is given, not the path's holder under the identity policy (the C++ reads the
  package it was handed). The prefix is checked against the one the store derives (`PrefixMismatch`, or `NotYet`
  where none derives), as `load_export` checks it; an index that is not positive is the decoder's "unsupported
  exported object request" (the C++ text); a code export or a class default object is `NotADefinition`. No overlay is
  applied to an instance (the C++ has none; a later lane that needs overlays on per-call instances decides how).
  A failed decode forgets the handles it freed, as a failed load does.
- **The bridge and the slice now.** `bridge::instantiate_export` (a `pub(crate)` wrapper that moves the store's log
  lines to the VM's log) replaces `instantiate_copy`, and `export_of` (the C++ `resourcePackage` / `resourceIndex`
  of a reference) replaces `copy_of`. `place_marcus` makes the director, his directive table, his AI class and his
  component through it at the C++ lines above, now also skipping an `AIClass` or `DialogComponent` reference that
  names no export, as the C++ does (before, such a reference was an error). `store_object` is removed: the director's
  and the dummy's body classes are the store's objects already, and the dialog loads them (`DialogSystem::load`,
  `ensure_loaded`). The slice's `place_dummy_pawn` makes its pawn through the same wrapper. Nothing of the workaround
  is kept.
- **Verified (automated).** New unit tests on invented packages, each in both parity modes:
  - `ow-defs` `an_instance_is_a_new_object_whose_references_are_the_store_s`: two instances and the store's object
    are three objects; the instance's reference is the store's object for the path and loads and reads; a write to
    one instance changes neither the other nor the definition; the prefix check (an actor component at 8 only); the
    C++ text for index 0; a class default object refused. `an_actor_archetype_instance_reads_from_4`: an archetype
    outside a level at 4 (26 refused), a fresh object per call, `NotYet` where its tags do not parse from 4.
  - `ow-mission` bridge `the_director_is_a_new_object_with_the_store_s_references_in_{reference,off}_mode` (a new toy
    package `TestDirector` and a `MissionDirectives` property on the toy `WillowAIPawn`): the placed director is not
    the store's object; his body class is the store's `Body_Toy`; his directive table is a new object with its tags;
    his component's `TriggerEvent` starts the line under the body class's name tag with no `store_object`; a second
    placement makes another director.
  - `ow-mission` slice `the_dummy_s_pawn_is_a_new_object_with_the_store_s_references` (a new `Map::pawn_reference`
    toy: the pawn's `Controller` names an invented export of its package): the pawn is not the store's object, its
    reference is the store's and reads, and each wrapper call makes another object.
  - The three `ow-mission` tests fail on origin/main's bridge and slice (checked by swapping the two files back,
    with the slice test calling the old `instantiate_copy`): the reference is a decoder stand-in, not the store's
    object.
- **Verified (local, installed game, pc-hummer; outputs under ignored `local/` only).** A release build of
  origin/main 07f07b2 ("before") and of this change ("after"), against the C++ `ow-package.exe` built from the
  unchanged `src/`.
  - `tools/verify_fire_event_order.py` with the Rust reader, `OW_PARITY=reference`: **31 of 33 before, 33 of 33
    after** (the C++: 33 of 33). FIX-PLAYTHROUGH (#341) had already brought the two turn-in rules (31 before, not 29).
    The store change alone gives 32 of 33: "the lines run has no errors" holds and the default run's two heap error
    lines are gone; the default run's "the run has no errors" then fails only on `unbound native
    GFxMovieDefinition.SpawnPlayerMovie(object,interface,object)` at the `turnin` step (in the before build too; not
    the director's). The stub key below brings it to 33 of 33.
  - `--slice-run` step records against the C++: 22 of 22 in the default run and 17 of 17 in the lines run, before and
    after. After the change the run's `errors` list is empty in both runs, as the C++'s (the base had the two heap
    lines and the movie line). `store_object`'s removal changes no step record.
  - `--mission-run` on the 77 missions the `Sanctuary_` packages import (every `MissionDefinition` import of the 26
    `Sanctuary_*` files without `Sanctuary_Hole_*`; all exported in `Startup.upk`), both step lists of decision
    2026-11-15-34, each Rust build in both modes against the C++ (stdout and exit code byte for byte): Reference,
    tracker 73 of 77 and bridge 1 of 77; Off, tracker 58 of 77 and bridge 1 of 77; the same before and after (decision
    39's numbers), and the after build's output equals the before build's in all 616 Rust runs. These runs cannot
    reach the change: they read `Startup.upk`, which holds no `GD_Marcus.Character.Pawn_Marcus`, so `--mission-run`
    places no director (the C++ neither), and 76 of the 77 bridge runs stop on the refused
    `Actor.LocalPlayerControllers` (59 also on `MissionTracker.GetObjectivesProgress`) in Reference mode. The
    director's installed paths are the slice's, above.
  - `tests/mission_script_test.py`'s commands replayed on both Rust builds against the C++ (W6-FIRE's local harness):
    `--slice-run` 13 of 20 under Reference and 5 of 20 under Off, `--mission-run` 34 of 34 and 33 of 34, before and
    after, the same calls in each case (decision 38's and 39's numbers).
- **The turn-in's movie spawn, a Reference-only stub (granted by the hub).** At the turn-in the controller script
  calls `SpawnPlayerMovie` on a movie definition it reaches through the globals data. The C++ holds that reference as a
  stand-in with no class and logs `UNIMPLEMENTED function ?.SpawnPlayerMovie` (the `?.SpawnPlayerMovie` key W6-FIRE put
  in the slice's Reference allow-list), returning nothing. The Rust store resolves the reference to its
  `GFxMovieDefinition`, whose `SpawnPlayerMovie` is a native, so the call reaches the VM's stub policy under that
  native's key instead. Reached only since FIX-PLAYTHROUGH made the reward page reachable; the before build has the
  same line.

  | Stand-in | Reference (the C++) | Off | Pinned by |
  |---|---|---|---|
  | `GFxMovieDefinition.SpawnPlayerMovie(object,interface,object)` (`REFERENCE_STUBS` in `cmd/slice.rs`, the 19th of decision 38's list beside `?.SpawnPlayerMovie`) | allowed as a logged zero-result stub (the C++ only logs it; the logged text is the Rust stub line, not the C++ `function ?.` text, and no rule reads the stub list) | refused, `VmError::UnboundNative` (architecture 3.8), as every key of the list | CLI unit test `the_reference_allow_list_is_reference_only` (every key allowed in Reference and refused in Off, and this key by name); the installed Fire run, 33 of 33 |

  Like the other keys it is an unread rule: what the movie spawn does in the game is not read, and Off mode refuses it
  until it is.
- **Open items.**
  - `crates/ow-combat/src/hit.rs` has its own `instantiate_copy` (W6-COMBAT, decision 36's workaround list); it can
    switch to `DefStore::instance` in its own lane.
  - `crates/ow-mission/src/kismet.rs` still decodes its ops and variables outside the store (decision 2026-11-15-31's
    reason, not the prefix; unchanged).
  - The script report's `stubs` and `notes` lists still differ from the C++ (their order, the decoder's and the
    store's log lines); no rule reads them.
- **UNVERIFIED.** Everything the director and the dummy do in the game: these are C++-parity choices, and the C++
  reading was not checked in the running game.
