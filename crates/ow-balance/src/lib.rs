//! Balance rules: playthrough, OP level, player count and game-stage types, the XP curve, region game stage, item
//! stage cap, playthrough modifiers, mission and kill XP, and the offline balance world. Never depends on `ow-vm`.
//!
//! Engine-agnostic gameplay rules. Each rule keeps the evidence label it has in `docs/verification/` (verified
//! how, or UNVERIFIED); porting a rule does not change what is known about it.
//!
//! Scaffold (W0-SCAFFOLD): every module file exists with its owner lane named in its header; lanes fill their
//! own files and never edit this `lib.rs` (architecture section 2.3).

pub mod cap;
pub mod curve;
pub mod offline;
pub mod playthrough;
pub mod stage;
pub mod types;
pub mod xp;

pub use types::GameStage;
