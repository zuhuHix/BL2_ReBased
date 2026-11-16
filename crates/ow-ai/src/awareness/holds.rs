//! Owner: lane FIX-AIHOLD. The AI hold behavior run through the VM, and the hold natives this lane read as analyst.
//! AI-assisted (Claude Code). Decision 2026-11-16-74.
//!
//! **The behavior is script.** `Behavior_AIHold.ApplyBehaviorToContext` is installed script (read with
//! `research/script_disasm.py`). It asks its context object for `IAIInterface` (`QueryInterface`); with no interface,
//! or with a `Reason` of None, it does nothing. Otherwise its `Action` picks a hold (0) or a release (any other value),
//! and its `Type` picks the kind: 0 the base hold (`Hold` / `Release`), 1 target, 2 movement, 3 demigod, 4 god,
//! 5 dialog. Each call goes to the interface's `GetAIComponent`, with the `Reason` as the hold name and the context as
//! the tag. [`apply`] runs that script on each context through the VM, passing the context as `ContextObject` (the
//! event declares five more parameters, which the script does not read; they keep their defaults, as
//! `ow-combat`'s class provider does).
//!
//! **The pawn question (not read).** The installed class tables list `IAIInterface` for `GearboxMind` and
//! `WillowMind`, not for `WillowAIPawn` or its parents, so `QueryInterface` on a pawn answers None and the script does
//! nothing. Yet in the running game the Fire dummy's own `Behavior_AIHold_59` made the pawn demigod
//! (COMBAT_GOLDEN_TRACE.md, question 8). So either that behavior's context was not the pawn, or a pawn answers the
//! interface in a way the tables do not show. Neither is read (NATIVE_AI_HOLDS.md, "Not read"). Until it is, a context
//! that has an AI to reach (its `Controller` or `MyWillowMind` set) is `NotYet`, rather than a silent no-op the game
//! contradicts; a context with neither runs the script, which can reach no AI component either way (the component is
//! the mind's).
//!
//! **The kinds.** The base and target holds are part 2's bindings ([`super::actions::Pack`], from G26). This lane read
//! the movement, demigod, god and dialog kinds in the executable as analyst (NATIVE_AI_HOLDS.md); under the clean-room
//! rule they are not implemented here: [`Pack`] binds them as `NotYet` in Off for a fresh implementer, and leaves them
//! unbound under Reference. A key another pack has already bound keeps that binding.

use std::rc::Rc;

use ow_combat::hit::access::object;
use ow_combat::hit::call_method;
use ow_core::{NotYet, ParityMode};
use ow_object::ids::ObjRef;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, NativePack, Vm};
use ow_vm::error::VmError;

/// The analyst note of the hold kinds and of the pawn question.
pub const RECORD_HOLDS: &str = "docs/verification/NATIVE_AI_HOLDS.md";

/// The hold natives this lane read as analyst (module documentation): `NotYet` in Off.
pub const READ_AS_ANALYST: [&str; 12] = [
    "GearboxFramework.AIComponent.HoldMovement",
    "GearboxFramework.AIComponent.ReleaseMovement",
    "GearboxFramework.AIComponent.MovementOnHold",
    "GearboxFramework.AIComponent.HoldDemigod",
    "GearboxFramework.AIComponent.ReleaseDemigod",
    "GearboxFramework.AIComponent.DemigodOnHold",
    "GearboxFramework.AIComponent.HoldGod",
    "GearboxFramework.AIComponent.ReleaseGod",
    "GearboxFramework.AIComponent.GodOnHold",
    "GearboxFramework.AIComponent.HoldDialog",
    "GearboxFramework.AIComponent.ReleaseDialog",
    "GearboxFramework.AIComponent.DialogOnHold",
];

fn error(message: String) -> VmError {
    VmError::Script { function: "ai holds".to_string(), message }
}

/// Runs the hold behavior `behavior`'s installed script on each of `contexts` (module documentation). A context with
/// an AI to reach is `NotYet` (the pawn question); an action class without the script event is an error.
pub fn apply(vm: &mut Vm, behavior: ObjRef, contexts: &[ObjRef]) -> Result<(), VmError> {
    for &context in contexts {
        for link in ["Controller", "MyWillowMind"] {
            if object(vm, context, link)?.is_some() {
                return Err(NotYet::not_read(
                    format!(
                        "Behavior_AIHold on {}, whose {link} is set: how a pawn context reaches its AI interface (the \
                         class tables list IAIInterface for the mind only, and the live hold reached the dummy's AI)",
                        ow_combat::hit::access::class_name(vm, context)
                    ),
                    RECORD_HOLDS,
                )
                .into());
            }
        }
        if call_method(vm, behavior, "ApplyBehaviorToContext", vec![Value::Object(Some(context))])?.is_none() {
            return Err(error(format!(
                "the hold behavior {} has no ApplyBehaviorToContext",
                ow_combat::hit::access::class_name(vm, behavior)
            )));
        }
    }
    Ok(())
}

/// Binds [`READ_AS_ANALYST`] as `NotYet` in Off (module documentation); nothing under Reference.
#[derive(Debug, Clone, Copy, Default)]
pub struct Pack;

impl NativePack for Pack {
    fn register(&self, vm: &mut Vm) -> Result<(), VmError> {
        if vm.parity != ParityMode::Off {
            return Ok(());
        }
        for path in READ_AS_ANALYST {
            let Ok(id) = vm.find_function(path) else {
                continue;
            };
            let Some(key) = vm.function(id).and_then(|d| d.native_key.clone()) else {
                continue;
            };
            if vm.has_native(&key) {
                continue;
            }
            vm.register_native(
                &key,
                Rc::new(move |_c: &mut NativeCall<'_>| -> Result<Value, VmError> {
                    Err(NotYet::not_ported(
                        format!("{path}: awaiting a clean-room implementation from the analyst note"),
                        RECORD_HOLDS,
                    )
                    .into())
                }),
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
