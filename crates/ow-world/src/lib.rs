//! The running world: `World` (heap, definitions, VM, clock, randomness, attribute system, balance), its `AttrWorld`
//! implementation, actor spawn and destroy, iterators, timers, resource pools and the pool manager, pawn health glue
//! and the level-up rebase.
//!
//! Engine-agnostic gameplay rules. Each rule keeps the evidence label it has in `docs/verification/` (verified
//! how, or UNVERIFIED); porting a rule does not change what is known about it.
//!
//! Scaffold (W0-SCAFFOLD): every module file exists with its owner lane named in its header; lanes fill their
//! own files and never edit this `lib.rs` (architecture section 2.3).

pub mod actors;
pub mod levelup;
pub mod placement;
pub mod pools;
pub mod query;
pub mod timers;
pub mod touch_trace;
pub mod trace_filter;
pub mod world;
