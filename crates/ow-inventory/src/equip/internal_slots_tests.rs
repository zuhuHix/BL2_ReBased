//! `ApplyInternalSlotEffectModifiers` on the invented world of `crate::equip::tests` (AGENTS.md rule 9; decision
//! 2026-11-19-97). Expected values are written by hand from NATIVE_APPLY_SLOT_EFFECT_MODIFIERS.md. The item is a weapon
//! subclass, so the weapon context resolver gives the item itself and its `Bonus` stack attribute shows the adds.
//! AI-assisted (Claude Code).

use ow_core::Missing;
use ow_object::heap::Origin;
use ow_object::ids::{ClassId, FuncId, ObjRef, PropAddr};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use std::rc::Rc;

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

// ---- review r1 of #483 -------------------------------------------------------------------------------------------

/// A toy value resolver over the context's `Bonus` whose read-only probe answers but whose write is refused: a refusal
/// the preflight cannot see (as a change notification's script would be).
struct WriteFails;

impl ow_attr::api::ValueResolver for WriteFails {
    fn value(
        &self,
        _sys: &ow_attr::AttributeSystem,
        _resolver: ObjRef,
        _context: ObjRef,
        _previous: f32,
        _cx: &mut ow_attr::api::EvalCx<'_>,
    ) -> Result<f32, ow_attr::api::AttrError> {
        Ok(0.0)
    }

    fn storage(
        &self,
        _sys: &ow_attr::AttributeSystem,
        _resolver: ObjRef,
        _context: ObjRef,
        _cx: &mut ow_attr::api::EvalCx<'_>,
    ) -> Result<Option<ow_attr::api::Storage>, ow_attr::api::AttrError> {
        Err(ow_core::NotYet::not_read("the write-fails test resolver", "TOY_RECORD.md").into())
    }

    fn probe_storage(
        &self,
        _sys: &ow_attr::AttributeSystem,
        _resolver: ObjRef,
        _context: ObjRef,
        _op: ow_attr::api::ProbeOp,
        _cx: &mut ow_attr::api::EvalCx<'_>,
    ) -> Result<(), ow_attr::api::AttrError> {
        Ok(())
    }
}

fn with_write_fails(w: &mut World) {
    let object = w.class("Core.Object");
    w.vm.space.classes.define("Test.WriteFails", Some(object), vec![]).unwrap();
    let attrs = ow_attr::AttributeSystem::standard().unwrap().with_value_resolver("Test.WriteFails", WriteFails).unwrap();
    w.vm.attrs = std::rc::Rc::new(attrs);
}

/// A `NotYet` after a write (review r1 of #483): terminal, matched by its variant, "stopped after partial writes: "
/// once, its kind and record kept; the earlier writes stay (slot 0's modifier on the stack, the list emptied).
#[test]
fn a_not_yet_after_a_write_is_prefixed_and_the_writes_stay() {
    let mut w = World::new();
    with_write_fails(&mut w);
    let (class, f) = setup(&mut w);
    let a = w.weapon_bonus;
    let fails = chained(&mut w, "Test.Fails", &[Some("Test.WriteFails")]);
    let s0 = slot(&mut w, true, false, false, Some(a), 2.5);
    let s1 = slot(&mut w, true, false, false, Some(fails), 1.0);
    let item = item_with(&mut w, class, vec![s0, s1]);
    let e = call(&mut w, f, item, true, SLOTS, Value::array(vec![])).unwrap_err();
    let VmError::NotYet(n) = &e else { panic!("{e:?}") };
    assert_eq!(n.what, "stopped after partial writes: the write-fails test resolver");
    assert_eq!((n.missing.clone(), n.record), (Missing::NotRead, "TOY_RECORD.md"));
    assert_eq!(w.float(item, "Bonus"), 2.5, "slot 0's add stays");
    let backup = applied_from_values(&w.vm, &w.get(item, "AppliedAttributeSlotEffects")).unwrap();
    assert_eq!(backup.0.len(), 1, "slot 0's record stays in the backup list");
}

/// P9 on a removal's chain (review r1 of #483): two backup records naming one modifier, through a chain whose second
/// entry is not read. Executed in order, the second removal would stop at the first entry (the modifier already gone);
/// the check still covers every entry, so the call is refused before any write.
#[test]
fn every_entry_of_every_removal_is_checked_before_any_write() {
    let mut w = World::new();
    let (class, f) = setup(&mut w);
    let a = chained(&mut w, "Test.TwoEntries", &[Some("Engine.ObjectPropertyAttributeValueResolver"), Some("Test.UnreadValueResolver")]);
    let item = item_with(&mut w, class, vec![]);
    let m = w.obj("Core.AttributeModifier", "Test.Shared");
    let ty = w.vm.space.classes.struct_by_path(APPLIED_STRUCT).unwrap();
    let r = ow_attr::api::AppliedEffect { context: item, attribute: a, modifier: m };
    let recs = applied_values(&w.vm, ty, &ow_attr::api::Applied(vec![r, r])).unwrap();
    w.set(item, "AppliedAttributeSlotEffects", Value::array(recs.clone()));
    let e = call(&mut w, f, item, false, SLOTS, Value::array(vec![])).unwrap_err();
    assert!(matches!(e, VmError::NotYet(_)), "{e:?}");
    assert!(!format!("{e:?}").contains("partial writes"));
    assert_eq!(w.get(item, "AppliedAttributeSlotEffects"), Value::array(recs), "the list unchanged");
}

/// A "not writable" entry does not end the check: a playthrough-count entry (no store in Off), then an unread one.
#[test]
fn a_not_writable_entry_does_not_end_the_check() {
    let mut w = World::new();
    let object = w.class("Core.Object");
    w.vm.space.classes.define("WillowGame.PlayThroughCountAttributeValueResolver", Some(object), vec![]).unwrap();
    let (class, f) = setup(&mut w);
    let a = chained(&mut w, "Test.NotWritableFirst", &[Some("WillowGame.PlayThroughCountAttributeValueResolver"), Some("Test.UnreadValueResolver")]);
    let s0 = slot(&mut w, true, false, false, Some(a), 1.0);
    let item = item_with(&mut w, class, vec![s0]);
    let e = call(&mut w, f, item, true, SLOTS, Value::array(vec![])).unwrap_err();
    assert!(matches!(e, VmError::NotYet(_)), "{e:?}");
    assert_eq!(w.get(item, "AppliedAttributeSlotEffects"), Value::array(vec![]));
}

/// B3 (review r1 of #483): an add the first entry accepts and the second refuses answers false, so nothing is
/// recorded, and the object stays on the first entry's stack.
#[test]
fn a_false_add_after_an_accepting_entry_leaves_its_object() {
    let mut w = World::new();
    let (class, f) = setup(&mut w);
    let a = chained(&mut w, "Test.AcceptThenRefuse", &[Some("Engine.ObjectPropertyAttributeValueResolver"), Some("Engine.ObjectPropertyAttributeValueResolver")]);
    // The second entry names a property that is not an attribute: not writable.
    let chain = w.get(a, "ValueResolverChain");
    let second = ow_attr::eval::object("e", &chain.elements().unwrap()[1]).unwrap().unwrap();
    let n = Value::Name(w.vm.space.names.intern("DefinitionData"));
    w.set(second, "PropertyName", n);
    let s0 = slot(&mut w, true, false, false, Some(a), 4.0);
    let item = item_with(&mut w, class, vec![s0]);
    let out = call(&mut w, f, item, true, SLOTS, Value::array(vec![])).unwrap();
    assert_eq!(out, Value::array(vec![]), "the false add records nothing");
    assert_eq!(w.get(item, "AppliedAttributeSlotEffects"), Value::array(vec![]));
    assert_eq!(w.float(item, "Bonus"), 4.0, "the object stays on the first entry's stack");
}

/// A type outside the enum is a data error only on a qualifying slot (review r1 of #483, an Off assumption): an
/// external slot with one is not checked.
#[test]
fn the_modifier_type_is_checked_on_qualifying_slots_only() {
    let mut w = World::new();
    let (class, f) = setup(&mut w);
    let a = w.weapon_bonus;
    let mut odd = slot(&mut w, true, true, false, Some(a), 1.0);
    let ty = w.vm.space.classes.struct_by_path("Test.InternalSlotData").unwrap();
    let at = {
        let id = w.vm.space.names.find("ModifierType").unwrap();
        w.vm.space.classes.struct_layout(ty).unwrap().field(id).unwrap().0 as usize
    };
    odd.struct_mut().unwrap().fields[at] = Value::Byte(9);
    let item = item_with(&mut w, class, vec![odd.clone()]);
    call(&mut w, f, item, true, SLOTS, Value::array(vec![])).unwrap();
    let mut internal = odd;
    let ext = {
        let id = w.vm.space.names.find("bExternalSlot").unwrap();
        w.vm.space.classes.struct_layout(ty).unwrap().field(id).unwrap().0 as usize
    };
    internal.struct_mut().unwrap().fields[ext] = Value::Bool(false);
    let item = item_with(&mut w, class, vec![internal]);
    let e = call(&mut w, f, item, true, SLOTS, Value::array(vec![])).unwrap_err();
    assert!(matches!(e, VmError::Script { .. }), "{e:?}");
}

/// The shape checks (P1, P3, P6): each a data error before any write.
#[test]
fn the_shape_checks_refuse_before_any_write() {
    // P6: a slot field of the wrong kind is caught by the declaration check (P2) already; a None attribute is fine.
    let mut w = World::new();
    let (class, f) = setup(&mut w);
    let item = item_with(&mut w, class, vec![]);
    // P1: the out argument must be an array of records.
    let object = w.class("Core.Object");
    let params = vec![
        param(&mut w, "bBackupSlotEffectsApplied", PropKind::Bool, 0),
        param(&mut w, "MaxSlotsActivated", PropKind::Int, 0),
        param(&mut w, "AttributeModifiers", PropKind::Array(Box::new(PropKind::Object { class: object })), PropertyDecl::OUT),
    ];
    let path = ow_core::ObjectPath::parse("Test.SlotWeapon.ApplyInternalSlotEffectModifiers").unwrap();
    let g = w.vm.declare_function(FunctionDecl::native(path, INTERNAL_SLOTS).with_owner(class).with_params(params)).unwrap();
    let e = call(&mut w, g, item, true, SLOTS, Value::array(vec![])).unwrap_err();
    assert!(matches!(e, VmError::Script { .. }), "P1: {e:?}");
    // P3: a class without AppliedAttributeSlotEffects.
    let slotty = w.vm.space.classes.struct_by_path("Test.InternalSlotData").unwrap();
    let decls = vec![PropertyDecl::new(w.vm.space.names.intern("AttributeSlots"), PropKind::Struct(slotty)).with_array_dim(SLOTS)];
    let weapon = w.class("WillowGame.WillowWeapon");
    let bare = w.vm.space.classes.define("Test.BareSlotWeapon", Some(weapon), decls).unwrap();
    let item = item_with(&mut w, bare, vec![]);
    let e = call(&mut w, f, item, true, SLOTS, Value::array(vec![])).unwrap_err();
    assert!(matches!(e, VmError::Script { .. }), "P3: {e:?}");
    let _ = object;
}

/// The path of `AttributeInitializationData`.
const AID: &str = "Engine.AttributeInitializationDefinition.AttributeInitializationData";

/// The noun classes: a subclass of the item class `class` with a `NounState`, and the noun resolver class.
fn noun_classes(w: &mut World, class: ClassId) -> (ClassId, ClassId) {
    let object = w.class("Core.Object");
    let aid = w.vm.space.classes.struct_by_path(AID).unwrap();
    let d = |w: &mut World, n: &str, k: PropKind| PropertyDecl::new(w.vm.space.names.intern(n), k);
    let fields = vec![
        d(w, "NounName", PropKind::Name),
        d(w, "Value", PropKind::Float),
        d(w, "BaseValue", PropKind::Float),
        d(w, "ModifierStack", PropKind::Array(Box::new(PropKind::Object { class: object }))),
        d(w, "ReplicationStrategy", PropKind::Byte { enum_: None }),
        d(w, "IndexInReplicationArray", PropKind::Int),
    ];
    let state = w.vm.space.classes.define_struct("GearboxFramework.INounAttributeProvider.NounAttributeState", None, fields).unwrap();
    let decls = vec![d(w, "NounState", PropKind::Array(Box::new(PropKind::Struct(state))))];
    let noun_item = w.vm.space.classes.define("Test.NounSlotWeapon", Some(class), decls).unwrap();
    let decls = vec![d(w, "DefaultValue", PropKind::Struct(aid)), d(w, "ReplicationStrategy", PropKind::Byte { enum_: None })];
    let noun = w.vm.space.classes.define("GearboxFramework.NounAttributeValueResolver", Some(object), decls).unwrap();
    (noun_item, noun)
}

/// A noun attribute named `name` on the weapon context, its noun resolver a subobject (the key is the attribute's
/// name) with `DefaultValue` `default`.
fn noun_attribute(w: &mut World, noun: ClassId, name: &str, default: Value) -> ObjRef {
    let ctx = w.obj("Engine.WeaponAttributeContextResolver", &format!("{name}.Context"));
    let attr = w.obj("Engine.AttributeDefinition", name);
    let r = w.vm.space.alloc(noun, "NounAttributeValueResolver_0", Some(attr), Origin::Runtime).unwrap();
    w.set(r, "DefaultValue", default);
    w.set(attr, "ContextResolverChain", Value::array(vec![Value::Object(Some(ctx))]));
    w.set(attr, "ValueResolverChain", Value::array(vec![Value::Object(Some(r))]));
    attr
}

/// The noun case of review r1 of #483: slot 0's attribute goes through a noun resolver and the item holds no entry for
/// it, and slot 1's value resolver is not read. The chain check of slot 0 must not create the entry, so the call is
/// refused by slot 1's `NotYet`, unprefixed, with `NounState` unchanged.
#[test]
fn the_chain_check_creates_no_noun_entry() {
    let mut w = World::new();
    let (class, f) = setup(&mut w);
    let (noun_item, noun) = noun_classes(&mut w, class);
    let default = w.init(3.0);
    let attr = noun_attribute(&mut w, noun, "Attribute_ToyNoun", default);
    let unread = chained(&mut w, "Test.UnreadValue", &[Some("Test.UnreadValueResolver")]);
    let s0 = slot(&mut w, true, false, false, Some(attr), 1.0);
    let s1 = slot(&mut w, true, false, false, Some(unread), 1.0);
    let item = item_with(&mut w, noun_item, vec![s0, s1]);
    let e = call(&mut w, f, item, true, SLOTS, Value::array(vec![])).unwrap_err();
    let VmError::NotYet(n) = &e else { panic!("{e:?}") };
    assert!(!n.what.contains("partial writes"), "{n}");
    assert!(n.what.contains("Test.UnreadValueResolver"), "slot 1's refusal: {n}");
    assert_eq!(w.get(item, "NounState"), Value::array(vec![]), "no noun entry created");
    // The control: without slot 1 the call goes through, and W3's add creates the entry.
    let s0 = slot(&mut w, true, false, false, Some(attr), 1.0);
    let item = item_with(&mut w, noun_item, vec![s0]);
    call(&mut w, f, item, true, SLOTS, Value::array(vec![])).unwrap();
    assert_eq!(w.get(item, "NounState").elements().unwrap().len(), 1, "the add created the entry");
}

/// The first removal's change notification is a `NotYet` (review r2 of #483): the removal wrote the stack and the value
/// first, so the call stops marked, kind and record kept, and the stack change stays.
#[test]
fn the_first_removal_s_notification_not_yet_is_marked() {
    let mut w = World::new();
    let (class, f) = setup(&mut w);
    let modifier = w.class("Core.AttributeModifier");
    let d = |w: &mut World, n: &str, k: PropKind, flags: u64| PropertyDecl::new(w.vm.space.names.intern(n), k).with_flags(flags);
    let decls = vec![
        d(&mut w, "Loud", PropKind::FloatAttribute, PropertyDecl::STACK_ATTRIBUTE | PropertyDecl::NOTIFY),
        d(&mut w, "LoudBaseValue", PropKind::FloatAttribute, PropertyDecl::PLAIN_ATTRIBUTE),
        d(&mut w, "LoudModifierStack", PropKind::Array(Box::new(PropKind::Object { class: modifier })), 0),
    ];
    let loud_class = w.vm.space.classes.define("Test.LoudSlotWeapon", Some(class), decls).unwrap();
    let (v, b, s) = (w.vm.space.names.intern("Loud"), w.vm.space.names.intern("LoudBaseValue"), w.vm.space.names.intern("LoudModifierStack"));
    w.vm.space.classes.link_attribute(loud_class, v, b, s).unwrap();
    w.vm
        .register_attribute_notify(
            "Test.LoudSlotWeapon",
            std::rc::Rc::new(|_vm, _owner, _prop, _forms| {
                Err(ow_core::NotYet::not_read("the loud test notification", "TOY_NOTIFY.md").into())
            }),
        )
        .unwrap();
    let ctx = w.obj("Engine.WeaponAttributeContextResolver", "Test.Loud.Context");
    let k = w.obj("Engine.ObjectPropertyAttributeValueResolver", "Test.Loud.Value");
    let n = Value::Name(w.vm.space.names.intern("Loud"));
    w.set(k, "PropertyName", n);
    let loud = w.obj("Engine.AttributeDefinition", "Test.Loud");
    w.set(loud, "ContextResolverChain", Value::array(vec![Value::Object(Some(ctx))]));
    w.set(loud, "ValueResolverChain", Value::array(vec![Value::Object(Some(k))]));
    let item = item_with(&mut w, loud_class, vec![]);
    let m = w.obj("Core.AttributeModifier", "Test.LoudModifier");
    w.set(m, "Type", Value::Byte(2));
    w.set(m, "Value", Value::Float(1.0));
    w.set(item, "LoudModifierStack", Value::array(vec![Value::Object(Some(m))]));
    let ty = w.vm.space.classes.struct_by_path(APPLIED_STRUCT).unwrap();
    let rec = applied_values(&w.vm, ty, &ow_attr::api::Applied(vec![ow_attr::api::AppliedEffect { context: item, attribute: loud, modifier: m }])).unwrap();
    w.set(item, "AppliedAttributeSlotEffects", Value::array(rec));
    let e = call(&mut w, f, item, false, SLOTS, Value::array(vec![])).unwrap_err();
    let VmError::NotYet(n) = &e else { panic!("{e:?}") };
    assert_eq!(n.what, "stopped after partial writes: the loud test notification");
    assert_eq!((n.missing.clone(), n.record), (Missing::NotRead, "TOY_NOTIFY.md"));
    assert_eq!(w.get(item, "LoudModifierStack"), Value::array(vec![]), "the removal's stack change stays");
}

// ---- lane FIX-483-FOLLOWUP (the hub's review follow-up of landed #483) -------------------------------------------

/// Slot 0 is a noun attribute with no entry on the item, whose `DefaultValue` `default` makes; slot 1's value resolver
/// is not read. Returns the world, the item and the call's refusal; the caller checks what the probe must not have
/// done.
fn noun_probe_then_refusal(default: impl FnOnce(&mut World, ClassId) -> Value) -> (World, ObjRef, VmError) {
    let mut w = World::new();
    let (class, f) = setup(&mut w);
    let (noun_item, noun) = noun_classes(&mut w, class);
    let d = default(&mut w, noun);
    let attr = noun_attribute(&mut w, noun, "Attribute_ToyOuter", d);
    let unread = chained(&mut w, "Test.UnreadValue", &[Some("Test.UnreadValueResolver")]);
    let s0 = slot(&mut w, true, false, false, Some(attr), 1.0);
    let s1 = slot(&mut w, true, false, false, Some(unread), 1.0);
    let item = item_with(&mut w, noun_item, vec![s0, s1]);
    let e = call(&mut w, f, item, true, SLOTS, Value::array(vec![])).unwrap_err();
    (w, item, e)
}

/// The refusal is slot 1's typed `NotYet`, unprefixed (the preflight wrote nothing).
fn assert_slot_1_refusal(e: &VmError) {
    let VmError::NotYet(n) = e else { panic!("{e:?}") };
    assert!(!n.what.contains("partial writes"), "{n}");
    assert!(n.what.contains("Test.UnreadValueResolver"), "slot 1's refusal: {n}");
}

/// F1: a missing entry's `DefaultValue` names another noun attribute on the same context (`BaseValueAttribute`). The
/// probe validates the default without evaluating it, so the nested noun's entry is not created before slot 1's
/// refusal.
#[test]
fn the_probe_evaluates_no_nested_noun_default() {
    let (w, item, e) = noun_probe_then_refusal(|w, noun| {
        let inner_default = w.init(2.0);
        let inner = noun_attribute(w, noun, "Attribute_ToyInner", inner_default);
        w.st(AID, vec![("BaseValueAttribute", Value::Object(Some(inner))), ("BaseValueScaleConstant", Value::Float(1.0))])
    });
    assert_slot_1_refusal(&e);
    assert_eq!(w.get(item, "NounState"), Value::array(vec![]), "no entry for the nested noun or the outer one");
}

/// An `InitializationDefinition` subclass whose `RandomVariance` is enabled, from `low` to 5 (the other parts off).
fn variance_definition(w: &mut World, low: Value) -> ObjRef {
    let aid = w.vm.space.classes.struct_by_path(AID).unwrap();
    let d = |w: &mut World, n: &str, k: PropKind| PropertyDecl::new(w.vm.space.names.intern(n), k);
    let st = |w: &mut World, path: &str, fields: &[(&str, PropKind)]| {
        let decls = fields.iter().map(|(n, k)| d(w, n, k.clone())).collect();
        w.vm.space.classes.define_struct(path, None, decls).unwrap()
    };
    let formula = st(w, "Test.VarianceDefinition.Formula", &[("bEnabled", PropKind::Bool)]);
    let conditional = st(w, "Test.VarianceDefinition.Conditional", &[("bEnabled", PropKind::Bool)]);
    let variance = st(
        w,
        "Test.VarianceDefinition.Variance",
        &[
            ("bEnabled", PropKind::Bool),
            ("LowerBound", PropKind::Struct(aid)),
            ("UpperBound", PropKind::Struct(aid)),
            ("bUseIntegerRandomization", PropKind::Bool),
        ],
    );
    let range = st(
        w,
        "Test.VarianceDefinition.Range",
        &[("bEnableMinValueRestriction", PropKind::Bool), ("bEnableMaxValueRestriction", PropKind::Bool)],
    );
    let byte = PropKind::Byte { enum_: None };
    let decls = vec![
        d(w, "ValueFormula", PropKind::Struct(formula)),
        d(w, "ConditionalInitialization", PropKind::Struct(conditional)),
        d(w, "RandomVariance", PropKind::Struct(variance)),
        d(w, "RangeRestriction", PropKind::Struct(range)),
        d(w, "BaseValueMode", byte.clone()),
        d(w, "RoundingMode", byte),
    ];
    let parent = w.class("Engine.AttributeInitializationDefinition");
    w.vm.space.classes.define("Test.VarianceDefinition", Some(parent), decls).unwrap();
    let def = w.obj("Test.VarianceDefinition", "Test.Variance");
    let high = w.init(5.0);
    let v = w.st("Test.VarianceDefinition.Variance", vec![("bEnabled", Value::Bool(true)), ("LowerBound", low), ("UpperBound", high)]);
    w.set(def, "RandomVariance", v);
    def
}

/// F1: a missing entry's `DefaultValue` has an enabled random variance. The probe draws nothing before slot 1's
/// refusal; the control (no slot 1) shows the default is evaluated once, by the add that creates the entry.
#[test]
fn the_probe_draws_no_variance_and_the_add_evaluates_once() {
    let mut rng = None;
    let (w, item, e) = noun_probe_then_refusal(|w, _| {
        let low = w.init(1.0);
        let def = variance_definition(w, low);
        rng = Some(w.vm.rng.clone());
        w.st(AID, vec![("InitializationDefinition", Value::Object(Some(def))), ("BaseValueScaleConstant", Value::Float(1.0))])
    });
    assert_slot_1_refusal(&e);
    assert_eq!(w.get(item, "NounState"), Value::array(vec![]), "no entry created");
    assert_eq!(Some(w.vm.rng.clone()), rng, "no variance drawn");
    // The control: one slot, so the call goes through; the entry's creation draws once.
    let mut w = World::new();
    let (class, f) = setup(&mut w);
    let (noun_item, noun) = noun_classes(&mut w, class);
    let low = w.init(1.0);
    let def = variance_definition(&mut w, low);
    let default = w.st(AID, vec![("InitializationDefinition", Value::Object(Some(def))), ("BaseValueScaleConstant", Value::Float(1.0))]);
    let attr = noun_attribute(&mut w, noun, "Attribute_ToyOuter", default);
    let s0 = slot(&mut w, true, false, false, Some(attr), 1.0);
    let item = item_with(&mut w, noun_item, vec![s0]);
    let mut once = w.vm.rng.clone();
    ow_core::rand::Rand15::rand15(once.crt(ow_core::rand::CrtStream::AttributeVariance));
    call(&mut w, f, item, true, SLOTS, Value::array(vec![])).unwrap();
    assert_eq!(w.get(item, "NounState").elements().unwrap().len(), 1, "the add created the entry");
    assert_eq!(w.vm.rng, once, "one draw: the default evaluated once");
}

/// F2: the out argument aliases the item's `Mods` (as a script passing an instance variable does). Slot 0's add
/// succeeds; slot 1's add notifies, and the notification appends a sentinel to `Mods`. Each record goes to the live out
/// argument at once, so a refusal at slot 1 keeps slot 0's record and the sentinel; without the refusal, no final
/// assignment overwrites the sentinel.
#[test]
fn each_record_goes_to_the_live_out_argument_at_once() {
    use std::cell::Cell;
    let mut w = World::new();
    let (class, f) = setup(&mut w);
    let modifier = w.class("Core.AttributeModifier");
    let applied = w.vm.space.classes.struct_by_path(APPLIED_STRUCT).unwrap();
    let d = |w: &mut World, n: &str, k: PropKind, flags: u64| PropertyDecl::new(w.vm.space.names.intern(n), k).with_flags(flags);
    let decls = vec![
        d(&mut w, "Loud", PropKind::FloatAttribute, PropertyDecl::STACK_ATTRIBUTE | PropertyDecl::NOTIFY),
        d(&mut w, "LoudBaseValue", PropKind::FloatAttribute, PropertyDecl::PLAIN_ATTRIBUTE),
        d(&mut w, "LoudModifierStack", PropKind::Array(Box::new(PropKind::Object { class: modifier })), 0),
        d(&mut w, "Mods", PropKind::Array(Box::new(PropKind::Struct(applied))), 0),
    ];
    let alias_class = w.vm.space.classes.define("Test.AliasSlotWeapon", Some(class), decls).unwrap();
    let (v, b, s) = (w.vm.space.names.intern("Loud"), w.vm.space.names.intern("LoudBaseValue"), w.vm.space.names.intern("LoudModifierStack"));
    w.vm.space.classes.link_attribute(alias_class, v, b, s).unwrap();
    let mods = w.vm.space.prop(alias_class, "Mods").unwrap();
    let sentinel_mod = w.obj("Core.AttributeModifier", "Test.Sentinel");
    let a = w.weapon_bonus;
    let sentinel = ow_attr::api::AppliedEffect { context: sentinel_mod, attribute: a, modifier: sentinel_mod };
    let refuse = Rc::new(Cell::new(true));
    let r = refuse.clone();
    w.vm
        .register_attribute_notify(
            "Test.AliasSlotWeapon",
            Rc::new(move |vm, owner, _prop, _forms| {
                // The sentinel goes to the aliased out argument and to the backup list (review r1 of #518, item 6).
                let class = vm.space.heap.get(owner).unwrap().class;
                let backup = vm.space.prop(class, "AppliedAttributeSlotEffects").unwrap();
                for p in [mods, backup] {
                    let mut list = vm.space.read(owner, PropAddr::scalar(p)).unwrap().elements().unwrap().to_vec();
                    list.extend(applied_values(vm, applied, &ow_attr::api::Applied(vec![sentinel])).unwrap());
                    vm.space.write(owner, PropAddr::scalar(p), Value::array(list)).unwrap();
                }
                if r.get() {
                    return Err(ow_core::NotYet::not_read("the alias test notification", "TOY_NOTIFY.md").into());
                }
                Ok(())
            }),
        )
        .unwrap();
    let loud = w.attribute("Test.Loud", "Engine.WeaponAttributeContextResolver", "Loud");
    let run = |w: &mut World| {
        let s0 = slot(w, true, false, false, Some(a), 2.5);
        let s1 = slot(w, true, false, false, Some(loud), 1.0);
        let item = item_with(w, alias_class, vec![s0, s1]);
        let args = vec![
            ow_vm::api::Arg::new(Value::Bool(true), false, true),
            ow_vm::api::Arg::new(Value::Int(SLOTS), false, true),
            ow_vm::api::Arg::aliasing_property(item, PropAddr::scalar(mods), Value::array(vec![])),
        ];
        let mut call = ow_vm::api::NativeCall::new(&mut w.vm, f, Some(item), args);
        let r = native(&mut call);
        (item, r)
    };
    let (item, r) = run(&mut w);
    let e = r.unwrap_err();
    let VmError::NotYet(n) = &e else { panic!("{e:?}") };
    assert_eq!(n.what, "stopped after partial writes: the alias test notification");
    let out = applied_from_values(&w.vm, &w.get(item, "Mods")).unwrap();
    assert_eq!(out.0.len(), 2, "slot 0's record, then the sentinel: {out:?}");
    assert_eq!((out.0[0].context, out.0[0].attribute), (item, a), "slot 0's record");
    assert_eq!(out.0[1], sentinel);
    let backup = applied_from_values(&w.vm, &w.get(item, "AppliedAttributeSlotEffects")).unwrap();
    assert_eq!(backup.0, out.0[..2].to_vec(), "slot 0's record, then the sentinel, in the backup list too");
    // Without the refusal: slot 0's record, the sentinel slot 1's notification added, then slot 1's record.
    refuse.set(false);
    let (item, r) = run(&mut w);
    r.unwrap();
    let out = applied_from_values(&w.vm, &w.get(item, "Mods")).unwrap();
    assert_eq!(out.0.len(), 3, "{out:?}");
    assert_eq!(out.0[1], sentinel, "not overwritten");
    assert_eq!((out.0[2].attribute, out.0[2].context), (loud, item), "slot 1's record last");
    let backup = applied_from_values(&w.vm, &w.get(item, "AppliedAttributeSlotEffects")).unwrap();
    assert_eq!(backup.0, out.0, "the backup list's sentinel survives slot 1's append");
}

/// F4: a W1 removal through a plain attribute writes nothing before its change notification, and the notification
/// refuses at once: no write came first, so the `NotYet` is unprefixed, its kind and record kept, and the list is left.
#[test]
fn a_w1_refusal_before_any_write_is_not_marked() {
    let mut w = World::new();
    let (class, f) = setup(&mut w);
    let d = PropertyDecl::new(w.vm.space.names.intern("Quiet"), PropKind::FloatAttribute)
        .with_flags(PropertyDecl::PLAIN_ATTRIBUTE | PropertyDecl::NOTIFY);
    let plain_class = w.vm.space.classes.define("Test.PlainSlotWeapon", Some(class), vec![d]).unwrap();
    w.vm
        .register_attribute_notify(
            "Test.PlainSlotWeapon",
            Rc::new(|_vm, _owner, _prop, _forms| Err(ow_core::NotYet::not_read("the plain test notification", "TOY_NOTIFY.md").into())),
        )
        .unwrap();
    let quiet = w.attribute("Test.Quiet", "Engine.WeaponAttributeContextResolver", "Quiet");
    let item = item_with(&mut w, plain_class, vec![]);
    let m = w.obj("Core.AttributeModifier", "Test.QuietModifier");
    let ty = w.vm.space.classes.struct_by_path(APPLIED_STRUCT).unwrap();
    let rec = applied_values(&w.vm, ty, &ow_attr::api::Applied(vec![ow_attr::api::AppliedEffect { context: item, attribute: quiet, modifier: m }])).unwrap();
    w.set(item, "AppliedAttributeSlotEffects", Value::array(rec.clone()));
    let e = call(&mut w, f, item, false, SLOTS, Value::array(vec![])).unwrap_err();
    let VmError::NotYet(n) = &e else { panic!("{e:?}") };
    assert_eq!(n.what, "the plain test notification", "unprefixed");
    assert_eq!((n.missing, n.record), (Missing::NotRead, "TOY_NOTIFY.md"));
    assert_eq!(w.get(item, "AppliedAttributeSlotEffects"), Value::array(rec), "W2 not reached");
}

/// F5: P1 checks that the third parameter is `out`. With the flag cleared, the call is a data error before any write:
/// the backup list, the attribute's value and the heap are unchanged.
#[test]
fn an_out_argument_that_is_not_out_is_refused_before_any_write() {
    let mut w = World::new();
    let (class, f) = setup(&mut w);
    let slots = five(&mut w);
    let item = item_with(&mut w, class, slots);
    call(&mut w, f, item, true, SLOTS, Value::array(vec![])).unwrap();
    let applied = w.vm.space.classes.struct_by_path(APPLIED_STRUCT).unwrap();
    let params = vec![
        param(&mut w, "bBackupSlotEffectsApplied", PropKind::Bool, 0),
        param(&mut w, "MaxSlotsActivated", PropKind::Int, 0),
        param(&mut w, "AttributeModifiers", PropKind::Array(Box::new(PropKind::Struct(applied))), 0),
    ];
    let path = ow_core::ObjectPath::parse("Test.SlotWeapon.ApplyInternalSlotEffectModifiers").unwrap();
    let g = w.vm.declare_function(FunctionDecl::native(path, INTERNAL_SLOTS).with_owner(class).with_params(params)).unwrap();
    let before = (w.get(item, "AppliedAttributeSlotEffects"), w.float(item, "Bonus"), format!("{:?}", w.vm.space.heap));
    let e = call(&mut w, g, item, false, SLOTS, Value::array(vec![])).unwrap_err();
    assert!(matches!(e, VmError::Script { .. }), "{e:?}");
    let after = (w.get(item, "AppliedAttributeSlotEffects"), w.float(item, "Bonus"), format!("{:?}", w.vm.space.heap));
    assert!(after == before, "nothing written");
}

/// F6: a backup record on a noun entry with a replicated slot, whose stack does not hold the record's modifier. The
/// removal would change nothing (not present), so the probe does not refuse it: the call goes through, the entry is
/// untouched, and W2 empties the list.
#[test]
fn a_replicated_entry_without_the_modifier_is_not_refused() {
    let mut w = World::new();
    let (class, f) = setup(&mut w);
    let (noun_item, noun) = noun_classes(&mut w, class);
    let default = w.init(3.0);
    let attr = noun_attribute(&mut w, noun, "Attribute_ToyNoun", default);
    let item = item_with(&mut w, noun_item, vec![]);
    let key = Value::Name(w.vm.space.names.intern("Attribute_ToyNoun"));
    let entry = w.st(
        "GearboxFramework.INounAttributeProvider.NounAttributeState",
        vec![
            ("NounName", key),
            ("Value", Value::Float(3.0)),
            ("BaseValue", Value::Float(3.0)),
            ("ModifierStack", Value::array(vec![])),
            ("ReplicationStrategy", Value::Byte(0)),
            ("IndexInReplicationArray", Value::Int(2)),
        ],
    );
    w.set(item, "NounState", Value::array(vec![entry]));
    let m = w.obj("Core.AttributeModifier", "Test.Absent");
    let ty = w.vm.space.classes.struct_by_path(APPLIED_STRUCT).unwrap();
    let rec = applied_values(&w.vm, ty, &ow_attr::api::Applied(vec![ow_attr::api::AppliedEffect { context: item, attribute: attr, modifier: m }])).unwrap();
    w.set(item, "AppliedAttributeSlotEffects", Value::array(rec));
    let state = w.get(item, "NounState");
    call(&mut w, f, item, false, SLOTS, Value::array(vec![])).unwrap();
    assert_eq!(w.get(item, "NounState"), state, "the entry untouched");
    assert_eq!(w.get(item, "AppliedAttributeSlotEffects"), Value::array(vec![]), "W2 emptied the list");
}

// ---- review r1 of #518 ---------------------------------------------------------------------------------------------

/// A plain notifying attribute `Quiet` on a subclass of the item class, whose notification runs `callback` (it gets
/// the owner and the subclass's `Items` array), then refuses with a `NotYet`. Returns the item, its backup record and
/// the call's refusal.
fn plain_removal_refusing_after(callback: impl Fn(&mut ow_vm::api::Vm, ObjRef, ow_object::ids::PropId) + 'static) -> (World, ObjRef, Value, VmError) {
    let mut w = World::new();
    let (class, f) = setup(&mut w);
    let object = w.class("Core.Object");
    let decls = vec![
        PropertyDecl::new(w.vm.space.names.intern("Quiet"), PropKind::FloatAttribute)
            .with_flags(PropertyDecl::PLAIN_ATTRIBUTE | PropertyDecl::NOTIFY),
        PropertyDecl::new(w.vm.space.names.intern("Items"), PropKind::Array(Box::new(PropKind::Object { class: object }))),
    ];
    let plain_class = w.vm.space.classes.define("Test.CallbackSlotWeapon", Some(class), decls).unwrap();
    let items = w.vm.space.prop(plain_class, "Items").unwrap();
    w.vm
        .register_attribute_notify(
            "Test.CallbackSlotWeapon",
            Rc::new(move |vm, owner, _prop, _forms| {
                callback(vm, owner, items);
                Err(ow_core::NotYet::not_read("the callback test notification", "TOY_NOTIFY.md").into())
            }),
        )
        .unwrap();
    let quiet = w.attribute("Test.Quiet", "Engine.WeaponAttributeContextResolver", "Quiet");
    let item = item_with(&mut w, plain_class, vec![]);
    let kept = w.obj("Core.Object", "Test.Kept");
    w.set(item, "Items", Value::array(vec![Value::Object(Some(kept))]));
    let m = w.obj("Core.AttributeModifier", "Test.QuietModifier");
    let ty = w.vm.space.classes.struct_by_path(APPLIED_STRUCT).unwrap();
    let rec = Value::array(applied_values(&w.vm, ty, &ow_attr::api::Applied(vec![ow_attr::api::AppliedEffect { context: item, attribute: quiet, modifier: m }])).unwrap());
    w.set(item, "AppliedAttributeSlotEffects", rec.clone());
    let e = call(&mut w, f, item, false, SLOTS, Value::array(vec![])).unwrap_err();
    (w, item, rec, e)
}

/// Item 2 of review r1 of #518: a notification that "removes" an item its array does not hold (it reads the array,
/// drops nothing and writes it back) changed nothing, so its `NotYet` is unprefixed; one that writes a changed array is
/// marked. Kind and record are kept either way.
#[test]
fn only_a_committed_change_marks_a_w1_refusal() {
    let absent = |vm: &mut ow_vm::api::Vm, owner: ObjRef, items: ow_object::ids::PropId| {
        let gone = Value::Object(None);
        let list: Vec<Value> = vm.space.read(owner, PropAddr::scalar(items)).unwrap().elements().unwrap().iter().filter(|v| **v != gone).cloned().collect();
        vm.space.write(owner, PropAddr::scalar(items), Value::array(list)).unwrap();
    };
    let (w, item, rec, e) = plain_removal_refusing_after(absent);
    let VmError::NotYet(n) = &e else { panic!("{e:?}") };
    assert_eq!(n.what, "the callback test notification", "a no-op: unprefixed");
    assert_eq!((n.missing, n.record), (Missing::NotRead, "TOY_NOTIFY.md"));
    assert_eq!(w.get(item, "AppliedAttributeSlotEffects"), rec, "W2 not reached");
    let writes = |vm: &mut ow_vm::api::Vm, owner: ObjRef, items: ow_object::ids::PropId| {
        vm.space.write(owner, PropAddr::scalar(items), Value::array(vec![])).unwrap();
    };
    let (_, _, _, e) = plain_removal_refusing_after(writes);
    let VmError::NotYet(n) = &e else { panic!("{e:?}") };
    assert_eq!(n.what, "stopped after partial writes: the callback test notification", "a write: marked");
    assert_eq!((n.missing, n.record), (Missing::NotRead, "TOY_NOTIFY.md"));
}

/// Item 3 of review r1 of #518: slot 0's noun default has a random variance whose lower bound names an attribute with
/// a value resolver the attribute system does not know. The deepened default check refuses it in the preflight: the
/// typed `NotYet`, unprefixed, before W2 empties the backup list and before W3 makes a modifier.
#[test]
fn an_unsupported_resolver_in_a_variance_bound_is_refused_before_w2() {
    let mut w = World::new();
    let (class, f) = setup(&mut w);
    let (noun_item, noun) = noun_classes(&mut w, class);
    // Its context resolves from the item (the weapon context), so evaluating the bound would reach the resolver.
    let unread = chained(&mut w, "Test.UnreadBound", &[Some("Test.UnreadValueResolver")]);
    let low = w.st(AID, vec![("BaseValueAttribute", Value::Object(Some(unread))), ("BaseValueScaleConstant", Value::Float(1.0))]);
    let def = variance_definition(&mut w, low);
    let default = w.st(AID, vec![("InitializationDefinition", Value::Object(Some(def))), ("BaseValueScaleConstant", Value::Float(1.0))]);
    let attr = noun_attribute(&mut w, noun, "Attribute_ToyOuter", default);
    let s0 = slot(&mut w, true, false, false, Some(attr), 1.0);
    let item = item_with(&mut w, noun_item, vec![s0]);
    let ty = w.vm.space.classes.struct_by_path(APPLIED_STRUCT).unwrap();
    let a = w.weapon_bonus;
    let m = w.obj("Core.AttributeModifier", "Test.Backed");
    let rec = Value::array(applied_values(&w.vm, ty, &ow_attr::api::Applied(vec![ow_attr::api::AppliedEffect { context: item, attribute: a, modifier: m }])).unwrap());
    w.set(item, "AppliedAttributeSlotEffects", rec.clone());
    let before = (format!("{:?}", w.vm.space.heap), w.vm.rng.clone());
    let e = call(&mut w, f, item, true, SLOTS, Value::array(vec![])).unwrap_err();
    let VmError::NotYet(n) = &e else { panic!("{e:?}") };
    assert!(!n.what.contains("partial writes"), "{n}");
    assert_eq!(w.get(item, "AppliedAttributeSlotEffects"), rec, "W2 not reached");
    assert_eq!((format!("{:?}", w.vm.space.heap), w.vm.rng.clone()), before, "no modifier made, no entry, no draw");
}

/// Item 4 of review r1 of #518: an out parameter whose records are a struct named `AppliedAttributeEffect` but not the
/// canonical one (`APPLIED_STRUCT`), with OUT set, is a preflight data error.
#[test]
fn a_same_named_record_struct_is_not_the_canonical_one() {
    let mut w = World::new();
    let (class, _) = setup(&mut w);
    let slots = five(&mut w);
    let item = item_with(&mut w, class, slots);
    let object = w.class("Core.Object");
    let decls = ["Context", "Attribute", "Modifier"]
        .map(|n| PropertyDecl::new(w.vm.space.names.intern(n), PropKind::Object { class: object }))
        .to_vec();
    let other = w.vm.space.classes.define_struct("Test.Elsewhere.AppliedAttributeEffect", None, decls).unwrap();
    let params = vec![
        param(&mut w, "bBackupSlotEffectsApplied", PropKind::Bool, 0),
        param(&mut w, "MaxSlotsActivated", PropKind::Int, 0),
        param(&mut w, "AttributeModifiers", PropKind::Array(Box::new(PropKind::Struct(other))), PropertyDecl::OUT),
    ];
    let path = ow_core::ObjectPath::parse("Test.SlotWeapon.ApplyInternalSlotEffectModifiers").unwrap();
    let g = w.vm.declare_function(FunctionDecl::native(path, INTERNAL_SLOTS).with_owner(class).with_params(params)).unwrap();
    let before = format!("{:?}", w.vm.space.heap);
    let e = call(&mut w, g, item, true, SLOTS, Value::array(vec![])).unwrap_err();
    assert!(matches!(&e, VmError::Script { message, .. } if message.contains(APPLIED_STRUCT)), "{e:?}");
    assert_eq!(format!("{:?}", w.vm.space.heap), before, "nothing written");
}
