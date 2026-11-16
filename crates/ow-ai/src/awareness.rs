//! Owner: lane W7-AI (part 1: awareness and targeting). How an AI pawn's `AIComponent` keeps its target list, notices
//! a target, scores and picks one, and the order of that work inside one AI tick. AI-assisted (Claude Code).
//! Decision 2026-11-16-59. Later AI lanes (melee, leaps and throws) add their sibling modules through this file.
//!
//! **Sources** (no C++ reference: no AI runs in `src/`, so both parity modes run the same rules here):
//! `docs/verification/NATIVE_AI_MELEE.md` (G25: cadence, the target list, the iterators, `ChooseBestTarget`),
//! `docs/verification/NATIVE_AI_AWARENESS.md` (G26: threat levels, `ValidTarget`, `ValidCurrentTarget`,
//! `TargetChanged`, `WillowTI_CalcThreat`), and the live records `AI_MELEE_TARGETING_LIVE.md` (L8) and
//! `AI_AWARENESS_LIVE.md` (L9). Where a note and the game disagree, the game wins. Every rule is **UNVERIFIED** unless
//! its doc says which live record confirmed it, and every live confirmation was of a **den-spawned** bullymong on one
//! map.
//!
//! **What part 1 covers: a component with no parent** (a lone pawn, which is what the Rust world spawns: it has no
//! population dens yet). A component with a parent (a den member) is `NotYet { NotPorted }` naming the dens.
//!
//! **Layers.**
//! - [`record`]: the plain data (threat level, exposure, a target record, the settings read from the definitions).
//! - [`threat`]: `SetThreatLevel`, `GetThreatLevel`, the forget fall, `IsAwareOfTarget`, `NotifyAttackedBy`.
//! - [`iterators`]: the Knowledge iterators (distances, exposure, can-fire, threat) and the Prioritization ones.
//! - [`targets`]: `ValidTarget`, `FindTargets`, `AddTarget`, removal, `ValidCurrentTarget`, `ChooseBestTarget`,
//!   `SetTarget` and `TargetChanged`.
//! - [`tick`]: one AI tick of a component, in the recorded order.
//! - [`vm`]: the adapter over the VM objects (pawn, mind, component, definitions) and the script-facing natives.
//!
//! **What the engine and the host answer** ([`Senses`]): each actor's facts (location, facing, kind, flags), the AI
//! pawn's opinion of another actor, the exposure of a target to a viewer (the engine's visibility service: line of
//! sight through the level's collision, which the host owns; `ow-ai` never depends on `ow-host`), and the world's
//! registry of targetables. The rules never guess these.

use ow_core::units::Rotator;
use ow_object::ids::ObjRef;
use ow_vm::error::VmError;

pub mod iterators;
pub mod record;
pub mod targets;
pub mod threat;
pub mod tick;
pub mod vm;

#[cfg(test)]
mod tests;

pub use record::{
    ActorFacts, ActorKind, Component, Exposure, Knowledge, Opinion, Prioritizer, Record, Settings, ThreatLevel,
};

/// The records the rules come from.
pub const RECORD_MELEE: &str = "docs/verification/NATIVE_AI_MELEE.md";
pub const RECORD_AWARENESS: &str = "docs/verification/NATIVE_AI_AWARENESS.md";
pub const RECORD_LIVE_TARGETING: &str = "docs/verification/AI_MELEE_TARGETING_LIVE.md";
pub const RECORD_LIVE_AWARENESS: &str = "docs/verification/AI_AWARENESS_LIVE.md";
/// The decision that records this module.
pub const DECISION: &str = "docs/decisions/2026-11/2026-11-16-59-rust-ai-awareness-and-targeting-part-1.md";

/// A Z-up vector in uu (architecture rule R9).
pub type Vec3 = [f32; 3];

/// What the engine and the host answer for the rules (module documentation).
pub trait Senses {
    /// The facts of `actor` now.
    fn facts(&mut self, actor: ObjRef) -> Result<ActorFacts, VmError>;
    /// The opinion `pawn` (an AI pawn) has of `other` (`Pawn.GetOpinion`).
    fn opinion(&mut self, pawn: ObjRef, other: ObjRef) -> Result<Opinion, VmError>;
    /// The exposure of `target` to `viewer` (`TargetExposureToMe`: a fraction, 0 to 1; the game also shows -1).
    fn exposure(&mut self, viewer: ObjRef, target: ObjRef) -> Result<f32, VmError>;
    /// The world's registry of targetables, in its order (`FindTargets` walks it).
    fn targetables(&mut self) -> Result<Vec<ObjRef>, VmError>;
    /// How many AIs have `actor` as their current target (`WillowPawn.NumAICurrentlyTargeting`); 1 when the actor has
    /// no readable counter (NATIVE_AI_MELEE.md, `WillowTI_PrioritizeIdle`).
    fn targeted_by(&mut self, actor: ObjRef) -> Result<i32, VmError>;
    /// `ValidTarget` step 2 (G26): the actor the test is about. A controller is replaced by its pawn (`NotYet` when it
    /// has none); an actor whose controller has a pawn is replaced by that pawn; any other actor is itself.
    fn tested_actor(&mut self, actor: ObjRef) -> Result<ObjRef, VmError>;
}

/// The unit facing vector of a rotator (pitch and yaw; roll does not turn the facing), as the dot products of
/// `WillowTI_CalcDistances` use it.
pub fn facing(r: Rotator) -> Vec3 {
    // No oracle: the angles are converted in f64 and the vector rounded to the record's floats.
    let unit = std::f64::consts::TAU / 65536.0;
    let (pitch, yaw) = (f64::from(r.pitch) * unit, f64::from(r.yaw) * unit);
    let cp = ow_core::num::cos_f64(pitch);
    [
        (cp * ow_core::num::cos_f64(yaw)) as f32,
        (cp * ow_core::num::sin_f64(yaw)) as f32,
        ow_core::num::sin_f64(pitch) as f32,
    ]
}

/// `b - a`.
pub fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [b[0] - a[0], b[1] - a[1], b[2] - a[2]]
}

/// The length of `v` (3D).
pub fn length(v: Vec3) -> f32 {
    let (x, y, z) = (f64::from(v[0]), f64::from(v[1]), f64::from(v[2]));
    ow_core::num::sqrt_f64(x * x + y * y + z * z) as f32
}

/// The horizontal length of `v` (X and Y only).
pub fn length_2d(v: Vec3) -> f32 {
    let (x, y) = (f64::from(v[0]), f64::from(v[1]));
    ow_core::num::sqrt_f64(x * x + y * y) as f32
}

/// The dot product of `a` with the unit vector of `b` (0 when `b` is the zero vector).
pub fn dot_unit(a: Vec3, b: Vec3) -> f32 {
    let n = length(b);
    if n == 0.0 {
        return 0.0;
    }
    (a[0] * b[0] + a[1] * b[1] + a[2] * b[2]) / n
}
