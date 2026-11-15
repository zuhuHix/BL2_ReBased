//! Owner: lane W2-PROPS. The tagged-property tag walker and its serialized tree (architecture section 3.2): the port
//! of the C++ `Package::properties`, `tags`, `structure` and `floating` (`src/package.cpp`).
//!
//! [`read_tagged`] walks the tag stream of one export from a given offset to its `None` tag and returns the tree,
//! with object references still package-relative, and where the walk stopped (the start of any untagged tail). It
//! decodes exactly what the C++ walker decodes and nothing more: the same types, the same fixed struct layouts
//! (Vector, Vector2D, Rotator, Guid, LinearColor, Color, Quat, Plane, Box, Matrix), the same nesting bound and every
//! C++ check in the same order with the same message. A value the C++ walker leaves undecoded (an unknown tag type,
//! an array with elements and no element type) is kept as [`PropValue::Raw`] bytes, never guessed.
//!
//! Array element types come from an [`ArrayTyper`], asked with an [`ArrayScope`] that says where the array stands.
//! Until W3-DECODE's reflection typer lands, the typer is the [`crate::schema::SchemaTyper`] built from the
//! `--array-schema` files, which, like the C++ `arrayTypes` map, looks the property up by name only.
//!
//! Status: parity with the C++ walker only. The fixed struct layouts and the W,X,Y,Z order of `Plane` are the C++
//! reader's observations of 832/46 packages; none is confirmed against the running game (UNVERIFIED).

use std::fmt;

use crate::package::{Package, PackageError};
use crate::reader::{ReadError, Reader};
use crate::tables;

/// The deepest nesting the walker accepts: a tag stream or a struct at a depth above this fails with "property
/// nesting exceeds 32" (C++ `tags` and `structure`).
pub const MAX_DEPTH: u32 = 32;

/// The largest array element count the walker accepts (C++ `tags`, "invalid property array count").
pub const MAX_ARRAY_COUNT: i32 = 1_000_000;

/// The element type of an `ArrayProperty`, as an `--array-schema` line names it (the text after `=`).
///
/// More kinds may be added (`ClassProperty`, `ComponentProperty`, `InterfaceProperty`, a 32-bit `BoolProperty`, as
/// `src/vm.cpp` handles them); [`ElementType::parse`] then keeps mapping each text to what the C++ `--array-schema`
/// walker does with it, so a schema's output does not change.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElementType {
    /// `IntProperty`
    Int,
    /// `FloatProperty`
    Float,
    /// `NameProperty`
    Name,
    /// `StrProperty`
    Str,
    /// `ObjectProperty` (only this spelling: the C++ array branch has no Class or Component form)
    Object,
    /// `ByteProperty`
    Byte,
    /// `StructProperty:<struct name>`
    Struct(String),
    /// Any other text: no decoder in this walker yet. The C++ schema loader accepts it; decoding an element of it
    /// fails with "unsupported array schema type", and an empty array of it decodes.
    Other(String),
}

impl ElementType {
    /// The type a schema value names. Every text maps to exactly one type and [`ElementType::spec`] gives it back.
    pub fn parse(spec: &str) -> ElementType {
        match spec {
            "IntProperty" => ElementType::Int,
            "FloatProperty" => ElementType::Float,
            "NameProperty" => ElementType::Name,
            "StrProperty" => ElementType::Str,
            "ObjectProperty" => ElementType::Object,
            "ByteProperty" => ElementType::Byte,
            _ => match spec.strip_prefix("StructProperty:") {
                Some(name) => ElementType::Struct(name.to_string()),
                None => ElementType::Other(spec.to_string()),
            },
        }
    }

    /// The schema text of this type (what `--properties` prints as `element_type`).
    pub fn spec(&self) -> String {
        match self {
            ElementType::Int => "IntProperty".into(),
            ElementType::Float => "FloatProperty".into(),
            ElementType::Name => "NameProperty".into(),
            ElementType::Str => "StrProperty".into(),
            ElementType::Object => "ObjectProperty".into(),
            ElementType::Byte => "ByteProperty".into(),
            ElementType::Struct(name) => format!("StructProperty:{name}"),
            ElementType::Other(text) => text.clone(),
        }
    }
}

/// Where a tag stream starts: what owns it. [`read_tagged`] takes it from the export ([`StreamRoot::for_export`]);
/// a caller that knows more (W3-DECODE decoding a struct's defaults, say) passes its own to [`read_tagged_in`].
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamRoot {
    /// The owner text the typer sees for the stream's own tags.
    pub owner: String,
    /// The package-relative reference of the owning class or struct, when known.
    pub owner_ref: Option<i32>,
}

impl StreamRoot {
    pub fn new(owner: impl Into<String>, owner_ref: Option<i32>) -> Self {
        StreamRoot {
            owner: owner.into(),
            owner_ref,
        }
    }

    /// The root of export `export`'s own tags: its class object's name (`Class` when the class reference is 0) and,
    /// for a nonzero class, that reference.
    pub fn for_export(pkg: &Package, export: i32) -> Result<StreamRoot, PropError> {
        let class = pkg.object(export)?.class;
        Ok(if class == 0 {
            StreamRoot::new("Class", None)
        } else {
            StreamRoot::new(pkg.object(class)?.name.clone(), Some(class))
        })
    }
}

/// Where an array property stands, for [`ArrayTyper::element`]. The walker fills it.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArrayScope<'a> {
    /// The owner text: the root's owner for the stream's own tags, the struct name inside a struct (what the
    /// schema typer's lookups have always seen; it ignores it).
    pub owner: &'a str,
    /// The package-relative reference of the owning class or struct, where the walker knows it: the root's
    /// `owner_ref` for the stream's own tags, `None` inside a struct (a tag names its struct by name only).
    pub owner_ref: Option<i32>,
    /// The names of the enclosing struct and array properties from the stream's root, outermost first (empty for
    /// the stream's own tags; `Rows`, `Inner` for an array in struct `Inner` in an element of array `Rows`).
    pub path: &'a [String],
    /// The walk's root (the export's class from [`StreamRoot::for_export`], or the caller's root given to
    /// [`read_tagged_in`]): the same for every array of one walk, so with `path` it tells apart same-named structs
    /// of different owners (`A.State` and `B.State` both seen as owner `State` inside a struct).
    pub root: &'a StreamRoot,
}

impl<'a> ArrayScope<'a> {
    pub fn new(owner: &'a str, owner_ref: Option<i32>, path: &'a [String], root: &'a StreamRoot) -> Self {
        ArrayScope {
            owner,
            owner_ref,
            path,
            root,
        }
    }
}

/// Gives the element type of array `property` in `scope`. `None`: not known, and the array is kept as raw bytes.
pub trait ArrayTyper {
    fn element(&self, scope: &ArrayScope<'_>, property: &str) -> Option<ElementType>;
}

/// A typer that knows no array: every array with elements stays raw (the C++ walker with no `--array-schema`).
#[derive(Debug, Clone, Copy, Default)]
pub struct NoArrayTypes;

impl ArrayTyper for NoArrayTypes {
    fn element(&self, _scope: &ArrayScope<'_>, _property: &str) -> Option<ElementType> {
        None
    }
}

/// One serialized value.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum PropValue {
    Int(i32),
    /// Always finite: the walker refuses a non-finite float.
    Float(f32),
    Bool(bool),
    /// A `ByteProperty` whose enum is `None`, a byte array element, a `Color` channel or a `Box`'s `IsValid`.
    Byte(u8),
    /// A `ByteProperty` of a named enum: the value's name.
    Enum(String),
    Name(String),
    Str(String),
    /// An `ObjectProperty`, `ClassProperty` or `ComponentProperty` (the tag's type says which), or an object array
    /// element: a reference into this package (0 none, negative an import, positive an export). The walker checks
    /// that a nonzero reference names an object whose path can be built, as the C++ walker does.
    Object(i32),
    /// Reserved by the contract for a decoded `DelegateProperty`. The walker never produces it: the C++ walker
    /// leaves delegate tags undecoded, so they are [`PropValue::Raw`] here (W3-DECODE decides).
    Delegate { object: i32, function: String },
    /// A struct. A tagged struct holds its tag stream, each entry with its [`Tag`] (whose offsets count from this
    /// struct's start); a fixed-layout struct (Vector, Rotator, Box, ...) holds its fields in serialized order, each
    /// entry with `tag: None`. `start` is where the struct's bytes begin, from the export's payload start.
    ///
    /// `==` on a `PropValue` compares `start` too, so the same struct value at two places is not equal: a consumer
    /// comparing values compares `ty` and `fields`.
    Struct { ty: String, fields: Vec<PropEntry>, start: usize },
    /// The elements of an array whose element type was given.
    Array(Vec<PropValue>),
    /// The bytes of a value the walker does not decode: an unknown tag type (the whole value), or an array with
    /// elements and no element type (the bytes after the count). Kept, never guessed.
    Raw(Vec<u8>),
}

/// One property: its name, array index and value, and the tag it was read from (`None` for a fixed-layout struct
/// field, which has no tag).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct PropEntry {
    pub name: String,
    pub index: i32,
    pub value: PropValue,
    pub tag: Option<Tag>,
}

impl PropEntry {
    pub fn new(name: impl Into<String>, index: i32, value: PropValue, tag: Option<Tag>) -> Self {
        PropEntry {
            name: name.into(),
            index,
            value,
            tag,
        }
    }
}

/// What a property tag held besides the name and array index.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tag {
    /// The tag's type name (`IntProperty`, `StructProperty`, ...).
    pub ty: String,
    /// The struct or enum name of a `StructProperty` or `ByteProperty` tag (`None` text for a plain byte); `None`
    /// for every other type.
    pub type_name: Option<String>,
    /// Where the tag starts, from the walk's base: the export's payload start for the export's own tags (so the
    /// first offset is the start offset given to [`read_tagged`]), the enclosing struct's start inside a struct
    /// (add that struct's `start` to place it in the payload).
    pub offset: usize,
    /// The tag's size word: the value's byte count, not counting a bool's byte or the type name.
    pub size: i32,
    /// For an `ArrayProperty`: its element count and the element type the typer gave.
    pub array: Option<ArrayTag>,
}

impl Tag {
    pub fn new(ty: impl Into<String>, type_name: Option<String>, offset: usize, size: i32, array: Option<ArrayTag>) -> Self {
        Tag {
            ty: ty.into(),
            type_name,
            offset,
            size,
            array,
        }
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArrayTag {
    pub count: i32,
    pub element: Option<ElementType>,
}

impl ArrayTag {
    pub fn new(count: i32, element: Option<ElementType>) -> Self {
        ArrayTag { count, element }
    }
}

/// The result of [`read_tagged`].
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct TaggedProps {
    pub entries: Vec<PropEntry>,
    /// Where the walk stopped, from the export's payload start: just past the `None` tag, so the start of any
    /// untagged tail.
    pub end: usize,
}

impl TaggedProps {
    pub fn new(entries: Vec<PropEntry>, end: usize) -> Self {
        TaggedProps { entries, end }
    }
}

/// Why a tag stream could not be walked. `Display` gives the C++ exception text; a failure inside a tagged struct
/// is [`PropError::InStruct`], whose cause is its `source()`, so a chained report reads `struct X: <cause>` as the
/// C++ message does.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropError {
    /// A reader or package error (a truncated value, an invalid name or reference, a cyclic outer chain).
    Package(PackageError),
    /// "properties requires a positive export index"
    NotPositiveExport,
    /// "property offset exceeds export size"
    OffsetPastExport,
    /// "property nesting exceeds 32"
    TooDeep,
    /// "negative property size/index"
    NegativeSizeOrIndex,
    /// "invalid property boolean"
    InvalidBool,
    /// "non-finite property float"
    NonFiniteFloat,
    /// "non-finite struct/array float"
    NonFiniteStructFloat,
    /// "invalid Box validity byte"
    InvalidBoxValidity,
    /// "invalid property array count"
    InvalidArrayCount,
    /// "unsupported array schema type"
    UnsupportedElementType,
    /// "property size does not match decoded value"
    SizeMismatch,
    /// "struct <ty>", caused by `error` (the C++ `"struct " + type + ": " + what()`).
    InStruct { ty: String, error: Box<PropError> },
}

impl fmt::Display for PropError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PropError::Package(e) => e.fmt(f),
            PropError::NotPositiveExport => f.write_str("properties requires a positive export index"),
            PropError::OffsetPastExport => f.write_str("property offset exceeds export size"),
            PropError::TooDeep => f.write_str("property nesting exceeds 32"),
            PropError::NegativeSizeOrIndex => f.write_str("negative property size/index"),
            PropError::InvalidBool => f.write_str("invalid property boolean"),
            PropError::NonFiniteFloat => f.write_str("non-finite property float"),
            PropError::NonFiniteStructFloat => f.write_str("non-finite struct/array float"),
            PropError::InvalidBoxValidity => f.write_str("invalid Box validity byte"),
            PropError::InvalidArrayCount => f.write_str("invalid property array count"),
            PropError::UnsupportedElementType => f.write_str("unsupported array schema type"),
            PropError::SizeMismatch => f.write_str("property size does not match decoded value"),
            PropError::InStruct { ty, .. } => write!(f, "struct {ty}"),
        }
    }
}

impl std::error::Error for PropError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            // The package error's own text is this error's text, so its cause (if any) is this error's cause.
            PropError::Package(e) => e.source(),
            PropError::InStruct { error, .. } => Some(error.as_ref()),
            _ => None,
        }
    }
}

impl From<PackageError> for PropError {
    fn from(e: PackageError) -> Self {
        PropError::Package(e)
    }
}

impl From<ReadError> for PropError {
    fn from(e: ReadError) -> Self {
        PropError::Package(PackageError::Read(e))
    }
}

/// The byte range of export `export`'s tag stream: (`start` bytes into its payload, the payload's end), both
/// absolute in the package data. The C++ checks of `Package::properties`, in its order: a positive export index, an
/// existing export, `start` not past the payload's size.
pub fn property_range(pkg: &Package, export: i32, start: usize) -> Result<(usize, usize), PropError> {
    if export <= 0 {
        return Err(PropError::NotPositiveExport);
    }
    let object = pkg.object(export)?;
    // The loader checked offset and size to be non-negative and inside the data.
    let size = object.size as usize;
    let offset = object.offset as usize;
    if start > size {
        return Err(PropError::OffsetPastExport);
    }
    Ok((offset + start, offset + size))
}

/// Walks export `export`'s tag stream from `start` bytes into its payload to its `None` tag (C++
/// `Package::properties` without the JSON), with the root taken from the export ([`StreamRoot::for_export`]). The
/// reads are limited to the export's payload. `end` in the result is where the walk stopped, from the payload's
/// start.
///
/// The C++ `properties` builds the export's path between the range checks and the walk; a caller that wants the
/// C++ error order calls [`property_range`] and [`Package::path_of`] first (as `--properties` does).
pub fn read_tagged(
    pkg: &Package,
    export: i32,
    start: usize,
    typer: &dyn ArrayTyper,
) -> Result<TaggedProps, PropError> {
    property_range(pkg, export, start)?;
    let root = StreamRoot::for_export(pkg, export)?;
    read_tagged_in(pkg, export, start, &root, typer)
}

/// [`read_tagged`] with the stream's root given by the caller.
pub fn read_tagged_in(
    pkg: &Package,
    export: i32,
    start: usize,
    root: &StreamRoot,
    typer: &dyn ArrayTyper,
) -> Result<TaggedProps, PropError> {
    let (begin, object_end) = property_range(pkg, export, start)?;
    let base = pkg.object(export)?.offset as usize;
    let mut r = pkg.reader();
    r.set_pos(begin);
    r.set_limit(object_end);
    let mut walk = Walk {
        pkg,
        typer,
        payload: base,
        path: Vec::new(),
        root,
    };
    let entries = walk.tags(&mut r, base, 0, &root.owner, root.owner_ref)?;
    Ok(TaggedProps {
        entries,
        end: r.pos() - base,
    })
}

/// How a fixed-layout struct's fields are stored.
#[derive(Clone, Copy)]
enum FieldKind {
    Float,
    Int,
    Byte,
}

/// The fixed layouts of `Package::structure` besides Box and Matrix: field names in serialized order and their
/// kind. Observed 832/46 tagged `Plane` (and `Matrix` row) serialization is W,X,Y,Z (the C++ reader's note).
fn fixed_layout(ty: &str) -> Option<(&'static [&'static str], FieldKind)> {
    Some(match ty {
        "Vector" => (&["X", "Y", "Z"], FieldKind::Float),
        "Vector2D" => (&["X", "Y"], FieldKind::Float),
        "Rotator" => (&["Pitch", "Yaw", "Roll"], FieldKind::Int),
        "Guid" => (&["A", "B", "C", "D"], FieldKind::Int),
        "LinearColor" => (&["R", "G", "B", "A"], FieldKind::Float),
        "Color" => (&["B", "G", "R", "A"], FieldKind::Byte),
        "Quat" => (&["X", "Y", "Z", "W"], FieldKind::Float),
        "Plane" => (&["W", "X", "Y", "Z"], FieldKind::Float),
        _ => return None,
    })
}

fn field(name: &str, value: PropValue) -> PropEntry {
    PropEntry::new(name, 0, value, None)
}

struct Walk<'p> {
    pkg: &'p Package,
    typer: &'p dyn ArrayTyper,
    /// The export's payload start (absolute), for each struct's `start`.
    payload: usize,
    /// The enclosing struct and array property names, outermost first.
    path: Vec<String>,
    /// The walk's root, passed to the typer with every array.
    root: &'p StreamRoot,
}

impl Walk<'_> {
    fn name(&self, r: &mut Reader<'_>) -> Result<String, PropError> {
        Ok(tables::read_name(r, &self.pkg.names)?)
    }

    fn byte(r: &mut Reader<'_>) -> Result<u8, PropError> {
        Ok(r.take(1)?[0])
    }

    /// An object reference: read, and a nonzero one checked to have a path (the C++ prints `path(reference)` at
    /// this point, so a bad reference fails here and not later).
    fn reference(&self, r: &mut Reader<'_>) -> Result<i32, PropError> {
        let reference = r.i32()?;
        if reference != 0 {
            self.pkg.path_of(reference)?;
        }
        Ok(reference)
    }

    /// A struct or array float (C++ `Package::floating`).
    fn floating(r: &mut Reader<'_>) -> Result<f32, PropError> {
        let value = f32::from_bits(r.u32()?);
        if !value.is_finite() {
            return Err(PropError::NonFiniteStructFloat);
        }
        Ok(value)
    }

    /// C++ `Package::structure`.
    fn structure(&mut self, r: &mut Reader<'_>, ty: &str, depth: u32) -> Result<PropValue, PropError> {
        if depth > MAX_DEPTH {
            return Err(PropError::TooDeep);
        }
        let start = r.pos() - self.payload;
        let fields = if ty == "Box" {
            let minimum = self.structure(r, "Vector", depth + 1)?;
            let maximum = self.structure(r, "Vector", depth + 1)?;
            let valid = Self::byte(r)?;
            if valid > 1 {
                return Err(PropError::InvalidBoxValidity);
            }
            vec![
                field("Min", minimum),
                field("Max", maximum),
                field("IsValid", PropValue::Byte(valid)),
            ]
        } else if ty == "Matrix" {
            let mut planes = Vec::with_capacity(4);
            for axis in ["XPlane", "YPlane", "ZPlane", "WPlane"] {
                planes.push(field(axis, self.structure(r, "Plane", depth + 1)?));
            }
            planes
        } else if let Some((names, kind)) = fixed_layout(ty) {
            let mut fields = Vec::with_capacity(names.len());
            for name in names {
                let value = match kind {
                    FieldKind::Byte => PropValue::Byte(Self::byte(r)?),
                    FieldKind::Int => PropValue::Int(r.i32()?),
                    FieldKind::Float => PropValue::Float(Self::floating(r)?),
                };
                fields.push(field(name, value));
            }
            fields
        } else {
            let base = r.pos();
            self.tags(r, base, depth, ty, None).map_err(|error| PropError::InStruct {
                ty: ty.to_string(),
                error: Box::new(error),
            })?
        };
        Ok(PropValue::Struct {
            ty: ty.to_string(),
            fields,
            start,
        })
    }

    /// C++ `Package::tags`: tags until `None`; each value limited to its tag's size and required to use all of it.
    fn tags(
        &mut self,
        r: &mut Reader<'_>,
        base: usize,
        depth: u32,
        owner: &str,
        owner_ref: Option<i32>,
    ) -> Result<Vec<PropEntry>, PropError> {
        if depth > MAX_DEPTH {
            return Err(PropError::TooDeep);
        }
        let stream_end = r.limit();
        let mut entries = Vec::new();
        loop {
            // The cursor only moves forward from `base`.
            let offset = r.pos() - base;
            let name = self.name(r)?;
            if name == "None" {
                break;
            }
            let ty = self.name(r)?;
            let size = r.i32()?;
            let index = r.i32()?;
            if size < 0 || index < 0 {
                return Err(PropError::NegativeSizeOrIndex);
            }
            let type_name = if ty == "StructProperty" || ty == "ByteProperty" {
                Some(self.name(r)?)
            } else {
                None
            };
            let mut boolean = false;
            if ty == "BoolProperty" {
                let b = Self::byte(r)?;
                if b > 1 {
                    return Err(PropError::InvalidBool);
                }
                boolean = b == 1;
            }
            let size_bytes = size as usize;
            r.require(size_bytes)?;
            let end = r.pos() + size_bytes;
            r.set_limit(end);
            let detail = type_name.as_deref().unwrap_or("");
            let mut array = None;
            let value = match ty.as_str() {
                "IntProperty" => PropValue::Int(r.i32()?),
                "FloatProperty" => {
                    let value = f32::from_bits(r.u32()?);
                    if !value.is_finite() {
                        return Err(PropError::NonFiniteFloat);
                    }
                    PropValue::Float(value)
                }
                "BoolProperty" => PropValue::Bool(boolean),
                "NameProperty" => PropValue::Name(self.name(r)?),
                "StrProperty" => PropValue::Str(r.string()?),
                "ObjectProperty" | "ClassProperty" | "ComponentProperty" => PropValue::Object(self.reference(r)?),
                "ByteProperty" if detail == "None" => PropValue::Byte(Self::byte(r)?),
                "ByteProperty" => PropValue::Enum(self.name(r)?),
                "StructProperty" => {
                    self.path.push(name.clone());
                    let value = self.structure(r, detail, depth + 1)?;
                    self.path.pop();
                    value
                }
                "ArrayProperty" => {
                    let count = r.i32()?;
                    if !(0..=MAX_ARRAY_COUNT).contains(&count) {
                        return Err(PropError::InvalidArrayCount);
                    }
                    let element = self.typer
                        .element(&ArrayScope::new(owner, owner_ref, &self.path, self.root), &name);
                    let value = match &element {
                        None if count != 0 => {
                            let rest = end - r.pos();
                            PropValue::Raw(r.take(rest)?.to_vec())
                        }
                        _ => {
                            self.path.push(name.clone());
                            let mut values = Vec::with_capacity(count.min(1024) as usize);
                            for _ in 0..count {
                                values.push(self.element(r, element.as_ref(), depth)?);
                            }
                            self.path.pop();
                            PropValue::Array(values)
                        }
                    };
                    array = Some(ArrayTag { count, element });
                    value
                }
                _ => PropValue::Raw(r.take(size_bytes)?.to_vec()),
            };
            if r.pos() != end {
                return Err(PropError::SizeMismatch);
            }
            r.set_limit(stream_end);
            entries.push(PropEntry {
                name,
                index,
                value,
                tag: Some(Tag {
                    ty,
                    type_name,
                    offset,
                    size,
                    array,
                }),
            });
        }
        Ok(entries)
    }

    /// One array element of type `element` (C++ `tags`, the array branch). Only reached with a type: an untyped
    /// array has no element loop unless it is empty.
    fn element(&mut self, r: &mut Reader<'_>, element: Option<&ElementType>, depth: u32) -> Result<PropValue, PropError> {
        Ok(match element {
            Some(ElementType::Int) => PropValue::Int(r.i32()?),
            Some(ElementType::Float) => PropValue::Float(Self::floating(r)?),
            Some(ElementType::Name) => PropValue::Name(self.name(r)?),
            Some(ElementType::Str) => PropValue::Str(r.string()?),
            Some(ElementType::Object) => PropValue::Object(self.reference(r)?),
            Some(ElementType::Byte) => PropValue::Byte(Self::byte(r)?),
            Some(ElementType::Struct(name)) => self.structure(r, name, depth + 1)?,
            Some(ElementType::Other(_)) | None => return Err(PropError::UnsupportedElementType),
        })
    }
}

#[cfg(test)]
mod tests {
    //! The cases of `tests/properties_test.py` on `testkit` packages (invented values only), at the tree level; the
    //! JSON text is pinned in `ow-cli`'s `cmd/properties.rs` tests.

    use super::*;
    use crate::schema::SchemaTyper;
    use crate::testkit::{Export, PackageBuilder, ints};

    type Build<'a> = Box<dyn Fn(&mut PackageBuilder) -> Vec<u8> + 'a>;

    /// A package with import -1 `Root` (class `Class`) and export 1 `Child_1` of class -1 holding the payload
    /// `build` writes.
    fn package_with(build: impl FnOnce(&mut PackageBuilder) -> Vec<u8>) -> Package {
        let mut b = PackageBuilder::new_empty();
        // UTF-16 names, as `properties_test.py`'s `fstring()` writes them (an ANSI name is read as Latin-1).
        b.names_utf16(["None", "Root", "Child", "Class", "Wide😀", "Quoted\"\\\n"]);
        let root = b.add_import("Root", "Class", 0, "Root");
        let payload = build(&mut b);
        let mut export = Export::new(root, "Child", payload);
        export.number = 2;
        b.add_export_with(export);
        Package::from_decoded(std::path::PathBuf::new(), "synthetic".into(), b.build()).unwrap()
    }

    /// `ints(&[0])`, the tags `build` writes, `None`.
    fn stream(build: impl FnOnce(&mut PackageBuilder) -> Vec<u8>) -> Package {
        package_with(|b| {
            let mut out = ints(&[0]);
            out.extend(build(b));
            out.extend(b.none());
            out
        })
    }

    fn walk(p: &Package, start: usize, typer: &dyn ArrayTyper) -> Result<TaggedProps, PropError> {
        read_tagged(p, 1, start, typer)
    }

    fn floats(ty: &str, fields: &[(&str, f32)]) -> PropValue {
        PropValue::Struct {
            ty: ty.into(),
            fields: fields.iter().map(|(n, v)| field(n, PropValue::Float(*v))).collect(),
            start: 0,
        }
    }

    /// `value` with every struct's `start` set to 0, to compare shapes (`struct_starts_place_nested_values` checks
    /// the starts).
    fn unplaced(value: &PropValue) -> PropValue {
        match value {
            PropValue::Struct { ty, fields, .. } => PropValue::Struct {
                ty: ty.clone(),
                fields: fields
                    .iter()
                    .map(|f| PropEntry::new(f.name.clone(), f.index, unplaced(&f.value), f.tag.clone()))
                    .collect(),
                start: 0,
            },
            PropValue::Array(values) => PropValue::Array(values.iter().map(unplaced).collect()),
            other => other.clone(),
        }
    }

    fn le(values: &[f32]) -> Vec<u8> {
        values.iter().flat_map(|x| x.to_le_bytes()).collect()
    }

    #[test]
    fn every_scalar_kind_and_the_tail() {
        let p = package_with(|b| {
            let mut out = ints(&[123]);
            out.extend(b.int_tag("Test", -42));
            out.extend(b.float_tag("Test", 1.25));
            out.extend(b.bool_tag("Test", true));
            let wide = b.fname("Wide😀", 3);
            out.extend(b.tag("Test", "NameProperty", &wide));
            out.extend(b.object_tag("Test", -1));
            out.extend(b.object_tag("Test", 0));
            out.extend(b.str_tag("Test", "hello😀\"\n"));
            out.extend(b.byte_tag("Test", 127));
            out.extend(b.enum_tag("Test", "Enum", "Choice"));
            out.extend(b.struct_tag("Test", "Vector", &[0; 12]));
            out.extend(b.array_tag("Test", 2, &ints(&[11, 22])));
            out.extend(b.tag("Test", "UnknownProperty", b"opaque"));
            out.extend(b.tag_full("Quoted\"\\\n", 3, "IntProperty", 4, &[], &ints(&[99])));
            out.extend(b.none());
            out.extend(b"tail");
            out
        });
        let t = walk(&p, 4, &NoArrayTypes).unwrap();
        let values: Vec<PropValue> = t.entries.iter().map(|e| unplaced(&e.value)).collect();
        assert_eq!(
            values,
            vec![
                PropValue::Int(-42),
                PropValue::Float(1.25),
                PropValue::Bool(true),
                PropValue::Name("Wide😀_2".into()),
                PropValue::Object(-1),
                PropValue::Object(0),
                PropValue::Str("hello😀\"\n".into()),
                PropValue::Byte(127),
                PropValue::Enum("Choice".into()),
                floats("Vector", &[("X", 0.0), ("Y", 0.0), ("Z", 0.0)]),
                PropValue::Raw(ints(&[11, 22])),
                PropValue::Raw(b"opaque".to_vec()),
                PropValue::Int(99),
            ]
        );
        let last = &t.entries[12];
        assert_eq!((last.name.as_str(), last.index), ("Quoted\"\\\n", 3));
        let first = t.entries[0].tag.as_ref().unwrap();
        assert_eq!((first.ty.as_str(), first.offset, first.size), ("IntProperty", 4, 4));
        assert_eq!(t.entries[1].tag.as_ref().unwrap().offset, 4 + 28);
        let array = t.entries[10].tag.as_ref().unwrap().array.clone();
        assert_eq!(array, Some(ArrayTag { count: 2, element: None }));
        assert_eq!(t.entries[7].tag.as_ref().unwrap().type_name.as_deref(), Some("None"));
        assert_eq!(t.entries[0].tag.as_ref().unwrap().type_name, None);
        // The walk stops after `None`; the 4 tail bytes stay.
        assert_eq!(p.exports[0].size as usize - t.end, 4);
    }

    #[test]
    fn fixed_structs_decode_and_refuse_short_bodies() {
        let mut box_raw = le(&[-3.0, -2.0, -1.0, 4.0, 5.0, 6.0]);
        box_raw.push(1);
        let cases: Vec<(&str, Vec<u8>)> = vec![
            ("Plane", le(&[7.0, 2.0, 3.0, 5.0])),
            ("Box", box_raw.clone()),
            ("Matrix", le(&[0., 0., 1., 0., 0., -1., 0., 0., 0., 0., 0., 1., 1., 11., 22., 33.])),
            ("Rotator", ints(&[1, -2, 65536])),
            ("Guid", ints(&[1, 2, 3, -4])),
            ("Color", vec![1, 2, 3, 255]),
            ("Vector2D", le(&[1.5, -2.0])),
            ("LinearColor", le(&[0.5, 0.25, 1.0, 0.0])),
            ("Quat", le(&[0.0, 0.0, 0.0, 1.0])),
        ];
        for (kind, raw) in &cases {
            let t = walk(&stream(|b| b.struct_tag("Test", kind, raw)), 4, &NoArrayTypes).unwrap();
            let PropValue::Struct { ty, fields, .. } = &t.entries[0].value else { panic!("{kind}") };
            assert_eq!(ty, kind);
            assert!(fields.iter().all(|f| f.tag.is_none()), "{kind}");
            for end in 0..raw.len() {
                let p = stream(|b| b.struct_tag("Test", kind, &raw[..end]));
                assert!(walk(&p, 4, &NoArrayTypes).is_err(), "{kind} cut at {end}");
            }
        }
        // Plane is W,X,Y,Z; Color B,G,R,A; a Box two Vectors and its validity byte.
        let p = stream(|b| {
            let mut out = b.struct_tag("Test", "Plane", &le(&[7.0, 2.0, 3.0, 5.0]));
            out.extend(b.struct_tag("Test", "Box", &box_raw));
            out.extend(b.struct_tag("Test", "Color", &[1, 2, 3, 4]));
            out
        });
        let t = walk(&p, 4, &NoArrayTypes).unwrap();
        assert_eq!(unplaced(&t.entries[0].value), floats("Plane", &[("W", 7.0), ("X", 2.0), ("Y", 3.0), ("Z", 5.0)]));
        let PropValue::Struct { fields, .. } = &t.entries[1].value else { panic!() };
        assert_eq!(unplaced(&fields[0].value), floats("Vector", &[("X", -3.0), ("Y", -2.0), ("Z", -1.0)]));
        assert_eq!((fields[2].name.as_str(), &fields[2].value), ("IsValid", &PropValue::Byte(1)));
        let PropValue::Struct { fields, .. } = &t.entries[2].value else { panic!() };
        let order: Vec<(&str, &PropValue)> = fields.iter().map(|f| (f.name.as_str(), &f.value)).collect();
        assert_eq!(
            order,
            [("B", &PropValue::Byte(1)), ("G", &PropValue::Byte(2)), ("R", &PropValue::Byte(3)), ("A", &PropValue::Byte(4))]
        );
        let mut invalid = box_raw.clone();
        invalid[24] = 2;
        let p = stream(|b| b.struct_tag("Test", "Box", &invalid));
        assert_eq!(walk(&p, 4, &NoArrayTypes), Err(PropError::InvalidBoxValidity));
    }

    #[test]
    fn arrays_follow_the_typer() {
        let mut schema = SchemaTyper::new_empty();
        schema.add_text("Test=IntProperty\nRows=StructProperty:Root\nOdd=ClassProperty\n").unwrap();
        let p = stream(|b| {
            let mut out = b.array_tag("Test", 2, &ints(&[11, -22]));
            let mut rows = Vec::new();
            for _ in 0..2 {
                rows.extend(b.int_tag("Test", -42));
                rows.extend(b.none());
            }
            out.extend(b.array_tag("Rows", 2, &rows));
            out.extend(b.array_tag("Odd", 0, &[]));
            out.extend(b.array_tag("Untyped", 0, &[]));
            out
        });
        let t = walk(&p, 4, &schema).unwrap();
        assert_eq!(t.entries[0].value, PropValue::Array(vec![PropValue::Int(11), PropValue::Int(-22)]));
        let PropValue::Array(rows) = &t.entries[1].value else { panic!() };
        assert_eq!(rows.len(), 2);
        let PropValue::Struct { ty, fields, .. } = &rows[1] else { panic!() };
        assert_eq!((ty.as_str(), &fields[0].value), ("Root", &PropValue::Int(-42)));
        // An empty array decodes whatever its type, even an unsupported one or none.
        assert_eq!(t.entries[2].value, PropValue::Array(vec![]));
        assert_eq!(
            t.entries[2].tag.as_ref().unwrap().array,
            Some(ArrayTag { count: 0, element: Some(ElementType::Other("ClassProperty".into())) })
        );
        assert_eq!(t.entries[3].tag.as_ref().unwrap().array, Some(ArrayTag { count: 0, element: None }));

        let bad = |name: &str, count: i32, body: &[u8]| {
            walk(&stream(|b| b.array_tag(name, count, body)), 4, &schema).unwrap_err()
        };
        assert_eq!(bad("Test", 3, &ints(&[11, 22])), ReadError::Truncated.into());
        assert_eq!(bad("Test", -1, &[]), PropError::InvalidArrayCount);
        assert_eq!(bad("Test", 1_000_001, &[]), PropError::InvalidArrayCount);
        assert_eq!(bad("Odd", 1, &ints(&[5])), PropError::UnsupportedElementType);
        // Fewer elements than the size covers: the size check fails.
        assert_eq!(bad("Test", 1, &ints(&[11, 22])), PropError::SizeMismatch);
    }

    #[test]
    fn the_scope_reaches_the_typer() {
        type Seen = Vec<(String, Option<i32>, Vec<String>, String, StreamRoot)>;
        struct Record(std::cell::RefCell<Seen>);
        impl ArrayTyper for Record {
            fn element(&self, scope: &ArrayScope<'_>, property: &str) -> Option<ElementType> {
                self.0
                    .borrow_mut()
                    .push((scope.owner.into(), scope.owner_ref, scope.path.to_vec(), property.into(), scope.root.clone()));
                (property == "Rows").then(|| ElementType::Struct("Row".into()))
            }
        }
        let p = stream(|b| {
            let mut inner = b.array_tag("Inner", 0, &[]);
            inner.extend(b.none());
            let mut row = b.array_tag("Deep", 0, &[]);
            row.extend(b.none());
            let mut out = b.array_tag("Outer", 0, &[]);
            out.extend(b.struct_tag("Test", "MyStruct", &inner));
            out.extend(b.array_tag("Rows", 2, &[row.clone(), row].concat()));
            out
        });
        let record = Record(Default::default());
        walk(&p, 4, &record).unwrap();
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        // Every array of one walk, nested ones included, sees the export's root.
        let export = StreamRoot::new("Root", Some(-1));
        let expected: Seen = vec![
            ("Root".into(), Some(-1), vec![], "Outer".into(), export.clone()),
            ("MyStruct".into(), None, s(&["Test"]), "Inner".into(), export.clone()),
            ("Root".into(), Some(-1), vec![], "Rows".into(), export.clone()),
            ("Row".into(), None, s(&["Rows"]), "Deep".into(), export.clone()),
            ("Row".into(), None, s(&["Rows"]), "Deep".into(), export.clone()),
        ];
        assert_eq!(record.0.into_inner(), expected);
        // A caller-given root (a struct's defaults, say) replaces the export's, for the nested arrays too: the same
        // nested scope under another root is told apart by `root`.
        let record = Record(Default::default());
        let given = StreamRoot::new("Owner", Some(7));
        read_tagged_in(&p, 1, 4, &given, &record).unwrap();
        let seen = record.0.into_inner();
        assert_eq!(seen[0], ("Owner".into(), Some(7), vec![], "Outer".into(), given.clone()));
        assert_eq!(seen[1], ("MyStruct".into(), None, s(&["Test"]), "Inner".into(), given.clone()));
        assert_eq!(seen[3].4, given);
        assert_ne!(seen[1], expected[1]);
        assert_eq!(StreamRoot::for_export(&p, 1), Ok(StreamRoot::new("Root", Some(-1))));
    }

    #[test]
    fn struct_starts_place_nested_values() {
        let p = stream(|b| {
            let mut inner = b.int_tag("Test", 81);
            inner.extend(b.struct_tag("Test", "Vector", &[0; 12]));
            inner.extend(b.none());
            b.struct_tag("Test", "Root", &inner)
        });
        let t = walk(&p, 4, &NoArrayTypes).unwrap();
        // Outer tag at 4: name, type, size, index (24 bytes) and the struct name (8), so the struct starts at 36.
        let PropValue::Struct { fields, start, .. } = &t.entries[0].value else { panic!() };
        assert_eq!(*start, 36);
        // Inside it, the Vector tag sits 28 bytes in (after the int tag), struct-relative; its value starts after
        // that tag's 24 + 8 header bytes: 36 + 28 + 32.
        assert_eq!(fields[1].tag.as_ref().unwrap().offset, 28);
        let PropValue::Struct { start: vector, .. } = &fields[1].value else { panic!() };
        assert_eq!(*vector, 36 + 28 + 32);
        let at = p.exports[0].offset as usize + vector;
        assert_eq!(&p.data[at..at + 12], &[0; 12]);
    }

    #[test]
    fn nesting_stops_past_32_and_struct_errors_name_their_struct() {
        let nest = |levels: usize| {
            package_with(|b| {
                let mut body = b.none().to_vec();
                for _ in 0..levels {
                    let mut wrapped = b.struct_tag("Test", "Root", &body);
                    wrapped.extend(b.none());
                    body = wrapped;
                }
                let mut out = ints(&[0]);
                out.extend(body);
                out
            })
        };
        assert!(walk(&nest(32), 4, &NoArrayTypes).is_ok());
        let error = walk(&nest(34), 4, &NoArrayTypes).unwrap_err();
        let mut text = error.to_string();
        let mut cause = std::error::Error::source(&error);
        while let Some(c) = cause {
            text = format!("{text}: {c}");
            cause = c.source();
        }
        assert!(text.starts_with("struct Root: struct Root: "), "{text}");
        assert!(text.ends_with("property nesting exceeds 32"), "{text}");
        // An error inside a fixed-layout struct is not wrapped.
        let p = stream(|b| b.struct_tag("Test", "Vector", &f32::NAN.to_le_bytes()));
        assert_eq!(walk(&p, 4, &NoArrayTypes), Err(PropError::NonFiniteStructFloat));
    }

    #[test]
    fn malformed_tags_are_refused() {
        let cases: Vec<(Build, PropError)> = vec![
            (Box::new(|b| b.tag("Test", "IntProperty", &[0])), ReadError::Truncated.into()),
            (Box::new(|b| b.tag("Test", "IntProperty", &[0; 8])), PropError::SizeMismatch),
            (Box::new(|b| b.tag_detail("Test", "BoolProperty", &[2], &[])), PropError::InvalidBool),
            (Box::new(|b| b.tag_detail("Test", "BoolProperty", &[1], &[0])), PropError::SizeMismatch),
            (Box::new(|b| b.object_tag("Test", 2)), ReadError::InvalidReference.into()),
            (Box::new(|b| b.tag("Test", "NameProperty", &ints(&[999, 0]))), ReadError::InvalidName.into()),
            (Box::new(|b| b.float_tag("Test", f32::NAN)), PropError::NonFiniteFloat),
            (
                Box::new(|b| b.tag("Test", "StrProperty", &[ints(&[-2]), vec![0, 0xd8, 0, 0]].concat())),
                ReadError::InvalidSurrogate.into(),
            ),
            (Box::new(|b| b.tag_full("Test", -1, "IntProperty", 4, &[], &ints(&[0]))), PropError::NegativeSizeOrIndex),
            (Box::new(|b| b.tag_full("Test", 0, "IntProperty", -1, &[], &[])), PropError::NegativeSizeOrIndex),
            (Box::new(|b| b.tag_full("Test", 0, "IntProperty", 999_999, &[], &[])), ReadError::Truncated.into()),
        ];
        for (i, (make, expected)) in cases.into_iter().enumerate() {
            let p = package_with(|b| {
                let mut out = ints(&[123]);
                out.extend(make(b));
                out.extend(b.none());
                out
            });
            assert_eq!(walk(&p, 4, &NoArrayTypes), Err(expected), "case {i}");
        }
        // A cut anywhere in a tag stream fails.
        let full = package_with(|b| {
            let mut out = ints(&[123]);
            out.extend(b.int_tag("Test", -42));
            out.extend(b.none());
            out
        });
        let at = full.exports[0].offset as usize;
        let bytes = full.data[at..at + full.exports[0].size as usize].to_vec();
        for end in 4..bytes.len() {
            let p = package_with(|_| bytes[..end].to_vec());
            assert!(walk(&p, 4, &NoArrayTypes).is_err(), "cut at {end}");
        }
    }

    #[test]
    fn range_checks_come_first() {
        let p = stream(|_| Vec::new());
        assert_eq!(walk(&p, 4, &NoArrayTypes), Ok(TaggedProps { entries: vec![], end: 12 }));
        // Starting at the payload's end is allowed (and then the read fails); one past it is refused.
        assert_eq!(walk(&p, 12, &NoArrayTypes), Err(ReadError::Truncated.into()));
        assert_eq!(walk(&p, 13, &NoArrayTypes), Err(PropError::OffsetPastExport));
        assert_eq!(read_tagged(&p, 0, 4, &NoArrayTypes), Err(PropError::NotPositiveExport));
        assert_eq!(read_tagged(&p, -1, 4, &NoArrayTypes), Err(PropError::NotPositiveExport));
        assert_eq!(read_tagged(&p, 2, 4, &NoArrayTypes), Err(ReadError::InvalidReference.into()));
        assert_eq!(PropError::OffsetPastExport.to_string(), "property offset exceeds export size");
    }
}
