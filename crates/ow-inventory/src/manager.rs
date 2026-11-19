//! Owner: lane W5-EQUIP. The inventory manager natives, the inventory engine queries and this crate's native pack.
//! AI-assisted (Claude Code).
//!
//! Rules from NATIVE_INVENTORY_EQUIP.md (read from the executable by lane G12; field names checked against the
//! installed classes through the Rust reflection, 2026-11-16) and NATIVE_ATTRIBUTES.md section 5. None was checked in
//! the running game: **UNVERIFIED** throughout.
//!
//! **Natives of `WillowInventoryManager`** ([`register_natives`]):
//! - `CountUnreadiedInventory()` answers the stored `BackpackInventoryCount` (not the backpack's length), and
//!   `GetUnreadiedInventoryMaxSize()` the stored `InventorySlotMax_Misc`.
//! - `SetInventoryMaxSize(NewSize, optional bOverrideDefaultMin)` stores [`MIN_INVENTORY_SIZE`] (12) for a size below
//!   it unless `bOverrideDefaultMin`, else the size as given.
//! - `CountReadiedWeapons()` walks `InventoryChain` through each entry's `Inventory` and counts the `WillowWeapon`
//!   entries whose `bReadied` is set; a chain that comes back to an entry it already passed answers -1 (the record:
//!   "a chain that loops back to itself yields -1"; which revisit the native tests is not recorded, and any revisit is
//!   taken here).
//! - `GetWeaponReadyMax(optional bWantBaseValue)` answers the stored `WeaponReadyMax` value, the flag ignored.
//! - `ItemActors(BaseClass, out Inv, optional bOnlyReadied)`, an iterator, in `ParityMode::Off` only (lane
//!   IMPL-INVENTORY-CONTEXT, decision 2026-11-19-67, coded from NATIVE_INVENTORY_CONTEXT.md section 3 alone): the
//!   manager's `ItemChain` from its head through each entry's `Inventory` link, in chain order. An entry is given when
//!   its class is `BaseClass` or below it and, with `bOnlyReadied` set, its `bReadied` is set. The walk counts the
//!   entries it skips over the whole iteration and ends after more than [`ITEM_ACTORS_SKIP_CAP`] (100) of them. A None
//!   `BaseClass`, in any of the VM's null forms, is `NotYet { NotRead }`: the engine falls back to a class it looks up, which the note does not
//!   identify. A chain that comes back to an entry it already passed is a data error (a Rust-only refusal: the note
//!   does not say what the walk does there). The VM's iterator takes the whole list when the loop starts, so a body
//!   that changes the chain during the loop does not change what is visited (as `AllParts`; UNVERIFIED). Reference
//!   leaves it unbound (rule R3: the C++ has no reading).
//! - Not registered, so a gameplay run stops at them (`UnboundNative`): `FindLeastValuableWeapon` / `Item` (the
//!   per-class eligibility virtual is not read), the holster natives (presentation).
//!
//! **Engine queries** ([`register_queries`], the kinds W5-WORLD keys by [`QueryKind`]):
//! - `EquippedInventory { location }`: the source's pawn (a controller's `Pawn`, or the pawn itself) must hold a
//!   `WillowInventoryManager` in `InvManager` (`GetPawnInventoryManager`'s rule, NATIVE_CONTROLLER_HELPERS.md); the
//!   answer is the pawn's `EquippedItems[location]`, the slot `WillowPawn.EquipItem` fills (locations 0 to 3; 4 is
//!   "none" and answers None). The record says "the item the source's inventory manager holds at the location";
//!   that the manager's lookup lands on this array is this reading, UNVERIFIED. A source that is neither a controller
//!   nor a pawn is `NotYet { NotRead }`.
//! - `Inventory { definition, equipped_only }`: the record names "a lookup through the source's interface" (low
//!   confidence) and nothing more, so the handler is `NotYet { NotRead }`. (W3-ATTRCTX's resolver answers `NotYet`
//!   before it asks, for the same reason.)
//!
//! **The pack** ([`Pack`]) registers the natives, the queries (on a VM with W5-WORLD's query service), the two script
//! overrides of `crate::equip`, the Off attribute and slot natives of `crate::equip::natives`, and the spawn natives
//! of `crate::spawn`.

use std::rc::Rc;

use ow_attr::api::{AttrError, EngineQuery, WorldParts};
use ow_core::{DetSet, NotYet, ParityMode};
use ow_object::heap::ObjectSpace;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_vm::api::{IteratorFn, IteratorResult, NativeCall, NativeFn, NativePack, Vm};
use ow_vm::error::VmError;
use ow_world::query::{QueryFn, QueryKind, WorldQuery};

use crate::equip::{bad, object_of};

/// The records this module reads.
pub const RECORD: &str = "NATIVE_INVENTORY_EQUIP.md (inventory manager natives); NATIVE_ATTRIBUTES.md section 5";

/// The smallest backpack `SetInventoryMaxSize` stores unless told otherwise.
pub const MIN_INVENTORY_SIZE: i32 = 12;
/// The gear slots of `WillowPawn.EquippedItems` (shield 0, grenade mod 1, class mod 2, relic 3).
pub const EQUIPMENT_SLOTS: u8 = 4;
/// A chain walk refuses past this many entries (a Rust-only bound; a readied set holds at most four weapons and the
/// chain holds what the pawn carries).
pub const MAX_CHAIN: usize = 1 << 16;

const MANAGER: &str = "WillowGame.WillowInventoryManager";

/// `vm`'s object `this` must be a `WillowInventoryManager`.
fn manager_of(vm: &mut Vm, this: Option<ObjRef>, what: &str) -> Result<ObjRef, VmError> {
    let m = this.ok_or_else(|| bad(what, "called without an inventory manager"))?;
    let class = vm.space.heap.get(m)?.class;
    let willow = vm.find_class(MANAGER)?;
    if !vm.space.classes.is_child_of(class, willow) {
        return Err(bad(what, format!("{} is not a {MANAGER}", ow_attr::eval::class_path(&vm.space, m))));
    }
    Ok(m)
}

fn int_prop(space: &ObjectSpace, o: ObjRef, name: &str, what: &str) -> Result<i32, VmError> {
    let class = space.heap.get(o)?.class;
    let p = space
        .prop(class, name)
        .ok_or_else(|| bad(what, format!("{} declares no {name}", ow_attr::eval::class_path(space, o))))?;
    match space.read(o, PropAddr::scalar(p))? {
        Value::Int(i) => Ok(*i),
        other => Err(bad(what, format!("{name} holds {}, not an int", other.kind_name()))),
    }
}

/// `CountUnreadiedInventory`: the stored `BackpackInventoryCount`.
pub fn count_unreadied_inventory(space: &ObjectSpace, manager: ObjRef) -> Result<i32, VmError> {
    int_prop(space, manager, "BackpackInventoryCount", "CountUnreadiedInventory")
}

/// `GetUnreadiedInventoryMaxSize`: the stored `InventorySlotMax_Misc`.
pub fn unreadied_inventory_max_size(space: &ObjectSpace, manager: ObjRef) -> Result<i32, VmError> {
    int_prop(space, manager, "InventorySlotMax_Misc", "GetUnreadiedInventoryMaxSize")
}

/// The size `SetInventoryMaxSize` stores for `new_size`.
pub fn stored_inventory_size(new_size: i32, override_default_min: bool) -> i32 {
    if !override_default_min && new_size < MIN_INVENTORY_SIZE {
        MIN_INVENTORY_SIZE
    } else {
        new_size
    }
}

/// `SetInventoryMaxSize`: writes [`stored_inventory_size`] into `InventorySlotMax_Misc`.
pub fn set_inventory_max_size(
    space: &mut ObjectSpace,
    manager: ObjRef,
    new_size: i32,
    override_default_min: bool,
) -> Result<(), VmError> {
    const WHAT: &str = "SetInventoryMaxSize";
    int_prop(space, manager, "InventorySlotMax_Misc", WHAT)?;
    let class = space.heap.get(manager)?.class;
    let p = space
        .prop(class, "InventorySlotMax_Misc")
        .ok_or_else(|| bad(WHAT, "no InventorySlotMax_Misc"))?;
    space.write(
        manager,
        PropAddr::scalar(p),
        Value::Int(stored_inventory_size(new_size, override_default_min)),
    )?;
    Ok(())
}

fn object_field(space: &ObjectSpace, o: ObjRef, name: &str, what: &str) -> Result<Option<ObjRef>, VmError> {
    let class = space.heap.get(o)?.class;
    match space.prop(class, name) {
        Some(p) => object_of(space.read(o, PropAddr::scalar(p))?, what),
        None => Ok(None),
    }
}

/// `CountReadiedWeapons` (module documentation); `weapon` is the `WillowWeapon` class.
pub fn count_readied_weapons(
    space: &ObjectSpace,
    manager: ObjRef,
    weapon: ow_object::ids::ClassId,
) -> Result<i32, VmError> {
    const WHAT: &str = "CountReadiedWeapons";
    let mut seen: DetSet<ObjRef> = DetSet::new();
    let mut cursor = object_field(space, manager, "InventoryChain", WHAT)?;
    let mut count: i32 = 0;
    while let Some(inv) = cursor {
        if !seen.insert(inv) {
            return Ok(-1);
        }
        if seen.len() > MAX_CHAIN {
            return Err(bad(WHAT, format!("an inventory chain longer than {MAX_CHAIN} entries")));
        }
        let class = space.heap.get(inv)?.class;
        if space.classes.is_child_of(class, weapon) {
            let readied = match space.prop(class, "bReadied") {
                Some(p) => matches!(space.read(inv, PropAddr::scalar(p))?, Value::Bool(true)),
                None => false,
            };
            if readied {
                count = count
                    .checked_add(1)
                    .ok_or_else(|| bad(WHAT, "a readied count past i32"))?;
            }
        }
        cursor = object_field(space, inv, "Inventory", WHAT)?;
    }
    Ok(count)
}

/// `GetWeaponReadyMax`: the stored value of the `WeaponReadyMax` attribute (an int attribute's value property).
pub fn weapon_ready_max(space: &ObjectSpace, manager: ObjRef) -> Result<i32, VmError> {
    int_prop(space, manager, "WeaponReadyMax", "GetWeaponReadyMax")
}

fn native(vm: &mut Vm, key: &str, f: impl Fn(&mut NativeCall<'_>) -> Result<Value, VmError> + 'static) -> Result<(), VmError> {
    let f: NativeFn = Rc::new(f);
    vm.register_native(key, f)
}

fn bool_arg(call: &NativeCall<'_>, i: usize) -> Result<bool, VmError> {
    if !call.has(i) {
        return Ok(false);
    }
    match call.arg(i)? {
        Value::Bool(b) => Ok(*b),
        other => Err(bad("WillowInventoryManager", format!("expected a bool, found {}", other.kind_name()))),
    }
}

/// Registers the five manager natives (keys as the loader derives them from the installed `WillowGame.upk`).
pub fn register_natives(vm: &mut Vm) -> Result<(), VmError> {
    native(vm, "WillowInventoryManager.CountUnreadiedInventory()", |c| {
        let m = manager_of(c.vm, c.this, "CountUnreadiedInventory")?;
        Ok(Value::Int(count_unreadied_inventory(&c.vm.space, m)?))
    })?;
    native(vm, "WillowInventoryManager.GetUnreadiedInventoryMaxSize()", |c| {
        let m = manager_of(c.vm, c.this, "GetUnreadiedInventoryMaxSize")?;
        Ok(Value::Int(unreadied_inventory_max_size(&c.vm.space, m)?))
    })?;
    native(vm, "WillowInventoryManager.SetInventoryMaxSize(int,bool)", |c| {
        let m = manager_of(c.vm, c.this, "SetInventoryMaxSize")?;
        let size = match c.arg(0)? {
            Value::Int(i) => *i,
            other => return Err(bad("SetInventoryMaxSize", format!("NewSize holds {}", other.kind_name()))),
        };
        let keep = bool_arg(c, 1)?;
        set_inventory_max_size(&mut c.vm.space, m, size, keep)?;
        Ok(Value::None)
    })?;
    native(vm, "WillowInventoryManager.CountReadiedWeapons()", |c| {
        let m = manager_of(c.vm, c.this, "CountReadiedWeapons")?;
        let weapon = c.vm.find_class("WillowGame.WillowWeapon")?;
        Ok(Value::Int(count_readied_weapons(&c.vm.space, m, weapon)?))
    })?;
    native(vm, "WillowInventoryManager.GetWeaponReadyMax(bool)", |c| {
        let m = manager_of(c.vm, c.this, "GetWeaponReadyMax")?;
        Ok(Value::Int(weapon_ready_max(&c.vm.space, m)?))
    })
}

/// Whether `o`'s class is the class at `path` or below it; a class the space has not laid out is not (the query
/// handlers see only the space).
fn is_a(space: &ObjectSpace, o: ObjRef, path: &str) -> Result<bool, AttrError> {
    let class = space.heap.get(o)?.class;
    Ok(match space.classes.by_path(path) {
        Some(base) => space.classes.is_child_of(class, base),
        None => false,
    })
}

fn attr_object(space: &ObjectSpace, o: ObjRef, name: &str) -> Result<Option<ObjRef>, AttrError> {
    let class = space.heap.get(o)?.class;
    match space.prop(class, name) {
        Some(p) => match space.read(o, PropAddr::scalar(p))? {
            Value::Object(x) => Ok(*x),
            other => Err(AttrError::BadData {
                reason: format!("{name} holds {}, not an object", other.kind_name()).into(),
            }),
        },
        None => Ok(None),
    }
}

/// The script function of the item iterator (NATIVE_INVENTORY_CONTEXT.md section 3).
pub const ITEM_ACTORS: &str = "WillowGame.WillowInventoryManager.ItemActors";
/// The walk ends after more than this many skipped entries, counted over the whole iteration (section 3).
pub const ITEM_ACTORS_SKIP_CAP: usize = 100;

/// The entries `ItemActors` gives for `manager`, `base` (the `BaseClass` argument) and `only_readied` (module
/// documentation).
pub fn item_actors(
    space: &ObjectSpace,
    manager: ObjRef,
    base: Option<ow_object::ids::ClassId>,
    only_readied: bool,
) -> Result<Vec<ObjRef>, VmError> {
    const WHAT: &str = "ItemActors";
    let Some(base) = base else {
        return Err(NotYet::not_read(
            "ItemActors with a None BaseClass (the class the engine falls back to is not identified)",
            "NATIVE_INVENTORY_CONTEXT.md section 3",
        )
        .into());
    };
    let mut seen: DetSet<ObjRef> = DetSet::new();
    let mut out = Vec::new();
    let mut skipped = 0usize;
    let mut cursor = object_field(space, manager, "ItemChain", WHAT)?;
    while let Some(entry) = cursor {
        if !seen.insert(entry) {
            return Err(bad(WHAT, "the item chain comes back to an entry it already passed"));
        }
        if seen.len() > MAX_CHAIN {
            return Err(bad(WHAT, format!("an item chain longer than {MAX_CHAIN} entries")));
        }
        let class = space.heap.get(entry)?.class;
        let mut given = space.classes.is_child_of(class, base);
        if given && only_readied {
            let p = space.prop(class, "bReadied").ok_or_else(|| {
                bad(WHAT, format!("{} declares no bReadied", ow_attr::eval::class_path(space, entry)))
            })?;
            given = match space.read(entry, PropAddr::scalar(p))? {
                Value::Bool(b) => *b,
                other => return Err(bad(WHAT, format!("bReadied holds {}, not a bool", other.kind_name()))),
            };
        }
        if given {
            out.push(entry);
        } else {
            skipped += 1;
            if skipped > ITEM_ACTORS_SKIP_CAP {
                break;
            }
        }
        cursor = object_field(space, entry, "Inventory", WHAT)?;
    }
    Ok(out)
}

/// The `ItemActors` iterator body (the out argument is parameter 1).
fn item_actors_iterator() -> IteratorFn {
    Rc::new(|call: &mut NativeCall<'_>| {
        let m = manager_of(call.vm, call.this, "ItemActors")?;
        // The VM's null forms are all a None class (review r1 of #455): a literal None reaches a class parameter as
        // a None object reference, and an omitted value as a bare None.
        let base = match call.arg(0)? {
            Value::Class(c) => *c,
            Value::Object(None) | Value::Interface(None) | Value::None => None,
            other => return Err(bad("ItemActors", format!("BaseClass holds {}, not a class", other.kind_name()))),
        };
        let only_readied = bool_arg(call, 2)?;
        let items = item_actors(&call.vm.space, m, base, only_readied)?;
        Ok(IteratorResult::new(1, items.into_iter().map(|o| Value::Object(Some(o))).collect()))
    })
}

/// Binds `ItemActors` in `ParityMode::Off` under its declaration's own native key (a VM without the declaration binds
/// nothing); Reference binds nothing (module documentation).
pub fn register_iterators(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    let Ok(id) = vm.find_function(ITEM_ACTORS) else {
        return Ok(());
    };
    if let Some(key) = vm.function(id).and_then(|d| d.native_key.clone()) {
        vm.register_iterator(&key, item_actors_iterator())?;
    }
    Ok(())
}

/// The `EquippedInventory` query (module documentation).
pub fn equipped_inventory(space: &ObjectSpace, source: ObjRef, location: u8) -> Result<Option<ObjRef>, AttrError> {
    let pawn = if is_a(space, source, "Engine.Controller")? {
        attr_object(space, source, "Pawn")?
    } else if is_a(space, source, "Engine.Pawn")? {
        Some(source)
    } else {
        return Err(NotYet::not_read(
            format!(
                "EquippedInventoryAttributeContextResolver from a {} source (only a controller or pawn is read)",
                ow_attr::eval::class_path(space, source)
            ),
            RECORD,
        )
        .into());
    };
    let Some(pawn) = pawn else {
        return Ok(None);
    };
    let manager = attr_object(space, pawn, "InvManager")?;
    match manager {
        Some(m) if is_a(space, m, MANAGER)? => {}
        _ => return Ok(None),
    }
    if location >= EQUIPMENT_SLOTS {
        return Ok(None);
    }
    let class = space.heap.get(pawn)?.class;
    let Some(p) = space.prop(class, "EquippedItems") else {
        return Ok(None);
    };
    match space.read(pawn, PropAddr::element(p, u32::from(location)))? {
        Value::Object(o) => Ok(*o),
        other => Err(AttrError::BadData {
            reason: format!("EquippedItems holds {}, not an object", other.kind_name()).into(),
        }),
    }
}

/// Registers the two inventory query handlers on `vm`'s world query service (W5-WORLD); a VM without one is
/// `NotYet { LayerAbsent }`.
pub fn register_queries(vm: &mut Vm) -> Result<(), VmError> {
    let service = vm.service::<WorldQuery>().ok_or_else(|| {
        VmError::from(NotYet::layer_absent(
            "the world query service the inventory query handlers register on",
            "ow-world (World::new installs it)",
            RECORD,
        ))
    })?;
    let mut q = service
        .try_borrow_mut()
        .map_err(|_| VmError::ServiceBorrowed("WorldQuery"))?;
    let equipped: QueryFn = Rc::new(|query, source, w: WorldParts<'_>| match query {
        EngineQuery::EquippedInventory { location } => equipped_inventory(w.space, source, location),
        other => Err(AttrError::World(format!("the equipped-inventory handler was given {other:?}"))),
    });
    q.register(QueryKind::EquippedInventory, equipped)?;
    let inventory: QueryFn = Rc::new(|_query, _source, _w| {
        Err(NotYet::not_read(
            "InventoryAttributeContextResolver's lookup through the source's interface",
            RECORD,
        )
        .into())
    });
    q.register(QueryKind::Inventory, inventory)
}

/// This crate's native pack (module documentation). Register it after W5-WORLD's `world::Pack` (`World::new` takes
/// the packs in order); the query handlers need the world's query service.
#[derive(Debug, Clone, Copy, Default)]
pub struct Pack;

impl NativePack for Pack {
    fn register(&self, vm: &mut Vm) -> Result<(), VmError> {
        register_natives(vm)?;
        register_iterators(vm)?;
        register_queries(vm)?;
        crate::equip::bind_overrides(vm)?;
        crate::equip::natives::register(vm)?;
        crate::definition::register(vm)?;
        crate::equip::value::register(vm)?;
        crate::pickup::register(vm)?;
        crate::globals::register(vm)?;
        crate::equip::slots::register(vm)?;
        crate::equip::internal_slots::register(vm)?;
        crate::anim_tree::register(vm)?;
        crate::spawn::register(vm)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod script_tests;
