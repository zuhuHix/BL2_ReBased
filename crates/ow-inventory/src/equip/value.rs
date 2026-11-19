//! Owner: lane FIX-COMPUTE-VALUE. AI-assisted (Claude Code). An inventory's value: `ComputeValueOfParts` (weapon and
//! item), implemented from NATIVE_INVENTORY_VALUE.md alone (decision
//! 2026-11-19-36). Every rule is the record's reading of the executable, **UNVERIFIED** in the running game; the value
//! formula the money step feeds matched 69 of 69 cards (the record's section 4), its order of effects did not.
//!
//! **ComputeValueOfParts()** writes three fields, in this order (section 1):
//! 1. nothing without the definition data's first definition (`WeaponTypeDefinition`, `ItemDefinition`);
//! 2. `RarityLevel` (section 2): for a weapon, and for an item whose definition is not a `MissionItemDefinition`, the
//!    definition's `BaseRarity` plus each part's `Rarity`, each init value evaluated with **its own definition or part**
//!    as context and truncated toward zero before it is added. An unset value adds 0 (the evaluator's constant times
//!    scale of an all-zero struct), and an empty slot adds nothing. For a mission item, the globals definition's
//!    `MissionItemRarityLevel` (the installed `GetGlobalsDefinition` on the world's globals object), 0 without one;
//! 3. `MonetaryValueModifierTotal`, the `WillowInventory` property of that name in the packages: the definition's
//!    `BaseMonetaryValueModifier`, then multiplied one part at a time, in walk order, by the part's `MonetaryValueMod`
//!    attribute (evaluated by its value chain with the inventory as context), each product stored back in single
//!    precision, nothing rounded to an integer. A part with no `MonetaryValueMod`, and an empty slot, leave the total
//!    as it is. The value calculators read the total as the attribute
//!    `D_Attributes.Inventory.InventoryPartMonetaryValueModifierTotal`; how that attribute reaches the property is not
//!    read (UNVERIFIED), and this module writes only the property;
//! 4. `MonetaryValue` = `ComputeMonetaryValue()`'s body, `crate::definition::monetary_value_of` (lane FIX-POOL-SPAWN-B,
//!    which binds that native). It still answers `NotYet` for a `MonetaryValue` that reads an attribute or an
//!    initialization definition, the context NATIVE_LOOT_SPAWN.md left open; NATIVE_INVENTORY_VALUE.md section 3 now
//!    answers it (the inventory itself), so that refusal can be lifted there.
//!
//! The part walk is the `AllParts` order (NATIVE_EQUIP_PARTS.md section 2) **with the name parts**: the nine part
//! slots, then the two name-part slots; empty slots are skipped.
//!
//! The mission rule of step 2 does not apply to weapons: the weapon rule has no mission-item case.
//!
//! **Only in `ParityMode::Off`.** The C++ reference has no stand-in, so under Reference both stay unbound and the stub
//! policy answers them (architecture R3).
//!
//! Open (the record's questions, not reached by the spawn path these natives are bound for): whether the natives run
//! on items outside the spawn (shop, save load), and the next unread natives of the weapon's `InitializeInternal`.

use std::rc::Rc;

use ow_attr::api::EvalCx;
use ow_core::ParityMode;
use ow_object::ids::ObjRef;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, NativeFn, Vm};
use ow_vm::error::VmError;

use super::{bad, field, object_of, read, required, write};

/// The record these natives follow.
pub const RECORD: &str = "NATIVE_INVENTORY_VALUE.md";

/// The native keys [`register`] binds in Off (`ComputeMonetaryValue` is `crate::definition`'s, lane FIX-POOL-SPAWN-B).
pub const WEAPON_VALUE: &str = "WillowWeapon.ComputeValueOfParts()";
pub const ITEM_VALUE: &str = "WillowItem.ComputeValueOfParts()";

/// The definition data's first definition and part slots, in the `AllParts` order, name parts last.
struct Layout {
    definition: &'static str,
    slots: [&'static str; 11],
}

const WEAPON: Layout = Layout {
    definition: "WeaponTypeDefinition",
    slots: [
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
    ],
};

const ITEM: Layout = Layout {
    definition: "ItemDefinition",
    slots: [
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
    ],
};

/// The inventory's first definition and its set parts in walk order (name parts included).
fn definition_and_parts(vm: &mut Vm, inv: ObjRef, layout: &Layout, what: &str) -> Result<(Option<ObjRef>, Vec<ObjRef>), VmError> {
    let data = required(vm, inv, "DefinitionData", what)?;
    let definition = object_of(&field(vm, &data, layout.definition)?, what)?;
    let mut parts = Vec::new();
    for slot in layout.slots {
        if let Some(p) = object_of(&field(vm, &data, slot)?, what)? {
            parts.push(p);
        }
    }
    for o in definition.iter().chain(parts.iter()) {
        loaded(vm, *o)?;
    }
    Ok((definition, parts))
}

fn loaded(vm: &mut Vm, o: ObjRef) -> Result<(), VmError> {
    if vm.defs.is_unloaded(&vm.space, o) {
        let r = vm.defs.ensure_loaded(&mut vm.space, o);
        let log = vm.defs.take_log();
        vm.log.extend(log);
        r.map_err(VmError::Def)?;
    }
    Ok(())
}

/// An init-data property of `holder` evaluated with `context` as its context; `None` when the class declares none.
fn init_value(vm: &mut Vm, holder: ObjRef, name: &str, context: ObjRef) -> Result<Option<f32>, VmError> {
    let Some(v) = read(vm, holder, name)? else {
        return Ok(None);
    };
    let data = ow_attr::eval::init_data(&vm.space, &v)?;
    let attrs = vm.attrs.clone();
    Ok(Some(attrs.evaluate_init(&data, Some(context), None, &mut EvalCx::new(vm))?))
}

/// Truncation toward zero of a finite value that fits an i64, else a data error.
fn trunc(x: f32, what: &str) -> Result<i64, VmError> {
    let t = f64::from(x).trunc();
    if !t.is_finite() || t.abs() >= 9.2e18 {
        return Err(bad(what, format!("{x} does not truncate to an integer")));
    }
    Ok(t as i64)
}

/// An i64 as the script's int (a data error past its range).
fn int(v: i64, what: &str) -> Result<Value, VmError> {
    i32::try_from(v).map(Value::Int).map_err(|_| bad(what, format!("{v} does not fit an int")))
}

/// `RarityLevel` (module documentation, step 2).
fn rarity(vm: &mut Vm, definition: ObjRef, parts: &[ObjRef], what: &str) -> Result<i64, VmError> {
    if let Ok(mission) = vm.find_class("WillowGame.MissionItemDefinition") {
        let own = vm.space.heap.get(definition)?.class;
        if vm.space.classes.is_child_of(own, mission) {
            let globals = {
                let cx = EvalCx::new(vm);
                cx.world.globals()?
            };
            let f = vm.find_function("WillowGame.WillowGlobals.GetGlobalsDefinition")?;
            let globals_definition = object_of(&vm.call(f, Some(globals), Vec::new())?.ret, what)?;
            let Some(gd) = globals_definition else { return Ok(0) };
            return match read(vm, gd, "MissionItemRarityLevel")? {
                Some(Value::Int(i)) => Ok(i64::from(i)),
                other => Err(bad(what, format!("MissionItemRarityLevel is {other:?}, not an int"))),
            };
        }
    }
    let mut level = match init_value(vm, definition, "BaseRarity", definition)? {
        Some(x) => trunc(x, what)?,
        None => 0,
    };
    for &part in parts {
        if let Some(x) = init_value(vm, part, "Rarity", part)? {
            level += trunc(x, what)?;
        }
    }
    Ok(level)
}

/// `ComputeValueOfParts()` on `inv` with `layout` (module documentation).
fn compute_value_of_parts(vm: &mut Vm, inv: ObjRef, layout: &Layout, what: &str) -> Result<(), VmError> {
    let (definition, parts) = definition_and_parts(vm, inv, layout, what)?;
    let Some(definition) = definition else { return Ok(()) };
    let level = rarity(vm, definition, &parts, what)?;
    write(vm, inv, "RarityLevel", int(level, what)?, what)?;
    let mut total = match read(vm, definition, "BaseMonetaryValueModifier")? {
        Some(Value::Float(f)) => f,
        other => return Err(bad(what, format!("BaseMonetaryValueModifier is {other:?}, not a float"))),
    };
    for &part in &parts {
        let Some(attr) = read(vm, part, "MonetaryValueMod")? else { continue };
        let Some(attr) = object_of(&attr, what)? else { continue };
        let attrs = vm.attrs.clone();
        let factor = attrs.value_from_context(attr, Some(inv), &mut EvalCx::new(vm))?;
        total *= factor;
    }
    write(vm, inv, "MonetaryValueModifierTotal", Value::Float(total), what)?;
    let money = crate::definition::monetary_value_of(vm, inv)?;
    write(vm, inv, "MonetaryValue", money, what)
}

fn this(c: &NativeCall<'_>, what: &str) -> Result<ObjRef, VmError> {
    c.this.ok_or_else(|| bad(what, "called without an inventory"))
}

/// Binds the module's natives in `ParityMode::Off`; Reference binds nothing (module documentation).
pub fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    let natives: [(&str, NativeFn); 2] = [
        (
            WEAPON_VALUE,
            Rc::new(|c: &mut NativeCall<'_>| {
                let inv = this(c, WEAPON_VALUE)?;
                compute_value_of_parts(c.vm, inv, &WEAPON, WEAPON_VALUE)?;
                Ok(Value::None)
            }),
        ),
        (
            ITEM_VALUE,
            Rc::new(|c: &mut NativeCall<'_>| {
                let inv = this(c, ITEM_VALUE)?;
                compute_value_of_parts(c.vm, inv, &ITEM, ITEM_VALUE)?;
                Ok(Value::None)
            }),
        ),
    ];
    for (key, f) in natives {
        vm.register_native(key, f)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "value_tests.rs"]
mod tests;
