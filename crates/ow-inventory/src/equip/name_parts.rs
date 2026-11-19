//! Owner: lane IMPL-CACHE-WEAPON-CARD. A weapon's name parts and its card cache in `ParityMode::Off`, coded from
//! NATIVE_CACHE_WEAPON_CARD.md alone (clean room: the implementer did not read the native code). AI-assisted (Claude
//! Code). **Every rule is UNVERIFIED in the running game**; the note's last section lists the bl-sdk checks.
//!
//! - **`WillowWeapon.ChooseRandomNameParts()`** (the note's sections 2 and 5.2), [`choose_random_name_parts`]:
//!   - with no `DefinitionData.WeaponTypeDefinition` it writes nothing;
//!   - the best entry of one name-part list: entries in order, a None entry skipped; an entry qualifies when
//!     `MinExpLevelRequirement` <= the weapon's `ExpLevel` <= `MaxExpLevelRequirement`, its `Priority` is above 0 and
//!     its `Expressions` hold (`EvaluateExpressions` in And mode, the weapon as the context source, no override); a
//!     qualifying entry replaces the best when there is none yet or when the best's `Priority` is at most its own, so
//!     a later entry wins a tie. Priorities compare as floats;
//!   - the prefix and the title start as the best of the type's `PrefixList` and `TitleList`; then each of the nine
//!     part slots in [`super::WEAPON_PART_FIELDS`] order (a None slot skipped, the name-part slots not walked) offers
//!     its best, which replaces the current pick when the current one is None, or when the part's best is not None and
//!     the current one's `Priority` is at most its own. A part with no qualifying entry never clears a pick;
//!   - both picks are computed first, then both fields are written, None included. No random draw, no event, no
//!     other field.
//!   - Name parts are read as the definition store loads them: a field the package leaves untagged holds its archetype
//!     or class default (`WeaponNamePartDefinition`'s defaults, the note's section 5.1).
//!   - Any `NotYet` it returns (an expression's, or a lazy definition load's) is returned as it is, **unprefixed**: the
//!     native writes only at its very end, so no refusal comes after a write of its own. The "stopped after partial
//!     writes" prefix is added by the caller layer that wrote (review r1 of #506, the hub's rule; the same as
//!     `ApplyInternalSlotEffectModifiers`, #483): in `--slice-gate`'s link 8, the equip driver, whose `make` step
//!     wrote before the init runs.
//! - **The name-part schema** ([`name_part_schema`], section 5.1), each failure a `VmError::Script` naming the field,
//!   run by the equip driver's preflight ([`super::card::preflight`]) before the weapon is made.
//! - **`WillowWeapon.CacheWeaponCard()`** is [`super::card`]'s (lane IMPL-CACHE-WEAPON-CARD-BODY): the body note
//!   replaced the outline's refusal at entry (section 5.3) and the preflight's card refusal (section 5.1).
//!
//! **Reference** (architecture rule R3): the C++ VM does not bind the native, so [`register`] binds nothing there.

use std::rc::Rc;

use ow_attr::api::{Comparison, EvalCx, ExprData, ExprMode, Operand2Usage};
use ow_core::ParityMode;
use ow_object::ids::{ClassId, ObjRef};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;

use super::{WEAPON_PART_FIELDS, bad, field, object_of, read, write};

/// The record of this module.
pub const RECORD: &str = "docs/verification/NATIVE_CACHE_WEAPON_CARD.md";

/// The native's script function.
pub const CHOOSE_RANDOM_NAME_PARTS: &str = "WillowGame.WillowWeapon.ChooseRandomNameParts";

/// The classes the name-part rule reads.
pub const WEAPON_CLASS: &str = "WillowGame.WillowWeapon";
pub const WEAPON_TYPE_CLASS: &str = "WillowGame.WeaponTypeDefinition";
pub const WEAPON_PART_CLASS: &str = "WillowGame.WeaponPartDefinition";
pub const NAME_PART_CLASS: &str = "WillowGame.WeaponNamePartDefinition";

const SCHEMA: &str = "ChooseRandomNameParts (the name-part schema)";

/// The declared kinds the name-part rule reads (the note's section 5.1), checked on the declared classes; each
/// failure is a `VmError::Script` naming the field.
pub fn name_part_schema(vm: &mut Vm) -> Result<(), VmError> {
    let weapon = vm.find_class(WEAPON_CLASS)?;
    let weapon_type = vm.find_class(WEAPON_TYPE_CLASS)?;
    let part = vm.find_class(WEAPON_PART_CLASS)?;
    let name_part = vm.find_class(NAME_PART_CLASS)?;
    let vm = &*vm;
    let is_name_part = |kind: &PropKind| matches!(kind, PropKind::Object { class } if vm.space.classes.is_child_of(*class, name_part));
    // DefinitionData: a WeaponDefinitionData struct of object fields, the name parts typed as name parts.
    let data = declared(vm, weapon, "DefinitionData")?;
    let PropKind::Struct(ty) = data else {
        return Err(schema(format!("{WEAPON_CLASS}.DefinitionData is not a struct")));
    };
    let layout = vm
        .space
        .classes
        .struct_layout(ty)
        .ok_or_else(|| schema(format!("{WEAPON_CLASS}.DefinitionData's struct has no layout")))?;
    if !own_name_is(layout.path(), "WeaponDefinitionData") {
        return Err(schema(format!("{WEAPON_CLASS}.DefinitionData is a {}, not a WeaponDefinitionData", layout.path().as_str())));
    }
    let fields = std::iter::once("WeaponTypeDefinition")
        .chain(WEAPON_PART_FIELDS)
        .chain(["PrefixPartDefinition", "TitlePartDefinition"]);
    for name in fields {
        let kind = vm
            .space
            .names
            .find(name)
            .and_then(|id| layout.field(id))
            .filter(|(_, d)| d.array_dim == 1)
            .map(|(_, d)| d.kind.clone());
        let ok = match (&kind, name) {
            (Some(k), "PrefixPartDefinition" | "TitlePartDefinition") => is_name_part(k),
            (Some(PropKind::Object { .. }), _) => true,
            _ => false,
        };
        if !ok {
            return Err(schema(format!("WeaponDefinitionData.{name} is {kind:?}, not the object field the rule reads")));
        }
    }
    // PrefixList and TitleList: dynamic arrays of name-part references, on the type and on the part.
    for (class, path) in [(weapon_type, WEAPON_TYPE_CLASS), (part, WEAPON_PART_CLASS)] {
        for list in ["PrefixList", "TitleList"] {
            let kind = declared(vm, class, list)?;
            if !matches!(&kind, PropKind::Array(inner) if is_name_part(inner)) {
                return Err(schema(format!("{path}.{list} is {kind:?}, not an array of {NAME_PART_CLASS} references")));
            }
        }
    }
    // The name part's own fields.
    for (name, want) in [
        ("MinExpLevelRequirement", "an int"),
        ("MaxExpLevelRequirement", "an int"),
        ("Priority", "a float"),
        ("Expressions", "an array of AttributeExpressionData"),
    ] {
        let kind = declared(vm, name_part, name)?;
        let ok = match want {
            "an int" => matches!(kind, PropKind::Int),
            "a float" => matches!(kind, PropKind::Float),
            _ => matches!(&kind, PropKind::Array(inner) if matches!(**inner, PropKind::Struct(s)
                if vm.space.classes.struct_layout(s).is_some_and(|l| own_name_is(l.path(), "AttributeExpressionData")))),
        };
        if !ok {
            return Err(schema(format!("{NAME_PART_CLASS}.{name} is {kind:?}, not {want}")));
        }
    }
    if !matches!(declared(vm, weapon, "ExpLevel")?, PropKind::Int) {
        return Err(schema(format!("{WEAPON_CLASS}.ExpLevel is not an int")));
    }
    Ok(())
}

/// Whether the last component of `path` (a struct's own name, after the last `.` or `:`) is exactly `name` (review r1
/// of #506, item 4: a suffix match would let `FooWeaponDefinitionData` pass; review r2: an outer `:` counts too).
fn own_name_is(path: &ow_core::ObjectPath, name: &str) -> bool {
    path.name() == name
}

fn schema(message: String) -> VmError {
    bad(SCHEMA, message)
}

/// The declared kind of `class`'s scalar property `name`; none, or an array dimension other than 1, is a schema
/// error.
fn declared(vm: &Vm, class: ClassId, name: &str) -> Result<PropKind, VmError> {
    let path = vm.space.classes.get_layout(class).map(|l| l.path().as_str().to_string()).unwrap_or_default();
    let p = vm.space.prop(class, name).ok_or_else(|| schema(format!("{path} declares no {name}")))?;
    let decl = vm.space.classes.decl(p).ok_or_else(|| schema(format!("{path}.{name} has no declaration")))?;
    if decl.array_dim != 1 {
        return Err(schema(format!("{path}.{name} is a static array")));
    }
    Ok(decl.kind.clone())
}

const NATIVE: &str = "WillowWeapon.ChooseRandomNameParts";

/// A loaded definition `o` (a lazy one loads; loading is not a write).
pub(crate) fn loaded(vm: &mut Vm, o: ObjRef) -> Result<(), VmError> {
    if vm.defs.is_unloaded(&vm.space, o) {
        let r = vm.defs.ensure_loaded(&mut vm.space, o);
        vm.log.extend(vm.defs.take_log());
        r.map_err(VmError::from)?;
    }
    Ok(())
}

/// `o`'s int property `name` (declared, else a data error).
fn int_of(vm: &Vm, o: ObjRef, name: &str) -> Result<i32, VmError> {
    match super::required(vm, o, name, NATIVE)? {
        Value::Int(x) => Ok(x),
        other => Err(bad(NATIVE, format!("{name} holds {}, not an int", other.kind_name()))),
    }
}

fn priority(vm: &Vm, o: ObjRef) -> Result<f32, VmError> {
    match super::required(vm, o, "Priority", NATIVE)? {
        Value::Float(x) => Ok(x),
        other => Err(bad(NATIVE, format!("Priority holds {}, not a float", other.kind_name()))),
    }
}

/// One `AttributeExpressionData` value as `ow-attr`'s expression (enum bytes by the package values).
fn expr_data(vm: &Vm, v: &Value) -> Result<ExprData, VmError> {
    let byte = |name: &str| match field(vm, v, name)? {
        Value::Byte(b) => Ok(i64::from(b)),
        other => Err(bad(NATIVE, format!("{name} holds {}, not a byte", other.kind_name()))),
    };
    let float = |name: &str| match field(vm, v, name)? {
        Value::Float(x) => Ok(x),
        other => Err(bad(NATIVE, format!("{name} holds {}, not a float", other.kind_name()))),
    };
    Ok(ExprData {
        attribute_operand1: object_of(&field(vm, v, "AttributeOperand1")?, NATIVE)?,
        comparison_operator: Comparison::from_raw(byte("ComparisonOperator")?)
            .ok_or_else(|| bad(NATIVE, "ComparisonOperator outside its enum"))?,
        operand2_usage: Operand2Usage::from_raw(byte("Operand2Usage")?)
            .ok_or_else(|| bad(NATIVE, "Operand2Usage outside its enum"))?,
        attribute_operand2: object_of(&field(vm, v, "AttributeOperand2")?, NATIVE)?,
        constant_operand2: float("ConstantOperand2")?,
    })
}

/// Whether name part `entry` qualifies for `weapon` at `level` (module documentation).
fn qualifies(vm: &mut Vm, weapon: ObjRef, level: i32, entry: ObjRef) -> Result<bool, VmError> {
    loaded(vm, entry)?;
    let (min, max) = (int_of(vm, entry, "MinExpLevelRequirement")?, int_of(vm, entry, "MaxExpLevelRequirement")?);
    // "Above 0" as a float comparison: a NaN priority does not qualify.
    let above_zero = priority(vm, entry)?.partial_cmp(&0.0) == Some(std::cmp::Ordering::Greater);
    if !(min..=max).contains(&level) || !above_zero {
        return Ok(false);
    }
    let rows = match super::required(vm, entry, "Expressions", NATIVE)? {
        Value::Array(rows) => rows,
        other => return Err(bad(NATIVE, format!("Expressions holds {}, not an array", other.kind_name()))),
    };
    if rows.is_empty() {
        return Ok(true);
    }
    let exprs = rows.iter().map(|r| expr_data(vm, r)).collect::<Result<Vec<_>, _>>()?;
    let sys = vm.attrs.clone();
    Ok(sys.expressions_hold(&exprs, ExprMode::And, Some(weapon), None, &mut EvalCx::new(vm))?)
}

/// The best entry of `holder`'s list `name` for `weapon` at `level` (module documentation); None when none qualifies.
fn best_of(vm: &mut Vm, weapon: ObjRef, level: i32, holder: ObjRef, name: &str) -> Result<Option<ObjRef>, VmError> {
    loaded(vm, holder)?;
    let entries = match read(vm, holder, name)? {
        Some(Value::Array(items)) => items,
        Some(other) => return Err(bad(NATIVE, format!("{name} holds {}, not an array", other.kind_name()))),
        None => {
            return Err(bad(
                NATIVE,
                format!("{} declares no {name}", ow_attr::eval::class_path(&vm.space, holder)),
            ));
        }
    };
    let mut best: Option<(ObjRef, f32)> = None;
    for e in entries.iter() {
        let Some(entry) = object_of(e, NATIVE)? else {
            continue;
        };
        if !qualifies(vm, weapon, level, entry)? {
            continue;
        }
        let p = priority(vm, entry)?;
        if best.is_none_or(|(_, b)| b <= p) {
            best = Some((entry, p));
        }
    }
    Ok(best.map(|(o, _)| o))
}

/// `current` after a part offers `offered` (module documentation): replaced when `current` is None, or when the offer
/// is not None and `current`'s priority is at most the offer's.
fn merged(vm: &Vm, current: Option<ObjRef>, offered: Option<ObjRef>) -> Result<Option<ObjRef>, VmError> {
    Ok(match (current, offered) {
        (None, offer) => offer,
        (Some(c), Some(o)) if priority(vm, c)? <= priority(vm, o)? => Some(o),
        (keep, _) => keep,
    })
}

/// `ChooseRandomNameParts` on `weapon` in Off (module documentation).
pub fn choose_random_name_parts(vm: &mut Vm, weapon: ObjRef) -> Result<(), VmError> {
    picks(vm, weapon).and_then(|p| match p {
        None => Ok(()),
        Some((prefix, title)) => {
            let mut data = super::required(vm, weapon, "DefinitionData", NATIVE)?;
            set(vm, &mut data, "PrefixPartDefinition", prefix)?;
            set(vm, &mut data, "TitlePartDefinition", title)?;
            write(vm, weapon, "DefinitionData", data, NATIVE)
        }
    })
}

/// The prefix and the title picks.
type Picks = (Option<ObjRef>, Option<ObjRef>);

/// The two picks, writing nothing; None when the weapon has no type.
fn picks(vm: &mut Vm, weapon: ObjRef) -> Result<Option<Picks>, VmError> {
    let data = super::required(vm, weapon, "DefinitionData", NATIVE)?;
    let Some(weapon_type) = object_of(&field(vm, &data, "WeaponTypeDefinition")?, NATIVE)? else {
        return Ok(None);
    };
    let level = int_of(vm, weapon, "ExpLevel")?;
    let mut prefix = best_of(vm, weapon, level, weapon_type, "PrefixList")?;
    let mut title = best_of(vm, weapon, level, weapon_type, "TitleList")?;
    for slot in WEAPON_PART_FIELDS {
        let Some(part) = object_of(&field(vm, &data, slot)?, NATIVE)? else {
            continue;
        };
        let offered = best_of(vm, weapon, level, part, "PrefixList")?;
        prefix = merged(vm, prefix, offered)?;
        let offered = best_of(vm, weapon, level, part, "TitleList")?;
        title = merged(vm, title, offered)?;
    }
    Ok(Some((prefix, title)))
}

fn set(vm: &Vm, data: &mut Value, name: &str, v: Option<ObjRef>) -> Result<(), VmError> {
    let Value::Struct(s) = data else {
        return Err(bad(NATIVE, "DefinitionData is not a struct"));
    };
    let ty = s.ty;
    let layout = vm.space.classes.struct_layout(ty).ok_or_else(|| bad(NATIVE, "DefinitionData has no layout"))?;
    let (i, _) = vm
        .space
        .names
        .find(name)
        .and_then(|id| layout.field(id))
        .ok_or_else(|| bad(NATIVE, format!("DefinitionData has no {name}")))?;
    let slot = Rc::make_mut(s)
        .fields
        .get_mut(i as usize)
        .ok_or_else(|| bad(NATIVE, format!("DefinitionData.{name} is out of range")))?;
    *slot = Value::Object(v);
    Ok(())
}

/// Binds the native in `ParityMode::Off` by its declaration's own native key (a package without the function binds
/// nothing); Reference binds nothing (module documentation).
pub fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    type Body = fn(&mut Vm, ObjRef) -> Result<(), VmError>;
    let natives: [(&str, Body); 1] = [(CHOOSE_RANDOM_NAME_PARTS, choose_random_name_parts)];
    for (path, body) in natives {
        let Ok(id) = vm.find_function(path) else {
            continue;
        };
        let Some(key) = vm.function(id).and_then(|d| d.native_key.clone()) else {
            continue;
        };
        vm.register_native(
            &key,
            Rc::new(move |c: &mut NativeCall<'_>| {
                let me = c.this.ok_or_else(|| bad(path, "called without a weapon"))?;
                body(c.vm, me)?;
                Ok(Value::None)
            }),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
