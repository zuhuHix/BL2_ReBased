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
pub mod merge;
pub mod names;
pub mod pick;
pub mod pools;
pub mod rarity;
pub mod registry;
pub mod roll;
pub mod stats;
pub mod value;

/// Why an item rule could not run (architecture section 3.7). The store's errors are transparent: they print as the
/// store's error, and `source()` is its cause; a `NotYet` from any layer below is [`ItemError::NotYet`].
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum ItemError {
    Defs(ow_defs::store::DefError),
    NotYet(ow_core::NotYet),
}

impl std::fmt::Display for ItemError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ItemError::Defs(e) => e.fmt(f),
            ItemError::NotYet(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for ItemError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ItemError::Defs(e) => e.source(),
            ItemError::NotYet(_) => None,
        }
    }
}

impl From<ow_defs::store::DefError> for ItemError {
    fn from(e: ow_defs::store::DefError) -> Self {
        match e {
            ow_defs::store::DefError::NotYet(n) => ItemError::NotYet(n),
            other => ItemError::Defs(other),
        }
    }
}

impl From<ow_core::NotYet> for ItemError {
    fn from(e: ow_core::NotYet) -> Self {
        ItemError::NotYet(e)
    }
}
