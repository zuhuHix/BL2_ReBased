//! Owner: lane IMPL-PSC-SETCOLORPARAMETER. `ParticleSystemComponent.SetColorParameter(name, color)` in
//! `ParityMode::Off`, link 8's next stop after the glow read (`WillowWeapon.InitEffects`). AI-assisted (Claude Code).
//!
//! **Clean room.** Implemented from docs/verification/NATIVE_PSC_SET_COLOR_PARAMETER.md section 3 alone (#719 at its
//! review r1 head, lane ANALYST-PSC-SETCOLORPARAMETER, decision 2026-11-25-54); this lane did not read the executable.
//! **Every rule is UNVERIFIED in the running game.** Reference binds nothing (the stub policy).
//!
//! **Preflight**, read only, no load, nothing interned, in this order:
//! 1. no receiver: a data error;
//! 2. a None name: nothing, and nothing else is read. This comes before the receiver's load check, because the game
//!    reads nothing of the receiver then;
//! 3. a receiver the store has not loaded: [`NOT_LOADED`] (`NotRead`);
//! 4. the declaration: `InstanceParameters`, an array of a struct with `Name` (a name), `ParamType` (a byte) and
//!    `Color` (a struct of four bytes `R`, `G`, `B`, `A`); else `VmError::Script` naming it. Each element must hold
//!    those kinds, else `VmError::Script`;
//! 5. the plan: the first element with that `Name` and `ParamType` [`PARAM_COLOR`] gets its `Color` replaced (its
//!    other fields kept); with none, the struct's zero value with the three fields set is appended. An element with the
//!    name and another type is not matched. The whole array is built here and checked against the declaration.
//!
//! **Write:** one, `InstanceParameters` := the planned array. No other field is written: no reattach, no transform
//! update, no render-side state.

use std::rc::Rc;

use ow_core::NotYet;
use ow_object::ids::{NameId, ObjRef};
use ow_object::layout::PropKind;
use ow_object::value::{StructValue, Value};
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;
use ow_world::world::component_setters::fits;

use super::particle_activation::{self as pa, K};
use crate::equip::{bad, field, required, write};

/// The record of the rule.
pub const RECORD: &str = "docs/verification/NATIVE_PSC_SET_COLOR_PARAMETER.md";
/// The native's key.
pub const SET_COLOR_PARAMETER: &str = "ParticleSystemComponent.SetColorParameter(name,color)";
/// `PSPT_Color` (the note's enum read).
pub const PARAM_COLOR: u8 = 5;
/// A receiver the store has not loaded.
pub const NOT_LOADED: &str = "SetColorParameter on a component not loaded at set-up (a load is a write)";
const WHAT: &str = "ParticleSystemComponent.SetColorParameter";

const COLOR: K = K::StructOf(&[("R", K::Byte), ("G", K::Byte), ("B", K::Byte), ("A", K::Byte)]);
const PARAMETER: K = K::StructOf(&[("Name", K::Name), ("ParamType", K::Byte), ("Color", COLOR)]);
const FIELDS: [(&str, K); 1] = [("InstanceParameters", K::ArrayOf(&PARAMETER))];
const CHANNELS: [&str; 4] = ["R", "G", "B", "A"];

fn byte_of(vm: &Vm, v: &Value, name: &str) -> Result<u8, VmError> {
    match field(vm, v, name)? {
        Value::Byte(b) => Ok(b),
        other => Err(bad(WHAT, format!("{name} holds {}, not a byte", other.kind_name()))),
    }
}

/// A colour's four channels, in `CHANNELS` order.
fn channels(vm: &Vm, color: &Value) -> Result<[u8; 4], VmError> {
    Ok([byte_of(vm, color, "R")?, byte_of(vm, color, "G")?, byte_of(vm, color, "B")?, byte_of(vm, color, "A")?])
}

/// `v`, a struct, with the named fields replaced.
fn with_fields(vm: &Vm, v: &Value, fields: &[(&str, Value)]) -> Result<Value, VmError> {
    let Value::Struct(s) = v else { return Err(bad(WHAT, format!("{} is not a struct", v.kind_name()))) };
    let layout = vm.space.classes.struct_layout(s.ty).ok_or_else(|| bad(WHAT, "a struct with no layout"))?;
    let mut out = s.fields.clone();
    for (name, value) in fields {
        let (i, _) = vm.space.names.find(name).and_then(|n| layout.field(n)).ok_or_else(|| bad(WHAT, format!("no field {name}")))?;
        out[i as usize] = value.clone();
    }
    Ok(Value::Struct(Rc::new(StructValue { ty: s.ty, fields: out })))
}

/// `base`, a `Color` struct of the element's own type, with the four channels set.
fn color_like(vm: &Vm, base: &Value, rgba: [u8; 4]) -> Result<Value, VmError> {
    let fields: Vec<(&str, Value)> = CHANNELS.iter().zip(rgba).map(|(c, b)| (*c, Value::Byte(b))).collect();
    with_fields(vm, base, &fields)
}

fn resident(vm: &Vm, o: ObjRef) -> Result<(), VmError> {
    let x = vm.space.heap.get(o)?;
    let short = vm.space.classes.get_layout(x.class).is_some_and(|l| x.slots.len() < l.slot_count() as usize);
    if vm.defs.is_unloaded(&vm.space, o) || vm.defs.is_failed(&vm.space, o) || short {
        return Err(NotYet::not_read(NOT_LOADED, RECORD).into());
    }
    Ok(())
}

/// The planned `InstanceParameters` (module documentation), or None for a None name.
fn plan(vm: &Vm, c: ObjRef, name: NameId, rgba: [u8; 4]) -> Result<Option<Value>, VmError> {
    if name == NameId::NONE || vm.space.names.text(name).eq_ignore_ascii_case("None") {
        return Ok(None);
    }
    resident(vm, c)?;
    pa::check(vm, c, &FIELDS, WHAT)?;
    let list = required(vm, c, "InstanceParameters", WHAT)?;
    let mut params = list.elements().map(<[Value]>::to_vec).ok_or_else(|| bad(WHAT, "InstanceParameters holds no array"))?;
    let mut at = None;
    for (i, p) in params.iter().enumerate() {
        let n = match field(vm, p, "Name")? {
            Value::Name(n) => n,
            other => return Err(bad(WHAT, format!("a parameter's Name holds {}, not a name", other.kind_name()))),
        };
        let ty = byte_of(vm, p, "ParamType")?;
        channels(vm, &field(vm, p, "Color")?)?;
        if at.is_none() && n == name && ty == PARAM_COLOR {
            at = Some(i);
        }
    }
    match at {
        Some(i) => {
            let color = color_like(vm, &field(vm, &params[i], "Color")?, rgba)?;
            params[i] = with_fields(vm, &params[i], &[("Color", color)])?;
        }
        None => {
            let class = vm.space.heap.get(c)?.class;
            let p = vm.space.prop(class, "InstanceParameters").ok_or_else(|| bad(WHAT, "no InstanceParameters"))?;
            let Some(PropKind::Array(inner)) = vm.space.classes.decl(p).map(|d| d.kind.clone()) else {
                return Err(bad(WHAT, "InstanceParameters is not an array"));
            };
            let zero = vm.space.classes.zero_value(&inner);
            let color = color_like(vm, &field(vm, &zero, "Color")?, rgba)?;
            params.push(with_fields(
                vm,
                &zero,
                &[("Name", Value::Name(name)), ("ParamType", Value::Byte(PARAM_COLOR)), ("Color", color)],
            )?);
        }
    }
    let value = Value::array(params);
    fits(vm, c, "InstanceParameters", &value, WHAT)?;
    Ok(Some(value))
}

/// `ParticleSystemComponent.SetColorParameter(name ParameterName, Color Param)` in Off (module documentation).
pub fn set_color_parameter(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let component = c.this.ok_or_else(|| bad(WHAT, "called without a component"))?;
    let name = match c.arg(0)? {
        Value::Name(n) => *n,
        other => return Err(bad(WHAT, format!("ParameterName holds {}, not a name", other.kind_name()))),
    };
    let color = c.arg(1)?.clone();
    let rgba = channels(c.vm, &color)?;
    if let Some(value) = plan(c.vm, component, name, rgba)? {
        write(c.vm, component, "InstanceParameters", value, WHAT)?;
    }
    Ok(Value::None)
}

#[cfg(test)]
#[path = "psc_color_tests.rs"]
mod tests;
