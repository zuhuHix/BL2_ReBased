//! Owner: lane W5-EQUIP. Item spawn natives, calling `ow-items`; registers no behavior handler (those are
//! `ow-combat::loot`'s). AI-assisted (Claude Code).
//!
//! The two natives of NATIVE_LOOT.md (read from the executable by lane G1; **UNVERIFIED** in the running game):
//!
//! - `ItemPool.SpawnBalancedInventoryFromInventoryBalanceDefinition(InvBalanceDefinition, Quantity, GameStage,
//!   AwesomeLevel, ContextSource, out SpawnedInventory)`:
//!   1. the balance expands into its candidates at `min(GameStage, cap)` (`ow_items::candidates::expand`, the
//!      record's "balance candidate expansion"), the cap from the world's balance state
//!      (`ow_balance::cap::item_stage_cap`), the spawn modifiers evaluated with `ContextSource` as the source;
//!   2. `Quantity` times, one weighted pick among them with replacement (`ow_items::candidates::pick_candidate`, on
//!      the world's `CrtStream::LootPool`), and one inventory spawned for it ([`spawn_one`]);
//!   3. the answer is true when every requested spawn succeeded; zero candidates spawn nothing and answer false.
//! - `ItemPool.SpawnBalancedInventoryFromPool` is `NotYet { NotPorted }`. W4-POOLS' `ow_items::pools::roll_pool`
//!   (decision 2026-11-15-25) serves the offline roll, and the native needs four things its shape does not give: the
//!   caller's stage (the roll takes the snapshot's), the caller's `GameStageVarianceFormula` object (the roll takes
//!   registry data), the native's `ContextSource` for every evaluation (the roll evaluates with none), and a spawn
//!   inside the pick loop (the roll returns every drop first, so the item rolls cannot interleave with the pool
//!   draws as the native's do). A contract item for the hub, not a change here.
//!
//! **One spawn** ([`resolve`], then [`spawn_one`]): the candidate's definition must name an `InventoryClass` that is
//! a `WillowInventory`, else the pick fails; the definition, the manufacturer and the class's functions are found
//! before anything is made. An actor of that class is then spawned (`ow_world::actors::spawn`);
//! `SourceDefinitionName` = the balance's object name; `SetGameStage(candidate stage)` and `SetAwesomeLevel(awesome)`
//! are called on it; the script event `InitializeInventory(balance, manufacturer, grade-or-level, ContextSource)`
//! runs; `bDropOnDeath` is set; the actor is appended to the out array. A step that fails after the actor exists
//! destroys it again, and a pick that fails destroys every actor the call made before the error is returned
//! ([`spawn_picks`]), so an `Err` leaves no spawned inventory behind. A destroy that fails or that the world refuses
//! (an actor whose `Role` is below authority, for one; `ow_world::actors::destroy`) is reported: the error then names
//! the cleanup's failure and the step's, and outranks the step's. Two choices this port makes where the
//! record is silent, each UNVERIFIED:
//! - the actor is spawned by the level's `WorldInfo` (its location, rotation and instigator): where the native places
//!   the actor is not read;
//! - `SourceResponsibleName` is not written: the record says it is "taken from" the context source without saying
//!   which field (telemetry only; nothing in the roll reads it).
//!
//! The registry and the roll's data are built once per world from the store (they read the whole census), so they
//! are a VM service, [`ItemSource`], that the host installs with [`install`]; without it the natives are
//! `NotYet { LayerAbsent }`.

use std::cell::RefCell;
use std::rc::Rc;

use ow_attr::api::{EvalCx, InitData};
use ow_balance::cap::item_stage_cap;
use ow_core::rand::CrtStream;
use ow_core::{NotYet, ObjectPath};
use ow_items::candidates::{PoolCandidate, expand, pick_candidate};
use ow_items::merge::RollData;
use ow_items::registry::{Pv, Registry};
use ow_items::ItemError;
use ow_object::ids::{ClassId, ObjRef};
use ow_object::value::Value;
use ow_vm::api::{NativeCall, NativeFn, Vm};
use ow_vm::error::VmError;
use ow_world::actors::{SpawnRequest, actors_of, destroy, send_event, spawn};
use ow_world::world::BalanceState;

use crate::equip::{bad, cleanup_failed, item_error, object_of, text, write};

/// The record this module follows.
pub const RECORD: &str = "NATIVE_LOOT.md (ItemPool spawn natives)";

/// The native keys, as the loader derives them from the installed `WillowGame.upk`.
pub const FROM_BALANCE: &str =
    "ItemPool.SpawnBalancedInventoryFromInventoryBalanceDefinition(object,int,int,int,object,array)";
pub const FROM_POOL: &str = "ItemPool.SpawnBalancedInventoryFromPool(object,int,int,object,array,object,float,bool)";

/// What the spawn natives read about balances (module documentation). A VM service.
#[non_exhaustive]
pub struct ItemSource {
    pub registry: Registry,
    pub data: RollData,
}

impl ItemSource {
    /// The registry and the roll data built over the same census input (`ow_items::registry::build_from_census`,
    /// `ow_items::merge::RollData::new`).
    pub fn new(registry: Registry, data: RollData) -> Self {
        ItemSource { registry, data }
    }
}

impl std::fmt::Debug for ItemSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ItemSource").finish_non_exhaustive()
    }
}

/// Adds the [`ItemSource`] service to `vm` (once).
pub fn install(vm: &mut Vm, source: ItemSource) -> Result<Rc<RefCell<ItemSource>>, VmError> {
    vm.add_service(source)
}

/// The object at `path` through the VM's store.
fn find(vm: &mut Vm, path: &str, what: &str) -> Result<ObjRef, VmError> {
    let p = ObjectPath::parse(path).map_err(|e| bad(what, format!("{path}: {e}")))?;
    vm.defs
        .find(&mut vm.space, &p)?
        .ok_or_else(|| bad(what, format!("{path}: no package holds it")))
}

fn find_opt(vm: &mut Vm, v: &Pv, what: &str) -> Result<Option<ObjRef>, VmError> {
    match v.as_str() {
        Some(p) if !p.is_empty() => find(vm, p, what).map(Some),
        _ => Ok(None),
    }
}

/// A registry value read for the expansion: a number as it is, `{"init": ...}` through the evaluator with `source`
/// (the init as the tag stream holds it: an untagged constant 0, an untagged scale 1, as `ow-items`' roll reads it),
/// nothing for no value. A constant or scale that is present but not a number is `BadData`, never a default.
fn registry_value(vm: &mut Vm, v: &Pv, source: Option<ObjRef>) -> Result<Option<f64>, ItemError> {
    const WHAT: &str = "a balance's spawn modifier";
    let wrap = |e: VmError| ItemError::BadData(format!("{WHAT}: {e}"));
    match v {
        Pv::Null => Ok(None),
        Pv::Int(_) | Pv::Float(_) | Pv::Bool(_) => Ok(v.number()),
        other => {
            let Some(init) = other.get("init") else {
                return Err(ItemError::BadData(format!("{WHAT}: a registry value {}", other.to_json())));
            };
            // An absent field is its untagged default; a present one must be a number. The registry holds the
            // tag stream's single-precision value as a double, so the narrowing is exact.
            let number = |name: &str, untagged: f64| -> Result<f32, ItemError> {
                match init.at(name) {
                    Pv::Null => Ok(untagged as f32),
                    x => x
                        .number()
                        .map(|v| v as f32)
                        .ok_or_else(|| ItemError::BadData(format!("{WHAT}: {name} is {}", x.to_json()))),
                }
            };
            let data = InitData {
                base_value_constant: number("BaseValueConstant", 0.0)?,
                base_value_attribute: find_opt(vm, init.at("BaseValueAttribute"), WHAT).map_err(wrap)?,
                initialization_definition: find_opt(vm, init.at("InitializationDefinition"), WHAT).map_err(wrap)?,
                base_value_scale_constant: number("BaseValueScaleConstant", 1.0)?,
            };
            let attrs = vm.attrs.clone();
            Ok(Some(f64::from(attrs.evaluate_init(&data, source, None, &mut EvalCx::new(vm))?)))
        }
    }
}

/// The item stage cap of the world's balance state.
fn stage_cap(vm: &Vm) -> Result<i32, VmError> {
    let state = vm.service::<BalanceState>().ok_or_else(|| {
        VmError::from(NotYet::layer_absent(
            "the world's balance state (the item stage cap)",
            "ow-world (World::new)",
            RECORD,
        ))
    })?;
    let s = state
        .try_borrow()
        .map_err(|_| VmError::ServiceBorrowed("BalanceState"))?;
    let snap = &s.snapshot;
    let cap = item_stage_cap(snap.playthrough, snap.op, snap.level_cap_packs).map_err(|e| {
        item_error(FROM_BALANCE, ItemError::from(e))
    })?;
    Ok(cap.0)
}

/// The balance's candidates at `min(stage, cap)` (module documentation, step 1).
pub fn candidates(
    vm: &mut Vm,
    balance: &str,
    stage: i32,
    context: Option<ObjRef>,
) -> Result<Vec<PoolCandidate>, VmError> {
    let cap = stage_cap(vm)?;
    let source = vm.service::<ItemSource>().ok_or_else(|| {
        VmError::from(NotYet::layer_absent(
            "the item registry the spawn natives read",
            "ow-inventory spawn::install (the host builds it from the store)",
            RECORD,
        ))
    })?;
    // The service is not borrowed while the evaluator runs script-free reads through the VM: the record is cloned
    // out first. The roll data stays borrowed only for the expansion, which reads tag streams and calls `value`,
    // whose evaluation borrows no service.
    let record = {
        let s = source.try_borrow().map_err(|_| VmError::ServiceBorrowed("ItemSource"))?;
        s.registry
            .balance(balance)
            .cloned()
            .ok_or_else(|| bad(FROM_BALANCE, format!("{balance}: not a balance the registry holds")))?
    };
    let s = source.try_borrow().map_err(|_| VmError::ServiceBorrowed("ItemSource"))?;
    let mut value = |v: &Pv| registry_value(vm, v, context);
    expand(&s.data, &record, stage, cap, &mut value).map_err(|e| item_error(FROM_BALANCE, e))
}

/// What one pick spawns, every object found and every class checked before anything is made.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resolved {
    /// The class to spawn: the definition's `InventoryClass`, a `WillowInventory`.
    pub class: ClassId,
    pub manufacturer: ObjRef,
    /// The stage stored on the item and the grade-or-level word `InitializeInventory` receives.
    pub stage: i32,
    pub grade_or_level: i32,
}

/// The three functions one spawn calls on the new inventory, in order (module documentation).
pub const SPAWN_CALLS: [&str; 3] = ["SetGameStage", "SetAwesomeLevel", "InitializeInventory"];

/// [`Resolved`] for a definition and manufacturer already found. `Ok(None)`: the pick fails as the record says (no
/// `InventoryClass`, or one that is not a `WillowInventory`). The class must declare the three functions
/// [`spawn_one`] calls; one it lacks is a data error, found here, before any actor exists.
pub fn resolve_objects(
    vm: &mut Vm,
    definition: ObjRef,
    manufacturer: ObjRef,
    stage: i32,
    grade_or_level: i32,
) -> Result<Option<Resolved>, VmError> {
    const WHAT: &str = "ItemPool spawn";
    let class = match ow_attr::eval::property(&mut EvalCx::new(vm), definition, "InventoryClass")? {
        Some(Value::Class(c)) => c,
        Some(other) => return Err(bad(WHAT, format!("InventoryClass holds {}", other.kind_name()))),
        None => None,
    };
    let Some(class) = class else {
        return Ok(None);
    };
    let willow = vm.find_class("Engine.WillowInventory")?;
    if !vm.space.classes.is_child_of(class, willow) {
        return Ok(None);
    }
    for name in SPAWN_CALLS {
        if vm.find_method(class, name, None)?.is_none() {
            return Err(bad(WHAT, format!("the inventory class declares no {name}")));
        }
    }
    Ok(Some(Resolved {
        class,
        manufacturer,
        stage,
        grade_or_level,
    }))
}

/// [`resolve_objects`] for a candidate: its definition and manufacturer found through the store.
pub fn resolve(vm: &mut Vm, candidate: &PoolCandidate) -> Result<Option<Resolved>, VmError> {
    const WHAT: &str = "ItemPool spawn";
    let definition = find(vm, &candidate.definition, WHAT)?;
    let manufacturer = find(vm, &candidate.manufacturer, WHAT)?;
    resolve_objects(vm, definition, manufacturer, candidate.stage, candidate.grade_or_level)
}

/// One spawn of a resolved pick (module documentation). `Ok(None)`: the spawn gave nothing. An `Err` after the actor
/// was made destroys it again before it returns; a destroy that fails or is refused makes the error the cleanup's
/// ([`cleanup_failed`], naming both).
pub fn spawn_one(
    vm: &mut Vm,
    balance: ObjRef,
    r: &Resolved,
    awesome: i32,
    context: Option<ObjRef>,
) -> Result<Option<ObjRef>, VmError> {
    const WHAT: &str = "ItemPool spawn";
    let name = vm.space.heap.get(balance)?.name;
    let spawner = actors_of(vm)?
        .try_borrow()
        .map_err(|_| VmError::ServiceBorrowed("Actors"))?
        .world_info()
        .ok_or_else(|| {
            VmError::from(NotYet::layer_absent(
                "the level's WorldInfo (the spawner of an item actor)",
                "the map load",
                RECORD,
            ))
        })?;
    let Some(item) = spawn(vm, spawner, &SpawnRequest::new(Some(r.class)))? else {
        return Ok(None);
    };
    let filled = (|| -> Result<(), VmError> {
        write(vm, item, "SourceDefinitionName", Value::Name(name), WHAT)?;
        call_method(vm, item, "SetGameStage", vec![Value::Int(r.stage)])?;
        call_method(vm, item, "SetAwesomeLevel", vec![Value::Int(awesome)])?;
        send_event(
            vm,
            item,
            "InitializeInventory",
            vec![
                Value::Object(Some(balance)),
                Value::Object(Some(r.manufacturer)),
                Value::Int(r.grade_or_level),
                Value::Object(context),
            ],
        )?;
        write(vm, item, "bDropOnDeath", Value::Bool(true), WHAT)
    })();
    match filled {
        Ok(()) => Ok(Some(item)),
        Err(e) => match destroyed(vm, item) {
            Ok(()) => Err(e),
            Err(why) => Err(cleanup_failed(WHAT, &e, &why)),
        },
    }
}

/// Destroys `item` after a failed spawn; `Err` says why it is still there (a destroy that errs or refuses, for one an
/// actor whose `Role` is below authority).
fn destroyed(vm: &mut Vm, item: ObjRef) -> Result<(), String> {
    match destroy(vm, item) {
        Ok(true) => Ok(()),
        Ok(false) => Err(format!(
            "the world refused to destroy the spawned {}",
            ow_attr::eval::class_path(&vm.space, item)
        )),
        Err(e) => Err(format!("destroying the spawned actor: {}", text(&e))),
    }
}

/// `quantity` picks (module documentation, step 2): `pick` answers each pick's resolved spawn (`None`: the pick
/// failed), then [`spawn_one`] runs. Returns the actors made and whether every pick spawned. An `Err` destroys every
/// actor this call made before it returns; one it cannot destroy makes the error the cleanup's ([`cleanup_failed`]).
pub fn spawn_picks(
    vm: &mut Vm,
    balance: ObjRef,
    quantity: i32,
    awesome: i32,
    context: Option<ObjRef>,
    mut pick: impl FnMut(&mut Vm) -> Result<Option<Resolved>, VmError>,
) -> Result<(Vec<ObjRef>, bool), VmError> {
    let mut made: Vec<ObjRef> = Vec::new();
    let mut all = true;
    for _ in 0..quantity.max(0) {
        let step = pick(vm).and_then(|r| match r {
            Some(r) => spawn_one(vm, balance, &r, awesome, context),
            None => Ok(None),
        });
        match step {
            Ok(Some(item)) => made.push(item),
            Ok(None) => all = false,
            Err(e) => {
                let mut failures = Vec::new();
                for item in made {
                    if let Err(why) = destroyed(vm, item) {
                        failures.push(why);
                    }
                }
                if failures.is_empty() {
                    return Err(e);
                }
                return Err(cleanup_failed("ItemPool spawn", &e, &failures.join("; ")));
            }
        }
    }
    Ok((made, all))
}

/// Calls `name` on `o` (its class's function; a class without one is a data error).
fn call_method(vm: &mut Vm, o: ObjRef, name: &str, args: Vec<Value>) -> Result<(), VmError> {
    let (class, state) = {
        let x = vm.space.heap.get(o)?;
        (x.class, x.state)
    };
    let f = vm
        .find_method(class, name, state)?
        .ok_or_else(|| bad("ItemPool spawn", format!("the inventory has no {name}")))?;
    vm.call(f, Some(o), args)?;
    Ok(())
}

fn int_arg(call: &NativeCall<'_>, i: usize, what: &str) -> Result<i32, VmError> {
    match call.arg(i)? {
        Value::Int(v) => Ok(*v),
        other => Err(bad(what, format!("argument {i} holds {}, not an int", other.kind_name()))),
    }
}

/// `SpawnBalancedInventoryFromInventoryBalanceDefinition` (module documentation).
fn from_balance(call: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let Some(balance) = object_of(call.arg(0)?, FROM_BALANCE)? else {
        return Ok(Value::Bool(false));
    };
    let quantity = int_arg(call, 1, FROM_BALANCE)?;
    let stage = int_arg(call, 2, FROM_BALANCE)?;
    let awesome = int_arg(call, 3, FROM_BALANCE)?;
    let context = object_of(call.arg(4)?, FROM_BALANCE)?;
    let mut out = ow_attr::eval::elements("SpawnedInventory", call.arg(5)?)?.to_vec();
    // The registry is keyed by the balance's object path.
    let path = object_path(call.vm, balance)?;
    let candidates = candidates(call.vm, &path, stage, context)?;
    let (made, all) = spawn_picks(call.vm, balance, quantity, awesome, context, |vm| {
        let picked = pick_candidate(&candidates, vm.rng.crt(CrtStream::LootPool));
        match picked.choice.and_then(|i| candidates.get(i)) {
            Some(c) => resolve(vm, c),
            None => Ok(None),
        }
    })?;
    let every = all && !candidates.is_empty();
    out.extend(made.into_iter().map(|o| Value::Object(Some(o))));
    call.set_out(5, Value::array(out))?;
    Ok(Value::Bool(every))
}

/// `o`'s full object path, the form the registry and the definition store key objects by.
///
/// - An object the store made from a package export (or a resource reference to one) is named as the store names
///   it (`ow_defs::identity::full_path`, crate-private there, so its rule is restated here): an import's path is
///   already full; an export whose outermost export is a `Package` object is named by its path in the file; any
///   other export is `<file stem>.<path in the file>`.
/// - Any other object: its outers' names, then its own (a chain deeper than 64 is a data error).
pub fn object_path(vm: &Vm, o: ObjRef) -> Result<String, VmError> {
    use ow_object::heap::Origin;
    let x = vm.space.heap.get(o)?;
    if let Origin::Export { package, index } | Origin::Resource { package, index } = x.origin {
        let pkg = vm.defs.packages().get(package);
        let text = |e: ow_package::package::PackageError| bad("an object path", e.to_string());
        let path = pkg.path_of(index).map_err(text)?;
        if index <= 0 {
            return Ok(path);
        }
        let mut cursor = index;
        loop {
            let entry = pkg.object(cursor).map_err(text)?;
            if entry.outer == 0 {
                break;
            }
            if entry.outer < 0 {
                return Ok(path);
            }
            cursor = entry.outer;
        }
        let outermost = pkg.object(cursor).map_err(text)?;
        let is_package = outermost.class != 0 && pkg.object(outermost.class).map_err(text)?.name == "Package";
        return Ok(if is_package { path } else { format!("{}.{path}", pkg.name) });
    }
    let mut names = Vec::new();
    let mut cursor = Some(o);
    while let Some(c) = cursor {
        if names.len() >= 64 {
            return Err(bad("an object path", "an outer chain deeper than 64"));
        }
        let x = vm.space.heap.get(c)?;
        names.push(vm.space.names.text(x.name).to_string());
        cursor = x.outer;
    }
    names.reverse();
    Ok(names.join("."))
}

/// Registers the two spawn natives.
pub fn register(vm: &mut Vm) -> Result<(), VmError> {
    let balance: NativeFn = Rc::new(from_balance);
    vm.register_native(FROM_BALANCE, balance)?;
    let pool: NativeFn = Rc::new(|_call: &mut NativeCall<'_>| {
        Err(NotYet::not_ported(
            "ItemPool.SpawnBalancedInventoryFromPool: ow_items::pools::roll_pool takes the snapshot's stage, registry              variance data and no context source, and returns the drops before any spawn (a contract item, module              documentation)",
            RECORD,
        )
        .into())
    });
    vm.register_native(FROM_POOL, pool)
}

#[cfg(test)]
mod tests;
