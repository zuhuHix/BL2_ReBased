## 2026-11-19: Rust InitializePickupForRBPhysics in Off: the minimal answer, the asset branch at rest, a seeded stand-in stream

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-PICKUP-RB. The file number is a placeholder until the
maintainer gives one. Coded from `docs/verification/NATIVE_PICKUP_RB_MINIMAL.md` alone (analyst lane
ANALYST-PICKUP-RB-MINIMAL, #574, accepted at 7c212484), with the hub's rulings on #574 on top. This lane did not read
the native (clean room). No real-game session was run. No dependency was added and no bounds check was loosened.
**Every rule is UNVERIFIED in the running game.**

**Base.** This branch is stacked on #520 (`fix/rust-attach-mesh-to-pickup-2` @ 4d0539b2), which sits on #511. Its
first commit takes #518's seven write-counter files byte-identical from 1f1a2bcc.

- **What changed** (`crates/ow-inventory/src/attach_mesh/pickup_rb.rs`, new; Off only; Reference binds nothing):
  `WillowPickup.InitializePickupForRBPhysics()` no longer refuses with #520's rigid-body `LayerAbsent`. The old
  refusal (`RB_PHYSICS`, `RB_LAYER`, `initialize_pickup_for_rb_physics`) is removed from `attach_mesh.rs`.
  - **Preflight** (note section 3), write-free and in the note's order:
    1. `bUseRBPhysics`; clear is nothing at all;
    2. C, the `CollisionComponent` when it is a `PrimitiveComponent`;
    3. the pickup's fields of steps 1, 2, 5, 6 and 7, and the timer service;
    4. with C only: G (the world's globals and their definition, with their own refusals), the declarations of C's,
       the pickup's and G's fields, the deferred-reattach check when `CollideActors` is clear, and the
       physics-asset test (`inventory_definition_of`, no load; `PhysicsAsset` read as a handle and not followed).
  - **Writes** (note section 2), in order:
    - `bForceRBToSleep` false, and the timers `MaxRBAwakeTimer` (once, after `MaxRBAwakeTime`) and
      `CheckForRigidBodySleepState` (1.0 s, looping);
    - `RotationRate` zero (fitted);
    - with C: `Scale3D` (1, 1, 1), `BlockRigidBody`, `CollideActors` with `bNeedsReattach` when it was clear,
      `BlockActors` false, the asset branch's three fields;
    - with C and G: the three impact fields;
    - with C: `bNotifyRigidBodyCollision`, then with G the collision threshold (× 0.25, truncated);
    - with C: `RBChannel` and the nine channel bits, as one write that keeps every other bit;
    - `Physics` = `PHYS_RigidBody`;
    - with C: the spin's draws, or the temporary torque's clear;
    - `EnableRagdollCollision` (0.3 s, once).

    Enum values are read by name from the declared enums.
- **The hub's rulings on #574.**
  - **The asset branch is answered at rest.** `PhysicsWeight`, `PhysicsAsset` and `bHasPhysicsAssetInstance` are
    written, and no rigid body is made. The run's log says so ("answered at rest ... UNVERIFIED"). This replaces the
    note's refusal 1.
  - **The spin draws on a new stream.** `GameRng::engine_seeded` is a port stand-in for the engine's seeded
    generator: a `CrtRand` seeded from `ENGINE_SEEDED_ID` = 0x200, after every `CrtStream` and `ScriptStream` id, so
    no other stream's seed moves. It is **UNVERIFIED**: only the three-draw count is claimed. The engine's constants
    and seed were not read, and the values feed only an impulse the port does not apply. With G and `bTorque`
    clear, there are three draws; with `bTorque` set, none. This removes refusal 6.
  - **Refusals 2 to 5 stay**, with the note's texts, all in the preflight and unmarked:
    - 2: `NotRead`, no physics asset;
    - 3 and 4: #520's reattach texts;
    - 5: `NotRead`, an unloaded definition.
- **Partial writes.** Nothing refuses after the first write. A `NotYet` would be marked "stopped after partial
  writes: " only when #518's counters (heap writes, draws, load transitions) moved since entry.
- **Left out** (the empty physics layer, as the hub ruled): the filter updates, the body's wake and position, the
  mode change's own physics work and the impulses.
- **Landing conflict with #518.** `crates/ow-core/src/rand.rs` here is #518's file plus the new stream. When #518
  lands first, this branch's rand.rs conflicts with it. To resolve: take #518's version, then re-add four things —
  the `engine_seeded` field of `GameRng`, the `ENGINE_SEEDED_ID` constant, its line in `from_world_seed`, and the
  `engine_seeded()` accessor with its term in `draws()` — plus the test
  `the_engine_seeded_stream_moves_no_other_stream`. The other six counter files are byte-identical to 1f1a2bcc and
  merge cleanly.
- **Tests:**
  - `attach_mesh/pickup_rb_tests.rs`, 10 new, on invented packages, with enums in orders unlike the game's: the skip;
    the no-component path in its two cases; the asset branch at rest with every write and three draws only on the
    stand-in; the torque path; refusal 2 four ways; refusal 5 without a load; refusals 3 and 4 and the flag-only
    owner cases; the globals refusal before any write; Reference.
  - In `attach_mesh_tests.rs`, the old `LayerAbsent` test is replaced by the skip, plus a data error on that toy.
  - `rand.rs`: 1 new.
- **Installed slice gate (Off, strict):** see `docs/verification/GATES.md`. This is what the host did, not parity.
- **Open:** the note's open questions: `InitializeRBPhysics`, the reattach's owner update, the matched entries'
  side effects, `RotationRate`'s constant, and the engine generator itself.

## Edits: review r1 of #587

- **No class load in the preflight.** G is looked up with `globals::globals_definition_loaded`, which resolves both
  classes through the new `globals::ready_class`: a defined layout and loaded code (`Vm::class_code_loaded`, the same
  read-only query #579 adds, applied byte-identically from 2a428257, so it merges with #579). `Vm::find_class` could
  register functions and states for a class whose layout was there. A cold class is a `NotYet { NotRead }` before any
  write. The globals refusals are unchanged; `globals_definition` keeps its behaviour for its other callers.
- **The whole commit is checked first.** The preflight plans every step. Each write's slot is read at its address and
  its value checked by the heap's own write rules. The timer borrow is taken before the first write.
- **Fields only on their branch.** The reattach fields are checked only when `CollideActors` is clear, and the spin
  range only on the spin's branch. The note's refusal order is kept.
- **Tests:** 14 in `pickup_rb_tests.rs`, 4 of them new: a cold class refused and left cold; a borrowed timer service;
  a missing destination slot; the branch-only checks. The channel test now starts with the seven bits clear and an
  unrelated bit set. Every refusal compares all three of #518's counters.
