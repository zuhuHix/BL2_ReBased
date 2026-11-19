//! Owner: lane H6-SKIN (H6 stage 2). Maya's first-person arms in the play window: the mesh and her hands AnimTree
//! (H7) driven on the simulation's clock, skinned on the GPU ([`super::gpu`]) and attached to the pawn's eye.
//!
//! AI-assisted (Claude Code). Decision: `docs/decisions/2026-11/2026-11-16-64-*`.
//!
//! **What is posed.** The hands tree (`GD_PlayerShared.Character.AnimTree_PlayerHands`) with no input set: the idle
//! path, as H7's installed report drives it, with the same AnimSets (the hands component's three and the pistol's).
//! Its two `NotYet` nodes are passed to their first child ([`Unsupported::PassFirstChild`]), as in that report.
//! The clock is the host's frame-time clock (`Snapshot::sim_time`, the sum of the real frame times, dropped time
//! included), cut into step-sized chunks (`crate::play::step_seconds`, 1/60 s) with the remainder carried to the
//! next frame. So the pose after `n` chunks is the same whatever the frame times. It is not tied to
//! `Snapshot::step`: after a stall the simulation takes at most `crate::play::MAX_STEPS_PER_FRAME` (8) steps and
//! drops the rest, while the pose runs up to [`MAX_POSE_STEPS`] (240) chunks. On a 250 ms frame the simulation
//! takes 8 steps and the arms 15.
//!
//! **Where the arms go** (a host choice, UNVERIFIED: the game's rule, which `WillowAnimNode_AddCameraBone` takes part
//! in, is not read). The posed `Camera` bone is put at the eye with its axes on the view's: the mesh's placement is
//! the inverse of that bone's model matrix, then the eye's placement (`frame::Placement` of the eye and the view
//! rotation, unit scale). The retired UE5 host anchored the arms on the same bone. Every matrix goes through
//! `crate::frame` (`Affine::then`, `Affine::to_engine`, the perspective and view matrices): this module converts
//! nothing itself.
//!
//! **Their own projection** (a host choice, UNVERIFIED): the scene's vertical field of view
//! (`crate::play::vertical_fov_deg`) with a near plane of [`ARMS_NEAR`] uu, not the scene's 10 uu
//! (`crate::play::NEAR`), which cut the forearms where they run back toward the eye. The arms clear their own depth
//! (`super::gpu`), so the two near planes do not meet. The game's first-person near plane and field of view are not
//! read.
//!
//! **Debt.** [`capture_with`] repeats the readback of `crate::render::SceneRenderer::capture`, so the arms can be
//! drawn into the same captured frame without changing that file; the two should become one helper.

use std::path::PathBuf;
use std::time::Duration;

use ow_assets::skeletal::{SkeletalMesh, read_psk_file};
use ow_defs::DefStore;
use ow_object::heap::ObjectSpace;

use super::gpu::{SkinnedMesh, bind_normals};
use super::math::rigid_inverse;
use super::pose::{Pose, Skeleton};
use super::skin::skin_matrices;
use super::tree::eval::DriverState;
use super::tree::report::{HANDS_SETS, HANDS_TREE, library, locate};
use super::tree::{ClipLibrary, Driver, Inputs, TreeDef, Unsupported, load_tree};
use crate::frame::{Affine, Mat4, Placement, ToEngine};
use crate::render::Lighting;
use crate::sim::ViewPose;

/// The bone the arms hang from (module notes).
pub const CAMERA_BONE: &str = "Camera";
/// The arms mesh in a UModel export folder of `GD_Siren_Streaming_SF`.
pub const ARMS_MESH: &str = "SkeletalMesh3/Hands_Siren.psk";
/// The arms' near plane, uu (module notes: a host choice).
pub const ARMS_NEAR: f32 = 1.0;
/// Most step-sized chunks one frame advances the pose by: a longer gap (a stall) is advanced in this many chunks and
/// the rest dropped. Not the simulation's limit (`crate::play::MAX_STEPS_PER_FRAME`, 8): see the module notes.
pub const MAX_POSE_STEPS: u32 = 240;

/// The arms, loaded: mesh, skeleton, tree, clips, bind normals, and the driver's state between frames.
pub struct Arms {
    pub mesh: SkeletalMesh,
    pub skeleton: Skeleton,
    pub tree: TreeDef,
    pub clips: ClipLibrary,
    pub normals: Vec<[f32; 3]>,
    pub camera_bone: usize,
    state: Option<DriverState>,
}

/// Loads the arms from UModel exports under `roots` and the hands tree and AnimSet tags from `defs` (the store the
/// scene was loaded with, and its object space).
pub fn load(
    defs: &mut DefStore,
    space: &mut ObjectSpace,
    roots: &[PathBuf],
) -> Result<Arms, String> {
    let mesh = read_psk_file(&locate(roots, ARMS_MESH).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let skeleton = Skeleton::new(&mesh).map_err(|e| e.to_string())?;
    let camera_bone = mesh
        .bone_index(CAMERA_BONE)
        .ok_or("the arms mesh has no Camera bone")?;
    let tree = load_tree(defs, space, HANDS_TREE).map_err(|e| e.to_string())?;
    let clips = library(defs, space, &mesh, &HANDS_SETS, roots).map_err(|e| e.to_string())?;
    let normals = bind_normals(&mesh);
    let mut arms = Arms {
        mesh,
        skeleton,
        tree,
        clips,
        normals,
        camera_bone,
        state: None,
    };
    // Check the tree evaluates at once, so a broken set-up fails at load and not on the first frame.
    arms.advance(0.0)?;
    Ok(arms)
}

impl Arms {
    /// Ticks the tree `steps` times by `step` seconds and returns the pose.
    pub fn advance_steps(&mut self, steps: u32, step: f64) -> Result<Pose, String> {
        let mut d = match self.state.take() {
            Some(s) => Driver::resume(
                &self.tree,
                &self.clips,
                &self.skeleton,
                Inputs::default(),
                s,
            ),
            None => Driver::new(&self.tree, &self.clips, &self.skeleton, Inputs::default()),
        }
        .map_err(|e| e.to_string())?;
        d.unsupported = Unsupported::PassFirstChild;
        for _ in 0..steps {
            d.tick(step).map_err(|e| e.to_string())?;
        }
        let locals = d.locals().map_err(|e| e.to_string())?;
        // Notifies are reported, not applied (H7); a long session must not keep them.
        d.log.notifies.clear();
        self.state = Some(d.into_state());
        self.skeleton
            .pose_from_locals(locals)
            .map_err(|e| e.to_string())
    }

    /// The mesh's placement in the world for `pose` seen from `view` (module notes). `None` when the eye's placement
    /// is not finite.
    pub fn placement(&self, pose: &Pose, view: &ViewPose) -> Option<Affine> {
        let camera = pose.model.get(self.camera_bone)?;
        let eye = Placement::new(view.eye, view.rotation, 1.0, [1.0; 3]).affine()?;
        let placed = rigid_inverse(camera).then(&eye);
        placed.is_finite().then_some(placed)
    }

    /// Ticks the tree once by `dt` seconds and returns the pose.
    pub fn advance(&mut self, dt: f64) -> Result<Pose, String> {
        self.advance_steps(1, dt)
    }

    /// The palette into `out` (cleared first, its allocation kept): each bone's skin matrix in model space, in the
    /// engine's form. The placement goes to the GPU apart (`super::gpu`).
    pub fn palette_into(&self, pose: &Pose, out: &mut Vec<Mat4>) {
        out.clear();
        out.extend(
            skin_matrices(&self.skeleton, pose)
                .iter()
                .map(Affine::to_engine),
        );
    }
}

/// The arms in the play window: [`Arms`] plus its GPU mesh, attached when the device exists, posed on the
/// simulation's clock.
pub struct ArmsLayer {
    arms: Arms,
    /// The palette of the last frame, its allocation reused.
    palette: Vec<Mat4>,
    gpu: Option<SkinnedMesh>,
    view: Option<ViewPose>,
    /// The simulation time of the last snapshot, and of the last frame drawn.
    sim_time: Duration,
    drawn_at: Duration,
    /// Simulated seconds not yet advanced: less than one step after each frame.
    carry: f64,
}

impl ArmsLayer {
    pub fn new(arms: Arms) -> ArmsLayer {
        ArmsLayer {
            arms,
            palette: Vec::new(),
            gpu: None,
            view: None,
            sim_time: Duration::ZERO,
            drawn_at: Duration::ZERO,
            carry: 0.0,
        }
    }

    pub fn attach(
        &mut self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
    ) -> Result<(), String> {
        self.gpu = Some(SkinnedMesh::new(
            device,
            format,
            &self.arms.mesh,
            &self.arms.normals,
        )?);
        Ok(())
    }

    /// The snapshot's view and clock. A fly-camera view hides nothing: the arms follow whatever view is drawn.
    pub fn snapshot(&mut self, view: Option<ViewPose>, sim_time: Duration) {
        if view.is_some() {
            self.view = view;
        }
        self.sim_time = sim_time;
    }

    /// How many whole simulation steps the pose advances by now, the remainder carried (module notes).
    fn steps_due(&mut self) -> u32 {
        let delta = self.sim_time.saturating_sub(self.drawn_at).as_secs_f64();
        self.drawn_at = self.sim_time;
        whole_steps(&mut self.carry, delta, crate::play::step_seconds())
    }

    /// The arms' view-projection: the scene's vertical field of view with the [`ARMS_NEAR`] plane, through
    /// `crate::frame`. `None` for a zero size or a view the boundary refuses.
    pub fn view_projection(view: &ViewPose, size: (u32, u32)) -> Option<Mat4> {
        crate::play::view_projection_near(view, size.0, size.1, ARMS_NEAR)
    }

    /// Advances the pose to the latest frame-time clock in step-sized chunks, skins, and draws over `target` with the
    /// arms' own projection, lit by the scene's `lighting`.
    pub fn encode(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        lighting: &Lighting,
    ) -> Result<(), String> {
        let view = self.view.ok_or("the arms have no view yet")?;
        let steps = self.steps_due();
        let pose = self
            .arms
            .advance_steps(steps, crate::play::step_seconds())?;
        let placement = self
            .arms
            .placement(&pose, &view)
            .ok_or("the arms' placement is not finite")?;
        let view_proj = ArmsLayer::view_projection(&view, size)
            .ok_or("the arms' view matrices are not finite")?;
        self.arms.palette_into(&pose, &mut self.palette);
        let gpu = self.gpu.as_mut().ok_or("the arms are not attached")?;
        gpu.skin(queue, encoder, &self.palette, &placement.to_engine())?;
        gpu.draw(device, queue, encoder, target, size, &view_proj, lighting);
        Ok(())
    }
}

/// Adds `delta` seconds to `carry` and takes the whole `step`s out of it: their count, at most [`MAX_POSE_STEPS`]
/// (steps past that are dropped with the carry). The carry is left in `0..step`, also after a huge jump, where
/// `carry - whole * step` could round to a value outside it.
pub fn whole_steps(carry: &mut f64, delta: f64, step: f64) -> u32 {
    if delta.is_finite() && delta > 0.0 {
        *carry += delta;
    }
    let whole = (*carry / step).floor();
    if !(step > 0.0 && whole.is_finite() && whole >= 1.0) {
        return 0;
    }
    *carry = (*carry - whole * step).rem_euclid(step);
    if !(*carry >= 0.0 && *carry < step) {
        // rem_euclid can round up to exactly `step` for a tiny negative input: that is a carry of 0.
        *carry = 0.0;
    }
    // Checked: 1 <= whole, and it is capped at MAX_POSE_STEPS, so the float-to-int `as` is exact here (N5).
    whole.min(f64::from(MAX_POSE_STEPS)) as u32
}

/// Draws one frame with `encode` into a texture of its own and reads it back: `width` x `height` RGBA bytes, rows top
/// first. The same readback as `crate::render::SceneRenderer::capture` (kept here so the arms can be drawn into the
/// same frame without changing that file).
pub fn capture_with(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    format: wgpu::TextureFormat,
    size: (u32, u32),
    encode: &mut dyn FnMut(&mut wgpu::CommandEncoder, &wgpu::TextureView) -> Result<(), String>,
) -> Result<Vec<u8>, String> {
    use std::sync::{Arc, Mutex};
    let (width, height) = size;
    let bgra = crate::render::capture_order(format)
        .ok_or_else(|| format!("no capture conversion for {format:?}"))?;
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
        format,
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
    encode(&mut encoder, &view)?;
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
    match mapped
        .lock()
        .map_err(|_| "the capture callback panicked")?
        .take()
    {
        Some(Ok(())) => {}
        Some(Err(e)) => return Err(e.to_string()),
        None => return Err("the capture buffer was not mapped".into()),
    }
    let data = slice.get_mapped_range().map_err(|e| e.to_string())?;
    let row = usize::try_from(row).map_err(|e| e.to_string())?;
    let padded = usize::try_from(padded).map_err(|e| e.to_string())?;
    let mut out = Vec::with_capacity(row * height as usize);
    for y in 0..height as usize {
        for px in data[y * padded..y * padded + row].chunks_exact(4) {
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

#[cfg(test)]
mod tests;
