//! Owner: lane H4-MOVEMENT. The player mover: input to acceleration, the ground velocity step and braking, the
//! falling step, jump and landing, and a walking step with step-up, ramps, wall slides and a floor snap.
//!
//! AI-assisted (Claude Code). Decision: `docs/decisions/2026-11/2026-11-16-52-*`. Rules from
//! `docs/verification/NATIVE_PLAYER_MOVEMENT.md` (sections named on each item) and the live values of
//! `COLLISION_WALKING_VERIFICATION.md` (2026-10-21); nothing here was compared with a measured movement trace.
//!
//! **Where each rule comes from.**
//! - Read from native code or script (UNVERIFIED in the running game): the input direction (section 9), the ground
//!   velocity step and braking (section 4), the falling step's air control, start kick, sub-steps, fluid friction
//!   and terminal velocity (section 7), the jump (section 13), the landing test on `WalkableFloorZ` (section 7.5).
//! - **Host stand-ins**, because the game's walking step, floor finding and step-up were not read (section 20, "Not
//!   read yet"): how a walking move is split into moves along the floor, up a ramp (the move is turned along the
//!   ramp plane), over a step (up by `MaxStepHeight`, across, back down onto a walkable floor), and along a wall
//!   (the horizontal wall normal removed), and the floor snap after a walking move (down by up to `MaxStepHeight`
//!   onto a walkable floor, else falling). How the falling step integrates a sub-step's move (the new velocity times
//!   the sub-step) and that a landing drops the rest of the sub-step are stand-ins too. Each is our choice; none is
//!   claimed as the game's.
//! - Not here: sprint, crouch, the walk key, ladders, swimming, falling damage (the landing speed is reported, not
//!   applied), the eye-height easing and bob (section 15), moving bases, and pawn-to-pawn collision.
//!
//! **Determinism.** The state is `f64` and every step is a pure function of the state, the input, the time step and
//! the collision world: the same sequence gives the same bits (tests). Rotations are rotator units (`i32`).

use super::collide::{CollisionWorld, Hit, SKIN, Shape};
use super::geom::{V3, ZERO, add, along_plane, dot, horizontal, length, normalize, scale, sub};
use crate::frame::rotator_axes;
use ow_core::units::Rotator;

/// The pawn's movement numbers, uu and seconds.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MovementParams {
    /// The pawn's collision shape.
    pub shape: Shape,
    /// `MaxStepHeight`.
    pub max_step_height: f64,
    /// `WalkableFloorZ`: the least floor normal Z that is walkable.
    pub walkable_floor_z: f64,
    /// `GroundSpeed`.
    pub ground_speed: f64,
    /// `AccelRate`.
    pub accel_rate: f64,
    /// The pawn speed fraction (section 3): 1 for Maya running on the keyboard.
    pub speed_fraction: f64,
    /// The physics volume's `GroundFriction`.
    pub ground_friction: f64,
    /// Braking: friction factor 2, sub-steps of at most 0.03 s, stop below 10 uu/s (section 4).
    pub braking_friction_factor: f64,
    pub braking_substep: f64,
    pub stop_speed: f64,
    /// `JumpZ`.
    pub jump_z: f64,
    /// `AirControl` (the player's, `PlayerAirControl`).
    pub air_control: f64,
    /// `bLimitFallAccel`.
    pub limit_fall_accel: bool,
    /// The start kick's speed threshold, uu/s (section 7.1).
    pub air_kick_speed: f64,
    /// The pawn's gravity Z, uu/s squared.
    pub gravity_z: f64,
    /// The physics volume's `FluidFriction` and `TerminalVelocity`.
    pub fluid_friction: f64,
    pub terminal_velocity: f64,
    /// Falling sub-steps: at most this long, at most this many per tick (section 7.3).
    pub fall_substep: f64,
    pub max_fall_substeps: u32,
    /// `BaseEyeHeight`, above the shape's centre.
    pub base_eye_height: f64,
    /// `ViewPitchMin`, `ViewPitchMax` (rotator units).
    pub view_pitch_min: i32,
    pub view_pitch_max: i32,
}

impl MovementParams {
    /// Maya on the keyboard, running.
    ///
    /// Read in the running game (bl-sdk, 2026-10-21, one session, `COLLISION_WALKING_VERIFICATION.md`): cylinder
    /// radius 42 and half height 80, `MaxStepHeight` 35, `WalkableFloorZ` 0.70, `GroundSpeed` 440, `AccelRate` 2048,
    /// `AirControl` 0.11, `JumpZ` 630, gravity -500, `BaseEyeHeight` 70. These are live property reads, not measured
    /// motion.
    ///
    /// Cooked values or native constants, UNVERIFIED in the game (`NATIVE_PLAYER_MOVEMENT.md`): `GroundFriction` 8,
    /// `FluidFriction` 0.3, `TerminalVelocity` 4000 (section 1, `Default__PhysicsVolume`); braking factor 2, 0.03 s
    /// sub-steps, the 10 uu/s stop (section 4); the falling sub-steps of 0.05 s, at most 8, and the 10 uu/s kick
    /// (section 7); `bLimitFallAccel` true, `ViewPitchMin`/`Max` -16384/16383 and the speed fraction 1 (sections
    /// 1 and 3).
    pub fn maya() -> MovementParams {
        MovementParams {
            shape: Shape::Cylinder { radius: 42.0, half_height: 80.0 },
            max_step_height: 35.0,
            walkable_floor_z: 0.7,
            ground_speed: 440.0,
            accel_rate: 2048.0,
            speed_fraction: 1.0,
            ground_friction: 8.0,
            braking_friction_factor: 2.0,
            braking_substep: 0.03,
            stop_speed: 10.0,
            jump_z: 630.0,
            air_control: 0.11,
            limit_fall_accel: true,
            air_kick_speed: 10.0,
            gravity_z: -500.0,
            fluid_friction: 0.3,
            terminal_velocity: 4000.0,
            fall_substep: 0.05,
            max_fall_substeps: 8,
            base_eye_height: 70.0,
            view_pitch_min: -16384,
            view_pitch_max: 16383,
        }
    }
}

/// The physics mode.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Physics {
    Walking,
    Falling,
}

/// One tick's input.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MoveInput {
    /// Forward and strafe (right) input, each -1 to 1 (keyboard: -1, 0 or 1).
    pub forward: f64,
    pub strafe: f64,
    /// Jump pressed this tick.
    pub jump: bool,
    /// View turn this tick, rotator units.
    pub yaw: i32,
    pub pitch: i32,
}

impl MoveInput {
    pub fn new(forward: f64, strafe: f64, jump: bool, yaw: i32, pitch: i32) -> MoveInput {
        MoveInput { forward, strafe, jump, yaw, pitch }
    }
}

/// What happened in a tick, besides the new state.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TickEvents {
    /// The pawn landed this tick, at this Z velocity (negative: down).
    pub landed: Option<f64>,
    /// The pawn left the ground this tick (a jump or a ledge).
    pub left_ground: bool,
    /// A walking step went up (uu climbed by step-ups this tick).
    pub stepped_up: f64,
    /// A trace started inside a brush this tick.
    pub start_solid: bool,
}

/// The pawn's movement state.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mover {
    pub params: MovementParams,
    /// The shape's centre, uu.
    pub location: V3,
    pub velocity: V3,
    pub physics: Physics,
    /// The view rotation (yaw drives the move; pitch only the view).
    pub view: Rotator,
    /// The floor's normal while walking.
    pub floor_normal: V3,
}

/// How many moves one walking or falling move may be split into (hits handled), before the rest is dropped.
const MAX_MOVE_PARTS: u32 = 4;

/// How far [`Mover::lift_out_of_solid`] searches upward, uu: the pawn's full height.
pub const SPAWN_LIFT: f64 = 160.0;

impl Mover {
    /// A pawn at `location`, falling (it settles onto the floor below, if any), facing `view`.
    pub fn spawn(params: MovementParams, location: V3, view: Rotator) -> Mover {
        Mover { params, location, velocity: ZERO, physics: Physics::Falling, view, floor_normal: [0.0, 0.0, 1.0] }
    }

    /// Lifts a pawn that starts inside a brush to the first free height up to [`SPAWN_LIFT`] uu above, in 1 uu steps,
    /// and returns how far it was lifted (`None`: no free height there, the pawn is left where it was). A **host
    /// stand-in**: how the game places a pawn at a player start (its spot search) was not read. Sanctuary's start is
    /// exactly half the cylinder's height above the floor under it, so the cylinder touches the floor, which the
    /// sweep counts as inside: the walk lifts it 1 uu.
    pub fn lift_out_of_solid(&mut self, world: &CollisionWorld) -> Option<f64> {
        let shape = self.params.shape;
        let mut lift = 0.0;
        while lift <= SPAWN_LIFT {
            let at = add(self.location, [0.0, 0.0, lift]);
            if !world.trace(&shape, at, at).start_solid {
                self.location = at;
                return Some(lift);
            }
            lift += 1.0;
        }
        None
    }

    /// The view location: the centre plus `BaseEyeHeight` (section 17 without bob and eye-height easing).
    pub fn eye(&self) -> V3 {
        add(self.location, [0.0, 0.0, self.params.base_eye_height])
    }

    /// Advances by `dt` seconds.
    pub fn tick(&mut self, world: &CollisionWorld, input: &MoveInput, dt: f64) -> TickEvents {
        let mut events = TickEvents::default();
        if !(dt > 0.0 && dt.is_finite()) {
            return events;
        }
        self.look(input);
        let accel = self.acceleration(input);
        if input.jump && self.physics == Physics::Walking {
            // Section 13: walking, Z velocity becomes JumpZ and physics falling.
            self.velocity[2] = self.params.jump_z;
            self.physics = Physics::Falling;
            events.left_ground = true;
        }
        match self.physics {
            Physics::Walking => self.walk(world, accel, dt, &mut events),
            Physics::Falling => self.fall(world, accel, dt, &mut events),
        }
        events
    }

    fn look(&mut self, input: &MoveInput) {
        let yaw = self.view.yaw.wrapping_add(input.yaw);
        let pitch = self
            .view
            .pitch
            .saturating_add(input.pitch)
            .clamp(self.params.view_pitch_min, self.params.view_pitch_max);
        // Yaw wraps into one signed turn (`frame::normalize_axis`).
        self.view = Rotator::new(pitch, crate::frame::normalize_axis(yaw), self.view.roll);
    }

    /// Section 9: (forward input x the yaw-only forward vector) + (strafe input x its right vector), Z zeroed,
    /// normalised, times `AccelRate`.
    fn acceleration(&self, input: &MoveInput) -> V3 {
        let axes = rotator_axes(Rotator::new(0, self.view.yaw, 0));
        let dir = add(scale(axes[0], input.forward), scale(axes[1], input.strafe));
        match normalize(horizontal(dir)) {
            Some(d) => scale(d, self.params.accel_rate),
            None => ZERO,
        }
    }

    /// Section 4: the ground velocity step (with input) or braking (without).
    fn ground_velocity(&mut self, accel: V3, dt: f64) {
        let p = self.params;
        let f = p.speed_fraction;
        let max_accel = p.accel_rate * f;
        let max_speed = p.ground_speed * f;
        let mut v = horizontal(self.velocity);
        match normalize(accel) {
            Some(dir) => {
                let a = scale(dir, max_accel);
                let speed = length(v);
                // V = V - (V - Dir |V|) dt Friction.
                v = sub(v, scale(sub(v, scale(dir, speed)), dt * p.ground_friction));
                v = add(v, scale(a, dt));
                let s = length(v);
                if s > max_speed {
                    v = scale(v, max_speed / s);
                }
            }
            None => {
                let start = v;
                let mut remaining = dt;
                let mut sum = ZERO;
                let mut forward_time = 0.0;
                while remaining > 0.0 {
                    let sub_dt = remaining.min(p.braking_substep);
                    v = sub(v, scale(v, p.braking_friction_factor * sub_dt * p.ground_friction));
                    if dot(v, start) > 0.0 {
                        sum = add(sum, scale(v, sub_dt));
                        forward_time += sub_dt;
                    }
                    remaining -= sub_dt;
                }
                v = if forward_time > 0.0 { scale(sum, 1.0 / forward_time) } else { ZERO };
                if dot(v, start) <= 0.0 || length(v) < p.stop_speed {
                    v = ZERO;
                }
            }
        }
        self.velocity = v;
    }

    fn walk(&mut self, world: &CollisionWorld, accel: V3, dt: f64, events: &mut TickEvents) {
        self.ground_velocity(accel, dt);
        let delta = scale(self.velocity, dt);
        let walls = self.move_along_floor(world, delta, events);
        // The velocity loses what pushed into the walls the move slid along (stand-in).
        for n in walls {
            let into = dot(self.velocity, n);
            if into < 0.0 {
                self.velocity = sub(self.velocity, scale(n, into));
            }
        }
        self.find_floor(world, events);
    }

    /// The walking move (stand-in, module documentation). Returns the horizontal wall normals it slid along.
    fn move_along_floor(&mut self, world: &CollisionWorld, delta: V3, events: &mut TickEvents) -> Vec<V3> {
        let p = self.params;
        let shape = p.shape;
        let mut remaining = delta;
        let mut walls: Vec<V3> = Vec::new();
        let mut stepped = false;
        for _ in 0..MAX_MOVE_PARTS {
            if length(remaining) < 1e-9 {
                break;
            }
            let hit = world.trace(&shape, self.location, add(self.location, remaining));
            events.start_solid |= hit.start_solid;
            self.location = hit.end;
            if !hit.blocked() {
                break;
            }
            let left = scale(remaining, 1.0 - hit.fraction);
            if hit.normal[2] >= p.walkable_floor_z {
                // A walkable ramp ahead: the rest of the horizontal move goes along its plane.
                let h = horizontal(left);
                let rise = -(hit.normal[0] * h[0] + hit.normal[1] * h[1]) / hit.normal[2];
                remaining = [h[0], h[1], rise];
                continue;
            }
            if !stepped {
                stepped = true;
                if let Some(climb) = self.step_up(world, horizontal(left), events) {
                    events.stepped_up += climb;
                    // The step took the whole horizontal rest of the move.
                    remaining = ZERO;
                    continue;
                }
            }
            // A wall: slide along it, horizontally.
            let Some(n) = normalize(horizontal(hit.normal)) else {
                break;
            };
            let mut slide = horizontal(along_plane(left, n));
            for w in &walls {
                // A slide back into an earlier wall: a corner, stop.
                if dot(slide, *w) < -1e-9 {
                    slide = ZERO;
                }
            }
            walls.push(n);
            remaining = slide;
        }
        walls
    }

    /// Up by `MaxStepHeight`, across by `across`, back down onto a walkable floor (stand-in). Returns how far up the
    /// pawn ended, or `None` (nothing moved) when the step does not land on a walkable floor or makes no progress.
    fn step_up(&mut self, world: &CollisionWorld, across: V3, events: &mut TickEvents) -> Option<f64> {
        let p = self.params;
        let shape = p.shape;
        let start = self.location;
        let up = world.trace(&shape, start, add(start, [0.0, 0.0, p.max_step_height]));
        events.start_solid |= up.start_solid;
        let lifted = up.end[2] - start[2];
        if lifted <= SKIN {
            return None;
        }
        let over = world.trace(&shape, up.end, add(up.end, across));
        events.start_solid |= over.start_solid;
        if length(horizontal(sub(over.end, up.end))) < 0.5 * length(across).min(1.0) {
            return None;
        }
        let down = world.trace(&shape, over.end, sub(over.end, [0.0, 0.0, lifted + 2.0 * SKIN]));
        events.start_solid |= down.start_solid;
        if down.start_solid || (down.blocked() && down.normal[2] < p.walkable_floor_z) {
            return None;
        }
        self.location = down.end;
        if down.blocked() {
            self.floor_normal = down.normal;
        }
        Some(down.end[2] - start[2])
    }

    /// After a walking move: down by up to `MaxStepHeight` onto a walkable floor, else falling (stand-in).
    fn find_floor(&mut self, world: &CollisionWorld, events: &mut TickEvents) {
        let p = self.params;
        let probe = p.max_step_height + 2.0 * SKIN;
        let down: Hit = world.trace(&p.shape, self.location, sub(self.location, [0.0, 0.0, probe]));
        events.start_solid |= down.start_solid;
        if down.blocked() && down.normal[2] >= p.walkable_floor_z {
            self.location = down.end;
            self.floor_normal = down.normal;
            self.velocity[2] = 0.0;
        } else {
            self.physics = Physics::Falling;
            self.velocity[2] = 0.0;
            events.left_ground = true;
        }
    }

    /// Section 7: the falling step.
    fn fall(&mut self, world: &CollisionWorld, accel: V3, dt: f64, events: &mut TickEvents) {
        let p = self.params;
        let mut remaining = dt;
        let mut steps = 0u32;
        while remaining > 0.0 && steps < p.max_fall_substeps {
            steps = steps.saturating_add(1);
            let sub_dt = remaining.min(p.fall_substep);
            remaining -= sub_dt;
            let mut a = horizontal(accel);
            let h_speed = length(horizontal(self.velocity));
            if p.limit_fall_accel {
                let mut limit = p.accel_rate * p.air_control;
                if h_speed < p.air_kick_speed && p.air_control > 0.0 {
                    limit += (p.air_kick_speed - h_speed) / sub_dt;
                }
                let m = length(a);
                if m > limit {
                    a = scale(a, limit / m);
                }
            }
            a[2] += p.gravity_z;
            let old_h = h_speed;
            self.velocity = add(scale(self.velocity, 1.0 - p.fluid_friction * sub_dt), scale(a, sub_dt));
            // Section 7.2: at low air control, horizontal speed already at GroundSpeed may not grow.
            if p.air_control <= 0.05 && old_h >= p.ground_speed {
                let h = horizontal(self.velocity);
                let s = length(h);
                if s > old_h {
                    let k = old_h / s;
                    self.velocity = [h[0] * k, h[1] * k, self.velocity[2]];
                }
            }
            let landed = self.fall_move(world, scale(self.velocity, sub_dt), events);
            let speed = length(self.velocity);
            if speed > p.terminal_velocity {
                self.velocity = scale(self.velocity, p.terminal_velocity / speed);
            }
            if landed {
                break;
            }
        }
    }

    /// One falling sub-step's move; true when it landed.
    fn fall_move(&mut self, world: &CollisionWorld, delta: V3, events: &mut TickEvents) -> bool {
        let p = self.params;
        let mut remaining = delta;
        for _ in 0..MAX_MOVE_PARTS {
            if length(remaining) < 1e-9 {
                break;
            }
            let hit = world.trace(&p.shape, self.location, add(self.location, remaining));
            events.start_solid |= hit.start_solid;
            self.location = hit.end;
            if !hit.blocked() {
                break;
            }
            if hit.normal[2] >= p.walkable_floor_z {
                // Section 7.5: a walkable surface lands the pawn.
                events.landed = Some(self.velocity[2]);
                self.physics = Physics::Walking;
                self.floor_normal = hit.normal;
                self.velocity[2] = 0.0;
                return true;
            }
            // Otherwise it slides along the surface and goes on.
            let n = hit.normal;
            remaining = along_plane(scale(remaining, 1.0 - hit.fraction), n);
            let into = dot(self.velocity, n);
            if into < 0.0 {
                self.velocity = sub(self.velocity, scale(n, into));
            }
        }
        false
    }
}
