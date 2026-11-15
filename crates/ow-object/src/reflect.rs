//! Owner: lane W2-REFLECT. Reflection from the code packages: the class loader of `src/vm.cpp` (`Runtime::classAt`,
//! `loadClass`, `resolveRef`, `indexOf`, `findExport`, `implements`, `implementsStructurally`) with its two oracles
//! (`checkClassBodies`, `compareInterfaces` in `src/vm_sweep.cpp`), and the loading of Class, ScriptStruct, Enum and
//! property exports into [`ClassTable`] layouts (architecture section 3.3, decision D14).
//!
//! **Two halves.** [`CodeIndex`] first loads classes the C++ way: lazily, by package-relative reference, caching
//! each reference's answer and each class by its export. A class keeps what the C++ `Class` keeps for the interface
//! rules: its super class, its decoded body ([`decode_class_body`]) or the reason it did not decode, and the names of
//! the functions it declares. That half answers `--class-check` and `--interface-compare` exactly as the C++ binary
//! does. Its second half, [`CodeIndex::define_layout`], turns a loaded class into a [`ClassTable`] definition: the
//! super class first, then the class's own property declarations typed through their references (a struct by value
//! defined first, an enum read from its export, a referenced class only declared), then the attribute companions.
//! The C++ VM has no such table (it keeps a name-keyed bag); the declarations it reads (`readProperty`,
//! `childProperties`, `structAt`) are the same ones read here.
//!
//! **Declarations.** A class's or struct's own properties are its child exports whose class name ends in `Property`,
//! in descending export order (the C++ rule: children are exported in reverse declaration order; checked there over
//! 5,980 of 5,984 Core and Engine functions). A child smaller than the 40-byte header is skipped, as the C++ skips it
//! (a property-class default object). The header gives the array dimension (low 16 bits of the word at +16, 0 read
//! as 1), the 64-bit flags at +20 and, from +40 (+42 when flag `0x20` adds a replication offset), up to three type
//! references. Which reference means what is the C++ reading: an object, component or interface property names its
//! class first, a class property its metaclass second, a struct property its struct, an array property its inner
//! declaration, a byte property its enum (or 0); an attribute value property names its modifier stack and then its
//! base, and a byte attribute names its enum first, so there the stack is the second reference and the base the
//! third (the byte-attribute shift, `companions()` in `src/interp.cpp`, NATIVE_BYTECODE_OPCODES.md).
//!
//! **Enums** are `[net index][None][next][count][count names]` and must end exactly at the serial size (the count
//! bound 1024 is `enumNames` in `src/behavior.cpp`; the exact end is the structural check PHASELOCK_STOCK_DATA.md
//! records, which holds for all 792 Enum exports of the nine code packages).
//!
//! **Maps.** A `MapProperty` is a [`PropKind::Map`] whose key and value kinds are read from its first two references
//! where they are not 0 (key first, UNVERIFIED: no C++ code reads them). All 50 map properties of the nine code
//! packages (32 classes, among them `Engine.WorldInfo` and `WillowGame.WillowPlayerController`) have both at 0, so
//! their key and value are `None`.
//!
//! **Bad input.** Cycles give typed errors: a super chain (`SuperCycle`), a struct held by value inside itself
//! (`StructCycle`; through an array or a map it is legal, and such structs are defined after the class that reached
//! them), an array or map whose inner declarations lead back to it (`PropertyCycle`). All recursion nests at most
//! [`MAX_NESTING`] deep (`TooDeep`, a Rust-only bound: the C++ has none). A class is published in the [`ClassTable`]
//! only once its declarations and attribute companions all hold.
//!
//! **Not yet.** Tagged default values, struct defaults and class default objects are W3-DECODE's; function parameter
//! layouts are the VM's.
//!
//! Status: a port. Every rule here has the label its record gives it; the interface table rule and the meaning of
//! the class body's fields are UNVERIFIED in the running game (NATIVE_CLASS_SERIAL_LAYOUT.md), and so is the
//! declaration order rule above.

use std::collections::BTreeMap;
use std::fmt;
use std::rc::Rc;

use ow_core::{FixedHashMap, FixedHashSet, NotYet, PackageName};
use ow_package::PackageId;
use ow_package::package::{Package, PackageError};
use ow_package::reader::{ReadError, Reader};
use ow_package::store::{PackageStore, StoreError};
use ow_package::tables;

use crate::class_body::{ClassBody, ClassBodyError, decode_class_body};
use crate::ids::{ClassId, EnumId, PropId, StructId};
use crate::layout::{AttrCompanions, ClassTable, LayoutError, PropKind, PropertyDecl};
use crate::names::NameTable;

/// The nine code packages, in the order the C++ loads them (`buildNativeIndex`, `compareInterfaces`).
pub const CODE_PACKAGES: [&str; 9] = [
    "Core",
    "Engine",
    "GameFramework",
    "GearboxFramework",
    "WillowGame",
    "GFxUI",
    "IpDrv",
    "OnlineSubsystemSteamworks",
    "AkAudio",
];

/// The interfaces `--interface-compare` reports one by one (the C++ `slice` map, in its key order).
pub const SLICE_INTERFACES: [&str; 4] = [
    "IMission",
    "IMissionDirector",
    "IMissionObjective",
    "IUsable",
];

/// How many import links `resolveRef` follows before it stops (its `depth < 64`).
pub const MAX_IMPORT_DEPTH: usize = 64;

/// The largest enum `enumNames` (`src/behavior.cpp`) reads.
pub const MAX_ENUM_COUNT: i32 = 1024;

/// The 40-byte property header (`readProperty`).
pub const PROPERTY_HEADER: usize = 40;

/// Property flag `0x20`: a 16-bit replication offset follows the header.
pub const CPF_NET: u64 = 0x20;

/// How deep class loading, layouts, struct definitions and array or map expansion may nest before reflection refuses
/// with [`ReflectError::TooDeep`]. A Rust-only bound (the C++ recursion has none), so a hostile package gives a typed
/// error instead of a stack overflow; the installed code packages nest far less (decision 2026-11-15-06).
pub const MAX_NESTING: usize = 128;

/// Why reflection failed. `Display` gives the C++ text where the C++ has one; wrappers name the class, struct, enum
/// or property and carry the cause as their `source()`.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReflectError {
    /// A store operation failed. Transparent.
    Store(StoreError),
    /// A package object could not be named. Transparent.
    Package(PackageError),
    /// A read failed. Transparent.
    Read(ReadError),
    /// "export is not a class: <path>"
    NotAClass(String),
    /// "class path must be Package.Class: <path>"
    ClassPath(String),
    /// "class not found: <path>"
    ClassNotFound(String),
    /// "not a property export": the export a reference names is missing or smaller than the 40-byte header.
    NotAProperty,
    /// "read past the package data"
    PastData,
    /// A child export of a class or struct whose class name ends in `Property` but is no property class read here.
    UnknownPropertyClass(String),
    /// "<property> names no <what>": a type reference that is 0 where one is required.
    NoReference {
        property: String,
        what: &'static str,
    },
    /// "<what> reference <n> of <property> resolves to nothing": no installed package holds the target.
    Unresolved {
        property: String,
        what: &'static str,
        reference: i32,
    },
    /// "<what> reference of <property> names a <found> export": the target is not of the expected class.
    WrongTarget {
        property: String,
        what: &'static str,
        found: String,
    },
    /// "invalid enum count" (`enumNames`), or a count past the export's end.
    EnumCount,
    /// "enum does not end at the serial size"
    EnumTrailing,
    /// "<path> holds itself by value": a struct cycle that is not broken by an array.
    StructCycle(String),
    /// "cyclic super chain at <path>"
    SuperCycle(String),
    /// "<property> contains itself": an array or map property whose inner declarations lead back to it.
    PropertyCycle(String),
    /// "<property> names <what> <package>:<index>, which is not one of the class's own declarations"
    CompanionNotOwn {
        property: String,
        what: &'static str,
        target: (PackageId, i32),
    },
    /// "reflection nests deeper than <n> levels": the Rust-only [`MAX_NESTING`] bound.
    TooDeep(usize),
    /// The class table refused a definition. Transparent.
    Layout(LayoutError),
    /// "class <path>", with the cause as `source()`.
    InClass {
        path: String,
        error: Box<ReflectError>,
    },
    /// "struct <path>", with the cause as `source()`.
    InStruct {
        path: String,
        error: Box<ReflectError>,
    },
    /// "enum <path>", with the cause as `source()`.
    InEnum {
        path: String,
        error: Box<ReflectError>,
    },
    NotYet(NotYet),
}

impl fmt::Display for ReflectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReflectError::Store(e) => e.fmt(f),
            ReflectError::Package(e) => e.fmt(f),
            ReflectError::Read(e) => e.fmt(f),
            ReflectError::NotAClass(path) => write!(f, "export is not a class: {path}"),
            ReflectError::ClassPath(path) => write!(f, "class path must be Package.Class: {path}"),
            ReflectError::ClassNotFound(path) => write!(f, "class not found: {path}"),
            ReflectError::NotAProperty => f.write_str("not a property export"),
            ReflectError::PastData => f.write_str("read past the package data"),
            ReflectError::UnknownPropertyClass(class) => {
                write!(f, "unknown property class {class}")
            }
            ReflectError::NoReference { property, what } => write!(f, "{property} names no {what}"),
            ReflectError::Unresolved {
                property,
                what,
                reference,
            } => write!(
                f,
                "{what} reference {reference} of {property} resolves to nothing"
            ),
            ReflectError::WrongTarget {
                property,
                what,
                found,
            } => write!(f, "{what} reference of {property} names a {found} export"),
            ReflectError::EnumCount => f.write_str("invalid enum count"),
            ReflectError::EnumTrailing => f.write_str("enum does not end at the serial size"),
            ReflectError::StructCycle(path) => write!(f, "{path} holds itself by value"),
            ReflectError::SuperCycle(path) => write!(f, "cyclic super chain at {path}"),
            ReflectError::PropertyCycle(property) => write!(f, "{property} contains itself"),
            ReflectError::CompanionNotOwn {
                property,
                what,
                target,
            } => write!(
                f,
                "{property} names {what} {}:{}, which is not one of the class's own declarations",
                target.0, target.1
            ),
            ReflectError::TooDeep(limit) => {
                write!(f, "reflection nests deeper than {limit} levels")
            }
            ReflectError::Layout(e) => e.fmt(f),
            ReflectError::InClass { path, .. } => write!(f, "class {path}"),
            ReflectError::InStruct { path, .. } => write!(f, "struct {path}"),
            ReflectError::InEnum { path, .. } => write!(f, "enum {path}"),
            ReflectError::NotYet(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for ReflectError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ReflectError::Store(e) => e.source(),
            ReflectError::Package(e) => e.source(),
            ReflectError::InClass { error, .. }
            | ReflectError::InStruct { error, .. }
            | ReflectError::InEnum { error, .. } => Some(error.as_ref()),
            _ => None,
        }
    }
}

impl From<StoreError> for ReflectError {
    fn from(e: StoreError) -> Self {
        ReflectError::Store(e)
    }
}
impl From<PackageError> for ReflectError {
    fn from(e: PackageError) -> Self {
        ReflectError::Package(e)
    }
}
impl From<ReadError> for ReflectError {
    fn from(e: ReadError) -> Self {
        ReflectError::Read(e)
    }
}
impl From<LayoutError> for ReflectError {
    fn from(e: LayoutError) -> Self {
        ReflectError::Layout(e)
    }
}

fn in_class(path: &str) -> impl FnOnce(ReflectError) -> ReflectError + '_ {
    move |error| ReflectError::InClass {
        path: path.to_string(),
        error: Box::new(error),
    }
}

fn in_struct(path: &str) -> impl FnOnce(ReflectError) -> ReflectError + '_ {
    move |error| ReflectError::InStruct {
        path: path.to_string(),
        error: Box::new(error),
    }
}

/// ASCII lower case (`lower()` in `src/vm.cpp`: `std::tolower` in the C locale, byte by byte).
fn lower(text: &str) -> String {
    text.to_ascii_lowercase()
}

/// A package file's identity for [`CodeIndex`]: its source path (absolute and lexically normal, as `Package::load`
/// keeps it) with `/` separators on Windows, ASCII case folded: the key of the store's path cache (C++ `loadPath`).
fn file_key(source: &std::path::Path) -> String {
    let text = source.to_string_lossy();
    let generic = if cfg!(windows) {
        text.replace('\\', "/")
    } else {
        text.into_owned()
    };
    lower(&generic)
}

/// A class loaded by [`CodeIndex::class_at`]: an index into the index's class list, stable for its life.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ClassKey(u32);

impl ClassKey {
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// One loaded class: the part of the C++ `Class` the interface rules and the layout need.
#[derive(Debug, Clone)]
pub struct ClassInfo {
    /// The package the class was loaded from, as its filed id ([`CodeIndex::filed_id`]).
    pub package: PackageId,
    pub export: i32,
    /// The export's name.
    pub name: String,
    /// `<package file stem>.<name>`, the C++ `Class::path`; the class's path in the [`ClassTable`].
    pub path: String,
    pub super_class: Option<ClassKey>,
    /// The decoded body, or why it did not decode (synthetic packages give classes a 16-byte stand-in body).
    pub body: Result<ClassBody, ClassBodyError>,
    /// Lower-cased function name -> the function export (the C++ `Class::functions` keys). A later child of the same
    /// lower-cased name replaces an earlier one, as the C++ map assignment does.
    pub functions: BTreeMap<String, i32>,
    /// The states the class declares: lower-cased state name -> the state (the C++ `Class::states` keys).
    states: BTreeMap<String, StateInfo>,
    /// The error the super chain gave while this class loaded. As the C++ `loadClass`, the class stays registered
    /// without its super class (and without its functions), and a later [`CodeIndex::class_at`] returns it; only
    /// [`CodeIndex::define_layout`] refuses it with this error.
    load_error: Option<ReflectError>,
    /// The resolved interface entries of the body: `None` until resolved (lazily, entry by entry, as the C++).
    interface_targets: Vec<Option<Option<ClassKey>>>,
}

impl ClassInfo {
    /// The C++ `Class::isInterface`: the body decoded and carries class flag bit 14.
    pub fn is_interface(&self) -> bool {
        self.body.as_ref().is_ok_and(ClassBody::is_interface)
    }

    pub fn body_decoded(&self) -> bool {
        self.body.is_ok()
    }

    /// The states the class itself declares (not its super classes'), by lower-cased name: the C++ `Class::states`
    /// that `loadClass` collects. A State export is a child of the class export whose class name is `State`; its
    /// functions are its own children whose class name is `Function`. Empty until the class loaded its functions
    /// (a class whose super chain failed to load keeps none, as it keeps no functions).
    pub fn states(&self) -> &BTreeMap<String, StateInfo> {
        &self.states
    }
}

/// One state a class declares ([`ClassInfo::states`]).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateInfo {
    /// The State export's name as the package spells it.
    pub name: String,
    /// The State export (in the class's package).
    pub export: i32,
    /// Lower-cased function name -> the function export, as [`ClassInfo::functions`]: a later child of the same
    /// lower-cased name replaces an earlier one, as the C++ map assignment does.
    pub functions: BTreeMap<String, i32>,
}

/// The C++ `PackageIndex` of one package: each export's class name, each export's children, the first export of each
/// lower-cased object path.
#[derive(Debug, Default)]
struct PackageIndex {
    class_names: Vec<String>,
    children: FixedHashMap<i32, Vec<i32>>,
    by_path: FixedHashMap<String, i32>,
}

/// `classNameOf`: `Class` for an export with class reference 0, `?` where the class reference cannot be named.
fn class_name_of(package: &Package, index: i32) -> String {
    let Some(object) = usize::try_from(index)
        .ok()
        .filter(|&i| i >= 1)
        .and_then(|i| package.exports.get(i - 1))
    else {
        return "Class".to_string();
    };
    if object.class == 0 {
        return "Class".to_string();
    }
    match package.object(object.class) {
        Ok(class) => class.name.clone(),
        Err(_) => "?".to_string(),
    }
}

fn build_index(package: &Package) -> PackageIndex {
    let mut index = PackageIndex {
        class_names: Vec::with_capacity(package.exports.len()),
        ..PackageIndex::default()
    };
    let mut paths: Vec<String> = Vec::with_capacity(package.exports.len());
    for (position, object) in package.exports.iter().enumerate() {
        let reference = position as i32 + 1;
        index.class_names.push(class_name_of(package, reference));
        if object.outer > 0 {
            index
                .children
                .entry(object.outer)
                .or_default()
                .push(reference);
        }
        // Outers usually come before what they hold; otherwise the slow path, and the bare name if that fails.
        let outer = object.outer;
        let path = if outer > 0 && outer < reference && !paths[outer as usize - 1].is_empty() {
            format!("{}.{}", paths[outer as usize - 1], object.name)
        } else if outer == 0 {
            object.name.clone()
        } else {
            package
                .path_of(reference)
                .unwrap_or_else(|_| object.name.clone())
        };
        index.by_path.entry(lower(&path)).or_insert(reference);
        paths.push(path);
    }
    index
}

/// One property declaration as read (`readProperty`), before it is typed.
#[derive(Debug, Clone)]
struct RawProperty {
    package: PackageId,
    index: i32,
    name: String,
    class: String,
    flags: u64,
    array_dim: i32,
    refs: [i32; 3],
}

/// The C++ `checkClassBodies` summary of one package.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClassCheck {
    pub package: String,
    pub classes: usize,
    pub exact: usize,
    pub default_objects: usize,
    pub interface_classes: usize,
    pub classes_with_interfaces: usize,
    pub interface_entries: usize,
    pub entries_to_interface_classes: usize,
    pub table_struct_properties: usize,
    pub table_null: usize,
    pub failures: Vec<String>,
}

/// The C++ `compareInterfaces` counts over the nine code packages.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InterfaceComparison {
    pub classes: usize,
    pub interfaces: usize,
    pub both_yes: usize,
    pub table_only_yes: usize,
    pub coded_only_yes: usize,
    /// The functions-only rule (no `I<Upper>` name condition): both, table only, stand-in only.
    pub functions_only: [usize; 3],
    /// Per [`SLICE_INTERFACES`] name: table yes, table no, disagreements with the coded rule and with the
    /// functions-only rule.
    pub slice: BTreeMap<String, [usize; 4]>,
}

/// What [`CodeIndex::define_layouts`] did over one set of classes.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LayoutSummary {
    pub classes: usize,
    pub defined: usize,
    /// Every class that did not get a layout, with the error (its cause chain names the first failure).
    pub failures: Vec<(String, ReflectError)>,
}

/// The reflection index over a store: loaded classes, cached reference answers, per-package lookup tables, and the
/// layouts defined from them. Built empty with [`CodeIndex::new`] and filled lazily, or by [`load_code_packages`].
/// Everything is keyed by package file, not by store id: the store may hand out several ids for one file, and the
/// index files them all under the first it met ([`CodeIndex::filed_id`], decision 2026-11-15-10). Import answers
/// are cached from the scan they were made in: after `scan()` the index still files second ids correctly, but an
/// import it answered before keeps that answer, a miss included.
#[derive(Debug, Default)]
pub struct CodeIndex {
    classes: Vec<ClassInfo>,
    by_export: FixedHashMap<(PackageId, i32), ClassKey>,
    ref_cache: FixedHashMap<(PackageId, i32), Option<(PackageId, i32)>>,
    indexes: FixedHashMap<PackageId, Rc<PackageIndex>>,
    layouts: FixedHashMap<ClassKey, Result<ClassId, ReflectError>>,
    by_class_id: FixedHashMap<ClassId, ClassKey>,
    class_in_progress: FixedHashSet<ClassKey>,
    structs: FixedHashMap<(PackageId, i32), Result<StructId, ReflectError>>,
    struct_in_progress: FixedHashMap<(PackageId, i32), StructId>,
    /// Structs reached only through an array or a map, declared and defined after the class that reached them.
    deferred_structs: Vec<(PackageId, i32)>,
    /// The structs each struct reached through an array or a map while it was defined (its deferred dependencies).
    struct_deps: FixedHashMap<(PackageId, i32), Vec<(PackageId, i32)>>,
    /// Structs whose deferred dependencies are all defined, transitively.
    complete_structs: FixedHashSet<(PackageId, i32)>,
    /// The structs being defined, innermost last.
    struct_stack: Vec<(PackageId, i32)>,
    property_in_progress: FixedHashSet<(PackageId, i32)>,
    enums: FixedHashMap<(PackageId, i32), Result<EnumId, ReflectError>>,
    /// Every package id this index met, and the id it files that package under: the first id it met for the same
    /// file with the same bytes. The store hands out several ids for one file (after `scan()`, and for a package its
    /// global fallback matched; decision 2026-11-15-05), and every cache here is keyed by the filed id, so one
    /// declaration reached under two ids is one class, one struct and one enum.
    filed_as: FixedHashMap<PackageId, PackageId>,
    /// The folded source path of each file met (as the store's path cache folds it), and the ids filed under it: one
    /// per distinct content, so a file rewritten between two loads keeps its two packages apart.
    by_file: FixedHashMap<String, Vec<PackageId>>,
    nesting: usize,
    max_nesting: usize,
}

/// Loads the nine code packages ([`CODE_PACKAGES`]) and every class they export, and defines each class's layout
/// in `classes`. A package that is not in the store fails the call. A class whose layout cannot be defined (a `NotYet`
/// declaration, a reference no package holds) does not: it stays declared, and [`CodeIndex::layout_of`] and
/// [`CodeIndex::layout_failures`] report it.
pub fn load_code_packages(
    store: &mut PackageStore,
    names: &mut NameTable,
    classes: &mut ClassTable,
) -> Result<CodeIndex, ReflectError> {
    let mut index = CodeIndex::new();
    let mut packages = Vec::with_capacity(CODE_PACKAGES.len());
    for name in CODE_PACKAGES {
        packages.push(index.package(store, name)?);
    }
    for package in packages {
        index.define_layouts(store, names, classes, package)?;
    }
    Ok(index)
}

impl CodeIndex {
    pub fn new() -> Self {
        CodeIndex::default()
    }

    /// Runs `f` one nesting level deeper, refused past [`MAX_NESTING`].
    fn nested<T>(
        &mut self,
        f: impl FnOnce(&mut Self) -> Result<T, ReflectError>,
    ) -> Result<T, ReflectError> {
        if self.nesting >= MAX_NESTING {
            return Err(ReflectError::TooDeep(MAX_NESTING));
        }
        self.nesting += 1;
        self.max_nesting = self.max_nesting.max(self.nesting);
        let result = f(self);
        self.nesting -= 1;
        result
    }

    /// The deepest nesting any call of this index reached so far (at most [`MAX_NESTING`]).
    pub fn max_nesting(&self) -> usize {
        self.max_nesting
    }

    /// The id `package` is filed under (see [`CodeIndex::filed_id`]), recorded on first meeting. A package with no
    /// source path (bytes given directly) is its own file.
    fn file(&mut self, store: &PackageStore, package: PackageId) -> PackageId {
        if let Some(&filed) = self.filed_as.get(&package) {
            return filed;
        }
        let pkg = store.get(package);
        let filed = if pkg.source.as_os_str().is_empty() {
            package
        } else {
            let ids = self.by_file.entry(file_key(&pkg.source)).or_default();
            match ids.iter().copied().find(|&other| {
                let other = store.get(other);
                other.name == pkg.name && other.data == pkg.data
            }) {
                Some(other) => other,
                None => {
                    ids.push(package);
                    package
                }
            }
        };
        self.filed_as.insert(package, filed);
        filed
    }

    /// The id this index files `package` under: the first id it met for the same file (the same source path,
    /// folded as the store's path cache folds it) with the same bytes. Every [`PackageId`] the index hands out
    /// (in [`ClassInfo::package`], [`CodeIndex::resolve_ref`], declaration sources) is a filed id. An id the index
    /// has not met yet is returned unchanged.
    pub fn filed_id(&self, package: PackageId) -> PackageId {
        self.filed_as.get(&package).copied().unwrap_or(package)
    }

    // Read-only, cache-only lookups (for a reflection typer behind `&self`): each answers only from what earlier
    // `&mut` calls loaded, and loads nothing. They take any id the index met for the package.

    /// The loaded class of export `index` of `package`, if [`CodeIndex::class_at`] loaded it.
    pub fn loaded_class(&self, package: PackageId, index: i32) -> Option<ClassKey> {
        self.by_export
            .get(&(self.filed_id(package), index))
            .copied()
    }

    /// [`CodeIndex::resolve_ref`] from the cache: `Some(answer)` where the answer is known without loading (0, an
    /// export, an import resolved before), `None` where it is not.
    pub fn cached_ref(
        &self,
        store: &PackageStore,
        package: PackageId,
        reference: i32,
    ) -> Option<Option<(PackageId, i32)>> {
        let filed = self.filed_id(package);
        match reference {
            0 => Some(None),
            r if r > 0 => {
                Some((r as usize <= store.get(package).exports.len()).then_some((filed, r)))
            }
            r => self.ref_cache.get(&(filed, r)).copied(),
        }
    }

    /// The [`ClassTable`] class defined from loaded class `key`, once its layout succeeded.
    pub fn layout_id(&self, key: ClassKey) -> Option<ClassId> {
        self.layouts
            .get(&key)
            .and_then(|o| o.as_ref().ok().copied())
    }

    /// The struct defined from ScriptStruct export `index` of `package`, once defined.
    pub fn struct_of(&self, package: PackageId, index: i32) -> Option<StructId> {
        self.structs
            .get(&(self.filed_id(package), index))
            .and_then(|o| o.as_ref().ok().copied())
    }

    /// The enum defined from Enum export `index` of `package`, once defined.
    pub fn enum_of(&self, package: PackageId, index: i32) -> Option<EnumId> {
        self.enums
            .get(&(self.filed_id(package), index))
            .and_then(|o| o.as_ref().ok().copied())
    }

    /// The loaded class `key`.
    ///
    /// # Panics
    /// When `key` was not made by this index (a caller bug).
    pub fn class(&self, key: ClassKey) -> &ClassInfo {
        &self.classes[key.index()]
    }

    /// Every class loaded so far, in load order.
    pub fn classes(&self) -> impl Iterator<Item = (ClassKey, &ClassInfo)> {
        self.classes
            .iter()
            .enumerate()
            .map(|(i, c)| (ClassKey(i as u32), c))
    }

    /// The package named `name` (C++ `Runtime::package`, `PackageStore::loadPackage`).
    pub fn package(
        &mut self,
        store: &mut PackageStore,
        name: &str,
    ) -> Result<PackageId, ReflectError> {
        let parsed = PackageName::parse(name)
            .map_err(|_| ReflectError::Store(StoreError::PackageNotFound(name.to_string())))?;
        let id = store.load(&parsed)?;
        Ok(self.file(store, id))
    }

    fn index_of(&mut self, store: &PackageStore, package: PackageId) -> Rc<PackageIndex> {
        let package = self.file(store, package);
        Rc::clone(
            self.indexes
                .entry(package)
                .or_insert_with(|| Rc::new(build_index(store.get(package)))),
        )
    }

    /// The class name of export `index` of `package` (`classNameOf`).
    pub fn class_name(&mut self, store: &PackageStore, package: PackageId, index: i32) -> String {
        let table = self.index_of(store, package);
        usize::try_from(index)
            .ok()
            .filter(|&i| i >= 1)
            .and_then(|i| table.class_names.get(i - 1))
            .cloned()
            .unwrap_or_else(|| "Class".to_string())
    }

    /// The first export of `package` at `object_path`, ASCII case folded, a leading `<package>.` dropped; 0 for none
    /// (C++ `Runtime::findExport`).
    pub fn find_export(
        &mut self,
        store: &PackageStore,
        package: PackageId,
        object_path: &str,
    ) -> i32 {
        let table = self.index_of(store, package);
        let mut wanted = lower(object_path);
        let prefix = format!("{}.", lower(&store.get(package).name));
        if wanted.starts_with(&prefix) {
            wanted.drain(..prefix.len());
        }
        table.by_path.get(&wanted).copied().unwrap_or(0)
    }

    /// Resolves `reference` of `package` to the package and export that hold the object (C++ `resolveRef`), cached.
    /// `None` where the target is in no installed package (some imports name engine-native objects such as
    /// `Core.Class`, which have no export). An import whose chain is all imports is looked up in the package its root
    /// names, without the store's global fallback; one nested in a local export goes through
    /// [`PackageStore::resolve`]. Every failure is "nothing", as the C++ catches it.
    pub fn resolve_ref(
        &mut self,
        store: &mut PackageStore,
        package: PackageId,
        reference: i32,
    ) -> Option<(PackageId, i32)> {
        if reference == 0 {
            return None;
        }
        let package = self.file(store, package);
        if reference > 0 {
            let exports = store.get(package).exports.len();
            return (reference as usize <= exports).then_some((package, reference));
        }
        if let Some(&cached) = self.ref_cache.get(&(package, reference)) {
            return cached;
        }
        let result = self
            .resolve_import(store, package, reference)
            .map(|(target, index)| (self.file(store, target), index));
        self.ref_cache.insert((package, reference), result);
        result
    }

    fn resolve_import(
        &mut self,
        store: &mut PackageStore,
        package: PackageId,
        reference: i32,
    ) -> Option<(PackageId, i32)> {
        let source = store.shared(package);
        let mut cursor = reference;
        let mut all_imports = true;
        let mut depth = 0;
        while cursor != 0 && depth < MAX_IMPORT_DEPTH {
            let object = source.object(cursor).ok()?;
            if cursor > 0 {
                all_imports = false;
                break;
            }
            if object.outer == 0 {
                break;
            }
            cursor = object.outer;
            depth += 1;
        }
        if all_imports && cursor < 0 {
            let root = source.object(cursor).ok()?.name.clone();
            let full = source.path_of(reference).ok()?;
            let target = self.package(store, &root).ok()?;
            let index = self.find_export(store, target, &full);
            (index != 0).then_some((target, index))
        } else {
            store.resolve(package, reference).ok().flatten()
        }
    }

    /// The class `reference` of `package` names, loaded on first use (C++ `Runtime::classAt`). `Ok(None)` where the
    /// reference resolves to nothing. A target that is not a Class export is an error ("export is not a class"), and
    /// so is a super chain that holds one, on the call that loads the class; as in the C++, the class stays registered
    /// without its super class and later calls return it.
    pub fn class_at(
        &mut self,
        store: &mut PackageStore,
        package: PackageId,
        reference: i32,
    ) -> Result<Option<ClassKey>, ReflectError> {
        let Some(target) = self.resolve_ref(store, package, reference) else {
            return Ok(None);
        };
        if let Some(&key) = self.by_export.get(&target) {
            return Ok(Some(key));
        }
        self.nested(|index| index.load_class(store, target.0, target.1))
            .map(Some)
    }

    /// C++ `loadClass`: registered before its super class loads, so a cyclic reference finds it.
    fn load_class(
        &mut self,
        store: &mut PackageStore,
        package: PackageId,
        index: i32,
    ) -> Result<ClassKey, ReflectError> {
        let pkg = store.shared(package);
        if self.class_name(store, package, index) != "Class" {
            return Err(ReflectError::NotAClass(pkg.path_of(index)?));
        }
        let exported = &pkg.exports[index as usize - 1];
        let key = ClassKey(u32::try_from(self.classes.len()).expect("fewer than 2^32 classes"));
        let body = decode_class_body(&pkg, index);
        let interface_targets = match &body {
            Ok(b) => vec![None; b.interfaces.len()],
            Err(_) => Vec::new(),
        };
        self.classes.push(ClassInfo {
            package,
            export: index,
            name: exported.name.clone(),
            path: format!("{}.{}", pkg.name, exported.name),
            super_class: None,
            body,
            functions: BTreeMap::new(),
            states: BTreeMap::new(),
            load_error: None,
            interface_targets,
        });
        self.by_export.insert((package, index), key);
        if exported.super_ != 0 {
            match self.class_at(store, package, exported.super_) {
                Ok(super_class) => self.classes[key.index()].super_class = super_class,
                Err(error) => {
                    self.classes[key.index()].load_error = Some(error.clone());
                    return Err(error);
                }
            }
        }
        let table = self.index_of(store, package);
        let mut functions = BTreeMap::new();
        let mut states = BTreeMap::new();
        let children = |of: i32| table.children.get(&of).map(Vec::as_slice).unwrap_or(&[]);
        for &child in children(index) {
            match table.class_names[child as usize - 1].as_str() {
                "Function" => {
                    functions.insert(lower(&pkg.exports[child as usize - 1].name), child);
                }
                "State" => {
                    let mut state = StateInfo {
                        name: pkg.exports[child as usize - 1].name.clone(),
                        export: child,
                        functions: BTreeMap::new(),
                    };
                    for &member in children(child) {
                        if table.class_names[member as usize - 1] == "Function" {
                            state
                                .functions
                                .insert(lower(&pkg.exports[member as usize - 1].name), member);
                        }
                    }
                    states.insert(lower(&state.name), state);
                }
                _ => {}
            }
        }
        self.classes[key.index()].functions = functions;
        self.classes[key.index()].states = states;
        Ok(key)
    }

    /// The class at `path` (`Package.Class`, C++ `Runtime::findClass`).
    pub fn find_class(
        &mut self,
        store: &mut PackageStore,
        path: &str,
    ) -> Result<ClassKey, ReflectError> {
        let Some((package, rest)) = path.split_once('.') else {
            return Err(ReflectError::ClassPath(path.to_string()));
        };
        let id = self.package(store, package)?;
        let index = self.find_export(store, id, rest);
        if index == 0 {
            return Err(ReflectError::ClassNotFound(path.to_string()));
        }
        self.class_at(store, id, index)?
            .ok_or_else(|| ReflectError::ClassNotFound(path.to_string()))
    }

    /// The class and its super classes, nearest first. A cyclic chain stops after every loaded class was visited.
    pub fn super_chain(&self, key: ClassKey) -> impl Iterator<Item = ClassKey> + '_ {
        let mut cursor = Some(key);
        let mut left = self.classes.len();
        std::iter::from_fn(move || {
            let here = cursor.filter(|_| left > 0)?;
            left -= 1;
            cursor = self.classes[here.index()].super_class;
            Some(here)
        })
    }

    /// Whether `class` is `base` or derives from it (C++ `Class::isChildOf`).
    pub fn is_child_of(&self, class: ClassKey, base: ClassKey) -> bool {
        self.super_chain(class).any(|c| c == base)
    }

    /// Whether the class chain of `class` declares a function named `name`, ASCII case folded (C++ `findMethod`
    /// with no state).
    pub fn find_method(&self, class: ClassKey, name: &str) -> Option<i32> {
        let key = lower(name);
        self.super_chain(class)
            .find_map(|c| self.classes[c.index()].functions.get(&key).copied())
    }

    /// C++ `Runtime::implements`: `iface` is an interface class and the interface table of `class` or of a super
    /// class names `iface` or a class derived from it (NATIVE_CLASS_SERIAL_LAYOUT.md, UNVERIFIED in the running
    /// game). Entries resolve on first use, one by one, as the C++ resolves them; an entry that fails to load is no
    /// class. A chain whose bodies did not all decode (synthetic packages) falls back to
    /// [`CodeIndex::implements_structurally`], as the C++ does.
    pub fn implements(
        &mut self,
        store: &mut PackageStore,
        class: ClassKey,
        iface: ClassKey,
    ) -> bool {
        let decoded = self.classes[iface.index()].body_decoded()
            && self
                .super_chain(class)
                .all(|c| self.classes[c.index()].body_decoded());
        if !decoded {
            return self.implements_structurally(class, iface);
        }
        if !self.classes[iface.index()].is_interface() {
            return false;
        }
        let chain: Vec<ClassKey> = self.super_chain(class).collect();
        for cursor in chain {
            let entries = self.classes[cursor.index()].interface_targets.len();
            for entry in 0..entries {
                let target = match self.classes[cursor.index()].interface_targets[entry] {
                    Some(target) => target,
                    None => {
                        let info = &self.classes[cursor.index()];
                        let (package, class_ref) = (
                            info.package,
                            info.body
                                .as_ref()
                                .map_or(0, |b| b.interfaces[entry].class_ref),
                        );
                        let target = self.class_at(store, package, class_ref).ok().flatten();
                        self.classes[cursor.index()].interface_targets[entry] = Some(target);
                        target
                    }
                };
                if target.is_some_and(|t| self.is_child_of(t, iface)) {
                    return true;
                }
            }
        }
        false
    }

    /// C++ `implementsStructurally`, the earlier stand-in kept as the fallback and the comparison baseline: the
    /// interface is named `I<Upper>...`, declares at least one function, and the class chain defines every one.
    pub fn implements_structurally(&self, class: ClassKey, iface: ClassKey) -> bool {
        let name = self.classes[iface.index()].name.as_bytes();
        if name.len() < 2 || name[0] != b'I' || !name[1].is_ascii_uppercase() {
            return false;
        }
        self.functions_only(class, iface)
    }

    /// The structural rule as NATIVE_CLASS_SERIAL_LAYOUT.md describes it (functions only, no name condition).
    pub fn functions_only(&self, class: ClassKey, iface: ClassKey) -> bool {
        let functions = &self.classes[iface.index()].functions;
        !functions.is_empty()
            && functions
                .keys()
                .all(|f| self.find_method(class, f).is_some())
    }

    /// C++ `checkClassBodies`: over every export of `package` with class reference 0, the body decodes and ends
    /// exactly at the serial size, the default object is `Default__<Class>` of that class, each interface entry
    /// names an interface class, and each table pointer is a `VfTable_` StructProperty child naming the interface.
    pub fn check_class_bodies(
        &mut self,
        store: &mut PackageStore,
        package: PackageId,
    ) -> Result<ClassCheck, ReflectError> {
        let package = self.file(store, package);
        let pkg = store.shared(package);
        let table = self.index_of(store, package);
        let mut check = ClassCheck {
            package: pkg.name.clone(),
            ..ClassCheck::default()
        };
        for (position, exported) in pkg.exports.iter().enumerate() {
            let index = position as i32 + 1;
            if exported.class != 0 {
                continue;
            }
            check.classes += 1;
            let path = pkg.path_of(index)?;
            let body = match decode_class_body(&pkg, index) {
                Ok(body) => body,
                Err(error) => {
                    check.failures.push(format!("{path}: {error}"));
                    continue;
                }
            };
            check.exact += 1;
            let default_ok = body.default_object > 0 && {
                let default = &pkg.exports[body.default_object as usize - 1];
                default.name == format!("Default__{}", exported.name) && default.class == index
            };
            if default_ok {
                check.default_objects += 1;
            } else {
                check.failures.push(format!(
                    "{path}: default object is not Default__{} of this class",
                    exported.name
                ));
            }
            if body.is_interface() {
                check.interface_classes += 1;
            }
            if !body.interfaces.is_empty() {
                check.classes_with_interfaces += 1;
            }
            for entry in &body.interfaces {
                check.interface_entries += 1;
                let target = self
                    .class_at(store, package, entry.class_ref)
                    .ok()
                    .flatten();
                let interface_name = &pkg.object(entry.class_ref)?.name;
                if target.is_some_and(|t| self.classes[t.index()].is_interface()) {
                    check.entries_to_interface_classes += 1;
                } else {
                    check.failures.push(format!(
                        "{path}: interface entry {interface_name} is not an interface class"
                    ));
                }
                if entry.table_property == 0 {
                    check.table_null += 1;
                    continue;
                }
                let property = pkg.object(entry.table_property)?;
                if entry.table_property > 0
                    && table.class_names[entry.table_property as usize - 1] == "StructProperty"
                    && property.outer == index
                    && property.name.starts_with("VfTable_")
                    && property.name.contains(interface_name.as_str())
                {
                    check.table_struct_properties += 1;
                } else {
                    check.failures.push(format!(
                        "{path}: table pointer {} is not a VfTable_ StructProperty child",
                        property.name
                    ));
                }
            }
        }
        Ok(check)
    }

    /// C++ `compareInterfaces`: every (class, interface) pair of the nine code packages, answered by the interface
    /// table ([`CodeIndex::implements`]), by the coded structural rule and by the functions-only rule. Classes whose
    /// body did not decode are left out, as the C++ leaves them out.
    pub fn compare_interfaces(
        &mut self,
        store: &mut PackageStore,
    ) -> Result<InterfaceComparison, ReflectError> {
        let mut interfaces = Vec::new();
        let mut subjects = Vec::new();
        for name in CODE_PACKAGES {
            let package = self.package(store, name)?;
            let exports = store.get(package).exports.len();
            for index in 1..=exports as i32 {
                if store.get(package).exports[index as usize - 1].class != 0 {
                    continue;
                }
                let Some(class) = self.class_at(store, package, index)? else {
                    continue;
                };
                let info = &self.classes[class.index()];
                if !info.body_decoded() {
                    continue;
                }
                if info.is_interface() {
                    interfaces.push(class);
                } else {
                    subjects.push(class);
                }
            }
        }
        let mut result = InterfaceComparison {
            classes: subjects.len(),
            interfaces: interfaces.len(),
            slice: SLICE_INTERFACES
                .iter()
                .map(|n| (n.to_string(), [0; 4]))
                .collect(),
            ..InterfaceComparison::default()
        };
        for &class in &subjects {
            for &iface in &interfaces {
                let table = self.implements(store, class, iface);
                let coded = self.implements_structurally(class, iface);
                let plain = self.functions_only(class, iface);
                match (table, coded) {
                    (true, true) => result.both_yes += 1,
                    (true, false) => result.table_only_yes += 1,
                    (false, true) => result.coded_only_yes += 1,
                    (false, false) => {}
                }
                match (table, plain) {
                    (true, true) => result.functions_only[0] += 1,
                    (true, false) => result.functions_only[1] += 1,
                    (false, true) => result.functions_only[2] += 1,
                    (false, false) => {}
                }
                if let Some(counts) = result.slice.get_mut(&self.classes[iface.index()].name) {
                    counts[if table { 0 } else { 1 }] += 1;
                    if table != coded {
                        counts[2] += 1;
                    }
                    if table != plain {
                        counts[3] += 1;
                    }
                }
            }
        }
        Ok(result)
    }

    // ------------------------------------------------------------------------------------------------ layouts

    /// The layout outcome of a loaded class, once [`CodeIndex::define_layout`] ran for it.
    pub fn layout_of(&self, key: ClassKey) -> Option<&Result<ClassId, ReflectError>> {
        self.layouts.get(&key)
    }

    /// The loaded class a [`ClassTable`] class was defined from.
    pub fn key_of(&self, class: ClassId) -> Option<ClassKey> {
        self.by_class_id.get(&class).copied()
    }

    /// Every class whose layout failed, in load order, with its error.
    pub fn layout_failures(&self) -> Vec<(&str, &ReflectError)> {
        self.classes()
            .filter_map(|(key, info)| match self.layouts.get(&key) {
                Some(Err(e)) => Some((info.path.as_str(), e)),
                _ => None,
            })
            .collect()
    }

    /// The typed declarations of `outer`'s own child property exports, in declaration order: a function's parameters,
    /// locals and return value, or any other export's children (the C++ `childProperties`, which `functionAt` reads
    /// for a function). Each is typed exactly as [`CodeIndex::define_layout`] types a class's own declarations (flags,
    /// array dimension, source export; a struct held by value defined now, one reached through an array or a map
    /// defined before this returns; a referenced class only declared). Attribute companions are not linked: those
    /// belong to a class's layout, which [`CodeIndex::define_layout`] builds. Nothing is cached: each call reads the
    /// exports again (the structs, enums and classes they name are cached as usual). On an error, the structs this
    /// call queued stay declared only, as after a class whose layout fails.
    pub fn child_declarations(
        &mut self,
        store: &mut PackageStore,
        names: &mut NameTable,
        classes: &mut ClassTable,
        package: PackageId,
        outer: i32,
    ) -> Result<Vec<PropertyDecl>, ReflectError> {
        let package = self.file(store, package);
        self.nested(|index| {
            let raws = index.child_properties(store, package, outer)?;
            let deferred_from = index.deferred_structs.len();
            let outcome = index.typed_children(store, names, classes, &raws, deferred_from);
            if outcome.is_err() {
                index.deferred_structs.truncate(deferred_from);
            }
            outcome
        })
    }

    fn typed_children(
        &mut self,
        store: &mut PackageStore,
        names: &mut NameTable,
        classes: &mut ClassTable,
        raws: &[RawProperty],
        deferred_from: usize,
    ) -> Result<Vec<PropertyDecl>, ReflectError> {
        let mut decls = Vec::with_capacity(raws.len());
        for raw in raws {
            decls.push(self.declaration(store, names, classes, raw)?);
        }
        while self.deferred_structs.len() > deferred_from {
            let target = self.deferred_structs.remove(deferred_from);
            self.define_struct(store, names, classes, target)?;
        }
        Ok(decls)
    }

    /// Loads every export of `package` whose class name is `Class` and defines its layout. Only a load failure that
    /// the C++ would throw (a super chain holding a non-class) ends the call; layout failures are counted in the
    /// summary and kept per class.
    pub fn define_layouts(
        &mut self,
        store: &mut PackageStore,
        names: &mut NameTable,
        classes: &mut ClassTable,
        package: PackageId,
    ) -> Result<LayoutSummary, ReflectError> {
        let package = self.file(store, package);
        let mut summary = LayoutSummary::default();
        let exports = store.get(package).exports.len() as i32;
        for index in 1..=exports {
            if self.class_name(store, package, index) != "Class" {
                continue;
            }
            let Some(key) = self.class_at(store, package, index)? else {
                continue;
            };
            summary.classes += 1;
            match self.define_layout(store, names, classes, key) {
                Ok(_) => summary.defined += 1,
                Err(error) => summary
                    .failures
                    .push((self.classes[key.index()].path.clone(), error)),
            }
        }
        Ok(summary)
    }

    /// Defines the layout of loaded class `key` in `classes` (once; later calls return the first outcome): the super
    /// class's layout first, then this class's own declarations, then its attribute companions. Each class, struct
    /// and enum path is `<package file stem>.<object path>`; a class a property refers to is only declared.
    pub fn define_layout(
        &mut self,
        store: &mut PackageStore,
        names: &mut NameTable,
        classes: &mut ClassTable,
        key: ClassKey,
    ) -> Result<ClassId, ReflectError> {
        if let Some(outcome) = self.layouts.get(&key) {
            return outcome.clone();
        }
        let path = self.classes[key.index()].path.clone();
        if !self.class_in_progress.insert(key) {
            return Err(ReflectError::SuperCycle(path));
        }
        let outcome = self
            .nested(|index| index.define_layout_inner(store, names, classes, key, &path))
            .map_err(in_class(&path));
        self.class_in_progress.remove(&key);
        if let Ok(id) = outcome {
            self.by_class_id.insert(id, key);
        }
        self.layouts.insert(key, outcome.clone());
        outcome
    }

    fn define_layout_inner(
        &mut self,
        store: &mut PackageStore,
        names: &mut NameTable,
        classes: &mut ClassTable,
        key: ClassKey,
        path: &str,
    ) -> Result<ClassId, ReflectError> {
        if let Some(error) = &self.classes[key.index()].load_error {
            return Err(error.clone());
        }
        let (package, export) = (
            self.classes[key.index()].package,
            self.classes[key.index()].export,
        );
        let super_id = match self.classes[key.index()].super_class {
            Some(s) => Some(self.define_layout(store, names, classes, s)?),
            None => {
                // The C++ loader leaves a super class it cannot resolve null; a layout without it would be wrong.
                let super_ref = store.get(package).exports[export as usize - 1].super_;
                if super_ref != 0 {
                    return Err(ReflectError::Unresolved {
                        property: path.to_string(),
                        what: "super class",
                        reference: super_ref,
                    });
                }
                None
            }
        };
        let raws = self.child_properties(store, package, export)?;
        let deferred_from = self.deferred_structs.len();
        let outcome = self.class_declarations(store, names, classes, path, super_id, &raws);
        if outcome.is_err() {
            // Structs this class deferred stay declared only; the next reference queues them again.
            self.deferred_structs.truncate(deferred_from);
        }
        outcome
    }

    /// The declarations of a class, their attribute companions, the structs they reached through arrays and maps,
    /// then the definition. Nothing is published in `classes` before every part succeeded: the companions are given
    /// to [`ClassTable::define`], which checks them and leaves the class undefined if they do not hold.
    fn class_declarations(
        &mut self,
        store: &mut PackageStore,
        names: &mut NameTable,
        classes: &mut ClassTable,
        path: &str,
        super_id: Option<ClassId>,
        raws: &[RawProperty],
    ) -> Result<ClassId, ReflectError> {
        let deferred_from = self.deferred_structs.len();
        let mut decls = Vec::with_capacity(raws.len());
        for raw in raws {
            decls.push(self.declaration(store, names, classes, raw)?);
        }
        // Slots as the table lays them out (decision D14): after the super class's, each declaration `array_dim`
        // long. `define` checks every companion against the declaration at its slot.
        let id = classes.declare(path)?;
        let mut cursor = super_id.map_or(0, |s| classes.layout(s).slot_count());
        let mut slots = Vec::with_capacity(raws.len());
        for raw in raws {
            slots.push(cursor);
            cursor = cursor.saturating_add(raw.array_dim as u32);
        }
        for (position, raw) in raws.iter().enumerate() {
            if let Some((stack, base)) = self.companions(store, raws, raw)? {
                let prop = |at: usize| PropId {
                    slot: slots[at],
                    declared_in: id,
                };
                let attr = AttrCompanions {
                    value: prop(position),
                    base: prop(base),
                    stack: prop(stack),
                };
                decls[position].attr = Some(attr);
                decls[base].attr = Some(attr);
            }
        }
        while self.deferred_structs.len() > deferred_from {
            let target = self.deferred_structs.remove(deferred_from);
            self.define_struct(store, names, classes, target)?;
        }
        Ok(classes.define(path, super_id, decls)?)
    }

    /// The own declarations an attribute value property (flag bit 63) names as its modifier stack and its base, by
    /// position in `raws`: its first two references, shifted by one for a byte attribute (enum first). Each must
    /// resolve to one of `raws` itself (the same package and export), never a property that only shares a name.
    fn companions(
        &mut self,
        store: &mut PackageStore,
        raws: &[RawProperty],
        raw: &RawProperty,
    ) -> Result<Option<(usize, usize)>, ReflectError> {
        let byte = raw.class == "ByteAttributeProperty";
        let attribute =
            byte || raw.class == "FloatAttributeProperty" || raw.class == "IntAttributeProperty";
        if !attribute || raw.flags & PropertyDecl::STACK_ATTRIBUTE == 0 {
            return Ok(None);
        }
        let (stack_ref, base_ref) = if byte {
            (raw.refs[1], raw.refs[2])
        } else {
            (raw.refs[0], raw.refs[1])
        };
        let mut own = |reference: i32, what: &'static str| {
            if reference == 0 {
                return Err(ReflectError::NoReference {
                    property: raw.name.clone(),
                    what,
                });
            }
            let target = self
                .resolve_ref(store, raw.package, reference)
                .ok_or_else(|| ReflectError::Unresolved {
                    property: raw.name.clone(),
                    what,
                    reference,
                })?;
            raws.iter()
                .position(|r| (r.package, r.index) == target)
                .ok_or_else(|| ReflectError::CompanionNotOwn {
                    property: raw.name.clone(),
                    what,
                    target,
                })
        };
        let stack = own(stack_ref, "modifier stack")?;
        let base = own(base_ref, "base value")?;
        Ok(Some((stack, base)))
    }

    /// The property export `reference` of `raw`'s package names (C++ `declAt`).
    fn referenced_property(
        &mut self,
        store: &mut PackageStore,
        raw: &RawProperty,
        reference: i32,
        what: &'static str,
    ) -> Result<RawProperty, ReflectError> {
        if reference == 0 {
            return Err(ReflectError::NoReference {
                property: raw.name.clone(),
                what,
            });
        }
        let (package, index) =
            self.resolve_ref(store, raw.package, reference)
                .ok_or_else(|| ReflectError::Unresolved {
                    property: raw.name.clone(),
                    what,
                    reference,
                })?;
        self.read_property(store, package, index)
    }

    /// C++ `childProperties`: the child exports of `outer` whose class name ends in `Property`, in descending export
    /// order, each read with [`CodeIndex::read_property`]; one smaller than the header is not a declaration.
    fn child_properties(
        &mut self,
        store: &mut PackageStore,
        package: PackageId,
        outer: i32,
    ) -> Result<Vec<RawProperty>, ReflectError> {
        let table = self.index_of(store, package);
        let mut found: Vec<i32> = table
            .children
            .get(&outer)
            .map(Vec::as_slice)
            .unwrap_or(&[])
            .iter()
            .copied()
            .filter(|&c| table.class_names[c as usize - 1].ends_with("Property"))
            .collect();
        found.sort_unstable_by(|a, b| b.cmp(a));
        let mut result = Vec::with_capacity(found.len());
        for index in found {
            match self.read_property(store, package, index) {
                Ok(raw) => result.push(raw),
                Err(ReflectError::NotAProperty) => {} // a property-class default object, not a declaration
                Err(e) => return Err(e),
            }
        }
        Ok(result)
    }

    /// C++ `readProperty`: the 40-byte header and up to three type references inside the export.
    fn read_property(
        &mut self,
        store: &PackageStore,
        package: PackageId,
        index: i32,
    ) -> Result<RawProperty, ReflectError> {
        let class = self.class_name(store, package, index);
        let pkg = store.get(package);
        let object = usize::try_from(index)
            .ok()
            .filter(|&i| i >= 1)
            .and_then(|i| pkg.exports.get(i - 1))
            .ok_or(ReflectError::NotAProperty)?;
        if object.size < PROPERTY_HEADER as i32 {
            return Err(ReflectError::NotAProperty);
        }
        let at = usize::try_from(object.offset).map_err(|_| ReflectError::PastData)?;
        let end = at + object.size as usize;
        let u32_at = |offset: usize| -> Result<u32, ReflectError> {
            pkg.data
                .get(offset..offset + 4)
                .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                .ok_or(ReflectError::PastData)
        };
        let array_dim = ((u32_at(at + 16)? & 0xFFFF) as i32).max(1);
        let flags = u64::from(u32_at(at + 20)?) | (u64::from(u32_at(at + 24)?) << 32);
        let type_at = at + PROPERTY_HEADER + if flags & CPF_NET != 0 { 2 } else { 0 };
        let mut refs = [0i32; 3];
        for (slot, value) in refs.iter_mut().enumerate() {
            let word = type_at + 4 * slot;
            if word + 4 > end {
                break;
            }
            *value = u32_at(word)? as i32;
        }
        Ok(RawProperty {
            package,
            index,
            name: object.name.clone(),
            class,
            flags,
            array_dim,
            refs,
        })
    }

    /// A typed declaration from a raw one.
    fn declaration(
        &mut self,
        store: &mut PackageStore,
        names: &mut NameTable,
        classes: &mut ClassTable,
        raw: &RawProperty,
    ) -> Result<PropertyDecl, ReflectError> {
        let kind = self.kind(store, names, classes, raw, true)?;
        Ok(PropertyDecl::new(names.intern(&raw.name), kind)
            .with_flags(raw.flags)
            .with_array_dim(raw.array_dim)
            .with_source(raw.package, raw.index))
    }

    /// The kind a property class and its references give. `by_value` is false inside an array, where a struct that
    /// is being defined (an array of itself) needs only its declaration.
    fn kind(
        &mut self,
        store: &mut PackageStore,
        names: &mut NameTable,
        classes: &mut ClassTable,
        raw: &RawProperty,
        by_value: bool,
    ) -> Result<PropKind, ReflectError> {
        Ok(match raw.class.as_str() {
            "IntProperty" => PropKind::Int,
            "FloatProperty" => PropKind::Float,
            "BoolProperty" => PropKind::Bool,
            "StrProperty" => PropKind::Str,
            "NameProperty" => PropKind::Name,
            "DelegateProperty" => PropKind::Delegate,
            "FloatAttributeProperty" => PropKind::FloatAttribute,
            "IntAttributeProperty" => PropKind::IntAttribute,
            "ByteProperty" => PropKind::Byte {
                enum_: self.enum_ref(store, names, classes, raw, raw.refs[0])?,
            },
            "ByteAttributeProperty" => PropKind::ByteAttribute {
                enum_: self.enum_ref(store, names, classes, raw, raw.refs[0])?,
            },
            "ObjectProperty" | "ComponentProperty" => PropKind::Object {
                class: self.class_ref(store, classes, raw, raw.refs[0], "class")?,
            },
            "InterfaceProperty" => PropKind::Interface {
                class: self.class_ref(store, classes, raw, raw.refs[0], "interface class")?,
            },
            "ClassProperty" => PropKind::Class {
                meta: self.class_ref(store, classes, raw, raw.refs[1], "metaclass")?,
            },
            "StructProperty" => {
                PropKind::Struct(self.struct_ref(store, names, classes, raw, by_value)?)
            }
            "ArrayProperty" | "MapProperty" => {
                return self.expanding(raw, |index| {
                    index.container_kind(store, names, classes, raw)
                });
            }
            other => return Err(ReflectError::UnknownPropertyClass(other.to_string())),
        })
    }

    /// Runs `f` with `raw` marked as being expanded, one nesting level deeper; a property already being expanded is a
    /// cycle (an array whose inner declaration leads back to it).
    fn expanding<T>(
        &mut self,
        raw: &RawProperty,
        f: impl FnOnce(&mut Self) -> Result<T, ReflectError>,
    ) -> Result<T, ReflectError> {
        let key = (raw.package, raw.index);
        if !self.property_in_progress.insert(key) {
            return Err(ReflectError::PropertyCycle(raw.name.clone()));
        }
        let result = self.nested(f);
        self.property_in_progress.remove(&key);
        result
    }

    /// The kind of an array (its inner declaration) or a map (key, then value; the order is UNVERIFIED: no C++ code
    /// reads them, and every map property of the nine code packages names neither, so both are `None` there).
    fn container_kind(
        &mut self,
        store: &mut PackageStore,
        names: &mut NameTable,
        classes: &mut ClassTable,
        raw: &RawProperty,
    ) -> Result<PropKind, ReflectError> {
        if raw.class == "ArrayProperty" {
            let inner = self.referenced_property(store, raw, raw.refs[0], "inner property")?;
            return Ok(PropKind::Array(Box::new(
                self.kind(store, names, classes, &inner, false)?,
            )));
        }
        let mut inner = [None, None];
        for (slot, &reference) in inner.iter_mut().zip(&raw.refs[..2]) {
            if reference != 0 {
                let declared = self.referenced_property(store, raw, reference, "map property")?;
                *slot = Some(Box::new(
                    self.kind(store, names, classes, &declared, false)?,
                ));
            }
        }
        let [key, value] = inner;
        Ok(PropKind::Map { key, value })
    }

    /// `<package stem>.<object path>` of export `index`.
    fn object_path(
        store: &PackageStore,
        package: PackageId,
        index: i32,
    ) -> Result<String, ReflectError> {
        let pkg = store.get(package);
        Ok(format!("{}.{}", pkg.name, pkg.path_of(index)?))
    }

    /// The class a reference names, declared in `classes`: a resolved Class export is `<stem>.<name>`; an import no
    /// installed package holds (an engine-native class such as `Core.Class`) is declared by its import path.
    fn class_ref(
        &mut self,
        store: &mut PackageStore,
        classes: &mut ClassTable,
        raw: &RawProperty,
        reference: i32,
        what: &'static str,
    ) -> Result<ClassId, ReflectError> {
        if reference == 0 {
            return Err(ReflectError::NoReference {
                property: raw.name.clone(),
                what,
            });
        }
        let path = match self.resolve_ref(store, raw.package, reference) {
            Some((package, index)) => {
                let found = self.class_name(store, package, index);
                if found != "Class" {
                    return Err(ReflectError::WrongTarget {
                        property: raw.name.clone(),
                        what,
                        found,
                    });
                }
                let pkg = store.get(package);
                format!("{}.{}", pkg.name, pkg.exports[index as usize - 1].name)
            }
            None if reference < 0 => store.get(raw.package).path_of(reference)?,
            None => {
                return Err(ReflectError::Unresolved {
                    property: raw.name.clone(),
                    what,
                    reference,
                });
            }
        };
        Ok(classes.declare(&path)?)
    }

    /// The enum a byte property names (`None` for reference 0), read and defined on first use.
    fn enum_ref(
        &mut self,
        store: &mut PackageStore,
        names: &mut NameTable,
        classes: &mut ClassTable,
        raw: &RawProperty,
        reference: i32,
    ) -> Result<Option<EnumId>, ReflectError> {
        if reference == 0 {
            return Ok(None);
        }
        let target = self
            .resolve_ref(store, raw.package, reference)
            .ok_or_else(|| ReflectError::Unresolved {
                property: raw.name.clone(),
                what: "enum",
                reference,
            })?;
        let found = self.class_name(store, target.0, target.1);
        if found != "Enum" {
            return Err(ReflectError::WrongTarget {
                property: raw.name.clone(),
                what: "enum",
                found,
            });
        }
        if let Some(outcome) = self.enums.get(&target) {
            return outcome.clone().map(Some);
        }
        let outcome = Self::object_path(store, target.0, target.1).and_then(|path| {
            read_enum(store.get(target.0), target.1)
                .and_then(|values| {
                    let ids = values.iter().map(|v| names.intern(v)).collect();
                    Ok(classes.define_enum(&path, ids)?)
                })
                .map_err(|error| ReflectError::InEnum {
                    path: path.clone(),
                    error: Box::new(error),
                })
        });
        self.enums.insert(target, outcome.clone());
        outcome.map(Some)
    }

    /// The struct a struct property names: defined now when held by value (its super struct and by-value fields
    /// first), only declared and queued when reached through an array or a map.
    fn struct_ref(
        &mut self,
        store: &mut PackageStore,
        names: &mut NameTable,
        classes: &mut ClassTable,
        raw: &RawProperty,
        by_value: bool,
    ) -> Result<StructId, ReflectError> {
        let reference = raw.refs[0];
        if reference == 0 {
            return Err(ReflectError::NoReference {
                property: raw.name.clone(),
                what: "struct",
            });
        }
        let target = self
            .resolve_ref(store, raw.package, reference)
            .ok_or_else(|| ReflectError::Unresolved {
                property: raw.name.clone(),
                what: "struct",
                reference,
            })?;
        let found = self.class_name(store, target.0, target.1);
        if found != "ScriptStruct" {
            return Err(ReflectError::WrongTarget {
                property: raw.name.clone(),
                what: "struct",
                found,
            });
        }
        if !by_value
            && let Some(&current) = self.struct_stack.last()
            && current != target
        {
            let deps = self.struct_deps.entry(current).or_default();
            if !deps.contains(&target) {
                deps.push(target);
            }
        }
        if let Some(outcome) = self.structs.get(&target).cloned() {
            if outcome.is_ok() {
                self.complete_deps(target)?;
            }
            return outcome;
        }
        if let Some(&id) = self.struct_in_progress.get(&target) {
            if by_value {
                return Err(ReflectError::StructCycle(Self::object_path(
                    store, target.0, target.1,
                )?));
            }
            return Ok(id);
        }
        if by_value {
            return self.define_struct(store, names, classes, target);
        }
        // Reached through an array or a map: only the declaration is needed now. The struct is defined after the
        // class that reached it, so a cycle is checked along by-value edges only, whatever the load order.
        let id = classes.declare_struct(&Self::object_path(store, target.0, target.1)?)?;
        if !self.deferred_structs.contains(&target) {
            self.deferred_structs.push(target);
        }
        Ok(id)
    }

    /// C++ `structAt` as a [`ClassTable`] definition: the super struct, then the struct's own child properties.
    fn define_struct(
        &mut self,
        store: &mut PackageStore,
        names: &mut NameTable,
        classes: &mut ClassTable,
        target: (PackageId, i32),
    ) -> Result<StructId, ReflectError> {
        if let Some(outcome) = self.structs.get(&target).cloned() {
            if outcome.is_ok() {
                self.complete_deps(target)?;
            }
            return outcome;
        }
        let path = Self::object_path(store, target.0, target.1)?;
        let declared = classes.declare_struct(&path)?;
        self.struct_in_progress.insert(target, declared);
        self.struct_stack.push(target);
        let outcome = self
            .nested(|index| index.define_struct_inner(store, names, classes, target, &path))
            .map_err(in_struct(&path));
        self.struct_stack.pop();
        self.struct_in_progress.remove(&target);
        self.structs.insert(target, outcome.clone());
        outcome
    }

    /// Makes sure every struct a defined struct reached through an array or a map (transitively) is defined, or
    /// queued to be defined before the current class is published: a struct is cached once defined, but the deferred
    /// definitions it queued are dropped when the class that reached it fails. A dependency whose definition failed
    /// passes its error on, so no class is published holding a struct whose array element cannot be defined.
    fn complete_deps(&mut self, target: (PackageId, i32)) -> Result<(), ReflectError> {
        if self.complete_structs.contains(&target) {
            return Ok(());
        }
        let mut pending = vec![target];
        let mut seen: FixedHashSet<(PackageId, i32)> = FixedHashSet::default();
        seen.insert(target);
        let mut complete = true;
        while let Some(at) = pending.pop() {
            for dep in self.struct_deps.get(&at).cloned().unwrap_or_default() {
                if !seen.insert(dep) {
                    continue;
                }
                match self.structs.get(&dep) {
                    Some(Ok(_)) => {
                        if !self.complete_structs.contains(&dep) {
                            pending.push(dep);
                        }
                    }
                    Some(Err(error)) => return Err(error.clone()),
                    None => {
                        complete = false;
                        if !self.struct_in_progress.contains_key(&dep)
                            && !self.deferred_structs.contains(&dep)
                        {
                            self.deferred_structs.push(dep);
                        }
                    }
                }
            }
        }
        if complete {
            self.complete_structs.insert(target);
        }
        Ok(())
    }

    fn define_struct_inner(
        &mut self,
        store: &mut PackageStore,
        names: &mut NameTable,
        classes: &mut ClassTable,
        target: (PackageId, i32),
        path: &str,
    ) -> Result<StructId, ReflectError> {
        let super_ref = store.get(target.0).exports[target.1 as usize - 1].super_;
        let super_id = if super_ref == 0 {
            None
        } else {
            let parent = self
                .resolve_ref(store, target.0, super_ref)
                .ok_or_else(|| ReflectError::Unresolved {
                    property: path.to_string(),
                    what: "super struct",
                    reference: super_ref,
                })?;
            if self.struct_in_progress.contains_key(&parent) {
                return Err(ReflectError::StructCycle(path.to_string()));
            }
            Some(self.define_struct(store, names, classes, parent)?)
        };
        let raws = self.child_properties(store, target.0, target.1)?;
        let mut fields = Vec::with_capacity(raws.len());
        for raw in &raws {
            fields.push(self.declaration(store, names, classes, raw)?);
        }
        Ok(classes.define_struct(path, super_id, fields)?)
    }
}

/// The values of Enum export `index`: `[net index][None][next][count][count names]`, count at most
/// [`MAX_ENUM_COUNT`], ending exactly at the serial size.
fn read_enum(package: &Package, index: i32) -> Result<Vec<String>, ReflectError> {
    let object = package.object(index)?;
    let start = usize::try_from(object.offset).map_err(|_| ReflectError::PastData)?;
    let size = usize::try_from(object.size).map_err(|_| ReflectError::PastData)?;
    let mut r = Reader::new(&package.data).with_limit(start + size);
    r.set_pos(start + 16);
    let count = r.i32()?;
    if !(0..=MAX_ENUM_COUNT).contains(&count) {
        return Err(ReflectError::EnumCount);
    }
    let mut values = Vec::with_capacity(count as usize);
    for _ in 0..count {
        values.push(tables::read_name(&mut r, &package.names)?);
    }
    if r.pos() != r.limit() {
        return Err(ReflectError::EnumTrailing);
    }
    Ok(values)
}

#[cfg(test)]
mod tests;
