//! Owner: lane H1. The window and the frame loop: a winit event loop on the main thread, one simulation step and one
//! frame per redraw.
//!
//! AI-assisted (Claude Code).
//!
//! Each redraw: the commands gathered since the last one (resizes, a close request) and the real time since the last
//! redraw go to [`Simulation::step`]; the [`Snapshot`] it returns is all the renderer reads. Redraws are asked for
//! continuously; with vsync, acquiring the next frame paces the loop. While no frame can be drawn (minimised or
//! occluded window) the loop waits [`SKIP_WAIT`] between tries instead of spinning.
//!
//! **Play input** (lane H4-PLAY, [`run_play`]): with an [`InputCollector`] the window's keys, clicks, focus and raw
//! mouse motion go to it, and each redraw sends its [`HostCommand::Play`] message before the step. The cursor is
//! grabbed while the collector wants it: locked if the platform allows, else confined to the window, else left free
//! and visible with the raw mouse motion still turning the view ([`Grab::Refused`], the typed fallback). Every change
//! is kept in [`RunReport::cursor`]. A snapshot that says `finished` ends the run as [`Outcome::Completed`] after its
//! frame is presented, or after [`FINISHED_SKIP_LIMIT`] frames in a row that could not be presented. A window that
//! gets no redraw at all (some minimised windows) never counts a skip, so such a run still waits; UNVERIFIED in a real
//! window.
//!
//! A `--frames` run has a deadline ([`frames_time_limit`], or `--time-limit`). It is checked on every wake-up of the
//! loop, not only on redraws, and the loop always asks to be woken by it: a minimised window may get no redraw at all,
//! and the run must still end.

use std::sync::Arc;
use std::time::{Duration, Instant};

use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalSize};
use winit::event::{DeviceEvent, DeviceId, StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{CursorGrabMode, Window, WindowId};

use crate::gpu::{AdapterSummary, FrameOutcome, Gpu};
use crate::play::InputCollector;
use crate::render::Drawer;
use crate::sim::{HostCommand, Simulation};
use crate::stats::{FrameTimer, MAX_SAMPLES};

pub const WINDOW_TITLE: &str = "BL2_ReBased";
/// The window's first inner size, in logical pixels.
pub const WINDOW_SIZE: (u32, u32) = (1280, 720);
/// How long the loop waits after a frame it could not draw.
pub const SKIP_WAIT: Duration = Duration::from_millis(10);
/// How many frames in a row a finished simulation may fail to present (a minimised or hidden window) before the run
/// ends without a last presented frame: about a second at [`SKIP_WAIT`] (review r1 of PR #355).
pub const FINISHED_SKIP_LIMIT: u32 = 100;

/// Whether a run whose simulation says `finished` ends now: on a presented frame at once, else after
/// [`FINISHED_SKIP_LIMIT`] frames in a row that could not be presented.
pub fn finished_run_ends(finished: bool, presented: bool, skips_since_finished: u32) -> bool {
    finished && (presented || skips_since_finished >= FINISHED_SKIP_LIMIT)
}
/// The fixed part of a `--frames` run's default time limit.
pub const FRAMES_TIME_BASE: Duration = Duration::from_secs(60);
/// The part of the default limit per frame asked for: a whole second, about 4 times the slowest frame seen so far
/// (252 ms with vsync on a sleeping display, 2026-11-16).
pub const FRAMES_TIME_PER_FRAME: Duration = Duration::from_secs(1);

/// The default time limit of `--frames n`: [`FRAMES_TIME_BASE`] plus [`FRAMES_TIME_PER_FRAME`] for each frame (360 s
/// for 300). A run that has not presented its frames by then stops with an error (a window kept minimised, say).
pub fn frames_time_limit(n: u32) -> Duration {
    FRAMES_TIME_BASE.saturating_add(FRAMES_TIME_PER_FRAME.saturating_mul(n))
}

/// How to run the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunOptions {
    /// Stop after this many presented frames; `None` runs until the window is closed.
    pub frames: Option<u32>,
    pub vsync: bool,
    /// Replaces [`frames_time_limit`] for a `--frames` run; ignored without `frames`.
    pub time_limit: Option<Duration>,
}

impl RunOptions {
    /// The time limit of a `--frames` run; `None` for a run until closed.
    pub fn limit(&self) -> Option<Duration> {
        self.frames.map(|n| self.time_limit.unwrap_or_else(|| frames_time_limit(n)))
    }
}

/// How the run ended.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Outcome {
    /// `--frames N` presented its N frames.
    Completed,
    /// The window was closed (by the user, or the system).
    Closed,
    /// A window, GPU or simulation error stopped the loop.
    Failed(String),
}

/// What the cursor grab got.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Grab {
    /// Not grabbed (never asked, or let go).
    Released,
    /// Locked in place and hidden.
    Locked,
    /// Kept inside the window and hidden (the platform refused a lock).
    Confined,
    /// The platform refused both; the cursor stays free and visible and the raw motion still turns the view. The
    /// platform's reason.
    Refused(String),
}

/// What a run did, for the binary to print.
#[derive(Debug, Clone)]
pub struct RunReport {
    pub outcome: Outcome,
    pub adapter: Option<AdapterSummary>,
    pub present_mode: Option<String>,
    pub presented: u64,
    pub skipped: u64,
    /// Wall-clock intervals between successive presents: all of them counted, at most [`MAX_SAMPLES`] kept for the
    /// median, and none kept by a run until closed.
    pub timer: FrameTimer,
    /// From the first present to the last.
    pub elapsed: Duration,
    /// Each cursor grab change of a play run, in order.
    pub cursor: Vec<Grab>,
}

/// Runs the event loop on this (the main) thread until the run ends. Errors before the loop starts (no display, an
/// event loop already made) come back as `Err`; everything after is in the report's [`Outcome`].
pub fn run<S: Simulation>(options: RunOptions, sim: S) -> Result<RunReport, String> {
    run_with(options, sim, None)
}

/// [`run`] with a renderer plugged in (lane H2's seam): `drawer` is attached once the device exists, draws each
/// frame instead of the simulation's clear colour, and hears the window's input. With `None` this is [`run`].
pub fn run_with<S: Simulation>(
    options: RunOptions,
    sim: S,
    drawer: Option<Box<dyn Drawer>>,
) -> Result<RunReport, String> {
    let event_loop = EventLoop::new().map_err(|e| format!("cannot start the event loop: {e}"))?;
    let mut app = App::new(options, sim);
    app.drawer = drawer;
    event_loop.run_app(&mut app).map_err(|e| format!("event loop: {e}"))?;
    Ok(app.report)
}

/// [`run_with`] for the play mode: the window's input goes to `input` (module documentation).
pub fn run_play<S: Simulation>(
    options: RunOptions,
    sim: S,
    drawer: Option<Box<dyn Drawer>>,
    input: InputCollector,
) -> Result<RunReport, String> {
    let event_loop = EventLoop::new().map_err(|e| format!("cannot start the event loop: {e}"))?;
    let mut app = App::new(options, sim);
    app.drawer = drawer;
    app.input = Some(input);
    event_loop.run_app(&mut app).map_err(|e| format!("event loop: {e}"))?;
    Ok(app.report)
}

struct App<S> {
    options: RunOptions,
    sim: S,
    window: Option<Arc<Window>>,
    gpu: Option<Gpu>,
    /// The renderer plugged in by [`run_with`], if any.
    drawer: Option<Box<dyn Drawer>>,
    /// The play mode's input ([`run_play`]), and the grab it has now.
    input: Option<InputCollector>,
    grab: Grab,
    pending: Vec<HostCommand>,
    /// When a `--frames` run fails, set when the window opens.
    deadline: Option<Instant>,
    last_step: Option<Instant>,
    first_present: Option<Instant>,
    last_present: Option<Instant>,
    /// The last frame could not be drawn: wait [`SKIP_WAIT`] before the next try.
    backing_off: bool,
    /// Set by the first [`App::stop`]; the outcome it recorded is final.
    stopped: bool,
    /// Frames in a row not presented since the simulation said finished.
    skips_since_finished: u32,
    report: RunReport,
}

impl<S: Simulation> App<S> {
    fn new(options: RunOptions, sim: S) -> Self {
        App {
            options,
            sim,
            window: None,
            gpu: None,
            drawer: None,
            input: None,
            grab: Grab::Released,
            pending: Vec::new(),
            deadline: None,
            last_step: None,
            first_present: None,
            last_present: None,
            backing_off: false,
            stopped: false,
            skips_since_finished: 0,
            report: RunReport {
                outcome: Outcome::Closed,
                adapter: None,
                present_mode: None,
                presented: 0,
                skipped: 0,
                timer: FrameTimer::new(if options.frames.is_some() { MAX_SAMPLES } else { 0 }),
                elapsed: Duration::ZERO,
                cursor: Vec::new(),
            },
        }
    }

    fn stop(&mut self, event_loop: &ActiveEventLoop, outcome: Outcome) {
        self.record_stop(outcome);
        event_loop.exit();
    }

    /// Records the run's outcome the first time only: after a deadline failure, an event already queued (a redraw
    /// that presents frame N, a close) must not turn it into a success. True when this call recorded it.
    fn record_stop(&mut self, outcome: Outcome) -> bool {
        if self.stopped {
            return false;
        }
        self.stopped = true;
        self.report.outcome = outcome;
        true
    }

    fn open(&mut self, event_loop: &ActiveEventLoop) {
        let attributes = Window::default_attributes().with_title(WINDOW_TITLE);
        let attributes = match self.drawer.as_ref().and_then(|d| d.window_size()) {
            Some((w, h)) => attributes.with_inner_size(PhysicalSize::new(w, h)),
            None => attributes.with_inner_size(LogicalSize::new(WINDOW_SIZE.0, WINDOW_SIZE.1)),
        };
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(e) => return self.stop(event_loop, Outcome::Failed(format!("cannot open the window: {e}"))),
        };
        match Gpu::new(Arc::clone(&window), event_loop.owned_display_handle(), self.options.vsync) {
            Ok(gpu) => {
                self.report.adapter = Some(gpu.adapter.clone());
                self.report.present_mode = Some(format!("{:?}", gpu.present_mode()));
                if let Some(drawer) = self.drawer.as_mut()
                    && let Err(e) = drawer.attach(gpu.device(), gpu.queue(), gpu.format())
                {
                    return self.stop(event_loop, Outcome::Failed(format!("attaching the renderer: {e}")));
                }
                let size = window.inner_size();
                self.pending.push(HostCommand::Resized { width: size.width, height: size.height });
                self.gpu = Some(gpu);
            }
            Err(e) => return self.stop(event_loop, Outcome::Failed(e.to_string())),
        }
        self.deadline = self.options.limit().map(|limit| Instant::now() + limit);
        window.request_redraw();
        self.window = Some(window);
    }

    /// Stops a `--frames` run past its deadline; true when it did.
    fn past_deadline(&mut self, event_loop: &ActiveEventLoop) -> bool {
        let (Some(deadline), Some(n), Some(limit)) = (self.deadline, self.options.frames, self.options.limit()) else {
            return false;
        };
        if Instant::now() < deadline || event_loop.exiting() {
            return false;
        }
        let message = format!("{} of {n} frames presented within the {} s limit", self.report.presented, limit.as_secs());
        self.stop(event_loop, Outcome::Failed(message));
        true
    }

    /// Grabs or lets go of the cursor to match what the play input wants, and tells the input whether the mouse
    /// turns the view.
    fn sync_grab(&mut self) {
        let (Some(input), Some(window)) = (self.input.as_mut(), self.window.as_ref()) else { return };
        let want = input.wants_grab();
        let have = !matches!(self.grab, Grab::Released);
        if want != have {
            let got = if want {
                match window.set_cursor_grab(CursorGrabMode::Locked) {
                    Ok(()) => Grab::Locked,
                    Err(_) => match window.set_cursor_grab(CursorGrabMode::Confined) {
                        Ok(()) => Grab::Confined,
                        Err(e) => Grab::Refused(e.to_string()),
                    },
                }
            } else {
                // Letting go cannot be refused in a way that matters: the cursor is shown either way.
                let _ = window.set_cursor_grab(CursorGrabMode::None);
                Grab::Released
            };
            window.set_cursor_visible(!matches!(got, Grab::Locked | Grab::Confined));
            self.report.cursor.push(got.clone());
            self.grab = got;
        }
        input.set_look(!matches!(self.grab, Grab::Released));
    }

    fn frame(&mut self, event_loop: &ActiveEventLoop) {
        // A redraw queued before the run was stopped draws nothing.
        if event_loop.exiting() || self.past_deadline(event_loop) {
            return;
        }
        let now = Instant::now();
        let dt = self.last_step.map_or(Duration::ZERO, |last| now.duration_since(last));
        self.last_step = Some(now);
        self.sync_grab();
        if let Some(input) = self.input.as_mut() {
            self.pending.push(HostCommand::Play(input.take()));
        }
        let snapshot = match self.sim.step(&self.pending, dt) {
            Ok(snapshot) => snapshot,
            Err(e) => return self.stop(event_loop, Outcome::Failed(e.to_string())),
        };
        self.pending.clear();
        if let Some(drawer) = self.drawer.as_mut() {
            drawer.snapshot(&snapshot);
        }
        let Some(gpu) = self.gpu.as_mut() else { return };
        let rendered = match self.drawer.as_mut() {
            Some(drawer) => gpu.render_with(&mut |device, queue, encoder, view, size| {
                drawer.encode(device, queue, encoder, view, size, dt)
            }),
            None => gpu.render(snapshot.clear),
        };
        match rendered {
            Ok(FrameOutcome::Presented) => {
                if let Some(drawer) = self.drawer.as_mut()
                    && let Err(e) = drawer.presented(gpu.device(), gpu.queue())
                {
                    return self.stop(event_loop, Outcome::Failed(e));
                }
                let presented_at = Instant::now();
                if let Some(last) = self.last_present {
                    self.report.timer.push(presented_at.duration_since(last));
                }
                let first = *self.first_present.get_or_insert(presented_at);
                self.last_present = Some(presented_at);
                self.report.elapsed = presented_at.duration_since(first);
                self.report.presented += 1;
                self.backing_off = false;
                if self.options.frames.is_some_and(|n| self.report.presented >= u64::from(n))
                    || finished_run_ends(snapshot.finished, true, 0)
                {
                    self.stop(event_loop, Outcome::Completed);
                }
            }
            Ok(_) => {
                self.report.skipped += 1;
                self.backing_off = true;
                // A script that finishes while the window cannot present (minimised, hidden) ends without its last
                // frame after a bounded wait, instead of waiting for the window to come back.
                if snapshot.finished {
                    self.skips_since_finished = self.skips_since_finished.saturating_add(1);
                }
                if finished_run_ends(snapshot.finished, false, self.skips_since_finished) {
                    self.stop(event_loop, Outcome::Completed);
                }
            }
            Err(e) => self.stop(event_loop, Outcome::Failed(e.to_string())),
        }
    }
}

impl<S: Simulation> ApplicationHandler for App<S> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            self.open(event_loop);
        }
    }

    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        if self.past_deadline(event_loop) {
            return;
        }
        if let (StartCause::ResumeTimeReached { .. }, Some(window)) = (cause, &self.window) {
            window.request_redraw();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        if let Some(drawer) = self.drawer.as_mut() {
            drawer.window_event(&event);
        }
        if let Some(input) = self.input.as_mut() {
            input.window_event(&event);
        }
        match event {
            WindowEvent::CloseRequested => {
                self.pending.push(HostCommand::CloseRequested);
                // The simulation hears of the close in one last step; its snapshot is not drawn.
                let dt = self.last_step.map_or(Duration::ZERO, |last| last.elapsed());
                if let Err(e) = self.sim.step(&self.pending, dt) {
                    return self.stop(event_loop, Outcome::Failed(e.to_string()));
                }
                self.pending.clear();
                self.stop(event_loop, Outcome::Closed);
            }
            WindowEvent::Resized(size) => {
                if let Some(gpu) = self.gpu.as_mut() {
                    gpu.resize(size.width, size.height);
                }
                self.pending.push(HostCommand::Resized { width: size.width, height: size.height });
                self.backing_off = false;
            }
            WindowEvent::RedrawRequested => self.frame(event_loop),
            _ => {}
        }
    }

    fn device_event(&mut self, _event_loop: &ActiveEventLoop, _id: DeviceId, event: DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta } = event {
            if let Some(drawer) = self.drawer.as_mut() {
                drawer.mouse_motion(delta.0, delta.1);
            }
            if let Some(input) = self.input.as_mut() {
                input.mouse_motion(delta.0, delta.1);
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if event_loop.exiting() {
            return;
        }
        // Wake for the next try after a skipped frame, and in any case by the deadline.
        let retry = self.backing_off.then(|| Instant::now() + SKIP_WAIT);
        let wake = match (retry, self.deadline) {
            (Some(retry), Some(deadline)) => Some(retry.min(deadline)),
            (retry, deadline) => retry.or(deadline),
        };
        event_loop.set_control_flow(wake.map_or(ControlFlow::Wait, ControlFlow::WaitUntil));
        if !self.backing_off {
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_outcome_is_final() {
        let options = RunOptions { frames: Some(10), vsync: true, time_limit: None };
        let mut app = App::new(options, crate::sim::StubWorld::new());
        assert!(app.record_stop(Outcome::Failed("deadline".into())));
        assert!(!app.record_stop(Outcome::Completed));
        assert!(!app.record_stop(Outcome::Closed));
        assert_eq!(app.report.outcome, Outcome::Failed("deadline".into()));
    }

    #[test]
    fn a_finished_run_ends_on_a_present_or_after_bounded_skips() {
        assert!(finished_run_ends(true, true, 0));
        assert!(!finished_run_ends(true, false, FINISHED_SKIP_LIMIT - 1));
        assert!(finished_run_ends(true, false, FINISHED_SKIP_LIMIT));
        assert!(!finished_run_ends(false, true, 0));
        assert!(!finished_run_ends(false, false, u32::MAX));
    }

    #[test]
    fn the_default_limit_scales_with_the_frames() {
        assert_eq!(frames_time_limit(300), Duration::from_secs(360));
        assert_eq!(frames_time_limit(1), Duration::from_secs(61));
        assert_eq!(frames_time_limit(u32::MAX), Duration::from_secs(60 + u64::from(u32::MAX)));
    }

    #[test]
    fn the_limit_applies_to_frames_runs_only() {
        let window = RunOptions { frames: None, vsync: true, time_limit: Some(Duration::from_secs(5)) };
        assert_eq!(window.limit(), None);
        let frames = RunOptions { frames: Some(10), vsync: true, time_limit: None };
        assert_eq!(frames.limit(), Some(Duration::from_secs(70)));
        let capped = RunOptions { time_limit: Some(Duration::from_secs(15)), ..frames };
        assert_eq!(capped.limit(), Some(Duration::from_secs(15)));
    }
}
