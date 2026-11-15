//! Owner: lane W1-OBJ. The handles of the object model (architecture section 3.3, contract W1-OBJ).
//!
//! Every handle is a small `Copy` index into the table that made it: [`ObjRef`] into the
//! [`Heap`](crate::heap::Heap), [`ClassId`], [`StructId`] and [`EnumId`] into the
//! [`ClassTable`](crate::layout::ClassTable), [`NameId`] into the [`NameTable`](crate::names::NameTable), and
//! [`FuncId`] into the function table the reflection lane (W2-REFLECT) builds. A handle means nothing without its table,
//! and a handle from one table used with another is a caller bug the tables can only partly catch (an index past the
//! end is refused; an index that happens to be valid is not).
//!
//! An [`ObjRef`] also carries a generation, so a handle to an object that was freed and whose arena slot was reused is
//! refused instead of reaching the new occupant. Only the heap makes `ObjRef`s.
//!
//! Properties are addressed by slot, not by name (decision D14): a [`PropId`] is the first slot of a property in the
//! layout of the class that declares it, and the same `PropId` is valid in every subclass, because a subclass's layout
//! is its super chain's slots followed by its own. A static array (`array_dim > 1`) takes `array_dim` consecutive slots,
//! and one element is a [`PropAddr`]: the property plus an index below `array_dim`, checked on every heap access. These
//! slots are this project's own layout, not the original engine's memory offsets; no native offset is used (AGENTS.md
//! rule 4 is not touched).

use std::fmt;

/// A handle to one object in the [`Heap`](crate::heap::Heap): an arena slot and the generation the slot had when the
/// object was allocated. Freeing the object bumps the slot's generation, so this handle is then refused
/// (`HeapError::Stale`) even after the slot holds a new object.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjRef {
    index: u32,
    generation: u32,
}

impl ObjRef {
    /// Made only by the heap.
    pub(crate) const fn new(index: u32, generation: u32) -> Self {
        ObjRef { index, generation }
    }

    /// The arena slot (for logs and tests; it is reused after the object is freed).
    pub const fn index(self) -> u32 {
        self.index
    }

    /// The slot's generation when this object was allocated.
    pub const fn generation(self) -> u32 {
        self.generation
    }
}

impl fmt::Debug for ObjRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ObjRef({}#{})", self.index, self.generation)
    }
}

macro_rules! table_id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u32);

        impl $name {
            /// The handle for entry `raw` of its table. Tables make their own handles; other code uses this only to
            /// rebuild a handle it stored as a number.
            pub const fn new(raw: u32) -> Self {
                $name(raw)
            }

            /// The table index.
            pub const fn raw(self) -> u32 {
                self.0
            }

            /// The table index as a `usize`.
            pub const fn index(self) -> usize {
                self.0 as usize
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, concat!(stringify!($name), "({})"), self.0)
            }
        }
    };
}

table_id!(
    /// A class in the [`ClassTable`](crate::layout::ClassTable), in declaration order.
    ClassId
);
table_id!(
    /// A struct type in the [`ClassTable`](crate::layout::ClassTable).
    StructId
);
table_id!(
    /// An enum type in the [`ClassTable`](crate::layout::ClassTable).
    EnumId
);
table_id!(
    /// A function in the reflection lane's function table (W2-REFLECT); this crate defines only the handle.
    FuncId
);
/// A name, as a UE3 FName holds it: an entry of the [`NameTable`](crate::names::NameTable) (the base text) and an
/// instance number. Number 0 means no suffix; number `n > 0` prints as `<base>_<n - 1>`, so `Projectile_1234` is the
/// entry `Projectile` with number 1235 and spawning many numbered objects does not grow the table. Two `NameId`s are
/// equal exactly when their texts are equal ignoring ASCII case (the base is folded, the number is exact), as FNames
/// compare. Packages store names the same way (a name-table index and a number, `Reader::name` in `ow-package`).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NameId {
    index: u32,
    number: u32,
}

impl NameId {
    /// The name `None` (entry 0, no number), interned first by every [`NameTable`](crate::names::NameTable). It is
    /// the zero value of a `NameProperty`, as in the C++ VM (`Runtime::zeroValue`).
    pub const NONE: NameId = NameId {
        index: 0,
        number: 0,
    };

    /// The name with base entry `index` and instance `number`. Only the name table makes ids: built here, a pair could
    /// name a text that already has another id (`("Foo_5", 0)` against `("Foo", 6)`).
    pub(crate) const fn new(index: u32, number: u32) -> Self {
        NameId { index, number }
    }

    /// The base entry in the name table.
    pub const fn index(self) -> usize {
        self.index as usize
    }

    /// The base entry as stored.
    pub const fn raw_index(self) -> u32 {
        self.index
    }

    /// The instance number: 0 for none, `n` for the suffix `_<n - 1>`.
    pub const fn number(self) -> u32 {
        self.number
    }
}

impl fmt::Debug for NameId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.number == 0 {
            write!(f, "NameId({})", self.index)
        } else {
            write!(f, "NameId({}_{})", self.index, self.number - 1)
        }
    }
}

/// A property: its first slot in the layout of the class that declares it, and that class. Valid in the declaring
/// class and every subclass; refused on any other class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PropId {
    /// The property's first slot (element 0 of a static array).
    pub slot: u32,
    /// The class whose declarations hold the property.
    pub declared_in: ClassId,
}

/// One element of a property: `index` is 0 for a scalar property and below `array_dim` for a static array. Checked on
/// every heap access; an index at or past `array_dim` is refused, never clamped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PropAddr {
    pub prop: PropId,
    pub index: u32,
}

impl PropAddr {
    /// Element 0 (the whole property, for a scalar).
    pub const fn scalar(prop: PropId) -> Self {
        PropAddr { prop, index: 0 }
    }

    /// Element `index` of a static array.
    pub const fn element(prop: PropId, index: u32) -> Self {
        PropAddr { prop, index }
    }
}

impl From<PropId> for PropAddr {
    fn from(prop: PropId) -> Self {
        PropAddr::scalar(prop)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_round_trip_their_index() {
        assert_eq!(ClassId::new(7).raw(), 7);
        assert_eq!(StructId::new(3).index(), 3);
        assert_eq!(format!("{:?}", EnumId::new(2)), "EnumId(2)");
        assert_eq!((NameId::NONE.index(), NameId::NONE.number()), (0, 0));
        let n = NameId::new(3, 0);
        assert_eq!(NameId::new(3, 5).number(), 5);
        assert_ne!(n, NameId::new(3, 1));
        assert_eq!(format!("{n:?}"), "NameId(3)");
        assert_eq!(format!("{:?}", NameId::new(3, 1)), "NameId(3_0)");
        let r = ObjRef::new(4, 9);
        assert_eq!((r.index(), r.generation()), (4, 9));
        assert_eq!(format!("{r:?}"), "ObjRef(4#9)");
    }

    #[test]
    fn a_scalar_address_is_element_zero() {
        let p = PropId {
            slot: 5,
            declared_in: ClassId::new(1),
        };
        assert_eq!(PropAddr::from(p), PropAddr::element(p, 0));
        assert_eq!(PropAddr::scalar(p).index, 0);
    }
}
