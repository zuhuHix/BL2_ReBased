## 2026-11-19: Rust slice gate: the Phaselock cast spawns its actors through the world

AI-assisted (Claude Code). Lane LINK-9-CAST-SPAWN. Lane c376 found that link 9 of `ow-package --slice-gate` still
stopped at `GearboxPawn.GetSMComponent` after #435 (decision 2026-11-19-44). The cast made its pawns with
`instantiate_from_defaults`, which copies the class defaults' slots and shares their components, instead of spawning
them. No native code or analysis output was opened, and there was no real-game session: the rules the spawn applies
keep their own labels (UNVERIFIED). No dependency was added, no bounds check was loosened and no `NotYet` guard was
relaxed.

- **What changed** (`crates/ow-cli/src/cmd/slice_gate.rs`, link 9 only):
  - The cast's three actors (Maya's `WillowPlayerPawn`, her `WillowPlayerController` and the target `WillowAIPawn`)
    are now spawned through the world's own `ow_world::actors::spawn`. The level's `WorldInfo` is the spawner and the
    request is plain: no template, no owner, the spawner's place. So each actor gets its own copy of its components,
    as a spawned actor does (decision -44).
  - Nothing is made by hand and no component is faked. A spawn the world refuses (`None`) is a typed error; a rule
    the spawn has not read is its own `NotYet`, and that is the link's stop.
  - Link 9 now has two steps, `spawn` then `cast`, so a stop shows which of them it came from. The link's scope text
    says the actors are spawned.
  - Reference is unchanged: the gate runs in Off only.
- **Installed `--slice-gate`** (pc-hummer, main bb46e113; rows under ignored `local/`):
  - Before: link 9 `notyet`, step `cast`: `GearboxPawn.GetSMComponent on a pawn whose SMComponent is not its own
    instance` (`NATIVE_SPECIAL_MOVE_COMPONENT.md`).
  - After: link 9 `notyet`, step `spawn`: `the spawn placement spot search: needs collision (the world's collision
    hash is not modelled)` (`NATIVE_ENGINE_CORE.md`, "Spawning and lifetime").
    - The pawn collides with the world (`bCollideWorld`), and the spawn's placement spot search needs a collision
      layer this world does not have. It is the spawn's own `NotYet`, unchanged.
    - Asking the spawn to skip it (`bNoCollisionFail`) would be a rule nobody read for these spawns, so it is not
      done.
  - Strict 0/14/0 and past-errors 0/14/0, before and after; only link 9's row changes.
- **Tests:**
  - `ow-world`, `a_plain_spawn_gives_each_actor_its_own_class_default_component`, on invented packages: a plain spawn
    from the class defaults gives each actor its own copy of the class default's component, outered to it, and the
    class default keeps its own.
  - `ow-cli`'s slice-gate tests pin link 9's two steps.
- **Review r2 of #454.**
  - **Committed spawns** (item 1). `phaselock_actors` counts the spawns it has committed. A `NotYet` after one has
    succeeded is marked "stopped after partial writes", its kind and record kept. The earlier actor stays registered,
    nothing is rolled back, and other errors pass unchanged.
  - **Typed grading** (item 2). Link 9's driver (`drive_phaselock`) grades a step from its typed error: a
    `VmError::NotYet` is the not-yet stop, whatever its text. Any other spawn error is a hard stop under both gradings,
    because the cast cannot run without its actors. A cast error follows the grading as any step's does.
  - **Tests** (item 3). `slice_gate_driver_tests.rs`, on invented packages, 4 tests:
    - all three actors spawned and registered, each with its own component;
    - a refusal before the first spawn: an unmarked `NotYet`, no cast;
    - a `NotYet` after a committed spawn: marked, its kind kept, the pawn still registered, no cast;
    - an ordinary spawn error is a hard stop in both gradings.
