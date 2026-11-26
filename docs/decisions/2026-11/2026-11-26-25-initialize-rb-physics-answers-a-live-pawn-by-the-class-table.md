## 2026-11-26: InitializeRBPhysics answers a live pawn by the class table

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-LINK1-RIGID-BODY-CLASS-TABLE (queue 0ad), on main 4bd8d4f. Clean room:
implemented from docs/verification/NATIVE_LINK1_RIGID_BODY_LAYER.md alone (#763 with its r1 fix, read at 005f9c09);
this lane did not read the executable. No session was run. Every rule is UNVERIFIED in the running game.

- **What was decided** (the note's section 3.1 and the hub's r1 rulings), in `ow-world`'s `player_pawn_tail`:
  - On a **live** receiver, each attached `Components` entry is answered by its **first native class**
    (`render_hooks::first_native`, now `pub(crate)`). This replaces 2.A's attached-entry physics `LayerAbsent`. The
    visit is **fixed** unless the pawn's `Physics` is `PHYS_RigidBody` (10).
    - The non-primitive classes (`ActorComponent`, `DynamicLightEnvironmentComponent`, `SpecialMoveComponent`,
      `GearboxDialogComponent`, `StatusEffectsComponent`), and `PrimitiveComponent` and `CylinderComponent` (no
      shape): nothing.
    - The four skeletal classes take the skeletal rule:
      - with `bDisableAllRigidBody` set: nothing;
      - `bUseSingleBodyPhysics` not 0, `bHasPhysicsAssetInstance` with a `SkeletalMesh`, or
        `bEnableSoftBodySimulation`: the physics `LayerAbsent`;
      - the mesh's cloth bool with `ClothSim` zero: `NotYet { NotRead }`, "InitializeRBPhysics: the cloth set-up".
        The bool is `bClothMetal` by place (fitted, UNVERIFIED), and is read only when `ClothSim` is zero and there is
        a mesh;
      - otherwise the writes: `bSkelCompFixed` := fixed, and `ClothMeshIndexData` emptied when `ClothSim` is zero;
      - then each `Attachments` component in the attach record under the pawn, by the same table, recursively. One
        not in the record is not visited.
    - Any other first native class: `NotYet { NotRead }`, "WillowPawn.InitializeRBPhysics: the rigid-body set-up of
      <class>".
  - **The preflight** is load-free and comes before the first write, so no refusal needs the partial-writes prefix:
    - every declaration with its kind (`VmError::Script`);
    - every entry, attachment component and read mesh resident (the not-loaded `NotRead`);
    - refusal L for a live entry not in the attach record (kept, the r1 ruling);
    - `SetPushesRigidBodies`' own refusal, folded in (the r1 ruling).

    A component met twice in the walk is a data error, a guard of the port's.
  - **A template receiver** (`Origin::Default`) keeps the physics `LayerAbsent` and is never written (the r1 ruling).
  - Queue 0ac: `tree_ticks`' "o resident, else not_loaded naming role" doc line moves onto `fn require`.
- **Reference:** unchanged; it binds no `InitializeRBPhysics` (no C++ stand-in).
- **The slice gate** (installed, Off, strict, release build, under the machine lock; main 4bd8d4f and this change): 6
  ok / 8 `NotYet`, 47.0% on both.
  - Links 1, 3, 4, 5, 13 and 14 move off "WillowPawn.InitializeRBPhysics: a component's rigid body". They do **not**
    reach `Engine.GetPhysXLevel()`, the note's prediction. They stop in `InitializeRBPhysics` at **refusal L**,
    "Actor.GetBoundingCylinder: the colliding components' bounds" (`LayerAbsent`, the record
    NATIVE_LINK1_RBPHYSICS_AND_AFTER.md).
  - **Why** (a local diagnostic, not committed): Maya's `Components` hold seven entries. The five non-primitive ones
    are in the attach record and now answer nothing. **The body mesh (`SkeletalMeshComponent`) and the cylinder are
    not in the record**: each has `Owner` = the pawn, written once, and neither was ever published.
  - By reading, they are the body composition's new components. `body_composition::attach_to_actor` writes `Owner`
    and appends to `Components` but publishes no attach. Before this change, the first attached non-primitive entry
    refused first, which hid this.
  - **For the hub:** whether the body composition's actor attach publishes to the attach record (as the spawn's
    component update does). Until then, refusal L holds, as ruled.
  - Every other row is unchanged.
- **Tests** (`player_pawn_tail_tests.rs`, invented classes), the note's tests 1 to 13, plus three:
  - an unloaded attachment component or mesh is the not-loaded `NotRead`;
  - a skeletal class missing a field is a data error;
  - a script subclass of the cylinder takes its native class's answer.

  2.A's attached-entry test now expects nothing for an attached `PrimitiveComponent`. Eight mutations each fail a
  test:
  - fixed always true;
  - `bDisableAllRigidBody` ignored;
  - the asset instance without the mesh test;
  - no cloth step;
  - the record ignored for attachments;
  - an unread class answered as nothing;
  - the pushing check after the writes;
  - refusal L skipped.
- **Verified:** the tests on invented data and the installed rows above. **Not verified:** everything in the running
  game, the cloth bool's identity, and the attach of the body composition's components.
- **Sensitive files:** none.
