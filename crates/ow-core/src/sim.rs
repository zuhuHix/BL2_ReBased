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
//! ("Not read yet", and the Open item of "Actor timers"). [`SimClock::advance`] is a placeholder modelled on public
//! UE3 engine behaviour, not on anything read from this game: world delta = real delta times `TimeDilation` (one f32
//! multiply), `TimeSeconds` summed in f32. Two things it leaves out are open: public UE3 clamps the dilated delta to
//! a minimum and maximum frame time after dilation, and pause stops `TimeSeconds` by a route not read here. Whether
//! and how this game does either is UNVERIFIED; the settling read is a bl-sdk capture of `TimeSeconds`,
//! `RealTimeSeconds` and the frame's `DeltaSeconds` per frame under `slomo 0` and while paused.
//! `CustomTimeDilation` is per actor and is not applied here.
//!
//! The clock's dilation is a copy: the authoritative value is the heap's `WorldInfo.TimeDilation`, which script,
//! Kismet, the console and mods can write. The world (W5-WORLD) copies it into the clock with
//! [`SimClock::set_time_dilation`] before each tick. When that call refuses a value (`Err`), the clock keeps its
//! last good dilation while the heap holds the refused one, so the two differ; what W5-WORLD does then (log it, or
//! return `NotYet`) is that lane's decision.

/// The default simulation step, 1/60 s (decision D13). Callers pass it to [`SimClock::advance`]; a golden replay
/// passes its recorded deltas instead.
pub const DEFAULT_STEP_SECONDS: f32 = 1.0 / 60.0;

/// What one [`SimClock::advance`] produced.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Step {
    /// The world delta of this step in seconds: the real delta times the time dilation, in f32. Status: UNVERIFIED
    /// (see the module comment).
    pub dt: f32,
    /// The undilated delta the step was given.
    pub real_dt: f32,
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
            real_dt,
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

    /// Sets the world's time dilation for the following steps, copied from the heap's `WorldInfo.TimeDilation` (the
    /// authoritative value; see the module comment). At 0.0 this placeholder gives a zero world delta; whether the
    /// game clamps the dilated delta afterwards is UNVERIFIED. No clamp is applied to `d`: any clamp the game's
    /// script or engine applies (for example in the game-speed function) was not read and belongs to its caller.
    ///
    /// Script, Kismet, the console or a mod can write any float into `WorldInfo.TimeDilation`, so a bad value is a
    /// runtime case, not a caller bug: a negative, NaN or infinite `d` is refused with [`ClockError`] and the clock
    /// keeps its previous dilation. Whether the game itself guards against such values is UNVERIFIED.
    pub fn set_time_dilation(&mut self, d: f32) -> Result<(), ClockError> {
        if !(d.is_finite() && d >= 0.0) {
            return Err(ClockError::BadTimeDilation(d));
        }
        self.time_dilation = d;
        Ok(())
    }
}

/// A value the clock refuses.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ClockError {
    /// A time dilation that is negative, NaN or infinite.
    BadTimeDilation(f32),
}

impl std::fmt::Display for ClockError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClockError::BadTimeDilation(d) => {
                write!(f, "time dilation must be finite and not negative, got {d}")
            }
        }
    }
}

impl std::error::Error for ClockError {}

/// The game's tick groups, `Object.ETickingGroup` in this build's `Core.upk`: five groups plus `TG_MAX`, which is a
/// count, not a group. The variants are declared in the enum's order, so `Ord` and [`TickGroup::index`] follow it.
///
/// Status: the names, their number and their values are read from the installed `Core.upk` (2026-11-15). The
/// package's name table is alphabetical and says nothing about order; the values come from the enum export itself.
/// `ow-package Core.upk --exports` gives `Object.ETickingGroup` as export 119 (class `Core.Enum`), and
/// `ow-package Core.upk --payload 119` gives its bytes, read with the enum layout the C++ reader already uses
/// (`src/vm.cpp:739`: net index, `None`, next, count, then one name per value, a value's number being its
/// position). The count is 6 and the names in position order are TG_PreAsyncWork (0), TG_DuringAsyncWork (1),
/// TG_PostAsyncWork (2), TG_PostUpdateWork (3), TG_EffectsUpdateWork (4), TG_MAX (5). The export is 68 bytes =
/// 20 + 6 x 8, so the name list ends exactly at the export's end (the structural check of
/// `docs/verification/PHASELOCK_STOCK_DATA.md`, "Enums"). (`--object-dump` refuses an
/// enum export and `--disasm` needs a function, so neither could show it.) That a frame runs the groups in value
/// order, and what each group contains, was not read from the executable: UNVERIFIED.
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
        c.set_time_dilation(0.5).unwrap();
        let s = c.advance(0.1);
        assert_eq!(s.dt.to_bits(), (0.1f32 * 0.5f32).to_bits());
        assert_eq!(c.time_seconds().to_bits(), (0.1f32 * 0.5f32).to_bits());
        assert_eq!(c.real_time_seconds().to_bits(), 0.1f32.to_bits());

        c.set_time_dilation(0.0).unwrap();
        let s = c.advance(0.1);
        assert_eq!(s.dt, 0.0);
        assert_eq!(c.time_seconds().to_bits(), (0.1f32 * 0.5f32).to_bits());
        assert_eq!(c.real_time_seconds().to_bits(), (0.1f32 + 0.1f32).to_bits());

        c.set_time_dilation(2.0).unwrap();
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
                    c.set_time_dilation(d).unwrap();
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
                .map(|s| (s.dt.to_bits(), s.real_dt.to_bits(), s.tick))
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
    fn bad_dilation_is_refused_and_the_old_value_kept() {
        let mut c = SimClock::new_empty();
        c.set_time_dilation(0.5).unwrap();
        for bad in [f32::INFINITY, f32::NEG_INFINITY, f32::NAN, -0.5] {
            let err = c.set_time_dilation(bad).unwrap_err();
            assert!(matches!(err, ClockError::BadTimeDilation(d) if d.to_bits() == bad.to_bits()));
            assert!(err.to_string().contains("finite and not negative"));
            assert_eq!(c.time_dilation(), 0.5);
        }
    }

    #[test]
    fn step_carries_the_undilated_delta() {
        let mut c = SimClock::new_empty();
        c.set_time_dilation(0.25).unwrap();
        let s = c.advance(0.2);
        assert_eq!(s.real_dt.to_bits(), 0.2f32.to_bits());
        assert_eq!(s.dt.to_bits(), (0.2f32 * 0.25f32).to_bits());
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
