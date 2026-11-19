## 2026-11-19: Host physics modes for the loaded world (H-PHYSICS-MODES)

AI-assisted (Claude Code, Claude Opus 5.5). Lane H-PHYSICS-MODES, after H-WORLD (decision 2026-11-19-01). Number 22 was
assigned by the hub. There was no real-game session, and nothing below is confirmed in the running game. No dependency
was added, no bounds check was loosened, and no `NotYet` guard was removed: the world's own guards (`World::tick`'s
base condition, the `RemoteRole` variant) are untouched. No package parsing behaviour changed.

- **Why.** H-WORLD's world halted on tick 1, because the host's physics step answered `NotYet` for every mode.
- **The census** (Sanctuary_P loaded in Off, 2,363 placed actors; `world::tests::sanctuary_physics_census`, counts only,
  output under ignored `local/p2/h-physics-modes/`). Modes are read by name from the `Physics` property's enum.
  - `PHYS_None` 2,204. The world never asks physics for these.
  - `PHYS_Interpolating` 146:
    - InterpActor 115 (22 of them with a `Base`);
    - CameraActor 18;
    - DynamicBlockingVolume 4;
    - WillowSpectatorPoint 3;
    - WillowInteractiveObject 2 (1 with a `Base`);
    - HeightFog, LightShafts, SkeletalMeshActor and SkyLightToggleable, 1 each.
  - `PHYS_Walking` 9 and `PHYS_NavMeshWalking` 2, all `WillowAIPawn`.
  - `PHYS_Rotating` 2 InterpActors, each with `RotationRate` yaw 1,820.
- **The Matinee scan** (`world::matinee_bound`, run once after the map load).
  - It loads every `SeqAct_Interp` export (or subclass) in the placed actors' map files: 81 ops.
  - It collects the actors their linked `SeqVar_Object`s hold: 199 actors. Any link counts, not only a group's, so the
    set errs towards "may be driven" (NATIVE_KISMET_MATINEE.md: the op binds its group actors through object-variable
    links).
  - 5 plain object variables are empty at load.
  - The other linked variables are 81 `InterpData`, one `SeqVar_Float` and two Willow day/night variables.
  - **The completeness rule** (review r1 of #420; `MatineeBindings::complete`). The scan is complete only if it read
    everything that could bind an actor. Otherwise every interpolating actor is `Unknown` (`NotYet`).
    - **Variables.** Only a variable of exactly `Engine.SeqVar_Object` is read by its `ObjValue`, and only such a
      variable can be empty. Any subclass (an object list, a group or player variable) is unreadable even when it has
      a value, because it can name actors another way. A class with no `ObjValue` is unreadable (a named or external
      variable, say) unless it is on an explicit list of classes that hold no actor: `InterpData`, the scalar
      `SeqVar_*` and the two Willow day/night variables (`NO_ACTOR_VARIABLES`).
    - **Ops.** An op whose `VariableLinks`, or one of its links' `LinkedVariables`, is not an array is counted.
    - **Classes.** An export class the VM cannot resolve is counted, because it might be a Matinee op. The exceptions
      are five classes named on a list of engine data with no UnrealScript class (`NOT_OP_CLASSES`): `Core.Package`
      2,066, `Engine.ShadowMap1D` 151, `Engine.LightMapTexture2D` 136, `Engine.Polys` 53 and
      `GearboxFramework.WiringMesh` 1 on Sanctuary. Every Kismet op derives from the script class
      `Engine.SequenceOp`, so it resolves. Without the list, the five classes alone would make every interpolating
      actor `Unknown`.
    - On Sanctuary the scan is complete. Its variables are only plain object variables and the listed kinds, so the
      counts below did not change with the stricter rule.
- **The rule per mode** (`world::physics_rule`, installed by `world::install_physics`).
  - `PHYS_None`: nothing.
  - **`PHYS_Interpolating`, an actor no loaded Matinee op names (complete scan, `ParityMode::Off`):** a step that
    moves nothing.
    - In `ParityMode::Reference` it answers `NotYet` (not ported): the C++ reference has no host physics to stand in
      (rule R3).
    - Stock UE3's interpolating mode is the one Matinee drives.
    - The world runs no other driver. It runs no Kismet, so the variables empty at load stay empty.
    - UNVERIFIED in the running game: that the game's interpolating step does nothing else for such an actor
      (velocity, touches, based actors).
  - **`PHYS_Interpolating`, a named actor:** `NotYet`, layer absent: "the Matinee playback that moves a bound
    interpolating <class>". Kismet and Matinee playback are not in the world.
  - **`PHYS_Interpolating`, the scan not complete:** `NotYet`, not read.
  - **`PHYS_Walking` and `PHYS_Falling`:** `NotYet`, not read. The H4 mover does not fit other pawns yet:
    - the walking step itself is not read (NATIVE_PLAYER_MOVEMENT.md, "Not read yet"; the H4 mover's ground step is
      that lane's own);
    - a placed pawn's movement values are not read;
    - its acceleration comes from a controller that does not exist before begin play (MAP-LOAD-C's step);
    - the collision the mover uses lives on the host side of D18, not with the world.
  - **`PHYS_NavMeshWalking`** (a Gearbox mode): `NotYet`, not read.
  - **`PHYS_Rotating`:** `NotYet`, not read. How `RotationRate` x `DeltaTime` becomes rotator units,
    `DesiredRotation`, and a turn that collision blocks are open questions.
  - **Every other mode:** `NotYet`, not read.
- **What tick 1 meets now** (the census forecast in list order, which agrees with the runs):
  - 57 actors pass: interpolating, named by no op, with no `Base`.
  - 23 stop at the world's own `Base` `NotYet`.
  - 66 stop at the Matinee playback.
  - 9 stop at AI walking, 2 at nav-mesh walking and 2 at rotating.
  - The first stop in list order is a Matinee-bound `WillowSpectatorPoint`.
- **Proof.**
  - **Synthetic** (`world::tests`, invented packages):
    - each mode's rule, with the record and kind of each `NotYet`;
    - an interpolating actor no op names ticks three times and stays put;
    - a bound one halts the tick with the playback `NotYet`;
    - a walking pawn halts it with the walking-step `NotYet`;
    - the scan over invented Kismet classes (review r1): a plain object variable binds its actor, an empty one counts
      as empty, and a subclass of it, a named variable, an op with no link array and an unresolved class each make
      the scan incomplete, so the driver is `Unknown`;
    - the no-op answers `NotYet` in Reference.
  - **Installed, headless** (`--map Sanctuary --play-script` with H4-PLAY's 60 s script, desktop, release, under the
    machine lock).
    - **Before** (H-WORLD, main f2d1da1's code): 0 ticks completed, halted at tick 1 on "the host's
      `PHYS_Interpolating` physics for a placed `Engine.InterpActor`".
    - **After, two runs with identical text apart from timings:**
      - 0 ticks completed, `TimeSeconds` 0.016667;
      - halted at tick 1 on "the Matinee playback that moves a bound interpolating `WillowGame.WillowSpectatorPoint`";
      - path digest `d7b95ebfe14df774`, world digest `c233c6faac880f05` (no world value changes before the stop);
      - all twenty segment digests as before.
    - **The tick count does not rise.** Every one of the 100 remaining stops is either the world's own guard or a
      question this lane may not answer without a reading.
- **Open items** (to get past tick 1):
  - Kismet and Matinee playback in the world (66 actors);
  - `World::tick`'s base condition (23);
  - the walking step and an AI pawn's movement inputs (9), and nav-mesh walking (2);
  - rotating physics (2);
  - begin play and the controllers (MAP-LOAD-C).
