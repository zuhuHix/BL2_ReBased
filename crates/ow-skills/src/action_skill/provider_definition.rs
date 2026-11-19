//! Owner: lane IMPL-GET-BEHAVIOR-PROVIDER-DEFINITION. `ActionSkill.GetBehaviorProviderDefinition()` in
//! `ParityMode::Off`, coded from docs/verification/NATIVE_GET_BEHAVIOR_PROVIDER_DEFINITION.md alone (PR #553 at
//! ff49fd67; clean room: lane ANALYST-GET-BEHAVIOR-PROVIDER-DEFINITION read the native, this lane did not), with the
//! hub's ruling to take the note's proposed answer. AI-assisted (Claude Code). **UNVERIFIED in the running game.**
//! Reference binds nothing (its stub policy stands).
//!
//! **The rule** (note section 4): the answer is the receiver's own `BehaviorProviderDefinition` field, as stored, None
//! included. No archetype lookup, no class default, no load: a reference read is not a load, and the event that
//! receives the definition loads or refuses it when it reads it.
//! - **UNVERIFIED (low: a fitted step; the note's open question 1):** that the C++ interface method `ActionSkill`'s
//!   native reaches is the one that reads this field. The note ties it by a structural fit, not by name. The hub ruled
//!   to take it, labelled, rather than refuse. It agrees with the consumer registration (the skill registering with the
//!   archetype's definition) only if a spawned skill's field still names the archetype's subobject (open question 2).
//! - A receiver whose class does not declare `BehaviorProviderDefinition` as an object reference is the port's data
//!   error (`VmError::Script`, naming the field).
//! - A receiver the definition store has not loaded is `NotYet { NotRead }`, "GetBehaviorProviderDefinition: an
//!   unloaded receiver", unmarked: reading its field would need a load, and the rule loads nothing. So is a stand-in
//!   with no slots whose class has some (an import or resource stand-in the store does not report as unloaded). The
//!   field is read straight from the heap, never through a loading read.
//!
//! The native writes nothing, draws nothing and loads nothing, so there is no preflight and no partial-write mark.
//! It is bound only for `ActionSkill`. Its subclasses call the same declaration: in the installed packages neither
//! `LiftActionSkill` nor `ExecuteActionSkill` declares its own copy. The other 21 classes that declare their own native
//! stay unbound (note section 4, item 3).

use std::rc::Rc;

use ow_core::{NotYet, ParityMode};
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;

/// The function path the rule is bound under.
pub const GET_BEHAVIOR_PROVIDER_DEFINITION: &str = "WillowGame.ActionSkill.GetBehaviorProviderDefinition";
/// The record of the rule.
pub const RECORD: &str = "NATIVE_GET_BEHAVIOR_PROVIDER_DEFINITION.md";
const UNIT: &str = "ActionSkill.GetBehaviorProviderDefinition";
const FIELD: &str = "BehaviorProviderDefinition";

/// Binds the rule in Off, first-wins; nothing under Reference.
pub fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    ow_combat::hit::bind(
        vm,
        GET_BEHAVIOR_PROVIDER_DEFINITION,
        Rc::new(|c: &mut NativeCall<'_>| {
            let me = c.this.ok_or_else(|| script("called with no receiver".to_string()))?;
            provider_definition(c.vm, me).map(Value::Object)
        }),
    )
}

fn script(message: String) -> VmError {
    VmError::Script { function: UNIT.to_string(), message: format!("{UNIT}: {message}") }
}

/// `me`'s own `BehaviorProviderDefinition` field, as stored (module documentation). Reads only.
pub fn provider_definition(vm: &Vm, me: ObjRef) -> Result<Option<ObjRef>, VmError> {
    let class = vm.space.heap.get(me)?.class;
    let path = vm.space.classes.layout(class).path().to_string();
    let Some(p) = vm.space.prop(class, FIELD) else {
        return Err(script(format!("{path} declares no {FIELD}")));
    };
    if !matches!(vm.space.classes.decl(p).map(|d| &d.kind), Some(PropKind::Object { .. })) {
        return Err(script(format!("{path}.{FIELD} is not an object reference")));
    }
    // An unloaded handle, or a stand-in with no slots although its class has some (an import or resource stand-in;
    // the test `loaded_for_read` makes, review r1 of #560): reading the field would need a load.
    let stand_in = {
        let o = vm.space.heap.get(me)?;
        o.slots.is_empty() && vm.space.classes.get_layout(o.class).is_some_and(|l| l.slot_count() > 0)
    };
    if vm.defs.is_unloaded(&vm.space, me) || stand_in {
        return Err(NotYet::not_read("GetBehaviorProviderDefinition: an unloaded receiver", RECORD).into());
    }
    match vm.space.read(me, PropAddr::scalar(p))? {
        Value::Object(o) => Ok(*o),
        other => Err(script(format!("{path}.{FIELD} holds {}, not an object reference", other.kind_name()))),
    }
}

#[cfg(test)]
#[path = "provider_definition_tests.rs"]
mod tests;
