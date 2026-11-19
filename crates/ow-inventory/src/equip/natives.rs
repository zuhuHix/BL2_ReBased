//! Owner: lane FIX-EQUIP-NATIVES. The natives the installed equip and unequip scripts reach that a record
//! describes, bound in `ParityMode::Off` (decision 2026-11-19-03). AI-assisted (Claude Code).
//!
//! Written from the records named on each item, not from native code; none was checked in the running game:
//! **UNVERIFIED** throughout. The stops come from the installed equip run (`ow-package --inventory-move
//! --equip-run`, lane FIX-EQUIP-NATIVES).
//!
//! - **`AttributeDefinitionBase.ResolveContext(ContextSource)`, `AddAttributeModifier(Context, Modifier,
//!   bSuppressNotify)` and `RemoveAttributeModifier(...)`** (NATIVE_ATTRIBUTES.md section 1) are `ow-attr`'s
//!   `resolve_context`, `add_modifier` and `remove_modifier` on the definition the script calls them on. The add and
//!   remove take the **resolved** context. A None modifier answers false (the record: nothing to add or remove),
//!   and so does a refused one; the script sees the chain's success as the bool. The unequip script
//!   (`AttributeEffect.RemoveAttributeEffects`) reaches the remove; the add and the resolve are what the effect script
//!   calls when it is not overridden (the `--script-diff` harness bound the same three, harness only, decision
//!   2026-11-15-33).
//! - **`WillowInventoryDefinition.GetAttributeSlotMaxActivated()`** answers the definition's
//!   `AttributeSlotMaxActivated`, and 19 for a value outside 1 to 19 ([`MAX_ATTRIBUTE_SLOTS`]). The records give that
//!   clamp for the number of slots the slot natives consider (NATIVE_ATTRIBUTES.md section 9, NATIVE_WEAPON_RULES.md
//!   "Attribute slots"); that this getter is where it is applied is this reading.
//! - **`WillowInventory.ApplyExternalSlotEffectModifiers(ContextSource, MaxSlotsActivated, out AttributeModifiers,
//!   OverrideContextSource)`** (NATIVE_ATTRIBUTES.md section 9): for each of the item's first `MaxSlotsActivated`
//!   `AttributeSlots` that is activated, external, not run as a skill and names an attribute, one modifier of the
//!   slot's `ModifierType` and stored `ComputedModifierValue`, the context resolved from the override first and then
//!   from the source, added, and its record appended to `AttributeModifiers` on success. It is `ow-attr`'s
//!   `apply_effects` on one effect per slot whose value is that constant (a constant evaluates to itself), so the
//!   modifiers, the context order and "a failed add leaves nothing recorded" are that function's. The value is read
//!   as stored; how `InitializeAttributeSlots` computes it is not read (NATIVE_ATTRIBUTES.md section 9, "Open").
//!
//! **Reference** (architecture rule R3): the C++ VM binds none of these, so [`register`] binds nothing there and the
//! stub policy answers, as before.
//!
//! Not bound (a run stops at them): `WillowItem.AllParts` / `WillowWeapon.AllParts`, read by this lane as analyst
//! (NATIVE_EQUIP_PARTS.md; a fresh implementer binds it); `ApplyInternalSlotEffectModifiers` and
//! `InitializeAttributeSlots` (the spawn path, which stops earlier at `ComputeValueOfParts`).

use std::rc::Rc;

use ow_attr::api::{EffectData, EvalCx, InitData};
use ow_attr::stack::ModifierType;
use ow_core::ParityMode;
use ow_object::ids::ObjRef;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, NativeFn, Vm};
use ow_vm::error::VmError;

use super::{applied_values, bad, field, object_of, param_struct, required};

/// The records this module reads.
pub const RECORD: &str = "NATIVE_ATTRIBUTES.md sections 1 and 9; NATIVE_WEAPON_RULES.md (attribute slots)";

/// The native keys [`register`] binds in Off.
pub const RESOLVE_CONTEXT: &str = "AttributeDefinitionBase.ResolveContext(object)";
pub const ADD_MODIFIER: &str = "AttributeDefinitionBase.AddAttributeModifier(object,object,bool)";
pub const REMOVE_MODIFIER: &str = "AttributeDefinitionBase.RemoveAttributeModifier(object,object,bool)";
pub const SLOT_MAX: &str = "WillowInventoryDefinition.GetAttributeSlotMaxActivated()";
pub const EXTERNAL_SLOTS: &str = "WillowInventory.ApplyExternalSlotEffectModifiers(object,int,array,object)";

/// The most attribute slots an item considers, and the answer for a stored count outside 1 to this.
pub const MAX_ATTRIBUTE_SLOTS: i32 = 19;

fn this_of(call: &NativeCall<'_>, what: &str) -> Result<ObjRef, VmError> {
    call.this
        .ok_or_else(|| bad(what, "called without an object"))
}

/// Argument `i` as an object; a missing optional argument is None.
fn arg_object(call: &NativeCall<'_>, i: usize, what: &str) -> Result<Option<ObjRef>, VmError> {
    if !call.has(i) {
        return Ok(None);
    }
    object_of(call.arg(i)?, what)
}

fn suppress(call: &NativeCall<'_>) -> Result<bool, VmError> {
    Ok(call.has(2) && matches!(call.arg(2)?, Value::Bool(true)))
}

fn resolve_context(call: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let attr = this_of(call, RESOLVE_CONTEXT)?;
    let source = arg_object(call, 0, RESOLVE_CONTEXT)?;
    let attrs = call.vm.attrs.clone();
    let context = attrs.resolve_context(attr, source, &mut EvalCx::new(call.vm))?;
    Ok(Value::Object(context))
}

fn add_modifier(call: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let attr = this_of(call, ADD_MODIFIER)?;
    let context = arg_object(call, 0, ADD_MODIFIER)?;
    let Some(modifier) = arg_object(call, 1, ADD_MODIFIER)? else {
        return Ok(Value::Bool(false));
    };
    let s = suppress(call)?;
    let attrs = call.vm.attrs.clone();
    let r = attrs.add_modifier(attr, context, modifier, s, &mut EvalCx::new(call.vm))?;
    Ok(Value::Bool(r.succeeded()))
}

fn remove_modifier(call: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let attr = this_of(call, REMOVE_MODIFIER)?;
    let context = arg_object(call, 0, REMOVE_MODIFIER)?;
    let Some(modifier) = arg_object(call, 1, REMOVE_MODIFIER)? else {
        return Ok(Value::Bool(false));
    };
    let s = suppress(call)?;
    let attrs = call.vm.attrs.clone();
    let r = attrs.remove_modifier(attr, context, modifier, s, &mut EvalCx::new(call.vm))?;
    Ok(Value::Bool(r.succeeded()))
}

/// The count `GetAttributeSlotMaxActivated` answers for a stored `AttributeSlotMaxActivated` (module documentation).
pub fn clamped_slot_max(stored: i32) -> i32 {
    if (1..=MAX_ATTRIBUTE_SLOTS).contains(&stored) {
        stored
    } else {
        MAX_ATTRIBUTE_SLOTS
    }
}

fn slot_max(call: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let def = this_of(call, SLOT_MAX)?;
    match required(call.vm, def, "AttributeSlotMaxActivated", SLOT_MAX)? {
        Value::Int(i) => Ok(Value::Int(clamped_slot_max(i))),
        other => Err(bad(
            SLOT_MAX,
            format!("AttributeSlotMaxActivated holds {}, not an int", other.kind_name()),
        )),
    }
}

fn flag(vm: &Vm, slot: &Value, name: &str) -> Result<bool, VmError> {
    match field(vm, slot, name)? {
        Value::Bool(b) => Ok(b),
        other => Err(bad(
            EXTERNAL_SLOTS,
            format!("{name} holds {}, not a bool", other.kind_name()),
        )),
    }
}

/// The effects `ApplyExternalSlotEffectModifiers` applies for `item`'s first `max` slots (module documentation).
pub fn external_slot_effects(vm: &Vm, item: ObjRef, max: i32) -> Result<Vec<EffectData>, VmError> {
    let slots = required(vm, item, "AttributeSlots", EXTERNAL_SLOTS)?;
    let slots = ow_attr::eval::elements("AttributeSlots", &slots)?;
    let count = usize::try_from(max.max(0)).unwrap_or(0).min(slots.len());
    let mut out = Vec::new();
    for slot in &slots[..count] {
        if !flag(vm, slot, "bActivated")?
            || !flag(vm, slot, "bExternalSlot")?
            || flag(vm, slot, "bRunEffectsAsSkill")?
        {
            continue;
        }
        let Some(attribute) = ow_attr::eval::object("AttributeToModify", &field(vm, slot, "AttributeToModify")?)? else {
            continue;
        };
        let kind = match field(vm, slot, "ModifierType")? {
            Value::Byte(b) => b,
            other => {
                return Err(bad(
                    EXTERNAL_SLOTS,
                    format!("ModifierType holds {}, not a byte", other.kind_name()),
                ));
            }
        };
        let kind = ModifierType::from_raw(i64::from(kind))
            .ok_or_else(|| bad(EXTERNAL_SLOTS, format!("ModifierType {kind} is outside the enum")))?;
        let value = match field(vm, slot, "ComputedModifierValue")? {
            Value::Float(f) => f,
            other => {
                return Err(bad(
                    EXTERNAL_SLOTS,
                    format!("ComputedModifierValue holds {}, not a float", other.kind_name()),
                ));
            }
        };
        out.push(EffectData::new(Some(attribute), kind, InitData::constant(value)));
    }
    Ok(out)
}

fn external_slots(call: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let item = this_of(call, EXTERNAL_SLOTS)?;
    let source = arg_object(call, 0, EXTERNAL_SLOTS)?;
    let max = match call.arg(1)? {
        Value::Int(i) => *i,
        other => {
            return Err(bad(
                EXTERNAL_SLOTS,
                format!("MaxSlotsActivated holds {}, not an int", other.kind_name()),
            ));
        }
    };
    let existing = call.arg(2)?.clone();
    let override_source = arg_object(call, 3, EXTERNAL_SLOTS)?;
    let record = param_struct(call.vm, call.function, 2, EXTERNAL_SLOTS)?;
    let effects = external_slot_effects(call.vm, item, max)?;
    let mut out = ow_attr::eval::elements("AttributeModifiers", &existing)?.to_vec();
    if !effects.is_empty() {
        let attrs = call.vm.attrs.clone();
        let applied = attrs.apply_effects(&effects, source, override_source, &mut EvalCx::new(call.vm))?;
        out.extend(applied_values(call.vm, record, &applied)?);
    }
    call.set_out(2, Value::array(out))?;
    Ok(Value::None)
}

/// Binds the module's natives in `ParityMode::Off`; Reference binds nothing (module documentation).
pub fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    let natives: [(&str, NativeFn); 5] = [
        (RESOLVE_CONTEXT, Rc::new(resolve_context)),
        (ADD_MODIFIER, Rc::new(add_modifier)),
        (REMOVE_MODIFIER, Rc::new(remove_modifier)),
        (SLOT_MAX, Rc::new(slot_max)),
        (EXTERNAL_SLOTS, Rc::new(external_slots)),
    ];
    for (key, f) in natives {
        vm.register_native(key, f)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
