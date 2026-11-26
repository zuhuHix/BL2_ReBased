//! Owner: lane IMPL-INVENTORY-ACTORS. `Engine.InventoryManager.InventoryActors(BaseClass, out Inv, optional
//! bOnlyReadied)`, an iterator, in `ParityMode::Off`: link 6's next stop (`WillowInventoryManager.HandlePickupQuery`
//! walks the weapon chain with it). AI-assisted (Claude Code).
//!
//! **Clean room.** Implemented from docs/verification/NATIVE_INVENTORY_ACTORS.md section 3 alone (#725 at its review
//! r1 head 64b849c3, lane ANALYST-INVENTORY-ACTORS, decision 2026-11-25-60); this lane did not read the executable.
//! **Every rule is UNVERIFIED in the running game.** Reference binds nothing (the stub policy).
//!
//! **The list**, read only: nothing is loaded, written or interned, and no class code is loaded:
//! 1. `BaseClass` in any of the VM's null forms means `Engine.Inventory`, found load-free with
//!    `space.classes.by_path` (never `Vm::find_class`, which can load class code). Not laid out yet:
//!    [`NO_FALLBACK`] (`NotRead`). Any other kind that is not a class: `VmError::Script`. `bOnlyReadied` omitted is
//!    false;
//! 2. the receiver is an `Engine.InventoryManager` or below it, and declares `InventoryChain` as an object reference.
//!    Otherwise: `VmError::Script`;
//! 3. the walk runs from `InventoryChain` through each entry's `Inventory`, in chain order. It examines at most
//!    [`EXAMINE_CAP`] (100) entries, counting each one, given or skipped, over the whole walk. The 101st is not
//!    tested;
//! 4. per entry: a store handle not loaded yet (`DefStore::is_unloaded`, or a slotless stand-in) is [`NOT_LOADED`]
//!    (`NotRead`), before any field of the entry is read, its class included. An entry is given when its class is
//!    `BaseClass` or below it. `bReadied` is read only when `bOnlyReadied` is set and the class matches. A missing
//!    `Inventory` or `bReadied`, or one of the wrong kind, on an entry that is read: `VmError::Script` naming it;
//! 5. **no revisit refusal.** A chain that loops is given as far as the cap allows, entries repeating. This is the
//!    one place this differs from the `ItemActors` port.
//!
//! **The snapshot** (the port's iterator model, as `ItemActors`, UNVERIFIED): the list is taken whole when the loop
//! starts, and the VM's `IteratorNext` skips objects destroyed since then. The game reads each next link just before
//! running the body. The two differ only when the body changes the chain past the current entry. Link 6's body,
//! `DenyPickupQuery`, changes nothing.

use std::rc::Rc;

use ow_core::NotYet;
use ow_object::ids::{ClassId, ObjRef, PropAddr};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{IteratorFn, IteratorResult, NativeCall, Vm};
use ow_vm::error::VmError;

use crate::equip::bad;

/// The script function.
pub const INVENTORY_ACTORS: &str = "Engine.InventoryManager.InventoryActors";
/// The record of the rule.
pub const RECORD: &str = "docs/verification/NATIVE_INVENTORY_ACTORS.md";
/// The walk examines at most this many entries, given or skipped, over the whole iteration (section 2, item 4).
pub const EXAMINE_CAP: usize = 100;
/// A None `BaseClass` while `Engine.Inventory` is not laid out.
pub const NO_FALLBACK: &str =
    "InventoryActors with a None BaseClass: Engine.Inventory is not laid out at set-up (a class load is a write)";
/// A chain entry the store has not loaded.
pub const NOT_LOADED: &str = "InventoryActors over an inventory entry not loaded at set-up (a load is a write)";
const WHAT: &str = "InventoryActors";
const FALLBACK: &str = "Engine.Inventory";
const MANAGER: &str = "Engine.InventoryManager";

/// The object `o` holds in `name`, which its class must declare as an object reference.
fn object_link(vm: &Vm, o: ObjRef, name: &str) -> Result<Option<ObjRef>, VmError> {
    let space = &vm.space;
    let class = space.heap.get(o)?.class;
    let path = || ow_attr::eval::class_path(space, o);
    let p = space.prop(class, name).ok_or_else(|| bad(WHAT, format!("{} declares no {name}", path())))?;
    if !matches!(space.classes.decl(p).map(|d| &d.kind), Some(PropKind::Object { .. })) {
        return Err(bad(WHAT, format!("{} declares {name} as something other than an object reference", path())));
    }
    match space.read(o, PropAddr::scalar(p))? {
        Value::Object(x) => Ok(*x),
        other => Err(bad(WHAT, format!("{name} holds {}, not an object", other.kind_name()))),
    }
}

/// `entry`'s `bReadied`, which its class must declare as a bool.
fn readied(vm: &Vm, entry: ObjRef) -> Result<bool, VmError> {
    let space = &vm.space;
    let class = space.heap.get(entry)?.class;
    let path = || ow_attr::eval::class_path(space, entry);
    let p = space.prop(class, "bReadied").ok_or_else(|| bad(WHAT, format!("{} declares no bReadied", path())))?;
    if !matches!(space.classes.decl(p).map(|d| &d.kind), Some(PropKind::Bool)) {
        return Err(bad(WHAT, format!("{} declares bReadied as something other than a bool", path())));
    }
    match space.read(entry, PropAddr::scalar(p))? {
        Value::Bool(b) => Ok(*b),
        other => Err(bad(WHAT, format!("bReadied holds {}, not a bool", other.kind_name()))),
    }
}

/// A store handle not loaded yet, or a slotless stand-in: [`NOT_LOADED`]. Reads the heap and the store only. The
/// stand-in test is `ow-attr`'s `slotless_refusal` one (review of #736): an `Origin::Resource` object with no slots.
fn loaded(vm: &Vm, entry: ObjRef) -> Result<(), VmError> {
    let o = vm.space.heap.get(entry)?;
    let slotless = matches!(o.origin, ow_object::heap::Origin::Resource { .. }) && o.slots.is_empty();
    if vm.defs.is_unloaded(&vm.space, entry) || slotless {
        return Err(NotYet::not_read(NOT_LOADED, RECORD).into());
    }
    Ok(())
}

/// The entries `InventoryActors` gives on `manager` for `base` (None: `Engine.Inventory`) and `only_readied`
/// (module documentation).
pub fn inventory_actors(
    vm: &Vm,
    manager: ObjRef,
    base: Option<ClassId>,
    only_readied: bool,
) -> Result<Vec<ObjRef>, VmError> {
    let base = match base {
        Some(b) => b,
        None => vm.space.classes.by_path(FALLBACK).ok_or_else(|| VmError::from(NotYet::not_read(NO_FALLBACK, RECORD)))?,
    };
    let class = vm.space.heap.get(manager)?.class;
    let is_manager = vm.space.classes.by_path(MANAGER).is_some_and(|m| vm.space.classes.is_child_of(class, m));
    if !is_manager {
        return Err(bad(WHAT, format!("{} is not an {MANAGER}", ow_attr::eval::class_path(&vm.space, manager))));
    }
    let mut cursor = object_link(vm, manager, "InventoryChain")?;
    let mut out = Vec::new();
    let mut examined = 0usize;
    while let Some(entry) = cursor {
        if examined == EXAMINE_CAP {
            break;
        }
        examined += 1;
        loaded(vm, entry)?;
        let class = vm.space.heap.get(entry)?.class;
        let given = vm.space.classes.is_child_of(class, base) && (!only_readied || readied(vm, entry)?);
        if given {
            out.push(entry);
        }
        cursor = object_link(vm, entry, "Inventory")?;
    }
    Ok(out)
}

/// The iterator body (the out argument is parameter 1).
fn iterator() -> IteratorFn {
    Rc::new(|call: &mut NativeCall<'_>| {
        let m = call.this.ok_or_else(|| bad(WHAT, "called without an inventory manager"))?;
        let base = match call.arg(0)? {
            Value::Class(c) => *c,
            Value::Object(None) | Value::Interface(None) | Value::None => None,
            other => return Err(bad(WHAT, format!("BaseClass holds {}, not a class", other.kind_name()))),
        };
        let only_readied = if call.has(2) {
            match call.arg(2)? {
                Value::Bool(b) => *b,
                other => return Err(bad(WHAT, format!("bOnlyReadied holds {}, not a bool", other.kind_name()))),
            }
        } else {
            false
        };
        let items = inventory_actors(call.vm, m, base, only_readied)?;
        Ok(IteratorResult::new(1, items.into_iter().map(|o| Value::Object(Some(o))).collect()))
    })
}

/// Binds `InventoryActors` in `ParityMode::Off` under its declaration's own native key (a VM without the declaration
/// binds nothing); Reference binds nothing.
pub fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ow_core::ParityMode::Off {
        return Ok(());
    }
    let Ok(id) = vm.find_function(INVENTORY_ACTORS) else {
        return Ok(());
    };
    if let Some(key) = vm.function(id).and_then(|d| d.native_key.clone()) {
        vm.register_iterator(&key, iterator())?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "inventory_actors_tests.rs"]
mod tests;
