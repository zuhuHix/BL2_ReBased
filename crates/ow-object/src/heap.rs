//! Owner: lane W1-OBJ. The object heap: an arena of objects addressed by generational handles (architecture
//! section 3.3, contract W1-OBJ), and [`ObjectSpace`], the heap with its class and name tables.
//!
//! **Arena, not shared pointers.** The C++ VM holds objects in `std::shared_ptr`, so an object lives as long as anything
//! refers to it. Here an object lives in a heap slot until it is freed, referenced or not, and an [`ObjRef`] names the
//! slot and its generation. Life has three steps:
//!
//! 1. [`Heap::alloc`] creates the object with every slot at its kind's default value (zero, struct defaults applied:
//!    [`ClassTable::default_slots`]), after checking those values as a write would ([`Heap::check_value`]), so a
//!    struct default that names a freed object or an object of the wrong class is refused, never copied into the new
//!    object.
//! 2. [`Heap::destroy`] marks it destroyed (UE3's pending kill, the C++ `Object::destroyed`): it stays readable and
//!    writable, so script that runs after `Destroy()` still works, but [`Heap::live`] no longer yields it.
//! 3. [`Heap::free`] is the garbage-collection step. It removes the object and, in the same call, clears every
//!    reference to it that the heap holds: object and interface references become null (`Object(None)`,
//!    `Interface(None)`), a delegate bound to it becomes unbound (`Delegate(None)`), inside arrays and structs too, and
//!    an object whose outer it was gets no outer. The slot's generation goes up, so a handle kept outside the heap (a VM
//!    local, a frame: clearing those is the VM's job, W2-VMAPI) is refused from then on ([`HeapError::Stale`]), also
//!    after the slot holds a new object. A slot whose generation would wrap is retired instead of reused, so an old
//!    handle can never become valid again. Each call walks every object in the heap once, so freeing many objects
//!    (a tick's dead projectiles) should go through [`Heap::free_many`], which clears a whole set in one walk.
//!
//! The heap does not reach the class table, where struct defaults live: [`ObjectSpace::free`] and
//! [`ObjectSpace::free_many`] free in the heap **and** clear the dead handles out of every struct default, so a later
//! object never starts with one. Freeing through [`Heap::free`] alone leaves such a default stale, and the next
//! [`Heap::alloc`] that would copy it fails with [`HeapError::Stale`].
//!
//! Freeing an object that is another object's outer leaves that inner object with no outer. UE3 keeps an outer alive
//! while its inner objects refer to it, or collects them together; which of the two the world does, and whether this
//! case arises, is for the lanes that call `free` (UNVERIFIED: not observed in the game).
//!
//! UE3 clears references to an object when its garbage collector removes it, not when `Destroy()` runs; how long after
//! `Destroy()` that happens in BL2, and whether a delegate keeps its function name with a null object, is UNVERIFIED.
//! Who frees what and when (the world after a tick, a collector for non-actor objects) is for the lanes above.
//! Decision file 2026-11-15-02, "Object model: slot layouts in an arena heap, not Send".
//!
//! **Live objects.** [`Heap::live`] yields the instances ([`Origin::Runtime`] and [`Origin::Export`]) of a class and its
//! subclasses in creation order, skipping destroyed ones. Class default objects and resource stand-ins are not live
//! objects, as in the C++ VM (`Runtime::liveObjects`, which only tracks what `instantiate` made).
//!
//! **Checked access.** [`Heap::read`], [`Heap::write`] and [`Heap::slot_mut`] check the address against the object's
//! class layout (the declaring class is the object's class or a super class, the index is below `array_dim`), and
//! `write` checks the value against the declaration ([`Heap::check_value`]): the variant matches the kind, an object
//! reference and a delegate's receiver name objects that are not freed, an object reference one of the declared class
//! or a subclass, a class reference a class of the table that is the declared metaclass or a subclass, a struct has the
//! declared type and its layout's entry count, and arrays and struct fields are checked element by element. An
//! interface reference is checked only for not being freed: which classes implement an interface comes from the class
//! bodies (W2-REFLECT). [`Heap::write_in`] changes one array element or struct field inside a property and checks only
//! the new value, so `Arr[i] = x` and `Location.X = 1` do not recheck the whole aggregate. [`Heap::slot_mut`] hands out
//! the slot unchecked for the value; [`Object::slots`] is public for code that has resolved its slots already. Writing
//! through either skips the value checks.
//!
//! **Not `Send`** (decision D18): see [`crate::value`].

use std::fmt;
use std::rc::Rc;

use ow_package::PackageId;

use crate::ids::{ClassId, NameId, ObjRef, PropAddr, PropId, StructId};
use crate::layout::{ClassTable, LayoutError, PropKind};
use crate::names::NameTable;
use crate::value::{DelegateValue, Value, clear_refs};
use ow_core::FixedHashSet;

/// Where an object came from.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Origin {
    /// Made at run time (spawned, `new`ed, or by a test).
    Runtime,
    /// Instantiated from a package export.
    Export { package: PackageId, index: i32 },
    /// The class default object of `class`.
    Default { class: ClassId },
    /// A reference to an exported asset that was not instantiated (textures, meshes, archetypes...), the C++
    /// `Object::resourcePackage` / `resourceIndex`.
    Resource { package: PackageId, index: i32 },
}

impl Origin {
    /// An instance, which [`Heap::live`] yields: made at run time or instantiated from an export. Class default objects
    /// and resource stand-ins are not.
    pub fn is_instance(self) -> bool {
        matches!(self, Origin::Runtime | Origin::Export { .. })
    }
}

/// One object: its class, name, outer, property slots (in the class layout's order), current state and origin.
/// The heap makes objects; [`Object::new`] is for code that builds one to compare with (the struct may gain fields).
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct Object {
    pub class: ClassId,
    pub name: NameId,
    pub outer: Option<ObjRef>,
    pub slots: Vec<Value>,
    /// The current state; `None` is the class's default state.
    pub state: Option<NameId>,
    /// Destroyed (pending kill): still readable, no longer yielded by [`Heap::live`].
    pub destroyed: bool,
    pub origin: Origin,
}

impl Object {
    /// An object in the default state, not destroyed.
    pub fn new(
        class: ClassId,
        name: NameId,
        outer: Option<ObjRef>,
        slots: Vec<Value>,
        origin: Origin,
    ) -> Self {
        Object {
            class,
            name,
            outer,
            slots,
            state: None,
            destroyed: false,
            origin,
        }
    }
}

/// One step into a property's value for [`Heap::write_in`].
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Element `i` of a dynamic array (it must exist: growing the array is the caller's).
    Element(u32),
    /// Entry `e` of a struct, as [`StructLayout::entry_field`](crate::layout::StructLayout::entry_field) counts them
    /// (a static-array field's elements are consecutive entries).
    Field(u32),
}

/// Why a heap operation was refused.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeapError {
    /// The handle's object was freed (or the handle was not made by this heap).
    Stale(ObjRef),
    /// The address or the class was refused by the class table.
    Layout(LayoutError),
    /// The value's variant does not fit the declaration's kind.
    WrongKind {
        expected: &'static str,
        found: &'static str,
    },
    /// An object or class reference outside the declared class: `found` is not `expected` or a subclass.
    WrongClass { expected: ClassId, found: ClassId },
    /// A struct value of another type than declared.
    WrongStruct { expected: StructId, found: StructId },
    /// A struct value whose entry count is not its layout's.
    WrongFieldCount {
        ty: StructId,
        expected: u32,
        found: usize,
    },
    /// The object has fewer slots than its layout (its public `slots` were shortened).
    SlotMissing { object: ObjRef, slot: usize },
    /// Step `at` of a [`Heap::write_in`] path does not lead anywhere.
    BadPath { at: usize, reason: &'static str },
    /// The same handle twice in one [`Heap::free_many`].
    FreedTwice(ObjRef),
}

impl fmt::Display for HeapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HeapError::Stale(o) => write!(f, "stale object handle {o:?}"),
            HeapError::Layout(e) => e.fmt(f),
            HeapError::WrongKind { expected, found } => {
                write!(f, "a {found} value written to a {expected} property")
            }
            HeapError::WrongClass { expected, found } => {
                write!(f, "class {found:?} is not {expected:?} or a subclass of it")
            }
            HeapError::WrongStruct { expected, found } => {
                write!(
                    f,
                    "a struct of type {found:?} written where {expected:?} is declared"
                )
            }
            HeapError::WrongFieldCount {
                ty,
                expected,
                found,
            } => {
                write!(
                    f,
                    "a struct {ty:?} value with {found} entries, its layout has {expected}"
                )
            }
            HeapError::SlotMissing { object, slot } => {
                write!(f, "object {object:?} has no slot {slot}")
            }
            HeapError::BadPath { at, reason } => write!(f, "path step {at}: {reason}"),
            HeapError::FreedTwice(o) => write!(f, "{o:?} given twice to free_many"),
        }
    }
}

impl std::error::Error for HeapError {}

impl From<LayoutError> for HeapError {
    fn from(e: LayoutError) -> Self {
        HeapError::Layout(e)
    }
}

#[derive(Debug, Clone)]
struct Entry {
    generation: u32,
    object: Option<Object>,
}

/// The arena. See the module documentation.
#[derive(Debug, Clone, Default)]
pub struct Heap {
    entries: Vec<Entry>,
    /// Freed slots to reuse, last freed first.
    free: Vec<u32>,
    /// Every allocation in creation order; freed handles are skipped and compacted away now and then.
    order: Vec<ObjRef>,
    stale_in_order: usize,
    count: usize,
}

impl Heap {
    /// An empty heap.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates an object of class `class` (defined in `classes`) with every slot at its default value
    /// ([`ClassTable::default_slots`]), each checked as a write would check it. The outer, if given, must be a handle
    /// to an object that is not freed.
    pub fn alloc(
        &mut self,
        classes: &ClassTable,
        class: ClassId,
        name: NameId,
        outer: Option<ObjRef>,
        origin: Origin,
    ) -> Result<ObjRef, HeapError> {
        let slots = classes.default_slots(class)?;
        let mut cursor = slots.iter();
        for (_, decl) in classes.props(class)? {
            for _ in 0..decl.array_dim {
                let value = cursor.next().expect("default_slots follows props");
                self.check_value(classes, &decl.kind, value)?;
            }
        }
        if let Some(o) = outer {
            self.get(o)?;
        }
        let object = Object::new(class, name, outer, slots, origin);
        let handle = match self.free.pop() {
            Some(index) => {
                let entry = &mut self.entries[index as usize];
                entry.object = Some(object);
                ObjRef::new(index, entry.generation)
            }
            None => {
                let index = u32::try_from(self.entries.len()).expect("more than 2^32 heap slots");
                self.entries.push(Entry {
                    generation: 0,
                    object: Some(object),
                });
                ObjRef::new(index, 0)
            }
        };
        self.order.push(handle);
        self.count += 1;
        Ok(handle)
    }

    fn is_current(entries: &[Entry], o: ObjRef) -> bool {
        entries
            .get(o.index() as usize)
            .is_some_and(|e| e.generation == o.generation() && e.object.is_some())
    }

    /// Whether `o` names an object that is not freed (destroyed objects count).
    pub fn contains(&self, o: ObjRef) -> bool {
        Self::is_current(&self.entries, o)
    }

    /// The object `o` names.
    pub fn get(&self, o: ObjRef) -> Result<&Object, HeapError> {
        self.entries
            .get(o.index() as usize)
            .filter(|e| e.generation == o.generation())
            .and_then(|e| e.object.as_ref())
            .ok_or(HeapError::Stale(o))
    }

    /// The object `o` names, for changing.
    pub fn get_mut(&mut self, o: ObjRef) -> Result<&mut Object, HeapError> {
        self.entries
            .get_mut(o.index() as usize)
            .filter(|e| e.generation == o.generation())
            .and_then(|e| e.object.as_mut())
            .ok_or(HeapError::Stale(o))
    }

    /// Element `a` of object `o`, after checking the address against the object's class.
    pub fn read(&self, classes: &ClassTable, o: ObjRef, a: PropAddr) -> Result<&Value, HeapError> {
        let object = self.get(o)?;
        let slot = classes.resolve(object.class, a)?;
        object
            .slots
            .get(slot)
            .ok_or(HeapError::SlotMissing { object: o, slot })
    }

    /// Element `a` of object `o` for changing in place, after checking the address (not the value written later)
    /// against the object's class.
    pub fn slot_mut(
        &mut self,
        classes: &ClassTable,
        o: ObjRef,
        a: PropAddr,
    ) -> Result<&mut Value, HeapError> {
        let object = self.get_mut(o)?;
        let slot = classes.resolve(object.class, a)?;
        object
            .slots
            .get_mut(slot)
            .ok_or(HeapError::SlotMissing { object: o, slot })
    }

    /// Writes element `a` of object `o`, after checking the address against the object's class and the value against
    /// the declaration (module documentation).
    pub fn write(
        &mut self,
        classes: &ClassTable,
        o: ObjRef,
        a: PropAddr,
        v: Value,
    ) -> Result<(), HeapError> {
        let kind = Self::decl_kind(classes, a)?;
        self.read(classes, o, a)?;
        self.check_value(classes, kind, &v)?;
        *self.slot_mut(classes, o, a)? = v;
        Ok(())
    }

    fn decl_kind(classes: &ClassTable, a: PropAddr) -> Result<&PropKind, HeapError> {
        Ok(&classes
            .decl(a.prop)
            .ok_or(LayoutError::NotAProperty(a.prop))?
            .kind)
    }

    /// One step down from (`kind`, `v`): the kind and value it leads to, or why it leads nowhere.
    fn step<'k, 'v>(
        classes: &'k ClassTable,
        kind: &'k PropKind,
        v: &'v Value,
        at: usize,
        step: Step,
    ) -> Result<(&'k PropKind, &'v Value), HeapError> {
        let bad = |reason| HeapError::BadPath { at, reason };
        match (kind, v, step) {
            (PropKind::Array(inner), Value::Array(items), Step::Element(i)) => items
                .get(i as usize)
                .map(|item| (&**inner, item))
                .ok_or_else(|| bad("index past the array's end")),
            (PropKind::Struct(ty), Value::Struct(s), Step::Field(e)) => {
                let layout = classes
                    .struct_layout(*ty)
                    .ok_or(LayoutError::StructNotDefined(*ty))?;
                let (_, decl) = layout
                    .entry_field(e)
                    .ok_or_else(|| bad("entry past the struct's layout"))?;
                let field = s
                    .fields
                    .get(e as usize)
                    .ok_or_else(|| bad("the struct value has no such entry"))?;
                Ok((&decl.kind, field))
            }
            (_, _, Step::Element(_)) => Err(bad("not an array")),
            (_, _, Step::Field(_)) => Err(bad("not a struct")),
        }
    }

    /// Writes `v` at `path` inside element `a` of object `o` (`Arr[i] = x`, `Location.X = 1`,
    /// `Arr[i].Field = x`), checking the address, each step, and the new value against the kind it lands in; the
    /// rest of the aggregate is not rechecked. A shared aggregate on the path is copied first (copy on write). An
    /// empty path is [`Heap::write`].
    pub fn write_in(
        &mut self,
        classes: &ClassTable,
        o: ObjRef,
        a: PropAddr,
        path: &[Step],
        v: Value,
    ) -> Result<(), HeapError> {
        let mut kind = Self::decl_kind(classes, a)?;
        let mut current = self.read(classes, o, a)?;
        for (at, &s) in path.iter().enumerate() {
            (kind, current) = Self::step(classes, kind, current, at, s)?;
        }
        self.check_value(classes, kind, &v)?;
        let mut target = self.slot_mut(classes, o, a)?;
        for &s in path {
            // Every step was checked above on the same value.
            target = match (target, s) {
                (Value::Array(items), Step::Element(i)) => &mut Rc::make_mut(items)[i as usize],
                (Value::Struct(st), Step::Field(e)) => &mut Rc::make_mut(st).fields[e as usize],
                _ => unreachable!("the path was checked"),
            };
        }
        *target = v;
        Ok(())
    }

    /// Whether `v` may be stored in a property of kind `kind` (module documentation).
    pub fn check_value(
        &self,
        classes: &ClassTable,
        kind: &PropKind,
        v: &Value,
    ) -> Result<(), HeapError> {
        let wrong = || HeapError::WrongKind {
            expected: kind.kind_name(),
            found: v.kind_name(),
        };
        match (kind, v) {
            (PropKind::Int | PropKind::IntAttribute, Value::Int(_))
            | (PropKind::Float | PropKind::FloatAttribute, Value::Float(_))
            | (PropKind::Bool, Value::Bool(_))
            | (PropKind::Byte { .. } | PropKind::ByteAttribute { .. }, Value::Byte(_))
            | (PropKind::Str, Value::Str(_))
            | (PropKind::Name, Value::Name(_))
            | (PropKind::Map { .. }, Value::None) => Ok(()),
            (PropKind::Delegate, Value::Delegate(d)) => match d {
                Some(DelegateValue {
                    object: Some(r), ..
                }) => self.get(*r).map(|_| ()),
                _ => Ok(()),
            },
            (PropKind::Object { class }, Value::Object(r)) => match r {
                None => Ok(()),
                Some(r) => {
                    let found = self.get(*r)?.class;
                    if classes.is_child_of(found, *class) {
                        Ok(())
                    } else {
                        Err(HeapError::WrongClass {
                            expected: *class,
                            found,
                        })
                    }
                }
            },
            (PropKind::Interface { .. }, Value::Interface(r)) => match r {
                None => Ok(()),
                Some(r) => self.get(*r).map(|_| ()),
            },
            (PropKind::Class { meta }, Value::Class(c)) => match c {
                None => Ok(()),
                Some(c) if classes.get_layout(*c).is_none() => {
                    Err(LayoutError::UnknownClass(*c).into())
                }
                Some(c) if !classes.is_child_of(*c, *meta) => Err(HeapError::WrongClass {
                    expected: *meta,
                    found: *c,
                }),
                Some(_) => Ok(()),
            },
            (PropKind::Struct(ty), Value::Struct(s)) => {
                if s.ty != *ty {
                    return Err(HeapError::WrongStruct {
                        expected: *ty,
                        found: s.ty,
                    });
                }
                let layout = classes
                    .struct_layout(*ty)
                    .ok_or(LayoutError::StructNotDefined(*ty))?;
                if s.fields.len() != layout.slot_count() as usize {
                    return Err(HeapError::WrongFieldCount {
                        ty: *ty,
                        expected: layout.slot_count(),
                        found: s.fields.len(),
                    });
                }
                for (first, decl) in layout.fields() {
                    for i in 0..decl.array_dim {
                        let entry = &s.fields[first as usize + i as usize];
                        self.check_value(classes, &decl.kind, entry)?;
                    }
                }
                Ok(())
            }
            (PropKind::Array(inner), Value::Array(items)) => items
                .iter()
                .try_for_each(|item| self.check_value(classes, inner, item)),
            _ => Err(wrong()),
        }
    }

    /// Marks `o` destroyed (pending kill). It stays readable; [`Heap::live`] skips it. Destroying twice is allowed.
    pub fn destroy(&mut self, o: ObjRef) -> Result<(), HeapError> {
        self.get_mut(o)?.destroyed = true;
        Ok(())
    }

    /// Removes `o` and clears every reference to it held in the heap (module documentation). Every handle to it is
    /// refused from now on; the slot is reused with a new generation (or retired if the generation would wrap). This
    /// does not reach struct defaults in the class table: [`ObjectSpace::free`] does both.
    pub fn free(&mut self, o: ObjRef) -> Result<Object, HeapError> {
        let mut freed = self.free_many(&[o])?;
        Ok(freed.pop().expect("one object freed"))
    }

    /// [`Heap::free`] for a set of objects in one walk over the heap: removes each, then clears every reference to any
    /// of them. Refused, with nothing changed, if a handle is stale or appears twice. Returns the objects in the order
    /// given.
    pub fn free_many(&mut self, objects: &[ObjRef]) -> Result<Vec<Object>, HeapError> {
        let mut dead = FixedHashSet::default();
        for &o in objects {
            self.get(o)?;
            if !dead.insert(o) {
                return Err(HeapError::FreedTwice(o));
            }
        }
        let mut freed = Vec::with_capacity(objects.len());
        for &o in objects {
            let entry = &mut self.entries[o.index() as usize];
            freed.push(entry.object.take().expect("checked above"));
            if let Some(next) = entry.generation.checked_add(1) {
                entry.generation = next;
                self.free.push(o.index());
            } else {
                // Retired: the generation stays at u32::MAX with no object, so no handle matches it again.
            }
        }
        let is_dead = |r: ObjRef| dead.contains(&r);
        for other in self.entries.iter_mut().filter_map(|e| e.object.as_mut()) {
            if other.outer.is_some_and(is_dead) {
                other.outer = None;
            }
            other.slots.iter_mut().for_each(|v| clear_refs(v, &is_dead));
        }
        self.count -= objects.len();
        self.stale_in_order += objects.len();
        if self.stale_in_order > 32 && self.stale_in_order * 2 > self.order.len() {
            let entries = &self.entries;
            self.order.retain(|r| Self::is_current(entries, *r));
            self.stale_in_order = 0;
        }
        Ok(freed)
    }

    /// The instances ([`Origin::is_instance`]) of class `base` or a subclass that are neither destroyed nor freed, in
    /// creation order (the order the C++ iterators visit).
    pub fn live<'a>(
        &'a self,
        classes: &'a ClassTable,
        base: ClassId,
    ) -> impl Iterator<Item = ObjRef> + 'a {
        self.order.iter().copied().filter(move |r| {
            self.get(*r).is_ok_and(|o| {
                !o.destroyed && o.origin.is_instance() && classes.is_child_of(o.class, base)
            })
        })
    }

    /// How many objects are allocated and not freed (destroyed ones included).
    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
}

/// The heap with the class and name tables it is read through: what offline tools and the VM share.
#[derive(Debug, Clone, Default)]
pub struct ObjectSpace {
    pub heap: Heap,
    pub classes: ClassTable,
    pub names: NameTable,
}

impl ObjectSpace {
    /// An empty space (the name table holds only `None`).
    pub fn new() -> Self {
        Self::default()
    }

    /// [`Heap::alloc`] with the name interned.
    pub fn alloc(
        &mut self,
        class: ClassId,
        name: &str,
        outer: Option<ObjRef>,
        origin: Origin,
    ) -> Result<ObjRef, HeapError> {
        let name = self.names.intern(name);
        self.heap.alloc(&self.classes, class, name, outer, origin)
    }

    /// [`Heap::read`] through this space's classes.
    pub fn read(&self, o: ObjRef, a: PropAddr) -> Result<&Value, HeapError> {
        self.heap.read(&self.classes, o, a)
    }

    /// [`Heap::write`] through this space's classes.
    pub fn write(&mut self, o: ObjRef, a: PropAddr, v: Value) -> Result<(), HeapError> {
        self.heap.write(&self.classes, o, a, v)
    }

    /// [`Heap::write_in`] through this space's classes.
    pub fn write_in(
        &mut self,
        o: ObjRef,
        a: PropAddr,
        path: &[Step],
        v: Value,
    ) -> Result<(), HeapError> {
        self.heap.write_in(&self.classes, o, a, path, v)
    }

    /// [`Heap::slot_mut`] through this space's classes.
    pub fn slot_mut(&mut self, o: ObjRef, a: PropAddr) -> Result<&mut Value, HeapError> {
        self.heap.slot_mut(&self.classes, o, a)
    }

    /// The property `name` (any case) of class `c`, without interning the name.
    pub fn prop(&self, c: ClassId, name: &str) -> Option<PropId> {
        self.classes.prop(c, self.names.find(name)?)
    }

    /// [`Heap::live`] through this space's classes.
    pub fn live(&self, base: ClassId) -> impl Iterator<Item = ObjRef> + '_ {
        self.heap.live(&self.classes, base)
    }

    /// Frees `o` in the heap ([`Heap::free`]) and clears it out of every struct default in the class table.
    pub fn free(&mut self, o: ObjRef) -> Result<Object, HeapError> {
        let mut freed = self.free_many(&[o])?;
        Ok(freed.pop().expect("one object freed"))
    }

    /// Frees a set of objects in one heap walk ([`Heap::free_many`]) and clears them out of every struct default.
    pub fn free_many(&mut self, objects: &[ObjRef]) -> Result<Vec<Object>, HeapError> {
        let freed = self.heap.free_many(objects)?;
        let dead: FixedHashSet<ObjRef> = objects.iter().copied().collect();
        self.classes.clear_refs_in_defaults(&|r| dead.contains(&r));
        Ok(freed)
    }

    /// Sets default entry `entry` of struct `s` (a static-array element is an entry of its own), as one tag of the
    /// struct's default stream sets one field, after checking `v` as a heap write would ([`Heap::check_value`]: a
    /// reference names an object that is not freed, of the declared class). Applied by
    /// [`ClassTable::struct_defaults`].
    pub fn set_struct_default(
        &mut self,
        s: StructId,
        entry: u32,
        v: Value,
    ) -> Result<(), HeapError> {
        let layout = self
            .classes
            .struct_layout(s)
            .ok_or(LayoutError::StructNotDefined(s))?;
        if let Some((_, decl)) = layout.entry_field(entry) {
            self.heap.check_value(&self.classes, &decl.kind, &v)?;
        }
        Ok(self.classes.set_struct_default(s, entry, v)?)
    }

    /// Sets every default entry of struct `s` from the complete struct value `v`, after checking it as a heap write
    /// would.
    pub fn set_struct_defaults(&mut self, s: StructId, v: Value) -> Result<(), HeapError> {
        self.heap
            .check_value(&self.classes, &PropKind::Struct(s), &v)?;
        Ok(self.classes.set_struct_defaults(s, v)?)
    }
}

crate::value::assert_not_send!(Heap);
crate::value::assert_not_send!(ObjectSpace);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::PropertyDecl;

    /// Engine.Actor { int Health; float Ammo[3]; Actor Target; class<Actor> Kind; array<int> List; Vec V;
    ///                array<Actor> Others; Hit LastHit; delegate OnDone; interface Iface; }
    /// Game.Pawn extends Actor { bool Alive; }   Engine.Info (unrelated) { int Health; }
    /// Vec { float X; int Y[2]; }   Hit { Actor Who; float Time; }
    struct World {
        space: ObjectSpace,
        actor: ClassId,
        pawn: ClassId,
        info: ClassId,
        vec: StructId,
    }

    fn world() -> World {
        let mut space = ObjectSpace::new();
        let n = |s: &mut ObjectSpace, t: &str| s.names.intern(t);
        let x = PropertyDecl::new(n(&mut space, "X"), PropKind::Float);
        let y = PropertyDecl::new(n(&mut space, "Y"), PropKind::Int).with_array_dim(2);
        let vec = space
            .classes
            .define_struct("Core.Object.Vec", None, vec![x, y])
            .unwrap();
        let actor = space.classes.declare("Engine.Actor").unwrap();
        let who = PropertyDecl::new(n(&mut space, "Who"), PropKind::Object { class: actor });
        let time = PropertyDecl::new(n(&mut space, "Time"), PropKind::Float);
        let hit = space
            .classes
            .define_struct("Engine.Actor.Hit", None, vec![who, time])
            .unwrap();
        let decls = vec![
            PropertyDecl::new(n(&mut space, "Health"), PropKind::Int),
            PropertyDecl::new(n(&mut space, "Ammo"), PropKind::Float).with_array_dim(3),
            PropertyDecl::new(n(&mut space, "Target"), PropKind::Object { class: actor }),
            PropertyDecl::new(n(&mut space, "Kind"), PropKind::Class { meta: actor }),
            PropertyDecl::new(
                n(&mut space, "List"),
                PropKind::Array(Box::new(PropKind::Int)),
            ),
            PropertyDecl::new(n(&mut space, "V"), PropKind::Struct(vec)),
            PropertyDecl::new(
                n(&mut space, "Others"),
                PropKind::Array(Box::new(PropKind::Object { class: actor })),
            ),
            PropertyDecl::new(n(&mut space, "LastHit"), PropKind::Struct(hit)),
            PropertyDecl::new(n(&mut space, "OnDone"), PropKind::Delegate),
            PropertyDecl::new(n(&mut space, "Iface"), PropKind::Interface { class: actor }),
        ];
        space.classes.define("Engine.Actor", None, decls).unwrap();
        let alive = PropertyDecl::new(n(&mut space, "Alive"), PropKind::Bool);
        let pawn = space
            .classes
            .define("Game.Pawn", Some(actor), vec![alive])
            .unwrap();
        let health = PropertyDecl::new(n(&mut space, "Health"), PropKind::Int);
        let info = space
            .classes
            .define("Engine.Info", None, vec![health])
            .unwrap();
        World {
            space,
            actor,
            pawn,
            info,
            vec,
        }
    }

    impl World {
        fn p(&self, c: ClassId, name: &str) -> PropId {
            self.space.prop(c, name).unwrap()
        }
        fn new_obj(&mut self, c: ClassId, name: &str) -> ObjRef {
            self.space.alloc(c, name, None, Origin::Runtime).unwrap()
        }
    }

    #[test]
    fn objects_start_at_zero_and_take_checked_writes() {
        let mut w = world();
        let pawn = w.new_obj(w.pawn, "Pawn_0");
        let health = w.p(w.pawn, "health");
        assert_eq!(w.space.read(pawn, health.into()), Ok(&Value::Int(0)));
        w.space.write(pawn, health.into(), Value::Int(-5)).unwrap();
        assert_eq!(w.space.read(pawn, health.into()), Ok(&Value::Int(-5)));
        // The parent's PropId works on the child object; the child's own property sits after the parent's slots.
        assert_eq!(health.declared_in, w.actor);
        let alive = w.p(w.pawn, "Alive");
        assert_eq!(alive.slot, w.space.classes.layout(w.actor).slot_count());
        assert_eq!(w.space.read(pawn, alive.into()), Ok(&Value::Bool(false)));
        let obj = w.space.heap.get(pawn).unwrap();
        assert_eq!(
            obj.slots.len(),
            w.space.classes.layout(w.pawn).slot_count() as usize
        );
        assert_eq!(w.space.names.text(obj.name), "Pawn_0");
        assert_eq!(obj.origin, Origin::Runtime);
        // A wrong variant is refused, never converted.
        assert_eq!(
            w.space.write(pawn, health.into(), Value::Float(1.0)),
            Err(HeapError::WrongKind {
                expected: "Int",
                found: "Float"
            })
        );
        assert!(matches!(
            w.space.write(pawn, health.into(), Value::None),
            Err(HeapError::WrongKind { .. })
        ));
    }

    #[test]
    fn a_property_of_an_unrelated_class_is_refused() {
        let mut w = world();
        let info = w.new_obj(w.info, "Info_0");
        let actor_health = w.p(w.actor, "Health");
        assert!(matches!(
            w.space.read(info, actor_health.into()),
            Err(HeapError::Layout(LayoutError::NotInClass { .. }))
        ));
        assert!(matches!(
            w.space.write(info, actor_health.into(), Value::Int(1)),
            Err(HeapError::Layout(LayoutError::NotInClass { .. }))
        ));
        let own = w.p(w.info, "Health");
        assert_ne!(own, actor_health);
        w.space.write(info, own.into(), Value::Int(1)).unwrap();
    }

    #[test]
    fn static_array_elements_by_index() {
        let mut w = world();
        let a = w.new_obj(w.actor, "A");
        let ammo = w.p(w.actor, "Ammo");
        for i in 0..3 {
            w.space
                .write(a, PropAddr::element(ammo, i), Value::Float(i as f32 + 0.5))
                .unwrap();
        }
        for i in 0..3 {
            assert_eq!(
                w.space.read(a, PropAddr::element(ammo, i)),
                Ok(&Value::Float(i as f32 + 0.5))
            );
        }
        let health = w.p(w.actor, "Health");
        assert_eq!(
            w.space.read(a, health.into()),
            Ok(&Value::Int(0)),
            "neighbours untouched"
        );
        for bad in [3, 4, u32::MAX] {
            assert_eq!(
                w.space.read(a, PropAddr::element(ammo, bad)),
                Err(HeapError::Layout(LayoutError::IndexOutOfRange {
                    prop: ammo,
                    index: bad,
                    dim: 3
                }))
            );
            assert!(
                w.space
                    .write(a, PropAddr::element(ammo, bad), Value::Float(0.0))
                    .is_err()
            );
        }
    }

    #[test]
    fn stale_handles_are_refused_after_free_and_slot_reuse() {
        let mut w = world();
        let a = w.new_obj(w.actor, "A");
        let health = w.p(w.actor, "Health");
        let freed = w.space.heap.free(a).unwrap();
        assert_eq!(freed.class, w.actor);
        assert_eq!(w.space.heap.get(a).err(), Some(HeapError::Stale(a)));
        assert_eq!(w.space.read(a, health.into()), Err(HeapError::Stale(a)));
        assert_eq!(
            w.space.write(a, health.into(), Value::Int(1)),
            Err(HeapError::Stale(a))
        );
        assert_eq!(w.space.heap.destroy(a), Err(HeapError::Stale(a)));
        assert!(w.space.heap.free(a).is_err());
        // The slot is reused for the next object with a new generation; the old handle still fails.
        let b = w.new_obj(w.actor, "B");
        assert_eq!(b.index(), a.index());
        assert_ne!(b.generation(), a.generation());
        assert!(w.space.heap.get(a).is_err());
        assert!(w.space.heap.contains(b));
        assert!(!w.space.heap.contains(a));
        // An outer that was freed is refused.
        w.space.heap.free(b).unwrap();
        assert_eq!(
            w.space.alloc(w.actor, "C", Some(b), Origin::Runtime),
            Err(HeapError::Stale(b))
        );
    }

    #[test]
    fn a_slot_whose_generation_would_wrap_is_retired() {
        let mut w = world();
        let a = w.new_obj(w.actor, "A");
        w.space.heap.entries[a.index() as usize].generation = u32::MAX;
        let at_max = ObjRef::new(a.index(), u32::MAX);
        w.space.heap.free(at_max).unwrap();
        let b = w.new_obj(w.actor, "B");
        assert_ne!(b.index(), a.index(), "the retired slot is not reused");
        assert!(w.space.heap.get(at_max).is_err());
    }

    #[test]
    fn live_is_creation_order_and_skips_destroyed_and_freed() {
        let mut w = world();
        let a = w.new_obj(w.actor, "A");
        let p = w.new_obj(w.pawn, "P");
        let i = w.new_obj(w.info, "I");
        let b = w.new_obj(w.actor, "B");
        let live: Vec<_> = w.space.live(w.actor).collect();
        assert_eq!(live, [a, p, b]);
        assert_eq!(w.space.live(w.pawn).collect::<Vec<_>>(), [p]);
        assert_eq!(w.space.live(w.info).collect::<Vec<_>>(), [i]);
        w.space.heap.destroy(p).unwrap();
        w.space.heap.destroy(p).unwrap();
        assert_eq!(w.space.live(w.actor).collect::<Vec<_>>(), [a, b]);
        // Destroyed is still readable.
        let health = w.p(w.actor, "Health");
        assert!(w.space.read(p, health.into()).is_ok());
        assert!(w.space.heap.get(p).unwrap().destroyed);
        // A reused slot keeps creation order: the new object comes last.
        w.space.heap.free(a).unwrap();
        let c = w.new_obj(w.actor, "C");
        assert_eq!(c.index(), a.index());
        assert_eq!(w.space.live(w.actor).collect::<Vec<_>>(), [b, c]);
        assert_eq!(w.space.heap.len(), 4, "p (destroyed), i, b, c");
    }

    #[test]
    fn creation_order_survives_compaction() {
        let mut w = world();
        let all: Vec<_> = (0..200)
            .map(|k| w.new_obj(w.actor, &format!("A{k}")))
            .collect();
        // Free three of every four: past half of the order list stale, it is compacted.
        for (k, r) in all.iter().enumerate() {
            if k % 4 != 0 {
                w.space.heap.free(*r).unwrap();
            }
        }
        let later: Vec<_> = (0..10)
            .map(|k| w.new_obj(w.actor, &format!("L{k}")))
            .collect();
        let expected: Vec<_> = all.iter().step_by(4).chain(&later).copied().collect();
        assert_eq!(w.space.live(w.actor).collect::<Vec<_>>(), expected);
        assert!(
            w.space.heap.order.len() < 210,
            "freed handles were compacted away"
        );
        assert_eq!(w.space.heap.len(), 60);
    }

    #[test]
    fn reference_writes_are_checked_against_the_declared_class() {
        let mut w = world();
        let a = w.new_obj(w.actor, "A");
        let p = w.new_obj(w.pawn, "P");
        let i = w.new_obj(w.info, "I");
        let target = w.p(w.actor, "Target");
        assert_eq!(w.space.read(a, target.into()), Ok(&Value::Object(None)));
        w.space
            .write(a, target.into(), Value::Object(Some(p)))
            .unwrap();
        w.space
            .write(a, target.into(), Value::Object(None))
            .unwrap();
        assert_eq!(
            w.space.write(a, target.into(), Value::Object(Some(i))),
            Err(HeapError::WrongClass {
                expected: w.actor,
                found: w.info
            })
        );
        w.space.heap.free(i).unwrap();
        assert_eq!(
            w.space.write(a, target.into(), Value::Object(Some(i))),
            Err(HeapError::Stale(i))
        );
        // A destroyed (not freed) object can still be referenced.
        w.space.heap.destroy(p).unwrap();
        w.space
            .write(a, target.into(), Value::Object(Some(p)))
            .unwrap();

        let kind = w.p(w.actor, "Kind");
        w.space
            .write(a, kind.into(), Value::Class(Some(w.pawn)))
            .unwrap();
        assert_eq!(
            w.space.write(a, kind.into(), Value::Class(Some(w.info))),
            Err(HeapError::WrongClass {
                expected: w.actor,
                found: w.info
            })
        );
        assert!(matches!(
            w.space.write(a, kind.into(), Value::Object(Some(p))),
            Err(HeapError::WrongKind { .. })
        ));
    }

    #[test]
    fn aggregate_writes_are_checked_and_copied_on_write() {
        let mut w = world();
        let a = w.new_obj(w.actor, "A");
        let b = w.new_obj(w.actor, "B");
        let list = w.p(w.actor, "List");
        let v = w.p(w.actor, "V");

        let shared = Value::array(vec![Value::Int(1), Value::Int(2)]);
        w.space.write(a, list.into(), shared.clone()).unwrap();
        w.space.write(b, list.into(), shared).unwrap();
        let slot = list.slot as usize;
        w.space.heap.get_mut(a).unwrap().slots[slot]
            .array_mut()
            .unwrap()
            .push(Value::Int(3));
        assert_eq!(
            w.space
                .read(a, list.into())
                .unwrap()
                .elements()
                .unwrap()
                .len(),
            3
        );
        assert_eq!(
            w.space
                .read(b, list.into())
                .unwrap()
                .elements()
                .unwrap()
                .len(),
            2,
            "b kept its copy"
        );
        assert!(matches!(
            w.space.write(
                a,
                list.into(),
                Value::array(vec![Value::Int(1), Value::Float(2.0)])
            ),
            Err(HeapError::WrongKind {
                expected: "Int",
                found: "Float"
            })
        ));

        let zero = w.space.read(a, v.into()).unwrap().clone();
        assert_eq!(
            zero,
            w.space.classes.struct_layout(w.vec).unwrap().zero().clone()
        );
        let mut changed = zero.clone();
        changed.struct_mut().unwrap().fields[0] = Value::Float(4.0);
        w.space.write(a, v.into(), changed).unwrap();
        assert_eq!(
            w.space.read(b, v.into()),
            Ok(&zero),
            "the zero struct shared by b is unchanged"
        );
        let short = Value::structure(w.vec, vec![Value::Float(1.0)]);
        assert_eq!(
            w.space.write(a, v.into(), short),
            Err(HeapError::WrongFieldCount {
                ty: w.vec,
                expected: 3,
                found: 1
            })
        );
        let bad_field = Value::structure(
            w.vec,
            vec![Value::Float(1.0), Value::Int(1), Value::Float(1.0)],
        );
        assert!(matches!(
            w.space.write(a, v.into(), bad_field),
            Err(HeapError::WrongKind { .. })
        ));
        let other = Value::structure(StructId::new(7), vec![]);
        assert_eq!(
            w.space.write(a, v.into(), other),
            Err(HeapError::WrongStruct {
                expected: w.vec,
                found: StructId::new(7)
            })
        );
    }

    #[test]
    fn alloc_needs_a_defined_class() {
        let mut w = world();
        let later = w.space.classes.declare("Engine.Later").unwrap();
        assert_eq!(
            w.space.alloc(later, "X", None, Origin::Runtime),
            Err(HeapError::Layout(LayoutError::NotDefined(later)))
        );
        assert_eq!(
            w.space.alloc(ClassId::new(99), "X", None, Origin::Runtime),
            Err(HeapError::Layout(LayoutError::UnknownClass(ClassId::new(
                99
            ))))
        );
        let outer = w.new_obj(w.actor, "Outer");
        let inner = w
            .space
            .alloc(
                w.info,
                "Inner",
                Some(outer),
                Origin::Export {
                    package: 0,
                    index: 3,
                },
            )
            .unwrap();
        let obj = w.space.heap.get(inner).unwrap();
        assert_eq!(obj.outer, Some(outer));
        assert_eq!(
            obj.origin,
            Origin::Export {
                package: 0,
                index: 3
            }
        );
        assert_eq!(obj.state, None);
    }

    #[test]
    fn a_shortened_slot_vector_is_reported() {
        let mut w = world();
        let a = w.new_obj(w.pawn, "A");
        let alive = w.p(w.pawn, "Alive");
        w.space.heap.get_mut(a).unwrap().slots.truncate(1);
        assert!(matches!(
            w.space.read(a, alive.into()),
            Err(HeapError::SlotMissing { .. })
        ));
    }

    fn obj(r: ObjRef) -> Value {
        Value::Object(Some(r))
    }

    #[test]
    fn free_clears_every_reference_in_the_heap() {
        let mut w = world();
        let a = w.new_obj(w.actor, "A");
        let b = w.new_obj(w.actor, "B");
        let p = w.new_obj(w.pawn, "P");
        let inner = w
            .space
            .alloc(w.info, "Inner", Some(p), Origin::Runtime)
            .unwrap();
        let f = w.space.names.intern("Done");
        let (target, others, last_hit, on_done, iface) = (
            w.p(w.actor, "Target"),
            w.p(w.actor, "Others"),
            w.p(w.actor, "LastHit"),
            w.p(w.actor, "OnDone"),
            w.p(w.actor, "Iface"),
        );
        let hit_ty = match w.space.read(a, last_hit.into()).unwrap() {
            Value::Struct(s) => s.ty,
            other => panic!("{other:?}"),
        };
        let list = Value::array(vec![obj(p), obj(b), obj(p)]);
        let outside = list.clone();
        w.space.write(a, target.into(), obj(p)).unwrap();
        w.space.write(a, others.into(), list.clone()).unwrap();
        w.space.write(b, others.into(), list).unwrap();
        w.space
            .write(
                a,
                last_hit.into(),
                Value::structure(hit_ty, vec![obj(p), Value::Float(2.0)]),
            )
            .unwrap();
        let bound = Value::Delegate(Some(DelegateValue {
            object: Some(p),
            function: f,
        }));
        w.space.write(a, on_done.into(), bound).unwrap();
        w.space
            .write(a, iface.into(), Value::Interface(Some(p)))
            .unwrap();
        w.space.write(b, target.into(), obj(a)).unwrap();

        w.space.heap.free(p).unwrap();

        assert_eq!(w.space.read(a, target.into()), Ok(&Value::Object(None)));
        let cleared = Value::array(vec![Value::Object(None), obj(b), Value::Object(None)]);
        assert_eq!(w.space.read(a, others.into()), Ok(&cleared));
        assert_eq!(w.space.read(b, others.into()), Ok(&cleared));
        assert_eq!(
            w.space.read(a, last_hit.into()),
            Ok(&Value::structure(
                hit_ty,
                vec![Value::Object(None), Value::Float(2.0)]
            ))
        );
        assert_eq!(w.space.read(a, on_done.into()), Ok(&Value::Delegate(None)));
        assert_eq!(w.space.read(a, iface.into()), Ok(&Value::Interface(None)));
        assert_eq!(w.space.heap.get(inner).unwrap().outer, None);
        // References to other objects stay; a copy held outside the heap is not touched (copy on write).
        assert_eq!(w.space.read(b, target.into()), Ok(&obj(a)));
        assert_eq!(outside.elements().unwrap()[0], obj(p));
        // A handle kept outside the heap is still refused.
        assert_eq!(
            w.space.write(a, target.into(), obj(p)),
            Err(HeapError::Stale(p))
        );
    }

    #[test]
    fn a_freed_delegate_receiver_is_refused() {
        let mut w = world();
        let a = w.new_obj(w.actor, "A");
        let p = w.new_obj(w.pawn, "P");
        let f = w.space.names.intern("Done");
        let on_done = w.p(w.actor, "OnDone");
        let to = |o| {
            Value::Delegate(Some(DelegateValue {
                object: Some(o),
                function: f,
            }))
        };
        w.space.write(a, on_done.into(), to(p)).unwrap();
        let unbound_receiver = Value::Delegate(Some(DelegateValue {
            object: None,
            function: f,
        }));
        w.space.write(a, on_done.into(), unbound_receiver).unwrap();
        w.space.heap.free(p).unwrap();
        assert_eq!(
            w.space.write(a, on_done.into(), to(p)),
            Err(HeapError::Stale(p))
        );
        let nested = PropKind::Array(Box::new(PropKind::Delegate));
        assert_eq!(
            w.space
                .heap
                .check_value(&w.space.classes, &nested, &Value::array(vec![to(a), to(p)])),
            Err(HeapError::Stale(p))
        );
        assert_eq!(
            w.space
                .heap
                .check_value(&w.space.classes, &nested, &Value::array(vec![to(a)])),
            Ok(())
        );
    }

    #[test]
    fn live_yields_instances_only() {
        let mut w = world();
        let actor = w.actor;
        let default = w.space.alloc(
            actor,
            "Default__Actor",
            None,
            Origin::Default { class: actor },
        );
        let resource = w.space.alloc(
            actor,
            "Res",
            None,
            Origin::Resource {
                package: 0,
                index: 1,
            },
        );
        let exported = w
            .space
            .alloc(
                actor,
                "Placed",
                None,
                Origin::Export {
                    package: 0,
                    index: 2,
                },
            )
            .unwrap();
        let spawned = w.new_obj(actor, "Spawned");
        assert!(default.is_ok() && resource.is_ok());
        assert_eq!(w.space.live(actor).collect::<Vec<_>>(), [exported, spawned]);
        assert!(Origin::Runtime.is_instance());
        assert!(!Origin::Default { class: actor }.is_instance());
    }

    #[test]
    fn a_class_value_must_name_a_class_of_the_table() {
        let mut w = world();
        let a = w.new_obj(w.actor, "A");
        let kind = w.p(w.actor, "Kind");
        let unknown = ClassId::new(99);
        assert_eq!(
            w.space.write(a, kind.into(), Value::Class(Some(unknown))),
            Err(HeapError::Layout(LayoutError::UnknownClass(unknown)))
        );
        w.space.write(a, kind.into(), Value::Class(None)).unwrap();
    }

    #[test]
    fn slot_mut_checks_the_address() {
        let mut w = world();
        let a = w.new_obj(w.actor, "A");
        let i = w.new_obj(w.info, "I");
        let health = w.p(w.actor, "Health");
        *w.space.slot_mut(a, health.into()).unwrap() = Value::Int(9);
        assert_eq!(w.space.read(a, health.into()), Ok(&Value::Int(9)));
        assert!(matches!(
            w.space.slot_mut(i, health.into()),
            Err(HeapError::Layout(LayoutError::NotInClass { .. }))
        ));
        let ammo = w.p(w.actor, "Ammo");
        assert!(w.space.slot_mut(a, PropAddr::element(ammo, 3)).is_err());
    }

    #[test]
    fn write_in_changes_one_element_or_field() {
        let mut w = world();
        let a = w.new_obj(w.actor, "A");
        let b = w.new_obj(w.actor, "B");
        let p = w.new_obj(w.pawn, "P");
        let i = w.new_obj(w.info, "I");
        let (list, v, others, last_hit) = (
            w.p(w.actor, "List"),
            w.p(w.actor, "V"),
            w.p(w.actor, "Others"),
            w.p(w.actor, "LastHit"),
        );
        let shared = Value::array(vec![Value::Int(1), Value::Int(2)]);
        w.space.write(a, list.into(), shared.clone()).unwrap();
        w.space.write(b, list.into(), shared).unwrap();
        // Arr[1] = 5 on a; b keeps its copy.
        w.space
            .write_in(a, list.into(), &[Step::Element(1)], Value::Int(5))
            .unwrap();
        assert_eq!(
            w.space.read(a, list.into()),
            Ok(&Value::array(vec![Value::Int(1), Value::Int(5)]))
        );
        assert_eq!(
            w.space.read(b, list.into()),
            Ok(&Value::array(vec![Value::Int(1), Value::Int(2)]))
        );
        // V.X = 1.5 and V.Y[1] = 3 (entry 2: X is entry 0, Y[0] entry 1).
        w.space
            .write_in(a, v.into(), &[Step::Field(0)], Value::Float(1.5))
            .unwrap();
        w.space
            .write_in(a, v.into(), &[Step::Field(2)], Value::Int(3))
            .unwrap();
        assert_eq!(
            w.space
                .read(a, v.into())
                .unwrap()
                .as_struct()
                .unwrap()
                .fields,
            [Value::Float(1.5), Value::Int(0), Value::Int(3)]
        );
        assert_eq!(
            w.space.read(b, v.into()).unwrap(),
            w.space.classes.struct_layout(w.vec).unwrap().zero(),
            "b's zero struct was not changed"
        );
        // Others[0] = P, then the nested struct: LastHit.Who = P.
        w.space
            .write(a, others.into(), Value::array(vec![Value::Object(None)]))
            .unwrap();
        w.space
            .write_in(a, others.into(), &[Step::Element(0)], obj(p))
            .unwrap();
        w.space
            .write_in(a, last_hit.into(), &[Step::Field(0)], obj(p))
            .unwrap();
        // Only the new value is checked, and it is checked against the kind it lands in.
        assert_eq!(
            w.space
                .write_in(a, others.into(), &[Step::Element(0)], obj(i)),
            Err(HeapError::WrongClass {
                expected: w.actor,
                found: w.info
            })
        );
        assert!(matches!(
            w.space
                .write_in(a, list.into(), &[Step::Element(0)], Value::Float(1.0)),
            Err(HeapError::WrongKind {
                expected: "Int",
                found: "Float"
            })
        ));
        assert_eq!(
            w.space
                .write_in(a, list.into(), &[Step::Element(2)], Value::Int(0)),
            Err(HeapError::BadPath {
                at: 0,
                reason: "index past the array's end"
            })
        );
        assert_eq!(
            w.space
                .write_in(a, v.into(), &[Step::Field(3)], Value::Int(0)),
            Err(HeapError::BadPath {
                at: 0,
                reason: "entry past the struct's layout"
            })
        );
        assert_eq!(
            w.space
                .write_in(a, list.into(), &[Step::Field(0)], Value::Int(0)),
            Err(HeapError::BadPath {
                at: 0,
                reason: "not a struct"
            })
        );
        assert_eq!(
            w.space.write_in(
                a,
                v.into(),
                &[Step::Field(0), Step::Element(0)],
                Value::Int(0)
            ),
            Err(HeapError::BadPath {
                at: 1,
                reason: "not an array"
            })
        );
        // An empty path is a whole-property write.
        w.space
            .write_in(a, list.into(), &[], Value::array(vec![]))
            .unwrap();
        assert_eq!(w.space.read(a, list.into()), Ok(&Value::array(vec![])));
    }

    #[test]
    fn alloc_applies_struct_defaults() {
        let mut w = world();
        let vec = w.vec;
        w.space
            .classes
            .set_struct_default(vec, 0, Value::Float(1.0))
            .unwrap();
        let a = w.new_obj(w.actor, "A");
        let v = w.p(w.actor, "V");
        assert_eq!(
            w.space
                .read(a, v.into())
                .unwrap()
                .as_struct()
                .unwrap()
                .fields,
            [Value::Float(1.0), Value::Int(0), Value::Int(0)]
        );
        let zeros = w.space.classes.zero_slots(w.actor).unwrap();
        assert_eq!(
            &zeros[v.slot as usize],
            w.space.classes.struct_layout(vec).unwrap().zero()
        );
    }

    #[test]
    fn object_new_starts_in_the_default_state() {
        let o = Object::new(ClassId::new(0), NameId::NONE, None, vec![], Origin::Runtime);
        assert_eq!((o.state, o.destroyed), (None, false));
    }

    /// The Hit struct's id and its `Who` entry (entry 0) through the Actor's LastHit property.
    fn hit_ty(w: &World) -> StructId {
        let last_hit = w.p(w.actor, "LastHit");
        match &w.space.classes.decl(last_hit).unwrap().kind {
            PropKind::Struct(s) => *s,
            other => panic!("{other:?}"),
        }
    }

    fn who_of_last_hit(w: &World, o: ObjRef) -> Value {
        let last_hit = w.p(w.actor, "LastHit");
        w.space
            .read(o, last_hit.into())
            .unwrap()
            .as_struct()
            .unwrap()
            .fields[0]
            .clone()
    }

    #[test]
    fn struct_defaults_are_checked_against_the_heap() {
        let mut w = world();
        let hit = hit_ty(&w);
        let p = w.new_obj(w.pawn, "P");
        let i = w.new_obj(w.info, "I");
        let gone = w.new_obj(w.actor, "Gone");
        w.space.free(gone).unwrap();
        // A freed object and an object of the wrong class are refused as defaults.
        assert_eq!(
            w.space.set_struct_default(hit, 0, obj(gone)),
            Err(HeapError::Stale(gone))
        );
        assert_eq!(
            w.space.set_struct_default(hit, 0, obj(i)),
            Err(HeapError::WrongClass {
                expected: w.actor,
                found: w.info
            })
        );
        let whole = |who| Value::structure(hit, vec![who, Value::Float(0.0)]);
        assert_eq!(
            w.space.set_struct_defaults(hit, whole(obj(gone))),
            Err(HeapError::Stale(gone))
        );
        assert!(matches!(
            w.space.set_struct_defaults(hit, whole(obj(i))),
            Err(HeapError::WrongClass { .. })
        ));
        assert!(matches!(
            w.space.set_struct_default(hit, 5, Value::Int(0)),
            Err(HeapError::Layout(LayoutError::BadDefault { .. }))
        ));
        assert!(
            w.space
                .classes
                .struct_layout(hit)
                .unwrap()
                .own_defaults()
                .is_empty()
        );
        // A live object of the right class is accepted and reaches new objects.
        w.space.set_struct_default(hit, 0, obj(p)).unwrap();
        let a = w.new_obj(w.actor, "A");
        assert_eq!(who_of_last_hit(&w, a), obj(p));
        w.space
            .set_struct_defaults(hit, whole(Value::Object(None)))
            .unwrap();
        let b = w.new_obj(w.actor, "B");
        assert_eq!(who_of_last_hit(&w, b), Value::Object(None));
    }

    #[test]
    fn object_space_free_clears_struct_defaults() {
        let mut w = world();
        let hit = hit_ty(&w);
        let p = w.new_obj(w.pawn, "P");
        w.space.set_struct_default(hit, 0, obj(p)).unwrap();
        let before = w.new_obj(w.actor, "Before");
        w.space.free(p).unwrap();
        // The default no longer names p; the object made before had its copy cleared by the heap walk.
        assert_eq!(
            w.space.classes.struct_layout(hit).unwrap().own_defaults(),
            [(0, Value::Object(None))]
        );
        assert_eq!(who_of_last_hit(&w, before), Value::Object(None));
        // The slot is reused, and a new object never starts with the stale handle.
        let reuse = w.new_obj(w.pawn, "Reuse");
        assert_eq!(reuse.index(), p.index());
        let after = w.new_obj(w.actor, "After");
        assert_eq!(who_of_last_hit(&w, after), Value::Object(None));
    }

    #[test]
    fn alloc_refuses_a_default_the_heap_cannot_vouch_for() {
        let mut w = world();
        let hit = hit_ty(&w);
        let p = w.new_obj(w.pawn, "P");
        w.space.set_struct_default(hit, 0, obj(p)).unwrap();
        // Freeing through the heap alone does not reach the class table: the default goes stale, and alloc refuses
        // to copy it.
        w.space.heap.free(p).unwrap();
        assert_eq!(
            w.space.alloc(w.actor, "A", None, Origin::Runtime),
            Err(HeapError::Stale(p))
        );
        // The unchecked (crate-private) setter can store a wrong-class reference; alloc refuses that too.
        let i = w.new_obj(w.info, "I");
        w.space.classes.set_struct_default(hit, 0, obj(i)).unwrap();
        assert_eq!(
            w.space.alloc(w.actor, "B", None, Origin::Runtime),
            Err(HeapError::WrongClass {
                expected: w.actor,
                found: w.info
            })
        );
        // Classes that do not hold the struct are not affected.
        assert!(w.space.alloc(w.info, "J", None, Origin::Runtime).is_ok());
        let count = w.space.heap.len();
        w.space
            .set_struct_default(hit, 0, Value::Object(None))
            .unwrap();
        assert!(w.space.alloc(w.actor, "C", None, Origin::Runtime).is_ok());
        assert_eq!(
            w.space.heap.len(),
            count + 1,
            "the refused allocs added nothing"
        );
    }

    #[test]
    fn free_many_clears_a_set_in_one_call() {
        let mut w = world();
        let a = w.new_obj(w.actor, "A");
        let x = w.new_obj(w.actor, "X");
        let y = w.new_obj(w.pawn, "Y");
        let keep = w.new_obj(w.actor, "Keep");
        let others = w.p(w.actor, "Others");
        let target = w.p(w.actor, "Target");
        w.space
            .write(
                a,
                others.into(),
                Value::array(vec![obj(x), obj(keep), obj(y)]),
            )
            .unwrap();
        w.space.write(a, target.into(), obj(y)).unwrap();
        let hit = hit_ty(&w);
        w.space.set_struct_default(hit, 0, obj(x)).unwrap();
        // A stale handle or a repeated one refuses the whole call, changing nothing.
        let gone = w.new_obj(w.actor, "Gone");
        w.space.free(gone).unwrap();
        assert_eq!(
            w.space.free_many(&[x, gone]).err(),
            Some(HeapError::Stale(gone))
        );
        assert_eq!(
            w.space.free_many(&[x, y, x]).err(),
            Some(HeapError::FreedTwice(x))
        );
        assert!(w.space.heap.contains(x) && w.space.heap.contains(y));
        let len = w.space.heap.len();

        let freed = w.space.free_many(&[x, y]).unwrap();
        assert_eq!(
            freed.iter().map(|o| o.class).collect::<Vec<_>>(),
            [w.actor, w.pawn]
        );
        assert_eq!(w.space.heap.len(), len - 2);
        assert_eq!(
            w.space.read(a, others.into()),
            Ok(&Value::array(vec![
                Value::Object(None),
                obj(keep),
                Value::Object(None)
            ]))
        );
        assert_eq!(w.space.read(a, target.into()), Ok(&Value::Object(None)));
        assert_eq!(
            w.space.classes.struct_layout(hit).unwrap().own_defaults(),
            [(0, Value::Object(None))]
        );
        assert_eq!(w.space.live(w.actor).collect::<Vec<_>>(), [a, keep]);
        assert!(w.space.free_many(&[]).unwrap().is_empty());
    }

    #[test]
    fn freeing_an_outer_leaves_its_inner_objects_without_one() {
        let mut w = world();
        let outer = w.new_obj(w.actor, "Outer");
        let inner = w
            .space
            .alloc(w.info, "Inner", Some(outer), Origin::Runtime)
            .unwrap();
        w.space.free(outer).unwrap();
        assert_eq!(w.space.heap.get(inner).unwrap().outer, None);
        assert!(
            w.space.heap.contains(inner),
            "the inner object is not freed with it (UNVERIFIED rule)"
        );
    }

    /// A native map holds `Value::None` (W2-REFLECT, decision 2026-11-15-06): a class declaring one, and a class
    /// inheriting one, allocate, and only `None` may be written to it.
    #[test]
    fn a_map_slot_holds_none_declared_or_inherited() {
        let mut space = ObjectSpace::new();
        let lookup = space.names.intern("Lookup");
        let extra = space.names.intern("Extra");
        let map = PropKind::Map {
            key: Some(Box::new(PropKind::Name)),
            value: None,
        };
        let base = space
            .classes
            .define("Engine.Mapped", None, vec![PropertyDecl::new(lookup, map)])
            .unwrap();
        let below = space
            .classes
            .define(
                "Engine.Below",
                Some(base),
                vec![PropertyDecl::new(extra, PropKind::Int)],
            )
            .unwrap();
        let own = space.alloc(base, "M", None, Origin::Runtime).unwrap();
        let inherited = space.alloc(below, "B", None, Origin::Runtime).unwrap();
        let slot = PropAddr::scalar(space.prop(base, "Lookup").unwrap());
        assert_eq!(space.read(own, slot), Ok(&Value::None));
        assert_eq!(space.read(inherited, slot), Ok(&Value::None));
        assert_eq!(space.write(inherited, slot, Value::None), Ok(()));
        assert_eq!(
            space.write(own, slot, Value::Int(1)),
            Err(HeapError::WrongKind {
                expected: "Map",
                found: "Int"
            })
        );
    }
}
