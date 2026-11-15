//! Owner: lane W1-SIM. The simulation clock ([`SimClock`]) and the tick groups ([`TickGroup`]).
//!
//! Game time comes only from a [`SimClock`] (architecture rule R6, section 5.1 N9). The clock is advanced by a delta
//! that is an input (decision D13: a fixed [`DEFAULT_STEP_SECONDS`] by default, a recorded dt list for golden
//! replays), so the same dt sequence always gives the same clock, bit for bit.
//!
//! This module holds the clock and the tick groups only. There is no timer queue here: the game's timers are
//! per-actor `Timers` arrays with their own count, rate and per-timer dilation
//! (`docs/verification/NATIVE_ENGINE_CORE.md`, "Actor timers"), so they live on the actor in `ow-world` (W5-WORLD).
//! Actor ticking, latent script and the Kismet impulse stack are not here either.
//!
//! Status: UNVERIFIED as a whole. `NATIVE_ENGINE_CORE.md` records that `WorldInfo.TimeSeconds` advances with the
//! dilated delta and `RealTimeSeconds` with wall time, but where and by what formula the world advances them, and
//! where the world's `TimeDilation` and an actor's `CustomTimeDilation` enter an actor's tick delta, were not located
//! ("Not read yet", and the Open item of "Actor timers"). [`SimClock::advance`] follows the public UE3 convention
//! (world delta = real delta times `TimeDilation`, one f32 multiply; `TimeSeconds` summed in f32) as a placeholder.
//! `CustomTimeDilation` is per actor and is not applied here.

/// The default simulation step, 1/60 s (decision D13). Callers pass it to [`SimClock::advance`]; a golden replay
/// passes its recorded deltas instead.
pub const DEFAULT_STEP_SECONDS: f32 = 1.0 / 60.0;

/// What one [`SimClock::advance`] produced.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Step {
    /// The world delta of this step in seconds: the real delta times the time dilation, in f32. Status: UNVERIFIED
    /// (see the module comment).
    pub dt: f32,
    /// The number of this step, counting from 1 (the first `advance` returns 1). Equals [`SimClock::tick`] after
    /// the call.
    pub tick: u64,
}

/// The simulation clock: a tick counter, game time and real time, and the world's time dilation.
///
/// `time_seconds` is the game's `WorldInfo.TimeSeconds`: an f32 to which each step's world delta is added in f32, so
/// it carries the same rounding the game's f32 field does (later steps lose precision as it grows). `time_f64` sums
/// the same deltas in f64 and is for diagnostics only; no game rule reads it. `real_time_seconds` sums the undilated
/// deltas in f32 (`WorldInfo.RealTimeSeconds`).
#[derive(Debug, Clone, PartialEq)]
pub struct SimClock {
    tick: u64,
    time_f64: f64,
    time_seconds: f32,
    real_time_seconds: f32,
    time_dilation: f32,
}

impl Default for SimClock {
    fn default() -> Self {
        Self::new_empty()
    }
}

impl SimClock {
    /// A clock at its start: tick 0, all times 0, time dilation 1.0 (`WorldInfo.TimeDilation`'s default in the
    /// public UE3 class; UNVERIFIED for this build).
    pub fn new_empty() -> Self {
        SimClock {
            tick: 0,
            time_f64: 0.0,
            time_seconds: 0.0,
            real_time_seconds: 0.0,
            time_dilation: 1.0,
        }
    }

    /// Advances the clock by one step of `real_dt` seconds and returns the step.
    ///
    /// The world delta is `real_dt * time_dilation` in f32; it is added to `time_seconds` (f32) and `time_f64`
    /// (f64), and `real_dt` is added to `real_time_seconds` (f32). Status: UNVERIFIED (where the game applies
    /// `TimeDilation` was not located; see the module comment). A replay must also repeat every
    /// [`SimClock::set_time_dilation`] call before the same step, since the dilation is part of the input.
    ///
    /// # Panics
    ///
    /// If `real_dt` is negative, NaN or infinite. A bad delta is a caller error; it is refused, never clamped.
    pub fn advance(&mut self, real_dt: f32) -> Step {
        assert!(
            real_dt.is_finite() && real_dt >= 0.0,
            "SimClock::advance: the delta must be finite and not negative, got {real_dt}"
        );
        let dt = real_dt * self.time_dilation;
        self.tick += 1;
        self.time_seconds += dt;
        self.real_time_seconds += real_dt;
        // Diagnostic sum only: the f32 delta widened exactly, then summed in f64.
        self.time_f64 += f64::from(dt);
        Step {
            dt,
            tick: self.tick,
        }
    }

    /// The number of steps taken so far.
    pub fn tick(&self) -> u64 {
        self.tick
    }

    /// Game time, `WorldInfo.TimeSeconds`: the f32 sum of the world deltas.
    pub fn time_seconds(&self) -> f32 {
        self.time_seconds
    }

    /// Real time, `WorldInfo.RealTimeSeconds`: the f32 sum of the undilated deltas.
    pub fn real_time_seconds(&self) -> f32 {
        self.real_time_seconds
    }

    /// The f64 sum of the world deltas. Diagnostics only; game rules read [`SimClock::time_seconds`].
    pub fn time_f64(&self) -> f64 {
        self.time_f64
    }

    /// The world's time dilation (`WorldInfo.TimeDilation`) applied by the next [`SimClock::advance`].
    pub fn time_dilation(&self) -> f32 {
        self.time_dilation
    }

    /// Sets the world's time dilation for the following steps. 0.0 stops game time while real time runs on. No
    /// lower or upper clamp is applied: any clamp the game's script or engine applies (for example in the game-speed
    /// function) was not read and belongs to its caller. Status: UNVERIFIED (where the game applies it).
    ///
    /// # Panics
    ///
    /// If `d` is negative, NaN or infinite.
    pub fn set_time_dilation(&mut self, d: f32) {
        assert!(
            d.is_finite() && d >= 0.0,
            "SimClock::set_time_dilation: the dilation must be finite and not negative, got {d}"
        );
        self.time_dilation = d;
    }
}

/// The game's tick groups, `Object.ETickingGroup` in this build's `Core.upk`: five groups plus `TG_MAX`, which is a
/// count, not a group. The variants are declared in the enum's order, so `Ord` and [`TickGroup::index`] follow it.
///
/// Status: the names, their number and their order are read from the installed `Core.upk` (the enum's name list,
/// 2026-11-15). That a frame runs the groups in this order, and what each group contains, is the public UE3
/// convention and was not read from the executable: UNVERIFIED.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TickGroup {
    /// `TG_PreAsyncWork`, value 0.
    PreAsyncWork,
    /// `TG_DuringAsyncWork`, value 1.
    DuringAsyncWork,
    /// `TG_PostAsyncWork`, value 2.
    PostAsyncWork,
    /// `TG_PostUpdateWork`, value 3.
    PostUpdateWork,
    /// `TG_EffectsUpdateWork`, value 4.
    EffectsUpdateWork,
}

impl TickGroup {
    /// Every group, in the enum's order.
    pub const ALL: [TickGroup; 5] = [
        TickGroup::PreAsyncWork,
        TickGroup::DuringAsyncWork,
        TickGroup::PostAsyncWork,
        TickGroup::PostUpdateWork,
        TickGroup::EffectsUpdateWork,
    ];

    /// The enum value as the game stores it in a byte property (an actor's `TickGroup`).
    pub fn index(self) -> u8 {
        match self {
            TickGroup::PreAsyncWork => 0,
            TickGroup::DuringAsyncWork => 1,
            TickGroup::PostAsyncWork => 2,
            TickGroup::PostUpdateWork => 3,
            TickGroup::EffectsUpdateWork => 4,
        }
    }

    /// The group for a stored enum value, or `None` for `TG_MAX` (5) and anything above it.
    pub fn from_index(value: u8) -> Option<TickGroup> {
        TickGroup::ALL.get(usize::from(value)).copied()
    }

    /// The game's name of the group, as the enum spells it.
    pub fn game_name(self) -> &'static str {
        match self {
            TickGroup::PreAsyncWork => "TG_PreAsyncWork",
            TickGroup::DuringAsyncWork => "TG_DuringAsyncWork",
            TickGroup::PostAsyncWork => "TG_PostAsyncWork",
            TickGroup::PostUpdateWork => "TG_PostUpdateWork",
            TickGroup::EffectsUpdateWork => "TG_EffectsUpdateWork",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Invented deltas: a mix of exact, inexact and tiny values.
    const DTS: [f32; 8] = [
        1.0 / 60.0,
        0.016,
        0.033_333,
        0.0,
        0.1,
        1.0 / 30.0,
        0.000_1,
        0.25,
    ];

    fn bits(c: &SimClock) -> (u64, u64, u32, u32, u32) {
        (
            c.tick(),
            c.time_f64().to_bits(),
            c.time_seconds().to_bits(),
            c.real_time_seconds().to_bits(),
            c.time_dilation().to_bits(),
        )
    }

    #[test]
    fn new_clock_is_at_zero_with_dilation_one() {
        let c = SimClock::new_empty();
        assert_eq!(bits(&c), (0, 0, 0, 0, 1.0f32.to_bits()));
        assert_eq!(SimClock::default(), c);
    }

    #[test]
    fn time_seconds_is_the_hand_computed_f32_sum() {
        let mut c = SimClock::new_empty();
        let mut expect: f32 = 0.0;
        let mut expect64: f64 = 0.0;
        for &dt in &DTS {
            c.advance(dt);
            expect += dt;
            expect64 += f64::from(dt);
        }
        assert_eq!(c.time_seconds().to_bits(), expect.to_bits());
        assert_eq!(c.real_time_seconds().to_bits(), expect.to_bits());
        assert_eq!(c.time_f64().to_bits(), expect64.to_bits());
    }

    #[test]
    fn f32_accumulation_rounds_each_step() {
        // 600 steps of 1/60: the f32 running sum drifts from 10.0, and the clock must carry that drift rather than
        // compute 600 * dt or sum in f64.
        let mut c = SimClock::new_empty();
        let mut expect: f32 = 0.0;
        for _ in 0..600 {
            c.advance(DEFAULT_STEP_SECONDS);
            expect += DEFAULT_STEP_SECONDS;
        }
        assert_eq!(c.time_seconds().to_bits(), expect.to_bits());
        // Computed by hand outside Rust (Python, each add rounded to single): 1/60 in f32 is 0x3C888889 and 600 such
        // f32 adds end at 0x41200014 (10.000019...), not 10.0 (0x41200000).
        assert_eq!(DEFAULT_STEP_SECONDS.to_bits(), 0x3C88_8889);
        assert_eq!(c.time_seconds().to_bits(), 0x4120_0014);
        assert_ne!(f64::from(c.time_seconds()), c.time_f64());
        assert_eq!(c.tick(), 600);
    }

    #[test]
    fn time_seconds_stops_growing_once_dt_is_below_half_an_ulp() {
        // At 2^20 s an f32 ulp is 0.125, so a 1/60 step is lost entirely: the f32 field the game keeps does this.
        let mut c = SimClock::new_empty();
        c.advance(1_048_576.0);
        let before = c.time_seconds().to_bits();
        c.advance(DEFAULT_STEP_SECONDS);
        assert_eq!(c.time_seconds().to_bits(), before);
        assert!(c.time_f64() > 1_048_576.0);
    }

    #[test]
    fn time_dilation_scales_the_step_and_not_real_time() {
        let mut c = SimClock::new_empty();
        c.set_time_dilation(0.5);
        let s = c.advance(0.1);
        assert_eq!(s.dt.to_bits(), (0.1f32 * 0.5f32).to_bits());
        assert_eq!(c.time_seconds().to_bits(), (0.1f32 * 0.5f32).to_bits());
        assert_eq!(c.real_time_seconds().to_bits(), 0.1f32.to_bits());

        c.set_time_dilation(0.0);
        let s = c.advance(0.1);
        assert_eq!(s.dt, 0.0);
        assert_eq!(c.time_seconds().to_bits(), (0.1f32 * 0.5f32).to_bits());
        assert_eq!(c.real_time_seconds().to_bits(), (0.1f32 + 0.1f32).to_bits());

        c.set_time_dilation(2.0);
        assert_eq!(c.advance(0.25).dt, 0.5);
        assert_eq!(c.time_dilation(), 2.0);
    }

    #[test]
    fn replay_of_a_recorded_dt_list_is_identical() {
        // The input of a run is the dt list plus the dilation changes before given steps.
        let dilation_at = |i: usize| match i {
            3 => Some(0.25f32),
            6 => Some(1.0f32),
            _ => None,
        };
        let run = || {
            let mut c = SimClock::new_empty();
            let mut steps = Vec::new();
            for (i, &dt) in DTS.iter().enumerate() {
                if let Some(d) = dilation_at(i) {
                    c.set_time_dilation(d);
                }
                steps.push(c.advance(dt));
            }
            (c, steps)
        };
        let (a, sa) = run();
        let (b, sb) = run();
        assert_eq!(bits(&a), bits(&b));
        let step_bits = |s: &[Step]| {
            s.iter()
                .map(|s| (s.dt.to_bits(), s.tick))
                .collect::<Vec<_>>()
        };
        assert_eq!(step_bits(&sa), step_bits(&sb));
        assert_eq!(a, b);
    }

    #[test]
    fn tick_counter_counts_steps_from_one() {
        let mut c = SimClock::new_empty();
        assert_eq!(c.tick(), 0);
        assert_eq!(c.advance(0.0).tick, 1);
        assert_eq!(c.advance(DEFAULT_STEP_SECONDS).tick, 2);
        assert_eq!(c.tick(), 2);
        // A zero delta is a step: the counter moves, the times do not.
        assert_eq!(c.time_seconds().to_bits(), DEFAULT_STEP_SECONDS.to_bits());
    }

    #[test]
    #[should_panic(expected = "finite and not negative")]
    fn negative_delta_is_refused() {
        SimClock::new_empty().advance(-0.016);
    }

    #[test]
    #[should_panic(expected = "finite and not negative")]
    fn nan_delta_is_refused() {
        SimClock::new_empty().advance(f32::NAN);
    }

    #[test]
    #[should_panic(expected = "finite and not negative")]
    fn infinite_dilation_is_refused() {
        SimClock::new_empty().set_time_dilation(f32::INFINITY);
    }

    #[test]
    fn tick_groups_follow_the_enum_order() {
        for (i, g) in TickGroup::ALL.iter().enumerate() {
            assert_eq!(usize::from(g.index()), i);
            assert_eq!(TickGroup::from_index(g.index()), Some(*g));
        }
        assert!(TickGroup::ALL.windows(2).all(|w| w[0] < w[1]));
        assert_eq!(TickGroup::from_index(5), None); // TG_MAX
        assert_eq!(TickGroup::from_index(255), None);
        assert_eq!(
            TickGroup::EffectsUpdateWork.game_name(),
            "TG_EffectsUpdateWork"
        );
    }
}
