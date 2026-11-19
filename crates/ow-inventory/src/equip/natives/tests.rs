//! Tests of the Off attribute and slot natives on the invented world of `crate::equip::tests` (AGENTS.md rule 9:
//! invented classes, objects and values; the class and property names are the ones the natives read).

use ow_core::ParityMode;
use ow_object::heap::Origin;
use ow_object::ids::{ClassId, FuncId, ObjRef};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_vm::api::FunctionDecl;

use super::*;
use crate::equip::tests::World;
use crate::equip::{APPLIED_STRUCT, applied_from_values, applied_values};

fn param(w: &mut World, name: &str, kind: PropKind, flags: u64) -> PropertyDecl {
    PropertyDecl::new(w.vm.space.names.intern(name), kind).with_flags(PropertyDecl::PARAM | flags)
}

/// Declares the native `path` with `key` and `params` on `owner`, so a call runs what is registered under `key`.
fn native_fn(w: &mut World, path: &str, key: &str, owner: ClassId, params: Vec<PropertyDecl>) -> FuncId {
    let path = ow_core::ObjectPath::parse(path).unwrap();
    w.vm
        .declare_function(FunctionDecl::native(path, key).with_owner(owner).with_params(params))
        .unwrap()
}

/// The three attribute natives, declared on `Engine.AttributeDefinitionBase` as the loader declares them.
fn attribute_natives(w: &mut World) -> (FuncId, FuncId, FuncId) {
    let base = w.class("Engine.AttributeDefinitionBase");
    let object = w.class("Core.Object");
    let obj = PropKind::Object { class: object };
    let source = param(w, "ContextSource", obj.clone(), 0);
    let resolve = native_fn(w, "Engine.AttributeDefinitionBase.ResolveContext", RESOLVE_CONTEXT, base, vec![source]);
    let change = |w: &mut World, name: &str, key: &str| {
        let params = vec![
            param(w, "Context", obj.clone(), 0),
            param(w, "Modifier", obj.clone(), 0),
            param(w, "bSuppressNotify", PropKind::Bool, PropertyDecl::OPTIONAL),
        ];
        native_fn(w, &format!("Engine.AttributeDefinitionBase.{name}"), key, base, params)
    };
    let add = change(w, "AddAttributeModifier", ADD_MODIFIER);
    let remove = change(w, "RemoveAttributeModifier", REMOVE_MODIFIER);
    (resolve, add, remove)
}

fn truth(v: Value) -> bool {
    match v {
        Value::Bool(b) => b,
        other => panic!("{other:?}"),
    }
}

fn modifier(w: &mut World, kind: u8, value: f32) -> ObjRef {
    let m = w.obj("Core.AttributeModifier", "Test.Modifier");
    w.set(m, "Type", Value::Byte(kind));
    w.set(m, "Value", Value::Float(value));
    m
}

#[test]
fn the_attribute_natives_resolve_add_and_remove_through_ow_attr() {
    let mut w = World::new();
    register(&mut w.vm).unwrap();
    let (resolve, add, remove) = attribute_natives(&mut w);
    let (a, controller) = (w.controller_bonus, w.controller);
    let r = w.vm.call(resolve, Some(a), vec![Value::Object(Some(controller))]).unwrap();
    assert_eq!(r.ret, Value::Object(Some(controller)), "the controller resolver: a controller is itself");
    let r = w.vm.call(resolve, Some(a), vec![Value::Object(None)]).unwrap();
    assert_eq!(r.ret, Value::Object(None), "a None source resolves to None");
    let m = modifier(&mut w, 2, 3.0);
    let args = |m: Option<ObjRef>| vec![Value::Object(Some(controller)), Value::Object(m)];
    assert!(truth(w.vm.call(add, Some(a), args(Some(m))).unwrap().ret));
    assert_eq!(w.float(controller, "Bonus"), 3.0, "PostAdd 3 on a 0 base");
    assert!(!truth(w.vm.call(add, Some(a), args(Some(m))).unwrap().ret), "a second add of one modifier is refused");
    assert!(truth(w.vm.call(remove, Some(a), args(Some(m))).unwrap().ret));
    assert_eq!(w.float(controller, "Bonus"), 0.0);
    assert!(!truth(w.vm.call(remove, Some(a), args(Some(m))).unwrap().ret), "nothing left to remove");
    assert!(!truth(w.vm.call(add, Some(a), args(None)).unwrap().ret), "a None modifier");
    let none_context = vec![Value::Object(None), Value::Object(Some(m))];
    assert!(!truth(w.vm.call(add, Some(a), none_context).unwrap().ret), "a None context");
}

#[test]
fn off_binds_every_key_and_reference_binds_none() {
    let mut w = World::new();
    register(&mut w.vm).unwrap();
    let keys = [RESOLVE_CONTEXT, ADD_MODIFIER, REMOVE_MODIFIER, SLOT_MAX, EXTERNAL_SLOTS];
    assert!(keys.iter().all(|k| w.vm.has_native(k)), "Off binds all five");
    // Reference: the C++ VM binds none of them, so register leaves them to the stub policy (R3). Only the parity
    // field is switched (the world's VM is built in Off); register reads nothing else.
    let mut w = World::new();
    w.vm.parity = ParityMode::Reference;
    register(&mut w.vm).unwrap();
    assert!(keys.iter().all(|k| !w.vm.has_native(k)), "Reference binds none");
}

#[test]
fn the_slot_count_is_the_stored_one_inside_one_to_nineteen_else_nineteen() {
    for (stored, answer) in [(1, 1), (7, 7), (19, 19), (0, 19), (20, 19), (-4, 19)] {
        assert_eq!(clamped_slot_max(stored), answer, "{stored}");
    }
}

/// The slot classes the natives read: an item class with `AttributeSlots` and a definition class with
/// `AttributeSlotMaxActivated` (invented here; the field names are the installed ones).
fn slot_classes(w: &mut World) -> (ClassId, ClassId) {
    let object = w.class("Core.Object");
    let obj = PropKind::Object { class: object };
    let fields = [
        ("bActivated", PropKind::Bool),
        ("bExternalSlot", PropKind::Bool),
        ("bRunEffectsAsSkill", PropKind::Bool),
        ("AttributeToModify", obj),
        ("ModifierType", PropKind::Byte { enum_: None }),
        ("ComputedModifierValue", PropKind::Float),
    ];
    let decls = fields
        .into_iter()
        .map(|(n, k)| PropertyDecl::new(w.vm.space.names.intern(n), k))
        .collect();
    let slot = w
        .vm
        .space
        .classes
        .define_struct("Test.AttributeSlotData", None, decls)
        .unwrap();
    let slots = PropertyDecl::new(
        w.vm.space.names.intern("AttributeSlots"),
        PropKind::Array(Box::new(PropKind::Struct(slot))),
    );
    let inventory = w.class("Engine.WillowInventory");
    let item = w.vm.space.classes.define("Test.SlotItem", Some(inventory), vec![slots]).unwrap();
    let max = PropertyDecl::new(w.vm.space.names.intern("AttributeSlotMaxActivated"), PropKind::Int);
    let def = w
        .vm
        .space
        .classes
        .define("WillowGame.WillowInventoryDefinition", Some(object), vec![max])
        .unwrap();
    (item, def)
}

fn slot(w: &mut World, activated: bool, external: bool, skill: bool, attribute: Option<ObjRef>, value: f32) -> Value {
    w.st(
        "Test.AttributeSlotData",
        vec![
            ("bActivated", Value::Bool(activated)),
            ("bExternalSlot", Value::Bool(external)),
            ("bRunEffectsAsSkill", Value::Bool(skill)),
            ("AttributeToModify", Value::Object(attribute)),
            ("ModifierType", Value::Byte(2)),
            ("ComputedModifierValue", Value::Float(value)),
        ],
    )
}

/// `ApplyExternalSlotEffectModifiers` declared on `item_class` as the loader declares it.
fn external_slots_fn(w: &mut World, item_class: ClassId) -> FuncId {
    let object = w.class("Core.Object");
    let applied = w.vm.space.classes.struct_by_path(APPLIED_STRUCT).unwrap();
    let params = vec![
        param(w, "ContextSource", PropKind::Object { class: object }, 0),
        param(w, "MaxSlotsActivated", PropKind::Int, 0),
        param(
            w,
            "AttributeModifiers",
            PropKind::Array(Box::new(PropKind::Struct(applied))),
            PropertyDecl::OUT,
        ),
        param(w, "OverrideContextSource", PropKind::Object { class: object }, PropertyDecl::OPTIONAL),
    ];
    native_fn(w, "Engine.WillowInventory.ApplyExternalSlotEffectModifiers", EXTERNAL_SLOTS, item_class, params)
}

#[test]
fn only_activated_external_non_skill_slots_with_an_attribute_apply_within_the_count() {
    let mut w = World::new();
    register(&mut w.vm).unwrap();
    let (item_class, _) = slot_classes(&mut w);
    let f = external_slots_fn(&mut w, item_class);
    let a = w.controller_bonus;
    let slots = vec![
        slot(&mut w, true, true, false, Some(a), 2.5),
        slot(&mut w, true, false, false, Some(a), 100.0),
        slot(&mut w, true, true, true, Some(a), 200.0),
        slot(&mut w, false, true, false, Some(a), 300.0),
        slot(&mut w, true, true, false, None, 400.0),
        slot(&mut w, true, true, false, Some(a), 0.5),
        slot(&mut w, true, true, false, Some(a), 500.0),
    ];
    let item = w.vm.space.alloc(item_class, "Test.Item", None, Origin::Runtime).unwrap();
    w.set(item, "AttributeSlots", Value::array(slots));
    let ty = w.vm.space.classes.struct_by_path(APPLIED_STRUCT).unwrap();
    let earlier = ow_attr::api::AppliedEffect {
        context: w.pawn,
        attribute: a,
        modifier: w.obj("Core.AttributeModifier", "Earlier"),
    };
    let existing = Value::array(applied_values(&w.vm, ty, &ow_attr::api::Applied(vec![earlier])).unwrap());
    let controller = w.controller;
    let r = w
        .vm
        .call(f, Some(item), vec![Value::Object(Some(controller)), Value::Int(6), existing])
        .unwrap();
    let out = applied_from_values(&w.vm, &r.outs[2]).unwrap();
    assert_eq!(out.0.len(), 3, "the earlier record, then the first and the sixth slot; the seventh is past the count");
    assert_eq!(out.0[0], earlier);
    assert_eq!((out.0[1].context, out.0[2].context), (controller, controller));
    assert_eq!(w.float(controller, "Bonus"), 3.0, "PostAdd 2.5 and 0.5, the stored values");
    let r = w
        .vm
        .call(f, Some(item), vec![Value::Object(Some(controller)), Value::Int(0), Value::array(vec![])])
        .unwrap();
    assert_eq!(r.outs[2], Value::array(vec![]), "a count of 0 considers no slot");
}

#[test]
fn the_slot_context_is_the_override_s_first() {
    let mut w = World::new();
    register(&mut w.vm).unwrap();
    let (item_class, _) = slot_classes(&mut w);
    let f = external_slots_fn(&mut w, item_class);
    let a = w.weapon_bonus;
    let s = slot(&mut w, true, true, false, Some(a), 4.0);
    let item = w.vm.space.alloc(item_class, "Test.Item", None, Origin::Runtime).unwrap();
    w.set(item, "AttributeSlots", Value::array(vec![s]));
    let weapon = w.weapon(None, &[]);
    let controller = w.controller;
    let args = vec![
        Value::Object(Some(controller)),
        Value::Int(19),
        Value::array(vec![]),
        Value::Object(Some(weapon)),
    ];
    let r = w.vm.call(f, Some(item), args).unwrap();
    let out = applied_from_values(&w.vm, &r.outs[2]).unwrap();
    assert_eq!(out.0.len(), 1);
    assert_eq!(out.0[0].context, weapon, "the weapon resolver on the override: the weapon itself");
    assert_eq!(w.float(weapon, "Bonus"), 4.0);
}

#[test]
fn the_slot_count_native_reads_the_definition() {
    let mut w = World::new();
    register(&mut w.vm).unwrap();
    let (_, def_class) = slot_classes(&mut w);
    let f = native_fn(
        &mut w,
        "WillowGame.WillowInventoryDefinition.GetAttributeSlotMaxActivated",
        SLOT_MAX,
        def_class,
        vec![],
    );
    let def = w.vm.space.alloc(def_class, "Test.Def", None, Origin::Runtime).unwrap();
    for (stored, answer) in [(5, 5), (0, 19), (25, 19)] {
        w.set(def, "AttributeSlotMaxActivated", Value::Int(stored));
        assert_eq!(w.vm.call(f, Some(def), vec![]).unwrap().ret, Value::Int(answer), "{stored}");
    }
}
