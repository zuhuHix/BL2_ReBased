//! Owner: lane H6-ANIM. Quaternion maths for bone transforms, in `f64` (as `frame.rs` computes), every root through
//! `ow_core::num` (N3).
//!
//! A quaternion is `[x, y, z, w]` (Hamilton), and rotates a point as `q p q*`. A bone transform becomes a
//! [`crate::frame::Affine`] in its row-vector form: row `i` is where the bone's local axis `i` lands in the parent's
//! frame, so `p_parent = p * rows + translation`, and a child's model matrix is `local.then(&parent_model)`.

use ow_assets::skeletal::BoneTransform;
use ow_core::num::sqrt_f64;

use crate::frame::Affine;

pub type Quat = [f64; 4];

/// The quaternion of a stored transform, widened to `f64`.
pub fn quat_of(t: &BoneTransform) -> Quat {
    t.rotation.map(f64::from)
}

/// `q / |q|`, or `None` when `q` has zero (or non-finite) length.
pub fn normalize(q: Quat) -> Option<Quat> {
    let n = sqrt_f64(dot(q, q));
    if n.is_finite() && n > 0.0 {
        Some(q.map(|c| c / n))
    } else {
        None
    }
}

pub fn dot(a: Quat, b: Quat) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3]
}

/// The Hamilton product `a b` (first `b`, then `a`).
pub fn mul(a: Quat, b: Quat) -> Quat {
    let [ax, ay, az, aw] = a;
    let [bx, by, bz, bw] = b;
    [
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    ]
}

pub fn conjugate(q: Quat) -> Quat {
    [-q[0], -q[1], -q[2], q[3]]
}

/// `q p q*` for a unit `q`.
pub fn rotate(q: Quat, p: [f64; 3]) -> [f64; 3] {
    let r = mul(mul(q, [p[0], p[1], p[2], 0.0]), conjugate(q));
    [r[0], r[1], r[2]]
}

/// Normalised linear interpolation on the shorter arc: `b` is negated first when `a . b < 0`. `None` for a zero
/// result (only reachable with non-unit input). Which interpolation the game uses between keys is UNVERIFIED (the
/// export resamples keys to one per frame; the game's own key lerp is not read).
pub fn nlerp(a: Quat, b: Quat, alpha: f64) -> Option<Quat> {
    let b = if dot(a, b) < 0.0 { b.map(|c| -c) } else { b };
    normalize([0, 1, 2, 3].map(|i| a[i] * (1.0 - alpha) + b[i] * alpha))
}

/// A bone transform (unit-normalised rotation, translation) as an [`Affine`]. `None` for a zero quaternion.
pub fn affine(rotation: Quat, translation: [f64; 3]) -> Option<Affine> {
    let q = normalize(rotation)?;
    let rows = [
        rotate(q, [1.0, 0.0, 0.0]),
        rotate(q, [0.0, 1.0, 0.0]),
        rotate(q, [0.0, 0.0, 1.0]),
    ];
    Some(Affine { rows, translation })
}

/// The inverse of a rigid [`Affine`] (orthonormal rows, as [`affine`] builds): the transposed rows and the
/// translation taken back through them.
pub fn rigid_inverse(a: &Affine) -> Affine {
    let mut rows = [[0.0; 3]; 3];
    for (i, row) in rows.iter_mut().enumerate() {
        for (j, cell) in row.iter_mut().enumerate() {
            *cell = a.rows[j][i];
        }
    }
    let t = a.translation;
    let translation =
        [0, 1, 2].map(|j| -(t[0] * a.rows[j][0] + t[1] * a.rows[j][1] + t[2] * a.rows[j][2]));
    Affine { rows, translation }
}

/// The largest absolute difference between two affines' entries.
pub fn max_difference(a: &Affine, b: &Affine) -> f64 {
    let mut worst: f64 = 0.0;
    for i in 0..3 {
        for j in 0..3 {
            worst = worst.max((a.rows[i][j] - b.rows[i][j]).abs());
        }
        worst = worst.max((a.translation[i] - b.translation[i]).abs());
    }
    worst
}
