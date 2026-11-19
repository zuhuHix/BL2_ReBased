//! Owner: lane IMPL-APPLY-SLOT-EFFECT-MODIFIERS. AI-assisted (Claude Code).
//! `WillowInventory.ApplyInternalSlotEffectModifiers(bBackupSlotEffectsApplied, MaxSlotsActivated, out
//! AttributeModifiers)` in `ParityMode::Off`, implemented from NATIVE_APPLY_SLOT_EFFECT_MODIFIERS.md alone (decision
//! 2026-11-19-97; the analyst was lane ANALYST-APPLY-SLOT-EFFECT-MODIFIERS, decision 2026-11-19-87, PR #471 at
//! the final text on main, a3c8e22). Every rule is the note's reading of the executable, **UNVERIFIED** in the running game.
//!
//! **What it does** (the note's section 1):
//! 1. with the flag false, each record of `AppliedAttributeSlotEffects` whose context and attribute are set: the
//!    attribute removes the record's modifier from the context, notifications on, its result ignored;
//! 2. `AppliedAttributeSlotEffects` is emptied, always;
//! 3. the first `MaxSlotsActivated` slots (none for 0 or less), in index order: a slot that is `bActivated`, not
//!    `bExternalSlot`, and has an `AttributeToModify` (`bRunEffectsAsSkill` is not read) gets a new
//!    `Core.AttributeModifier` of its `ModifierType` and stored `ComputedModifierValue`, added, notifications on, to
//!    the context its attribute's chain resolves from the item. A true add appends a record to the out array (after
//!    what it already holds) and, with the flag true, to `AppliedAttributeSlotEffects` too. Nothing else is recorded.
//!
//! **The note's Off assumptions:** B1, a count above the declared dimension stops at it; B2, every qualifying slot's
//! context is resolved in the preflight, before the removals; B3, a modifier is made only for a slot whose context
//! resolved; one whose add answers false keeps its object where the chain left it (nothing rolled back, not recorded).
//! The modifier's outer is None: the port has no transient package (the native's is that package).
//!
//! **Preflight**, P0 to P9 of the note's section 3, before any write: the out argument's record struct; the slot
//! array's shape (five fields, not `bRunEffectsAsSkill`); `AppliedAttributeSlotEffects`'s shape; the modifier class
//! with `Type` and `Value`; the count; each slot's five values (a `ModifierType` outside the enum is a data error);
//! each record's three objects (with the flag false, a record is a removal only when its context, attribute and
//! modifier are all set; one with a None modifier is a no-op); P0, every definition the call will touch loaded
//! ([`load_attribute`]; the note's ruling: definition loads are not writes), after the reads that name them; each
//! qualifying slot's context; and the chain check of every entry of every planned removal and add
//! ([`ow_attr::AttributeSystem::check_modifier_chain`], through each resolver's write-free
//! `ValueResolver::probe_storage`; no outcome predicted). The noun resolver's probe neither creates a missing entry
//! nor misses a replicated slot's refusal (review r1 of #483); definition loads it may make are not writes under the
//! note's ruling. The installed weapons' slot
//! attributes hold their resolvers as store handles not yet loaded, so P0 is needed there.
//!
//! **Writes**, W1 to W3, in the note's order. A `NotYet` after the first write is terminal, matched by its typed
//! variant, keeps its kind and record and gets the "stopped after partial writes: " prefix
//! (`ow_world::pools::after_partial_writes`); nothing is rolled back; other errors propagate unchanged. Every failure
//! of W1, the first removal's included, is marked (review r2 of #483): a removal writes the stack and the value before
//! its change notification runs, so its `NotYet` may follow a write. With P0, P8 and P9 passed, such a `NotYet` comes
//! only from a stack write's own consequences (a change notification's script), as the note's section 3 says.
//!
//! **Reference** (architecture R3): unbound, the stub policy answers it, as before.

use std::rc::Rc;

use ow_attr::api::{Applied, AppliedEffect, EvalCx};
use ow_attr::stack::ModifierType;
use ow_core::ParityMode;
use ow_object::ids::{ObjRef, PropAddr, PropId, StructId};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;

use super::natives::declared_slots;
use super::{applied_values, bad, field, object_of, param_struct};

/// The record this native follows.
pub const RECORD: &str = "NATIVE_APPLY_SLOT_EFFECT_MODIFIERS.md";
/// The native key [`register`] binds in Off.
pub const INTERNAL_SLOTS: &str = "WillowInventory.ApplyInternalSlotEffectModifiers(bool,int,array)";
/// The class of the modifiers it makes.
pub const MODIFIER_CLASS: &str = "Core.AttributeModifier";

/// The slot fields this native reads (the note's P2: not `bRunEffectsAsSkill`).
const FIELDS: [(&str, &str); 5] = [
    ("bActivated", "a bool"),
    ("bExternalSlot", "a bool"),
    ("AttributeToModify", "an object"),
    ("ModifierType", "a byte"),
    ("ComputedModifierValue", "a float"),
];

fn bool_field(vm: &Vm, v: &Value, name: &str, i: usize) -> Result<bool, VmError> {
    match field(vm, v, name)? {
        Value::Bool(b) => Ok(b),
        other => Err(bad(INTERNAL_SLOTS, format!("slot {i}: {name} holds a {}, not a bool", other.kind_name()))),
    }
}

/// One qualifying slot: its attribute, its modifier's type and value.
struct Slot {
    attribute: ObjRef,
    kind: ModifierType,
    value: f32,
}

/// The slots P5 and P6 read, the qualifying ones only, in index order.
fn qualifying(vm: &Vm, item: ObjRef, max: i32) -> Result<Vec<Slot>, VmError> {
    let mut out = Vec::new();
    for (i, slot) in declared_slots(vm, item, max, &FIELDS, INTERNAL_SLOTS)?.iter().enumerate() {
        let activated = bool_field(vm, slot, "bActivated", i)?;
        let external = bool_field(vm, slot, "bExternalSlot", i)?;
        let attribute = object_of(&field(vm, slot, "AttributeToModify")?, INTERNAL_SLOTS)
            .map_err(|_| bad(INTERNAL_SLOTS, format!("slot {i}: AttributeToModify holds no object")))?;
        let raw = match field(vm, slot, "ModifierType")? {
            Value::Byte(b) => b,
            other => {
                return Err(bad(INTERNAL_SLOTS, format!("slot {i}: ModifierType holds a {}, not a byte", other.kind_name())));
            }
        };
        let value = match field(vm, slot, "ComputedModifierValue")? {
            Value::Float(f) => f,
            other => {
                return Err(bad(
                    INTERNAL_SLOTS,
                    format!("slot {i}: ComputedModifierValue holds a {}, not a float", other.kind_name()),
                ));
            }
        };
        if let (true, false, Some(attribute)) = (activated, external, attribute) {
            // Off assumption (review r1 of #483), UNVERIFIED: a qualifying slot's type outside the enum is a data
            // error. The note does not say what the native makes of one; `ow-attr`'s stack treats such a modifier as a
            // no-op. A slot that does not qualify is not checked.
            let kind = ModifierType::from_raw(i64::from(raw))
                .ok_or_else(|| bad(INTERNAL_SLOTS, format!("slot {i}: ModifierType {raw} is outside the enum")))?;
            out.push(Slot { attribute, kind, value });
        }
    }
    Ok(out)
}

/// P3: `AppliedAttributeSlotEffects`, a dynamic array of a struct whose `Context`, `Attribute` and `Modifier` are
/// objects: its property and record struct.
fn applied_list(vm: &Vm, item: ObjRef) -> Result<(PropId, StructId), VmError> {
    let class = vm.space.heap.get(item)?.class;
    let declared = |m: &str| bad(INTERNAL_SLOTS, format!("{} {m}", ow_attr::eval::class_path(&vm.space, item)));
    let p = vm.space.prop(class, "AppliedAttributeSlotEffects").ok_or_else(|| declared("declares no AppliedAttributeSlotEffects"))?;
    let ty = match vm.space.classes.decl(p).map(|d| &d.kind) {
        Some(PropKind::Array(inner)) => match **inner {
            PropKind::Struct(t) => t,
            _ => return Err(declared("declares AppliedAttributeSlotEffects as an array of something other than records")),
        },
        _ => return Err(declared("declares AppliedAttributeSlotEffects as something other than a dynamic array")),
    };
    record_shape(vm, ty, "AppliedAttributeSlotEffects")?;
    Ok((p, ty))
}

/// A record struct must declare `Context`, `Attribute` and `Modifier` as objects.
fn record_shape(vm: &Vm, ty: StructId, what: &str) -> Result<(), VmError> {
    let layout = vm
        .space
        .classes
        .struct_layout(ty)
        .ok_or_else(|| bad(INTERNAL_SLOTS, format!("{what}'s record struct has no layout")))?;
    for name in ["Context", "Attribute", "Modifier"] {
        let ok = vm
            .space
            .names
            .find(name)
            .and_then(|id| layout.field(id))
            .is_some_and(|(_, d)| matches!(d.kind, PropKind::Object { .. }));
        if !ok {
            return Err(bad(INTERNAL_SLOTS, format!("{what}'s record struct does not declare {name} as an object")));
        }
    }
    Ok(())
}

/// P7: the records of `AppliedAttributeSlotEffects` as (context, attribute, modifier); a field holding anything but an
/// object or None is a data error naming the record.
fn records(vm: &Vm, item: ObjRef, p: PropId) -> Result<Vec<(Option<ObjRef>, Option<ObjRef>, Option<ObjRef>)>, VmError> {
    let v = vm.space.read(item, PropAddr::scalar(p))?.clone();
    let mut out = Vec::new();
    for (i, r) in ow_attr::eval::elements("AppliedAttributeSlotEffects", &v)?.iter().enumerate() {
        let get = |n: &str| -> Result<Option<ObjRef>, VmError> {
            match field(vm, r, n)? {
                Value::Object(o) => Ok(o),
                Value::None => Ok(None),
                other => Err(bad(INTERNAL_SLOTS, format!("record {i}: {n} holds a {}, not an object", other.kind_name()))),
            }
        };
        out.push((get("Context")?, get("Attribute")?, get("Modifier")?));
    }
    Ok(out)
}

/// P4: the modifier class, with `Type` a byte and `Value` a float. Its metadata must already be laid out: a class
/// lookup that would lay it out now is answered by the store at the write, so here a class the space does not know
/// is looked up through the VM, whose own error is the answer.
fn modifier_class(vm: &mut Vm) -> Result<(), VmError> {
    let class = match vm.space.classes.by_path(MODIFIER_CLASS).filter(|c| vm.space.classes.layout(*c).is_defined()) {
        Some(c) => c,
        None => vm.find_class(MODIFIER_CLASS)?,
    };
    for (name, want, ok) in [
        ("Type", "a byte", (|k: &PropKind| matches!(k, PropKind::Byte { .. })) as fn(&PropKind) -> bool),
        ("Value", "a float", |k: &PropKind| matches!(k, PropKind::Float)),
    ] {
        let fits = vm.space.prop(class, name).and_then(|p| vm.space.classes.decl(p)).is_some_and(|d| ok(&d.kind));
        if !fits {
            return Err(bad(INTERNAL_SLOTS, format!("{MODIFIER_CLASS} does not declare {name} as {want}")));
        }
    }
    Ok(())
}

/// P0 (the note's ruling, review r1 of #471: definition loads are not writes): `attr` and every entry of its context and
/// value resolver chains, loaded if they are store handles not loaded yet; a load's `NotYet` or error propagates as the
/// store gives it (`VmError::from`), before any write.
fn load_attribute(vm: &mut Vm, attr: ObjRef) -> Result<(), VmError> {
    let load = |vm: &mut Vm, o: ObjRef| -> Result<(), VmError> {
        if vm.defs.is_unloaded(&vm.space, o) {
            let r = vm.defs.ensure_loaded(&mut vm.space, o);
            let log = vm.defs.take_log();
            vm.log.extend(log);
            r.map_err(VmError::from)?;
        }
        Ok(())
    };
    load(vm, attr)?;
    for chain in ["ContextResolverChain", "ValueResolverChain"] {
        let Some(v) = super::read(vm, attr, chain)? else { continue };
        let entries: Vec<ObjRef> = ow_attr::eval::elements(chain, &v)?
            .iter()
            .filter_map(|e| object_of(e, INTERNAL_SLOTS).ok().flatten())
            .collect();
        for o in entries {
            load(vm, o)?;
        }
    }
    Ok(())
}

/// What the plan decided.
struct Plan {
    applied_prop: PropId,
    applied_ty: StructId,
    out_ty: StructId,
    removals: Vec<(ObjRef, ObjRef, ObjRef)>,
    adds: Vec<(Slot, ObjRef)>,
}

fn plan(vm: &mut Vm, call_fn: ow_object::ids::FuncId, item: ObjRef, flag: bool, max: i32) -> Result<Plan, VmError> {
    // P1: the out argument's record struct.
    let out_ty = param_struct(vm, call_fn, 2, INTERNAL_SLOTS)?;
    record_shape(vm, out_ty, "the out argument")?;
    // P2, P5, P6: the slots.
    let slots = qualifying(vm, item, max)?;
    // P3: the applied list.
    let (applied_prop, applied_ty) = applied_list(vm, item)?;
    // P4: the modifier class.
    modifier_class(vm)?;
    // P7: the records.
    let recs = records(vm, item, applied_prop)?;
    let removals: Vec<(ObjRef, ObjRef, ObjRef)> = if flag {
        Vec::new()
    } else {
        recs.into_iter()
            .filter_map(|(c, a, m)| match (c, a, m) {
                (Some(c), Some(a), Some(m)) => Some((c, a, m)),
                _ => None,
            })
            .collect()
    };
    // P0: every definition the call will touch, loaded (the note's ruling: definition loads are not writes). The
    // slots and records P2 to P7 read name them, so P0 runs after those reads and before P8.
    for a in slots.iter().map(|s| s.attribute).chain(removals.iter().map(|r| r.1)) {
        load_attribute(vm, a)?;
    }
    let attrs = vm.attrs.clone();
    // P8: each qualifying slot's context, from the item (B2); an unresolved slot is skipped and gets no modifier (B3).
    let mut adds = Vec::new();
    for slot in slots {
        if let Some(context) = attrs.resolve_context(slot.attribute, Some(item), &mut EvalCx::new(vm))? {
            adds.push((slot, context));
        }
    }
    // P9: the chain check of every entry of every planned removal and add (write-free probes; no outcome predicted).
    for &(context, attribute, _) in &removals {
        attrs.check_modifier_chain(attribute, context, &mut EvalCx::new(vm))?;
    }
    for (slot, context) in &adds {
        attrs.check_modifier_chain(slot.attribute, *context, &mut EvalCx::new(vm))?;
    }
    Ok(Plan { applied_prop, applied_ty, out_ty, removals, adds })
}

/// W1 to W3 (module documentation); `existing` is what the out argument held. Returns the out array's new value.
fn writes(vm: &mut Vm, item: ObjRef, flag: bool, plan: Plan, existing: Vec<Value>) -> Result<Value, VmError> {
    let attrs = vm.attrs.clone();
    // W1: the removals, notifications on, results ignored. Every failure from here on is marked (review r2 of #483):
    // a removal writes the stack and the value before its change notification runs, so even the first removal's
    // `NotYet` can follow a write; the refusals that come before any write were decided by P9.
    for &(context, attribute, modifier) in &plan.removals {
        attrs
            .remove_modifier(attribute, Some(context), modifier, false, &mut EvalCx::new(vm))
            .map_err(|e| ow_world::pools::after_partial_writes(e.into()))?;
    }
    // W2: the list emptied. From here on every failure comes after a write.
    vm.space.write(item, PropAddr::scalar(plan.applied_prop), Value::array(Vec::new()))?;
    writes_after(vm, item, flag, plan, existing).map_err(ow_world::pools::after_partial_writes)
}

/// W3, after W2's write (module documentation).
fn writes_after(vm: &mut Vm, item: ObjRef, flag: bool, plan: Plan, existing: Vec<Value>) -> Result<Value, VmError> {
    let attrs = vm.attrs.clone();
    // W3: the adds.
    let mut out = existing;
    let mut backup = Applied::default();
    for (slot, context) in plan.adds {
        let modifier = attrs.new_modifier(None, slot.kind, slot.value, &mut EvalCx::new(vm))?;
        let added = attrs.add_modifier(slot.attribute, Some(context), modifier, false, &mut EvalCx::new(vm))?;
        if !added.succeeded() {
            continue;
        }
        let record = Applied(vec![AppliedEffect { context, attribute: slot.attribute, modifier }]);
        out.extend(applied_values(vm, plan.out_ty, &record)?);
        if flag {
            backup.0.extend(record.0);
            let list = applied_values(vm, plan.applied_ty, &backup)?;
            vm.space.write(item, PropAddr::scalar(plan.applied_prop), Value::array(list))?;
        }
    }
    Ok(Value::array(out))
}

fn native(call: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let item = call.this.ok_or_else(|| bad(INTERNAL_SLOTS, "called without an inventory"))?;
    let flag = call.arg(0)?.truth(&call.vm.space.names);
    let max = match call.arg(1)? {
        Value::Int(i) => *i,
        other => return Err(bad(INTERNAL_SLOTS, format!("MaxSlotsActivated holds a {}, not an int", other.kind_name()))),
    };
    let existing = ow_attr::eval::elements("AttributeModifiers", call.arg(2)?)?.to_vec();
    let function = call.function;
    let plan = plan(call.vm, function, item, flag, max)?;
    let out = writes(call.vm, item, flag, plan, existing)?;
    call.set_out(2, out)?;
    Ok(Value::None)
}

/// Binds the native in `ParityMode::Off`; Reference binds nothing (module documentation).
pub fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    vm.register_native(INTERNAL_SLOTS, Rc::new(native))
}

#[cfg(test)]
#[path = "internal_slots_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "internal_slots_store_tests.rs"]
mod store_tests;
