//! Owner: lane W1-TESTKIT. Synthetic packages for Rust tests (AGENTS.md rule 9): the Rust counterparts of the
//! `package()`, `tag()` and `container()` helpers in `tests/` (header 832/46, names, imports, exports, payloads, LZO
//! containers made of literal runs, the partial-compression shape). Compiled for this crate's tests and, behind the
//! `test-support` feature, for every other crate's tests; later lanes extend it here instead of writing their own.
//!
//! Placeholder (W0-SCAFFOLD): W1-TESTKIT writes the builder and the `tag`, `container` and `partial` functions of
//! architecture section 3.2.

/// Placeholder for the synthetic package builder. W1-TESTKIT replaces the body and keeps the name and
/// [`PackageBuilder::new_empty`].
#[derive(Debug, Default)]
pub struct PackageBuilder {
    _private: (),
}

impl PackageBuilder {
    /// A builder with no names, imports or exports. The real type keeps this constructor.
    pub fn new_empty() -> Self {
        PackageBuilder { _private: () }
    }
}
