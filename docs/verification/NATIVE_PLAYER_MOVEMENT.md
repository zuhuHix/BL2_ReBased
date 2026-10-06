# Native player movement and camera: Maya walking Sanctuary (2026-10-06)

AI-assisted (Claude), analyst lane G16. Read from the game executable with Ghidra (tools/ghidra/), written in our own
words; no decompiler output, pseudo-code or listing structure is reproduced here. **Every rule is UNVERIFIED in the
running game** unless a line says how it was confirmed. Script rules were read from the installed packages' script
(`research/script_disasm.py` listings); data values were decoded from the installed packages with `ow-package` and are
exact copies of cooked values (still UNVERIFIED as "what the running game ends up using"). Units are Unreal units (uu);
the host (UE5) uses centimetres and currently treats 1 uu as 1 cm (the Walker already scales gravity to the installed
ini's -500). Lines marked **UE5 (ours)** are our own mapping suggestions, not game facts.

Scope and method. Script first (WillowPlayerController, WillowPlayerInput, WillowPlayerPawn, WillowPawn, Engine.Pawn,
Engine.PlayerController), then the natives the script cannot show: the pawn's speed fraction, velocity step, falling
step, eye-height/bob update, first-person camera and the FOV helpers. Where no script name exists (physics steps, the
virtual camera and velocity routines) the function was found through the pawn class's C++ virtual table and is named
here by what it does.

## Summary

| Native / rule | Script signature (from the package) | Slice relevance | Confidence | Status |
|---|---|---|---|---|
| Maya movement data (class, pawn, volume, globals) | data, not a native | Walk speed, jump, air control, crouch, eye height | high (cooked values) | UNVERIFIED in game |
| Attribute-driven speeds (GroundSpeed, AirSpeed, JumpZ, FootSpeed) | attribute properties on Pawn; `FootSpeed` resolves to `GroundSpeed` | Sprint and skills change speed | high | UNVERIFIED |
| Pawn speed fraction (virtual, no script name) | called by the velocity step and anim-rate code | Walk key, crouch, sprint, encumbrance | high (formula), medium (two Willow fields) | UNVERIFIED |
| Velocity step (virtual calc-velocity) | no script name | Every ground and air-steered move | high | UNVERIFIED |
| Braking (virtual, no input) | no script name | Stopping feel | high | UNVERIFIED |
| Max speed by physics mode | `Pawn` helper, no script name | Flying, swimming, ground | high | UNVERIFIED |
| Falling step (air control, gravity, terminal velocity) | no script name | Jump, falling | medium-high | UNVERIFIED |
| Gravity getters | no script name | Jump arc | medium | UNVERIFIED |
| Player move input to acceleration | `PlayerController.PlayerWalking.PlayerMove / ProcessMove` (script) | Walking | high | UNVERIFIED |
| Sprint entry and exit | script: `WillowPlayerInput.SprintPressed / PlayerInput`, `WillowPlayerController.CheckJumpOrDuck / BeginSprint / EndSprint / PlayerWalking.ProcessMove`, `WillowPlayerPawn.DoSprint / CanSprint / CanContinueSprinting` | Walking Sanctuary | high | UNVERIFIED |
| Sprint effects (speed, accuracy, stance, FOV) | data: `GD_PlayerShared.Sprint.SprintDefinition_Default` | Sprint speed 594 | high | UNVERIFIED |
| Crouch | script: `WillowPlayerInput.DuckPressed/DuckReleased`, `Pawn.ShouldCrouch / StartCrouch / EndCrouch`, `WillowPlayerPawn.SetBaseEyeheight` | Crouch | medium-high | UNVERIFIED |
| Jump | script: `Pawn.CanJump / DoJump`, `WillowPlayerPawn.CanJump / DoJump` | Jump | high | UNVERIFIED |
| Landing and falling damage | script `Pawn.TakeFallingDamage`, native `WillowPawn.GetFallingDamageScale`, native `WillowPlayerPawn.ProcessFallDistance` | Falling off ledges | high (script), medium (scale) | UNVERIFIED |
| Eye-height and bob update (virtual, no script name) | writes `EyeHeight`, `BobTime`, `WalkBob` | First-person camera feel | medium-high | UNVERIFIED |
| WillowPawn.WeaponBob | `native function vector WeaponBob(float Speed)` shape | Arms sway | high | UNVERIFIED |
| Pawn view location and first-person camera | natives `Pawn.GetPawnViewLocation`, `WillowPlayerPawn.CalcCamera` | Camera position | medium | UNVERIFIED |
| FOV helpers | natives `WillowPlayerController.ToVFOV / ToHFOV / ScaleFOV / GetDefaultDefaultFOV / GetVerticalDefaultDefaultFOV / CalculateFlexibleFOV / CalculateFlexibleFOVModifier / CalculateInverseFlexibleFOV / CalculateInverseFlexibleFOVModifier / UpdateFOVAspectRatioScalar / GetFOVAngle / IsZoomed` | FOV defaults, sprint FOV, zoom FOV | high (maths), medium (use) | UNVERIFIED |
| Sprint FOV and FOV smoothing | script `WillowPlayerController.BeginSprint / AdjustFOV / SetPlayerFOV` | Sprint feel | high | UNVERIFIED |
| Weapon zoom FOV, no movement penalty | data on `WeaponTypeDefinition` | Zoomed walking | high (data scan) | UNVERIFIED |

## 1. Maya movement data (class, pawn, volume, globals)

- **Signature:** not a native; cooked values.
- **Reads (where each value lives):**
  - `GD_Siren.Character.CharClass_Siren` (Startup.upk, a `PlayerClassDefinition`): `AirSpeed` 500, `WalkingPct` 0.3, `JumpZ` 630,
    `MaxFallSpeed` 1500, `InitialFOVModifierSpeed` 10, `InitialEyeHeightModifierSpeed` 10, `SprintSettings`
    = `GD_PlayerShared.Sprint.SprintDefinition_Default`, `PawnArchetypePath` = `GD_Siren_Streaming.Pawn_Siren`.
    Fields it does not set take the `CharacterClassDefinition` class defaults: `GroundSpeed` 440, `CrouchedPct` 0.5,
    `SprintingPct` 1.
  - `GD_Siren_Streaming.Pawn_Siren` (GD_Siren_Streaming_SF.upk, a `WillowPlayerPawn` archetype): `BaseEyeHeight` 70,
    `EyeHeight` 77 (start value), `RotationRate` yaw 65536, `BodyClass` = `GD_Siren_Streaming.BodyClass_Siren`; its
    cylinder: `CollisionHeight` 80 (half height), `CollisionRadius` 42.
  - `Default__WillowPawn` (WillowGame.upk): attribute `GroundSpeed` 440, `AirSpeed` 440, `JumpZ` 322 (all superseded for
    the player by the class values), `WalkingPct` 0.4, `CrouchedPct` 0.4, `CrouchHeight` 50, `CrouchRadius` 21,
    `BaseEyeHeight` 75, `bCanCrouch` true, `Bob` 0.005, `EyeHeightModifierSpeed` 2, `CustomGravityScaling` 1.
  - `Default__Pawn` (Engine.upk): `AccelRate` 2048, `MaxStepHeight` 35, `MaxJumpHeight` 96 (AI), `WalkableFloorZ` 0.7
    (steeper than about 45.6 degrees is not walkable), `LedgeCheckThreshold` 4, `AirControl` 0.05, `LadderSpeed` 200,
    `WaterSpeed` 300 (WillowPawn 220), `MaxFallSpeed` 1200, `ViewPitchMin` -16384, `ViewPitchMax` 16383, `bLimitFallAccel`
    true, `MovementSpeedModifier` 1, `SprintingPct` 1, `AnalogMovePct` -1 (not analog), `bCanClimbLadders` true on
    `Default__WillowPlayerPawn`.
  - `Default__PhysicsVolume`: `GroundFriction` 8, `FluidFriction` 0.3, `TerminalVelocity` 4000.
  - `GD_Globals.General.Globals`: `PlayerAirControl` 0.11 (class default 0.05), `GlobalFallingDamageScale` = player max
    health attribute (`HealthMaxValue_Player`) times 0.1, `UnarmedFirstPersonFOV` 60; the class default `DefaultFOV` 70.
  - `Default__WorldInfo.DefaultGravityZ` -500 (the installed ini sets the same, as the host's Walker already notes).
    Sanctuary_P's WorldInfo carries no gravity override (checked by name table), so -500 applies there (UNVERIFIED; the
    level WorldInfo export did not decode with `ow-package`).
  - `BodyClass_Siren` / `BodyClassDefinition` defaults: `CrouchHeight` 50, `LandedMinVel` 300, `SkidCameraLurchMagnitude` 6,
    `SkidCameraLurchSpeed` 8; stances Run/Patrol/Sprint/Crouch/Injured all have `SpeedScale` 1 or unset (1).
  - `Default__WillowPlayerController`: `bLandingShake` true, `PlayerMovementType` 2 (analog), `FOVAngle`/`DefaultFOV` 170 as a
    placeholder (replaced at start, see section 19), `FOVModifierSpeed` attribute 2 (set to the class value 10 at
    start), `MatineeCameraClass`/`CameraClass` `WillowPlayerCamera`.
  - `Default__WillowPlayerInput`: `RunWalkTransitionThreshold` 0.75, `ButtonHoldEventTime` 0.35.
- **Derived numbers (arithmetic only):** run 440 uu/s, walk-key 132, crouch 220, sprint 594 (section 11); jump apex
  396.9 uu and 1.26 s to the apex at gravity 500 (section 8); in-air lateral acceleration 225 uu/s squared.
- **Edge cases:** the class's `GroundSpeed` is not set in `CharClass_Siren`, so 440 comes from the class default. The
  native that copies the class values onto the possessed pawn was not located (Open).
- **UE5 (ours):** CharacterMovementComponent: `MaxWalkSpeed` 440, `MaxWalkSpeedCrouched` 220, `MaxAcceleration` 2048, `JumpZVelocity` 630,
  `AirControl` 0.11, capsule half height 80 radius 42, `CrouchedHalfHeight` 50, `SetWalkableFloorZ(0.7)`, `MaxStepHeight` 35,
  `GroundFriction` 8, gravity -500 (the host's `GravityScale` trick). The Walker currently uses 450 / 420 / 650 / 45 degrees: all
  estimates; the data above supersedes them.
- **Implementer checklist:** read these from the decoded manifest, not constants; Maya's `JumpZ` is 630 not the pawn's 322 or
  the Pawn default 420.
- **Open:** how the class numbers reach the pawn (native, not found); whether profile or difficulty alters them (none seen).

## 2. Attribute-driven speeds

- **Signature:** `GroundSpeed`, `AirSpeed`, `JumpZ`, `AccelRate` (and `Bob`, `EyeHeightModifier`, `EyeHeightModifierSpeed`,
  `CustomGravityScaling`, `TotalEncumbrance`, `EncumbranceResistance`) are attribute properties on the pawn: a current value, a base value
  and a modifier stack. `bIsSprinting` is an integer attribute.
- **Does:** an `AttributeDefinition` such as `D_Attributes.GameplayAttributes.FootSpeed` resolves through a pawn context to the
  property named `GroundSpeed`; `Sprinting` resolves to `bIsSprinting`. Effects (skills, sprint) add modifiers to the stack. The
  stack formula is the one in NATIVE_WEAPON_RULES section 1: value = (base + sum of PreAdd) x ((1 + sum of positive Scale) /
  (1 - sum of non-positive Scale)) + sum of PostAdd, no clamp.
- **Constants:** sprint adds FootSpeed `MT_Scale` +0.35, so 440 x 1.35 = 594.
- **UE5 (ours):** keep the stack in the host's attribute layer and write the result to `MaxWalkSpeed` whenever it changes.
- **Open:** other attribute effects on `FootSpeed` from skills (Maya's trees were not scanned for FootSpeed).

## 3. Pawn speed fraction (virtual, called by the velocity step)

- **Reads:** `MovementSpeedModifier`, `DesiredSpeed`, `AnalogMovePct`, `bIsCrouched`, `bIsWalking`, the integer `bIsSprinting`,
  `WalkingPct`, `CrouchedPct`, `SprintingPct`; Willow adds `TotalEncumbrance`, `EncumbranceResistance`, a per-pawn speed scale
  field and the current or override stance's `SpeedScale`.
- **Does (formula):**
  1. Base = 1 for a player-controlled pawn, otherwise `DesiredSpeed` (default 1).
  2. If `AnalogMovePct` is 0 or more (gamepad analog): crouched multiplies by `CrouchedPct`; else sprinting (`bIsSprinting` above 0)
     multiplies by `SprintingPct`; else multiplies by `WalkingPct + (1 - WalkingPct) x AnalogMovePct`.
  3. If `AnalogMovePct` is negative (keyboard): crouched multiplies by `CrouchedPct`; else `bIsWalking` multiplies by `WalkingPct`;
     else sprinting multiplies by `SprintingPct`; else no factor.
  4. Pawn result = `MovementSpeedModifier` x base.
  5. Willow result = pawn result x clamp(1 - TotalEncumbrance x clamp(1 - EncumbranceResistance, 0, 1), 0, 1) x (per-pawn speed
     scale field) x (the stance object's `SpeedScale` when one is set).
- **Constants / formulas:** Maya: run 1.0, walk 0.3, crouch 0.5, sprint 1.0 (the sprint speed rise comes from the attribute, not
  from `SprintingPct`). Stance `SpeedScale` are all 1 for Maya.
- **Edge cases:** setting the integer sprint attribute above 0 via `Pawn.SetSprinting` also clears `bIsWalking`. Two Willow
  fields (speed scale, stance) are located by offset where the layout tool is four bytes off, so which field each is stays
  medium confidence (Open).
- **UE5 (ours):** multiply `MaxWalkSpeed` and `MaxAcceleration` by this fraction each tick; crouch is covered by `MaxWalkSpeedCrouched`.
- **Implementer checklist:** fraction scales both maximum speed and acceleration magnitude (section 4); walk key only matters if the
  host exposes a walk toggle (the stock keyboard default is run).

## 4. Velocity step (virtual calc-velocity) and braking

- **Signature (inferred):** acceleration direction, delta time, maximum speed, friction, fluid flag, brake flag, buoyancy flag.
  Called by the flying, swimming and spider physics steps; the ground step (not located) is assumed to call it with `GroundSpeed`
  and the volume's `GroundFriction` (UNVERIFIED).
- **Does (in order):**
  1. If root-motion velocity is forced, velocity becomes the stored root-motion velocity and nothing else happens.
  2. Speed fraction f (section 3). Maximum acceleration = `AccelRate` x f. Maximum speed = f x the maximum-speed argument.
  3. Acceleration = acceleration direction x maximum acceleration (magnitude clamped to maximum acceleration).
  4. When there is input, or braking is not requested: velocity steers toward the acceleration direction with friction:
     V = V - (V - Dir x |V|) x dt x Friction (component-wise).
  5. Then V = (1 - Fluid x Friction x dt) x V + Acceleration x dt, where Fluid is 1 or 0.
  6. If buoyant, vertical velocity gains gravity x (1 - buoyancy) x dt.
  7. If |V| is above maximum speed, V is rescaled to maximum speed.
- **Braking (no input, brake requested):** the frame is cut into sub-steps of at most 0.03 s; each sub-step does V = V - V x 2 x
  sub-dt x Friction. The reported velocity is the time-weighted average of the sub-step velocities that still point forward. If
  the result points opposite the starting velocity, or its speed is below 10 uu/s, velocity is set to zero.
- **Constants:** `AccelRate` 2048; `GroundFriction` 8. So full speed (440) is reached in about 0.2 s; stopping decays with rate 16
  per second and snaps to 0 below 10 uu/s (about 0.25 s from 440).
- **UE5 (ours):** `GroundFriction` 8 and `BrakingFrictionFactor` 2 with `bUseSeparateBrakingFriction` true and `BrakingDecelerationWalking`
  chosen to match; implement the snap-to-zero below 10 uu/s in a custom velocity hook if the feel is off.
- **Edge cases:** the AI path (a stuck or moving-base case) bypasses the friction step. Ground physics not located (Open).
- **Open:** the walking and falling step's exact call into this routine; ladder step.

## 6. Max speed by physics mode

- **Does:** the pawn's "maximum speed" getter returns `AirSpeed` when physics is flying, `WaterSpeed` when swimming, otherwise
  `GroundSpeed`. Falling does not use it (section 7).
- **UE5 (ours):** `MaxFlySpeed`/`MaxSwimSpeed`/`MaxWalkSpeed`.

## 7. Falling step (air control, gravity, terminal velocity)

- **Reads:** `AirControl`, `AccelRate`, `GroundSpeed`, `bLimitFallAccel`, volume `FluidFriction` and `TerminalVelocity`, gravity getter,
  `WalkableFloorZ`.
- **Does (in order):**
  1. When `bLimitFallAccel` is set the horizontal acceleration magnitude is limited to `AccelRate` x `AirControl`; if the horizontal
     speed is below 10 and `AirControl` is above 0, the limit is raised by (10 - speed) / dt, giving a start kick.
  2. If `AirControl` is 0.05 or less and the horizontal speed already reaches `GroundSpeed`, horizontal speed may not grow
     beyond its current value.
  3. The frame is cut into sub-steps of at most 0.05 s (at most 8 per tick). Each sub-step updates velocity as V = V x (1 - fluidFriction
     x dt) + (1 - buoyancy) x acceleration x dt, where acceleration is the steering acceleration with gravity added to its Z.
  4. Velocity magnitude is clamped to the volume's `TerminalVelocity` after the move.
  5. Collision: hitting a surface whose normal Z is at or above `WalkableFloorZ` triggers landing (script `Landed`, section 14);
     otherwise the pawn slides along it and continues the remaining time.
- **Constants:** `AirControl` 0.11 for the player (`PlayerAirControl` copied by `WillowGameInfo.ApplyGlobalPlayerMovementSettings`),
  `FluidFriction` 0.3, `TerminalVelocity` 4000, `AccelRate` 2048: air acceleration 225 uu/s squared.
- **Edge cases:** the speed cap in step 2 applies only for low air control; at 0.11 there is no horizontal cap in the air.
- **UE5 (ours):** `AirControl` 0.11; `AirControlBoostMultiplier`/`AirControlBoostVelocityThreshold` approximate the start kick; vertical
  drag has no direct CMC setting (host tick hook); terminal velocity via `APhysicsVolume::TerminalVelocity` or a clamp.
- **Open:** exact interplay of the kick with `AirControlBoost` is a tuning job; walking step not read.

## 8. Gravity getters

- **Does:** pawn gravity Z = (volume or world gravity Z) x `CustomGravityScaling` (attribute, 1 by default); when the pawn is leaping
  (a flag) an extra leap gravity scale multiplies it; rigid-body physics use the rigid-body gravity scaling instead.
- **Constants:** world gravity -500 uu/s squared. With `JumpZ` 630 the apex is 630^2 / 1000 = 396.9 uu, 1.26 s up, 2.52 s on level ground.
  The previous host estimate (420) predicted 176 cm. **This is large; confirm with a measured jump** (Open).
- **UE5 (ours):** `GravityScale` = 500 / project gravity (already done in the Walker).

## 9. Player move input to acceleration (script)

- **Does (each tick, `PlayerController.PlayerWalking.PlayerMove` then `ProcessMove`):** the move direction is (forward input x
  forward vector of the yaw-only view rotation) + (strafe input x right vector); Z is zeroed; acceleration = `AccelRate` x
  normalized direction (analog stick magnitude is not used here; analog shows up in `AnalogMovePct`). The Willow version first returns
  if there is no pawn or the pawn is rigid-body physics, zeroes `GroundPitch`, and when a melee lunge target is set forces forward
  input 1 and strafe 0. `ProcessMove` copies the acceleration to the pawn, decompresses `AnalogMovePct`, then runs the jump/duck
  check (sections 12 and 13) and the sprint state machine (section 10). `bPressedSprint` is cleared at the end of every move.
- **Edge cases:** on the keyboard `AnalogMovePct` is the "not analog" -1; on a gamepad (`bEnableAnalogMovement` and gamepad in use) it is the larger
  of the two stick magnitudes. The stock `PlayerMovementType` 2 is analog; 0 (keyboard) and 1 (gamepad walk/run) are set by the settings.
- **UE5 (ours):** `AddMovementInput` with a normalized vector scaled by 1 (keyboard).

## 10. Sprint entry and exit (script, with the sprint input rule)

- **Does (in order of the tick):**
  1. Input (`WillowPlayerInput.PlayerInput`, every tick): `bCanSprint` = the normalized (strafe, forward) input has a dot product of
     at least 0.7071 with forward (within 45 degrees of straight ahead; no input is false). If sprint is wanted (`bTryToSprint`) and a pawn
     exists and `bCanSprint`: crouch is cancelled (`bDuck` 0, toggle hold cleared), `bPressedSprint` is set and `bTryToSprint` cleared.
     `SprintPressed` does the same at once when the input already allows sprint, otherwise it just sets `bTryToSprint`;
     `SprintReleased` clears `bTryToSprint`.
  2. `CheckJumpOrDuck`: if `bPressedSprint`, a Willow pawn exists and move input is not ignored: a toggled zoom is released (stop alt fire),
     `bWantsToSprint` is set, then `WillowPlayerPawn.DoSprint`.
  3. `DoSprint`: refuses without a class `SprintSettings`; if `CanSprint` it begins sprint at once when `IsOnGroundOrShortFall`, else
     keeps `bWantsToSprint` pending.
  4. `ProcessMove` (local controller): if sprinting, end sprint when the pawn is gone, `CanContinueSprinting` is false or the input no
     longer allows it. Otherwise if `bWantsToSprint`: drop it when the input does not allow sprint; else when `IsOnGroundOrShortFall`
     and not zoomed and `CanSprint`, begin sprint and drop the want. Non-local controllers follow the replicated `bClientIsSprinting`.
  5. `EndState` of the walking state ends sprint and clears `bPressedSprint`.
- **Conditions:** `CanSprint`: class sprint settings exist, not already sprinting, no melee attack in progress, the weapon is not firing and
  not zoomed, the off-hand weapon is not firing. `CanContinueSprinting`: a controller exists, `bDuck` is 0, pawn not deleted or
  dead, not injured (unless `bCanSprintWhileInjured`), weapon not firing and not zoomed, off-hand not firing, no shared weapon
  action in progress. `IsOnGroundOrShortFall`: physics is walking, or `PlayerFallDuration` is below 0.3 s (a jump sets it to 0.5, so
  sprint cannot be begun in a jump, but a running sprint continues in the air).
- **BeginSprint:** sets the sprint state, applies the `SprintDefinition` attribute effects to the controller (section 11), starts the sprint
  FOV (section 19), sets the default stance (sprint stance) and stops weapon recoil animations, zeroes `CurrentSprintDistance`.
  **EndSprint:** clears the state, starts a camera "skid" lurch (location magnitude -6, rotation magnitude -600, duration 0.25 s, falloff 16, from the
  body class lurch values 6 and 8), removes the sprint attribute effects, resets the stance and records the longest sprint stat.
- **UE5 (ours):** no CMC sprint: a host state machine toggling a 1.35 speed scale, FOV target and the pool effects.
- **Implementer checklist:** sprint needs forward input within 45 degrees; firing, zoom, crouch key, melee and injury end or block it; pressing
  sprint while standing still keeps it pending until you move forward.

## 11. Sprint effects (data)

- `SprintDefinition_Default` effects: `FootSpeed` MT_Scale +0.35; `AccuracyResourcePool.AccuracyMinValue` and `AccuracyMaxValue`
  MT_PostAdd +15 each; `AccuracyOnIdleRegenerationRate` MT_PostAdd +3; `Sprinting` MT_PostAdd +1 (sets the integer sprint attribute).
  Class defaults: `FOVModifier` 0.35, `EyeHeightModifier` -0.02, `BobScalar` 0.01 (not overridden by Maya's definition).
- The stance changes to the body class `SprintStance` (SpeedScale 1).
- No melee, reload or weapon-swap block beyond the conditions of section 10 was found in script; `DualWieldActionSkill` has its own sprint
  transition (not read).

## 12. Crouch, 13. Jump (script)

- **Crouch:** `DuckPressed` (ignored while trading): with the crouch toggle on, a repeat inside `DoubleClickTime` is ignored, otherwise it
  flips `bHoldDuck`/`bDuck`; with hold-to-crouch it sets `bDuck` 1. `DuckReleased` clears `bDuck` only in hold mode. Each move, unless
  physics is falling and `bCanCrouch` allows it, `CheckJumpOrDuck` calls `ShouldCrouch(bDuck != 0)`; the pawn variant refuses to start
  crouching while injured. Native pawn code resizes the cylinder: height `CrouchHeight` 50 (half-height), radius `CrouchRadius` 21, then script `StartCrouch`
  lowers `EyeHeight` by the height change and `SetBaseEyeheight` sets the crouched base eye height = min(0.8 x CrouchHeight, CrouchHeight - 10)
  = 40 (standing: 70). The mesh translation and `OldLocationZ` are adjusted so the view does not pop. Crouched speed = 0.5 x 440 = 220.
  Sprint and jump presses clear `bDuck` first. **UE5 (ours):** `Crouch()`/`UnCrouch`, `CrouchedHalfHeight` 50, `CrouchedEyeHeight` 40;
  crouched radius 21 has no CMC setting (resize the capsule).
- **Jump:** `CanJump` = jump capable and (stuck-jump case, or not crouched, not wanting to crouch, and physics is walking, ladder or spider);
  Willow also needs no GFx menu open and no deferred movie blocking. A pressed jump is kept until it can fire (the base move loop re-arms it
  when `CannotJumpNow`). `DoJump`: walking: Z velocity = `JumpZ` (plus the base's upward velocity when standing on a moving non-world base);
  ladder: Z velocity 0 then fall; spider: velocity = `JumpZ` x floor normal; then physics becomes falling. Willow adds the jump foot-impact effect, the
  `DET_Jump` dialog event, remembers `LocationFellFrom`, sets `bWasFalling` and `PlayerFallDuration` 0.5. Crouched: the press only stands up first.
  **UE5 (ours):** `JumpZVelocity` 630, `JumpMaxHoldTime` 0, `bCanJumpWhileCrouched` default false, matching.

## 14. Landing and falling damage

- **Does:** on landing, `Pawn.Landed` runs `TakeFallingDamage` then, if alive, `PlayLanded` with the landing Z velocity (foot impact effect and `DET_JumpLand`
  dialog when |speed| is at least `LandedMinVel` 300). `WillowPlayerPawn.Landed` also calls the native `ProcessFallDistance` with the fall
  distance; **that native does nothing** (empty implementation on the player pawn's table). `WillowPawn.TakeFallingDamage` skips damage while leaping or hard attached.
- **`TakeFallingDamage` rule:** with v = landing Z velocity (negative) and M = `MaxFallSpeed` (1500): noise 1.0 is made when v is below -0.5 M;
  damage applies only when v is below -M after adding +100 when touching a water volume, and not when `ImmuneToFallingDamage`:
  damage = -((v + M) / M) x `GetFallingDamageScale`, delivered as a `DmgType_Fell` hit with no instigator. Otherwise lesser landings make noise 0.5 (below -1.4 `JumpZ`)
  or 0.2 (below -0.8 `JumpZ`) and no damage.
- **`WillowPawn.GetFallingDamageScale` (native):** evaluates `GlobalsDefinition.GlobalFallingDamageScale` with the pawn as context (= max health x 0.1) and returns 1 when
  no globals exist. At v = -3000 the factor is (3000 - 1500) / 1500 = 1, so the damage is 10 percent of max health; terminal speed 4000 gives 16.7 percent.
  With gravity 500 a fall of 2250 uu reaches 1500.
- **UE5 (ours):** `Landed` callback plus `LastUpdateVelocity.Z`; apply damage with the same formula.
- **Open:** whether the camera landing dip (the `bLandingShake` flag, which only returns that flag in script) is applied natively.

## 15. Eye-height and bob update (virtual, every tick)

- **Does (in order):**
  1. When the pawn is under matinee-style control the eye height is simply reset; otherwise:
  2. Eye height easing: target = `BaseEyeHeight`; when on ground (walking or navmesh walking) the current eye height is first shifted by the vertical
     change since the last tick (`OldLocationZ`) so stairs do not jerk the camera; then `EyeHeight` moves toward `BaseEyeHeight` by alpha = min(1, dt x 10)
     (linear step; speed 10).
  3. `AppliedEyeHeightModifier` eases toward `EyeHeightModifier` at speed `EyeHeightModifierSpeed` (class sets 10); while sprinting the target
     modifier is the sprint definition's -0.02. `EyeHeight` is divided by (1 - m) for negative m and multiplied by (1 + m) otherwise.
  4. `EyeHeight` is clamped between -0.5 x cylinder half-height and cylinder half-height + `MaxStepHeight`.
  5. Bob (when `bUpdateEyeheight` is on): amplitude b = `Bob` (0.005), clamped to +-0.05; while sprinting b = `BobBaseValue` x `SprintDefinition.BobScalar`.
     On ground, `BobTime` advances by dt x ((horizontal speed x 0.7) / `GroundSpeed` + 0.3) at 10 uu/s or more, or by dt x 0.2 below it;
     `WalkBob` horizontal = right-vector x b x speed x sin(8 x BobTime), vertical = b x 0.75 x speed x sin(16 x BobTime) when speed is above 10.
     Airborne: `BobTime` resets to 0 and `WalkBob` decays by (1 - min(1, 8 dt)) each tick.
  6. A footstep is triggered when the integer part of (BobTime x 9 / pi + pi/2) changes while on ground at speed 10 or more (local player only, not crouch/ladder-flagged).
- **UE5 (ours):** drive the camera height from a host-side filter; eye height is a function of the capsule.
- **Open:** whether the sprint bob scalar really shrinks the amplitude 100 times (0.005 x 0.01 as read); confirm by screenshots.

## 16. WillowPawn.WeaponBob

- **Does:** returns a vector from `WalkBob`: X = speed x WalkBob.X, Y = speed x WalkBob.Y, Z = (0.55 x speed + 0.45) x WalkBob.Z; and when vertical velocity
  is above 0 (rising) the Z is lowered by min(vertical velocity, 300) x 0.005. It offsets the arms/weapon, not the camera.

## 17. Pawn view location and first-person camera

- **Pawn view location (`GetPawnViewLocation`):** with `bUpdateEyeheight` set: Location + `WalkBob` + (0, 0, `EyeHeight`); otherwise Location + (0, 0, `BaseEyeHeight`).
- **`WillowPlayerPawn.CalcCamera`:** picks injured, dying, awaiting-respawn and similar special cameras by pawn state flags; the normal path
  chooses by controller state: free camera, behind-view (third person using `CameraScale` zoom values 5, up 1, right 2, range 3 to 40) or the
  first-person case. First person: location from the pawn view location, rotation from the controller's view rotation, then a camera-animation offset
  (rotated into the view) is added; the final FOV comes from the controller (section 18). The pawn's pitch limits come from `ViewPitchMin/Max`
  (+-16384 rotator units, +-90 degrees).
- **UE5 (ours):** camera at capsule centre + (0, 0, eye height) with controller rotation, using the arms camera-anim offset if reproduced.
- **Open:** the camera animation offset routine and the injured cameras were not read.

## 18. FOV helpers (natives, exact maths)

All angles are degrees. The game's "VFOV/HFOV" pair assumes a fixed 16:9 reference.
- `ToVFOV(h)` = 2 x atan(tan(h/2) x 0.5625). `ToHFOV(v)` = 2 x atan(tan(v/2) x 1.77778). `ScaleFOV(fov, s)` = 2 x atan(tan(fov/2) x s).
- `GetDefaultDefaultFOV` = `GlobalsDefinition.DefaultFOV` (70) or 70 when no globals; `GetVerticalDefaultDefaultFOV` = ToVFOV of it (43.0).
- `CalculateFlexibleFOV(f)` = (CalculateFlexibleFOVModifier(f / ToVFOV(DefaultFOVglobal) - 1) + 1) x controller `DefaultFOV`. `CalculateFlexibleFOVModifier(m)` is
  the modifier m (defined on the 16:9 vertical default view) re-expressed for the controller's `DefaultFOV` P: 2 x atan(tan(P/2) x tan((m+1) x V/2) /
  tan(V/2)) / P - 1, with V = ToVFOV(global default). At the stock default the modifier is unchanged (0.35 stays 0.35). The `Inverse` forms undo them.
- `UpdateFOVAspectRatioScalar`: sets the view/foreground scalars to 1 and, in a split screen, to the values stored in the controller (stock 1, 2, 0.667 and so on).
- `GetFOVAngle` (virtual, cached by the base FOV): the base angle comes from the player camera (else `FOVAngle`); when a weapon is zoomed the weapon's FOV
  (converted with `CalculateFlexibleFOV` from ToVFOV of `ZoomedFOV`) replaces it; the result is scaled in tangent space by the view aspect scalar (1 in single player).
- `IsZoomed`: the pawn has a weapon whose `ZoomState` is not 0 (any non-out zoom state).
- **UE5 (ours):** use the horizontal FOV directly: default 70, sprint about 89.2 (at 16:9), zoom FOV per weapon type below.
- **Open:** which axis the renderer treats these as (vertical-derived values at 16:9 assumed); verify against a real-game screenshot.

## 19. FOV defaults, sprint FOV and smoothing (script)

- At `PostBeginPlay` the controller calls `SetPlayerFOV(GetDefaultDefaultFOV)` = 70 horizontal, stored as `DefaultFOV` = ToVFOV (43.0) with
  `DesiredFOV` and `FOVAngle` set equal; after class defaults it reads the profile FOV setting (id 129, clamped 5 to 175 by the `FOV` console command)
  when the profile has one; the stock default profile list has no entry for it, so a fresh profile stays at 70 (UNVERIFIED).
  The class defaults of 170 are placeholders.
- `BeginSprint`: `SprintDesiredFOV` = `DefaultFOV` x (1 + CalculateFlexibleFOVModifier(`FOVModifier`)) = 43.0 x 1.35 = 58.0 (about 89.2 horizontal at 16:9);
  `SprintFOVAngle` starts at the current `FOVAngle`. `AdjustFOV` moves `FOVAngle` and `SprintFOVAngle` toward their desired values each tick:
  new = lerp(current, target, dt x max(`FOVModifierSpeed`, 0.001)), snapping when the sign of the difference flips; `FOVModifierSpeed` is set to the class value 10 at
  start (default attribute 2). `RemoveSprintFOV` and `EndSprint` restore the angle.
- Weapon zoom FOV: `WeaponTypeDefinition.ZoomedEndFOV` (pistols 50, SMGs 45, assault rifles 45, launchers 45, shotguns 60, snipers 30 to 38, authored as the 16:9
  horizontal numbers); `ZoomedFOV` is set to the global default (70) when the state is out or zooming out and to `ZoomedEndFOV` otherwise, then eased by the weapon's native
  zoom tick (not read). At the default FOV the converted zoomed FOV equals the authored one.
- **Zoom movement penalty:** none found. A scan of all 33 `WeaponTypeDefinition` objects in Startup.upk found zoom effect lists only on accuracy pool min/max
  and weapon spread, burst count and fire interval; none names FootSpeed or GroundSpeed. Zoom does end sprint (section 10).
- **UE5 (ours):** camera FOV target blended with an exponential filter at rate 10.

## 20. Ladders, steps, floors

`MaxStepHeight` 35, `WalkableFloorZ` 0.7, `LedgeCheckThreshold` 4, `LadderSpeed` 200, `bCanClimbLadders` true on the player pawn. The ground and ladder steps were not read
(Open). **UE5 (ours):** `MaxStepHeight` 35, `SetWalkableFloorZ(0.7)`; ladders need a custom mode.

## Not read yet

- The walking physics step, ladder step, floor-finding and step-up rules; the native that copies class `GroundSpeed`/`AirSpeed`/`JumpZ`/`WalkingPct`/`MaxFallSpeed` onto the pawn.
- Where `PlayerFallDuration` is incremented; the landing camera dip; the injured, dying and awaiting-respawn cameras and the camera-animation offset;
  `GetFOVAngleForeground`; `TickZoom`; where the sprint `EyeHeightModifier` is consumed beyond the eye-height update; skid lurch application;
  `CurrentSprintDistance` accumulation.
- The melee lunge helper (temporarily multiplies `AccelRate` by 24 and drives a ground-speed modifier toward a target over a given time).
- Which skills in Maya's trees touch FootSpeed or jump (this note covers stock sprint only).

## Corrections to earlier notes

- NATIVE_WEAPON_FIRING.md says no movement or stance modifier exists in the stock data for the player's accuracy pool. The stock sprint definition
  `SprintDefinition_Default` does add to the pool while sprinting: min +15, max +15 (post-add) and idle regeneration rate +3. That scan only covered weapon, part, artifact
  and class-mod objects. UNVERIFIED in game; firing is blocked while sprinting, so the effect shows mainly in the first shots after sprint ends.
- The host Walker uses estimates (450 / 420 / 650 / 45 degrees); the data here gives 440 / 630 / 594 / 0.7 floor Z.
- Native operator numbers 244 to 247 in the script listing are Min, Max, Clamp and Lerp (float), not add; read this way throughout.
