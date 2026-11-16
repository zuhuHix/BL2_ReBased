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
//!
//! **The view.** The eye is the pawn's centre plus `BaseEyeHeight` 70 (read live, `MovementParams::maya`, the same as
//! `--walk`'s [`Mover::eye`]); the game's eye-height easing and bob (`NATIVE_PLAYER_MOVEMENT.md` section 15) are not
//! applied. The field of view is the record's default (section 19): 70 degrees horizontal at 16:9, which the game
//! stores as the vertical angle `2 atan(tan(35 deg) x 0.5625)`, about 43.07 degrees ([`vertical_fov_deg`]); kept
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
use crate::render::{DrawReport, Drawer, SceneRenderer, TextureSource};
use crate::sim::{HostCommand, PlayInput, SimError, Simulation, Snapshot, ViewPose};

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
    if width == 0 || height == 0 {
        return None;
    }
    let aspect = width as f32 / height as f32;
    let projection = frame::perspective_reverse_z_infinite(vertical_fov_deg() as f32, aspect, NEAR)?;
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
    pub fn set_look(&mut self, look: bool) {
        self.look = look;
        if !look {
            self.turn = [0.0; 2];
        }
    }

    pub fn window_event(&mut self, event: &winit::event::WindowEvent) {
        use winit::event::{ElementState, MouseButton, WindowEvent};
        use winit::keyboard::PhysicalKey;
        match event {
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    self.key(code, event.state == ElementState::Pressed, event.repeat);
                }
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } => self.click(),
            WindowEvent::Focused(false) => self.focus_lost(),
            _ => {}
        }
    }

    /// A key went down (`down`) or up; `repeat` for the system's auto-repeat of a held key.
    pub fn key(&mut self, code: winit::keyboard::KeyCode, down: bool, repeat: bool) {
        use winit::keyboard::KeyCode;
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
    /// For a scripted run: at each segment's end, its name, the steps so far and the digest so far.
    pub segments: Vec<(String, u64, PathDigest)>,
    /// The pawn's location every [`TRACE_EVERY`] steps (step count, location).
    pub trace: Vec<(u64, V3)>,
    pub end: V3,
    /// The script ran to its end.
    pub finished: bool,
    pub fly_toggles: u32,
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
        }
    }

    pub fn mover(&self) -> &Mover {
        &self.mover
    }

    /// The next step's input: the script's, or the player's (nothing while the fly camera is on). `None`: the script
    /// is done.
    fn next_input(&mut self) -> Option<MoveInput> {
        if let Some(cursor) = self.script.as_mut() {
            return cursor.next_input().map(|(_, input)| input);
        }
        if self.fly.is_some() {
            return Some(MoveInput::default());
        }
        let axis = |plus: bool, minus: bool| f64::from(u8::from(plus)) - f64::from(u8::from(minus));
        let (yaw, pitch) = std::mem::take(&mut self.turn);
        Some(MoveInput::new(
            axis(self.held.forward, self.held.back),
            axis(self.held.right, self.held.left),
            std::mem::take(&mut self.jump),
            yaw,
            pitch,
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
        while self.accumulator >= dt && taken < MAX_STEPS_PER_FRAME && !self.finished() {
            let Some(input) = self.next_input() else { break };
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
            if let Some(cursor) = &self.script
                && cursor.at_segment_end()
                && let Some(s) = cursor.script().get(cursor.segment())
            {
                let (steps, digest) = (stats.steps, stats.digest);
                stats.segments.push((s.name.to_string(), steps, digest));
            }
        }
        let finished = self.finished();
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
        })
    }
}

/// Runs `script` from `mover` in `world` through [`PlayWorld`] with no window, the frame times cycling through
/// `frames` (each frame one [`Simulation::step`] with no messages), until the script is done.
pub fn run_headless(world: CollisionWorld, mover: Mover, script: Vec<Segment>, frames: &[Duration]) -> Result<PlayStats, SimError> {
    let stats = Rc::new(RefCell::new(PlayStats::default()));
    let mut sim = PlayWorld::new(world, mover, Some(script), Rc::clone(&stats));
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
    for (i, (name, steps, digest)) in s.segments.iter().enumerate() {
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

/// H2's scene renderer seen from the snapshot's view.
pub struct PlayDrawer {
    plan: Option<DrawPlan>,
    textures: TextureSource,
    renderer: Option<SceneRenderer>,
    view: Option<ViewPose>,
    finished: bool,
    size: Option<(u32, u32)>,
    window: Option<(u32, u32)>,
    /// The start screenshot after this many presented frames, and the end one when the snapshot says finished.
    start_shot: Option<(u64, PathBuf)>,
    end_shot: Option<PathBuf>,
    presented: u64,
    last_present: Option<Instant>,
    report: Rc<RefCell<PlayDrawReport>>,
}

impl PlayDrawer {
    pub fn new(
        plan: DrawPlan,
        textures: TextureSource,
        window: Option<(u32, u32)>,
        start_shot: Option<(u64, PathBuf)>,
        end_shot: Option<PathBuf>,
        report: Rc<RefCell<PlayDrawReport>>,
    ) -> PlayDrawer {
        PlayDrawer {
            plan: Some(plan),
            textures,
            renderer: None,
            view: None,
            finished: false,
            size: None,
            window,
            start_shot,
            end_shot,
            presented: 0,
            last_present: None,
            report,
        }
    }

    fn shoot(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, path: &PathBuf) -> Result<PathBuf, String> {
        let (view, size) = (self.view.ok_or("no view yet")?, self.size.ok_or("no frame yet")?);
        let vp = view_projection(&view, size.0, size.1).ok_or("the view's matrices are not finite")?;
        let renderer = self.renderer.as_mut().ok_or("the scene is not attached")?;
        let rgba = renderer.capture(device, queue, size, &vp)?;
        let png = ow_assets::png::encode(size.0, size.1, &rgba).map_err(|e| e.to_string())?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(path, png).map_err(|e| e.to_string())?;
        Ok(path.clone())
    }
}

impl Drawer for PlayDrawer {
    fn window_size(&self) -> Option<(u32, u32)> {
        self.window
    }

    fn attach(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Result<(), String> {
        let started = Instant::now();
        let plan = self.plan.take().ok_or("the scene is attached already")?;
        let renderer = SceneRenderer::new(device, queue, format, &plan, &mut self.textures)?;
        let mut report = self.report.borrow_mut();
        report.draw.upload_time = started.elapsed();
        report.draw.upload = renderer.upload;
        report.draw.draw_calls = renderer.draw_calls();
        self.renderer = Some(renderer);
        Ok(())
    }

    /// Input goes to the [`InputCollector`], not here.
    fn window_event(&mut self, _event: &winit::event::WindowEvent) {}

    fn snapshot(&mut self, snapshot: &Snapshot) {
        if snapshot.view.is_some() {
            self.view = snapshot.view;
        }
        self.finished = snapshot.finished;
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
        renderer.encode(device, queue, encoder, target, size, &vp);
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
        if let Some((frame, path)) = self.start_shot.clone()
            && frame == self.presented
        {
            let shot = self.shoot(device, queue, &path);
            self.report.borrow_mut().start_shot = Some(shot);
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
