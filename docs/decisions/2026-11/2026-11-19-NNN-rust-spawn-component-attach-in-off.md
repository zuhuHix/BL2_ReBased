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
  - `ow-inventory` `attach_mesh.rs` (`REATTACH_OWNER`): reachable only for an attached component with `Owner` set,
    and the step attaches only the two AI classes.

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
- **Not covered by a test:** the note's "a script `GainedChild` or `PreBeginPlay` that reads the component's `Owner`
  sees the actor". The writes' place, before `set_owner` and the begin-play events, gives it.
