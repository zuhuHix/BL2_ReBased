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
//! determinant is +1, so nothing is mirrored anywhere on the way to the screen.
//!
//! Why this and not a right-handed Y-up world (glTF's): that needs a reflection, and a reflection flips triangle
//! winding, cross products and the sign of every roll. Keeping the game's frame puts every one of those risks in
//! one 3x3 permutation that the tests pin, and leaves package data (vertices, normals, placements) as stored.
//!
//! **Clip space** is wgpu's: NDC x right, y up, depth 0..1. Depth is **reverse-Z** (near plane 1, far plane 0,
//! compare `Greater`), as the Phase 0R wgpu benchmark (B2) used (`docs/verification/ENGINE_BENCHMARK.md`, "What was
//! missing and had to be written"); with an `f32` depth buffer it keeps precision across Sanctuary's ten thousand
//! and more uu. `local/bench/` was not on this PC, so only the record was read.
//!
//! Front faces: the frame is proper, so the package's index order reaches the screen with its handedness intact.
//! Which wgpu `FrontFace` value that is, is for the renderer lane (H2) to settle with a drawn test; the retired UE5
//! host's OBJ adapter noted that UE3 index order gives face cross products opposite to the stored outward normals,
//! which in this frame predicts counter-clockwise in NDC (UNVERIFIED until drawn).
//!
//! # Where each convention comes from
//!
//! - Rotator to axes: `src/natives_core.cpp:437-443` (`GetAxes`: rows X forward, Y right, Z up), the same matrix as
//!   `tools/crosscheck_blcmm_dumps.py` `rotation_matrix`.
//! - Placement: `tools/prepare_level.py` `transform` (scale = `DrawScale3D` x `DrawScale`, for a component
//!   `Scale3D` x `Scale`), `tools/crosscheck_blcmm_dumps.py` `transform_matrix` (scale multiplies each row of the
//!   rotation, row-vector points, translation last) and `compose` (component first, then actor). Composed that way,
//!   4209 Sanctuary placements reproduced the running engine's own `_LocalToWorld` from its `obj dump` to 1e-3 on
//!   rotation entries and 0.05 uu on translation (`docs/verification/BLCMM_DUMP_CROSSCHECK.md`, "Actor placement").
//!   That is the one convention here confirmed against the game, and how.
//! - The cooked `Matrix` of a collection entry: `tools/crosscheck_blcmm_dumps.py` `actor_matrix` (rows times the
//!   entry's scale, translation from the fourth row) and `tools/prepare_level.py` `collection_transforms` (refuses a
//!   non-finite or non-affine matrix).
//! - Rotator wrap: `src/natives_core.cpp:51-55` (`normalizeAxis`).
//! - Direction to rotator: `src/interp.cpp:242-251` (`vectorToRotator`; truncation toward zero, roll 0).
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

/// A vector in uu (or a unit direction), Z-up, as the game stores it.
pub type Vec3 = [f32; 3];

/// Engine units per uu. The engine keeps the game's length unit.
pub const ENGINE_UNITS_PER_UU: f32 = 1.0;

/// Rotator units in a half turn: `natives_core.cpp:440` and `interp.cpp:232` scale by `pi / 32768`.
const HALF_TURN: f64 = 32768.0;

/// Rounds the `f64` working value to the `f32` the engine stores.
fn to_f32(x: f64) -> f32 {
    // Float-to-float `as` rounds to nearest even (infinity beyond the f32 range); it never saturates like float-to-int.
    x as f32
}

/// Rotator units to radians, after the 16-bit wrap.
fn radians(units: i32) -> f64 {
    f64::from(normalize_axis(units)) * (std::f64::consts::PI / HALF_TURN)
}

/// Radians to the nearest rotator unit, wrapped into -32768..=32767. No oracle rounds this way (see
/// [`rotator_from_engine`]); nearest makes a rotator survive the round trip through an `f32` matrix.
fn units_nearest(rad: f64) -> i32 {
    // round_half_even gives a whole number; the VM form's int conversion is exact on it (NaN gives 0).
    normalize_axis(trunc_i64_wrap_i32(round_half_even(
        rad * (HALF_TURN / std::f64::consts::PI),
    )))
}

/// One rotator axis wrapped into a signed 16-bit turn, -32768..=32767 (`natives_core.cpp:52-55`, `normalizeAxis`):
/// the low 16 bits, read as signed. So 32768 and -32768 are the same angle and give -32768.
pub fn normalize_axis(angle: i32) -> i32 {
    let low = angle & 0xFFFF;
    if low > 32767 { low - 65536 } else { low }
}

/// Every axis of a rotator wrapped by [`normalize_axis`] (`Object.Normalize(rotator)`, `natives_core.cpp:432-435`).
pub fn normalize(r: Rotator) -> Rotator {
    Rotator::new(
        normalize_axis(r.pitch),
        normalize_axis(r.yaw),
        normalize_axis(r.roll),
    )
}

/// The rotator's three axes as rows: X forward, Y right, Z up (`natives_core.cpp:437-443`, `GetAxes`). Each axis is
/// wrapped first, so rotators a whole turn apart give the same bits; for -32768..=32767 this is the C++ formula.
pub fn rotator_axes(r: Rotator) -> [[f64; 3]; 3] {
    let (p, y, rl) = (radians(r.pitch), radians(r.yaw), radians(r.roll));
    let (sp, cp) = (sin_f64(p), cos_f64(p));
    let (sy, cy) = (sin_f64(y), cos_f64(y));
    let (sr, cr) = (sin_f64(rl), cos_f64(rl));
    [
        [cp * cy, cp * sy, sp],
        [sr * sp * cy - cr * sy, sr * sp * sy + cr * cy, -sr * cp],
        [-(cr * sp * cy + sr * sy), cy * sr - cr * sp * sy, cr * cp],
    ]
}

/// A UE3 affine transform in the row-vector convention the C++ and the Python composer use: a point `p` maps to
/// `p * rows + translation`, so row `i` is where the local axis `i` lands (scaled). The same shape as
/// `crosscheck_blcmm_dumps.py`'s `dict(rows, translation)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Affine {
    pub rows: [[f64; 3]; 3],
    pub translation: [f64; 3],
}

impl Affine {
    pub const IDENTITY: Affine = Affine {
        rows: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        translation: [0.0; 3],
    };

    /// `self` first, then `outer` (`crosscheck_blcmm_dumps.py` `compose(inner, outer)`): a component's transform
    /// relative to its actor, then the actor's.
    pub fn then(&self, outer: &Affine) -> Affine {
        let mut rows = [[0.0; 3]; 3];
        let mut translation = [0.0; 3];
        for (i, row) in rows.iter_mut().enumerate() {
            for (j, cell) in row.iter_mut().enumerate() {
                *cell = self.rows[i][0] * outer.rows[0][j]
                    + self.rows[i][1] * outer.rows[1][j]
                    + self.rows[i][2] * outer.rows[2][j];
            }
        }
        for (j, cell) in translation.iter_mut().enumerate() {
            *cell = self.translation[0] * outer.rows[0][j]
                + self.translation[1] * outer.rows[1][j]
                + self.translation[2] * outer.rows[2][j]
                + outer.translation[j];
        }
        Affine { rows, translation }
    }

    /// A point through the transform (`crosscheck_blcmm_dumps.py` `transform_point`).
    pub fn point(&self, p: [f64; 3]) -> [f64; 3] {
        let d = self.direction(p);
        [
            d[0] + self.translation[0],
            d[1] + self.translation[1],
            d[2] + self.translation[2],
        ]
    }

    /// A direction through the transform: rotation and scale, no translation.
    pub fn direction(&self, v: [f64; 3]) -> [f64; 3] {
        let mut out = [0.0; 3];
        for (k, cell) in out.iter_mut().enumerate() {
            *cell = v[0] * self.rows[0][k] + v[1] * self.rows[1][k] + v[2] * self.rows[2][k];
        }
        out
    }

    /// The engine matrix: the same frame, column vectors, column-major. Column `i` is the UE3 row `i`, so the 16
    /// floats are in the order of the UE3 `Matrix`'s `XPlane`, `YPlane`, `ZPlane`, `WPlane` (each X, Y, Z, W).
    pub fn to_engine(&self) -> Mat4 {
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

    /// A cooked collection entry's `Matrix` and the scale kept beside it (`crosscheck_blcmm_dumps.py` `actor_matrix`:
    /// row `i` times `scale[i]`, translation from `WPlane`). `None` for a non-finite matrix or one whose W column is
    /// not (0, 0, 0, 1) within 1e-5, which `prepare_level.py` `collection_transforms` refuses too.
    pub fn from_ue3_matrix(m: &Ue3Matrix, scale: Vec3) -> Option<Affine> {
        let planes = [m.x_plane, m.y_plane, m.z_plane, m.w_plane];
        if planes
            .iter()
            .flatten()
            .chain(scale.iter())
            .any(|v| !v.is_finite())
        {
            return None;
        }
        if planes[..3].iter().any(|p| p[3].abs() > 1e-5) || (planes[3][3] - 1.0).abs() > 1e-5 {
            return None;
        }
        let mut rows = [[0.0; 3]; 3];
        for (i, row) in rows.iter_mut().enumerate() {
            for (j, cell) in row.iter_mut().enumerate() {
                *cell = f64::from(planes[i][j]) * f64::from(scale[i]);
            }
        }
        let w = planes[3];
        Some(Affine {
            rows,
            translation: [f64::from(w[0]), f64::from(w[1]), f64::from(w[2])],
        })
    }
}

/// The UE3 `Matrix` struct as the reader decodes it (`src/package.cpp:284-290`; the serialized W, X, Y, Z order of
/// each plane is the reader's business): four planes of X, Y, Z, W, row-vector convention, translation in `WPlane`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ue3Matrix {
    pub x_plane: [f32; 4],
    pub y_plane: [f32; 4],
    pub z_plane: [f32; 4],
    pub w_plane: [f32; 4],
}

/// Where an actor or a component is placed, in the game's units: for an actor `Location`, `Rotation`, `DrawScale`
/// and `DrawScale3D`; for a component `Translation`, `Rotation`, `Scale` and `Scale3D` (`prepare_level.py`
/// `transform`). [`Default`] is the property defaults that function assumes: origin, no rotation, scales 1.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    pub location: Vec3,
    pub rotation: Rotator,
    pub scale: f32,
    pub scale_3d: Vec3,
}

impl Default for Placement {
    fn default() -> Self {
        Placement {
            location: [0.0; 3],
            rotation: Rotator::default(),
            scale: 1.0,
            scale_3d: [1.0; 3],
        }
    }
}

impl Placement {
    pub fn new(location: Vec3, rotation: Rotator, scale: f32, scale_3d: Vec3) -> Self {
        Placement {
            location,
            rotation,
            scale,
            scale_3d,
        }
    }

    /// The per-axis scale: `DrawScale3D` x `DrawScale` (`prepare_level.py:202-203`). The product of two `f32` is exact
    /// in `f64`, which is what the Python keeps; where the game rounds it to single is not read.
    pub fn scale_vector(&self) -> [f64; 3] {
        let s = f64::from(self.scale);
        [
            f64::from(self.scale_3d[0]) * s,
            f64::from(self.scale_3d[1]) * s,
            f64::from(self.scale_3d[2]) * s,
        ]
    }

    /// Scale, then rotation, then translation (`crosscheck_blcmm_dumps.py` `transform_matrix`: each row of the rotation
    /// times its axis' scale, the location as translation).
    pub fn affine(&self) -> Affine {
        let axes = rotator_axes(self.rotation);
        let s = self.scale_vector();
        let mut rows = [[0.0; 3]; 3];
        for (i, row) in rows.iter_mut().enumerate() {
            for (j, cell) in row.iter_mut().enumerate() {
                *cell = axes[i][j] * s[i];
            }
        }
        let l = self.location;
        Affine {
            rows,
            translation: [f64::from(l[0]), f64::from(l[1]), f64::from(l[2])],
        }
    }
}

/// A component's world transform: its own placement first, then its actor's (`crosscheck_blcmm_dumps.py`
/// `compose(component, actor)`). Not for a component with `PrePivot` or an absolute flag (module docs).
pub fn component_to_world(component: &Placement, actor: &Placement) -> Affine {
    component.affine().then(&actor.affine())
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
/// view y = its up (Z), view z = its forward (X), each measured from the eye. A cyclic swap, determinant +1.
pub fn view_matrix(eye: Vec3, rotation: Rotator) -> Mat4 {
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
    Mat4 { cols }
}

/// View space to wgpu clip space, reverse-Z: a point at `near` gets depth 1, one at `far` depth 0 (use a `Greater`
/// depth test and clear to 0). `vertical_fov_deg` is the full vertical angle in degrees; which angle the host passes
/// (the game's FOV helpers, `NATIVE_PLAYER_MOVEMENT.md` section 18, `WEAPON_VISUALS.md` section 24) is the camera
/// lane's. `None` unless 0 < fov < 180, aspect > 0 and 0 < near < far, all finite.
pub fn perspective_reverse_z(
    vertical_fov_deg: f32,
    aspect: f32,
    near: f32,
    far: f32,
) -> Option<Mat4> {
    let fov_ok = vertical_fov_deg > 0.0 && vertical_fov_deg < 180.0;
    let ok =
        fov_ok && aspect.is_finite() && aspect > 0.0 && near > 0.0 && far.is_finite() && near < far;
    if !ok {
        return None;
    }
    let half = f64::from(vertical_fov_deg) * (std::f64::consts::PI / 360.0);
    let focal = 1.0 / tan_f64(half);
    let (n, f) = (f64::from(near), f64::from(far));
    // depth = a + b / z: 1 at z = near, 0 at z = far.
    let a = n / (n - f);
    let b = n * f / (f - n);
    Some(Mat4 {
        cols: [
            [to_f32(focal / f64::from(aspect)), 0.0, 0.0, 0.0],
            [0.0, to_f32(focal), 0.0, 0.0],
            [0.0, 0.0, to_f32(a), 1.0],
            [0.0, 0.0, to_f32(b), 0.0],
        ],
    })
}

/// The forward unit vector of a rotator (row X of [`rotator_axes`]): `interp.cpp:231-240` `rotatorToVector` for
/// rotators in -32768..=32767.
pub fn rotator_forward(r: Rotator) -> Vec3 {
    let x = rotator_axes(r)[0];
    [to_f32(x[0]), to_f32(x[1]), to_f32(x[2])]
}

/// A direction to the rotator that faces it, roll 0 (`interp.cpp:242-251` `vectorToRotator`, ported in
/// `ow-vm` `interp/convert.rs`): yaw = atan2(y, x), pitch = atan2(z, |xy|), each truncated toward zero as the C++
/// `int64_t` does, so a value a hair under a whole unit loses it. The zero vector gives the zero rotator.
pub fn direction_to_rotator(d: Vec3) -> Rotator {
    let k = HALF_TURN / std::f64::consts::PI;
    let (x, y, z) = (f64::from(d[0]), f64::from(d[1]), f64::from(d[2]));
    let yaw = trunc_i64_wrap_i32(atan2_f64(y, x) * k);
    let pitch = trunc_i64_wrap_i32(atan2_f64(z, sqrt_f64(x * x + y * y)) * k);
    Rotator::new(pitch, yaw, 0)
}

/// The rotator of an engine matrix's rotation (its first three columns are the X, Y and Z axes, each normalised
/// here, so a positive per-axis scale is fine). Pitch and yaw come from the X axis; roll is the angle of the Z axis
/// toward the roll-free right axis (-sin yaw, cos yaw, 0), which follows from the `GetAxes` rows: there Y and Z are
/// the roll-free axes turned by roll. Each angle is rounded to the nearest unit (no oracle: the C++ has no matrix to
/// rotator), so pitch lands in -16384..=16384 and yaw and roll in -32768..=32767. At pitch +-16384 yaw and roll are
/// one angle and come back as one of the equivalent pairs. `None` when an axis is zero or not finite, or the matrix
/// mirrors (negative determinant), since no rotator describes a mirror.
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
