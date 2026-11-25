//! `GetExpLevelFromManufacturerData` in Off (lane IMPL-GET-EXP-LEVEL) on invented classes with the packages' class,
//! struct and field names, defined by hand (AGENTS.md rule 9). Unloaded objects are slotless stand-ins.
//! AI-assisted (Claude Code).

use std::rc::Rc;

use ow_attr::AttributeSystem;
use ow_core::notyet::Missing;
use ow_core::{ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ClassId, ObjRef, PropAddr, StructId};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_vm::api::{FunctionDecl, Vm};
use ow_vm::error::VmError;

use super::*;

struct Fx {
    vm: Vm,
    balance: ClassId,
    bare: ClassId,
    maker: ClassId,
    entry: StructId,
    grade: StructId,
    modifiers: StructId,
    odd_entry: StructId,
    text_modifiers: StructId,
}

fn fx(parity: ParityMode) -> Fx {
    let mut s = ObjectSpace::new();
    let object = s.classes.define("Core.Object", None, vec![]).unwrap();
    let vm = Vm::new(s, DefStore::new_empty(), Rc::new(AttributeSystem::new_empty()), parity);
    build(vm, object)
}

/// The hand-defined classes on `vm`, under its `Core.Object` class `object`.
fn build(mut vm: Vm, object: ClassId) -> Fx {
    let s = &mut vm.space;
    let d = |s: &mut ObjectSpace, n: &str, k: PropKind| PropertyDecl::new(s.names.intern(n), k);
    let o = PropKind::Object { class: object };
    let decls = vec![d(s, "ExpLevel", PropKind::Int)];
    let modifiers = s.classes.define_struct("WillowGame.InventoryBalanceDefinition.GradeModifiers", None, decls).unwrap();
    let decls = vec![d(s, "GradeModifiers", PropKind::Struct(modifiers))];
    let grade = s.classes.define_struct("WillowGame.InventoryBalanceDefinition.ManufacturerGrade", None, decls).unwrap();
    let decls = vec![d(s, "Manufacturer", o.clone()), d(s, "Grades", PropKind::Array(Box::new(PropKind::Struct(grade))))];
    let entry = s.classes.define_struct("WillowGame.InventoryBalanceDefinition.ManufacturerEntry", None, decls).unwrap();
    // An entry struct without Grades, for the shape check.
    let decls = vec![d(s, "Manufacturer", o.clone())];
    let odd_entry = s.classes.define_struct("Toy.OddEntry", None, decls).unwrap();
    let decls = vec![
        d(s, "bInterpolateExpLevel", PropKind::Bool),
        d(s, "BaseDefinition", o.clone()),
        d(s, "Manufacturers", PropKind::Array(Box::new(PropKind::Struct(entry)))),
    ];
    let balance = s.classes.define("WillowGame.InventoryBalanceDefinition", Some(object), decls).unwrap();
    let pad = vec![d(s, "Pad", PropKind::Int)];
    let bare = s.classes.define("Toy.NotABalance", Some(object), pad).unwrap();
    let pad = vec![d(s, "Pad", PropKind::Int)];
    let maker = s.classes.define("WillowGame.ManufacturerDefinition", Some(object), pad).unwrap();
    // A string-valued ExpLevel grade, for the shape check (review r1 of #577).
    let decls = vec![d(s, "ExpLevel", PropKind::Str)];
    let text_modifiers = s.classes.define_struct("Toy.TextModifiers", None, decls).unwrap();
    let object_class = object;
    let params = vec![
        PropertyDecl::new(vm.space.names.intern("Manufacturer"), PropKind::Object { class: object_class }).with_flags(PropertyDecl::PARAM),
        PropertyDecl::new(vm.space.names.intern("GradeIndex"), PropKind::Int).with_flags(PropertyDecl::PARAM),
    ];
    let mut decl = FunctionDecl::native(ObjectPath::parse(PATH).unwrap(), "InventoryBalanceDefinition.GetExpLevelFromManufacturerData(object,int)")
        .with_owner(balance)
        .with_result(PropertyDecl::new(vm.space.names.intern("ReturnValue"), PropKind::Int));
    decl.params = params;
    vm.declare_function(decl).unwrap();
    register(&mut vm).unwrap();
    Fx { vm, balance, bare, maker, entry, grade, modifiers, odd_entry, text_modifiers }
}

impl Fx {
    fn obj(&mut self, class: ClassId, name: &str) -> ObjRef {
        self.vm.space.alloc(class, name, None, Origin::Runtime).unwrap()
    }

    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let c = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(c, name).unwrap();
        *self.vm.space.slot_mut(o, PropAddr::scalar(p)).unwrap() = v;
    }

    /// A balance: interpolating or not, with a base, and entries (manufacturer, grade `ExpLevel`s; None: no field).
    fn balance(&mut self, interpolate: bool, base: Option<ObjRef>, entries: &[(Option<ObjRef>, &[Option<i32>])]) -> ObjRef {
        let class = self.balance;
        let b = self.obj(class, "Balance");
        self.set(b, "bInterpolateExpLevel", Value::Bool(interpolate));
        self.set(b, "BaseDefinition", Value::Object(base));
        let list: Vec<Value> = entries
            .iter()
            .map(|&(m, grades)| {
                let gs = grades
                    .iter()
                    .map(|&level| {
                        let modifiers = match level {
                            Some(l) => Value::structure(self.modifiers, vec![Value::Int(l)]),
                            None => Value::structure(self.odd_entry, vec![Value::Object(None)]),
                        };
                        Value::structure(self.grade, vec![modifiers])
                    })
                    .collect();
                Value::structure(self.entry, vec![Value::Object(m), Value::array(gs)])
            })
            .collect();
        self.set(b, "Manufacturers", Value::array(list));
        b
    }

    fn ask(&mut self, b: ObjRef, m: Option<ObjRef>, word: i32) -> Result<i32, VmError> {
        exp_level(&mut self.vm, b, m, word)
    }
}

fn not_read(r: Result<i32, VmError>, what: &str) {
    match r {
        Err(VmError::NotYet(n)) => {
            assert_eq!(n.missing, Missing::NotRead, "{n}");
            assert_eq!(n.what, what, "unmarked");
        }
        other => panic!("expected NotRead, got {other:?}"),
    }
}

/// An interpolating balance answers the word, 0 included, without looking at its manufacturers (here malformed).
#[test]
fn an_interpolating_balance_answers_the_word() {
    let mut x = fx(ParityMode::Off);
    let b = x.balance(true, None, &[]);
    x.set(b, "Manufacturers", Value::Int(3));
    assert_eq!(x.ask(b, None, 0).unwrap(), 0, "0 is a valid answer");
    assert_eq!(x.ask(b, None, 7).unwrap(), 7);
}

/// A non-interpolating balance: the identity-matched entry's grade `word & 0xffff`: its `ExpLevel` (0 included) when the
/// high half is 0, else the high half capped at 63; an absent `ExpLevel` is 1.
#[test]
fn the_grade_of_the_matching_manufacturer() {
    let mut x = fx(ParityMode::Off);
    let maker = x.maker;
    let (bandit, other) = (x.obj(maker, "Bandit"), x.obj(maker, "Other"));
    let b = x.balance(false, None, &[(Some(other), &[Some(40)]), (Some(bandit), &[Some(5), Some(0), None])]);
    assert_eq!(x.ask(b, Some(bandit), 0).unwrap(), 5);
    assert_eq!(x.ask(b, Some(bandit), 1).unwrap(), 0, "a grade's ExpLevel 0 is answered as 0");
    assert_eq!(x.ask(b, Some(bandit), 2).unwrap(), 1, "an absent ExpLevel is 1");
    assert_eq!(x.ask(b, Some(bandit), 12 << 16).unwrap(), 12);
    assert_eq!(x.ask(b, Some(bandit), 100 << 16).unwrap(), 63);
    assert_eq!(x.ask(b, Some(other), 0).unwrap(), 40);
}

/// Not resolved here: the base's answer; nothing anywhere: 1. A None manufacturer matches no entry; an index past the
/// grades is not resolved.
#[test]
fn the_base_then_one() {
    let mut x = fx(ParityMode::Off);
    let maker = x.maker;
    let bandit = x.obj(maker, "Bandit");
    let base = x.balance(true, None, &[]);
    let b = x.balance(false, Some(base), &[(Some(bandit), &[Some(5)])]);
    assert_eq!(x.ask(b, None, 9).unwrap(), 9, "None falls through to the interpolating base");
    assert_eq!(x.ask(b, Some(bandit), 3).unwrap(), 3, "an index past the grades asks the base");
    let lone = x.balance(false, None, &[(Some(bandit), &[Some(5)])]);
    assert_eq!(x.ask(lone, None, 9).unwrap(), 1);
}

/// The call-local cycle guard answers 1 (checked before the interpolation test); a chain deeper than 64 is the port's
/// script error.
#[test]
fn the_cycle_guard_and_the_depth_limit() {
    let mut x = fx(ParityMode::Off);
    let a = x.balance(false, None, &[]);
    let b = x.balance(false, Some(a), &[]);
    x.set(a, "BaseDefinition", Value::Object(Some(b)));
    assert_eq!(x.ask(a, None, 9).unwrap(), 1);
    let mut next = None;
    for _ in 0..70 {
        next = Some(x.balance(false, next, &[]));
    }
    assert!(matches!(x.ask(next.unwrap(), None, 9), Err(VmError::Script { .. })));
}

/// The refusals, before anything is written: an unloaded visited balance (the balance itself or a base) and a listed
/// manufacturer stand-in identity cannot settle are `NotRead`; malformed shapes are script errors.
#[test]
fn the_preflight_refusals() {
    let mut x = fx(ParityMode::Off);
    let class = x.balance;
    let stand_in = x.vm.space.alloc_stand_in(class, "Balance_StandIn", Origin::Default { class }).unwrap();
    let heap = format!("{:?}", x.vm.space.heap);
    not_read(x.ask(stand_in, None, 1), UNLOADED);
    let b = x.balance(false, Some(stand_in), &[]);
    not_read(x.ask(b, None, 1), UNLOADED);
    let maker = x.maker;
    let bandit = x.obj(maker, "Bandit");
    let _ = heap;
    // Shapes.
    let bare = x.bare;
    let not_balance = x.obj(bare, "NotABalance");
    assert!(matches!(x.ask(not_balance, None, 1), Err(VmError::Script { .. })));
    let odd = x.balance(false, None, &[]);
    let entry = Value::structure(x.odd_entry, vec![Value::Object(Some(bandit))]);
    x.set(odd, "Manufacturers", Value::array(vec![entry]));
    assert!(matches!(x.ask(odd, Some(bandit), 0), Err(VmError::Script { .. })));
    let flagless = x.balance(false, None, &[]);
    x.set(flagless, "bInterpolateExpLevel", Value::Int(1));
    assert!(matches!(x.ask(flagless, None, 0), Err(VmError::Script { .. })));
}

/// Nothing is written by an answer, and the native is bound in Off through the VM (an int), not under Reference.
#[test]
fn the_binding_writes_nothing() {
    let mut x = fx(ParityMode::Off);
    let maker = x.maker;
    let bandit = x.obj(maker, "Bandit");
    let b = x.balance(true, None, &[(Some(bandit), &[Some(5)])]);
    let heap = format!("{:?}", x.vm.space.heap);
    let f = x.vm.find_function(PATH).unwrap();
    let ret = x.vm.call(f, Some(b), vec![Value::Object(Some(bandit)), Value::Int(4)]).unwrap().ret;
    assert_eq!(ret, Value::Int(4));
    assert_eq!(format!("{:?}", x.vm.space.heap), heap, "nothing written");
    let mut r = fx(ParityMode::Reference);
    let f = r.vm.find_function(PATH).unwrap();
    let key = r.vm.function(f).and_then(|d| d.native_key.clone()).unwrap();
    assert!(!r.vm.has_native(&key));
}

/// Review r1 of #577, item 2: malformed data that section 4 rejects is a script error before any answer: an
/// interpolating balance's unused `BaseDefinition` holding an int, and a picked grade's `ExpLevel` holding a string
/// although the high half answers.
#[test]
fn shapes_are_checked_before_answering() {
    let mut x = fx(ParityMode::Off);
    let b = x.balance(true, None, &[]);
    x.set(b, "BaseDefinition", Value::Int(3));
    assert!(matches!(x.ask(b, None, 7), Err(VmError::Script { .. })));
    let maker = x.maker;
    let bandit = x.obj(maker, "Bandit");
    let c = x.balance(false, None, &[(Some(bandit), &[Some(5)])]);
    let text = Value::structure(x.text_modifiers, vec![Value::Str("five".into())]);
    let grade = Value::structure(x.grade, vec![text]);
    let entry = Value::structure(x.entry, vec![Value::Object(Some(bandit)), Value::array(vec![grade])]);
    x.set(c, "Manufacturers", Value::array(vec![entry]));
    assert!(matches!(x.ask(c, Some(bandit), 12 << 16), Err(VmError::Script { .. })));
}

/// Review r1 of #577, item 4: the depth limit counts every fresh visit in Off, an interpolating tail included: a chain
/// of 64 balances answers, one of 65 is the port's script error.
#[test]
fn an_interpolating_tail_counts_for_the_depth_limit() {
    for (depth, ok) in [(64usize, true), (65, false)] {
        let mut x = fx(ParityMode::Off);
        let mut next = Some(x.balance(true, None, &[]));
        for _ in 1..depth {
            next = Some(x.balance(false, next, &[]));
        }
        let r = x.ask(next.unwrap(), None, 9);
        if ok {
            assert_eq!(r.unwrap(), 9, "depth {depth}");
        } else {
            assert!(matches!(r, Err(VmError::Script { .. })), "depth {depth}: {r:?}");
        }
    }
}

/// Review r1 of #577, item 3, on a synthetic package: a listed resource stand-in that is not one of the store's
/// definitions (its class declares no slots) is the unsettled identity, `NotRead`; a listed canonical unloaded export
/// settles by identity (no match, so 1). Neither is loaded, and nothing is written.
#[test]
fn a_stand_in_outside_the_store_is_unsettled_and_a_canonical_handle_is_not() {
    let dir = std::env::temp_dir().join(format!("ow-combat-explevel-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let core = {
        let mut t = ow_package::testkit::script::ScriptToy::new(&[]);
        t.class("Object", 0);
        t.build()
    };
    let lazy = {
        let mut t = ow_package::testkit::script::ScriptToy::with_imports(&["Class", "ObjectProperty"]);
        let core = t.p.add_import("Core", "Package", 0, "Core");
        let object = t.p.add_import("Core", "Class", core, "Object");
        t.class("ToyMaker", object);
        let holder = t.class("ToyHolder", object);
        t.prop("Object", holder, "Maker", 0, object);
        t.build()
    };
    let gd = {
        let mut t = ow_package::testkit::script::ScriptToy::with_imports(&["Class"]);
        let package = t.p.add_import("Core", "Package", 0, "ToyLazy");
        let maker = t.p.add_import("Core", "Class", package, "ToyMaker");
        let holder = t.p.add_import("Core", "Class", package, "ToyHolder");
        let mut empty = ow_package::testkit::ints(&[0]);
        empty.extend_from_slice(&t.none);
        let m = t.p.add_export(maker, "Maker_0", empty);
        let mut payload = ow_package::testkit::ints(&[0]);
        payload.extend(t.p.object_tag("Maker", m));
        payload.extend_from_slice(&t.none);
        t.p.add_export(holder, "Holder", payload);
        t.build()
    };
    let files: Vec<(&str, &[u8])> = vec![("Core.upk", core.as_slice()), ("ToyLazy.upk", lazy.as_slice()), ("GD_ToyMaker.upk", gd.as_slice())];
    let defs = DefStore::from_synthetic(&dir, &files, ow_defs::identity::IdentityPolicy::FirstInScanOrder, ParityMode::Off).unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
    let object = vm.find_class("Core.Object").unwrap();
    let holder = vm.defs.find(&mut vm.space, &ObjectPath::parse("GD_ToyMaker.Holder").unwrap()).unwrap().unwrap();
    let mut x = build(vm, object);
    let c = x.vm.space.heap.get(holder).unwrap().class;
    let p = x.vm.space.prop(c, "Maker").unwrap();
    let Value::Object(Some(canonical)) = x.vm.space.read(holder, PropAddr::scalar(p)).unwrap().clone() else { panic!() };
    assert!(x.vm.defs.is_unloaded(&x.vm.space, canonical), "the canonical export arrives unloaded");
    let (package, index) = match x.vm.space.heap.get(canonical).unwrap().origin {
        Origin::Export { package, index } | Origin::Resource { package, index } => (package, index),
        other => panic!("{other:?}"),
    };
    let maker_class = x.vm.space.heap.get(canonical).unwrap().class;
    let stand_in = x.vm.space.alloc_stand_in(maker_class, "Maker_0", Origin::Resource { package, index }).unwrap();
    assert!(!x.vm.defs.is_loaded(&x.vm.space, stand_in) && !x.vm.defs.is_unloaded(&x.vm.space, stand_in));
    let maker = x.maker;
    let asked = x.obj(maker, "Asked");
    let heap = format!("{:?}", x.vm.space.heap);
    let unsettled = x.balance(false, None, &[(Some(stand_in), &[Some(5)])]);
    let heap_after_setup = format!("{:?}", x.vm.space.heap);
    not_read(x.ask(unsettled, Some(asked), 0), UNSETTLED);
    let settled = x.balance(false, None, &[(Some(canonical), &[Some(5)])]);
    assert_eq!(x.ask(settled, Some(asked), 0).unwrap(), 1, "a canonical handle settles by identity: no match");
    assert!(x.vm.defs.is_unloaded(&x.vm.space, canonical), "the canonical export is not loaded");
    assert!(!x.vm.defs.is_loaded(&x.vm.space, stand_in), "the stand-in is not loaded");
    let _ = (heap, heap_after_setup);
    drop(x);
    let _ = std::fs::remove_dir_all(&dir);
}
