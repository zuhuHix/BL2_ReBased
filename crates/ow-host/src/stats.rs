//! Owner: lane H1. Frame-time figures for `--frames` runs.
//!
//! AI-assisted (Claude Code). A frame time here is the wall-clock interval between two successive presents
//! (throughput, not GPU time), as the Phase 0R benchmark measured it.

use std::time::Duration;

/// Median, minimum and maximum of a set of frame intervals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameStats {
    pub intervals: usize,
    pub median: Duration,
    pub min: Duration,
    pub max: Duration,
}

/// `None` for an empty set. For an even count the median is the mean of the two middle values (rounded down to a
/// nanosecond).
pub fn frame_stats(intervals: &[Duration]) -> Option<FrameStats> {
    let mut sorted = intervals.to_vec();
    sorted.sort_unstable();
    let (&min, &max) = (sorted.first()?, sorted.last()?);
    let n = sorted.len();
    let median = if n % 2 == 1 {
        sorted[n / 2]
    } else {
        let (a, b) = (sorted[n / 2 - 1], sorted[n / 2]);
        a + (b - a) / 2
    };
    Some(FrameStats { intervals: n, median, min, max })
}

/// How many intervals a `--frames` run keeps for its median: 100,000 (1.6 MB). A longer run takes the median of its
/// first 100,000 intervals; the count, minimum and maximum still cover every interval.
pub const MAX_SAMPLES: usize = 100_000;

/// Frame intervals in fixed memory: a running count, minimum and maximum over every interval, and the first `cap`
/// intervals kept for the median. A `--window` run uses a cap of 0, so it keeps nothing however long it runs.
#[derive(Debug, Clone, Default)]
pub struct FrameTimer {
    count: usize,
    min: Option<Duration>,
    max: Duration,
    samples: Vec<Duration>,
    cap: usize,
}

/// What a [`FrameTimer`] saw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameSummary {
    /// Every interval pushed.
    pub intervals: usize,
    pub min: Duration,
    pub max: Duration,
    /// Over the kept samples; `None` when none were kept.
    pub median: Option<Duration>,
    pub sampled: usize,
}

impl FrameTimer {
    pub fn new(cap: usize) -> Self {
        FrameTimer { cap, ..Self::default() }
    }

    pub fn push(&mut self, interval: Duration) {
        self.count += 1;
        self.min = Some(self.min.map_or(interval, |m| m.min(interval)));
        self.max = self.max.max(interval);
        if self.samples.len() < self.cap {
            self.samples.push(interval);
        }
    }

    /// `None` before the first interval.
    pub fn summary(&self) -> Option<FrameSummary> {
        let min = self.min?;
        Some(FrameSummary {
            intervals: self.count,
            min,
            max: self.max,
            median: frame_stats(&self.samples).map(|s| s.median),
            sampled: self.samples.len(),
        })
    }
}

/// Milliseconds with three decimals, through `ow_core::fmt` (architecture 5.1 N6).
pub fn millis(d: Duration) -> String {
    ow_core::fmt::fmt_f(d.as_secs_f64() * 1000.0, 3)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(v: &[u64]) -> Vec<Duration> {
        v.iter().map(|&m| Duration::from_millis(m)).collect()
    }

    #[test]
    fn empty_has_no_stats() {
        assert_eq!(frame_stats(&[]), None);
    }

    #[test]
    fn odd_and_even_medians() {
        let odd = frame_stats(&ms(&[9, 1, 5])).unwrap();
        assert_eq!(odd.intervals, 3);
        assert_eq!((odd.median, odd.min, odd.max), (Duration::from_millis(5), Duration::from_millis(1), Duration::from_millis(9)));
        let even = frame_stats(&ms(&[4, 1, 3, 2])).unwrap();
        assert_eq!(even.median, Duration::from_micros(2500));
    }

    #[test]
    fn the_timer_keeps_at_most_its_cap() {
        let mut timer = FrameTimer::new(3);
        assert_eq!(timer.summary(), None);
        for m in [7, 2, 9, 1, 30] {
            timer.push(Duration::from_millis(m));
        }
        let s = timer.summary().unwrap();
        assert_eq!((s.intervals, s.sampled), (5, 3));
        assert_eq!((s.min, s.max), (Duration::from_millis(1), Duration::from_millis(30)));
        assert_eq!(s.median, Some(Duration::from_millis(7)));
    }

    #[test]
    fn a_window_timer_keeps_nothing() {
        let mut timer = FrameTimer::new(0);
        for _ in 0..1000 {
            timer.push(Duration::from_millis(5));
        }
        let s = timer.summary().unwrap();
        assert_eq!((s.intervals, s.sampled, s.median), (1000, 0, None));
        assert_eq!(timer.samples.capacity(), 0);
    }

    #[test]
    fn millis_prints_three_decimals() {
        assert_eq!(millis(Duration::from_micros(16_667)), "16.667");
        assert_eq!(millis(Duration::ZERO), "0.000");
    }
}
