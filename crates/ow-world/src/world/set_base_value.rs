//! Owner: lane IMPL-SET-BASE-VALUE. AI-assisted (Claude Code). `AttributeInitializationDefinition.SetBaseValue
//! (DestAttribute, out BaseValue, ValueContextSource, AttributeContextSource)`, implemented from NATIVE_ATTRIBUTES.md
//! section 1 ("AttributeInitializationDefinition.EvaluateInitializationData and SetBaseValue") alone (this lane's
//! decision 2026-11-19-187). Every rule is the record's reading, **UNVERIFIED** in the running game.
//!
//! **What it does** (the record): it evaluates `BaseValue` with `ValueContextSource` (no override source), then calls
//! `DestAttribute.SetAttributeBaseValue(AttributeContextSource, result)` and returns its bool; with `DestAttribute`
//! None it returns false. The two halves are `ow-attr`'s `evaluate_init` and `set_base`. The record names no
//! `bSuppressNotify`, so the call takes the script default (false: the change notification runs). That reading is
//! UNVERIFIED. The `out` struct is left as it was passed: the record names no write to it (UNVERIFIED).
//!
//! **The preflight** (write-free, loads nothing; a lazy load is a write under #518):
//! - `DestAttribute`, `BaseValue.BaseValueAttribute` or `BaseValue.InitializationDefinition` an unloaded definition
//!   handle: `NotYet { NotRead }` [`UNLOADED`], with nothing loaded;
//! - `BaseValue` not an `AttributeInitializationData` struct: the evaluator's typed data error.
//!
//! Deeper objects the evaluation or the set reaches (a resolver's definitions, an attribute's chain) are loaded as
//! `ow-attr` loads them today. So the native snapshots #518's three counters (`Heap::committed_writes`,
//! `GameRng::draws`, `DefStore::load_transitions`) on entry. A `NotYet` leaving it after any counter moved carries the
//! "stopped after partial writes: " mark, once, with its kind and record kept. Every other error passes through
//! unchanged.
//!
//! **Only in `ParityMode::Off`.** The C++ reference implements `EvaluateInitializationData` but not `SetBaseValue`, so
//! under Reference it stays unbound and the stub policy answers it (architecture R3).

use std::rc::Rc;

use ow_attr::api::EvalCx;
use ow_core::{NotYet, ParityMode};
use ow_object::value::Value;
use ow_vm::api::{NativeCall, NativeFn, Vm};
use ow_vm::error::VmError;

use crate::actors::arg_object;

/// The native key [`register`] binds in Off.
pub const SET_BASE_VALUE: &str =
    "AttributeInitializationDefinition.SetBaseValue(object,attributeinitializationdata,object,object)";

/// The record.
pub const RECORD: &str = "NATIVE_ATTRIBUTES.md section 1 (EvaluateInitializationData and SetBaseValue)";

/// The refusal for an unloaded handle the call names (a load is a write).
pub const UNLOADED: &str = "AttributeInitializationDefinition.SetBaseValue: a definition it names is not loaded (a load is a write)";

/// #518's three counters, taken on entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Counters {
    heap: u64,
    draws: u64,
    loads: u64,
    /// The VM's code loads: a load is a write (lane RUNTIME-MARK-CODE-LOADS).
    code_loads: u64,
}

fn counters(vm: &Vm) -> Counters {
    Counters { heap: vm.space.heap.committed_writes(), draws: vm.rng.draws(), loads: vm.defs.load_transitions(), code_loads: vm.class_code_loads() }
}

/// The native's body: preflight, evaluation, set.
fn set_base_value(c: &mut NativeCall<'_>) -> Result<bool, VmError> {
    let dest = arg_object(c, SET_BASE_VALUE, 0)?;
    let base = c.arg(1)?.clone();
    let value_source = arg_object(c, SET_BASE_VALUE, 2)?;
    let attribute_source = arg_object(c, SET_BASE_VALUE, 3)?;
    let Some(dest) = dest else {
        return Ok(false);
    };
    // The preflight: read-only, nothing loaded.
    let data = ow_attr::eval::init_data(&c.vm.space, &base)?;
    for o in [Some(dest), data.base_value_attribute, data.initialization_definition].into_iter().flatten() {
        if c.vm.defs.is_unloaded(&c.vm.space, o) {
            return Err(NotYet::not_read(UNLOADED, RECORD).into());
        }
    }
    let attrs = c.vm.attrs.clone();
    // The rest of the preflight (review r1 of #572): every definition the call reaches, loading nothing, so a refusal
    // known before any write (a multi-context destination, a lazy or unimplemented resolver) comes before the
    // evaluation can draw or load.
    attrs.preflight_set_base_value(dest, &data, &mut EvalCx::new(c.vm))?;
    let start = counters(c.vm);
    let result = (|| -> Result<bool, VmError> {
        let mut cx = EvalCx::new(c.vm);
        let x = attrs.evaluate_init(&data, value_source, None, &mut cx)?;
        Ok(attrs.set_base(dest, attribute_source, x, false, &mut cx)?)
    })();
    match result {
        Err(VmError::NotYet(n)) if counters(c.vm) != start => Err(VmError::NotYet(n.after_partial_writes())),
        other => other,
    }
}

/// Binds the native in `ParityMode::Off`; Reference binds nothing (module documentation).
pub fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    let f: NativeFn = Rc::new(|c: &mut NativeCall<'_>| Ok(Value::Bool(set_base_value(c)?)));
    vm.register_native(SET_BASE_VALUE, f)
}

#[cfg(test)]
#[path = "set_base_value_tests.rs"]
mod tests;
