//! Owner: lane H4-WEAPON. The play mode's placeholder first-person overlay: a crosshair, a grey block where the held
//! weapon will be (the weapon mesh is lane H6 stage 2's), a muzzle flash on the frame a shot fires, and a marker on
//! the last shot's impact point for two seconds (red when the shot has a damage number, orange when it is `NotYet`).
//! Nothing here is the game's HUD or weapon presentation; sizes and colours are the host's.
//!
//! AI-assisted (Claude Code). Decision: `docs/decisions/2026-11/2026-11-16-61-*`.
//!
//! [`rects`] builds the overlay as screen rectangles (pixels, top left origin), a pure function of the drawable size,
//! the snapshot's last shot and the view-projection (the impact is projected with `crate::frame`'s matrix, the only
//! conversion). [`Overlay`] draws them in one pass over the frame (load, no depth), and [`paint`] paints the same
//! rectangles into a screenshot's pixels, since the scene capture draws the scene only.

use ow_core::num::pow_f64;

use crate::frame::Mat4;
use crate::sim::ShotView;

/// How long the impact marker stays, simulation steps (2 s at 1/60 s).
pub const MARKER_STEPS: u64 = 120;
/// How many rectangles one frame may draw.
pub const MAX_RECTS: usize = 16;

/// A filled rectangle: pixels `x0..x1` by `y0..y1` (top left origin) in an sRGB colour.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
    pub srgb: [u8; 4],
}

impl Rect {
    pub fn new(x0: f32, y0: f32, x1: f32, y1: f32, srgb: [u8; 4]) -> Rect {
        Rect { x0, y0, x1, y1, srgb }
    }
}

const WHITE: [u8; 4] = [235, 235, 235, 255];
const BODY: [u8; 4] = [70, 72, 76, 255];
const BARREL: [u8; 4] = [40, 41, 44, 255];
const FLASH: [u8; 4] = [255, 214, 90, 255];
const HIT: [u8; 4] = [230, 40, 40, 255];
const NOT_YET: [u8; 4] = [255, 150, 30, 255];

/// The overlay of a `width` x `height` frame (module documentation).
pub fn rects(width: u32, height: u32, shot: Option<&ShotView>, view_proj: &Mat4) -> Vec<Rect> {
    let (w, h) = (width as f32, height as f32);
    if width == 0 || height == 0 {
        return Vec::new();
    }
    let (cx, cy) = (w / 2.0, h / 2.0);
    let mut out = vec![
        Rect::new(cx - 9.0, cy - 1.0, cx - 3.0, cy + 1.0, WHITE),
        Rect::new(cx + 3.0, cy - 1.0, cx + 9.0, cy + 1.0, WHITE),
        Rect::new(cx - 1.0, cy - 9.0, cx + 1.0, cy - 3.0, WHITE),
        Rect::new(cx - 1.0, cy + 3.0, cx + 1.0, cy + 9.0, WHITE),
        // The weapon placeholder: a body at the lower right and a barrel toward the centre.
        Rect::new(0.62 * w, 0.80 * h, 0.82 * w, h, BODY),
        Rect::new(0.58 * w, 0.76 * h, 0.66 * w, 0.82 * h, BARREL),
    ];
    let Some(shot) = shot else { return out };
    if shot.this_frame {
        out.push(Rect::new(0.555 * w, 0.725 * h, 0.595 * w, 0.775 * h, FLASH));
    }
    if let Some(p) = shot.impact
        && shot.age_steps < MARKER_STEPS
    {
        let c = view_proj.apply([p[0], p[1], p[2], 1.0]);
        if c[3] > 0.0 {
            let (nx, ny) = (c[0] / c[3], c[1] / c[3]);
            if (-1.0..=1.0).contains(&nx) && (-1.0..=1.0).contains(&ny) {
                let (px, py) = ((nx + 1.0) / 2.0 * w, (1.0 - ny) / 2.0 * h);
                let colour = if shot.damage_known { HIT } else { NOT_YET };
                out.push(Rect::new(px - 4.0, py - 4.0, px + 4.0, py + 4.0, colour));
            }
        }
    }
    out
}

/// sRGB byte to linear (the standard curve), for a target that encodes linear colour as sRGB.
fn linear(byte: u8) -> f32 {
    let c = f64::from(byte) / 255.0;
    let l = if c <= 0.04045 { c / 12.92 } else { pow_f64((c + 0.055) / 1.055, 2.4) };
    l as f32
}

/// Paints `rects` into `rgba` (`width` x `height`, rows top first, 4 bytes a pixel), clipped to the image.
pub fn paint(rgba: &mut [u8], width: u32, height: u32, rects: &[Rect]) {
    let (w, h) = (width as usize, height as usize);
    if rgba.len() < w.saturating_mul(h).saturating_mul(4) {
        return;
    }
    let clip = |v: f32, n: usize| -> usize { crate::movement::geom::floor_index(f64::from(v), n) };
    for r in rects {
        let (x0, x1) = (clip(r.x0, w), clip(r.x1, w));
        let (y0, y1) = (clip(r.y0, h), clip(r.y1, h));
        for y in y0..y1 {
            for x in x0..x1 {
                let i = (y * w + x) * 4;
                rgba[i..i + 4].copy_from_slice(&r.srgb);
            }
        }
    }
}

const SHADER: &str = "
struct Out { @builtin(position) position: vec4<f32>, @location(0) colour: vec4<f32> };
@vertex fn vs_main(@location(0) position: vec2<f32>, @location(1) colour: vec4<f32>) -> Out {
    var o: Out;
    o.position = vec4<f32>(position, 0.0, 1.0);
    o.colour = colour;
    return o;
}
@fragment fn fs_main(i: Out) -> @location(0) vec4<f32> { return i.colour; }
";

/// Bytes a vertex: a clip-space position (2 floats) and a colour (4 floats).
const VERTEX: u64 = 24;

/// The overlay's pipeline and vertex buffer.
pub struct Overlay {
    pipeline: wgpu::RenderPipeline,
    vertices: wgpu::Buffer,
    srgb_target: bool,
}

impl Overlay {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Overlay {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("overlay shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("overlay"),
            bind_group_layouts: &[],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("overlay"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: VERTEX,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4],
                })],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
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
        let vertices = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("overlay vertices"),
            size: (MAX_RECTS as u64) * 6 * VERTEX,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Overlay { pipeline, vertices, srgb_target: format.is_srgb() }
    }

    /// Draws `rects` over `target` (`size` pixels), keeping what is there.
    pub fn encode(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        rects: &[Rect],
    ) {
        let (w, h) = (size.0 as f32, size.1 as f32);
        if size.0 == 0 || size.1 == 0 || rects.is_empty() {
            return;
        }
        let mut bytes = Vec::with_capacity(rects.len().min(MAX_RECTS) * 6 * VERTEX as usize);
        for r in rects.iter().take(MAX_RECTS) {
            let colour = r.srgb.map(|b| if self.srgb_target { linear(b) } else { f32::from(b) / 255.0 });
            let (x0, x1) = (r.x0 / w * 2.0 - 1.0, r.x1 / w * 2.0 - 1.0);
            let (y0, y1) = (1.0 - r.y0 / h * 2.0, 1.0 - r.y1 / h * 2.0);
            for (x, y) in [(x0, y0), (x1, y0), (x1, y1), (x0, y0), (x1, y1), (x0, y1)] {
                for v in [x, y, colour[0], colour[1], colour[2], 1.0] {
                    bytes.extend_from_slice(&v.to_le_bytes());
                }
            }
        }
        let count = (bytes.len() as u64 / VERTEX) as u32;
        queue.write_buffer(&self.vertices, 0, &bytes);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("overlay"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
            })],
            ..Default::default()
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.draw(0..count, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::{perspective_reverse_z_infinite, view_matrix};
    use ow_core::units::Rotator;

    fn vp() -> Mat4 {
        let p = perspective_reverse_z_infinite(43.0, 16.0 / 9.0, 10.0).unwrap();
        p.mul(&view_matrix([0.0; 3], Rotator::default()).unwrap())
    }

    fn shot(impact: Option<[f32; 3]>, age: u64, this_frame: bool, known: bool) -> ShotView {
        ShotView { impact, age_steps: age, this_frame, damage_known: known }
    }

    #[test]
    fn the_crosshair_and_the_placeholder_are_always_there() {
        let r = rects(1280, 720, None, &vp());
        assert_eq!(r.len(), 6);
        assert!(r[0].x0 < 640.0 && r[1].x1 > 640.0 && r[2].y0 < 360.0);
        assert!(rects(0, 720, None, &vp()).is_empty());
    }

    #[test]
    fn a_fresh_shot_flashes_and_marks_its_impact_ahead() {
        let r = rects(1280, 720, Some(&shot(Some([1000.0, 0.0, 0.0]), 0, true, true)), &vp());
        assert!(r.iter().any(|x| x.srgb == FLASH));
        let marker = r.iter().find(|x| x.srgb == HIT).expect("a marker");
        // Straight ahead of the eye: the centre of the screen.
        assert!(((marker.x0 + marker.x1) / 2.0 - 640.0).abs() < 0.01 && ((marker.y0 + marker.y1) / 2.0 - 360.0).abs() < 0.01);
        // NotYet damage is orange; behind the eye, or too old, no marker.
        let r = rects(1280, 720, Some(&shot(Some([1000.0, 0.0, 0.0]), 5, false, false)), &vp());
        assert!(r.iter().any(|x| x.srgb == NOT_YET) && !r.iter().any(|x| x.srgb == FLASH));
        assert_eq!(rects(1280, 720, Some(&shot(Some([-1000.0, 0.0, 0.0]), 0, false, true)), &vp()).len(), 6);
        assert_eq!(rects(1280, 720, Some(&shot(Some([1000.0, 0.0, 0.0]), MARKER_STEPS, false, true)), &vp()).len(), 6);
    }

    #[test]
    fn painting_clips_to_the_image() {
        let mut px = vec![0u8; 4 * 4 * 4];
        paint(&mut px, 4, 4, &[Rect::new(-5.0, 1.0, 2.0, 2.0, [9, 8, 7, 255]), Rect::new(3.0, 3.0, 99.0, 99.0, WHITE)]);
        assert_eq!(&px[(4 + 0) * 4..(4 + 0) * 4 + 4], &[9, 8, 7, 255]);
        assert_eq!(&px[(4 + 1) * 4..(4 + 1) * 4 + 4], &[9, 8, 7, 255]);
        assert_eq!(&px[(4 + 2) * 4..(4 + 2) * 4 + 4], &[0, 0, 0, 0]);
        assert_eq!(&px[(3 * 4 + 3) * 4..(3 * 4 + 3) * 4 + 4], &WHITE);
        // A short buffer is left alone.
        let mut short = vec![0u8; 3];
        paint(&mut short, 4, 4, &[Rect::new(0.0, 0.0, 4.0, 4.0, WHITE)]);
        assert_eq!(short, vec![0, 0, 0]);
    }
}
