//! Owner: lane H4-MOVEMENT (host track, architecture section 8, "Wave 7 and the host track"). Player movement and
//! collision in the game's units (uu, rotator units, Z-up, seconds), with no physics crate (hub decision,
//! 2026-11-16: the game walks by UE3's own rules, which a general physics engine would not reproduce).
//!
//! AI-assisted (Claude Code). Decision: `docs/decisions/2026-11/2026-11-16-52-host-movement-and-collision-h4.md`.
//!
//! **Why here.** Architecture section 2.1 gives no crate below the host a movement rule, and rule R9 lets only
//! `ow-host` hold the engine-frame conversion; movement needs none (it stays in uu, Z-up, and the engine frame is the
//! game's frame: `frame.rs`). A library crate would mean a 23rd crate and new edges in section 2.2, an architecture
//! change this lane does not make. When a game crate needs movement (AI pawns, `ow-world`'s physics tick), the
//! module moves down whole: it reads only `ow-core`, `ow-assets` and the scene.
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
