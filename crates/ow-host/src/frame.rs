//! Owner: lane H3. The unit and axis boundary (architecture rule R9, decision D11): the one module where the game's
//! units meet the engine's frame. Every crate below `ow-host` keeps the game's units (uu, rotator units at 65,536 per
//! turn, Z-up, seconds as `f32`); the renderer gets its matrices from here and input goes back to uu through here.
//!
//! AI-assisted (Claude Code). Decision: `docs/decisions/2026-11/2026-11-16-45-*`.
//!
//! # The engine frame
//!
//! **The engine's world frame is the game's frame**: X forward, Y right, Z up, left-handed, one engine unit = one uu,
//! `f32`. A position therefore crosses the boundary unchanged ([`position_to_engine`] is the identity), and an
//! actor's matrix is the UE3 matrix stored column-major ([`Affine::to_engine`]). The one real change of axes is the
//! view: [`view_matrix`] maps the camera's axes (X forward, Y right, Z up) to wgpu's view space (x right, y up,
//! z into the screen), which is the cyclic swap (Y, Z, X) of a left-handed frame onto a left-handed frame. Its
//! determinant is +1, so the view mirrors nothing. A placement can: a negative `DrawScale3D` (or `Scale3D`)
//! component gives the model matrix a negative determinant ([`Affine::determinant`], [`Affine::mirrors`]).
//!
//! Why this and not a right-handed Y-up world (glTF's): that needs a reflection, and a reflection flips triangle
//! winding, cross products and the sign of every roll. Keeping the game's frame puts every one of those risks in
//! one 3x3 permutation that the tests pin, and leaves package data (vertices, normals, placements) as stored.
//!
//! **Clip space** is wgpu's: NDC x right, y up, depth 0..1. Depth is **reverse-Z** (near plane 1, far plane 0,
//! compare `Greater`, clear to 0); with an `f32` depth buffer it keeps precision across Sanctuary's ten thousand
//! and more uu. Two forms exist: [`perspective_reverse_z_infinite`] (no far plane, depth = near / z), which is the
//! form the Phase 0R wgpu benchmark (B2) used (its `perspective_infinite_reverse_lh` call, read by the review of
//! PR #343 in the benchmark source under the desktop's `local/bench/`, not on this PC;
//! `docs/verification/ENGINE_BENCHMARK.md` names only reverse-Z), and [`perspective_reverse_z`] with a finite far
//! plane.
//!
//! Front faces: the view is proper, so the package's index order reaches the screen with its handedness intact
//! except where the model matrix mirrors; the renderer flips the front face for a draw whose
//! [`Affine::determinant`] is negative (B2 used `FrontFace::Cw` for those draws). Which `FrontFace` value the
//! unmirrored case takes is for the renderer lane (H2) to settle with a drawn test; the retired UE5 host's OBJ
//! adapter noted that UE3 index order gives face cross products opposite to the stored outward normals, which in
//! this frame predicts counter-clockwise in NDC (UNVERIFIED until drawn).
//!
//! **Bad input.** Everything built from game data returns `Option` and is `None` unless every output entry is a
//! finite `f32`: [`Placement::affine`], [`component_to_world`], [`Affine::from_ue3_matrix`], [`view_matrix`], the
//! two perspective forms and [`rotator_from_engine`]. [`Affine::then`] and [`Affine::to_engine`] take any
//! [`Affine`]; a caller that builds or composes one by hand checks it with [`Affine::is_finite`] first.
//!
//! # Where each convention comes from
//!
//! - Rotator to axes: `src/natives_core.cpp:437-443` (`GetAxes`: rows X forward, Y right, Z up), the same matrix as
//!   `tools/crosscheck_blcmm_dumps.py` `rotation_matrix`.
//! - Placement: `tools/prepare_level.py` `transform` (scale = `DrawScale3D` x `DrawScale`, for a component
//!   `Scale3D` x `Scale`), `tools/crosscheck_blcmm_dumps.py` `transform_matrix` (scale multiplies each row of the
//!   rotation, row-vector points, translation last) and `compose` (component first, then actor). Composed that way,
//!   4209 Sanctuary placements reproduced the running engine's own `_LocalToWorld` from its `obj dump` to 1e-3 on
//!   rotation entries and 0.05 cm on translation, the record's unit, which is one uu as the scene is placed
//!   (`docs/verification/BLCMM_DUMP_CROSSCHECK.md`, "Actor placement").
//!   That is the one convention here confirmed against the game, and how.
//! - The cooked `Matrix` of a collection entry: `tools/crosscheck_blcmm_dumps.py` `actor_matrix` (rows times the
//!   entry's scale, translation from the fourth row) and `tools/prepare_level.py` `collection_transforms` (refuses a
//!   non-finite or non-affine matrix).
//! - Rotator wrap: `src/natives_core.cpp:51-55` (`normalizeAxis`).
//! - Direction to rotator: `src/interp.cpp:242-251` (`vectorToRotator`; truncation toward zero, roll 0).
//!
//! **Two rules also live in `ow-vm`**, ported faithfully from the C++: `natives_core.rs` `normalize_axis` (on `i64`)
//! and the `Object.GetAxes` native, which, like the C++, does not wrap the axes before the trigonometry. This module
//! wraps first, on purpose, so rotators a whole turn apart give the same bits; the two agree exactly for axes in
//! -32768..=32767 and to the last bits of `f64` outside it. Moving both into `ow_core::units` is a follow-up, not
//! this lane's.
//!
//! # Precision
//!
//! The maths runs in `f64` and rounds to `f32` once at the end, as the C++ `GetAxes` and the Python composer do; the
//! game's own precision (single, or a sine table) is not read and stays UNVERIFIED. Every trigonometric call goes
//! through `ow_core::num` (N3).
//!
//! # Not here yet
//!
//! `PrePivot` and the component flags `AbsoluteTranslation` / `AbsoluteRotation` / `AbsoluteScale` are not handled:
//! the project's sources refuse or skip them (`tools/collision_volumes.py`, `tools/prepare_minimap.py`), and their
//! order in the game's matrix is not read. A caller with one of them set must not use [`component_to_world`].

use ow_core::num::{
    atan2_f64, cos_f64, round_half_even, sin_f64, sqrt_f64, tan_f64, trunc_i64_wrap_i32,
};
use ow_core::units::Rotator;

// The game-frame half (rotator wrap and axes, `Affine`, `Placement`, `component_to_world`, the direction helpers)
// lives in `ow_assets::placement`, so that the headless mover in `ow-cli` uses the same code (lane
// LINK-2-HEADLESS-MOVER, decision 2026-11-19-66). Re-exported here, so every `crate::frame::...` path is unchanged.
pub use ow_assets::placement::*;

/// Engine units per uu. The engine keeps the game's length unit.
pub const ENGINE_UNITS_PER_UU: f32 = 1.0;

/// The engine matrix of an [`Affine`] (rule R9: the conversion stays in `ow-host`). A trait because [`Affine`] moved
/// to `ow_assets::placement`; `Affine::to_engine` and `a.to_engine()` read as before wherever this module's items are
/// imported.
pub trait ToEngine {
    /// The engine matrix: the same frame, column vectors, column-major. Column `i` is the UE3 row `i`, so the 16
    /// floats are in the order of the UE3 `Matrix`'s `XPlane`, `YPlane`, `ZPlane`, `WPlane` (each X, Y, Z, W).
    fn to_engine(&self) -> Mat4;
}

impl ToEngine for Affine {
    fn to_engine(&self) -> Mat4 {
        let mut cols = [[0.0; 4]; 4];
        for (col, row) in cols.iter_mut().zip(&self.rows) {
            *col = [to_f32(row[0]), to_f32(row[1]), to_f32(row[2]), 0.0];
        }
        cols[3] = [
            to_f32(self.translation[0]),
            to_f32(self.translation[1]),
            to_f32(self.translation[2]),
            1.0,
        ];
        Mat4 { cols }
    }
}

/// Radians to the nearest rotator unit, wrapped into -32768..=32767. No oracle rounds this way (see
/// [`rotator_from_engine`]); nearest makes a rotator survive the round trip through an `f32` matrix.
fn units_nearest(rad: f64) -> i32 {
    // round_half_even gives a whole number; the VM form's int conversion is exact on it (NaN gives 0).
    normalize_axis(trunc_i64_wrap_i32(round_half_even(
        rad * (HALF_TURN / std::f64::consts::PI),
    )))
}

/// An engine matrix: column-major, column vectors (`m * v`), as WGSL's `mat4x4<f32>` reads it; `cols[c][r]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mat4 {
    pub cols: [[f32; 4]; 4],
}

impl Mat4 {
    pub const IDENTITY: Mat4 = Mat4 {
        cols: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    };

    /// `self * rhs`: `rhs` applies first.
    pub fn mul(&self, rhs: &Mat4) -> Mat4 {
        let mut cols = [[0.0; 4]; 4];
        for (c, col) in cols.iter_mut().enumerate() {
            *col = self.apply(rhs.cols[c]);
        }
        Mat4 { cols }
    }

    /// True when all 16 entries are finite.
    pub fn is_finite(&self) -> bool {
        self.cols.iter().flatten().all(|v| v.is_finite())
    }

    /// `self * v` for a homogeneous column vector.
    pub fn apply(&self, v: [f32; 4]) -> [f32; 4] {
        let mut out = [0.0; 4];
        for (r, cell) in out.iter_mut().enumerate() {
            *cell = self.cols[0][r] * v[0]
                + self.cols[1][r] * v[1]
                + self.cols[2][r] * v[2]
                + self.cols[3][r] * v[3];
        }
        out
    }
}

/// A position in uu to the engine: the same numbers (the engine keeps uu and the game's axes).
pub fn position_to_engine(p: Vec3) -> Vec3 {
    p
}

/// An engine position back to uu (input, picking): the same numbers.
pub fn position_from_engine(p: Vec3) -> Vec3 {
    p
}

/// World to view space for a camera at `eye` looking along `rotation`: view x = the camera's right (its Y axis),
/// view y = its up (Z), view z = its forward (X), each measured from the eye. A cyclic swap, determinant +1. `None`
/// when the eye is NaN or infinite, or so far out that the translation does not fit an `f32`.
pub fn view_matrix(eye: Vec3, rotation: Rotator) -> Option<Mat4> {
    let [forward, right, up] = rotator_axes(rotation);
    let e = [f64::from(eye[0]), f64::from(eye[1]), f64::from(eye[2])];
    let dot = |a: [f64; 3]| a[0] * e[0] + a[1] * e[1] + a[2] * e[2];
    let mut cols = [[0.0; 4]; 4];
    for c in 0..3 {
        cols[c] = [to_f32(right[c]), to_f32(up[c]), to_f32(forward[c]), 0.0];
    }
    cols[3] = [
        to_f32(-dot(right)),
        to_f32(-dot(up)),
        to_f32(-dot(forward)),
        1.0,
    ];
    let view = Mat4 { cols };
    view.is_finite().then_some(view)
}

/// View space to wgpu clip space, reverse-Z with a finite far plane: a point at `near` gets depth 1, one at `far`
/// depth 0 (use a `Greater` depth test and clear to 0). `vertical_fov_deg` is the full vertical angle in degrees;
/// which angle the host passes (the game's FOV helpers, `NATIVE_PLAYER_MOVEMENT.md` section 18, `WEAPON_VISUALS.md`
/// section 24) is the camera lane's. `None` unless 0 < fov < 180, aspect > 0 and 0 < near < far, all finite, and
/// every entry of the result is a finite `f32` (a fov of 1e-38 or an aspect of 1e-45 passes the first test and
/// fails this one).
pub fn perspective_reverse_z(
    vertical_fov_deg: f32,
    aspect: f32,
    near: f32,
    far: f32,
) -> Option<Mat4> {
    if !(far.is_finite() && near < far) {
        return None;
    }
    let (n, f) = (f64::from(near), f64::from(far));
    // depth = a + b / z: 1 at z = near, 0 at z = far.
    perspective(vertical_fov_deg, aspect, near, n / (n - f), n * f / (f - n))
}

/// View space to wgpu clip space, reverse-Z with no far plane: depth = near / z, 1 at `near` and falling toward 0
/// with distance; the finite form's depth terms as far goes to infinity (a = 0, b = near). This is the form the B2
/// benchmark used. `None` unless 0 < fov < 180, aspect > 0 and near > 0, all finite, and every entry of the result
/// is a finite `f32`.
pub fn perspective_reverse_z_infinite(
    vertical_fov_deg: f32,
    aspect: f32,
    near: f32,
) -> Option<Mat4> {
    perspective(vertical_fov_deg, aspect, near, 0.0, f64::from(near))
}

/// The shared projection: x and y scaled by the focal length (x divided by the aspect), clip w = view z, clip z =
/// `a` z + `b`.
fn perspective(vertical_fov_deg: f32, aspect: f32, near: f32, a: f64, b: f64) -> Option<Mat4> {
    let fov_ok = vertical_fov_deg > 0.0 && vertical_fov_deg < 180.0;
    if !(fov_ok && aspect.is_finite() && aspect > 0.0 && near.is_finite() && near > 0.0) {
        return None;
    }
    let half = f64::from(vertical_fov_deg) * (std::f64::consts::PI / 360.0);
    let focal = 1.0 / tan_f64(half);
    let projection = Mat4 {
        cols: [
            [to_f32(focal / f64::from(aspect)), 0.0, 0.0, 0.0],
            [0.0, to_f32(focal), 0.0, 0.0],
            [0.0, 0.0, to_f32(a), 1.0],
            [0.0, 0.0, to_f32(b), 0.0],
        ],
    };
    projection.is_finite().then_some(projection)
}

/// The rotator of an engine matrix's rotation (its first three columns are the X, Y and Z axes, each normalised
/// here, so a positive per-axis scale is fine). Pitch and yaw come from the X axis; roll is the angle of the Z axis
/// toward the roll-free right axis (-sin yaw, cos yaw, 0), which follows from the `GetAxes` rows: there Y and Z are
/// the roll-free axes turned by roll. Each angle is rounded to the nearest unit (no oracle: the C++ has no matrix to
/// rotator), so pitch lands in -16384..=16384 and yaw and roll in -32768..=32767. At pitch +-16384 yaw and roll are
/// one angle and may come back as another pair with the same axes. `None` when an axis is zero or not finite, or the
/// matrix mirrors (negative determinant), since no rotator describes a mirror.
///
/// UNVERIFIED: the game's own matrix-to-rotator (if the input path uses one) is not read; this inverts `GetAxes`
/// algebraically and is checked only by round trips in the tests.
pub fn rotator_from_engine(m: &Mat4) -> Option<Rotator> {
    let axis = |c: usize| -> Option<[f64; 3]> {
        let v = [
            f64::from(m.cols[c][0]),
            f64::from(m.cols[c][1]),
            f64::from(m.cols[c][2]),
        ];
        let len = sqrt_f64(v[0] * v[0] + v[1] * v[1] + v[2] * v[2]);
        (len.is_finite() && len > 0.0).then(|| [v[0] / len, v[1] / len, v[2] / len])
    };
    let (x, y, z) = (axis(0)?, axis(1)?, axis(2)?);
    let det = x[0] * (y[1] * z[2] - y[2] * z[1]) - x[1] * (y[0] * z[2] - y[2] * z[0])
        + x[2] * (y[0] * z[1] - y[1] * z[0]);
    if det.is_nan() || det <= 0.0 {
        return None;
    }
    let yaw = atan2_f64(x[1], x[0]);
    let pitch = atan2_f64(x[2], sqrt_f64(x[0] * x[0] + x[1] * x[1]));
    let (sy, cy) = (sin_f64(yaw), cos_f64(yaw));
    let free_right = [-sy, cy];
    let roll = atan2_f64(
        z[0] * free_right[0] + z[1] * free_right[1],
        y[0] * free_right[0] + y[1] * free_right[1],
    );
    Some(Rotator::new(
        units_nearest(pitch),
        units_nearest(yaw),
        units_nearest(roll),
    ))
}

#[cfg(test)]
mod tests;
