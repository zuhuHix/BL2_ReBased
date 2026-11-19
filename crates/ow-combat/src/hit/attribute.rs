//! `AttributeDefinition.GetValue` as a script native (lane FIX-ATTR-GETVALUE-NATIVE, decision 2026-11-19-58).
//! AI-assisted (Claude Code). The rule is `ow-attr`'s existing library rule, [`ow_attr::AttributeSystem::value_of`]
//! (NATIVE_ATTRIBUTES.md, "AttributeDefinition.GetValue"), through its shared steps (`preflight_read`, `value_context`,
//! `value_from_context`); this module only adapts the script call and writes no second copy of the rule. UNVERIFIED in the running game.
//!
//! `ParityMode::Off` only. The C++ reference has no script native for it (its attribute reads are library calls), so
//! Reference binds nothing and the call stays the unbound native it was.

use ow_attr::api::EvalCx;
use ow_core::{NotYet, ParityMode};
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

/// The note's rule (NATIVE_ATTRIBUTES.md, "AttributeDefinition.GetValue"), in its order, on `ow-attr`'s shared steps:
/// 1. the context: the override source first when one is given, then `ContextSource`, through the definition's context
///    chain ([`ow_attr::AttributeSystem::value_context`]);
/// 2. the context written to the caller's `ResolvedContext` (None when nothing resolved), when the caller passed it, so
///    script the value chain reaches sees the new object;
/// 3. with a context, the value chain's result ([`ow_attr::AttributeSystem::value_from_context`]); with none, 0.0.
///
/// **Refusals (the hub's ruling in review r1 of #442, the same rule as #437 and #439).** Everything that can refuse is
/// decided before anything has an effect:
/// - the arguments: `ContextSource`, a supplied `ResolvedContext` and a supplied `OptionalOverrideContextSource` must
///   each be an object reference;
/// - the definition's own chains, through [`ow_attr::AttributeSystem::preflight_read`]: not multi-context, every
///   resolver class one the library implements, no None value chain entry.
///
/// Those refuse with nothing changed: no generator drawn, no saved random value written, no out argument written.
/// Once step 1 starts, a refusal can come from what the steps reach (a nested attribute, a resolver's own data, script
/// a context resolver runs). It may follow effects (a random draw, the out write), and the port does not track
/// whether one happened. So a `NotYet` there is **terminal**: nothing is put back, its text starts "stopped after
/// partial writes", and its kind is kept. Any other error is returned unchanged (review r2 of #442).
fn get_value(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let attr = this(c, UNIT)?;
    let source = object_arg(c, 0, "ContextSource")?;
    let out = c.arg_count() > 1 && c.has(1);
    if out {
        object_arg(c, 1, "ResolvedContext")?;
    }
    let override_source = if c.arg_count() > 2 && c.has(2) {
        object_arg(c, 2, "OptionalOverrideContextSource")?
    } else {
        None
    };
    let attrs = c.vm.attrs.clone();
    attrs.preflight_read(attr, &mut EvalCx::new(c.vm))?;
    let committed = (|| -> Result<f32, VmError> {
        let context = attrs.value_context(attr, source, override_source, &mut EvalCx::new(c.vm))?;
        if out {
            c.set_out(1, Value::Object(context))?;
        }
        Ok(attrs.value_from_context(attr, context, &mut EvalCx::new(c.vm))?)
    })();
    committed.map(Value::Float).map_err(after_partial_writes)
}

/// A refusal after evaluation started (the hub's ruling in review r2 of #442): a `NotYet` is terminal, its text
/// prefixed "stopped after partial writes" at the very start, its kind kept. Any other error already stops the run and
/// is returned unchanged, kind and text.
fn after_partial_writes(e: VmError) -> VmError {
    match e {
        // The attribute library may have marked it already (its fold, review r1 of #458): the prefix is added once.
        VmError::NotYet(n) if n.what.starts_with("stopped after partial writes") => VmError::NotYet(n),
        VmError::NotYet(n) => NotYet::new(format!("stopped after partial writes: {}", n.what), n.missing, n.record).into(),
        other => other,
    }
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
