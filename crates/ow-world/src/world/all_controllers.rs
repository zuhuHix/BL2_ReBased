//! Owner: lane IMPL-LINK6-AFTER-TAKE (decision 2026-11-26-57). The iterator `WorldInfo.AllControllers(BaseClass,
//! out C)` in `ParityMode::Off`. AI-assisted (Claude Code).
//!
//! **Clean room.** Implemented from docs/verification/NATIVE_LINK6_AFTER_TAKE.md section 4.1 (#797 as landed) alone;
//! this lane did not read the executable. **UNVERIFIED in the running game.** Reference binds nothing.
//!
//! The walk: the world info's `ControllerList` (by fit, the list head), then each controller's `NextController`, in
//! order; both declared as objects, else `VmError::Script`. A None class, or exactly `Engine.Controller`, visits every
//! entry; any other class the entries of that class or below. A controller the store has not loaded is [`NOT_LOADED`]
//! (`NotRead`); a loop in the list is `VmError::Script`. The VM's iterators take their list at the call, the game reads
//! the links live, so a copy of the chain is kept and compared after each body (#789's guard): a changed chain is
//! [`CHANGED`] (`NotRead`, prefixed). The guard is stricter than the game, which tolerates unlinking the current
//! controller.

use std::rc::Rc;

use ow_core::NotYet;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{IteratorFn, IteratorResult, NativeCall, Vm};
use ow_vm::error::VmError;

/// The record of the rule.
pub const RECORD: &str = "docs/verification/NATIVE_LINK6_AFTER_TAKE.md (section 4.1)";
/// The iterator's key.
pub const KEY: &str = "WorldInfo.AllControllers(class,object)";
pub const NOT_LOADED: &str = "AllControllers: a controller the store has not loaded (a load is a write)";
pub const CHANGED: &str = "AllControllers: the loop body changed ControllerList";
const WHAT: &str = "WorldInfo.AllControllers";

fn bad(message: impl Into<String>) -> VmError {
    VmError::Script { function: WHAT.to_string(), message: message.into() }
}

fn link(vm: &Vm, o: ObjRef, name: &str) -> Result<Option<ObjRef>, VmError> {
    let class = vm.space.heap.get(o)?.class;
    let p = vm.space.prop(class, name).ok_or_else(|| bad(format!("declares no {name}")))?;
    match (vm.space.classes.decl(p).map(|d| &d.kind), vm.space.read(o, PropAddr::scalar(p))?) {
        (Some(PropKind::Object { .. }), Value::Object(x)) => Ok(*x),
        _ => Err(bad(format!("{name} is not declared as an object"))),
    }
}

fn resident(vm: &Vm, o: ObjRef) -> Result<(), VmError> {
    let x = vm.space.heap.get(o)?;
    let short = vm.space.classes.get_layout(x.class).is_some_and(|l| x.slots.len() < l.slot_count() as usize);
    if vm.defs.is_unloaded(&vm.space, o) || vm.defs.is_failed(&vm.space, o) || short {
        return Err(NotYet::not_read(NOT_LOADED, RECORD).into());
    }
    Ok(())
}

/// The controller chain from `world`'s `ControllerList` (module documentation). Reads only.
fn chain(vm: &Vm, world: ObjRef) -> Result<Vec<ObjRef>, VmError> {
    let mut out = Vec::new();
    let mut cursor = link(vm, world, "ControllerList")?;
    while let Some(c) = cursor {
        if out.contains(&c) || out.len() > 65_536 {
            return Err(bad("the controller list loops"));
        }
        resident(vm, c)?;
        out.push(c);
        cursor = link(vm, c, "NextController")?;
    }
    Ok(out)
}

fn all_controllers() -> IteratorFn {
    Rc::new(|c: &mut NativeCall<'_>| {
        let world = c.this.ok_or_else(|| bad("called without a world info"))?;
        let base = match c.arg(0)? {
            Value::Class(cl) => *cl,
            Value::Object(None) | Value::None => None,
            other => return Err(bad(format!("BaseClass holds {}, not a class", other.kind_name()))),
        };
        let vm: &Vm = c.vm;
        let copy = chain(vm, world)?;
        let controller = vm.space.classes.by_path("Engine.Controller");
        let every = base.is_none() || base == controller;
        let mut items = Vec::new();
        for entry in &copy {
            let class = vm.space.heap.get(*entry)?.class;
            if every || base.is_some_and(|b| vm.space.classes.is_child_of(class, b)) {
                items.push(Value::Object(Some(*entry)));
            }
        }
        let guard: ow_vm::api::IteratorGuard = Rc::new(move |vm: &Vm| {
            if chain(vm, world)? != copy {
                return Err(NotYet::not_read(CHANGED, RECORD).after_partial_writes().into());
            }
            Ok(())
        });
        Ok(IteratorResult::new(1, items).with_guard(guard))
    })
}

/// Binds the iterator (the caller binds in Off only).
pub(crate) fn register(vm: &mut Vm) -> Result<(), VmError> {
    vm.register_iterator(KEY, all_controllers())
}

// Tests: `world/tests.rs`, module `link6` (they need the world fixture).
