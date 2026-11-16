//! Owner: lane H6-SKIN (H6 stage 2). Maya's first-person arms in the play window: the mesh and her hands AnimTree
//! (H7) driven on the simulation's clock, skinned on the GPU ([`super::gpu`]) and attached to the pawn's eye.
//!
//! AI-assisted (Claude Code). Decision: `docs/decisions/2026-11/2026-11-16-64-*`.
//!
//! **What is posed.** The hands tree (`GD_PlayerShared.Character.AnimTree_PlayerHands`) with no input set: the idle
//! path, as H7's installed report drives it, with the same AnimSets (the hands component's three and the pistol's).
//! Its two `NotYet` nodes are passed to their first child ([`Unsupported::PassFirstChild`]), as in that report.
//! The clock is the simulation's (`Snapshot::sim_time`), so a scripted run poses the same way whatever the frame
//! times.
//!
//! **Where the arms go** (a host choice, UNVERIFIED: the game's rule, which `WillowAnimNode_AddCameraBone` takes part
//! in, is not read). The posed `Camera` bone is put at the eye with its axes on the view's: the mesh's placement is
//! the inverse of that bone's model matrix, then the eye's placement (`frame::Placement` of the eye and the view
//! rotation, unit scale). The retired UE5 host anchored the arms on the same bone. Every matrix goes through
//! `crate::frame` (`Affine::then`, `Affine::to_engine`): this module converts nothing itself.

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
use crate::frame::{Affine, Mat4, Placement};
use crate::sim::ViewPose;

/// The bone the arms hang from (module notes).
pub const CAMERA_BONE: &str = "Camera";
/// The arms mesh in a UModel export folder of `GD_Siren_Streaming_SF`.
pub const ARMS_MESH: &str = "SkeletalMesh3/Hands_Siren.psk";

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
    /// Ticks the tree by `dt` seconds and returns the pose.
    pub fn advance(&mut self, dt: f64) -> Result<Pose, String> {
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
        d.tick(dt).map_err(|e| e.to_string())?;
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

    /// One palette matrix per bone: each bone's skin matrix, then `placement`, in the engine's form.
    pub fn palette(&self, pose: &Pose, placement: &Affine) -> Vec<Mat4> {
        skin_matrices(&self.skeleton, pose)
            .iter()
            .map(|s| s.then(placement).to_engine())
            .collect()
    }
}

/// The arms in the play window: [`Arms`] plus its GPU mesh, attached when the device exists, posed on the
/// simulation's clock.
pub struct ArmsLayer {
    arms: Arms,
    gpu: Option<SkinnedMesh>,
    view: Option<ViewPose>,
    /// The simulation time of the last snapshot, and the one the pose was last advanced to.
    sim_time: Duration,
    posed_at: Duration,
}

impl ArmsLayer {
    pub fn new(arms: Arms) -> ArmsLayer {
        ArmsLayer {
            arms,
            gpu: None,
            view: None,
            sim_time: Duration::ZERO,
            posed_at: Duration::ZERO,
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

    /// Advances the pose to the latest simulation time, skins, and draws over `target`.
    pub fn encode(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        size: (u32, u32),
        view_proj: &Mat4,
    ) -> Result<(), String> {
        let view = self.view.ok_or("the arms have no view yet")?;
        let dt = self.sim_time.saturating_sub(self.posed_at).as_secs_f64();
        self.posed_at = self.sim_time;
        let pose = self.arms.advance(dt)?;
        let placement = self
            .arms
            .placement(&pose, &view)
            .ok_or("the arms' placement is not finite")?;
        let palette = self.arms.palette(&pose, &placement);
        let gpu = self.gpu.as_mut().ok_or("the arms are not attached")?;
        gpu.skin(queue, encoder, &palette)?;
        gpu.draw(device, queue, encoder, target, size, view_proj);
        Ok(())
    }
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
