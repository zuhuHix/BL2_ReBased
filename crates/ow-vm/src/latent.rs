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
