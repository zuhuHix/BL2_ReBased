//! Owner: lane H4-MOVEMENT. Vector helpers in `f64`, uu, Z-up. Every root goes through `ow_core::num` (N3).

use ow_core::num::{sqrt_f64, trunc_i32_f64};

/// A point or a direction in uu, Z-up, `f64`.
pub type V3 = [f64; 3];

pub const ZERO: V3 = [0.0; 3];
pub const UP: V3 = [0.0, 0.0, 1.0];

pub fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub fn scale(a: V3, s: f64) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}

pub fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

pub fn length(a: V3) -> f64 {
    sqrt_f64(dot(a, a))
}

/// The horizontal part (Z zeroed).
pub fn horizontal(a: V3) -> V3 {
    [a[0], a[1], 0.0]
}

/// `a` scaled to length 1, or `None` when its length is below `1e-12` or not finite.
pub fn normalize(a: V3) -> Option<V3> {
    let l = length(a);
    (l.is_finite() && l > 1e-12).then(|| scale(a, 1.0 / l))
}

/// `a` with its component along unit `n` removed (a slide along the plane of normal `n`).
pub fn along_plane(a: V3, n: V3) -> V3 {
    sub(a, scale(n, dot(a, n)))
}

pub fn is_finite(a: V3) -> bool {
    a.iter().all(|v| v.is_finite())
}

/// `x` rounded down to an index in `0..=max` (rule N5: no bare float-to-int `as`): NaN and negatives give 0, anything
/// at or above `max` gives `max`. `max` is capped at `i32::MAX`, the range of the conversion used
/// (`ow_core::num::trunc_i32_f64`, which truncates toward zero, so it rounds a non-negative value down).
pub fn floor_index(x: f64, max: usize) -> usize {
    let cap = max.min(i32::MAX as usize);
    if !(x >= 0.0) {
        return 0;
    }
    if x >= cap as f64 {
        return cap;
    }
    usize::try_from(trunc_i32_f64(x)).unwrap_or(0)
}
