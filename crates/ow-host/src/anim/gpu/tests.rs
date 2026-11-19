//! Owner: lane H6-SKIN. GPU skinning against the CPU reference on invented meshes (AGENTS.md rule 9). The drawn
//! tests need a GPU adapter (or the platform's software one), so they are `#[ignore]`d in the ordinary run, as
//! `crate::render`'s are: `cargo test -p ow-host --locked -- --ignored anim::gpu`.

use ow_assets::animation::BoundClip;
use ow_assets::skeletal::{Bone, BoneTransform, Influence, SkeletalMesh, SkinSection, SkinVertex};
use ow_core::units::Rotator;

use super::*;
use crate::anim::pose::{Skeleton, Wrap};
use crate::anim::skin::{skin, skin_matrices};
use crate::executor::block_on;
use crate::frame::{Affine, Placement, ToEngine};

/// Largest difference allowed between a GPU-skinned position and the CPU reference, uu. The CPU path is `f64`
/// rounded once; the GPU's is `f32` throughout, so the two differ by a few `f32` steps at the mesh's size.
pub const GPU_TOLERANCE: f32 = 1e-4;

type Errors = std::sync::Arc<std::sync::Mutex<Vec<String>>>;

/// A device with no window: the preferred adapter, else the software fallback, with an error handler that records.
pub(crate) fn headless() -> Option<(wgpu::Device, wgpu::Queue, Errors)> {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .ok()
    .or_else(|| {
        block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            force_fallback_adapter: true,
            ..Default::default()
        }))
        .ok()
    })?;
    let (device, queue) =
        block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).ok()?;
    let errors = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let sink = std::sync::Arc::clone(&errors);
    device.on_uncaptured_error(std::sync::Arc::new(move |e: wgpu::Error| {
        if let Ok(mut list) = sink.lock() {
            list.push(e.to_string());
        }
    }));
    Some((device, queue, errors))
}

const ID: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
const S: f32 = std::f32::consts::FRAC_1_SQRT_2;

/// A 3-bone chain along X (Root at 0, B1 at 10, B2 at 20) under a strip of four quads from x 0 to 30, one point per
/// vertex. Weights: on Root below x 10, blended Root/B1 at 10, B1 at 15, blended B1/B2 (with a third and fourth
/// influence of weight 0.1 each on the Root) at 20, B2 beyond.
fn strip() -> SkeletalMesh {
    let bones = vec![
        Bone::new("Root", None, BoneTransform::new(ID, [0.0; 3])),
        Bone::new("B1", Some(0), BoneTransform::new(ID, [10.0, 0.0, 0.0])),
        Bone::new("B2", Some(1), BoneTransform::new(ID, [10.0, 0.0, 0.0])),
    ];
    let xs: [f32; 5] = [0.0, 10.0, 15.0, 20.0, 30.0];
    let weights = |x: f32| -> Vec<Influence> {
        match x {
            x if x < 10.0 => vec![Influence::new(0, 1.0)],
            x if x < 15.0 => vec![Influence::new(0, 0.5), Influence::new(1, 0.5)],
            x if x < 20.0 => vec![Influence::new(1, 1.0)],
            x if x < 30.0 => vec![
                Influence::new(1, 0.4),
                Influence::new(2, 0.4),
                Influence::new(0, 0.1),
                Influence::new(0, 0.1),
            ],
            _ => vec![Influence::new(2, 1.0)],
        }
    };
    let mut vertices = Vec::new();
    for (k, x) in xs.iter().enumerate() {
        for z in [-2.0f32, 2.0] {
            let point = vertices.len();
            vertices.push(SkinVertex::new(
                [*x, 1.0 + k as f32 * 0.25, z],
                [0.0, 0.0],
                point,
                weights(*x),
            ));
        }
    }
    let mut indices = Vec::new();
    for q in 0..4u32 {
        let (a, b, c, d) = (2 * q, 2 * q + 1, 2 * q + 2, 2 * q + 3);
        indices.extend_from_slice(&[a, b, c, b, d, c]);
    }
    SkeletalMesh::new(
        bones,
        vertices,
        indices,
        vec![SkinSection::new(0, 0, 8)],
        vec!["M".into()],
    )
}

/// B1 turned 90 degrees about Z and B2 a further -45 degrees about Y, one frame.
fn bend() -> BoundClip {
    let root = Some(vec![BoneTransform::new(ID, [0.0; 3])]);
    let b1 = Some(vec![BoneTransform::new([0.0, 0.0, S, S], [10.0, 0.0, 0.0])]);
    let y45 = [0.0, -0.382_683_43, 0.0, 0.923_879_5];
    let b2 = Some(vec![BoneTransform::new(y45, [10.0, 0.0, 0.0])]);
    BoundClip::new("Bend", 10.0, 1, vec![root, b1, b2])
}

fn worst(a: &[[f32; 3]], b: &[[f32; 3]]) -> f32 {
    a.iter()
        .zip(b)
        .flat_map(|(p, q)| (0..3).map(move |k| (p[k] - q[k]).abs()))
        .fold(0.0, f32::max)
}

#[test]
fn bind_normals_face_out_of_ue3_index_order() {
    // One triangle in the XY plane. Its index order (0, 1, 2) turns counter-clockwise seen from +Z, so its cross
    // product points along +Z; UE3's order runs against the outward normal, which is therefore -Z.
    let bones = vec![Bone::new("Root", None, BoneTransform::new(ID, [0.0; 3]))];
    let v = |p: [f32; 3], i: usize| SkinVertex::new(p, [0.0, 0.0], i, vec![Influence::new(0, 1.0)]);
    let mut mesh = SkeletalMesh::new(
        bones,
        vec![
            v([0.0, 0.0, 0.0], 0),
            v([1.0, 0.0, 0.0], 1),
            v([0.0, 1.0, 0.0], 2),
        ],
        vec![0, 1, 2],
        vec![SkinSection::new(0, 0, 1)],
        vec!["M".into()],
    );
    for n in bind_normals(&mesh) {
        assert_eq!(n, [0.0, 0.0, -1.0]);
    }
    // A vertex no triangle uses gets a zero normal, not NaN.
    mesh.vertices.push(v([5.0, 5.0, 5.0], 3));
    assert_eq!(bind_normals(&mesh)[3], [0.0; 3]);
}

#[test]
#[ignore = "needs a GPU adapter or the software fallback"]
fn gpu_skinning_matches_the_cpu_reference() {
    let Some((device, queue, errors)) = headless() else {
        panic!("no adapter, not even the software fallback");
    };
    let mesh = strip();
    let skeleton = Skeleton::new(&mesh).unwrap();
    let gpu = SkinnedMesh::new(
        &device,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        &mesh,
        &bind_normals(&mesh),
    )
    .unwrap();
    // At the bind pose the skinned vertices are the mesh's.
    let bind = skeleton.bind_pose();
    let palette: Vec<Mat4> = skin_matrices(&skeleton, &bind)
        .iter()
        .map(Affine::to_engine)
        .collect();
    let got = gpu
        .read_back(&device, &queue, &palette, &Mat4::IDENTITY)
        .unwrap();
    let stored: Vec<[f32; 3]> = mesh.vertices.iter().map(|v| v.position).collect();
    assert!(
        worst(&got, &stored) <= GPU_TOLERANCE,
        "bind: {}",
        worst(&got, &stored)
    );
    // A bent pose: the GPU equals the CPU reference.
    let pose = skeleton.sample(&bend(), 0.0, Wrap::Clamp).unwrap();
    let matrices = skin_matrices(&skeleton, &pose);
    let cpu = skin(&mesh, &matrices).unwrap();
    let palette: Vec<Mat4> = matrices.iter().map(Affine::to_engine).collect();
    let got = gpu
        .read_back(&device, &queue, &palette, &Mat4::IDENTITY)
        .unwrap();
    let diff = worst(&got, &cpu);
    assert!(diff <= GPU_TOLERANCE, "bent: {diff}");
    // The pose moved the vertices (the comparison is not of two unmoved meshes).
    assert!(worst(&cpu, &stored) > 5.0);
    // Placed after the weighted sum (as the play window does): the CPU's model-space result, placed.
    let placed = Placement::new(
        [100.0, -50.0, 20.0],
        Rotator::new(0, 16384, 0),
        1.0,
        [1.0; 3],
    )
    .affine()
    .unwrap();
    let cpu_placed = place_all(&cpu, &placed);
    let got = gpu
        .read_back(&device, &queue, &palette, &placed.to_engine())
        .unwrap();
    assert!(
        worst(&got, &cpu_placed) <= GPU_TOLERANCE * 10.0,
        "placed: {}",
        worst(&got, &cpu_placed)
    );
    assert_eq!(errors.lock().unwrap().as_slice(), &[] as &[String]);
}

/// Each point through `placement`, rounded to `f32`.
fn place_all(points: &[[f32; 3]], placement: &Affine) -> Vec<[f32; 3]> {
    points
        .iter()
        // Positions of a few thousand uu: f64 to f32 rounds.
        .map(|p| placement.point(p.map(f64::from)).map(|c| c as f32))
        .collect()
}

#[test]
#[ignore = "needs a GPU adapter or the software fallback"]
fn a_weight_sum_off_one_is_not_scaled_by_the_placement() {
    // One vertex at (10, 0, 0) on the root with weight 0.999 (within the 1e-3 the reader allows), placed 3,600 uu
    // out. Skinned in model space and then placed, it lands 0.01 uu from where weight 1 puts it; skinned with the
    // placement inside the palette, the missing 0.001 would scale the whole world position: about 3.6 uu.
    let Some((device, queue, errors)) = headless() else {
        panic!("no adapter, not even the software fallback");
    };
    let bones = vec![Bone::new("Root", None, BoneTransform::new(ID, [0.0; 3]))];
    let mesh = SkeletalMesh::new(
        bones,
        vec![SkinVertex::new(
            [10.0, 0.0, 0.0],
            [0.0, 0.0],
            0,
            vec![Influence::new(0, 0.999)],
        )],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    let gpu = SkinnedMesh::new(
        &device,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        &mesh,
        &[[0.0; 3]],
    )
    .unwrap();
    let placed = Placement::new([3600.0, 0.0, 0.0], Rotator::default(), 1.0, [1.0; 3])
        .affine()
        .unwrap();
    let got = gpu
        .read_back(&device, &queue, &[Mat4::IDENTITY], &placed.to_engine())
        .unwrap();
    assert!((got[0][0] - 3609.99).abs() < 1e-2, "{:?}", got[0]);
    // The old order, for contrast: the placement in the palette scales with the weight.
    let old = gpu
        .read_back(&device, &queue, &[placed.to_engine()], &Mat4::IDENTITY)
        .unwrap();
    assert!((old[0][0] - 3610.0).abs() > 3.0, "{:?}", old[0]);
    assert_eq!(errors.lock().unwrap().as_slice(), &[] as &[String]);
}

#[test]
#[ignore = "needs a GPU adapter or the software fallback"]
fn bad_meshes_and_palettes_are_refused() {
    let Some((device, queue, _errors)) = headless() else {
        panic!("no adapter, not even the software fallback");
    };
    let mesh = strip();
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let gpu = SkinnedMesh::new(&device, format, &mesh, &bind_normals(&mesh)).unwrap();
    assert_eq!(gpu.bone_count(), 3);
    let id = Mat4::IDENTITY;
    assert!(gpu.read_back(&device, &queue, &[id; 2], &id).is_err());
    let mut nan = Mat4::IDENTITY;
    nan.cols[0][0] = f32::NAN;
    assert!(gpu.read_back(&device, &queue, &[nan, id, id], &id).is_err());
    assert!(gpu.read_back(&device, &queue, &[id; 3], &nan).is_err());
    assert!(SkinnedMesh::new(&device, format, &mesh, &[]).is_err());
    let mut stray = strip();
    stray.vertices[0].influences[0] = Influence::new(7, 1.0);
    assert!(SkinnedMesh::new(&device, format, &stray, &bind_normals(&stray)).is_err());
}

#[test]
#[ignore = "needs a GPU adapter or the software fallback"]
fn the_skinned_strip_is_drawn_where_the_cpu_puts_it() {
    // The strip, bent, placed 60 uu in front of a camera at the origin looking along +X: the centre pixel of a
    // 64 x 64 capture shows the flat skin colour over the clear colour, and a pixel far to the side does not.
    let Some((device, queue, errors)) = headless() else {
        panic!("no adapter, not even the software fallback");
    };
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let mesh = strip();
    let skeleton = Skeleton::new(&mesh).unwrap();
    let mut gpu = SkinnedMesh::new(&device, format, &mesh, &bind_normals(&mesh)).unwrap();
    let bind = skeleton.bind_pose();
    // Turn the strip to face the camera: its X runs along the view's Y, centred on the view axis.
    let placed = Placement::new([60.0, -15.0, 0.0], Rotator::new(0, 16384, 0), 1.0, [1.0; 3])
        .affine()
        .unwrap();
    let palette: Vec<Mat4> = skin_matrices(&skeleton, &bind)
        .iter()
        .map(Affine::to_engine)
        .collect();
    let place = placed.to_engine();
    let lighting = crate::render::Lighting::viewer();
    let camera = crate::camera::FlyCamera::at([0.0; 3], Rotator::default());
    let vp = camera.view_projection(64, 64).unwrap();
    let rgba =
        crate::anim::arms::capture_with(&device, &queue, format, (64, 64), &mut |encoder, view| {
            // Clear to the scene's sky colour first, as the scene pass would.
            {
                let _clear = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("clear"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(crate::render::CLEAR),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    ..Default::default()
                });
            }
            gpu.skin(&queue, encoder, &palette, &place)?;
            gpu.draw(&device, &queue, encoder, view, (64, 64), &vp, &lighting);
            Ok(())
        })
        .unwrap();
    let px = |x: usize, y: usize| &rgba[(y * 64 + x) * 4..][..3];
    let centre = px(32, 32);
    // The skin colour is redder than blue; the clear colour is bluer than red.
    assert!(centre[0] > centre[2], "centre {centre:?}");
    let corner = px(1, 1);
    assert!(corner[2] > corner[0], "corner {corner:?}");
    assert_eq!(errors.lock().unwrap().as_slice(), &[] as &[String]);
}
