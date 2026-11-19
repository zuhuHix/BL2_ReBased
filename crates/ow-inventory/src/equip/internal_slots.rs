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
//! **Preflight**, P0 to P9 of the note's section 3, before any write, and writing nothing: no stack, noun entry,
//! generator state or heap object (the hub's ruling, lane FIX-483-FOLLOWUP; definition loads are not writes, the
//! note's ruling). In this order (F3: the loads first, so a load's refusal is the answer, not an independent check's):
//! 1. the reads that name what P0 loads: the slot array's shape (five fields, not `bRunEffectsAsSkill`), the count and
//!    each slot's five values (a qualifying slot's `ModifierType` outside the enum is a data error);
//!    `AppliedAttributeSlotEffects`'s shape and each record's three objects (with the flag false, a record is a
//!    removal only when its context, attribute and modifier are all set; one with a None modifier is a no-op);
//! 2. P0 ([`load_attribute`]): each qualifying slot's attribute with its context and value chains (an add resolves
//!    its context), and each removal's attribute with its value chain only (a removal uses the record's context, so
//!    its attribute's context chain is not touched);
//! 3. the independent checks: P1, the out argument is an `out` parameter, an array of `AppliedAttributeEffect`
//!    records (F5); P4, the modifier class with `Type` and `Value`;
//! 4. P8, each qualifying slot's context; P9, the chain check of every entry of every planned removal and add
//!    ([`ow_attr::AttributeSystem::check_modifier_chain`], through each resolver's write-free
//!    `ValueResolver::probe_storage` for that operation; no outcome predicted). The noun resolver's probe creates no
//!    missing entry and evaluates no default (F1: it validates it), and refuses a replicated slot only where the
//!    operation would rewrite the entry (F6: not for the removal of a modifier the entry does not hold).
//!
//! The installed weapons' slot attributes hold their resolvers as store handles not yet loaded, so P0 is needed there.
//!
//! **Writes**, W1 to W3, in the note's order. A `NotYet` after an actual write is terminal, matched by its typed
//! variant, keeps its kind and record and gets the "stopped after partial writes: " prefix
//! (`ow_world::pools::after_partial_writes`); nothing is rolled back; other errors propagate unchanged. The prefix goes
//! on **only after an actual write** (the hub's ruling, F4, superseding decision 2026-11-19-97's "every W1 failure is
//! marked"): in W1 a removal may refuse in its change notification with nothing written (a plain attribute's removal
//! writes nothing before it notifies), or after its stack write, and a notification's script may write before it
//! refuses, so W1 compares the heap's change count and the generator state with their values at its start
//! ([`Mark`]). From W2 on a write has been made, so every `NotYet` is marked. Each W3 record is appended at once to
//! the live out argument and, with the flag true, to `AppliedAttributeSlotEffects` as they stand then (F2), so a later
//! slot's refusal keeps the earlier records, and no final assignment overwrites what a notification's script wrote
//! through the out argument's alias.
//!
//! **Reference** (architecture R3): unbound, the stub policy answers it, as before.

use std::rc::Rc;

use ow_attr::api::{Applied, AppliedEffect, EvalCx, ProbeOp};
use ow_attr::stack::ModifierType;
use ow_core::ParityMode;
use ow_core::rand::GameRng;
use ow_object::ids::{FuncId, ObjRef, PropAddr, PropId, StructId};
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

/// P0 (the note's ruling, review r1 of #471: definition loads are not writes): `attr` and every entry of its value
/// resolver chain, and of its context resolver chain when `with_context` (an add; a removal uses its record's context,
/// F3), loaded if they are store handles not loaded yet; a load's `NotYet` or error propagates as the store gives it
/// (`VmError::from`), before any write.
fn load_attribute(vm: &mut Vm, attr: ObjRef, with_context: bool) -> Result<(), VmError> {
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
    let chains: &[&str] = if with_context { &["ContextResolverChain", "ValueResolverChain"] } else { &["ValueResolverChain"] };
    for &chain in chains {
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

/// P1 (F5): the third parameter is an `out` parameter, an array of `AppliedAttributeEffect` records whose `Context`,
/// `Attribute` and `Modifier` are objects. Its record struct.
fn out_record(vm: &Vm, f: FuncId) -> Result<StructId, VmError> {
    if !vm.function(f).and_then(|d| d.params.get(2)).is_some_and(|p| p.is_out()) {
        return Err(bad(INTERNAL_SLOTS, "parameter 2 (AttributeModifiers) is not an out parameter"));
    }
    let ty = param_struct(vm, f, 2, INTERNAL_SLOTS)?;
    let path = vm.space.classes.struct_layout(ty).map(|l| l.path().clone());
    if !path.as_ref().is_some_and(|p| p.name().eq_ignore_ascii_case("AppliedAttributeEffect")) {
        let named = path.map_or_else(|| "a struct with no layout".to_string(), |p| p.as_str().to_string());
        return Err(bad(INTERNAL_SLOTS, format!("the out argument's records are {named}, not AppliedAttributeEffect")));
    }
    record_shape(vm, ty, "the out argument")?;
    Ok(ty)
}

/// What the plan decided.
struct Plan {
    applied_prop: PropId,
    applied_ty: StructId,
    out_ty: StructId,
    removals: Vec<(ObjRef, ObjRef, ObjRef)>,
    adds: Vec<(Slot, ObjRef)>,
}

/// The preflight, in the order of the module documentation. Writes nothing.
fn plan(vm: &mut Vm, call_fn: FuncId, item: ObjRef, flag: bool, max: i32) -> Result<Plan, VmError> {
    // The reads that name what P0 loads: P2, P5 and P6 (the slots), P3 and P7 (the applied list and its records).
    let slots = qualifying(vm, item, max)?;
    let (applied_prop, applied_ty) = applied_list(vm, item)?;
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
    // P0, before the independent checks (F3): an add's attribute with both chains, a removal's with its value chain.
    for slot in &slots {
        load_attribute(vm, slot.attribute, true)?;
    }
    for removal in &removals {
        load_attribute(vm, removal.1, false)?;
    }
    // P1 (F5) and P4.
    let out_ty = out_record(vm, call_fn)?;
    modifier_class(vm)?;
    let attrs = vm.attrs.clone();
    // P8: each qualifying slot's context, from the item (B2); an unresolved slot is skipped and gets no modifier (B3).
    let mut adds = Vec::new();
    for slot in slots {
        if let Some(context) = attrs.resolve_context(slot.attribute, Some(item), &mut EvalCx::new(vm))? {
            adds.push((slot, context));
        }
    }
    // P9: the chain check of every entry of every planned removal and add (write-free probes of that operation; no
    // outcome predicted).
    for &(context, attribute, modifier) in &removals {
        attrs.check_modifier_chain(attribute, context, ProbeOp::Remove(modifier), &mut EvalCx::new(vm))?;
    }
    for (slot, context) in &adds {
        attrs.check_modifier_chain(slot.attribute, *context, ProbeOp::Add, &mut EvalCx::new(vm))?;
    }
    Ok(Plan { applied_prop, applied_ty, out_ty, removals, adds })
}

/// What W1 compares against to tell whether it wrote anything (F4): the heap's change count and the generator state
/// at its start. The count may say "changed" for a mutable borrow that changed nothing
/// (`ow_object::heap::Heap::mutations`), so a refusal is then marked although nothing changed; it never misses a
/// change. The generator is compared whole (a draw advances it).
struct Mark {
    heap: u64,
    rng: GameRng,
}

impl Mark {
    fn new(vm: &Vm) -> Self {
        Mark { heap: vm.space.heap.mutations(), rng: vm.rng.clone() }
    }

    fn wrote(&self, vm: &Vm) -> bool {
        vm.space.heap.mutations() != self.heap || vm.rng != self.rng
    }
}

/// W1 to W3 (module documentation).
fn writes(call: &mut NativeCall<'_>, item: ObjRef, flag: bool, plan: &Plan) -> Result<(), VmError> {
    let attrs = call.vm.attrs.clone();
    // W1: the removals, notifications on, results ignored. A refusal is marked only after an actual write (F4).
    let mark = Mark::new(call.vm);
    for &(context, attribute, modifier) in &plan.removals {
        if let Err(e) = attrs.remove_modifier(attribute, Some(context), modifier, false, &mut EvalCx::new(call.vm)) {
            let e = VmError::from(e);
            return Err(if mark.wrote(call.vm) { ow_world::pools::after_partial_writes(e) } else { e });
        }
    }
    // W2: the list emptied. From here on every refusal comes after a write.
    call.vm.space.write(item, PropAddr::scalar(plan.applied_prop), Value::array(Vec::new()))?;
    writes_after(call, item, flag, plan).map_err(ow_world::pools::after_partial_writes)
}

/// W3, after W2's write (module documentation): each record appended at once to the live out argument and, with the
/// flag true, to the live backup list (F2).
fn writes_after(call: &mut NativeCall<'_>, item: ObjRef, flag: bool, plan: &Plan) -> Result<(), VmError> {
    let attrs = call.vm.attrs.clone();
    let list_at = PropAddr::scalar(plan.applied_prop);
    for (slot, context) in &plan.adds {
        let modifier = attrs.new_modifier(None, slot.kind, slot.value, &mut EvalCx::new(call.vm))?;
        let added = attrs.add_modifier(slot.attribute, Some(*context), modifier, false, &mut EvalCx::new(call.vm))?;
        if !added.succeeded() {
            continue;
        }
        let record = Applied(vec![AppliedEffect { context: *context, attribute: slot.attribute, modifier }]);
        // The out argument as it stands now: from script, the caller's storage, which the add's notification may have
        // changed through another alias.
        let mut out = ow_attr::eval::elements("AttributeModifiers", call.arg(2)?)?.to_vec();
        out.extend(applied_values(call.vm, plan.out_ty, &record)?);
        call.set_out(2, Value::array(out))?;
        if flag {
            let mut list = ow_attr::eval::elements("AppliedAttributeSlotEffects", call.vm.space.read(item, list_at)?)?.to_vec();
            list.extend(applied_values(call.vm, plan.applied_ty, &record)?);
            call.vm.space.write(item, list_at, Value::array(list))?;
        }
    }
    Ok(())
}

fn native(call: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let item = call.this.ok_or_else(|| bad(INTERNAL_SLOTS, "called without an inventory"))?;
    let flag = call.arg(0)?.truth(&call.vm.space.names);
    let max = match call.arg(1)? {
        Value::Int(i) => *i,
        other => return Err(bad(INTERNAL_SLOTS, format!("MaxSlotsActivated holds a {}, not an int", other.kind_name()))),
    };
    let function = call.function;
    let plan = plan(call.vm, function, item, flag, max)?;
    // P1's value half: the out argument holds an array (its records are read when W3 appends to it).
    ow_attr::eval::elements("AttributeModifiers", call.arg(2)?)?;
    writes(call, item, flag, &plan)?;
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
