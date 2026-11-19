//! Owner: lane H4-MOVEMENT (host track, architecture section 8, "Wave 7 and the host track"). Player movement and
//! collision in the game's units (uu, rotator units, Z-up, seconds), with no physics crate (hub decision,
//! 2026-11-16: the game walks by UE3's own rules, which a general physics engine would not reproduce).
//!
//! AI-assisted (Claude Code). Decision: `docs/decisions/2026-11/2026-11-16-52-host-movement-and-collision-h4.md`.
//!
//! **Why here.** Architecture section 2.1 gives no crate below the host a movement rule; lane H4 put this module in
//! `ow-host`. Lane LINK-2-HEADLESS-MOVER (decision 2026-11-19-66) moved it down whole, unchanged, so that
//! `ow-package --slice-gate` walks Sanctuary headless through the same mover (link 2). It lives in `ow-assets`
//! because it reads `ow-core`, `ow-assets`' own meshes and bodies, and the scene (`crate::scene`), and `ow-world`
//! may not depend on `ow-assets` (architecture section 2.2); it can move to `ow-world` if that edge is ever added.
//! Movement needs no engine-frame conversion (it stays in uu, Z-up), which stays in `ow-host` (rule R9).
//!
//! - [`collide`]: convex brushes and the swept-shape trace (our own, a stand-in for the game's contact rule).
//! - [`mover`]: the movement rules of `NATIVE_PLAYER_MOVEMENT.md` and the walking-step stand-ins.
//! - [`level`]: a scene's collision from its placements and their meshes' bodies, with a census.
//! - [`walk`]: a scripted walk and its two checks (no fall through, no sticking).
//! - [`sanctuary`]: `ow-host --game <install> --walk`, the walk on the installed Sanctuary, headless.
//!
//! **The shape is a cylinder, not a capsule.** The pawn's collision is its `CylinderComponent` (radius 42, half
//! height 80, read live), and the one live slope observation fits a flat-bottomed cylinder: on Sanctuary's stair
//! ramp hulls (floor normal Z 0.80, about 37 degrees) the original's feet stand 35-42 uu above the hull surface under
//! its centre, close to `r tan(a)` = 31.5 uu plus a small gap, where a capsule's would stand `r (1 / cos(a) - 1)` =
//! 10.5 uu above it (`COLLISION_WALKING_VERIFICATION.md`, 2026-10-21; that record gives the angle as 33 degrees and
//! `r tan(a)` as 31, which do not agree with each other; the normal Z of 0.80 is the measured number). The
//! sweep supports both ([`Shape`]); [`MovementParams::maya`] uses the cylinder.

pub mod collide;
pub mod geom;
pub mod level;
pub mod mover;
pub mod sanctuary;
pub mod walk;

pub use collide::{Brush, CollisionWorld, Hit, Shape};
pub use mover::{MoveInput, MovementParams, Mover, Physics};

#[cfg(test)]
mod tests;
