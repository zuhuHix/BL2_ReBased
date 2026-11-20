//! `RecomputeAttributeBaseValues` on an invented class set (AGENTS.md rule 9; this lane's decision 2026-11-19-169).
//! Expected values are written by hand from NATIVE_RECOMPUTE_ATTRIBUTE_BASE_VALUES.md section 4.

use std::cell::Cell;
use std::rc::Rc;

use ow_attr::AttributeSystem;
use ow_core::{ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ClassId, ObjRef, PropAddr};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_vm::api::{FunctionDecl, Vm};
use ow_vm::error::VmError;

use super::*;
use crate::equip::read_prop;

const PATH: &str = "WillowGame.WillowWeapon.RecomputeAttributeBaseValues";

struct Toy {
    vm: Vm,
}

fn decl(s: &mut ObjectSpace, name: &str, kind: PropKind, flags: u64) -> PropertyDecl {
    PropertyDecl::new(s.names.intern(name), kind).with_flags(flags)
}

/// The three declarations of one attribute: value, base and stack (of modifier objects).
fn triple(s: &mut ObjectSpace, name: &str, value: PropKind, base: PropKind, modifier: ClassId) -> Vec<PropertyDecl> {
    vec![
        decl(s, name, value, PropertyDecl::STACK_ATTRIBUTE | PropertyDecl::NOTIFY),
        decl(s, &format!("{name}BaseValue"), base, PropertyDecl::PLAIN_ATTRIBUTE),
        decl(s, &format!("{name}ModifierStack"), PropKind::Array(Box::new(PropKind::Object { class: modifier })), 0),
    ]
}

fn link(s: &mut ObjectSpace, class: ClassId, name: &str) {
    let v = s.names.intern(name);
    let b = s.names.intern(&format!("{name}BaseValue"));
    let k = s.names.intern(&format!("{name}ModifierStack"));
    s.classes.link_attribute(class, v, b, k).unwrap();
}

impl Toy {
    /// `Engine.Weapon` declares one triple (`InstantHitDamage`); `WillowGame.WillowWeapon` declares three more (an
    /// int, a float and a byte attribute), a plain float and `WeaponAttributeModifiers`. `Test.MixedWeapon` adds a
    /// triple whose base is an int under a float value, and `Test.NoRecords` lacks `WeaponAttributeModifiers`.
    fn new(mode: ParityMode) -> Toy {
        let mut s = ObjectSpace::new();
        let object = s.classes.define("Core.Object", None, vec![]).unwrap();
        let modifier = s.classes.define("Engine.AttributeModifier", Some(object), vec![]).unwrap();
        let fl = || PropKind::FloatAttribute;
        let inherited = triple(&mut s, "InstantHitDamage", fl(), fl(), modifier);
        let weapon_base = s.classes.define("Engine.Weapon", Some(object), inherited).unwrap();
        link(&mut s, weapon_base, "InstantHitDamage");
        let records = decl(&mut s, "WeaponAttributeModifiers", PropKind::Array(Box::new(PropKind::Int)), 0);
        let mut own = triple(&mut s, "ClipSize", PropKind::IntAttribute, PropKind::IntAttribute, modifier);
        own.extend(triple(&mut s, "ZoomedEndFOV", fl(), PropKind::Float, modifier));
        let byte = PropKind::ByteAttribute { enum_: None };
        own.extend(triple(&mut s, "Tier", byte.clone(), PropKind::Byte { enum_: None }, modifier));
        own.push(decl(&mut s, "Plain", PropKind::Float, 0));
        own.push(records.clone());
        let weapon = s.classes.define("WillowGame.WillowWeapon", Some(weapon_base), own).unwrap();
        for n in ["ClipSize", "ZoomedEndFOV", "Tier"] {
            link(&mut s, weapon, n);
        }
        let mixed_decls = triple(&mut s, "Odd", fl(), PropKind::Int, modifier);
        let mixed = s.classes.define("Test.MixedWeapon", Some(weapon), mixed_decls).unwrap();
        link(&mut s, mixed, "Odd");
        let short = triple(&mut s, "Lone", fl(), fl(), modifier);
        let no_records = s.classes.define("Test.NoRecords", Some(weapon_base), short).unwrap();
        link(&mut s, no_records, "Lone");
        let attrs = AttributeSystem::standard().unwrap();
        let mut vm = Vm::new(s, DefStore::new_empty(), Rc::new(attrs), mode);
        vm.declare_function(FunctionDecl::native(ObjectPath::parse(PATH).unwrap(), RECOMPUTE_BASE_VALUES)).unwrap();
        register(&mut vm).unwrap();
        Toy { vm }
    }

    fn obj(&mut self, class: &str, name: &str) -> ObjRef {
        let c = self.vm.space.classes.by_path(class).unwrap();
        self.vm.space.alloc(c, name, None, Origin::Runtime).unwrap()
    }

    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap();
        self.vm.space.write(o, PropAddr::scalar(p), v).unwrap();
    }

    fn get(&self, o: ObjRef, name: &str) -> Value {
        read_prop(&self.vm, o, name).unwrap()
    }

    fn call(&mut self, this: ObjRef) -> Result<Value, VmError> {
        let f = self.vm.find_function(PATH)?;
        Ok(self.vm.call(f, Some(this), Vec::new())?.ret)
    }

    /// A weapon of `class` whose triples hold a modified state: value, a different base, and one modifier each.
    fn modified(&mut self, class: &str) -> (ObjRef, ObjRef) {
        let w = self.obj(class, "Test.Gun");
        let m = self.obj("Engine.AttributeModifier", "Test.Gun.Mod");
        let stack = Value::Array(Rc::new(vec![Value::Object(Some(m))]));
        for (name, value, base) in [
            ("InstantHitDamage", Value::Float(63.633), Value::Float(46.789)),
            ("ClipSize", Value::Int((1 << 24) + 1), Value::Int(12)),
            ("ZoomedEndFOV", Value::Float(55.5), Value::Float(70.0)),
            ("Tier", Value::Byte(3), Value::Byte(1)),
        ] {
            self.set(w, name, value);
            self.set(w, &format!("{name}BaseValue"), base);
            self.set(w, &format!("{name}ModifierStack"), stack.clone());
        }
        self.set(w, "Plain", Value::Float(7.0));
        self.set(w, "WeaponAttributeModifiers", Value::Array(Rc::new(vec![Value::Int(1), Value::Int(2)])));
        (w, m)
    }
}

fn empty() -> Value {
    Value::Array(Rc::new(Vec::new()))
}

/// Section 4 item 3: every triple of the runtime class, the inherited one included, keeps its value, takes it as its
/// base (an int above 2^24 exactly, a byte as a byte) and loses its stack; then the applied-effect records are emptied.
/// A plain field is not touched, and the modifier object itself stays alive.
#[test]
fn every_triple_is_baked_inherited_ones_included() {
    let mut t = Toy::new(ParityMode::Off);
    let (w, m) = t.modified("WillowGame.WillowWeapon");
    assert_eq!(t.call(w).unwrap(), Value::None);
    for (name, value) in [
        ("InstantHitDamage", Value::Float(63.633)),
        ("ClipSize", Value::Int((1 << 24) + 1)),
        ("ZoomedEndFOV", Value::Float(55.5)),
        ("Tier", Value::Byte(3)),
    ] {
        assert_eq!(t.get(w, name), value, "{name} value");
        assert_eq!(t.get(w, &format!("{name}BaseValue")), value, "{name} base");
        assert_eq!(t.get(w, &format!("{name}ModifierStack")), empty(), "{name} stack");
    }
    assert_eq!(t.get(w, "Plain"), Value::Float(7.0));
    assert_eq!(t.get(w, "WeaponAttributeModifiers"), empty());
    assert!(t.vm.space.heap.get(m).is_ok(), "the modifier is not destroyed");
}

/// A second call changes nothing: base and value already agree, and the stacks are empty.
#[test]
fn a_second_call_is_a_no_op() {
    let mut t = Toy::new(ParityMode::Off);
    let (w, _) = t.modified("WillowGame.WillowWeapon");
    t.call(w).unwrap();
    let before: Vec<Value> = ["ClipSizeBaseValue", "InstantHitDamageBaseValue", "TierBaseValue"].iter().map(|n| t.get(w, n)).collect();
    t.call(w).unwrap();
    let after: Vec<Value> = ["ClipSizeBaseValue", "InstantHitDamageBaseValue", "TierBaseValue"].iter().map(|n| t.get(w, n)).collect();
    assert_eq!(before, after);
}

/// Section 3: no attribute-change notification runs (the writes are plain property writes). A notification override
/// that would refuse is never reached.
#[test]
fn no_notification_runs() {
    let mut t = Toy::new(ParityMode::Off);
    let calls = Rc::new(Cell::new(0));
    let seen = calls.clone();
    t.vm
        .register_attribute_notify(
            "WillowGame.WillowWeapon",
            Rc::new(move |_vm, _owner, _prop, _forms| {
                seen.set(seen.get() + 1);
                Err(VmError::Script { function: "notify".into(), message: "must not run".into() }.into())
            }),
        )
        .unwrap();
    let (w, _) = t.modified("WillowGame.WillowWeapon");
    t.call(w).unwrap();
    assert_eq!(calls.get(), 0);
}

/// Section 4 item 2: a base of another numeric kind than its value is a data error raised before the first write, so
/// the earlier (inherited) triple keeps its modified state.
#[test]
fn a_mismatched_base_refuses_before_any_write() {
    let mut t = Toy::new(ParityMode::Off);
    let (w, _) = t.modified("Test.MixedWeapon");
    let e = t.call(w).unwrap_err();
    assert!(matches!(&e, VmError::Script { message, .. } if message.contains("Odd")), "{e:?}");
    assert_eq!(t.get(w, "InstantHitDamageBaseValue"), Value::Float(46.789));
    assert_ne!(t.get(w, "InstantHitDamageModifierStack"), empty());
    assert_ne!(t.get(w, "WeaponAttributeModifiers"), empty());
}

/// Section 4 item 2: a class without `WeaponAttributeModifiers` is a data error, and nothing is written.
#[test]
fn a_class_without_the_records_refuses_before_any_write() {
    let mut t = Toy::new(ParityMode::Off);
    let w = t.obj("Test.NoRecords", "Test.Short");
    t.set(w, "Lone", Value::Float(2.0));
    t.set(w, "LoneBaseValue", Value::Float(1.0));
    let e = t.call(w).unwrap_err();
    assert!(matches!(&e, VmError::Script { message, .. } if message.contains("WeaponAttributeModifiers")), "{e:?}");
    assert_eq!(t.get(w, "LoneBaseValue"), Value::Float(1.0));
}

/// Reference binds nothing (no C++ stand-in).
#[test]
fn reference_binds_nothing() {
    let t = Toy::new(ParityMode::Reference);
    assert!(!t.vm.has_native(RECOMPUTE_BASE_VALUES));
}
