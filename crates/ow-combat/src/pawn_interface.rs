//! Owner: lane IMPL-PAWN-AI-INTERFACE. How a pawn answers `Object.QueryInterface` in Off: `Pawn`'s rule, and
//! `WillowAIPawn`'s one extra step. AI-assisted (Claude Code). Decision 2026-11-19-62.
//!
//! **Clean room.** Implemented from `docs/verification/NATIVE_PAWN_AI_INTERFACE.md` alone (the analyst was lane
//! ANALYST-PAWN-AI-INTERFACE, decision 2026-11-19-51), by a lane that did not read the natives. Every rule is
//! **UNVERIFIED** in the running game.
//!
//! **Pawn's rule** ([`pawn_answer`], the note's section 2). Asked for interface class `I`, a pawn gives the first answer
//! that is not none of:
//! 1. **its own class**: the pawn itself when its class or a parent lists `I` (the plain `Object` answer,
//!    [`Vm::plain_interface`]);
//! 2. **one controller**: its `Controller` when set, whatever that controller answers (a declining controller does not
//!    lead to the vehicle); only when `Controller` is not set, the `Controller` of its `DrivenVehicle`, when both are
//!    set;
//! 3. **its `Mesh`**, when set;
//! 4. otherwise none.
//!
//! The three fields are read by name and must be declared (a class without one is an error, never "not set"). A
//! question that comes back to an object already answering it gets none ([`Vm::query_interface`]).
//!
//! **What stays `NotYet` in Off** (the note's "Open" list, never a guess). Steps 2 and 3 hand the question to another
//! object, which "answers by its own rule": [`Vm::query_interface`], the same dispatch as a direct `QueryInterface`
//! on that object (review r1 of #456, item 3). The answers the note leaves open are bound in Off as rules that are
//! `NotYet { NotRead }`, so a direct question and a handed-on one meet the same refusal:
//! - `GearboxMind`'s own answer, for every interface (whether it adds steps before or after its class's declared
//!   interfaces); a `WillowMind`, the Fire dummy's mind, takes it;
//! - `WillowPlayerController`'s answer for `IAIInterface` (other interfaces keep the plain answer: the note's open item
//!   is that one question);
//! - a skeletal mesh component's answer, for every interface.
//!
//! A handed-on `NotYet` names the step ("goes to its Controller, a ...") ahead of the open answer. Any other class
//! answers by its registered rule or the plain one, as a direct question does. Nothing is written by any step: a
//! `NotYet` here leaves the world as it was.
//!
//! **`WillowAIPawn`** ([`willow_ai_pawn_answer`], section 3): `Pawn`'s rule, then, only when that gives none and the
//! asked class is `IPlayerMaster`, its player master. That field's identification is open (UNVERIFIED), so the step
//! is `NotYet`. For `IAIInterface` a `WillowAIPawn` answers exactly as `Pawn` does.
//!
//! **`WillowVehicle`** answers its own way (the note's "Vehicles", recorded for completeness and not on the slice path):
//! not implemented, so a vehicle's question is `NotYet` rather than `Pawn`'s rule.
//!
//! **Reference** (R3) registers none of this: `crate::hit::ai_interface_for` keeps the C++ stand-in there.

use std::rc::Rc;

use ow_core::{NotYet, ParityMode};
use ow_object::ids::{ClassId, ObjRef};
use ow_vm::api::{InterfaceRuleFn, Vm};
use ow_vm::error::VmError;

use crate::hit::access::{object_of, required};

/// The analyst note this module implements.
pub const RECORD: &str = "docs/verification/NATIVE_PAWN_AI_INTERFACE.md";

/// The classes whose answers are registered.
pub const PAWN: &str = "Engine.Pawn";
pub const WILLOW_AI_PAWN: &str = "WillowGame.WillowAIPawn";
pub const WILLOW_VEHICLE: &str = "WillowGame.WillowVehicle";

/// The interface `Behavior_AIHold` asks for, and the one `WillowAIPawn` adds a step for.
pub const AI_INTERFACE: &str = "GearboxFramework.IAIInterface";
pub const PLAYER_MASTER_INTERFACE: &str = "WillowGame.IPlayerMaster";

/// The classes whose own answer the note leaves open, with how it names them, and whether the open item is the
/// `IAIInterface` question only (module notes).
pub const OPEN_ANSWERS: [(&str, &str, bool); 3] = [
    (
        "GearboxFramework.GearboxMind",
        "GearboxMind's own QueryInterface answer (whether it adds steps before or after its class's declared \
         interfaces)",
        false,
    ),
    ("WillowGame.WillowPlayerController", "WillowPlayerController's QueryInterface answer for IAIInterface", true),
    ("Engine.SkeletalMeshComponent", "what a skeletal mesh component answers to QueryInterface", false),
];

/// Registers `Pawn`'s, `WillowAIPawn`'s and `WillowVehicle`'s answers, and the open answers as `NotYet` rules, in
/// Off, for each class the installed packages hold; nothing under Reference. A class that already has an answer keeps
/// it. A class lookup that fails for another reason than the class being absent is returned as the error, so a class
/// that is present but fails to load never quietly takes a parent's rule (review r1 of #456, item 4).
pub fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    let mut rules: Vec<(&str, InterfaceRuleFn)> = vec![
        (PAWN, Rc::new(pawn_answer)),
        (WILLOW_AI_PAWN, Rc::new(willow_ai_pawn_answer)),
        (WILLOW_VEHICLE, Rc::new(vehicle_answer)),
    ];
    for (path, words, ai_only) in OPEN_ANSWERS {
        rules.push((path, open_answer(words, ai_only)));
    }
    for (path, rule) in rules {
        let Some(class) = vm.find_class_if_present(path)? else {
            continue;
        };
        if !vm.has_interface_rule(class) {
            vm.register_interface_rule(class, rule)?;
        }
    }
    Ok(())
}

/// An open answer's rule: `NotYet { NotRead }` naming it (for `IAIInterface` only when `ai_only`; the plain answer
/// for the other interfaces then).
fn open_answer(words: &'static str, ai_only: bool) -> InterfaceRuleFn {
    Rc::new(move |vm: &mut Vm, this: ObjRef, iface: ClassId| {
        if ai_only && !is_interface(vm, iface, AI_INTERFACE)? {
            return vm.plain_interface(this, iface);
        }
        Err(NotYet::not_read(
            format!(
                "{words} is not read (asked of {} for {})",
                class_path(vm, vm.space.heap.get(this)?.class),
                class_path(vm, iface)
            ),
            RECORD,
        )
        .into())
    })
}

/// Whether `iface` is exactly the class at `path` (an absent class is never it; another lookup failure is the error).
fn is_interface(vm: &mut Vm, iface: ClassId, path: &str) -> Result<bool, VmError> {
    Ok(vm.find_class_if_present(path)? == Some(iface))
}

/// The class path of `class`.
fn class_path(vm: &Vm, class: ClassId) -> String {
    vm.space.classes.layout(class).path().to_string()
}

/// Object property `name` of `o`, which its class must declare.
fn field(vm: &mut Vm, o: ObjRef, name: &str) -> Result<Option<ObjRef>, VmError> {
    Ok(object_of(&required(vm, o, name, "the pawn QueryInterface rule")?))
}

/// Steps 2 and 3: `step`'s object `o` answers `iface` by its own rule, through the one dispatch
/// ([`Vm::query_interface`]); its `NotYet` is prefixed with the step (module notes).
fn ask(vm: &mut Vm, o: ObjRef, iface: ClassId, step: &str) -> Result<Option<ObjRef>, VmError> {
    match vm.query_interface(o, iface) {
        Err(VmError::NotYet(n)) => {
            let class = class_path(vm, vm.space.heap.get(o)?.class);
            let text = format!(
                "a pawn's QueryInterface({}) goes to {step}, a {class}: {}",
                class_path(vm, iface),
                n.what
            );
            Err(NotYet::new(text, n.missing, n.record).into())
        }
        other => other,
    }
}

/// `Pawn`'s answer (module notes, steps 1 to 4).
pub fn pawn_answer(vm: &mut Vm, pawn: ObjRef, iface: ClassId) -> Result<Option<ObjRef>, VmError> {
    if let Some(me) = vm.plain_interface(pawn, iface)? {
        return Ok(Some(me));
    }
    if let Some(controller) = field(vm, pawn, "Controller")? {
        if let Some(answer) = ask(vm, controller, iface, "its Controller")? {
            return Ok(Some(answer));
        }
    } else if let Some(vehicle) = field(vm, pawn, "DrivenVehicle")?
        && let Some(controller) = field(vm, vehicle, "Controller")?
        && let Some(answer) = ask(vm, controller, iface, "its driven vehicle's Controller")?
    {
        return Ok(Some(answer));
    }
    if let Some(mesh) = field(vm, pawn, "Mesh")?
        && let Some(answer) = ask(vm, mesh, iface, "its Mesh")?
    {
        return Ok(Some(answer));
    }
    Ok(None)
}

/// `WillowAIPawn`'s answer: `Pawn`'s, then the player master step for exactly `IPlayerMaster` (`NotYet`: the field is
/// not identified).
pub fn willow_ai_pawn_answer(vm: &mut Vm, pawn: ObjRef, iface: ClassId) -> Result<Option<ObjRef>, VmError> {
    if let Some(answer) = pawn_answer(vm, pawn, iface)? {
        return Ok(Some(answer));
    }
    if is_interface(vm, iface, PLAYER_MASTER_INTERFACE)? {
        return Err(NotYet::not_read(
            "WillowAIPawn's QueryInterface(IPlayerMaster) after Pawn's rule declines: it asks the pawn's player \
             master, a field the layout does not place exactly (PlayerMaster, identified by meaning only)",
            RECORD,
        )
        .into());
    }
    Ok(None)
}

/// `WillowVehicle`'s answer: its own rule is recorded for completeness only and not implemented.
fn vehicle_answer(vm: &mut Vm, vehicle: ObjRef, iface: ClassId) -> Result<Option<ObjRef>, VmError> {
    Err(NotYet::not_read(
        format!(
            "WillowVehicle's own QueryInterface({}) rule (its controller unless a human player's, IAnimBehavior from \
             its Mesh), recorded for completeness and not implemented, asked of {}",
            class_path(vm, iface),
            crate::hit::access::class_name(vm, vehicle)
        ),
        RECORD,
    )
    .into())
}

#[cfg(test)]
mod tests;
