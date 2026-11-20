//! Owner: lane IMPL-GET-PARTICLE-EFFECT. AI-assisted (Claude Code). `EffectCollectionDefinition.GetParticleEffect(object)`,
//! implemented from NATIVE_GET_PARTICLE_EFFECT.md alone (PR #562 at 865ba911, accepted; this lane's decision
//! 2026-11-19-188). Every rule is the note's reading, **UNVERIFIED** in the running game.
//!
//! **What it does** (the note's section 2): it walks the collection's `ParticleEffects`, an array of
//! `ConditionalParticleEffectData { Expression, ParticleEffect }`, in array order, and returns the `ParticleEffect` of
//! the first entry that passes. An entry with no `Expression` passes at once; otherwise the entry's evaluator is asked
//! through the VM (`Evaluate`, the evaluator class's own binding, native or script) with the argument as its context,
//! and the entry passes when it answers true. No entry passing, or an empty array, gives None. The default entry is
//! never returned without evaluating the entries before it. The returned `ParticleEffect` handle is given back as
//! stored, loaded or not; the native never loads it.
//!
//! **The preflight** (section 4; write-free, loads nothing, since under #518 a lazy load is a write):
//! 1. the collection is an unloaded handle: `NotYet { NotRead }`, [`COLLECTION_NOT_LOADED`];
//! 2. `ParticleEffects` is declared as a dynamic array of a struct whose `Expression` and `ParticleEffect` are object
//!    references, else a `VmError::Script` naming the field;
//! 3. the entries are checked in array order up to and including the first with no `Expression` (later ones can never
//!    be reached): an `Expression` there that is an unloaded handle is `NotYet { NotRead }`, [`EVALUATOR_NOT_LOADED`].
//!    That is a **port convention, not game behaviour**: the game evaluates on demand and stops at the first entry
//!    that passes, so this check is stricter than the game's. It keeps the preflight free of evaluator calls.
//!
//! 4. for each reachable `AttributeExpressionEvaluator`, its operands are checked resident and their resolver dispatch
//!    known ([`check_operands`]; review r1 of #573, item 2, a port convention too).
//!
//! A handle whose load failed (the collection, or an evaluator in that range) is a `VmError::Script`: the note does
//! not cover it, and reading it as an empty object would hide the failure.
//!
//! **The error rule** (section 4): the three #518 counters are read on entry, before the preflight. A `NotYet` from an
//! evaluator, when any counter has moved since then, takes the "stopped after partial writes: " mark once, its kind and
//! record kept; when none has moved it passes through unchanged. Every other error passes through unchanged, an unbound
//! evaluator included. An `Evaluate` that answers anything but a bool is a `VmError::Script`. The native itself writes
//! nothing, so any movement comes from the evaluators; the preflight's refusals come before any evaluator runs.
//!
//! **Only in `ParityMode::Off`.** The C++ reference has no stand-in, so under Reference the native stays unbound and
//! the stub policy answers it (architecture R3).

use std::rc::Rc;

use ow_core::{NotYet, ParityMode};
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, NativeFn, Vm};
use ow_vm::error::VmError;

use super::bad;

/// The record this native follows.
pub const RECORD: &str = "NATIVE_GET_PARTICLE_EFFECT.md";

/// The native key [`register`] binds in Off.
pub const GET_PARTICLE_EFFECT: &str = "EffectCollectionDefinition.GetParticleEffect(object)";

/// Preflight rule 1's refusal text.
pub const COLLECTION_NOT_LOADED: &str = "GetParticleEffect: the collection is not loaded (a load is a write)";

/// Preflight rule 3's refusal text.
pub const EVALUATOR_NOT_LOADED: &str = "GetParticleEffect: an entry's evaluator is not loaded";

/// The collection's one field.
const EFFECTS: &str = "ParticleEffects";

/// The #518 counters: committed writes, random draws, load transitions.
fn counters(vm: &Vm) -> (u64, u64, u64) {
    (vm.space.heap.committed_writes(), vm.rng.draws(), vm.defs.load_transitions())
}

/// One entry as the walk reads it: `(Expression, ParticleEffect)`.
type Entry = (Option<ObjRef>, Option<ObjRef>);

/// The preflight (module documentation): every entry the walk can reach, in array order. Reads only; nothing is
/// written or loaded.
fn plan(vm: &Vm, collection: ObjRef) -> Result<Vec<Entry>, VmError> {
    let what = GET_PARTICLE_EFFECT;
    if vm.defs.is_failed(&vm.space, collection) {
        return Err(bad(what, "the collection's load failed"));
    }
    if vm.defs.is_unloaded(&vm.space, collection) {
        return Err(NotYet::not_read(COLLECTION_NOT_LOADED, RECORD).into());
    }
    let class = vm.space.heap.get(collection)?.class;
    let classes = &vm.space.classes;
    let p = vm.space.prop(class, EFFECTS).ok_or_else(|| bad(what, format!("the collection declares no {EFFECTS}")))?;
    let ty = match classes.decl(p).map(|d| (&d.kind, d.array_dim)) {
        Some((PropKind::Array(inner), 1)) => match **inner {
            PropKind::Struct(ty) => ty,
            ref k => return Err(bad(what, format!("{EFFECTS} holds {k:?}, not a struct"))),
        },
        other => return Err(bad(what, format!("{EFFECTS} is declared as {other:?}, not a dynamic array"))),
    };
    let layout = classes.struct_layout(ty).ok_or_else(|| bad(what, format!("{EFFECTS}' struct has no layout")))?;
    let index = |name: &str| -> Result<usize, VmError> {
        let decl = vm.space.names.find(name).and_then(|id| layout.field(id));
        match decl {
            Some((i, d)) if matches!(d.kind, PropKind::Object { .. }) && d.array_dim == 1 => Ok(i as usize),
            Some((_, d)) => Err(bad(what, format!("{EFFECTS}.{name} is declared as {:?}, not an object", d.kind))),
            None => Err(bad(what, format!("{EFFECTS}' struct declares no {name}"))),
        }
    };
    let (expression, effect) = (index("Expression")?, index("ParticleEffect")?);
    let value = vm.space.read(collection, PropAddr::scalar(p))?;
    let entries = value.elements().ok_or_else(|| bad(what, format!("{EFFECTS} holds {}", value.kind_name())))?;
    let mut reachable = Vec::new();
    for entry in entries {
        let s = entry.as_struct().ok_or_else(|| bad(what, format!("an entry holds {}", entry.kind_name())))?;
        let object = |i: usize, name: &str| match s.fields.get(i) {
            Some(Value::Object(o)) => Ok(*o),
            Some(v) => Err(bad(what, format!("an entry's {name} holds {}", v.kind_name()))),
            None => Err(bad(what, format!("an entry has no {name}"))),
        };
        let (e, fx) = (object(expression, "Expression")?, object(effect, "ParticleEffect")?);
        if let Some(e) = e {
            if vm.defs.is_failed(&vm.space, e) {
                return Err(bad(what, "an entry's evaluator failed to load"));
            }
            if vm.defs.is_unloaded(&vm.space, e) {
                return Err(NotYet::not_read(EVALUATOR_NOT_LOADED, RECORD).into());
            }
        }
        reachable.push((e, fx));
        if e.is_none() {
            break;
        }
    }
    Ok(reachable)
}

/// The attribute evaluator class whose operands the preflight checks (review r1 of #573, item 2).
pub const ATTRIBUTE_EVALUATOR: &str = "GearboxFramework.AttributeExpressionEvaluator";

/// Preflight rule 4 (review r1 of #573, item 2; a port convention, not game behaviour): for each reachable evaluator
/// that is an [`ATTRIBUTE_EVALUATOR`], each set operand of its `Expression` goes through
/// [`ow_attr::AttributeSystem::preflight_read_resident`]. A cold operand or resolver is `NotYet { NotRead }` before any
/// load, and a value resolver class nothing implements is refused by its name before any evaluation. Loads and writes
/// nothing. A refusal only an evaluation can find stays the walk's, under the partial-writes rule.
fn check_operands(vm: &mut Vm, entries: &[Entry]) -> Result<(), VmError> {
    let Some(class) = vm.space.classes.by_path(ATTRIBUTE_EVALUATOR) else {
        return Ok(());
    };
    for &(e, _) in entries {
        let Some(e) = e else { continue };
        if !vm.space.classes.is_child_of(vm.space.heap.get(e)?.class, class) {
            continue;
        }
        let expression = crate::equip::read_prop(vm, e, "Expression")
            .ok_or_else(|| bad(GET_PARTICLE_EFFECT, "an attribute evaluator has no Expression"))?;
        let mut operands = Vec::new();
        for name in ["AttributeOperand1", "AttributeOperand2"] {
            match ow_attr::eval::field(&vm.space, &expression, name)? {
                Value::Object(Some(o)) => operands.push(*o),
                Value::Object(None) => {}
                other => return Err(bad(GET_PARTICLE_EFFECT, format!("{name} holds {}", other.kind_name()))),
            }
        }
        let sys = vm.attrs.clone();
        for o in operands {
            sys.preflight_read_resident(o, &mut ow_attr::api::EvalCx::new(&mut *vm))?;
        }
    }
    Ok(())
}

/// One entry's `Evaluate(context)` through the VM (the evaluator class's own binding).
fn evaluate(vm: &mut Vm, evaluator: ObjRef, context: Option<ObjRef>) -> Result<bool, VmError> {
    let (class, state) = {
        let o = vm.space.heap.get(evaluator)?;
        (o.class, o.state)
    };
    let f = vm
        .find_method(class, "Evaluate", state)?
        .ok_or_else(|| bad(GET_PARTICLE_EFFECT, "an entry's evaluator has no Evaluate"))?;
    match vm.call(f, Some(evaluator), vec![Value::Object(context)])?.ret {
        Value::Bool(b) => Ok(b),
        other => Err(bad(GET_PARTICLE_EFFECT, format!("an evaluator's Evaluate returned {}", other.kind_name()))),
    }
}

/// The native's body (module documentation).
pub fn get_particle_effect(vm: &mut Vm, collection: ObjRef, context: Option<ObjRef>) -> Result<Option<ObjRef>, VmError> {
    let start = counters(vm);
    let entries = plan(vm, collection)?;
    check_operands(vm, &entries)?;
    for (expression, effect) in entries {
        let Some(evaluator) = expression else {
            return Ok(effect);
        };
        match evaluate(vm, evaluator, context) {
            Ok(true) => return Ok(effect),
            Ok(false) => {}
            Err(VmError::NotYet(n)) if counters(vm) != start => return Err(n.after_partial_writes().into()),
            Err(e) => return Err(e),
        }
    }
    Ok(None)
}

/// Binds the native in `ParityMode::Off`; Reference binds nothing (module documentation).
pub fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    let f: NativeFn = Rc::new(|c: &mut NativeCall<'_>| {
        let collection = c.this.ok_or_else(|| bad(GET_PARTICLE_EFFECT, "called without a collection"))?;
        let context = match c.arg(0)? {
            Value::Object(o) => *o,
            other => return Err(bad(GET_PARTICLE_EFFECT, format!("the argument holds {}", other.kind_name()))),
        };
        Ok(Value::Object(get_particle_effect(c.vm, collection, context)?))
    });
    vm.register_native(GET_PARTICLE_EFFECT, f)
}

#[cfg(test)]
#[path = "particle_effect_tests.rs"]
mod tests;
