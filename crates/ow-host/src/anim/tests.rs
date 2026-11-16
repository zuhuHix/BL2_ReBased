//! Owner: lane H6-ANIM. Synthetic tests on invented values (AGENTS.md rule 9), and one ignored test that writes
//! the installed-game report from UModel exports under `local/`.

use ow_assets::animation::BoundClip;
use ow_assets::skeletal::{Bone, BoneTransform, Influence, SkeletalMesh, SkinSection, SkinVertex};

use super::math::{affine, max_difference, nlerp, rigid_inverse, rotate};
use super::pose::{AnimError, Skeleton, Wrap, keys_at};
use super::skin::{skin, skin_matrices};
use crate::frame::Affine;

const S: f32 = std::f32::consts::FRAC_1_SQRT_2;
const ID: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
/// 90 degrees about +Z: X forward turns to Y.
const Z90: [f32; 4] = [0.0, 0.0, S, S];
/// -90 degrees about +Z.
const ZM90: [f32; 4] = [0.0, 0.0, -S, S];

fn t(rotation: [f32; 4], translation: [f32; 3]) -> BoneTransform {
    BoneTransform::new(rotation, translation)
}

/// A 3-bone chain along X: Root at 0, B1 at 10, B2 at 20 (bind, all unrotated). Four vertices: on Root, half Root
/// and half B1 at x = 15, on B1 at x = 15, on B2 at x = 25.
fn chain() -> SkeletalMesh {
    let bones = vec![
        Bone::new("Root", None, t(ID, [0.0; 3])),
        Bone::new("B1", Some(0), t(ID, [10.0, 0.0, 0.0])),
        Bone::new("B2", Some(1), t(ID, [10.0, 0.0, 0.0])),
    ];
    let v = |p: [f32; 3], inf: Vec<Influence>| SkinVertex::new(p, [0.0, 0.0], 0, inf);
    let vertices = vec![
        v([1.0, 2.0, 3.0], vec![Influence::new(0, 1.0)]),
        v(
            [15.0, 0.0, 0.0],
            vec![Influence::new(0, 0.5), Influence::new(1, 0.5)],
        ),
        v([15.0, 0.0, 0.0], vec![Influence::new(1, 1.0)]),
        v(
            [25.0, 1.0, 0.0],
            vec![Influence::new(2, 0.75), Influence::new(1, 0.25)],
        ),
    ];
    SkeletalMesh::new(
        bones,
        vertices,
        vec![0, 1, 2, 1, 2, 3],
        vec![SkinSection::new(0, 0, 2)],
        vec!["M".into()],
    )
}

/// Three frames at 10 fps; B1 turns 0, 90, -90 degrees about Z; Root's track stays at the bind; B2 has no track.
fn turn_clip() -> BoundClip {
    let root = vec![t(ID, [0.0; 3]); 3];
    let b1 = vec![
        t(ID, [10.0, 0.0, 0.0]),
        t(Z90, [10.0, 0.0, 0.0]),
        t(ZM90, [10.0, 0.0, 0.0]),
    ];
    BoundClip::new("Turn", 10.0, 3, vec![Some(root), Some(b1), None])
}

fn close(a: [f64; 3], b: [f64; 3], eps: f64) -> bool {
    (0..3).all(|k| (a[k] - b[k]).abs() < eps)
}

fn close32(a: [f32; 3], b: [f32; 3], eps: f32) -> bool {
    (0..3).all(|k| (a[k] - b[k]).abs() < eps)
}

#[test]
fn weights_of_the_chain_sum_to_one() {
    for v in &chain().vertices {
        let sum: f32 = v.influences.iter().map(|i| i.weight).sum();
        assert!((sum - 1.0).abs() < 1e-6);
    }
}

#[test]
fn bind_matrices_compose_along_the_chain() {
    let sk = Skeleton::new(&chain()).unwrap();
    assert_eq!(sk.bind_model[2].translation, [20.0, 0.0, 0.0]);
    for (m, inv) in sk.bind_model.iter().zip(&sk.inverse_bind) {
        assert!(max_difference(&m.then(inv), &Affine::IDENTITY) < 1e-12);
    }
}

#[test]
fn rotation_maths_is_hamilton_and_rigid_inverse_inverts() {
    let q = Z90.map(f64::from);
    assert!(close(rotate(q, [1.0, 0.0, 0.0]), [0.0, 1.0, 0.0], 1e-6));
    let a = affine(q, [1.0, 2.0, 3.0]).unwrap();
    // Row 0 is where local X lands: on Y.
    assert!(close(a.rows[0], [0.0, 1.0, 0.0], 1e-6));
    assert!(max_difference(&a.then(&rigid_inverse(&a)), &Affine::IDENTITY) < 1e-6);
    // nlerp half way from identity to 90 degrees is 45 degrees, and takes the short arc for a negated end.
    let h = nlerp([0.0, 0.0, 0.0, 1.0], q, 0.5).unwrap();
    let c = std::f64::consts::FRAC_1_SQRT_2;
    assert!(close(rotate(h, [1.0, 0.0, 0.0]), [c, c, 0.0], 1e-6));
    let h2 = nlerp([0.0, 0.0, 0.0, 1.0], q.map(|x| -x), 0.5).unwrap();
    assert!(close(rotate(h2, [1.0, 0.0, 0.0]), [c, c, 0.0], 1e-6));
    assert!(affine([0.0; 4], [0.0; 3]).is_none());
}

#[test]
fn samples_the_start_middle_and_end() {
    let mesh = chain();
    let sk = Skeleton::new(&mesh).unwrap();
    let clip = turn_clip();
    // Start: the bind pose.
    let p = sk.sample(&clip, 0.0, Wrap::Clamp).unwrap();
    assert!(close(p.bone_position(2).unwrap(), [20.0, 0.0, 0.0], 1e-9));
    // Middle of key 0 and key 1 (t = 0.05 s, frame 0.5): B1 at 45 degrees, B2 (bind local) follows it.
    let p = sk.sample(&clip, 0.05, Wrap::Clamp).unwrap();
    let c = 10.0 * std::f64::consts::FRAC_1_SQRT_2;
    assert!(close(p.bone_position(2).unwrap(), [10.0 + c, c, 0.0], 1e-6));
    // On key 1: 90 degrees.
    let p = sk.sample(&clip, 0.1, Wrap::Clamp).unwrap();
    assert!(close(p.bone_position(2).unwrap(), [10.0, 10.0, 0.0], 1e-6));
    // End and past it, clamped: the last key (-90 degrees).
    for time in [0.2, 0.25, 9.0] {
        let p = sk.sample(&clip, time, Wrap::Clamp).unwrap();
        assert!(
            close(p.bone_position(2).unwrap(), [10.0, -10.0, 0.0], 1e-6),
            "{time}"
        );
    }
    // Before the start, clamped: key 0.
    let p = sk.sample(&clip, -1.0, Wrap::Clamp).unwrap();
    assert!(close(p.bone_position(2).unwrap(), [20.0, 0.0, 0.0], 1e-9));
}

#[test]
fn the_loop_wraps_from_the_last_key_to_the_first() {
    let mesh = chain();
    let sk = Skeleton::new(&mesh).unwrap();
    let clip = turn_clip();
    // Three keys at 10 fps loop over 0.3 s; at 0.25 s (frame 2.5) the pose is half way from the last key (-90
    // degrees) back to key 0: -45 degrees, so B2 sits at (10 + 10 cos 45, -10 sin 45, 0).
    let p = sk.sample(&clip, 0.25, Wrap::Loop).unwrap();
    let c = 10.0 * std::f64::consts::FRAC_1_SQRT_2;
    assert!(close(
        p.bone_position(2).unwrap(),
        [10.0 + c, -c, 0.0],
        1e-6
    ));
    // One period on, and one period back, are the same pose as the start.
    for time in [0.3, 0.6, -0.3] {
        let p = sk.sample(&clip, time, Wrap::Loop).unwrap();
        assert!(
            close(p.bone_position(2).unwrap(), [20.0, 0.0, 0.0], 1e-6),
            "{time}"
        );
    }
    assert_eq!(keys_at(2.5, 3, Wrap::Loop), Some((2, 0, 0.5)));
    assert_eq!(keys_at(-0.5, 3, Wrap::Loop), Some((2, 0, 0.5)));
    assert_eq!(keys_at(2.5, 3, Wrap::Clamp), Some((2, 2, 0.0)));
    assert_eq!(keys_at(f64::NAN, 3, Wrap::Loop), None);
}

#[test]
fn skinning_at_the_bind_pose_returns_the_mesh() {
    let mesh = chain();
    let sk = Skeleton::new(&mesh).unwrap();
    let out = skin(&mesh, &skin_matrices(&sk, &sk.bind_pose())).unwrap();
    for (v, s) in mesh.vertices.iter().zip(&out) {
        assert!(close32(v.position, *s, 1e-5));
    }
}

#[test]
fn skinning_result_checked_by_hand() {
    // At key 1, B1 has turned 90 degrees about Z at x = 10.
    // - vertex 0 (Root only): unchanged, (1, 2, 3).
    // - vertex 2 (B1 only, at x 15): 5 along B1's X, which now points along Y: (10, 5, 0).
    // - vertex 1 (half Root, half B1, at x 15): half of (15, 0, 0) and half of (10, 5, 0) = (12.5, 2.5, 0).
    // - vertex 3 (0.75 B2, 0.25 B1, at (25, 1, 0)): B2 now sits at (10, 10, 0), turned 90 degrees. B2 alone:
    //   (5, 1, 0) from B2 turns to (-1, 5, 0), plus (10, 10, 0): (9, 15, 0). B1 alone: (15, 1, 0) from B1 turns
    //   to (-1, 15, 0), plus (10, 0, 0): (9, 15, 0). Both agree (the chain moves rigidly), so (9, 15, 0).
    let mesh = chain();
    let sk = Skeleton::new(&mesh).unwrap();
    let pose = sk.sample(&turn_clip(), 0.1, Wrap::Clamp).unwrap();
    let out = skin(&mesh, &skin_matrices(&sk, &pose)).unwrap();
    assert!(close32(out[0], [1.0, 2.0, 3.0], 1e-5));
    assert!(close32(out[1], [12.5, 2.5, 0.0], 1e-5));
    assert!(close32(out[2], [10.0, 5.0, 0.0], 1e-5));
    assert!(close32(out[3], [9.0, 15.0, 0.0], 1e-5));
}

#[test]
fn refuses_bad_clips_times_and_skeletons() {
    let mesh = chain();
    let sk = Skeleton::new(&mesh).unwrap();
    let mut short = turn_clip();
    short.tracks.pop();
    assert_eq!(
        sk.sample(&short, 0.0, Wrap::Loop).unwrap_err(),
        AnimError::ClipBones {
            skeleton: 3,
            clip: 2
        }
    );
    assert_eq!(
        sk.sample(&turn_clip(), f64::INFINITY, Wrap::Loop)
            .unwrap_err(),
        AnimError::BadTime
    );
    let mut no_rate = turn_clip();
    no_rate.frame_rate = 0.0;
    assert_eq!(
        sk.sample(&no_rate, 0.0, Wrap::Loop).unwrap_err(),
        AnimError::BadClip
    );
    let mut truncated = turn_clip();
    if let Some(Some(keys)) = truncated.tracks.get_mut(1) {
        keys.pop();
    }
    assert_eq!(
        sk.sample(&truncated, 0.2, Wrap::Clamp).unwrap_err(),
        AnimError::BadClip
    );
    let mut zero = turn_clip();
    if let Some(Some(keys)) = zero.tracks.get_mut(1) {
        keys[0].rotation = [0.0; 4];
    }
    assert_eq!(
        sk.sample(&zero, 0.0, Wrap::Clamp).unwrap_err(),
        AnimError::Degenerate { bone: 1 }
    );

    let mut cyclic = chain();
    cyclic.bones[1].parent = Some(2);
    assert_eq!(
        Skeleton::new(&cyclic).unwrap_err(),
        AnimError::BoneOrder { bone: 1 }
    );
    let mut two_roots = chain();
    two_roots.bones[2].parent = None;
    assert_eq!(
        Skeleton::new(&two_roots).unwrap_err(),
        AnimError::BoneOrder { bone: 2 }
    );
    let mut stray = chain();
    stray.vertices[0].influences[0] = Influence::new(9, 1.0);
    assert_eq!(
        skin(&stray, &skin_matrices(&sk, &sk.bind_pose())).unwrap_err(),
        AnimError::VertexBone { vertex: 0 }
    );
}

#[test]
fn engine_matrices_are_the_frame_module_s() {
    let sk = Skeleton::new(&chain()).unwrap();
    let pose = sk.bind_pose();
    let m = pose.engine_matrices();
    assert_eq!(m.len(), 3);
    assert_eq!(m[2], sk.bind_model[2].to_engine());
}

/// The installed-game report: `OW_H6_ANIM_EXPORTS` names UModel's export folder for `GD_Siren_Streaming_SF`
/// (under `local/`), `OW_H6_ANIM_OUT` the JSON file to write (under `local/`). Run with
/// `cargo test -p ow-host --lib anim::tests::installed_report -- --ignored`. It fails if the report does, including
/// a failed handedness check.
#[test]
#[ignore = "needs UModel exports of the installed game under local/"]
fn installed_report() {
    let root = std::env::var_os("OW_H6_ANIM_EXPORTS").expect("set OW_H6_ANIM_EXPORTS");
    let out = std::env::var_os("OW_H6_ANIM_OUT").expect("set OW_H6_ANIM_OUT");
    let text = super::report::run(std::path::Path::new(&root), &super::report::MAYA).unwrap();
    std::fs::write(out, text).unwrap();
}

#[test]
fn the_handedness_check_tells_right_from_left() {
    // Camera at the origin looking along +X; R_Hand on +Y (the game's right), L_Hand on -Y.
    let arms = |left_y: f32, right_y: f32| {
        let bone = |name: &str, p: [f32; 3]| {
            Bone::new(
                name,
                if name == "Camera" { None } else { Some(0) },
                t(ID, p),
            )
        };
        let bones = vec![
            bone("Camera", [0.0; 3]),
            bone("L_Hand", [20.0, left_y, -10.0]),
            bone("R_Hand", [20.0, right_y, -10.0]),
        ];
        SkeletalMesh::new(bones, Vec::new(), Vec::new(), Vec::new(), Vec::new())
    };
    let check = |mesh: &SkeletalMesh| {
        let pose = Skeleton::new(mesh).unwrap().bind_pose();
        super::report::handedness(mesh, &pose, super::report::Handedness::Arms)
            .unwrap()
            .1
    };
    assert!(check(&arms(-8.0, 8.0)));
    // The same arms mirrored in Y: refused.
    assert!(!check(&arms(8.0, -8.0)));
}
