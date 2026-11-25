//! Owner: lane IMPL-SETMATERIAL-AND-GLOW. `MeshComponent.SetMaterial(ElementIndex, Material)` and
//! `MaterialInterface.GetScalarParameterValue(ParameterName, out OutValue)` in `ParityMode::Off`, link 8's next stops
//! after `BuildWeaponMaterial`. AI-assisted (Claude Code).
//!
//! **Clean room.** Implemented from docs/verification/NATIVE_INITMESHES_MATERIAL_GLOW.md section 4 alone (#705, lane
//! ANALYST-INITMESHES-SETMATERIAL-GLOW, decision 2026-11-25-39); this lane did not read the executable. **Every rule is
//! UNVERIFIED in the running game.** Reference binds nothing (the stub policy answers).
//!
//! **`SetMaterial`** ([`set_material`]). The preflight comes before any write, in this order:
//! 1. the declarations, as `plan_flag` checks them: `component_setters::schemas` on `REATTACH_FIELDS`, and `Materials`
//!    declared as an array of objects (else `VmError::Script` naming the field). So the negative-index shortcut comes
//!    after the declarations;
//! 2. a negative index: nothing;
//! 3. an index within `Materials` whose entry is already the material: nothing;
//! 4. otherwise the deferred reattach's preflight, today's `component_setters::reattach_check`: `Owner` set is
//!    `REATTACH_OWNER`, no `Owner` with a `Scene` set is `REATTACH_SCENE`. PR #709 is amending that check's owner
//!    branch; this lane builds to the check as it is on its base.
//!
//! The commit grows `Materials` to reach the index (**UNVERIFIED (fitted): the new entries are None**), sets the entry,
//! then `bNeedsReattach` = true. Every write after the first goes through `component_setters::after_writes`. A receiver
//! the store has not loaded is [`SET_MATERIAL_NOT_LOADED`] (`NotRead`).
//!
//! **`GetScalarParameterValue`** ([`get_scalar_parameter_value`]), by the receiver's first native class (the fitted
//! native-bit walk):
//! - `Engine.MaterialInterface` or `Engine.MaterialInstance`: false;
//! - `Engine.MaterialInstanceConstant`: false when its `ReentrantFlag` is set; else the first `ScalarParameterValues`
//!   entry whose `ParameterName` equals the name (names compared as names, number included) gives its `ParameterValue`
//!   and true; else its `Parent` is asked; else false. The game's set-and-clear of `ReentrantFlag` around the parent
//!   call is kept **without writing**: the walk carries the instances it is asking, and one met again answers false,
//!   as the flag would;
//! - `Engine.Material`: the first `Expressions` entry that is a `MaterialExpressionScalarParameter` (by fit) with that
//!   `ParameterName` gives its `DefaultValue` and true; else false. A None entry is skipped. Each other entry must be
//!   resident, and the class test needs `Engine.MaterialExpressionScalarParameter` ready (`globals::ready_class`),
//!   checked only when a non-None entry is tested;
//! - any other first native class: [`scalar_lookup_of`] (`NotRead`).
//!
//! The walk is decided read-only. Then, on true only, `OutValue` is written through the out argument; on false nothing
//! is written. A non-resident object the walk must read is [`SCALAR_NOT_LOADED`] (`NotRead`). A field missing or of
//! the wrong kind is a `VmError::Script` naming it.

use std::rc::Rc;

use ow_core::{NotYet, ParityMode};
use ow_object::ids::{ClassId, ObjRef};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;
use ow_world::world::component_setters::{self, Slots};

use super::bad;

/// The record of both rules.
pub const RECORD: &str = "docs/verification/NATIVE_INITMESHES_MATERIAL_GLOW.md";
/// `SetMaterial`'s native key.
pub const SET_MATERIAL: &str = "MeshComponent.SetMaterial(int,object)";
/// `GetScalarParameterValue`'s native key.
pub const GET_SCALAR_PARAMETER_VALUE: &str = "MaterialInterface.GetScalarParameterValue(name,float)";
/// A `SetMaterial` receiver the store has not loaded.
pub const SET_MATERIAL_NOT_LOADED: &str = "SetMaterial: an object the store has not loaded (a load is a write)";
/// An object the scalar walk must read that the store has not loaded.
pub const SCALAR_NOT_LOADED: &str = "GetScalarParameterValue: an object the store has not loaded (a load is a write)";
/// The scalar-parameter expression class `Material`'s walk tests against (by fit).
pub const SCALAR_EXPRESSION: &str = "Engine.MaterialExpressionScalarParameter";

const SET_WHAT: &str = "MeshComponent.SetMaterial";
const GET_WHAT: &str = "MaterialInterface.GetScalarParameterValue";

/// The refusal of a receiver of a first native class whose scalar lookup was not read.
pub fn scalar_lookup_of(class: &str) -> String {
    format!("GetScalarParameterValue: the scalar lookup of {class}")
}

/// The refusal when [`SCALAR_EXPRESSION`] is not ready with no load.
pub fn expression_class_not_ready() -> String {
    format!("GetScalarParameterValue: the class {SCALAR_EXPRESSION}, not loaded (a class load is a write)")
}

fn class_path(vm: &Vm, c: ClassId) -> String {
    vm.space.classes.get_layout(c).map(|l| l.path().to_string()).unwrap_or_else(|| format!("{c:?}"))
}

/// `o` resident: not a store handle still unloaded or failed, and its slots built.
fn resident(vm: &Vm, o: ObjRef, refusal: &'static str) -> Result<(), VmError> {
    let x = vm.space.heap.get(o)?;
    let short = vm.space.classes.get_layout(x.class).is_some_and(|l| x.slots.len() < l.slot_count() as usize);
    if vm.defs.is_unloaded(&vm.space, o) || vm.defs.is_failed(&vm.space, o) || short {
        return Err(NotYet::not_read(refusal, RECORD).into());
    }
    Ok(())
}

/// The path of `class`'s first native class, from `class` up: the class body's fitted native bit where the code index
/// holds the class (a body that did not decode counts as native), else the class table's mark. Reads only.
fn first_native(vm: &Vm, class: ClassId) -> Option<String> {
    let code = vm.defs.code();
    let mut cursor = Some(class);
    while let Some(c) = cursor {
        let layout = vm.space.classes.get_layout(c)?;
        let native = match code.key_of(c) {
            Some(k) => code.class(k).body.as_ref().map_or(true, |b| b.is_native()),
            None => vm.space.classes.is_native(c),
        };
        if native {
            return Some(layout.path().as_str().to_string());
        }
        cursor = layout.super_class();
    }
    None
}

// ---- SetMaterial --------------------------------------------------------------------------------------------------

/// What `SetMaterial` will do, decided before any write.
#[derive(Debug, Clone, PartialEq)]
enum MaterialPlan {
    Nothing,
    /// The new `Materials` array, then `bNeedsReattach` = true.
    Write(Vec<Value>),
}

/// `SetMaterial`'s preflight (module documentation), read only.
fn plan_set_material(vm: &Vm, component: ObjRef, index: i32, material: Option<ObjRef>) -> Result<MaterialPlan, VmError> {
    resident(vm, component, SET_MATERIAL_NOT_LOADED)?;
    // 1. The declarations first.
    component_setters::schemas(vm, component, &component_setters::REATTACH_FIELDS, SET_WHAT)?;
    match component_setters::declared(vm, component, "Materials", SET_WHAT)? {
        PropKind::Array(inner) if matches!(*inner, PropKind::Object { .. }) => {}
        other => return Err(bad(SET_WHAT, format!("Materials is declared as {other:?}, not an array of objects"))),
    }
    // 2. A negative index.
    let Ok(index) = usize::try_from(index) else {
        return Ok(MaterialPlan::Nothing);
    };
    let list = Slots::of(component).required(vm, "Materials", SET_WHAT)?;
    let Some(entries) = list.elements() else {
        return Err(bad(SET_WHAT, format!("Materials holds {}, not an array", list.kind_name())));
    };
    // 3. The entry already holds the material.
    let want = Value::Object(material);
    if entries.get(index) == Some(&want) {
        return Ok(MaterialPlan::Nothing);
    }
    // 4. The deferred reattach's refusals (today's reattach_check; PR #709 amends its owner branch).
    component_setters::reattach_check(vm, Slots::of(component), SET_WHAT)?;
    let mut next = entries.to_vec();
    if next.len() <= index {
        // UNVERIFIED (fitted): the grown entries are None (NATIVE_INITMESHES_MATERIAL_GLOW.md section 4.1).
        next.resize(index + 1, Value::Object(None));
    }
    next[index] = want;
    Ok(MaterialPlan::Write(next))
}

/// `MeshComponent.SetMaterial(int ElementIndex, MaterialInterface Material)` in Off (module documentation).
pub fn set_material(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let component = c.this.ok_or_else(|| bad(SET_WHAT, "called without a component"))?;
    let index = match c.arg(0)? {
        Value::Int(i) => *i,
        other => return Err(bad(SET_WHAT, format!("ElementIndex holds {}, not an int", other.kind_name()))),
    };
    let material = match c.arg(1)? {
        Value::Object(o) | Value::Interface(o) => *o,
        other => return Err(bad(SET_WHAT, format!("Material holds {}, not an object", other.kind_name()))),
    };
    match plan_set_material(c.vm, component, index, material)? {
        MaterialPlan::Nothing => {}
        MaterialPlan::Write(entries) => {
            component_setters::write(c.vm, component, "Materials", Value::array(entries), SET_WHAT)?;
            component_setters::write(c.vm, component, "bNeedsReattach", Value::Bool(true), SET_WHAT)
                .map_err(component_setters::after_writes)?;
        }
    }
    Ok(Value::None)
}

// ---- GetScalarParameterValue --------------------------------------------------------------------------------------

/// Field `name` of the resident `o` with its declared kind; a missing declaration is a data error naming it.
fn field_of(vm: &Vm, o: ObjRef, name: &str) -> Result<(PropKind, Value), VmError> {
    let kind = component_setters::declared(vm, o, name, GET_WHAT)?;
    Ok((kind, Slots::of(o).required(vm, name, GET_WHAT)?))
}

fn bool_of(vm: &Vm, o: ObjRef, name: &str) -> Result<bool, VmError> {
    match field_of(vm, o, name)? {
        (PropKind::Bool, Value::Bool(b)) => Ok(b),
        (k, _) => Err(bad(GET_WHAT, format!("{name} is declared as {}, not a bool", k.kind_name()))),
    }
}

fn object_of(vm: &Vm, o: ObjRef, name: &str) -> Result<Option<ObjRef>, VmError> {
    match field_of(vm, o, name)? {
        (PropKind::Object { .. }, Value::Object(x)) => Ok(x),
        (k, _) => Err(bad(GET_WHAT, format!("{name} is declared as {}, not an object", k.kind_name()))),
    }
}

/// The array field `name` of `o`, its elements; a field not declared as an array is a data error.
fn array_of(vm: &Vm, o: ObjRef, name: &str) -> Result<Vec<Value>, VmError> {
    match field_of(vm, o, name)? {
        (PropKind::Array(_), v) => {
            Ok(v.elements().map(<[Value]>::to_vec).ok_or_else(|| bad(GET_WHAT, format!("{name} holds no array")))?)
        }
        (k, _) => Err(bad(GET_WHAT, format!("{name} is declared as {}, not an array", k.kind_name()))),
    }
}

fn struct_field(vm: &Vm, entry: &Value, owner: &str, name: &str) -> Result<Value, VmError> {
    ow_attr::eval::field(&vm.space, entry, name)
        .cloned()
        .map_err(|e| bad(GET_WHAT, format!("{owner}.{name}: {e}")))
}

/// The scalar walk's answer for `o` (module documentation), read only: `Some(value)` for true, `None` for false.
/// `asking` holds the instances already being asked on this walk.
fn scalar_of(vm: &Vm, o: ObjRef, name: &Value, asking: &mut Vec<ObjRef>) -> Result<Option<f32>, VmError> {
    resident(vm, o, SCALAR_NOT_LOADED)?;
    let class = vm.space.heap.get(o)?.class;
    let native = first_native(vm, class);
    let is = |p: &str| native.as_deref().is_some_and(|n| n.eq_ignore_ascii_case(p));
    if is("Engine.MaterialInterface") || is("Engine.MaterialInstance") {
        return Ok(None);
    }
    if is("Engine.MaterialInstanceConstant") {
        // The guard, without writing: an instance already being asked answers as its set flag would.
        if asking.contains(&o) || bool_of(vm, o, "ReentrantFlag")? {
            return Ok(None);
        }
        for entry in array_of(vm, o, "ScalarParameterValues")? {
            if struct_field(vm, &entry, "ScalarParameterValues", "ParameterName")? == *name {
                return match struct_field(vm, &entry, "ScalarParameterValues", "ParameterValue")? {
                    Value::Float(f) => Ok(Some(f)),
                    other => Err(bad(GET_WHAT, format!("ParameterValue holds {}, not a float", other.kind_name()))),
                };
            }
        }
        let Some(parent) = object_of(vm, o, "Parent")? else {
            return Ok(None);
        };
        asking.push(o);
        let answer = scalar_of(vm, parent, name, asking);
        asking.pop();
        return answer;
    }
    if is("Engine.Material") {
        let mut scalar: Option<ClassId> = None;
        for entry in array_of(vm, o, "Expressions")? {
            let e = match entry {
                Value::Object(None) => continue,
                Value::Object(Some(e)) => e,
                other => return Err(bad(GET_WHAT, format!("an Expressions entry holds {}", other.kind_name()))),
            };
            resident(vm, e, SCALAR_NOT_LOADED)?;
            let scalar = match scalar {
                Some(s) => s,
                None => {
                    let s = crate::globals::ready_class(vm, SCALAR_EXPRESSION)
                        .map_err(|_| VmError::from(NotYet::not_read(expression_class_not_ready(), RECORD)))?;
                    *scalar.insert(s)
                }
            };
            let e_class = vm.space.heap.get(e)?.class;
            if !vm.space.classes.is_child_of(e_class, scalar) {
                continue;
            }
            if field_of(vm, e, "ParameterName")?.1 == *name {
                return match field_of(vm, e, "DefaultValue")? {
                    (PropKind::Float, Value::Float(f)) => Ok(Some(f)),
                    (k, _) => Err(bad(GET_WHAT, format!("DefaultValue is declared as {}, not a float", k.kind_name()))),
                };
            }
        }
        return Ok(None);
    }
    let named = native.unwrap_or_else(|| class_path(vm, class));
    Err(NotYet::not_read(scalar_lookup_of(&named), RECORD).into())
}

/// `MaterialInterface.GetScalarParameterValue(name ParameterName, out float OutValue)` in Off (module documentation).
pub fn get_scalar_parameter_value(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let material = c.this.ok_or_else(|| bad(GET_WHAT, "called without a material"))?;
    let name = match c.arg(0)? {
        v @ Value::Name(_) => v.clone(),
        other => return Err(bad(GET_WHAT, format!("ParameterName holds {}, not a name", other.kind_name()))),
    };
    let answer = scalar_of(c.vm, material, &name, &mut Vec::new())?;
    match answer {
        Some(value) => {
            c.set_out(1, Value::Float(value))?;
            Ok(Value::Bool(true))
        }
        None => Ok(Value::Bool(false)),
    }
}

/// Binds both natives in Off, first-wins: a key already bound is kept. Reference binds nothing.
pub fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    if !vm.has_native(SET_MATERIAL) {
        vm.register_native(SET_MATERIAL, Rc::new(set_material))?;
    }
    if !vm.has_native(GET_SCALAR_PARAMETER_VALUE) {
        vm.register_native(GET_SCALAR_PARAMETER_VALUE, Rc::new(get_scalar_parameter_value))?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "material_natives_tests.rs"]
mod tests;
