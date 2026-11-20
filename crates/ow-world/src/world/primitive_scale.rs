//! Owner: lane IMPL-SETSCALE. `PrimitiveComponent.SetScale(NewScale)` in `ParityMode::Off`, from
//! docs/verification/NATIVE_PRIMITIVE_SETSCALE.md section 3 (a stock engine native this lane read and implements, as
//! the hub's brief allows for a plain native). AI-assisted (Claude Code, Opus 5.5). **Every rule here is UNVERIFIED in
//! the running game.** Reference binds nothing, so the VM's stub policy answers there.
//!
//! The preflight writes, loads and draws nothing, and each refusal comes before any write:
//! 1. no receiver, or an argument that is not a float: `VmError::Script`;
//! 2. an unloaded receiver: `NotYet { NotRead }`;
//! 3. the declarations: the component's `Scale` (float), `bNeedsUpdateTransform` and `bAttached` (bools) and `Owner`
//!    (an object reference), and with an owner its `bStatic`, `bTickIsDisabled` and `bDeleteMe` (bools). Anything
//!    else is `VmError::Script`;
//! 4. `NewScale` equal to `Scale` (a float inequality: a NaN counts as a change): nothing at all;
//! 5. the deferred transform update's unread virtuals, `NotYet { NotRead }`:
//!    - [`OWN_UPDATE`]: no `Owner` and `bAttached`;
//!    - [`OWNER_UPDATE`]: an owner that is `bStatic` or `bTickIsDisabled`, and not `bDeleteMe`. The owner's two
//!      object-flag tests are not modelled, so the port may refuse where the game does nothing;
//!    - an owner the store has not loaded ([`UNLOADED_OWNER`]);
//! 6. the writes: `Scale` := `NewScale`, then `bNeedsUpdateTransform` := true.

use std::rc::Rc;

use ow_core::NotYet;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;

/// The native's registry key.
pub const SET_SCALE: &str = "PrimitiveComponent.SetScale(float)";
/// The record of the rule.
pub const RECORD: &str = "NATIVE_PRIMITIVE_SETSCALE.md";
/// The two unread virtuals of the deferred transform update, and an owner whose flags cannot be read.
pub const OWN_UPDATE: &str = "SetScale: the deferred transform update: the component's own transform update";
pub const OWNER_UPDATE: &str = "SetScale: the deferred transform update: the owner's component update";
pub const UNLOADED_OWNER: &str =
    "SetScale: the deferred transform update: an owner the store has not loaded (a load is a write)";
const UNIT: &str = "SetScale";

/// Binds the native (module documentation); the caller binds it in Off only.
pub(crate) fn register(vm: &mut Vm) -> Result<(), VmError> {
    vm.register_native(SET_SCALE, Rc::new(set_scale))
}

fn script(message: String) -> VmError {
    VmError::Script { function: UNIT.to_string(), message: format!("{UNIT}: {message}") }
}

fn unloaded(vm: &Vm, o: ObjRef) -> bool {
    vm.defs.is_unloaded(&vm.space, o) || vm.defs.is_failed(&vm.space, o)
}

/// The declared kind of `name` on `o`'s class, which must be `want` (by its variant).
fn declared(vm: &Vm, o: ObjRef, name: &str, want: fn(&PropKind) -> bool) -> Result<PropAddr, VmError> {
    let class = vm.space.heap.get(o)?.class;
    let path = || vm.space.classes.layout(class).path().to_string();
    let p = vm.space.prop(class, name).ok_or_else(|| script(format!("{} declares no {name}", path())))?;
    if !vm.space.classes.decl(p).is_some_and(|d| want(&d.kind)) {
        return Err(script(format!("{}.{name} is not declared as the native reads it", path())));
    }
    // The slot must be there, so the commit's write cannot fail.
    let a = PropAddr::scalar(p);
    vm.space.read(o, a)?;
    Ok(a)
}

fn is_float(k: &PropKind) -> bool {
    matches!(k, PropKind::Float)
}
fn is_bool(k: &PropKind) -> bool {
    matches!(k, PropKind::Bool)
}
fn is_object(k: &PropKind) -> bool {
    matches!(k, PropKind::Object { .. })
}

fn read_bool(vm: &Vm, o: ObjRef, a: PropAddr, name: &str) -> Result<bool, VmError> {
    match vm.space.read(o, a)? {
        Value::Bool(b) => Ok(*b),
        other => Err(script(format!("{name} holds {}, not a bool", other.kind_name()))),
    }
}

/// The preflight (module documentation, steps 2 to 5): the two writes to make, or none.
pub fn plan(vm: &Vm, component: ObjRef, new_scale: f32) -> Result<Option<[(PropAddr, Value); 2]>, VmError> {
    if unloaded(vm, component) {
        return Err(NotYet::not_read(format!("{UNIT}: the component is an unloaded handle (a load is a write)"), RECORD)
            .into());
    }
    let scale = declared(vm, component, "Scale", is_float)?;
    let needs = declared(vm, component, "bNeedsUpdateTransform", is_bool)?;
    let attached = declared(vm, component, "bAttached", is_bool)?;
    let owner_at = declared(vm, component, "Owner", is_object)?;
    let current = match vm.space.read(component, scale)? {
        Value::Float(f) => *f,
        other => return Err(script(format!("Scale holds {}, not a float", other.kind_name()))),
    };
    let owner = match vm.space.read(component, owner_at)? {
        Value::Object(o) => *o,
        other => return Err(script(format!("Owner holds {}, not an object reference", other.kind_name()))),
    };
    // The owner's declarations are checked whether or not the scale changes: the path with an owner reads them.
    let owner_flags = match owner {
        Some(o) if unloaded(vm, o) => None,
        Some(o) => Some((
            o,
            declared(vm, o, "bStatic", is_bool)?,
            declared(vm, o, "bTickIsDisabled", is_bool)?,
            declared(vm, o, "bDeleteMe", is_bool)?,
        )),
        None => None,
    };
    if new_scale == current {
        return Ok(None);
    }
    match (owner, owner_flags) {
        (None, _) => {
            if read_bool(vm, component, attached, "bAttached")? {
                return Err(NotYet::not_read(OWN_UPDATE, RECORD).into());
            }
        }
        (Some(_), None) => return Err(NotYet::not_read(UNLOADED_OWNER, RECORD).into()),
        (Some(_), Some((o, stat, tick, delete))) => {
            let fixed = read_bool(vm, o, stat, "bStatic")? || read_bool(vm, o, tick, "bTickIsDisabled")?;
            if fixed && !read_bool(vm, o, delete, "bDeleteMe")? {
                return Err(NotYet::not_read(OWNER_UPDATE, RECORD).into());
            }
        }
    }
    // Both slots were read above with their declared kinds, so neither write can fail after the other.
    Ok(Some([(scale, Value::Float(new_scale)), (needs, Value::Bool(true))]))
}

fn set_scale(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let component = c.this.ok_or_else(|| script("called with no component".to_string()))?;
    let new_scale = match c.arg(0)? {
        Value::Float(f) => *f,
        other => return Err(script(format!("NewScale holds {}, not a float", other.kind_name()))),
    };
    if let Some(writes) = plan(c.vm, component, new_scale)? {
        for (a, v) in writes {
            c.vm.space.write(component, a, v)?;
        }
    }
    Ok(Value::None)
}

#[cfg(test)]
#[path = "primitive_scale_tests.rs"]
mod tests;
