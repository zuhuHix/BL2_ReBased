//! Owner: lane FIX-AIHOLD (the behavior run, analyst) and lane FIX-AIHOLD-NATIVES (the hold kinds, implementer). The
//! AI hold behavior run through the VM, and the movement, demigod, god and dialog hold natives. AI-assisted (Claude
//! Code). Decisions 2026-11-16-74, 2026-11-16-83 and 2026-11-19-62.
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
//! **The pawn question (read, UNVERIFIED).** The installed class tables list `IAIInterface` for `GearboxMind` and
//! `WillowMind`, not for `WillowAIPawn` or its parents, yet in the running game the Fire dummy's own
//! `Behavior_AIHold_59` made the pawn demigod (COMBAT_GOLDEN_TRACE.md, question 8). NATIVE_PAWN_AI_INTERFACE.md reads
//! how a pawn answers: its class, then its `Controller` (or its driven vehicle's), then its `Mesh`. In Off that rule
//! is `ow_combat::pawn_interface` (decision 2026-11-19-62, registered by [`Pack`]): a pawn with nothing to ask runs the
//! script, which holds nothing; a step whose object's own answer is not read (the dummy's `WillowMind`, a
//! `GearboxMind`) is `NotYet` naming it, with no context run. Under Reference a pawn context stays `NotYet` as
//! before; in both modes a context that is not a pawn but has `Controller` or `MyWillowMind` set is `NotYet`.
//!
//! **The kinds.** The base and target holds are part 2's bindings ([`super::actions::Pack`], from G26). The movement,
//! demigod, god and dialog kinds are implemented from NATIVE_AI_HOLDS.md, and `GearboxMind.GetAIComponent` (the
//! mind's own `AIComponent`) from NATIVE_GEARBOXMIND_GETAICOMPONENT.md, in `ow_combat::holds` (clean room, decisions
//! 2026-11-16-83 and 2026-11-19-210), because
//! `ow-combat`'s pawns reach the demigod kind through their script and `ow-combat` sits below this crate. [`Pack`]
//! binds them in Off for a world without `ow_combat::hit::Pack` (which binds them too), and nothing under Reference.
//! A key another pack has already bound keeps that binding.

use ow_object::ids::ObjRef;
use ow_vm::api::{NativePack, Vm};
use ow_vm::error::VmError;

/// The analyst note of the hold kinds and of the pawn question.
pub const RECORD_HOLDS: &str = ow_combat::holds::RECORD_HOLDS;

/// Runs the hold behavior `behavior`'s installed script on each of `contexts` (module documentation). Every context is
/// judged before any script runs; a context whose AI is not reachable by a read rule is `NotYet` (the pawn question);
/// an action class without the script event is an error. The one implementation is
/// `ow_combat::holds::apply_hold_behavior`, which `--combat-run` uses too.
pub fn apply(vm: &mut Vm, behavior: ObjRef, contexts: &[ObjRef]) -> Result<(), VmError> {
    ow_combat::holds::apply_hold_behavior(vm, behavior, contexts)
}

/// Binds the four hold kinds' twelve natives and `GetAIComponent` (the mind's `AIComponent`) in Off
/// (`ow_combat::holds::register`);
/// nothing under Reference.
#[derive(Debug, Clone, Copy, Default)]
pub struct Pack;

impl NativePack for Pack {
    fn register(&self, vm: &mut Vm) -> Result<(), VmError> {
        ow_combat::holds::register(vm)
    }
}

#[cfg(test)]
mod tests;
