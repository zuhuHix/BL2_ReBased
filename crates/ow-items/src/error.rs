//! Owner: lane W4-REGISTRY (hub decision, review r2 of PR #310). `ItemError`, the error of the item rules
//! (architecture section 3.7).

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
