//! Owner: lane H6-SKIN. The installed-game check of the arms: Maya's `Hands_Siren` posed by the hands AnimTree's
//! idle, skinned on the GPU and on the CPU (stage 1's reference), compared vertex by vertex. Needs the install, UModel
//! exports under `local/` and a GPU adapter, so it is `#[ignore]`d:
//! `OW_H6_SKIN_COOKED=<CookedPCConsole> OW_H6_SKIN_EXPORTS=<export folder>[;<more>] OW_H6_SKIN_OUT=<file under local/>
//! cargo test -p ow-host --lib anim::arms::tests::installed_gpu_matches_cpu -- --ignored`.

use std::fmt::Write as _;
use std::path::PathBuf;

use ow_core::ParityMode;
use ow_core::fmt::fmt_g;
use ow_core::units::Rotator;
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;

use super::*;
use crate::anim::gpu::tests::{GPU_TOLERANCE, headless};
use crate::anim::skin::skin;
use crate::frame::Affine;

fn worst(a: &[[f32; 3]], b: &[[f32; 3]]) -> f32 {
    a.iter()
        .zip(b)
        .flat_map(|(p, q)| (0..3).map(move |k| (p[k] - q[k]).abs()))
        .fold(0.0, f32::max)
}

#[test]
#[ignore = "needs the installed game, UModel exports under local/ and a GPU adapter"]
fn installed_gpu_matches_cpu() {
    let cooked =
        PathBuf::from(std::env::var_os("OW_H6_SKIN_COOKED").expect("set OW_H6_SKIN_COOKED"));
    let roots: Vec<PathBuf> = std::env::var("OW_H6_SKIN_EXPORTS")
        .expect("set OW_H6_SKIN_EXPORTS")
        .split(';')
        .map(PathBuf::from)
        .collect();
    let out = std::env::var_os("OW_H6_SKIN_OUT").expect("set OW_H6_SKIN_OUT");
    let mut defs =
        DefStore::open(cooked, IdentityPolicy::FirstInScanOrder, ParityMode::Off).unwrap();
    let mut space = ObjectSpace::new();
    let mut arms = load(&mut defs, &mut space, &roots).unwrap();
    let (device, queue, errors) = headless().expect("no adapter");
    let gpu = SkinnedMesh::new(
        &device,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        &arms.mesh,
        &arms.normals,
    )
    .unwrap();
    let mut text = String::new();
    let _ = writeln!(
        text,
        "Hands_Siren: {} bones, {} vertices, {} triangles; GPU tolerance {} uu",
        arms.mesh.bones.len(),
        arms.mesh.vertices.len(),
        arms.mesh.triangle_count(),
        fmt_g(f64::from(GPU_TOLERANCE), 3)
    );
    // Poses along the idle, model space and placed at an eye in Sanctuary-sized coordinates.
    let view = ViewPose {
        eye: [1200.0, -3400.0, 250.0],
        rotation: Rotator::new(-1200, 21000, 0),
        fly: false,
    };
    let mut model_worst: f32 = 0.0;
    let mut world_worst: f32 = 0.0;
    for step in 0..6 {
        let pose = arms.advance(if step == 0 { 0.0 } else { 0.5 }).unwrap();
        let matrices = crate::anim::skin::skin_matrices(&arms.skeleton, &pose);
        let cpu = skin(&arms.mesh, &matrices).unwrap();
        let palette: Vec<Mat4> = matrices.iter().map(Affine::to_engine).collect();
        let got = gpu.read_back(&device, &queue, &palette).unwrap();
        let m = worst(&got, &cpu);
        model_worst = model_worst.max(m);
        let placement = arms.placement(&pose, &view).unwrap();
        let world: Vec<Affine> = matrices.iter().map(|s| s.then(&placement)).collect();
        let cpu_world = skin(&arms.mesh, &world).unwrap();
        let got_world = gpu
            .read_back(&device, &queue, &arms.palette(&pose, &placement))
            .unwrap();
        let w = worst(&got_world, &cpu_world);
        world_worst = world_worst.max(w);
        // Where the hands sit relative to the eye, in the view's frame (x forward, y right, z up).
        let camera = pose.model[arms.camera_bone].translation;
        let hand = |name: &str| {
            arms.mesh
                .bone_index(name)
                .map(|i| pose.model[i].translation)
        };
        let rel = |p: [f64; 3]| [p[0] - camera[0], p[1] - camera[1], p[2] - camera[2]];
        let _ = writeln!(
            text,
            "t {} s: model worst {} uu, world worst {} uu; R_Hand from Camera {:?}, L_Hand {:?}",
            fmt_g(f64::from(u8::try_from(step).unwrap()) * 0.5, 3),
            fmt_g(f64::from(m), 3),
            fmt_g(f64::from(w), 3),
            hand("R_Hand").map(|p| rel(p).map(|c| fmt_g(c, 4))),
            hand("L_Hand").map(|p| rel(p).map(|c| fmt_g(c, 4)))
        );
    }
    let _ = writeln!(
        text,
        "model-space worst {} uu, world worst {} uu",
        fmt_g(f64::from(model_worst), 3),
        fmt_g(f64::from(world_worst), 3)
    );
    std::fs::write(out, &text).unwrap();
    assert!(model_worst <= GPU_TOLERANCE, "{text}");
    // Placed thousands of uu from the origin, f32 has a coarser step: 1e-3 uu there.
    assert!(world_worst <= 1e-3, "{text}");
    assert_eq!(errors.lock().unwrap().as_slice(), &[] as &[String]);
}
