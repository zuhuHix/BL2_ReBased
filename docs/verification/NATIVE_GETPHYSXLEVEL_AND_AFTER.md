# maya:8 after InitializeRBPhysics: Engine.GetPhysXLevel and the PhysX-effects set-up (2026-11-26)

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-GETPHYSXLEVEL-AND-AFTER (decision 2026-11-26-24). This is the
analyst half of the clean room: **another lane implements this note.** Raw output stays under ignored
`local/analysis/physx/`. The note is in our own words: no decompiler output, pseudo-code, addresses, offsets or listing
structure. **Every rule here is UNVERIFIED in the running game.**

**Why.** NATIVE_LINK1_RIGID_BODY_LAYER.md (#763, in review) predicts that `maya:8`'s next stop, after
`InitializeRBPhysics`, is the unbound `Engine.GetPhysXLevel`. `WillowPawn.SetupSkeletalMeshPhysicsForPhysXEffects`
calls it first. This note reads:
- the native;
- where its value comes from;
- what the function does at each level;
- `maya:8`'s path after it, as far as `InitializeBehaviorProviders`.

## 1. Evidence

- **bl-sdk live reads:** none. No session was granted.
- **The script** (`research/script_disasm.py`):
  - `WillowPawn.SetupSkeletalMeshPhysics` ends with `InitializeRBPhysics`, then
    `SetupSkeletalMeshPhysicsForPhysXEffects`, then returns;
  - `SetupSkeletalMeshPhysicsForPhysXEffects` and `WillowPawn.ApplyBodyClassStartingValues` are read in full, and so
    is `WillowPawn.CheckPainSounds`.
- **The natives:** Ghidra 12.1.4, headless and read-only, under the machine lock. Read:
  - `Engine.GetPhysXLevel`'s exec;
  - every reader and writer of the setting it reads;
  - the engine's start-up routine that copies the setting;
  - the PhysX SDK start-up that clamps it;
  - the system-settings name tables, which list `PhysXLevel` with the other video settings;
  - `SkeletalMeshComponent.SetHasPhysicsAssetInstance`'s exec and the routine it calls;
  - `NxForceFieldComponent.DoInitRBPhys`'s exec.
- **Field places:** the local copy of `tools/ghidra/class_layout.py`. The field the exec reads lands exactly on the
  declared `Engine.PhysXLevel`, with no shift (engine classes carry none).
- **Configuration:** the install's shipped engine configuration, and this machine's player configuration folder. Only
  the one value below was read; neither file is copied.

## 2. The natives

### 2.A `Engine.GetPhysXLevel()` (static final; returns an int)

**What the game does:**
- it returns the engine object's `PhysXLevel` field. That is an int, flagged transient and **not** config, so it is
  never loaded from a package or from the engine's own config section;
- the engine writes the field once, at the end of its start-up, from the **system settings'** `PhysXLevel`. That is
  the `[SystemSettings]` entry, listed in the settings tables beside the texture quality, shadow cascades, view
  distance, decal count and frame-rate lock;
- the PhysX SDK start-up clamps the same setting into 0 to 2. At 0 it logs that GPU support is off and asks for no GPU
  heap. Above 0 it logs that it is on, and reads the GPU heap and mesh cache sizes from the engine config;
- the clamped range is 0, 1 and 2. By fit, these are the video options' PhysX effects choice (low, medium, high).
  The options menu fills its list from this setting; the menu itself was not read.

**No hardware query** picks the level: it is the player's setting. The SDK start-up checks only that the PhysX
libraries load, and quits with a message box if the cooking library does not. That check is at every level.

**What a PC answers:**
- **per machine and per player.** It is the player's own `[SystemSettings] PhysXLevel`, else the shipped default;
- **the install's shipped default is 0**;
- this machine's player configuration folder holds saves only, with no engine configuration that would override it.
  A first launch would start from the shipped default;
- **not observed in the running game** (no session). Live check 1 confirms it.

**Not read:** the order of the SDK start-up and the engine's copy, so whether the engine's field is always the clamped
value. Also whether changing the option in a running game writes the engine's field again. An instruction search found
other writers at the same field place, on other classes; those were not followed.

**Proposed Off answer: a harness setting, read from the engine object, logged and UNVERIFIED.**
- **Set-up** (world set-up, before any flow runs, as `register_offline_engine` does for the online slot):
  - the slice worlds' engine object gets `PhysXLevel` from a harness setting. The default is **0**, the shipped
    default; the setting's name is the implementer's choice;
  - the setting must be 0, 1 or 2 (the game's clamped range), else the set-up refuses. The declaration (an int on
    `Engine`) is checked before anything is written;
  - the set-up logs one line naming the value and that it is a harness setting, UNVERIFIED. The game's value is the
    player's own setting.
- **The native** reads only: no write, no load, nothing interned:
  - the engine object (the `EngineObject` service) must be present, else a `NotYet { LayerAbsent }` naming the engine
    object, as `GetOnlineSubsystem` does;
  - `PhysXLevel` must be declared as an int on the engine object's class, else `VmError::Script`;
  - it returns the field.
- **Why level 0 for the slice:**
  - it is the shipped default, so it is what a fresh install answers;
  - it is the only level the port can carry today without a physics layer (section 3);
  - levels 1 and 2 change only physics effects. Nothing on Maya's slice path depends on them.
- **Reference** binds nothing (the stub policy answers).

### 2.B `SkeletalMeshComponent.SetHasPhysicsAssetInstance(bool bHasInstance, optional bool bUseCurrentPosition)`

Reached only at a level above 0 (section 3). What the game does:
- **the request matches the instance flag** (`bHasPhysicsAssetInstance` already as asked): nothing else; the flag is
  written with the same value;
- **false to true:**
  1. unless `bUseCurrentPosition`, a pose refresh first (by fit, the skeletal pose update: the routine was not read);
  2. the articulated set-up (the component's virtual that #763 names for the physics-asset instance), passed
     `bSkelCompFixed`. It makes the physics-asset instance: a body per physics-asset body, and the constraints between
     them, in the physics scene;
  3. then the flag is set;
- **true to false:** the articulated tear-down (by fit, the matching virtual: not read), then the flag is cleared.

**Proposed Off answer** (for a later lane; level 0 never reaches it):
- **preflight**, write-free and load-free:
  - `bHasPhysicsAssetInstance` and `bSkelCompFixed` declared as bools, else `VmError::Script`;
  - the receiver resident, else `NotYet { NotRead }`;
- the request matching the flag: nothing written (the game's own rewrite of the same value is not modelled);
- false to true: the physics `LayerAbsent` before any write. The instance needs the physics scene and the bodies. The
  pose refresh, if it were run first, would be a write, so the refusal comes first and needs no partial-writes prefix;
- true to false with the flag set: `NotYet { NotRead }` (the tear-down was not read). The port never sets the flag, so
  this does not arise on its own path.

### 2.C `NxForceFieldComponent.DoInitRBPhys()`

Reached only at a level above 0, on the player's branch (section 3). The exec only calls the component's own
virtual that makes the force field in the physics scene. **Not read further.** Proposed Off answer: the physics
`LayerAbsent`, nothing written.

## 3. `WillowPawn.SetupSkeletalMeshPhysicsForPhysXEffects` at each level

The function's only test of the level is **greater than 0**, so levels 1 and 2 do the same here.

**At level 0:** the function returns after the `GetPhysXLevel` call. **Nothing is written.** Cost: one bound native,
reading one field.

**At level 1 or 2**, in order:
1. if the pawn's `Mesh` has the bug-morph skeletal mesh and the pawn's `Role` is the authority's, it returns.
   - the test names a mesh in another character package as an object constant. Whether the port resolves that
     constant without a load is **not read**; it may be a cost of its own;
   - Maya's mesh is `Skel_SirenBody`, so she goes on;
2. `Mesh.SetHasPhysicsAssetInstance(true)` (2.B): Maya's body mesh has the flag clear, so this is false to true. That
   means her physics asset `Phat_Siren`'s bodies and constraints;
3. `Mesh.SetRBChannel(1)` (bound; Off has its own rule, NATIVE_LINK1_TAIL_SETTERS.md 2.D);
4. `Mesh.SetRBCollidesWithChannel(11, true)`. The port binds it only as the combat pack's presentation no-op;
5. by the pawn test:
   - **not a player's pawn:** `SetRBCollidesWithChannel(7, true)`;
   - **a player's pawn (Maya, with a player's controller):** for each foot, left then right:
     - a new `NxFootForceFieldComponent` (the component `New`, NATIVE_NEW_COMPONENT_INSTANCING.md);
     - attached, at the bone `L_Foot` (then `R_Foot`), by `SkeletalMeshComponent.AttachComponent` to the mesh the
       virtual `GetSkelMesh()` returns (not `Mesh` directly; review of #767). The port binds neither;
     - `DoInitRBPhys` on it (2.C).

**What a level above 0 costs the port:**
- a physics layer: scene, articulated bodies, force fields;
- a pose refresh;
- the bone attachment, and `GetSkelMesh()` (not read, no binding);
- two component instances;
- the cross-package constant.

None of these exists today. Each would be a stop, and the first, 2.B's, is reached by every pawn with a mesh.

Elsewhere in the game, levels 1 and 2 may differ (effects content: not read). This function does not tell them apart.

## 4. After it: `maya:8` at level 0 (a prediction, not a run)

`SetupSkeletalMeshPhysics` returns to `WillowPawn.ApplyBodyClassStartingValues`, which goes on:
1. `Mass` from the body class (a plain assignment);
2. at the authority's role with a body class: `bAlwaysRelevant` from `bPawnIsAlwaysRelevant`;
3. `CheckPainSounds`: script only. It walks the body class's `PainDialogLines` and sets five bools on the pawn, one per
   kind of line. No native;
4. `CrouchHeight` from the body class (a plain assignment);
5. **`InitializeBehaviorProviders`** (bound in Off, NATIVE_INITIALIZE_BEHAVIOR_PROVIDERS.md section 2). For Maya, that
   note's preflight 7 meets `PlayerEvents_Siren`'s conditional sequences: **the pass-2 `LayerAbsent`, with nothing
   written**, and `ConsumerHandle` still -1.

**Predicted next stop:** step 5's pass-2 `LayerAbsent`. Two caveats:
- the links' measured stops are earlier today (#762's measurement: the colliding bounds in `GetBoundingCylinder`);
- this order assumes #763's rule and this note's are implemented.

Past it, NATIVE_LINK1_RBPHYSICS_AND_AFTER.md section 3 lists what follows: `SetDeathDefinition`, the provider
registration, then the player part.

## 5. Tests (synthetic fixtures, for the implementing lane)

1. The engine object with `PhysXLevel` 0: `GetPhysXLevel` answers 0, nothing written.
2. The same with the field set to 2: it answers 2.
3. No engine object: the `LayerAbsent` naming the engine object, nothing written.
4. An engine class with no `PhysXLevel`, or with a float `PhysXLevel`: `VmError::Script`, nothing written.
5. Set-up with the setting at 1: the field is written 1 and one log line names it. With 3: refused, nothing written.
6. A synthetic `SetupSkeletalMeshPhysicsForPhysXEffects`-shaped function at level 0: it returns, and a recording
   `SetHasPhysicsAssetInstance` is not called.
7. The same at level 1: the recorder is called once with true. The gate is "greater than 0", not "equals 2".
8. `SetHasPhysicsAssetInstance(true)` with the flag already set: nothing written. With the flag clear: the physics
   `LayerAbsent`, the flag still clear.

## Open questions

1. **UNVERIFIED:** every rule above, and what this machine's game would answer.
2. **Not read:**
   - the start-up order (clamp before copy);
   - a mid-session option change;
   - the pose refresh, the articulated set-up and tear-down;
   - the force field's set-up;
   - the object constant's resolution in the port;
   - what levels 1 and 2 change elsewhere.
3. **The hub's call:** the harness setting's name, and whether the slice harness should refuse levels above 0 outright
   instead of running into 2.B's `LayerAbsent`.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. In a running game, read the engine object's `PhysXLevel`. Expect the player's `[SystemSettings] PhysXLevel`, 0 on
   a fresh install.
2. Change the PhysX effects option and read the field again: does it change without a restart?
3. After Maya spawns at level 0, read her body mesh's `bHasPhysicsAssetInstance`. Expect false, with no
   `NxFootForceFieldComponent` among her components. At level 1 or 2, expect true and two force fields.
