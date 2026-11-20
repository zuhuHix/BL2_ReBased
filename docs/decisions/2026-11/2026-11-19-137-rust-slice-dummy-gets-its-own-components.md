## 2026-11-19: The slice's Fire dummy gets its own components, in Off

AI-assisted (Claude Code, Claude Opus 5.5). Lane DUMMY-COMPONENTS, slice link 11, part A of the hub's spawn ruling: in
Off every actor gets its own copy of each component its template owns, at spawn, and natives such as
`QueryInterface` stay load-free. Synthetic fixtures only.

- **What changes (`ow-mission` `slice.rs` `place_dummy_pawn`).**
  - The Fire dummy is not made by `ow_world::actors::spawn`: the slice instantiates the archetype export directly
    (the C++ `instantiateExport`). That skipped the per-actor component plan, so the dummy's `Mesh` stayed the
    archetype's template `Pawn_TargetDummy:SkeletalMeshComponent_333`, an unloaded definition handle that the
    component `QueryInterface` rule refuses (#524).
  - In Off the dummy now goes through the same plan as the world's spawn (#478's
    `instancing::plan_components_tracked`, re-exported from `ow_world::actors`; a one-line change there):
    - the plan is worked out on the archetype's definition before the pawn is made. A lazy load during it,
      including the definition's own load, counts as a write, and a planning refusal after one is marked "stopped
      after partial writes: ";
    - then the pawn is instantiated and the plan applied (`apply_plan`): each copy is outered to the pawn and
      re-pointed in its slots. A refusal there comes after the pawn was made, so it is marked;
    - a copy whose class declares `Owner` gets the pawn there, as the game's attach step sets it. UNVERIFIED in this
      game: the port models that step nowhere else.
  - Reference is unchanged: the dummy keeps the archetype's references, as the C++ does.
- **Review r1 of #526:**
  1. The plan is built from the slots the pawn receives: the pawn is instantiated first, as the C++ makes it, and the
     plan is worked out on it, its identity the archetype's export. A cached definition carrying an overlay no longer
     decides which components are copied.
  2. Writes are tracked by #518's canonical counters, copied as they are so the rebase folds: `Heap::committed_writes`,
     `GameRng::draws` and `DefStore::load_transitions`, with the VM's in-place array changes reported. They are recorded
     when the placement begins, and a refusal is marked exactly when one of them moved.
  3. The dialog setup runs inside the tracked placement, so its `NotYet` after the pawn's writes is marked. The dummy
     is published only once the whole placement has succeeded, so a retry places it again.
- **Tests.** `ow-mission` `slice/tests.rs` `the_dummy_gets_its_own_mesh_in_off`, on an invented archetype owning a
  `SkeletalMeshComponent` export:
  - Off: the dummy's `Mesh` is a per-actor copy, outered to the dummy, with the dummy as its `Owner`, and the
    template is untouched;
  - Reference: the template.
  - The fixture's `Core.upk` gains `Core.Component`, and `Engine.upk` a `SkeletalMeshComponent` declaring `Owner`.
  - Review r1 of #526: an overlay that clears the archetype's `Mesh` still leaves the dummy its own copy of the mesh
    the export names. A body class whose load fails gives a marked `NotYet`, the dummy unpublished, and a retry places
    it again.
- **Checks.** `cargo test -p ow-mission` 207 passed; `cargo test --workspace` 2,449 passed, 0 failed, 46 ignored;
  crate edges OK.
- **Installed `--slice-gate`, Off, strict, under the machine lock,** on a throwaway local merge of this commit with
  #496 (32bfcd76), #497 (7af3ab76) and #524 (a0c5c13d). Test-file conflicts were taken from one side, and the release
  binary was built only for the run; it is never pushed. Result: 2 ok / 1 not yet / 11 error.
  - Link 10 ok, 8 of 8 steps.
  - Link 11 passes 4 of its 8 steps and stops in `hit:fire`. It is past the component `QueryInterface` now: the
    dummy's own mesh forwards to the dummy, the cycle guard gives none, and no hold is applied. The stop is the
    unbound native `WillowLeviathanService.RecordMissionObjectiveChangedEventForPlayer(object,int,int,int,name)`, a
    VM error, as NATIVE_PAWN_AI_INTERFACE.md section 6.5 predicted. The hub's ruling R3 covers it, but R3 is not on
    these branches.
  - Link 12 is blocked behind link 11, at the same native.
  - After review r1 of #526 (rerun with #496 32bfcd76, #497 7af3ab76, #524 d691534a): the same rows, 2 / 1 / 11; link 11
    4 of 8 at the same native.
- **UNVERIFIED.** Nothing here was checked in the running game. The `Owner` write stands for the game's attach step.
