//! Owner: lane W1-PKG. `Package` and its loading (architecture section 3.2): the port of the C++ `Package::load`,
//! `object`, `path` and `findExport` (`src/package.cpp`). `PackageId` is final from the scaffold (architecture
//! section 15).
//!
//! Loading reads the file, decodes it with `ow_container::decode_package` (fully compressed, partially compressed,
//! or returned as is), then reads the summary and the three tables from the decoded bytes. Every check of the C++
//! loader is here in the same order and with the same message, so a refused file is refused with the C++ text.
//! [`Package::from_decoded`] reads bytes that are already decoded (synthetic fixtures, tools that decode
//! themselves).
//!
//! `Package` is `Send + Sync` and shared as `Arc<Package>` (decision D18).

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ow_container::ContainerError;
use ow_core::{IdentError, PackageName};

use crate::header::{self, HeaderError};
use crate::reader::{ReadError, Reader};
use crate::tables::{self, ObjectEntry, TableError};

/// The index of a package in a `PackageStore`, stable for one store.
pub type PackageId = u32;

/// How deep an outer chain may be before `path_of` refuses it (the C++ `Package::path` bound).
pub const MAX_OUTER_DEPTH: usize = 256;

/// Why a package could not be loaded or an object in it could not be named. `Display` gives the C++ exception
/// text, which the CLI prints after `ow-package: `.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageError {
    /// "cannot open input package" (the file could not be read).
    Open,
    Container(ContainerError),
    Header(HeaderError),
    Table(TableError),
    Read(ReadError),
    /// "cyclic or excessively deep object outer chain"
    OuterChain,
}

impl fmt::Display for PackageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PackageError::Open => f.write_str("cannot open input package"),
            // The container's own text is its `source()`, so a chained reporter prints it once.
            PackageError::Container(_) => f.write_str("cannot decode the package container"),
            PackageError::Header(e) => e.fmt(f),
            PackageError::Table(e) => e.fmt(f),
            PackageError::Read(e) => e.fmt(f),
            PackageError::OuterChain => f.write_str("cyclic or excessively deep object outer chain"),
        }
    }
}

impl std::error::Error for PackageError {
    /// The container error behind [`PackageError::Container`]; the other variants print their whole cause.
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PackageError::Container(e) => Some(e),
            _ => None,
        }
    }
}

impl From<ContainerError> for PackageError {
    fn from(e: ContainerError) -> Self {
        PackageError::Container(e)
    }
}
impl From<HeaderError> for PackageError {
    fn from(e: HeaderError) -> Self {
        PackageError::Header(e)
    }
}
impl From<TableError> for PackageError {
    fn from(e: TableError) -> Self {
        PackageError::Table(e)
    }
}
impl From<ReadError> for PackageError {
    fn from(e: ReadError) -> Self {
        PackageError::Read(e)
    }
}

/// A loaded package: its decoded bytes and its three tables.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    /// The absolute, lexically normal path it was loaded from (empty for bytes given directly).
    pub source: PathBuf,
    /// The file stem, as the C++ `packageName` (`path.stem()`). Kept as text: a stem may hold characters a
    /// [`PackageName`] refuses (see [`Package::package_name`]).
    pub name: String,
    pub version: u16,
    pub licensee: u16,
    /// The decoded package bytes; export offsets point into these.
    pub data: Vec<u8>,
    pub names: Vec<String>,
    /// Import `-1` is `imports[0]`.
    pub imports: Vec<ObjectEntry>,
    /// Export `1` is `exports[0]`.
    pub exports: Vec<ObjectEntry>,
}

impl Package {
    /// Reads, decodes and parses the package file at `path` (C++ `Package::load`).
    pub fn load(path: &Path) -> Result<Arc<Package>, PackageError> {
        let raw = std::fs::read(path).map_err(|_| PackageError::Open)?;
        let source = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        Package::from_bytes(source, name, &raw).map(Arc::new)
    }

    /// Decodes `raw` (a package file's bytes) with `ow_container::decode_package` and parses the result.
    pub fn from_bytes(source: PathBuf, name: String, raw: &[u8]) -> Result<Package, PackageError> {
        let data = ow_container::decode_package(raw)?;
        Package::from_decoded(source, name, data)
    }

    /// Parses already decoded package bytes: the summary, then the name, import and export tables, with the C++
    /// loader's checks in its order.
    pub fn from_decoded(source: PathBuf, name: String, data: Vec<u8>) -> Result<Package, PackageError> {
        let summary = header::read_summary(&data)?;
        let mut r = Reader::new(&data);
        let names = tables::read_names(&mut r, summary.name_count, summary.name_offset)?;
        let imports = tables::read_imports(
            &mut r,
            &names,
            summary.import_count,
            summary.import_offset,
            summary.export_count,
        )?;
        let exports = tables::read_exports(
            &mut r,
            &names,
            summary.import_count,
            summary.export_count,
            summary.export_offset,
        )?;
        Ok(Package {
            source,
            name,
            version: summary.version,
            licensee: summary.licensee,
            data,
            names,
            imports,
            exports,
        })
    }

    /// The stem as a [`PackageName`], for the store and resolvers; fails where the stem is not a valid package name.
    pub fn package_name(&self) -> Result<PackageName, IdentError> {
        PackageName::parse(&self.name)
    }

    /// A reader over the decoded bytes, at position 0 with no limit.
    pub fn reader(&self) -> Reader<'_> {
        Reader::new(&self.data)
    }

    /// The import (`index < 0`) or export (`index > 0`) entry; 0 and out-of-range indices are refused with
    /// "invalid object reference" (C++ `Package::object`).
    pub fn object(&self, index: i32) -> Result<&ObjectEntry, PackageError> {
        let wide = i64::from(index);
        if index == 0 || wide < -(self.imports.len() as i64) || wide > self.exports.len() as i64 {
            return Err(PackageError::Read(ReadError::InvalidReference));
        }
        let entry = if index < 0 {
            &self.imports[(-wide - 1) as usize]
        } else {
            &self.exports[(wide - 1) as usize]
        };
        Ok(entry)
    }

    /// The object's path: the names of its outer chain, outermost first, joined with `.` (C++ `Package::path`).
    /// 0 is the empty path. A cycle or a chain of more than [`MAX_OUTER_DEPTH`] links is refused.
    pub fn path_of(&self, reference: i32) -> Result<String, PackageError> {
        let mut parts: Vec<&str> = Vec::new();
        let mut seen: Vec<i32> = Vec::new();
        let mut index = reference;
        while index != 0 {
            if seen.contains(&index) || parts.len() >= MAX_OUTER_DEPTH {
                return Err(PackageError::OuterChain);
            }
            seen.push(index);
            let current = self.object(index)?;
            parts.push(&current.name);
            index = current.outer;
        }
        parts.reverse();
        Ok(parts.join("."))
    }

    /// The first export whose path equals `object_path`, compared with ASCII case folded (C++ `findExport`, which
    /// folds with `tolower` in the C locale). A leading `<package name>.` is dropped first. `Ok(None)` when no export
    /// matches; an export whose path cannot be built fails the search, as in the C++.
    pub fn find_export(&self, object_path: &str) -> Result<Option<i32>, PackageError> {
        let prefix = format!("{}.", self.name);
        let wanted = if starts_with_folded(object_path, &prefix) {
            &object_path[prefix.len()..]
        } else {
            object_path
        };
        for index in 1..=self.exports.len() as i32 {
            if self.path_of(index)?.eq_ignore_ascii_case(wanted) {
                return Ok(Some(index));
            }
        }
        Ok(None)
    }
}

fn starts_with_folded(text: &str, prefix: &str) -> bool {
    text.len() >= prefix.len()
        && text.as_bytes()[..prefix.len()].eq_ignore_ascii_case(prefix.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn le(values: &[u32]) -> Vec<u8> {
        values.iter().flat_map(|v| v.to_le_bytes()).collect()
    }

    /// The synthetic package of `tests/package_test.py` (invented values: one name, one import, one export).
    fn valid() -> Vec<u8> {
        let mut data = le(&[0x9E2A83C1, 832 | (46 << 16), 48, 0, 0, 1, 48, 1, 93, 1, 65, 0]);
        data.extend_from_slice(&le(&[5]));
        data.extend_from_slice(b"None\0");
        data.extend_from_slice(&[0; 8]);
        data.extend_from_slice(&le(&[0, 0, 0, 0, 0, 0, 0]));
        data.extend_from_slice(&le(&[
            0xFFFFFFFF, 0, 0, 0, 0, 0, 0, 0, 0, 161, 0, 0, 0, 0, 0, 0, 0,
        ]));
        data
    }

    fn parse(data: Vec<u8>) -> Result<Package, PackageError> {
        Package::from_decoded(PathBuf::new(), "synthetic".into(), data)
    }

    #[test]
    fn reads_the_synthetic_package() {
        let p = parse(valid()).unwrap();
        assert_eq!((p.version, p.licensee), (832, 46));
        assert_eq!(p.names, vec!["None".to_string()]);
        assert_eq!(p.imports.len(), 1);
        assert_eq!(p.exports.len(), 1);
        assert_eq!(p.exports[0].class, -1);
        assert_eq!((p.exports[0].size, p.exports[0].offset), (0, 161));
        assert_eq!(p.path_of(1).unwrap(), "None");
        assert_eq!(p.path_of(-1).unwrap(), "None");
        assert_eq!(p.path_of(0).unwrap(), "");
        assert_eq!(p.find_export("synthetic.none").unwrap(), Some(1));
        assert_eq!(p.find_export("NONE").unwrap(), Some(1));
        assert_eq!(p.find_export("Other").unwrap(), None);
    }

    /// A package written by the shared testkit builder (W1-TESTKIT) reads back with every field it was given.
    #[test]
    fn reads_what_the_testkit_builder_writes() {
        use crate::testkit::{Export, PackageBuilder};
        let mut b = PackageBuilder::new_empty();
        b.name_utf16("Wide\u{1f600}");
        let class = b.add_import("Core", "Class", 0, "Thing");
        let inner = b.add_import("Core", "Package", class, "Inner");
        let outer = b.add_export(class, "Outer", vec![1, 2, 3]);
        let mut child = Export::new(inner, "Child", vec![9]);
        child.outer = outer;
        child.number = 4;
        child.archetype = outer;
        child.super_ = class;
        let child = b.add_export_with(child);
        let (data, layout) = b.build_with_layout();
        let p = parse(data).unwrap();
        assert!(p.names.iter().any(|n| n == "Wide\u{1f600}"));
        assert_eq!(p.path_of(inner).unwrap(), "Thing.Inner");
        assert_eq!(p.object(inner).unwrap().class_name, "Package");
        assert_eq!(p.path_of(child).unwrap(), "Outer.Child_3");
        let e = p.object(child).unwrap();
        assert_eq!((e.class, e.super_, e.outer, e.archetype), (inner, class, outer, outer));
        let o = p.object(outer).unwrap();
        assert_eq!(
            &p.data[o.offset as usize..(o.offset + o.size) as usize],
            &[1, 2, 3]
        );
        assert_eq!(p.data.len(), layout.len);
        assert_eq!(p.find_export("synthetic.OUTER.child_3").unwrap(), Some(child));
    }

    /// `from_bytes` decodes through `ow_container`: a plain package, the same package in an LZO container and the
    /// partially compressed form all read the same tables; a bad container is a `Container` error with its cause as
    /// `source()`.
    #[test]
    fn from_bytes_decodes_every_container_form() {
        use crate::testkit::{PackageBuilder, container};
        use std::error::Error as _;
        let mut b = PackageBuilder::new_empty();
        b.add_import("Core", "Class", 0, "Thing");
        b.add_export(-1, "Obj", vec![7, 8]);
        let plain = b.build();
        let load = |raw: &[u8]| Package::from_bytes(PathBuf::new(), "synthetic".into(), raw);
        let reference = load(&plain).unwrap();
        let compressed = load(&container(&plain, 4096)).unwrap();
        assert_eq!(compressed.data, plain);
        assert_eq!(compressed.exports, reference.exports);
        let partial = load(&b.build_partial()).unwrap();
        assert_eq!(partial.names, reference.names);
        assert_eq!(partial.imports, reference.imports);
        assert_eq!(partial.path_of(1).unwrap(), "Obj");

        let mut bad = container(&plain, 4096);
        bad[4..8].copy_from_slice(&1000u32.to_le_bytes());
        let error = load(&bad).unwrap_err();
        assert!(matches!(error, PackageError::Container(_)), "{error:?}");
        assert_eq!(error.to_string(), "cannot decode the package container");
        assert!(error.source().is_some());
    }

    #[test]
    fn every_truncation_is_refused() {
        let data = valid();
        for end in 0..data.len() {
            assert!(parse(data[..end].to_vec()).is_err(), "{end}");
        }
    }

    #[test]
    fn the_package_test_corruptions_are_refused_with_the_cxx_texts() {
        // The offsets and values of tests/package_test.py's corruption loop.
        let cases: [(usize, u32, &str); 8] = [
            (0, 0, "invalid package magic"),
            (4, 0, "requires decompressed BL2 version 832/46"),
            (20, 0xFFFFFFFF, "invalid table bounds"),
            (24, 999999, "invalid table bounds"),
            (65, 9, "invalid FName"),
            (93, 2, "invalid object reference"),
            (125, 999999, "invalid export payload bounds"),
            (137, 0xFFFFFFFF, "invalid net object count"),
        ];
        for (offset, value, text) in cases {
            let mut bad = valid();
            bad[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            let error = parse(bad).unwrap_err();
            assert_eq!(error.to_string(), text, "offset {offset}");
        }
    }

    #[test]
    fn object_and_path_check_references_and_cycles() {
        let mut p = parse(valid()).unwrap();
        assert_eq!(p.object(0).unwrap_err().to_string(), "invalid object reference");
        assert!(p.object(2).is_err());
        assert!(p.object(-2).is_err());
        assert!(p.object(i32::MIN).is_err());
        // An export that is its own outer is a cycle.
        p.exports[0].outer = 1;
        assert_eq!(p.path_of(1), Err(PackageError::OuterChain));
        assert_eq!(p.find_export("x"), Err(PackageError::OuterChain));
        // Import -> export -> import, named Outer.Inner.
        p.names = vec!["Outer".into(), "Inner".into()];
        p.exports[0] = ObjectEntry {
            name: "Outer".into(),
            ..ObjectEntry::default()
        };
        p.imports[0] = ObjectEntry {
            name: "Inner".into(),
            outer: 1,
            ..ObjectEntry::default()
        };
        assert_eq!(p.path_of(-1).unwrap(), "Outer.Inner");
    }

    #[test]
    fn deep_chains_stop_at_the_bound() {
        let mut p = parse(valid()).unwrap();
        let link = |outer: i32| ObjectEntry {
            name: "L".into(),
            outer,
            ..ObjectEntry::default()
        };
        p.exports = (0..MAX_OUTER_DEPTH as i32).map(link).collect();
        assert_eq!(
            p.path_of(MAX_OUTER_DEPTH as i32).unwrap().split('.').count(),
            MAX_OUTER_DEPTH
        );
        p.exports.push(link(MAX_OUTER_DEPTH as i32));
        assert_eq!(p.path_of(MAX_OUTER_DEPTH as i32 + 1), Err(PackageError::OuterChain));
    }

    #[test]
    fn load_reports_an_unreadable_file() {
        let missing = Path::new("this-file-does-not-exist.upk");
        assert_eq!(Package::load(missing), Err(PackageError::Open));
    }

    #[test]
    fn package_is_send_and_sync() {
        fn check<T: Send + Sync>() {}
        check::<Package>();
        check::<Arc<Package>>();
    }
}
