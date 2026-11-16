//! Owner: lane H1. The boundary between the simulation and the host (architecture 3.9 and decision D18).
//!
//! AI-assisted (Claude Code).
//!
//! The simulation (`ow_world::World` and the `Vm` inside it) is `!Send` on purpose (D18): it is created and stepped
//! on one thread. The host talks to it only through the message types here, which are `Send`: the host's
//! [`HostCommand`]s go in, a [`Snapshot`] comes out, and the renderer draws from the snapshot alone. Today the
//! simulation runs on the event-loop thread (one [`Simulation::step`] per frame); moving it to a side thread later
//! changes only `ow-host`, because nothing else crosses this boundary.
//!
//! H1 has no world to load yet, so [`StubWorld`] stands in: it counts steps and time and returns a fixed clear
//! colour. Scene loading (H2) replaces it with a `World`-backed implementation of [`Simulation`].

use std::fmt;
use std::time::Duration;

/// What the host tells the simulation, in the order it happened since the last step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum HostCommand {
    /// The drawable area is now `width` by `height` physical pixels (either may be 0 while minimised).
    Resized { width: u32, height: u32 },
    /// The window was asked to close; this is the last step.
    CloseRequested,
}

/// What the simulation gives the host after a step: everything the renderer may read.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Snapshot {
    /// Steps taken so far, this one included.
    pub step: u64,
    /// Simulated time so far: the sum of the real frame times the host passed in.
    pub sim_time: Duration,
    /// The drawable size the simulation last heard of.
    pub viewport: (u32, u32),
    /// The colour the frame is cleared to, linear RGBA.
    pub clear: [f64; 4],
}

/// Why a step failed. The host stops the frame loop and exits with an error.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct SimError {
    pub message: String,
}

impl fmt::Display for SimError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "simulation step failed: {}", self.message)
    }
}

impl std::error::Error for SimError {}

/// One simulation the host can step, on the thread that owns it.
pub trait Simulation {
    /// Applies `commands`, advances by `real_dt` and returns what to draw.
    fn step(&mut self, commands: &[HostCommand], real_dt: Duration) -> Result<Snapshot, SimError>;
}

/// The clear colour of the stub world: a dark blue-grey, so a presented frame is told apart from a black window.
pub const STUB_CLEAR: [f64; 4] = [0.05, 0.07, 0.10, 1.0];

/// Stands in for the world until H2: no objects, no packages read.
#[derive(Debug, Clone, Default)]
pub struct StubWorld {
    step: u64,
    sim_time: Duration,
    viewport: (u32, u32),
}

impl StubWorld {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Simulation for StubWorld {
    fn step(&mut self, commands: &[HostCommand], real_dt: Duration) -> Result<Snapshot, SimError> {
        for command in commands {
            if let HostCommand::Resized { width, height } = *command {
                self.viewport = (width, height);
            }
        }
        self.step += 1;
        self.sim_time = self.sim_time.saturating_add(real_dt);
        Ok(Snapshot { step: self.step, sim_time: self.sim_time, viewport: self.viewport, clear: STUB_CLEAR })
    }
}

// The message types must stay `Send` (D18): checked at compile time.
const _: fn() = || {
    fn is_send<T: Send>() {}
    is_send::<HostCommand>();
    is_send::<Snapshot>();
    is_send::<SimError>();
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stub_counts_steps_and_time() {
        let mut world = StubWorld::new();
        let a = world.step(&[], Duration::from_millis(16)).unwrap();
        let b = world.step(&[], Duration::from_millis(17)).unwrap();
        assert_eq!((a.step, b.step), (1, 2));
        assert_eq!(b.sim_time, Duration::from_millis(33));
        assert_eq!(b.clear, STUB_CLEAR);
    }

    #[test]
    fn the_stub_keeps_the_last_resize() {
        let mut world = StubWorld::new();
        let commands = [
            HostCommand::Resized { width: 640, height: 480 },
            HostCommand::Resized { width: 800, height: 600 },
        ];
        assert_eq!(world.step(&commands, Duration::ZERO).unwrap().viewport, (800, 600));
        assert_eq!(world.step(&[HostCommand::CloseRequested], Duration::ZERO).unwrap().viewport, (800, 600));
    }
}
