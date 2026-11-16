//! Owner: lane H6-SKIN (H6 stage 2). GPU skinning: our own WGSL, a bone palette, and a draw of the skinned mesh.
//!
//! AI-assisted (Claude Code). Written from the wgpu 30.0.1 API. Decision: `docs/decisions/2026-11/2026-11-16-64-*`.
//!
//! **Two steps, one buffer.** A compute pass skins every vertex into a buffer (`cs_main`: each vertex's position
//! and normal through its up to four bones' palette matrices, weighted as stored, the same linear blend as the CPU
//! reference `crate::anim::skin`). The draw then reads that very buffer as its vertex buffer. So the readback the
//! tests compare with the CPU reference ([`SkinnedMesh::read_back`]) is the data that is drawn, not a second path.
//!
//! **The palette** is one 4 x 4 matrix per bone, column-major, as `crate::frame::Affine::to_engine` gives it (the
//! one conversion point, H3): each bone's skin matrix, in the mesh's own (model) space. The skinned vertex is then
//! placed in the world by one more matrix, the placement, after the weighted sum. Skinning in model space keeps a
//! weight sum a little off 1 (`ow_assets::skeletal` allows 1e-3) from scaling the world position: off by 0.001 at
//! 170 uu from the mesh's origin is 0.17 uu, where at 3,600 uu out in the world it would be 3.6 uu. The weights are
//! used as stored, as in the CPU reference; nothing is renormalised. The shader multiplies column vectors (`m * p`),
//! as the scene shader does.
//!
//! **Normals.** An ActorX mesh has none, so [`bind_normals`] builds them at the bind pose: the area-weighted face
//! normals summed per welded point, with the face orientation of UE3's index order (`crate::render`: the order runs
//! against the outward normal). Skinned with the palette's 3 x 3 part, which is a rotation here (no scale).
//!
//! **Look.** Plain diffuse: one flat colour lit by the scene's own [`Lighting`] (H8) with the scene shader's rule:
//! the sun along its direction plus the sky's two hemispheres by the normal's height, times the exposure. Both faces
//! are drawn (the back face lit with the flipped normal). No texture or material: H8 owns them.
//!
//! **Depth.** Its own reverse-Z depth buffer, cleared before the mesh is drawn over the scene's colour. The game's
//! first-person depth handling is not read; drawing the arms over the world (they are never hidden by a wall) is a
//! host choice, UNVERIFIED.

use std::sync::{Arc, Mutex};

use ow_assets::skeletal::SkeletalMesh;
use wgpu::util::DeviceExt;

use crate::frame::Mat4;
use crate::render::{DEPTH_FORMAT, Lighting};

/// Most bones the palette holds (the installed arms have 47, the body 27).
pub const MAX_BONES: usize = 256;
/// Compute workgroup size (vertices per group).
pub const WORKGROUP: u32 = 64;
/// The arms' flat colour, linear RGB (a host choice: a neutral skin tone).
pub const SKIN_COLOR: [f32; 3] = [0.55, 0.42, 0.36];
/// Bytes of one skinned vertex in the output buffer: position (vec4) and normal (vec4).
pub const OUT_VERTEX_BYTES: u64 = 32;
/// Bytes of the compute uniform: the placement (mat4) and the vertex count (vec4<u32>).
const PARAMS_BYTES: u64 = 80;
/// Bytes of the draw uniform: view-projection, sun direction, sun, sky upper, sky lower, exposure, colour.
const GLOBALS_BYTES: u64 = 160;

const SHADER: &str = r#"
struct Params {
    place: mat4x4<f32>,
    count: vec4<u32>,
};

@group(0) @binding(0) var<storage, read> src: array<vec4<f32>>;
@group(0) @binding(1) var<storage, read> bones: array<vec4<u32>>;
@group(0) @binding(2) var<storage, read> palette: array<mat4x4<f32>>;
@group(0) @binding(3) var<storage, read_write> dst: array<vec4<f32>>;
@group(0) @binding(4) var<uniform> params: Params;

// Each source vertex is three vec4s: (position, 1), (normal, 0), (weights). Its four bone indices are one vec4<u32>.
@compute @workgroup_size(64)
fn cs_main(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.x;
    if (i >= params.count.x) {
        return;
    }
    let p = vec4<f32>(src[3u * i].xyz, 1.0);
    let n = vec4<f32>(src[3u * i + 1u].xyz, 0.0);
    let w = src[3u * i + 2u];
    let b = bones[i];
    var sp = vec4<f32>(0.0);
    var sn = vec4<f32>(0.0);
    for (var k = 0u; k < 4u; k = k + 1u) {
        if (w[k] != 0.0) {
            let m = palette[b[k]];
            sp = sp + w[k] * (m * p);
            sn = sn + w[k] * (m * n);
        }
    }
    // Skinned in model space; then placed (module notes).
    dst[2u * i] = params.place * vec4<f32>(sp.xyz, 1.0);
    dst[2u * i + 1u] = params.place * vec4<f32>(sn.xyz, 0.0);
}

struct Globals {
    view_proj: mat4x4<f32>,
    sun_dir: vec4<f32>,
    sun: vec4<f32>,
    sky_upper: vec4<f32>,
    sky_lower: vec4<f32>,
    params: vec4<f32>,
    color: vec4<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
};

@vertex
fn vs_main(@location(0) position: vec4<f32>, @location(1) normal: vec4<f32>) -> VertexOut {
    var out: VertexOut;
    out.clip = globals.view_proj * vec4<f32>(position.xyz, 1.0);
    out.normal = normal.xyz;
    return out;
}

@fragment
fn fs_main(in: VertexOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    var n = in.normal;
    if (!front) {
        n = -n;
    }
    // The scene shader's lit rule (crate::render, H8).
    let len = length(n);
    var unit = vec3<f32>(0.0, 0.0, 1.0);
    var sun = vec3<f32>(0.0, 0.0, 0.0);
    if (len > 0.0) {
        unit = n / len;
        sun = globals.sun.rgb * max(dot(unit, normalize(globals.sun_dir.xyz)), 0.0);
    }
    let sky = mix(globals.sky_lower.rgb, globals.sky_upper.rgb, unit.z * 0.5 + 0.5);
    let light = (sky + sun) * globals.params.x;
    return vec4<f32>(globals.color.rgb * light, 1.0);
}
"#;

/// Bind-pose normals per vertex (see the module notes). A vertex whose faces cancel gets a zero normal (lit by the
/// ambient term only).
pub fn bind_normals(mesh: &SkeletalMesh) -> Vec<[f32; 3]> {
    let mut by_point = vec![[0.0f64; 3]; mesh.point_count.max(1)];
    for tri in mesh.indices.chunks_exact(3) {
        let v = |k: usize| mesh.vertices.get(tri[k] as usize);
        let (Some(a), Some(b), Some(c)) = (v(0), v(1), v(2)) else {
            continue;
        };
        let p = |x: &ow_assets::skeletal::SkinVertex| x.position.map(f64::from);
        let (pa, pb, pc) = (p(a), p(b), p(c));
        let e1 = [pb[0] - pa[0], pb[1] - pa[1], pb[2] - pa[2]];
        let e2 = [pc[0] - pa[0], pc[1] - pa[1], pc[2] - pa[2]];
        // UE3's index order runs against the outward normal: the outward normal is the negated cross product.
        let n = [
            -(e1[1] * e2[2] - e1[2] * e2[1]),
            -(e1[2] * e2[0] - e1[0] * e2[2]),
            -(e1[0] * e2[1] - e1[1] * e2[0]),
        ];
        for x in [a, b, c] {
            if let Some(acc) = by_point.get_mut(x.point) {
                for k in 0..3 {
                    acc[k] += n[k];
                }
            }
        }
    }
    mesh.vertices
        .iter()
        .map(|v| {
            let n = by_point.get(v.point).copied().unwrap_or([0.0; 3]);
            let len = ow_core::num::sqrt_f64(n[0] * n[0] + n[1] * n[1] + n[2] * n[2]);
            if len > 0.0 && len.is_finite() {
                // Unit vectors fit f32; float-to-float `as` rounds.
                n.map(|c| (c / len) as f32)
            } else {
                [0.0; 3]
            }
        })
        .collect()
}

/// The palette's bytes: the matrices' 16 floats each, column by column.
fn palette_bytes(palette: &[Mat4]) -> Vec<u8> {
    palette
        .iter()
        .flat_map(|m| m.cols.iter().flatten().flat_map(|v| v.to_le_bytes()))
        .collect()
}

fn floats(values: &[f32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

/// A skeletal mesh on the GPU: its bind vertices, the palette, the skinned output, and the two pipelines.
pub struct SkinnedMesh {
    vertex_count: u32,
    index_count: u32,
    bone_count: usize,
    palette: wgpu::Buffer,
    out: wgpu::Buffer,
    indices: wgpu::Buffer,
    /// Kept with the mesh; the compute bind group reads it.
    params: wgpu::Buffer,
    compute: wgpu::ComputePipeline,
    compute_group: wgpu::BindGroup,
    draw: wgpu::RenderPipeline,
    globals: wgpu::Buffer,
    globals_group: wgpu::BindGroup,
    depth: Option<(wgpu::TextureView, u32, u32)>,
    color: [f32; 3],
}

impl SkinnedMesh {
    /// Uploads `mesh` (with its bind `normals`) for drawing into `format` targets. Refuses a mesh with more than
    /// [`MAX_BONES`] bones, no vertices, or a vertex naming a bone it does not have.
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        mesh: &SkeletalMesh,
        normals: &[[f32; 3]],
    ) -> Result<SkinnedMesh, String> {
        let bone_count = mesh.bones.len();
        if bone_count == 0 || bone_count > MAX_BONES {
            return Err(format!(
                "{bone_count} bones: the palette holds 1 to {MAX_BONES}"
            ));
        }
        if mesh.vertices.is_empty() || normals.len() != mesh.vertices.len() {
            return Err("no vertices, or not one normal per vertex".into());
        }
        let vertex_count = u32::try_from(mesh.vertices.len()).map_err(|_| "too many vertices")?;
        let index_count = u32::try_from(mesh.indices.len()).map_err(|_| "too many indices")?;
        let mut src = Vec::with_capacity(mesh.vertices.len() * 12);
        let mut bones = Vec::with_capacity(mesh.vertices.len() * 4);
        for (v, n) in mesh.vertices.iter().zip(normals) {
            if v.influences.len() > 4 || v.influences.iter().any(|i| i.bone >= bone_count) {
                return Err(
                    "a vertex has more than four influences or names a missing bone".into(),
                );
            }
            let mut w = [0.0f32; 4];
            let mut b = [0u32; 4];
            for (k, inf) in v.influences.iter().enumerate() {
                w[k] = inf.weight;
                b[k] = u32::try_from(inf.bone).map_err(|_| "bone index")?;
            }
            src.extend_from_slice(&[
                v.position[0],
                v.position[1],
                v.position[2],
                1.0,
                n[0],
                n[1],
                n[2],
                0.0,
            ]);
            src.extend_from_slice(&w);
            bones.extend_from_slice(&b);
        }
        if mesh.indices.iter().any(|i| *i >= vertex_count) {
            return Err("an index points past the vertices".into());
        }
        let src = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("skin source"),
            contents: &floats(&src),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let bones = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("skin bones"),
            contents: &bones
                .iter()
                .flat_map(|b| b.to_le_bytes())
                .collect::<Vec<u8>>(),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let palette = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("skin palette"),
            size: (bone_count * 64) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let out = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("skinned vertices"),
            size: u64::from(vertex_count) * OUT_VERTEX_BYTES,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::VERTEX
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("skin indices"),
            contents: &mesh
                .indices
                .iter()
                .flat_map(|i| i.to_le_bytes())
                .collect::<Vec<u8>>(),
            usage: wgpu::BufferUsages::INDEX,
        });
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("skin params"),
            size: PARAMS_BYTES,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("skin shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let storage = |binding: u32, read_only: bool| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let compute_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("skin compute"),
            entries: &[
                storage(0, true),
                storage(1, true),
                storage(2, true),
                storage(3, false),
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let compute_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("skin compute"),
            layout: &compute_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: src.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: bones.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: palette.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: out.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: params.as_entire_binding(),
                },
            ],
        });
        let compute_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("skin compute"),
                bind_group_layouts: &[Some(&compute_layout)],
                immediate_size: 0,
            });
        let compute = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("skin compute"),
            layout: Some(&compute_pipeline_layout),
            module: &shader,
            entry_point: Some("cs_main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("skin globals"),
            size: GLOBALS_BYTES,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("skin globals"),
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
        let globals_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("skin globals"),
            layout: &globals_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals.as_entire_binding(),
            }],
        });
        let draw_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("skin draw"),
            bind_group_layouts: &[Some(&globals_layout)],
            immediate_size: 0,
        });
        let draw = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("skin draw"),
            layout: Some(&draw_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: OUT_VERTEX_BYTES,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x4, 1 => Float32x4],
                })],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: crate::render::FRONT_FACE,
                cull_mode: None,
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
        });
        Ok(SkinnedMesh {
            vertex_count,
            index_count,
            bone_count,
            palette,
            out,
            indices,
            params,
            compute,
            compute_group,
            draw,
            globals,
            globals_group,
            depth: None,
            color: SKIN_COLOR,
        })
    }

    pub fn bone_count(&self) -> usize {
        self.bone_count
    }

    /// Writes the palette (one model-space matrix per bone) and the placement, and records the skinning pass.
    /// Refuses a palette of another length, or a palette or placement with a non-finite entry.
    pub fn skin(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        palette: &[Mat4],
        place: &Mat4,
    ) -> Result<(), String> {
        if palette.len() != self.bone_count {
            return Err(format!(
                "a palette of {} matrices for {} bones",
                palette.len(),
                self.bone_count
            ));
        }
        if !palette.iter().all(Mat4::is_finite) {
            return Err("a palette matrix is not finite".into());
        }
        if !place.is_finite() {
            return Err("the placement is not finite".into());
        }
        queue.write_buffer(&self.palette, 0, &palette_bytes(palette));
        let mut params = palette_bytes(std::slice::from_ref(place));
        for v in [self.vertex_count, 0, 0, 0] {
            params.extend_from_slice(&v.to_le_bytes());
        }
        queue.write_buffer(&self.params, 0, &params);
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("skin"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.compute);
        pass.set_bind_group(0, &self.compute_group, &[]);
        pass.dispatch_workgroups(self.vertex_count.div_ceil(WORKGROUP), 1, 1);
        Ok(())
    }

    fn depth_view(&mut self, device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
        if let Some((view, w, h)) = &self.depth
            && *w == width
            && *h == height
        {
            return view.clone();
        }
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("skin depth"),
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

    /// Records the draw of the skinned vertices over `target` (its colour kept, a fresh depth buffer), seen through
    /// `view_proj` and lit by `lighting`. Call after [`SkinnedMesh::skin`] in the same encoder.
    // The frame's parts, as `crate::render::SceneRenderer::encode` takes them (the same allow there).
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        view_proj: &Mat4,
        lighting: &Lighting,
    ) {
        self.set_view(queue, view_proj, lighting);
        let depth = self.depth_view(device, size.0, size.1);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("skin draw"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
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
        self.draw_in(&mut pass);
    }

    /// The draw's colour, linear RGB ([`SKIN_COLOR`] unless set): a flat stand-in for the mesh's material (H8 owns
    /// materials).
    pub fn set_color(&mut self, color: [f32; 3]) {
        self.color = color;
    }

    /// Writes the draw's view-projection, lighting and colour, for [`SkinnedMesh::draw_in`] (call before the pass).
    pub fn set_view(&self, queue: &wgpu::Queue, view_proj: &Mat4, lighting: &Lighting) {
        let mut globals = palette_bytes(std::slice::from_ref(view_proj));
        for v3 in [
            lighting.sun_direction,
            lighting.sun,
            lighting.sky_upper,
            lighting.sky_lower,
        ] {
            globals.extend(floats(&[v3[0], v3[1], v3[2], 0.0]));
        }
        globals.extend(floats(&[lighting.exposure, 0.0, 0.0, 0.0]));
        globals.extend(floats(&[self.color[0], self.color[1], self.color[2], 1.0]));
        queue.write_buffer(&self.globals, 0, &globals);
    }

    /// Draws the skinned vertices into a pass the caller owns, which must have this mesh's target format and a
    /// [`DEPTH_FORMAT`] depth attachment (the scene pass of `crate::render::SceneRenderer::encode_with` does). Call
    /// after [`SkinnedMesh::skin`] and [`SkinnedMesh::set_view`], before the pass begins.
    pub fn draw_in(&self, pass: &mut wgpu::RenderPass<'_>) {
        pass.set_pipeline(&self.draw);
        pass.set_bind_group(0, &self.globals_group, &[]);
        pass.set_vertex_buffer(0, self.out.slice(..));
        pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..self.index_count, 0, 0..1);
    }

    /// Skins with `palette` and `place` and reads the skinned positions back (blocking): one `[x, y, z]` per vertex.
    /// For tests.
    pub fn read_back(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        palette: &[Mat4],
        place: &Mat4,
    ) -> Result<Vec<[f32; 3]>, String> {
        let bytes = u64::from(self.vertex_count) * OUT_VERTEX_BYTES;
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("skin readback"),
            size: bytes,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("skin readback"),
        });
        self.skin(queue, &mut encoder, palette, place)?;
        encoder.copy_buffer_to_buffer(&self.out, 0, &readback, 0, bytes);
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
        match mapped
            .lock()
            .map_err(|_| "the readback callback panicked")?
            .take()
        {
            Some(Ok(())) => {}
            Some(Err(e)) => return Err(e.to_string()),
            None => return Err("the readback buffer was not mapped".into()),
        }
        let data = slice.get_mapped_range().map_err(|e| e.to_string())?;
        let out = data
            .chunks_exact(32)
            .map(|v| {
                let f = |k: usize| f32::from_le_bytes([v[k], v[k + 1], v[k + 2], v[k + 3]]);
                [f(0), f(4), f(8)]
            })
            .collect();
        drop(data);
        readback.unmap();
        Ok(out)
    }
}

#[cfg(test)]
pub(crate) mod tests;
