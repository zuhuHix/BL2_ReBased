//! Owner: lane FIX-EQUIP-NATIVES. The natives the installed equip and unequip scripts reach that a record
//! describes, bound in `ParityMode::Off` (decision 2026-11-19-03). AI-assisted (Claude Code).
//!
//! Written from the records named on each item, not from native code; none was checked in the running game:
//! **UNVERIFIED** throughout. The stops come from the installed equip run (`ow-package --inventory-move
//! --equip-run`, lane FIX-EQUIP-NATIVES).
//!
//! - **`AttributeDefinitionBase.ResolveContext`** is the shared rule with its preflight, [`resolve_context_native`]
//!   (lane IMPL-RESOLVE-CONTEXT, NATIVE_RESOLVE_CONTEXT.md section 5); the action-skill pack binds the same wrapper.
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
//!   The slots are read as the item's class declares them, a static array of slot structs (`AttributeSlots[19]` in the
//!   installed `WillowInventory`), the first `MaxSlotsActivated` of them and at most the declaration's dimension
//!   ([`attribute_slots`], lane FIX-ATTRIBUTE-SLOTS, decision 2026-11-19-49). The first version read them as a dynamic
//!   array, which the installed class is not. The declaration, including the six slot fields and their kinds, is
//!   checked before any slot is read, and no slot past the count is read (review r1 of #438).
//!
//! **Reference** (architecture rule R3): the C++ VM binds none of these, so [`register`] binds nothing there and the
//! stub policy answers, as before.
//!
//! - **`WillowWeapon.AllParts(DefinitionData, out Part, bIncludeNameParts)` and `WillowItem.AllParts(...)`** (lane
//!   FIX-ALLPARTS, decision 2026-11-19-27, coded from NATIVE_EQUIP_PARTS.md alone; the analyst was lane
//!   FIX-EQUIP-NATIVES): one iterator over the definition data the script passes. It visits the struct's eleven part
//!   slots by field name in their declared order ([`WEAPON_ALL_PARTS`], [`ITEM_ALL_PARTS`]), skips a slot that holds
//!   None, and visits the last two (the name parts) only when the flag is set (any nonzero value). Parts are not
//!   type-checked; a part in two slots is visited twice. A loop that runs to its end leaves the out argument None
//!   (the VM's engine-iterator rule, which is the note's). Two cases the note leaves open cannot be guarded here,
//!   because the VM's iterator hands over its whole list when the loop starts and is not called again: what the out
//!   argument holds after a `break` in the body (the VM keeps the last part), and whether a body that writes the
//!   definition data during the loop changes what is visited (here it does not: the list was taken first). No
//!   installed caller does either (the note); both are UNVERIFIED.
//!
//! `ApplyInternalSlotEffectModifiers` is [`super::internal_slots`]' (lane IMPL-APPLY-SLOT-EFFECT-MODIFIERS, decision
//! 2026-11-19-97). `InitializeAttributeSlots` and
//! `InitializeAttributeSlotsForNameParts` are [`super::slots`]' (lane IMPL-INIT-ATTRIBUTE-SLOTS, decision
//! 2026-11-19-82), which also computes the `ComputedModifierValue` the external slots read.

use std::rc::Rc;

use ow_attr::api::{EffectData, EvalCx, InitData};
use ow_attr::stack::ModifierType;
use ow_core::ParityMode;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::layout::PropKind;
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

/// The script functions of the part iterator (one native body for both, NATIVE_EQUIP_PARTS.md section 1).
pub const ALL_PARTS_WEAPON: &str = "WillowGame.WillowWeapon.AllParts";
pub const ALL_PARTS_ITEM: &str = "WillowGame.WillowItem.AllParts";

/// `WeaponDefinitionData`'s eleven part slots in declared order, the two name parts last (NATIVE_EQUIP_PARTS.md
/// section 2); the first nine are [`super::WEAPON_PART_FIELDS`].
pub const WEAPON_ALL_PARTS: [&str; 11] = [
    "BodyPartDefinition",
    "GripPartDefinition",
    "BarrelPartDefinition",
    "SightPartDefinition",
    "StockPartDefinition",
    "ElementalPartDefinition",
    "Accessory1PartDefinition",
    "Accessory2PartDefinition",
    "MaterialPartDefinition",
    "PrefixPartDefinition",
    "TitlePartDefinition",
];

/// `ItemDefinitionData`'s eleven part slots in declared order, the two name parts last; the first nine are
/// [`super::ITEM_PART_FIELDS`].
pub const ITEM_ALL_PARTS: [&str; 11] = [
    "AlphaItemPartDefinition",
    "BetaItemPartDefinition",
    "GammaItemPartDefinition",
    "DeltaItemPartDefinition",
    "EpsilonItemPartDefinition",
    "ZetaItemPartDefinition",
    "EtaItemPartDefinition",
    "ThetaItemPartDefinition",
    "MaterialItemPartDefinition",
    "PrefixItemNamePartDefinition",
    "TitleItemNamePartDefinition",
];

/// The parts `AllParts` visits for definition data `data` with `slots` (module documentation): the set slots in order,
/// the last two only with `names`. A slot the struct does not declare, or one holding a non-object, is a typed error.
pub fn all_parts(vm: &Vm, data: &Value, slots: &[&str; 11], names: bool) -> Result<Vec<ObjRef>, VmError> {
    let visited = if names { &slots[..] } else { &slots[..9] };
    let mut out = Vec::new();
    for f in visited {
        if let Some(p) = object_of(&field(vm, data, f)?, f)? {
            out.push(p);
        }
    }
    Ok(out)
}

/// The iterator body for `slots` (the out argument is parameter 1).
fn all_parts_iterator(slots: &'static [&'static str; 11]) -> ow_vm::api::IteratorFn {
    Rc::new(move |call: &mut NativeCall<'_>| {
        let data = call.arg(0)?.clone();
        let names = call.has(2) && call.arg(2)?.truth(&call.vm.space.names);
        let parts = all_parts(call.vm, &data, slots, names)?;
        Ok(ow_vm::api::IteratorResult::new(1, parts.into_iter().map(|p| Value::Object(Some(p))).collect()))
    })
}

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

/// The slot fields the native reads, each with the kind it must be declared as (`AttributeSlotData`).
const SLOT_FIELDS: [(&str, &str); 6] = [
    ("bActivated", "a bool"),
    ("bExternalSlot", "a bool"),
    ("bRunEffectsAsSkill", "a bool"),
    ("AttributeToModify", "an object"),
    ("ModifierType", "a byte"),
    ("ComputedModifierValue", "a float"),
];

/// Whether `kind` is the one `SLOT_FIELDS` names as `want`.
fn slot_field_kind(kind: &PropKind, want: &str) -> bool {
    match want {
        "a bool" => matches!(kind, PropKind::Bool),
        "an object" => matches!(kind, PropKind::Object { .. }),
        "a byte" => matches!(kind, PropKind::Byte { .. }),
        "a float" => matches!(kind, PropKind::Float),
        _ => false,
    }
}

/// `item`'s first `max` `AttributeSlots` (none for a count of 0 or below), as its class declares the property: a
/// static array of `AttributeSlotData` structs (the installed `WillowInventory` declares `AttributeSlots[19]`), read
/// element by element in index order, at most the declaration's `array_dim` (lane FIX-ATTRIBUTE-SLOTS). The
/// declaration is checked first, whatever the count: a class that declares no `AttributeSlots`, declares it as
/// anything but structs, with a negative dimension, or of a struct without one of the six slot fields as its kind
/// (review r1 of #438), is a data error. No element past the count is read.
pub fn attribute_slots(vm: &Vm, item: ObjRef, max: i32) -> Result<Vec<Value>, VmError> {
    declared_slots(vm, item, max, &SLOT_FIELDS, EXTERNAL_SLOTS)
}

/// [`attribute_slots`] with the slot fields `fields` checked instead (each with the kind it must be declared as),
/// errors naming `native` (lane IMPL-APPLY-SLOT-EFFECT-MODIFIERS: the internal slots' native does not read
/// `bRunEffectsAsSkill`).
pub(crate) fn declared_slots(
    vm: &Vm,
    item: ObjRef,
    max: i32,
    fields: &[(&str, &str)],
    native: &str,
) -> Result<Vec<Value>, VmError> {
    let class = vm.space.heap.get(item)?.class;
    let declared = |what: &str| {
        bad(
            native,
            format!("{} {what}", ow_attr::eval::class_path(&vm.space, item)),
        )
    };
    let p = vm
        .space
        .prop(class, "AttributeSlots")
        .ok_or_else(|| declared("declares no AttributeSlots"))?;
    let decl = vm
        .space
        .classes
        .decl(p)
        .ok_or_else(|| declared("has no declaration for AttributeSlots"))?;
    let PropKind::Struct(ty) = decl.kind else {
        return Err(declared("declares AttributeSlots as something other than slot structs"));
    };
    let dim = u32::try_from(decl.array_dim).map_err(|_| declared("declares AttributeSlots with a negative dimension"))?;
    let layout = vm
        .space
        .classes
        .struct_layout(ty)
        .ok_or_else(|| declared("declares AttributeSlots of a struct with no layout"))?;
    for &(name, want) in fields {
        let ok = vm
            .space
            .names
            .find(name)
            .and_then(|id| layout.field(id))
            .is_some_and(|(_, d)| d.array_dim == 1 && slot_field_kind(&d.kind, want));
        if !ok {
            return Err(declared(&format!(
                "declares AttributeSlots of a struct without {name} as {want}"
            )));
        }
    }
    let count = u32::try_from(max).unwrap_or(0).min(dim);
    (0..count)
        .map(|i| Ok(vm.space.read(item, PropAddr::element(p, i))?.clone()))
        .collect()
}

/// The effects `ApplyExternalSlotEffectModifiers` applies for `item`'s first `max` slots (module documentation).
pub fn external_slot_effects(vm: &Vm, item: ObjRef, max: i32) -> Result<Vec<EffectData>, VmError> {
    let mut out = Vec::new();
    for slot in &attribute_slots(vm, item, max)? {
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

/// The script function whose native key is [`RESOLVE_CONTEXT`], for the keep-existing binds (`ow_combat::hit::bind_fn`).
pub const RESOLVE_CONTEXT_FUNCTION: &str = "Engine.AttributeDefinitionBase.ResolveContext";

/// The class every `ResolveContext` receiver must be (the receiver check, by the class table's `by_path` only).
pub const ATTRIBUTE_DEFINITION_BASE: &str = "Engine.AttributeDefinitionBase";

/// `AttributeDefinitionBase.ResolveContext(ContextSource)` in Off (lane IMPL-RESOLVE-CONTEXT, NATIVE_RESOLVE_CONTEXT.md
/// section 5, **UNVERIFIED**): the one wrapper the equip pack and the action-skill pack bind. In order, the first failure
/// is the answer, and nothing before the walk loads, writes or draws:
/// 1. the receiver: set, and its class [`ATTRIBUTE_DEFINITION_BASE`] or a child: else `VmError::Script`;
/// 2. the argument `ContextSource`: an object reference or None: else `VmError::Script`;
/// 3. `ow-attr`'s [`ow_attr::AttributeSystem::resolve_context_native`]: R1, the chain's shape (`VmError::Script`), R2
///    for every entry, R3 for every entry, then the walk, whose `NotYet`s are terminal.
pub fn resolve_context_native(call: &mut NativeCall<'_>) -> Result<Value, VmError> {
    use ow_attr::resolve_native::{ResolveError, UNIT};
    let script = |message: String| VmError::Script { function: UNIT.to_string(), message };
    let attr = definition_receiver(call, UNIT)?;
    let source = match call.arg(0)? {
        Value::Object(o) => *o,
        other => return Err(script(format!("{UNIT}: ContextSource holds {}, not an object reference", other.kind_name()))),
    };
    let attrs = call.vm.attrs.clone();
    match attrs.resolve_context_native(attr, source, &mut EvalCx::new(call.vm)) {
        Ok(context) => Ok(Value::Object(context)),
        Err(ResolveError::Malformed(message)) => Err(script(message)),
        Err(ResolveError::Attr(e)) => Err(e.into()),
    }
}

/// Binds [`resolve_context_native`] at [`RESOLVE_CONTEXT`] in `ParityMode::Off` unless a pack bound the key first (the
/// keep-existing rule of `ow_combat::hit::bind_fn`, decision 2026-11-19-127, which this crate cannot call).
pub fn bind_resolve_context(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off || vm.has_native(RESOLVE_CONTEXT) {
        return Ok(());
    }
    vm.register_native(RESOLVE_CONTEXT, Rc::new(resolve_context_native))
}

/// The script function whose native key is [`ADD_MODIFIER`], for the keep-existing binds (`ow_combat::hit::bind_fn`).
pub const ADD_MODIFIER_FUNCTION: &str = "Engine.AttributeDefinitionBase.AddAttributeModifier";

/// The receiver check both wrappers share: `this` set and its class [`ATTRIBUTE_DEFINITION_BASE`] or a child, by the
/// class table's `by_path` only (no load). Otherwise a `VmError::Script` naming `unit`.
fn definition_receiver(call: &NativeCall<'_>, unit: &str) -> Result<ObjRef, VmError> {
    let script = |message: String| VmError::Script { function: unit.to_string(), message };
    let Some(attr) = call.this else {
        return Err(script(format!("{unit} called without a definition")));
    };
    let space = &call.vm.space;
    let class = space.heap.get(attr)?.class;
    if !space.classes.by_path(ATTRIBUTE_DEFINITION_BASE).is_some_and(|b| space.classes.is_child_of(class, b)) {
        let name = space.names.text(space.heap.get(attr)?.name).into_owned();
        return Err(script(format!(
            "{unit}: the receiver {name} is a {}, not an {ATTRIBUTE_DEFINITION_BASE}",
            space.classes.layout(class).path()
        )));
    }
    Ok(attr)
}

/// `AttributeDefinitionBase.AddAttributeModifier(Context, Modifier, bSuppressNotify)` in Off (lane
/// IMPL-ADD-ATTRIBUTE-MODIFIER, NATIVE_ADD_ATTRIBUTE_MODIFIER.md section 3, **UNVERIFIED**): the one wrapper the equip
/// pack and the action-skill pack bind. In order, the first failure is the answer, and nothing before the write loads,
/// writes or draws:
/// 1. the receiver ([`definition_receiver`]): else `VmError::Script`;
/// 2. `Context` and `Modifier` each an object reference or None, else `VmError::Script`; then either None answers
///    false at once, with the definition not read. A `bSuppressNotify` that is not a bool is false, not an error;
/// 3. `ow-attr`'s [`ow_attr::AttributeSystem::add_modifier_native`]: A1, A0, the chain's shape (`VmError::Script`), A2,
///    A3, the load-free storage probes, then the write, whose `NotYet`s are terminal.
pub fn add_modifier_native(call: &mut NativeCall<'_>) -> Result<Value, VmError> {
    use ow_attr::add_native::{AddError, UNIT};
    let script = |message: String| VmError::Script { function: UNIT.to_string(), message };
    let attr = definition_receiver(call, UNIT)?;
    let mut objects = [None, None];
    for (i, name) in ["Context", "Modifier"].iter().enumerate() {
        objects[i] = match call.arg(i)? {
            Value::Object(o) => *o,
            other => return Err(script(format!("{UNIT}: {name} holds {}, not an object reference", other.kind_name()))),
        };
    }
    let [Some(context), Some(modifier)] = objects else {
        return Ok(Value::Bool(false));
    };
    let s = suppress(call)?;
    let attrs = call.vm.attrs.clone();
    match attrs.add_modifier_native(attr, context, modifier, s, &mut EvalCx::new(call.vm)) {
        Ok(added) => Ok(Value::Bool(added)),
        Err(AddError::Malformed(message)) => Err(script(message)),
        Err(AddError::Attr(e)) => Err(e.into()),
    }
}

/// Binds [`add_modifier_native`] at [`ADD_MODIFIER`] in `ParityMode::Off` unless a pack bound the key first (the
/// keep-existing rule of `ow_combat::hit::bind_fn`, decision 2026-11-19-127, which this crate cannot call).
pub fn bind_add_modifier(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off || vm.has_native(ADD_MODIFIER) {
        return Ok(());
    }
    vm.register_native(ADD_MODIFIER, Rc::new(add_modifier_native))
}

/// Binds the module's natives in `ParityMode::Off`; Reference binds nothing (module documentation).
pub fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    // ResolveContext: the shared rule with its preflight (lane IMPL-RESOLVE-CONTEXT), keeping a binding made first.
    bind_resolve_context(vm)?;
    // AddAttributeModifier: the shared rule with its preflight (lane IMPL-ADD-ATTRIBUTE-MODIFIER), keep-existing too.
    bind_add_modifier(vm)?;
    let natives: [(&str, NativeFn); 3] = [
        (REMOVE_MODIFIER, Rc::new(remove_modifier)),
        (SLOT_MAX, Rc::new(slot_max)),
        (EXTERNAL_SLOTS, Rc::new(external_slots)),
    ];
    for (key, f) in natives {
        vm.register_native(key, f)?;
    }
    // AllParts: an iterator, keyed by each declaration's own native key (a package without the function binds none).
    for (path, slots) in [(ALL_PARTS_WEAPON, &WEAPON_ALL_PARTS), (ALL_PARTS_ITEM, &ITEM_ALL_PARTS)] {
        let Ok(id) = vm.find_function(path) else {
            continue;
        };
        if let Some(key) = vm.function(id).and_then(|d| d.native_key.clone()) {
            vm.register_iterator(&key, all_parts_iterator(slots))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod slot_package_tests;

#[cfg(test)]
mod script_tests;

#[cfg(test)]
#[path = "resolve_context_tests.rs"]
mod resolve_context_tests;

#[cfg(test)]
#[path = "add_modifier_tests.rs"]
mod add_modifier_tests;
