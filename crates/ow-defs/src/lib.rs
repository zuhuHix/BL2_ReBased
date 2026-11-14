//! The definition store: object identity (`IdentityPolicy`, `LoadSet`), lazy definition loading into the heap, the
//! overlay layers, typed property keys, raw trailing bytes per export and census iteration.
//!
//! Scaffold (W0-SCAFFOLD): every module file exists with its owner lane named in its header; lanes fill their
//! own files and never edit this `lib.rs` (architecture section 2.3).

pub mod identity;
pub mod keys;
pub mod overlay;
pub mod store;
pub mod tail;

pub use store::DefStore;
