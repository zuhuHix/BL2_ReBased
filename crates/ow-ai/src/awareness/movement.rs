//! Owner: lane W7-AI (part 3). The action-sequence latent waits, the action-sequence state step, the AI component's
//! target-record queries and the mind queries the note reads, coded from `docs/verification/NATIVE_AI_MOVEMENT.md`
//! alone by a lane that did not read the executable (clean room). AI-assisted (Claude Code). Decision `2026-11-19-16`.
//! Every rule is the note's reading, **UNVERIFIED** in the running game.
//!
//! **`ParityMode::Off` only.** There is no C++ AI to stand in for under Reference: there these natives keep the
//! bindings `actions` gives them (the latent moves `NotYet`, the rest unbound, so the stub policy refuses them).
//!
//! **The waits** run on the action object through the VM's host latent seam (`ow_vm::latent`, decision 2026-11-19-07):
//! the native starts the wait and the kind's poll ends it on a later step, as the note's shared shape says (a wait
//! starts in one step and is first polled in the next). A starter that does not begin a wait returns at once and the
//! script's next statement runs in the same step.
//! - `ActionSequence.Sleep(Seconds)`: only with the action's `bInitialized` set: `LatentFloat` = `Seconds`, then each
//!   poll subtracts the delta and the wait ends when what is left is below half the delta; the end sets `LatentFloat`
//!   to 0. While `bInitialized` is clear the poll changes nothing. Single precision.
//! - `WaitForPath()`: waits only with `MyNavHandle` set and the handle's `bInitialized` set; ends on the first poll on
//!   which the path is valid, by the movement host's `PathIsValid` answer (`Locomotion::path_is_valid`, the same as the
//!   non-latent native). No time-out.
//! - `FollowPath()`: an invalid path is no wait. A valid one asks the navigation handle to start following, and the
//!   wait's end is the handle's "active path" answer and the action's "path ended" routine: neither is a movement-host
//!   answer today, and the note leaves both open (its question 2): `NotYet { LayerAbsent }` naming the navigation mesh.
//! - `FinishPath()`: no handle is no wait; with one, the handle's "finished" answer is the nav mesh's
//!   (`NotYet { LayerAbsent }`).
//! - `WaitForPawnToTurn()`: always waits. With `MyWillowPawn` set the direction the pawn query returns is not read (the
//!   note's question 3): `NotYet { NotRead }`. Without one, the first poll ends it (no pawn). The poll's end rule
//!   itself (within 255 units, or an overshoot within a quarter turn) is [`turn_wait_continues`].
//! - `WaitForPawnToStop()`: always waits; ends when every component of `MyGearboxPawn`'s `Velocity` and
//!   `Acceleration` is exactly 0. With no `MyGearboxPawn` the native reads through a missing object; that is refused
//!   (`NotYet { NotRead }`), as the note asks.
//! - `ActionSequence.InterruptLatentAction()`: the end routine at once: the wait ends and `LatentFloat` becomes 0.
//!
//! **The state step** ([`step_action`]): `ow_vm::states::process_action_sequence` (the fifth state change ends the
//! step; code runs only while `bInitialized` stays set), then, when the step ran the code out on an action with a
//! `ParentSequence`, the parent's notice, which is not read for a tree action (`NotYet { NotRead }`). **Nothing calls
//! it from the brain yet:** which entry point the AI component uses for a running tree action, with which delta and in
//! what order against the action's `Update`, is the note's open question 1. So the bullymong's state code does not run
//! in any world yet, and the waits above are reached only by a caller of [`step_action`].
//!
//! **Target records** (`AIComponent.GetTargetRecord(optional bParent)`, `GetTargetRecordFor(A, optional bParent)`):
//! the first non-empty record of the component's `Targets` whose `Target` is `A` (for `GetTargetRecord`, `A` is
//! `CurrentTarget`); no actor answers None. With `bParent` the search climbs to the parent component: no component
//! has a parent in this port yet (`super::vm`, "No parent"), so a lone one searches its own list, and one whose
//! awareness state says it has a parent is `NotYet` (the den's records). The port keeps its records in
//! Rust (`super::record`) and writes no `WillowMindTargetInfo` objects into `Targets`, so these answer None, which is
//! why the attacks' `Target_Acquired` finds no record (the note's explanation). Mirroring the records as VM objects
//! needs the record's fields as the game fills them, which the note does not give: an open item.
//!
//! **The mind's queries** (`WillowMind`): `IsTooFarFromHome` is false with no `Pawn`; otherwise it needs the den.
//! `GetPopOpDen` needs the world's population master and its opportunity list, which the note names only as "read as"
//! (`NotYet { NotRead }`). `IsInScriptedMovement` is the note's answer when `FollowActor` is None, and `NotYet` when it
//! is set (the check that clears it, question 5). `ShouldLookAtPlayer` is false with no `MyWillowPawn` or `AIClass`,
//! else `NotYet` (question 4 and the global condition). `CheckForPerch` is `NotYet` (question 6).

use std::rc::Rc;

use ow_combat::hit::access::{array, get, number, object, set, truth};
use ow_core::{NotYet, ParityMode};
use ow_object::ids::ObjRef;
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, NativeFn, Vm};
use ow_vm::error::VmError;
use ow_vm::latent::{self, HostLatentKind, HostLatentPoll, LatentStatus};

use super::actions::{Locomotion, locomotion_host};

#[cfg(test)]
mod tests;

/// The note these rules come from.
pub const RECORD_MOVEMENT: &str = "docs/verification/NATIVE_AI_MOVEMENT.md";

/// The layer a path wait needs beyond what the movement host answers.
const NAV_MESH: &str = "the navigation mesh (the handle's path following and finish answers)";

pub const SLEEP: &str = "GearboxFramework.ActionSequence.Sleep";
pub const INTERRUPT: &str = "GearboxFramework.ActionSequence.InterruptLatentAction";
pub const WAIT_FOR_PATH: &str = "WillowGame.WillowActionSequencePawn.WaitForPath";
pub const FOLLOW_PATH: &str = "WillowGame.WillowActionSequencePawn.FollowPath";
pub const FINISH_PATH: &str = "WillowGame.WillowActionSequencePawn.FinishPath";
pub const WAIT_FOR_TURN: &str = "WillowGame.WillowActionSequencePawn.WaitForPawnToTurn";
pub const WAIT_FOR_STOP: &str = "WillowGame.WillowActionSequencePawn.WaitForPawnToStop";
pub const TARGET_RECORD: &str = "GearboxFramework.AIComponent.GetTargetRecord";
pub const TARGET_RECORD_FOR: &str = "GearboxFramework.AIComponent.GetTargetRecordFor";
pub const POP_OP_DEN: &str = "WillowGame.WillowMind.GetPopOpDen";
pub const TOO_FAR_FROM_HOME: &str = "WillowGame.WillowMind.IsTooFarFromHome";
pub const IN_SCRIPTED_MOVEMENT: &str = "WillowGame.WillowMind.IsInScriptedMovement";
pub const LOOK_AT_PLAYER: &str = "WillowGame.WillowMind.ShouldLookAtPlayer";
pub const CHECK_FOR_PERCH: &str = "WillowGame.WillowMind.CheckForPerch";

/// The host latent kinds, by name.
const KIND_SLEEP: &str = "ow-ai ActionSequence.Sleep";
const KIND_PATH: &str = "ow-ai WillowActionSequencePawn.WaitForPath";
const KIND_TURN: &str = "ow-ai WillowActionSequencePawn.WaitForPawnToTurn";
const KIND_STOP: &str = "ow-ai WillowActionSequencePawn.WaitForPawnToStop";

fn error(message: String) -> VmError {
    VmError::Script { function: "ai movement".to_string(), message }
}

fn this(c: &NativeCall<'_>) -> Result<ObjRef, VmError> {
    c.this.ok_or_else(|| error("an AI native called without an object".to_string()))
}

/// A bool field the class must declare.
fn flag(vm: &mut Vm, o: ObjRef, name: &str) -> Result<bool, VmError> {
    let v = get(vm, o, name)?.ok_or_else(|| error(format!("the object declares no {name}")))?;
    Ok(truth(vm, &v))
}

/// A float field the class must declare.
fn float(vm: &mut Vm, o: ObjRef, name: &str) -> Result<f32, VmError> {
    match get(vm, o, name)? {
        Some(Value::Float(f)) => Ok(f),
        Some(other) => Err(error(format!("{name} holds {}, not a float", other.kind_name()))),
        None => Err(error(format!("the object declares no {name}"))),
    }
}

fn write(vm: &mut Vm, o: ObjRef, name: &str, v: Value) -> Result<(), VmError> {
    if set(vm, o, name, v)? {
        Ok(())
    } else {
        Err(error(format!("the object declares no {name}")))
    }
}

/// The kind registered as `name`, registering it with `poll` on first use.
fn kind(vm: &mut Vm, name: &str, poll: fn() -> HostLatentPoll) -> Result<HostLatentKind, VmError> {
    match latent::host_latent_kind(vm, name)? {
        Some(k) => Ok(k),
        None => latent::register_host_latent(vm, name, poll()),
    }
}

/// The movement host, or `NotYet { LayerAbsent }` naming the navigation mesh.
fn host(vm: &mut Vm) -> Result<Rc<std::cell::RefCell<dyn Locomotion>>, VmError> {
    locomotion_host(vm)?.ok_or_else(|| {
        NotYet::layer_absent("a path wait: the path's validity", "the navigation mesh (a movement host)", RECORD_MOVEMENT)
            .into()
    })
}

/// The action's pawn's path validity, as the non-latent `PathIsValid` answers it.
fn path_is_valid(vm: &mut Vm, action: ObjRef) -> Result<bool, VmError> {
    let pawn = object(vm, action, "MyWillowPawn")?.ok_or_else(|| error("the action has no MyWillowPawn".to_string()))?;
    let h = host(vm)?;
    let mut h = h.try_borrow_mut().map_err(|_| VmError::ServiceBorrowed("the AI's movement host"))?;
    h.path_is_valid(vm, pawn)
}

// ---------------------------------------------------------------------------------------------------------------
// The waits
// ---------------------------------------------------------------------------------------------------------------

fn sleep_poll() -> HostLatentPoll {
    Rc::new(|vm, action, _ticket, delta| {
        if !flag(vm, action, "bInitialized")? {
            return Ok(LatentStatus::Pending);
        }
        let left = float(vm, action, "LatentFloat")? - delta;
        if left < 0.5 * delta {
            write(vm, action, "LatentFloat", Value::Float(0.0))?;
            Ok(LatentStatus::Done)
        } else {
            write(vm, action, "LatentFloat", Value::Float(left))?;
            Ok(LatentStatus::Pending)
        }
    })
}

fn n_sleep(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let me = this(c)?;
    if !flag(c.vm, me, "bInitialized")? {
        return Ok(Value::None);
    }
    let seconds = match c.arg(0)? {
        Value::Float(f) => *f,
        Value::None => 0.0,
        other => return Err(error(format!("Sleep's Seconds holds {}, not a float", other.kind_name()))),
    };
    write(c.vm, me, "LatentFloat", Value::Float(seconds))?;
    let k = kind(c.vm, KIND_SLEEP, sleep_poll)?;
    latent::start_host_latent(c, k)?;
    Ok(Value::None)
}

fn n_interrupt(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let me = this(c)?;
    if let Some(h) = latent::host_latent_of(c.vm, me)? {
        latent::finish_host_latent(c.vm, me, h.ticket())?;
    }
    write(c.vm, me, "LatentFloat", Value::Float(0.0))?;
    Ok(Value::None)
}

fn path_poll() -> HostLatentPoll {
    Rc::new(|vm, action, _ticket, _delta| {
        Ok(if path_is_valid(vm, action)? { LatentStatus::Done } else { LatentStatus::Pending })
    })
}

fn n_wait_for_path(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let me = this(c)?;
    let Some(handle) = object(c.vm, me, "MyNavHandle")? else {
        return Ok(Value::None);
    };
    if !flag(c.vm, handle, "bInitialized")? {
        return Ok(Value::None);
    }
    let k = kind(c.vm, KIND_PATH, path_poll)?;
    latent::start_host_latent(c, k)?;
    Ok(Value::None)
}

fn n_follow_path(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let me = this(c)?;
    if !path_is_valid(c.vm, me)? {
        return Ok(Value::None);
    }
    Err(NotYet::layer_absent(
        "FollowPath on a valid path: the handle's start of the follow, its \"active path\" answer and the action's \
         \"path ended\" routine (NATIVE_AI_MOVEMENT.md open question 2)",
        NAV_MESH,
        RECORD_MOVEMENT,
    )
    .into())
}

fn n_finish_path(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let me = this(c)?;
    if object(c.vm, me, "MyNavHandle")?.is_none() {
        return Ok(Value::None);
    }
    Err(NotYet::layer_absent("FinishPath with a navigation handle: the handle's \"finished\" answer", NAV_MESH, RECORD_MOVEMENT)
        .into())
}

/// The name of byte enum field `name` of `o`.
fn enum_name(vm: &mut Vm, o: ObjRef, name: &str) -> Result<String, VmError> {
    let class = vm.space.heap.get(o)?.class;
    let p = vm.space.prop(class, name).ok_or_else(|| error(format!("the object declares no {name}")))?;
    let Some(PropKind::Byte { enum_: Some(e) }) = vm.space.classes.decl(p).map(|d| d.kind.clone()) else {
        return Err(error(format!("{name} is not an enum byte")));
    };
    let b = match get(vm, o, name)? {
        Some(Value::Byte(b)) => b,
        _ => return Err(error(format!("{name} does not hold a byte"))),
    };
    let values = vm.space.classes.enum_layout(e).map(|l| l.values().to_vec()).unwrap_or_default();
    let n = values.get(usize::from(b)).ok_or_else(|| error(format!("{name} is {b}, outside its enum")))?;
    Ok(vm.space.names.text(*n).into_owned())
}

fn turn_not_read() -> VmError {
    NotYet::not_read(
        "WaitForPawnToTurn with a pawn: the direction the pawn's query returns (NATIVE_AI_MOVEMENT.md open question 3)",
        RECORD_MOVEMENT,
    )
    .into()
}

fn turn_poll() -> HostLatentPoll {
    Rc::new(|vm, action, _ticket, _delta| {
        let pawn = object(vm, action, "MyWillowPawn")?;
        let mind = object(vm, action, "MyWillowMind")?;
        let (Some(_), Some(mind)) = (pawn, mind) else {
            return Ok(LatentStatus::Done);
        };
        if enum_name(vm, mind, "FacingPolicy")? == "FACE_NoChange" {
            return Ok(LatentStatus::Done);
        }
        Err(turn_not_read())
    })
}

fn n_wait_for_turn(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let me = this(c)?;
    if object(c.vm, me, "MyWillowPawn")?.is_some() {
        return Err(turn_not_read());
    }
    let k = kind(c.vm, KIND_TURN, turn_poll)?;
    latent::start_host_latent(c, k)?;
    Ok(Value::None)
}

/// A direction's yaw in Unreal rotation units: `atan2(y, x) * 65535 / (2 pi)`, rounded to the nearest integer. The
/// note does not give the precision; this computes in double through `ow_core::num` (UNVERIFIED).
pub fn yaw_of(x: f32, y: f32) -> i32 {
    let turns = ow_core::num::atan2_f64(f64::from(y), f64::from(x)) * 65535.0 / std::f64::consts::TAU;
    turns.round() as i32
}

/// An angle wrapped to a signed 16-bit angle.
fn wrap16(a: i32) -> i32 {
    i32::from(a as i16)
}

/// `WaitForPawnToTurn`'s poll rule (the note's step 3): with `desired` the pawn's `DesiredRotation.Yaw`, `target` the
/// target yaw now and `last` the `LastYawCheck`, whether the wait continues: the error (`desired - target`, wrapped)
/// is above 255 in magnitude and either it and the previous error (`desired - last`, wrapped) are on the same side
/// (both positive, or both zero or negative), or either magnitude is above 16383. The caller then stores `target` in
/// `LastYawCheck`. Not reached yet: the direction behind `target` is open (module documentation).
pub fn turn_wait_continues(desired: i32, target: i32, last: i32) -> bool {
    let error = wrap16(desired.wrapping_sub(target));
    let previous = wrap16(desired.wrapping_sub(last));
    error.abs() > 255 && ((error > 0) == (previous > 0) || previous.abs() > 16383 || error.abs() > 16383)
}

fn stop_poll() -> HostLatentPoll {
    Rc::new(|vm, action, _ticket, _delta| {
        let Some(pawn) = object(vm, action, "MyGearboxPawn")? else {
            return Err(NotYet::not_read(
                "WaitForPawnToStop with no MyGearboxPawn (the native reads through a missing object)",
                RECORD_MOVEMENT,
            )
            .into());
        };
        for name in ["Velocity", "Acceleration"] {
            let v = get(vm, pawn, name)?.ok_or_else(|| error(format!("the pawn declares no {name}")))?;
            for axis in ["X", "Y", "Z"] {
                let x = ow_combat::hit::access::field(vm, &v, axis)
                    .ok_or_else(|| error(format!("{name} has no {axis}")))?;
                if number(&x) != 0.0 {
                    return Ok(LatentStatus::Pending);
                }
            }
        }
        Ok(LatentStatus::Done)
    })
}

fn n_wait_for_stop(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let k = kind(c.vm, KIND_STOP, stop_poll)?;
    latent::start_host_latent(c, k)?;
    Ok(Value::None)
}

// ---------------------------------------------------------------------------------------------------------------
// The state step
// ---------------------------------------------------------------------------------------------------------------

/// One action-sequence state step of `action` with `delta` (module documentation). Not called by the brain yet.
pub fn step_action(vm: &mut Vm, action: ObjRef, delta: f32) -> Result<(), VmError> {
    let had_code = ow_vm::states::has_state_code(vm, action)?;
    ow_vm::states::process_action_sequence(vm, action, delta)?;
    if had_code && !ow_vm::states::has_state_code(vm, action)? && object(vm, action, "ParentSequence")?.is_some() {
        return Err(NotYet::not_read(
            "the action-sequence step's notice to the ParentSequence when a child's code runs out (the ExecuteSequence \
             wait, poll 1001, is not read)",
            RECORD_MOVEMENT,
        )
        .into());
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// Target records and the mind's queries
// ---------------------------------------------------------------------------------------------------------------

/// The first non-empty record of `component`'s `Targets` whose `Target` is `actor`.
fn record_for(vm: &mut Vm, component: ObjRef, actor: Option<ObjRef>) -> Result<Value, VmError> {
    let Some(actor) = actor else {
        return Ok(Value::Object(None));
    };
    for record in array(vm, component, "Targets")? {
        let Value::Object(Some(r)) = record else {
            continue;
        };
        if object(vm, r, "Target")? == Some(actor) {
            return Ok(Value::Object(Some(r)));
        }
    }
    Ok(Value::Object(None))
}

/// The search with `bParent`: on a component with a parent the note climbs to the parent's records, which this port
/// does not model (`super::vm`, "No parent"), so that case is `NotYet`; otherwise the component's own list.
fn record_for_parent(
    vm: &mut Vm,
    component: ObjRef,
    actor: Option<ObjRef>,
    b_parent: bool,
    has_parent: bool,
) -> Result<Value, VmError> {
    if b_parent && has_parent && actor.is_some() {
        return Err(NotYet::not_ported(
            "AIComponent.GetTargetRecordFor with bParent on a component with a parent (a population den member): the \
             parent's records",
            RECORD_MOVEMENT,
        )
        .into());
    }
    record_for(vm, component, actor)
}

/// The optional `bParent` argument at `index` (absent: false).
fn b_parent(c: &NativeCall<'_>, index: usize) -> Result<bool, VmError> {
    if index >= c.arg_count() {
        return Ok(false);
    }
    Ok(truth(c.vm, c.arg(index)?))
}

fn n_target_record(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let me = this(c)?;
    let current = object(c.vm, me, "CurrentTarget")?;
    let parent = b_parent(c, 0)?;
    let has_parent = super::vm::component_has_parent(c.vm, me)?;
    record_for_parent(c.vm, me, current, parent, has_parent)
}

fn n_target_record_for(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let me = this(c)?;
    let actor = match c.arg(0)? {
        Value::Object(o) => *o,
        _ => None,
    };
    let parent = b_parent(c, 1)?;
    let has_parent = super::vm::component_has_parent(c.vm, me)?;
    record_for_parent(c.vm, me, actor, parent, has_parent)
}

fn den_not_read() -> VmError {
    NotYet::not_read(
        "WillowMind.GetPopOpDen: where the world holds its population master and that master's opportunity list (the \
         note reads both engine look-ups only as \"read as\")",
        RECORD_MOVEMENT,
    )
    .into()
}

fn n_too_far_from_home(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let me = this(c)?;
    if object(c.vm, me, "Pawn")?.is_none() {
        return Ok(Value::Bool(false));
    }
    Err(den_not_read())
}

fn n_in_scripted_movement(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let me = this(c)?;
    if object(c.vm, me, "FollowActor")?.is_some() {
        return Err(NotYet::not_read(
            "WillowMind.IsInScriptedMovement with a FollowActor: the check that clears it (NATIVE_AI_MOVEMENT.md open \
             question 5)",
            RECORD_MOVEMENT,
        )
        .into());
    }
    let any = object(c.vm, me, "ScriptedMoveTarget")?.is_some()
        || object(c.vm, me, "ScriptedFocus")?.is_some()
        || flag(c.vm, me, "bScriptedHoldPosition")?;
    Ok(Value::Bool(any))
}

fn n_look_at_player(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let me = this(c)?;
    if object(c.vm, me, "MyWillowPawn")?.is_none() || object(c.vm, me, "AIClass")?.is_none() {
        return Ok(Value::Bool(false));
    }
    Err(NotYet::not_read(
        "WillowMind.ShouldLookAtPlayer: the global condition, the mind's timed flag and the pawn check \
         (NATIVE_AI_MOVEMENT.md open question 4)",
        RECORD_MOVEMENT,
    )
    .into())
}

fn n_check_for_perch(_c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    Err(NotYet::not_read(
        "WillowMind.CheckForPerch: the den's perch refresh, the unnamed perch checks, the body-tag check and the mind \
         check (NATIVE_AI_MOVEMENT.md open question 6)",
        RECORD_MOVEMENT,
    )
    .into())
}

/// Binds the natives of this module in `ParityMode::Off` (module documentation); `actions::Pack` calls it before it
/// binds its own `NotYet` for the latent moves, which then keep only the ones left unbound here.
pub(crate) fn register(vm: &mut Vm) -> Result<(), VmError> {
    // Called directly (not through a fn pointer), so `actions::bind`'s #[track_caller] names this file (lane
    // SHADOW-AUDIT-FOLLOWUPS-2).
    for (path, body) in bodies(vm) {
        super::actions::bind(vm, path, body)?;
    }
    Ok(())
}

/// The paths [`register`] binds and their bodies: the 14 stock natives in `ParityMode::Off`, none under Reference.
pub(crate) fn bodies(vm: &Vm) -> Vec<(&'static str, NativeFn)> {
    if vm.parity != ParityMode::Off {
        return Vec::new();
    }
    let bodies: [(&'static str, NativeFn); 14] = [
        (SLEEP, Rc::new(n_sleep)),
        (INTERRUPT, Rc::new(n_interrupt)),
        (WAIT_FOR_PATH, Rc::new(n_wait_for_path)),
        (FOLLOW_PATH, Rc::new(n_follow_path)),
        (FINISH_PATH, Rc::new(n_finish_path)),
        (WAIT_FOR_TURN, Rc::new(n_wait_for_turn)),
        (WAIT_FOR_STOP, Rc::new(n_wait_for_stop)),
        (TARGET_RECORD, Rc::new(n_target_record)),
        (TARGET_RECORD_FOR, Rc::new(n_target_record_for)),
        (POP_OP_DEN, Rc::new(|_c| Err(den_not_read()))),
        (TOO_FAR_FROM_HOME, Rc::new(n_too_far_from_home)),
        (IN_SCRIPTED_MOVEMENT, Rc::new(n_in_scripted_movement)),
        (LOOK_AT_PLAYER, Rc::new(n_look_at_player)),
        (CHECK_FOR_PERCH, Rc::new(n_check_for_perch)),
    ];
    bodies.into_iter().collect()
}
