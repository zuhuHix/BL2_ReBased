//! Owner: lane FIX-POOL-SPAWN-B (decision 2026-11-19-29). The three `WillowInventory` natives the installed
//! `InitializeInventory` script calls on every spawned inventory, in `ParityMode::Off` only. AI-assisted (Claude Code).
//!
//! **Evidence.** NATIVE_LOOT_SPAWN.md, "The natives inside it" (analyst lane FIX-POOL-SPAWN-A, read from the
//! executable; this lane implements from the note alone). **Every rule is UNVERIFIED in the running game.**
//!
//! - `GenerateUniqueID()`: two C-runtime `rand()` draws, `first | (second << 16)`, and 0 becomes 1. The draws come
//!   from the world's [`CrtStream::LootPool`], the stream the Off loot path already shares: the pool pick of
//!   `ow_inventory::spawn` and `ow-combat`'s `OFF_STREAM`. Only the pool pick and these ID draws (and, once wired,
//!   the toss) share that stream: the parts roll draws from `CrtStream::ItemRoll` (D5), so the note's per-pool
//!   interleaving ("Draw order": the firing `FRand`; the pool's roll with each new inventory's two draws and its
//!   parts roll; then the toss draws; then the next pool) is not reproduced on one generator (UNVERIFIED).
//! - `ComputeMonetaryValue()`: 0 without an inventory definition; otherwise the definition's `MonetaryValue`
//!   (attribute initialization data) evaluated with the inventory itself as context (NATIVE_INVENTORY_VALUE.md
//!   section 3; decision 2026-11-19-45) and truncated toward zero, then capped by the definition's
//!   `FormOfCurrency`: credits 99,999,999, Eridium 500, the other in-range values 999, and a value past the enum's last
//!   entry 0. A negative value is not raised.
//! - `GetManufacturer()`: the inventory's `DefinitionData.ManufacturerDefinition`, as it is.
//!
//! **Choices where the note is silent, each UNVERIFIED.** "The inventory definition" is read as the one
//! `WillowInventoryDefinition` the inventory's `DefinitionData` holds: `WeaponTypeDefinition` (a weapon's data) or
//! `ItemDefinition` (an item's), the fields the installed `InitializeInventory` writes.
//!
//! **`NotYet { NotRead }`, each naming its question.** Whatever part of the `MonetaryValue` evaluation the attribute
//! evaluator cannot do (its own `NotYet`); a truncation of a NaN or a value outside the `int` range; a `FormOfCurrency` enum that is not the
//! note's 13 entries (credits, Eridium, Seraph crystals and ten reserved), or a byte on a trailing `_MAX` entry; an
//! inventory whose class declares no `DefinitionData`, or whose data holds neither definition field.
//!
//! **Reference: unbound.** `src/` has no stand-in for any of the three, so the VM's stub policy answers.

use std::rc::Rc;

use ow_attr::api::EvalCx;
use ow_core::rand::{CrtStream, Rand15};
use ow_core::{NotYet, ParityMode};
use ow_object::ids::ObjRef;
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, NativeFn, Vm};
use ow_vm::error::VmError;

use crate::equip::{bad, field, object_of};

/// The record this module follows.
pub const RECORD: &str =
    "docs/verification/NATIVE_LOOT_SPAWN.md (InitializeInventory, the natives inside it)";

/// The function paths this module binds (Off only).
pub const GENERATE_UNIQUE_ID: &str = "WillowGame.WillowInventory.GenerateUniqueID";
pub const COMPUTE_MONETARY_VALUE: &str = "WillowGame.WillowInventory.ComputeMonetaryValue";
pub const GET_MANUFACTURER: &str = "WillowGame.WillowInventory.GetManufacturer";

/// The `FormOfCurrency` caps (the note's constants).
pub const CREDITS_CAP: i32 = 99_999_999;
pub const ERIDIUM_CAP: i32 = 500;
pub const OTHER_CAP: i32 = 999;
/// The currency enum's entries the note describes: three named and ten reserved.
pub const CURRENCY_ENTRIES: usize = 13;

/// The stream the two `GenerateUniqueID` draws come from (module documentation).
pub const ID_STREAM: CrtStream = CrtStream::LootPool;

/// Binds the three natives in Off, by function path: a function the VM does not declare is skipped, and a key some
/// other pack bound first is left alone.
pub fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    let natives: [(&str, NativeFn); 3] = [
        (
            GENERATE_UNIQUE_ID,
            Rc::new(|c: &mut NativeCall<'_>| Ok(Value::Int(unique_id(c.vm.rng.crt(ID_STREAM))))),
        ),
        (COMPUTE_MONETARY_VALUE, Rc::new(compute_monetary_value)),
        (GET_MANUFACTURER, Rc::new(get_manufacturer)),
    ];
    for (path, f) in natives {
        let Ok(id) = vm.find_function(path) else {
            continue;
        };
        let Some(key) = vm.function(id).and_then(|d| d.native_key.clone()) else {
            continue;
        };
        if !vm.has_native(&key) {
            vm.register_native(&key, f)?;
        }
    }
    Ok(())
}

/// `GenerateUniqueID`'s rule on `r`: the first draw is the low half, the second the high half; 0 becomes 1.
pub fn unique_id(r: &mut dyn Rand15) -> i32 {
    let first = i32::from(r.rand15());
    let second = i32::from(r.rand15());
    match first | (second << 16) {
        0 => 1,
        id => id,
    }
}

/// The cap for the `FormOfCurrency` value named `name` at position `index` of an enum whose entries are `names`
/// (module documentation). `None` gives 0 (past the last entry); an `Err` is the `NotYet` question.
pub fn currency_cap(names: &[String], index: usize) -> Result<Option<i32>, String> {
    let real = match names.last() {
        Some(last) if last.ends_with("_MAX") => &names[..names.len() - 1],
        _ => names,
    };
    let named = [
        "CURRENCY_Credits",
        "CURRENCY_Eridium",
        "CURRENCY_SeraphCrystals",
    ];
    if real.len() != CURRENCY_ENTRIES || real[..3].iter().zip(named).any(|(a, b)| a != b) {
        return Err(format!(
            "a FormOfCurrency enum of {} entries starting {:?} (the note describes 13: credits, Eridium, Seraph \
             crystals and ten reserved)",
            real.len(),
            real.iter().take(3).collect::<Vec<_>>()
        ));
    }
    if index >= names.len() {
        return Ok(None);
    }
    if index >= real.len() {
        return Err(
            "a FormOfCurrency byte on the enum's _MAX entry (above the last entry or not)"
                .to_string(),
        );
    }
    Ok(Some(match index {
        0 => CREDITS_CAP,
        1 => ERIDIUM_CAP,
        _ => OTHER_CAP,
    }))
}

/// `ComputeMonetaryValue`'s last two steps: `value` truncated toward zero, then at most `cap` (`None`: 0).
pub fn monetary_value(value: f32, cap: Option<i32>) -> Result<i32, String> {
    if !(-2_147_483_648.0..2_147_483_648.0).contains(&value) {
        return Err(format!(
            "truncating a MonetaryValue of {value} (outside the int range, or NaN)"
        ));
    }
    // In range: `as` truncates toward zero exactly.
    let v = value as i32;
    Ok(cap.map_or(0, |c| v.min(c)))
}

fn not_read(what: impl Into<String>) -> VmError {
    NotYet::not_read(what.into(), RECORD).into()
}

/// The inventory's `DefinitionData` struct value, or the `NotYet` that its class declares none.
fn definition_data(vm: &Vm, me: ObjRef, native: &str) -> Result<Value, VmError> {
    crate::equip::read(vm, me, "DefinitionData")?.ok_or_else(|| {
        not_read(format!(
            "{native} on an inventory class that declares no DefinitionData"
        ))
    })
}

/// The one inventory definition `DefinitionData` holds (module documentation).
fn inventory_definition(vm: &Vm, data: &Value) -> Result<Option<ObjRef>, VmError> {
    let ty = data.as_struct().map(|s| s.ty).ok_or_else(|| {
        bad(
            "ComputeMonetaryValue",
            format!("DefinitionData holds {}", data.kind_name()),
        )
    })?;
    let has = |name: &str| {
        vm.space.names.find(name).is_some_and(|id| {
            vm.space
                .classes
                .struct_layout(ty)
                .is_some_and(|l| l.field(id).is_some())
        })
    };
    for name in ["WeaponTypeDefinition", "ItemDefinition"] {
        if has(name) {
            return object_of(&field(vm, data, name)?, name);
        }
    }
    Err(not_read(
        "ComputeMonetaryValue on a DefinitionData with neither WeaponTypeDefinition nor ItemDefinition",
    ))
}

fn this(c: &NativeCall<'_>, native: &str) -> Result<ObjRef, VmError> {
    c.this
        .ok_or_else(|| bad(native, "called without an inventory"))
}

fn compute_monetary_value(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let me = this(c, "ComputeMonetaryValue")?;
    monetary_value_of(c.vm, me)
}

/// `ComputeMonetaryValue()`'s body on the inventory `me` (module documentation). `ComputeValueOfParts` ends with this
/// same body (NATIVE_INVENTORY_VALUE.md section 3; `crate::equip::value`).
pub fn monetary_value_of(vm: &mut Vm, me: ObjRef) -> Result<Value, VmError> {
    const NATIVE: &str = "ComputeMonetaryValue";
    let data = definition_data(vm, me, NATIVE)?;
    let Some(definition) = inventory_definition(vm, &data)? else {
        return Ok(Value::Int(0));
    };
    // The definition is a package object: load it before reading its properties.
    if vm.defs.is_unloaded(&vm.space, definition) {
        let r = vm.defs.ensure_loaded(&mut vm.space, definition);
        let log = vm.defs.take_log();
        vm.log.extend(log);
        r.map_err(VmError::Def)?;
    }
    let raw = crate::equip::required(vm, definition, "MonetaryValue", NATIVE)?;
    let init = ow_attr::eval::init_data(&vm.space, &raw)?;
    // The context is the inventory itself (NATIVE_INVENTORY_VALUE.md section 3, which answers NATIVE_LOOT_SPAWN.md's
    // open question 2; lane FIX-VALUE-B, decision 2026-11-19-45). Whatever the evaluator cannot do answers its own
    // `NotYet`.
    let sys = vm.attrs.clone();
    let value = {
        let mut cx = EvalCx::new(&mut *vm);
        sys.evaluate_init(&init, Some(me), None, &mut cx)?
    };
    let cap = currency_cap_of(vm, definition)?;
    monetary_value(value, cap).map(Value::Int).map_err(not_read)
}

/// The cap of `definition`'s `FormOfCurrency`, by the enum's value names.
fn currency_cap_of(vm: &Vm, definition: ObjRef) -> Result<Option<i32>, VmError> {
    const NATIVE: &str = "ComputeMonetaryValue";
    let class = vm.space.heap.get(definition)?.class;
    let prop = vm.space.prop(class, "FormOfCurrency").ok_or_else(|| {
        bad(
            NATIVE,
            "the inventory definition declares no FormOfCurrency",
        )
    })?;
    let kind = vm.space.classes.decl(prop).map(|d| d.kind.clone());
    let e = match kind {
        Some(PropKind::Byte { enum_: Some(e) })
        | Some(PropKind::ByteAttribute { enum_: Some(e) }) => e,
        _ => return Err(bad(NATIVE, "FormOfCurrency names no enum")),
    };
    let names: Vec<String> = vm
        .space
        .classes
        .enum_layout(e)
        .ok_or_else(|| bad(NATIVE, "FormOfCurrency's enum is not laid out"))?
        .values()
        .iter()
        .map(|n| vm.space.names.text(*n).into_owned())
        .collect();
    let b = match crate::equip::required(vm, definition, "FormOfCurrency", NATIVE)? {
        Value::Byte(b) => usize::from(b),
        other => {
            return Err(bad(
                NATIVE,
                format!("FormOfCurrency holds {}", other.kind_name()),
            ));
        }
    };
    currency_cap(&names, b).map_err(|q| not_read(format!("ComputeMonetaryValue with {q}")))
}

fn get_manufacturer(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const NATIVE: &str = "GetManufacturer";
    let me = this(c, NATIVE)?;
    let data = definition_data(c.vm, me, NATIVE)?;
    let m = field(c.vm, &data, "ManufacturerDefinition")?;
    Ok(Value::Object(object_of(&m, "ManufacturerDefinition")?))
}

#[cfg(test)]
mod tests;
