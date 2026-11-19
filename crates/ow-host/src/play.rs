//! Owner: lane H4-PLAY. The play mode: walk a map in the window as the pawn, first person, with WASD, the mouse and
//! space, colliding with the world (`crate::movement`). `ow-host --game <install> --map Sanctuary --play`.
//!
//! AI-assisted (Claude Code). Decision: `docs/decisions/2026-11/2026-11-16-57-*`.
//!
//! **Three parts, split along the host boundary (D18).**
//! - [`InputCollector`], host side: winit's keys and mouse motion become a [`PlayInput`] message per frame. Mouse
//!   pixels become whole rotator units here ([`TURN_PER_PIXEL`], the remainder carried to the next frame), so the
//!   message is integers and booleans only. Escape releases the cursor, a left click takes it again, F1 toggles the
//!   fly camera, losing focus releases everything. The cursor itself is grabbed by `crate::app` from
//!   [`InputCollector::wants_grab`].
//! - [`PlayWorld`], the simulation (`crate::sim::Simulation`): the mover at the fixed step of `--walk`
//!   ([`step_seconds`], decision D13's 1/60 s), on the event-loop thread, from an accumulator of real frame time,
//!   at most [`MAX_STEPS_PER_FRAME`] steps a frame (time beyond that is dropped and counted, so a stall does not
//!   fast-forward the pawn). Its snapshot carries the view: the eye interpolated between the last two steps by the
//!   accumulator's leftover, and the rotation of the last step plus the turn not yet stepped (so the mouse answers
//!   at the frame rate; the rotation is not interpolated). A script ([`ScriptCursor`]) replaces the player's input
//!   when given: one input per step, whatever the frame times, so a scripted run is a function of its script only.
//! - [`PlayDrawer`], the renderer: H2's [`SceneRenderer`] seen from the snapshot's view, with the matrices from
//!   `crate::frame`.
//! - The loaded world (lane H-WORLD, `crate::world`), when given: ticked once per step, after the mover, by the step's
//!   length, so `WorldInfo.TimeSeconds` and `DeltaSeconds` advance with the simulation and a scripted run's world is a
//!   function of its script too. Its pawns go out in the snapshot for the drawer; nothing the drawer does reaches the
//!   world or the mover. The world's state is folded into its own digest ([`WorldStats::digest`]), so the path
//!   digests mean what they meant before.
//!
//! **The view.** The eye is the pawn's centre plus `BaseEyeHeight` 70 (read live, `MovementParams::maya`, the same as
//! `--walk`'s [`Mover::eye`]); the game's eye-height easing and bob (`NATIVE_PLAYER_MOVEMENT.md` section 15) are not
//! applied. The field of view is the record's default (section 19): 70 degrees horizontal at 16:9, which the game
//! stores as the vertical angle `2 atan(tan(35 deg) x 0.5625)`, 42.996 degrees ([`vertical_fov_deg`]); kept
//! vertical at other aspects. UNVERIFIED in the running game, as is everything about the pawn's movement.
//!
//! **Host choices, not the game's:** [`TURN_PER_PIXEL`] (the game's mouse sensitivity is not read), the bindings,
//! [`MAX_STEPS_PER_FRAME`], [`NEAR`], and the headless frame pattern [`HEADLESS_FRAMES`].

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use ow_core::num::{atan_f64, tan_f64, trunc_i32_f64};
use ow_core::sim::DEFAULT_STEP_SECONDS;
use ow_core::units::Rotator;

use crate::camera::{Controls, FlyCamera, PITCH_LIMIT};
use crate::draw::DrawPlan;
use crate::frame::{self, Mat4};
use crate::movement::collide::CollisionWorld;
use crate::movement::geom::{V3, floor_index};
use crate::movement::mover::{MoveInput, Mover};
use crate::movement::walk::{PathDigest, ScriptCursor, Segment};
use crate::render::{DrawReport, Drawer, Lighting, SceneRenderer, TextureSource};
use crate::sim::{HostCommand, PlayInput, ShotView, SimError, Simulation, Snapshot, ViewPose};
use crate::weapon::{Gun, RangeSource, Shot};
use crate::world::{WorldFrame, WorldTick};

/// The simulation step: `--walk`'s, decision D13's default 1/60 s, as `f64`.
pub fn step_seconds() -> f64 {
    f64::from(DEFAULT_STEP_SECONDS)
}

/// The most steps one frame may take; frame time beyond them is dropped ([`PlayStats::dropped_steps`]).
pub const MAX_STEPS_PER_FRAME: u32 = 8;
/// Rotator units per pixel of mouse motion (a host choice, as `crate::camera`'s: about 0.11 degrees).
pub const TURN_PER_PIXEL: f64 = 20.0;
/// The most turn one frame's mouse motion may add, rotator units (four turns): a wild motion is clamped, not
/// wrapped through the int conversion.
pub const MAX_TURN_PER_FRAME: f64 = 4.0 * 65536.0;
/// The default field of view: 70 degrees horizontal at 16:9 (`NATIVE_PLAYER_MOVEMENT.md` sections 18-19).
pub const DEFAULT_FOV_DEG: f64 = 70.0;
/// The near plane, uu (the host's; the game's is not read).
pub const NEAR: f32 = 10.0;
/// The frame times the headless runner cycles through: uneven on purpose, one beyond [`MAX_STEPS_PER_FRAME`] steps, so
/// a test of "the same path whatever the frame times" sees short, long and dropped frames.
pub const HEADLESS_FRAMES: [Duration; 6] = [
    Duration::from_micros(16_667),
    Duration::from_millis(7),
    Duration::from_millis(33),
    Duration::from_millis(4),
    Duration::from_millis(250),
    Duration::from_millis(16),
];
/// A trace sample every this many steps (a quarter second).
pub const TRACE_EVERY: u64 = 15;

/// The vertical field of view of [`DEFAULT_FOV_DEG`] at 16:9: `ToVFOV` of section 18, `2 atan(tan(h / 2) x 0.5625)`.
pub fn vertical_fov_deg() -> f64 {
    let half = DEFAULT_FOV_DEG.to_radians() / 2.0;
    (2.0 * atan_f64(tan_f64(half) * 0.5625)).to_degrees()
}

/// Projection times view for `view` in a `width` x `height` drawable (reverse-Z, no far plane), through
/// `crate::frame`. `None` for a zero size or a view the boundary refuses.
pub fn view_projection(view: &ViewPose, width: u32, height: u32) -> Option<Mat4> {
    view_projection_near(view, width, height, NEAR)
}

/// [`view_projection`] with another near plane, uu (the first-person arms use their own: `crate::anim::arms`).
pub fn view_projection_near(view: &ViewPose, width: u32, height: u32, near: f32) -> Option<Mat4> {
    if width == 0 || height == 0 {
        return None;
    }
    let aspect = width as f32 / height as f32;
    let projection = frame::perspective_reverse_z_infinite(vertical_fov_deg() as f32, aspect, near)?;
    Some(projection.mul(&frame::view_matrix(view.eye, view.rotation)?))
}

// ------------------------------------------------------------------------------------------------------ input

/// Host-side input: winit events in, one [`PlayInput`] per frame out (module documentation).
#[derive(Debug, Clone, Default)]
pub struct InputCollector {
    /// The player drives the pawn (not a scripted run): movement keys count and the cursor is wanted at start.
    interactive: bool,
    held: PlayInput,
    jump: bool,
    toggle_fly: bool,
    /// Turn not yet sent, rotator units (yaw, pitch), with the fraction kept.
    turn: [f64; 2],
    want_grab: bool,
    /// Whether mouse motion turns the view: set by the host from the grab it got.
    look: bool,
    /// The trigger was pressed since the last message (a click shorter than a frame still counts).
    fire_press: bool,
}

impl InputCollector {
    /// A collector for an interactive run (it wants the cursor at once) or a scripted one (keys other than Escape and
    /// F1 do nothing, and the cursor is left alone).
    pub fn new(interactive: bool) -> InputCollector {
        InputCollector { interactive, want_grab: interactive, ..InputCollector::default() }
    }

    pub fn wants_grab(&self) -> bool {
        self.want_grab
    }

    /// The host says whether mouse motion should turn the view (the cursor is grabbed, or grabbing was refused and
    /// the raw motion is used with a visible cursor).
    /// With the cursor let go (Escape, focus lost, a refused grab with no fallback) the trigger is let go too, so a
    /// held button does not keep firing behind the menu-less cursor (review r1 of PR #366).
    pub fn set_look(&mut self, look: bool) {
        self.look = look;
        if !look {
            self.turn = [0.0; 2];
            self.held.fire = false;
            self.fire_press = false;
        }
    }

    pub fn window_event(&mut self, event: &winit::event::WindowEvent) {
        use winit::event::{ElementState, MouseButton, WindowEvent};
        use winit::keyboard::PhysicalKey;
        match event {
            WindowEvent::KeyboardInput { event, is_synthetic, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    self.key(code, event.state == ElementState::Pressed, event.repeat, *is_synthetic);
                }
            }
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => {
                self.left_button(*state == ElementState::Pressed);
            }
            WindowEvent::Focused(false) => self.focus_lost(),
            _ => {}
        }
    }

    /// A key went down (`down`) or up; `repeat` for the system's auto-repeat of a held key; `synthetic` for the
    /// presses and releases winit makes up when the window gains or loses focus with keys held. A made-up press is
    /// ignored, so coming back to the window with W or space held neither walks nor jumps (review r1 of PR #355);
    /// a made-up release still releases.
    pub fn key(&mut self, code: winit::keyboard::KeyCode, down: bool, repeat: bool, synthetic: bool) {
        use winit::keyboard::KeyCode;
        if down && synthetic {
            return;
        }
        let press = down && !repeat;
        match code {
            KeyCode::Escape if press => self.want_grab = false,
            KeyCode::F1 if press => self.toggle_fly = true,
            _ if !self.interactive => {}
            KeyCode::KeyW => self.held.forward = down,
            KeyCode::KeyS => self.held.back = down,
            KeyCode::KeyA => self.held.left = down,
            KeyCode::KeyD => self.held.right = down,
            KeyCode::KeyE => self.held.up = down,
            KeyCode::KeyQ => self.held.down = down,
            KeyCode::ShiftLeft | KeyCode::ShiftRight => self.held.fast = down,
            KeyCode::Space if press => self.jump = true,
            _ => {}
        }
    }

    /// A left click in the window: an interactive run takes the cursor again.
    pub fn click(&mut self) {
        self.want_grab = self.interactive;
    }

    /// The left mouse button (lane H4-WEAPON): while the cursor is held (the view turns) it is the trigger; else a
    /// press only takes the cursor, so the click that takes it does not fire. A release always lets go of the
    /// trigger.
    pub fn left_button(&mut self, down: bool) {
        if !down {
            self.held.fire = false;
        } else if self.look && self.interactive {
            self.held.fire = true;
            self.fire_press = true;
        } else {
            self.click();
        }
    }

    /// The window lost focus: every key counts as released, a pending jump is dropped, the cursor is let go.
    pub fn focus_lost(&mut self) {
        self.held = PlayInput::default();
        self.jump = false;
        self.want_grab = false;
    }

    /// Raw mouse motion, pixels (right and down positive); counted only while the host says the view turns.
    pub fn mouse_motion(&mut self, dx: f64, dy: f64) {
        if !self.look || !dx.is_finite() || !dy.is_finite() {
            return;
        }
        let clamp = |v: f64| v.clamp(-MAX_TURN_PER_FRAME, MAX_TURN_PER_FRAME);
        self.turn[0] = clamp(self.turn[0] + dx * TURN_PER_PIXEL);
        // Mouse down is positive; pitch up is positive.
        self.turn[1] = clamp(self.turn[1] - dy * TURN_PER_PIXEL);
    }

    /// This frame's message: the held keys, the presses since the last one, and the whole rotator units turned (the
    /// fraction stays for the next frame).
    pub fn take(&mut self) -> PlayInput {
        let whole = |v: f64| trunc_i32_f64(v.trunc());
        let (yaw, pitch) = (whole(self.turn[0]), whole(self.turn[1]));
        self.turn[0] -= f64::from(yaw);
        self.turn[1] -= f64::from(pitch);
        let mut out = self.held;
        // Held now, or pressed since the last message (review r1 of PR #366: a click shorter than a frame).
        // The latch is taken first: `held || take(..)` would skip the take while held and fire again next frame.
        let pressed = std::mem::take(&mut self.fire_press);
        out.fire = self.held.fire || pressed;
        out.jump = std::mem::take(&mut self.jump);
        out.toggle_fly = std::mem::take(&mut self.toggle_fly);
        out.yaw = yaw;
        out.pitch = pitch;
        out
    }
}

// ------------------------------------------------------------------------------------------------- simulation

/// What a play run did, shared with the caller.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlayStats {
    pub steps: u64,
    pub frames: u64,
    /// Steps' worth of frame time dropped beyond [`MAX_STEPS_PER_FRAME`].
    pub dropped_steps: u64,
    pub max_steps_in_frame: u32,
    /// The path digest over every step's location (`crate::movement::walk::PathDigest`, the one `--walk` prints).
    pub digest: PathDigest,
    /// For a scripted run, one entry per script segment in script order, a segment of no tick included: its index
    /// in the script, its name, the steps and the digest at its end (for a segment of no tick, those before the next
    /// step, as `walk::run` records it).
    pub segments: Vec<(usize, String, u64, PathDigest)>,
    /// The pawn's location every [`TRACE_EVERY`] steps (step count, location).
    pub trace: Vec<(u64, V3)>,
    pub end: V3,
    /// The script ran to its end.
    pub finished: bool,
    pub fly_toggles: u32,
    /// Every shot of the weapon (lane H4-WEAPON), with the label of what it hit.
    pub shots: Vec<(Shot, String)>,
    /// A digest over every shot's step, end point and damage, so two runs that fire alike compare in one number.
    pub shot_digest: PathDigest,
    /// The loaded world's side of the run, when there is one (lane H-WORLD).
    pub world: Option<WorldStats>,
}

/// What the loaded world did over a play run (lane H-WORLD).
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WorldStats {
    /// Ticks the play loop asked for (one per step).
    pub requested: u64,
    /// What the last tick left (its halt included).
    pub last: WorldFrame,
    /// A digest over, per step: the world's completed ticks, `TimeSeconds` and `DeltaSeconds` (their bits, -1 when
    /// absent), the pawn count and every pawn's location and rotation.
    pub digest: PathDigest,
    /// Every [`TRACE_EVERY`] steps: the step, the world's completed ticks and `TimeSeconds`.
    pub trace: Vec<(u64, u64, Option<f32>)>,
}

impl WorldStats {
    /// Folds step `step`'s frame in.
    pub fn record(&mut self, step: u64, frame: WorldFrame) {
        self.requested = self.requested.saturating_add(1);
        let bits = |v: Option<f32>| v.map_or(-1.0, |f| f64::from(f.to_bits()));
        self.digest.push([frame.ticks as f64, bits(frame.time_seconds), bits(frame.delta_seconds)]);
        self.digest.push([frame.pawns.len() as f64, 0.0, 0.0]);
        for p in &frame.pawns {
            self.digest.push(p.location.map(f64::from));
            self.digest.push([f64::from(p.rotation.pitch), f64::from(p.rotation.yaw), f64::from(p.rotation.roll)]);
        }
        if step % TRACE_EVERY == 0 {
            self.trace.push((step, frame.ticks, frame.time_seconds));
        }
        self.last = frame;
    }
}

/// The play simulation (module documentation).
pub struct PlayWorld {
    world: CollisionWorld,
    mover: Mover,
    prev_eye: V3,
    accumulator: f64,
    script: Option<ScriptCursor>,
    held: PlayInput,
    jump: bool,
    /// Turn received and not yet stepped, rotator units (yaw, pitch).
    turn: (i32, i32),
    fly: Option<FlyCamera>,
    viewport: (u32, u32),
    sim_time: Duration,
    stats: Rc<RefCell<PlayStats>>,
    /// The next script segment to report in [`PlayStats::segments`].
    reported: usize,
    /// The held weapon (lane H4-WEAPON), the player's trigger, and the last shot (its step, impact, damage known).
    gun: Gun,
    fire: bool,
    /// The trigger came in a message since the last step: a press that ended before any step still fires one (latched
    /// as the jump is; review r1 of PR #366).
    fire_latched: bool,
    last_shot: Option<(u64, Option<V3>, bool)>,
    /// What each brush's placement is called (by `Brush::source`), for the shot log.
    labels: Vec<String>,
    /// Print each shot as it fires (an interactive run).
    echo: bool,
    /// The loaded world, ticked once per step (lane H-WORLD).
    loaded: Option<Box<dyn WorldTick>>,
}

impl PlayWorld {
    /// The pawn `mover` in `world`; driven by `script` when given, else by [`HostCommand::Play`] messages.
    pub fn new(world: CollisionWorld, mover: Mover, script: Option<Vec<Segment>>, stats: Rc<RefCell<PlayStats>>) -> PlayWorld {
        let prev_eye = mover.eye();
        stats.borrow_mut().end = mover.location;
        PlayWorld {
            world,
            mover,
            prev_eye,
            accumulator: 0.0,
            script: script.map(|s| ScriptCursor::new(s, step_seconds())),
            held: PlayInput::default(),
            jump: false,
            turn: (0, 0),
            fly: None,
            viewport: (0, 0),
            sim_time: Duration::ZERO,
            stats,
            reported: 0,
            gun: Gun::new(Err(crate::weapon::equipped_weapon())),
            fire: false,
            fire_latched: false,
            last_shot: None,
            labels: Vec::new(),
            echo: false,
            loaded: None,
        }
    }

    /// The same play run on a loaded world (lane H-WORLD): `world` is ticked once per step, after the mover, and
    /// `pawns` (the world as loaded) go out in the snapshots until the first tick.
    pub fn with_world(mut self, world: Box<dyn WorldTick>, pawns: Vec<crate::world::PawnActor>) -> PlayWorld {
        self.loaded = Some(world);
        let last = WorldFrame { pawns, ..WorldFrame::default() };
        self.stats.borrow_mut().world = Some(WorldStats { last, ..WorldStats::default() });
        self
    }

    /// The same world with `gun` held, `labels` naming each brush's placement (by `Brush::source`) in the shot log,
    /// and each shot printed as it fires when `echo` is set.
    pub fn with_gun(mut self, gun: Gun, labels: Vec<String>, echo: bool) -> PlayWorld {
        self.gun = gun;
        self.labels = labels;
        self.echo = echo;
        self
    }

    /// One step's trigger: the script segment's, else the player's, held or latched since the last step (never while
    /// the fly camera is on). The latch is used up by the step either way.
    fn trigger(&mut self, segment: Option<usize>) -> bool {
        let latched = std::mem::take(&mut self.fire_latched);
        match (&self.script, segment) {
            (Some(cursor), Some(s)) => cursor.script().get(s).is_some_and(|s| s.fire),
            (Some(_), None) => false,
            (None, _) => (self.fire || latched) && self.fly.is_none(),
        }
    }

    /// The label of brush `brush`'s placement.
    fn label(&self, brush: u32) -> String {
        self.world
            .brushes()
            .get(brush as usize)
            .and_then(|b| self.labels.get(b.source as usize))
            .cloned()
            .unwrap_or_else(|| format!("brush {brush}"))
    }

    /// Reports every script segment before `limit` not reported yet, at the steps and digest so far.
    fn report_segments_before(&mut self, limit: usize) {
        let Some(cursor) = &self.script else { return };
        let mut stats = self.stats.borrow_mut();
        while self.reported < limit {
            let Some(s) = cursor.script().get(self.reported) else { break };
            let (steps, digest) = (stats.steps, stats.digest);
            stats.segments.push((self.reported, s.name.to_string(), steps, digest));
            self.reported = self.reported.saturating_add(1);
        }
    }

    pub fn mover(&self) -> &Mover {
        &self.mover
    }

    /// The next step's input and, for a script, the segment it comes from: the script's, or the player's (nothing
    /// while the fly camera is on). `None`: the script is done.
    fn next_input(&mut self) -> Option<(Option<usize>, MoveInput)> {
        if let Some(cursor) = self.script.as_mut() {
            return cursor.next_input().map(|(segment, input)| (Some(segment), input));
        }
        if self.fly.is_some() {
            return Some((None, MoveInput::default()));
        }
        let axis = |plus: bool, minus: bool| f64::from(u8::from(plus)) - f64::from(u8::from(minus));
        let (yaw, pitch) = std::mem::take(&mut self.turn);
        Some((
            None,
            MoveInput::new(
                axis(self.held.forward, self.held.back),
                axis(self.held.right, self.held.left),
                std::mem::take(&mut self.jump),
                yaw,
                pitch,
            ),
        ))
    }

    fn hear(&mut self, input: &PlayInput) {
        if input.toggle_fly {
            self.stats.borrow_mut().fly_toggles += 1;
            self.fly = match self.fly {
                Some(_) => None,
                None => Some(FlyCamera::at(self.view(1.0).eye, self.view(1.0).rotation)),
            };
        }
        self.held = *input;
        self.fire = input.fire;
        self.fire_latched |= input.fire;
        match self.fly.as_mut() {
            Some(fly) => {
                fly.yaw = (f64::from(fly.yaw) + f64::from(input.yaw)).rem_euclid(65536.0) as f32;
                fly.pitch = (fly.pitch + input.pitch as f32).clamp(-PITCH_LIMIT, PITCH_LIMIT);
            }
            None if self.script.is_none() => {
                self.jump |= input.jump;
                self.turn = (self.turn.0.saturating_add(input.yaw), self.turn.1.saturating_add(input.pitch));
            }
            None => {}
        }
    }

    /// The view at `alpha` (0: the step before the last, 1: the last) of the way between the last two steps.
    fn view(&self, alpha: f64) -> ViewPose {
        if let Some(fly) = &self.fly {
            return ViewPose { eye: fly.position, rotation: fly.rotation(), fly: true };
        }
        let eye = self.mover.eye();
        let a = alpha.clamp(0.0, 1.0);
        let lerp = |i: usize| (self.prev_eye[i] + (eye[i] - self.prev_eye[i]) * a) as f32;
        let p = &self.mover.params;
        let pitch = self.mover.view.pitch.saturating_add(self.turn.1).clamp(p.view_pitch_min, p.view_pitch_max);
        let yaw = frame::normalize_axis(self.mover.view.yaw.wrapping_add(self.turn.0));
        ViewPose { eye: [lerp(0), lerp(1), lerp(2)], rotation: Rotator::new(pitch, yaw, 0), fly: false }
    }

    fn finished(&self) -> bool {
        self.script.as_ref().is_some_and(ScriptCursor::done)
    }
}

impl Simulation for PlayWorld {
    fn step(&mut self, commands: &[HostCommand], real_dt: Duration) -> Result<Snapshot, SimError> {
        for command in commands {
            match command {
                HostCommand::Resized { width, height } => self.viewport = (*width, *height),
                HostCommand::Play(input) => self.hear(input),
                _ => {}
            }
        }
        let dt = step_seconds();
        self.sim_time = self.sim_time.saturating_add(real_dt);
        self.accumulator += real_dt.as_secs_f64();
        let mut taken = 0u32;
        let shots_before = self.stats.borrow().shots.len();
        while self.accumulator >= dt && taken < MAX_STEPS_PER_FRAME && !self.finished() {
            let Some((segment, input)) = self.next_input() else { break };
            // Segments the cursor passed without a tick end where the previous one did.
            if let Some(segment) = segment {
                self.report_segments_before(segment);
            }
            self.prev_eye = self.mover.eye();
            self.mover.tick(&self.world, &input, dt);
            self.accumulator -= dt;
            taken += 1;
            let mut stats = self.stats.borrow_mut();
            stats.steps = stats.steps.saturating_add(1);
            stats.digest.push(self.mover.location);
            stats.end = self.mover.location;
            if stats.steps % TRACE_EVERY == 0 {
                let step = stats.steps;
                stats.trace.push((step, self.mover.location));
            }
            let step = stats.steps;
            drop(stats);
            if let Some(world) = self.loaded.as_mut() {
                let frame = world.tick(DEFAULT_STEP_SECONDS);
                if let Some(w) = self.stats.borrow_mut().world.as_mut() {
                    w.record(step, frame);
                }
            }
            let held = self.trigger(segment);
            if let Some(shot) = self.gun.step(&self.world, held, self.mover.eye(), self.mover.view, dt, step) {
                let label = shot.hit.map_or_else(String::new, |h| self.label(h.brush));
                let mut stats = self.stats.borrow_mut();
                let end = shot.hit.map_or_else(|| add_v(shot.origin, shot.direction, shot.range), |h| h.point);
                stats.shot_digest.push(end);
                stats.shot_digest.push([shot.step as f64, *shot.damage.as_ref().unwrap_or(&-1.0), shot.range]);
                self.last_shot = Some((shot.step, shot.hit.map(|h| h.point), shot.damage.is_ok()));
                if self.echo {
                    println!("{}", shot_text(stats.shots.len(), &shot, &label));
                }
                stats.shots.push((shot, label));
            }
            if let (Some(segment), Some(cursor)) = (segment, &self.script)
                && cursor.at_segment_end()
            {
                self.report_segments_before(segment.saturating_add(1));
            }
        }
        let finished = self.finished();
        if finished {
            // Trailing segments of no tick.
            let total = self.script.as_ref().map_or(0, |c| c.script().len());
            self.report_segments_before(total);
        }
        {
            let mut stats = self.stats.borrow_mut();
            stats.frames = stats.frames.saturating_add(1);
            stats.max_steps_in_frame = stats.max_steps_in_frame.max(taken);
            stats.finished = finished;
            if taken == MAX_STEPS_PER_FRAME && self.accumulator >= dt {
                let whole = floor_index(self.accumulator / dt, i32::MAX as usize);
                stats.dropped_steps = stats.dropped_steps.saturating_add(whole as u64);
                self.accumulator -= whole as f64 * dt;
            }
        }
        if finished {
            self.accumulator = self.accumulator.min(dt);
        }
        if let Some(fly) = self.fly.as_mut() {
            let h = self.held;
            let controls = Controls {
                forward: h.forward,
                back: h.back,
                left: h.left,
                right: h.right,
                down: h.down,
                up: h.up,
                fast: h.fast,
                look: true,
            };
            fly.advance(&controls, real_dt.as_secs_f32());
        }
        Ok(Snapshot {
            step: self.stats.borrow().steps,
            sim_time: self.sim_time,
            viewport: self.viewport,
            clear: crate::sim::STUB_CLEAR,
            view: Some(self.view(self.accumulator / dt)),
            finished,
            shot: self.last_shot.map(|(step, impact, damage_known)| ShotView {
                impact: impact.map(|p| [p[0] as f32, p[1] as f32, p[2] as f32]),
                age_steps: self.stats.borrow().steps.saturating_sub(step),
                this_frame: self.stats.borrow().shots.len() > shots_before,
                damage_known,
            }),
            pawns: self.stats.borrow().world.as_ref().map(|w| w.last.pawns.clone()),
        })
    }
}

/// Runs `script` from `mover` in `world` through [`PlayWorld`] with no window, the frame times cycling through
/// `frames` (each frame one [`Simulation::step`] with no messages), until the script is done. The weapon is the
/// player's equipped one, which is `NotYet` ([`crate::weapon::equipped_weapon`]); [`run_headless_with`] gives one.
pub fn run_headless(world: CollisionWorld, mover: Mover, script: Vec<Segment>, frames: &[Duration]) -> Result<PlayStats, SimError> {
    run_headless_with(world, mover, script, frames, Gun::new(Err(crate::weapon::equipped_weapon())), Vec::new())
}

/// [`run_headless`] with `gun` held and `labels` naming each brush's placement in the shot log.
pub fn run_headless_with(
    world: CollisionWorld,
    mover: Mover,
    script: Vec<Segment>,
    frames: &[Duration],
    gun: Gun,
    labels: Vec<String>,
) -> Result<PlayStats, SimError> {
    run_headless_on(world, mover, script, frames, gun, labels, None)
}

/// A loaded world for a play run (lane H-WORLD): the world and its pawns as loaded.
pub type LoadedWorld = (Box<dyn WorldTick>, Vec<crate::world::PawnActor>);

/// [`run_headless_with`] on a loaded world when `loaded` gives one (lane H-WORLD).
pub fn run_headless_on(
    world: CollisionWorld,
    mover: Mover,
    script: Vec<Segment>,
    frames: &[Duration],
    gun: Gun,
    labels: Vec<String>,
    loaded: Option<LoadedWorld>,
) -> Result<PlayStats, SimError> {
    let stats = Rc::new(RefCell::new(PlayStats::default()));
    let mut sim = PlayWorld::new(world, mover, Some(script), Rc::clone(&stats)).with_gun(gun, labels, false);
    if let Some((w, pawns)) = loaded {
        sim = sim.with_world(w, pawns);
    }
    let frames = if frames.is_empty() { &HEADLESS_FRAMES[..] } else { frames };
    // Ten million frames at the shortest pattern frame is hours of game time: a guard, not a limit anyone meets.
    for i in 0..10_000_000usize {
        let snapshot = sim.step(&[], frames[i % frames.len()])?;
        if snapshot.finished {
            break;
        }
    }
    let out = stats.borrow().clone();
    Ok(out)
}

/// `origin + direction x length`.
fn add_v(origin: V3, direction: V3, length: f64) -> V3 {
    [origin[0] + direction[0] * length, origin[1] + direction[1] * length, origin[2] + direction[2] * length]
}

/// One shot's log line: where it went, what it hit, and its damage number or why there is none.
pub fn shot_text(n: usize, shot: &Shot, label: &str) -> String {
    let f = |x: f64| ow_core::fmt::fmt_f(x, 2);
    let p3 = |v: V3| format!("{} {} {}", f(v[0]), f(v[1]), f(v[2]));
    let range = match shot.range_source {
        RangeSource::Weapon => format!("{} uu (WeaponRange)", f(shot.range)),
        _ => format!("{} uu (host probe range, no weapon range)", f(shot.range)),
    };
    let hit = match &shot.hit {
        Some(h) => format!("hit {} (normal {}) on {label} after {} uu", p3(h.point), p3(h.normal), f(h.distance)),
        None => "no hit".to_string(),
    };
    let damage = match &shot.damage {
        Ok(d) => {
            let pellets = shot.projectiles.map_or_else(String::new, |p| format!(" x{} projectiles", f(p)));
            format!("damage {} per projectile{pellets} (WeaponDamage); against the target: {}", f(*d), crate::weapon::target_damage())
        }
        Err(e) => format!("damage: {e}"),
    };
    format!(
        "shot {n} step {} t={}: from {} along {} for {range}: {hit}; {damage}",
        shot.step,
        f(shot.step as f64 * step_seconds()),
        p3(shot.origin),
        p3(shot.direction),
    )
}

/// The text of a play run's stats: steps, frames, drops, each segment's digest (the same digests `--walk` prints for
/// the same script), the end and the whole path's digest.
pub fn stats_text(s: &PlayStats) -> String {
    use std::fmt::Write as _;
    let p3 = |v: V3| {
        format!(
            "{} {} {}",
            ow_core::fmt::fmt_f(v[0], 2),
            ow_core::fmt::fmt_f(v[1], 2),
            ow_core::fmt::fmt_f(v[2], 2)
        )
    };
    let mut out = String::new();
    for (step, at) in &s.trace {
        let _ = writeln!(out, "play t={} at {}", ow_core::fmt::fmt_f(*step as f64 * step_seconds(), 2), p3(*at));
    }
    for (i, (shot, label)) in s.shots.iter().enumerate() {
        let _ = writeln!(out, "play {}", shot_text(i, shot, label));
    }
    for (i, name, steps, digest) in &s.segments {
        let _ = writeln!(out, "play segment {i} {name}: steps {steps}, digest {:016x}", digest.0);
    }
    let _ = writeln!(
        out,
        "play: {} steps over {} frames, at most {} steps in a frame, {} steps' time dropped, script finished: {}, \
         fly toggles {}, end {}, digest {:016x}",
        s.steps,
        s.frames,
        s.max_steps_in_frame,
        s.dropped_steps,
        s.finished,
        s.fly_toggles,
        p3(s.end),
        s.digest.0
    );
    let _ = writeln!(out, "play shots: {}, shot digest {:016x}", s.shots.len(), s.shot_digest.0);
    if let Some(w) = &s.world {
        out.push_str(&world_text(w));
    }
    out
}

/// The loaded world's lines of a play run's stats (lane H-WORLD): the trace, the pawns, the clock and the digest, and
/// why the world halted if it did.
pub fn world_text(w: &WorldStats) -> String {
    use std::fmt::Write as _;
    let time = |t: Option<f32>| t.map_or_else(|| "none".to_string(), |t| ow_core::fmt::fmt_f(f64::from(t), 6));
    let mut out = String::new();
    for (step, ticks, t) in &w.trace {
        let _ = writeln!(out, "play world step {step}: {ticks} ticks, TimeSeconds {}", time(*t));
    }
    for p in &w.last.pawns {
        let _ = writeln!(out, "play world pawn {}", crate::world::pawn_text(p));
    }
    let _ = writeln!(
        out,
        "play world: {} ticks asked, {} completed, TimeSeconds {}, DeltaSeconds {}, {} pawns, world digest {:016x}",
        w.requested,
        w.last.ticks,
        time(w.last.time_seconds),
        time(w.last.delta_seconds),
        w.last.pawns.len(),
        w.digest.0
    );
    match &w.last.halted {
        Some(h) => {
            let kind = if h.not_yet { "NotYet" } else { "error" };
            let _ = writeln!(out, "play world halted at tick {} ({kind}): {}", h.tick, h.message);
        }
        None => {
            let _ = writeln!(out, "play world halted: no");
        }
    }
    out
}

// --------------------------------------------------------------------------------------------------- drawing

/// Where the play drawer's screenshots went, or why they did not.
#[non_exhaustive]
#[derive(Debug, Clone, Default)]
pub struct PlayDrawReport {
    pub draw: DrawReport,
    pub start_shot: Option<Result<PathBuf, String>>,
    pub end_shot: Option<Result<PathBuf, String>>,
}

/// Whether the start screenshot (asked for at presented frame `frame`) is due at presented frame `presented`: at
/// that frame, or earlier when the run is finishing, so a script shorter than `frame` frames still gets one (it is
/// then the same frame as the end shot).
pub fn start_shot_due(frame: u64, presented: u64, finished: bool) -> bool {
    presented >= frame || finished
}

/// H2's scene renderer seen from the snapshot's view.
pub struct PlayDrawer {
    plan: Option<DrawPlan>,
    lighting: Lighting,
    textures: TextureSource,
    renderer: Option<SceneRenderer>,
    /// The placeholder first-person overlay (lane H4-WEAPON) and the last shot it draws.
    overlay: Option<crate::overlay::Overlay>,
    shot: Option<ShotView>,
    view: Option<ViewPose>,
    finished: bool,
    size: Option<(u32, u32)>,
    window: Option<(u32, u32)>,
    /// The start screenshot after this many presented frames (or on the last frame, [`start_shot_due`]), and the
    /// end one when the snapshot says finished.
    start_shot: Option<(u64, PathBuf)>,
    end_shot: Option<PathBuf>,
    presented: u64,
    last_present: Option<Instant>,
    report: Rc<RefCell<PlayDrawReport>>,
    /// Maya's first-person arms, drawn over the scene when given (lane H6-SKIN, `crate::anim::arms`).
    arms: Option<crate::anim::arms::ArmsLayer>,
    /// Third-person skinned pawns drawn in the scene pass (lane H-PAWNS, `crate::anim::pawns`).
    pawns: Option<crate::anim::pawns::PawnLayer>,
    /// The target format, for screenshots that draw the arms too.
    format: Option<wgpu::TextureFormat>,
}

impl PlayDrawer {
    pub fn new(
        plan: DrawPlan,
        lighting: Lighting,
        textures: TextureSource,
        window: Option<(u32, u32)>,
        start_shot: Option<(u64, PathBuf)>,
        end_shot: Option<PathBuf>,
        report: Rc<RefCell<PlayDrawReport>>,
    ) -> PlayDrawer {
        PlayDrawer {
            plan: Some(plan),
            lighting,
            textures,
            renderer: None,
            overlay: None,
            shot: None,
            view: None,
            finished: false,
            size: None,
            window,
            start_shot,
            end_shot,
            presented: 0,
            last_present: None,
            report,
            arms: None,
            pawns: None,
            format: None,
        }
    }

    /// Draws the first-person arms too (lane H6-SKIN).
    /// Draws third-person pawns too (lane H-PAWNS).
    pub fn with_pawns(mut self, pawns: Option<crate::anim::pawns::PawnLayer>) -> PlayDrawer {
        self.pawns = pawns;
        self
    }

    pub fn with_arms(mut self, arms: Option<crate::anim::arms::ArmsLayer>) -> PlayDrawer {
        self.arms = arms;
        self
    }

    fn shoot(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, path: &PathBuf) -> Result<PathBuf, String> {
        let (view, size) = (self.view.ok_or("no view yet")?, self.size.ok_or("no frame yet")?);
        let vp = view_projection(&view, size.0, size.1).ok_or("the view's matrices are not finite")?;
        let renderer = self.renderer.as_mut().ok_or("the scene is not attached")?;
        let eye = view.eye;
        let lighting = self.lighting;
        let (arms, pawns) = (&mut self.arms, &mut self.pawns);
        let mut rgba = match self.format {
            Some(format) if arms.is_some() || pawns.is_some() => {
                crate::anim::arms::capture_with(device, queue, format, size, &mut |encoder, target| {
                    draw_world(device, queue, encoder, target, size, &vp, eye, &lighting, renderer, arms, pawns)
                })?
            }
            _ => renderer.capture(device, queue, size, &vp, eye)?,
        };
        // The capture draws the scene (with the pawns and the arms) only: the overlay is painted onto it.
        crate::overlay::paint(&mut rgba, size.0, size.1, &crate::overlay::rects(size.0, size.1, self.shot.as_ref(), &vp));
        let png = ow_assets::png::encode(size.0, size.1, &rgba).map_err(|e| e.to_string())?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(path, png).map_err(|e| e.to_string())?;
        Ok(path.clone())
    }
}

/// The scene with the pawns drawn inside its pass (skinned first), then the first-person arms over it: one frame's
/// world, the same for the window and the screenshots (lanes H6-SKIN and H-PAWNS).
#[allow(clippy::too_many_arguments)]
fn draw_world(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    encoder: &mut wgpu::CommandEncoder,
    target: &wgpu::TextureView,
    size: (u32, u32),
    vp: &Mat4,
    eye: [f32; 3],
    lighting: &crate::render::Lighting,
    renderer: &mut SceneRenderer,
    arms: &mut Option<crate::anim::arms::ArmsLayer>,
    pawns: &mut Option<crate::anim::pawns::PawnLayer>,
) -> Result<(), String> {
    match pawns {
        Some(pawns) => {
            pawns.prepare(queue, encoder, vp, lighting)?;
            renderer.encode_with(device, queue, encoder, target, size, vp, eye, &mut |pass| pawns.draw(pass));
        }
        None => renderer.encode(device, queue, encoder, target, size, vp, eye),
    }
    if let Some(arms) = arms {
        arms.encode(device, queue, encoder, target, size, lighting)?;
    }
    Ok(())
}

impl Drawer for PlayDrawer {
    fn window_size(&self) -> Option<(u32, u32)> {
        self.window
    }

    fn attach(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Result<(), String> {
        let started = Instant::now();
        let plan = self.plan.take().ok_or("the scene is attached already")?;
        let renderer = SceneRenderer::new(device, queue, format, &plan, self.lighting, &mut self.textures)?;
        if let Some(arms) = &mut self.arms {
            arms.attach(device, format)?;
        }
        if let Some(pawns) = &mut self.pawns {
            pawns.attach(device, format)?;
        }
        self.format = Some(format);
        let mut report = self.report.borrow_mut();
        report.draw.upload_time = started.elapsed();
        report.draw.upload = renderer.upload;
        report.draw.draw_calls = renderer.draw_calls();
        report.draw.blended_instances = renderer.blended_instances();
        self.renderer = Some(renderer);
        self.overlay = Some(crate::overlay::Overlay::new(device, format));
        Ok(())
    }

    /// Input goes to the [`InputCollector`], not here.
    fn window_event(&mut self, _event: &winit::event::WindowEvent) {}

    fn snapshot(&mut self, snapshot: &Snapshot) {
        if snapshot.view.is_some() {
            self.view = snapshot.view;
        }
        if let Some(arms) = &mut self.arms {
            arms.snapshot(snapshot.view, snapshot.sim_time);
        }
        if let Some(pawns) = &mut self.pawns {
            pawns.snapshot(snapshot.sim_time);
            if let Some(actors) = &snapshot.pawns {
                pawns.place_world(actors);
            }
        }
        self.finished = snapshot.finished;
        self.shot = snapshot.shot;
    }

    fn mouse_motion(&mut self, _dx: f64, _dy: f64) {}

    fn encode(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        _dt: Duration,
    ) -> Result<(), String> {
        self.size = Some(size);
        let view = self.view.ok_or("the simulation gave no view")?;
        let vp = view_projection(&view, size.0, size.1).ok_or("the view's matrices are not finite")?;
        let renderer = self.renderer.as_mut().ok_or("the scene is not attached")?;
        let lighting = self.lighting;
        draw_world(device, queue, encoder, target, size, &vp, view.eye, &lighting, renderer, &mut self.arms, &mut self.pawns)?;
        if let Some(overlay) = &self.overlay {
            overlay.encode(queue, encoder, target, size, &crate::overlay::rects(size.0, size.1, self.shot.as_ref(), &vp));
        }
        Ok(())
    }

    fn presented(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) -> Result<(), String> {
        let now = Instant::now();
        self.presented += 1;
        {
            let mut report = self.report.borrow_mut();
            if let Some(last) = self.last_present {
                report.draw.intervals.push(now.duration_since(last));
            }
            report.draw.size = self.size;
        }
        self.last_present = Some(now);
        match self.start_shot.take() {
            Some((frame, path)) if start_shot_due(frame, self.presented, self.finished) => {
                let shot = self.shoot(device, queue, &path);
                self.report.borrow_mut().start_shot = Some(shot);
            }
            other => self.start_shot = other,
        }
        if self.finished
            && let Some(path) = self.end_shot.take()
        {
            let shot = self.shoot(device, queue, &path);
            self.report.borrow_mut().end_shot = Some(shot);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
