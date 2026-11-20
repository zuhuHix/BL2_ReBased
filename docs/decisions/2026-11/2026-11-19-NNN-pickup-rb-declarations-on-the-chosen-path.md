# InitializePickupForRBPhysics checks the chosen path's declarations, and the globals' reference checks look nothing up

AI-assisted (Claude Code, Claude Opus 5.5), lane FOLLOWUP-PICKUP-RB-VALIDATION, on main b0d9ca08, with review r1 of
#625 and the hub's ruling on it. It follows up findings a recheck on main confirmed in #587's code
(`WillowPickup.InitializePickupForRBPhysics` in Off). This thread did not write #587 or its note
(NATIVE_PICKUP_RB_MINIMAL.md), and read no native code (clean room). **Every rule stays UNVERIFIED** in the running
game.

**1. Declarations on the chosen path, before its first write** (`crates/ow-inventory/src/attach_mesh/pickup_rb.rs`).
The first round moved every declaration check ahead of the branches. The hub ruled that the note wins: its section 3
(at 7c212484) checks only what the chosen path needs. So #587's branch-scoped checks and review r1 of #587's item 3
tests are back as they were. Inputs that passed on #587 pass again: no primitive and no `Inventory` declaration, the
torque path with no spin range, and a mesh with `CollideActors` set and no reattach fields.

One rule is kept: on the chosen path, every declaration the path reads is checked before that path's first write.
What #587 left unchecked were G's impact fields. The path with C copies `DropImpactEffectDelay` and
`DropImpactEffectVelocityAkRtpc`, and reads `DropImpactEffectMax` and `DropImpactEffectThreshold`. Their declarations
(float, float, object, float) are now checked once that path's refusals have passed. On the spin's branch, each of the
four range fields' declarations is checked too. The preflight still plans every write before the first one, so a
malformed declaration there is a data error (`VmError::Script`) with nothing written. A refusal path that never reads
those fields still refuses with its own `NotYet`. `schemas` gains a float kind (`Want::Float`).

Review r2 of #625 restored two exact checks and moved a third:
- **`MaxRBAwakeTime`**, after the `bUseRBPhysics` guard, and **`PhysicsWeight`**, inside the asset path, are checked
  as exact floats again. A plain `FloatAttribute` declaration also holds a float value, so the value read and the
  heap's write check would accept it.
- **`Scene`** is checked only in the ownerless arm of the reattach, the only path that reads it. An owned mesh needs
  `Owner` and `bNeedsReattach` alone.

**2. No lookup in the globals' reference checks** (`crates/ow-inventory/src/globals.rs`). `declared_reference` used
to call `Vm::find_class` before checking the declaration. A lookup can load, and a load is a write in an Off
preflight. It now checks the declaration first, then compares the declared class with the wanted one (`class_named`).
The first round compared paths as text, case-insensitively, which dropped the class table's separator equivalence
(#625 r1, item 3). It now asks the class table's read-only `by_path` and compares ids: `by_path` parses the path, and
it neither loads nor interns. A path the table does not hold is a plain mismatch; a declared class with no layout is
`NotYet { NotRead }`. `pickup_list_declared`, in the same globals preflight, had the same lookup and gets the same
change. Both now take `&Vm`. Callers are unchanged: `globals_schemas` and the equip world's set-up in
`ow-cli/src/cmd/inventory/equip_run.rs`.

**3. The counters** (`pickup_rb_tests.rs`, `globals_tests.rs`):
- `a_malformed_declaration_on_the_chosen_path_refuses_before_any_write` covers three cases:
  - G's `DropImpactEffectDelay` declared an int, on an otherwise passing path: a data error;
  - a primitive whose `BlockActors` is declared an int, read before refusal 2: the data error comes first;
  - refusal 2 with the malformed G: the refusal stands, since that path never reads the impact fields.

  Each asserts the heap, the timers and #518's three counters (`Heap::committed_writes`, `GameRng::draws`,
  `DefStore::load_transitions`) unchanged.
- `the_declared_reference_check_looks_nothing_up` asserts the three counters and the name table's length after
  **each** call. It also checks that `GearboxFramework:GearboxGlobalsDefinition` matches the class stored with a dot.
- `a_declared_only_target_stays_cold` compares a reference declared to a class nothing looks up. The class's layout
  starts undefined and its code cold, and the matching check must succeed. Afterwards both are still cold, and
  neither the counters nor the name table's length has moved.
- `a_float_attribute_declaration_is_not_a_float` covers `MaxRBAwakeTime` and `PhysicsWeight`.
  `the_owned_reattach_reads_no_scene` covers the owned path (r2).

**Fixture changes (rule 9):**
- `ToyBadPrimitive`, a primitive that shadows `BlockActors` as an int;
- `ToyBadImpactGlobalsDefinition`, a globals definition that shadows `DropImpactEffectDelay` as an int;
- `ToyColdTarget` and `ToyColdHolder`;
- `ToyOwnedMesh`, an owned mesh with no `Scene`;
- `ToyAttrWeightMesh` and `ToyAttrAwakePickup`, which shadow `PhysicsWeight` and `MaxRBAwakeTime` as plain
  `FloatAttribute`s (r2).

All of them are invented.

**Verified (host only):** the test counts are in the GATES row. **Not verified:** anything in the running game. The
installed classes declare every field checked here. That the checks never refuse real slice data is shown only by
the slice gate's rows.
