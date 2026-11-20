## 2026-11-19: The spawn's component attach in Off

AI-assisted (Claude Code). Lane IMPL-SPAWN-COMPONENT-ATTACH, slice link 9. The decision number is pending (the landing
run numbers it). Coded from docs/verification/NATIVE_SPAWN_COMPONENT_ATTACH.md alone, the note as final at bc83e3b6
(PR #605; clean room: lane ANALYST-SPAWN-COMPONENT-OWNER wrote it), and the hub's rules on its review r1. The executable
and the analysis output were not opened. Synthetic fixtures only. Based on main c687c135.

- **What changes:** `ow-world`'s spawn runs the spawn's component update in Off (`actors::component_attach`), in two
  parts. Reference is unchanged.
  - **The preflight** runs in `spawn_from`'s pre-make block, after the instancing plan and `load_own_definitions` and
    before `vm.instantiate`. It reads the source's `Components` mapped through the instancing plan, so a refusal leaves
    no actor. It writes, loads and draws nothing (`vm.space.prop` and `vm.space.read` only). In order:
    1. `Components` and `AllComponents` must be declared as arrays (else `VmError::Script`);
    2. `AllComponents` must be empty (else `NotYet { NotRead }`);
    3. each non-None entry, in order:
       - must be this spawn's own copy (else `NotYet { NotRead }`, the note's text);
       - must be resident (else `NotYet { NotRead }`);
       - must declare the six `ActorComponent` fields (else `VmError::Script`);
       - must have `bAttached` false on the template component (else `NotYet { NotRead }`);
       - must not be an `AIComponent` subclass other than the two exact classes (else `NotYet { NotRead }`);
    - must not be a component already listed: the same entry twice, or an inherited reference next to the own
      sub-object it maps to, refuses `NotYet { NotRead }`, "listed twice" (the hub's ruling on #610 r1);
    4. the scene layer must be present, asked for only when an entry is to be attached (else
       `NotYet { LayerAbsent }`, with `Actor.AttachComponent`'s text).
  - **The writes** go in `after_insertion`, between `Rotation` and `set_owner`, before and after play began, on this
    spawn's copies in `Components` order:
    - every copy gets the generic attach: the two update flags false, `Owner` the actor, `Scene` left at its zero
      value, and `bSkipChildComponentUpdate` false;
    - only exactly `GearboxFramework.AIComponent` and `WillowGame.WillowAIComponent` also get `bAttached` true and are
      appended to the actor's `AllComponents`.

    Nothing is loaded or drawn.
- **The scene layer (the hub's ruling):**
  - `SceneLayer`, `add_empty_scene` and the `Actor.AttachComponent` texts moved from `ow-inventory` (`attach_mesh`) to
    `ow-world`, the lowest crate both reach with no new crate edge (`crate_edges_test.py` OK). `ow-inventory`
    re-exports them, so its behaviour and paths are unchanged.
  - The standard Off world set-up, `World::new`, installs the empty layer and logs it as an UNVERIFIED harness input.
    The map load, slice, combat and `phaselock_world` worlds are all built on it.
  - A bare VM keeps `NotYet { LayerAbsent }`.
- **What else reads the new `Owner`:**
  - `ow-combat` `holds.rs`: `HoldMovement` on an AI component with `Owner` set is the terminal `NotYet { NotRead }`. A
    test shows the change.
  - `ow-inventory` `attach_mesh.rs` (`REATTACH_OWNER`, corrected on review r1 of #610). The deferred reattach's
    preflight refuses whenever `Owner` is set, whatever `bAttached` holds. `SetForceRefPose`, `SetHidden` and
    `SetOwnerNoSee` go through it, so on a spawned actor's own component copy they now refuse `REATTACH_OWNER` before
    any write (`SetHidden` and `SetOwnerNoSee` only when the value would change: an unchanged flag returns first). That holds for a primitive copy too, which keeps `bAttached` false. Only `SetLightEnvironment` checks
    `bAttached` first: an unattached copy takes its write, and an attached one (an AI copy) meets the owner refusal.
    A test shows the `SetForceRefPose` case on a world-spawned copy.

  No installed slice row reaches either (below).
- **Ripple in existing tests:** 124 tests failed only because their invented `Actor` or component classes did not
  declare the arrays or fields the step reads. They were fixed in the six fixture builders, with no assertion changed:
  - `ow-world` `world/tests.rs` (`AllComponents`, and `ToyComponent`'s six fields);
  - `ow-world` `pools/tests.rs`;
  - `ow-inventory` `spawn/tests.rs`;
  - `ow-skills` `action_skill/manager/tests.rs`;
  - `ow-cli` `slice_gate_driver_tests.rs`;
  - `ow-combat` `run/fixture.rs`, which the run, loot, hit, damage and `--combat-run` tests share.

  `ow-combat`'s holds fixture gained the five fields for the new HoldMovement test.
- **Verified:**
  - host unit and spawn-level tests on invented classes. Every refusal leaves #518's counters unchanged, and at the
    spawn it leaves the level list and the heap as they were;
  - the installed Off strict slice gate is identical to main's, row by row.
- **Not verified:** the game's own rule. Nothing is checked against the running game, so every rule is
  **UNVERIFIED** in game.
- **Tests through the world's spawn** (review r1 of #610). In `ow-combat` (`holds/spawn_attach_tests.rs`), because it
  reaches the spawn, the deferred reattach and the hold kinds:
  - `SetForceRefPose` on a spawned primitive copy refuses `REATTACH_OWNER`;
  - `PreBeginPlay` after play began sees the copy's `Owner`;
  - `HoldMovement` on a spawned AI copy is terminal;
  - an `AIComponent` subclass other than the two refuses, and a bare VM without the layer refuses `LayerAbsent`, each
    leaving no actor;
  - an actor with no components spawns with no layer.

  The unloaded-handle refusal is not reachable through the spawn: the instancing plan loads each template component
  before this step, and refuses a stand-in with no slots itself. The unit tests cover it.
