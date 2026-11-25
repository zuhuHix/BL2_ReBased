//! Owner: lane W3-DECODE. Typed values from tagged properties (architecture section 3.3): the port of the C++ VM's
//! tag reader (`TagReader`, `applyTaggedDefaults`, `applyStructDefaults`, `Runtime::resource` in `src/vm.cpp`), typed
//! through the reflection layouts of [`crate::reflect`] instead of the C++ declaration bag.
//!
//! **What is read, and how.** [`ValueCx::apply_tags`] walks one export's tag stream from its property prefix to the
//! `None` tag and writes each value into an object's slots, as the C++ `applyTaggedDefaults` does: a tag's header
//! (name, type, size, array index; the struct or enum name of a struct or byte tag; a bool's byte) is read and
//! checked, its value is read inside a limit of exactly `size` bytes, and a value that fails to decode, or does not
//! end where the tag said, is skipped with the C++ log line (`defaults: skipped <Class>.<Property> (<Type>)`) without
//! upsetting the rest. A header that cannot be read ends the stream with an error; otherwise the call returns where
//! the stream ended (the start of any untagged tail). The value kinds are the C++ VM's: int, float, the float and int
//! attribute tags (one 4-byte number each), bool (its byte, any non-zero is true), name, string, object, component
//! and interface references (the object the [`ReferenceResolver`] gives; by default a stand-in per referenced
//! object, [`ValueCx::resource`]), class references, plain bytes and enum bytes (the value's position in the declared
//! enum), structs and dynamic arrays, whose elements are read by the inner declaration's kind. Other tag types
//! (delegates, maps, byte attributes) are skipped and logged, as in the C++.
//!
//! **Array element widths, by parity mode.** [`ParityMode::Reference`] reads an enum-byte element as one byte and a
//! bool element as a 4-byte word, as the C++ VM does. [`ParityMode::Off`] reads them as the installed packages store
//! them (FITTED from sizes that meet, UNVERIFIED in the game): an enum-byte element as an 8-byte name placed in the
//! declared enum (19 of 19 such array tags in the code packages' class defaults), a bool element as one byte (the one
//! bool array with elements in their struct defaults: 3 elements in 7 bytes). With the C++ widths those arrays do not
//! meet their tags' sizes, so the C++ VM skips them.
//!
//! **Structs.** A struct tag whose struct name is one of the eight in [`native_layout`] holds its fields untagged, in
//! the C++ order (`Plane` as W, X, Y, Z, `Color` as B, G, R, A); the fields are placed by name into the declared
//! layout, so a value is always in [`StructLayout`](crate::layout::StructLayout) order and only printing follows
//! the C++ order. Every other struct starts from its defaults ([`ValueCx::ensure_struct_defaults`]) and reads a
//! nested tag stream over them, where a field that fails is skipped silently and a header that fails fails the
//! whole struct (the C++ `tagged`). Box, Matrix and the other fixed-layout structs that the package walker
//! (`ow_package::props`) knows but the C++ VM does not are read as tagged streams, fail, and are skipped and logged,
//! as in the C++; their values stay at the declared defaults. Struct nesting deeper than [`MAX_STRUCT_DEPTH`] fails.
//!
//! **Struct defaults.** A ScriptStruct export ends with a tagged stream of the struct's default values after a
//! 52-byte header whose word at +44 is 0 (FITTED, `src/vm.cpp`; 1,275 of 1,275 ScriptStructs of the code packages).
//! It is applied only when the whole stream is consumed exactly; otherwise the zero value stands. Native structs
//! take no defaults. Defaults are read once per struct and stored in the [`ClassTable`] (inner by-value structs
//! first), so every object allocated after that starts from them.
//!
//! **Class properties.** In these packages a class property's tag says `ObjectProperty`, the reference naming the
//! class (every non-null class value of the code packages' class defaults is tagged so). The C++ VM decodes by the
//! tag's type and keeps an object stand-in in the class property; here the declaration decides and the value is the
//! class ([`Value::Class`]). The CLI therefore prints `class:<Package>.<Class>` where the C++ prints the class's
//! object path (decision 2026-11-15-08).
//!
//! **References and stand-ins.** A [`ReferenceResolver`] on the context decides what a decoded reference points at
//! (a definition store answers with the objects it loads). Resolver calls nest at most [`MAX_RESOLVE_DEPTH`] deep, for
//! every resolver ([`DecodeError::ResolveTooDeep`] past it, the counter restored on every path), and one top-level load
//! ([`ValueCx::load`]: a class default object or an instantiated export, with what a loading resolver decodes inside)
//! makes at most [`MAX_RESOLVES_PER_LOAD`] resolver calls; past that the load aborts ([`DecodeError::ResolveBudget`],
//! which no stream skips) and the objects it allocated are freed. An export reached again while it is being
//! instantiated is [`DecodeError::InstantiationCycle`] for that reference. The default,
//! [`StandIns`], gives each referenced object one heap object of its own class with [`Origin::Resource`] and no
//! property slots ([`ObjectSpace::alloc_stand_in`]), as the C++ stand-in holds no properties. A reference outside its
//! package's tables gets, under [`ParityMode::Reference`], the C++ stand-in named `?` (class `Core.Object`), whose path
//! cannot be printed; in [`ParityMode::Off`] it is an error. An object whose class no installed package exports (an
//! engine-native class such as `Engine.StaticMesh`, or `Core.Class` for a class object) takes a layout something else
//! defined at that class's path (an overlay, a native pack) in either mode; failing that, under
//! [`ParityMode::Reference`] only, the class is defined at its import path with no properties of its own under
//! `Core.Object` ([`DecodeState::native_classes`], [`ClassTable::is_native`]): nothing about its native
//! properties is invented, and the classes between it and `Core.Object`, if any, are UNVERIFIED. In
//! [`ParityMode::Off`] such a reference is a [`NotYet`] (the class's layout is not read) and the property is skipped.
//! Any other failure to find or lay out a class is returned as it is. A class an overlay or a native pack defines must
//! be defined before any decode.
//!
//! **Differences from the C++, by parity mode.** Besides the element widths and the engine-native classes above,
//! [`ParityMode::Reference`] keeps three C++ behaviours that [`ParityMode::Off`] does not (architecture section 6.1):
//! an array with elements inside a struct's default stream stays empty (the C++ reads those streams with no array
//! typing; Off types them through reflection), an enum value name the enum lacks gives 0 (Off refuses it and skips
//! the property), and an enum value is placed by the C++ `byteFromEnum` and `enumPosition` rules: the first Enum
//! export of the package being read with the tag's enum name, else the enum a `ByteProperty` declaration names when its
//! name matches, else 0; there, the enum export's own entries in their package's spelling and without their numbers
//! against the value's whole text, the position kept to 8 bits, so a numbered value (`EPSM_4`, stored as `EPSM` and a
//! number) matches nothing and gives 0 (Off finds its position in the declared enum). With those, `--object-dump`
//! can be compared with the C++ byte for byte except where slots cannot follow the bag (below) and for class
//! properties (above).
//!
//! **Struct defaults that need themselves.** Only Off reads arrays in default streams, so only there can a struct's
//! defaults wait on its own (`A { array<B> }` with `B { array<A> }`), or on a chain of others longer than
//! [`MAX_DEFAULTS_NESTING`]. Such defaults are refused: every struct whose stream met the cycle or the bound, swallowed
//! or not, fails with [`DecodeError::StructDefaultsCycle`] (or [`DecodeError::DefaultsTooDeep`]) and publishes
//! nothing; for a ring this holds whichever struct is asked first. A straight chain (no ring) longer than the bound is
//! the one order-dependent case: asked from its head it meets the bound and is refused, asked from far enough down it
//! fits and is read (a Rust-only Off-mode bound). A value of a refused struct, or a class holding one by value, fails
//! with it.
//! Each struct's and enum's export is the one its declaring property's type reference names, recorded when a class's
//! layout is first used, so a second package id for the same file still finds it.
//!
//! **What the C++ does that slots cannot.** The C++ object is a name-keyed bag, so a tag naming a property the class
//! does not declare (an editor-only property stripped from the cooked class) is stored under its name. A slot layout
//! has nowhere to put it: such a tag is dropped here. When the C++ would have failed to decode it (an array with
//! elements and no declaration, an unsupported type), the same skip line is logged; when the C++ would have decoded
//! it, nothing is logged and [`DecodeState::dropped`] records it. A value whose kind does not fit the declaration
//! (which the C++ bag would also take) is skipped and logged.
//!
//! **Enums.** In [`ParityMode::Off`] an enum byte is the position of its value name in the enum the declaration
//! names; a position past 255 is an error. The reference mode follows the C++ lookup (above).
//!
//! **Also here:** [`to_value`], a [`PropValue`] tree of the package walker typed by a declaration, and
//! [`ReflectionTyper`], the walker's [`ArrayTyper`] answered from reflection.
//!
//! Status: parity with the C++ VM's loading half. The tag formats are the C++ ones, observations of 832/46 packages;
//! none is confirmed against the running game (UNVERIFIED).

use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

use ow_core::{FixedHashMap, FixedHashSet, NotYet, ParityMode};
use ow_package::PackageId;
use ow_package::package::{Package, PackageError};
use ow_package::props::{ArrayScope, ArrayTyper, ElementType, PropValue};
use ow_package::reader::{ReadError, Reader};
use ow_package::store::{PackageStore, StoreError};
use ow_package::tables;

use crate::heap::{HeapError, ObjectSpace, Origin};
use crate::ids::{ClassId, EnumId, NameId, ObjRef, PropAddr, PropId, StructId};
use crate::layout::{ClassTable, LayoutError, PropKind, PropertyDecl};
use crate::names::NameTable;
use crate::reflect::{ClassKey, CodeIndex, ReflectError};
use crate::value::Value;

/// Structs nest at most this deep: a struct value at a deeper level fails with "struct nesting too deep" (the
/// C++ `depth > 16` of `structure` and `tagged`).
pub const MAX_STRUCT_DEPTH: u32 = 16;

/// The largest array length a tag may give (C++ "invalid array length").
pub const MAX_ARRAY_LENGTH: i32 = 1_000_000;

/// Where a ScriptStruct export's default tags start: a 48-byte header and the struct flags (FITTED, `src/vm.cpp`).
pub const STRUCT_DEFAULTS_START: usize = 52;

/// The header word that must be 0 for a ScriptStruct export to hold default tags (FITTED).
pub const STRUCT_DEFAULTS_ZERO_WORD: usize = 44;

/// How deeply the struct defaults of one struct may wait on those of others (through arrays of structs in a default
/// stream). A Rust-only bound: the C++ never reads such arrays (module documentation), so it has none.
pub const MAX_DEFAULTS_NESTING: usize = 64;

/// How deeply [`ReferenceResolver`] calls may nest (a resolver that decodes the target asks again for the references
/// found there). A Rust-only bound shared by every resolver: past it a reference is [`DecodeError::ResolveTooDeep`],
/// so a chain of lazily loaded definitions that leads back to itself ends instead of overflowing the stack.
pub const MAX_RESOLVE_DEPTH: usize = 64;

/// How many [`ReferenceResolver`] calls one top-level load may make ([`DecodeState::resolve_budget`]): one
/// `cdo::class_default` or `cdo::instantiate_export` call with everything a loading resolver decodes inside it. A
/// Rust-only bound: a graph of definitions that branches back on itself cannot grow without end (the nesting bound
/// alone would let each level fan out). Past it the load aborts with [`DecodeError::ResolveBudget`], which no stream
/// swallows, and the objects it allocated are freed.
pub const MAX_RESOLVES_PER_LOAD: usize = 1 << 16;

/// The decision file that records this module's rules (named by `NotYet` values).
pub const RECORD: &str = "docs/decisions/2026-11/2026-11-15-08-rust-typed-values-class-defaults-and-object-dumps-contract.md";

/// The kind of every field of a [native struct](native_layout).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeField {
    /// A 4-byte float.
    Float,
    /// A 4-byte int.
    Int,
    /// One byte.
    Byte,
}

/// The eight structs the C++ VM reads without tags (`nativeStruct` in `src/vm.cpp`): their field names in stored order
/// and the kind every field has. Matched by exact struct name. The orders are the C++ VM's observations
/// (UNVERIFIED in the game); `Plane` is stored W, X, Y, Z there.
pub fn native_layout(name: &str) -> Option<(&'static [&'static str], NativeField)> {
    Some(match name {
        "Vector" => (&["X", "Y", "Z"], NativeField::Float),
        "Vector2D" => (&["X", "Y"], NativeField::Float),
        "Rotator" => (&["Pitch", "Yaw", "Roll"], NativeField::Int),
        "Guid" => (&["A", "B", "C", "D"], NativeField::Int),
        "LinearColor" => (&["R", "G", "B", "A"], NativeField::Float),
        "Color" => (&["B", "G", "R", "A"], NativeField::Byte),
        "Quat" => (&["X", "Y", "Z", "W"], NativeField::Float),
        "Plane" => (&["W", "X", "Y", "Z"], NativeField::Float),
        _ => return None,
    })
}

/// Why a value, an object or a stream could not be decoded. `Display` gives the C++ text where the C++ has one (the
/// CLI prints those, and the class-default log carries them); wrappers are transparent.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum DecodeError {
    /// A read failed (a truncated value, an invalid name or reference). Transparent.
    Read(ReadError),
    /// A package object could not be named. Transparent.
    Package(PackageError),
    /// A store operation failed. Transparent.
    Store(StoreError),
    /// Reflection failed (a class that is not one, a layout that could not be defined). Transparent.
    Reflect(ReflectError),
    /// The heap refused an allocation or a write. Transparent.
    Heap(HeapError),
    /// A layout lookup failed. Transparent.
    Layout(LayoutError),
    /// "unsupported exported object request": an export index that is not positive.
    UnsupportedRequest,
    /// "cannot instantiate a null class": the export's class reference resolves to nothing.
    NullClass,
    /// "object property prefix outside export"
    PrefixOutsideExport,
    /// "negative property size or index"
    NegativeSizeOrIndex,
    /// "invalid array length"
    InvalidArrayLength,
    /// "array element type unknown": an array with elements and no declaration to type them (or, under
    /// [`ParityMode::Reference`], an array with elements in a struct's default stream).
    ElementTypeUnknown,
    /// "unsupported array element type `<kind>`" (the inner kind's name)
    UnsupportedElement(&'static str),
    /// "struct nesting too deep"
    StructTooDeep,
    /// A value did not end where its tag said (the C++ skips it without a text).
    SizeMismatch,
    /// A tag type the VM does not decode (delegates, maps, byte attributes...).
    UnsupportedTag(String),
    /// A tag names a property its class or struct does not declare.
    Undeclared(String),
    /// A value whose kind does not fit its declaration (Rust only: the C++ bag takes any value).
    DoesNotFit {
        property: String,
        error: HeapError,
    },
    /// An enum value name the declared enum does not have ([`ParityMode::Off`] only; the C++ gives 0).
    UnknownEnumValue {
        property: String,
        value: String,
    },
    /// A struct's defaults were needed while they were being read (an array of a struct in its own default
    /// stream, through any chain). Rust only: the C++ never reads those arrays.
    StructDefaultsCycle(String),
    /// Struct defaults waited on each other deeper than [`MAX_DEFAULTS_NESTING`].
    DefaultsTooDeep,
    /// A struct or enum of the class table whose export is not in its package.
    NoExport(String),
    /// A referenced object whose class has no layout: an invalid class reference, or an engine-native stand-in class
    /// (under [`ParityMode::Reference`]) before `Core.Object` has a layout.
    NoClass {
        object: String,
    },
    /// Reference resolution nested deeper than [`MAX_RESOLVE_DEPTH`].
    ResolveTooDeep(usize),
    /// A top-level load made more resolver calls than its budget ([`MAX_RESOLVES_PER_LOAD`]). It aborts the whole load
    /// ([`DecodeError::aborts`]): no stream skips it, and the load's objects are freed.
    ResolveBudget(usize),
    /// An export reached again while it is being instantiated (a loading resolver following a reference back to it).
    /// The reference that reached it is skipped; the instantiation in progress goes on.
    InstantiationCycle(String),
    /// An enum value whose position does not fit a byte ([`ParityMode::Off`]; the C++ keeps its low 8 bits).
    EnumPastByte {
        property: String,
        position: usize,
    },
    /// A serialized value that does not match its declaration's kind: a struct tag on a property that is not a
    /// struct, or a package-walker value of another kind ([`to_value`]).
    KindMismatch {
        property: String,
        kind: &'static str,
    },
    /// A package-walker entry whose array index is not below its static array's dimension ([`to_value`]).
    StaticIndex {
        property: String,
        index: i32,
    },
    NotYet(NotYet),
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecodeError::Read(e) => e.fmt(f),
            DecodeError::Package(e) => e.fmt(f),
            DecodeError::Store(e) => e.fmt(f),
            DecodeError::Reflect(e) => e.fmt(f),
            DecodeError::Heap(e) => e.fmt(f),
            DecodeError::Layout(e) => e.fmt(f),
            DecodeError::UnsupportedRequest => f.write_str("unsupported exported object request"),
            DecodeError::NullClass => f.write_str("cannot instantiate a null class"),
            DecodeError::PrefixOutsideExport => {
                f.write_str("object property prefix outside export")
            }
            DecodeError::NegativeSizeOrIndex => f.write_str("negative property size or index"),
            DecodeError::InvalidArrayLength => f.write_str("invalid array length"),
            DecodeError::ElementTypeUnknown => f.write_str("array element type unknown"),
            DecodeError::UnsupportedElement(kind) => {
                write!(f, "unsupported array element type {kind}")
            }
            DecodeError::StructTooDeep => f.write_str("struct nesting too deep"),
            DecodeError::SizeMismatch => {
                f.write_str("property value does not end at its tag's size")
            }
            DecodeError::UnsupportedTag(ty) => write!(f, "unsupported property type {ty}"),
            DecodeError::Undeclared(name) => write!(f, "{name} is not declared"),
            DecodeError::DoesNotFit { property, error } => {
                write!(
                    f,
                    "the value of {property} does not fit its declaration: {error}"
                )
            }
            DecodeError::UnknownEnumValue { property, value } => {
                write!(f, "{property}: the enum has no value {value}")
            }
            DecodeError::StructDefaultsCycle(path) => {
                write!(f, "the defaults of {path} need themselves")
            }
            DecodeError::DefaultsTooDeep => write!(
                f,
                "struct defaults nest deeper than {MAX_DEFAULTS_NESTING} levels"
            ),
            DecodeError::NoExport(path) => write!(f, "no export for {path}"),
            DecodeError::NoClass { object } => write!(f, "the class of {object} has no layout"),
            DecodeError::ResolveTooDeep(limit) => {
                write!(f, "reference resolution nests deeper than {limit} levels")
            }
            DecodeError::ResolveBudget(budget) => {
                write!(f, "a load resolves more than {budget} references")
            }
            DecodeError::InstantiationCycle(path) => {
                write!(f, "{path} is reached again while it is being instantiated")
            }
            DecodeError::EnumPastByte { property, position } => {
                write!(
                    f,
                    "{property}: enum position {position} does not fit a byte"
                )
            }
            DecodeError::KindMismatch { property, kind } => {
                write!(
                    f,
                    "the serialized value of {property} does not match its {kind} declaration"
                )
            }
            DecodeError::StaticIndex { property, index } => {
                write!(f, "{property}[{index}] is outside its static array")
            }
            DecodeError::NotYet(e) => e.fmt(f),
        }
    }
}

impl DecodeError {
    /// Whether this error aborts the whole top-level load instead of skipping one value
    /// ([`DecodeError::ResolveBudget`]).
    pub fn aborts(&self) -> bool {
        matches!(self, DecodeError::ResolveBudget(_))
    }
}

impl std::error::Error for DecodeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            // Transparent wrappers: the wrapped error's text is this one's, so its cause is this one's cause.
            DecodeError::Read(e) => e.source(),
            DecodeError::Package(e) => e.source(),
            DecodeError::Store(e) => e.source(),
            DecodeError::Reflect(e) => e.source(),
            DecodeError::Heap(e) => e.source(),
            DecodeError::Layout(e) => e.source(),
            _ => None,
        }
    }
}

impl From<ReadError> for DecodeError {
    fn from(e: ReadError) -> Self {
        DecodeError::Read(e)
    }
}

impl From<PackageError> for DecodeError {
    fn from(e: PackageError) -> Self {
        DecodeError::Package(e)
    }
}

impl From<StoreError> for DecodeError {
    fn from(e: StoreError) -> Self {
        DecodeError::Store(e)
    }
}

impl From<ReflectError> for DecodeError {
    fn from(e: ReflectError) -> Self {
        DecodeError::Reflect(e)
    }
}

impl From<HeapError> for DecodeError {
    fn from(e: HeapError) -> Self {
        DecodeError::Heap(e)
    }
}

impl From<LayoutError> for DecodeError {
    fn from(e: LayoutError) -> Self {
        DecodeError::Layout(e)
    }
}

/// Where a struct's default values stand: being read, or read with this outcome.
#[derive(Debug, Clone, PartialEq)]
enum Defaults {
    Reading,
    Done(Result<(), DecodeError>),
}

/// What decoding keeps between calls (the C++ `Runtime`'s caches and log): one stand-in object per referenced
/// object, the structs whose defaults are read, the class default objects, a per-package export-name index and the
/// log.
#[derive(Debug)]
pub struct DecodeState {
    resources: FixedHashMap<(PackageId, i32), ObjRef>,
    struct_defaults: FixedHashMap<StructId, Defaults>,
    /// While a discarded load runs ([`ValueCx::load_discarded`]): each struct whose defaults it read, with its default
    /// entries and cache entry from before, to be put back when it ends (review r3 of #464).
    discard_log: Option<Vec<(StructId, Vec<(u32, Value)>, Option<Defaults>)>>,
    defaults_nesting: usize,
    by_name: FixedHashMap<PackageId, Rc<FixedHashMap<String, Vec<i32>>>>,
    pub(crate) class_defaults: FixedHashMap<ClassKey, ObjRef>,
    /// The C++ `Runtime::log` lines this decoding produced, in order.
    pub log: Vec<String>,
    /// Tags dropped because the class does not declare their property, though the C++ VM would have decoded them
    /// into its bag (`<Class>.<Property> (<Type>)`, module documentation). Not printed.
    pub dropped: Vec<String>,
    /// The declared top-level properties [`ValueCx::apply_tags`] skipped (the "defaults: skipped" lines), by the
    /// object written and the property, so a reader can tell a skipped value from a decoded one (lane
    /// IMPL-AIMOFFSET-PROFILES). Undeclared tags (`dropped`) and fields skipped inside a struct are not listed. The
    /// definition store drains it per load (`ow_defs::DefStore::is_skipped`).
    pub skipped: Vec<(ObjRef, PropId)>,
    /// The engine-native classes defined for stand-ins, by path, in order ([`ValueCx::resource`]).
    pub native_classes: Vec<String>,
    /// How many [`ReferenceResolver`] calls are under way ([`MAX_RESOLVE_DEPTH`]).
    resolve_depth: usize,
    /// How many resolver calls one top-level load may make ([`MAX_RESOLVES_PER_LOAD`] unless changed).
    pub resolve_budget: usize,
    /// The top-level load under way: how deeply loads nest, how many resolver calls it made, what it allocated.
    pub(crate) load_depth: usize,
    load_resolves: usize,
    pub(crate) load_allocated: Vec<ObjRef>,
    /// The exports being instantiated right now, innermost last.
    pub(crate) instantiating: Vec<(PackageId, i32)>,
    /// How many times a struct-defaults refusal (a cycle, or the nesting bound) was returned, and how many of those were
    /// the nesting bound (read and compared by [`ValueCx::ensure_struct_defaults`], so a stream that swallowed one is not
    /// published).
    refusals: u64,
    depth_refusals: u64,
    /// Where each struct and enum of the class table was read from: the export its declaring property's type reference
    /// names (the C++ `structAt(decl.package, decl.typeRef)`), recorded when a class's layout is first used.
    sources: FixedHashMap<Source, (PackageId, i32)>,
    noted_classes: FixedHashSet<ClassId>,
    noted_structs: FixedHashSet<StructId>,
}

/// A struct or an enum whose export [`DecodeState`] records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Source {
    Struct(StructId),
    Enum(EnumId),
}

impl Default for DecodeState {
    fn default() -> Self {
        DecodeState {
            resources: FixedHashMap::default(),
            struct_defaults: FixedHashMap::default(),
            discard_log: None,
            defaults_nesting: 0,
            by_name: FixedHashMap::default(),
            class_defaults: FixedHashMap::default(),
            log: Vec::new(),
            dropped: Vec::new(),
            skipped: Vec::new(),
            native_classes: Vec::new(),
            resolve_depth: 0,
            resolve_budget: MAX_RESOLVES_PER_LOAD,
            load_depth: 0,
            load_resolves: 0,
            load_allocated: Vec::new(),
            instantiating: Vec::new(),
            refusals: 0,
            depth_refusals: 0,
            sources: FixedHashMap::default(),
            noted_classes: FixedHashSet::default(),
            noted_structs: FixedHashSet::default(),
        }
    }
}

impl DecodeState {
    pub fn new() -> Self {
        Self::default()
    }
}

/// How decoded object references become heap objects: [`ValueCx`] asks its resolver for every object, component and
/// interface reference it decodes. The default, [`StandIns`], gives one [`Origin::Resource`] stand-in per referenced
/// object ([`ValueCx::resource`]); a definition store can answer with objects it loads instead (W3-DEFS).
///
/// The resolver is called re-entrantly: decoding inside `resolve` (loading another definition) asks it again for the
/// references found there. An implementation that keeps state behind a `RefCell` must not hold a borrow across a call
/// into `cx`.
pub trait ReferenceResolver {
    /// The object `reference` (package-relative, not 0) of `package` names, or `None` for no object.
    fn resolve(
        &self,
        cx: &mut ValueCx<'_>,
        package: PackageId,
        reference: i32,
    ) -> Result<Option<ObjRef>, DecodeError>;
}

/// The default [`ReferenceResolver`]: the C++ `Runtime::resource` stand-ins.
#[derive(Debug, Clone, Copy, Default)]
pub struct StandIns;

impl ReferenceResolver for StandIns {
    fn resolve(
        &self,
        cx: &mut ValueCx<'_>,
        package: PackageId,
        reference: i32,
    ) -> Result<Option<ObjRef>, DecodeError> {
        cx.resource(package, reference)
    }
}

/// Everything decoding reads and writes: the store, the reflection index, the object space, the decode state, the
/// parity mode and the reference resolver ([`StandIns`] unless [`ValueCx::with_resolver`] sets another).
pub struct ValueCx<'a> {
    pub store: &'a mut PackageStore,
    pub code: &'a mut CodeIndex,
    pub space: &'a mut ObjectSpace,
    pub state: &'a mut DecodeState,
    pub mode: ParityMode,
    pub resolver: &'a dyn ReferenceResolver,
}

/// A property tag's header.
struct Header {
    name: String,
    ty: String,
    size: usize,
    index: i32,
    /// The struct or enum name of a struct or byte tag; empty for others.
    detail: String,
    /// A bool tag's byte.
    boolean: u8,
}

/// Which stream a value is read from.
#[derive(Debug, Clone, Copy)]
struct Stream {
    /// An object's own tags (attribute tags are read there only, as in the C++ `applyTaggedDefaults`).
    top: bool,
    /// A struct's default stream, at any nesting (arrays with elements fail there under `ParityMode::Reference`).
    defaults: bool,
    /// The C++ nesting depth of the stream (`tagged`'s `depth`; 0 for an object's own tags).
    depth: u32,
}

fn byte(r: &mut Reader<'_>) -> Result<u8, ReadError> {
    r.require(1)?;
    let b = r.byte(r.pos())?;
    r.set_pos(r.pos() + 1);
    Ok(b)
}

fn float(r: &mut Reader<'_>) -> Result<f32, ReadError> {
    Ok(f32::from_bits(r.u32()?))
}

/// The enum a byte or byte-attribute kind names.
fn enum_of(kind: &PropKind) -> Option<EnumId> {
    match kind {
        PropKind::Byte { enum_ } | PropKind::ByteAttribute { enum_ } => *enum_,
        _ => None,
    }
}

/// The last component of an object path (a struct's or class's own name).
fn last_name(path: &str) -> &str {
    path.rsplit('.').next().unwrap_or(path)
}

impl<'a> ValueCx<'a> {
    pub fn new(
        store: &'a mut PackageStore,
        code: &'a mut CodeIndex,
        space: &'a mut ObjectSpace,
        state: &'a mut DecodeState,
        mode: ParityMode,
    ) -> Self {
        ValueCx {
            store,
            code,
            space,
            state,
            mode,
            resolver: &StandIns,
        }
    }

    /// Runs `load` as one top-level load (or as part of the load under way): the [resolve budget](DecodeState::resolve_budget)
    /// counts from the start of the outermost one, and when that one fails with an [aborting](DecodeError::aborts)
    /// error, every object allocated during it is freed and dropped from the caches (stand-ins, class default
    /// objects; references to them in struct defaults are cleared by the free). Objects a resolver allocates by itself,
    /// not through this context, are not tracked; a resolver's own cache of the load's objects holds stale handles.
    pub fn load<T>(
        &mut self,
        load: impl FnOnce(&mut ValueCx<'a>) -> Result<T, DecodeError>,
    ) -> Result<T, DecodeError> {
        let outermost = self.state.load_depth == 0;
        if outermost {
            self.state.load_resolves = 0;
            self.state.load_allocated.clear();
        }
        self.state.load_depth += 1;
        let outcome = load(self);
        self.state.load_depth -= 1;
        if outermost {
            if outcome.as_ref().is_err_and(DecodeError::aborts) {
                self.roll_back();
            }
            self.state.load_allocated.clear();
        }
        outcome
    }

    /// [`ValueCx::load`] whose objects are always freed and dropped from the caches when it ends, failed or not: a
    /// read that publishes nothing (lane IMPL-SETTINGS-FILL, review r1 of #464). The struct defaults it read are put
    /// back as they were, entries and cache both (review r3 of #464, item 1), so a default that referred to one of its
    /// objects is not left cached without it. Objects a resolver allocates by itself are not tracked here: the
    /// definition store's `with_class_default` frees those. Inside a load already under way it is that load's part, and
    /// its objects are kept with the outer load's, as [`ValueCx::load`] keeps them.
    pub fn load_discarded<T>(
        &mut self,
        load: impl FnOnce(&mut ValueCx<'a>) -> Result<T, DecodeError>,
    ) -> Result<T, DecodeError> {
        if self.state.load_depth != 0 {
            return self.load(load);
        }
        self.state.load_resolves = 0;
        self.state.load_allocated.clear();
        self.state.discard_log = Some(Vec::new());
        self.state.load_depth += 1;
        let outcome = load(self);
        self.state.load_depth -= 1;
        self.roll_back();
        self.state.load_allocated.clear();
        let log = self.state.discard_log.take().unwrap_or_default();
        for (s, entries, cached) in log.into_iter().rev() {
            self.space.classes.restore_struct_default_entries(s, entries)?;
            match cached {
                Some(c) => self.state.struct_defaults.insert(s, c),
                None => self.state.struct_defaults.remove(&s),
            };
        }
        outcome
    }

    /// Records an object a load allocated, for [`ValueCx::load`]'s roll-back.
    pub(crate) fn allocated(&mut self, o: ObjRef) {
        if self.state.load_depth > 0 {
            self.state.load_allocated.push(o);
        }
    }

    fn roll_back(&mut self) {
        let mut dead: Vec<ObjRef> = Vec::new();
        for &o in &self.state.load_allocated {
            if self.space.heap.contains(o) && !dead.contains(&o) {
                dead.push(o);
            }
        }
        let gone: FixedHashSet<ObjRef> = self.state.load_allocated.iter().copied().collect();
        self.state.resources.retain(|_, o| !gone.contains(o));
        self.state.class_defaults.retain(|_, o| !gone.contains(o));
        let _ = self.space.free_many(&dead);
    }

    /// This context with `resolver` deciding what decoded references point at.
    pub fn with_resolver(mut self, resolver: &'a dyn ReferenceResolver) -> Self {
        self.resolver = resolver;
        self
    }

    /// The layout of loaded class `key`, defined on first use ([`CodeIndex::define_layout`]). The first time, the
    /// exports its structs and enums come from are recorded ([`DecodeState`]'s sources).
    pub fn layout(&mut self, key: ClassKey) -> Result<ClassId, DecodeError> {
        let id = self.code.define_layout(
            self.store,
            &mut self.space.names,
            &mut self.space.classes,
            key,
        )?;
        if self.state.noted_classes.insert(id) {
            let decls: Vec<PropertyDecl> = self
                .space
                .classes
                .props(id)?
                .into_iter()
                .map(|(_, d)| d.clone())
                .collect();
            for decl in &decls {
                self.note_kind(&decl.kind, decl.source, 0);
            }
        }
        Ok(id)
    }

    /// Records the export a kind's struct or enum comes from, through `property`, the property export whose type
    /// reference names it (an array's inner kind through its inner property), and the structs inside a struct.
    fn note_kind(&mut self, kind: &PropKind, property: Option<(PackageId, i32)>, depth: usize) {
        if depth > crate::reflect::MAX_NESTING {
            return;
        }
        match kind {
            PropKind::Struct(s) => {
                if let Some(src) = property.and_then(|(p, i)| self.property_type_ref(p, i)) {
                    self.state.sources.entry(Source::Struct(*s)).or_insert(src);
                }
                if self.state.noted_structs.insert(*s) {
                    let fields: Vec<PropertyDecl> = self
                        .space
                        .classes
                        .struct_layout(*s)
                        .map(|l| l.fields().map(|(_, d)| d.clone()).collect())
                        .unwrap_or_default();
                    for field in &fields {
                        self.note_kind(&field.kind, field.source, depth + 1);
                    }
                }
            }
            PropKind::Byte { enum_: Some(e) } | PropKind::ByteAttribute { enum_: Some(e) } => {
                if let Some(src) = property.and_then(|(p, i)| self.property_type_ref(p, i)) {
                    self.state.sources.entry(Source::Enum(*e)).or_insert(src);
                }
            }
            PropKind::Array(inner) => {
                let inner_property = property.and_then(|(p, i)| self.property_type_ref(p, i));
                self.note_kind(inner, inner_property, depth + 1);
            }
            _ => {}
        }
    }

    /// The object property export `index` of `package`'s first type reference names, resolved (the C++
    /// `readProperty`'s `typeRef`: from byte 40, 42 when flag `0x20` adds a replication offset), or `None`.
    fn property_type_ref(&mut self, package: PackageId, index: i32) -> Option<(PackageId, i32)> {
        let pkg = self.store.get(package);
        let object = pkg.object(index).ok()?;
        let (start, size) = (object.offset as usize, usize::try_from(object.size).ok()?);
        if size < crate::reflect::PROPERTY_HEADER {
            return None;
        }
        let word = |at: usize| -> Option<u32> {
            pkg.data
                .get(at..at + 4)
                .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        };
        let flags = u64::from(word(start + 20)?) | (u64::from(word(start + 24)?) << 32);
        let at = start
            + crate::reflect::PROPERTY_HEADER
            + if flags & crate::reflect::CPF_NET != 0 {
                2
            } else {
                0
            };
        if at + 4 > start + size {
            return None;
        }
        let reference = word(at)? as i32;
        self.code.resolve_ref(self.store, package, reference)
    }

    /// The exports of `package` named `name` (ASCII case folded), in ascending order (the C++ `PackageIndex::byName`).
    pub(crate) fn exports_named(&mut self, package: PackageId, name: &str) -> Vec<i32> {
        let store = &*self.store;
        let index = self.state.by_name.entry(package).or_insert_with(|| {
            let mut map: FixedHashMap<String, Vec<i32>> = FixedHashMap::default();
            for (i, e) in store.get(package).exports.iter().enumerate() {
                map.entry(e.name.to_ascii_lowercase())
                    .or_default()
                    .push(i as i32 + 1);
            }
            Rc::new(map)
        });
        index
            .get(&name.to_ascii_lowercase())
            .cloned()
            .unwrap_or_default()
    }

    // ------------------------------------------------------------------------------------------- object streams

    /// Applies export `export`'s tags (from `prefix` bytes into its payload) to `object`, typed by the declarations
    /// of class `class` and its super classes: the C++ `applyTaggedDefaults`. A value that cannot be decoded or stored
    /// is skipped and logged; an error is returned only where the C++ throws (a prefix outside the export, a tag
    /// header that cannot be read), after the tags before it were applied. Returns where the stream ended (just past
    /// its `None` tag), from the export's payload start: the start of any untagged tail.
    pub fn apply_tags(
        &mut self,
        object: ObjRef,
        class: ClassKey,
        package: PackageId,
        export: i32,
        prefix: usize,
    ) -> Result<usize, DecodeError> {
        let pkg = self.store.shared(package);
        let exported = pkg.object(export)?;
        let size = usize::try_from(exported.size).map_err(|_| DecodeError::PrefixOutsideExport)?;
        if prefix > size || size - prefix < 8 {
            return Err(DecodeError::PrefixOutsideExport);
        }
        let offset = exported.offset as usize;
        let class_id = self.layout(class)?;
        let class_name = self.code.class(class).name.clone();
        let mut r = pkg.reader();
        r.set_pos(offset + prefix);
        r.set_limit(offset + size);
        let stream = Stream {
            top: true,
            defaults: false,
            depth: 0,
        };
        while let Some(h) = self.header(&mut r, &pkg)? {
            let end = r.pos() + h.size;
            let saved = r.limit();
            r.set_limit(end);
            let prop = self
                .space
                .names
                .find(&h.name)
                .and_then(|n| self.space.classes.prop(class_id, n));
            let outcome = match prop {
                Some(prop) => {
                    let decl = self
                        .space
                        .classes
                        .decl(prop)
                        .cloned()
                        .ok_or(LayoutError::NotAProperty(prop))?;
                    self.value(&mut r, &pkg, package, &h, &decl, stream)
                        .and_then(|v| {
                            if r.pos() == end {
                                Ok(v)
                            } else {
                                Err(DecodeError::SizeMismatch)
                            }
                        })
                        .and_then(|v| self.store_slot(object, prop, &decl, h.index, v))
                }
                None => {
                    let skimmed = skim(&mut r, &pkg, &h).and_then(|()| {
                        if r.pos() == end {
                            Ok(())
                        } else {
                            Err(DecodeError::SizeMismatch)
                        }
                    });
                    if skimmed.is_ok() {
                        self.state
                            .dropped
                            .push(format!("{class_name}.{} ({})", h.name, h.ty));
                        Ok(())
                    } else {
                        skimmed
                    }
                }
            };
            r.set_limit(saved);
            r.set_pos(end);
            if let Err(e) = &outcome
                && e.aborts()
            {
                return Err(e.clone());
            }
            if outcome.is_err() {
                self.state.log.push(format!(
                    "defaults: skipped {class_name}.{} ({})",
                    h.name, h.ty
                ));
                if let Some(prop) = prop {
                    self.state.skipped.push((object, prop));
                }
            }
        }
        Ok(r.pos() - offset)
    }

    /// Writes a decoded top-level value: one element of a static array (an index past it is ignored, as the C++
    /// ignores it), or the whole property (whatever the tag's index).
    fn store_slot(
        &mut self,
        object: ObjRef,
        prop: PropId,
        decl: &PropertyDecl,
        index: i32,
        value: Value,
    ) -> Result<(), DecodeError> {
        let addr = if decl.array_dim > 1 && !matches!(decl.kind, PropKind::Array(_)) {
            if index >= decl.array_dim {
                return Ok(());
            }
            PropAddr::element(prop, index as u32)
        } else {
            PropAddr::scalar(prop)
        };
        self.space
            .write(object, addr, value)
            .map_err(|error| DecodeError::DoesNotFit {
                property: self.space.names.text(decl.name).into_owned(),
                error,
            })
    }

    /// One tag header, or `None` at the `None` tag. The C++ reads and checks it outside the per-value `try`, so a
    /// failure here ends the stream.
    fn header(&mut self, r: &mut Reader<'_>, pkg: &Package) -> Result<Option<Header>, DecodeError> {
        let name = tables::read_name(r, &pkg.names)?;
        if name == "None" {
            return Ok(None);
        }
        let ty = tables::read_name(r, &pkg.names)?;
        let size = r.i32()?;
        let index = r.i32()?;
        if size < 0 || index < 0 {
            return Err(DecodeError::NegativeSizeOrIndex);
        }
        let detail = if ty == "StructProperty" || ty == "ByteProperty" {
            tables::read_name(r, &pkg.names)?
        } else {
            String::new()
        };
        let boolean = if ty == "BoolProperty" { byte(r)? } else { 0 };
        let size = size as usize;
        r.require(size)?;
        Ok(Some(Header {
            name,
            ty,
            size,
            index,
            detail,
            boolean,
        }))
    }

    /// One tag's value, read by the tag's type and shaped by the declaration's kind.
    fn value(
        &mut self,
        r: &mut Reader<'_>,
        pkg: &Arc<Package>,
        package: PackageId,
        h: &Header,
        decl: &PropertyDecl,
        stream: Stream,
    ) -> Result<Value, DecodeError> {
        Ok(match h.ty.as_str() {
            "IntProperty" => Value::Int(r.i32()?),
            "FloatProperty" => Value::Float(float(r)?),
            // One plain 4-byte number, the payload of a float or int tag (FITTED, `src/vm.cpp`); the object's own
            // tags only, as in the C++.
            "FloatAttributeProperty" if stream.top => Value::Float(float(r)?),
            "IntAttributeProperty" if stream.top => Value::Int(r.i32()?),
            "BoolProperty" => Value::Bool(h.boolean != 0),
            "NameProperty" => {
                let text = tables::read_name(r, &pkg.names)?;
                Value::Name(self.space.names.intern(&text))
            }
            "StrProperty" => Value::string(&r.string()?),
            // A class property's tag says ObjectProperty in these packages (module documentation): its reference
            // names the class, which is what the declaration holds.
            "ObjectProperty" | "ComponentProperty" | "InterfaceProperty"
                if matches!(decl.kind, PropKind::Class { .. }) =>
            {
                let reference = r.i32()?;
                self.class_value(package, reference)?
            }
            "ObjectProperty" | "ComponentProperty" | "InterfaceProperty" => {
                let reference = r.i32()?;
                self.object_value(package, reference, &decl.kind)?
            }
            "ClassProperty" => {
                let reference = r.i32()?;
                self.class_value(package, reference)?
            }
            "ByteProperty" if h.detail == "None" => Value::Byte(byte(r)?),
            "ByteProperty" => {
                let value = tables::read_name(r, &pkg.names)?;
                if self.mode == ParityMode::Reference {
                    Value::Byte(self.cxx_byte_from_enum(package, &h.detail, &value, decl))
                } else {
                    Value::Byte(self.enum_value(enum_of(&decl.kind), decl.name, &value)?)
                }
            }
            "StructProperty" => {
                let PropKind::Struct(s) = decl.kind else {
                    return Err(DecodeError::KindMismatch {
                        property: h.name.clone(),
                        kind: decl.kind.kind_name(),
                    });
                };
                let depth = if stream.top { 0 } else { stream.depth + 1 };
                self.structure(r, pkg, package, &h.detail, s, depth, stream.defaults)?
            }
            "ArrayProperty" => {
                let length = r.i32()?;
                if !(0..=MAX_ARRAY_LENGTH).contains(&length) {
                    return Err(DecodeError::InvalidArrayLength);
                }
                let inner = match &decl.kind {
                    PropKind::Array(inner) => Some(inner.as_ref().clone()),
                    _ => None,
                };
                let typed = !(stream.defaults && self.mode == ParityMode::Reference);
                let depth = if stream.top { 0 } else { stream.depth };
                let mut elements = Vec::with_capacity(length.min(4096) as usize);
                for _ in 0..length {
                    let Some(inner) = inner.as_ref().filter(|_| typed) else {
                        return Err(DecodeError::ElementTypeUnknown);
                    };
                    elements.push(self.element(
                        r,
                        pkg,
                        package,
                        inner,
                        decl.name,
                        depth,
                        stream.defaults,
                    )?);
                }
                Value::array(elements)
            }
            other => return Err(DecodeError::UnsupportedTag(other.to_string())),
        })
    }

    /// An array element of kind `kind` of array property `property` (the C++ `element`, by the inner declaration's
    /// type). Under [`ParityMode::Reference`] with the C++ widths: an enum byte as one byte, a bool as a 4-byte word.
    /// Otherwise with the widths measured on the installed packages (FITTED, UNVERIFIED in the game): an enum byte as
    /// an 8-byte name placed in the declared enum (19 of 19 enum-byte array tags of the code packages' class defaults
    /// meet their size so), a bool as one byte (the one bool array with elements in their struct defaults: 3 elements
    /// in 7 bytes).
    #[allow(clippy::too_many_arguments)]
    fn element(
        &mut self,
        r: &mut Reader<'_>,
        pkg: &Arc<Package>,
        package: PackageId,
        kind: &PropKind,
        property: NameId,
        depth: u32,
        defaults: bool,
    ) -> Result<Value, DecodeError> {
        let cxx = self.mode == ParityMode::Reference;
        Ok(match kind {
            PropKind::Int => Value::Int(r.i32()?),
            PropKind::Float => Value::Float(float(r)?),
            PropKind::Name => {
                let text = tables::read_name(r, &pkg.names)?;
                Value::Name(self.space.names.intern(&text))
            }
            PropKind::Str => Value::string(&r.string()?),
            PropKind::Object { .. } | PropKind::Interface { .. } => {
                let reference = r.i32()?;
                self.object_value(package, reference, kind)?
            }
            PropKind::Class { .. } => {
                let reference = r.i32()?;
                self.class_value(package, reference)?
            }
            PropKind::Byte { enum_: Some(e) } if !cxx => {
                let text = tables::read_name(r, &pkg.names)?;
                Value::Byte(self.enum_value(Some(*e), property, &text)?)
            }
            PropKind::Byte { .. } => Value::Byte(byte(r)?),
            PropKind::Bool if cxx => Value::Bool(r.u32()? != 0),
            PropKind::Bool => Value::Bool(byte(r)? != 0),
            PropKind::Struct(s) => {
                let name = self.struct_name(*s);
                self.structure(r, pkg, package, &name, *s, depth + 1, defaults)?
            }
            other => return Err(DecodeError::UnsupportedElement(other.kind_name())),
        })
    }

    fn struct_name(&self, s: StructId) -> String {
        self.space
            .classes
            .struct_layout(s)
            .map(|l| last_name(l.path().as_str()).to_string())
            .unwrap_or_default()
    }

    /// A struct value of declared struct `s` whose tag (or element) names struct `name`: a native layout read
    /// untagged, or the struct's defaults with a nested tag stream over them (the C++ `structure`).
    #[allow(clippy::too_many_arguments)]
    fn structure(
        &mut self,
        r: &mut Reader<'_>,
        pkg: &Arc<Package>,
        package: PackageId,
        name: &str,
        s: StructId,
        depth: u32,
        defaults: bool,
    ) -> Result<Value, DecodeError> {
        if depth > MAX_STRUCT_DEPTH {
            return Err(DecodeError::StructTooDeep);
        }
        if let Some((fields, kind)) = native_layout(name) {
            let mut value = self.space.classes.zero_value(&PropKind::Struct(s));
            for field in fields {
                let v = match kind {
                    NativeField::Float => Value::Float(float(r)?),
                    NativeField::Int => Value::Int(r.i32()?),
                    NativeField::Byte => Value::Byte(byte(r)?),
                };
                let entry = self
                    .field_entry(s, field)
                    .ok_or_else(|| DecodeError::Undeclared(format!("{name}.{field}")))?;
                let sv = value.struct_mut().ok_or(LayoutError::StructNotDefined(s))?;
                sv.fields[entry as usize] = v;
            }
            return Ok(value);
        }
        self.ensure_struct_defaults(s)?;
        let mut value = self.space.classes.struct_defaults(s)?;
        self.tagged(
            r,
            pkg,
            package,
            &mut value,
            s,
            Stream {
                top: false,
                defaults,
                depth: depth + 1,
            },
        )?;
        Ok(value)
    }

    /// The entry of the scalar field `field` of struct `s`.
    fn field_entry(&self, s: StructId, field: &str) -> Option<u32> {
        let n = self.space.names.find(field)?;
        let (entry, decl) = self.space.classes.struct_layout(s)?.field(n)?;
        (decl.array_dim == 1).then_some(entry)
    }

    /// A nested tag stream into struct value `value` of struct `s` (the C++ `tagged`): a field that fails, or that
    /// the struct does not declare, is skipped silently; a header that fails fails the stream.
    fn tagged(
        &mut self,
        r: &mut Reader<'_>,
        pkg: &Arc<Package>,
        package: PackageId,
        value: &mut Value,
        s: StructId,
        stream: Stream,
    ) -> Result<(), DecodeError> {
        if stream.depth > MAX_STRUCT_DEPTH {
            return Err(DecodeError::StructTooDeep);
        }
        while let Some(h) = self.header(r, pkg)? {
            let end = r.pos() + h.size;
            let saved = r.limit();
            r.set_limit(end);
            let field = self
                .space
                .names
                .find(&h.name)
                .and_then(|n| self.space.classes.struct_layout(s)?.field(n))
                .map(|(entry, decl)| (entry, decl.clone()));
            let decoded = match &field {
                Some((_, decl)) => self.value(r, pkg, package, &h, decl, stream).and_then(|v| {
                    if r.pos() == end {
                        Ok(v)
                    } else {
                        Err(DecodeError::SizeMismatch)
                    }
                }),
                None => Err(DecodeError::Undeclared(h.name.clone())),
            };
            r.set_limit(saved);
            r.set_pos(end);
            if let Err(e) = &decoded
                && e.aborts()
            {
                return Err(e.clone());
            }
            let (Ok(v), Some((entry, decl))) = (decoded, field) else {
                continue;
            };
            if self
                .space
                .heap
                .check_value(&self.space.classes, &decl.kind, &v)
                .is_err()
            {
                continue;
            }
            let entry = if decl.array_dim > 1 && !matches!(decl.kind, PropKind::Array(_)) {
                if h.index >= decl.array_dim {
                    continue;
                }
                entry + h.index as u32
            } else {
                entry
            };
            let sv = value.struct_mut().ok_or(LayoutError::StructNotDefined(s))?;
            if let Some(slot) = sv.fields.get_mut(entry as usize) {
                *slot = v;
            }
        }
        Ok(())
    }

    // ------------------------------------------------------------------------------------------- struct defaults

    /// Reads struct `s`'s default tags once and stores them in the class table, after the defaults of the structs it
    /// holds by value (the C++ `applyStructDefaults`, which the C++ reruns on every zero value). A stream that fails,
    /// or does not end exactly at the export's end, leaves the zero value (no error); a native struct takes none.
    ///
    /// Defaults that need themselves (through arrays of structs in default streams, which only [`ParityMode::Off`]
    /// reads), or that wait on others deeper than [`MAX_DEFAULTS_NESTING`], are refused: every struct whose stream met
    /// such a refusal, swallowed or not, gets [`DecodeError::StructDefaultsCycle`] (or [`DecodeError::DefaultsTooDeep`]
    /// when the bound was met) and publishes nothing, so for a ring the outcome does not depend on which struct was
    /// asked first (a straight chain longer than the bound does; module documentation).
    pub fn ensure_struct_defaults(&mut self, s: StructId) -> Result<(), DecodeError> {
        let outcome = match self.state.struct_defaults.get(&s) {
            Some(Defaults::Done(outcome)) => outcome.clone(),
            Some(Defaults::Reading) => Err(DecodeError::StructDefaultsCycle(self.struct_path(s))),
            None => return self.read_struct_defaults_once(s),
        };
        self.count_refusal(&outcome);
        outcome
    }

    /// Counts a refused struct default (a cycle, or the nesting bound) for the no-publish barrier.
    fn count_refusal(&mut self, outcome: &Result<(), DecodeError>) {
        match outcome {
            Err(DecodeError::StructDefaultsCycle(_)) => self.state.refusals += 1,
            Err(DecodeError::DefaultsTooDeep) => {
                self.state.refusals += 1;
                self.state.depth_refusals += 1;
            }
            _ => {}
        }
    }

    fn read_struct_defaults_once(&mut self, s: StructId) -> Result<(), DecodeError> {
        if self.state.defaults_nesting >= MAX_DEFAULTS_NESTING {
            let outcome = Err(DecodeError::DefaultsTooDeep);
            self.count_refusal(&outcome);
            return outcome;
        }
        if self.state.discard_log.is_some() {
            let entries = self.space.classes.struct_default_entries(s)?;
            let cached = self.state.struct_defaults.get(&s).cloned();
            if let Some(log) = self.state.discard_log.as_mut() {
                log.push((s, entries, cached));
            }
        }
        self.state.struct_defaults.insert(s, Defaults::Reading);
        self.state.defaults_nesting += 1;
        let outcome = self.read_struct_defaults(s);
        self.state.defaults_nesting -= 1;
        if outcome.as_ref().is_err_and(DecodeError::aborts) {
            // An aborted load caches nothing: the next load reads these defaults again.
            self.state.struct_defaults.remove(&s);
            return outcome;
        }
        self.state
            .struct_defaults
            .insert(s, Defaults::Done(outcome.clone()));
        self.count_refusal(&outcome);
        outcome
    }

    fn struct_path(&self, s: StructId) -> String {
        self.space
            .classes
            .struct_layout(s)
            .map(|l| l.path().as_str().to_string())
            .unwrap_or_default()
    }

    fn read_struct_defaults(&mut self, s: StructId) -> Result<(), DecodeError> {
        let layout = self
            .space
            .classes
            .struct_layout(s)
            .ok_or(LayoutError::StructNotDefined(s))?;
        let inner: Vec<StructId> = layout
            .fields()
            .filter_map(|(_, d)| match d.kind {
                PropKind::Struct(t) => Some(t),
                _ => None,
            })
            .collect();
        let path = layout.path().clone();
        for t in inner {
            self.ensure_struct_defaults(t)?;
        }
        if native_layout(path.name()).is_some() {
            return Ok(());
        }
        let (package, index) = self.struct_export(s, path.as_str())?;
        let pkg = self.store.shared(package);
        let exported = pkg.object(index)?;
        if exported.size < STRUCT_DEFAULTS_START as i32 {
            return Ok(());
        }
        let start = exported.offset as usize;
        let end = start + exported.size as usize;
        let mut r = pkg.reader();
        r.set_pos(start + STRUCT_DEFAULTS_ZERO_WORD);
        if r.u32()? != 0 {
            return Ok(());
        }
        r.set_pos(start + STRUCT_DEFAULTS_START);
        r.set_limit(end);
        let (refusals_before, depth_before) = (self.state.refusals, self.state.depth_refusals);
        let mut candidate = self.space.classes.struct_defaults(s)?;
        let stream = Stream {
            top: false,
            defaults: true,
            depth: 1,
        };
        let read = self.tagged(&mut r, &pkg, package, &mut candidate, s, stream);
        if self.state.depth_refusals != depth_before {
            return Err(DecodeError::DefaultsTooDeep);
        }
        if self.state.refusals != refusals_before {
            return Err(DecodeError::StructDefaultsCycle(path.as_str().to_string()));
        }
        if let Err(e) = &read
            && e.aborts()
        {
            return Err(e.clone());
        }
        if read.is_err() || r.pos() != end {
            return Ok(());
        }
        Ok(self.space.set_struct_defaults(s, candidate)?)
    }

    /// The ScriptStruct export a class-table struct was defined from: the source recorded from its declaring property
    /// (the C++ `structAt(decl.package, decl.typeRef)`), else found by its path `<package stem>.<object path>` (a
    /// struct no recorded class declares).
    fn struct_export(&mut self, s: StructId, path: &str) -> Result<(PackageId, i32), DecodeError> {
        if let Some(&src) = self.state.sources.get(&Source::Struct(s)) {
            return Ok(src);
        }
        let missing = || DecodeError::NoExport(path.to_string());
        let (stem, _) = path.split_once('.').ok_or_else(missing)?;
        let package = self.code.package(self.store, stem)?;
        // The whole path: `find_export` drops one leading `<package>.`, so a struct of a class named like its package
        // (`Engine.Engine.StatColorMapping`) is still found.
        let index = self.code.find_export(self.store, package, path);
        if index == 0 || self.code.struct_of(package, index) != Some(s) {
            return Err(missing());
        }
        Ok((package, index))
    }

    // ------------------------------------------------------------------------------------------- references

    /// An object reference value of declared kind `kind` (an interface reference for an interface, an object
    /// reference otherwise), the object chosen by the [resolver](ReferenceResolver).
    fn object_value(
        &mut self,
        package: PackageId,
        reference: i32,
        kind: &PropKind,
    ) -> Result<Value, DecodeError> {
        let object = if reference == 0 {
            None
        } else {
            if self.state.resolve_depth >= MAX_RESOLVE_DEPTH {
                return Err(DecodeError::ResolveTooDeep(MAX_RESOLVE_DEPTH));
            }
            if self.state.load_depth > 0 {
                if self.state.load_resolves >= self.state.resolve_budget {
                    return Err(DecodeError::ResolveBudget(self.state.resolve_budget));
                }
                self.state.load_resolves += 1;
            }
            let resolver = self.resolver;
            self.state.resolve_depth += 1;
            let resolved = resolver.resolve(self, package, reference);
            self.state.resolve_depth -= 1;
            resolved?
        };
        Ok(match kind {
            PropKind::Interface { .. } => Value::Interface(object),
            _ => Value::Object(object),
        })
    }

    /// A class reference value: the class `reference` names, or null where it resolves to nothing or to an export
    /// that is not a class (the C++ catches both).
    fn class_value(&mut self, package: PackageId, reference: i32) -> Result<Value, DecodeError> {
        let key = match self.code.class_at(self.store, package, reference) {
            Ok(Some(key)) => key,
            Ok(None) | Err(_) => return Ok(Value::Class(None)),
        };
        Ok(Value::Class(Some(self.layout(key)?)))
    }

    /// The stand-in cache is keyed by `(filed package id, index)`: the id [`CodeIndex::resolve_ref`] and
    /// [`CodeIndex::filed_id`] file a package under (the first id met for its file, decision 2026-11-15-10), so a
    /// reference through a second id of the same file (decision 2026-11-15-05) finds the same stand-in.
    ///
    /// The stand-in for the object `reference` of `package` names (the C++ `Runtime::resource`), the default
    /// [`ReferenceResolver`]: an import is followed to the export that holds it where one does (else the stand-in
    /// names the import), and each object gets one stand-in, of the object's class, with [`Origin::Resource`] and no
    /// property slots ([`ObjectSpace::alloc_stand_in`]; the C++ stand-in holds no properties either). `None` for
    /// reference 0. A reference outside the package's tables gets, under [`ParityMode::Reference`], the C++ stand-in
    /// named `?` (of class `Core.Object`; printing its path fails, as in the C++), and is an error otherwise. Fails for a
    /// class that cannot be found or laid out (an engine-native class is a `NotYet` outside the reference mode, see
    /// the module documentation).
    pub fn resource(
        &mut self,
        package: PackageId,
        reference: i32,
    ) -> Result<Option<ObjRef>, DecodeError> {
        if reference == 0 {
            return Ok(None);
        }
        // Keyed by the id the reflection index files the package under: one object, one stand-in, whichever id of
        // its file the reference came through. `resolve_ref` files `package` first, also when it answers nothing.
        let (owner, index) = match self.code.resolve_ref(self.store, package, reference) {
            Some(target) => target,
            None => (self.code.filed_id(package), reference),
        };
        if let Some(&o) = self.state.resources.get(&(owner, index)) {
            return Ok(Some(o));
        }
        let pkg = self.store.shared(owner);
        let (name, class) = match pkg.object(index) {
            Ok(entry) => (entry.name.clone(), self.object_class(owner, &pkg, index)?),
            Err(_) if self.mode == ParityMode::Reference => {
                let object = self
                    .space
                    .classes
                    .by_path("Core.Object")
                    .filter(|&o| {
                        self.space
                            .classes
                            .get_layout(o)
                            .is_some_and(|l| l.is_defined())
                    })
                    .ok_or_else(|| DecodeError::NoClass {
                        object: "?".to_string(),
                    })?;
                ("?".to_string(), object)
            }
            Err(e) => return Err(e.into()),
        };
        let o = self.space.alloc_stand_in(
            class,
            &name,
            Origin::Resource {
                package: owner,
                index,
            },
        )?;
        self.state.resources.insert((owner, index), o);
        self.allocated(o);
        Ok(Some(o))
    }

    /// The class of object `index` of `package`: an export's class reference, or an unresolved import's class path.
    /// A class that is genuinely absent (its import resolves into no installed package, or the class path names a
    /// package or an export that does not exist) is an engine-native class ([`ValueCx::native_class`]); every other
    /// failure is returned as it is.
    fn object_class(
        &mut self,
        package: PackageId,
        pkg: &Package,
        index: i32,
    ) -> Result<ClassId, DecodeError> {
        let entry = pkg.object(index)?;
        let key = if index > 0 {
            match entry.class {
                // A class object: its class is Core.Class, which the installed Core does not export.
                0 => self.class_by_path("Core.Class")?,
                class => match self.code.class_at(self.store, package, class)? {
                    Some(key) => Ok(key),
                    None if class < 0 => Err(pkg.path_of(class)?),
                    None => {
                        return Err(DecodeError::NoClass {
                            object: pkg.path_of(index)?,
                        });
                    }
                },
            }
        } else {
            self.class_by_path(&format!("{}.{}", entry.class_package, entry.class_name))?
        };
        match key {
            Ok(key) => self.layout(key),
            Err(path) => self.native_class(&path),
        }
    }

    /// The loaded class at `path`, or `Err(path)` back when no installed package holds it (the package is missing, or
    /// has no such export). Other failures are errors.
    fn class_by_path(&mut self, path: &str) -> Result<Result<ClassKey, String>, DecodeError> {
        match self.code.find_class(self.store, path) {
            Ok(key) => Ok(Ok(key)),
            Err(ReflectError::ClassNotFound(_))
            | Err(ReflectError::Store(StoreError::PackageNotFound(_))) => Ok(Err(path.to_string())),
            Err(e) => Err(e.into()),
        }
    }

    /// The class of a stand-in for an object whose class no installed package exports. Only under
    /// [`ParityMode::Reference`] (the C++ stand-in needs no class): the class at its import path, defined (once, if
    /// nothing defined that path before) with no properties of its own under `Core.Object`, the root every class
    /// derives from, and listed in [`DecodeState::native_classes`] and marked in the class table ([`ClassTable::mark_native`]). Its native
    /// properties are not in the packages, so none is invented; the classes between it and `Core.Object`, if any, are
    /// not known (UNVERIFIED). A class an overlay or a native pack defines must be defined before any decode, or the
    /// stand-in class takes its path first. A layout defined at that path by anything else is used in either mode.
    /// Otherwise, in [`ParityMode::Off`], a [`NotYet`]: the class's layout is not read.
    fn native_class(&mut self, path: &str) -> Result<ClassId, DecodeError> {
        let classes = &mut self.space.classes;
        // A layout something else defined (an overlay, a native pack) is used in both modes; a stand-in class only
        // under the reference mode.
        if let Some(id) = classes.by_path(path)
            && classes.get_layout(id).is_some_and(|l| l.is_defined())
            && (self.mode == ParityMode::Reference || !classes.is_native(id))
        {
            return Ok(id);
        }
        if self.mode != ParityMode::Reference {
            return Err(DecodeError::NotYet(NotYet::not_read(
                format!("engine-native class {path}: no installed package exports it"),
                RECORD,
            )));
        }
        let classes = &mut self.space.classes;
        let object = classes
            .by_path("Core.Object")
            .filter(|&o| classes.get_layout(o).is_some_and(|l| l.is_defined()))
            .ok_or_else(|| DecodeError::NoClass {
                object: path.to_string(),
            })?;
        let id = classes.define(path, Some(object), Vec::new())?;
        self.state.native_classes.push(path.to_string());
        self.space.classes.mark_native(id)?;
        Ok(id)
    }

    /// The byte of enum value `value` of enum `enum_` for property `property`: its position in the declared enum
    /// ([`ParityMode::Off`]; the tags of the reference mode go through [`ValueCx::cxx_byte_from_enum`]). Under the
    /// reference mode (a walker tree) it is the C++ `enumPosition` over the declared enum's export, 0 where there is
    /// none.
    fn enum_value(
        &mut self,
        enum_: Option<EnumId>,
        property: NameId,
        value: &str,
    ) -> Result<u8, DecodeError> {
        if self.mode == ParityMode::Reference {
            let source = match enum_ {
                Some(e) => self.enum_export(e),
                None => None,
            };
            return Ok(match source {
                Some((package, index)) => self.cxx_enum_position(package, index, value),
                None => 0,
            });
        }
        let unknown = |cx: &Self| DecodeError::UnknownEnumValue {
            property: cx.space.names.text(property).into_owned(),
            value: value.to_string(),
        };
        let Some(e) = enum_ else {
            return Err(unknown(self));
        };
        let position = self
            .space
            .names
            .find(value)
            .and_then(|n| self.space.classes.enum_layout(e)?.index_of(n))
            .ok_or_else(|| unknown(self))?;
        u8::try_from(position).map_err(|_| DecodeError::EnumPastByte {
            property: self.space.names.text(property).into_owned(),
            position,
        })
    }

    /// The Enum export enum `e` was defined from: the source recorded from a declaring property, else found by its path
    /// `<package stem>.<object path>` (an enum whose declarations did not pass through this state's [`ValueCx::layout`]).
    fn enum_export(&mut self, e: EnumId) -> Option<(PackageId, i32)> {
        if let Some(&src) = self.state.sources.get(&Source::Enum(e)) {
            return Some(src);
        }
        let path = self
            .space
            .classes
            .enum_layout(e)?
            .path()
            .as_str()
            .to_string();
        let (stem, _) = path.split_once('.')?;
        let package = self.code.package(self.store, stem).ok()?;
        let index = self.code.find_export(self.store, package, &path);
        (index != 0 && self.code.enum_of(package, index) == Some(e)).then_some((package, index))
    }

    /// The C++ `byteFromEnum` (reference mode): the first Enum export of the package being read named exactly
    /// `enum_name` (the tag's enum name); otherwise the enum a `ByteProperty` declaration's type reference names, when
    /// that export is an Enum named exactly `enum_name`; otherwise 0 (UNVERIFIED there). The position is the C++
    /// `enumPosition` converted as `uint8_t` (257 is 1).
    fn cxx_byte_from_enum(
        &mut self,
        package: PackageId,
        enum_name: &str,
        value: &str,
        decl: &PropertyDecl,
    ) -> u8 {
        let is_enum = |cx: &mut Self, p: PackageId, i: i32| {
            cx.store.get(p).object(i).is_ok_and(|e| e.name == enum_name)
                && cx.code.class_name(cx.store, p, i) == "Enum"
        };
        for index in self.exports_named(package, enum_name) {
            if is_enum(self, package, index) {
                return self.cxx_enum_position(package, index, value);
            }
        }
        if matches!(decl.kind, PropKind::Byte { .. })
            && let Some((p, i)) = decl.source
            && let Some((tp, ti)) = self.property_type_ref(p, i)
            && is_enum(self, tp, ti)
        {
            return self.cxx_enum_position(tp, ti, value);
        }
        0
    }

    /// The C++ `enumPosition` over Enum export `index` of `package`: at +16 the count, then (name index, number)
    /// pairs; the first entry whose name-table text (without its number) equals `value` gives its position, kept to
    /// its low 8 bits as the C++ `uint8_t` keeps it; any other case 0.
    fn cxx_enum_position(&self, package: PackageId, index: i32, value: &str) -> u8 {
        let pkg = self.store.get(package);
        let Ok(object) = pkg.object(index) else {
            return 0;
        };
        let start = object.offset as usize;
        let end = start + object.size as usize;
        let word = |at: usize| -> Option<u32> {
            if at + 4 > end {
                return None;
            }
            pkg.data
                .get(at..at + 4)
                .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        };
        let Some(count) = word(start + 16) else {
            return 0;
        };
        let mut at = start + 20;
        let mut position: i32 = 0;
        while position < count as i32 && at + 8 <= end {
            let Some(name) = word(at) else { break };
            let text = usize::try_from(name as i32)
                .ok()
                .and_then(|n| pkg.names.get(n));
            if text.is_some_and(|t| t == value) {
                return position as u8;
            }
            position += 1;
            at += 8;
        }
        0
    }
}

/// Reads past an undeclared top-level tag's value as the C++ VM decodes it with no declaration, to tell whether that
/// decode would succeed (module documentation): scalars by their size, a byte enum's name, a native struct's fields,
/// any other struct's nested headers (each nested value is skipped by its size, as the C++ drops undeclared fields and
/// swallows their failures), an array only when empty, and nothing for other types.
fn skim(r: &mut Reader<'_>, pkg: &Package, h: &Header) -> Result<(), DecodeError> {
    match h.ty.as_str() {
        "IntProperty"
        | "FloatProperty"
        | "FloatAttributeProperty"
        | "IntAttributeProperty"
        | "ObjectProperty"
        | "ComponentProperty"
        | "InterfaceProperty"
        | "ClassProperty" => {
            r.skip(4)?;
        }
        "BoolProperty" => {}
        "NameProperty" => {
            tables::read_name(r, &pkg.names)?;
        }
        "StrProperty" => {
            r.string()?;
        }
        "ByteProperty" if h.detail == "None" => {
            byte(r)?;
        }
        "ByteProperty" => {
            tables::read_name(r, &pkg.names)?;
        }
        "StructProperty" => match native_layout(&h.detail) {
            Some((fields, NativeField::Byte)) => r.skip(fields.len())?,
            Some((fields, _)) => r.skip(4 * fields.len())?,
            None => loop {
                let name = tables::read_name(r, &pkg.names)?;
                if name == "None" {
                    break;
                }
                let ty = tables::read_name(r, &pkg.names)?;
                let size = r.i32()?;
                let index = r.i32()?;
                if size < 0 || index < 0 {
                    return Err(DecodeError::NegativeSizeOrIndex);
                }
                if ty == "StructProperty" || ty == "ByteProperty" {
                    tables::read_name(r, &pkg.names)?;
                }
                if ty == "BoolProperty" {
                    byte(r)?;
                }
                r.skip(size as usize)?;
            },
        },
        "ArrayProperty" => {
            let length = r.i32()?;
            if !(0..=MAX_ARRAY_LENGTH).contains(&length) {
                return Err(DecodeError::InvalidArrayLength);
            }
            if length > 0 {
                return Err(DecodeError::ElementTypeUnknown);
            }
        }
        other => return Err(DecodeError::UnsupportedTag(other.to_string())),
    }
    Ok(())
}

// ------------------------------------------------------------------------------------------- package-walker trees

/// Types a [`PropValue`] of the package walker (`ow_package::props::read_tagged`, references relative to `package`)
/// by declaration `decl`: numbers, names, strings and references as the declaration's kind takes them (an enum name
/// becomes its position in the declared enum), a struct as its defaults with the walked fields placed by name over
/// them, an array element by element. The value is one tag's: a static-array declaration's elements are separate
/// entries, which the caller places. A value the walker left raw, a delegate, and a value of another kind than the
/// declaration's are errors.
///
/// The rules of the VM-side reader hold here too: structs nest at most [`MAX_STRUCT_DEPTH`] deep counted as that
/// reader counts them (the walker itself accepts 32), and inside a struct a field the struct does not declare, a field
/// that does not type, and a value the heap would refuse for the field are skipped, as the C++ `tagged` skips them.
pub fn to_value(
    tree: &PropValue,
    decl: &PropertyDecl,
    package: PackageId,
    cx: &mut ValueCx<'_>,
) -> Result<Value, DecodeError> {
    tree_value(
        tree,
        decl,
        package,
        cx,
        Stream {
            top: true,
            defaults: false,
            depth: 0,
        },
    )
}

fn tree_value(
    tree: &PropValue,
    decl: &PropertyDecl,
    package: PackageId,
    cx: &mut ValueCx<'_>,
    stream: Stream,
) -> Result<Value, DecodeError> {
    let mismatch = |cx: &ValueCx<'_>| DecodeError::KindMismatch {
        property: cx.space.names.text(decl.name).into_owned(),
        kind: decl.kind.kind_name(),
    };
    Ok(match (&decl.kind, tree) {
        (PropKind::Int | PropKind::IntAttribute, PropValue::Int(i)) => Value::Int(*i),
        (PropKind::Float | PropKind::FloatAttribute, PropValue::Float(f)) => Value::Float(*f),
        (PropKind::Bool, PropValue::Bool(b)) => Value::Bool(*b),
        (PropKind::Byte { .. } | PropKind::ByteAttribute { .. }, PropValue::Byte(b)) => {
            Value::Byte(*b)
        }
        (PropKind::Byte { .. } | PropKind::ByteAttribute { .. }, PropValue::Enum(name)) => {
            Value::Byte(cx.enum_value(enum_of(&decl.kind), decl.name, name)?)
        }
        (PropKind::Name, PropValue::Name(text)) => Value::Name(cx.space.names.intern(text)),
        (PropKind::Str, PropValue::Str(text)) => Value::string(text),
        (PropKind::Object { .. } | PropKind::Interface { .. }, PropValue::Object(r)) => {
            cx.object_value(package, *r, &decl.kind)?
        }
        (PropKind::Class { .. }, PropValue::Object(r)) => cx.class_value(package, *r)?,
        (PropKind::Struct(s), PropValue::Struct { fields, .. }) => {
            let s = *s;
            // As the VM reader: a struct value at `depth`, its fields one stream deeper.
            let depth = if stream.top { 0 } else { stream.depth + 1 };
            // A native struct is read untagged, so only a struct that enters a tagged stream needs the next level.
            let tagged =
                !matches!(tree, PropValue::Struct { ty, .. } if native_layout(ty).is_some());
            if depth > MAX_STRUCT_DEPTH || (tagged && depth + 1 > MAX_STRUCT_DEPTH) {
                return Err(DecodeError::StructTooDeep);
            }
            let inner = Stream {
                top: false,
                defaults: false,
                depth: depth + 1,
            };
            cx.ensure_struct_defaults(s)?;
            let mut value = cx.space.classes.struct_defaults(s)?;
            for entry in fields {
                let field = cx
                    .space
                    .names
                    .find(&entry.name)
                    .and_then(|n| cx.space.classes.struct_layout(s)?.field(n))
                    .map(|(at, d)| (at, d.clone()));
                let Some((at, field_decl)) = field else {
                    continue;
                };
                let Ok(v) = tree_value(&entry.value, &field_decl, package, cx, inner) else {
                    continue;
                };
                if cx
                    .space
                    .heap
                    .check_value(&cx.space.classes, &field_decl.kind, &v)
                    .is_err()
                {
                    continue;
                }
                let at =
                    if field_decl.array_dim > 1 && !matches!(field_decl.kind, PropKind::Array(_)) {
                        if entry.index < 0 || entry.index >= field_decl.array_dim {
                            continue;
                        }
                        at + entry.index as u32
                    } else {
                        at
                    };
                let sv = value.struct_mut().ok_or(LayoutError::StructNotDefined(s))?;
                sv.fields[at as usize] = v;
            }
            value
        }
        (PropKind::Array(inner), PropValue::Array(items)) => {
            if items.len() > MAX_ARRAY_LENGTH as usize {
                return Err(DecodeError::InvalidArrayLength);
            }
            let element = PropertyDecl::new(decl.name, inner.as_ref().clone());
            // As the VM reader: an element's struct sits one level below the array's stream.
            let elements = Stream {
                top: false,
                defaults: false,
                depth: if stream.top { 0 } else { stream.depth },
            };
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(tree_value(item, &element, package, cx, elements)?);
            }
            Value::array(out)
        }
        _ => return Err(mismatch(cx)),
    })
}

// ------------------------------------------------------------------------------------------- the reflection typer

/// The package walker's [`ArrayTyper`] answered from reflection: the element type of an array is its declaration's
/// inner kind, found from the walk's root (the export's class, or a struct) along the scope's path of struct and array
/// property names. Read-only: it answers from what [`prepare_typer`] (or any earlier loading) put in the reflection
/// index and the class table, and gives `None` (the array stays raw) where it knows nothing.
///
/// Kinds the walker decodes map to its element types (an object or component reference to `ObjectProperty`, a struct
/// to `StructProperty:<name>`). The element widths follow the parity mode, as the VM-side reader's do
/// (module documentation): under [`ParityMode::Reference`] an enum-byte element is one byte (`ByteProperty`); otherwise
/// it is an 8-byte name placed in an enum, which no walker element type reads, so it gives `None`. Bool, class,
/// interface, delegate, attribute and map elements have no walker element type of their width in either mode, so they
/// give `None` rather than a type that would misread them.
pub struct ReflectionTyper<'a> {
    store: &'a PackageStore,
    code: &'a CodeIndex,
    classes: &'a ClassTable,
    names: &'a NameTable,
    package: PackageId,
    mode: ParityMode,
}

impl<'a> ReflectionTyper<'a> {
    /// A typer for the exports of `package`, with the element widths of `mode`.
    pub fn new(
        store: &'a PackageStore,
        code: &'a CodeIndex,
        space: &'a ObjectSpace,
        package: PackageId,
        mode: ParityMode,
    ) -> Self {
        ReflectionTyper {
            store,
            code,
            classes: &space.classes,
            names: &space.names,
            package,
            mode,
        }
    }

    /// The class or struct a stream root reference names.
    fn root(&self, reference: i32) -> Option<Owner> {
        let (p, i) = self
            .code
            .cached_ref(self.store, self.package, reference)??;
        if let Some(key) = self.code.loaded_class(p, i) {
            return self.code.layout_id(key).map(Owner::Class);
        }
        self.code.struct_of(p, i).map(Owner::Struct)
    }

    fn decl(&self, owner: Owner, name: &str) -> Option<&'a PropertyDecl> {
        let n = self.names.find(name)?;
        match owner {
            Owner::Class(c) => self.classes.decl(self.classes.prop(c, n)?),
            Owner::Struct(s) => self.classes.struct_layout(s)?.field(n).map(|(_, d)| d),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Owner {
    Class(ClassId),
    Struct(StructId),
}

impl ArrayTyper for ReflectionTyper<'_> {
    fn element(&self, scope: &ArrayScope<'_>, property: &str) -> Option<ElementType> {
        let mut owner = self.root(scope.root.owner_ref?)?;
        for name in scope.path {
            owner = match &self.decl(owner, name)?.kind {
                PropKind::Struct(s) => Owner::Struct(*s),
                PropKind::Array(inner) => match inner.as_ref() {
                    PropKind::Struct(s) => Owner::Struct(*s),
                    _ => return None,
                },
                _ => return None,
            };
        }
        let PropKind::Array(inner) = &self.decl(owner, property)?.kind else {
            return None;
        };
        Some(match inner.as_ref() {
            PropKind::Int => ElementType::Int,
            PropKind::Float => ElementType::Float,
            PropKind::Name => ElementType::Name,
            PropKind::Str => ElementType::Str,
            PropKind::Object { .. } => ElementType::Object,
            PropKind::Byte { enum_: None } => ElementType::Byte,
            PropKind::Byte { enum_: Some(_) } if self.mode == ParityMode::Reference => {
                ElementType::Byte
            }
            PropKind::Struct(s) => ElementType::Struct(
                last_name(self.classes.struct_layout(*s)?.path().as_str()).to_string(),
            ),
            _ => return None,
        })
    }
}

/// Loads the class of every export of `package` and defines its layout, so a [`ReflectionTyper`] over `package` can
/// answer from the cache. A class that does not load or lay out is left out (its arrays stay raw).
pub fn prepare_typer(cx: &mut ValueCx<'_>, package: PackageId) {
    let classes: Vec<i32> = cx
        .store
        .get(package)
        .exports
        .iter()
        .map(|e| e.class)
        .collect();
    for class in classes {
        if let Ok(Some(key)) = cx.code.class_at(cx.store, package, class) {
            let _ = cx.layout(key);
        }
    }
}

#[cfg(test)]
pub(crate) mod tests;
