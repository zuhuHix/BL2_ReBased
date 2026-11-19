//! Owner: lane W5-STATES. AI-assisted (Claude Code). Latent actions of state code and the behavior kernel's latent
//! wait: `Actor.Sleep` and its poll (NATIVE_ENGINE_CORE.md, "Actor.Sleep (latent) and the latent poll"), and the
//! 1/60 s floor on a latent behavior's wait (NATIVE_BEHAVIOR_POPULATION.md, "Latent wait", for the behavior kernel
//! of W5-BEHAVIOR).
//!
//! **Sleep.** `Sleep(Seconds)` stores `Seconds` in the actor's `LatentFloat` and sets the frame's latent action; the
//! statement then yields (`crate::states`). Each state-code step first polls it: `LatentFloat` loses the step's
//! delta, and once what is left is below half that delta the action is cleared and the code continues in the same
//! step, so a sleep ends up to half a frame early rather than late. The record gives the latent action a fixed code
//! without naming the number; here it is the enum [`LatentAction`]. The subtraction and the comparison are done in
//! single precision, as the float field is (UNVERIFIED).
//!
//! **Only outside `ParityMode::Reference`.** The C++ binds no `Sleep` and runs no state code, so under Reference
//! `Actor.Sleep` stays unbound (its stub line, as in the C++).
//!
//! **The kernel's floor.** A latent behavior asks for a wait; a negative wait means finished, any other is raised
//! to at least 1/60 s and the thread runs again at `now + wait` ([`kernel_resume_at`], in double precision as the
//! C++ `src/behavior.cpp` computes it). That rule belongs to the behavior kernel, not to state code: the record's
//! `Sleep` has no 1/60 floor, it ends on the first step whose remainder is under half the delta.
//!
//! **Host latent actions** (lane VM-LATENT-HOST, decision 2026-11-19-07). The engine's other latent functions
//! (`MoveTo`, `MoveToward`, `FinishAnim`, `FinishRotation`, `WaitForLanding` and the like) are not the VM's: their
//! effect is movement, animation or physics that the host or `ow-world` owns. This module gives them a seam, after the
//! stock UE3 pattern of a latent native and its poll (public UnrealScript semantics, not read from this executable):
//!
//! 1. The host registers a kind once with [`register_host_latent`]: a name and a poll function.
//! 2. The host binds the latent native itself (`Vm::register_native`). Its body does the host's set-up (a goal, an
//!    animation) and calls [`start_host_latent`], which allocates a [`LatentTicket`] and sets the frame's latent
//!    action to [`LatentAction::Host`]. The statement then yields, as after `Sleep`.
//! 3. Each later state-code step polls first ([`poll`], from `states::process_state`, in the order the caller steps
//!    its objects): the kind's poll function gets the object, the ticket and the step's delta and answers
//!    [`LatentStatus::Pending`] (the step ends) or [`LatentStatus::Done`] (the action is cleared and the code
//!    continues in the same step, as `Sleep`'s poll does).
//! 4. A host that learns of the end some other way (an animation-end notification) calls [`finish_host_latent`]
//!    instead; the next step then continues without calling the poll function.
//!
//! A state change (`GotoState`, a label, `PushState` / `PopState`) clears the latent action as it does for `Sleep`;
//! the host sees that as its ticket no longer being current ([`host_latent_of`], [`finish_host_latent`] answering
//! `false`). Nothing tells the host's own movement or animation to stop: that is the host's business, as in stock
//! UE3, where a state change leaves the pawn's move target alone. Tickets count up from 1 per VM in start order, so a
//! run that starts the same calls in the same order gets the same tickets. Which latent functions this game binds and
//! how Gearbox's own latent natives poll is not read (UNVERIFIED); the seam fixes only the stock shape.
//!
//! Status: the record's reading of the executable; UNVERIFIED in the running game.

use std::rc::Rc;

use ow_core::ParityMode;
use ow_object::ids::{ObjRef, PropAddr, PropId};
use ow_object::value::Value;

use crate::api::{NativeCall, Vm};
use crate::error::VmError;
use crate::states;

/// A pending latent action of a state frame.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LatentAction {
    /// `Actor.Sleep`: waits on the actor's `LatentFloat`.
    Sleep,
    /// A host-driven latent call ([`start_host_latent`]).
    Host(HostLatent),
}

/// A kind of host latent action, as [`register_host_latent`] returned it. A kind belongs to the VM that registered it:
/// it is an index into that VM's registry, and another VM may hold a different kind (or none) at the same index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HostLatentKind(u32);

/// One started host latent call: unique per VM, counting up from 1 in start order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LatentTicket(u64);

impl LatentTicket {
    /// The ticket's number.
    pub fn get(self) -> u64 {
        self.0
    }
}

/// The host latent call a state frame waits on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostLatent {
    kind: HostLatentKind,
    ticket: LatentTicket,
    finished: bool,
}

impl HostLatent {
    /// The call's kind.
    pub fn kind(self) -> HostLatentKind {
        self.kind
    }

    /// The call's ticket.
    pub fn ticket(self) -> LatentTicket {
        self.ticket
    }

    /// The host called [`finish_host_latent`]; the next step continues after the call.
    pub fn is_finished(self) -> bool {
        self.finished
    }
}

/// A poll function's answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LatentStatus {
    /// Still running: the state-code step ends.
    Pending,
    /// Finished: the latent action is cleared and the code continues in the same step.
    Done,
}

/// A host latent kind's poll: `(vm, object, ticket, delta)`, called once per state-code step while the call is
/// pending and not finished. It may call into the VM (the registry is not borrowed while it runs, so it may look kinds
/// up or register new ones); a state change it makes wins over its answer. An `Err` ends the step with that error and
/// leaves the call pending: the next step polls it again. A poll must not step its own object (`process_state` on the
/// object it is polling): that is a re-entrant state-code step, which the VM does not define.
pub type HostLatentPoll =
    Rc<dyn Fn(&mut Vm, ObjRef, LatentTicket, f32) -> Result<LatentStatus, VmError>>;

/// The registered kinds and the ticket counter (a VM service, added on first use).
#[derive(Default)]
struct HostLatents {
    kinds: Vec<(String, HostLatentPoll)>,
    last_ticket: u64,
}

/// Runs `f` on the host latent registry, borrowed only for `f` (never across a poll call).
fn with_hosts<R>(vm: &mut Vm, f: impl FnOnce(&mut HostLatents) -> R) -> Result<R, VmError> {
    let cell = match vm.service::<HostLatents>() {
        Some(h) => h,
        None => vm.add_service(HostLatents::default())?,
    };
    let mut guard = cell
        .try_borrow_mut()
        .map_err(|_| VmError::ServiceBorrowed("ow_vm::latent::HostLatents"))?;
    Ok(f(&mut guard))
}

/// Registers a host latent kind `name` with its poll function. A second kind of one name is refused, and so is any kind
/// under `ParityMode::Reference`: state code does not run there (the C++ runs none), so no host call could start, as
/// `Actor.Sleep` stays unbound there.
pub fn register_host_latent(
    vm: &mut Vm,
    name: &str,
    poll: HostLatentPoll,
) -> Result<HostLatentKind, VmError> {
    if vm.parity == ParityMode::Reference {
        return Err(VmError::Registration {
            key: name.to_string(),
            reason: "host latent kinds are not registered under ParityMode::Reference (no state code runs there)",
        });
    }
    with_hosts(vm, |h| {
        if h.kinds.iter().any(|(n, _)| n == name) {
            return Err(VmError::Registration {
                key: name.to_string(),
                reason: "duplicate host latent kind",
            });
        }
        let index = u32::try_from(h.kinds.len()).map_err(|_| VmError::Registration {
            key: name.to_string(),
            reason: "too many host latent kinds",
        })?;
        h.kinds.push((name.to_string(), poll));
        Ok(HostLatentKind(index))
    })?
}

/// The kind registered as `name`, if any. A lookup does not add the registry service to the VM.
pub fn host_latent_kind(vm: &mut Vm, name: &str) -> Result<Option<HostLatentKind>, VmError> {
    let Some(cell) = vm.service::<HostLatents>() else {
        return Ok(None);
    };
    let hosts = cell
        .try_borrow()
        .map_err(|_| VmError::ServiceBorrowed("ow_vm::latent::HostLatents"))?;
    Ok(hosts
        .kinds
        .iter()
        .position(|(n, _)| n == name)
        .and_then(|i| u32::try_from(i).ok())
        .map(HostLatentKind))
}

/// Starts a host latent call of `kind` from inside the latent native `c`: only from the object's own running state
/// code, as for `Sleep` (so never under `ParityMode::Reference`, where state code does not run). Sets the frame's
/// latent action; the statement yields after the native returns. A call started while another is pending replaces
/// it (one latent action per frame; the old ticket stops being current).
pub fn start_host_latent(
    c: &mut NativeCall<'_>,
    kind: HostLatentKind,
) -> Result<LatentTicket, VmError> {
    let Some(this) = c.this else {
        return Err(misuse(c, "a host latent function called with no object"));
    };
    if !states::running_code_of(c.vm, this)? {
        return Err(misuse(
            c,
            "a latent function runs only in the object's state code",
        ));
    }
    let ticket = with_hosts(c.vm, |h| {
        let known = usize::try_from(kind.0).is_ok_and(|i| i < h.kinds.len());
        known.then(|| {
            h.last_ticket += 1;
            LatentTicket(h.last_ticket)
        })
    })?
    .ok_or_else(|| misuse(c, "a host latent kind that is not registered"))?;
    states::set_latent(
        c.vm,
        this,
        Some(LatentAction::Host(HostLatent {
            kind,
            ticket,
            finished: false,
        })),
    )?;
    Ok(ticket)
}

/// The host latent call `o` waits on, if any. None for an object that is destroyed (or freed), even before its state
/// frame is forgotten: `states::process_state` never steps a destroyed object, so its call can never complete.
pub fn host_latent_of(vm: &mut Vm, o: ObjRef) -> Result<Option<HostLatent>, VmError> {
    if vm.space.heap.get(o).map_or(true, |x| x.destroyed) {
        return Ok(None);
    }
    Ok(match states::latent_of(vm, o)? {
        Some(LatentAction::Host(h)) => Some(h),
        _ => None,
    })
}

/// Marks `o`'s host latent call `ticket` finished: `o`'s next state-code step continues after the call without
/// polling it. `false` (and nothing changes) when `ticket` is not `o`'s current call (a state change cleared it,
/// another call replaced it, the ticket is another object's, or `o` is destroyed).
pub fn finish_host_latent(vm: &mut Vm, o: ObjRef, ticket: LatentTicket) -> Result<bool, VmError> {
    match host_latent_of(vm, o)? {
        Some(h) if h.ticket == ticket => {
            let finished = HostLatent {
                finished: true,
                ..h
            };
            states::set_latent(vm, o, Some(LatentAction::Host(finished)))?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

/// The host part of [`poll`]: `true` while the call is still pending.
fn poll_host(vm: &mut Vm, o: ObjRef, h: HostLatent, delta: f32) -> Result<bool, VmError> {
    if h.finished {
        states::set_latent(vm, o, None)?;
        return Ok(false);
    }
    let function = with_hosts(vm, |hosts| {
        usize::try_from(h.kind.0)
            .ok()
            .and_then(|i| hosts.kinds.get(i))
            .map(|(_, f)| Rc::clone(f))
    })?
    .ok_or_else(|| VmError::Script {
        function: "ow_vm::latent".to_string(),
        message: "a host latent kind that is not registered".to_string(),
    })?;
    let status = function(vm, o, h.ticket, delta)?;
    match states::latent_of(vm, o)? {
        Some(LatentAction::Host(now)) if now.ticket == h.ticket => {
            if status == LatentStatus::Done || now.finished {
                states::set_latent(vm, o, None)?;
                Ok(false)
            } else {
                Ok(true)
            }
        }
        // The poll changed the state, which cleared the action: the code continues at the new position.
        other => Ok(other.is_some()),
    }
}

/// The floor on a latent behavior's wait: 1/60 s (NATIVE_BEHAVIOR_POPULATION.md, "Latent wait"; the C++
/// `src/behavior.cpp` writes `1.0 / 60.0`).
pub const KERNEL_MIN_WAIT: f64 = 1.0 / 60.0;

/// When a latent behavior that asked for `wait` seconds at game time `now` runs again: `None` when `wait` is
/// negative (or NaN: the C++ tests `wait >= 0`), the behavior is finished; otherwise `now + max(wait, 1/60)`, the
/// maximum written as the C++ `std::max` (`wait < floor ? floor : wait`). Status: UNVERIFIED in the running game.
pub fn kernel_resume_at(now: f64, wait: f64) -> Option<f64> {
    if wait.is_nan() || wait < 0.0 {
        return None;
    }
    let floored = if wait < KERNEL_MIN_WAIT {
        KERNEL_MIN_WAIT
    } else {
        wait
    };
    Some(now + floored)
}

/// Registers `Actor.Sleep(float)` outside `ParityMode::Reference` (under Reference it stays unbound, as in the
/// C++). `natives_core::register` calls it.
pub(crate) fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity == ParityMode::Reference {
        return Ok(());
    }
    vm.register_native("Actor.Sleep(float)", Rc::new(sleep))
}

/// `Actor.Sleep(Seconds)`: only from the object's own running state code (the compiler allows latent calls only
/// there; a host call elsewhere is refused).
fn sleep(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let Some(this) = c.this else {
        return Err(misuse(c, "Actor.Sleep called with no object"));
    };
    if !states::running_code_of(c.vm, this)? {
        return Err(misuse(
            c,
            "Actor.Sleep is a latent function: it runs only in the object's state code",
        ));
    }
    let seconds = match c.arg(0)? {
        Value::Float(v) => *v,
        Value::None => 0.0,
        _ => {
            return Err(VmError::BadArgument {
                function: c.vm.function_path(c.function),
                index: 0,
                reason: "expected a float",
            });
        }
    };
    let prop = latent_float(c.vm, this)?;
    c.vm
        .space
        .write(this, PropAddr::scalar(prop), Value::Float(seconds))?;
    states::set_latent(c.vm, this, Some(LatentAction::Sleep))?;
    Ok(Value::None)
}

fn misuse(c: &NativeCall<'_>, message: &str) -> VmError {
    VmError::Script {
        function: c.vm.function_path(c.function),
        message: message.to_string(),
    }
}

/// The actor's `LatentFloat` property (a class without one is not an actor).
fn latent_float(vm: &Vm, o: ObjRef) -> Result<PropId, VmError> {
    let class = vm.space.heap.get(o)?.class;
    vm.space
        .prop(class, "LatentFloat")
        .ok_or_else(|| VmError::Script {
            function: "Actor.Sleep".to_string(),
            message: "the object has no LatentFloat property (Sleep is an actor's)".to_string(),
        })
}

/// The latent poll at the start of a state-code step with the tick's `delta`: `true` while the action is still
/// pending (the step ends), `false` when there is none or it just finished (the code continues in this step).
pub(crate) fn poll(vm: &mut Vm, o: ObjRef, delta: f32) -> Result<bool, VmError> {
    match states::latent_of(vm, o)? {
        None => Ok(false),
        Some(LatentAction::Host(h)) => poll_host(vm, o, h, delta),
        Some(LatentAction::Sleep) => {
            let prop = latent_float(vm, o)?;
            let before = match vm.space.read(o, PropAddr::scalar(prop))? {
                Value::Float(v) => *v,
                _ => {
                    return Err(VmError::Script {
                        function: "Actor.Sleep".to_string(),
                        message: "LatentFloat does not hold a float".to_string(),
                    });
                }
            };
            let left = before - delta;
            vm.space
                .write(o, PropAddr::scalar(prop), Value::Float(left))?;
            if left < 0.5 * delta {
                states::set_latent(vm, o, None)?;
                Ok(false)
            } else {
                Ok(true)
            }
        }
    }
}

#[cfg(test)]
mod host_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_kernel_wait_is_finished_when_negative_and_floored_at_a_sixtieth() {
        assert_eq!(kernel_resume_at(10.0, -1.0), None);
        assert_eq!(kernel_resume_at(10.0, -0.0), Some(10.0 + 1.0 / 60.0));
        assert_eq!(kernel_resume_at(10.0, f64::NAN), None);
        assert_eq!(kernel_resume_at(10.0, 0.0), Some(10.0 + 1.0 / 60.0));
        assert_eq!(kernel_resume_at(10.0, 0.01), Some(10.0 + 1.0 / 60.0));
        assert_eq!(kernel_resume_at(10.0, 1.0 / 60.0), Some(10.0 + 1.0 / 60.0));
        assert_eq!(kernel_resume_at(10.0, 0.5), Some(10.5));
    }
}
