//! Owner: lane W1-OBJ. Property declarations and the class, struct and enum tables with their slot layouts
//! (architecture section 3.3, contract W1-OBJ).
//!
//! **Slot layouts, not a name-keyed bag (decision D14).** The C++ VM keeps an object's properties in a map from
//! lower-cased name to value (`vm::Object::props`). Here a class's layout is its super class's slots followed by its own
//! declarations in order, so the slot of a property is fixed when the class is defined and a [`PropId`] taken from the
//! declaring class is the same slot in every subclass. Names are looked up once (the interpreter resolves its
//! instance-variable operands to `PropId`s when it links a function), not on every access. This is the project's own
//! layout: slot numbers are not the engine's memory offsets, and no native offset is used (AGENTS.md rule 4).
//!
//! **Static arrays** (`array_dim > 1`) take `array_dim` consecutive slots, each holding one element. The C++ VM holds
//! a static array as one array value in one map entry (`Runtime::zeroValue`); here an element is addressed by
//! [`PropAddr`] with its index checked against `array_dim` (an overlay patch `Prop[3].Field` and the interpreter's
//! static-array element both resolve to one). Every kind gets `array_dim` slots, a dynamic array too (the C++ ignored
//! `array_dim` on an `ArrayProperty`; the UnrealScript compiler does not make such properties, so no package is
//! expected to show the difference). Struct fields are laid out the same way inside a
//! [`StructValue`](crate::value::StructValue).
//!
//! **Order of definition.** A class's super class must be defined before it, and a struct's super struct and every
//! struct a field holds by value before that struct or class, so a layout never depends on something unfinished and a
//! by-value struct cycle cannot be built. A reference (an object, class or interface property) needs only a declared
//! class: [`ClassTable::declare`] reserves a class's id before its definition, so two classes can refer to each other.
//! The reflection lane (W2-REFLECT) defines in that order.
//!
//! **Attribute companions.** An attribute value property (`FloatAttributeProperty`, `IntAttributeProperty`,
//! `ByteAttributeProperty` with the stack flag) has a base property and a modifier-stack array in the same class
//! (NATIVE_BYTECODE_OPCODES.md, opcode 5F; NATIVE_ATTRIBUTES.md). [`AttrCompanions`] names all three; the table checks
//! them the way the C++ `companions()` (`src/interp.cpp`) does: the value side is an attribute kind with the stack
//! flag, the stack side is an array, the base side has the plain-attribute flag. Status: the property model is read
//! from the packages' flags and checked against the names over the nine code packages (the records' counts); the
//! game's use of it is UNVERIFIED in the running game.
//!
//! **Not `Send`** (decision D18): the table caches struct zero values, which hold `Rc`. See [`crate::value`].

use std::fmt;
use std::rc::Rc;

use ow_core::{FixedHashMap, IdentError, ObjectPath};
use ow_package::PackageId;

use crate::ids::{ClassId, EnumId, NameId, PropAddr, PropId, StructId};
use crate::value::Value;

/// What a property holds. Mirrors the property export classes (`IntProperty`, `ObjectProperty`, ...); a
/// `ComponentProperty` is an [`PropKind::Object`] of its component class, as the C++ VM treats it.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropKind {
    Int,
    Float,
    Bool,
    /// A byte, with the enum it is declared as (if any).
    Byte {
        enum_: Option<EnumId>,
    },
    Str,
    Name,
    /// A reference to an object of `class` or a subclass.
    Object {
        class: ClassId,
    },
    /// A reference to `meta` or a subclass of it.
    Class {
        meta: ClassId,
    },
    /// A reference to an object implementing the interface `class`.
    Interface {
        class: ClassId,
    },
    /// A struct held by value.
    Struct(StructId),
    /// A dynamic array of the inner kind.
    Array(Box<PropKind>),
    Delegate,
    FloatAttribute,
    IntAttribute,
    ByteAttribute {
        enum_: Option<EnumId>,
    },
}

impl PropKind {
    /// The kind's name, for error messages.
    pub fn kind_name(&self) -> &'static str {
        match self {
            PropKind::Int => "Int",
            PropKind::Float => "Float",
            PropKind::Bool => "Bool",
            PropKind::Byte { .. } => "Byte",
            PropKind::Str => "Str",
            PropKind::Name => "Name",
            PropKind::Object { .. } => "Object",
            PropKind::Class { .. } => "Class",
            PropKind::Interface { .. } => "Interface",
            PropKind::Struct(_) => "Struct",
            PropKind::Array(_) => "Array",
            PropKind::Delegate => "Delegate",
            PropKind::FloatAttribute => "FloatAttribute",
            PropKind::IntAttribute => "IntAttribute",
            PropKind::ByteAttribute { .. } => "ByteAttribute",
        }
    }

    /// One of the three attribute kinds.
    pub fn is_attribute(&self) -> bool {
        matches!(
            self,
            PropKind::FloatAttribute | PropKind::IntAttribute | PropKind::ByteAttribute { .. }
        )
    }
}

/// The three properties of one attribute, all declared in the same class: the value property (`X`), its base
/// (`XBaseValue`) and its modifier stack (`XModifierStack`). In the packages the value property's references list
/// the stack and the base, and the base's list the value (`typeRef`/`typeRef2`/`typeRef3` in `src/vm.hpp`; a byte
/// attribute lists its enum first); the reflection lane reads those and this table checks them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AttrCompanions {
    pub value: PropId,
    pub base: PropId,
    pub stack: PropId,
}

/// One property declaration: of a class, a struct, or (for the reflection lane) a function's parameter or local.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertyDecl {
    pub name: NameId,
    pub kind: PropKind,
    /// The property flags, low word then high word (`src/vm.cpp` `readProperty`).
    pub flags: u64,
    /// 1 for a scalar, the element count for a static array. Below 1 is refused by the table (the reader of the
    /// package turns the stored 0 into 1, as the C++ does).
    pub array_dim: i32,
    /// The attribute companions, on the value and the base property of an attribute.
    pub attr: Option<AttrCompanions>,
    /// The package and export index the declaration was read from; `None` for a synthetic one.
    pub source: Option<(PackageId, i32)>,
}

impl PropertyDecl {
    /// Optional parameter (`src/vm.hpp` `isOptional`).
    pub const OPTIONAL: u64 = 0x10;
    /// Function parameter (`isParam`).
    pub const PARAM: u64 = 0x80;
    /// `out` parameter (`isOut`).
    pub const OUT: u64 = 0x100;
    /// Return value (`isReturn`).
    pub const RETURN: u64 = 0x400;
    /// Stack attribute: the value property of an attribute triple. Bit 31 of the high flag word
    /// (NATIVE_ATTRIBUTES.md; `flags >> 63` in `src/interp.cpp`).
    pub const STACK_ATTRIBUTE: u64 = 1 << 63;
    /// Plain attribute: a base companion or a plain attribute field. Bit 30 of the high flag word.
    pub const PLAIN_ATTRIBUTE: u64 = 1 << 62;
    /// The attribute change notification runs for this property. Bit 29 of the high flag word.
    pub const NOTIFY: u64 = 1 << 61;

    /// A scalar declaration with no flags, no companions and no source.
    pub fn new(name: NameId, kind: PropKind) -> Self {
        PropertyDecl {
            name,
            kind,
            flags: 0,
            array_dim: 1,
            attr: None,
            source: None,
        }
    }

    /// The same declaration with `flags` set.
    pub fn with_flags(mut self, flags: u64) -> Self {
        self.flags = flags;
        self
    }

    /// The same declaration as a static array of `dim` elements.
    pub fn with_array_dim(mut self, dim: i32) -> Self {
        self.array_dim = dim;
        self
    }

    pub fn is_stack_attribute(&self) -> bool {
        self.flags & Self::STACK_ATTRIBUTE != 0
    }
    pub fn is_plain_attribute(&self) -> bool {
        self.flags & Self::PLAIN_ATTRIBUTE != 0
    }
    pub fn notifies(&self) -> bool {
        self.flags & Self::NOTIFY != 0
    }
    pub fn is_param(&self) -> bool {
        self.flags & Self::PARAM != 0
    }
    pub fn is_out(&self) -> bool {
        self.flags & Self::OUT != 0
    }
    pub fn is_optional(&self) -> bool {
        self.flags & Self::OPTIONAL != 0
    }
    pub fn is_return(&self) -> bool {
        self.flags & Self::RETURN != 0
    }
}

/// Why a definition or a property address was refused.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutError {
    /// The path is not a valid object path.
    BadPath {
        path: String,
        error: IdentError,
    },
    /// A class, struct or enum with this path is already defined.
    AlreadyDefined {
        path: String,
    },
    /// An id this table did not make.
    UnknownClass(ClassId),
    UnknownStruct(StructId),
    UnknownEnum(EnumId),
    /// The class is declared but not defined yet.
    NotDefined(ClassId),
    /// A super class must be defined before its subclasses.
    SuperNotDefined {
        class: String,
        super_: ClassId,
    },
    /// A super struct that is not in the table.
    UnknownSuperStruct {
        owner: String,
        super_: StructId,
    },
    /// Two declarations of one class or struct share a name.
    DuplicateName {
        owner: String,
        name: NameId,
    },
    /// `array_dim` below 1.
    BadArrayDim {
        owner: String,
        name: NameId,
        dim: i32,
    },
    /// The layout would pass `u32::MAX` slots.
    TooManySlots {
        owner: String,
    },
    /// The property is declared in a class that is not the object's class or one of its super classes.
    NotInClass {
        class: ClassId,
        prop: PropId,
    },
    /// No declaration starts at this slot of the declaring class.
    NotAProperty(PropId),
    /// The element index is at or past `array_dim`.
    IndexOutOfRange {
        prop: PropId,
        index: u32,
        dim: u32,
    },
    /// The attribute companions do not form a value / base / stack triple of this class.
    BadAttribute {
        owner: String,
        name: NameId,
        reason: &'static str,
    },
}

impl fmt::Display for LayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LayoutError::BadPath { path, error } => write!(f, "bad path {path:?}: {error}"),
            LayoutError::AlreadyDefined { path } => write!(f, "{path} is already defined"),
            LayoutError::UnknownClass(c) => write!(f, "unknown class {c:?}"),
            LayoutError::UnknownStruct(s) => write!(f, "unknown struct {s:?}"),
            LayoutError::UnknownEnum(e) => write!(f, "unknown enum {e:?}"),
            LayoutError::NotDefined(c) => write!(f, "class {c:?} is declared but not defined"),
            LayoutError::SuperNotDefined { class, super_ } => {
                write!(f, "{class}: super class {super_:?} is not defined yet")
            }
            LayoutError::UnknownSuperStruct { owner, super_ } => {
                write!(f, "{owner}: unknown super struct {super_:?}")
            }
            LayoutError::DuplicateName { owner, name } => {
                write!(f, "{owner}: two declarations of name {name:?}")
            }
            LayoutError::BadArrayDim { owner, name, dim } => {
                write!(f, "{owner}: {name:?} has array_dim {dim}, below 1")
            }
            LayoutError::TooManySlots { owner } => write!(f, "{owner}: more than u32::MAX slots"),
            LayoutError::NotInClass { class, prop } => {
                write!(
                    f,
                    "property {prop:?} is not declared in class {class:?} or its super classes"
                )
            }
            LayoutError::NotAProperty(p) => write!(f, "no property starts at {p:?}"),
            LayoutError::IndexOutOfRange { prop, index, dim } => {
                write!(
                    f,
                    "index {index} out of range for {prop:?} (array_dim {dim})"
                )
            }
            LayoutError::BadAttribute {
                owner,
                name,
                reason,
            } => {
                write!(f, "{owner}: attribute {name:?}: {reason}")
            }
        }
    }
}

impl std::error::Error for LayoutError {}

/// The layout of one class: its own declarations after its super class's slots.
#[derive(Debug, Clone)]
pub struct ClassLayout {
    path: ObjectPath,
    super_: Option<ClassId>,
    defined: bool,
    first_slot: u32,
    slot_count: u32,
    decls: Vec<PropertyDecl>,
    /// The first slot of each entry of `decls`, ascending.
    decl_slots: Vec<u32>,
    /// Own declarations by name (inherited ones are found by walking the super chain).
    by_name: FixedHashMap<NameId, usize>,
}

impl ClassLayout {
    pub fn path(&self) -> &ObjectPath {
        &self.path
    }
    pub fn super_class(&self) -> Option<ClassId> {
        self.super_
    }
    /// False for a class that is only declared.
    pub fn is_defined(&self) -> bool {
        self.defined
    }
    /// The first slot of this class's own declarations (the super class's slot count).
    pub fn first_slot(&self) -> u32 {
        self.first_slot
    }
    /// The slot count of an object of this class, inherited slots included.
    pub fn slot_count(&self) -> u32 {
        self.slot_count
    }
    /// This class's own declarations with their first slots, in slot order.
    pub fn own_decls(&self) -> impl Iterator<Item = (u32, &PropertyDecl)> {
        self.decl_slots.iter().copied().zip(&self.decls)
    }
    fn decl_at(&self, slot: u32) -> Option<usize> {
        self.decl_slots.binary_search(&slot).ok()
    }
}

/// The layout of one struct type: the super struct's fields, then its own.
#[derive(Debug, Clone)]
pub struct StructLayout {
    path: ObjectPath,
    super_: Option<StructId>,
    fields: Vec<PropertyDecl>,
    field_slots: Vec<u32>,
    slot_count: u32,
    zero: Value,
}

impl StructLayout {
    pub fn path(&self) -> &ObjectPath {
        &self.path
    }
    pub fn super_struct(&self) -> Option<StructId> {
        self.super_
    }
    /// The entry count of a value of this struct (a static-array field counts `array_dim` entries).
    pub fn slot_count(&self) -> u32 {
        self.slot_count
    }
    /// Every field, inherited ones first, with its first entry index.
    pub fn fields(&self) -> impl DoubleEndedIterator<Item = (u32, &PropertyDecl)> {
        self.field_slots.iter().copied().zip(&self.fields)
    }
    /// The field named `name` and its first entry index; a field of this struct shadows an inherited one.
    pub fn field(&self, name: NameId) -> Option<(u32, &PropertyDecl)> {
        self.fields().rev().find(|(_, d)| d.name == name)
    }
    /// The zero value: every field at its kind's zero (struct defaults from the packages are W3-DECODE's).
    pub fn zero(&self) -> &Value {
        &self.zero
    }
}

/// An enum type: its value names in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumLayout {
    path: ObjectPath,
    values: Vec<NameId>,
}

impl EnumLayout {
    pub fn path(&self) -> &ObjectPath {
        &self.path
    }
    pub fn values(&self) -> &[NameId] {
        &self.values
    }
    /// The index of value `name`.
    pub fn index_of(&self, name: NameId) -> Option<usize> {
        self.values.iter().position(|v| *v == name)
    }
}

/// Classes, structs and enums with their layouts. Paths compare like [`ObjectPath`]s (case-insensitive, `:` as `.`).
#[derive(Debug, Clone)]
pub struct ClassTable {
    classes: Vec<ClassLayout>,
    class_by_path: FixedHashMap<ObjectPath, ClassId>,
    structs: Vec<StructLayout>,
    struct_by_path: FixedHashMap<ObjectPath, StructId>,
    enums: Vec<EnumLayout>,
    enum_by_path: FixedHashMap<ObjectPath, EnumId>,
    empty_str: Rc<str>,
    empty_array: Value,
}

impl Default for ClassTable {
    fn default() -> Self {
        Self::new()
    }
}

fn parse_path(path: &str) -> Result<ObjectPath, LayoutError> {
    ObjectPath::parse(path).map_err(|error| LayoutError::BadPath {
        path: path.to_string(),
        error,
    })
}

fn next_id(len: usize) -> u32 {
    u32::try_from(len).expect("more than 2^32 table entries")
}

impl ClassTable {
    /// An empty table.
    pub fn new() -> Self {
        ClassTable {
            classes: Vec::new(),
            class_by_path: FixedHashMap::default(),
            structs: Vec::new(),
            struct_by_path: FixedHashMap::default(),
            enums: Vec::new(),
            enum_by_path: FixedHashMap::default(),
            empty_str: Rc::from(""),
            empty_array: Value::array(Vec::new()),
        }
    }

    /// The id of the class at `path`, reserving one (declared, not defined) if the path is new. A declared class can
    /// be named by object, class and interface properties before it is defined.
    pub fn declare(&mut self, path: &str) -> Result<ClassId, LayoutError> {
        let path = parse_path(path)?;
        if let Some(&id) = self.class_by_path.get(&path) {
            return Ok(id);
        }
        let id = ClassId::new(next_id(self.classes.len()));
        self.classes.push(ClassLayout {
            path: path.clone(),
            super_: None,
            defined: false,
            first_slot: 0,
            slot_count: 0,
            decls: Vec::new(),
            decl_slots: Vec::new(),
            by_name: FixedHashMap::default(),
        });
        self.class_by_path.insert(path, id);
        Ok(id)
    }

    /// Defines the class at `path` (declaring it if needed) with super class `super_` (already defined) and its own
    /// declarations in order. Checks every id the declarations name, that `array_dim` is at least 1, that no two own
    /// declarations share a name, and any attribute companions given.
    ///
    /// A declaration may share a name with an inherited one; [`ClassTable::prop`] then finds the nearer one, as a
    /// lookup that walks from the class up the super chain does. UnrealScript refuses such a redeclaration, so
    /// packages are not expected to hold one.
    pub fn define(
        &mut self,
        path: &str,
        super_: Option<ClassId>,
        props: Vec<PropertyDecl>,
    ) -> Result<ClassId, LayoutError> {
        let parsed = parse_path(path)?;
        if let Some(&id) = self.class_by_path.get(&parsed)
            && self.classes[id.index()].defined
        {
            return Err(LayoutError::AlreadyDefined {
                path: path.to_string(),
            });
        }
        let first_slot = match super_ {
            None => 0,
            Some(s) => {
                let layout = self
                    .classes
                    .get(s.index())
                    .ok_or(LayoutError::UnknownClass(s))?;
                if !layout.defined {
                    return Err(LayoutError::SuperNotDefined {
                        class: path.to_string(),
                        super_: s,
                    });
                }
                layout.slot_count
            }
        };
        // Declare first so a property can name its own class (`var Foo Next;`).
        let id = self.declare(path)?;
        let (decl_slots, slot_count, by_name) = self.lay_out(path, first_slot, &props)?;
        let layout = &mut self.classes[id.index()];
        layout.super_ = super_;
        layout.defined = true;
        layout.first_slot = first_slot;
        layout.slot_count = slot_count;
        layout.decls = props;
        layout.decl_slots = decl_slots;
        layout.by_name = by_name;
        let companions: Vec<(NameId, AttrCompanions)> = self.classes[id.index()]
            .decls
            .iter()
            .filter_map(|d| d.attr.map(|a| (d.name, a)))
            .collect();
        for (name, attr) in companions {
            if let Err(e) = self.check_companions(id, name, &attr) {
                // Leave the class declared but undefined, so a corrected definition can follow.
                let layout = &mut self.classes[id.index()];
                layout.defined = false;
                layout.decls.clear();
                layout.decl_slots.clear();
                layout.by_name = FixedHashMap::default();
                layout.slot_count = 0;
                layout.first_slot = 0;
                layout.super_ = None;
                return Err(e);
            }
        }
        Ok(id)
    }

    /// Slots and name index for `props` starting at `first`, after checking each declaration.
    #[allow(clippy::type_complexity)]
    fn lay_out(
        &self,
        owner: &str,
        first: u32,
        props: &[PropertyDecl],
    ) -> Result<(Vec<u32>, u32, FixedHashMap<NameId, usize>), LayoutError> {
        let mut slots = Vec::with_capacity(props.len());
        let mut by_name = FixedHashMap::default();
        let mut cursor = first;
        for (i, decl) in props.iter().enumerate() {
            self.check_kind(&decl.kind)?;
            let dim = u32::try_from(decl.array_dim)
                .ok()
                .filter(|d| *d >= 1)
                .ok_or_else(|| LayoutError::BadArrayDim {
                    owner: owner.to_string(),
                    name: decl.name,
                    dim: decl.array_dim,
                })?;
            if by_name.insert(decl.name, i).is_some() {
                return Err(LayoutError::DuplicateName {
                    owner: owner.to_string(),
                    name: decl.name,
                });
            }
            slots.push(cursor);
            cursor = cursor
                .checked_add(dim)
                .ok_or_else(|| LayoutError::TooManySlots {
                    owner: owner.to_string(),
                })?;
        }
        Ok((slots, cursor, by_name))
    }

    /// Every id a kind names must be in the table; a struct held by value is always defined (structs are defined in
    /// one step).
    fn check_kind(&self, kind: &PropKind) -> Result<(), LayoutError> {
        match kind {
            PropKind::Object { class }
            | PropKind::Interface { class }
            | PropKind::Class { meta: class } => {
                if class.index() >= self.classes.len() {
                    return Err(LayoutError::UnknownClass(*class));
                }
            }
            PropKind::Struct(s) => {
                if s.index() >= self.structs.len() {
                    return Err(LayoutError::UnknownStruct(*s));
                }
            }
            PropKind::Byte { enum_: Some(e) } | PropKind::ByteAttribute { enum_: Some(e) } => {
                if e.index() >= self.enums.len() {
                    return Err(LayoutError::UnknownEnum(*e));
                }
            }
            PropKind::Array(inner) => self.check_kind(inner)?,
            _ => {}
        }
        Ok(())
    }

    /// The check of the C++ `companions()` (`src/interp.cpp`): all three are declarations of `class`; the value side
    /// is an attribute kind with the stack flag; the stack side is a dynamic array; the base side has the
    /// plain-attribute flag; and the declaration `name` carrying them is the value or the base.
    fn check_companions(
        &self,
        class: ClassId,
        name: NameId,
        a: &AttrCompanions,
    ) -> Result<(), LayoutError> {
        let bad = |reason| LayoutError::BadAttribute {
            owner: self.classes[class.index()].path.as_str().to_string(),
            name,
            reason,
        };
        if a.value.declared_in != class
            || a.base.declared_in != class
            || a.stack.declared_in != class
        {
            return Err(bad("companions are not declared in the same class"));
        }
        let decl = |p: PropId| {
            self.decl(p)
                .ok_or_else(|| bad("a companion is not a property of the class"))
        };
        let (value, base, stack) = (decl(a.value)?, decl(a.base)?, decl(a.stack)?);
        if !value.kind.is_attribute() || !value.is_stack_attribute() {
            return Err(bad(
                "the value side is not an attribute property with the stack flag",
            ));
        }
        if !matches!(stack.kind, PropKind::Array(_)) {
            return Err(bad("the stack side is not an array property"));
        }
        if !base.is_plain_attribute() {
            return Err(bad("the base side has no plain-attribute flag"));
        }
        if value.name != name && base.name != name {
            return Err(bad(
                "the declaration is neither the value nor the base of its companions",
            ));
        }
        Ok(())
    }

    /// Links the attribute whose value, base and stack properties are the own declarations `value`, `base` and
    /// `stack` of `class`: checks them as [`ClassTable::define`] does and sets [`PropertyDecl::attr`] on the value
    /// and the base declaration. This is how the reflection lane links what it reads from the property references.
    pub fn link_attribute(
        &mut self,
        class: ClassId,
        value: NameId,
        base: NameId,
        stack: NameId,
    ) -> Result<AttrCompanions, LayoutError> {
        let layout = self.layout_checked(class)?;
        let own = |n: NameId| {
            layout.by_name.get(&n).map(|&i| PropId {
                slot: layout.decl_slots[i],
                declared_in: class,
            })
        };
        let missing = |n: NameId| LayoutError::BadAttribute {
            owner: layout.path.as_str().to_string(),
            name: n,
            reason: "no own declaration of this name",
        };
        let companions = AttrCompanions {
            value: own(value).ok_or_else(|| missing(value))?,
            base: own(base).ok_or_else(|| missing(base))?,
            stack: own(stack).ok_or_else(|| missing(stack))?,
        };
        self.check_companions(class, value, &companions)?;
        let layout = &mut self.classes[class.index()];
        for n in [value, base] {
            let i = layout.by_name[&n];
            layout.decls[i].attr = Some(companions);
        }
        Ok(companions)
    }

    /// Defines the struct at `path` with super struct `super_` and its own fields in order. Every struct a field holds
    /// by value is already in the table, so struct values cannot contain themselves.
    pub fn define_struct(
        &mut self,
        path: &str,
        super_: Option<StructId>,
        fields: Vec<PropertyDecl>,
    ) -> Result<StructId, LayoutError> {
        let parsed = parse_path(path)?;
        if self.struct_by_path.contains_key(&parsed) {
            return Err(LayoutError::AlreadyDefined {
                path: path.to_string(),
            });
        }
        let (mut all, first) = match super_ {
            None => (Vec::new(), 0),
            Some(s) => {
                let parent =
                    self.structs
                        .get(s.index())
                        .ok_or_else(|| LayoutError::UnknownSuperStruct {
                            owner: path.to_string(),
                            super_: s,
                        })?;
                (parent.fields.clone(), parent.slot_count)
            }
        };
        let (own_slots, slot_count, _) = self.lay_out(path, first, &fields)?;
        let id = StructId::new(next_id(self.structs.len()));
        let mut field_slots = match super_ {
            None => Vec::new(),
            Some(s) => self.structs[s.index()].field_slots.clone(),
        };
        field_slots.extend(own_slots);
        all.extend(fields);
        let mut zeros = Vec::with_capacity(slot_count as usize);
        for decl in &all {
            let zero = self.zero_value(&decl.kind);
            for _ in 0..decl.array_dim {
                zeros.push(zero.clone());
            }
        }
        self.structs.push(StructLayout {
            path: parsed.clone(),
            super_,
            fields: all,
            field_slots,
            slot_count,
            zero: Value::structure(id, zeros),
        });
        self.struct_by_path.insert(parsed, id);
        Ok(id)
    }

    /// Defines the enum at `path` with its value names in order.
    pub fn define_enum(&mut self, path: &str, values: Vec<NameId>) -> Result<EnumId, LayoutError> {
        let parsed = parse_path(path)?;
        if self.enum_by_path.contains_key(&parsed) {
            return Err(LayoutError::AlreadyDefined {
                path: path.to_string(),
            });
        }
        let id = EnumId::new(next_id(self.enums.len()));
        self.enums.push(EnumLayout {
            path: parsed.clone(),
            values,
        });
        self.enum_by_path.insert(parsed, id);
        Ok(id)
    }

    /// The layout of `c`.
    ///
    /// # Panics
    /// If `c` was not made by this table.
    pub fn layout(&self, c: ClassId) -> &ClassLayout {
        &self.classes[c.index()]
    }

    /// The layout of `c`, or `None` for an id this table did not make.
    pub fn get_layout(&self, c: ClassId) -> Option<&ClassLayout> {
        self.classes.get(c.index())
    }

    fn layout_checked(&self, c: ClassId) -> Result<&ClassLayout, LayoutError> {
        let layout = self
            .classes
            .get(c.index())
            .ok_or(LayoutError::UnknownClass(c))?;
        if !layout.defined {
            return Err(LayoutError::NotDefined(c));
        }
        Ok(layout)
    }

    /// The layout of struct `s`, or `None` for an id this table did not make.
    pub fn struct_layout(&self, s: StructId) -> Option<&StructLayout> {
        self.structs.get(s.index())
    }

    /// The layout of enum `e`, or `None` for an id this table did not make.
    pub fn enum_layout(&self, e: EnumId) -> Option<&EnumLayout> {
        self.enums.get(e.index())
    }

    /// Whether `c` is `base` or a subclass of it (the C++ `Class::isChildOf`). False for ids this table did not make.
    pub fn is_child_of(&self, c: ClassId, base: ClassId) -> bool {
        let mut cursor = Some(c);
        while let Some(x) = cursor {
            if x == base {
                return true;
            }
            cursor = self.classes.get(x.index()).and_then(|l| l.super_);
        }
        false
    }

    /// The property `name` of class `c`, its own or inherited, the nearest declaration first.
    pub fn prop(&self, c: ClassId, name: NameId) -> Option<PropId> {
        let mut cursor = Some(c);
        while let Some(x) = cursor {
            let layout = self.classes.get(x.index())?;
            if let Some(&i) = layout.by_name.get(&name) {
                return Some(PropId {
                    slot: layout.decl_slots[i],
                    declared_in: x,
                });
            }
            cursor = layout.super_;
        }
        None
    }

    /// The declaration a `PropId` names, or `None` if no declaration of its class starts at its slot.
    pub fn decl(&self, p: PropId) -> Option<&PropertyDecl> {
        let layout = self.classes.get(p.declared_in.index())?;
        layout.decl_at(p.slot).map(|i| &layout.decls[i])
    }

    /// The slot of element `a` in an object of class `c`, after checking that `c` is the declaring class or a subclass
    /// of it, that a declaration starts at the slot, and that the index is below `array_dim`.
    pub fn resolve(&self, c: ClassId, a: PropAddr) -> Result<usize, LayoutError> {
        self.layout_checked(c)?;
        if !self.is_child_of(c, a.prop.declared_in) {
            return Err(LayoutError::NotInClass {
                class: c,
                prop: a.prop,
            });
        }
        let decl = self.decl(a.prop).ok_or(LayoutError::NotAProperty(a.prop))?;
        // array_dim was checked to be at least 1 when the class was defined.
        let dim = u32::try_from(decl.array_dim).unwrap_or(1);
        if a.index >= dim {
            return Err(LayoutError::IndexOutOfRange {
                prop: a.prop,
                index: a.index,
                dim,
            });
        }
        Ok((a.prop.slot + a.index) as usize)
    }

    /// Every property of `c`, inherited ones first, in slot order.
    pub fn props(&self, c: ClassId) -> Result<Vec<(PropId, &PropertyDecl)>, LayoutError> {
        self.layout_checked(c)?;
        let mut chain = Vec::new();
        let mut cursor = Some(c);
        while let Some(x) = cursor {
            chain.push(x);
            cursor = self.classes[x.index()].super_;
        }
        let mut out = Vec::new();
        for &x in chain.iter().rev() {
            let layout = &self.classes[x.index()];
            out.extend(layout.own_decls().map(|(slot, d)| {
                (
                    PropId {
                        slot,
                        declared_in: x,
                    },
                    d,
                )
            }));
        }
        Ok(out)
    }

    /// The zero value of a kind, as the C++ `Runtime::zeroValue`: 0, `false`, `""`, the name `None`, null references,
    /// an empty array, an unbound delegate, and for a struct its layout's zero value.
    ///
    /// # Panics
    /// If `kind` is a struct whose id this table did not make (kinds of defined declarations are checked).
    pub fn zero_value(&self, kind: &PropKind) -> Value {
        match kind {
            PropKind::Int | PropKind::IntAttribute => Value::Int(0),
            PropKind::Float | PropKind::FloatAttribute => Value::Float(0.0),
            PropKind::Bool => Value::Bool(false),
            PropKind::Byte { .. } | PropKind::ByteAttribute { .. } => Value::Byte(0),
            PropKind::Str => Value::Str(self.empty_str.clone()),
            PropKind::Name => Value::Name(NameId::NONE),
            PropKind::Object { .. } => Value::Object(None),
            PropKind::Class { .. } => Value::Class(None),
            PropKind::Interface { .. } => Value::Interface(None),
            PropKind::Struct(s) => self.structs[s.index()].zero.clone(),
            PropKind::Array(_) => self.empty_array.clone(),
            PropKind::Delegate => Value::Delegate(None),
        }
    }

    /// The zero slots of an object of class `c`.
    pub fn zero_slots(&self, c: ClassId) -> Result<Vec<Value>, LayoutError> {
        let props = self.props(c)?;
        let mut slots = Vec::with_capacity(self.classes[c.index()].slot_count as usize);
        for (_, decl) in props {
            let zero = self.zero_value(&decl.kind);
            for _ in 0..decl.array_dim {
                slots.push(zero.clone());
            }
        }
        Ok(slots)
    }

    /// The class at `path`, declared or defined.
    pub fn by_path(&self, path: &str) -> Option<ClassId> {
        self.class_by_path
            .get(&ObjectPath::parse(path).ok()?)
            .copied()
    }

    /// The struct at `path`.
    pub fn struct_by_path(&self, path: &str) -> Option<StructId> {
        self.struct_by_path
            .get(&ObjectPath::parse(path).ok()?)
            .copied()
    }

    /// The enum at `path`.
    pub fn enum_by_path(&self, path: &str) -> Option<EnumId> {
        self.enum_by_path
            .get(&ObjectPath::parse(path).ok()?)
            .copied()
    }

    /// How many classes are declared (defined or not).
    pub fn len(&self) -> usize {
        self.classes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.classes.is_empty()
    }
}

crate::value::assert_not_send!(ClassTable);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::names::NameTable;

    struct Fixture {
        names: NameTable,
        classes: ClassTable,
    }

    impl Fixture {
        fn new() -> Self {
            Fixture {
                names: NameTable::new(),
                classes: ClassTable::new(),
            }
        }
        fn n(&mut self, s: &str) -> NameId {
            self.names.intern(s)
        }
        fn decl(&mut self, s: &str, kind: PropKind) -> PropertyDecl {
            PropertyDecl::new(self.n(s), kind)
        }
    }

    /// Test.Base { int A; float B[3]; }  Test.Child extends Base { bool C; }  Test.Other { int A; }
    fn chain() -> (Fixture, ClassId, ClassId, ClassId) {
        let mut f = Fixture::new();
        let a = f.decl("A", PropKind::Int);
        let b = f.decl("B", PropKind::Float).with_array_dim(3);
        let base = f.classes.define("Test.Base", None, vec![a, b]).unwrap();
        let c = f.decl("C", PropKind::Bool);
        let child = f.classes.define("Test.Child", Some(base), vec![c]).unwrap();
        let a2 = f.decl("A", PropKind::Int);
        let other = f.classes.define("Test.Other", None, vec![a2]).unwrap();
        (f, base, child, other)
    }

    #[test]
    fn a_subclass_lays_out_after_its_super_class() {
        let (mut f, base, child, _) = chain();
        assert_eq!(f.classes.layout(base).slot_count(), 4, "A + B[3]");
        assert_eq!(f.classes.layout(child).first_slot(), 4);
        assert_eq!(f.classes.layout(child).slot_count(), 5);
        let a = f.n("A");
        let c = f.n("C");
        let pa = f.classes.prop(base, a).unwrap();
        assert_eq!(
            pa,
            PropId {
                slot: 0,
                declared_in: base
            }
        );
        // The parent's PropId is the one found from the child.
        assert_eq!(f.classes.prop(child, a), Some(pa));
        assert_eq!(f.classes.prop(child, c).unwrap().slot, 4);
        assert_eq!(f.classes.prop(base, c), None);
        let all: Vec<_> = f
            .classes
            .props(child)
            .unwrap()
            .into_iter()
            .map(|(p, _)| p.slot)
            .collect();
        assert_eq!(all, [0, 1, 4]);
    }

    #[test]
    fn a_parent_prop_resolves_in_a_child_and_not_on_an_unrelated_class() {
        let (mut f, base, child, other) = chain();
        let a = f.n("A");
        let pa = f.classes.prop(base, a).unwrap();
        assert_eq!(f.classes.resolve(child, pa.into()), Ok(0));
        assert_eq!(f.classes.resolve(base, pa.into()), Ok(0));
        assert_eq!(
            f.classes.resolve(other, pa.into()),
            Err(LayoutError::NotInClass {
                class: other,
                prop: pa
            })
        );
        // A child's own property is not valid on its parent.
        let c = f.n("C");
        let pc = f.classes.prop(child, c).unwrap();
        assert!(matches!(
            f.classes.resolve(base, pc.into()),
            Err(LayoutError::NotInClass { .. })
        ));
        // A slot inside a static array is not the start of a property.
        let inside = PropId {
            slot: 2,
            declared_in: base,
        };
        assert_eq!(
            f.classes.resolve(child, inside.into()),
            Err(LayoutError::NotAProperty(inside))
        );
        assert!(f.classes.is_child_of(child, base));
        assert!(!f.classes.is_child_of(base, child));
        assert!(!f.classes.is_child_of(other, base));
    }

    #[test]
    fn static_array_elements_resolve_by_index_and_refuse_past_the_end() {
        let (mut f, base, child, _) = chain();
        let b = f.n("B");
        let pb = f.classes.prop(base, b).unwrap();
        assert_eq!(pb.slot, 1);
        for i in 0..3 {
            assert_eq!(
                f.classes.resolve(child, PropAddr::element(pb, i)),
                Ok(1 + i as usize)
            );
        }
        assert_eq!(
            f.classes.resolve(child, PropAddr::element(pb, 3)),
            Err(LayoutError::IndexOutOfRange {
                prop: pb,
                index: 3,
                dim: 3
            })
        );
        let a = f.n("A");
        let pa = f.classes.prop(base, a).unwrap();
        assert!(matches!(
            f.classes.resolve(base, PropAddr::element(pa, 1)),
            Err(LayoutError::IndexOutOfRange {
                index: 1,
                dim: 1,
                ..
            })
        ));
    }

    #[test]
    fn definitions_are_checked() {
        let mut f = Fixture::new();
        let x = f.decl("X", PropKind::Int);
        let x2 = f.decl("x", PropKind::Float);
        assert!(matches!(
            f.classes.define("T.Dup", None, vec![x.clone(), x2]),
            Err(LayoutError::DuplicateName { .. })
        ));
        let zero = f.decl("Z", PropKind::Int).with_array_dim(0);
        assert!(matches!(
            f.classes.define("T.Dim", None, vec![zero]),
            Err(LayoutError::BadArrayDim { dim: 0, .. })
        ));
        let ok = f.classes.define("T.Ok", None, vec![x.clone()]).unwrap();
        assert!(matches!(
            f.classes.define("t.OK", None, vec![]),
            Err(LayoutError::AlreadyDefined { .. })
        ));
        let declared = f.classes.declare("T.Later").unwrap();
        assert!(matches!(
            f.classes.define("T.Sub", Some(declared), vec![]),
            Err(LayoutError::SuperNotDefined { .. })
        ));
        let bad_ref = f.decl(
            "R",
            PropKind::Object {
                class: ClassId::new(99),
            },
        );
        assert_eq!(
            f.classes.define("T.Ref", None, vec![bad_ref]),
            Err(LayoutError::UnknownClass(ClassId::new(99)))
        );
        let bad_struct = f.decl(
            "S",
            PropKind::Array(Box::new(PropKind::Struct(StructId::new(4)))),
        );
        assert_eq!(
            f.classes.define("T.S", None, vec![bad_struct]),
            Err(LayoutError::UnknownStruct(StructId::new(4)))
        );
        assert!(matches!(
            f.classes.define("", None, vec![]),
            Err(LayoutError::BadPath { .. })
        ));
        assert_eq!(f.classes.by_path("T.OK"), Some(ok));
        assert_eq!(
            f.classes.resolve(
                declared,
                PropAddr::scalar(PropId {
                    slot: 0,
                    declared_in: ok
                })
            ),
            Err(LayoutError::NotDefined(declared))
        );
    }

    #[test]
    fn declared_classes_allow_mutual_references() {
        let mut f = Fixture::new();
        let b = f.classes.declare("T.B").unwrap();
        let to_b = f.decl("ToB", PropKind::Object { class: b });
        let a = f.classes.define("T.A", None, vec![to_b]).unwrap();
        let to_a = f.decl("ToA", PropKind::Object { class: a });
        let me = f.decl("Me", PropKind::Class { meta: b });
        assert_eq!(f.classes.define("T.B", None, vec![to_a, me]), Ok(b));
        assert!(f.classes.layout(b).is_defined());
        assert_eq!(f.classes.len(), 2);
        // A class may name itself.
        let next = f.decl(
            "Next",
            PropKind::Object {
                class: ClassId::new(2),
            },
        );
        assert_eq!(
            f.classes.define("T.Node", None, vec![next]),
            Ok(ClassId::new(2))
        );
    }

    #[test]
    fn structs_inherit_fields_and_have_zero_values() {
        let mut f = Fixture::new();
        let x = f.decl("X", PropKind::Float);
        let y = f.decl("Y", PropKind::Float);
        let v2 = f
            .classes
            .define_struct("Core.Object.Vector2", None, vec![x, y])
            .unwrap();
        let z = f.decl("Z", PropKind::Int).with_array_dim(2);
        let v3 = f
            .classes
            .define_struct("Core.Object.Vector3", Some(v2), vec![z])
            .unwrap();
        let zn = f.n("Z");
        let xn = f.n("X");
        let l = f.classes.struct_layout(v3).unwrap();
        assert_eq!(l.slot_count(), 4);
        assert_eq!(l.field(zn).map(|(s, _)| s), Some(2));
        assert_eq!(l.field(xn).map(|(s, _)| s), Some(0));
        assert_eq!(
            l.zero(),
            &Value::structure(
                v3,
                vec![
                    Value::Float(0.0),
                    Value::Float(0.0),
                    Value::Int(0),
                    Value::Int(0)
                ]
            )
        );
        let holder = f.decl("V", PropKind::Struct(v3));
        let s = f.decl("S", PropKind::Str);
        let n = f.decl("N", PropKind::Name);
        let a = f.decl("L", PropKind::Array(Box::new(PropKind::Int)));
        let d = f.decl("D", PropKind::Delegate);
        let c = f
            .classes
            .define("T.Holder", None, vec![holder, s, n, a, d])
            .unwrap();
        let zeros = f.classes.zero_slots(c).unwrap();
        assert_eq!(
            zeros[0],
            f.classes.struct_layout(v3).unwrap().zero().clone()
        );
        assert_eq!(zeros[1], Value::string(""));
        assert_eq!(zeros[2], Value::Name(NameId::NONE));
        assert_eq!(zeros[3], Value::array(vec![]));
        assert_eq!(zeros[4], Value::Delegate(None));
        assert_eq!(f.classes.struct_by_path("core.object.vector3"), Some(v3));
        assert!(matches!(
            f.classes.define_struct("Core.Object.Vector2", None, vec![]),
            Err(LayoutError::AlreadyDefined { .. })
        ));
        assert!(matches!(
            f.classes
                .define_struct("T.Bad", Some(StructId::new(9)), vec![]),
            Err(LayoutError::UnknownSuperStruct { .. })
        ));
    }

    #[test]
    fn enums() {
        let mut f = Fixture::new();
        let vals = vec![f.n("MT_Scale"), f.n("MT_PreAdd"), f.n("MT_PostAdd")];
        let e = f.classes.define_enum("T.EModifierType", vals).unwrap();
        let pre = f.n("mt_preadd");
        assert_eq!(f.classes.enum_layout(e).unwrap().index_of(pre), Some(1));
        assert_eq!(f.classes.enum_by_path("T.EModifierType"), Some(e));
        let byte = f.decl("Kind", PropKind::Byte { enum_: Some(e) });
        assert!(f.classes.define("T.Uses", None, vec![byte]).is_ok());
        let bad = f.decl(
            "Kind",
            PropKind::Byte {
                enum_: Some(EnumId::new(5)),
            },
        );
        assert_eq!(
            f.classes.define("T.Bad", None, vec![bad]),
            Err(LayoutError::UnknownEnum(EnumId::new(5)))
        );
    }

    fn attribute_class(f: &mut Fixture) -> (Vec<PropertyDecl>, NameId, NameId, NameId) {
        let value = f
            .decl("MaxValue", PropKind::FloatAttribute)
            .with_flags(PropertyDecl::STACK_ATTRIBUTE);
        let base = f
            .decl("MaxValueBaseValue", PropKind::FloatAttribute)
            .with_flags(PropertyDecl::PLAIN_ATTRIBUTE);
        let stack = f.decl(
            "MaxValueModifierStack",
            PropKind::Array(Box::new(PropKind::Int)),
        );
        let names = (value.name, base.name, stack.name);
        (vec![value, base, stack], names.0, names.1, names.2)
    }

    #[test]
    fn attribute_companions_link_and_are_checked() {
        let mut f = Fixture::new();
        let (decls, v, b, s) = attribute_class(&mut f);
        let c = f.classes.define("T.Pool", None, decls).unwrap();
        let links = f.classes.link_attribute(c, v, b, s).unwrap();
        assert_eq!(links.value.slot, 0);
        assert_eq!(links.base.slot, 1);
        assert_eq!(links.stack.slot, 2);
        assert_eq!(f.classes.decl(links.value).unwrap().attr, Some(links));
        assert_eq!(f.classes.decl(links.base).unwrap().attr, Some(links));
        assert_eq!(f.classes.decl(links.stack).unwrap().attr, None);
        assert!(f.classes.decl(links.value).unwrap().is_stack_attribute());
        // Swapped roles are refused, as the C++ companions() refuses them.
        assert!(matches!(
            f.classes.link_attribute(c, b, v, s),
            Err(LayoutError::BadAttribute { .. })
        ));
        assert!(matches!(
            f.classes.link_attribute(c, v, s, b),
            Err(LayoutError::BadAttribute { .. })
        ));
        let missing = f.n("Nope");
        assert!(matches!(
            f.classes.link_attribute(c, v, b, missing),
            Err(LayoutError::BadAttribute { .. })
        ));
    }

    #[test]
    fn companions_given_at_definition_are_checked() {
        let mut f = Fixture::new();
        let (mut decls, ..) = attribute_class(&mut f);
        let id = ClassId::new(0);
        let p = |slot| PropId {
            slot,
            declared_in: id,
        };
        let good = AttrCompanions {
            value: p(0),
            base: p(1),
            stack: p(2),
        };
        decls[0].attr = Some(good);
        decls[1].attr = Some(good);
        assert_eq!(f.classes.define("T.Pool", None, decls.clone()), Ok(id));

        let mut f = Fixture::new();
        let (mut decls, ..) = attribute_class(&mut f);
        decls[0].attr = Some(AttrCompanions {
            value: p(0),
            base: p(2),
            stack: p(1),
        });
        assert!(matches!(
            f.classes.define("T.Pool", None, decls.clone()),
            Err(LayoutError::BadAttribute { .. })
        ));
        // The refused class stays declared, and a corrected definition is accepted.
        assert!(!f.classes.layout(id).is_defined());
        decls[0].attr = Some(good);
        assert_eq!(f.classes.define("T.Pool", None, decls), Ok(id));
    }

    #[test]
    fn flag_bits() {
        let mut f = Fixture::new();
        let d = f.decl("P", PropKind::Int).with_flags(
            PropertyDecl::PARAM | PropertyDecl::OUT | PropertyDecl::OPTIONAL | PropertyDecl::NOTIFY,
        );
        assert!(d.is_param() && d.is_out() && d.is_optional() && d.notifies());
        assert!(!d.is_return() && !d.is_stack_attribute() && !d.is_plain_attribute());
        assert_eq!(
            PropertyDecl::STACK_ATTRIBUTE >> 32,
            1 << 31,
            "bit 31 of the high word"
        );
        assert_eq!(PropertyDecl::PLAIN_ATTRIBUTE >> 32, 1 << 30);
        assert_eq!(PropertyDecl::NOTIFY >> 32, 1 << 29);
    }
}
