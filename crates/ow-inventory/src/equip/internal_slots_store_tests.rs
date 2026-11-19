//! P0 of `ApplyInternalSlotEffectModifiers` on an invented `WillowGame.upk` in a synthetic store (AGENTS.md rule 9;
//! review r1 of #483). Loading `ToyHolder` leaves its references as store handles not yet loaded: `ToyAttr` (whose
//! value chain names `ToyResolver`), `ToyBroken` (a payload that cannot be read) and `ToyRemoval` (an attribute with no
//! value chain, whose context chain names `ToyBrokenResolver`, a payload that cannot be read either), with a value chain
//! naming `ToyRemovalResolver` (class `ToyStackResolver`, a toy resolver over the context's `Bonus` stack, review r1 of
//! #518). `ToyItem` is an invented item class (two slots, the backup list) for the calls of lane FIX-483-FOLLOWUP.
//! AI-assisted (Claude Code).

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::{ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_object::ids::{ClassId, FuncId, ObjRef, PropAddr};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_package::testkit::reflect::{property_payload, toy_struct};
use ow_package::testkit::script::ScriptToy;
use ow_package::testkit::{Export, ints};
use ow_vm::api::{FunctionDecl, Vm};
use ow_vm::error::VmError;

use super::{INTERNAL_SLOTS, MODIFIER_CLASS, load_attribute, register};

static NEXT: AtomicU32 = AtomicU32::new(0);

fn package() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&[
        "Class",
        "ObjectProperty",
        "ArrayProperty",
        "ScriptStruct",
        "BoolProperty",
        "ByteProperty",
        "FloatProperty",
        "StructProperty",
    ]);
    let object = t.class("Object", 0);
    let attr = t.class("ToyAttribute", object);
    for chain in ["ContextResolverChain", "ValueResolverChain"] {
        let inner = t.p.next_export();
        t.prop("Array", attr, chain, 0, inner + 1);
        t.prop("Object", inner, chain, 0, object);
    }
    let holder = t.class("Holder", object);
    t.prop("Object", holder, "Attr", 0, object);
    t.prop("Object", holder, "Broken", 0, object);
    t.prop("Object", holder, "Removal", 0, object);
    let mut empty = 0i32.to_le_bytes().to_vec();
    empty.extend_from_slice(&t.none);
    let resolver = t.p.add_export(object, "ToyResolver", empty);
    let mut a = 0i32.to_le_bytes().to_vec();
    a.extend(t.p.array_tag("ValueResolverChain", 1, &resolver.to_le_bytes()));
    a.extend_from_slice(&t.none);
    let toy_attr = t.p.add_export(attr, "ToyAttr", a);
    // A payload that ends inside its first tag: the store cannot read it.
    let broken = t.p.add_export(attr, "ToyBroken", vec![0, 0, 0, 0, 7]);
    let broken_resolver = t.p.add_export(object, "ToyBrokenResolver", vec![0, 0, 0, 0, 7]);
    let stack_resolver = t.class("ToyStackResolver", object);
    let mut empty = 0i32.to_le_bytes().to_vec();
    empty.extend_from_slice(&t.none);
    let removal_resolver = t.p.add_export(stack_resolver, "ToyRemovalResolver", empty);
    let mut a = 0i32.to_le_bytes().to_vec();
    a.extend(t.p.array_tag("ContextResolverChain", 1, &broken_resolver.to_le_bytes()));
    a.extend(t.p.array_tag("ValueResolverChain", 1, &removal_resolver.to_le_bytes()));
    a.extend_from_slice(&t.none);
    let removal = t.p.add_export(attr, "ToyRemoval", a);
    // The item class: `AttributeSlots[2]` of the five fields the native reads, and `AppliedAttributeSlotEffects`.
    let item = t.class("ToyItem", object);
    let data = toy_struct(&mut t, item, "AttributeSlotData", None);
    for flag in ["bActivated", "bExternalSlot"] {
        t.prop("Bool", data, flag, 0, 0);
    }
    t.prop("Object", data, "AttributeToModify", 0, object);
    t.prop("Byte", data, "ModifierType", 0, 0);
    t.prop("Float", data, "ComputedModifierValue", 0, 0);
    let payload = property_payload(&t.none, 2, 0, &ints(&[data]));
    let mut slots = Export::new(t.import("StructProperty"), "AttributeSlots", payload);
    slots.outer = item;
    t.p.add_export_with(slots);
    let applied = toy_struct(&mut t, item, "AppliedAttributeEffect", None);
    for field in ["Context", "Attribute", "Modifier"] {
        t.prop("Object", applied, field, 0, object);
    }
    let inner = t.p.next_export();
    t.prop("Array", item, "AppliedAttributeSlotEffects", 0, inner + 1);
    t.prop("Struct", inner, "AppliedAttributeSlotEffects", 0, applied);
    let mut h = 0i32.to_le_bytes().to_vec();
    h.extend(t.p.object_tag("Attr", toy_attr));
    h.extend(t.p.object_tag("Broken", broken));
    h.extend(t.p.object_tag("Removal", removal));
    h.extend_from_slice(&t.none);
    t.p.add_export(holder, "ToyHolder", h);
    t.p.build()
}

struct Fx {
    vm: Vm,
    dir: PathBuf,
    attr: ObjRef,
    broken: ObjRef,
    removal: ObjRef,
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn fx() -> Fx {
    let dir = std::env::temp_dir().join(format!("ow-inventory-internal-slots-store-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let bytes = package();
    let defs = DefStore::from_synthetic(&dir, &[("WillowGame.upk", &bytes)], IdentityPolicy::FirstInScanOrder, ParityMode::Off).unwrap();
    let attrs = AttributeSystem::new_empty().with_value_resolver("WillowGame.ToyStackResolver", ToyStack).unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(attrs), ParityMode::Off);
    let path = ObjectPath::parse("WillowGame.ToyHolder").unwrap();
    let holder = vm.defs.find(&mut vm.space, &path).unwrap().unwrap();
    let get = |vm: &Vm, n: &str| match crate::equip::read_prop(vm, holder, n) {
        Some(Value::Object(Some(o))) => o,
        other => panic!("{other:?}"),
    };
    let (attr, broken, removal) = (get(&vm, "Attr"), get(&vm, "Broken"), get(&vm, "Removal"));
    assert!(vm.defs.is_unloaded(&vm.space, attr) && vm.defs.is_unloaded(&vm.space, broken));
    assert!(vm.defs.is_unloaded(&vm.space, removal));
    Fx { vm, dir, attr, broken, removal }
}

/// P0 loads an unloaded attribute and every entry of its resolver chains.
#[test]
fn p0_loads_the_attribute_and_its_chain_entries() {
    let mut x = fx();
    let attr = x.attr;
    load_attribute(&mut x.vm, attr, true, &mut false).unwrap();
    assert!(!x.vm.defs.is_unloaded(&x.vm.space, attr), "the attribute is loaded");
    let chain = crate::equip::read_prop(&x.vm, attr, "ValueResolverChain").unwrap();
    let entry = ow_attr::eval::object("e", &chain.elements().unwrap()[0]).unwrap().unwrap();
    assert!(!x.vm.defs.is_unloaded(&x.vm.space, entry), "its value chain's entry is loaded");
}

/// A load that fails is returned as the store gives it (typed, through `VmError::from`), with no partial-writes mark:
/// P0 runs before any write.
#[test]
fn p0_returns_a_failed_load_unchanged() {
    let mut x = fx();
    let broken = x.broken;
    let e = load_attribute(&mut x.vm, broken, true, &mut false).unwrap_err();
    assert!(
        matches!(e, ow_vm::error::VmError::Def(ow_defs::store::DefError::Decode(_))),
        "the store's own decode error, typed: {e:?}"
    );
}

// ---- lane FIX-483-FOLLOWUP, F3: P0 first, and a removal's context chain not loaded --------------------------------

/// A toy value resolver over the context's `Bonus` stack attribute (review r1 of #518): the store's storage lookup for
/// that property, as an object-property resolver gives it.
struct ToyStack;

impl ow_attr::api::ValueResolver for ToyStack {
    fn value(
        &self,
        _sys: &AttributeSystem,
        _resolver: ObjRef,
        _context: ObjRef,
        _previous: f32,
        _cx: &mut ow_attr::api::EvalCx<'_>,
    ) -> Result<f32, ow_attr::api::AttrError> {
        Ok(0.0)
    }

    fn storage(
        &self,
        _sys: &AttributeSystem,
        _resolver: ObjRef,
        context: ObjRef,
        cx: &mut ow_attr::api::EvalCx<'_>,
    ) -> Result<Option<ow_attr::api::Storage>, ow_attr::api::AttrError> {
        let space = cx.world.parts().space;
        let class = space.heap.get(context)?.class;
        match space.prop(class, "Bonus") {
            Some(p) => ow_attr::storage::storage_of(space, context, p),
            None => Ok(None),
        }
    }
}

impl Fx {
    /// A `ToyItem` and the native declared on its class, bound in Off.
    fn item_and_native(&mut self) -> (ObjRef, FuncId) {
        let class = self.vm.find_class("WillowGame.ToyItem").unwrap();
        let item = ow_vm::interp::instantiate_from_defaults(&mut self.vm, class).unwrap();
        // The canonical record struct P1 asks for (review r1 of #518), declared as the installed one is.
        let applied = match self.vm.space.classes.struct_by_path(crate::equip::APPLIED_STRUCT) {
            Some(s) => s,
            None => {
                let object = self.vm.find_class("WillowGame.Object").unwrap();
                let decls = ["Context", "Attribute", "Modifier"]
                    .map(|n| PropertyDecl::new(self.vm.space.names.intern(n), PropKind::Object { class: object }))
                    .to_vec();
                self.vm.space.classes.define_struct(crate::equip::APPLIED_STRUCT, None, decls).unwrap()
            }
        };
        let mut param = |name: &str, kind: PropKind, flags: u64| {
            PropertyDecl::new(self.vm.space.names.intern(name), kind).with_flags(PropertyDecl::PARAM | flags)
        };
        let params = vec![
            param("bBackupSlotEffectsApplied", PropKind::Bool, 0),
            param("MaxSlotsActivated", PropKind::Int, 0),
            param("AttributeModifiers", PropKind::Array(Box::new(PropKind::Struct(applied))), PropertyDecl::OUT),
        ];
        let path = ObjectPath::parse("WillowGame.ToyItem.ApplyInternalSlotEffectModifiers").unwrap();
        let decl = FunctionDecl::native(path, INTERNAL_SLOTS).with_owner(class).with_params(params);
        let f = self.vm.declare_function(decl).unwrap();
        register(&mut self.vm).unwrap();
        (item, f)
    }

    /// `owner`'s property `name`.
    fn at(&self, owner: ObjRef, name: &str) -> PropAddr {
        let class = self.vm.space.heap.get(owner).unwrap().class;
        PropAddr::scalar(self.vm.space.prop(class, name).unwrap())
    }

    /// A struct value of `ty`'s path with `fields` set.
    fn st(&self, path: &str, fields: Vec<(&str, Value)>) -> Value {
        let ty = self.vm.space.classes.struct_by_path(path).unwrap();
        let mut v = self.vm.space.classes.struct_defaults(ty).unwrap();
        for (name, x) in fields {
            let id = self.vm.space.names.find(name).unwrap();
            let (k, _) = self.vm.space.classes.struct_layout(ty).unwrap().field(id).unwrap();
            v.struct_mut().unwrap().fields[usize::try_from(k).unwrap()] = x;
        }
        v
    }

    /// `Core.AttributeModifier`, declaring `Type` and `Value` (P4), under the toy package's `Object` (the class the
    /// record fields declare).
    fn modifier_class(&mut self) -> ClassId {
        let object = self.vm.find_class("WillowGame.Object").unwrap();
        let decls = vec![
            PropertyDecl::new(self.vm.space.names.intern("Type"), PropKind::Byte { enum_: None }),
            PropertyDecl::new(self.vm.space.names.intern("Value"), PropKind::Float),
        ];
        self.vm.space.classes.define(MODIFIER_CLASS, Some(object), decls).unwrap()
    }
}

/// F3: slot 0's attribute cannot be loaded, and the world has no `Core.AttributeModifier` (P4 would refuse). P0 runs
/// before the independent checks, so the answer is the store's own typed load error, not the class lookup's.
#[test]
fn a_load_refusal_comes_before_the_modifier_class_check() {
    let mut x = fx();
    let (item, f) = x.item_and_native();
    let broken = x.broken;
    let slot = x.st(
        "WillowGame.ToyItem.AttributeSlotData",
        vec![
            ("bActivated", Value::Bool(true)),
            ("AttributeToModify", Value::Object(Some(broken))),
            ("ModifierType", Value::Byte(2)),
            ("ComputedModifierValue", Value::Float(1.0)),
        ],
    );
    let slots = x.at(item, "AttributeSlots");
    x.vm.space.write(item, slots, slot).unwrap();
    assert!(x.vm.space.classes.by_path(MODIFIER_CLASS).is_none(), "no modifier class in this world");
    let e = x.vm.call(f, Some(item), vec![Value::Bool(true), Value::Int(2), Value::array(vec![])]).unwrap_err();
    assert!(matches!(e, VmError::Def(ow_defs::store::DefError::Decode(_))), "the store's own decode error, typed: {e:?}");
}

/// F3 (item 6 of review r1 of #518): a removal-only call whose record's attribute names an unloadable context
/// resolver it does not use, and a value chain that reaches a real stack holding the record's modifier. P0 loads a
/// removal's attribute and its value chain only, so the call goes through: the modifier leaves the stack, W2 empties
/// the list, and the context resolver stays an unloaded handle.
#[test]
fn a_removal_does_not_load_its_attribute_s_context_chain() {
    let mut x = fx();
    let (_, f) = x.item_and_native();
    let modifier_class = x.modifier_class();
    let item_class = x.vm.find_class("WillowGame.ToyItem").unwrap();
    let d = |x: &mut Fx, n: &str, k: PropKind, flags: u64| PropertyDecl::new(x.vm.space.names.intern(n), k).with_flags(flags);
    let decls = vec![
        d(&mut x, "Bonus", PropKind::FloatAttribute, PropertyDecl::STACK_ATTRIBUTE),
        d(&mut x, "BonusBaseValue", PropKind::FloatAttribute, PropertyDecl::PLAIN_ATTRIBUTE),
        d(&mut x, "BonusModifierStack", PropKind::Array(Box::new(PropKind::Object { class: modifier_class })), 0),
    ];
    let stack_class = x.vm.space.classes.define("Test.StackItem", Some(item_class), decls).unwrap();
    let (v, b, st) = (x.vm.space.names.intern("Bonus"), x.vm.space.names.intern("BonusBaseValue"), x.vm.space.names.intern("BonusModifierStack"));
    x.vm.space.classes.link_attribute(stack_class, v, b, st).unwrap();
    let item = x.vm.space.alloc(stack_class, "Test.StackItem_0", None, ow_object::heap::Origin::Runtime).unwrap();
    let m = x.vm.space.alloc(modifier_class, "Test.Removed", None, ow_object::heap::Origin::Runtime).unwrap();
    x.vm.space.write(m, x.at(m, "Type"), Value::Byte(2)).unwrap();
    x.vm.space.write(m, x.at(m, "Value"), Value::Float(1.0)).unwrap();
    let stack = x.at(item, "BonusModifierStack");
    x.vm.space.write(item, stack, Value::array(vec![Value::Object(Some(m))])).unwrap();
    let removal = x.removal;
    let rec = x.st(
        "WillowGame.ToyItem.AppliedAttributeEffect",
        vec![
            ("Context", Value::Object(Some(item))),
            ("Attribute", Value::Object(Some(removal))),
            ("Modifier", Value::Object(Some(m))),
        ],
    );
    let list = x.at(item, "AppliedAttributeSlotEffects");
    x.vm.space.write(item, list, Value::array(vec![rec])).unwrap();
    x.vm.call(f, Some(item), vec![Value::Bool(false), Value::Int(0), Value::array(vec![])]).unwrap();
    assert_eq!(x.vm.space.read(item, stack).unwrap(), &Value::array(vec![]), "the removal happened");
    assert_eq!(x.vm.space.read(item, list).unwrap(), &Value::array(vec![]), "W2 emptied the list");
    assert!(!x.vm.defs.is_unloaded(&x.vm.space, removal), "the removal's attribute is loaded");
    let chain = crate::equip::read_prop(&x.vm, removal, "ContextResolverChain").unwrap();
    let entry = ow_attr::eval::object("e", &chain.elements().unwrap()[0]).unwrap().unwrap();
    assert!(x.vm.defs.is_unloaded(&x.vm.space, entry), "its context resolver is not loaded");
}

/// Item 1 of review r1 of #518 (the hub's ruling: a load is a write): P0 loads the removal's attribute `ToyAttr`
/// lazily, and it succeeds; P9 then refuses its value resolver (a class the attribute system does not know). The
/// refusal comes after the load, so it is marked, its kind kept.
#[test]
fn a_refusal_after_a_lazy_load_is_marked() {
    let mut x = fx();
    let (item, f) = x.item_and_native();
    let modifier_class = x.modifier_class();
    let m = x.vm.space.alloc(modifier_class, "Test.Removed", None, ow_object::heap::Origin::Runtime).unwrap();
    let attr = x.attr;
    let rec = x.st(
        "WillowGame.ToyItem.AppliedAttributeEffect",
        vec![("Context", Value::Object(Some(item))), ("Attribute", Value::Object(Some(attr))), ("Modifier", Value::Object(Some(m)))],
    );
    let list = x.at(item, "AppliedAttributeSlotEffects");
    x.vm.space.write(item, list, Value::array(vec![rec.clone()])).unwrap();
    let e = x.vm.call(f, Some(item), vec![Value::Bool(false), Value::Int(0), Value::array(vec![])]).unwrap_err();
    let VmError::NotYet(n) = &e else { panic!("{e:?}") };
    assert!(n.what.starts_with("stopped after partial writes: "), "{n}");
    assert!(!x.vm.defs.is_unloaded(&x.vm.space, attr), "the load happened");
    assert_eq!(x.vm.space.read(item, list).unwrap(), &Value::array(vec![rec]), "W2 not reached");
}

/// Item 5 of review r1 of #518: slot 0's attribute cannot be loaded and its `ModifierType` is outside the enum. The type
/// is checked after P0, so the answer is the store's own load error.
#[test]
fn a_load_refusal_comes_before_the_modifier_type_check() {
    let mut x = fx();
    let (item, f) = x.item_and_native();
    x.modifier_class();
    let broken = x.broken;
    let slot = x.st(
        "WillowGame.ToyItem.AttributeSlotData",
        vec![
            ("bActivated", Value::Bool(true)),
            ("AttributeToModify", Value::Object(Some(broken))),
            ("ModifierType", Value::Byte(9)),
            ("ComputedModifierValue", Value::Float(1.0)),
        ],
    );
    let slots = x.at(item, "AttributeSlots");
    x.vm.space.write(item, slots, slot).unwrap();
    let e = x.vm.call(f, Some(item), vec![Value::Bool(true), Value::Int(2), Value::array(vec![])]).unwrap_err();
    assert!(matches!(e, VmError::Def(ow_defs::store::DefError::Decode(_))), "the store's own decode error: {e:?}");
}
