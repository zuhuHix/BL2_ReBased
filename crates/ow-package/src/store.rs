//! Owner: lane W2-STORE. Not started: this file is a scaffold stub (W0-SCAFFOLD); the owner lane fills it.

/// Placeholder for `PackageStore`: scan order, lazy load, import resolution and the global fallback index (architecture section 2.3). Lane W2-STORE replaces the body and keeps the name and
/// [`PackageStore::new_empty`]; until then a method a dependent needs returns `NotYet { LayerAbsent }`.
#[derive(Debug, Default)]
pub struct PackageStore {
    _private: (),
}

impl PackageStore {
    /// An empty value. The real type keeps this constructor, so tests written before it lands still compile.
    pub fn new_empty() -> Self {
        Self { _private: () }
    }
}
