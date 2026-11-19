//! Owner: lane H3 (moved by lane LINK-2-HEADLESS-MOVER, decision 2026-11-19-66). The game-frame half of `ow-host`'s
//! `frame` module: rotator wrap and axes, the UE3 affine transform, a placement's transform and a component's world
//! transform, all in the game's units (uu, rotator units at 65,536 per turn, Z-up). They moved here, unchanged, so
//! that the scene loader and the mover can run headless in `ow-cli` as well as in the host.
//!
//! **The engine frame is not here.** Architecture rule R9 keeps the one conversion to the engine's frame in
//! `ow-host` (`frame.rs`: the engine matrix, the view and projection matrices, positions to and from the engine, a
//! matrix back to a rotator). That module re-exports this one, so its paths are unchanged, and documents the
//! conventions both halves follow, with their sources.
//!
//! AI-assisted (Claude Code). Decision of the split: `docs/decisions/2026-11/2026-11-19-66-*`; of the rules:
//! `docs/decisions/2026-11/2026-11-16-45-*`.

use ow_core::num::{atan2_f64, cos_f64, sin_f64, sqrt_f64, trunc_i64_wrap_i32};
use ow_core::units::Rotator;

/// A vector in uu (or a unit direction), Z-up, as the game stores it.
pub type Vec3 = [f32; 3];

/// Rotator units in a half turn: `natives_core.cpp:440` and `interp.cpp:232` scale by `pi / 32768`.
pub const HALF_TURN: f64 = 32768.0;

/// Rounds the `f64` working value to the `f32` the engine stores.
pub fn to_f32(x: f64) -> f32 {
    // Float-to-float `as` rounds to nearest even (infinity beyond the f32 range); it never saturates like float-to-int.
    x as f32
}

/// True when `x` rounds to a finite `f32` (finite and no larger than `f32::MAX`; a value between `f32::MAX` and the
/// next power of two would round down to `f32::MAX`, but refusing it too costs nothing real).
pub fn fits_f32(x: f64) -> bool {
    x.is_finite() && x.abs() <= f64::from(f32::MAX)
}

/// Rotator units to radians, after the 16-bit wrap.
fn radians(units: i32) -> f64 {
    f64::from(normalize_axis(units)) * (std::f64::consts::PI / HALF_TURN)
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

    /// True when every entry rounds to a finite `f32`, so `ow-host`'s engine matrix of it is finite.
    pub fn is_finite(&self) -> bool {
        self.rows
            .iter()
            .flatten()
            .chain(&self.translation)
            .all(|&v| fits_f32(v))
    }

    /// The determinant of the 3x3 part. Negative for a mirroring placement (an odd number of negative scale
    /// components), where the renderer flips the front face; zero for a flattened one.
    pub fn determinant(&self) -> f64 {
        let [x, y, z] = self.rows;
        x[0] * (y[1] * z[2] - y[2] * z[1]) - x[1] * (y[0] * z[2] - y[2] * z[0])
            + x[2] * (y[0] * z[1] - y[1] * z[0])
    }

    /// True when the transform mirrors ([`Affine::determinant`] below 0): draw it with the other front face.
    pub fn mirrors(&self) -> bool {
        self.determinant() < 0.0
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
        let affine = Affine {
            rows,
            translation: [f64::from(w[0]), f64::from(w[1]), f64::from(w[2])],
        };
        affine.is_finite().then_some(affine)
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
    /// times its axis' scale, the location as translation). `None` when a location or scale value is NaN or
    /// infinite, or a scaled row would not fit an `f32` (the fields are decoded package floats).
    pub fn affine(&self) -> Option<Affine> {
        let axes = rotator_axes(self.rotation);
        let s = self.scale_vector();
        let mut rows = [[0.0; 3]; 3];
        for (i, row) in rows.iter_mut().enumerate() {
            for (j, cell) in row.iter_mut().enumerate() {
                *cell = axes[i][j] * s[i];
            }
        }
        let l = self.location;
        let affine = Affine {
            rows,
            translation: [f64::from(l[0]), f64::from(l[1]), f64::from(l[2])],
        };
        affine.is_finite().then_some(affine)
    }
}

/// A component's world transform: its own placement first, then its actor's (`crosscheck_blcmm_dumps.py`
/// `compose(component, actor)`). Not for a component with `PrePivot` or an absolute flag (module docs). `None` when
/// either placement is refused by [`Placement::affine`] or the product does not fit an `f32`.
pub fn component_to_world(component: &Placement, actor: &Placement) -> Option<Affine> {
    let world = component.affine()?.then(&actor.affine()?);
    world.is_finite().then_some(world)
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
