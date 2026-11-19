//! Owner: lane IMPL-ALL-NAVIGATION-POINTS (decision 2026-11-19-NNN, number pending). The engine iterator
//! `WorldInfo.AllNavigationPoints(BaseClass, out N)`, link 1's stop in `GameInfo.FindPlayerStart` (a `Teleporter` walk)
//! and `GameInfo.ChoosePlayerStart` (a `PlayerStart` walk). AI-assisted (Claude Code).
//!
//! **Sources.** The stock UE3 iterator as publicly documented, the installed script read with
//! `research/script_disasm.py` (the two callers) and the serialized fields `WorldInfo.NavigationPointList` and
//! `NavigationPoint.nextNavigationPoint`. The executable was not read for it (the hub's brief). **Every rule is
//! UNVERIFIED in the running game.** Off only: `World::new` binds it outside Reference, and Reference keeps the VM's stub
//! policy.
//!
//! **The rule coded** (the stock iterator): start at the receiver's `NavigationPointList`, follow each point's
//! `nextNavigationPoint`, and yield each point whose class is `BaseClass` or a subclass of it, in list order. The loop
//! binds `N` (index 1) and the script body runs once per yield. The list is taken once, when the loop starts; the stock
//! iterator follows `nextNavigationPoint` step by step, which differs only if a loop body relinks the list, and neither
//! caller's body writes a list field.
//!
//! **What the data and script do not settle**, each a `NotYet { NotRead }` with the exact text below, raised before the
//! loop runs (the iterator writes nothing, so nothing needs a preflight or a rollback; a caller that wrote first adds
//! the "stopped after partial writes: " prefix, as `SetupInputDevices` does):
//! - [`NONE_BASE`]: a None `BaseClass` (both callers pass a class constant);
//! - [`DELETED`]: a list entry that is destroyed, or has `bDeleteMe` or `bPendingDelete` set (whether the walk skips
//!   it; `Actor.Destroy` sets `bPendingDelete` before `Destroyed()` runs, while the actor is still placed);
//! - [`LOOP`]: a list that comes back to a point already visited (a broken, cyclic list);
//! - [`NOT_A_POINT`]: an entry that is not a `NavigationPoint` (a dangling or wrong-class reference);
//! - [`UNPLACED`]: an entry the level's actor list does not hold (a reference to an object the level never placed);
//! - [`UNREACHED`]: a placed, live navigation point the list does not reach. That covers an empty list in a level with
//!   points (whether the engine fills the list at load) and the streamed levels' points (whether their lists are joined
//!   to the persistent one, and in what order).
//!
//! An empty list in a level that places no navigation point yields nothing, which every reading agrees on. That is the
//! slice's case: the drivers' worlds load no map, so their `WorldInfo` has no list and the level no points.

use std::rc::Rc;

use ow_core::{DetSet, NotYet};
use ow_object::heap::ObjectSpace;
use ow_object::ids::{ClassId, ObjRef, PropAddr};
use ow_object::value::Value;
use ow_vm::api::{IteratorResult, NativeCall, Vm};
use ow_vm::error::VmError;

use crate::actors::{actors_of, is_destroyed};

/// The native key, as the installed declaration carries it.
pub const KEY: &str = "WorldInfo.AllNavigationPoints(class,object)";
/// The record these rules come from (no analyst note: the stock iterator, the packages and the script).
pub const RECORD: &str = "docs/decisions/2026-11/2026-11-19-NNN-all-navigation-points-in-off.md";
/// The navigation point base class.
pub const NAVIGATION_POINT: &str = "Engine.NavigationPoint";

/// The open questions, as the `NotYet` texts name them (module documentation).
pub const NONE_BASE: &str = "WorldInfo.AllNavigationPoints with a None BaseClass";
pub const DELETED: &str =
    "WorldInfo.AllNavigationPoints: whether a destroyed, bDeleteMe or bPendingDelete navigation point is visited";
pub const LOOP: &str = "WorldInfo.AllNavigationPoints: a navigation list that loops back to a visited point";
pub const NOT_A_POINT: &str = "WorldInfo.AllNavigationPoints: a navigation list entry that is not a NavigationPoint";
pub const UNPLACED: &str = "WorldInfo.AllNavigationPoints: a navigation list entry the level's actor list does not hold";
pub const UNREACHED: &str = "WorldInfo.AllNavigationPoints: placed navigation points the WorldInfo's list does not reach \
                             (an empty or partial list; the streamed levels' lists)";

const UNIT: &str = "WorldInfo.AllNavigationPoints";

fn bad(message: String) -> VmError {
    VmError::Script { function: UNIT.to_string(), message }
}

fn not_read(what: &'static str) -> VmError {
    NotYet::not_read(what, RECORD).into()
}

/// Binds the iterator by key (Off only; the caller checks the mode).
pub(crate) fn register(vm: &mut Vm) -> Result<(), VmError> {
    vm.register_iterator(KEY, Rc::new(all_navigation_points))
}

fn all_navigation_points(c: &mut NativeCall<'_>) -> Result<IteratorResult, VmError> {
    let me = c.this.ok_or_else(|| bad("called with no WorldInfo".to_string()))?;
    let base = match c.arg(0)? {
        Value::Class(k) => *k,
        Value::Object(None) | Value::None => None,
        other => return Err(bad(format!("BaseClass holds {}, not a class", other.kind_name()))),
    };
    let base = base.ok_or_else(|| not_read(NONE_BASE))?;
    let placed = actors_of(c.vm)?.try_borrow().map_err(|_| VmError::ServiceBorrowed("Actors"))?.list().to_vec();
    let points = navigation_points(&c.vm.space, me, &placed)?;
    let items = points
        .into_iter()
        .filter(|&(_, class)| c.vm.space.classes.is_child_of(class, base))
        .map(|(o, _)| Value::Object(Some(o)))
        .collect();
    Ok(IteratorResult::new(1, items))
}

/// The receiver's navigation list in order, each point with its class, checked against the level's actor list
/// `placed` (module documentation). Reads only.
pub fn navigation_points(
    space: &ObjectSpace,
    world_info: ObjRef,
    placed: &[ObjRef],
) -> Result<Vec<(ObjRef, ClassId)>, VmError> {
    let nav = space.classes.by_path(NAVIGATION_POINT);
    let is_point = |class: ClassId| nav.is_some_and(|n| space.classes.is_child_of(class, n));
    let mut cur = object_field(space, world_info, "NavigationPointList")?;
    // Review r1 of #513: sets for the membership tests; the vector keeps the yield order.
    let placed_set: DetSet<ObjRef> = placed.iter().copied().collect();
    let mut visited: DetSet<ObjRef> = DetSet::new();
    let mut out: Vec<(ObjRef, ClassId)> = Vec::new();
    while let Some(o) = cur {
        if !visited.insert(o) {
            return Err(not_read(LOOP));
        }
        if !space.heap.contains(o) {
            return Err(not_read(UNPLACED));
        }
        let class = space.heap.get(o)?.class;
        if !is_point(class) {
            return Err(not_read(NOT_A_POINT));
        }
        if !placed_set.contains(&o) {
            return Err(not_read(UNPLACED));
        }
        if going(space, o)? {
            return Err(not_read(DELETED));
        }
        out.push((o, class));
        cur = object_field(space, o, "nextNavigationPoint")?;
    }
    for &o in placed {
        if going(space, o)? {
            continue;
        }
        if is_point(space.heap.get(o)?.class) && !visited.contains(&o) {
            return Err(not_read(UNREACHED));
        }
    }
    Ok(out)
}

/// Whether `o` is destroyed (freed, the heap's destroyed bit, `bDeleteMe`) or being destroyed: `Actor.Destroy` sets
/// `bPendingDelete` before it calls `Destroyed()`, while the actor is still placed (review r1 of #513). A class that
/// does not declare `bPendingDelete` has no such state.
fn going(space: &ObjectSpace, o: ObjRef) -> Result<bool, VmError> {
    if is_destroyed(space, o)? {
        return Ok(true);
    }
    let class = space.heap.get(o)?.class;
    match space.prop(class, "bPendingDelete") {
        Some(p) => match space.read(o, PropAddr::scalar(p))? {
            Value::Bool(b) => Ok(*b),
            other => Err(bad(format!("bPendingDelete holds {}, not a bool", other.kind_name()))),
        },
        None => Ok(false),
    }
}

/// An object field of `o`; a class without it, or a value that is not an object, is a data error.
fn object_field(space: &ObjectSpace, o: ObjRef, name: &str) -> Result<Option<ObjRef>, VmError> {
    let class = space.heap.get(o)?.class;
    let p = space
        .prop(class, name)
        .ok_or_else(|| bad(format!("{} declares no {name}", ow_attr::eval::class_path(space, o))))?;
    match space.read(o, PropAddr::scalar(p))? {
        Value::Object(v) => Ok(*v),
        other => Err(bad(format!("{name} holds {}, not an object", other.kind_name()))),
    }
}

#[cfg(test)]
#[path = "nav_points_tests.rs"]
mod tests;
