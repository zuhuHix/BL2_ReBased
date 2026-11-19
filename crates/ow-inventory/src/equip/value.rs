//! Owner: lanes FIX-COMPUTE-VALUE and FIX-VALUE-B. AI-assisted (Claude Code). An inventory value: `ComputeValueOfParts`
//! (weapon and item) and a weapon `RecomputeNormalizedDamageValues`, implemented from NATIVE_INVENTORY_VALUE.md alone
//! (decisions 2026-11-19-36 and 2026-11-19-45). Every rule is the record's reading of the executable, **UNVERIFIED**
//! in the running game; the value formula the money step feeds matched 69 of 69 cards (the record's section 4), its
//! order of effects did not.
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
//!    which binds that native), which evaluates with the inventory itself as context (section 3; lane FIX-VALUE-B,
//!    decision 2026-11-19-45).
//!
//! The part walk is the `AllParts` order (NATIVE_EQUIP_PARTS.md section 2) **with the name parts**: the nine part
//! slots, then the two name-part slots; empty slots are skipped.
//!
//! The mission rule of step 2 does not apply to weapons: the weapon rule has no mission-item case, so a weapon skips
//! the class check entirely (review of #425).
//!
//! **RecomputeNormalizedDamageValues()** (weapon only; section 5; lane FIX-VALUE-B, decision 2026-11-19-45) rebuilds
//! two normalized damage values, instant-hit then melee. For each:
//! - the normalized stack (`NormalizedInstantHitDamageModifierStack`, `NormalizedMeleeDamageModifierStack`) becomes a
//!   copy of the weapon's stack (`InstantHitDamageModifierStack`, `MeleeDamageModifierStack`);
//! - the base (`NormalizedInstantHitDamageBaseValue`, `NormalizedMeleeDamageBaseValue`) is the weapon type
//!   definition's init value (`NormalizedInstantHitDamage`, `NormalizedMeleeDamage`) evaluated with **the weapon** as
//!   context;
//! - the value (`NormalizedInstantHitDamage`, `NormalizedMeleeDamage`) is that base with the copied stack applied by
//!   the stack formula, as a float, nothing rounded. The combine is the single-precision form this VM's attribute let
//!   uses (`super::let_forms`); which form the game uses is architecture 5.3's open question (UNVERIFIED). Modifiers
//!   are read as `ow-attr` reads a stack: a None element and a modifier type outside the three are skipped.
//!
//! Both values are computed before anything is written. The writes are plain property writes: whether the native
//! sends an attribute-change notification is not in the note (UNVERIFIED; none is sent). A weapon without a
//! `WeaponTypeDefinition` is `NotYet { NotRead }` (the note does not say what the native does then).
//!
//! **Only in `ParityMode::Off`.** The C++ reference has no stand-in, so under Reference both stay unbound and the stub
//! policy answers them (architecture R3).
//!
//! Open (the record's questions, not reached by the spawn path these natives are bound for): whether the natives run
//! on items outside the spawn (shop, save load), and the next unread natives of the weapon's `InitializeInternal`.

use std::rc::Rc;

use ow_attr::api::{EvalCx, modifier_id_of};
use ow_attr::stack::{Modifier, ModifierType, NumKind, combine};
use ow_core::ParityMode;
use ow_core::notyet::NotYet;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_vm::api::{NativeCall, NativeFn, Vm};
use ow_vm::error::VmError;

use super::{bad, field, object_of, read, required, write};

/// The record these natives follow.
pub const RECORD: &str = "NATIVE_INVENTORY_VALUE.md";

/// The native keys [`register`] binds in Off (`ComputeMonetaryValue` is `crate::definition`'s, lane FIX-POOL-SPAWN-B).
pub const WEAPON_VALUE: &str = "WillowWeapon.ComputeValueOfParts()";
pub const ITEM_VALUE: &str = "WillowItem.ComputeValueOfParts()";
pub const NORMALIZED_DAMAGE: &str = "WillowWeapon.RecomputeNormalizedDamageValues()";

/// One normalized damage value: the weapon's stack, the type definition's init value, and the three outputs.
struct Normalized {
    stack: &'static str,
    definition: &'static str,
    out_stack: &'static str,
    out_base: &'static str,
    out_value: &'static str,
}

/// Instant-hit, then melee (section 5).
const NORMALIZED: [Normalized; 2] = [
    Normalized {
        stack: "InstantHitDamageModifierStack",
        definition: "NormalizedInstantHitDamage",
        out_stack: "NormalizedInstantHitDamageModifierStack",
        out_base: "NormalizedInstantHitDamageBaseValue",
        out_value: "NormalizedInstantHitDamage",
    },
    Normalized {
        stack: "MeleeDamageModifierStack",
        definition: "NormalizedMeleeDamage",
        out_stack: "NormalizedMeleeDamageModifierStack",
        out_base: "NormalizedMeleeDamageBaseValue",
        out_value: "NormalizedMeleeDamage",
    },
];

/// The definition data's first definition and part slots, in the `AllParts` order, name parts last.
struct Layout {
    definition: &'static str,
    slots: [&'static str; 11],
    /// Whether the mission-item rarity rule applies: items only (the weapon rule has no mission-item case).
    mission_rule: bool,
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
    mission_rule: false,
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
    mission_rule: true,
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

pub(super) fn loaded(vm: &mut Vm, o: ObjRef) -> Result<(), VmError> {
    if vm.defs.is_unloaded(&vm.space, o) {
        let r = vm.defs.ensure_loaded(&mut vm.space, o);
        let log = vm.defs.take_log();
        vm.log.extend(log);
        r.map_err(VmError::from)?;
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

/// `RarityLevel` (module documentation, step 2). `mission_rule` is the layout's: a weapon never takes the mission rule.
fn rarity(vm: &mut Vm, definition: ObjRef, parts: &[ObjRef], mission_rule: bool, what: &str) -> Result<i64, VmError> {
    if mission_rule && let Ok(mission) = vm.find_class("WillowGame.MissionItemDefinition") {
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
    let level = rarity(vm, definition, &parts, layout.mission_rule, what)?;
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

/// The modifiers of a stack array, read as `ow-attr` reads one (module documentation).
fn modifiers(vm: &Vm, stack: &Value, what: &str) -> Result<Vec<Modifier>, VmError> {
    let Value::Array(items) = stack else {
        return Err(bad(what, format!("a modifier stack holds a {}, not an array", stack.kind_name())));
    };
    let mut mods = Vec::with_capacity(items.len());
    for item in items.iter() {
        let Some(m) = object_of(item, what)? else { continue };
        let raw = match required(vm, m, "Type", what)? {
            Value::Byte(b) => i64::from(b),
            Value::Int(i) => i64::from(i),
            other => return Err(bad(what, format!("a modifier Type is a {}", other.kind_name()))),
        };
        let value = match required(vm, m, "Value", what)? {
            Value::Float(x) => x,
            other => return Err(bad(what, format!("a modifier Value is a {}", other.kind_name()))),
        };
        if let Some(kind) = ModifierType::from_raw(raw) {
            mods.push(Modifier { kind, value, id: modifier_id_of(m) });
        }
    }
    Ok(mods)
}

/// `RecomputeNormalizedDamageValues()` on `weapon` (module documentation).
///
/// Everything that can refuse is settled before the first write (reviews of #428 and #446), in this order: the six
/// output addresses (each declared and readable on the weapon), then every stack's modifiers, then the bases (the only
/// step that runs a resolver), then each staged value against its output's declaration, and only then the writes. So
/// a class missing an output is refused before any resolver runs, and a base resolver that changes a modifier does
/// not change this recompute.
fn recompute_normalized_damage(vm: &mut Vm, weapon: ObjRef) -> Result<(), VmError> {
    let what = NORMALIZED_DAMAGE;
    let data = required(vm, weapon, "DefinitionData", what)?;
    // A bare None is an absent definition, as an empty object reference is (review r2 of #446).
    let definition = match field(vm, &data, WEAPON.definition)? {
        Value::None => None,
        v => object_of(&v, what)?,
    };
    let Some(definition) = definition else {
        return Err(NotYet::not_read(
            "RecomputeNormalizedDamageValues on a weapon without a WeaponTypeDefinition",
            RECORD,
        )
        .into());
    };
    loaded(vm, definition)?;
    // 1. The six output addresses.
    let class = vm.space.heap.get(weapon)?.class;
    let mut outputs = Vec::with_capacity(NORMALIZED.len());
    for n in &NORMALIZED {
        let mut at = Vec::with_capacity(3);
        for name in [n.out_stack, n.out_base, n.out_value] {
            let Some(p) = vm.space.prop(class, name) else {
                return Err(bad(what, format!("the weapon declares no {name}")));
            };
            let a = PropAddr::scalar(p);
            vm.space.read(weapon, a)?;
            at.push(a);
        }
        outputs.push(at);
    }
    // 2. Every stack's modifiers, before any base is evaluated.
    let mut stacks = Vec::with_capacity(NORMALIZED.len());
    for n in &NORMALIZED {
        let stack = required(vm, weapon, n.stack, what)?;
        let mods = modifiers(vm, &stack, what)?;
        stacks.push((stack, mods));
    }
    // 3. The bases.
    let form = super::let_forms().combine;
    let mut writes = Vec::with_capacity(NORMALIZED.len() * 3);
    for ((n, at), (stack, mods)) in NORMALIZED.iter().zip(outputs).zip(stacks) {
        let Some(base) = init_value(vm, definition, n.definition, weapon)? else {
            return Err(bad(what, format!("the weapon type definition declares no {}", n.definition)));
        };
        let value = combine(base, &mods, form, NumKind::Float).to_f32();
        writes.push((at[0], stack));
        writes.push((at[1], Value::Float(base)));
        writes.push((at[2], Value::Float(value)));
    }
    // 4. Every staged value against its output's declaration.
    for (a, v) in &writes {
        let kind = &vm
            .space
            .classes
            .decl(a.prop)
            .ok_or_else(|| bad(what, "an output property has no declaration"))?
            .kind;
        vm.space.heap.check_value(&vm.space.classes, kind, v)?;
    }
    // 5. The writes.
    for (a, v) in writes {
        vm.space.write(weapon, a, v)?;
    }
    Ok(())
}

fn this(c: &NativeCall<'_>, what: &str) -> Result<ObjRef, VmError> {
    c.this.ok_or_else(|| bad(what, "called without an inventory"))
}

/// Binds the module's natives in `ParityMode::Off`; Reference binds nothing (module documentation).
pub fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    let natives: [(&str, NativeFn); 3] = [
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
        (
            NORMALIZED_DAMAGE,
            Rc::new(|c: &mut NativeCall<'_>| {
                let weapon = this(c, NORMALIZED_DAMAGE)?;
                recompute_normalized_damage(c.vm, weapon)?;
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
