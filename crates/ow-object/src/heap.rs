//! Owner: lane W1-OBJ. The object heap: an arena of objects addressed by generational handles (architecture
//! section 3.3, contract W1-OBJ), and [`ObjectSpace`], the heap with its class and name tables.
//!
//! **Arena, not shared pointers.** The C++ VM holds objects in `std::shared_ptr`, so an object lives as long as anything
//! refers to it. Here an object lives in a heap slot until it is freed, referenced or not, and an [`ObjRef`] names the
//! slot and its generation. Life has three steps:
//!
//! 1. [`Heap::alloc`] creates the object with every slot at its kind's zero value.
//! 2. [`Heap::destroy`] marks it destroyed (UE3's pending kill, the C++ `Object::destroyed`): it stays readable and
//!    writable, so script that runs after `Destroy()` still works, but [`Heap::live`] no longer yields it.
//! 3. [`Heap::free`] removes it. The slot's generation goes up, so every handle to it is refused from then on
//!    ([`HeapError::Stale`]), also after the slot holds a new object. A slot whose generation would wrap is retired
//!    instead of reused, so an old handle can never become valid again.
//!
//! For actors, what the iterators walk, this matches UE3, where actors live until destroyed. Who frees what and when
//! (the world after a tick, a collector for non-actor objects) is for the lanes above; a garbage collector can be added
//! behind this API later. Decision file 2026-11-15, "Object model: slot layouts in an arena heap, not Send".
//!
//! **Checked access.** [`Heap::read`] and [`Heap::write`] check the address against the object's class layout (the
//! declaring class is the object's class or a super class, the index is below `array_dim`) and `write` checks the value
//! against the declaration: the variant matches the kind, an object reference names a live (not freed) object of the
//! declared class or a subclass, a class reference names the declared metaclass or a subclass, a struct has the
//! declared type and its layout's entry count, and arrays and struct fields are checked element by element.
//! An interface reference is checked only for being live: which classes implement an interface comes from the class
//! bodies (W2-REFLECT). [`Object::slots`] is public for code that has resolved its slots already; writing there skips
//! these checks.
//!
//! **Not `Send`** (decision D18): see [`crate::value`].

use std::fmt;

use ow_package::PackageId;

use crate::ids::{ClassId, NameId, ObjRef, PropAddr, PropId, StructId};
use crate::layout::{ClassTable, LayoutError, PropKind};
use crate::names::NameTable;
use crate::value::Value;

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

/// One object: its class, name, outer, property slots (in the class layout's order), current state and origin.
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

    /// Creates an object of class `class` (defined in `classes`) with every slot at its zero value. The outer, if
    /// given, must be a handle to an object that is not freed.
    pub fn alloc(
        &mut self,
        classes: &ClassTable,
        class: ClassId,
        name: NameId,
        outer: Option<ObjRef>,
        origin: Origin,
    ) -> Result<ObjRef, HeapError> {
        let slots = classes.zero_slots(class)?;
        if let Some(o) = outer {
            self.get(o)?;
        }
        let object = Object {
            class,
            name,
            outer,
            slots,
            state: None,
            destroyed: false,
            origin,
        };
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

    /// Writes element `a` of object `o`, after checking the address against the object's class and the value against
    /// the declaration (module documentation).
    pub fn write(
        &mut self,
        classes: &ClassTable,
        o: ObjRef,
        a: PropAddr,
        v: Value,
    ) -> Result<(), HeapError> {
        let slot = {
            let object = self.get(o)?;
            classes.resolve(object.class, a)?
        };
        let kind = &classes
            .decl(a.prop)
            .ok_or(LayoutError::NotAProperty(a.prop))?
            .kind;
        self.check_value(classes, kind, &v)?;
        let object = self.get_mut(o)?;
        let target = object
            .slots
            .get_mut(slot)
            .ok_or(HeapError::SlotMissing { object: o, slot })?;
        *target = v;
        Ok(())
    }

    /// Whether `v` may be stored in a property of kind `kind`.
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
            | (PropKind::Delegate, Value::Delegate(_)) => Ok(()),
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
                Some(c) if !classes.is_child_of(*c, *meta) => Err(HeapError::WrongClass {
                    expected: *meta,
                    found: *c,
                }),
                _ => Ok(()),
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
                    .ok_or(LayoutError::UnknownStruct(*ty))?;
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

    /// Removes `o`. Every handle to it is refused from now on; the slot is reused with a new generation (or retired if
    /// the generation would wrap).
    pub fn free(&mut self, o: ObjRef) -> Result<Object, HeapError> {
        self.get(o)?;
        let entry = &mut self.entries[o.index() as usize];
        let object = entry.object.take().ok_or(HeapError::Stale(o))?;
        if let Some(next) = entry.generation.checked_add(1) {
            entry.generation = next;
            self.free.push(o.index());
        } else {
            // Retired: the generation stays at u32::MAX with no object, so no handle matches it again.
        }
        self.count -= 1;
        self.stale_in_order += 1;
        if self.stale_in_order > 32 && self.stale_in_order * 2 > self.order.len() {
            let entries = &self.entries;
            self.order.retain(|r| Self::is_current(entries, *r));
            self.stale_in_order = 0;
        }
        Ok(object)
    }

    /// The objects of class `base` or a subclass that are neither destroyed nor freed, in creation order (the order
    /// the C++ iterators visit).
    pub fn live<'a>(
        &'a self,
        classes: &'a ClassTable,
        base: ClassId,
    ) -> impl Iterator<Item = ObjRef> + 'a {
        self.order.iter().copied().filter(move |r| {
            self.get(*r)
                .is_ok_and(|o| !o.destroyed && classes.is_child_of(o.class, base))
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

    /// The property `name` (any case) of class `c`, without interning the name.
    pub fn prop(&self, c: ClassId, name: &str) -> Option<PropId> {
        self.classes.prop(c, self.names.find(name)?)
    }

    /// [`Heap::live`] through this space's classes.
    pub fn live(&self, base: ClassId) -> impl Iterator<Item = ObjRef> + '_ {
        self.heap.live(&self.classes, base)
    }
}

crate::value::assert_not_send!(Heap);
crate::value::assert_not_send!(ObjectSpace);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::PropertyDecl;

    /// Engine.Actor { int Health; float Ammo[3]; Actor Target; class<Actor> Kind; array<int> List; Vec V; }
    /// Game.Pawn extends Actor { bool Alive; }   Engine.Info (unrelated) { int Health; }
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
}
