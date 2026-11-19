//! `AttributeDefinition.GetValue` as a script native (lane FIX-ATTR-GETVALUE-NATIVE, decision 2026-11-19-58).
//! AI-assisted (Claude Code). The rule is `ow-attr`'s existing library rule, [`ow_attr::AttributeSystem::value_of`]
//! (NATIVE_ATTRIBUTES.md, "AttributeDefinition.GetValue"); this module only adapts the script call to it and writes
//! no second copy of the rule. UNVERIFIED in the running game.
//!
//! `ParityMode::Off` only. The C++ reference has no script native for it (its attribute reads are library calls), so
//! Reference binds nothing and the call stays the unbound native it was.

use ow_attr::api::EvalCx;
use ow_core::ParityMode;
use ow_object::ids::ObjRef;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;

use super::{bind_fn, this};

/// The script function, `native final function float GetValue(Object ContextSource, optional out Object
/// ResolvedContext, optional Object OptionalOverrideContextSource)`.
pub const GET_VALUE: &str = "Engine.AttributeDefinition.GetValue";

const UNIT: &str = "AttributeDefinition.GetValue";

pub(super) fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    bind_fn(vm, GET_VALUE, get_value)
}

/// The note's rule, all of it in [`ow_attr::AttributeSystem::value_of`]: the override source is tried first when one
/// is given, then `ContextSource`, through the definition's context chain. With a context, the value chain's result
/// is returned and the context is written to `ResolvedContext`. With none, 0.0 and a None context. Anything the
/// library leaves unread (a resolver class it does not implement, a multi-context definition) comes back as its own
/// `NotYet`.
///
/// An argument that is not an object reference is a script error, never read as None. `ResolvedContext` is written
/// only when the caller passed it.
fn get_value(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let attr = this(c, UNIT)?;
    let source = object_arg(c, 0, "ContextSource")?;
    let override_source = if c.arg_count() > 2 && c.has(2) {
        object_arg(c, 2, "OptionalOverrideContextSource")?
    } else {
        None
    };
    let attrs = c.vm.attrs.clone();
    let read = attrs.value_of(attr, source, override_source, &mut EvalCx::new(c.vm))?;
    let (context, value) = match read {
        Some(v) => (Some(v.context), v.value),
        None => (None, 0.0),
    };
    if c.arg_count() > 1 && c.has(1) {
        c.set_out(1, Value::Object(context))?;
    }
    Ok(Value::Float(value))
}

fn object_arg(c: &NativeCall<'_>, i: usize, name: &str) -> Result<Option<ObjRef>, VmError> {
    match c.arg(i)? {
        Value::Object(o) => Ok(*o),
        other => Err(super::access::error(format!(
            "{UNIT}: {name} holds {}, not an object reference",
            other.kind_name()
        ))),
    }
}

#[cfg(test)]
mod tests;
