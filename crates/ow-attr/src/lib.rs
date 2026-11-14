//! The attribute system: modifier stacks, plain attributes, the `AttributeInitializationData` evaluator, value and
//! context resolvers, expressions, attribute effects and the notification hook. The one evaluator every rule uses
//! (architecture rule R2); it never depends on `ow-vm`.
//!
//! Engine-agnostic gameplay rules. Each rule keeps the evidence label it has in `docs/verification/` (verified
//! how, or UNVERIFIED); porting a rule does not change what is known about it.
//!
//! Scaffold (W0-SCAFFOLD): every module file exists with its owner lane named in its header; lanes fill their
//! own files and never edit this `lib.rs` (architecture section 2.3).

pub mod api;
pub mod context_resolvers;
pub mod effects;
pub mod eval;
pub mod expr;
pub mod math;
pub mod registry;
pub mod stack;
pub mod storage;
pub mod value_resolvers;

pub use registry::AttributeSystem;
