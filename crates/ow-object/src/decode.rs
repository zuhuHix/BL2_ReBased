//! Owner: lane W3-DECODE. Typed values from tagged properties (architecture section 3.3): the port of the C++ VM's
//! tag reader (`TagReader`, `applyTaggedDefaults`, `applyStructDefaults`, `Runtime::resource` in `src/vm.cpp`), typed
//! through the reflection layouts of [`crate::reflect`] instead of the C++ declaration bag.
//!
//! **What is read, and how.** [`ValueCx::apply_tags`] walks one export's tag stream from its property prefix to the
//! `None` tag and writes each value into an object's slots, as the C++ `applyTaggedDefaults` does: a tag's header
//! (name, type, size, array index; the struct or enum name of a struct or byte tag; a bool's byte) is read and
//! checked, its value is read inside a limit of exactly `size` bytes, and a value that fails to decode, or does not
//! end where the tag said, is skipped with the C++ log line (`defaults: skipped <Class>.<Property> (<Type>)`) without
//! upsetting the rest. A header that cannot be read ends the stream with an error. The value kinds are the C++ VM's:
//! int, float, the float and int attribute tags (one 4-byte number each), bool (its byte, any non-zero is true), name,
//! string, object, component and interface references (a stand-in per referenced object, [`ValueCx::resource`]),
//! class references, plain bytes and enum bytes (the value's position in the declared enum), structs and dynamic
//! arrays, whose elements are read by the inner declaration's kind with the C++ widths (a bool element 4 bytes, a
//! byte element 1 byte, enum or not). Other tag types (delegates, maps, byte attributes) are skipped and logged, as in
//! the C++.
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
//! **Stand-ins.** Each referenced object gets one heap object of its own class with [`Origin::Resource`]. An object
//! whose class no installed package exports (an engine-native class such as `Engine.StaticMesh`, or `Core.Class` for
//! a class object) gets that class defined at its import path with no properties of its own under `Core.Object`
//! ([`DecodeState::native_classes`]): nothing about its native properties is invented, and the classes between it
//! and `Core.Object`, if any, are UNVERIFIED.
//!
//! **Differences from the C++, by parity mode.** [`ParityMode::Reference`] keeps three C++ behaviours that
//! [`ParityMode::Off`] does not: an array with elements inside a struct's default stream stays empty (the C++ reads
//! those streams with no array typing; Off types them through reflection), an enum value name the enum lacks gives 0
//! (Off refuses it and skips the property), and a numbered enum value (`EPSM_4`, stored as `EPSM` and a number)
//! is compared to the enum's entries without their numbers, so it matches nothing and gives 0 (Off finds its
//! position). With those, `--object-dump` can be compared with the C++ byte for byte except where slots cannot
//! follow the bag (below) and for class properties (above).
//!
//! **What the C++ does that slots cannot.** The C++ object is a name-keyed bag, so a tag naming a property the class
//! does not declare (an editor-only property stripped from the cooked class) is stored under its name. A slot layout
//! has nowhere to put it: such a tag is dropped here. When the C++ would have failed to decode it (an array with
//! elements and no declaration, an unsupported type), the same skip line is logged; when the C++ would have decoded
//! it, nothing is logged and [`DecodeState::dropped`] records it. A value whose kind does not fit the declaration
//! (which the C++ bag would also take) is skipped and logged.
//!
//! **Enums.** An enum byte is the position of its value name in the enum the declaration names. The C++ first looks
//! for an enum of that name in the package being read; this reads the declared enum (no difference on the code
//! packages' class defaults).
//!
//! **Also here:** [`to_value`], a [`PropValue`] tree of the package walker typed by a declaration, and
//! [`ReflectionTyper`], the walker's [`ArrayTyper`] answered from reflection.
//!
//! Status: parity with the C++ VM's loading half. The tag formats are the C++ ones, observations of 832/46 packages;
//! none is confirmed against the running game (UNVERIFIED).

use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

use ow_core::{FixedHashMap, NotYet, ParityMode};
use ow_package::PackageId;
use ow_package::package::{Package, PackageError};
use ow_package::props::{ArrayScope, ArrayTyper, ElementType, PropValue};
use ow_package::reader::{ReadError, Reader};
use ow_package::store::{PackageStore, StoreError};
use ow_package::tables;

use crate::heap::{HeapError, ObjectSpace, Origin};
use crate::ids::{ClassId, ObjRef, PropAddr, PropId, StructId};
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
    /// A struct of the class table whose ScriptStruct export is not in its package.
    NoStructExport(String),
    /// A referenced object whose class has no layout (a class object, or a class no installed package holds).
    NoClass {
        object: String,
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
            DecodeError::NoStructExport(path) => write!(f, "no ScriptStruct export for {path}"),
            DecodeError::NoClass { object } => write!(f, "the class of {object} has no layout"),
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
#[derive(Debug, Default)]
pub struct DecodeState {
    resources: FixedHashMap<(PackageId, i32), ObjRef>,
    struct_defaults: FixedHashMap<StructId, Defaults>,
    defaults_nesting: usize,
    by_name: FixedHashMap<PackageId, Rc<FixedHashMap<String, Vec<i32>>>>,
    pub(crate) class_defaults: FixedHashMap<ClassKey, ObjRef>,
    /// The C++ `Runtime::log` lines this decoding produced, in order.
    pub log: Vec<String>,
    /// Tags dropped because the class does not declare their property, though the C++ VM would have decoded them
    /// into its bag (`<Class>.<Property> (<Type>)`, module documentation). Not printed.
    pub dropped: Vec<String>,
    /// The engine-native classes defined for stand-ins, by path, in order ([`ValueCx::resource`]).
    pub native_classes: Vec<String>,
}

impl DecodeState {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Everything decoding reads and writes: the store, the reflection index, the object space, the decode state and the
/// parity mode.
pub struct ValueCx<'a> {
    pub store: &'a mut PackageStore,
    pub code: &'a mut CodeIndex,
    pub space: &'a mut ObjectSpace,
    pub state: &'a mut DecodeState,
    pub mode: ParityMode,
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
        }
    }

    /// The layout of loaded class `key`, defined on first use ([`CodeIndex::define_layout`]).
    pub fn layout(&mut self, key: ClassKey) -> Result<ClassId, DecodeError> {
        Ok(self.code.define_layout(
            self.store,
            &mut self.space.names,
            &mut self.space.classes,
            key,
        )?)
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
    /// header that cannot be read), after the tags before it were applied.
    pub fn apply_tags(
        &mut self,
        object: ObjRef,
        class: ClassKey,
        package: PackageId,
        export: i32,
        prefix: usize,
    ) -> Result<(), DecodeError> {
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
            if outcome.is_err() {
                self.state.log.push(format!(
                    "defaults: skipped {class_name}.{} ({})",
                    h.name, h.ty
                ));
            }
        }
        Ok(())
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
                Value::Byte(self.enum_value(decl, &value)?)
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
                    elements.push(self.element(r, pkg, package, inner, depth, stream.defaults)?);
                }
                Value::array(elements)
            }
            other => return Err(DecodeError::UnsupportedTag(other.to_string())),
        })
    }

    /// An array element of kind `kind` (the C++ `element`, by the inner declaration's type).
    fn element(
        &mut self,
        r: &mut Reader<'_>,
        pkg: &Arc<Package>,
        package: PackageId,
        kind: &PropKind,
        depth: u32,
        defaults: bool,
    ) -> Result<Value, DecodeError> {
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
            PropKind::Byte { .. } => Value::Byte(byte(r)?),
            PropKind::Bool => Value::Bool(r.u32()? != 0),
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
    pub fn ensure_struct_defaults(&mut self, s: StructId) -> Result<(), DecodeError> {
        match self.state.struct_defaults.get(&s) {
            Some(Defaults::Done(outcome)) => return outcome.clone(),
            Some(Defaults::Reading) => {
                return Err(DecodeError::StructDefaultsCycle(self.struct_path(s)));
            }
            None => {}
        }
        if self.state.defaults_nesting >= MAX_DEFAULTS_NESTING {
            return Err(DecodeError::DefaultsTooDeep);
        }
        self.state.struct_defaults.insert(s, Defaults::Reading);
        self.state.defaults_nesting += 1;
        let outcome = self.read_struct_defaults(s);
        self.state.defaults_nesting -= 1;
        self.state
            .struct_defaults
            .insert(s, Defaults::Done(outcome.clone()));
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
        let mut candidate = self.space.classes.struct_defaults(s)?;
        let stream = Stream {
            top: false,
            defaults: true,
            depth: 1,
        };
        if self
            .tagged(&mut r, &pkg, package, &mut candidate, s, stream)
            .is_err()
            || r.pos() != end
        {
            return Ok(());
        }
        Ok(self.space.set_struct_defaults(s, candidate)?)
    }

    /// The ScriptStruct export a class-table struct was defined from: its path is `<package stem>.<object path>`.
    fn struct_export(&mut self, s: StructId, path: &str) -> Result<(PackageId, i32), DecodeError> {
        let missing = || DecodeError::NoStructExport(path.to_string());
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
    /// reference otherwise).
    fn object_value(
        &mut self,
        package: PackageId,
        reference: i32,
        kind: &PropKind,
    ) -> Result<Value, DecodeError> {
        let object = self.resource(package, reference)?;
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

    /// The stand-in for the object `reference` of `package` names (the C++ `Runtime::resource`): an import is
    /// followed to the export that holds it where one does (else the stand-in names the import), and each object
    /// gets one stand-in, of the object's class, with [`Origin::Resource`]. `None` for reference 0.
    pub fn resource(
        &mut self,
        package: PackageId,
        reference: i32,
    ) -> Result<Option<ObjRef>, DecodeError> {
        if reference == 0 {
            return Ok(None);
        }
        let (owner, index) = if reference < 0 {
            self.code
                .resolve_ref(self.store, package, reference)
                .unwrap_or((package, reference))
        } else {
            (package, reference)
        };
        if let Some(&o) = self.state.resources.get(&(owner, index)) {
            return Ok(Some(o));
        }
        let pkg = self.store.shared(owner);
        let entry = pkg.object(index)?;
        let class = self.object_class(owner, &pkg, index)?;
        let o = self.space.alloc(
            class,
            &entry.name,
            None,
            Origin::Resource {
                package: owner,
                index,
            },
        )?;
        self.state.resources.insert((owner, index), o);
        Ok(Some(o))
    }

    /// The class of object `index` of `package`: an export's class reference, or an unresolved import's class path.
    /// A class that no installed package exports (an engine-native class such as `Engine.StaticMesh` or `Core.Class`,
    /// the class of a class object) gets an [engine-native stand-in class](ValueCx::native_class).
    fn object_class(
        &mut self,
        package: PackageId,
        pkg: &Package,
        index: i32,
    ) -> Result<ClassId, DecodeError> {
        let entry = pkg.object(index)?;
        let no_class = || DecodeError::NoClass {
            object: pkg.path_of(index).unwrap_or_default(),
        };
        let (key, native_path) = if index > 0 {
            if entry.class == 0 {
                (None, "Core.Class".to_string())
            } else {
                let key = self
                    .code
                    .class_at(self.store, package, entry.class)
                    .map_err(|_| no_class())?;
                let path = if entry.class < 0 {
                    pkg.path_of(entry.class)?
                } else {
                    String::new()
                };
                (key, path)
            }
        } else {
            let path = format!("{}.{}", entry.class_package, entry.class_name);
            (self.code.find_class(self.store, &path).ok(), path)
        };
        match key {
            Some(key) => self.layout(key).map_err(|_| no_class()),
            None if !native_path.is_empty() => self.native_class(&native_path).ok_or_else(no_class),
            None => Err(no_class()),
        }
    }

    /// The class of a stand-in for an object whose class no installed package exports: the class at its import
    /// path, defined (once, if nothing defined it before) with no properties of its own under `Core.Object`, the root
    /// every class derives from. Its native properties are not in the packages, so none is invented; the classes
    /// between it and `Core.Object`, if any, are not known (UNVERIFIED). Each one defined here is listed in
    /// [`DecodeState::native_classes`]. `None` when `Core.Object` has no layout yet.
    fn native_class(&mut self, path: &str) -> Option<ClassId> {
        let classes = &mut self.space.classes;
        if let Some(id) = classes.by_path(path)
            && classes.get_layout(id).is_some_and(|l| l.is_defined())
        {
            return Some(id);
        }
        let object = classes
            .by_path("Core.Object")
            .filter(|&o| classes.get_layout(o).is_some_and(|l| l.is_defined()))?;
        let id = classes.define(path, Some(object), Vec::new()).ok()?;
        self.state.native_classes.push(path.to_string());
        Some(id)
    }

    /// The byte of enum value `value` for a byte declaration: its position in the declared enum.
    fn enum_value(&mut self, decl: &PropertyDecl, value: &str) -> Result<u8, DecodeError> {
        let layout = match decl.kind {
            PropKind::Byte { enum_: Some(e) } | PropKind::ByteAttribute { enum_: Some(e) } => {
                self.space.classes.enum_layout(e)
            }
            _ => None,
        };
        let position = match layout {
            // The C++ `enumPosition` compares each enum entry's text without its instance number to the value's whole
            // text, so a numbered value (`EPSM_4`, stored as `EPSM` and a number) matches nothing there and is 0.
            Some(layout) if self.mode == ParityMode::Reference => Some(
                layout
                    .values()
                    .iter()
                    .position(|&v| self.space.names.base_text(v) == value)
                    .unwrap_or(0),
            ),
            Some(layout) => self
                .space
                .names
                .find(value)
                .and_then(|n| layout.index_of(n)),
            None => None,
        };
        match position {
            Some(p) => u8::try_from(p).map_err(|_| DecodeError::UnknownEnumValue {
                property: self.space.names.text(decl.name).into_owned(),
                value: value.to_string(),
            }),
            // The C++ gives 0 to a value it cannot place (UNVERIFIED there); only the reference mode keeps that.
            None if self.mode == ParityMode::Reference => Ok(0),
            None => Err(DecodeError::UnknownEnumValue {
                property: self.space.names.text(decl.name).into_owned(),
                value: value.to_string(),
            }),
        }
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
pub fn to_value(
    tree: &PropValue,
    decl: &PropertyDecl,
    package: PackageId,
    cx: &mut ValueCx<'_>,
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
            Value::Byte(cx.enum_value(decl, name)?)
        }
        (PropKind::Name, PropValue::Name(text)) => Value::Name(cx.space.names.intern(text)),
        (PropKind::Str, PropValue::Str(text)) => Value::string(text),
        (PropKind::Object { .. } | PropKind::Interface { .. }, PropValue::Object(r)) => {
            cx.object_value(package, *r, &decl.kind)?
        }
        (PropKind::Class { .. }, PropValue::Object(r)) => cx.class_value(package, *r)?,
        (PropKind::Struct(s), PropValue::Struct { fields, .. }) => {
            let s = *s;
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
                    return Err(DecodeError::Undeclared(entry.name.clone()));
                };
                let v = to_value(&entry.value, &field_decl, package, cx)?;
                let at =
                    if field_decl.array_dim > 1 && !matches!(field_decl.kind, PropKind::Array(_)) {
                        if entry.index < 0 || entry.index >= field_decl.array_dim {
                            return Err(DecodeError::StaticIndex {
                                property: entry.name.clone(),
                                index: entry.index,
                            });
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
            let element = PropertyDecl::new(decl.name, inner.as_ref().clone());
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(to_value(item, &element, package, cx)?);
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
/// to `StructProperty:<name>`). Bool, class, interface, delegate, attribute and map elements have no walker decoder
/// that reads them as the C++ VM does, so they give `None` rather than a type that would misread them.
pub struct ReflectionTyper<'a> {
    store: &'a PackageStore,
    code: &'a CodeIndex,
    classes: &'a ClassTable,
    names: &'a NameTable,
    package: PackageId,
}

impl<'a> ReflectionTyper<'a> {
    /// A typer for the exports of `package`.
    pub fn new(
        store: &'a PackageStore,
        code: &'a CodeIndex,
        space: &'a ObjectSpace,
        package: PackageId,
    ) -> Self {
        ReflectionTyper {
            store,
            code,
            classes: &space.classes,
            names: &space.names,
            package,
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
            PropKind::Byte { .. } => ElementType::Byte,
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
