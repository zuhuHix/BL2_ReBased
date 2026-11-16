//! Owner: lane W3-DEFS. Object identity: which package's copy of an object path is the definition
//! ([`IdentityPolicy`], [`LoadSet`]), every holder of a path for audits ([`Owner`], [`Holders`], `DefStore::owners`),
//! and the census of every export of the scan ([`Census`], `DefStore::census`).
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
//!   The tools' file order is not quite the store's: they list `.upk` files only, the base game's
//!   `WillowGame/CookedPCConsole` first and then the `DLC` folders (`weapon_catalog.package_files`), while the store
//!   sorts every `.upk`, `.umap` and `.u` below its root by path. [`Census`] carries every holder with its file, so a
//!   tool that needs its own order (the registry) can apply it.
//! - [`IdentityPolicy::Loaded`]: the holder is the first package **of the load set, in load order**, that holds the
//!   path; a path no package of the set holds is not found. This models the game, where an object already in memory
//!   is found and not loaded again, so the first loaded copy wins (decision D16: `Loaded(Startup + map + DLC)` in the
//!   world). The load order of the real game and that rule are UNVERIFIED.
//!
//! A path matches a file as the store matches it: ASCII case folded, a leading `<file stem>.` dropped
//! (`Package::find_export`).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use ow_core::{FixedHashMap, PackageName};
use ow_package::PackageId;
use ow_package::package::{Package, PackageError};

use crate::store::DefError;

/// The packages the game has loaded, in load order (architecture section 3.4).
///
/// Packages are named by stem, as `PackageStore::load` names them: a stem with no file is `PackageNotFound`, and a
/// stem that two or more files share (a base package and a DLC copy of the same name) is `AmbiguousName`, both as
/// `DefError::Store` when a lookup reaches that entry. The store never picks one of several files for a stem.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LoadSet {
    /// Package names (file stems), first loaded first.
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

/// One export of one file at an object path.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Owner {
    /// The file's stem as the store names it (not checked as a [`PackageName`]: a stem can hold a `.`).
    pub package: String,
    /// The file, absolute and lexically normal, as in `PackageStore::scan_order`.
    pub file: PathBuf,
    /// The export of that file at the path.
    pub export: i32,
    /// The export's class as `ow-package --exports` prints it: the class reference's path in that file
    /// (`Package::path_of`, for example `WillowGame.WeaponPartDefinition`), `Class` for a class object (reference 0).
    pub class: String,
    /// The file's filed id when the package store holds the file loaded (`DefStore::owners` loads every holder it
    /// reports); `None` in a [`Census`], which reads each file once and keeps none of them (the installed game is
    /// tens of gigabytes of packages).
    pub id: Option<PackageId>,
}

impl Owner {
    pub fn new(
        package: String,
        file: PathBuf,
        export: i32,
        class: String,
        id: Option<PackageId>,
    ) -> Self {
        Owner {
            package,
            file,
            export,
            class,
            id,
        }
    }
}

/// A file that could not be read for a census or a lookup, with why. Reported, never dropped.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct FileFailure {
    pub file: PathBuf,
    pub error: DefError,
}

impl FileFailure {
    pub fn new(file: PathBuf, error: DefError) -> Self {
        FileFailure { file, error }
    }
}

/// Exactly what the reader's `ow-package <file> --exports` prints after `ow-package: ` for the same failure: the
/// error's text and each of its causes (`source()`), joined with `: `, with no path and no wrapper (what
/// `tools/item_registry.py` keeps as a scan error). `error`'s own `Display` is only the first part where the cause is
/// separate: a container failure prints `cannot decode the package container` there and keeps the container's text in
/// `source()`, so a chained reporter prints it once.
impl std::fmt::Display for FileFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.error)?;
        let mut cause = std::error::Error::source(&self.error);
        while let Some(c) = cause {
            write!(f, ": {c}")?;
            cause = c.source();
        }
        Ok(())
    }
}

/// Every holder of one object path, in scan order, and the candidate files that failed to load or search
/// (`DefStore::owners`).
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Holders {
    pub holders: Vec<Owner>,
    pub failures: Vec<FileFailure>,
}

/// Every export path of every file of the scan (`DefStore::census`), in the shape the tools' census builds
/// (`tools/item_registry.py` `RegistryCensus.add`, `weapon_catalog.scan`): enough to rebuild their owner choice,
/// duplicate count and conflicts in their own file order.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Census {
    /// Every file read, in scan order (the failed ones too).
    pub files: Vec<PathBuf>,
    /// Every path that is not a class default, in first-seen order (files in scan order, exports in index order), with
    /// every holder in that same order (a path a file exports twice is listed twice). Paths are grouped by their exact
    /// text as `--exports` prints it (`Package::path_of`), as the tools' dicts key them; `DefStore::find` matches folded.
    pub entries: Vec<CensusEntry>,
    /// The class default objects, kept apart: every export with a path segment starting `Default__` (the tools'
    /// `is_class_default`, which also covers `Default__X.PartList`), in scan and export order.
    pub class_defaults: Vec<CensusEntry>,
    /// Files whose export listing failed: such a file contributes no entry at all, as a failing `--exports` gives the
    /// tools no export (its error text is kept, as `scan_errors`).
    pub failures: Vec<FileFailure>,
}

/// One path of a [`Census`] and its holders.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CensusEntry {
    pub path: String,
    pub holders: Vec<Owner>,
}

impl CensusEntry {
    pub fn new(path: String, holders: Vec<Owner>) -> Self {
        CensusEntry { path, holders }
    }
}

/// One file's export paths for holder lookups that keep no package loaded: each export's path in the file, ASCII
/// folded, to the first export with that path, and the folded stem. `find` answers as `Package::find_export` does
/// (folded, a leading `<stem>.` dropped, the first match in export order). That search walks the exports in order and
/// fails at the first export whose path cannot be built (a broken outer chain): so only the exports before it are
/// listed, a match among them is found, and a path not among them is the search's error (`find` gives it back), which
/// the global fallback reads as "this file is no holder".
#[derive(Debug)]
pub(crate) struct FileIndex {
    stem: String,
    by_path: FixedHashMap<String, i32>,
    /// Why the walk stopped early: the first export whose path cannot be built.
    broken: Option<PackageError>,
}

impl FileIndex {
    /// Reads `file` and keeps only its paths, handing back the package it loaded too: a caller that needs the file
    /// loaded anyway keeps this load instead of decoding the file a second time, others drop it (lane
    /// FIX-SCENE-LOAD). A file that cannot be loaded is an error.
    pub(crate) fn read_with_package(
        file: &Path,
    ) -> Result<(FileIndex, Arc<Package>), PackageError> {
        let package = Package::load(file)?;
        let mut by_path = FixedHashMap::default();
        let mut broken = None;
        for index in 1..=package.exports.len() as i32 {
            match package.path_of(index) {
                Ok(path) => {
                    by_path.entry(path.to_ascii_lowercase()).or_insert(index);
                }
                Err(e) => {
                    broken = Some(e);
                    break;
                }
            }
        }
        let index = FileIndex {
            stem: package.name.to_ascii_lowercase(),
            by_path,
            broken,
        };
        Ok((index, package))
    }

    /// The first export at `path` (already `.`-separated), as `Package::find_export` answers it.
    pub(crate) fn find(&self, path: &str) -> Result<Option<i32>, PackageError> {
        let wanted = path.to_ascii_lowercase();
        let wanted = wanted
            .strip_prefix(&self.stem)
            .and_then(|rest| rest.strip_prefix('.'))
            .unwrap_or(&wanted);
        match (self.by_path.get(wanted), &self.broken) {
            (Some(&index), _) => Ok(Some(index)),
            (None, Some(e)) => Err(e.clone()),
            (None, None) => Ok(None),
        }
    }
}

/// `path` with every `:` read as `.`: the form `Package::find_export` and the store's indexes compare against (the
/// game writes `Outer:Subobject` for subobjects, the package tables join every outer with `.`).
pub(crate) fn dotted(path: &str) -> String {
    path.replace(':', ".")
}

// ---------------------------------------------------------------------------------------------------- path helpers

/// The identity key of an object path: ASCII case folded and `:` read as `.`, as `ObjectPath` compares paths.
pub(crate) fn path_key(path: &str) -> String {
    path.chars()
        .map(|c| {
            if c == ':' {
                '.'
            } else {
                c.to_ascii_lowercase()
            }
        })
        .collect()
}

/// A class default object or something inside one: a `.`-separated segment starting `Default__` (the tools'
/// `weapon_catalog.is_class_default`).
pub(crate) fn is_class_default(path: &str) -> bool {
    path.split('.')
        .any(|segment| segment.starts_with("Default__"))
}

/// The full object path of `reference` in `package`, as the game names the object: an import's path is already
/// full (its outer chain ends at the package it comes from). An export whose outermost export is a `Package` object
/// (a package cooked into another file, as `Startup.upk` holds `GD_*` packages) is named by its path in the file; any
/// other export is named after the file: `<file stem>.<path in the file>`. `Package::find_export` drops that leading
/// stem again, so either form finds the export in its own file.
pub fn full_path(package: &Package, reference: i32) -> Result<String, PackageError> {
    let path = package.path_of(reference)?;
    if reference <= 0 {
        return Ok(path);
    }
    // path_of has checked the chain: it ends, and within the bound.
    let mut cursor = reference;
    loop {
        let entry = package.object(cursor)?;
        if entry.outer == 0 {
            break;
        }
        if entry.outer < 0 {
            // An export inside an import: the import names the package already.
            return Ok(path);
        }
        cursor = entry.outer;
    }
    let outermost = package.object(cursor)?;
    let is_package = outermost.class != 0 && package.object(outermost.class)?.name == "Package";
    Ok(if is_package {
        path
    } else {
        format!("{}.{path}", package.name)
    })
}

/// An export's class as `ow-package --exports` prints it: the class reference's path in the file, `Class` for 0.
pub(crate) fn class_text(package: &Package, class: i32) -> Result<String, PackageError> {
    if class == 0 {
        Ok("Class".to_string())
    } else {
        package.path_of(class)
    }
}

/// An export's class as a `Package.Class` path: an imported class's path, `<file stem>.<path>` for a class the file
/// exports itself, `Core.Class` for 0.
pub(crate) fn full_class(package: &Package, class: i32) -> Result<String, PackageError> {
    match class {
        0 => Ok("Core.Class".to_string()),
        c if c < 0 => package.path_of(c),
        c => Ok(format!("{}.{}", package.name, package.path_of(c)?)),
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
