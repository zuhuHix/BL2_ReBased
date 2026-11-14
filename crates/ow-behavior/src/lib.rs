//! Behaviors: behavior provider decoding (the fitted variable block), the kernel, the handler registry (crates
//! above register handlers for the behavior classes they own; an unregistered class is `NotYet`) and the handlers
//! that need only the world (port of `src/behavior.*`). Never depends on `ow-combat`, `ow-skills` or `ow-items`.
//!
//! Engine-agnostic gameplay rules. Each rule keeps the evidence label it has in `docs/verification/` (verified
//! how, or UNVERIFIED); porting a rule does not change what is known about it.
//!
//! Scaffold (W0-SCAFFOLD): every module file exists with its owner lane named in its header; lanes fill their
//! own files and never edit this `lib.rs` (architecture section 2.3).

pub mod handlers;
pub mod kernel;
pub mod provider;
pub mod registry;
