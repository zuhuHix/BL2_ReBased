//! Owner: lane W1-PKG. `Package` and its loading (architecture section 3.2). `PackageId` is final from the scaffold
//! (architecture section 15); W1-PKG keeps it and writes the rest of this file.

/// The index of a package in a `PackageStore`, stable for one store.
pub type PackageId = u32;
