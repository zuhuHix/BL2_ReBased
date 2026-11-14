//! Owner: lane W2-ATTRAPI. Not started: this file is a scaffold stub (W0-SCAFFOLD); the owner lane fills it.

/// Placeholder for `AttributeSystem`: the resolver registry and the evaluator entry points (architecture section 2.3). Lane W2-ATTRAPI replaces the body and keeps the name and
/// [`AttributeSystem::new_empty`]; until then a method a dependent needs returns `NotYet { LayerAbsent }`.
#[derive(Debug, Default)]
pub struct AttributeSystem {
    _private: (),
}

impl AttributeSystem {
    /// An empty value. The real type keeps this constructor, so tests written before it lands still compile.
    pub fn new_empty() -> Self {
        Self { _private: () }
    }
}
