//! `ApplyInternalSlotEffectModifiers` on the invented world of `crate::equip::tests` (AGENTS.md rule 9; decision
//! 2026-11-19-97). Expected values are written by hand from NATIVE_APPLY_SLOT_EFFECT_MODIFIERS.md. The item is a weapon
//! subclass, so the weapon context resolver gives the item itself and its `Bonus` stack attribute shows the adds.
//! AI-assisted (Claude Code).

use ow_core::Missing;
use ow_object::heap::Origin;
use ow_object::ids::{ClassId, FuncId, ObjRef, PropAddr};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_vm::api::FunctionDecl;
use ow_vm::error::VmError;

use super::*;
use crate::equip::tests::World;
use crate::equip::{APPLIED_STRUCT, applied_from_values, applied_values};

const SLOTS: i32 = 5;

fn param(w: &mut World, name: &str, kind: PropKind, flags: u64) -> PropertyDecl {
    PropertyDecl::new(w.vm.space.names.intern(name), kind).with_flags(PropertyDecl::PARAM | flags)
}

/// The item class (`AttributeSlots[5]`, `AppliedAttributeSlotEffects`) and the native declared on it.
fn setup(w: &mut World) -> (ClassId, FuncId) {
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
    let decls = fields.into_iter().map(|(n, k)| PropertyDecl::new(w.vm.space.names.intern(n), k)).collect();
    let slot = w.vm.space.classes.define_struct("Test.InternalSlotData", None, decls).unwrap();
    let applied = w.vm.space.classes.struct_by_path(APPLIED_STRUCT).unwrap();
    let records = PropKind::Array(Box::new(PropKind::Struct(applied)));
    let decls = vec![
        PropertyDecl::new(w.vm.space.names.intern("AttributeSlots"), PropKind::Struct(slot)).with_array_dim(SLOTS),
        PropertyDecl::new(w.vm.space.names.intern("AppliedAttributeSlotEffects"), records.clone()),
    ];
    let weapon = w.class("WillowGame.WillowWeapon");
    let item = w.vm.space.classes.define("Test.SlotWeapon", Some(weapon), decls).unwrap();
    let params = vec![
        param(w, "bBackupSlotEffectsApplied", PropKind::Bool, 0),
        param(w, "MaxSlotsActivated", PropKind::Int, 0),
        param(w, "AttributeModifiers", records, PropertyDecl::OUT),
    ];
    let path = ow_core::ObjectPath::parse("Engine.WillowInventory.ApplyInternalSlotEffectModifiers").unwrap();
    let f = w.vm.declare_function(FunctionDecl::native(path, INTERNAL_SLOTS).with_owner(item).with_params(params)).unwrap();
    register(&mut w.vm).unwrap();
    (item, f)
}

fn slot(w: &mut World, activated: bool, external: bool, skill: bool, attribute: Option<ObjRef>, value: f32) -> Value {
    w.st(
        "Test.InternalSlotData",
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

fn item_with(w: &mut World, class: ClassId, slots: Vec<Value>) -> ObjRef {
    let item = w.vm.space.alloc(class, "Test.Item", None, Origin::Runtime).unwrap();
    let p = w.vm.space.prop(class, "AttributeSlots").unwrap();
    for (i, v) in slots.into_iter().enumerate() {
        w.vm.space.write(item, PropAddr::element(p, u32::try_from(i).unwrap()), v).unwrap();
    }
    item
}

fn call(w: &mut World, f: FuncId, item: ObjRef, flag: bool, max: i32, out: Value) -> Result<Value, VmError> {
    Ok(w.vm.call(f, Some(item), vec![Value::Bool(flag), Value::Int(max), out])?.outs[2].clone())
}

/// The five slots of the main case: internal (applied), external (skipped), internal run as a skill (applied: the
/// flag is not read), not activated (skipped), internal with no attribute (skipped).
fn five(w: &mut World) -> Vec<Value> {
    let a = w.weapon_bonus;
    vec![
        slot(w, true, false, false, Some(a), 2.5),
        slot(w, true, true, false, Some(a), 100.0),
        slot(w, true, false, true, Some(a), 0.5),
        slot(w, false, false, false, Some(a), 300.0),
        slot(w, true, false, false, None, 400.0),
    ]
}

/// With the flag true: each qualifying slot's modifier is added to the item, a record appended after what the out
/// array held, and the same records kept in `AppliedAttributeSlotEffects`.
#[test]
fn the_internal_slots_are_applied_and_backed_up_with_the_flag() {
    let mut w = World::new();
    let (class, f) = setup(&mut w);
    let slots = five(&mut w);
    let item = item_with(&mut w, class, slots);
    let ty = w.vm.space.classes.struct_by_path(APPLIED_STRUCT).unwrap();
    let a = w.weapon_bonus;
    let earlier = ow_attr::api::AppliedEffect { context: w.pawn, attribute: a, modifier: w.obj("Core.AttributeModifier", "Earlier") };
    let existing = Value::array(applied_values(&w.vm, ty, &ow_attr::api::Applied(vec![earlier])).unwrap());
    let raw = call(&mut w, f, item, true, SLOTS, existing).unwrap();
    let out = applied_from_values(&w.vm, &raw).unwrap();
    assert_eq!(out.0.len(), 3, "the earlier record, then slots 0 and 2");
    assert_eq!(out.0[0], earlier);
    assert_eq!((out.0[1].context, out.0[2].context), (item, item), "the context resolved from the item");
    assert_eq!(w.float(item, "Bonus"), 3.0, "PostAdd 2.5 and 0.5, the stored values");
    let backup = applied_from_values(&w.vm, &w.get(item, "AppliedAttributeSlotEffects")).unwrap();
    assert_eq!(backup.0, out.0[1..].to_vec(), "the backup list holds the new records only");
}

/// The second call, with the flag false: the backup list's modifiers are removed first, the list is emptied, and the
/// slots are applied again without being backed up; the out array keeps the first call's records.
#[test]
fn the_flag_false_removes_the_backed_up_modifiers_then_applies_again() {
    let mut w = World::new();
    let (class, f) = setup(&mut w);
    let slots = five(&mut w);
    let item = item_with(&mut w, class, slots);
    let first = call(&mut w, f, item, true, SLOTS, Value::array(vec![])).unwrap();
    let raw = call(&mut w, f, item, false, SLOTS, first).unwrap();
    let second = applied_from_values(&w.vm, &raw).unwrap();
    assert_eq!(second.0.len(), 4, "the first call's two records, then the second call's two");
    assert_eq!(w.float(item, "Bonus"), 3.0, "the first modifiers removed, the second added: not doubled");
    assert_eq!(w.get(item, "AppliedAttributeSlotEffects"), Value::array(vec![]), "emptied and not refilled");
}

/// With the flag true twice, the first call's modifiers are forgotten without being removed (the note's correction to
/// NATIVE_ATTRIBUTES.md section 9): the effects double.
#[test]
fn the_flag_true_forgets_the_backup_without_removing_it() {
    let mut w = World::new();
    let (class, f) = setup(&mut w);
    let slots = five(&mut w);
    let item = item_with(&mut w, class, slots);
    call(&mut w, f, item, true, SLOTS, Value::array(vec![])).unwrap();
    call(&mut w, f, item, true, SLOTS, Value::array(vec![])).unwrap();
    assert_eq!(w.float(item, "Bonus"), 6.0);
    let backup = applied_from_values(&w.vm, &w.get(item, "AppliedAttributeSlotEffects")).unwrap();
    assert_eq!(backup.0.len(), 2, "only the second call's records");
}

/// The count: 0 or less applies nothing but still empties the list; a count past the dimension stops at it (B1).
#[test]
fn the_count_bounds_the_slots_visited() {
    let mut w = World::new();
    let (class, f) = setup(&mut w);
    let slots = five(&mut w);
    let item = item_with(&mut w, class, slots);
    call(&mut w, f, item, true, SLOTS, Value::array(vec![])).unwrap();
    assert_eq!(call(&mut w, f, item, true, 0, Value::array(vec![])).unwrap(), Value::array(vec![]));
    assert_eq!(w.get(item, "AppliedAttributeSlotEffects"), Value::array(vec![]), "emptied with a count of 0");
    let raw = call(&mut w, f, item, false, 99, Value::array(vec![])).unwrap();
    let out = applied_from_values(&w.vm, &raw).unwrap();
    assert_eq!(out.0.len(), 2, "a count of 99 reads the five declared slots");
}

/// A slot whose attribute's context resolver is not read: the resolver's own `NotYet`, unchanged, before any write. The
/// backed-up modifier is not removed and the list keeps its record.
#[test]
fn a_context_that_cannot_be_resolved_refuses_before_any_write() {
    let mut w = World::new();
    let (class, f) = setup(&mut w);
    let slots = five(&mut w);
    let item = item_with(&mut w, class, slots);
    call(&mut w, f, item, true, SLOTS, Value::array(vec![])).unwrap();
    let unread = w.unread_attribute();
    let bad_slot = slot(&mut w, true, false, false, Some(unread), 9.0);
    let p = w.vm.space.prop(class, "AttributeSlots").unwrap();
    w.vm.space.write(item, PropAddr::element(p, 3), bad_slot).unwrap();
    let before = (w.float(item, "Bonus"), w.get(item, "AppliedAttributeSlotEffects"));
    let e = call(&mut w, f, item, false, SLOTS, Value::array(vec![])).unwrap_err();
    let VmError::NotYet(n) = &e else { panic!("{e:?}") };
    assert_eq!(n.missing, Missing::NotRead);
    assert!(!n.what.contains("partial writes"), "{n}");
    assert_eq!((w.float(item, "Bonus"), w.get(item, "AppliedAttributeSlotEffects")), before, "nothing written");
}

/// Reference binds nothing (architecture R3).
#[test]
fn reference_binds_nothing() {
    let mut w = World::new();
    w.vm.parity = ow_core::ParityMode::Reference;
    register(&mut w.vm).unwrap();
    assert!(!w.vm.has_native(INTERNAL_SLOTS));
}

/// An attribute on the item's `Bonus` whose value chain is `entries` (each a resolver class path, or None).
fn chained(w: &mut World, name: &str, entries: &[Option<&str>]) -> ObjRef {
    let c = w.obj("Engine.WeaponAttributeContextResolver", &format!("{name}.Context"));
    let mut chain = Vec::new();
    for (i, e) in entries.iter().enumerate() {
        chain.push(match e {
            Some(class) => {
                let r = w.obj(class, &format!("{name}.Value{i}"));
                if *class == "Engine.ObjectPropertyAttributeValueResolver" {
                    let n = Value::Name(w.vm.space.names.intern("Bonus"));
                    w.set(r, "PropertyName", n);
                }
                Value::Object(Some(r))
            }
            None => Value::Object(None),
        });
    }
    let a = w.obj("Engine.AttributeDefinition", name);
    w.set(a, "ContextResolverChain", Value::array(vec![Value::Object(Some(c))]));
    w.set(a, "ValueResolverChain", Value::array(chain));
    a
}

/// P9 checks every entry of a planned add's chain, whether or not the add will reach it (the final note, review r1 of
/// #471): a writable entry, then one whose resolver class is not read, is that class's `NotYet` before any write.
#[test]
fn every_chain_entry_is_checked_before_any_write() {
    let mut w = World::new();
    let (class, f) = setup(&mut w);
    let a = chained(&mut w, "Test.Chained", &[Some("Engine.ObjectPropertyAttributeValueResolver"), Some("Test.UnreadValueResolver")]);
    let s0 = slot(&mut w, true, false, false, Some(a), 1.0);
    let item = item_with(&mut w, class, vec![s0]);
    let before = (w.float(item, "Bonus"), w.get(item, "AppliedAttributeSlotEffects"));
    let e = call(&mut w, f, item, true, SLOTS, Value::array(vec![])).unwrap_err();
    assert!(matches!(e, VmError::NotYet(_)), "{e:?}");
    assert!(!format!("{e:?}").contains("partial writes"));
    assert_eq!((w.float(item, "Bonus"), w.get(item, "AppliedAttributeSlotEffects")), before, "nothing written");
}

/// A None chain entry ends the check, as it ends the add in any order: the unread entry past it is not checked, and
/// the add answers false, so nothing is recorded.
#[test]
fn a_none_entry_ends_the_chain_check() {
    let mut w = World::new();
    let (class, f) = setup(&mut w);
    let a = chained(&mut w, "Test.Stopped", &[None, Some("Test.UnreadValueResolver")]);
    let s0 = slot(&mut w, true, false, false, Some(a), 1.0);
    let item = item_with(&mut w, class, vec![s0]);
    let out = call(&mut w, f, item, true, SLOTS, Value::array(vec![])).unwrap();
    assert_eq!(out, Value::array(vec![]), "a false add records nothing");
    assert_eq!(w.get(item, "AppliedAttributeSlotEffects"), Value::array(vec![]));
}

/// With the flag false, a backup record with a None modifier is a no-op (P7): no error, nothing removed for it.
#[test]
fn a_backup_record_without_a_modifier_is_a_no_op() {
    let mut w = World::new();
    let (class, f) = setup(&mut w);
    let slots = five(&mut w);
    let item = item_with(&mut w, class, slots);
    let ty = w.vm.space.classes.struct_by_path(APPLIED_STRUCT).unwrap();
    let a = w.weapon_bonus;
    let mut rec = applied_values(&w.vm, ty, &ow_attr::api::Applied(vec![ow_attr::api::AppliedEffect { context: item, attribute: a, modifier: item }])).unwrap();
    let at = {
        let id = w.vm.space.names.find("Modifier").unwrap();
        w.vm.space.classes.struct_layout(ty).unwrap().field(id).unwrap().0
    };
    rec[0].struct_mut().unwrap().fields[at as usize] = Value::Object(None);
    w.set(item, "AppliedAttributeSlotEffects", Value::array(rec));
    let raw = call(&mut w, f, item, false, SLOTS, Value::array(vec![])).unwrap();
    let out = applied_from_values(&w.vm, &raw).unwrap();
    assert_eq!(out.0.len(), 2, "the two qualifying slots applied");
    assert_eq!(w.float(item, "Bonus"), 3.0);
    assert_eq!(w.get(item, "AppliedAttributeSlotEffects"), Value::array(vec![]), "emptied");
}
