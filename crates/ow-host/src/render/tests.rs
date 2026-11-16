//! Owner: lane H2-DRAW. Drawn tests of the renderer on an invented triangle, offscreen: no window, but a GPU adapter
//! (or the platform's software adapter) is needed, so they are `#[ignore]`d in the ordinary run and started with
//! `cargo test -p ow-host --locked -- --ignored render::tests`.

use super::*;
use crate::draw::{
    Batch, DiffuseCandidate, Instance, MaterialSlot, MeshData, SectionRange, Vertex,
};
use crate::executor::block_on;
use crate::frame::{self, Affine};
use crate::material::{Blend, SkyInputs};
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
        material: None,
        two_sided,
        mirrored: affine.mirrors(),
    };
    let plan = DrawPlan {
        meshes: vec![mesh],
        slots: Vec::new(),
        batches: vec![Batch {
            key,
            instances: vec![Instance::new(&affine)],
        }],
        stats: Default::default(),
    };
    (plan, affine)
}

/// An opaque, lit material slot with `diffuse` candidates.
fn slot(diffuse: Vec<DiffuseCandidate>) -> MaterialSlot {
    MaterialSlot {
        path: "Mats.Probe".into(),
        diffuse,
        constant: None,
        blend: Blend::Opaque,
        unlit: false,
        clip: crate::material::DEFAULT_CLIP,
        sky: None,
    }
}

/// Whether the centre pixel shows the (grey, lit) triangle rather than the sky clear colour, which is bluer.
fn drawn_at_centre(device: &wgpu::Device, queue: &wgpu::Queue, plan: &DrawPlan) -> bool {
    let mut none = |_: &str| -> Result<TextureAsset, String> { Err("no texture".into()) };
    let mut r = SceneRenderer::new(
        device,
        queue,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        plan,
        Lighting::viewer(),
        &mut none,
    )
    .unwrap();
    let camera = FlyCamera::at([0.0; 3], Rotator::default());
    let vp = camera.view_projection(32, 32).unwrap();
    let rgba = r.capture(device, queue, (32, 32), &vp, [0.0; 3]).unwrap();
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
    let candidate = |path: &str, priority| DiffuseCandidate {
        path: path.into(),
        priority,
        stub: false,
    };
    // The first material's best candidate does not decode: it falls back to the next. The second has only that one:
    // it is drawn grey, and the failure is not decoded twice.
    p.slots = vec![
        slot(vec![
            candidate("Tex.Broken", 100),
            candidate("Tex.Fine", 90),
        ]),
        slot(vec![candidate("Tex.Broken", 100)]),
    ];
    p.batches[0].key.material = Some(0);
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
        Lighting::viewer(),
        &mut source,
    )
    .unwrap();
    let u = r.upload;
    assert_eq!(
        (u.meshes, u.vertex_bytes, u.index_bytes, u.instance_bytes),
        (1, 96, 12, 112)
    );
    assert_eq!((u.textures, u.textures_failed, u.texture_bytes), (1, 1, 84));
    assert_eq!((u.fallbacks, u.grey_materials), (1, 1));
    // A target format the renderer does not draw into is refused, not drawn wrong.
    let refused = SceneRenderer::new(
        &device,
        &queue,
        wgpu::TextureFormat::R8Unorm,
        &p,
        Lighting::viewer(),
        &mut source,
    );
    assert!(refused.is_err());
    assert_eq!(r.draw_calls(), 1);
    // The same undecodable slot made translucent and drawn: grey would cover what is behind, so it is left out.
    p.slots[1].blend = Blend::Translucent;
    p.batches[0].key.material = Some(1);
    let r = SceneRenderer::new(
        &device,
        &queue,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        &p,
        Lighting::viewer(),
        &mut source,
    )
    .unwrap();
    let u = r.upload;
    assert_eq!(
        (u.grey_materials, u.blended_undecoded, u.skipped_instances),
        (1, 1, 1)
    );
    assert_eq!((r.draw_calls(), r.blended_instances()), (0, 0));
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
fn the_scenes_lights_set_the_exposure() {
    use crate::lighting::{SceneLights, SkyLight, Sun};
    let mut lights = SceneLights::default();
    assert_eq!(Lighting::from_scene(&lights), Lighting::viewer());
    lights.sun = Some(Sun {
        path: "S".into(),
        level: "L".into(),
        rotation: Rotator::default(),
        color: [1.0, 0.5, 0.5],
        brightness: 4.0,
        tags_at: 0,
    });
    lights.sky = Some(SkyLight {
        path: "K".into(),
        level: "L".into(),
        color: [0.25, 0.5, 1.0],
        brightness: 1.0,
        lower_color: [1.0, 1.0, 1.0],
        lower_brightness: 0.5,
    });
    let l = Lighting::from_scene(&lights);
    assert_eq!(
        (l.sun, l.sky_upper, l.sky_lower),
        ([4.0, 2.0, 2.0], [0.25, 0.5, 1.0], [0.5; 3])
    );
    // The brightest sum is red: 4 + 0.25.
    assert_eq!(l.exposure, 1.0 / 4.25);
    assert_eq!(l.sun_direction, [-1.0, 0.0, 0.0]);
}

/// A 4 x 4 PF_A8R8G8B8 texture of one colour (`bgra` as stored) with its one mip, through the `ow-assets` reader.
fn solid_texture(bgra: [u8; 4]) -> Result<TextureAsset, String> {
    use ow_package::package::Package;
    use ow_package::testkit::{PackageBuilder, words};
    let mut b = PackageBuilder::new_empty();
    b.names(["None"]);
    let class = b.add_import("Core", "Class", 0, "Texture2D");
    let mut payload = words(&[0]);
    let detail = b.fname("EPixelFormat", 0);
    let value = b.fname("PF_A8R8G8B8", 0);
    payload.extend(b.tag_detail("Format", "ByteProperty", &detail, &value));
    payload.extend(b.none());
    payload.extend(vec![0u8; 16]);
    payload.extend(words(&[1, 0, 64, 64, 0]));
    for _ in 0..16 {
        payload.extend(bgra);
    }
    payload.extend(words(&[4, 4]));
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

/// The centre pixel of `slots` drawn on two triangles: batch 0 on the first slot at `near` uu, batch 1 (when there is
/// a second slot) at 200 uu, unlit unless the slot says otherwise. Every slot's diffuse is `Tex.<index>` from
/// `colours` (BGRA).
fn centre_of(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    slots: Vec<MaterialSlot>,
    near: f32,
    colours: &[[u8; 4]],
) -> [u8; 4] {
    centre_sorted_from(device, queue, slots, near, colours, [0.0; 3])
}

/// [`centre_of`] with the blended instances sorted as seen from `sort_eye` (the camera stays at the origin).
fn centre_sorted_from(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    slots: Vec<MaterialSlot>,
    near: f32,
    colours: &[[u8; 4]],
    sort_eye: [f32; 3],
) -> [u8; 4] {
    let mut batches = Vec::new();
    for (i, distance) in [near, 200.0].into_iter().enumerate().take(slots.len()) {
        let affine = frame::Placement::new([distance, 0.0, 0.0], Rotator::default(), 1.0, [1.0; 3])
            .affine()
            .unwrap();
        batches.push(Batch {
            key: BatchKey {
                mesh: 0,
                section: 0,
                material: Some(u32::try_from(i).unwrap()),
                two_sided: true,
                mirrored: false,
            },
            instances: vec![Instance::new(&affine)],
        });
    }
    let plan = DrawPlan {
        meshes: vec![triangle(true)],
        slots,
        batches,
        stats: Default::default(),
    };
    let colours = colours.to_vec();
    let mut source = |path: &str| -> Result<TextureAsset, String> {
        let index: usize = path
            .trim_start_matches("Tex.")
            .parse()
            .map_err(|_| "a path")?;
        solid_texture(colours[index])
    };
    let mut r = SceneRenderer::new(
        device,
        queue,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        &plan,
        Lighting::viewer(),
        &mut source,
    )
    .unwrap();
    let camera = FlyCamera::at([0.0; 3], Rotator::default());
    let vp = camera.view_projection(32, 32).unwrap();
    let rgba = r.capture(device, queue, (32, 32), &vp, sort_eye).unwrap();
    let px = &rgba[(16 * 32 + 16) * 4..][..4];
    [px[0], px[1], px[2], px[3]]
}

fn unlit(texture: usize, blend: Blend) -> MaterialSlot {
    let mut s = slot(vec![DiffuseCandidate {
        path: format!("Tex.{texture}"),
        priority: 100,
        stub: false,
    }]);
    s.unlit = true;
    s.blend = blend;
    s
}

#[test]
#[ignore = "needs a GPU adapter or the software fallback"]
fn blend_modes_draw_as_their_material_says() {
    let Some((device, queue, errors)) = headless() else {
        panic!("no adapter, not even the software fallback");
    };
    // BGRA as stored: red with alpha 0, red opaque, white at half alpha, mid grey opaque, red at half alpha.
    let colours = [
        [0, 0, 255, 0],
        [0, 0, 255, 255],
        [255, 255, 255, 128],
        [128, 128, 128, 255],
        [0, 0, 255, 128],
    ];
    let clear = centre_of(&device, &queue, vec![], 200.0, &colours);
    // Masked: alpha 0 under the clip value is discarded, alpha 255 is drawn.
    assert_eq!(
        centre_of(
            &device,
            &queue,
            vec![unlit(0, Blend::Masked)],
            200.0,
            &colours
        ),
        clear
    );
    assert_eq!(
        centre_of(
            &device,
            &queue,
            vec![unlit(1, Blend::Masked)],
            200.0,
            &colours
        ),
        [255, 0, 0, 255]
    );
    // Translucent: white at half alpha over the sky lands between the two.
    let half = centre_of(
        &device,
        &queue,
        vec![unlit(2, Blend::Translucent)],
        200.0,
        &colours,
    );
    assert!(
        half[0] > clear[0] && half[0] < 255,
        "{half:?} over {clear:?}"
    );
    // Translucent over a diffuse with alpha 255 everywhere: its opacity is the graph's, unknown, so it is not drawn.
    assert_eq!(
        centre_of(
            &device,
            &queue,
            vec![unlit(1, Blend::Translucent)],
            200.0,
            &colours
        ),
        clear
    );
    // Additive: the red adds to what is behind (red channel full, blue kept from the sky).
    let added = centre_of(
        &device,
        &queue,
        vec![unlit(1, Blend::Additive)],
        200.0,
        &colours,
    );
    assert_eq!((added[0], added[2]), (255, clear[2]));
    // Modulate: grey at 100 uu multiplies the opaque red at 200 behind it: red dimmed, green and blue stay 0.
    let modulated = centre_of(
        &device,
        &queue,
        vec![unlit(3, Blend::Modulate), unlit(1, Blend::Opaque)],
        100.0,
        &colours,
    );
    assert!(
        modulated[0] > 0 && modulated[0] < 255 && modulated[1] == 0 && modulated[2] == 0,
        "{modulated:?}"
    );
    // Translucent surfaces write no depth. Sorted from an eye at x 1000, the white one at 100 uu (nearer the camera)
    // is drawn before the red one at 200 uu; the red one still shows through it.
    let near_only = centre_of(
        &device,
        &queue,
        vec![unlit(2, Blend::Translucent)],
        100.0,
        &colours,
    );
    let both = centre_sorted_from(
        &device,
        &queue,
        vec![unlit(2, Blend::Translucent), unlit(4, Blend::Translucent)],
        100.0,
        &colours,
        [1000.0, 0.0, 0.0],
    );
    assert!(
        both[0] > near_only[0] && both[1] < near_only[1],
        "{both:?} against the near one alone {near_only:?}"
    );
    // The sky: drawn first with no depth test, so a farther opaque triangle covers it.
    let mut sky = unlit(1, Blend::Opaque);
    sky.sky = Some(SkyInputs {
        strip: "Tex.1".into(),
        clouds: "Tex.1".into(),
        time_of_day: 0.0,
        sky_brightness: 1.0,
        cloud_brightness: 1.0,
        cloud_cap_opacity: 0.0,
    });
    assert_eq!(
        centre_of(&device, &queue, vec![sky.clone()], 200.0, &colours),
        [255, 0, 0, 255]
    );
    let covered = centre_of(
        &device,
        &queue,
        vec![sky, unlit(2, Blend::Opaque)],
        100.0,
        &colours,
    );
    assert_eq!(covered, [255, 255, 255, 255]);
    assert_eq!(errors.lock().unwrap().as_slice(), &[] as &[String]);
}

#[test]
fn captures_convert_rgba_and_bgra_only() {
    assert_eq!(
        capture_order(wgpu::TextureFormat::Rgba8UnormSrgb),
        Some(false)
    );
    assert_eq!(capture_order(wgpu::TextureFormat::Bgra8Unorm), Some(true));
    assert_eq!(capture_order(wgpu::TextureFormat::Rgba16Float), None);
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
