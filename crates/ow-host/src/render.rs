//! Owner: lane H2-DRAW (host track, stage 2). The scene renderer on wgpu: it uploads a [`DrawPlan`] once (LOD 0
//! vertex and index buffers, one instance buffer, the textures) and draws it: each opaque, masked or sky batch as one
//! instanced, indexed draw, each blended instance on its own (lane H8-MATERIALS, below). Nothing here claims BL2's
//! look (no normal maps, lightmaps, fog or shadows).
//!
//! AI-assisted (Claude Code). Written from the wgpu 30.0.1 API, not from any listing. Decision:
//! `docs/decisions/2026-11/2026-11-16-51-*`.
//!
//! **Depth** is reverse-Z with no far plane ([`crate::frame::perspective_reverse_z_infinite`]): a `Depth32Float`
//! buffer cleared to 0, compare `Greater`.
//!
//! **Front faces.** A draw whose world transform does not mirror uses [`FRONT_FACE`]; a mirroring one
//! ([`crate::frame::Affine::mirrors`]) the other face, `Cw`, as B2 did. [`FRONT_FACE`] is `Ccw`, settled by the drawn
//! test `a_known_triangle_renders_with_each_winding` (module `tests`): a triangle whose index order runs against its
//! stored normal, as the retired UE5 host's OBJ adapter read UE3's order, faces a camera on its normal's side when
//! drawn through the boundary's view and projection with `Ccw`. That UE3 meshes follow that order is the data side
//! of the question: the installed Sanctuary driver counts it (decision 51). One-sided materials cull back faces;
//! two-sided ones cull nothing and light the back side with the flipped normal.
//!
//! **Textures** are decoded one at a time through the source the caller gives (on the installed game,
//! [`crate::scene::Scene::decode_texture`], that is `ow-assets`) and uploaded with every resident mip that halves the
//! one before; the CPU copy is dropped at once. Each material's diffuse is the best of its candidates that decodes
//! ([`crate::draw::pick_from`]): a candidate that does not decode, or has no mip the device takes, is skipped for the
//! next, as the tool skips it; a material with none left is drawn with a plain grey texture. Both are counted.
//!
//! **Normals** go through each instance's normal matrix (the inverse transpose of its 3x3, computed on the CPU in
//! `f64`, [`crate::draw::normal_matrix`]), not the model matrix, so non-uniform scales light slanted faces correctly.
//!
//! **Materials and passes** (lane H8-MATERIALS). Each material slot of the plan has its own bind group: its colour
//! texture (the diffuse, or the sky's time-of-day strip), a data texture (the sky's clouds, uploaded linear), and a
//! uniform with its constant colour, clip value, lit or unlit, blend and sky inputs. A frame draws the sky first
//! (no depth write, no depth test, unlit, so everything else covers it), then the opaque and masked batches (a
//! masked one discards where the diffuse alpha is under its clip value: the diffuse alpha as the opacity mask is the
//! old host's approximation, the graphs being stripped), then every translucent, additive and modulate instance on
//! its own, back to front by its origin, with the depth test and no depth write. Translucent blends by the diffuse
//! alpha, additive adds, modulate multiplies what is behind. A translucent material is drawn only where its opacity
//! has a source the host can read: one with no diffuse is left out of the plan
//! ([`crate::draw::PlanStats::blended_skipped`], which leaves out additive and modulate ones with no diffuse too),
//! one whose diffuse has alpha 255 everywhere (DXT1, say) is left out here
//! ([`UploadStats::translucent_without_alpha`]), and so is any blended one none of whose candidates decodes
//! ([`UploadStats::blended_undecoded`]). Drawn, the first two covered the scene opaquely (Sanctuary's water decals
//! in white, a cloud layer's DXT1 dust as a yellow and black band); their opacity is the stripped graph's.
//! UNVERIFIED: the game draws them, and their look waits for a graph reading. The sky strip is sampled through its
//! own clamped sampler; the other textures repeat.
//!
//! **Baked lighting** (lane H9-LIGHTMAPS). Group 2 binds a batch's light map atlases (coefficients 0 and 1) and its
//! shadow atlas, uploaded linear, with a clamped sampler; vertex buffer 2 holds a per-vertex light map's samples (a
//! zero buffer for every other draw). An instance with a light map takes it as its indirect light instead of the sky
//! term: `coefficient 0 x scale 0 + coefficient 1 x scale 1` (texels or vertex samples, each channel over 255). The
//! sun is then multiplied by the shadow map's visibility: the texel as is for a shadow-factor map, or for a distance
//! field a smooth step of width [`DISTANCE_FIELD_HALF_WIDTH`] around one half. **The decode is a fit, UNVERIFIED**:
//! the coefficient textures' encoding (linear or gamma), how the game combines them and its shadow-map penumbra are
//! not read; only the data path is (decision `2026-11-16-65-*`).
//!
//! **Lighting** ([`Lighting`]) is the packages' when the caller reads them ([`Lighting::from_scene`]): the
//! dominant directional light along its direction, and the sky light as a hemisphere term (its lower colour facing
//! down, its upper colour facing up), the sum scaled by one exposure. Unlit materials take their colour as is.
//!
//! **Targets** are 8-bit RGBA or BGRA, sRGB or not ([`capture_order`]). The shader writes linear colour; on an
//! sRGB target the hardware encodes it, on a plain one it is stored as is (darker). The window's surface asks for an
//! sRGB format when the adapter offers one (`crate::gpu`).
//!
//! **No panic on the device's errors**: they go to the uncaptured-error handler of [`crate::gpu`] in a window run;
//! the headless context of the tests installs its own.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ow_assets::texture::TextureAsset;
use ow_core::DetMap;
use wgpu::util::DeviceExt;

use crate::camera::{Controls, FlyCamera};
use crate::draw::{
    BatchKey, DrawPlan, Instance, LightMode, MaterialSlot, ShadowMode, VERTEX_BYTES, back_to_front,
    pick_from,
};
use crate::frame::Mat4;
use crate::lighting::SceneLights;
use crate::material::{Blend, SKY_HORIZON_ROW};

/// The front face of a draw whose transform does not mirror (module documentation).
pub const FRONT_FACE: wgpu::FrontFace = wgpu::FrontFace::Ccw;
/// The front face of a mirroring draw.
pub const MIRRORED_FRONT_FACE: wgpu::FrontFace = wgpu::FrontFace::Cw;
/// The depth buffer's format.
pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// The clear colour (linear RGBA), a pale sky.
pub const CLEAR: wgpu::Color = wgpu::Color {
    r: 0.45,
    g: 0.6,
    b: 0.8,
    a: 1.0,
};
/// The direction toward the sun (Z-up, normalised in the shader): high, from the south-east.
pub const SUN_DIRECTION: [f32; 3] = [0.35, 0.45, 0.82];
/// Ambient and sun strengths (a viewer's choice, as B2's were).
pub const AMBIENT: f32 = 0.35;
pub const SUN: f32 = 0.75;
/// The grey of an untextured or undecoded draw (sRGB bytes).
pub const GREY: [u8; 4] = [180, 180, 180, 255];
/// Bytes of one instance: the column-major 4x4 world matrix, the normal matrix's three columns (each padded to four
/// floats), then the baked lighting (lane H9): the light map rectangle, the shadow map rectangle, coefficient 0's
/// scale with the light mode, coefficient 1's scale with the shadow mode.
const INSTANCE_BYTES: u64 = 176;
/// Bytes of one per-vertex light map sample in its vertex buffer: two `FColor`s.
const VERTEX_LIGHT_BYTES: u64 = 8;
/// Half the width of the band around one half over which a distance-field shadow map goes from shadowed to lit
/// (a host choice: the game's penumbra scale is not read; UNVERIFIED).
pub const DISTANCE_FIELD_HALF_WIDTH: f32 = 0.05;
/// Bytes of the per-frame uniforms: view-projection, sun direction, sun, sky upper, sky lower, exposure.
const GLOBALS_BYTES: u64 = 144;

const SHADER: &str = r#"
struct Globals {
    view_proj: mat4x4<f32>,
    sun_dir: vec4<f32>,
    sun: vec4<f32>,
    sky_upper: vec4<f32>,
    sky_lower: vec4<f32>,
    params: vec4<f32>,
};

struct Material {
    color: vec4<f32>,
    // x clip, y unlit, z masked, w alpha mode (0 opaque, 1 translucent, 2 additive, 3 modulate)
    params: vec4<f32>,
    // x strip column, y sky brightness, z cloud brightness, w cloud cap opacity
    sky: vec4<f32>,
    // x sky, y the strip row the clouds take their colour from
    flags: vec4<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(1) @binding(0) var colour: texture_2d<f32>;
@group(1) @binding(1) var colour_sampler: sampler;
@group(1) @binding(2) var data: texture_2d<f32>;
@group(1) @binding(3) var<uniform> material: Material;
// The sky strip's own sampler: clamped, so a column or row at the strip's edge does not blend with the other edge.
@group(1) @binding(4) var strip_sampler: sampler;
@group(2) @binding(0) var light_map_0: texture_2d<f32>;
@group(2) @binding(1) var light_map_1: texture_2d<f32>;
@group(2) @binding(2) var shadow_map: texture_2d<f32>;
@group(2) @binding(3) var light_sampler: sampler;

const DISTANCE_FIELD_HALF_WIDTH: f32 = 0.05;

struct VertexIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) m0: vec4<f32>,
    @location(4) m1: vec4<f32>,
    @location(5) m2: vec4<f32>,
    @location(6) m3: vec4<f32>,
    @location(7) n0: vec4<f32>,
    @location(8) n1: vec4<f32>,
    @location(9) n2: vec4<f32>,
    @location(10) light_uv: vec2<f32>,
    // Light map rectangle, shadow map rectangle, (scale 0, light mode), (scale 1, shadow mode).
    @location(11) l0: vec4<f32>,
    @location(12) l1: vec4<f32>,
    @location(13) l2: vec4<f32>,
    @location(14) l3: vec4<f32>,
    // A per-vertex light map's two samples, B G R A bytes each (one attribute: the default limit is 16).
    @location(15) samples: vec2<u32>,
};

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) light_uv: vec2<f32>,
    @location(3) @interpolate(flat) l0: vec4<f32>,
    @location(4) @interpolate(flat) l1: vec4<f32>,
    @location(5) @interpolate(flat) l2: vec4<f32>,
    @location(6) @interpolate(flat) l3: vec4<f32>,
    @location(7) s0: vec3<f32>,
    @location(8) s1: vec3<f32>,
};

@vertex
fn vs_main(v: VertexIn) -> VertexOut {
    let model = mat4x4<f32>(v.m0, v.m1, v.m2, v.m3);
    var out: VertexOut;
    out.clip = globals.view_proj * (model * vec4<f32>(v.position, 1.0));
    let normal_matrix = mat3x3<f32>(v.n0.xyz, v.n1.xyz, v.n2.xyz);
    out.normal = normal_matrix * v.normal;
    out.uv = v.uv;
    out.light_uv = v.light_uv;
    out.l0 = v.l0;
    out.l1 = v.l1;
    out.l2 = v.l2;
    out.l3 = v.l3;
    out.s0 = unpack4x8unorm(v.samples.x).zyx;
    out.s1 = unpack4x8unorm(v.samples.y).zyx;
    return out;
}

@fragment
fn fs_main(in: VertexOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    // Every sample first, in uniform control flow.
    let texel = textureSample(colour, colour_sampler, in.uv);
    let strip_sky = textureSample(colour, strip_sampler, vec2<f32>(material.sky.x, in.uv.y));
    let strip_cloud = textureSample(colour, strip_sampler, vec2<f32>(material.sky.x, material.flags.y));
    let cloud = textureSample(data, colour_sampler, in.uv).r;
    let coefficient_0 = textureSample(light_map_0, light_sampler, in.light_uv * in.l0.xy + in.l0.zw).rgb;
    let coefficient_1 = textureSample(light_map_1, light_sampler, in.light_uv * in.l0.xy + in.l0.zw).rgb;
    let shadow_texel = textureSample(shadow_map, light_sampler, in.light_uv * in.l1.xy + in.l1.zw).r;
    if (material.flags.x > 0.5) {
        // The sky record's formula (NATIVE_SKY_APPROXIMATION.md): the strip at the time-of-day column and the dome's
        // V, the clouds' colour from the strip's horizon row, mixed by the cloud texture's R times the cap opacity.
        let sky = strip_sky.rgb * material.sky.y;
        let cloud_colour = strip_cloud.rgb * material.sky.y * material.sky.z;
        let coverage = clamp(cloud * material.sky.w, 0.0, 1.0);
        return vec4<f32>(mix(sky, cloud_colour, coverage), 1.0);
    }
    let base = texel * material.color;
    if (material.params.z > 0.5 && base.a < material.params.x) {
        discard;
    }
    var light = vec3<f32>(1.0, 1.0, 1.0);
    if (material.params.y < 0.5) {
        var n = in.normal;
        if (!front) {
            n = -n;
        }
        let len = length(n);
        var unit = vec3<f32>(0.0, 0.0, 1.0);
        var sun = vec3<f32>(0.0, 0.0, 0.0);
        if (len > 0.0) {
            unit = n / len;
            sun = globals.sun.rgb * max(dot(unit, normalize(globals.sun_dir.xyz)), 0.0);
        }
        var indirect = mix(globals.sky_lower.rgb, globals.sky_upper.rgb, unit.z * 0.5 + 0.5);
        if (in.l2.w > 1.5) {
            indirect = coefficient_0 * in.l2.xyz + coefficient_1 * in.l3.xyz;
        } else if (in.l2.w > 0.5) {
            indirect = in.s0 * in.l2.xyz + in.s1 * in.l3.xyz;
        }
        var visible = 1.0;
        if (in.l3.w > 1.5) {
            visible = smoothstep(0.5 - DISTANCE_FIELD_HALF_WIDTH, 0.5 + DISTANCE_FIELD_HALF_WIDTH, shadow_texel);
        } else if (in.l3.w > 0.5) {
            visible = shadow_texel;
        }
        light = (indirect + sun * visible) * globals.params.x;
    }
    let rgb = base.rgb * light;
    if (material.params.w > 0.5 && material.params.w < 1.5) {
        return vec4<f32>(rgb, base.a);
    }
    return vec4<f32>(rgb, 1.0);
}
"#;

/// The 16 floats of a matrix, column by column.
fn matrix_bytes(m: &Mat4, out: &mut Vec<u8>) {
    for v in m.cols.iter().flatten() {
        out.extend_from_slice(&v.to_le_bytes());
    }
}

/// One instance as the instance buffer holds it ([`INSTANCE_BYTES`]).
/// `light`/`shadow` false drop the instance's light map or shadow map (their textures did not decode).
fn instance_bytes(i: &Instance, light: bool, shadow: bool, out: &mut Vec<u8>) {
    matrix_bytes(&i.world, out);
    for col in &i.normal {
        for v in col.iter().chain(&[0.0]) {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
    let l = &i.light;
    let mode = match l.mode {
        LightMode::Texture if light => 2.0,
        LightMode::Vertex if light => 1.0,
        _ => 0.0f32,
    };
    let shadowed = match l.shadow {
        ShadowMode::DistanceField if shadow => 2.0,
        ShadowMode::Factor if shadow => 1.0,
        _ => 0.0f32,
    };
    let (mode, shadowed) = ([mode], [shadowed]);
    let values = l
        .lightmap_rect
        .iter()
        .chain(&l.shadow_rect)
        .chain(&l.scales[0])
        .chain(&mode)
        .chain(&l.scales[1])
        .chain(&shadowed);
    for v in values {
        out.extend_from_slice(&v.to_le_bytes());
    }
}

/// Whether captured bytes of `format` come in RGBA order (`Some(false)`), BGRA order (`Some(true)`), or are a format
/// the capture does not convert (`None`).
pub fn capture_order(format: wgpu::TextureFormat) -> Option<bool> {
    match format {
        wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb => Some(false),
        wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb => Some(true),
        _ => None,
    }
}

/// The per-frame uniforms for `view_proj` and `lighting`.
fn globals_bytes(view_proj: &Mat4, lighting: &Lighting) -> Vec<u8> {
    let mut out = Vec::with_capacity(GLOBALS_BYTES as usize);
    matrix_bytes(view_proj, &mut out);
    for v3 in [
        lighting.sun_direction,
        lighting.sun,
        lighting.sky_upper,
        lighting.sky_lower,
        [lighting.exposure, 0.0, 0.0],
    ] {
        for v in v3.iter().chain(&[0.0]) {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
    out
}

/// The mips of `texture` the GPU can take as one chain: the first resident mip and each following one that halves
/// it (to at least 1), as long as the sizes fit `max_dimension`.
fn mip_chain(texture: &TextureAsset, max_dimension: u32) -> Vec<usize> {
    let mut chain = Vec::new();
    let mut expect: Option<(u32, u32)> = None;
    for (i, mip) in texture.mips.iter().enumerate() {
        let size = (mip.width, mip.height);
        match expect {
            None if size.0 <= max_dimension && size.1 <= max_dimension => {}
            None => continue,
            Some(e) if e == size => {}
            Some(_) => break,
        }
        chain.push(i);
        if size == (1, 1) {
            break;
        }
        expect = Some(((size.0 / 2).max(1), (size.1 / 2).max(1)));
    }
    chain
}

/// Where the renderer gets a decoded texture by object path.
pub type TextureSource = Box<dyn FnMut(&str) -> Result<TextureAsset, String>>;

/// What the upload counted.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UploadStats {
    pub meshes: usize,
    pub vertex_bytes: u64,
    pub index_bytes: u64,
    pub instance_bytes: u64,
    /// Textures decoded and uploaded (a texture uploaded both as colour and as data counts twice).
    pub textures: usize,
    /// Candidate textures that did not decode, or had no mip the device takes (each skipped for the next).
    pub textures_failed: usize,
    pub texture_bytes: u64,
    /// Materials drawn with a later candidate because a better one failed.
    pub fallbacks: usize,
    /// Materials with candidates of which none decoded, drawn grey.
    pub grey_materials: usize,
    /// Material slots uploaded.
    pub slots: usize,
    /// Sky batches drawn with both their textures (the strip and the clouds); a sky batch missing one is drawn
    /// with grey in its place and not counted.
    pub sky_batches: usize,
    /// Translucent slots not drawn because their diffuse has no alpha below 255 (a DXT1 texture, say): the opacity
    /// comes from the stripped graph, and drawing the diffuse's alpha would draw the whole surface opaque.
    pub translucent_without_alpha: usize,
    /// Blended slots (translucent, additive, modulate) with diffuse candidates of which none decoded, not drawn:
    /// grey in their place would cover, veil or darken what is behind (they are in `grey_materials` too).
    pub blended_undecoded: usize,
    /// Section instances of the slots of `translucent_without_alpha` and `blended_undecoded`, not drawn.
    pub skipped_instances: usize,
    /// Lighting slots uploaded (lane H9), and those whose light map or shadow map textures did not all decode (their
    /// instances are drawn without the part that failed).
    pub lightings: usize,
    pub lighting_failed: usize,
    /// Bytes of per-vertex light map samples uploaded.
    pub vertex_light_bytes: u64,
}

/// The scene's light as the shader takes it.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lighting {
    /// The direction toward the sun (normalised in the shader).
    pub sun_direction: [f32; 3],
    /// Linear colour times brightness.
    pub sun: [f32; 3],
    /// The sky light's upper and lower hemispheres, linear colour times brightness.
    pub sky_upper: [f32; 3],
    pub sky_lower: [f32; 3],
    /// What the lit sum is multiplied by.
    pub exposure: f32,
}

impl Lighting {
    /// Stage 2's viewer constants: [`SUN_DIRECTION`], [`SUN`] and [`AMBIENT`] in both hemispheres, exposure 1.
    pub fn viewer() -> Lighting {
        Lighting {
            sun_direction: SUN_DIRECTION,
            sun: [SUN; 3],
            sky_upper: [AMBIENT; 3],
            sky_lower: [AMBIENT; 3],
            exposure: 1.0,
        }
    }

    /// From the packages' lights: the sun's colour times brightness along its direction, the sky light's two
    /// hemispheres. The exposure maps the brightest sum (the sun at full incidence plus the upper sky) to 1: a viewer
    /// choice, since how BL2 exposes and tonemaps them, and the lightmaps that carry its indirect light, are not read.
    /// [`Lighting::viewer`] when the scene has no directional light.
    pub fn from_scene(lights: &SceneLights) -> Lighting {
        let Some(sun) = &lights.sun else {
            return Lighting::viewer();
        };
        let scale = |c: [f32; 3], b: f32| c.map(|v| v * b);
        let (upper, lower) = match &lights.sky {
            Some(s) => (
                scale(s.color, s.brightness),
                scale(s.lower_color, s.lower_brightness),
            ),
            None => ([0.0; 3], [0.0; 3]),
        };
        let sun_light = scale(sun.color, sun.brightness);
        let peak = (0..3)
            .map(|k| sun_light[k] + upper[k])
            .fold(0.0f32, f32::max);
        Lighting {
            sun_direction: sun.toward(),
            sun: sun_light,
            sky_upper: upper,
            sky_lower: lower,
            exposure: if peak > 0.0 && peak.is_finite() {
                1.0 / peak
            } else {
                1.0
            },
        }
    }
}

/// How a batch is drawn, in frame order: the sky first, then opaque and masked with depth writes, then the blended
/// ones back to front.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Pass {
    Sky,
    Opaque,
    Translucent,
    Additive,
    Modulate,
}

impl Pass {
    fn of(slot: Option<&MaterialSlot>) -> Pass {
        match slot {
            Some(s) if s.sky.is_some() => Pass::Sky,
            Some(s) => match s.blend {
                Blend::Translucent => Pass::Translucent,
                Blend::Additive => Pass::Additive,
                Blend::Modulate => Pass::Modulate,
                _ => Pass::Opaque,
            },
            None => Pass::Opaque,
        }
    }

    fn blended(self) -> bool {
        matches!(self, Pass::Translucent | Pass::Additive | Pass::Modulate)
    }

    /// The shader's alpha mode: 0 opaque, 1 translucent (alpha kept), 2 additive, 3 modulate.
    fn alpha_mode(self) -> f32 {
        match self {
            Pass::Translucent => 1.0,
            Pass::Additive => 2.0,
            Pass::Modulate => 3.0,
            _ => 0.0,
        }
    }
}

/// Bytes of one material uniform: colour, parameters, sky inputs, flags.
const MATERIAL_BYTES: u64 = 64;

/// A slot's uniform (the shader's `Material`).
fn material_bytes(slot: Option<&MaterialSlot>, pass: Pass) -> Vec<u8> {
    let mut v = [0f32; (MATERIAL_BYTES / 4) as usize];
    let color = slot.and_then(|s| s.constant).unwrap_or([1.0, 1.0, 1.0]);
    v[..3].copy_from_slice(&color);
    v[3] = 1.0;
    if let Some(s) = slot {
        v[4] = s.clip;
        v[5] = f32::from(u8::from(s.unlit || s.sky.is_some()));
        v[6] = f32::from(u8::from(s.blend == Blend::Masked));
        v[7] = pass.alpha_mode();
        if let Some(sky) = &s.sky {
            v[8] = sky.column();
            v[9] = sky.sky_brightness;
            v[10] = sky.cloud_brightness;
            v[11] = sky.cloud_cap_opacity;
            v[12] = 1.0;
            v[13] = SKY_HORIZON_ROW;
        }
    }
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// What a frame's render pass has bound so far: a draw sets only what differs.
#[derive(Default)]
struct Bound {
    pipeline: Option<(Pass, bool, bool)>,
    material: Option<Option<u32>>,
    lighting: Option<Option<u32>>,
    mesh: Option<u32>,
}

struct GpuMesh {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
}

struct GpuBatch {
    key: BatchKey,
    pass: Pass,
    first_index: u32,
    index_count: u32,
    instance_offset: u64,
    instances: u32,
    /// Each instance's origin, for the back-to-front order of blended batches.
    origins: Vec<[f32; 3]>,
}

/// The uploaded scene and the pipelines that draw it.
pub struct SceneRenderer {
    format: wgpu::TextureFormat,
    pipelines: DetMap<(Pass, bool, bool), wgpu::RenderPipeline>,
    globals: wgpu::Buffer,
    globals_group: wgpu::BindGroup,
    lighting: Lighting,
    material_groups: Vec<wgpu::BindGroup>,
    default_group: wgpu::BindGroup,
    lighting_groups: Vec<wgpu::BindGroup>,
    default_lighting: wgpu::BindGroup,
    /// By lighting slot: a per-vertex light map's samples.
    vertex_lights: Vec<Option<wgpu::Buffer>>,
    zero_light: wgpu::Buffer,
    meshes: Vec<GpuMesh>,
    instances: Option<wgpu::Buffer>,
    batches: Vec<GpuBatch>,
    depth: Option<(wgpu::TextureView, u32, u32)>,
    pub upload: UploadStats,
}

/// Texture views by (object path, sRGB), each decoded once, with whether every texel of the largest uploaded level
/// has alpha 255: `None` when it failed.
type Uploaded = DetMap<(String, bool), Option<(wgpu::TextureView, bool)>>;

impl SceneRenderer {
    /// Uploads `plan` for drawing into `format` targets lit by `lighting`, decoding its textures through `textures`.
    /// `Err` for a target format the renderer does not draw into ([`capture_order`]), a buffer larger than the device
    /// allows, or more instances in a batch than a draw can count.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        plan: &DrawPlan,
        lighting: Lighting,
        textures: &mut dyn FnMut(&str) -> Result<TextureAsset, String>,
    ) -> Result<SceneRenderer, String> {
        if capture_order(format).is_none() {
            return Err(format!(
                "the renderer draws into 8-bit RGBA or BGRA targets, not {format:?}"
            ));
        }
        let max_buffer = device.limits().max_buffer_size;
        let fits = |what: &str, bytes: u64| {
            if bytes > max_buffer {
                Err(format!(
                    "{what} of {bytes} bytes exceed the device's {max_buffer}-byte buffers"
                ))
            } else {
                Ok(())
            }
        };
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("scene shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let uniform = |binding, visibility| wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let texture_entry = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("globals"),
            entries: &[uniform(0, wgpu::ShaderStages::VERTEX_FRAGMENT)],
        });
        let material_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("material"),
            entries: &[
                texture_entry(0),
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                texture_entry(2),
                uniform(3, wgpu::ShaderStages::FRAGMENT),
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let lighting_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("lighting"),
            entries: &[
                texture_entry(0),
                texture_entry(1),
                texture_entry(2),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("scene"),
            bind_group_layouts: &[
                Some(&globals_layout),
                Some(&material_layout),
                Some(&lighting_layout),
            ],
            immediate_size: 0,
        });
        let pipeline = |pass: Pass, two_sided: bool, mirrored: bool| {
            let front_face = if mirrored {
                MIRRORED_FRONT_FACE
            } else {
                FRONT_FACE
            };
            let cull_mode = (!two_sided && pass != Pass::Sky).then_some(wgpu::Face::Back);
            let blend = match pass {
                Pass::Sky | Pass::Opaque => wgpu::BlendState::REPLACE,
                Pass::Translucent => wgpu::BlendState::ALPHA_BLENDING,
                Pass::Additive => wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::One,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent::OVER,
                },
                Pass::Modulate => wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::Dst,
                        dst_factor: wgpu::BlendFactor::Zero,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent::OVER,
                },
            };
            let (write, compare) = match pass {
                Pass::Opaque => (true, wgpu::CompareFunction::Greater),
                Pass::Sky => (false, wgpu::CompareFunction::Always),
                _ => (false, wgpu::CompareFunction::Greater),
            };
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("scene"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[
                        Some(wgpu::VertexBufferLayout {
                            array_stride: VERTEX_BYTES as u64,
                            step_mode: wgpu::VertexStepMode::Vertex,
                            attributes: &wgpu::vertex_attr_array![
                                0 => Float32x3, 1 => Float32x3, 2 => Float32x2, 10 => Float32x2
                            ],
                        }),
                        Some(wgpu::VertexBufferLayout {
                            array_stride: INSTANCE_BYTES,
                            step_mode: wgpu::VertexStepMode::Instance,
                            attributes: &wgpu::vertex_attr_array![
                                3 => Float32x4, 4 => Float32x4, 5 => Float32x4, 6 => Float32x4,
                                7 => Float32x4, 8 => Float32x4, 9 => Float32x4,
                                11 => Float32x4, 12 => Float32x4, 13 => Float32x4, 14 => Float32x4
                            ],
                        }),
                        Some(wgpu::VertexBufferLayout {
                            array_stride: VERTEX_LIGHT_BYTES,
                            step_mode: wgpu::VertexStepMode::Vertex,
                            attributes: &wgpu::vertex_attr_array![15 => Uint32x2],
                        }),
                    ],
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    front_face,
                    cull_mode,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(write),
                    depth_compare: Some(compare),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: Some(blend),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };

        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: GLOBALS_BYTES,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let globals_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("globals"),
            layout: &globals_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals.as_entire_binding(),
            }],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("material"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            anisotropy_clamp: 8,
            ..Default::default()
        });
        let strip_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("sky strip"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let group = |colour: &wgpu::TextureView, data: &wgpu::TextureView, params: &[u8]| {
            let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("material"),
                contents: params,
                usage: wgpu::BufferUsages::UNIFORM,
            });
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("material"),
                layout: &material_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(colour),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(data),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::Sampler(&strip_sampler),
                    },
                ],
            })
        };

        let mut upload = UploadStats::default();
        let grey =
            upload_rgba(device, queue, &[(1, 1, &GREY[..])], true).create_view(&Default::default());
        let white = upload_rgba(device, queue, &[(1, 1, &[255, 255, 255, 255][..])], true)
            .create_view(&Default::default());
        let default_group = group(&grey, &grey, &material_bytes(None, Pass::Opaque));
        let max_dimension = device.limits().max_texture_dimension_2d;
        let mut uploaded: Uploaded = DetMap::new();
        // Decodes and uploads `path` once per colour space; `None` when it fails.
        let mut fetch = |path: &str,
                         srgb: bool,
                         upload: &mut UploadStats|
         -> Option<(wgpu::TextureView, bool)> {
            let key = (path.to_string(), srgb);
            if let Some(view) = uploaded.get(&key) {
                return view.clone();
            }
            let decoded = textures(path).ok();
            let chain = decoded
                .as_ref()
                .map(|t| mip_chain(t, max_dimension))
                .unwrap_or_default();
            let view = match (&decoded, chain.is_empty()) {
                (Some(t), false) => {
                    let levels: Vec<(u32, u32, &[u8])> = chain
                        .iter()
                        .map(|&i| (t.mips[i].width, t.mips[i].height, t.mips[i].rgba.as_slice()))
                        .collect();
                    upload.texture_bytes += levels.iter().map(|l| l.2.len() as u64).sum::<u64>();
                    upload.textures += 1;
                    let opaque = levels
                        .first()
                        .is_some_and(|l| l.2.chunks_exact(4).all(|p| p[3] == 255));
                    Some((
                        upload_rgba(device, queue, &levels, srgb).create_view(&Default::default()),
                        opaque,
                    ))
                }
                _ => {
                    upload.textures_failed += 1;
                    None
                }
            };
            uploaded.insert(key, view.clone());
            view
        };
        let mut material_groups = Vec::with_capacity(plan.slots.len());
        let mut sky_complete = Vec::with_capacity(plan.slots.len());
        let mut skipped = vec![false; plan.slots.len()];
        for (index, slot) in plan.slots.iter().enumerate() {
            let pass = Pass::of(Some(slot));
            let params = material_bytes(Some(slot), pass);
            if let Some(sky) = &slot.sky {
                // The strip is colour (sRGB), the clouds are data (linear), as the sky record samples them.
                let strip = fetch(&sky.strip, true, &mut upload);
                let clouds = fetch(&sky.clouds, false, &mut upload);
                sky_complete.push(strip.is_some() && clouds.is_some());
                let strip = strip.map_or_else(|| grey.clone(), |s| s.0);
                let clouds = clouds.map_or_else(|| grey.clone(), |c| c.0);
                material_groups.push(group(&strip, &clouds, &params));
                continue;
            }
            sky_complete.push(false);
            if slot.constant.is_some() {
                material_groups.push(group(&white, &grey, &params));
                continue;
            }
            if slot.diffuse.is_empty() {
                material_groups.push(group(&grey, &grey, &params));
                continue;
            }
            let best = pick_from(&slot.diffuse, &|_| true).map(|c| c.path.clone());
            let mut failed: Vec<String> = Vec::new();
            let chosen = loop {
                let Some(pick) = pick_from(&slot.diffuse, &|p| !failed.iter().any(|f| f == p))
                else {
                    break None;
                };
                let path = pick.path.clone();
                match fetch(&path, true, &mut upload) {
                    Some((view, opaque)) => break Some((path, view, opaque)),
                    None => failed.push(path),
                }
            };
            match chosen {
                Some((path, view, opaque)) => {
                    if best.as_deref() != Some(path.as_str()) {
                        upload.fallbacks += 1;
                    }
                    if opaque && pass == Pass::Translucent {
                        upload.translucent_without_alpha += 1;
                        if let Some(s) = skipped.get_mut(index) {
                            *s = true;
                        }
                    }
                    material_groups.push(group(&view, &grey, &params));
                }
                None => {
                    upload.grey_materials += 1;
                    if pass.blended() {
                        upload.blended_undecoded += 1;
                        if let Some(s) = skipped.get_mut(index) {
                            *s = true;
                        }
                    }
                    material_groups.push(group(&grey, &grey, &params));
                }
            }
        }
        upload.slots = material_groups.len();

        // Lighting (lane H9): one group per lighting slot, the atlases linear; a per-vertex slot's samples in a
        // vertex buffer of its own, every other draw reading a zero buffer as long as the longest mesh.
        let light_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("lighting"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let black = upload_rgba(device, queue, &[(1, 1, &[0, 0, 0, 255][..])], false)
            .create_view(&Default::default());
        let lit = upload_rgba(device, queue, &[(1, 1, &[255, 255, 255, 255][..])], false)
            .create_view(&Default::default());
        let lighting_group =
            |c0: &wgpu::TextureView, c1: &wgpu::TextureView, shadow: &wgpu::TextureView| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("lighting"),
                    layout: &lighting_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(c0),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(c1),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::TextureView(shadow),
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: wgpu::BindingResource::Sampler(&light_sampler),
                        },
                    ],
                })
            };
        let default_lighting = lighting_group(&black, &black, &lit);
        let mut lighting_groups = Vec::with_capacity(plan.lightings.len());
        // Per lighting slot: whether its light map and its shadow map are usable.
        let mut lighting_ok = Vec::with_capacity(plan.lightings.len());
        let mut vertex_lights = Vec::with_capacity(plan.lightings.len());
        for l in &plan.lightings {
            let mut fetch_linear = |key: &Option<String>| match key {
                Some(k) => fetch(k, false, &mut upload)
                    .map(|(view, _)| Some(view))
                    .ok_or(()),
                None => Ok(None),
            };
            let c0 = fetch_linear(&l.textures[0]);
            let c1 = fetch_linear(&l.textures[1]);
            let shadow = fetch_linear(&l.shadow);
            let light_ok = c0.is_ok() && c1.is_ok();
            let shadow_ok = shadow.is_ok();
            if !light_ok || !shadow_ok {
                upload.lighting_failed += 1;
            }
            let c0 = c0.ok().flatten().unwrap_or_else(|| black.clone());
            let c1 = c1.ok().flatten().unwrap_or_else(|| black.clone());
            let shadow = shadow.ok().flatten().unwrap_or_else(|| lit.clone());
            lighting_groups.push(lighting_group(&c0, &c1, &shadow));
            lighting_ok.push((light_ok, shadow_ok));
            vertex_lights.push(l.samples.as_ref().map(|samples| {
                let bytes: Vec<u8> = samples.iter().flatten().copied().collect();
                upload.vertex_light_bytes += bytes.len() as u64;
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("vertex light"),
                    contents: &bytes,
                    usage: wgpu::BufferUsages::VERTEX,
                })
            }));
        }
        upload.lightings = lighting_groups.len();
        let longest = plan
            .meshes
            .iter()
            .map(|m| m.vertices.len())
            .max()
            .unwrap_or(1)
            .max(1);
        let zero_bytes = (longest as u64)
            .checked_mul(VERTEX_LIGHT_BYTES)
            .ok_or("the zero light buffer's size overflows")?;
        fits("the zero light buffer", zero_bytes)?;
        let zero_light = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("no vertex light"),
            size: zero_bytes,
            usage: wgpu::BufferUsages::VERTEX,
            mapped_at_creation: false,
        });

        let mut meshes = Vec::with_capacity(plan.meshes.len());
        for m in &plan.meshes {
            let (vertices, indices) = (m.vertex_bytes(), m.index_bytes());
            fits("vertices", vertices.len() as u64)?;
            fits("indices", indices.len() as u64)?;
            upload.vertex_bytes += vertices.len() as u64;
            upload.index_bytes += indices.len() as u64;
            meshes.push(GpuMesh {
                vertices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("vertices"),
                    contents: &vertices,
                    usage: wgpu::BufferUsages::VERTEX,
                }),
                indices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("indices"),
                    contents: &indices,
                    usage: wgpu::BufferUsages::INDEX,
                }),
            });
        }
        upload.meshes = meshes.len();

        let mut instance_data = Vec::new();
        let mut batches = Vec::with_capacity(plan.batches.len());
        let mut pipelines = DetMap::new();
        for b in &plan.batches {
            let Some(range) = plan
                .meshes
                .get(b.key.mesh as usize)
                .and_then(|m| m.sections.get(b.key.section as usize))
            else {
                continue;
            };
            let slot = b.key.material.and_then(|m| plan.slots.get(m as usize));
            if b.key
                .material
                .and_then(|m| skipped.get(m as usize))
                .copied()
                .unwrap_or(false)
            {
                upload.skipped_instances += b.instances.len();
                continue;
            }
            let pass = Pass::of(slot);
            if pass == Pass::Sky
                && b.key
                    .material
                    .and_then(|m| sky_complete.get(m as usize))
                    .copied()
                    .unwrap_or(false)
            {
                upload.sky_batches += 1;
            }
            let pipeline_key = (pass, b.key.two_sided, b.key.mirrored);
            pipelines
                .entry(pipeline_key)
                .or_insert_with(|| pipeline(pass, b.key.two_sided, b.key.mirrored));
            let instance_offset = instance_data.len() as u64;
            let (light_ok, shadow_ok) = b
                .key
                .lighting
                .and_then(|l| lighting_ok.get(l as usize))
                .copied()
                .unwrap_or((false, false));
            for i in &b.instances {
                instance_bytes(i, light_ok, shadow_ok, &mut instance_data);
            }
            batches.push(GpuBatch {
                key: b.key,
                pass,
                first_index: range.first_index,
                index_count: range.index_count,
                instance_offset,
                instances: u32::try_from(b.instances.len())
                    .map_err(|_| format!("a batch of {} instances", b.instances.len()))?,
                origins: b
                    .instances
                    .iter()
                    .map(|i| [i.world.cols[3][0], i.world.cols[3][1], i.world.cols[3][2]])
                    .collect(),
            });
        }
        // Frame order: sky, opaque (and masked), then the blended passes.
        batches.sort_by_key(|b| b.pass);
        upload.instance_bytes = instance_data.len() as u64;
        fits("instances", upload.instance_bytes)?;
        let instances = (!instance_data.is_empty()).then(|| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("instances"),
                contents: &instance_data,
                usage: wgpu::BufferUsages::VERTEX,
            })
        });

        Ok(SceneRenderer {
            format,
            pipelines,
            globals,
            globals_group,
            lighting,
            material_groups,
            default_group,
            lighting_groups,
            default_lighting,
            vertex_lights,
            zero_light,
            meshes,
            instances,
            batches,
            depth: None,
            upload,
        })
    }

    /// The number of instanced draws a frame issues for opaque, masked and sky batches; blended batches are drawn
    /// one instance at a time ([`SceneRenderer::blended_instances`]).
    pub fn draw_calls(&self) -> usize {
        self.batches.iter().filter(|b| !b.pass.blended()).count()
    }

    /// Blended instances, each its own draw in back-to-front order.
    pub fn blended_instances(&self) -> usize {
        self.batches
            .iter()
            .filter(|b| b.pass.blended())
            .map(|b| b.instances as usize)
            .sum()
    }

    /// The depth buffer for a `width` x `height` target, made on first use and on a size change.
    fn depth_view(&mut self, device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
        if let Some((view, w, h)) = &self.depth
            && (*w, *h) == (width, height)
        {
            return view.clone();
        }
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("depth"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        self.depth = Some((view.clone(), width, height));
        view
    }

    /// Encodes one frame into `target` (`width` x `height`, the renderer's format) seen through `view_proj` from
    /// `eye` (uu): the sky, the opaque and masked batches, then every blended instance back to front.
    #[allow(clippy::too_many_arguments)]
    pub fn encode(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        view_proj: &Mat4,
        eye: [f32; 3],
    ) {
        self.encode_with(device, queue, encoder, target, size, view_proj, eye, &mut |_| {});
    }

    /// [`SceneRenderer::encode`] with `world` called inside the scene pass, after the opaque and masked batches and
    /// before the blended ones (lane H-PAWNS: skinned pawns drawn as world objects, depth-tested against the scene
    /// and under its translucency). `world` may set any pipeline, bind group and buffer whose layout fits the pass
    /// (this format, [`DEPTH_FORMAT`], one sample); the scene's own bindings are set again after it.
    #[allow(clippy::too_many_arguments)]
    pub fn encode_with(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        view_proj: &Mat4,
        eye: [f32; 3],
        world: &mut dyn FnMut(&mut wgpu::RenderPass<'_>),
    ) {
        queue.write_buffer(&self.globals, 0, &globals_bytes(view_proj, &self.lighting));
        let depth = self.depth_view(device, size.0, size.1);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("scene"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(CLEAR),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0.0),
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
        let Some(instances) = &self.instances else {
            world(&mut pass);
            return;
        };
        pass.set_bind_group(0, &self.globals_group, &[]);
        // The instance buffer is bound once and each batch addressed by its first instance; the pipeline, bind
        // groups and mesh buffers are set only when they change from the previous draw (batches are sorted by mesh,
        // then material, then lighting, so consecutive draws often share them).
        pass.set_vertex_buffer(1, instances.slice(..));
        let mut bound = Bound::default();
        let draw = |pass: &mut wgpu::RenderPass<'_>,
                    bound: &mut Bound,
                    b: &GpuBatch,
                    range: std::ops::Range<u32>| {
                let (Some(mesh), Some(pipeline)) = (
                    self.meshes.get(b.key.mesh as usize),
                    self.pipelines
                        .get(&(b.pass, b.key.two_sided, b.key.mirrored)),
                ) else {
                    return;
                };
                let pipeline_key = (b.pass, b.key.two_sided, b.key.mirrored);
                if bound.pipeline != Some(pipeline_key) {
                    pass.set_pipeline(pipeline);
                    bound.pipeline = Some(pipeline_key);
                }
                if bound.material != Some(b.key.material) {
                    let group = b
                        .key
                        .material
                        .and_then(|m| self.material_groups.get(m as usize))
                        .unwrap_or(&self.default_group);
                    pass.set_bind_group(1, group, &[]);
                    bound.material = Some(b.key.material);
                }
                if bound.lighting != Some(b.key.lighting) {
                    let lighting = b.key.lighting.map(|l| l as usize);
                    let light_group = lighting
                        .and_then(|l| self.lighting_groups.get(l))
                        .unwrap_or(&self.default_lighting);
                    pass.set_bind_group(2, light_group, &[]);
                    let vertex_light = lighting
                        .and_then(|l| self.vertex_lights.get(l))
                        .and_then(Option::as_ref)
                        .unwrap_or(&self.zero_light);
                    pass.set_vertex_buffer(2, vertex_light.slice(..));
                    bound.lighting = Some(b.key.lighting);
                }
                if bound.mesh != Some(b.key.mesh) {
                    pass.set_vertex_buffer(0, mesh.vertices.slice(..));
                    pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
                    bound.mesh = Some(b.key.mesh);
                }
                let Ok(base) = u32::try_from(b.instance_offset / INSTANCE_BYTES) else {
                    return;
                };
                let first = b.first_index;
                pass.draw_indexed(
                    first..first + b.index_count,
                    0,
                    base + range.start..base + range.end,
                );
            };
        let mut blended = Vec::new();
        for (i, b) in self.batches.iter().enumerate() {
            if b.pass.blended() {
                for (k, origin) in b.origins.iter().enumerate() {
                    blended.push((i, u32::try_from(k).unwrap_or(u32::MAX), *origin));
                }
            } else {
                draw(&mut pass, &mut bound, b, 0..b.instances);
            }
        }
        // The world objects drawn by the caller (encode_with), then the scene's bindings set again and the bound
        // state forgotten, so the next batch rebinds everything it needs.
        world(&mut pass);
        pass.set_bind_group(0, &self.globals_group, &[]);
        pass.set_vertex_buffer(1, instances.slice(..));
        bound = Bound::default();
        for (i, k) in back_to_front(&blended, eye) {
            if let Some(b) = self.batches.get(i) {
                draw(&mut pass, &mut bound, b, k..k + 1);
            }
        }
    }

    /// Draws one frame into a texture of its own and reads it back: `width` x `height` RGBA bytes, rows top first.
    pub fn capture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        size: (u32, u32),
        view_proj: &Mat4,
        eye: [f32; 3],
    ) -> Result<Vec<u8>, String> {
        let (width, height) = size;
        let bgra = capture_order(self.format)
            .ok_or_else(|| format!("no capture conversion for {:?}", self.format))?;
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("capture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = target.create_view(&Default::default());
        let row = u64::from(width) * 4;
        let align = u64::from(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let padded = row.div_ceil(align) * align;
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("capture readback"),
            size: padded * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("capture"),
        });
        self.encode(device, queue, &mut encoder, &view, size, view_proj, eye);
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(u32::try_from(padded).map_err(|e| e.to_string())?),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        queue.submit([encoder.finish()]);
        let slice = readback.slice(..);
        // wgpu wants a Send callback; a Mutex in an Arc carries the outcome back to this thread.
        let mapped = Arc::new(Mutex::new(None));
        let done = Arc::clone(&mapped);
        slice.map_async(wgpu::MapMode::Read, move |r| {
            if let Ok(mut slot) = done.lock() {
                *slot = Some(r);
            }
        });
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| e.to_string())?;
        let outcome = mapped
            .lock()
            .map_err(|_| "the capture callback panicked")?
            .take();
        match outcome {
            Some(Ok(())) => {}
            Some(Err(e)) => return Err(e.to_string()),
            None => return Err("the capture buffer was not mapped".into()),
        }
        let data = slice.get_mapped_range().map_err(|e| e.to_string())?;
        let mut out = Vec::with_capacity((row * u64::from(height)) as usize);
        for y in 0..u64::from(height) {
            let start = (y * padded) as usize;
            for px in data[start..start + row as usize].chunks_exact(4) {
                if bgra {
                    out.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
                } else {
                    out.extend_from_slice(px);
                }
            }
        }
        drop(data);
        readback.unmap();
        Ok(out)
    }
}

/// An RGBA texture with the given mip levels (largest first), sRGB for colour or linear for data.
fn upload_rgba(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    levels: &[(u32, u32, &[u8])],
    srgb: bool,
) -> wgpu::Texture {
    let (width, height) = levels.first().map_or((1, 1), |l| (l.0, l.1));
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("diffuse"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: u32::try_from(levels.len().max(1)).unwrap_or(1),
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: if srgb {
            wgpu::TextureFormat::Rgba8UnormSrgb
        } else {
            wgpu::TextureFormat::Rgba8Unorm
        },
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for (level, (w, h, rgba)) in levels.iter().enumerate() {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: u32::try_from(level).unwrap_or(0),
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w * 4),
                rows_per_image: Some(*h),
            },
            wgpu::Extent3d {
                width: *w,
                height: *h,
                depth_or_array_layers: 1,
            },
        );
    }
    texture
}

// ----------------------------------------------------------------------------------------------- the window hook

/// What a renderer plugged into the frame loop of [`crate::app`] does. The loop calls [`Drawer::attach`] once the
/// device exists, [`Drawer::encode`] for each frame instead of its clear, and passes window and mouse events on.
pub trait Drawer {
    /// The window's inner size in physical pixels, when the drawer wants one (a timing run at 1920 x 1080).
    fn window_size(&self) -> Option<(u32, u32)> {
        None
    }
    fn attach(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
    ) -> Result<(), String>;
    fn window_event(&mut self, event: &winit::event::WindowEvent);
    /// The simulation's snapshot of this frame, before [`Drawer::encode`] (lane H4-PLAY: the play mode's view comes
    /// this way). The default ignores it.
    fn snapshot(&mut self, _snapshot: &crate::sim::Snapshot) {}
    /// Raw mouse motion in pixels (right and down positive).
    fn mouse_motion(&mut self, dx: f64, dy: f64);
    /// Encodes the frame into `target` of `size`, `dt` after the last one.
    fn encode(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        dt: Duration,
    ) -> Result<(), String>;
    /// After each presented frame.
    fn presented(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) -> Result<(), String>;
}

/// What a [`SceneDrawer`] measured, shared with the caller through an `Rc`.
#[non_exhaustive]
#[derive(Debug, Clone, Default)]
pub struct DrawReport {
    /// From `attach` start to the end of the upload.
    pub upload_time: Duration,
    pub upload: UploadStats,
    pub draw_calls: usize,
    /// Blended instances, each drawn on its own in back-to-front order.
    pub blended_instances: usize,
    /// Intervals between successive presents.
    pub intervals: Vec<Duration>,
    /// Where a screenshot was written, or why it was not.
    pub screenshot: Option<Result<PathBuf, String>>,
    pub size: Option<(u32, u32)>,
}

impl DrawReport {
    /// The `q`-quantile (0..=1) of the intervals by the nearest-rank rule, or `None` with no interval.
    pub fn quantile(&self, q: f64) -> Option<Duration> {
        let mut sorted = self.intervals.clone();
        sorted.sort_unstable();
        if sorted.is_empty() || !(0.0..=1.0).contains(&q) {
            return None;
        }
        let rank = ow_core::num::trunc_i32_f64(q * sorted.len() as f64 + 0.999_999);
        let index = usize::try_from(rank).unwrap_or(0).clamp(1, sorted.len()) - 1;
        sorted.get(index).copied()
    }
}

/// A [`DrawPlan`] in the window, seen by a [`FlyCamera`].
pub struct SceneDrawer {
    plan: Option<DrawPlan>,
    lighting: Lighting,
    textures: TextureSource,
    renderer: Option<SceneRenderer>,
    camera: FlyCamera,
    controls: Controls,
    size: Option<(u32, u32)>,
    /// The window size asked for.
    window: Option<(u32, u32)>,
    /// Write a screenshot after this many presented frames, to this file.
    screenshot: Option<(u64, PathBuf)>,
    presented: u64,
    last_present: Option<Instant>,
    report: Rc<RefCell<DrawReport>>,
}

impl SceneDrawer {
    pub fn new(
        plan: DrawPlan,
        lighting: Lighting,
        textures: TextureSource,
        camera: FlyCamera,
        window: Option<(u32, u32)>,
        screenshot: Option<(u64, PathBuf)>,
        report: Rc<RefCell<DrawReport>>,
    ) -> SceneDrawer {
        SceneDrawer {
            plan: Some(plan),
            lighting,
            textures,
            renderer: None,
            camera,
            controls: Controls::default(),
            size: None,
            window,
            screenshot,
            presented: 0,
            last_present: None,
            report,
        }
    }

    fn key(&mut self, code: winit::keyboard::KeyCode, down: bool) {
        use winit::keyboard::KeyCode;
        let c = &mut self.controls;
        match code {
            KeyCode::KeyW => c.forward = down,
            KeyCode::KeyS => c.back = down,
            KeyCode::KeyA => c.left = down,
            KeyCode::KeyD => c.right = down,
            KeyCode::KeyQ => c.down = down,
            KeyCode::KeyE => c.up = down,
            KeyCode::ShiftLeft | KeyCode::ShiftRight => c.fast = down,
            _ => {}
        }
    }
}

impl Drawer for SceneDrawer {
    fn window_size(&self) -> Option<(u32, u32)> {
        self.window
    }

    fn attach(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
    ) -> Result<(), String> {
        let started = Instant::now();
        let plan = self.plan.take().ok_or("the scene is attached already")?;
        let renderer = SceneRenderer::new(
            device,
            queue,
            format,
            &plan,
            self.lighting,
            &mut self.textures,
        )?;
        let mut report = self.report.borrow_mut();
        report.upload_time = started.elapsed();
        report.upload = renderer.upload;
        report.draw_calls = renderer.draw_calls();
        report.blended_instances = renderer.blended_instances();
        self.renderer = Some(renderer);
        Ok(())
    }

    fn window_event(&mut self, event: &winit::event::WindowEvent) {
        use winit::event::{ElementState, MouseButton, WindowEvent};
        use winit::keyboard::PhysicalKey;
        match event {
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    self.key(code, event.state == ElementState::Pressed);
                }
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Right,
                ..
            } => self.controls.look = *state == ElementState::Pressed,
            WindowEvent::Focused(false) => self.controls = Controls::default(),
            _ => {}
        }
    }

    fn mouse_motion(&mut self, dx: f64, dy: f64) {
        self.camera.turn(&self.controls, dx, dy);
    }

    fn encode(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        dt: Duration,
    ) -> Result<(), String> {
        self.camera.advance(&self.controls, dt.as_secs_f32());
        self.size = Some(size);
        let view_proj = self
            .camera
            .view_projection(size.0, size.1)
            .ok_or("the camera's matrices are not finite")?;
        let renderer = self.renderer.as_mut().ok_or("the scene is not attached")?;
        renderer.encode(
            device,
            queue,
            encoder,
            target,
            size,
            &view_proj,
            self.camera.position,
        );
        Ok(())
    }

    fn presented(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) -> Result<(), String> {
        let now = Instant::now();
        self.presented += 1;
        let mut report = self.report.borrow_mut();
        if let Some(last) = self.last_present {
            report.intervals.push(now.duration_since(last));
        }
        self.last_present = Some(now);
        report.size = self.size;
        if let (Some((frame, path)), Some(size)) = (&self.screenshot, self.size)
            && *frame == self.presented
        {
            let result = self
                .camera
                .view_projection(size.0, size.1)
                .ok_or_else(|| "the camera's matrices are not finite".to_string())
                .and_then(|vp| {
                    let renderer = self.renderer.as_mut().ok_or("the scene is not attached")?;
                    renderer.capture(device, queue, size, &vp, self.camera.position)
                })
                .and_then(|rgba| {
                    ow_assets::png::encode(size.0, size.1, &rgba).map_err(|e| e.to_string())
                })
                .and_then(|png| {
                    if let Some(parent) = path.parent() {
                        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                    }
                    std::fs::write(path, png).map_err(|e| e.to_string())?;
                    Ok(path.clone())
                });
            report.screenshot = Some(result);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
