## 2026-11-19: The skip flag and the per-component check in Off: GetBoundingCylinder and the spawn extent

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-SKIP-FLAG, slice links 6 and 7. Coded from
docs/verification/NATIVE_COMPONENT_SKIP_FLAG.md section 5 alone, as accepted at 4fd8587d (#653, with the hub's rulings
on its review). Clean room: lane ANALYST-SKIP-FLAG read the native, and this lane did not open the executable or the
analysis output. Synthetic fixtures only. The note's copy on this branch drops out when #653 lands.

**What changes** (Off only; Reference keeps the stub policy). Refusal S is lifted everywhere it stood:
- **The skip check** reads the bool `BulletListener`; another kind is a `VmError::Script` naming it. Set skips the
  entry, with no refusal. Clear goes on. The note reads the field as bit 10 of the collision bool word; its name is a
  structural fit, UNVERIFIED.
- **The per-component check** (`world::bounding_cylinder::component_check`) is asked last, only for an entry that
  every other test would count, right before its box.
  - It passes for the exact classes `Engine.PrimitiveComponent`, `Engine.CylinderComponent` and
    `WillowGame.BulletListenerComponent`, as the note read.
  - Every other class is `NotYet { NotRead }`, "Actor.GetBoundingCylinder: the component check of <class> (not
    read)", with this note as its record. That covers the skeletal, static-mesh and particle components and
    subclasses of the cylinder.
  - The virtual is taken as a pure predicate (UNVERIFIED).
- **`Actor.GetBoundingCylinder`, per entry:**
  - live: None or not a primitive (skipped), N, `CollideActors`, the attach record (L), the skip check, the
    component check, then `entry_box`;
  - template: None or not a primitive, N, `bAttached`, `CollideActors`, the skip check, the component check, then
    L.
- **`placement::actor_rule`, the spawn extent:** after `bAttached` and `CollideActors`, the skip check, then the
  component check, then the bounds step. That step is `NotYet { LayerAbsent }` with the bounding cylinder's refusal
  L text and layer, on the extent's record.
- **`trace_filter::SKIP_FLAG`** drops "unresolved": "the trace filter's skip flag (`BulletListener`, bit 10 of a
  primitive's collision flag word)". The comment on `SKIP_FLAG_FIELD` now says both paths read the flag. No refusal
  is raised with `SKIP_FLAG` on them any more.

**Tests** (synthetic; every refusal and answer asserts #518's three counters and the name table unchanged, and the
bounding-cylinder tests also the heap fingerprint and the RNG).

`bounding_cylinder_tests.rs`: the fixture gains `Vector`, `Translation`, the three Absolute flags and `Location`,
the static-mesh and particle classes, a toy cylinder subclass and `BulletListenerComponent`. Its plain primitive entry
is now an `Engine.PrimitiveComponent`. The note's tests:
1. a set flag skips, live and template;
2. a clear one gives the cylinder's box live, and L on a template;
3. another kind is a Script error;
4. the check passes the read classes, refuses four others, and is not asked for a set bit or a non-colliding
   entry;
5. an unrecorded entry is L before the flag;
6. the spawn extent: `placement_extent_tests.rs` and `placement_pickup_tests.rs` (set skips; clear is L's
   `LayerAbsent`; an unread class is the check's `NotRead`), and the spawn path in `world::tests`;
7. Reference unchanged, by the existing binding test.

The tests that expected S now expect the cylinder's box, L or N.

**Not verified:**
- anything in the running game;
- the field's name;
- the per-component check of every class but the three read, and that it has no side effects;
- who sets `BulletListener` on an instance.

The test counts and the slice-gate reading are in the GATES row.

**Review round 1** (Sol xhigh):
- **No partial out write.** The native checks both out destinations before either write, so a bad height destination
  leaves the radius unwritten. A script-caller regression test asserts the radius property unchanged and the four
  counters.
- **The skip flag's declaration.** `actors::declared_flag` requires the declaration to be a bool as well as the
  value. All three paths use it: the live path, the template path and the spawn extent. Each is tested with an int
  declaration holding a bool.
- **Stronger fixtures:**
  - an unrecorded entry with a malformed flag is still L;
  - the spawn extent's clear `CylinderComponent` sits in `Components` with `CollisionComponent` unset.
