//! Tests of the Off attribute and slot natives on the invented world of `crate::equip::tests` (AGENTS.md rule 9:
//! invented classes, objects and values; the class and property names are the ones the natives read).

use ow_core::ParityMode;
use ow_object::heap::Origin;
use ow_object::ids::{ClassId, FuncId, ObjRef};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_vm::api::{Arg, FunctionDecl};

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

/// The fixture's slot count (the installed class has 19; any count the declaration gives is read).
const SLOTS: i32 = 7;

/// Writes `slots` into `item`'s static `AttributeSlots`, element by element from index 0.
fn set_slots(w: &mut World, item: ObjRef, slots: Vec<Value>) {
    let class = w.vm.space.heap.get(item).unwrap().class;
    let p = w.vm.space.prop(class, "AttributeSlots").unwrap();
    for (i, v) in slots.into_iter().enumerate() {
        let at = ow_object::ids::PropAddr::element(p, u32::try_from(i).unwrap());
        w.vm.space.write(item, at, v).unwrap();
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
    // A static array of slot structs, as the installed `WillowInventory` declares `AttributeSlots[19]` (seven here).
    let slots = PropertyDecl::new(w.vm.space.names.intern("AttributeSlots"), PropKind::Struct(slot))
        .with_array_dim(SLOTS);
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
    set_slots(&mut w, item, slots);
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
fn every_declared_slot_is_read_and_a_dynamic_array_is_refused() {
    let mut w = World::new();
    let (item_class, _) = slot_classes(&mut w);
    let a = w.controller_bonus;
    let item = w.vm.space.alloc(item_class, "Test.Item", None, Origin::Runtime).unwrap();
    // Only the last declared element is activated: it is read, so the whole declared dimension is walked.
    let last = slot(&mut w, true, true, false, Some(a), 1.5);
    let mut slots = vec![slot(&mut w, false, false, false, None, 0.0); usize::try_from(SLOTS - 1).unwrap()];
    slots.push(last);
    set_slots(&mut w, item, slots);
    assert_eq!(attribute_slots(&w.vm, item, SLOTS).unwrap().len(), usize::try_from(SLOTS).unwrap());
    assert_eq!(external_slot_effects(&w.vm, item, SLOTS).unwrap().len(), 1);
    assert!(external_slot_effects(&w.vm, item, SLOTS - 1).unwrap().is_empty(), "past the count");
    // A class that declares the slots as a dynamic array is not the installed shape: a data error.
    let slot_ty = w.vm.space.classes.struct_by_path("Test.AttributeSlotData").unwrap();
    let dynamic = PropertyDecl::new(
        w.vm.space.names.intern("AttributeSlots"),
        PropKind::Array(Box::new(PropKind::Struct(slot_ty))),
    );
    let object = w.class("Core.Object");
    let other = w.vm.space.classes.define("Test.DynamicSlotItem", Some(object), vec![dynamic]).unwrap();
    let odd = w.vm.space.alloc(other, "Test.Odd", None, Origin::Runtime).unwrap();
    let e = external_slot_effects(&w.vm, odd, SLOTS).unwrap_err();
    assert!(e.to_string().contains("other than slot structs"), "{e}");
}

#[test]
fn no_slot_past_the_count_is_read_but_the_declaration_is_checked_first() {
    let mut w = World::new();
    let (item_class, _) = slot_classes(&mut w);
    let a = w.controller_bonus;
    let item = w.vm.space.alloc(item_class, "Test.Item", None, Origin::Runtime).unwrap();
    let first = slot(&mut w, true, true, false, Some(a), 1.5);
    set_slots(&mut w, item, vec![first]);
    // Cut the object's slots just after `AttributeSlots[0]`: every later element is missing from the heap.
    let p = w.vm.space.prop(item_class, "AttributeSlots").unwrap();
    let at = w.vm.space.classes.resolve(item_class, ow_object::ids::PropAddr::element(p, 0)).unwrap();
    w.vm.space.heap.get_mut(item).unwrap().slots.truncate(at + 1);
    assert_eq!(external_slot_effects(&w.vm, item, 1).unwrap().len(), 1, "slot 0 only, slot 1 is never read");
    assert!(external_slot_effects(&w.vm, item, 0).unwrap().is_empty(), "a count of 0 reads no slot");
    assert!(external_slot_effects(&w.vm, item, -3).unwrap().is_empty(), "nor does a negative count");
    assert!(external_slot_effects(&w.vm, item, 2).is_err(), "within the count, a missing slot is an error");
}

#[test]
fn a_struct_without_the_slot_fields_is_refused_whatever_the_count() {
    let mut w = World::new();
    // A static array of a struct that holds only `bActivated`: its false defaults would skip every slot.
    let only = PropertyDecl::new(w.vm.space.names.intern("bActivated"), PropKind::Bool);
    let ty = w.vm.space.classes.define_struct("Test.NotASlot", None, vec![only]).unwrap();
    let slots = PropertyDecl::new(w.vm.space.names.intern("AttributeSlots"), PropKind::Struct(ty)).with_array_dim(SLOTS);
    let object = w.class("Core.Object");
    let class = w.vm.space.classes.define("Test.OddSlotItem", Some(object), vec![slots]).unwrap();
    let item = w.vm.space.alloc(class, "Test.Odd", None, Origin::Runtime).unwrap();
    for max in [SLOTS, 0] {
        let e = external_slot_effects(&w.vm, item, max).unwrap_err();
        assert!(e.to_string().contains("without bExternalSlot as a bool"), "{e}");
    }
    // A slot field declared as the wrong kind is refused too.
    let fields = [
        ("bActivated", PropKind::Bool),
        ("bExternalSlot", PropKind::Bool),
        ("bRunEffectsAsSkill", PropKind::Bool),
        ("AttributeToModify", PropKind::Object { class: object }),
        ("ModifierType", PropKind::Int),
        ("ComputedModifierValue", PropKind::Float),
    ];
    let decls = fields.into_iter().map(|(n, k)| PropertyDecl::new(w.vm.space.names.intern(n), k)).collect();
    let ty = w.vm.space.classes.define_struct("Test.IntModifierSlot", None, decls).unwrap();
    let slots = PropertyDecl::new(w.vm.space.names.intern("AttributeSlots"), PropKind::Struct(ty)).with_array_dim(SLOTS);
    let class = w.vm.space.classes.define("Test.IntSlotItem", Some(object), vec![slots]).unwrap();
    let item = w.vm.space.alloc(class, "Test.IntOdd", None, Origin::Runtime).unwrap();
    for count in [SLOTS, 0] {
        let e = external_slot_effects(&w.vm, item, count).unwrap_err();
        assert!(e.to_string().contains("without ModifierType as a byte"), "count {count}: {e}");
    }
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
    set_slots(&mut w, item, vec![s]);
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

// ------------------------------------------------------------------------------------------ AllParts (FIX-ALLPARTS)

/// `AllParts(DefinitionData, out Part, bIncludeNameParts)` declared on the weapon or item class, as the loader declares
/// it, under the key `key`.
fn all_parts_fn(w: &mut World, path: &str, key: &str, data_struct: &str) -> FuncId {
    let object = w.class("Core.Object");
    let owner = w.class("Engine.Actor");
    let data = w.vm.space.classes.struct_by_path(data_struct).unwrap();
    let params = vec![
        param(w, "DefinitionData", PropKind::Struct(data), 0),
        param(w, "Part", PropKind::Object { class: object }, PropertyDecl::OUT),
        param(w, "bIncludeNameParts", PropKind::Bool, 0),
    ];
    native_fn(w, path, key, owner, params)
}

/// The parts the bound iterator of `f` yields for `data` and `flag`.
fn iterate(w: &mut World, f: FuncId, data: Value, flag: Value) -> Result<Vec<Value>, VmError> {
    let it = w.vm.iterator_for(f).expect("AllParts is bound");
    let args = vec![Arg::new(data, false, true), Arg::new(Value::None, true, true), Arg::new(flag, false, true)];
    let mut call = NativeCall::new(&mut w.vm, f, None, args);
    let out = it(&mut call)?;
    assert_eq!(out.out_param, 1, "the out argument is the part");
    Ok(out.items)
}

const WEAPON_KEY: &str = "WillowWeapon.AllParts(weapondefinitiondata,object,bool)";
const ITEM_KEY: &str = "WillowItem.AllParts(itemdefinitiondata,object,bool)";

/// NATIVE_EQUIP_PARTS.md section 2: the set slots in declared order, None skipped, the two name parts only with the
/// flag (the material slot either way), a part in two slots visited twice, nothing for an all-None definition.
#[test]
fn all_parts_visits_the_set_slots_in_order_and_the_name_parts_with_the_flag() {
    let mut w = World::new();
    let f = all_parts_fn(&mut w, ALL_PARTS_WEAPON, WEAPON_KEY, "Test.WeaponDefinitionData");
    register(&mut w.vm).unwrap();
    let [body, barrel, material, prefix, title] =
        ["Body", "Barrel", "Material", "Prefix", "Title"].map(|n| w.obj("Test.Definition", &format!("Test.{n}")));
    let o = |x: ObjRef| Value::Object(Some(x));
    let data = w.st(
        "Test.WeaponDefinitionData",
        vec![
            ("BodyPartDefinition", o(body)),
            ("BarrelPartDefinition", o(barrel)),
            // A part in two slots: visited for each.
            ("Accessory2PartDefinition", o(barrel)),
            ("MaterialPartDefinition", o(material)),
            ("PrefixPartDefinition", o(prefix)),
            ("TitlePartDefinition", o(title)),
        ],
    );
    let without = iterate(&mut w, f, data.clone(), Value::Bool(false)).unwrap();
    assert_eq!(without, vec![o(body), o(barrel), o(barrel), o(material)]);
    let with = iterate(&mut w, f, data, Value::Bool(true)).unwrap();
    assert_eq!(with, vec![o(body), o(barrel), o(barrel), o(material), o(prefix), o(title)]);
    let empty = w.st("Test.WeaponDefinitionData", vec![]);
    assert!(iterate(&mut w, f, empty, Value::Bool(true)).unwrap().is_empty());
    // The first nine slots are the equip's WEAPON_PART_FIELDS (the note's check).
    assert_eq!(WEAPON_ALL_PARTS[..9], crate::equip::WEAPON_PART_FIELDS);
    assert_eq!(ITEM_ALL_PARTS[..9], crate::equip::ITEM_PART_FIELDS);
}

/// The item's declaration gets the same rule over `ItemDefinitionData`'s slots.
#[test]
fn all_parts_on_an_item_walks_the_item_slots() {
    let mut w = World::new();
    let f = all_parts_fn(&mut w, ALL_PARTS_ITEM, ITEM_KEY, "Test.ItemDefinitionData");
    register(&mut w.vm).unwrap();
    let [alpha, theta, title] = ["Alpha", "Theta", "Title"].map(|n| w.obj("Test.Definition", &format!("Test.{n}")));
    let o = |x: ObjRef| Value::Object(Some(x));
    let data = w.st(
        "Test.ItemDefinitionData",
        vec![
            ("TitleItemNamePartDefinition", o(title)),
            ("ThetaItemPartDefinition", o(theta)),
            ("AlphaItemPartDefinition", o(alpha)),
        ],
    );
    assert_eq!(iterate(&mut w, f, data.clone(), Value::Bool(false)).unwrap(), vec![o(alpha), o(theta)]);
    assert_eq!(iterate(&mut w, f, data, Value::Bool(true)).unwrap(), vec![o(alpha), o(theta), o(title)]);
}

/// A slot holding something other than an object is a typed error, never a skipped slot.
#[test]
fn all_parts_refuses_a_slot_that_is_not_an_object() {
    let mut w = World::new();
    let f = all_parts_fn(&mut w, ALL_PARTS_WEAPON, WEAPON_KEY, "Test.WeaponDefinitionData");
    register(&mut w.vm).unwrap();
    let data = w.st("Test.WeaponDefinitionData", vec![("GripPartDefinition", Value::Int(3))]);
    let e = iterate(&mut w, f, data, Value::Bool(false)).unwrap_err();
    assert!(e.to_string().contains("GripPartDefinition"), "{e}");
}

/// Reference: the C++ VM binds no AllParts, so the iterator stays unbound there (R3).
#[test]
fn all_parts_is_bound_in_off_only() {
    let mut w = World::new();
    w.vm.parity = ParityMode::Reference;
    let f = all_parts_fn(&mut w, ALL_PARTS_WEAPON, WEAPON_KEY, "Test.WeaponDefinitionData");
    register(&mut w.vm).unwrap();
    assert!(w.vm.iterator_for(f).is_none());
}
