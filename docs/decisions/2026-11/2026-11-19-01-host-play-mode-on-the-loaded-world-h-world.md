## 2026-11-19: Host play mode on the loaded world (H-WORLD)

AI-assisted (Claude Code, Claude Opus 5.5). Lane H-WORLD, step 6 of the slice run plan
(`docs/rust-port/SLICE_RUN_PLAN.md`). Number 01 was assigned by the hub. There was no real-game session, so nothing
below is confirmed in the running game. No dependency was added, no bounds check was loosened and no `NotYet` guard was
removed. No package parsing behaviour changed.

- **What changed.**
  - New: `crates/ow-host/src/world.rs` and `world/tests.rs`, plus one `pub mod world;` line in `lib.rs`.
  - `play.rs`: `PlayWorld::with_world`, `WorldStats`, `run_headless_on` and the world's lines in the stats text.
  - `sim.rs`: `Snapshot::pawns`.
  - `anim/pawns.rs`: `load_archetype`, `world_placement`, `PawnLayer::place_world`.
  - `args.rs` and `main.rs`: `--no-world` and `--world-pawns DIR`.
- **What the play mode does now.**
  - `ow-host --play` and the headless `--play-script` load the map as a world before the run: W5-MAPLOAD's
    `load_map` into an `ow_world` world, in `ParityMode::Off`.
    - The world has the Core natives, the behavior handlers and the INT localization roots, as `ow-cli --slice-run`
      builds its VM.
    - The balance inputs are a host choice (normal playthrough, one player, level 1), as `ow-cli`'s combat run uses.
      The game reads them from the save, and the play mode has no save yet.
  - **One world tick per play step:** `World::tick(DEFAULT_STEP_SECONDS)`, after the mover. So `WorldInfo.TimeSeconds`
    and `DeltaSeconds` follow the simulation's fixed 1/60 s step, never the frame times.
  - `--no-world` runs on the scene alone, as before.
- **Where the world runs.** `World` is `!Send` (D18), and the VM wants its deep stack. So the world is built and ticked
  on one thread with `ow_vm::interp::STACK_SIZE`, and the play loop sends each tick and waits for the answer.
  - Only plain data comes back: the time fields and the placed pawns.
  - A run stays a function of its script, and the window and the headless runner tick the world alike.
- **When a tick fails, the world halts.**
  - The error and its tick are kept, and no later tick runs. A world stopped partway through its actor pass is not a
    state to build the next tick on.
  - The walk goes on, because the pawn's movement is the host's.
  - The time fields are still read back after a failed tick: the world advances its clock before the actor pass.
- **The host's physics step** (`World::set_physics_step`) answers `NotYet` for every mode, naming the actor's class
  and its mode.
  - The H4 mover is the player pawn's walking and falling. It is not applied to other actors: a placed pawn's movement
    parameters and its controller's acceleration are not read.
  - The world has no player pawn before the local-player step that W5-MAPLOAD leaves pending.
- **Pawns.**
  - A placed actor whose class derives from `Engine.Pawn` goes out with each snapshot: its path, class, archetype (its
    export's archetype reference), `Location` and `Rotation`.
  - With `--world-pawns DIR`, the drawer loads each pawn's look from its archetype through the scene's own store
    (H-PAWNS' path) and places it from the world's transform each frame. It never reads from or loads into the world's
    object space.
  - UNVERIFIED: that a placed actor does not override its archetype's mesh component.
- **What stays `NotYet`, reported and not touched:**
  - W5-MAPLOAD's three pending steps: the begin-play pass, the game info and the local player (NATIVE_MAP_LOAD.md).
    The pc-hummer lane implements them; this lane does not run them or stand in for them.
  - **On Sanctuary, tick 1 halts.** The first stop is the host's `PHYS_Interpolating` physics for a placed
    `Engine.InterpActor`.
  - **The census of the loaded world (2,363 actors, Off)** shows what stands behind that stop. Physics modes are read
    as the stored byte; only 7, `PHYS_Interpolating`, was named by the installed halt message.
    - 145 actors with `Physics` 7, interpolating: InterpActors, cameras, blocking volumes and others.
    - 2 InterpActors with `Physics` 5.
    - 9 `WillowAIPawn` with `Physics` 1 and 2 with `Physics` 12.
    - 23 actors with physics and a `Base`, which `World::tick` answers with its own `NotYet` (the base condition was
      not read).
    - No actor is in a state, because no begin-play ran.
    - No placed actor has `RemoteRole` 2, so the tick variant `NotYet` is not met.
- **Proof.**
  - **Synthetic** (`world::tests`, invented `Engine` and map packages):
    - the map loads on the world thread, with its pawn and the three pending `NotYet`s;
    - six ticks give `TimeSeconds` as the f32 sum of the steps and `DeltaSeconds` as the step;
    - a `RemoteRole` 2 pawn halts tick 1 with the `NotYet`, and a halted world runs no more ticks;
    - the physics step's `NotYet` names `PHYS_Walking` and the class, and the time fields are read back after the
      failed tick;
    - a builder error is returned by `spawn`.
  - **Synthetic** (`play::tests`, an invented world):
    - one tick per step, each by the step, at uneven and steady frame times, with the same world digest for both;
    - the path digests are unchanged by the world;
    - a halt stops the ticks and the walk finishes;
    - the snapshot carries the pawns;
    - the `args` tests cover `--no-world` and `--world-pawns`.
  - **Installed, headless** (`--map Sanctuary --play-script` with H4-PLAY's 60 s script, desktop, release, under the
    machine lock; output under ignored `local/p2/h-world/`):
    - the world loads 2,363 actors, 0 refused, over 10 levels, in about 9 s;
    - 11 placed pawns, each with its archetype;
    - 3,600 ticks asked, 0 completed, halted at tick 1 as above, `TimeSeconds` 0.016667.
    - Two runs gave the same text except for timings: path digest `d7b95ebfe14df774` (H4-PLAY's baseline, the same
      with `--no-world`), world digest `c233c6faac880f05`, and all twenty segment digests.
  - **Installed, window** (`--play --play-script ... --size 1280x720 --screenshot --no-vsync --world-pawns <four
    UModel export folders>`; RTX 4070, Vulkan; 14.5 GB of free commit memory before the run):
    - exit 0;
    - the same path digest, world digest and twenty segment digests as headless;
    - 32,917 frames, median 1.724 ms.
    - **One of the 11 pawns is drawn** (Tannis, `Skel_Tannis`, 114 bones). The other ten are refused by H7's clip
      library: a shared AnimSet has tracks for bones their meshes lack (`L_CoatTail01`; `Bone_PCordA` for Claptrap).
    - Neither screenshot frames Tannis: she stands off the scripted path. So the pawn draw is shown by the run, not by
      a picture.
  - **UModel** (build 1590, under the lock): `-export -psk -notex -nostat` of Sanctuary_P, Sanctuary_Px,
    Sanctuary_Dynamic and Sanctuary_Combat, about 1 s each, 53 MB of meshes and AnimSets under ignored `local/`.
- **Open items.**
  - The host physics per mode: interpolating (Matinee), rotating, and the AI pawns' walking.
  - `World::tick`'s base condition.
  - The begin-play pass, the game info and the local player (the pc-hummer lane). Once those are in, the world's
    player pawn can be driven by the H4 mover.
  - H7's clip library refuses AnimSets with tracks for bones the mesh lacks. How the game maps tracks to bones is not
    read.
  - A screenshot that frames a world pawn.
  - The pawns' materials.
