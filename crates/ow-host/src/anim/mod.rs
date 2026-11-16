//! Owner: lane H6-ANIM (host track, stage 1). Skeletal poses and CPU skinning, headless, in the game's units (uu,
//! Z-up, quaternions), from the plain data of `ow_assets::skeletal` and `ow_assets::animation`.
//!
//! AI-assisted (Claude Code). Decision: `docs/decisions/2026-11/2026-11-16-53-*`.
//!
//! - [`math`]: quaternions and bone transforms as [`crate::frame::Affine`]s (the row-vector UE3 form H3 uses).
//! - [`pose`]: the skeleton's bind matrices, the sampler (time to local transforms to model-space matrices) and its
//!   two end rules, looping and clamping.
//! - [`skin`]: linear blend skinning on the CPU, the reference the stage 2 GPU path will be tested against.
//! - [`report`]: the installed-game check (counts, bind pose, clips) over UModel exports under `local/`.
//!
//! **Frames.** The data is already in the game's frame when it arrives (`ow_assets::actorx` undoes the export's
//! mirror), and the engine's frame is the game's (`frame.rs`, H3), so nothing here converts axes. A model-space bone
//! matrix reaches the renderer through [`crate::frame::Affine::to_engine`], the one conversion H3 owns.
//!
//! **Not here.** The AnimTree (blending, slots, additive nodes, skeletal controls) is not ported in this stage: a pose
//! is one clip at one time. Root motion, `bNoLoopingInterpolation`, `RateScale` as a separate play rate (the export
//! has already folded it into the frame rate) and the game's own key interpolation (UE3 compresses keys; the export
//! resamples them to one key per frame) are open items of the decision.

pub mod math;
pub mod pose;
pub mod report;
pub mod skin;
pub mod tree;

pub use pose::{AnimError, Pose, Skeleton, Wrap};
pub use skin::{skin, skin_matrices};

#[cfg(test)]
mod tests;
