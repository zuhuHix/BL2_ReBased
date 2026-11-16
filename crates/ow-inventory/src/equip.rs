//! Owner: lane W5-EQUIP. Equip and unequip effects, and the two script functions bound to Rust rules
//! (architecture rule R2, `override_script`). AI-assisted (Claude Code).
//!
//! Every rule here is read from the installed script (`research/script_disasm.py` on `WillowGame.upk` and
//! `Engine.upk`, 2026-11-16; the listings stay under ignored `local/analysis/`) or from the records named on each
//! item. None was checked in the running game: **UNVERIFIED** throughout.
//!
//! **The two overrides** ([`bind_overrides`]):
//! - `Engine.AttributeEffect.ApplyAttributeEffects(ContextSource, InAttributeEffects, out OutModifiers,
//!   optional OptionalOverrideContextSource)` is `ow-attr`'s `apply_effects` (NATIVE_ATTRIBUTES.md section 7). The
//!   script **appends** its records to `OutModifiers` (it never empties the array), so the override appends too.
//! - `WillowGame.WillowWeapon.CalculateWeaponBaseValues()` is `ow-items`' `weapon_base_values` on the weapon and its
//!   `DefinitionData.WeaponTypeDefinition` (decision 2026-11-15-24). The script writes each base with the attribute
//!   let (opcode 0x5F), which this VM recomputes in `CombineForm::Vm5F`; the override passes the same form, so a
//!   value the script and the rule write is the same in this world. A weapon without a type is
//!   `NotYet { NotRead }`: the script would read every field through a None context and write the zeros that gives,
//!   a path not ported ([`weapon_type_of`]).
//!
//! **Equip and unequip** (the external effects; an item's or weapon's own effects are applied at its
//! initialisation, `ow-items` W4-STATS):
//! - [`apply_item_external_effects`] is `WillowItem.ApplyAllExternalAttributeEffects`. The source is the owner's
//!   controller ([`equip_controller`]), the override the item. Nothing happens when the item's
//!   `ExternalAttributeModifiers` already holds records (the script's guard against applying twice). Otherwise the
//!   definition's `ExternalAttributeEffects` are appended to `ExternalAttributeModifiers`; then, when
//!   `PartExternalAttributeModifiers` is empty, each part's `ExternalAttributeEffects` to it; then the external
//!   attribute slots ([`external_slots`]). Every read and refusal comes before the first apply, and a failed apply
//!   takes back everything the call added (all or nothing; [`apply_item_external_effects`]); a rollback that fails
//!   is reported as such, naming both errors.
//! - [`apply_weapon_external_effects`] is `WillowWeapon.ApplyAllExternalAttributeEffects`: the same guard, then the
//!   type's and every part's `ExternalAttributeEffects` into `ExternalAttributeModifiers` (no separate part guard),
//!   with the instigator's `Controller` as the source (the weapon's script passes `Instigator.Controller` itself, not
//!   the owning player controller it looks up for the slots), then the external slots.
//! - [`remove_item_external_effects`] and [`remove_weapon_external_effects`] are the `RemoveAll...` scripts: each
//!   record's modifier is removed from its context (`RemoveAttributeModifier`, notification on), then the array is
//!   emptied; an item also empties its part records the same way. Every record is tried and every array emptied
//!   even when one removal fails; the first error is returned afterwards.
//! - The parts are the definition data's part fields in their declared order, the material included and the name
//!   parts not (the script walks them with the native iterator `AllParts`, whose order and whose `bool` argument are
//!   not read: UNVERIFIED).
//! - The records live in the item's own heap arrays as `AppliedAttributeEffect` structs, exactly where the script
//!   keeps them, so a script remove and a Rust remove see the same records.

use std::rc::Rc;

use ow_attr::api::{AppliedEffect, Applied, EffectData, EvalCx, NumForms, RemoveModifierOutcome};
use ow_attr::stack::ModifierType;
use ow_attr::stack::CombineForm;
use ow_core::NotYet;
use ow_items::ItemError;
use ow_object::ids::{ObjRef, PropAddr, StructId};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, NativeFn, Vm};
use ow_vm::error::VmError;

/// The records this module reads.
pub const RECORD: &str = "NATIVE_ATTRIBUTES.md sections 7 and 9; NATIVE_INVENTORY_EQUIP.md (equip); script read 2026-11-16";

/// The script function `ApplyAttributeEffects` (static).
pub const APPLY_ATTRIBUTE_EFFECTS: &str = "Engine.AttributeEffect.ApplyAttributeEffects";
/// The script function `CalculateWeaponBaseValues`.
pub const CALCULATE_WEAPON_BASE_VALUES: &str = "WillowGame.WillowWeapon.CalculateWeaponBaseValues";
/// The struct of one applied record.
pub const APPLIED_STRUCT: &str = "Engine.AttributeEffect.AppliedAttributeEffect";

/// An `ow-items` error as the VM reports it: `NotYet`, attribute, store and heap errors keep their kinds; a data error
/// is a script error naming `what`.
pub fn item_error(what: &str, e: ItemError) -> VmError {
    match e {
        ItemError::NotYet(n) => VmError::NotYet(n),
        ItemError::Attr(a) => VmError::from(a),
        ItemError::Defs(d) => VmError::from(d),
        other => VmError::Script {
            function: what.to_string(),
            message: other.to_string(),
        },
    }
}

/// A data error of this module.
pub(crate) fn bad(what: &str, message: impl Into<String>) -> VmError {
    VmError::Script {
        function: what.to_string(),
        message: message.into(),
    }
}

/// The property `name` of `object`, `None` when its class declares none.
pub(crate) fn read(vm: &Vm, object: ObjRef, name: &str) -> Result<Option<Value>, VmError> {
    let class = vm.space.heap.get(object)?.class;
    match vm.space.prop(class, name) {
        Some(p) => Ok(Some(vm.space.read(object, PropAddr::scalar(p))?.clone())),
        None => Ok(None),
    }
}

/// The property `name` of `object` as it stands; `None` when the object is not there or its class declares none.
pub fn read_prop(vm: &Vm, object: ObjRef, name: &str) -> Option<Value> {
    read(vm, object, name).ok().flatten()
}

/// The property `name` of `object`; a class that declares none is a data error (the script names it).
pub(crate) fn required(vm: &Vm, object: ObjRef, name: &str, what: &str) -> Result<Value, VmError> {
    read(vm, object, name)?.ok_or_else(|| {
        bad(
            what,
            format!(
                "{} declares no {name}",
                ow_attr::eval::class_path(&vm.space, object)
            ),
        )
    })
}

/// Writes `object`'s property `name` (declared, else a data error).
pub(crate) fn write(vm: &mut Vm, object: ObjRef, name: &str, v: Value, what: &str) -> Result<(), VmError> {
    let class = vm.space.heap.get(object)?.class;
    let p = vm.space.prop(class, name).ok_or_else(|| {
        bad(
            what,
            format!(
                "{} declares no {name}",
                ow_attr::eval::class_path(&vm.space, object)
            ),
        )
    })?;
    vm.space.write(object, PropAddr::scalar(p), v)?;
    Ok(())
}

/// An object reference held in `v` (an object or interface value), else a data error.
pub(crate) fn object_of(v: &Value, what: &str) -> Result<Option<ObjRef>, VmError> {
    match v {
        Value::Object(o) | Value::Interface(o) => Ok(*o),
        other => Err(bad(what, format!("expected an object, found {}", other.kind_name()))),
    }
}

/// The object property `name` of `object` (None when it holds none or the class declares none).
pub(crate) fn object_prop(vm: &Vm, object: ObjRef, name: &str, what: &str) -> Result<Option<ObjRef>, VmError> {
    match read(vm, object, name)? {
        Some(v) => object_of(&v, what),
        None => Ok(None),
    }
}

/// Field `name` of the struct `v`.
pub(crate) fn field(vm: &Vm, v: &Value, name: &str) -> Result<Value, VmError> {
    Ok(ow_attr::eval::field(&vm.space, v, name)?.clone())
}

/// One heap `AttributeEffectData` as `ow-attr`'s effect (`AttributeToModify`, `ModifierType`, `BaseModifierValue`).
pub fn effect_data(vm: &Vm, v: &Value) -> Result<EffectData, VmError> {
    let attribute = ow_attr::eval::object("AttributeToModify", &field(vm, v, "AttributeToModify")?)?;
    let kind = match field(vm, v, "ModifierType")? {
        Value::Byte(b) => b,
        other => {
            return Err(bad(
                "AttributeEffectData",
                format!("ModifierType holds {}, not a byte", other.kind_name()),
            ));
        }
    };
    let kind = ModifierType::from_raw(i64::from(kind))
        .ok_or_else(|| bad("AttributeEffectData", format!("ModifierType {kind} is outside the enum")))?;
    let init = ow_attr::eval::init_data(&vm.space, &field(vm, v, "BaseModifierValue")?)?;
    Ok(EffectData::new(attribute, kind, init))
}

/// The effects in the array value `v`.
pub fn effects_in(vm: &Vm, v: &Value) -> Result<Vec<EffectData>, VmError> {
    let mut out = Vec::new();
    for e in ow_attr::eval::elements("an AttributeEffectData array", v)? {
        out.push(effect_data(vm, e)?);
    }
    Ok(out)
}

/// `holder`'s `ExternalAttributeEffects` (none when the class declares none: an empty list).
pub fn external_effects(vm: &Vm, holder: ObjRef) -> Result<Vec<EffectData>, VmError> {
    match read(vm, holder, "ExternalAttributeEffects")? {
        Some(v) => effects_in(vm, &v),
        None => Ok(Vec::new()),
    }
}

/// The records as heap `AppliedAttributeEffect` structs of struct type `ty` (fields `Context`, `Attribute`,
/// `Modifier`).
pub fn applied_values(vm: &Vm, ty: StructId, applied: &Applied) -> Result<Vec<Value>, VmError> {
    let layout = vm
        .space
        .classes
        .struct_layout(ty)
        .ok_or_else(|| bad(APPLIED_STRUCT, "the record struct has no layout"))?;
    let mut entries = Vec::new();
    for name in ["Context", "Attribute", "Modifier"] {
        let id = vm
            .space
            .names
            .find(name)
            .and_then(|n| layout.field(n))
            .map(|(i, _)| i)
            .ok_or_else(|| bad(APPLIED_STRUCT, format!("the record struct has no {name}")))?;
        entries.push(usize::try_from(id).map_err(|_| bad(APPLIED_STRUCT, "a field index past usize"))?);
    }
    let defaults = vm.space.classes.struct_defaults(ty).map_err(ow_object::heap::HeapError::Layout)?;
    let mut out = Vec::with_capacity(applied.0.len());
    for r in &applied.0 {
        let mut v = defaults.clone();
        let fields = &mut v
            .struct_mut()
            .ok_or_else(|| bad(APPLIED_STRUCT, "the record defaults are not a struct"))?
            .fields;
        for (at, o) in entries.iter().zip([r.context, r.attribute, r.modifier]) {
            let slot = fields
                .get_mut(*at)
                .ok_or_else(|| bad(APPLIED_STRUCT, "a field past the struct's values"))?;
            *slot = Value::Object(Some(o));
        }
        out.push(v);
    }
    Ok(out)
}

/// The records held in the array value `v`; a record whose context, attribute or modifier is None is left out (the
/// remove natives change nothing for it: a None context is refused, a None attribute is a None script context).
pub fn applied_from_values(vm: &Vm, v: &Value) -> Result<Applied, VmError> {
    let mut out = Applied::default();
    for r in ow_attr::eval::elements("an AppliedAttributeEffect array", v)? {
        let get = |name: &str| -> Result<Option<ObjRef>, VmError> {
            Ok(ow_attr::eval::object(name, &field(vm, r, name)?)?)
        };
        if let (Some(context), Some(attribute), Some(modifier)) =
            (get("Context")?, get("Attribute")?, get("Modifier")?)
        {
            out.0.push(AppliedEffect {
                context,
                attribute,
                modifier,
            });
        }
    }
    Ok(out)
}

/// The struct type of the elements of `object`'s array property `name` (an `AppliedAttributeEffect` list).
fn record_struct(vm: &Vm, object: ObjRef, name: &str, what: &str) -> Result<StructId, VmError> {
    let class = vm.space.heap.get(object)?.class;
    let p = vm.space.prop(class, name).ok_or_else(|| {
        bad(what, format!("{} declares no {name}", ow_attr::eval::class_path(&vm.space, object)))
    })?;
    match vm.space.classes.decl(p).map(|d| &d.kind) {
        Some(PropKind::Array(inner)) => match inner.as_ref() {
            PropKind::Struct(s) => Ok(*s),
            _ => Err(bad(what, format!("{name} is not an array of records"))),
        },
        _ => Err(bad(what, format!("{name} is not an array of records"))),
    }
}

/// `ApplyAttributeEffects` (module documentation): `effects` applied with `source` and `override_source`, the records
/// appended to `existing` (the out array's value) and returned as the new array value.
pub fn apply_attribute_effects(
    vm: &mut Vm,
    source: Option<ObjRef>,
    effects: &[EffectData],
    existing: &Value,
    override_source: Option<ObjRef>,
    record: StructId,
) -> Result<Value, VmError> {
    let mut out = ow_attr::eval::elements("OutModifiers", existing)?.to_vec();
    let attrs = vm.attrs.clone();
    let applied = attrs.apply_effects(effects, source, override_source, &mut EvalCx::new(vm))?;
    out.extend(applied_values(vm, record, &applied)?);
    Ok(Value::array(out))
}

/// Removes every record of `object`'s arrays `names` from its context, then empties every array (the `RemoveAll...`
/// scripts). Every record is tried (`ow-attr`'s `remove_effects`) and every array emptied whatever happens; the first
/// error is returned afterwards. An array that does not read as records is an error before anything changes.
fn remove_records(vm: &mut Vm, object: ObjRef, names: &[&str], what: &str) -> Result<(), VmError> {
    let mut applied = Applied::default();
    for name in names {
        let v = required(vm, object, name, what)?;
        applied.0.extend(applied_from_values(vm, &v)?.0);
    }
    let attrs = vm.attrs.clone();
    let removed = attrs.remove_effects(applied, &mut EvalCx::new(vm));
    let mut first: Option<VmError> = removed.err().map(VmError::from);
    for name in names {
        if let Err(e) = write(vm, object, name, Value::array(Vec::new()), what) {
            first.get_or_insert(e);
        }
    }
    first.map_or(Ok(()), Err)
}

/// Applies each effect list in order with `source` and the override `item`, adding each call's records to `done`
/// (`ow-attr`'s `apply_effects` undoes a failing call itself).
fn apply_lists(
    vm: &mut Vm,
    source: Option<ObjRef>,
    item: ObjRef,
    lists: &[Vec<EffectData>],
    done: &mut Applied,
) -> Result<(), VmError> {
    let attrs = vm.attrs.clone();
    for effects in lists {
        let applied = attrs.apply_effects(effects, source, Some(item), &mut EvalCx::new(vm))?;
        done.0.extend(applied.0);
    }
    Ok(())
}

/// The error of a failed step whose cleanup failed too: it names both, and it is the cleanup's, not the step's (the
/// state is no longer the one before the call: a modifier no record names may be on a stack, or an actor may be
/// left over), so a caller does not take it for the plain refusal `first` was.
pub(crate) fn cleanup_failed(what: &str, first: &VmError, cleanup: &str) -> VmError {
    VmError::Script {
        function: what.to_string(),
        message: format!(
            "the cleanup after a failure failed, so the state is not the one before the call: {cleanup} (the \
             failure: {})",
            crate::equip::text(first)
        ),
    }
}

/// An error's text and its causes'.
pub(crate) fn text(e: &dyn std::error::Error) -> String {
    let mut t = e.to_string();
    let mut cause = e.source();
    while let Some(c) = cause {
        t.push_str(": ");
        t.push_str(&c.to_string());
        cause = c.source();
    }
    t
}

/// Takes back everything `done` added and writes `originals` back into `item`'s arrays, after the failed equip whose
/// error is `first`.
///
/// Each record's modifier is removed from its context; a modifier whose removal succeeded is freed (no record, and so
/// no script copy, names it after a rollback), as `ow-attr`'s own failed apply frees its modifiers. A removal that
/// fails or is refused leaves its modifier on the heap (a stack may still hold it). Every record is tried and every
/// array written back whatever happens. When all of it succeeds the answer is `first`; otherwise it is
/// [`cleanup_failed`], which names both and outranks `first`.
fn undo(vm: &mut Vm, item: ObjRef, done: Applied, originals: &[(&str, Value)], what: &str, first: VmError) -> VmError {
    let attrs = vm.attrs.clone();
    let mut failures: Vec<String> = Vec::new();
    let mut free: Vec<ObjRef> = Vec::new();
    for r in done.0 {
        let removed = attrs.remove_modifier(r.attribute, Some(r.context), r.modifier, false, &mut EvalCx::new(vm));
        match removed {
            Ok(RemoveModifierOutcome::Removed { .. } | RemoveModifierOutcome::PlainNoOp) => free.push(r.modifier),
            Ok(other) => failures.push(format!("removing a modifier answered {other:?}")),
            Err(e) => failures.push(format!("removing a modifier: {}", text(&e))),
        }
    }
    if !free.is_empty()
        && let Err(e) = vm.space.free_many(&free)
    {
        failures.push(format!("freeing the removed modifiers: {}", text(&e)));
    }
    for (name, v) in originals {
        if let Err(e) = write(vm, item, name, v.clone(), what) {
            failures.push(format!("restoring {name}: {}", text(&e)));
        }
    }
    if failures.is_empty() {
        first
    } else {
        cleanup_failed(what, &first, &failures.join("; "))
    }
}

/// The array value `existing` with `added`'s records appended.
fn appended(vm: &Vm, existing: &Value, ty: StructId, added: &Applied) -> Result<Value, VmError> {
    let mut out = ow_attr::eval::elements("an AppliedAttributeEffect array", existing)?.to_vec();
    out.extend(applied_values(vm, ty, added)?);
    Ok(Value::array(out))
}

fn is_empty_array(v: &Value) -> bool {
    matches!(v, Value::Array(a) if a.is_empty())
}

/// The controller an item's external effects take as their source (`WillowItem.ApplyAllExternalAttributeEffects`):
/// the instigator's `Controller`; when that is None, the owning player controller of the instigator's inventory
/// manager (`WillowInventoryManager.GetOwningPlayerController`, a script function, run here), if the manager is a
/// `WillowInventoryManager`. No instigator: None.
pub fn equip_controller(vm: &mut Vm, item: ObjRef) -> Result<Option<ObjRef>, VmError> {
    const WHAT: &str = "WillowItem.ApplyAllExternalAttributeEffects";
    let Some(instigator) = object_prop(vm, item, "Instigator", WHAT)? else {
        return Ok(None);
    };
    if let Some(c) = object_prop(vm, instigator, "Controller", WHAT)? {
        return Ok(Some(c));
    }
    let Some(manager) = object_prop(vm, instigator, "InvManager", WHAT)? else {
        return Ok(None);
    };
    let class = vm.space.heap.get(manager)?.class;
    let willow = vm.find_class("WillowGame.WillowInventoryManager")?;
    if !vm.space.classes.is_child_of(class, willow) {
        return Ok(None);
    }
    let Some(f) = vm.find_method(class, "GetOwningPlayerController", None)? else {
        return Err(bad(WHAT, "the inventory manager has no GetOwningPlayerController"));
    };
    let r = vm.call(f, Some(manager), Vec::new())?;
    object_of(&r.ret, WHAT)
}

/// What an external-effects apply did.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalOutcome {
    /// The records were already there (the script's guard): nothing applied.
    AlreadyApplied,
    /// Applied: the number of records added to each array (the item's own, its parts').
    Applied { records: usize, part_records: usize },
}

/// The parts of a definition data struct value: its part fields in declared order (module documentation).
fn parts_of(vm: &Vm, data: &Value, fields: &[&str]) -> Result<Vec<ObjRef>, VmError> {
    let mut out = Vec::new();
    for f in fields {
        if let Some(p) = ow_attr::eval::object(f, &field(vm, data, f)?)? {
            out.push(p);
        }
    }
    Ok(out)
}

/// The part fields of `WeaponDefinitionData`, in declared order without the name parts.
pub const WEAPON_PART_FIELDS: [&str; 9] = [
    "BodyPartDefinition",
    "GripPartDefinition",
    "BarrelPartDefinition",
    "SightPartDefinition",
    "StockPartDefinition",
    "ElementalPartDefinition",
    "Accessory1PartDefinition",
    "Accessory2PartDefinition",
    "MaterialPartDefinition",
];

/// The part fields of `ItemDefinitionData`, in declared order without the name parts.
pub const ITEM_PART_FIELDS: [&str; 9] = [
    "AlphaItemPartDefinition",
    "BetaItemPartDefinition",
    "GammaItemPartDefinition",
    "DeltaItemPartDefinition",
    "EpsilonItemPartDefinition",
    "ZetaItemPartDefinition",
    "EtaItemPartDefinition",
    "ThetaItemPartDefinition",
    "MaterialItemPartDefinition",
];

/// `WillowInventory.ApplyExternalSlotEffectModifiers` (NATIVE_ATTRIBUTES.md section 9): applies the item's activated
/// external slots that are not `bRunEffectsAsSkill`. The item's slot list and each slot's `ComputedModifierValue` are
/// made by `InitializeAttributeSlots`, which is not read (NATIVE_INVENTORY_EQUIP.md, "Not read yet"), so a
/// definition that lists such a slot is `NotYet { NotRead }`. A definition that lists none gives the loop nothing to
/// apply, and the call does nothing (that reading: UNVERIFIED).
pub fn external_slots(vm: &Vm, definition: ObjRef) -> Result<(), VmError> {
    const WHAT: &str = "WillowInventory.ApplyExternalSlotEffectModifiers";
    let Some(slots) = read(vm, definition, "AttributeSlotEffects")? else {
        return Ok(());
    };
    for slot in ow_attr::eval::elements("AttributeSlotEffects", &slots)? {
        let flag = |name: &str| -> Result<bool, VmError> {
            Ok(matches!(field(vm, slot, name)?, Value::Bool(true)))
        };
        if flag("bExternalSlot")? && !flag("bRunEffectsAsSkill")? {
            return Err(NotYet::not_read(
                format!("{WHAT}: an external attribute slot (the item's slot list and its computed values, InitializeAttributeSlots)"),
                "NATIVE_INVENTORY_EQUIP.md \"Not read yet\"; NATIVE_ATTRIBUTES.md section 9",
            )
            .into());
        }
    }
    Ok(())
}

/// `WillowItem.ApplyAllExternalAttributeEffects` (module documentation).
///
/// **All or nothing.** Every read comes first (the controller, both record arrays, the parts, every effect list, and
/// the external-slot check, which the script reaches last but which can only refuse here), so a refusal changes
/// nothing. If an apply then fails, everything this call added is removed, its modifiers freed, and both arrays get
/// their old values back, so the apply's own error leaves the stacks and the records as they were and a retry is not
/// answered `AlreadyApplied`. If that rollback fails too, the error says so and names both ([`cleanup_failed`]): the
/// stacks may then hold a modifier no record names.
pub fn apply_item_external_effects(vm: &mut Vm, item: ObjRef) -> Result<ExternalOutcome, VmError> {
    const WHAT: &str = "WillowItem.ApplyAllExternalAttributeEffects";
    const OWN: &str = "ExternalAttributeModifiers";
    const PARTS: &str = "PartExternalAttributeModifiers";
    let controller = equip_controller(vm, item)?;
    let records = required(vm, item, OWN, WHAT)?;
    if !is_empty_array(&records) {
        return Ok(ExternalOutcome::AlreadyApplied);
    }
    let parts_records = required(vm, item, PARTS, WHAT)?;
    let data = required(vm, item, "DefinitionData", WHAT)?;
    let definition = ow_attr::eval::object("ItemDefinition", &field(vm, &data, "ItemDefinition")?)?;
    let ty = record_struct(vm, item, OWN, WHAT)?;
    let pty = record_struct(vm, item, PARTS, WHAT)?;
    let own = match definition {
        Some(d) => vec![external_effects(vm, d)?],
        None => Vec::new(),
    };
    let mut parts = Vec::new();
    if is_empty_array(&parts_records) {
        for part in parts_of(vm, &data, &ITEM_PART_FIELDS)? {
            parts.push(external_effects(vm, part)?);
        }
    }
    if let Some(d) = definition {
        external_slots(vm, d)?;
    }
    let (mut own_done, mut parts_done) = (Applied::default(), Applied::default());
    let run = apply_lists(vm, controller, item, &own, &mut own_done)
        .and_then(|()| apply_lists(vm, controller, item, &parts, &mut parts_done))
        .and_then(|()| {
            let a = appended(vm, &records, ty, &own_done)?;
            let b = appended(vm, &parts_records, pty, &parts_done)?;
            write(vm, item, OWN, a, WHAT)?;
            write(vm, item, PARTS, b, WHAT)
        });
    if let Err(e) = run {
        let mut done = own_done;
        done.0.extend(parts_done.0);
        return Err(undo(vm, item, done, &[(OWN, records), (PARTS, parts_records)], WHAT, e));
    }
    Ok(ExternalOutcome::Applied {
        records: own_done.0.len(),
        part_records: parts_done.0.len(),
    })
}

/// `WillowItem.RemoveAllExternalAttributeEffects` and `RemoveAllPartExternalAttributeEffects`.
pub fn remove_item_external_effects(vm: &mut Vm, item: ObjRef) -> Result<(), VmError> {
    const WHAT: &str = "WillowItem.RemoveAllExternalAttributeEffects";
    remove_records(vm, item, &["ExternalAttributeModifiers", "PartExternalAttributeModifiers"], WHAT)
}

/// `WillowWeapon.ApplyAllExternalAttributeEffects` (module documentation), all or nothing as
/// [`apply_item_external_effects`].
pub fn apply_weapon_external_effects(vm: &mut Vm, weapon: ObjRef) -> Result<ExternalOutcome, VmError> {
    const WHAT: &str = "WillowWeapon.ApplyAllExternalAttributeEffects";
    const OWN: &str = "ExternalAttributeModifiers";
    let source = match object_prop(vm, weapon, "Instigator", WHAT)? {
        Some(i) => object_prop(vm, i, "Controller", WHAT)?,
        None => None,
    };
    let records = required(vm, weapon, OWN, WHAT)?;
    if !is_empty_array(&records) {
        return Ok(ExternalOutcome::AlreadyApplied);
    }
    let data = required(vm, weapon, "DefinitionData", WHAT)?;
    let weapon_type =
        ow_attr::eval::object("WeaponTypeDefinition", &field(vm, &data, "WeaponTypeDefinition")?)?;
    let ty = record_struct(vm, weapon, OWN, WHAT)?;
    let mut lists = Vec::new();
    if let Some(t) = weapon_type {
        lists.push(external_effects(vm, t)?);
    }
    for part in parts_of(vm, &data, &WEAPON_PART_FIELDS)? {
        lists.push(external_effects(vm, part)?);
    }
    if let Some(t) = weapon_type {
        external_slots(vm, t)?;
    }
    let mut done = Applied::default();
    let run = apply_lists(vm, source, weapon, &lists, &mut done).and_then(|()| {
        let a = appended(vm, &records, ty, &done)?;
        write(vm, weapon, OWN, a, WHAT)
    });
    if let Err(e) = run {
        return Err(undo(vm, weapon, done, &[(OWN, records)], WHAT, e));
    }
    Ok(ExternalOutcome::Applied {
        records: done.0.len(),
        part_records: 0,
    })
}

/// `WillowWeapon.RemoveAllExternalAttributeEffects`.
pub fn remove_weapon_external_effects(vm: &mut Vm, weapon: ObjRef) -> Result<(), VmError> {
    remove_records(
        vm,
        weapon,
        &["ExternalAttributeModifiers"],
        "WillowWeapon.RemoveAllExternalAttributeEffects",
    )
}

/// The weapon type of `weapon` (`DefinitionData.WeaponTypeDefinition`). None is `NotYet { NotRead }`: the script
/// reads every type field through a None context (an Accessed None, which gives zero values) and writes the bases
/// from those zeros, and that path (the zero each read gives, `EvaluateInitializationData` on a zeroed struct) is not
/// ported.
pub fn weapon_type_of(vm: &Vm, weapon: ObjRef) -> Result<ObjRef, VmError> {
    let data = required(vm, weapon, "DefinitionData", CALCULATE_WEAPON_BASE_VALUES)?;
    let t = ow_attr::eval::object("WeaponTypeDefinition", &field(vm, &data, "WeaponTypeDefinition")?)?;
    t.ok_or_else(|| {
        NotYet::not_read(
            "CalculateWeaponBaseValues on a weapon without a WeaponTypeDefinition (the script's Accessed None reads \
             and the zero bases it writes from them)",
            RECORD,
        )
        .into()
    })
}

/// The forms of a base written by the attribute let in this VM (module documentation).
pub fn let_forms() -> NumForms {
    NumForms {
        combine: CombineForm::Vm5F,
        ..NumForms::default()
    }
}

/// `CalculateWeaponBaseValues` on `weapon` (module documentation).
pub fn calculate_weapon_base_values(vm: &mut Vm, weapon: ObjRef) -> Result<(), VmError> {
    let weapon_type = weapon_type_of(vm, weapon)?;
    let attrs = vm.attrs.clone();
    let mut cx = EvalCx::new(vm).with_forms(let_forms());
    ow_items::stats::weapon_base_values(weapon, weapon_type, &attrs, &mut cx)
        .map_err(|e| item_error(CALCULATE_WEAPON_BASE_VALUES, e))
}

/// The struct type of `f`'s parameter `index` if it is an array of structs.
fn param_struct(vm: &Vm, f: ow_object::ids::FuncId, index: usize, what: &str) -> Result<StructId, VmError> {
    let kind = vm
        .function(f)
        .and_then(|d| d.params.get(index))
        .map(|p| p.kind.clone());
    match kind {
        Some(PropKind::Array(inner)) => match *inner {
            PropKind::Struct(s) => Ok(s),
            _ => Err(bad(what, format!("parameter {index} is not an array of structs"))),
        },
        _ => Err(bad(what, format!("parameter {index} is not an array of structs"))),
    }
}

/// The override of `ApplyAttributeEffects`: arguments as the script declares them.
fn apply_override(call: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let source = object_of(call.arg(0)?, APPLY_ATTRIBUTE_EFFECTS)?;
    let effects = call.arg(1)?.clone();
    let existing = call.arg(2)?.clone();
    let override_source = if call.has(3) {
        object_of(call.arg(3)?, APPLY_ATTRIBUTE_EFFECTS)?
    } else {
        None
    };
    let record = param_struct(call.vm, call.function, 2, APPLY_ATTRIBUTE_EFFECTS)?;
    let effects = effects_in(call.vm, &effects)?;
    let out = apply_attribute_effects(call.vm, source, &effects, &existing, override_source, record)?;
    call.set_out(2, out)?;
    Ok(Value::None)
}

/// The override of `CalculateWeaponBaseValues`.
fn base_values_override(call: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let weapon = call
        .this
        .ok_or_else(|| bad(CALCULATE_WEAPON_BASE_VALUES, "called without a weapon"))?;
    calculate_weapon_base_values(call.vm, weapon)?;
    Ok(Value::None)
}

/// Binds the two script functions of the module documentation to their Rust rules. Each is loaded first (a function
/// is declared when its class's code loads, and only a declared script function can be overridden), so a VM whose
/// code packages lack either is refused with the loader's error.
pub fn bind_overrides(vm: &mut Vm) -> Result<(), VmError> {
    vm.find_function(APPLY_ATTRIBUTE_EFFECTS)?;
    vm.find_function(CALCULATE_WEAPON_BASE_VALUES)?;
    let apply: NativeFn = Rc::new(apply_override);
    vm.override_script(APPLY_ATTRIBUTE_EFFECTS, apply)?;
    let bases: NativeFn = Rc::new(base_values_override);
    vm.override_script(CALCULATE_WEAPON_BASE_VALUES, bases)
}

#[cfg(test)]
pub(crate) mod tests;
