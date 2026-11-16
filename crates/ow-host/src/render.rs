//! Owner: lane H2-DRAW (host track, stage 2). The scene renderer on wgpu: it uploads a [`DrawPlan`] once (LOD 0
//! vertex and index buffers, one instance buffer, the diffuse textures) and draws every batch as one instanced,
//! indexed draw, lit by plain diffuse plus one sun and an ambient term. That is the B2 benchmark's level of drawing;
//! nothing here claims BL2's look (no normal maps, lightmaps, fog, shadows, translucency or masks).
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
use crate::draw::{BatchKey, DrawPlan, Instance, VERTEX_BYTES, pick_from};
use crate::frame::Mat4;

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
/// Bytes of one instance: the column-major 4x4 world matrix, then the normal matrix's three columns, each padded to
/// four floats.
const INSTANCE_BYTES: u64 = 112;
/// Bytes of the per-frame uniforms: view-projection, sun, ambient.
const GLOBALS_BYTES: u64 = 96;

const SHADER: &str = r#"
struct Globals {
    view_proj: mat4x4<f32>,
    sun: vec4<f32>,
    ambient: vec4<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(1) @binding(0) var diffuse: texture_2d<f32>;
@group(1) @binding(1) var diffuse_sampler: sampler;

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
};

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) uv: vec2<f32>,
};

@vertex
fn vs_main(v: VertexIn) -> VertexOut {
    let model = mat4x4<f32>(v.m0, v.m1, v.m2, v.m3);
    var out: VertexOut;
    out.clip = globals.view_proj * (model * vec4<f32>(v.position, 1.0));
    let normal_matrix = mat3x3<f32>(v.n0.xyz, v.n1.xyz, v.n2.xyz);
    out.normal = normal_matrix * v.normal;
    out.uv = v.uv;
    return out;
}

@fragment
fn fs_main(in: VertexOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    var n = in.normal;
    if (!front) {
        n = -n;
    }
    let len = length(n);
    var light = globals.ambient.x;
    if (len > 0.0) {
        light = light + globals.sun.w * max(dot(n / len, normalize(globals.sun.xyz)), 0.0);
    }
    let color = textureSample(diffuse, diffuse_sampler, in.uv).rgb;
    return vec4<f32>(color * light, 1.0);
}
"#;

/// The 16 floats of a matrix, column by column.
fn matrix_bytes(m: &Mat4, out: &mut Vec<u8>) {
    for v in m.cols.iter().flatten() {
        out.extend_from_slice(&v.to_le_bytes());
    }
}

/// One instance as the instance buffer holds it ([`INSTANCE_BYTES`]).
fn instance_bytes(i: &Instance, out: &mut Vec<u8>) {
    matrix_bytes(&i.world, out);
    for col in &i.normal {
        for v in col.iter().chain(&[0.0]) {
            out.extend_from_slice(&v.to_le_bytes());
        }
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

/// The per-frame uniforms for `view_proj`.
fn globals_bytes(view_proj: &Mat4) -> Vec<u8> {
    let mut out = Vec::with_capacity(GLOBALS_BYTES as usize);
    matrix_bytes(view_proj, &mut out);
    for v in SUN_DIRECTION
        .iter()
        .chain(&[SUN])
        .chain(&[AMBIENT, 0.0, 0.0, 0.0])
    {
        out.extend_from_slice(&v.to_le_bytes());
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
    /// Textures decoded and uploaded.
    pub textures: usize,
    /// Candidate textures that did not decode, or had no mip the device takes (each skipped for the next).
    pub textures_failed: usize,
    pub texture_bytes: u64,
    /// Materials drawn with a later candidate because a better one failed.
    pub fallbacks: usize,
    /// Materials with candidates of which none decoded, drawn grey.
    pub grey_materials: usize,
}

struct GpuMesh {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
}

struct GpuBatch {
    key: BatchKey,
    first_index: u32,
    index_count: u32,
    instance_offset: u64,
    instances: u32,
}

/// The uploaded scene and the pipelines that draw it.
pub struct SceneRenderer {
    format: wgpu::TextureFormat,
    pipelines: [wgpu::RenderPipeline; 4],
    globals: wgpu::Buffer,
    globals_group: wgpu::BindGroup,
    texture_groups: Vec<wgpu::BindGroup>,
    grey_group: wgpu::BindGroup,
    meshes: Vec<GpuMesh>,
    instances: Option<wgpu::Buffer>,
    batches: Vec<GpuBatch>,
    depth: Option<(wgpu::TextureView, u32, u32)>,
    pub upload: UploadStats,
}

impl SceneRenderer {
    /// Uploads `plan` for drawing into `format` targets, decoding its textures through `textures`. `Err` for a target
    /// format the renderer does not draw into ([`capture_order`]), a buffer larger than the device allows, or more
    /// instances in a batch than a draw can count.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        plan: &DrawPlan,
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
        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("globals"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("diffuse"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("scene"),
            bind_group_layouts: &[Some(&globals_layout), Some(&texture_layout)],
            immediate_size: 0,
        });
        let pipeline = |front_face, cull_mode| {
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
                            attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2],
                        }),
                        Some(wgpu::VertexBufferLayout {
                            array_stride: INSTANCE_BYTES,
                            step_mode: wgpu::VertexStepMode::Instance,
                            attributes: &wgpu::vertex_attr_array![
                                3 => Float32x4, 4 => Float32x4, 5 => Float32x4, 6 => Float32x4,
                                7 => Float32x4, 8 => Float32x4, 9 => Float32x4
                            ],
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
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::Greater),
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
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        // Indexed by `pipeline_index`: (one-sided, plain), (one-sided, mirrored), (two-sided, plain), (two-sided,
        // mirrored).
        let pipelines = [
            pipeline(FRONT_FACE, Some(wgpu::Face::Back)),
            pipeline(MIRRORED_FRONT_FACE, Some(wgpu::Face::Back)),
            pipeline(FRONT_FACE, None),
            pipeline(MIRRORED_FRONT_FACE, None),
        ];

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
            label: Some("diffuse"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            anisotropy_clamp: 8,
            ..Default::default()
        });
        let group = |view: &wgpu::TextureView| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("diffuse"),
                layout: &texture_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
            })
        };

        let mut upload = UploadStats::default();
        let grey = upload_rgba(device, queue, &[(1, 1, &GREY[..])]);
        let grey_group = group(&grey.create_view(&Default::default()));
        let max_dimension = device.limits().max_texture_dimension_2d;
        // Each candidate path decoded at most once: its bind group, or `None` when it failed.
        let mut uploaded: DetMap<String, Option<wgpu::BindGroup>> = DetMap::new();
        let mut texture_groups = Vec::with_capacity(plan.diffuse.len());
        for candidates in &plan.diffuse {
            let best = pick_from(candidates, &|_| true).map(|c| c.path.clone());
            let chosen = loop {
                let failed = |path: &str| matches!(uploaded.get(path), Some(None));
                let Some(pick) = pick_from(candidates, &|path| !failed(path)) else {
                    break None;
                };
                let path = pick.path.clone();
                if let Some(Some(group)) = uploaded.get(&path) {
                    break Some((path, group.clone()));
                }
                let decoded = textures(&path).ok();
                let chain = decoded
                    .as_ref()
                    .map(|t| mip_chain(t, max_dimension))
                    .unwrap_or_default();
                match (&decoded, chain.is_empty()) {
                    (Some(t), false) => {
                        let levels: Vec<(u32, u32, &[u8])> = chain
                            .iter()
                            .map(|&i| {
                                (t.mips[i].width, t.mips[i].height, t.mips[i].rgba.as_slice())
                            })
                            .collect();
                        upload.texture_bytes +=
                            levels.iter().map(|l| l.2.len() as u64).sum::<u64>();
                        let texture = upload_rgba(device, queue, &levels);
                        let group = group(&texture.create_view(&Default::default()));
                        upload.textures += 1;
                        uploaded.insert(path.clone(), Some(group.clone()));
                        break Some((path, group));
                    }
                    _ => {
                        upload.textures_failed += 1;
                        uploaded.insert(path, None);
                    }
                }
            };
            match chosen {
                Some((path, group)) => {
                    if best.as_deref() != Some(path.as_str()) {
                        upload.fallbacks += 1;
                    }
                    texture_groups.push(group);
                }
                None => {
                    upload.grey_materials += 1;
                    texture_groups.push(grey_group.clone());
                }
            }
        }

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
        for b in &plan.batches {
            let Some(range) = plan
                .meshes
                .get(b.key.mesh as usize)
                .and_then(|m| m.sections.get(b.key.section as usize))
            else {
                continue;
            };
            let instance_offset = instance_data.len() as u64;
            for i in &b.instances {
                instance_bytes(i, &mut instance_data);
            }
            batches.push(GpuBatch {
                key: b.key,
                first_index: range.first_index,
                index_count: range.index_count,
                instance_offset,
                instances: u32::try_from(b.instances.len())
                    .map_err(|_| format!("a batch of {} instances", b.instances.len()))?,
            });
        }
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
            texture_groups,
            grey_group,
            meshes,
            instances,
            batches,
            depth: None,
            upload,
        })
    }

    /// The number of instanced draws a frame issues.
    pub fn draw_calls(&self) -> usize {
        self.batches.len()
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

    /// Encodes one frame into `target` (`width` x `height`, the renderer's format) seen through `view_proj`.
    pub fn encode(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        view_proj: &Mat4,
    ) {
        queue.write_buffer(&self.globals, 0, &globals_bytes(view_proj));
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
            return;
        };
        pass.set_bind_group(0, &self.globals_group, &[]);
        for b in &self.batches {
            let Some(mesh) = self.meshes.get(b.key.mesh as usize) else {
                continue;
            };
            pass.set_pipeline(&self.pipelines[pipeline_index(&b.key)]);
            let texture = b
                .key
                .texture
                .and_then(|t| self.texture_groups.get(t as usize))
                .unwrap_or(&self.grey_group);
            pass.set_bind_group(1, texture, &[]);
            pass.set_vertex_buffer(0, mesh.vertices.slice(..));
            let end = b.instance_offset + u64::from(b.instances) * INSTANCE_BYTES;
            pass.set_vertex_buffer(1, instances.slice(b.instance_offset..end));
            pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
            let first = b.first_index;
            pass.draw_indexed(first..first + b.index_count, 0, 0..b.instances);
        }
    }

    /// Draws one frame into a texture of its own and reads it back: `width` x `height` RGBA bytes, rows top first.
    pub fn capture(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        size: (u32, u32),
        view_proj: &Mat4,
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
        self.encode(device, queue, &mut encoder, &view, size, view_proj);
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

/// The pipeline of a batch: one-sided or two-sided, plain or mirrored.
fn pipeline_index(key: &BatchKey) -> usize {
    usize::from(key.two_sided) * 2 + usize::from(key.mirrored)
}

/// An sRGB RGBA texture with the given mip levels (largest first).
fn upload_rgba(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    levels: &[(u32, u32, &[u8])],
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
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
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
        textures: TextureSource,
        camera: FlyCamera,
        window: Option<(u32, u32)>,
        screenshot: Option<(u64, PathBuf)>,
        report: Rc<RefCell<DrawReport>>,
    ) -> SceneDrawer {
        SceneDrawer {
            plan: Some(plan),
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
        let renderer = SceneRenderer::new(device, queue, format, &plan, &mut self.textures)?;
        let mut report = self.report.borrow_mut();
        report.upload_time = started.elapsed();
        report.upload = renderer.upload;
        report.draw_calls = renderer.draw_calls();
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
        renderer.encode(device, queue, encoder, target, size, &view_proj);
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
                    renderer.capture(device, queue, size, &vp)
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
