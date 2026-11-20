//! Owner: lane IMPL-STATIC-GET-WEAPON-DAMAGE-TYPE (decision 2026-11-19-192, number pending).
//! `WillowWeapon.StaticGetWeaponDamageType(WeaponDefinitionData)` in `ParityMode::Off`. AI-assisted (Claude Code).
//!
//! **Clean room.** Implemented from `docs/verification/NATIVE_STATIC_GET_WEAPON_DAMAGE_TYPE.md` (#576, accepted at 3225f887,
//! analyst lane ANALYST-STATIC-GET-WEAPON-DAMAGE-TYPE) and the hub's points on #576 r1; this lane did not read the
//! executable. **Every rule is UNVERIFIED in the running game.** Reference binds nothing (the stub policy).
//!
//! **The rule** (the note's section 2), with `D` the definition data passed by value:
//! 1. `D.WeaponTypeDefinition` None: the answer is None. Nothing else is read, so no field is checked on that path
//!    (hub point 1 on #576 r1);
//! 2. otherwise the type's `DefaultDamageTypeDefinition`, replaced by the elemental part's `CustomDamageTypeDefinition`
//!    when the part is set and that field is set.
//!
//! **The preflight** ([`damage_type`]) is write-free, load-free and draws nothing: the note's checks 1 to 7 (#576 at
//! 3225f887, section 4) in the native's read order, the first failing one answering:
//! 1. `D` declares `WeaponTypeDefinition` as an object field (else a `VmError::Script` naming it);
//! 2. a None type answers None, the `ElementalPartDefinition` field not looked at;
//! 3. an unloaded type is `NotYet { NotRead }` ([`TYPE_UNLOADED`]): a load would be a write;
//! 4. the type's class declares `DefaultDamageTypeDefinition` as an object (else `VmError::Script`), before anything
//!    of the elemental part, so a loaded elemental part with its own type never answers over the type;
//! 5. `D` declares `ElementalPartDefinition` as an object field (else `VmError::Script`);
//! 6. a set, unloaded part is `NotYet { NotRead }` ([`PART_UNLOADED`]);
//! 7. a set part's class declares `CustomDamageTypeDefinition` as an object (else `VmError::Script`).
//!
//! The answer is returned as stored, never loaded. The native neither writes nor loads, so none of its refusals takes
//! the "stopped after partial writes" mark (the note's section 4).
//!
//! **Not shared with the shot pipeline.** `ow-combat`'s `hit::shot::weapon_damage_type` loads its definitions and is
//! left as it is; [`choose`] is this binding's own load-free core over references already read.

use std::rc::Rc;

use ow_core::{NotYet, ParityMode};
use ow_object::ids::ObjRef;
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;

use super::bad;

/// The analyst note.
pub const RECORD: &str = "docs/verification/NATIVE_STATIC_GET_WEAPON_DAMAGE_TYPE.md";
/// The native's key.
pub const KEY: &str = "WillowWeapon.StaticGetWeaponDamageType(weapondefinitiondata)";
const NATIVE: &str = "WillowWeapon.StaticGetWeaponDamageType";

/// The note's refusal texts (its section 4).
pub const TYPE_UNLOADED: &str =
    "StaticGetWeaponDamageType: the weapon type definition is not loaded (a load is a write)";
pub const PART_UNLOADED: &str =
    "StaticGetWeaponDamageType: the elemental part definition is not loaded (a load is a write)";

/// The rule's choice over references already read: the part's custom type when set, else the type's default.
pub fn choose(type_default: Option<ObjRef>, part_custom: Option<ObjRef>) -> Option<ObjRef> {
    part_custom.or(type_default)
}

/// Struct field `name` of `data` as an object reference; a missing or non-object field is a data error.
fn struct_object(vm: &Vm, data: &Value, name: &str) -> Result<Option<ObjRef>, VmError> {
    // Both errors name the field read (review r1 of #582, item 1).
    let s = data
        .as_struct()
        .ok_or_else(|| bad(NATIVE, format!("WeaponDefinitionData.{name}: the argument is not a WeaponDefinitionData struct")))?;
    let layout = vm
        .space
        .classes
        .struct_layout(s.ty)
        .ok_or_else(|| bad(NATIVE, format!("WeaponDefinitionData.{name}: the argument's struct has no registered layout")))?;
    let (index, decl) = vm
        .space
        .names
        .find(name)
        .and_then(|n| layout.field(n))
        .ok_or_else(|| bad(NATIVE, format!("WeaponDefinitionData declares no {name}")))?;
    if !matches!(decl.kind, PropKind::Object { .. }) {
        return Err(bad(NATIVE, format!("WeaponDefinitionData.{name} is not an object field")));
    }
    match usize::try_from(index).ok().and_then(|i| s.fields.get(i)) {
        Some(Value::Object(o)) => Ok(*o),
        _ => Err(bad(NATIVE, format!("WeaponDefinitionData.{name} holds no object"))),
    }
}

/// Object property `name` of the loaded definition `o`; a class that does not declare it as an object is a data error.
fn definition_object(vm: &Vm, o: ObjRef, name: &str) -> Result<Option<ObjRef>, VmError> {
    let class = vm.space.heap.get(o)?.class;
    let path = vm.space.classes.get_layout(class).map(|l| l.path().as_str().to_string()).unwrap_or_default();
    let p = vm.space.prop(class, name).ok_or_else(|| bad(NATIVE, format!("{path} declares no {name}")))?;
    if !vm.space.classes.decl(p).is_some_and(|d| matches!(d.kind, PropKind::Object { .. })) {
        return Err(bad(NATIVE, format!("{path}.{name} is not an object property")));
    }
    match vm.space.read(o, ow_object::ids::PropAddr::scalar(p))? {
        Value::Object(x) => Ok(*x),
        other => Err(bad(NATIVE, format!("{path}.{name} holds {}, not an object", other.kind_name()))),
    }
}

fn unloaded(vm: &Vm, o: ObjRef) -> bool {
    vm.defs.is_unloaded(&vm.space, o) || vm.defs.is_failed(&vm.space, o)
}

/// The native's answer for definition data `data` (module documentation): the note's checks 1 to 7 in the native's
/// read order, the first failing one answering. Reads only.
pub fn damage_type(vm: &Vm, data: &Value) -> Result<Option<ObjRef>, VmError> {
    // 1 and 2: the type field; a None type answers None, the elemental field not looked at.
    let Some(weapon_type) = struct_object(vm, data, "WeaponTypeDefinition")? else {
        return Ok(None);
    };
    // 3: an unloaded type.
    if unloaded(vm, weapon_type) {
        return Err(NotYet::not_read(TYPE_UNLOADED, RECORD).into());
    }
    // 4: the type's default, before anything of the elemental part.
    let type_default = definition_object(vm, weapon_type, "DefaultDamageTypeDefinition")?;
    // 5 to 7: the elemental field, an unloaded part, the part's custom field.
    let part_custom = match struct_object(vm, data, "ElementalPartDefinition")? {
        None => None,
        Some(p) => {
            if unloaded(vm, p) {
                return Err(NotYet::not_read(PART_UNLOADED, RECORD).into());
            }
            definition_object(vm, p, "CustomDamageTypeDefinition")?
        }
    };
    Ok(choose(type_default, part_custom))
}

fn native(call: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let data = call.arg(0)?.clone();
    Ok(Value::Object(damage_type(call.vm, &data)?))
}

/// Binds the native in `ParityMode::Off`; Reference binds nothing (module documentation). A key bound first keeps its
/// binding.
pub fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off || vm.has_native(KEY) {
        return Ok(());
    }
    vm.register_native(KEY, Rc::new(native))
}

#[cfg(test)]
#[path = "damage_type_tests.rs"]
mod tests;
