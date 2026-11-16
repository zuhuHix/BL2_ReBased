//! Owner: lane H2-DRAW. Drawn tests of the renderer on an invented triangle, offscreen: no window, but a GPU adapter
//! (or the platform's software adapter) is needed, so they are `#[ignore]`d in the ordinary run and started with
//! `cargo test -p ow-host --locked -- --ignored render::tests`.

use super::*;
use crate::draw::{Batch, MeshData, SectionRange, Vertex};
use crate::executor::block_on;
use crate::frame::{self, Affine};
use ow_core::units::Rotator;

/// Validation errors the device reported, in order.
type Errors = std::sync::Arc<std::sync::Mutex<Vec<String>>>;

/// A device with no window: the preferred adapter, else the software fallback. The error handler records instead of
/// panicking, so a validation error fails the test with its text.
fn headless() -> Option<(wgpu::Device, wgpu::Queue, Errors)> {
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

/// An invented right triangle in the local YZ plane, its stored normal along -X. `against` gives the index order
/// whose cross product points along +X, against the normal (the order the retired UE5 host's OBJ adapter read in
/// UE3 meshes); otherwise the reverse.
fn triangle(against: bool) -> MeshData {
    let normal = [-1.0, 0.0, 0.0];
    let vertices = [
        [0.0, -50.0, -50.0],
        [0.0, 150.0, -50.0],
        [0.0, -50.0, 150.0],
    ]
    .map(|position| Vertex {
        position,
        normal,
        uv: [0.0, 0.0],
    })
    .to_vec();
    let indices = if against {
        vec![0, 1, 2]
    } else {
        vec![0, 2, 1]
    };
    MeshData {
        path: "Meshes.Probe".into(),
        vertices,
        indices,
        sections: vec![SectionRange {
            first_index: 0,
            index_count: 3,
        }],
    }
}

/// One placement of `mesh` 200 uu in front of a camera at the origin facing +X, with `scale3d`.
fn plan(mesh: MeshData, scale3d: [f32; 3], two_sided: bool) -> (DrawPlan, Affine) {
    let affine = frame::Placement::new([200.0, 0.0, 0.0], Rotator::default(), 1.0, scale3d)
        .affine()
        .unwrap();
    let key = BatchKey {
        mesh: 0,
        section: 0,
        texture: None,
        two_sided,
        mirrored: affine.mirrors(),
    };
    let plan = DrawPlan {
        meshes: vec![mesh],
        textures: Vec::new(),
        batches: vec![Batch {
            key,
            instances: vec![affine.to_engine()],
        }],
        stats: Default::default(),
    };
    (plan, affine)
}

/// Whether the centre pixel shows the (grey, lit) triangle rather than the sky clear colour, which is bluer.
fn drawn_at_centre(device: &wgpu::Device, queue: &wgpu::Queue, plan: &DrawPlan) -> bool {
    let mut none = |_: &str| -> Result<TextureAsset, String> { Err("no texture".into()) };
    let mut r = SceneRenderer::new(
        device,
        queue,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        plan,
        &mut none,
    );
    let camera = FlyCamera::at([0.0; 3], Rotator::default());
    let vp = camera.view_projection(32, 32).unwrap();
    let rgba = r.capture(device, queue, (32, 32), &vp).unwrap();
    let px = &rgba[(16 * 32 + 16) * 4..][..4];
    (i32::from(px[2]) - i32::from(px[0])).abs() < 8
}

#[test]
#[ignore = "needs a GPU adapter or the software fallback"]
fn a_known_triangle_renders_with_each_winding() {
    let Some((device, queue, errors)) = headless() else {
        panic!("no adapter, not even the software fallback");
    };
    // Unmirrored, one-sided: the order against the stored normal is the front face with FRONT_FACE (Ccw).
    let (p, affine) = plan(triangle(true), [1.0; 3], false);
    assert!(!affine.mirrors());
    assert!(
        drawn_at_centre(&device, &queue, &p),
        "the UE3-ordered triangle is culled"
    );
    let (p, _) = plan(triangle(false), [1.0; 3], false);
    assert!(
        !drawn_at_centre(&device, &queue, &p),
        "the reversed triangle is drawn"
    );
    // Mirrored (Scale3D Y = -1; the triangle still faces the camera): drawn with MIRRORED_FRONT_FACE (Cw).
    let (p, affine) = plan(triangle(true), [1.0, -1.0, 1.0], false);
    assert!(affine.mirrors());
    assert!(
        drawn_at_centre(&device, &queue, &p),
        "the mirrored triangle is culled"
    );
    let (p, _) = plan(triangle(false), [1.0, -1.0, 1.0], false);
    assert!(
        !drawn_at_centre(&device, &queue, &p),
        "the reversed mirrored triangle is drawn"
    );
    // Two-sided: both orders are drawn.
    let (p, _) = plan(triangle(false), [1.0; 3], true);
    assert!(
        drawn_at_centre(&device, &queue, &p),
        "the two-sided triangle is culled"
    );
    // And the other front face would have shown the opposite: the test tells the two apart.
    assert_ne!(FRONT_FACE, MIRRORED_FRONT_FACE);
    assert_eq!(errors.lock().unwrap().as_slice(), &[] as &[String]);
}

#[test]
#[ignore = "needs a GPU adapter or the software fallback"]
fn the_upload_counts_what_it_sends() {
    let Some((device, queue, errors)) = headless() else {
        panic!("no adapter, not even the software fallback");
    };
    let (mut p, _) = plan(triangle(true), [1.0; 3], false);
    p.textures = vec!["Tex.Fine".into(), "Tex.Broken".into()];
    p.batches[0].key.texture = Some(0);
    // A 4 x 4 texture with its full chain, then one that does not decode.
    let mut source = |path: &str| -> Result<TextureAsset, String> {
        if path != "Tex.Fine" {
            return Err("invented failure".into());
        }
        fine_texture()
    };
    let r = SceneRenderer::new(
        &device,
        &queue,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        &p,
        &mut source,
    );
    let u = r.upload;
    assert_eq!(
        (u.meshes, u.vertex_bytes, u.index_bytes, u.instance_bytes),
        (1, 96, 12, 64)
    );
    assert_eq!((u.textures, u.textures_failed, u.texture_bytes), (1, 1, 84));
    assert_eq!(r.draw_calls(), 1);
    assert_eq!(errors.lock().unwrap().as_slice(), &[] as &[String]);
}

/// A 4 x 4 PF_G8 texture with mips 4, 2 and 1, through the `ow-assets` reader (an invented package).
fn fine_texture() -> Result<TextureAsset, String> {
    use ow_package::package::Package;
    use ow_package::testkit::{PackageBuilder, words};
    let mut b = PackageBuilder::new_empty();
    b.names(["None"]);
    let class = b.add_import("Core", "Class", 0, "Texture2D");
    let mut payload = words(&[0]);
    let detail = b.fname("EPixelFormat", 0);
    let value = b.fname("PF_G8", 0);
    payload.extend(b.tag_detail("Format", "ByteProperty", &detail, &value));
    payload.extend(b.none());
    payload.extend(vec![0u8; 16]);
    payload.extend(words(&[3]));
    for size in [4u32, 2, 1] {
        let bytes = size * size;
        payload.extend(words(&[0, bytes, bytes, 0]));
        payload.extend(vec![100u8; bytes as usize]);
        payload.extend(words(&[size, size]));
    }
    b.add_export(class, "Probe", payload);
    let package = Package::from_decoded(Default::default(), "probe".into(), b.build())
        .map_err(|e| e.to_string())?;
    ow_assets::texture::read_texture(
        &package,
        1,
        4,
        &ow_package::props::NoArrayTypes,
        std::path::Path::new("."),
    )
    .map_err(|e| e.to_string())
}

#[test]
fn a_mip_chain_stops_where_the_sizes_stop_halving() {
    let mut t = fine_texture().unwrap();
    assert_eq!(mip_chain(&t, 8192), [0, 1, 2]);
    assert_eq!(mip_chain(&t, 2), [1, 2]);
    t.mips.remove(1);
    assert_eq!(mip_chain(&t, 8192), [0]);
}

#[test]
fn the_report_quantiles_use_the_nearest_rank() {
    let mut r = DrawReport::default();
    assert_eq!(r.quantile(0.5), None);
    r.intervals = (1..=20).rev().map(Duration::from_millis).collect();
    assert_eq!(r.quantile(0.5), Some(Duration::from_millis(10)));
    assert_eq!(r.quantile(0.95), Some(Duration::from_millis(19)));
    assert_eq!(r.quantile(1.0), Some(Duration::from_millis(20)));
}
