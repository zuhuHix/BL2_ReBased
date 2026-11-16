## 2026-11-16: Rust definition store: an actor outside a level reads from 4, as the C++ reads archetypes

AI-assisted (Claude Code). Lane FIX-DEFS-PREFIX changes one branch of the definition store's prefix rule (decision
2026-11-15-16, "Prefix"; `crates/ow-defs/src/store.rs`, `Loader::derive_prefix`). The byte counts are unchanged
(`ow_object::prefix`), no decode changed, and no bounds check was touched. There was no real-game session: the rule
stays **UNVERIFIED** as a rule of the game, as every prefix is.

- **The cause.** Decision 16's rule gave 26 to an `Engine.Actor` export whose outer export is a `Level`, and for any
  other actor took 26 as the "derived" prefix under the structural check (the tag stream must reach its `None` inside
  the export), else `NotYet`. An actor archetype (outer chain through package objects, no level) has its tags at 4, so
  the stream never parses from 26 and every such load was `NotYet` ("its tags do not parse from the 26-byte prefix
  its class gives"). Three lanes worked around it by calling `ow_object::cdo::instantiate_export` at 4 outside the
  store (W5-KISMET decision 2026-11-15-31, W6-MISSION decision 2026-11-15-34 / PR #334, W6-COMBAT decision
  2026-11-15-35 / PR #335).
- **What the C++ does.** The C++ never derives a prefix: each caller of `Runtime::instantiateExport` passes it by hand
  (`src/vm.cpp`). It passes 26 only for actors placed in a level (`mission_script.cpp` `placeWaypoint`,
  `TheWorld.PersistentLevel.WillowWaypoint_9` from `slice.cpp`; `mover.cpp`, the level's `InterpActor`), 8 for
  components, and 4 for every actor archetype it reads: `combat.cpp` `spawnEnemy` (every pawn archetype a combat run
  spawns), `mission_script.cpp` `placeMarcus` (`GD_Marcus.Character.Pawn_Marcus`) and `slice.cpp` `placeDummyPawn`
  (`GD_TargetDummy.Character.Pawn_TargetDummy`).
- **The rule now.** Unchanged: 8 for an `Engine.ActorComponent` subclass; 26, without a check, for an `Engine.Actor`
  whose outer export is a `Level`; 4 for a class deriving from neither `Core.Component` nor `Engine.Actor`. Every other
  export still loads only when the structural check holds at its derived prefix, else `NotYet`; what changed is the
  derived prefix of an actor whose outer is not a level:
  - its chain of outer exports reaches no `Level` (an archetype): **4**, the C++ archetype prefix (was 26);
  - its outer chain reaches a `Level` further up (an actor inside an actor of a level): 26, as before. No C++ call reads
    such an actor, so it keeps the old behaviour rather than a new guess.
  The outer walk follows exports only (an import or 0 ends it) and stops after as many steps as the package has
  exports, so a broken or cyclic chain counts as "no level" and still meets the structural check.
- **Edits outside the lane's files** (`crates/ow-defs/src/*`), both fixtures of W5-DIALOG's mover (decision
  2026-11-15-32), which landed on main while this lane ran: its invented `Engine.upk` placed the mover actors at the
  top of the package, not inside a level, and passed only because the old rule took 26 for any actor whose stream
  parsed there. Under the new rule those exports are read from 4 (their 26 zero bytes even parse from 4 to an empty
  stream) and the mover's `load_export(.., PlacedActor)` is a `PrefixMismatch`. In the game and in the C++ the
  mover's actor is a level's (`TheWorld.PersistentLevel.<actor>`), so the fixtures now give them a level:
  `crates/ow-mission/src/mover/tests.rs` declares a `Level` class and a `TheLevel` export as the package's last
  exports (no other index moves) and re-parents the eight placed actors to it, their paths in the tests becoming
  `TheLevel.<name>`; `tests/mover_parity_test.py` does the same for `PlacedMover` (`TheLevel.PlacedMover`, and `Level`
  declared last in the toy `Engine.upk`). No assertion and no expected value changed besides those paths. Not granted
  in the brief: the owner and the hub should look at these two files.
- **Verified (automated).** New `ow-defs` test `an_actor_archetype_outside_a_level_reads_from_4` on an invented
  package: an archetype inside a package object and one with no outer load through `find` and `load_export` at 4
  (`PrefixMismatch` at 26); an archetype with no tag stream from 4 is `NotYet`; the placed actor still reads from 26;
  an actor nested in a placed actor reads from 26 under the check and is `NotYet` when its tags start at 4. The test
  fails on the old rule with exactly the lanes' error. Two ignored local checks were added beside decision 16's
  (`real_prefix_sample`, `real_pawn_archetypes`, output under `local/` only).
- **Verified (local, installed game, pc-hummer; one release build of main 7c85b366 before the change and one after,
  both parity modes; the C++ `ow-package.exe` built from the unchanged `src/`; outputs under ignored `local/` only).**
  - **Item registry.** The Rust `--item-registry --cooked <Borderlands 2>` before and after: exit 0 both, output
    byte-identical (14,248,914 bytes).
  - **Sanctuary prefix sample.** Every export of the 26 `Sanctuary*` packages of `CookedPCConsole` (111,654 exports):
    the derived prefix, and `load_export` at it in one store per mode with a digest of the loaded slots. Before:
    5,080 at 26, 86,669 at 4, 19,794 at 8, 111 `NotYet`. After: 5,080 / 86,780 / 19,794, none `NotYet`. Of the
    111,654 lines, 111,543 are identical before and after in the prefix file and in each mode's load file; the 111
    that differ are exactly the former `NotYet`s, all actors outside a level (now 4). Of those 111, 25 load from
    their Sanctuary file in both modes; 86 are `NotADefinition` there because another file holds their path first
    under `FirstInScanOrder` (identity by path, decision 16), not a prefix refusal. The pawns among them load through
    `find` (below). The figure of 10,122 runs per mode named in the lane brief has no record in the tree, so
    this sample stands in for it; the hub should check it against its own record.
  - **Pawn archetypes.** Every census path of the whole game root (2,010 files, 2 that do not read) whose first
    holder's class derives from `Engine.Pawn`: 946 paths. Before: 42 (placed in a level) load at 26, 904 are
    `NotYet`. After, in both modes: the same 42 at 26, and all 904 load through `find` at 4, none `NotYet`.
    `GD_Marcus.Character.Pawn_Marcus`, `GD_TargetDummy.Character.Pawn_TargetDummy` and
    `GD_PrimalBeast.Character.Pawn_PrimalBeast` are among them.
  - **Against the C++.** Marcus, the target dummy, the primal beast and every 30th of the 904 archetypes (34 paths;
    the 18 that only DLC folders hold were skipped, the comparison searched `CookedPCConsole` only): 16 pawns, each
    in both modes, Rust `--def-dump <path> --cooked <CookedPCConsole>` against the C++ `<first holder file>
    --object-dump <index> 4 --cooked <CookedPCConsole> --all`, and against the Rust `--object-dump <index> 4 --all`
    of the same export (W3-DECODE's decode with no store, the decode the lanes' workarounds use). All 32 store runs
    exit 0 and are byte-identical to the Rust `--object-dump` (line ends normalised): the store adds nothing to the
    decode at 4. None is byte-identical to the C++: all 16 pawns differ in the same five properties in both modes,
    and all five are the decoder's or its printer's, not the prefix's (every other one of the 961 C++ properties is
    equal): `BloodEmitterClass`, `ControllerClass` and `InventoryManagerClass` print as `class:<Package.Class>` where
    the C++ prints the bare class name; `EditorIconColor` and `EditorQuat` (editor structs on `Engine.Actor`, untagged,
    their defaults) are not in the Rust output; and in Off mode the Rust log has one more line (`defaults: skipped
    Actor.ReplicatedCollisionType (ByteProperty)`). These are reported to W3-DECODE's owner, not fixed here.
- **Workaround sites** (not changed here; their owners can switch to the store):
  - W6-COMBAT, PR #335 (`feature/rust-w6-combat` at fae49594): `crates/ow-combat/src/hit.rs` `instantiate_copy` (line
    1222, the `instantiate_export` call at 1247, used at 1274 and from `crates/ow-combat/src/run.rs` lines 1269 and
    1276). Every pawn archetype now loads through the store. The C++ makes a *copy* of the archetype per spawn
    (`instantiateExport` builds a new object every call), and the store gives one shared definition per path, so a
    switch needs a copy of the loaded definition, not the definition itself.
  - W6-MISSION, PR #334 (on main at 413eafd0): `crates/ow-mission/src/bridge.rs` `instantiate_copy` (line 399, the
    call at 409), `copy_of` (416, the call at 422) and Marcus's director (1799). Marcus's archetype now loads through
    the store; the same copy semantics apply (the C++ `placeMarcus` instantiates a fresh object). Its
    `load_export(.., PropertyPrefix::PlacedActor)` of the waypoint is a placed actor and is unaffected.
  - W5-KISMET (on main at 413eafd0, decision 2026-11-15-31): `crates/ow-mission/src/kismet.rs` lines 396 (the ops)
    and 999 (the variables). These exports are `SequenceOp` / `SeqVar` objects, not actors: the store already read
    them at 4, and this change does not touch them. Decision 31's reason is a different one (the executor keeps the
    decoder's stand-ins and fresh objects, which `DefStore` has no call for), so this fix alone does not let that site
    switch.
- **UNVERIFIED.** That 4 is the archetype prefix of the game (the C++ and this store agree; neither was checked in the
  running game); the 26-under-check case for an actor nested below a placed actor; the per-class rule as a whole.
