//! Owner: lane H6-ANIM. The skeleton's bind matrices and the pose sampler.
//!
//! **Model space.** Bones arrive in parent order (`ow_assets::skeletal` refuses anything else, and
//! [`Skeleton::new`] checks it again), so one pass in index order composes every parent before its child: no
//! recursion, and a hierarchy with a cycle cannot be represented. The depth is bounded by the bone count.
//!
//! **Sampling** ([`Skeleton::sample`]). The clip's keys sit at `k / frame_rate` seconds, `k = 0 .. frame_count - 1`
//! (one key per frame, as the export writes them). At time `t` the frame position is `f = t * frame_rate`:
//! - [`Wrap::Loop`]: `f` is taken modulo `frame_count`, and the interval after the last key runs back to key 0, so
//!   the clip lasts `frame_count / frame_rate` seconds. This matches the installed sequences' tags: for example a
//!   91-frame idle has `SequenceLength` 3.0333 s, 91 frames at 30 fps, one interval more than 90 keys span.
//! - [`Wrap::Clamp`]: before key 0 the pose is key 0, after the last key it is the last key, with no wrap interval.
//!
//! Between two keys translation is interpolated linearly and rotation by [`super::math::nlerp`]. Which of these
//! the game does at a clip's end (UE3's `bLooping` and `bNoLoopingInterpolation`) and how it interpolates are
//! UNVERIFIED: not read from native code and not seen in the running game.

use std::fmt;

use ow_assets::animation::BoundClip;
use ow_assets::skeletal::{BoneTransform, SkeletalMesh};

use super::math::{Quat, affine, nlerp, quat_of, rigid_inverse};
use crate::frame::{Affine, ToEngine};

/// What happens past a clip's ends.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wrap {
    Loop,
    Clamp,
}

/// Why a skeleton or a pose could not be built.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum AnimError {
    /// A bone's parent does not come before it.
    BoneOrder { bone: usize },
    /// A bone's rotation has zero length, or a matrix is not finite.
    Degenerate { bone: usize },
    /// The clip has another bone count than the skeleton.
    ClipBones { skeleton: usize, clip: usize },
    /// The clip has no frames, a rate that is not positive, or a track with another key count.
    BadClip,
    /// The time is not finite.
    BadTime,
    /// A vertex names a bone the matrices do not have.
    VertexBone { vertex: usize },
    /// A skinned position does not fit an `f32`.
    NotFinite { vertex: usize },
}

impl fmt::Display for AnimError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AnimError::BoneOrder { bone } => {
                write!(f, "bone {bone}'s parent does not come before it")
            }
            AnimError::Degenerate { bone } => {
                write!(f, "bone {bone} has a degenerate or non-finite transform")
            }
            AnimError::ClipBones { skeleton, clip } => {
                write!(f, "the clip has {clip} bones, the skeleton {skeleton}")
            }
            AnimError::BadClip => {
                f.write_str("the clip has no frames, a bad rate or a short track")
            }
            AnimError::BadTime => f.write_str("the time is not finite"),
            AnimError::VertexBone { vertex } => {
                write!(f, "vertex {vertex} names a bone outside the pose")
            }
            AnimError::NotFinite { vertex } => {
                write!(f, "vertex {vertex} skins to a non-finite position")
            }
        }
    }
}

impl std::error::Error for AnimError {}

/// A bone's local transform in `f64`.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Local {
    pub rotation: Quat,
    pub translation: [f64; 3],
}

impl Local {
    pub fn of(t: &BoneTransform) -> Local {
        Local {
            rotation: quat_of(t),
            translation: t.translation.map(f64::from),
        }
    }
}

/// The reference skeleton: parents, the bind pose, and its model-space matrices and their inverses.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct Skeleton {
    pub names: Vec<String>,
    pub parents: Vec<Option<usize>>,
    pub bind: Vec<Local>,
    pub bind_model: Vec<Affine>,
    pub inverse_bind: Vec<Affine>,
}

/// One pose: per bone its local transform and its model-space matrix.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct Pose {
    pub locals: Vec<Local>,
    pub model: Vec<Affine>,
}

impl Pose {
    /// Bone `i`'s position in the mesh's frame (uu).
    pub fn bone_position(&self, i: usize) -> Option<[f64; 3]> {
        self.model.get(i).map(|m| m.translation)
    }

    /// The model matrices in the engine's form (`frame.rs`, H3): what stage 2 uploads.
    pub fn engine_matrices(&self) -> Vec<crate::frame::Mat4> {
        self.model.iter().map(Affine::to_engine).collect()
    }
}

impl Skeleton {
    pub fn new(mesh: &SkeletalMesh) -> Result<Skeleton, AnimError> {
        let parents: Vec<Option<usize>> = mesh.bones.iter().map(|b| b.parent).collect();
        for (i, p) in parents.iter().enumerate() {
            match p {
                Some(p) if *p < i => {}
                None if i == 0 => {}
                _ => return Err(AnimError::BoneOrder { bone: i }),
            }
        }
        let bind: Vec<Local> = mesh.bones.iter().map(|b| Local::of(&b.bind)).collect();
        let bind_model = compose(&parents, &bind)?;
        let inverse_bind = bind_model.iter().map(rigid_inverse).collect();
        Ok(Skeleton {
            names: mesh.bones.iter().map(|b| b.name.clone()).collect(),
            parents,
            bind,
            bind_model,
            inverse_bind,
        })
    }

    pub fn bone_count(&self) -> usize {
        self.parents.len()
    }

    /// The bind pose as a [`Pose`].
    pub fn bind_pose(&self) -> Pose {
        Pose {
            locals: self.bind.clone(),
            model: self.bind_model.clone(),
        }
    }

    /// Model matrices for the given locals (one per bone).
    pub fn pose_from_locals(&self, locals: Vec<Local>) -> Result<Pose, AnimError> {
        if locals.len() != self.bone_count() {
            return Err(AnimError::ClipBones {
                skeleton: self.bone_count(),
                clip: locals.len(),
            });
        }
        let model = compose(&self.parents, &locals)?;
        Ok(Pose { locals, model })
    }

    /// The pose of `clip` at `time` seconds (see the module notes). A bone without a track keeps its bind local.
    pub fn sample(&self, clip: &BoundClip, time: f64, wrap: Wrap) -> Result<Pose, AnimError> {
        if clip.tracks.len() != self.bone_count() {
            return Err(AnimError::ClipBones {
                skeleton: self.bone_count(),
                clip: clip.tracks.len(),
            });
        }
        let n = clip.frame_count;
        let rate = f64::from(clip.frame_rate);
        if n == 0 || !(rate.is_finite() && rate > 0.0) {
            return Err(AnimError::BadClip);
        }
        if !time.is_finite() {
            return Err(AnimError::BadTime);
        }
        let (k0, k1, alpha) = keys_at(time * rate, n, wrap).ok_or(AnimError::BadTime)?;
        let mut locals = Vec::with_capacity(self.bone_count());
        for (bone, track) in clip.tracks.iter().enumerate() {
            let local = match track {
                None => self.bind[bone],
                Some(keys) => {
                    let (Some(a), Some(b)) = (keys.get(k0), keys.get(k1)) else {
                        return Err(AnimError::BadClip);
                    };
                    let (a, b) = (Local::of(a), Local::of(b));
                    let rotation = nlerp(a.rotation, b.rotation, alpha)
                        .ok_or(AnimError::Degenerate { bone })?;
                    let translation = [0, 1, 2]
                        .map(|i| a.translation[i] * (1.0 - alpha) + b.translation[i] * alpha);
                    Local {
                        rotation,
                        translation,
                    }
                }
            };
            locals.push(local);
        }
        self.pose_from_locals(locals)
    }
}

/// The two keys around frame position `f` and the weight of the second, for `n` keys. `None` only for a
/// non-finite `f`.
pub fn keys_at(f: f64, n: usize, wrap: Wrap) -> Option<(usize, usize, f64)> {
    if !f.is_finite() {
        return None;
    }
    // usize to f64 is exact below 2^53, far above any frame count a file can hold here.
    let count = n as f64;
    match wrap {
        Wrap::Loop => {
            let g = f.rem_euclid(count);
            let k0 = frame_floor(g, n)?;
            Some((k0, (k0 + 1) % n, g - k0 as f64))
        }
        Wrap::Clamp => {
            let last = n - 1;
            if f <= 0.0 {
                Some((0, 0, 0.0))
            } else if f >= last as f64 {
                Some((last, last, 0.0))
            } else {
                let k0 = frame_floor(f, n)?;
                Some((k0, (k0 + 1).min(last), f - k0 as f64))
            }
        }
    }
}

/// `floor(g)` as a key index, for `0 <= g <= n`. `rem_euclid` can round up to exactly `n` for a tiny negative
/// input; that is key 0 one period on, and `n - 1` with the caller's weight `g - k0` of 1.0 gives that same pose.
fn frame_floor(g: f64, n: usize) -> Option<usize> {
    let floor = g.floor();
    // Checked range: 0 <= floor <= n, so the float-to-int `as` (which saturates) is exact here (N5).
    if !(0.0..=n as f64).contains(&floor) {
        return None;
    }
    let k = floor as usize;
    Some(k.min(n - 1))
}

fn compose(parents: &[Option<usize>], locals: &[Local]) -> Result<Vec<Affine>, AnimError> {
    let mut model: Vec<Affine> = Vec::with_capacity(locals.len());
    for (bone, (local, parent)) in locals.iter().zip(parents).enumerate() {
        let a = affine(local.rotation, local.translation).ok_or(AnimError::Degenerate { bone })?;
        let m = match parent {
            Some(p) => a.then(model.get(*p).ok_or(AnimError::BoneOrder { bone })?),
            None => a,
        };
        if !m.is_finite() {
            return Err(AnimError::Degenerate { bone });
        }
        model.push(m);
    }
    Ok(model)
}
