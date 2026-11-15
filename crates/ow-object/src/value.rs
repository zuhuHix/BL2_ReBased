//! Owner: lane W1-OBJ. The script value (architecture section 3.3, contract W1-OBJ).
//!
//! [`Value`] is what a property slot, a local, an argument or a struct field holds. It is the Rust form of the C++
//! `vm::Value` (`src/vm.hpp`): structs and dynamic arrays are values, copied on assignment; objects, classes,
//! interfaces and delegates are references.
//!
//! - **Ints** are `i32` and script arithmetic on them wraps (the C++ `makeInt` keeps 32 bits). **Floats** are `f32`:
//!   the C++ holds a double kept at float precision (`makeFloat` rounds through `float`), and a single `+ - * /` of two
//!   f32 values equals "double, then round" (architecture 5.1 N4), so storing `f32` loses nothing. **Bytes** are `u8`.
//! - **Strings** are `Rc<str>` and **structs and arrays** are `Rc`: cloning a value shares the text or the aggregate,
//!   and [`Value::array_mut`] / [`Value::struct_mut`] detach a shared aggregate before changing it (`Rc::make_mut`, the
//!   C++ `Value::mut()`). A copy therefore never sees a later change to the original.
//! - **Strings are UTF-8** (`str`), as the C++ `std::string`. The game's `FString` is UTF-16, so `Len`, `Mid`,
//!   `InStr`, `Left`/`Right` and `Asc`/`Chr` count UTF-16 code units there, where a character outside the Basic
//!   Multilingual Plane is two units. Which unit the VM counts in is a decision for W2-VMAPI / W3-CORENAT, not made
//!   here (UNVERIFIED: no live check of a non-ASCII string yet); this type only stores the text.
//! - **Names** are [`NameId`]s from the [`NameTable`] (a base entry and an FName instance number), so name equality
//!   is id equality (case-insensitive, as FNames).
//! - **References** are `Option<ObjRef>` / `Option<ClassId>`: `None` inside is the null reference. [`Value::None`] is
//!   "no value" (a void result, an unset local), the C++ `Kind::None`; it compares equal to a null reference, but a
//!   heap write of it to a reference property is refused (write `Value::Object(None)`). The C++ interpreter's `None`
//!   literal is a null object reference (`EX_NoObject` in `src/interp.cpp`), so W3-INTERP produces
//!   `Value::Object(None)` for it.
//!
//! **Not `Send`, on purpose (decision D18, approved 2026-11-14).** `Value` holds `Rc`, so it, the heap, the class table
//! and [`ObjectSpace`](crate::heap::ObjectSpace) cannot move to another thread, and the compiler enforces it (the
//! assertion at the end of this file stops the build if `Value` ever becomes `Send`). The simulation runs on one
//! thread and talks to the host through `Send` message types; a tool that wants threads builds one object space per
//! thread over shared `Arc<Package>` bytes. Changing this is a breaking contract change (architecture section 7).
//!
//! `#[derive(PartialEq)]` on [`Value`] is structural equality for tests (same variant, same contents, floats by IEEE
//! `==`). UnrealScript's `==` is [`same_value`].

use std::rc::Rc;

use crate::ids::{ClassId, NameId, ObjRef, StructId};
use crate::names::NameTable;

/// A script value. See the module documentation.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// No value (a void result, an unset local).
    None,
    /// A 32-bit int; script arithmetic wraps.
    Int(i32),
    /// A single-precision float.
    Float(f32),
    Bool(bool),
    Byte(u8),
    Str(Rc<str>),
    Name(NameId),
    /// An object reference; `None` is the null reference.
    Object(Option<ObjRef>),
    /// A class reference; `None` is the null reference.
    Class(Option<ClassId>),
    /// An interface reference (the object that implements it); `None` is the null reference.
    Interface(Option<ObjRef>),
    /// A struct value, shared until changed.
    Struct(Rc<StructValue>),
    /// A dynamic array, shared until changed.
    Array(Rc<Vec<Value>>),
    /// A delegate; `None` is an unbound delegate.
    Delegate(Option<DelegateValue>),
}

/// The fields of a struct value, in the order of its [`StructLayout`](crate::layout::StructLayout): a static-array
/// field takes `array_dim` consecutive entries, as a class property takes consecutive slots.
#[derive(Debug, Clone, PartialEq)]
pub struct StructValue {
    pub ty: StructId,
    pub fields: Vec<Value>,
}

/// A bound delegate: the object it calls on (`None` for a delegate bound without an object) and the function name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DelegateValue {
    pub object: Option<ObjRef>,
    pub function: NameId,
}

impl Value {
    /// A string value.
    pub fn string(s: &str) -> Value {
        Value::Str(Rc::from(s))
    }

    /// A dynamic array value.
    pub fn array(elements: Vec<Value>) -> Value {
        Value::Array(Rc::new(elements))
    }

    /// A struct value. The field count is not checked here; heap writes check it against the layout.
    pub fn structure(ty: StructId, fields: Vec<Value>) -> Value {
        Value::Struct(Rc::new(StructValue { ty, fields }))
    }

    /// The variant's name, for error messages.
    pub fn kind_name(&self) -> &'static str {
        match self {
            Value::None => "None",
            Value::Int(_) => "Int",
            Value::Float(_) => "Float",
            Value::Bool(_) => "Bool",
            Value::Byte(_) => "Byte",
            Value::Str(_) => "Str",
            Value::Name(_) => "Name",
            Value::Object(_) => "Object",
            Value::Class(_) => "Class",
            Value::Interface(_) => "Interface",
            Value::Struct(_) => "Struct",
            Value::Array(_) => "Array",
            Value::Delegate(_) => "Delegate",
        }
    }

    /// The elements of an array value, or `None` for any other variant.
    pub fn elements(&self) -> Option<&[Value]> {
        match self {
            Value::Array(a) => Some(a),
            _ => None,
        }
    }

    /// The elements of an array value for changing, detached first if the array is shared (copy on write).
    pub fn array_mut(&mut self) -> Option<&mut Vec<Value>> {
        match self {
            Value::Array(a) => Some(Rc::make_mut(a)),
            _ => None,
        }
    }

    /// The struct of a struct value, or `None` for any other variant.
    pub fn as_struct(&self) -> Option<&StructValue> {
        match self {
            Value::Struct(s) => Some(s),
            _ => None,
        }
    }

    /// The struct of a struct value for changing, detached first if it is shared (copy on write).
    pub fn struct_mut(&mut self) -> Option<&mut StructValue> {
        match self {
            Value::Struct(s) => Some(Rc::make_mut(s)),
            _ => None,
        }
    }

    /// The condition truth of a value, as the C++ `Value::truth` (`src/vm.cpp`): a number is true when it is not zero
    /// (a NaN float is true, `-0.0` false), a reference when it is not null, a string when it is not empty, a name
    /// when it is neither empty nor `None` (any case); `None`, structs, arrays and delegates are false.
    pub fn truth(&self, names: &NameTable) -> bool {
        match self {
            Value::Int(i) => *i != 0,
            Value::Bool(b) => *b,
            Value::Byte(b) => *b != 0,
            Value::Float(f) => *f != 0.0,
            Value::Object(o) | Value::Interface(o) => o.is_some(),
            Value::Class(c) => c.is_some(),
            Value::Str(s) => !s.is_empty(),
            Value::Name(n) => {
                *n != NameId::NONE
                    && names
                        .get(*n)
                        .is_some_and(|t| !t.is_empty() && !t.eq_ignore_ascii_case("none"))
            }
            Value::None | Value::Struct(_) | Value::Array(_) | Value::Delegate(_) => false,
        }
    }
}

/// The value of a number-like variant as a double, as the C++ `Value::number()` reads it.
fn number(v: &Value) -> Option<f64> {
    match v {
        Value::Float(f) => Some(f64::from(*f)),
        Value::Int(i) => Some(f64::from(*i)),
        Value::Byte(b) => Some(f64::from(*b)),
        Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        _ => None,
    }
}

/// The integer of an integer-like variant, as the C++ keeps Int, Bool and Byte in one integer field.
fn integer(v: &Value) -> Option<i64> {
    match v {
        Value::Int(i) => Some(i64::from(*i)),
        Value::Byte(b) => Some(i64::from(*b)),
        Value::Bool(b) => Some(i64::from(*b)),
        _ => None,
    }
}

/// UnrealScript `==` on two values, as the C++ `sameValue` (`src/vm.cpp`) decides it for two values of the same
/// type. The C++ uses it for the object, interface and vector `==` natives, struct comparison, `switch`/`case`, and the
/// array `Find` / `RemoveItem` natives.
///
/// - If either side is a float, both are compared as doubles (so `Int(3) == Float(3.0)`; NaN equals nothing; `-0.0`
///   equals `0.0`).
/// - Int, Bool and Byte compare by their integer (`Byte(1) == Int(1) == Bool(true)`).
/// - Strings compare case-sensitively, byte for byte; names by id, which is case-insensitive.
/// - Object and interface references compare by handle, classes by id; `Value::None` equals every null reference
///   (object, interface, class) in either order.
/// - Structs compare field by field in order (the struct type is not compared, as in the C++), arrays by length and
///   then element by element.
/// - Delegates are equal when they name the same object and the same function, which is what the C++ interpreter's
///   delegate operators compare (`EX_EqualEqual_DelDel` and its three siblings in `src/interp.cpp`), except that the
///   function names compare case-insensitively here because names are interned (the interpreter compares the two
///   strings byte for byte). UNVERIFIED in the running game.
///
/// Where the C++ `sameValue` differs, on purpose (no C++ test pins any of these):
/// - `sameValue` answers by the left side's kind, so mixed kinds are lopsided there: `Name == Str` compares the
///   lower-cased texts and `Str == Name` the bytes; `Class(null) == None` is true and `None == Class(null)` false; an
///   int against a string, an object or `None` compares the integer fields both leave at 0. Here a name and a string
///   are unequal in both orders, a null class equals `None` in both orders, and other unrelated variants are unequal.
/// - `sameValue` returns false for every delegate pair (it has no case for them); the interpreter's own delegate
///   operators compare object and function, and this follows those (above).
pub fn same_value(a: &Value, b: &Value) -> bool {
    use Value as V;
    if matches!(a, V::Float(_)) || matches!(b, V::Float(_)) {
        return match (number(a), number(b)) {
            (Some(x), Some(y)) => x == y,
            _ => false,
        };
    }
    if let (Some(x), Some(y)) = (integer(a), integer(b)) {
        return x == y;
    }
    match (a, b) {
        (V::Str(x), V::Str(y)) => x == y,
        (V::Name(x), V::Name(y)) => x == y,
        (V::Object(x) | V::Interface(x), V::Object(y) | V::Interface(y)) => x == y,
        (V::Class(x), V::Class(y)) => x == y,
        (V::None, V::None) => true,
        (V::None, V::Object(r) | V::Interface(r)) | (V::Object(r) | V::Interface(r), V::None) => {
            r.is_none()
        }
        (V::None, V::Class(c)) | (V::Class(c), V::None) => c.is_none(),
        (V::Struct(x), V::Struct(y)) => all_same(&x.fields, &y.fields),
        (V::Array(x), V::Array(y)) => all_same(x, y),
        (V::Delegate(x), V::Delegate(y)) => x == y,
        _ => false,
    }
}

fn all_same(x: &[Value], y: &[Value]) -> bool {
    x.len() == y.len() && x.iter().zip(y).all(|(p, q)| same_value(p, q))
}

// D18: the object model must stay `!Send`. `assert_not_send!(T)` stops the build if `T` implements `Send`: for a
// `Send` type both impls below apply, so the type parameter cannot be inferred and the call is ambiguous. (The usual
// "assert a type does not implement a trait" idiom, written out here; no crate.)
pub(crate) trait AmbiguousIfSend<A> {
    fn some_item() {}
}
impl<T: ?Sized> AmbiguousIfSend<()> for T {}
pub(crate) struct IsSend;
impl<T: ?Sized + Send> AmbiguousIfSend<IsSend> for T {}

macro_rules! assert_not_send {
    ($t:ty) => {
        const _: fn() = || {
            let _ = <$t as $crate::value::AmbiguousIfSend<_>>::some_item;
        };
    };
}
pub(crate) use assert_not_send;

assert_not_send!(Value);

#[cfg(test)]
mod tests {
    use super::*;

    fn obj(index: u32, generation: u32) -> Value {
        Value::Object(Some(ObjRef::new(index, generation)))
    }

    #[test]
    fn numbers_compare_across_int_byte_bool_and_float() {
        assert!(same_value(&Value::Int(3), &Value::Float(3.0)));
        assert!(same_value(&Value::Float(3.0), &Value::Byte(3)));
        assert!(!same_value(&Value::Float(3.5), &Value::Int(3)));
        assert!(same_value(&Value::Byte(1), &Value::Int(1)));
        assert!(same_value(&Value::Bool(true), &Value::Int(1)));
        assert!(same_value(&Value::Bool(false), &Value::Byte(0)));
        assert!(same_value(&Value::Float(-0.0), &Value::Float(0.0)));
        assert!(!same_value(
            &Value::Float(f32::NAN),
            &Value::Float(f32::NAN)
        ));
        assert!(
            !same_value(&Value::Int(-1), &Value::Byte(255)),
            "a byte is 0..=255, an int keeps its sign"
        );
        assert!(same_value(&Value::Int(i32::MIN), &Value::Int(i32::MIN)));
    }

    #[test]
    fn strings_are_case_sensitive_and_names_are_not() {
        let mut names = NameTable::new();
        assert!(same_value(&Value::string("Abc"), &Value::string("Abc")));
        assert!(!same_value(&Value::string("Abc"), &Value::string("abc")));
        let a = names.intern("Abc");
        let b = names.intern("ABC");
        assert!(same_value(&Value::Name(a), &Value::Name(b)));
        assert!(!same_value(&Value::Name(a), &Value::Name(NameId::NONE)));
    }

    #[test]
    fn references_and_none() {
        assert!(same_value(&obj(1, 0), &obj(1, 0)));
        assert!(
            !same_value(&obj(1, 0), &obj(1, 1)),
            "a reused slot is another object"
        );
        assert!(same_value(
            &Value::Interface(Some(ObjRef::new(1, 0))),
            &obj(1, 0)
        ));
        assert!(same_value(&Value::None, &Value::Object(None)));
        assert!(same_value(&Value::Object(None), &Value::None));
        assert!(same_value(&Value::Interface(None), &Value::None));
        assert!(same_value(&Value::Class(None), &Value::None));
        assert!(same_value(&Value::None, &Value::Class(None)));
        assert!(!same_value(&Value::None, &obj(1, 0)));
        assert!(same_value(
            &Value::Class(Some(ClassId::new(2))),
            &Value::Class(Some(ClassId::new(2)))
        ));
        assert!(!same_value(
            &Value::Class(Some(ClassId::new(2))),
            &Value::Class(None)
        ));
        // A null class equals None in both orders (the C++ sameValue only when the class is on the left);
        // a class that is not null equals None in neither.
        let some_class = Value::Class(Some(ClassId::new(2)));
        assert!(!same_value(&some_class, &Value::None));
        assert!(!same_value(&Value::None, &some_class));
        assert!(
            !same_value(&Value::Object(None), &Value::Class(None)),
            "different reference kinds"
        );
    }

    #[test]
    fn aggregates_compare_by_contents() {
        let s = |v: f32| Value::structure(StructId::new(0), vec![Value::Float(v), Value::Int(1)]);
        assert!(same_value(&s(1.0), &s(1.0)));
        assert!(!same_value(&s(1.0), &s(2.0)));
        // The C++ ignores the struct type; so does this.
        let other_type = Value::structure(StructId::new(5), vec![Value::Float(1.0), Value::Int(1)]);
        assert!(same_value(&s(1.0), &other_type));
        let short = Value::structure(StructId::new(0), vec![Value::Float(1.0)]);
        assert!(!same_value(&s(1.0), &short));
        let a = Value::array(vec![Value::Int(1), Value::Int(2)]);
        assert!(same_value(
            &a,
            &Value::array(vec![Value::Byte(1), Value::Float(2.0)])
        ));
        assert!(!same_value(&a, &Value::array(vec![Value::Int(1)])));
        assert!(same_value(&Value::array(vec![]), &Value::array(vec![])));
    }

    #[test]
    fn delegates_and_unrelated_variants() {
        let mut names = NameTable::new();
        let f = names.intern("OnDone");
        let d = |o: Option<ObjRef>| {
            Value::Delegate(Some(DelegateValue {
                object: o,
                function: f,
            }))
        };
        assert!(same_value(&d(None), &d(None)));
        assert!(!same_value(&d(None), &d(Some(ObjRef::new(0, 0)))));
        assert!(same_value(&Value::Delegate(None), &Value::Delegate(None)));
        // Deliberate differences from the C++ (see same_value's documentation).
        assert!(!same_value(&Value::Int(0), &Value::string("x")));
        assert!(!same_value(&Value::Float(0.0), &Value::Object(None)));
        assert!(!same_value(&Value::Int(0), &Value::None));
        assert!(!same_value(&Value::None, &Value::Int(0)));
        // A name and a string are unequal in both orders (the C++ compared texts, lopsidedly).
        let none_text = Value::string("None");
        assert!(!same_value(&Value::Name(NameId::NONE), &none_text));
        assert!(!same_value(&none_text, &Value::Name(NameId::NONE)));
    }

    #[test]
    fn truth_per_variant() {
        let mut names = NameTable::new();
        let empty = names.intern("");
        let other = names.intern("Begin");
        let none_lower = NameId::NONE;
        assert!(Value::Int(-1).truth(&names));
        assert!(!Value::Int(0).truth(&names));
        assert!(Value::Float(f32::NAN).truth(&names));
        assert!(!Value::Float(-0.0).truth(&names));
        assert!(!Value::Byte(0).truth(&names));
        assert!(Value::string("x").truth(&names));
        assert!(!Value::string("").truth(&names));
        assert!(Value::Name(other).truth(&names));
        assert!(!Value::Name(empty).truth(&names));
        assert!(!Value::Name(none_lower).truth(&names));
        assert!(obj(0, 0).truth(&names));
        assert!(!Value::Object(None).truth(&names));
        assert!(!Value::None.truth(&names));
        assert!(!Value::array(vec![Value::Int(1)]).truth(&names));
    }

    #[test]
    fn copies_do_not_see_later_changes() {
        let mut a = Value::array(vec![Value::Int(1)]);
        let copy = a.clone();
        a.array_mut().unwrap().push(Value::Int(2));
        assert_eq!(copy.elements().unwrap().len(), 1);
        assert_eq!(a.elements().unwrap().len(), 2);

        let mut s = Value::structure(StructId::new(0), vec![Value::Float(1.0)]);
        let copy = s.clone();
        s.struct_mut().unwrap().fields[0] = Value::Float(2.0);
        assert_eq!(copy.as_struct().unwrap().fields[0], Value::Float(1.0));
        assert_eq!(s.as_struct().unwrap().fields[0], Value::Float(2.0));

        // An unshared aggregate is changed in place, not copied.
        let mut lone = Value::array(vec![Value::Int(1)]);
        let before = lone.elements().unwrap().as_ptr();
        lone.array_mut().unwrap()[0] = Value::Int(5);
        assert_eq!(lone.elements().unwrap().as_ptr(), before);
        assert!(Value::Int(1).array_mut().is_none());
        assert!(Value::Int(1).struct_mut().is_none());
    }

    #[test]
    fn kind_names() {
        assert_eq!(Value::None.kind_name(), "None");
        assert_eq!(Value::Interface(None).kind_name(), "Interface");
        assert_eq!(Value::Delegate(None).kind_name(), "Delegate");
    }
}
