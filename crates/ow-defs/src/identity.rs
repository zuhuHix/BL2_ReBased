//! Owner: lane W3-DEFS. Object identity: which package's copy of an object path is the definition
//! ([`IdentityPolicy`], [`LoadSet`]), and every holder of a path for audits ([`Owner`], `DefStore::owners`).
//!
//! **Identity is by path.** Several package files can hold the same object path (a definition cooked into
//! `Startup.upk` and into its own package, a DLC copy). The definition store gives each object path one heap object:
//! the export of the one holder the policy picks. Every way of reaching the path (`DefStore::find`, an export
//! reference inside the holder, an import naming the path from another package) gives that same object.
//!
//! - [`IdentityPolicy::FirstInScanOrder`]: the holder is the first file **in the store's scan order**
//!   (`PackageStore::scan_order`) whose `Package::find_export` matches the path. This is the tools' convention
//!   (`tools/item_registry.py`, `tools/weapon_catalog.py` `Census`: "the first package holding a path owns it") and
//!   the order of the store's own global fallback. It is used for offline tools and parity. UNVERIFIED as a game rule.
//! - [`IdentityPolicy::Loaded`]: the holder is the first package **of the load set, in load order**, that holds the
//!   path; a path no package of the set holds is not found. This models the game, where an object already in memory
//!   is found and not loaded again, so the first loaded copy wins (decision D16: `Loaded(Startup + map + DLC)` in the
//!   world). The load order of the real game and that rule are UNVERIFIED.
//!
//! A path matches a file as the store matches it: ASCII case folded, a leading `<file stem>.` dropped
//! (`Package::find_export`).

use std::path::PathBuf;

use ow_core::PackageName;

/// The packages the game has loaded, in load order (architecture section 3.4).
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LoadSet {
    /// Package names (file stems), first loaded first. A name the store cannot load by name (no file, or several
    /// files with that stem) is an error when a lookup reaches it, never a guessed file.
    pub packages: Vec<PackageName>,
}

impl LoadSet {
    pub fn new(packages: Vec<PackageName>) -> Self {
        LoadSet { packages }
    }
}

/// Which holder of an object path is the definition (module documentation).
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum IdentityPolicy {
    /// The first holder in the store's scan order: the tools' convention, for parity. UNVERIFIED as a game rule.
    #[default]
    FirstInScanOrder,
    /// The first holder among the loaded packages, in load order. UNVERIFIED as a game rule.
    Loaded(LoadSet),
}

/// One file holding an object path (`DefStore::owners`).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Owner {
    /// The file's stem as the store names it (not checked as a [`PackageName`]: a stem can hold a `.`).
    pub package: String,
    /// The file, absolute and lexically normal, as in `PackageStore::scan_order`.
    pub file: PathBuf,
    /// The export of that file at the path.
    pub export: i32,
}

impl Owner {
    pub fn new(package: String, file: PathBuf, export: i32) -> Self {
        Owner {
            package,
            file,
            export,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_policy_is_the_tools_convention() {
        assert_eq!(IdentityPolicy::default(), IdentityPolicy::FirstInScanOrder);
        let set = LoadSet::new(vec![PackageName::parse("Startup").unwrap()]);
        assert_eq!(set.packages.len(), 1);
    }
}
