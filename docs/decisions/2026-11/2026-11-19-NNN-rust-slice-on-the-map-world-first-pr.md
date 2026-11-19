## 2026-11-19: Rust slice on the map world, first PR: the bridge adopts the map's objects, link 1's group behind --map

AI-assisted (Claude Code). Lane IMPL-SLICE-MAP-WORLD, the first PR of lane 4 in the map-load design
(docs/rust-port/SLICE_MAPLOAD.md, final at fee80eb on `research/rust-slice-mapload`, #530; sections 3 and 4). It is
stacked on lane 3 (`fix/rust-mapload-player`, #547), which is stacked on lane 2 (#537). There was no real-game
session, so every rule is **UNVERIFIED** in the running game. No dependency was added, no bounds check was loosened
and no existing `NotYet` guard was relaxed. Reference is unchanged, and so is every slice route without `--map`.

- **`MissionScript::adopt(world, mission)`** (new, `ow-mission` `bridge.rs`, Off only; Reference `NotYet { NotPorted
  }`). This is the bridge's bind and adopt split (design section 3, steps 1 and 8), next to `MissionScript::new`, which
  is unchanged. It makes none of the graph's objects:
  - it takes the map's `WorldInfo`, `WorldInfo.GRI`, the GRI's `MissionTracker`, the world's first local player
    controller and its `PlayerReplicationInfo`, and the world's globals object (`WorldQuery`);
  - **before the first write** it refuses, typed and unmarked, each of these missing, and any of the four globals
    natives not yet bound by its owner. A class lacking a property it reads is a data error;
  - then it does what the C++ bridge did and the script does not:
    - the playthrough record, logged as a harness input (Q6);
    - the bridge's natives, except the four globals natives;
    - the mission data validation reply;
    - the tracker's hooks.

  A `NotYet` it returns is marked "stopped after partial writes" exactly when one of #518's counters moved since its
  entry. On this route the player's pawn is the controller's possessed `Pawn`, read on each call. It is never made from
  class defaults, and a controller without one is `NotYet { NotRead }`.
- **The four globals natives, one owner each** (design section 3, step 2, the hub's ruling). This lane picks the first
  of the design's two options: the map route's owners are never bound into a bridge world.
  - On the map route, `ow_inventory::globals::register` owns `GetWillowGlobals`, `GetGearboxGlobals` and
    `GetGlobalsDefinition`. The new `ow_behavior::kernel::natives::register_kernel_getter` owns `GetBehaviorKernel`:
    it answers the world's globals' `TheBehaviorKernel` as stored (None, since the kernel is not modelled), and with no
    globals object it is `NotYet { LayerAbsent }`.
  - The getter is not part of the kernel `Pack`, so the old route, where the bridge binds all four, is unchanged.
    `register_native` refuses a second binding of a key, so a successful adopt over the owners' bindings shows that the
    bridge bound none of them.
- **The map route's world** (new, `ow-cli` `cmd/combat/map_route.rs`, `build(cooked, map, notes)`). The steps, on one
  `World`, in this order:
  1. the combat run's world, as today;
  2. the owners above;
  3. the globals: a harness input, as the pickup world makes it, plus the pawn data manager's globals set-up, as the
     old route runs it;
  4. one `Engine.LocalPlayer` from class defaults, a harness input standing for engine start-up;
  5. `load_map`, then lane 2's `bring_up_map_for_play`, with the configured default game, no options and every actor
     routed;
  6. lane 3's local-player spawn, with the map's name as the URL text, the empty id and `ROLE_SimulatedProxy`;
  7. `MissionBridge::adopt` over the old route's stock mission.

  Each harness line goes to the VM's log and to the caller, including the bring-up's failure count and the game info's
  own stops. The bring-up counts its failures and goes on (its own rule): whether a link may pass over an incomplete
  bring-up is Q4, the maintainer's call. A `Login` None with an error text is this route's failure, with the text kept.
- **Drivers behind a flag.** Without `--map` nothing changes.
  - `--combat-run --cooked <dir> --map <name> <step>...` runs the steps on that world. It is Off only: under Reference
    it is a `NotYet`, exit 3. The usage text is unchanged.
  - `--slice-gate --cooked <dir> [--past-errors] --map <name>` runs the `combat-run` group (links 1, 3 to 5, 13 and 14)
    on the map route; the other groups are unchanged. The route's harness lines go to the first combat link's
    `harness`. A refusal while building the route is that link's `setup` stop, graded by its type.
  - The links' scope texts are still the old route's. Changing link 1's scope is the maintainer's call (design section
    7).
- **Tests** (invented packages, `ow-mission` `bridge/adopt_tests.rs`). The world is built the map route's way by hand;
  the possession is a test input (a pawn written to the controller's `Pawn`), not the game's `RestartPlayer`. They
  cover:
  - adopt uses the world's objects, runs the validation reply and logs its one harness line; the player's pawn is the
    possessed pawn on every call, and no pawn is made;
  - with no possessed pawn the accessor refuses, typed, and makes nothing;
  - the one-owner test: all four keys are bound, the bridge binds none, and the owners answer (the world's globals; a
    None kernel);
  - six preflight refusals, unmarked, with #518's counters unchanged;
  - Reference.
- **Installed (under the machine lock).** The numbers are in the GATES row. On Sanctuary the route runs through:
  - the globals;
  - the map load;
  - the bring-up (`WillowGame.WillowCoopGameInfo`, with 1,855 failures; its `InitGame` and `PreBeginPlay` stop,
    marked, at `GotoState('Auto')`);
  - the local-player spawn, into `Login`.

  On this branch alone, `Login` stops at the unbound `WillowGameInfo.IsEditor()` (#508's lane). On a throwaway local
  merge with #508 (`fix/rust-is-editor` @ 13992c2f) it stops at the unbound `Engine.IsSplitScreen()`. That is the same
  native that stops the old route's `maya:8` on that merge, so link 1 reaches the same native on both routes; the map
  route meets it before its driver's first step, at `setup`.
- **Not in this PR:**
  - the `--slice-run` family, and the pickup, equip and phaselock worlds, on the adopt path;
  - the full installed run, a player spawned and adopted on Sanctuary. It needs `IsSplitScreen`, then the auto-state
    work for the GRI and the tracker.
