//! Owner: lane IMPL-UPDATEAMMOCOUNTS. The engine natives link 6's take reaches (`PickupPickupable` → `GiveTo` →
//! `AddInventory`), in `ParityMode::Off`: `Actor.ComponentList`, `Actor.MakeNoise`, `Controller.IsUsingVehicle` and
//! `Actor.SetOwner`. AI-assisted (Claude Code).
//!
//! **Clean room.** Implemented from docs/verification/NATIVE_UPDATE_AMMO_COUNTS_AND_TAKE.md sections 3.3, 3.4, 3.5 and
//! 3.7 alone (lane ANALYST-UPDATEAMMOCOUNTS, #785, landed at 0195ce94); this lane did not read the executable. **Every rule is
//! UNVERIFIED in the running game.** Reference binds nothing (the stub policy answers).
//!
//! - **`ComponentList(BaseClass, out Component)`** (3.3): a None class iterates nothing; otherwise each non-None entry
//!   of the actor's `Components` whose class is `BaseClass` or below it, in array order, by the class table only (a
//!   lazy entry is judged by the class its handle carries). `Components` declared as an array of objects, else
//!   `VmError::Script`. The game re-reads the array at each step, and the VM's iterators hand over a list taken at the
//!   call. So the iterator keeps a copy of `Components` and compares it after each body (the VM's iterator guard, the
//!   hub's correction on #785): a body that changed it is [`COMPONENTS_CHANGED`] (`NotRead`), marked "stopped after
//!   partial writes" (the body wrote). On the slice the body never runs (the pickup has no `WaypointComponent`).
//! - **`MakeNoise(Loudness, NoiseType)`** (3.4): nothing (it does nothing in this build).
//! - **`IsUsingVehicle(bCheckDrivenVehicle)`** (3.5): no `Pawn`: false. A pawn whose first native class is
//!   `WillowGame.WillowPlayerPawn`, `WillowGame.WillowPawn` or `GearboxFramework.GearboxPawn`: false, unless the flag is
//!   set with a `DrivenVehicle`, which is [`VEHICLE_ANSWER`] (`NotRead`: a vehicle's body was not read). Any other first
//!   native class: [`PAWN_ANSWER`] (`NotRead`). `Pawn` and `DrivenVehicle` declared as objects, else `VmError::Script`;
//!   a pawn the store has not loaded is [`NOT_LOADED`] (`NotRead`). It replaces `ow-combat`'s `NotRead` for this key.
//! - **`SetOwner(NewOwner)`** (3.7): `ow_world::actors::set_owner`'s rule, then, when that ran to its end, the two
//!   steps it leaves out: the owner-visibility refresh, then `bNetDirty`. The refresh: each primitive component (by the
//!   class table, `Engine.PrimitiveComponent` or below) in the actor's `AllComponents` with `bOnlyOwnerSee` or
//!   `bOwnerNoSee` set, or every primitive component when the actor's `bOnlyOwnerSee` is set, gets the deferred
//!   reattach (`component_setters`: its owner and scene refusals, then `bNeedsReattach`). The "marked" flag is by fit
//!   `bNeedsReattach`. **Planned before the first write**: the declarations (`AllComponents`, the three flags,
//!   `bNetDirty`, the reattach's fields) and every reattach refusal, only when the owner would change. After
//!   `LostChild` and `GainedChild` (script) the plan is made again on the state they left (the hub's correction on
//!   #785): a different set of components is [`REFRESH_CHANGED`] (`NotRead`), and the plan's own refusals then stand.
//!   A refusal after the first write carries the "stopped after partial writes" prefix (#518's counters).

use std::rc::Rc;

use ow_core::NotYet;
use ow_object::ids::{ClassId, ObjRef};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{IteratorFn, IteratorResult, NativeCall, NativeFn, Vm};
use ow_vm::error::VmError;

use super::component_setters::{REATTACH_FIELDS, Slots, Want, bad, bool_of, object_of, reattach_check, schemas, write};

/// The record of the rules.
pub const RECORD: &str = "docs/verification/NATIVE_UPDATE_AMMO_COUNTS_AND_TAKE.md";
/// The native keys.
pub const COMPONENT_LIST: &str = "Actor.ComponentList(class,object)";
pub const MAKE_NOISE: &str = "Actor.MakeNoise(float,name)";
pub const IS_USING_VEHICLE: &str = "Controller.IsUsingVehicle(bool)";
pub const SET_OWNER: &str = "Actor.SetOwner(object)";
/// `IsUsingVehicle`'s refusals.
pub const VEHICLE_ANSWER: &str = "Controller.IsUsingVehicle: the driven vehicle's own answer";
pub const PAWN_ANSWER: &str = "Controller.IsUsingVehicle: the vehicle answer of";
pub const NOT_LOADED: &str = "Controller.IsUsingVehicle: a pawn the store has not loaded (a load is a write)";
/// The pawn classes whose vehicle virtual answers none.
pub const ON_FOOT: [&str; 3] = ["WillowGame.WillowPlayerPawn", "WillowGame.WillowPawn", "GearboxFramework.GearboxPawn"];
const PRIMITIVE: &str = "Engine.PrimitiveComponent";
/// `ComponentList`'s refusal for a loop body that changed `Components`.
pub const COMPONENTS_CHANGED: &str = "ComponentList: the loop body changed Components";
/// `SetOwner`'s refusal when the owner events changed which components the refresh reattaches.
pub const REFRESH_CHANGED: &str = "Actor.SetOwner: LostChild or GainedChild changed the owner-visibility refresh";

fn read(vm: &Vm, o: ObjRef, name: &str, what: &str) -> Result<Value, VmError> {
    Slots::of(o).required(vm, name, what)
}

fn is_a(vm: &Vm, class: ClassId, path: &str) -> bool {
    vm.space.classes.by_path(path).is_some_and(|b| vm.space.classes.is_child_of(class, b))
}

/// `name` on `o` declared as an array of objects, its entries (None kept), else a data error.
fn objects(vm: &Vm, o: ObjRef, name: &str, what: &str) -> Result<Vec<Option<ObjRef>>, VmError> {
    match super::component_setters::declared(vm, o, name, what)? {
        PropKind::Array(inner) if matches!(*inner, PropKind::Object { .. }) => {}
        other => return Err(bad(what, format!("{name} is declared as {}, not an array of objects", other.kind_name()))),
    }
    let v = read(vm, o, name, what)?;
    v.elements().unwrap_or(&[]).iter().map(|e| object_of(e, what)).collect()
}

/// `ComponentList` (module documentation).
fn component_list() -> IteratorFn {
    Rc::new(|c: &mut NativeCall<'_>| {
        const WHAT: &str = "Actor.ComponentList";
        let actor = c.this.ok_or_else(|| bad(WHAT, "called without an actor"))?;
        let base = match c.arg(0)? {
            Value::Class(cl) => *cl,
            Value::Object(None) | Value::None => None,
            other => return Err(bad(WHAT, format!("BaseClass holds {}, not a class", other.kind_name()))),
        };
        let Some(base) = base else {
            return Ok(IteratorResult::new(1, Vec::new()));
        };
        let vm: &Vm = c.vm;
        let copy = objects(vm, actor, "Components", WHAT)?;
        let mut items = Vec::new();
        for entry in copy.iter().flatten() {
            let class = vm.space.heap.get(*entry)?.class;
            if vm.space.classes.is_child_of(class, base) {
                items.push(Value::Object(Some(*entry)));
            }
        }
        let guard: ow_vm::api::IteratorGuard = Rc::new(move |vm: &Vm| {
            if objects(vm, actor, "Components", WHAT)? != copy {
                return Err(NotYet::not_read(COMPONENTS_CHANGED, RECORD).after_partial_writes().into());
            }
            Ok(())
        });
        Ok(IteratorResult::new(1, items).with_guard(guard))
    })
}

/// `IsUsingVehicle` (module documentation). Reads only.
fn is_using_vehicle(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const WHAT: &str = "Controller.IsUsingVehicle";
    let controller = c.this.ok_or_else(|| bad(WHAT, "called without a controller"))?;
    let check_driven = bool_of(c.arg(0)?, "bCheckDrivenVehicle", WHAT)?;
    let vm: &Vm = c.vm;
    schemas(vm, controller, &[("Pawn", Want::Object)], WHAT)?;
    let Some(pawn) = object_of(&read(vm, controller, "Pawn", WHAT)?, WHAT)? else {
        return Ok(Value::Bool(false));
    };
    if crate::touch_trace::is_unloaded(vm, pawn) || vm.defs.is_failed(&vm.space, pawn) {
        return Err(NotYet::not_read(NOT_LOADED, RECORD).into());
    }
    let class = vm.space.heap.get(pawn)?.class;
    let native = super::render_hooks::first_native(vm, class);
    if !native.as_deref().is_some_and(|n| ON_FOOT.iter().any(|p| n.eq_ignore_ascii_case(p))) {
        let named = native.unwrap_or_else(|| ow_attr::eval::class_path(&vm.space, pawn));
        return Err(NotYet::not_read(format!("{PAWN_ANSWER} {named}"), RECORD).into());
    }
    schemas(vm, pawn, &[("DrivenVehicle", Want::Object)], WHAT)?;
    if check_driven && object_of(&read(vm, pawn, "DrivenVehicle", WHAT)?, WHAT)?.is_some() {
        return Err(NotYet::not_read(VEHICLE_ANSWER, RECORD).into());
    }
    Ok(Value::Bool(false))
}

/// The components `SetOwner`'s refresh reattaches (module documentation), each checked by the deferred reattach's
/// preflight. Reads only.
fn refresh_plan(vm: &Vm, actor: ObjRef) -> Result<Vec<ObjRef>, VmError> {
    const WHAT: &str = "Actor.SetOwner";
    schemas(vm, actor, &[("bOnlyOwnerSee", Want::Bool), ("bNetDirty", Want::Bool)], WHAT)?;
    let all = bool_of(&read(vm, actor, "bOnlyOwnerSee", WHAT)?, "bOnlyOwnerSee", WHAT)?;
    let mut planned = Vec::new();
    for component in objects(vm, actor, "AllComponents", WHAT)?.into_iter().flatten() {
        let class = vm.space.heap.get(component)?.class;
        if !is_a(vm, class, PRIMITIVE) {
            continue;
        }
        schemas(vm, component, &[("bOnlyOwnerSee", Want::Bool), ("bOwnerNoSee", Want::Bool)], WHAT)?;
        let own = |name: &str| -> Result<bool, VmError> { bool_of(&read(vm, component, name, WHAT)?, name, WHAT) };
        if !(all || own("bOnlyOwnerSee")? || own("bOwnerNoSee")?) {
            continue;
        }
        schemas(vm, component, &REATTACH_FIELDS, WHAT)?;
        reattach_check(vm, Slots::of(component), WHAT)?;
        planned.push(component);
    }
    Ok(planned)
}

/// `SetOwner` (module documentation).
fn set_owner_native(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const WHAT: &str = "Actor.SetOwner";
    let actor = c.this.ok_or_else(|| bad(WHAT, "called without an actor"))?;
    let owner = object_of(c.arg(0)?, WHAT)?;
    if crate::actors::set_owner_is_noop(c.vm, actor, owner)? {
        return Ok(Value::None);
    }
    let refresh = refresh_plan(c.vm, actor)?;
    let before = crate::pools::Counters::read(c.vm);
    let done = (|| -> Result<(), VmError> {
        if !crate::actors::set_owner_completed(c.vm, actor, owner)? {
            return Ok(());
        }
        // The refresh on the state the owner events left: the same components, or the refusal.
        if refresh_plan(c.vm, actor)? != refresh {
            return Err(NotYet::not_read(REFRESH_CHANGED, RECORD).into());
        }
        for component in refresh {
            write(c.vm, component, "bNeedsReattach", Value::Bool(true), WHAT)?;
        }
        write(c.vm, actor, "bNetDirty", Value::Bool(true), WHAT)
    })();
    done.map_err(|e| crate::pools::after_counted_writes(c.vm, before, e))?;
    Ok(Value::None)
}

/// Binds the four keys (Off only; the caller checks the mode). No pack binds them since `ow-combat` gave up its
/// `IsUsingVehicle` placeholder, so a duplicate is a registration error.
pub(crate) fn register(vm: &mut Vm) -> Result<(), VmError> {
    let natives: [(&str, NativeFn); 3] = [
        (MAKE_NOISE, Rc::new(|_: &mut NativeCall<'_>| Ok(Value::None))),
        (IS_USING_VEHICLE, Rc::new(is_using_vehicle)),
        (SET_OWNER, Rc::new(set_owner_native)),
    ];
    for (key, f) in natives {
        vm.register_native(key, f)?;
    }
    vm.register_iterator(COMPONENT_LIST, component_list())
}

#[cfg(test)]
#[path = "actor_take_tests.rs"]
mod tests;
