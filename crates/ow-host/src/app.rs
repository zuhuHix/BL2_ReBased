//! Owner: lane H1. The window and the frame loop: a winit event loop on the main thread, one simulation step and one
//! frame per redraw.
//!
//! AI-assisted (Claude Code).
//!
//! Each redraw: the commands gathered since the last one (resizes, a close request) and the real time since the last
//! redraw go to [`Simulation::step`]; the [`Snapshot`] it returns is all the renderer reads. Redraws are asked for
//! continuously; with vsync, acquiring the next frame paces the loop. While no frame can be drawn (minimised or
//! occluded window) the loop waits [`SKIP_WAIT`] between tries instead of spinning.

use std::sync::Arc;
use std::time::{Duration, Instant};

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use crate::gpu::{AdapterSummary, FrameOutcome, Gpu};
use crate::sim::{HostCommand, Simulation};

pub const WINDOW_TITLE: &str = "BL2_ReBased";
/// The window's first inner size, in logical pixels.
pub const WINDOW_SIZE: (u32, u32) = (1280, 720);
/// How long the loop waits after a frame it could not draw.
pub const SKIP_WAIT: Duration = Duration::from_millis(10);
/// A `--frames` run that has not presented its frames by then stops with an error (a window kept minimised, say).
pub const FRAMES_TIME_LIMIT: Duration = Duration::from_secs(120);

/// How to run the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunOptions {
    /// Stop after this many presented frames; `None` runs until the window is closed.
    pub frames: Option<u32>,
    pub vsync: bool,
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

/// What a run did, for the binary to print.
#[derive(Debug, Clone)]
pub struct RunReport {
    pub outcome: Outcome,
    pub adapter: Option<AdapterSummary>,
    pub present_mode: Option<String>,
    pub presented: u64,
    pub skipped: u64,
    /// Wall-clock intervals between successive presents.
    pub intervals: Vec<Duration>,
    /// From the first present to the last.
    pub elapsed: Duration,
}

/// Runs the event loop on this (the main) thread until the run ends. Errors before the loop starts (no display, an
/// event loop already made) come back as `Err`; everything after is in the report's [`Outcome`].
pub fn run<S: Simulation>(options: RunOptions, sim: S) -> Result<RunReport, String> {
    let event_loop = EventLoop::new().map_err(|e| format!("cannot start the event loop: {e}"))?;
    let mut app = App::new(options, sim);
    event_loop.run_app(&mut app).map_err(|e| format!("event loop: {e}"))?;
    Ok(app.report)
}

struct App<S> {
    options: RunOptions,
    sim: S,
    window: Option<Arc<Window>>,
    gpu: Option<Gpu>,
    pending: Vec<HostCommand>,
    started: Option<Instant>,
    last_step: Option<Instant>,
    first_present: Option<Instant>,
    last_present: Option<Instant>,
    /// The last frame could not be drawn: wait [`SKIP_WAIT`] before the next try.
    backing_off: bool,
    report: RunReport,
}

impl<S: Simulation> App<S> {
    fn new(options: RunOptions, sim: S) -> Self {
        App {
            options,
            sim,
            window: None,
            gpu: None,
            pending: Vec::new(),
            started: None,
            last_step: None,
            first_present: None,
            last_present: None,
            backing_off: false,
            report: RunReport {
                outcome: Outcome::Closed,
                adapter: None,
                present_mode: None,
                presented: 0,
                skipped: 0,
                intervals: Vec::new(),
                elapsed: Duration::ZERO,
            },
        }
    }

    fn stop(&mut self, event_loop: &ActiveEventLoop, outcome: Outcome) {
        self.report.outcome = outcome;
        event_loop.exit();
    }

    fn open(&mut self, event_loop: &ActiveEventLoop) {
        let attributes = Window::default_attributes()
            .with_title(WINDOW_TITLE)
            .with_inner_size(LogicalSize::new(WINDOW_SIZE.0, WINDOW_SIZE.1));
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(e) => return self.stop(event_loop, Outcome::Failed(format!("cannot open the window: {e}"))),
        };
        match Gpu::new(Arc::clone(&window), event_loop.owned_display_handle(), self.options.vsync) {
            Ok(gpu) => {
                self.report.adapter = Some(gpu.adapter.clone());
                self.report.present_mode = Some(format!("{:?}", gpu.present_mode()));
                let size = window.inner_size();
                self.pending.push(HostCommand::Resized { width: size.width, height: size.height });
                self.gpu = Some(gpu);
            }
            Err(e) => return self.stop(event_loop, Outcome::Failed(e.to_string())),
        }
        self.started = Some(Instant::now());
        window.request_redraw();
        self.window = Some(window);
    }

    fn frame(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        if let (Some(_), Some(started)) = (self.options.frames, self.started) {
            if now.duration_since(started) > FRAMES_TIME_LIMIT {
                let message = format!(
                    "{} of the frames presented within {} s",
                    self.report.presented,
                    FRAMES_TIME_LIMIT.as_secs()
                );
                return self.stop(event_loop, Outcome::Failed(message));
            }
        }
        let dt = self.last_step.map_or(Duration::ZERO, |last| now.duration_since(last));
        self.last_step = Some(now);
        let snapshot = match self.sim.step(&self.pending, dt) {
            Ok(snapshot) => snapshot,
            Err(e) => return self.stop(event_loop, Outcome::Failed(e.to_string())),
        };
        self.pending.clear();
        let Some(gpu) = self.gpu.as_mut() else { return };
        match gpu.render(snapshot.clear) {
            Ok(FrameOutcome::Presented) => {
                let presented_at = Instant::now();
                if let Some(last) = self.last_present {
                    self.report.intervals.push(presented_at.duration_since(last));
                }
                let first = *self.first_present.get_or_insert(presented_at);
                self.last_present = Some(presented_at);
                self.report.elapsed = presented_at.duration_since(first);
                self.report.presented += 1;
                self.backing_off = false;
                if self.options.frames.is_some_and(|n| self.report.presented >= u64::from(n)) {
                    self.stop(event_loop, Outcome::Completed);
                }
            }
            Ok(_) => {
                self.report.skipped += 1;
                self.backing_off = true;
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

    fn new_events(&mut self, _event_loop: &ActiveEventLoop, cause: StartCause) {
        if let (StartCause::ResumeTimeReached { .. }, Some(window)) = (cause, &self.window) {
            window.request_redraw();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
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

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if event_loop.exiting() {
            return;
        }
        if self.backing_off {
            event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + SKIP_WAIT));
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }
    }
}
