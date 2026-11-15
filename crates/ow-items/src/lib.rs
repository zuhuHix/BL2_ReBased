//! Items: `DefinitionData` (item identity), the registry of every kind including DLC, balance merge, manufacturer
//! candidates, part pick, names, rarity, value, level, stats and cards for every kind, and item pools. Never depends
//! on `ow-vm`.
//!
//! Engine-agnostic gameplay rules. Each rule keeps the evidence label it has in `docs/verification/` (verified
//! how, or UNVERIFIED); porting a rule does not change what is known about it.
//!
//! Scaffold (W0-SCAFFOLD): every module file exists with its owner lane named in its header; lanes fill their
//! own files and never edit this `lib.rs` (architecture section 2.3).

pub mod candidates;
pub mod cards;
pub mod data;
pub mod error;
pub mod merge;
pub mod names;
pub mod pick;
pub mod pools;
pub mod rarity;
pub mod registry;
pub mod roll;
pub mod stats;
pub mod value;

pub use error::ItemError;
