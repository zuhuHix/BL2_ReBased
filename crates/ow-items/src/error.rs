//! Owner: lane W4-REGISTRY (hub decision, review r2 of PR #310). `ItemError`, the error of the item rules
//! (architecture section 3.7). `Attr`, `Balance`, `BadData` and `WrongMode` added by W4-ROLL (hub decision, review r1
//! of PR #319): additive, the enum is `#[non_exhaustive]`.

use ow_core::ParityMode;

/// Why an item rule could not run (architecture section 3.7). The store's, the evaluator's and the balance rules'
/// errors are transparent: they print as their own error, and `source()` is its cause; a `NotYet` from any layer
/// below is [`ItemError::NotYet`].
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum ItemError {
    Defs(ow_defs::store::DefError),
    NotYet(ow_core::NotYet),
    /// An `ow-attr` error that is not a `NotYet`.
    Attr(ow_attr::api::AttrError),
    /// An `ow-balance` error that is not a `NotYet` or an evaluator error.
    Balance(ow_balance::types::BalanceError),
    /// Data the rule cannot use (a path that does not parse, an init naming an object no package holds, a number
    /// out of range where the oracle raises).
    BadData(String),
    /// A call made in a parity mode it does not serve (`roll_candidate` is Off-mode only: Reference rolls go
    /// through `roll`).
    WrongMode {
        call: &'static str,
        mode: ParityMode,
    },
}

impl std::fmt::Display for ItemError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ItemError::Defs(e) => e.fmt(f),
            ItemError::NotYet(e) => e.fmt(f),
            ItemError::Attr(e) => e.fmt(f),
            ItemError::Balance(e) => e.fmt(f),
            ItemError::BadData(reason) => write!(f, "bad item data: {reason}"),
            ItemError::WrongMode { call, mode } => write!(
                f,
                "{call} does not serve parity mode {mode:?}: Reference rolls go through roll"
            ),
        }
    }
}

impl std::error::Error for ItemError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ItemError::Defs(e) => e.source(),
            ItemError::Attr(e) => e.source(),
            ItemError::Balance(e) => e.source(),
            ItemError::NotYet(_) | ItemError::BadData(_) | ItemError::WrongMode { .. } => None,
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

impl From<ow_attr::api::AttrError> for ItemError {
    fn from(e: ow_attr::api::AttrError) -> Self {
        match e {
            ow_attr::api::AttrError::NotYet(n) => ItemError::NotYet(n),
            other => ItemError::Attr(other),
        }
    }
}

impl From<ow_balance::types::BalanceError> for ItemError {
    fn from(e: ow_balance::types::BalanceError) -> Self {
        match e {
            ow_balance::types::BalanceError::NotYet(n) => ItemError::NotYet(n),
            ow_balance::types::BalanceError::Attr(a) => a.into(),
            other => ItemError::Balance(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ow_attr::api::AttrError;
    use ow_balance::types::BalanceError;
    use ow_core::NotYet;

    #[test]
    fn a_not_yet_from_any_layer_stays_not_yet() {
        let n = NotYet::not_read("x", "r");
        assert_eq!(
            ItemError::from(AttrError::NotYet(n.clone())),
            ItemError::NotYet(n.clone())
        );
        assert_eq!(
            ItemError::from(BalanceError::Attr(AttrError::NotYet(n.clone()))),
            ItemError::NotYet(n.clone())
        );
        assert_eq!(
            ItemError::from(BalanceError::NotYet(n.clone())),
            ItemError::NotYet(n)
        );
        let bad = AttrError::BadData { reason: "y".into() };
        assert_eq!(ItemError::from(bad.clone()), ItemError::Attr(bad.clone()));
        assert_eq!(
            ItemError::from(BalanceError::Attr(bad.clone())),
            ItemError::Attr(bad)
        );
        let range = BalanceError::OutOfRange {
            what: "z",
            value: 3,
        };
        assert_eq!(ItemError::from(range.clone()), ItemError::Balance(range));
    }
}
