# InitializePickupForRBPhysics checks its declarations up front, and the globals' reference checks look nothing up

AI-assisted (Claude Code, Claude Opus 5.5), lane FOLLOWUP-PICKUP-RB-VALIDATION, on main b0d9ca08. It follows up three
findings a recheck on main confirmed in #587's code (`WillowPickup.InitializePickupForRBPhysics` in Off). This thread
did not write #587 or its note (NATIVE_PICKUP_RB_MINIMAL.md), and read no native code (clean room). **Every rule stays
UNVERIFIED** in the running game.

**1. Every declaration before the branches** (`crates/ow-inventory/src/attach_mesh/pickup_rb.rs`). This replaces
review r1 of #587's item 3 ("each field checked only on the branch that uses it"). A bad declaration is now a data
error (`VmError::Script`) on every path past `bUseRBPhysics`:
- **the pickup**, once `bUseRBPhysics` is set: `CollisionComponent`, `bForceRBToSleep`, `MaxRBAwakeTime`,
  `RotationRate`, `bTorque`, `bTorqueTemporary`, `Torque`, `MaxImpactEffectPlayCount` and `Inventory`, and the enum
  `Physics`;
- **C**, once known: its six component fields, the three reattach fields (`Owner`, `Scene`, `bNeedsReattach`) whether
  or not the reattach runs, and the enum `RBChannel`;
- **G**, once known: `DropImpactEffectMax`, `DropImpactEffectThreshold` and the four spin fields, on the torque path
  too. The spin values are still read only on the spin path.

Two exceptions, each set by what is declared rather than by a branch:
- with `bUseRBPhysics` clear the native reads nothing else (#520's answer), so nothing else is checked there;
- `PhysicsWeight`, `PhysicsAsset` and `bHasPhysicsAssetInstance` are declared only on a `SkeletalMeshComponent`. They
  are checked once C is known to be one; a plain primitive is refusal 2 before them, as before.

`schemas` gains a float kind (`Want::Float`) for these checks.

**2. No lookup in the globals' reference checks** (`crates/ow-inventory/src/globals.rs`). `declared_reference` used
to call `Vm::find_class` before checking the declaration. A lookup can load, and a load is a write in an Off
preflight. It now checks the declaration first, then compares the declared class with the wanted one **by the class's
own path** (`class_named`, case-insensitive), so nothing is looked up or loaded. A declared class with no layout is
`NotYet { NotRead }`. `pickup_list_declared`, in the same globals preflight, had the same lookup and gets the same
change. Both now take `&Vm`. Callers are unchanged: `globals_schemas` and the equip world's set-up in
`ow-cli/src/cmd/inventory/equip_run.rs`.

**3. The counters on the refusals** (`pickup_rb_tests.rs`, `globals_tests.rs`):
- the missing-spin-range case now asserts #518's three counters (`Heap::committed_writes`, `GameRng::draws`,
  `DefStore::load_transitions`) unchanged, with the heap and the timers;
- so does each new refusal: the spin range on the torque path, the reattach fields with `CollideActors` set, `Scene`
  on the owned path, and the globals' mismatches;
- a wanted class no package holds is a plain mismatch, which pins that no lookup is made.

**Fixture changes (rule 9):** two invented component classes. `ToyOwnedMesh` has `Owner` but no `Scene`.
`ToyPrimitive` is a primitive with the reattach fields, so refusal 2 is still reached for a plain primitive.

**Verified (host only):** `cargo test -p ow-inventory` 326 passed / 0 failed. The r1-item-3 test became its reverse,
`every_declaration_is_checked_before_the_branches`, and `the_declared_reference_check_looks_nothing_up` is new.
**Not verified:** anything in the running game. The installed classes declare every field checked here; that the
new checks never refuse real slice data is shown only by the slice gate's unchanged rows, if it runs.
