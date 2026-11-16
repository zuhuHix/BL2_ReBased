//! Owner: lane W2-STORE. `PackageStore`: the port of the C++ `PackageStore` (`src/package.cpp`, `src/package.hpp`):
//! the scan of a cooked package tree, lazy loading by name or path, import resolution and the global fallback index
//! (`buildGlobalIndex`, `findGlobal`), with its caches (matches, root matches, misses).
//!
//! What it does, in the C++ order:
//!
//! - [`PackageStore::scan`] walks the cooked root and every folder below it, keeps each regular file whose extension
//!   folds to `.upk`, `.umap` or `.u`, and sorts the list as `std::filesystem::path` sorts (element by element, so a
//!   separator sorts before any other character; [`scan_order`](PackageStore::scan_order)). Files are also listed by
//!   their folded stem; a name with two files is ambiguous and is refused by name, never chosen
//!   ([`paths_named`](PackageStore::paths_named) reports every candidate).
//! - [`PackageStore::resolve`] follows an import's outer chain to its root import. The root's name (or, for an import
//!   nested in a local export, the first segment of the import's path) names the package; the rest of the path is
//!   looked up there with `Package::find_export`. When that package is not a file or does not hold the path, the
//!   cached global answers are tried, then the global fallback: the first file **in scan order** whose
//!   `find_export(path)` matches, among the files the index lists for the path's first segment and the files whose
//!   roots are not known. `docs/verification/PACKAGE_GLOBAL_INDEX.md` says why the index cannot change the answer.
//! - The store chooses among packages only where the C++ does (that first-in-scan-order fallback). Which of several
//!   copies of an object is the game's is object identity policy (W3-DEFS), not decided here.
//!
//! The check oracle of the C++ (`OPENWILLOW_RESOLVE_CHECK`: each global lookup also runs the full scan and stops on a
//! difference) is [`ResolveCheck`], set by the caller ([`set_resolve_check`](PackageStore::set_resolve_check)); the
//! library never reads the environment. The lines the C++ writes to stderr under the check are collected and handed
//! to the caller ([`take_check_log`](PackageStore::take_check_log)), so the binary prints them.
//!
//! Folding is the C++ `folded()` (ASCII `tolower`, byte by byte), not `ow_core::ObjectPath`'s fold: the C++ key keeps
//! `:` and `.` apart (see `ow_core::ident`).
//!
//! `PackageId`s are positions in the store's list of loaded packages. They stay valid for the store's life: a new
//! [`scan`](PackageStore::scan) empties the caches (as the C++ does) but keeps the packages already handed out, as a
//! C++ caller's `shared_ptr` keeps its package. Equal ids mean the same package. One file can have several ids,
//! though: a load after `scan`, and a package the global fallback matched, each get a new id. For file identity,
//! compare `get(id).source`. Every package handed out stays alive for the store's lifetime.
//!
//! **Candidate files.** The global fallback keeps, per candidate file it has read (until the next
//! [`scan`](PackageStore::scan)), a 64-bit hash of each folded export path up to the first path that cannot be built,
//! with the file's size and modification time when it was read. A later lookup skips a file whose hashes do not hold
//! its path, without loading it again; the C++ loads every candidate afresh on each lookup. A hash that does hold the
//! path is confirmed by loading the file afresh (`try_file`), and that load is returned, as in the C++, so a hash
//! collision only costs a load. On the first read the load just made is searched with `find_export` itself. So the
//! answers are the C++'s, with two limits (decision 2026-11-16-63):
//! - a file whose size or modification time has changed since it was read is read again, so a rewritten file is
//!   searched as it now is (the C++ fallback and the review fix of PR #299 see rewrites too); a rewrite that keeps
//!   both the size and the modification time is not noticed, and the file is skipped by its old paths;
//! - a file that fails to load is not remembered: it is tried again on the next lookup, as the C++ tries it.
//!
//! **Global index build.** The index is built on the first global lookup by decoding every file of the scan once to
//! learn its roots. The files are decoded on worker threads (at most `INDEX_THREADS`, 16) and their roots folded into the
//! index in scan order, so the index is the one a single pass builds (lane FIX-SCENE-LOAD, decision 2026-11-16-68).
//!
//! Status: a port. The fallback rule's evidence is the C++ record (PACKAGE_GLOBAL_INDEX.md); whether the game picks
//! the same file for a path held by several packages is UNVERIFIED (object identity, W3-DEFS).

use std::cmp::Ordering;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ow_core::{DetMap, DetSet, FixedHashMap, FixedHashSet, NotYet, PackageName};

use crate::package::{Package, PackageError, PackageId, lexically_normal};

/// How many outer links `buildGlobalIndex` follows from an export before it gives up on the file's roots (the C++
/// `maxOuterChain`; the walk makes at most `MAX_OUTER_CHAIN - 1` steps).
pub const MAX_OUTER_CHAIN: usize = 256;

/// Why a store operation failed. `Display` gives the C++ exception text, which the CLI prints after `ow-package: `.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreError {
    /// "cooked package root is not a directory"
    NotADirectory,
    /// "package not found: <name>": no file of the tree has this stem (folded).
    PackageNotFound(String),
    /// "ambiguous package name: <name>": several files have this stem (folded); the store does not choose.
    AmbiguousName(String),
    /// "package has no source path for cross-package resolution": [`PackageStore::adopt`] of a package made from
    /// bytes.
    NoSourcePath,
    /// "cyclic import outer chain"
    CyclicImport,
    /// "local import has no package-qualified path": an import nested in a local export whose path has no `.`.
    NoPackageQualifiedPath,
    /// "import target not found: <package>.<object>"
    ImportNotFound {
        package: String,
        object: String,
    },
    /// "global index disagrees with the full scan: <path>": the [`ResolveCheck`] oracle found a difference.
    CheckDisagrees(String),
    /// A package failed to load or an object in it could not be named. Transparent: it prints as the package error
    /// and its `source()` is that error's cause, as the C++ rethrows the reader's exception unchanged.
    Package(PackageError),
    NotYet(NotYet),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::NotADirectory => f.write_str("cooked package root is not a directory"),
            StoreError::PackageNotFound(name) => write!(f, "package not found: {name}"),
            StoreError::AmbiguousName(name) => write!(f, "ambiguous package name: {name}"),
            StoreError::NoSourcePath => {
                f.write_str("package has no source path for cross-package resolution")
            }
            StoreError::CyclicImport => f.write_str("cyclic import outer chain"),
            StoreError::NoPackageQualifiedPath => {
                f.write_str("local import has no package-qualified path")
            }
            StoreError::ImportNotFound { package, object } => {
                write!(f, "import target not found: {package}.{object}")
            }
            StoreError::CheckDisagrees(path) => {
                write!(f, "global index disagrees with the full scan: {path}")
            }
            StoreError::Package(e) => e.fmt(f),
            StoreError::NotYet(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for StoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            StoreError::Package(e) => e.source(),
            _ => None,
        }
    }
}

impl From<PackageError> for StoreError {
    fn from(e: PackageError) -> Self {
        StoreError::Package(e)
    }
}

/// The C++ `OPENWILLOW_RESOLVE_CHECK` oracle, as a value the caller sets: every global lookup also runs the full scan
/// of the old fallback and fails with [`StoreError::CheckDisagrees`] on any difference in file or index.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolveCheck {
    /// The test seam `OPENWILLOW_RESOLVE_CHECK_FAIL_BUILD`: the file with this exact stem fails to load during the
    /// index pass only, as a passing failure would (`tests/global_resolve_test.py`).
    pub fail_in_build: Option<String>,
}

/// The global fallback's candidates: positions in the scan order, ascending in each list.
#[derive(Debug, Default)]
struct GlobalIndex {
    /// A path's first segment, folded -> the files that could hold a path starting with it.
    by_root: FixedHashMap<String, Vec<u32>>,
    /// Files whose roots are not known: a candidate for every path.
    any_root: Vec<u32>,
}

/// The package store over one cooked package tree (architecture section 3.2).
#[derive(Debug, Default)]
pub struct PackageStore {
    root: PathBuf,
    /// Every package file of the tree, in the C++ sort order.
    indexed: Vec<PathBuf>,
    /// Folded stem -> its files, each list in the same order.
    by_name: DetMap<String, Vec<PathBuf>>,
    /// Every package handed out, by `PackageId`; never shrinks.
    packages: Vec<Arc<Package>>,
    /// Folded generic form of a normal absolute path -> its loaded package.
    loaded: DetMap<String, PackageId>,
    global_matches: DetMap<String, (PathBuf, i32)>,
    global_root_matches: DetMap<String, PathBuf>,
    global_misses: DetSet<String>,
    global_index: Option<GlobalIndex>,
    /// Scan position -> what the global fallback read of that file (module documentation, "Candidate files").
    global_files: FixedHashMap<u32, FileExports>,
    /// File stem -> how many times the global fallback loaded it (`find_global`), for the tests.
    #[cfg(test)]
    global_file_loads: FixedHashMap<String, usize>,
    check: Option<ResolveCheck>,
    check_log: Vec<String>,
}

impl PackageStore {
    /// An empty store: no root, nothing scanned. Resolving an import in it finds nothing.
    pub fn new_empty() -> Self {
        Self::default()
    }

    /// A store over `cooked_root`, made absolute and lexically normal as the C++ constructor does. Unlike the C++
    /// constructor it does not scan: call [`scan`](Self::scan), or use [`open`](Self::open).
    pub fn new(cooked_root: PathBuf) -> Self {
        let root = std::path::absolute(&cooked_root).unwrap_or(cooked_root);
        PackageStore {
            root: lexically_normal(&root),
            ..Self::default()
        }
    }

    /// [`new`](Self::new) then [`scan`](Self::scan): the C++ constructor.
    pub fn open(cooked_root: PathBuf) -> Result<Self, StoreError> {
        let mut store = Self::new(cooked_root);
        store.scan()?;
        Ok(store)
    }

    /// The cooked root, absolute and lexically normal.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Sets or clears the check oracle (C++ `OPENWILLOW_RESOLVE_CHECK`).
    pub fn set_resolve_check(&mut self, check: Option<ResolveCheck>) {
        self.check = check;
    }

    /// The lines the check oracle wrote since the last call (`OWRESOLVECHECK ...`, without a line end), in order:
    /// what the C++ writes to stderr.
    pub fn take_check_log(&mut self) -> Vec<String> {
        std::mem::take(&mut self.check_log)
    }

    /// Lists the tree again (C++ `scan`): every regular file below the root whose extension folds to `.upk`,
    /// `.umap` or `.u`. Empties the caches and the global index; packages already handed out keep their ids.
    ///
    /// A folder or entry that cannot be read is skipped and the walk goes on. The C++ iterator
    /// (`skip_permission_denied`) skips only permission errors; on any other error it ends the whole walk.
    /// UNVERIFIED: how far that differs in practice. Symbolic links to files count as files; links to folders are
    /// not followed (the C++ iterator's default). Junctions were not checked (UNVERIFIED).
    pub fn scan(&mut self) -> Result<(), StoreError> {
        if !std::fs::metadata(&self.root).is_ok_and(|m| m.is_dir()) {
            return Err(StoreError::NotADirectory);
        }
        self.indexed.clear();
        self.by_name.clear();
        self.loaded.clear();
        self.global_matches.clear();
        self.global_root_matches.clear();
        self.global_misses.clear();
        self.global_index = None;
        self.global_files.clear();
        let mut found = Vec::new();
        walk(&self.root, &mut found);
        for path in found {
            let path = lexically_normal(&path);
            let stem = path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            self.by_name
                .entry(fold(&stem))
                .or_default()
                .push(path.clone());
            self.indexed.push(path);
        }
        self.indexed.sort_by(|a, b| path_order(a, b));
        for paths in self.by_name.values_mut() {
            paths.sort_by(|a, b| path_order(a, b));
        }
        Ok(())
    }

    /// Every package file of the tree, in the order the global fallback tries them.
    pub fn scan_order(&self) -> &[PathBuf] {
        &self.indexed
    }

    /// How many package files the scan found (C++ `indexedPackageCount`).
    pub fn indexed_package_count(&self) -> usize {
        self.indexed.len()
    }

    /// Every file whose stem folds to `name`'s fold, in scan order: all the candidates for a package name. Empty
    /// when there is none; more than one means [`load`](Self::load) refuses the name as ambiguous.
    pub fn paths_named(&self, name: &str) -> &[PathBuf] {
        self.by_name
            .get(&fold(name))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Loads the one file named `name` (C++ `loadPackage`): an error when no file or several files have that stem.
    pub fn load(&mut self, name: &PackageName) -> Result<PackageId, StoreError> {
        self.load_named(name.as_str())
    }

    /// Loads the package file at `path`, or returns the one already loaded from it (C++ `loadPath`). The cache key
    /// is the absolute, lexically normal path with `/` separators, ASCII case folded.
    pub fn load_path(&mut self, path: &Path) -> Result<PackageId, StoreError> {
        let normal = normal_absolute(path);
        let key = path_key(&normal);
        if let Some(&id) = self.loaded.get(&key) {
            return Ok(id);
        }
        let package = Package::load(&normal)?;
        Ok(self.insert(key, package))
    }

    /// Puts a package that is already loaded into the store under its `source` path. It is meant for a package from
    /// `Package::load`, whose `source` is the real file. If that path is already loaded, the loaded package wins:
    /// its id is returned and `package` is dropped. Where the C++ resolves from a package it was handed
    /// (`resolve(const Package&)`), it calls `loadPath` on the source path, which also returns the cached package.
    /// When the path is not loaded yet, this keeps the package given, which has the same bytes unless the file
    /// changed.
    pub fn adopt(&mut self, package: Arc<Package>) -> Result<PackageId, StoreError> {
        if package.source.as_os_str().is_empty() {
            return Err(StoreError::NoSourcePath);
        }
        let key = path_key(&normal_absolute(&package.source));
        if let Some(&id) = self.loaded.get(&key) {
            return Ok(id);
        }
        Ok(self.insert(key, package))
    }

    /// The package with this id.
    ///
    /// # Panics
    /// When `id` was not handed out by this store (a caller bug, not a data error).
    pub fn get(&self, id: PackageId) -> &Package {
        &self.packages[id as usize]
    }

    /// The package with this id, shared (for threaded tools, D18). Panics as [`get`](Self::get).
    pub fn shared(&self, id: PackageId) -> Arc<Package> {
        Arc::clone(&self.packages[id as usize])
    }

    /// Resolves `reference` of package `from` (C++ `PackageStore::resolve`): `None` for 0; an export (> 0) is checked
    /// and is itself; an import (< 0) is followed to the package and export that define it. An import whose target
    /// is found nowhere is [`StoreError::ImportNotFound`].
    pub fn resolve(
        &mut self,
        from: PackageId,
        reference: i32,
    ) -> Result<Option<(PackageId, i32)>, StoreError> {
        if reference == 0 {
            return Ok(None);
        }
        let source = self.shared(from);
        if reference > 0 {
            source.object(reference)?;
            return Ok(Some((from, reference)));
        }
        self.resolve_import(&source, reference).map(Some)
    }

    /// The export at `path` in the one file named `package` (C++ `resolvePath`). `None` where the package has no
    /// such export; the C++ throws "object not found: <package>.<path>" there.
    pub fn resolve_path(
        &mut self,
        package: &PackageName,
        path: &str,
    ) -> Result<Option<(PackageId, i32)>, StoreError> {
        let id = self.load(package)?;
        Ok(self.get(id).find_export(path)?.map(|index| (id, index)))
    }

    fn insert(&mut self, key: String, package: Arc<Package>) -> PackageId {
        let id = self.push(package);
        self.loaded.insert(key, id);
        id
    }

    /// A new id for `package`, outside the path cache.
    fn push(&mut self, package: Arc<Package>) -> PackageId {
        let id = self.packages.len() as PackageId;
        self.packages.push(package);
        id
    }

    fn load_named(&mut self, name: &str) -> Result<PackageId, StoreError> {
        let path = match self.by_name.get(&fold(name)) {
            None => return Err(StoreError::PackageNotFound(name.to_string())),
            Some(paths) if paths.len() != 1 => {
                return Err(StoreError::AmbiguousName(name.to_string()));
            }
            Some(paths) => paths[0].clone(),
        };
        self.load_path(&path)
    }

    /// C++ `resolveLoaded`, step for step.
    fn resolve_import(
        &mut self,
        source: &Package,
        reference: i32,
    ) -> Result<(PackageId, i32), StoreError> {
        source.object(reference)?;
        let mut root_reference = reference;
        let mut nested_in_local_export = false;
        let mut seen: FixedHashSet<i32> = FixedHashSet::default();
        loop {
            if !seen.insert(root_reference) {
                return Err(StoreError::CyclicImport);
            }
            let outer = source.object(root_reference)?.outer;
            if outer == 0 {
                break;
            }
            if outer > 0 {
                nested_in_local_export = true;
                break;
            }
            root_reference = outer;
        }

        let root_name = source.object(root_reference)?.name.clone();
        let global_path = source.path_of(reference)?;
        let package_name = if nested_in_local_export {
            match global_path.find('.') {
                Some(separator) => global_path[..separator].to_string(),
                None => return Err(StoreError::NoPackageQualifiedPath),
            }
        } else {
            root_name
        };
        let root_prefix = format!("{package_name}.");
        let object_path = if starts_with_folded(&global_path, &root_prefix) {
            global_path[root_prefix.len()..].to_string()
        } else {
            global_path.clone()
        };

        let mut target: Option<PackageId> = None;
        let mut target_index = 0;
        if self.by_name.contains_key(&fold(&package_name)) {
            let id = self.load_named(&package_name)?;
            target = Some(id);
            target_index = self.get(id).find_export(&object_path)?.unwrap_or(0);
        }
        // Some cooked imports are rooted at a package object name that is not a file name: the global answers.
        let global_key = fold(&global_path);
        if target_index == 0 {
            if let Some((path, index)) = self.global_matches.get(&global_key).cloned() {
                target = Some(self.load_path(&path)?);
                target_index = index;
            }
        }
        if target_index == 0 {
            if let Some(path) = self.global_root_matches.get(&fold(&package_name)).cloned() {
                let id = self.load_path(&path)?;
                target = Some(id);
                target_index = self.get(id).find_export(&object_path)?.unwrap_or(0);
                if target_index == 0 {
                    target_index = self.get(id).find_export(&global_path)?.unwrap_or(0);
                }
            }
        }
        if target_index == 0 && !self.global_misses.contains(&global_key) {
            let found = self.find_global(&global_path);
            if self.check.is_some() {
                let scanned = self.scan_global(&global_path);
                let same = match (&scanned, &found) {
                    (None, None) => true,
                    (Some((a, ai)), Some((b, bi))) => ai == bi && a.source == b.source,
                    _ => false,
                };
                if !same {
                    return Err(StoreError::CheckDisagrees(global_path));
                }
                let (name, index) = match &found {
                    Some((package, index)) => (package.name.as_str(), *index),
                    None => ("none", 0),
                };
                self.check_log.push(format!(
                    "OWRESOLVECHECK same {global_path} -> {name} {index}"
                ));
            }
            match found {
                Some((package, index)) => {
                    // The exact package the lookup matched, under an id of its own: the C++ returns this fresh
                    // candidate and does not put it in the path cache, so neither does this (a copy loaded from the
                    // same path earlier may hold older bytes, and the index belongs to these).
                    let path = package.source.clone();
                    let id = self.push(package);
                    target = Some(id);
                    target_index = index;
                    self.global_matches
                        .insert(global_key, (path.clone(), index));
                    self.global_root_matches.insert(fold(&package_name), path);
                }
                None => {
                    self.global_misses.insert(global_key);
                }
            }
        }
        match target {
            Some(id) if target_index != 0 => Ok((id, target_index)),
            _ => Err(StoreError::ImportNotFound {
                package: package_name,
                object: object_path,
            }),
        }
    }

    /// C++ `buildGlobalIndex`: which files could answer which paths, from one pass over the tree. Published only when
    /// complete (it cannot fail part way here: every per-file failure makes the file a candidate for every path).
    fn build_global_index(&mut self) {
        let mut by_root: FixedHashMap<String, Vec<u32>> = FixedHashMap::default();
        let mut any_root = Vec::new();
        let fail_in_build = self.check.as_ref().and_then(|c| c.fail_in_build.clone());
        // Each file's roots, read on worker threads (lane FIX-SCENE-LOAD: the pass decodes every package of the tree,
        // which dominated a scene load), then folded here in scan order, as the single pass did: the index is the
        // same whatever the threads' order.
        let roots = file_roots(&self.indexed, fail_in_build.as_deref());
        for (position, roots) in roots.into_iter().enumerate() {
            let position = position as u32;
            match roots {
                Some(roots) => {
                    for root in roots {
                        by_root.entry(root).or_default().push(position);
                    }
                }
                None => any_root.push(position),
            }
        }
        if self.check.is_some() {
            let entries: usize = by_root.values().map(Vec::len).sum();
            self.check_log.push(format!(
                "OWRESOLVECHECK index files={} roots={} entries={entries} any_root={}",
                self.indexed.len(),
                by_root.len(),
                any_root.len()
            ));
        }
        self.global_index = Some(GlobalIndex { by_root, any_root });
    }

    /// C++ `findGlobal`: the first file in scan order that has the path, among the files that can have it. A file
    /// that fails to load or to search is no candidate.
    ///
    /// A candidate's export paths are kept as hashes (`FileExports`), so later lookups skip a file whose paths do not
    /// hold `global_path` without loading it again. A file is loaded afresh where its hashes hold the path (a hash
    /// collision costs one extra load), where it failed to load before (it is retried, as the C++ does), and where its
    /// size or modification time changed; the package returned is that fresh load, as the C++ returns its fresh
    /// candidate (module documentation, "Candidate files", review r2 of PR #362).
    fn find_global(&mut self, global_path: &str) -> Option<(Arc<Package>, i32)> {
        if self.global_index.is_none() {
            self.build_global_index();
        }
        let index = self.global_index.as_ref()?;
        let rooted = index
            .by_root
            .get(&first_segment(global_path))
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        for position in merge(rooted, &index.any_root) {
            let path = self.indexed[position as usize].clone();
            let stamp = file_stamp(&path);
            if let Some(file) = self
                .global_files
                .get(&position)
                .filter(|f| stamp.is_some() && f.stamp == stamp)
            {
                if !file.may_hold(global_path) {
                    continue;
                }
                // A hash that holds the path: the file loaded afresh says whether it does (a collision does not).
                self.count_load(&path);
                if let Some(found) = try_file(&path, global_path) {
                    return Some(found);
                }
                continue;
            }
            self.count_load(&path);
            let Ok(package) = Package::load(&path) else {
                // Not remembered: the next lookup tries the file again, as the C++ does.
                self.global_files.remove(&position);
                continue;
            };
            let found = package.find_export(global_path).ok().flatten();
            self.global_files
                .insert(position, FileExports::read(&package, stamp));
            if let Some(index) = found {
                return Some((package, index));
            }
        }
        None
    }

    /// Counts a load of `path` by the global fallback (tests only).
    #[cfg(test)]
    fn count_load(&mut self, path: &Path) {
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        *self.global_file_loads.entry(stem).or_default() += 1;
    }

    #[cfg(not(test))]
    fn count_load(&mut self, _path: &Path) {}

    /// The files that could hold `global_path`, in scan order: the candidates [`find_global`](Self::find_global) tries,
    /// from the global index (built on first use, as for the global fallback): the files whose roots include the
    /// path's first segment and the files whose roots are not known, merged. The first of them whose
    /// `Package::find_export(global_path)` matches is the global fallback's answer; a file not listed holds no export
    /// at that path (`docs/verification/PACKAGE_GLOBAL_INDEX.md`). Nothing is loaded into the store and no cache
    /// changes. Added for W3-DEFS (granted): the definition store's `FirstInScanOrder` lookup and `owners()` use it,
    /// so there is one global index.
    pub fn global_candidates(&mut self, global_path: &str) -> Vec<PathBuf> {
        if self.global_index.is_none() {
            self.build_global_index();
        }
        let Some(index) = self.global_index.as_ref() else {
            return Vec::new();
        };
        let rooted = index
            .by_root
            .get(&first_segment(global_path))
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        merge(rooted, &index.any_root)
            .into_iter()
            .map(|position| self.indexed[position as usize].clone())
            .collect()
    }

    /// C++ `scanGlobal`: the fallback before the index, every file in order. Only the check oracle runs it.
    fn scan_global(&self, global_path: &str) -> Option<(Arc<Package>, i32)> {
        self.indexed
            .iter()
            .find_map(|path| try_file(path, global_path))
    }
}

/// What the global fallback keeps of a candidate file (module documentation, "Candidate files").
#[derive(Debug)]
struct FileExports {
    /// `<package name>.`, folded: the prefix `find_export` strips.
    prefix: String,
    /// The hash of each export's folded path, up to the first export whose path cannot be built (`find_export` fails
    /// there unless an earlier export matched).
    hashes: FixedHashSet<u64>,
    /// The file's size and modification time when it was read; `None` when they could not be read (the file is then
    /// read again on every lookup).
    stamp: Option<FileStamp>,
}

impl FileExports {
    fn read(package: &Package, stamp: Option<FileStamp>) -> FileExports {
        let mut hashes: FixedHashSet<u64> = FixedHashSet::default();
        for index in 1..=package.exports.len() as i32 {
            match package.path_of(index) {
                Ok(path) => {
                    hashes.insert(path_hash(&fold(&path)));
                }
                Err(_) => break,
            }
        }
        FileExports {
            prefix: fold(&format!("{}.", package.name)),
            hashes,
            stamp,
        }
    }

    /// Whether `Package::find_export(global_path)` may match in this file: `false` only where it gives `Ok(None)` or an
    /// error (both "not here" to the global fallback); `true` may be a hash collision.
    fn may_hold(&self, global_path: &str) -> bool {
        let wanted = fold(global_path);
        let wanted = wanted.strip_prefix(&self.prefix).unwrap_or(&wanted);
        self.hashes.contains(&path_hash(wanted))
    }
}

/// A file's size and modification time.
type FileStamp = (u64, std::time::SystemTime);

fn file_stamp(path: &Path) -> Option<FileStamp> {
    let meta = std::fs::metadata(path).ok()?;
    Some((meta.len(), meta.modified().ok()?))
}

/// A fixed 64-bit hash of a folded path (the standard library's `DefaultHasher` with its fixed keys).
fn path_hash(text: &str) -> u64 {
    use std::hash::Hasher as _;
    let mut h = std::hash::DefaultHasher::new();
    h.write(text.as_bytes());
    h.finish()
}

/// Loads `path` and looks for `global_path` in it; any failure is "not here".
fn try_file(path: &Path, global_path: &str) -> Option<(Arc<Package>, i32)> {
    let package = Package::load(path).ok()?;
    let index = package.find_export(global_path).ok()??;
    Some((package, index))
}

/// The roots ([`roots_of`]) of every file of `files`, by scan position: `None` for a file that does not load, whose
/// roots are not known, or whose stem is the check's `fail_in_build` seam. The files are split among worker threads
/// (at most [`INDEX_THREADS`]); each loads its files one at a time and keeps only their roots.
fn file_roots(files: &[PathBuf], fail_in_build: Option<&str>) -> Vec<Option<FixedHashSet<String>>> {
    let roots_of_file = |path: &PathBuf| {
        let seam = fail_in_build.is_some_and(|stem| {
            path.file_stem()
                .is_some_and(|s| s.to_string_lossy() == stem)
        });
        if seam {
            return None;
        }
        Package::load(path)
            .ok()
            .and_then(|package| roots_of(&package))
    };
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .clamp(1, INDEX_THREADS);
    let per_thread = files.len().div_ceil(threads).max(1);
    std::thread::scope(|scope| {
        let workers: Vec<_> = files
            .chunks(per_thread)
            .map(|chunk| scope.spawn(move || chunk.iter().map(roots_of_file).collect::<Vec<_>>()))
            .collect();
        workers
            .into_iter()
            // A worker that panicked read nothing usable: its files count as unknown roots, as a load failure does.
            .zip(files.chunks(per_thread))
            .flat_map(|(worker, chunk)| worker.join().unwrap_or_else(|_| vec![None; chunk.len()]))
            .collect()
    })
}

/// The most worker threads [`PackageStore`]'s global index build uses.
const INDEX_THREADS: usize = 16;

/// The folded first segments a path matched in this package can start with: its stem's, and each export's
/// outermost object's. `None` when they are not known: an outer chain that fails or does not end within the bound,
/// or an empty outermost name (`Package::path_of` drops it, so the real path starts at a child).
fn roots_of(package: &Package) -> Option<FixedHashSet<String>> {
    let mut roots: FixedHashSet<String> = FixedHashSet::default();
    roots.insert(first_segment(&package.name));
    for index in 1..=package.exports.len() as i32 {
        let mut cursor = index;
        let mut steps = 0;
        loop {
            let outer = package.object(cursor).ok()?.outer;
            if outer == 0 {
                break;
            }
            steps += 1;
            if steps >= MAX_OUTER_CHAIN {
                break;
            }
            cursor = outer;
        }
        let outermost = package.object(cursor).ok()?;
        if outermost.outer != 0 || outermost.name.is_empty() {
            return None;
        }
        roots.insert(first_segment(&outermost.name));
    }
    Some(roots)
}

/// `std::merge` of two ascending lists (duplicates kept, the first list's first on a tie).
fn merge(a: &[u32], b: &[u32]) -> Vec<u32> {
    let mut out = Vec::with_capacity(a.len() + b.len());
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        if b[j] < a[i] {
            out.push(b[j]);
            j += 1;
        } else {
            out.push(a[i]);
            i += 1;
        }
    }
    out.extend_from_slice(&a[i..]);
    out.extend_from_slice(&b[j..]);
    out
}

/// Every file below `dir` with a package extension. Unreadable folders and entries are skipped.
fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        if kind.is_dir() {
            walk(&path, out);
            continue;
        }
        let regular = if kind.is_symlink() {
            std::fs::metadata(&path).is_ok_and(|m| m.is_file())
        } else {
            kind.is_file()
        };
        if regular && has_package_extension(&path) {
            out.push(path);
        }
    }
}

fn has_package_extension(path: &Path) -> bool {
    let Some(extension) = path.extension() else {
        return false;
    };
    let extension = fold(&extension.to_string_lossy());
    extension == "upk" || extension == "umap" || extension == "u"
}

/// The C++ `folded()`: ASCII lower case, byte by byte (`std::tolower` in the C locale).
fn fold(text: &str) -> String {
    text.to_ascii_lowercase()
}

/// The first segment of an object path (up to the first `.`), folded (C++ `firstSegment`).
fn first_segment(path: &str) -> String {
    fold(path.split('.').next().unwrap_or_default())
}

fn starts_with_folded(text: &str, prefix: &str) -> bool {
    text.len() >= prefix.len()
        && text.as_bytes()[..prefix.len()].eq_ignore_ascii_case(prefix.as_bytes())
}

fn normal_absolute(path: &Path) -> PathBuf {
    lexically_normal(&std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf()))
}

/// The C++ `loadPath` cache key: `key(normalized.generic_string())`, the path with `/` separators, folded.
fn path_key(normal: &Path) -> String {
    let text = normal.to_string_lossy();
    let generic = if cfg!(windows) {
        text.replace('\\', "/")
    } else {
        text.into_owned()
    };
    fold(&generic)
}

/// The order of `std::filesystem::path::operator<` for the paths of one tree: element by element, where a run of
/// separators sorts before any other character and each element compares by code unit (UTF-16 on Windows, as MSVC
/// compares its `wchar_t` text; bytes elsewhere). Root names are compared as plain text, which is exact for paths
/// under one root (every path a scan produces).
fn path_order(a: &Path, b: &Path) -> Ordering {
    units(a).cmp(&units(b))
}

/// The path's code units with each run of separators as one 0 and every other unit shifted up by one.
fn units(path: &Path) -> Vec<u32> {
    let raw: Vec<u32> = {
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            path.as_os_str().encode_wide().map(u32::from).collect()
        }
        #[cfg(not(windows))]
        {
            path.as_os_str()
                .as_encoded_bytes()
                .iter()
                .map(|&b| u32::from(b))
                .collect()
        }
    };
    let is_sep = |u: u32| u == u32::from(b'/') || (cfg!(windows) && u == u32::from(b'\\'));
    let mut out = Vec::with_capacity(raw.len());
    for u in raw {
        if is_sep(u) {
            if out.last() != Some(&0) {
                out.push(0);
            }
        } else {
            out.push(u + 1);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::{Export, PackageBuilder};

    /// A temporary folder for one test's invented packages, removed on drop.
    struct Tree(PathBuf);

    impl Tree {
        fn new(tag: &str) -> Tree {
            let dir = std::env::temp_dir()
                .join(format!("ow-package-w2store-{}-{tag}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Tree(dir)
        }
        fn write(&self, relative: &str, bytes: &[u8]) -> PathBuf {
            let path = self.0.join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, bytes).unwrap();
            path
        }
        fn store(&self) -> PackageStore {
            PackageStore::open(self.0.clone()).unwrap()
        }
    }

    impl Drop for Tree {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// `global_resolve_test.py`'s `package()`: imports (class package, class, outer, name), exports (name, outer).
    fn package(imports: &[(&str, &str, i32, &str)], exports: &[(&str, i32)]) -> Vec<u8> {
        let mut b = PackageBuilder::new_empty();
        for &(class_package, class, outer, name) in imports {
            b.add_import(class_package, class, outer, name);
        }
        for &(name, outer) in exports {
            let mut e = Export::new(0, name, vec![]);
            e.outer = outer;
            b.add_export_with(e);
        }
        b.build()
    }

    /// The import `GDX.Obj` (-2), rooted at a package `GDX` that no file is called.
    const GDX_OBJ: &[(&str, &str, i32, &str)] =
        &[("Core", "Package", 0, "GDX"), ("Core", "Object", -1, "Obj")];
    const HOLDS: &[(&str, i32)] = &[("GDX", 0), ("Obj", 1)];

    fn resolve_in(
        tree: &Tree,
        store: &mut PackageStore,
        reference: i32,
    ) -> Result<(String, i32), StoreError> {
        let source = store.load_path(&tree.0.join("Source.upk")).unwrap();
        let (id, index) = store.resolve(source, reference)?.unwrap();
        Ok((store.get(id).name.clone(), index))
    }

    fn checked(tree: &Tree, reference: i32) -> (Result<(String, i32), StoreError>, Vec<String>) {
        let mut store = tree.store();
        store.set_resolve_check(Some(ResolveCheck::default()));
        let result = resolve_in(tree, &mut store, reference);
        (result, store.take_check_log())
    }

    #[test]
    fn resolves_an_import_to_the_named_package() {
        let tree = Tree::new("named");
        tree.write("B.upk", &package(&[], &[("Target", 0)]));
        tree.write(
            "A.upk",
            &package(
                &[
                    ("Core", "Package", 0, "B"),
                    ("Core", "Object", -1, "Target"),
                ],
                &[],
            ),
        );
        let mut store = tree.store();
        assert_eq!(store.indexed_package_count(), 2);
        let a = store.load_path(&tree.0.join("A.upk")).unwrap();
        let (b, index) = store.resolve(a, -2).unwrap().unwrap();
        assert_eq!((store.get(b).name.as_str(), index), ("B", 1));
        assert_eq!(store.resolve(a, 0).unwrap(), None);
        // The same file twice is one package.
        assert_eq!(
            store
                .load_path(&tree.0.join("sub").join("..").join("a.UPK"))
                .unwrap(),
            a
        );
        assert_eq!(store.load(&PackageName::parse("b").unwrap()).unwrap(), b);
        assert_eq!(
            store
                .resolve_path(&PackageName::parse("B").unwrap(), "b.target")
                .unwrap(),
            Some((b, 1))
        );
        assert_eq!(
            store
                .resolve_path(&PackageName::parse("B").unwrap(), "Nothing")
                .unwrap(),
            None
        );
    }

    /// `runtime_test.py`'s nested case: an import whose outer is a local export takes its package from its path.
    #[test]
    fn an_import_in_a_local_export_takes_the_package_from_its_path() {
        let tree = Tree::new("nested");
        tree.write("B.upk", &package(&[], &[("Target", 0)]));
        tree.write(
            "Source.upk",
            &package(&[("Core", "Object", 1, "Target")], &[("B", 0)]),
        );
        let mut store = tree.store();
        assert_eq!(resolve_in(&tree, &mut store, -1).unwrap(), ("B".into(), 1));
        // A positive reference is itself, once checked.
        let source = store.load_path(&tree.0.join("Source.upk")).unwrap();
        assert_eq!(store.resolve(source, 1).unwrap(), Some((source, 1)));
        assert!(matches!(
            store.resolve(source, 2),
            Err(StoreError::Package(_))
        ));
        assert!(matches!(
            store.resolve(source, -5),
            Err(StoreError::Package(_))
        ));
    }

    #[test]
    fn a_local_import_without_a_dot_is_refused() {
        let tree = Tree::new("nodot");
        // Export 1 has an empty name, so the import's path is just "Target".
        tree.write(
            "Source.upk",
            &package(&[("Core", "Object", 1, "Target")], &[("", 0)]),
        );
        let mut store = tree.store();
        assert_eq!(
            resolve_in(&tree, &mut store, -1),
            Err(StoreError::NoPackageQualifiedPath)
        );
    }

    #[test]
    fn a_cyclic_import_chain_is_refused() {
        let tree = Tree::new("cycle");
        tree.write(
            "Source.upk",
            &package(
                &[("Core", "Object", -2, "A"), ("Core", "Object", -1, "B")],
                &[],
            ),
        );
        let mut store = tree.store();
        assert_eq!(
            resolve_in(&tree, &mut store, -1),
            Err(StoreError::CyclicImport)
        );
    }

    /// The cases of `tests/global_resolve_test.py`, with the check oracle on.
    #[test]
    fn the_global_fallback_takes_the_first_file_in_scan_order() {
        let other: &[(&str, i32)] = &[("Unrelated", 0), ("Obj", 1)];
        let tree = Tree::new("copies");
        tree.write("Source.upk", &package(GDX_OBJ, &[]));
        tree.write("Mid.upk", &package(&[], HOLDS));
        tree.write("Zeta.upk", &package(&[], HOLDS));
        tree.write("Other.upk", &package(&[], other));
        let (result, log) = checked(&tree, -2);
        assert_eq!(result.unwrap(), ("Mid".into(), 2));
        assert_eq!(log.last().unwrap(), "OWRESOLVECHECK same GDX.Obj -> Mid 2");
        assert!(
            log[0].starts_with("OWRESOLVECHECK index files=4 "),
            "{log:?}"
        );

        let tree = Tree::new("folded");
        tree.write("Source.upk", &package(GDX_OBJ, &[]));
        tree.write("Later.upk", &package(&[], HOLDS));
        tree.write("Early.upk", &package(&[], &[("gdx", 0), ("OBJ", 1)]));
        assert_eq!(checked(&tree, -2).0.unwrap(), ("Early".into(), 2));

        let tree = Tree::new("dotted");
        tree.write("Source.upk", &package(GDX_OBJ, &[]));
        tree.write("Dotted.upk", &package(&[], &[("GDX.Obj", 0)]));
        tree.write("Later.upk", &package(&[], HOLDS));
        assert_eq!(checked(&tree, -2).0.unwrap(), ("Dotted".into(), 1));
    }

    /// Lane FIX-PKG-LOOKUP (decision 2026-11-16-63): the global fallback reads each candidate file once per store, and
    /// answers as the full scan does (the check oracle runs on every lookup here).
    #[test]
    fn the_global_fallback_reads_each_candidate_file_once() {
        let tree = Tree::new("once");
        tree.write(
            "Source.upk",
            &package(
                &[
                    ("Core", "Package", 0, "GDX"),
                    ("Core", "Object", -1, "ObjA"),
                    ("Core", "Object", -1, "ObjB"),
                    ("Core", "Object", -1, "ObjC"),
                    ("Core", "Object", -1, "Missing"),
                    ("Core", "Object", -1, "Extra"),
                    ("Core", "Object", -1, "Gone"),
                ],
                &[],
            ),
        );
        // Every file below holds the root GDX, so each is a candidate for every path; Broken's roots are unknown.
        tree.write("Afile.upk", &package(&[], &[("GDX", 0), ("Other", 1)]));
        tree.write(
            "Bfile.upk",
            &package(&[], &[("GDX", 0), ("ObjA", 1), ("Extra", 1)]),
        );
        tree.write("Cfile.upk", &package(&[], &[("GDX", 0), ("ObjB", 1)]));
        tree.write(
            "Dfile.upk",
            &package(&[], &[("GDX", 0), ("ObjC", 1), ("objc", 1)]),
        );
        tree.write("Broken.upk", &[1, 2, 3]);
        let mut store = tree.store();
        store.set_resolve_check(Some(ResolveCheck::default()));
        let mut answers = Vec::new();
        let mut loads = Vec::new();
        for reference in [-2, -3, -4, -5, -6, -7, -2] {
            answers.push(resolve_in(&tree, &mut store, reference).map_err(|e| e.to_string()));
            loads.push(store.global_file_loads.values().sum::<usize>());
        }
        let found = |name: &str, index| Ok((name.to_string(), index));
        let missing = |object: &str| {
            Err(StoreError::ImportNotFound {
                package: "GDX".into(),
                object: object.into(),
            }
            .to_string())
        };
        assert_eq!(
            answers,
            [
                found("Bfile", 2),
                found("Cfile", 2),
                // The first of two exports that fold to the same path.
                found("Dfile", 2),
                missing("Missing"),
                found("Bfile", 3),
                missing("Gone"),
                found("Bfile", 2),
            ]
        );
        // The check oracle's full scan agreed on every global lookup (a disagreement is an error above).
        let log = store.take_check_log();
        assert_eq!(
            log.iter()
                .filter(|l| l.starts_with("OWRESOLVECHECK same"))
                .count(),
            6
        );
        // Scan order: Afile, Bfile, Broken, Cfile, Dfile. ObjA reads Afile and Bfile (a match). ObjB skips both, tries
        // Broken (it does not load, and is not remembered) and reads Cfile; ObjC tries Broken again and reads Dfile;
        // each readable file once. The miss and Gone only try Broken again. Extra's hash is in Bfile, read before:
        // that one file is loaded afresh to confirm it, as the C++ returns a fresh candidate. ObjA again is the cached
        // match.
        assert_eq!(loads, [2, 4, 6, 7, 8, 9, 9]);
        let per_file: std::collections::BTreeMap<_, _> =
            store.global_file_loads.clone().into_iter().collect();
        let expected: std::collections::BTreeMap<String, usize> = [
            ("Afile", 1),
            ("Bfile", 2),
            ("Broken", 4),
            ("Cfile", 1),
            ("Dfile", 1),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
        assert_eq!(per_file, expected);
        assert_eq!(store.global_files.len(), 4, "Broken is not remembered");
        // A new scan forgets every file read.
        store.scan().unwrap();
        assert!(store.global_files.is_empty());
    }

    /// Decision 2026-11-16-63 (review r1 of PR #362): a file that failed to load is tried again on a later lookup, and a
    /// file read before is read again when its size or modification time changed, so both are searched as they now
    /// are. The check oracle's full scan agrees on every lookup.
    #[test]
    fn a_failed_or_rewritten_candidate_is_read_again() {
        let tree = Tree::new("again");
        tree.write(
            "Source.upk",
            &package(
                &[
                    ("Core", "Package", 0, "GDX"),
                    ("Core", "Object", -1, "ObjA"),
                    ("Core", "Object", -1, "ObjB"),
                    ("Core", "Object", -1, "ObjC"),
                    ("Core", "Object", -1, "Nowhere"),
                ],
                &[],
            ),
        );
        // Scan order: Aaa (does not load yet), Bbb (holds ObjA), Ccc (holds nothing asked yet).
        tree.write("Aaa.upk", &[1, 2, 3]);
        tree.write("Bbb.upk", &package(&[], &[("GDX", 0), ("ObjA", 1)]));
        let ccc = tree.write("Ccc.upk", &package(&[], &[("GDX", 0)]));
        let mut store = tree.store();
        store.set_resolve_check(Some(ResolveCheck::default()));
        // A path no file holds: every file is tried, and Bbb and Ccc are read (Aaa does not load).
        assert!(matches!(
            resolve_in(&tree, &mut store, -5),
            Err(StoreError::ImportNotFound { .. })
        ));
        assert_eq!(store.global_files.len(), 2);
        assert_eq!(
            resolve_in(&tree, &mut store, -2).unwrap(),
            ("Bbb".into(), 2)
        );
        // Aaa becomes readable and holds ObjB; Ccc, read before, now holds ObjC (a different size).
        tree.write("Aaa.upk", &package(&[], &[("GDX", 0), ("ObjB", 1)]));
        std::fs::write(&ccc, package(&[], &[("GDX", 0), ("Pad", 1), ("ObjC", 1)])).unwrap();
        assert_eq!(
            resolve_in(&tree, &mut store, -3).unwrap(),
            ("Aaa".into(), 2)
        );
        assert_eq!(
            resolve_in(&tree, &mut store, -4).unwrap(),
            ("Ccc".into(), 3)
        );
        let log = store.take_check_log();
        assert_eq!(
            log.iter()
                .filter(|l| l.starts_with("OWRESOLVECHECK same"))
                .count(),
            4
        );
    }

    /// The summary stops where `find_export` would fail (an export whose path cannot be built), and the first file in
    /// scan order wins between two whose summaries are both cached; the check oracle agrees on each.
    #[test]
    fn the_summary_follows_find_export_past_a_broken_path_and_keeps_scan_order() {
        let tree = Tree::new("summary");
        tree.write(
            "Source.upk",
            &package(
                &[
                    ("Core", "Package", 0, "GDX"),
                    ("Core", "Object", -1, "Obj"),
                    ("Core", "Object", -1, "Late"),
                    ("Core", "Object", -1, "Other"),
                    ("Core", "Object", -1, "Dup"),
                ],
                &[],
            ),
        );
        // Afile: GDX.Obj, then an export whose outer is itself (its path cannot be built), then GDX.Late.
        tree.write(
            "Afile.upk",
            &package(&[], &[("GDX", 0), ("Obj", 1), ("Loop", 3), ("Late", 1)]),
        );
        tree.write("Bfile.upk", &package(&[], &[("GDX", 0), ("Dup", 1)]));
        tree.write("Cfile.upk", &package(&[], &[("GDX", 0), ("Dup", 1)]));
        tree.write("Dfile.upk", &package(&[], &[("GDX", 0), ("Other", 1)]));
        let mut store = tree.store();
        store.set_resolve_check(Some(ResolveCheck::default()));
        // Other reads every file (Afile up to its broken export) and matches Dfile; the root cache then points there.
        assert_eq!(
            resolve_in(&tree, &mut store, -4).unwrap(),
            ("Dfile".into(), 2)
        );
        assert_eq!(store.global_files.len(), 4);
        // Late is past Afile's broken export: find_export fails there, so no file holds it (Afile is skipped by its
        // summary).
        assert!(matches!(
            resolve_in(&tree, &mut store, -3),
            Err(StoreError::ImportNotFound { .. })
        ));
        // Dup: both Bfile and Cfile hold it and both summaries are cached; the first in scan order wins.
        assert_eq!(
            resolve_in(&tree, &mut store, -5).unwrap(),
            ("Bfile".into(), 2)
        );
        // Obj is before the broken export: Afile's summary holds it.
        assert_eq!(
            resolve_in(&tree, &mut store, -2).unwrap(),
            ("Afile".into(), 2)
        );
        let log = store.take_check_log();
        assert_eq!(
            log.iter()
                .filter(|l| l.starts_with("OWRESOLVECHECK same"))
                .count(),
            4
        );
    }

    #[test]
    fn global_candidates_are_the_fallback_s_files_in_scan_order() {
        let tree = Tree::new("candidates");
        tree.write("Source.upk", &package(GDX_OBJ, &[]));
        tree.write("Zeta.upk", &package(&[], HOLDS));
        tree.write("Mid.upk", &package(&[], HOLDS));
        tree.write("Other.upk", &package(&[], &[("Unrelated", 0), ("Obj", 1)]));
        tree.write("Broken.upk", &[1, 2, 3]);
        let mut store = tree.store();
        let names: Vec<String> = store
            .global_candidates("GDX.Obj")
            .iter()
            .map(|p| p.file_stem().unwrap().to_string_lossy().into_owned())
            .collect();
        // The files whose roots hold GDX, and the file whose roots are unknown (it does not load), in scan order.
        assert_eq!(names, ["Broken", "Mid", "Zeta"]);
        // Nothing was loaded or cached: the fallback still answers as before.
        assert!(store.loaded.is_empty());
        assert!(store.global_matches.is_empty());
    }

    #[test]
    fn the_index_read_on_worker_threads_keeps_the_scan_order() {
        // More files than the most worker threads, holders and broken files spread among them, so the chunks
        // the threads read meet: the candidates still come in scan order.
        let tree = Tree::new("threads");
        tree.write("Source.upk", &package(GDX_OBJ, &[]));
        let mut expected = Vec::new();
        for i in 0..(3 * INDEX_THREADS + 5) {
            let name = format!("F{i:03}");
            let bytes = match i % 4 {
                0 => package(&[], HOLDS),
                1 => vec![1, 2, 3],
                _ => package(&[], &[("Unrelated", 0)]),
            };
            tree.write(&format!("{name}.upk"), &bytes);
            if i % 4 < 2 {
                expected.push(name);
            }
        }
        let mut store = tree.store();
        let names: Vec<String> = store
            .global_candidates("GDX.Obj")
            .iter()
            .map(|p| p.file_stem().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, expected);
    }

    #[test]
    fn files_with_unknown_roots_stay_candidates() {
        // A loop after the match: still found there.
        let tree = Tree::new("loop1");
        tree.write("Source.upk", &package(GDX_OBJ, &[]));
        tree.write(
            "Afirst.upk",
            &package(&[], &[("GDX", 0), ("Obj", 1), ("Loop", 3)]),
        );
        tree.write("Later.upk", &package(&[], HOLDS));
        let (result, log) = checked(&tree, -2);
        assert_eq!(result.unwrap(), ("Afirst".into(), 2));
        assert!(log[0].ends_with("any_root=1"), "{log:?}");
        // A loop before the match: the search throws there and moves on.
        let tree = Tree::new("loop2");
        tree.write("Source.upk", &package(GDX_OBJ, &[]));
        tree.write(
            "Afirst.upk",
            &package(&[], &[("Loop", 1), ("GDX", 0), ("Obj", 2)]),
        );
        tree.write("Later.upk", &package(&[], HOLDS));
        assert_eq!(checked(&tree, -2).0.unwrap(), ("Later".into(), 2));
        // A file that does not load.
        let tree = Tree::new("broken");
        tree.write("Source.upk", &package(GDX_OBJ, &[]));
        tree.write("Broken.upk", b"not a package");
        tree.write("Later.upk", &package(&[], HOLDS));
        assert_eq!(checked(&tree, -2).0.unwrap(), ("Later".into(), 2));
        // An empty outermost name.
        let tree = Tree::new("empty");
        tree.write(
            "Source.upk",
            &package(&[("Core", "Package", 0, "Obj")], &[]),
        );
        tree.write("Afirst.upk", &package(&[], &[("", 0), ("Obj", 1)]));
        tree.write("Later.upk", &package(&[], &[("Obj", 0)]));
        let (result, log) = checked(&tree, -1);
        assert_eq!(result.unwrap(), ("Afirst".into(), 2));
        assert!(log[0].ends_with("any_root=1"), "{log:?}");
    }

    #[test]
    fn a_file_that_fails_only_in_the_index_pass_still_wins() {
        let tree = Tree::new("seam");
        tree.write("Source.upk", &package(GDX_OBJ, &[]));
        tree.write("Afirst.upk", &package(&[], HOLDS));
        tree.write("Later.upk", &package(&[], HOLDS));
        let mut store = tree.store();
        store.set_resolve_check(Some(ResolveCheck {
            fail_in_build: Some("Afirst".into()),
        }));
        assert_eq!(
            resolve_in(&tree, &mut store, -2).unwrap(),
            ("Afirst".into(), 2)
        );
        let log = store.take_check_log();
        assert!(log[0].ends_with("any_root=1"), "{log:?}");
        assert_eq!(log[1], "OWRESOLVECHECK same GDX.Obj -> Afirst 2");
        assert!(store.take_check_log().is_empty());
    }

    #[test]
    fn a_path_no_file_has_is_an_error_and_a_cached_miss() {
        let tree = Tree::new("miss");
        tree.write("Source.upk", &package(GDX_OBJ, &[]));
        tree.write("Other.upk", &package(&[], &[("Unrelated", 0)]));
        let mut store = tree.store();
        store.set_resolve_check(Some(ResolveCheck::default()));
        let error = resolve_in(&tree, &mut store, -2).unwrap_err();
        assert_eq!(error.to_string(), "import target not found: GDX.Obj");
        assert_eq!(
            store.take_check_log().last().unwrap(),
            "OWRESOLVECHECK same GDX.Obj -> none 0"
        );
        // The miss is cached: no second lookup, no second check line.
        assert!(resolve_in(&tree, &mut store, -2).is_err());
        assert!(store.take_check_log().is_empty());
    }

    #[test]
    fn a_global_answer_is_cached_for_the_path_and_its_root() {
        let tree = Tree::new("cache");
        tree.write(
            "Source.upk",
            &package(
                &[
                    ("Core", "Package", 0, "GDX"),
                    ("Core", "Object", -1, "Obj"),
                    ("Core", "Object", -1, "Two"),
                ],
                &[],
            ),
        );
        tree.write(
            "Holder.upk",
            &package(&[], &[("GDX", 0), ("Obj", 1), ("Two", 1)]),
        );
        let mut store = tree.store();
        store.set_resolve_check(Some(ResolveCheck::default()));
        assert_eq!(
            resolve_in(&tree, &mut store, -2).unwrap(),
            ("Holder".into(), 2)
        );
        assert_eq!(store.take_check_log().len(), 2);
        // GDX.Two comes from the root cache (no global lookup) and GDX.Obj from the path cache.
        assert_eq!(
            resolve_in(&tree, &mut store, -3).unwrap(),
            ("Holder".into(), 3)
        );
        assert_eq!(
            resolve_in(&tree, &mut store, -2).unwrap(),
            ("Holder".into(), 2)
        );
        assert!(store.take_check_log().is_empty());
    }

    #[test]
    fn ambiguous_and_missing_names_are_refused() {
        let tree = Tree::new("ambiguous");
        tree.write("Twice.upk", &package(&[], &[("X", 0)]));
        tree.write("deeper/twice.umap", &package(&[], &[("X", 0)]));
        tree.write(
            "Source.upk",
            &package(
                &[("Core", "Package", 0, "Twice"), ("Core", "Object", -1, "X")],
                &[],
            ),
        );
        tree.write("notes.txt", b"not listed");
        let mut store = tree.store();
        assert_eq!(store.indexed_package_count(), 3);
        assert_eq!(store.paths_named("TWICE").len(), 2);
        assert!(store.paths_named("missing").is_empty());
        let twice = PackageName::parse("Twice").unwrap();
        assert_eq!(
            store.load(&twice),
            Err(StoreError::AmbiguousName("Twice".into()))
        );
        let missing = PackageName::parse("Missing").unwrap();
        assert_eq!(
            store.load(&missing).unwrap_err().to_string(),
            "package not found: Missing"
        );
        // Resolving through an ambiguous name fails as the C++ does; it does not pick one.
        assert_eq!(
            resolve_in(&tree, &mut store, -2),
            Err(StoreError::AmbiguousName("Twice".into()))
        );
    }

    #[test]
    fn scan_needs_a_directory_and_keeps_ids_valid() {
        let tree = Tree::new("rescan");
        let file = tree.write("A.upk", &package(&[], &[("X", 0)]));
        assert_eq!(
            PackageStore::open(file.clone()).unwrap_err(),
            StoreError::NotADirectory
        );
        assert_eq!(
            PackageStore::open(tree.0.join("missing")).unwrap_err(),
            StoreError::NotADirectory
        );
        let mut store = tree.store();
        let a = store.load_path(&file).unwrap();
        store.scan().unwrap();
        assert_eq!(store.get(a).name, "A");
        assert_ne!(store.load_path(&file).unwrap(), a, "the cache was emptied");
        let bytes =
            Arc::new(Package::from_decoded(PathBuf::new(), "B".into(), package(&[], &[])).unwrap());
        assert_eq!(store.adopt(bytes), Err(StoreError::NoSourcePath));
        assert!(PackageStore::new_empty().scan_order().is_empty());
    }

    /// `std::filesystem::path` order: element by element, so `a/b/x` sorts before `a/b-c/x`, which plain text order
    /// (`\` or `/` above `-`) would reverse.
    #[test]
    fn scan_order_is_the_cxx_path_order() {
        let tree = Tree::new("order");
        for name in ["b-c/x.upk", "b/x.upk", "B.upk", "a.u", "b/y.UMAP"] {
            tree.write(name, &package(&[], &[]));
        }
        let store = tree.store();
        let order: Vec<String> = store
            .scan_order()
            .iter()
            .map(|p| {
                p.strip_prefix(store.root())
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        assert_eq!(order, ["B.upk", "a.u", "b/x.upk", "b/y.UMAP", "b-c/x.upk"]);
        assert_eq!(
            path_order(Path::new("a/b"), Path::new("a//b")),
            Ordering::Equal
        );
        assert_eq!(merge(&[1, 4, 6], &[2, 4, 9]), [1, 2, 4, 4, 6, 9]);
    }

    #[test]
    fn errors_print_the_cxx_texts() {
        let cases = [
            (
                StoreError::NotADirectory,
                "cooked package root is not a directory",
            ),
            (StoreError::CyclicImport, "cyclic import outer chain"),
            (
                StoreError::NoPackageQualifiedPath,
                "local import has no package-qualified path",
            ),
            (
                StoreError::CheckDisagrees("A.B".into()),
                "global index disagrees with the full scan: A.B",
            ),
            (
                StoreError::Package(PackageError::OuterChain),
                "cyclic or excessively deep object outer chain",
            ),
        ];
        for (error, text) in cases {
            assert_eq!(error.to_string(), text);
        }
    }

    /// Review r1 of PR #299: a package loaded before its file was rewritten must not stand in for the fresh one the
    /// global fallback matched. The answer is the matched package itself, under a new id; the old id is unchanged.
    #[test]
    fn the_global_answer_is_the_package_it_matched() {
        let tree = Tree::new("stale");
        tree.write("Source.upk", &package(GDX_OBJ, &[]));
        let holder = tree.write("Holder.upk", &package(&[], HOLDS));
        let mut store = tree.store();
        let old = store.load_path(&holder).unwrap();
        // Rewritten: GDX.Obj is export 3 now, past the end of the old copy's two exports.
        tree.write(
            "Holder.upk",
            &package(&[], &[("Pad", 0), ("GDX", 0), ("Obj", 2)]),
        );
        let source = store.load_path(&tree.0.join("Source.upk")).unwrap();
        let (id, index) = store.resolve(source, -2).unwrap().unwrap();
        assert_ne!(id, old);
        assert_eq!(index, 3);
        assert_eq!(store.get(id).path_of(index).unwrap(), "GDX.Obj");
        assert_eq!(
            store.get(old).exports.len(),
            2,
            "the old id keeps its package"
        );
        assert_eq!(
            store.get(id).source,
            store.get(old).source,
            "same file, two ids"
        );
    }

    #[test]
    fn the_store_is_send_and_sync() {
        fn check<T: Send + Sync>() {}
        check::<PackageStore>();
    }
}
