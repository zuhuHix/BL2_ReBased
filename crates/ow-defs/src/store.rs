//! Owner: lane W3-DEFS. Not started: this file is a scaffold stub (W0-SCAFFOLD); the owner lane fills it.

/// Placeholder for `DefStore`: object identity, lazy definition loading, overlays (architecture section 2.3). Lane W3-DEFS replaces the body and keeps the name and
/// [`DefStore::new_empty`]; until then a method a dependent needs returns `NotYet { LayerAbsent }`.
#[derive(Debug, Default)]
pub struct DefStore {
    _private: (),
}

impl DefStore {
    /// An empty value. The real type keeps this constructor, so tests written before it lands still compile.
    pub fn new_empty() -> Self {
        Self { _private: () }
    }
}
