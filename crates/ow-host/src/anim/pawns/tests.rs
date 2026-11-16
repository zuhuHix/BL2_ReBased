//! Owner: lane H-PAWNS. Synthetic tests (invented values, AGENTS.md rule 9); a drawn test of the depth test inside
//! the scene pass (needs a GPU adapter, `#[ignore]`d as `crate::render::tests` are); and the installed-game check of
//! the bullymong and Maya's body (needs the install and UModel exports under `local/`).

use std::fmt::Write as _;
use std::path::PathBuf;

use ow_assets::skeletal::{Bone, BoneTransform, Influence, SkeletalMesh, SkinSection, SkinVertex};
use ow_core::units::Rotator;
use ow_core::{OrderedMap, ParityMode};

use super::*;
use crate::anim::gpu::tests::{GPU_TOLERANCE, headless};
use crate::anim::skin::skin;
use crate::anim::tree::{Child, Node};
use crate::draw::{Batch, BatchKey, DrawPlan, Instance, MeshData, SectionRange, Vertex};
use crate::render::SceneRenderer;

#[test]
fn kinds_are_found_by_name() {
    assert_eq!(
        kind("Bullymong").map(|k| k.archetype),
        Some("GD_PrimalBeastAdult.Character.Pawn_PrimalBeastAdult")
    );
    assert_eq!(
        kind("maya").map(|k| k.archetype),
        Some("GD_Siren_Streaming.Pawn_Siren")
    );
    assert!(kind("skag").is_none());
}

#[test]
fn the_pawn_yaw_is_rounded_to_rotator_units() {
    let p = actor_placement([1.0, 2.0, 3.0], 90.0).unwrap();
    assert_eq!(
        (p.location, p.rotation),
        ([1.0, 2.0, 3.0], Rotator::new(0, 16384, 0))
    );
    assert_eq!(
        actor_placement([0.0; 3], 225.0).unwrap().rotation.yaw,
        40960
    );
    assert_eq!(
        actor_placement([0.0; 3], -90.0).unwrap().rotation.yaw,
        49152
    );
    assert_eq!(actor_placement([0.0; 3], 0.01).unwrap().rotation.yaw, 2);
    assert!(actor_placement([f32::NAN, 0.0, 0.0], 0.0).is_none());
    assert!(actor_placement([0.0; 3], f64::INFINITY).is_none());
}

#[test]
fn the_census_counts_not_yet_classes() {
    let node = |path: &str, class: &str, kind: Kind, children: &[(&str, Option<i32>)]| {
        Node::new(
            path,
            class,
            kind,
            children.iter().map(|(n, c)| Child::new(*n, *c)).collect(),
        )
    };
    let nodes: OrderedMap<i32, Node> = [
        (
            1,
            node("T", "Engine.AnimTree", Kind::Root, &[("Child", Some(2))]),
        ),
        (
            2,
            node(
                "T.A",
                "Test.Unread",
                Kind::NotYet { reason: "x" },
                &[("In", Some(3))],
            ),
        ),
        (
            3,
            node(
                "T.B",
                "Test.Unread",
                Kind::NotYet { reason: "x" },
                &[("In", Some(4))],
            ),
        ),
        (
            4,
            node("T.C", "Test.Other", Kind::NotYet { reason: "y" }, &[]),
        ),
    ]
    .into_iter()
    .collect();
    let c = census(&TreeDef::new("T", 1, nodes));
    assert_eq!((c.nodes, c.evaluated), (4, 1));
    assert_eq!(c.not_yet.get("Test.Unread"), Some(&2));
    assert_eq!(c.not_yet.get("Test.Other"), Some(&1));
}

/// A 40 x 40 uu quad in its local YZ plane on one bone, facing -X (towards a camera at the origin looking +X).
fn quad() -> SkeletalMesh {
    let bones = vec![Bone::new(
        "Root",
        None,
        BoneTransform::new([0.0, 0.0, 0.0, 1.0], [0.0; 3]),
    )];
    let v = |p: [f32; 3], i: usize| SkinVertex::new(p, [0.0, 0.0], i, vec![Influence::new(0, 1.0)]);
    SkeletalMesh::new(
        bones,
        vec![
            v([0.0, -20.0, -20.0], 0),
            v([0.0, 20.0, -20.0], 1),
            v([0.0, 20.0, 20.0], 2),
            v([0.0, -20.0, 20.0], 3),
        ],
        vec![0, 1, 2, 0, 2, 3],
        vec![SkinSection::new(0, 0, 2)],
        vec!["M".into()],
    )
}

/// A scene of one grey wall at x = 200, 200 x 200 uu around the view axis, as `crate::render::tests` places its
/// probe.
fn wall() -> DrawPlan {
    let normal = [-1.0, 0.0, 0.0];
    let vertices = [
        [0.0, -100.0, -100.0],
        [0.0, 100.0, -100.0],
        [0.0, 100.0, 100.0],
        [0.0, -100.0, 100.0],
    ]
    .map(|position| Vertex {
        position,
        normal,
        uv: [0.0, 0.0],
        lightmap_uv: [0.0, 0.0],
    })
    .to_vec();
    let affine = Placement::new([200.0, 0.0, 0.0], Rotator::default(), 1.0, [1.0; 3])
        .affine()
        .unwrap();
    DrawPlan {
        meshes: vec![MeshData {
            path: "Meshes.Wall".into(),
            vertices,
            indices: vec![0, 1, 2, 0, 2, 3],
            sections: vec![SectionRange {
                first_index: 0,
                index_count: 6,
            }],
            lightmap_uvs: false,
        }],
        slots: Vec::new(),
        lightings: Vec::new(),
        batches: vec![Batch {
            key: BatchKey {
                mesh: 0,
                section: 0,
                material: None,
                lighting: None,
                two_sided: true,
                mirrored: false,
            },
            instances: vec![Instance::new(&affine)],
        }],
        stats: Default::default(),
    }
}

#[test]
#[ignore = "needs a GPU adapter or the software fallback"]
fn pawns_are_depth_tested_inside_the_scene_pass() {
    // The quad drawn inside the scene pass, in front of the wall (x = 100) and behind it (x = 300). In front its
    // colour shows at the centre; behind, the wall's grey does, because the quad is tested against the scene's depth.
    let Some((device, queue, errors)) = headless() else {
        panic!("no adapter, not even the software fallback");
    };
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let lighting = Lighting::viewer();
    let mut none =
        |_: &str| -> Result<ow_assets::texture::TextureAsset, String> { Err("no texture".into()) };
    let mut scene =
        SceneRenderer::new(&device, &queue, format, &wall(), lighting, &mut none).unwrap();
    let mesh = quad();
    let skeleton = Skeleton::new(&mesh).unwrap();
    let mut gpu = SkinnedMesh::new(&device, format, &mesh, &bind_normals(&mesh)).unwrap();
    gpu.set_color([0.9, 0.1, 0.1]);
    let palette: Vec<Mat4> = skin_matrices(&skeleton, &skeleton.bind_pose())
        .iter()
        .map(Affine::to_engine)
        .collect();
    let camera = crate::camera::FlyCamera::at([0.0; 3], Rotator::default());
    let vp = camera.view_projection(32, 32).unwrap();
    let mut centre_at = |x: f32| {
        let place = Placement::new([x, 0.0, 0.0], Rotator::default(), 1.0, [1.0; 3])
            .affine()
            .unwrap()
            .to_engine();
        let rgba = crate::anim::arms::capture_with(
            &device,
            &queue,
            format,
            (32, 32),
            &mut |encoder, target| {
                gpu.skin(&queue, encoder, &palette, &place)?;
                gpu.set_view(&queue, &vp, &lighting);
                scene.encode_with(
                    &device,
                    &queue,
                    encoder,
                    target,
                    (32, 32),
                    &vp,
                    [0.0; 3],
                    &mut |pass| gpu.draw_in(pass),
                );
                Ok(())
            },
        )
        .unwrap();
        let px = &rgba[(16 * 32 + 16) * 4..][..3];
        [px[0], px[1], px[2]]
    };
    let front = centre_at(100.0);
    assert!(
        i32::from(front[0]) - i32::from(front[2]) > 40,
        "in front: {front:?}"
    );
    let behind = centre_at(300.0);
    assert!(
        (i32::from(behind[0]) - i32::from(behind[2])).abs() < 20,
        "behind: {behind:?}"
    );
    assert_eq!(errors.lock().unwrap().as_slice(), &[] as &[String]);
}

fn worst(a: &[[f32; 3]], b: &[[f32; 3]]) -> f32 {
    a.iter()
        .zip(b)
        .flat_map(|(p, q)| (0..3).map(move |k| (p[k] - q[k]).abs()))
        .fold(0.0, f32::max)
}

/// The installed-game check: `OW_HP_COOKED` (CookedPCConsole), `OW_HP_EXPORTS` (`;`-separated UModel package export
/// folders, holding the bullymong's and Maya's meshes and sets), `OW_HP_OUT` (a file under `local/`). Loads both
/// pawns through the store, reports their trees' censuses, and compares the bullymong's GPU skinning with the CPU
/// reference along its idle (fails above 1e-4 uu in model space).
#[test]
#[ignore = "needs the installed game, UModel exports under local/ and a GPU adapter"]
fn installed_pawns_load_and_gpu_matches_cpu() {
    let cooked = PathBuf::from(std::env::var_os("OW_HP_COOKED").expect("set OW_HP_COOKED"));
    let roots: Vec<PathBuf> = std::env::var("OW_HP_EXPORTS")
        .expect("set OW_HP_EXPORTS")
        .split(';')
        .map(PathBuf::from)
        .collect();
    let out = std::env::var_os("OW_HP_OUT").expect("set OW_HP_OUT");
    let mut defs = DefStore::open(
        cooked,
        ow_defs::identity::IdentityPolicy::FirstInScanOrder,
        ParityMode::Off,
    )
    .unwrap();
    let mut space = ObjectSpace::new();
    let mut text = String::new();
    let mut pawns = Vec::new();
    for k in KINDS {
        let mut pawn = load(
            &mut defs,
            &mut space,
            k,
            &roots,
            actor_placement([0.0; 3], 0.0).unwrap(),
        )
        .unwrap();
        pawn.advance_steps(60, 1.0 / 60.0).unwrap();
        let _ = writeln!(
            text,
            "{}: archetype {}, component {}, mesh {}, sets {:?}, tree {}, component placement {:?}",
            k.name,
            pawn.def.archetype,
            pawn.def.component,
            pawn.def.mesh,
            pawn.def.anim_sets,
            pawn.def.tree,
            pawn.def.component_placement
        );
        let _ = writeln!(
            text,
            "  {} bones, {} vertices, {} triangles; tree {} nodes, {} evaluated, NotYet {:?}; NotYet evaluations \
             passed to the first child after 1 s: {}",
            pawn.skeleton.bone_count(),
            pawn.mesh.vertices.len(),
            pawn.mesh.triangle_count(),
            pawn.census.nodes,
            pawn.census.evaluated,
            pawn.census.not_yet,
            pawn.passed_through
        );
        // Which clips the idle path plays, and which the tree names but no set has.
        let mut d = Driver::new(&pawn.tree, &pawn.clips, &pawn.skeleton, inputs()).unwrap();
        d.unsupported = Unsupported::PassFirstChild;
        d.tick(1.0 / 60.0).unwrap();
        let _ = d.locals().unwrap();
        let weighted: Vec<String> = d
            .effective_weights()
            .iter()
            .filter_map(|(id, w)| match &pawn.tree.nodes.get(id)?.kind {
                Kind::Sequence(p) if *w > 1e-6 => Some(format!(
                    "{} x{}",
                    p.clip.as_deref().unwrap_or("None"),
                    ow_core::fmt::fmt_g(*w, 3)
                )),
                _ => None,
            })
            .collect();
        let _ = writeln!(
            text,
            "  idle path plays {weighted:?}; clips no set has: {:?}",
            d.log.missing_clips
        );
        pawns.push(pawn);
    }
    let (device, queue, errors) = headless().expect("no adapter");
    let bully = &mut pawns[0];
    let gpu = SkinnedMesh::new(
        &device,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        &bully.mesh,
        &bully.normals,
    )
    .unwrap();
    let mut model_worst: f32 = 0.0;
    let mut palette = Vec::new();
    for _ in 0..6 {
        let pose = bully.advance_steps(30, 1.0 / 60.0).unwrap();
        let matrices = skin_matrices(&bully.skeleton, &pose);
        let cpu = skin(&bully.mesh, &matrices).unwrap();
        bully.palette_into(&pose, &mut palette);
        let got = gpu
            .read_back(&device, &queue, &palette, &Mat4::IDENTITY)
            .unwrap();
        model_worst = model_worst.max(worst(&got, &cpu));
        let far = pose
            .model
            .iter()
            .map(|m| m.translation)
            .fold(0.0f64, |a, t| {
                a.max(ow_core::num::sqrt_f64(
                    t[0] * t[0] + t[1] * t[1] + t[2] * t[2],
                ))
            });
        let _ = writeln!(
            text,
            "bullymong pose: farthest bone {} uu from the mesh origin",
            ow_core::fmt::fmt_g(far, 5)
        );
    }
    let _ = writeln!(
        text,
        "bullymong GPU against CPU, six idle times: worst {} uu",
        ow_core::fmt::fmt_g(f64::from(model_worst), 3)
    );
    std::fs::write(out, &text).unwrap();
    assert!(model_worst <= GPU_TOLERANCE, "{text}");
    assert_eq!(errors.lock().unwrap().as_slice(), &[] as &[String]);
}
