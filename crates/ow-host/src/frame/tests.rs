//! Tests for the frame boundary. Every value is invented (AGENTS.md rule 9). Where a test says "Python composer", the
//! expected matrix was printed by the project's own `tools/crosscheck_blcmm_dumps.py` `transform_matrix` / `compose`
//! and `tools/prepare_level.py` `transform` (Python 3.12.10, pc-hummer, 2026-11-16), rounded to `f32` and printed
//! with `%.9g`; that composer is the one that matched the running engine's `_LocalToWorld` on 4209 placements
//! (`docs/verification/BLCMM_DUMP_CROSSCHECK.md`). The C++ binary has no command that prints a placement matrix, so
//! those values are the pin. The other expected values are derived by hand in the comments.

use super::*;
use ow_core::fmt::fmt_g;

fn close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

fn assert_vec(got: [f64; 3], want: [f64; 3], tol: f64, what: &str) {
    for k in 0..3 {
        assert!(
            close(got[k], want[k], tol),
            "{what}[{k}]: got {}, want {}",
            fmt_g(got[k], 17),
            fmt_g(want[k], 17)
        );
    }
}

fn f64v(v: Vec3) -> [f64; 3] {
    [f64::from(v[0]), f64::from(v[1]), f64::from(v[2])]
}

/// Within one `f32` step of the pinned value (`f32::EPSILON` times its size, plus 1e-15 for the rounding residues
/// near 0: P1 carries `2 cos(pi / 2)` as 1.2e-16 where the hand value is 0). The pins are `%.9g` text of `f32`
/// values, read here as `f64`, so they are not bit-exact; and the Python sums its three products with compensated
/// summation (`sum`, 3.12 and later), the Rust left to right.
fn assert_pinned(got: &Affine, rows: [[f64; 3]; 3], translation: [f64; 3], what: &str) {
    let m = got.to_engine();
    for i in 0..3 {
        for (j, &w) in rows[i].iter().enumerate() {
            let g = f64::from(m.cols[i][j]);
            let step = f64::from(f32::EPSILON) * w.abs();
            assert!(
                close(g, w, step + 1e-15),
                "{what} row {i} col {j}: got {}, want {}",
                fmt_g(g, 9),
                fmt_g(w, 9)
            );
        }
        let (g, w) = (f64::from(m.cols[3][i]), translation[i]);
        let step = f64::from(f32::EPSILON) * w.abs();
        assert!(
            close(g, w, step + 1e-15),
            "{what} t {i}: got {}, want {}",
            fmt_g(g, 9),
            fmt_g(w, 9)
        );
    }
}

#[test]
fn normalize_axis_wraps_into_a_signed_16_bit_turn() {
    // natives_core.cpp:52-55: low 16 bits, read as signed.
    for (input, want) in [
        (0, 0),
        (16384, 16384),
        (32767, 32767),
        (32768, -32768),
        (-32768, -32768),
        (-32769, 32767),
        (65536, 0),
        (65536 + 16384, 16384),
        (-65536 - 16384, -16384),
        (49152, -16384),
        (i32::MAX, -1),
        (i32::MIN, 0),
    ] {
        assert_eq!(normalize_axis(input), want, "normalize_axis({input})");
    }
    assert_eq!(
        normalize(Rotator::new(32768, 65537, -98304)),
        Rotator::new(-32768, 1, -32768)
    );
}

#[test]
fn a_whole_turn_apart_gives_the_same_bits() {
    let base = Rotator::new(1234, -5678, 9012);
    let turned = Rotator::new(1234 + 65536, -5678 - 2 * 65536, 9012 + 65536);
    assert_eq!(rotator_axes(base), rotator_axes(turned));
    // +32768 and -32768 are one angle.
    assert_eq!(
        rotator_axes(Rotator::new(0, 32768, 0)),
        rotator_axes(Rotator::new(0, -32768, 0))
    );
    assert_eq!(
        rotator_axes(Rotator::new(32768, 0, 32768)),
        rotator_axes(Rotator::new(-32768, 0, -32768))
    );
}

#[test]
fn the_zero_rotator_is_the_identity() {
    assert_eq!(rotator_axes(Rotator::default()), Affine::IDENTITY.rows);
    assert_eq!(Placement::default().affine(), Affine::IDENTITY);
    assert_eq!(Affine::IDENTITY.to_engine(), Mat4::IDENTITY);
}

#[test]
fn quarter_turns_move_the_axes_as_get_axes_says() {
    let tol = 1e-15;
    // Yaw 16384 = 90 degrees: forward +X turns to +Y (a turn to the right seen from above), right +Y to -X.
    let [x, y, z] = rotator_axes(Rotator::new(0, 16384, 0));
    assert_vec(x, [0.0, 1.0, 0.0], tol, "yaw 16384 forward");
    assert_vec(y, [-1.0, 0.0, 0.0], tol, "yaw 16384 right");
    assert_vec(z, [0.0, 0.0, 1.0], tol, "yaw 16384 up");
    // Yaw -16384 the other way; yaw +-32768 faces -X.
    assert_vec(
        rotator_axes(Rotator::new(0, -16384, 0))[0],
        [0.0, -1.0, 0.0],
        tol,
        "yaw -16384 forward",
    );
    assert_vec(
        rotator_axes(Rotator::new(0, 32768, 0))[0],
        [-1.0, 0.0, 0.0],
        tol,
        "yaw 32768 forward",
    );
    // Pitch 16384: forward points up, up points back (-X).
    let [x, y, z] = rotator_axes(Rotator::new(16384, 0, 0));
    assert_vec(x, [0.0, 0.0, 1.0], tol, "pitch 16384 forward");
    assert_vec(y, [0.0, 1.0, 0.0], tol, "pitch 16384 right");
    assert_vec(z, [-1.0, 0.0, 0.0], tol, "pitch 16384 up");
    // Roll 16384: right points down (-Z), up points right (+Y); from row Y (..., -sr cp) and row Z (..., cy sr, ...).
    let [x, y, z] = rotator_axes(Rotator::new(0, 0, 16384));
    assert_vec(x, [1.0, 0.0, 0.0], tol, "roll 16384 forward");
    assert_vec(y, [0.0, 0.0, -1.0], tol, "roll 16384 right");
    assert_vec(z, [0.0, 1.0, 0.0], tol, "roll 16384 up");
    // 8192 is an eighth of a turn: forward at 45 degrees between +X and +Y.
    let h = std::f64::consts::FRAC_1_SQRT_2;
    assert_vec(
        rotator_axes(Rotator::new(0, 8192, 0))[0],
        [h, h, 0.0],
        1e-15,
        "yaw 8192 forward",
    );
}

#[test]
fn the_axes_are_orthonormal_and_proper() {
    for r in [
        Rotator::new(1000, 2000, 3000),
        Rotator::new(-15000, 31000, -32768),
        Rotator::new(16384, 5, 7),
    ] {
        let [x, y, z] = rotator_axes(r);
        let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        for (a, b) in [(x, y), (y, z), (z, x)] {
            assert!(dot(a, b).abs() < 1e-15);
        }
        for a in [x, y, z] {
            assert!(close(dot(a, a), 1.0, 1e-15));
        }
        // X x Y = Z with the cross product's formula: no mirror in GetAxes.
        let cross = [
            x[1] * y[2] - x[2] * y[1],
            x[2] * y[0] - x[0] * y[2],
            x[0] * y[1] - x[1] * y[0],
        ];
        assert_vec(cross, z, 1e-15, "X x Y");
    }
}

#[test]
fn draw_scale_times_draw_scale_3d() {
    // prepare_level.py:202-203: each DrawScale3D component times DrawScale.
    let p = Placement::new([0.0; 3], Rotator::default(), 1.5, [1.0, 2.0, 0.5]);
    assert_eq!(p.scale_vector(), [1.5, 3.0, 0.75]);
    // The product is kept exact in f64: 0.1f32 * 0.3f32 is not rounded back to single.
    let q = Placement::new([0.0; 3], Rotator::default(), 0.1, [0.3, 1.0, 1.0]);
    assert_eq!(q.scale_vector()[0], f64::from(0.1f32) * f64::from(0.3f32));
}

#[test]
fn scale_applies_before_rotation_and_translation_last() {
    // Local (1, 0, 0), scale (2, 1, 1), yaw 16384, location (10, 20, 30): scaled (2, 0, 0), turned (0, 2, 0),
    // moved (10, 22, 30). Rotation before scale would give (10, 21, 30).
    let p = Placement::new(
        [10.0, 20.0, 30.0],
        Rotator::new(0, 16384, 0),
        1.0,
        [2.0, 1.0, 1.0],
    );
    assert_vec(
        p.affine().point([1.0, 0.0, 0.0]),
        [10.0, 22.0, 30.0],
        1e-12,
        "point",
    );
    assert_vec(
        p.affine().direction([1.0, 0.0, 0.0]),
        [0.0, 2.0, 0.0],
        1e-15,
        "direction",
    );
}

#[test]
fn a_component_is_placed_first_then_its_actor() {
    // Component: translation (10, 0, 0). Actor: location (100, 0, 0), yaw 16384, DrawScale 2. The component origin
    // goes to (10, 0, 0), then scaled (20, 0, 0), turned (0, 20, 0), moved (100, 20, 0). The other order would put it
    // at (110, 0, 0).
    let component = Placement::new([10.0, 0.0, 0.0], Rotator::default(), 1.0, [1.0; 3]);
    let actor = Placement::new([100.0, 0.0, 0.0], Rotator::new(0, 16384, 0), 2.0, [1.0; 3]);
    let world = component_to_world(&component, &actor);
    assert_vec(
        world.point([0.0; 3]),
        [100.0, 20.0, 0.0],
        1e-12,
        "component then actor",
    );
    let wrong = actor.affine().then(&component.affine());
    assert_vec(
        wrong.point([0.0; 3]),
        [110.0, 0.0, 0.0],
        1e-12,
        "actor then component",
    );
    // then() is the composition of point maps: (a then b)(p) = b(a(p)).
    let p = [3.0, -4.0, 5.0];
    assert_vec(
        world.point(p),
        actor.affine().point(component.affine().point(p)),
        1e-12,
        "composition",
    );
}

#[test]
fn placements_match_the_python_composer() {
    // P1: location (100, -200, 50), yaw 16384, DrawScale 2. Hand check: rows (0, 2, 0), (-2, 0, 0), (0, 0, 2).
    let p1 = Placement::new(
        [100.0, -200.0, 50.0],
        Rotator::new(0, 16384, 0),
        2.0,
        [1.0; 3],
    );
    assert_pinned(
        &p1.affine(),
        [
            [1.22464685e-16, 2.0, 0.0],
            [-2.0, 1.22464685e-16, -0.0],
            [-0.0, 0.0, 2.0],
        ],
        [100.0, -200.0, 50.0],
        "P1",
    );
    // P2: location (1234.5, 678.25, -90), rotator (4096, -8192, 2048), DrawScale 1.5, DrawScale3D (1, 2, 0.5).
    let p2 = Placement::new(
        [1234.5, 678.25, -90.0],
        Rotator::new(4096, -8192, 2048),
        1.5,
        [1.0, 2.0, 0.5],
    );
    assert_pinned(
        &p2.affine(),
        [
            [0.979922235, -0.979922235, 0.574025154],
            [2.23893285, 1.92218661, -0.540719867],
            [-0.0955866724, 0.302511215, 0.67959559],
        ],
        [1234.5, 678.25, -90.0],
        "P2",
    );
    // P3: component (10, 0, 5), roll 16384, Scale3D (2, 1, 1) inside actor (-500, 250, 1000), rotator
    // (-3000, 30000, 0), DrawScale 0.75.
    let component = Placement::new(
        [10.0, 0.0, 5.0],
        Rotator::new(0, 0, 16384),
        1.0,
        [2.0, 1.0, 1.0],
    );
    let actor = Placement::new(
        [-500.0, 250.0, 1000.0],
        Rotator::new(-3000, 30000, 0),
        0.75,
        [1.0; 3],
    );
    assert_pinned(
        &component_to_world(&component, &actor),
        [
            [-1.38802898, 0.377251208, -0.425508201],
            [0.205306277, -0.0558000207, -0.719191015],
            [-0.196706027, -0.723744929, 4.40377492e-17],
        ],
        [-507.966675, 252.165253, 1001.46838],
        "P3",
    );
    // P4: rotator (65536 + 1000, -70000, 32768) unwrapped as stored, DrawScale3D (1, -1, 1) mirrored. The Python takes
    // the angles unwrapped (degrees), this module wraps first; the pins agree within one f32 step.
    let p4 = Placement::new(
        [9000.0, 9000.0, 3000.0],
        Rotator::new(65536 + 1000, -70000, 32768),
        1.0,
        [1.0, -1.0, 1.0],
    );
    assert_pinned(
        &p4.affine(),
        [
            [0.905627549, -0.413128436, 0.0957269892],
            [0.415034413, 0.909805715, 1.21902282e-16],
            [0.0870929658, -0.0397299975, -0.995407641],
        ],
        [9000.0, 9000.0, 3000.0],
        "P4",
    );
}

#[test]
fn a_collection_matrix_scales_its_rows() {
    // Hand-derived (crosscheck_blcmm_dumps.py actor_matrix): a yaw-16384 matrix at (5, 6, 7) with scale (0.97, 1, 1)
    // keeps rows Y and Z and scales row X to (0, 0.97, 0).
    let m = Ue3Matrix {
        x_plane: [0.0, 1.0, 0.0, 0.0],
        y_plane: [-1.0, 0.0, 0.0, 0.0],
        z_plane: [0.0, 0.0, 1.0, 0.0],
        w_plane: [5.0, 6.0, 7.0, 1.0],
    };
    let a = Affine::from_ue3_matrix(&m, [0.97, 1.0, 1.0]).expect("affine");
    assert_eq!(
        a.rows,
        [
            [0.0, f64::from(0.97f32), 0.0],
            [-1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0]
        ]
    );
    assert_eq!(a.translation, [5.0, 6.0, 7.0]);
    assert_vec(
        a.point([1.0, 0.0, 0.0]),
        [5.0, 6.0 + f64::from(0.97f32), 7.0],
        1e-12,
        "point",
    );
    // The same placement given as location, rotation and scale composes to the same matrix.
    let p = Placement::new(
        [5.0, 6.0, 7.0],
        Rotator::new(0, 16384, 0),
        1.0,
        [0.97, 1.0, 1.0],
    );
    assert_eq!(p.affine().to_engine().cols[3], a.to_engine().cols[3]);
    for i in 0..3 {
        for j in 0..3 {
            assert!(close(p.affine().rows[i][j], a.rows[i][j], 1e-15));
        }
    }
}

#[test]
fn a_bad_collection_matrix_is_refused() {
    let good = Ue3Matrix {
        x_plane: [1.0, 0.0, 0.0, 0.0],
        y_plane: [0.0, 1.0, 0.0, 0.0],
        z_plane: [0.0, 0.0, 1.0, 0.0],
        w_plane: [0.0, 0.0, 0.0, 1.0],
    };
    assert!(Affine::from_ue3_matrix(&good, [1.0; 3]).is_some());
    let mut projective = good;
    projective.x_plane[3] = 0.5;
    assert!(Affine::from_ue3_matrix(&projective, [1.0; 3]).is_none());
    let mut w_not_one = good;
    w_not_one.w_plane[3] = 2.0;
    assert!(Affine::from_ue3_matrix(&w_not_one, [1.0; 3]).is_none());
    let mut nan = good;
    nan.y_plane[1] = f32::NAN;
    assert!(Affine::from_ue3_matrix(&nan, [1.0; 3]).is_none());
    assert!(Affine::from_ue3_matrix(&good, [1.0, f32::INFINITY, 1.0]).is_none());
}

#[test]
fn the_engine_matrix_is_the_ue3_matrix_column_major() {
    let p = Placement::new(
        [1.0, 2.0, 3.0],
        Rotator::new(100, 200, 300),
        1.25,
        [1.0, 2.0, 3.0],
    );
    let a = p.affine();
    let m = a.to_engine();
    for i in 0..3 {
        for j in 0..3 {
            assert_eq!(m.cols[i][j], a.rows[i][j] as f32);
        }
        assert_eq!(m.cols[i][3], 0.0);
    }
    assert_eq!(m.cols[3], [1.0, 2.0, 3.0, 1.0]);
    // m * (p, 1) is the UE3 p * M + t.
    let local = [0.5f32, -1.5, 2.0];
    let got = m.apply([local[0], local[1], local[2], 1.0]);
    let want = a.point(f64v(local));
    assert_vec(
        [f64::from(got[0]), f64::from(got[1]), f64::from(got[2])],
        want,
        1e-5,
        "apply",
    );
    assert_eq!(got[3], 1.0);
}

#[test]
fn mat4_mul_applies_the_right_hand_side_first() {
    let a = Placement::new([0.0, 0.0, 10.0], Rotator::default(), 1.0, [1.0; 3]).affine();
    let b = Placement::new([0.0; 3], Rotator::new(0, 16384, 0), 1.0, [1.0; 3]).affine();
    // b * a: move up 10, then turn; a point at (1, 0, 0) goes to (1, 0, 10), then (0, 1, 10).
    let ba = b.to_engine().mul(&a.to_engine());
    let got = ba.apply([1.0, 0.0, 0.0, 1.0]);
    assert_vec(
        [f64::from(got[0]), f64::from(got[1]), f64::from(got[2])],
        [0.0, 1.0, 10.0],
        1e-6,
        "b * a",
    );
    // The same as Affine::then.
    let t = a.then(&b).to_engine();
    for c in 0..4 {
        for r in 0..4 {
            assert!(close(
                f64::from(ba.cols[c][r]),
                f64::from(t.cols[c][r]),
                1e-6
            ));
        }
    }
}

#[test]
fn positions_cross_unchanged() {
    for p in [
        [0.0, 0.0, 0.0],
        [9595.84, 9384.23, 3404.0],
        [-1.0e6, 0.5, f32::MIN_POSITIVE],
    ] {
        assert_eq!(position_to_engine(p), p);
        assert_eq!(position_from_engine(position_to_engine(p)), p);
    }
    assert_eq!(ENGINE_UNITS_PER_UU, 1.0);
}

#[test]
fn the_view_puts_right_up_forward_on_x_y_z() {
    let v = view_matrix([0.0; 3], Rotator::default());
    let at = |p: [f32; 3]| v.apply([p[0], p[1], p[2], 1.0]);
    assert_eq!(at([100.0, 0.0, 0.0]), [0.0, 0.0, 100.0, 1.0]);
    assert_eq!(at([0.0, 10.0, 0.0]), [10.0, 0.0, 0.0, 1.0]);
    assert_eq!(at([0.0, 0.0, 10.0]), [0.0, 10.0, 0.0, 1.0]);
    // The basis is the cyclic swap (Y, Z, X): determinant +1, no mirror.
    let c = v.cols;
    let det = c[0][0] * (c[1][1] * c[2][2] - c[2][1] * c[1][2])
        - c[1][0] * (c[0][1] * c[2][2] - c[2][1] * c[0][2])
        + c[2][0] * (c[0][1] * c[1][2] - c[1][1] * c[0][2]);
    assert_eq!(det, 1.0);
}

#[test]
fn the_view_follows_the_camera() {
    // Eye at (100, 200, 300), yaw 16384 (facing +Y): a point 50 ahead is at view z 50; 20 to the camera's right (-X
    // in the world) is at view x 20; 5 above at view y 5.
    let eye = [100.0, 200.0, 300.0];
    let v = view_matrix(eye, Rotator::new(0, 16384, 0));
    let at = |p: [f32; 3]| {
        let o = v.apply([p[0], p[1], p[2], 1.0]);
        [f64::from(o[0]), f64::from(o[1]), f64::from(o[2])]
    };
    assert_vec(at([100.0, 250.0, 300.0]), [0.0, 0.0, 50.0], 1e-5, "ahead");
    assert_vec(at([80.0, 200.0, 300.0]), [20.0, 0.0, 0.0], 1e-5, "right");
    assert_vec(at([100.0, 200.0, 305.0]), [0.0, 5.0, 0.0], 1e-5, "up");
    assert_vec(at(eye), [0.0; 3], 1e-5, "eye");
}

#[test]
fn reverse_z_depth_is_one_at_near_and_zero_at_far() {
    let p = perspective_reverse_z(90.0, 16.0 / 9.0, 10.0, 100_000.0).expect("valid");
    let ndc = |v: [f32; 3]| {
        let c = p.apply([v[0], v[1], v[2], 1.0]);
        [c[0] / c[3], c[1] / c[3], c[2] / c[3]]
    };
    let near = ndc([0.0, 0.0, 10.0]);
    assert!(
        close(f64::from(near[2]), 1.0, 1e-6),
        "near depth {}",
        fmt_g(f64::from(near[2]), 9)
    );
    let far = ndc([0.0, 0.0, 100_000.0]);
    assert!(
        close(f64::from(far[2]), 0.0, 1e-6),
        "far depth {}",
        fmt_g(f64::from(far[2]), 9)
    );
    // Depth falls with distance.
    assert!(ndc([0.0, 0.0, 100.0])[2] > ndc([0.0, 0.0, 1000.0])[2]);
    // Vertical fov 90: a point as high as it is far is on the top edge (y 1); x is divided by the aspect.
    let edge = ndc([0.0, 500.0, 500.0]);
    assert!(close(f64::from(edge[1]), 1.0, 1e-6));
    let side = ndc([500.0, 0.0, 500.0]);
    assert!(close(f64::from(side[0]), 9.0 / 16.0, 1e-6));
    // Up is +y and right is +x in NDC, as wgpu has them.
    assert!(ndc([10.0, 20.0, 500.0])[0] > 0.0 && ndc([10.0, 20.0, 500.0])[1] > 0.0);
}

#[test]
fn a_bad_projection_is_refused() {
    assert!(perspective_reverse_z(0.0, 1.0, 1.0, 2.0).is_none());
    assert!(perspective_reverse_z(180.0, 1.0, 1.0, 2.0).is_none());
    assert!(perspective_reverse_z(f32::NAN, 1.0, 1.0, 2.0).is_none());
    assert!(perspective_reverse_z(90.0, 0.0, 1.0, 2.0).is_none());
    assert!(perspective_reverse_z(90.0, f32::INFINITY, 1.0, 2.0).is_none());
    assert!(perspective_reverse_z(90.0, 1.0, 0.0, 2.0).is_none());
    assert!(perspective_reverse_z(90.0, 1.0, 2.0, 2.0).is_none());
    assert!(perspective_reverse_z(90.0, 1.0, 1.0, f32::INFINITY).is_none());
}

#[test]
fn direction_to_rotator_truncates_as_the_cxx() {
    assert_eq!(direction_to_rotator([1.0, 0.0, 0.0]), Rotator::new(0, 0, 0));
    assert_eq!(
        direction_to_rotator([-1.0, 0.0, 0.0]),
        Rotator::new(0, 32768, 0)
    );
    assert_eq!(direction_to_rotator([0.0, 0.0, 0.0]), Rotator::new(0, 0, 0));
    // atan2(1, 0) * 32768 / pi and atan2(1, 1) * 32768 / pi come out whole on this platform (pinned, so a change in
    // the choke points shows), so truncation keeps them.
    let k = 32768.0 / std::f64::consts::PI;
    let quarter = atan2_f64(1.0, 0.0) * k;
    let eighth = atan2_f64(1.0, 1.0) * k;
    assert_eq!(fmt_g(quarter, 17), "16384");
    assert_eq!(fmt_g(eighth, 17), "8192");
    assert_eq!(direction_to_rotator([0.0, 1.0, 0.0]).yaw, 16384);
    assert_eq!(direction_to_rotator([0.0, -1.0, 0.0]).yaw, -16384);
    assert_eq!(direction_to_rotator([1.0, 1.0, 0.0]).yaw, 8192);
    assert_eq!(
        direction_to_rotator([0.0, 1.0, 1.0]),
        Rotator::new(8192, 16384, 0)
    );
    // Truncation, not rounding: 0.9999 of a unit is 0, and -0.9999 is 0 too.
    let tiny = (0.9999 / k) as f32;
    assert_eq!(direction_to_rotator([1.0, tiny, 0.0]).yaw, 0);
    assert_eq!(direction_to_rotator([1.0, -tiny, 0.0]).yaw, 0);
    assert_eq!(direction_to_rotator([1.0, 3.0 * tiny, 0.0]).yaw, 2);
    // A direction and its forward go round: yaw and pitch back to within one unit (the truncation).
    for r in [
        Rotator::new(1000, 2000, 0),
        Rotator::new(-4000, -30000, 0),
        Rotator::new(12000, 32000, 0),
    ] {
        let back = direction_to_rotator(rotator_forward(r));
        assert!(
            (back.pitch - r.pitch).abs() <= 1 && (back.yaw - r.yaw).abs() <= 1,
            "{r:?} -> {back:?}"
        );
    }
}

#[test]
fn rotators_round_trip_through_the_engine_matrix() {
    // Pitch strictly inside the poles; yaw and roll over the whole turn, -32768 included.
    let mut checked = 0;
    for pitch in (-16383..=16383).step_by(1489) {
        for yaw in (-32768..=32767).step_by(2731) {
            for roll in (-32768..=32767).step_by(3277) {
                let r = Rotator::new(pitch, yaw, roll);
                let m = Placement::new([1.0, 2.0, 3.0], r, 2.5, [1.0, 3.0, 0.5])
                    .affine()
                    .to_engine();
                assert_eq!(rotator_from_engine(&m), Some(r), "rotator {r:?}");
                checked += 1;
            }
        }
    }
    assert_eq!(checked, 23 * 24 * 20);
    // Out-of-range axes come back wrapped.
    let m = Placement::new(
        [0.0; 3],
        Rotator::new(65536 + 100, 32768, -98304),
        1.0,
        [1.0; 3],
    )
    .affine()
    .to_engine();
    assert_eq!(
        rotator_from_engine(&m),
        Some(Rotator::new(100, -32768, -32768))
    );
    // A pitch past the pole is the same rotation as yaw and roll turned by half a turn.
    let over = Placement::new([0.0; 3], Rotator::new(20000, 1000, 2000), 1.0, [1.0; 3]).affine();
    let back = rotator_from_engine(&over.to_engine()).expect("rotation");
    assert_eq!(
        back,
        Rotator::new(32768 - 20000, 1000 - 32768, 2000 - 32768)
    );
}

#[test]
fn a_mirror_or_a_degenerate_matrix_has_no_rotator() {
    let mirrored = Placement::new([0.0; 3], Rotator::new(0, 100, 0), 1.0, [1.0, -1.0, 1.0])
        .affine()
        .to_engine();
    assert_eq!(rotator_from_engine(&mirrored), None);
    let flat = Placement::new([0.0; 3], Rotator::default(), 1.0, [1.0, 1.0, 0.0])
        .affine()
        .to_engine();
    assert_eq!(rotator_from_engine(&flat), None);
    let mut nan = Mat4::IDENTITY;
    nan.cols[1][1] = f32::NAN;
    assert_eq!(rotator_from_engine(&nan), None);
    assert_eq!(
        rotator_from_engine(&Mat4::IDENTITY),
        Some(Rotator::default())
    );
}
