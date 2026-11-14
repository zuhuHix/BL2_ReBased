//! Owner: lane W1-SIM. The simulation clock and tick groups; the bodies below are placeholders (W0-SCAFFOLD).
//!
//! This module holds `SimClock` and `TickGroup` only. There is no timer queue here: the game's timers are per-actor
//! arrays with their own count and rate (`docs/verification/NATIVE_ENGINE_CORE.md`), so they belong to the world
//! crate.

/// Placeholder for the simulation clock (tick counter, time, `TimeSeconds` accumulated as f32 the way UE3 does,
/// time dilation). Lane W1-SIM replaces the body and keeps the name and [`SimClock::new_empty`].
#[derive(Debug, Default)]
pub struct SimClock {
    _private: (),
}

impl SimClock {
    /// A clock at its start. The real type keeps this constructor, so tests written before it lands still compile.
    pub fn new_empty() -> Self {
        SimClock { _private: () }
    }
}

/// The UE3 tick groups, in the order a frame runs them (architecture section 3.1). Lane W1-SIM owns this type.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TickGroup {
    PreAsyncWork,
    DuringAsyncWork,
    PostAsyncWork,
    PostUpdateWork,
}
