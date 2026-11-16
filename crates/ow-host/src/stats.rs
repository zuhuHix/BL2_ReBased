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
    fn millis_prints_three_decimals() {
        assert_eq!(millis(Duration::from_micros(16_667)), "16.667");
        assert_eq!(millis(Duration::ZERO), "0.000");
    }
}
