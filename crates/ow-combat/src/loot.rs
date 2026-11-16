//! Owner: lane W6-LOOT. Loot on death (plan step S-g) and the loot-spawn behavior handlers (registered into the
//! `ow-behavior` kernel). Port of `src/combat_loot.cpp` under [`ParityMode::Reference`]; the game's rules, or `NotYet`,
//! in [`ParityMode::Off`] (decision 2026-11-16-41). AI-assisted (Claude Code).
//!
//! The records are NATIVE_LOOT.md (read from the executable by lane G1) and COMBAT_LOOT.md (lane Sg, from the note,
//! the live records and the installed script). **Every rule is UNVERIFIED in the running game** unless COMBAT_LOOT.md
//! names the live row it reproduces; the C++ is the oracle for every Reference text and number, which says nothing
//! about the game.
//!
//! **What runs in both modes.**
//! - [`setup_loot`]: what the stock spawn does for the loot (`PopulationFactoryBalancedAIPawn.SetupBalancedPopulationActor`,
//!   script, read by lane Sg): the pawn's item game stage is its `ExpLevel` when the balance's `BaseItemGameStage` is 0,
//!   else its `GameStage`, plus the truncated `ItemGameStageOffset` evaluated with the pawn as the source; then the
//!   script's call `AIPawnBalanceDefinition.SetupPawnItemPoolList(pawn)` through the VM. A step of the run harness.
//! - `AIPawnBalanceDefinition.SetupPawnItemPoolList` ([`setup_item_pool_list`]): the playthrough entry's custom lists
//!   replace the balance's default lists when not empty (each list on its own), the included lists come first, depth
//!   first (`ItemPoolListDefinition.AddToItemPoolList`), then the own pools; the pawn's `ItemPoolList` is written and
//!   the answer is whether it is not empty; a balance with no playthrough entry answers false and writes nothing.
//!   `GetPlayThroughIndex`: **Reference** takes entry 0 (the C++ models the first playthrough only); **Off** takes the
//!   world's playthrough index, at most the last entry (NATIVE_LOOT.md), from the world's balance state.
//!
//! **Reference only (the C++ stand-ins, architecture R3 and 6.1).**
//! - One generator for the whole loot path, the C++ `rand()` stand-in: the Microsoft C runtime LCG, default seed
//!   [`DEFAULT_SEED`], restarted by the `seed` step. It is the VM's [`REFERENCE_STREAM`], which the attribute evaluator
//!   also draws its random variance from, so the evaluator's draws and the pool draws share it as the C++
//!   `evaluator_.setRandom(rand15)` makes them share it.
//! - `ItemPool.SpawnBalancedInventoryFromPool` as `combat_loot.cpp` reads NATIVE_LOOT.md (the gate, the flat weighted
//!   list, single-precision picks with replacement, nested pools rolled at once, item levels from the balance getters),
//!   with **stand-in inventories**: an instance of the item's inventory class with `bDropOnDeath` set and nothing
//!   else (no `InitializeInventory`, no parts, no stats). A caller's variance formula is the C++ "not implemented" note.
//! - `WillowPawn.DropLootOnDeath` overridden by the script's loop (the C++ replaces the installed script because its VM
//!   has no inventory actors or pickups): `FRand` first, then the pool's probability with the pawn's controller as
//!   the source (the `AmmoDrops_PerPlayer` multiplier 1.0 once per death), the pool tried when the chance is above 0
//!   and the draw at most the chance; an item is "dropped" when its flag is set and the pawn is not below the level's
//!   `KillZ` with a weapon. The drop is a record ([`LootDrop`]); the run reports it.
//!
//! **Off mode.**
//! - The installed `DropLootOnDeath` script runs (no override): its loop is world script (architecture R2), and the
//!   pickup half has no Rust native yet, so what it reaches unported stops it.
//! - `ItemPool.SpawnBalancedInventoryFromPool` is `NotYet { NotPorted }` before anything is drawn or spawned. The pool
//!   rule belongs to `ow_items::pools::roll_pool` (W4-POOLS, R2: one implementation), whose shape lacks three inputs
//!   the native needs (decision 2026-11-16-41 settles W5-EQUIP's contract 2 item by item): it evaluates every value
//!   with no context source where the native evaluates with `ContextSource` (a value whose context resolver needs the
//!   source would read silently wrong, not `NotYet`), it takes the variance as registry data where the native gets
//!   the caller's `GameStageVarianceFormula` object, and it does not report the native's return value (whether the
//!   gate passed and every pick found a candidate). The caller's stage (a balance snapshot with the call's stage) and
//!   the draw order (the pool draws on `CrtStream::LootPool`, an item's own roll on its own stream, so spawning after
//!   the picks draws the same numbers) would not block it.
//!
//! **The loot-spawn behaviors** (`Behavior_SpawnItems`, `Behavior_SpawnLoot`, `Behavior_SpawnLootAroundPoint`,
//! `Behavior_SpawnLootAtPoints`, `Behavior_DropItems`) are script classes (`research/native_by_class.json` lists no
//! native for any of them) whose rules NATIVE_LOOT.md has not read. Their installed `ApplyBehaviorToContext` needs the
//! consumer (`SelfObject`), which `ow_behavior::registry::HandlerFn` does not receive (decision 2026-11-15-30). **No
//! handler is registered for them, in either mode** (review r1 of PR #340): the kernel's default for an unregistered
//! class is what the C++ does (`src/behavior.cpp`: it records `unsupported behavior class ...`, here with the
//! registry's `NotYet { NotPorted }`, and stops that branch only), where a registered handler that returns an error
//! would abort the whole event. A provider's own handlers still come first (the combat run lists
//! `Behavior_SpawnItems` at its death providers' boundary, as the C++ does).

use std::cell::RefCell;
use std::rc::Rc;

use ow_attr::api::EvalCx;
use ow_core::fmt::{fmt_f, fmt_g, json_quote};
use ow_core::num::trunc_i32;
use ow_core::rand::{CrtRand, CrtStream, Rand15, frand_f32};
use ow_core::{NotYet, ParityMode};
use ow_object::ids::{ClassId, ObjRef};
use ow_object::value::Value;
use ow_vm::api::{NativeCall, NativeFn, NativePack, Vm};
use ow_vm::error::VmError;
use ow_world::world::BalanceState;

use crate::hit::access::{
    array, error, error_text, field, get, integer, loaded, number, object_of, path_of, required,
    set_required, truth,
};
use crate::hit::{arg_object, evaluate_for, not_implemented, note, with_state};

/// The records this module follows.
pub const RECORD: &str = "NATIVE_LOOT.md; COMBAT_LOOT.md; src/combat_loot.cpp";

/// The C++ default seed of the loot's `rand()` stand-in (`CombatSystem::DefaultSeed`). Reference only.
pub const DEFAULT_SEED: u32 = 20_261_009;

/// The stream the Reference loot generator lives on: the one the attribute evaluator draws its variance from.
pub const REFERENCE_STREAM: CrtStream = CrtStream::AttributeVariance;

/// The stream the `seed` step restarts in Off mode: the pool roll's (`ow_items::pools`).
pub const OFF_STREAM: CrtStream = CrtStream::LootPool;

/// The global level cap on the first playthrough (NATIVE_LOOT.md, "Level cap"; the third-playthrough branch is not
/// modelled by the C++).
const LEVEL_CAP: i32 = 50;
/// A candidate weighing this or less is not added.
const WEIGHT_FLOOR: f32 = 1e-8;
/// Nested pools and included lists (the C++ bound; the game has no guard).
const MAX_DEPTH: u32 = 16;
/// The balance chains (`BaseDefinition`): a Rust-only bound, the C++ guards them by the balances in the walk only.
const MAX_BALANCE_CHAIN: usize = 64;
/// `GetAmmoDropsPerPlayerMultiplier` below the third playthrough (NATIVE_LOOT.md; the later table is unread).
const AMMO_DROPS_PER_PLAYER_MULTIPLIER: f64 = 1.0;
/// An item level's bound in the C++ (`std::min(..., 63)`).
const MAX_ITEM_LEVEL: i32 = 63;

/// The functions this module binds or overrides, by path.
pub const SETUP: &str = "WillowGame.AIPawnBalanceDefinition.SetupPawnItemPoolList";
pub const FROM_POOL: &str = "WillowGame.ItemPool.SpawnBalancedInventoryFromPool";
pub const DROP_LOOT: &str = "WillowGame.WillowPawn.DropLootOnDeath";

/// The loot-spawn behavior classes, left unregistered (module documentation).
pub const SPAWN_BEHAVIORS: [&str; 5] = [
    "WillowGame.Behavior_SpawnItems",
    "WillowGame.Behavior_SpawnLoot",
    "WillowGame.Behavior_SpawnLootAroundPoint",
    "WillowGame.Behavior_SpawnLootAtPoints",
    "WillowGame.Behavior_DropItems",
];

/// The unit name of the C++ error texts.
const UNIT: &str = "combat loot";

// ---------------------------------------------------------------------------------------------------------------
// Records
// ---------------------------------------------------------------------------------------------------------------

/// One item a Reference pool roll made (C++ `LootItem`): a record, not an inventory.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct LootItem {
    pub pool: String,
    pub balance: String,
    pub manufacturer: String,
    /// The inventory class's name (C++ `inventoryClass`).
    pub class_name: String,
    pub class: Option<ClassId>,
    pub level: i32,
    pub drop_on_death: bool,
    pub dropped: bool,
}

/// One pool of a death roll (C++ `LootPool`).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LootPool {
    pub pool: String,
    /// The text of the error that stopped this pool (empty when none).
    pub error: String,
    pub chance: f64,
    pub roll: f64,
    pub tried: bool,
    pub returned: bool,
    pub items: Vec<LootItem>,
}

/// One death roll (C++ `LootDrop`).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct LootDrop {
    pub pawn: ObjRef,
    pub stage: i32,
    pub awesome: i32,
    pub below_kill_z: bool,
    pub pools: Vec<LootPool>,
}

/// The death rolls so far, in order. A VM service ([`enable`]).
#[non_exhaustive]
#[derive(Debug, Default)]
pub struct LootState {
    pub drops: Vec<LootDrop>,
}

fn loot_state(vm: &Vm) -> Result<Rc<RefCell<LootState>>, VmError> {
    vm.service::<LootState>()
        .ok_or_else(|| NotYet::layer_absent("the loot state", "ow-combat loot::enable", RECORD).into())
}

/// The death rolls recorded so far (a copy).
pub fn drops(vm: &Vm) -> Result<Vec<LootDrop>, VmError> {
    let s = loot_state(vm)?;
    let s = s.try_borrow().map_err(|_| VmError::ServiceBorrowed("LootState"))?;
    Ok(s.drops.clone())
}

/// The death rolls recorded from index `start` on (a copy; none when `start` is past the end).
pub fn drops_from(vm: &Vm, start: usize) -> Result<Vec<LootDrop>, VmError> {
    let s = loot_state(vm)?;
    let s = s.try_borrow().map_err(|_| VmError::ServiceBorrowed("LootState"))?;
    Ok(s.drops.get(start..).map(<[LootDrop]>::to_vec).unwrap_or_default())
}

/// How many death rolls are recorded.
pub fn drop_count(vm: &Vm) -> Result<usize, VmError> {
    let s = loot_state(vm)?;
    let s = s.try_borrow().map_err(|_| VmError::ServiceBorrowed("LootState"))?;
    Ok(s.drops.len())
}

/// Forgets every recorded death roll (the C++ batch's `drops().clear()`).
pub fn clear_drops(vm: &Vm) -> Result<(), VmError> {
    let s = loot_state(vm)?;
    let mut s = s
        .try_borrow_mut()
        .map_err(|_| VmError::ServiceBorrowed("LootState"))?;
    s.drops.clear();
    Ok(())
}

fn push_drop(vm: &Vm, drop: LootDrop) -> Result<(), VmError> {
    let s = loot_state(vm)?;
    let mut s = s
        .try_borrow_mut()
        .map_err(|_| VmError::ServiceBorrowed("LootState"))?;
    s.drops.push(drop);
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// Set-up
// ---------------------------------------------------------------------------------------------------------------

/// The native key the loader derived for `path`, when the package set declares that function native.
fn declared_native_key(vm: &mut Vm, path: &str) -> Option<String> {
    let id = vm.find_function(path).ok()?;
    vm.function(id).and_then(|d| d.native_key.clone())
}

/// Binds the loot natives where the package set declares them native, overrides `DropLootOnDeath` in Reference mode,
/// restarts the Reference loot generator from [`DEFAULT_SEED`], and adds the [`LootState`] service last. Once per
/// VM: a call after a successful one does nothing. A native key that is already bound (another pack's, or one an
/// earlier call bound before it failed) is refused before anything is bound (`VmError::Registration`): the key has
/// one owner (review r1 of PR #340). The C++ binds these only with `enableDeaths`; the combat run calls this right
/// after it. No behavior handler is registered (module documentation).
pub fn enable(vm: &mut Vm) -> Result<(), VmError> {
    if vm.service::<LootState>().is_some() {
        return Ok(());
    }
    let natives: [(&str, NativeFn); 2] = [
        (FROM_POOL, Rc::new(spawn_from_pool_native)),
        (SETUP, Rc::new(setup_native)),
    ];
    let mut bound = Vec::new();
    for (path, f) in natives {
        let Some(key) = declared_native_key(vm, path) else {
            continue;
        };
        if vm.has_native(&key) {
            return Err(VmError::Registration {
                key,
                reason: "a loot native's key is already bound (ow-combat::loot owns it)",
            });
        }
        bound.push((key, f));
    }
    for (key, f) in bound {
        vm.register_native(&key, f)?;
    }
    if vm.parity == ParityMode::Reference {
        // The C++ catches the override's error: a set without the script function (or with a native one) keeps it.
        match vm.override_script(DROP_LOOT, Rc::new(drop_loot_native)) {
            Ok(()) | Err(VmError::Registration { .. }) => {}
            Err(e) => return Err(e),
        }
        seed(vm, DEFAULT_SEED);
    }
    vm.add_service(LootState::default())?;
    Ok(())
}

/// The loot pack for a host: [`enable`].
pub struct Pack;

impl NativePack for Pack {
    fn register(&self, vm: &mut Vm) -> Result<(), VmError> {
        enable(vm)
    }
}

/// The `seed` step: the loot's generator restarts from `n` (`srand(n)`). Reference: [`REFERENCE_STREAM`], the C++
/// stand-in; Off: [`OFF_STREAM`], the pool roll's stream (a harness control for repeatable runs; how the game seeds
/// its generator is not known).
pub fn seed(vm: &mut Vm, n: u32) {
    let stream = match vm.parity {
        ParityMode::Reference => REFERENCE_STREAM,
        ParityMode::Off => OFF_STREAM,
    };
    *vm.rng.crt(stream) = CrtRand::new(n);
}

// ---------------------------------------------------------------------------------------------------------------
// Small readers
// ---------------------------------------------------------------------------------------------------------------

/// The object a value names, loaded (C++ `load`).
fn load(vm: &mut Vm, v: &Value) -> Result<Option<ObjRef>, VmError> {
    match object_of(v) {
        Some(o) => loaded(vm, o).map(Some),
        None => Ok(None),
    }
}

/// The C++ `sameObject`: the same object, or two objects with the same non-empty stock path.
fn same_object(vm: &Vm, a: ObjRef, b: ObjRef) -> bool {
    if a == b {
        return true;
    }
    let path = path_of(vm, Some(a));
    !path.is_empty() && path == path_of(vm, Some(b))
}

/// The last component of a dotted path (C++ `leaf`).
fn leaf(path: &str) -> &str {
    path.rsplit_once('.').map_or(path, |(_, l)| l)
}

/// The elements of an array value (none for anything else).
fn items(v: &Value) -> Vec<Value> {
    v.elements().map(<[Value]>::to_vec).unwrap_or_default()
}

/// A property's truth, false when the class does not declare it (the C++ `p && p->truth()`).
fn flag(vm: &mut Vm, o: ObjRef, name: &str) -> Result<bool, VmError> {
    Ok(match get(vm, o, name)? {
        Some(v) => truth(vm, &v),
        None => false,
    })
}

/// A struct field's integer, which must be there (the C++ dereferences it).
fn int_field(vm: &Vm, v: &Value, name: &str) -> Result<i32, VmError> {
    match field(vm, v, name) {
        // An int field keeps its low 32 bits (C++ `int(integer())`).
        Some(x) => Ok(crate::hit::access::low_i32(integer(&x))),
        None => Err(error(format!("{UNIT}: a struct without {name}"))),
    }
}

/// A struct field that must be there, evaluated with `source` (C++ `evaluator_.evaluate(*field, context)`).
fn evaluate_field(vm: &mut Vm, v: &Value, name: &str, source: Option<ObjRef>) -> Result<f32, VmError> {
    match field(vm, v, name) {
        Some(data) => evaluate_for(vm, &data, source),
        None => Err(error(format!("{UNIT}: a struct without {name}"))),
    }
}

/// C++ `evaluateAttribute`: the value of the attribute an object reference names with `source`, `None` for no
/// object or no value.
fn attribute_value(vm: &mut Vm, v: &Value, source: Option<ObjRef>) -> Result<Option<f32>, VmError> {
    let Some(attribute) = load(vm, v)? else {
        return Ok(None);
    };
    let attrs = vm.attrs.clone();
    let value = attrs.value_of(attribute, source, None, &mut EvalCx::new(vm))?;
    Ok(value.map(|v| v.value))
}

/// The world's playthrough index (Off mode's `GetPlayThroughIndex`), from the balance state.
fn world_playthrough(vm: &Vm) -> Result<usize, VmError> {
    let state = vm.service::<BalanceState>().ok_or_else(|| {
        VmError::from(NotYet::layer_absent(
            "the world's balance state (the playthrough GetPlayThroughIndex reads)",
            "ow-world (World::new)",
            RECORD,
        ))
    })?;
    let s = state
        .try_borrow()
        .map_err(|_| VmError::ServiceBorrowed("BalanceState"))?;
    Ok(usize::from(s.snapshot.playthrough.index()))
}

// ---------------------------------------------------------------------------------------------------------------
// The pool list (AIPawnBalanceDefinition.SetupPawnItemPoolList)
// ---------------------------------------------------------------------------------------------------------------

/// `ItemPoolListDefinition.AddToItemPoolList`: the included lists first, depth first, then the list's own pools.
/// `seen` holds the lists above (Off mode refuses a list that includes itself; Reference, as the C++, stops at the
/// depth bound).
fn append_pools(
    vm: &mut Vm,
    list: ObjRef,
    out: &mut Vec<Value>,
    depth: u32,
    seen: &mut Vec<ObjRef>,
) -> Result<(), VmError> {
    if depth > MAX_DEPTH {
        return Err(error(format!("{UNIT}: item pool lists nest too deep")));
    }
    if vm.parity == ParityMode::Off && seen.contains(&list) {
        return Err(error(format!(
            "{UNIT}: the item pool list {} includes itself",
            path_of(vm, Some(list))
        )));
    }
    seen.push(list);
    for entry in array(vm, list, "ItemPoolIncludedLists")? {
        if let Some(included) = load(vm, &entry)? {
            append_pools(vm, included, out, depth + 1, seen)?;
        }
    }
    seen.pop();
    out.extend(array(vm, list, "ItemPools")?);
    Ok(())
}

/// `GetPlayThroughIndex` (module documentation): `None` for a balance with no playthrough entry.
fn play_through_index(vm: &mut Vm, entries: usize) -> Result<Option<usize>, VmError> {
    if entries == 0 {
        return Ok(None);
    }
    Ok(Some(match vm.parity {
        ParityMode::Reference => 0,
        ParityMode::Off => world_playthrough(vm)?.min(entries - 1),
    }))
}

/// `SetupPawnItemPoolList` (module documentation): writes `pawn`'s `ItemPoolList`; whether it is not empty.
pub fn setup_item_pool_list(vm: &mut Vm, balance: ObjRef, pawn: ObjRef) -> Result<bool, VmError> {
    let Some(entries) = get(vm, balance, "PlayThroughs")? else {
        return Ok(false);
    };
    let entries = items(&entries);
    let Some(index) = play_through_index(vm, entries.len())? else {
        return Ok(false);
    };
    let play_through = entries[index].clone();
    let chosen = |vm: &mut Vm, custom: &str, standard: &str| -> Result<Vec<Value>, VmError> {
        match field(vm, &play_through, custom) {
            Some(c) if !items(&c).is_empty() => Ok(items(&c)),
            _ => array(vm, balance, standard),
        }
    };
    let included = chosen(vm, "CustomItemPoolIncludedLists", "DefaultItemPoolIncludedLists")?;
    let own = chosen(vm, "CustomItemPoolList", "DefaultItemPoolList")?;
    let mut pools = Vec::new();
    for entry in included {
        if let Some(list) = load(vm, &entry)? {
            append_pools(vm, list, &mut pools, 0, &mut vec![balance])?;
        }
    }
    pools.extend(own);
    let listed = !pools.is_empty();
    set_required(vm, pawn, "ItemPoolList", Value::array(pools), UNIT)?;
    Ok(listed)
}

fn setup_native(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let (Some(balance), Some(pawn)) = (c.this, arg_object(c, 0)?) else {
        return Ok(Value::Bool(false));
    };
    Ok(Value::Bool(setup_item_pool_list(c.vm, balance, pawn)?))
}

/// What [`setup_loot`] made: the item game stage and the pool list's pool paths.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LootSetup {
    pub item_stage: i32,
    pub pools: Vec<String>,
}

/// The spawn's loot setup (module documentation); logs the C++ event line.
pub fn setup_loot(vm: &mut Vm, pawn: ObjRef, balance: ObjRef) -> Result<LootSetup, VmError> {
    let base = get(vm, balance, "BaseItemGameStage")?;
    let by_level = base.as_ref().is_some_and(|b| integer(b) == 0);
    let level = required(vm, pawn, if by_level { "ExpLevel" } else { "GameStage" }, UNIT)?;
    let mut stage = crate::hit::access::low_i32(integer(&level));
    if let Some(offset) = get(vm, balance, "ItemGameStageOffset")? {
        // C++ `int(std::trunc(float))`: toward zero, i32::MIN out of range (N5).
        let add = trunc_i32(evaluate_for(vm, &offset, Some(pawn))?);
        stage = stage
            .checked_add(add)
            .ok_or_else(|| error(format!("{UNIT}: the item game stage overflows")))?;
    }
    set_required(vm, pawn, "GameStageForSpawnedInventory", Value::Int(stage), UNIT)?;
    let class = vm.space.heap.get(balance)?.class;
    let Some(setup) = vm.find_method(class, "SetupPawnItemPoolList", None)? else {
        return Err(error(format!(
            "{UNIT}: the balance class has no SetupPawnItemPoolList"
        )));
    };
    let listed = vm.call(setup, Some(balance), vec![Value::Object(Some(pawn))])?.ret;
    let listed = truth(vm, &listed);
    let list = get(vm, pawn, "ItemPoolList")?
        .map(|v| items(&v))
        .ok_or_else(|| error(format!("{UNIT}: the pawn has no property ItemPoolList")))?;
    note(
        vm,
        format!(
            "SetupPawnItemPoolList {}: {} pools, item game stage {stage}{}",
            leaf(&path_of(vm, Some(balance))),
            list.len(),
            if listed { "" } else { " (no list)" }
        ),
    )?;
    let mut pools = Vec::new();
    for entry in &list {
        let pool = field(vm, entry, "ItemPool").as_ref().and_then(object_of);
        pools.push(path_of(vm, pool));
    }
    Ok(LootSetup {
        item_stage: stage,
        pools,
    })
}

// ---------------------------------------------------------------------------------------------------------------
// Inventory balances (Reference: combat_loot.cpp's reading of NATIVE_LOOT.md)
// ---------------------------------------------------------------------------------------------------------------

/// One weighted choice of a pool's roll: a nested pool, or one manufacturer of a balance (C++ `LootCandidate`).
#[derive(Debug, Clone)]
struct Candidate {
    pool: Option<ObjRef>,
    balance: Option<ObjRef>,
    manufacturer: Option<ObjRef>,
    word: i32,
    weight: f32,
    drop: bool,
}

/// The manufacturer entry of `balance` for `manufacturer`, if any (the first that names it).
fn manufacturer_entry(vm: &mut Vm, balance: ObjRef, manufacturer: ObjRef) -> Result<Option<Value>, VmError> {
    for entry in array(vm, balance, "Manufacturers")? {
        let Some(listed) = field(vm, &entry, "Manufacturer") else {
            continue;
        };
        let Some(listed) = load(vm, &listed)? else {
            continue;
        };
        if same_object(vm, listed, manufacturer) {
            return Ok(Some(entry));
        }
    }
    Ok(None)
}

/// The grade `word & 0xffff` of a manufacturer entry, if the entry has that many.
fn grade_of(vm: &Vm, entry: &Value, word: i32) -> Option<Value> {
    let grades = items(&field(vm, entry, "Grades")?);
    let index = usize::try_from(word & 0xffff).ok()?;
    grades.get(index).cloned()
}

/// A definition reference that names an `InventoryClass` (a definition without one is no answer).
fn definition_of(vm: &mut Vm, reference: Option<Value>) -> Result<Option<ObjRef>, VmError> {
    let Some(reference) = reference else {
        return Ok(None);
    };
    let Some(definition) = load(vm, &reference)? else {
        return Ok(None);
    };
    let has = matches!(
        get(vm, definition, "InventoryClass")?,
        Some(Value::Class(Some(_)) | Value::Object(Some(_)))
    );
    Ok(has.then_some(definition))
}

/// `GetInventoryDefinitionForManufacturerGrade` (C++ `inventoryDefinitionFor`): this balance's own entry for the
/// manufacturer decides; no answer asks the base; a balance already in the walk yields nothing.
fn inventory_definition_for(
    vm: &mut Vm,
    balance: ObjRef,
    manufacturer: ObjRef,
    word: i32,
    active: &mut Vec<ObjRef>,
) -> Result<Option<ObjRef>, VmError> {
    if active.contains(&balance) {
        return Ok(None);
    }
    if active.len() >= MAX_BALANCE_CHAIN {
        return Err(error(format!("{UNIT}: a balance chain deeper than {MAX_BALANCE_CHAIN}")));
    }
    active.push(balance);
    let interpolate = flag(vm, balance, "bInterpolateExpLevel")?;
    let mut answer = None;
    if let Some(entry) = manufacturer_entry(vm, balance, manufacturer)? {
        if interpolate {
            let own = get(vm, balance, "InventoryDefinition")?;
            answer = definition_of(vm, own)?;
        } else if let Some(grade) = grade_of(vm, &entry, word) {
            let custom = field(vm, &grade, "GradeModifiers")
                .and_then(|m| field(vm, &m, "CustomInventoryDefinition"))
                .filter(|c| object_of(c).is_some());
            let reference = match custom {
                Some(c) => Some(c),
                None => get(vm, balance, "InventoryDefinition")?,
            };
            answer = definition_of(vm, reference)?;
        }
    }
    if answer.is_none()
        && let Some(base) = get(vm, balance, "BaseDefinition")?
        && let Some(base) = load(vm, &base)?
    {
        answer = inventory_definition_for(vm, base, manufacturer, word, active)?;
    }
    active.pop();
    Ok(answer)
}

/// `GetExpLevelFromManufacturerData` (C++ `expLevelFor`): 1 by default; an interpolating balance answers the word;
/// otherwise the grade's `ExpLevel` (or the word's high half, at most 63) from this balance's own entry, else the
/// base's answer.
fn exp_level_for(
    vm: &mut Vm,
    balance: ObjRef,
    manufacturer: ObjRef,
    word: i32,
    active: &mut Vec<ObjRef>,
) -> Result<i32, VmError> {
    if active.contains(&balance) {
        return Ok(1);
    }
    if flag(vm, balance, "bInterpolateExpLevel")? {
        return Ok(word);
    }
    if active.len() >= MAX_BALANCE_CHAIN {
        return Err(error(format!("{UNIT}: a balance chain deeper than {MAX_BALANCE_CHAIN}")));
    }
    active.push(balance);
    let mut level = 1;
    let mut resolved = false;
    if let Some(entry) = manufacturer_entry(vm, balance, manufacturer)?
        && let Some(grade) = grade_of(vm, &entry, word)
    {
        let high = (word >> 16) & 0xffff;
        level = if high == 0 {
            match field(vm, &grade, "GradeModifiers").and_then(|m| field(vm, &m, "ExpLevel")) {
                Some(x) => crate::hit::access::low_i32(integer(&x)),
                None => 1,
            }
        } else {
            high.min(MAX_ITEM_LEVEL)
        };
        resolved = true;
    }
    if !resolved
        && let Some(base) = get(vm, balance, "BaseDefinition")?
        && let Some(base) = load(vm, &base)?
    {
        level = exp_level_for(vm, base, manufacturer, word, active)?;
    }
    active.pop();
    Ok(level)
}

/// The candidates of a balance at a stage (C++ `expandBalance`): the balance, then its bases, stopping at the first
/// that gives any; one per manufacturer entry whose first grade window holds `min(stage, cap)` and whose definition
/// the original balance can name; the weight is the grade's modifier interpolated across the window (single
/// precision).
fn expand_balance(
    vm: &mut Vm,
    original: ObjRef,
    stage: i32,
    cap: i32,
    source: Option<ObjRef>,
) -> Result<Vec<Candidate>, VmError> {
    let mut result = Vec::new();
    let s = stage.min(if cap > 0 { cap } else { LEVEL_CAP });
    let interpolate = flag(vm, original, "bInterpolateExpLevel")?;
    let mut visited: Vec<ObjRef> = Vec::new();
    let mut current = Some(original);
    while let Some(c) = current.filter(|c| !visited.contains(c)) {
        if visited.len() >= MAX_BALANCE_CHAIN {
            return Err(error(format!("{UNIT}: a balance chain deeper than {MAX_BALANCE_CHAIN}")));
        }
        visited.push(c);
        for entry in array(vm, c, "Manufacturers")? {
            let listed = field(vm, &entry, "Manufacturer");
            let (Some(listed), Some(grades)) = (listed.filter(|l| object_of(l).is_some()), field(vm, &entry, "Grades"))
            else {
                continue;
            };
            for (g, grade) in items(&grades).iter().enumerate() {
                let (low, high) = match field(vm, grade, "GameStageRequirement") {
                    Some(window) => (int_field(vm, &window, "MinGameStage")?, int_field(vm, &window, "MaxGameStage")?),
                    None => (0, 0),
                };
                if s < low || s > high {
                    continue;
                }
                let Some(manufacturer) = load(vm, &listed)? else {
                    break;
                };
                let word = if interpolate {
                    s
                } else {
                    i32::try_from(g).map_err(|_| error(format!("{UNIT}: too many grades")))?
                };
                if inventory_definition_for(vm, original, manufacturer, word, &mut Vec::new())?.is_none() {
                    break;
                }
                let minimum = evaluate_field(vm, grade, "MinSpawnProbabilityModifier", source)?;
                let maximum = evaluate_field(vm, grade, "MaxSpawnProbabilityModifier", source)?;
                let span = high
                    .checked_sub(low)
                    .ok_or_else(|| error(format!("{UNIT}: a grade window out of range")))?;
                // s - low cannot overflow: low <= s here. Both convert to single as the C++ `float(int)`.
                let weight = if span > 0 {
                    minimum + (s - low) as f32 / span as f32 * (maximum - minimum)
                } else {
                    minimum
                };
                result.push(Candidate {
                    pool: None,
                    balance: Some(original),
                    manufacturer: Some(manufacturer),
                    word,
                    weight,
                    drop: true,
                });
                break;
            }
        }
        if !result.is_empty() {
            break;
        }
        current = match get(vm, c, "BaseDefinition")? {
            Some(base) => load(vm, &base)?,
            None => None,
        };
    }
    Ok(result)
}

// ---------------------------------------------------------------------------------------------------------------
// The pool roll (Reference: combat_loot.cpp's SpawnBalancedInventoryFromPool)
// ---------------------------------------------------------------------------------------------------------------

/// One draw of the Reference generator.
fn rand15(vm: &mut Vm) -> u16 {
    vm.rng.crt(REFERENCE_STREAM).rand15()
}

/// The C++ pool gate: the stage window admits the stage (minimum 0, or at most the stage and the cap; maximum 0, or
/// at least the stage), and some entry can be reached (a nested pool with entries that passes the same gate, or an
/// entry with a balance).
fn pool_passes(
    vm: &mut Vm,
    pool: ObjRef,
    stage: i32,
    cap: i32,
    source: Option<ObjRef>,
    depth: u32,
) -> Result<bool, VmError> {
    if depth > MAX_DEPTH {
        return Ok(false);
    }
    let bound = |vm: &mut Vm, name: &str| -> Result<i32, VmError> {
        let value = match get(vm, pool, name)? {
            Some(attribute) => attribute_value(vm, &attribute, source)?,
            None => None,
        };
        // An unset attribute is 0; C++ `int(std::trunc(float))` (N5).
        Ok(value.map_or(0, trunc_i32))
    };
    let minimum = bound(vm, "MinGameStageRequirement")?;
    let maximum = bound(vm, "MaxGameStageRequirement")?;
    if !((minimum == 0 || (minimum <= stage && minimum <= cap)) && (maximum == 0 || stage <= maximum)) {
        return Ok(false);
    }
    let Some(entries) = get(vm, pool, "BalancedItems")? else {
        return Ok(false);
    };
    for entry in items(&entries) {
        if let Some(nested) = field(vm, &entry, "ItmPoolDefinition")
            && let Some(inner) = load(vm, &nested)?
        {
            let reachable = get(vm, inner, "BalancedItems")?.is_some_and(|e| !items(&e).is_empty());
            if reachable && pool_passes(vm, inner, stage, cap, source, depth + 1)? {
                return Ok(true);
            }
        }
        if field(vm, &entry, "InvBalanceDefinition").as_ref().and_then(object_of).is_some() {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The C++ `spawnFromPool` (Reference only): the records of what the pool makes are appended to `out`; whether the
/// gate passed and every pick found a candidate.
#[allow(clippy::too_many_arguments)]
fn spawn_from_pool(
    vm: &mut Vm,
    pool: ObjRef,
    stage: i32,
    awesome: i32,
    source: Option<ObjRef>,
    may_drop: bool,
    out: &mut Vec<LootItem>,
    depth: u32,
) -> Result<bool, VmError> {
    if depth > MAX_DEPTH {
        return Err(error(format!("{UNIT}: item pools nest too deep")));
    }
    let entries = get(vm, pool, "BalancedItems")?;
    let quantity = required(vm, pool, "Quantity", UNIT)?;
    // C++ `int(std::trunc(float))` (N5).
    let quantity = trunc_i32(evaluate_for(vm, &quantity, source)?);
    if !pool_passes(vm, pool, stage, LEVEL_CAP, source, 0)? {
        return Ok(false);
    }
    let Some(entries) = entries else {
        return Ok(false);
    };
    let mut candidates: Vec<Candidate> = Vec::new();
    for entry in items(&entries) {
        let nested = field(vm, &entry, "ItmPoolDefinition");
        let balance = field(vm, &entry, "InvBalanceDefinition");
        let drop = field(vm, &entry, "bDropOnDeath").is_some_and(|d| truth(vm, &d));
        let weight = evaluate_field(vm, &entry, "Probability", source)?;
        let mut inner = match nested {
            Some(n) => load(vm, &n)?,
            None => None,
        };
        if let Some(i) = inner
            && !get(vm, i, "BalancedItems")?.is_some_and(|e| !items(&e).is_empty())
        {
            inner = None; // an empty pool counts as absent
        }
        if let Some(inner) = inner {
            let weight = if pool_passes(vm, inner, stage, LEVEL_CAP, source, 0)? {
                weight
            } else {
                0.0
            };
            if weight > WEIGHT_FLOOR {
                candidates.push(Candidate {
                    pool: Some(inner),
                    balance: None,
                    manufacturer: None,
                    word: 0,
                    weight,
                    drop,
                });
            }
        } else if let Some(b) = balance
            && let Some(loaded_balance) = load(vm, &b)?
        {
            let expanded = expand_balance(vm, loaded_balance, stage, LEVEL_CAP, source)?;
            // At most a few hundred candidates; the count converts to single exactly (C++ `float(size())`).
            let n = expanded.len() as f32;
            for mut c in expanded {
                c.weight = c.weight * weight / n;
                c.drop = drop;
                if c.weight > WEIGHT_FLOOR {
                    candidates.push(c);
                }
            }
        }
    }
    let total = candidates.iter().fold(0.0f32, |t, c| t + c.weight);
    let mut ok = true;
    for _ in 0..quantity.max(0) {
        if candidates.is_empty() {
            ok = false;
            continue;
        }
        let r = total * (f32::from(rand15(vm)) / 32_767.0);
        let mut sum = 0.0f32;
        let mut picked = None;
        for c in &candidates {
            if r >= sum && r <= sum + c.weight {
                picked = Some(c.clone());
                break;
            }
            sum += c.weight;
        }
        let Some(picked) = picked else {
            ok = false;
            continue;
        };
        if let Some(inner) = picked.pool {
            spawn_from_pool(vm, inner, stage, awesome, source, picked.drop && may_drop, out, depth + 1)?;
            continue;
        }
        let (Some(balance), Some(manufacturer)) = (picked.balance, picked.manufacturer) else {
            ok = false;
            continue;
        };
        let definition = inventory_definition_for(vm, balance, manufacturer, picked.word, &mut Vec::new())?;
        let class_value = match definition {
            Some(d) => get(vm, d, "InventoryClass")?,
            None => None,
        };
        let Some(class_value) = class_value else {
            ok = false;
            continue;
        };
        let level = exp_level_for(vm, balance, manufacturer, picked.word, &mut Vec::new())?.min(MAX_ITEM_LEVEL);
        let (class, class_name) = match &class_value {
            Value::Class(Some(k)) => {
                let path = vm
                    .space
                    .classes
                    .get_layout(*k)
                    .map(|l| l.path().as_str().to_string())
                    .unwrap_or_default();
                (Some(*k), leaf(&path).to_string())
            }
            other => (None, leaf(&path_of(vm, object_of(other))).to_string()),
        };
        out.push(LootItem {
            pool: path_of(vm, Some(pool)),
            balance: path_of(vm, Some(balance)),
            manufacturer: path_of(vm, Some(manufacturer)),
            class_name,
            class,
            level,
            drop_on_death: picked.drop && may_drop,
            dropped: false,
        });
    }
    Ok(ok)
}

/// The Off-mode answer of the pool native (module documentation).
fn off_mode_pool() -> VmError {
    NotYet::not_ported(
        "ItemPool.SpawnBalancedInventoryFromPool in Off mode: ow_items::pools::roll_pool evaluates with no context \
         source where the native evaluates with ContextSource, takes the variance as registry data where the native \
         gets the caller's GameStageVarianceFormula object, and does not report the native's return value (whether \
         the gate passed and every pick found a candidate); contract items for W4-POOLS (decision 2026-11-16-41)",
        RECORD,
    )
    .into()
}

/// `ItemPool.SpawnBalancedInventoryFromPool(Definition, GameStage, AwesomeLevel, ContextSource, out
/// SpawnedInventory, GameStageVarianceFormula, OuterPoolChance, bInventoryMayDropOnDeath)` (static; module
/// documentation). Reference: the C++ stand-in inventories; Off: `NotYet` before anything is drawn.
fn spawn_from_pool_native(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    if c.vm.parity == ParityMode::Off {
        return Err(off_mode_pool());
    }
    let Some(pool) = arg_object(c, 0)? else {
        return Ok(Value::Bool(false));
    };
    let pool = loaded(c.vm, pool)?;
    if c.has(5) && arg_object(c, 5)?.is_some() {
        not_implemented(c.vm, "SpawnBalancedInventoryFromPool: a game stage variance formula", RECORD)?;
    }
    // An omitted flag is true; one passed as None is true too (the C++ `kind == None`).
    let may_drop = !c.has(7) || matches!(c.arg(7)?, Value::None) || truth(c.vm, c.arg(7)?);
    let stage = crate::hit::access::low_i32(integer(c.arg(1)?));
    let awesome = crate::hit::access::low_i32(integer(c.arg(2)?));
    let source = arg_object(c, 3)?;
    let mut made = Vec::new();
    let ok = spawn_from_pool(c.vm, pool, stage, awesome, source, may_drop, &mut made, 0)?;
    let mut spawned = match c.arg(4)? {
        v @ Value::Array(_) => items(v),
        _ => Vec::new(),
    };
    for item in &made {
        // The stand-in inventory (Reference only); the C++ ignores an object it cannot make.
        let class = match item.class {
            Some(k) => Ok(k),
            None => c.vm.find_class("Engine.Inventory"),
        };
        let Ok(class) = class else { continue };
        let Ok(object) = ow_vm::interp::instantiate_from_defaults(c.vm, class) else {
            continue;
        };
        crate::hit::access::set(c.vm, object, "bDropOnDeath", Value::Bool(item.drop_on_death))?;
        spawned.push(Value::Object(Some(object)));
    }
    c.set_out(4, Value::array(spawned))?;
    Ok(Value::Bool(ok))
}

// ---------------------------------------------------------------------------------------------------------------
// The death roll (Reference: WillowPawn.DropLootOnDeath overridden by the script's loop)
// ---------------------------------------------------------------------------------------------------------------

/// The pools of one death roll after the chance draws (module documentation).
fn roll_pools(
    vm: &mut Vm,
    pawn: ObjRef,
    drop: &mut LootDrop,
    weapon: Option<ClassId>,
) -> Result<usize, VmError> {
    let controller = get(vm, pawn, "Controller")?;
    let list = get(vm, pawn, "ItemPoolList")?.map(|v| items(&v)).unwrap_or_default();
    let mut scaled = false;
    let mut skipped = 0;
    for entry in list {
        let Some(reference) = field(vm, &entry, "ItemPool") else {
            continue;
        };
        let Some(pool) = load(vm, &reference)? else {
            continue;
        };
        if flag(vm, pool, "bAutoReadyItems")? {
            skipped = count_up(skipped, "carried-gear pools")?;
            continue;
        }
        let mut rolled = LootPool {
            pool: path_of(vm, Some(pool)),
            roll: f64::from(frand_f32(vm.rng.crt(REFERENCE_STREAM))),
            ..LootPool::default()
        };
        let attempt = (|| -> Result<(), VmError> {
            let probability = field(vm, &entry, "PoolProbability");
            let (Some(probability), Some(controller)) = (probability, controller.as_ref()) else {
                return Err(error("the pool entry has no probability or the pawn no controller"));
            };
            rolled.chance = f64::from(evaluate_for(vm, &probability, object_of(controller))?);
            let definition = field(vm, &probability, "InitializationDefinition")
                .as_ref()
                .and_then(object_of);
            if definition.is_some()
                && leaf(&path_of(vm, definition)) == "AmmoDrops_PerPlayer"
                && !scaled
            {
                rolled.chance *= AMMO_DROPS_PER_PLAYER_MULTIPLIER;
                scaled = true;
            }
            rolled.tried = rolled.chance > 0.0 && rolled.roll <= rolled.chance;
            if rolled.tried {
                // The items go straight into the record, as the C++ `spawnFromPool` appends them: an error part way
                // keeps the ones made before it (none of them dropped).
                rolled.returned =
                    spawn_from_pool(vm, pool, drop.stage, drop.awesome, Some(pawn), true, &mut rolled.items, 0)?;
                for item in &mut rolled.items {
                    let is_weapon = match (item.class, weapon) {
                        (Some(k), Some(w)) => vm.space.classes.is_child_of(k, w),
                        _ => false,
                    };
                    item.dropped = is_dropped(item.drop_on_death, drop.below_kill_z, is_weapon);
                }
            }
            Ok(())
        })();
        match attempt {
            Ok(()) => {}
            // Three kinds stop the roll (review r1 and r2 of PR #340): a `NotYet` (an unread resolver, a missing
            // layer: the C++ evaluates or catches there, the port does not guess), a borrowed service (a wiring
            // bug), and the step budget (the run's harness limit, not this pool's data).
            Err(e @ (VmError::NotYet(_) | VmError::ServiceBorrowed(_) | VmError::StepLimit)) => return Err(e),
            // Everything else is what the C++ catches as its `RuntimeError`: a rule error of this unit, a data
            // error of the attribute system, the definition store or the heap (a definition without a field the
            // evaluator reads, a reference that does not load). It is reported on this pool, nothing is dropped for
            // it, and the roll goes on.
            Err(e) => {
                rolled.error = error_text(&e);
                rolled.tried = false;
            }
        }
        drop.pools.push(rolled);
    }
    Ok(skipped)
}

/// `n + 1`, or an error naming what overflowed (checked arithmetic over data).
fn count_up(n: usize, what: &str) -> Result<usize, VmError> {
    n.checked_add(1)
        .ok_or_else(|| error(format!("{UNIT}: too many {what}")))
}

/// The script's drop test: an item with its flag set is dropped unless the pawn is below the level's `KillZ` and the
/// item is a weapon (the others are destroyed).
fn is_dropped(drop_on_death: bool, below_kill_z: bool, is_weapon: bool) -> bool {
    drop_on_death && !(below_kill_z && is_weapon)
}

/// Whether the pawn's `Z` is below the world's `KillZ`; false when either is missing.
fn below_kill_z(z: Option<f64>, kill_z: Option<f64>) -> bool {
    match (z, kill_z) {
        (Some(z), Some(k)) => z < k,
        _ => false,
    }
}

/// The C++ `dropLoot`: the death roll of `pawn`, logged and recorded. A pawn without the item stage or the awesome
/// level (only an AI pawn has them) drops nothing. Reference only (the override of the installed script); Off mode
/// is `NotYet`, as the installed script is what runs there.
pub fn drop_loot(vm: &mut Vm, pawn: ObjRef) -> Result<(), VmError> {
    if vm.parity == ParityMode::Off {
        return Err(NotYet::not_ported(
            "the C++ DropLootOnDeath override (Reference only: Off mode runs the installed script)",
            RECORD,
        )
        .into());
    }
    let (Some(stage), Some(awesome)) = (
        get(vm, pawn, "GameStageForSpawnedInventory")?,
        get(vm, pawn, "AwesomeLevel")?,
    ) else {
        return Ok(());
    };
    let world = with_state(vm, |s| s.world)?;
    let z = match get(vm, pawn, "Location")? {
        Some(location) => field(vm, &location, "Z"),
        None => None,
    };
    let kill_z = match world {
        Some(w) => get(vm, w, "KillZ")?,
        None => None,
    };
    let below_kill_z = below_kill_z(z.as_ref().map(number), kill_z.as_ref().map(number));
    let weapon = vm.find_class("WillowGame.WillowWeapon").ok();
    let mut drop = LootDrop {
        pawn,
        stage: crate::hit::access::low_i32(integer(&stage)),
        awesome: crate::hit::access::low_i32(integer(&awesome)),
        below_kill_z,
        pools: Vec::new(),
    };
    let skipped = roll_pools(vm, pawn, &mut drop, weapon)?;
    let mut tried = 0usize;
    let mut dropped = 0usize;
    for rolled in &drop.pools {
        if !rolled.error.is_empty() {
            note(vm, format!("loot {}: not evaluated ({})", leaf(&rolled.pool), rolled.error))?;
            continue;
        }
        if !rolled.tried {
            continue;
        }
        tried = count_up(tried, "tried pools")?;
        let mut text = format!(
            "loot {} chance={} roll={} returned {}",
            leaf(&rolled.pool),
            fmt_f(rolled.chance, 6),
            fmt_f(rolled.roll, 6),
            if rolled.returned { "true" } else { "false" }
        );
        for item in &rolled.items {
            text.push_str(&format!(
                " {}@{}{}",
                leaf(&item.balance),
                item.level,
                if item.dropped { "" } else { " (destroyed)" }
            ));
            if item.dropped {
                dropped = count_up(dropped, "dropped items")?;
            }
        }
        note(vm, text)?;
    }
    note(
        vm,
        format!(
            "DropLootOnDeath: item stage {}, {} pools rolled ({skipped} carried-gear pools skipped), {tried} tried, \
             {dropped} items dropped (records only: the VM has no pickups)",
            drop.stage,
            drop.pools.len()
        ),
    )?;
    push_drop(vm, drop)
}

fn drop_loot_native(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let Some(pawn) = c.this else {
        return Ok(Value::None);
    };
    drop_loot(c.vm, pawn)?;
    Ok(Value::None)
}

// ---------------------------------------------------------------------------------------------------------------
// Reports (the run's JSON and rows, C++ `lootJson` and the row keys)
// ---------------------------------------------------------------------------------------------------------------

/// The C++ `number()` of `combat_run.cpp` (`setprecision(9)`).
fn num(x: f64) -> String {
    fmt_g(x, 9)
}

fn yes(b: bool) -> &'static str {
    if b { "true" } else { "false" }
}

/// A death roll as JSON (C++ `lootJson`).
pub fn loot_json(drop: &LootDrop) -> String {
    let mut out = format!(
        "{{\"item_stage\":{},\"awesome\":{},\"below_kill_z\":{},\"pools\":[",
        drop.stage,
        drop.awesome,
        yes(drop.below_kill_z)
    );
    for (i, pool) in drop.pools.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            "{{\"pool\":{},\"chance\":{},\"roll\":{},\"tried\":{},\"returned\":{}",
            json_quote(&pool.pool),
            num(pool.chance),
            num(pool.roll),
            yes(pool.tried),
            yes(pool.returned)
        ));
        if !pool.error.is_empty() {
            out.push_str(&format!(",\"error\":{}", json_quote(&pool.error)));
        }
        out.push_str(",\"items\":[");
        for (k, item) in pool.items.iter().enumerate() {
            if k > 0 {
                out.push(',');
            }
            out.push_str(&format!(
                "{{\"balance\":{},\"manufacturer\":{},\"class\":{},\"level\":{},\"drop_on_death\":{},\"dropped\":{}}}",
                json_quote(&item.balance),
                json_quote(&item.manufacturer),
                json_quote(&item.class_name),
                item.level,
                yes(item.drop_on_death),
                yes(item.dropped)
            ));
        }
        out.push_str("]}");
    }
    out.push_str("]}");
    out
}

/// The rows of one death roll after a hit (C++ `lootAfter`), keyed `loot.<key>.*`, in the C++ order.
pub fn loot_rows(drop: &LootDrop, key: &str) -> Vec<(String, f64)> {
    let mut rows = Vec::new();
    let mut tried = 0usize;
    let mut dropped = 0usize;
    for pool in drop.pools.iter().filter(|p| p.tried) {
        tried += 1;
        dropped += pool.items.iter().filter(|i| i.dropped).count();
        let name = leaf(&pool.pool);
        rows.push((format!("loot.{key}.{name}.returned"), if pool.returned { 1.0 } else { 0.0 }));
        // Counts convert to double exactly (far below 2^53).
        rows.push((format!("loot.{key}.{name}.items"), pool.items.len() as f64));
    }
    rows.push((format!("loot.{key}.item_stage"), f64::from(drop.stage)));
    rows.push((format!("loot.{key}.pools_tried"), tried as f64));
    rows.push((format!("loot.{key}.items_dropped"), dropped as f64));
    rows
}

/// The sums of one pool over a loot batch (C++ `lootBatch`'s `Sum`).
#[derive(Debug, Clone)]
struct Sum {
    chance: f64,
    chance_max: f64,
    tried: f64,
    returned: f64,
    items: f64,
    dropped: f64,
    errors: f64,
}

impl Default for Sum {
    fn default() -> Self {
        Sum {
            chance: -1.0,
            chance_max: 0.0,
            tried: 0.0,
            returned: 0.0,
            items: 0.0,
            dropped: 0.0,
            errors: 0.0,
        }
    }
}

/// What a loot batch summed (C++ `lootBatch`): fed one death roll at a time, then [`Batch::rows`].
#[derive(Debug, Default)]
pub struct Batch {
    kills: u32,
    level_mismatches: u32,
    sums: ow_core::DetMap<String, Sum>,
    stages: ow_core::DetMap<i32, u32>,
    quantities: ow_core::DetMap<usize, u32>,
}

impl Batch {
    pub fn new() -> Self {
        Batch::default()
    }

    /// Adds one kill's death roll. A counter past `u32` is an error (checked arithmetic).
    pub fn add(&mut self, drop: &LootDrop) -> Result<(), VmError> {
        let up = |n: u32, what: &str| -> Result<u32, VmError> {
            n.checked_add(1)
                .ok_or_else(|| error(format!("{UNIT}: a loot batch counts too many {what}")))
        };
        self.kills = up(self.kills, "kills")?;
        let at_stage = self.stages.entry(drop.stage).or_insert(0);
        *at_stage = up(*at_stage, "kills at one stage")?;
        for pool in &drop.pools {
            let sum = self.sums.entry(leaf(&pool.pool).to_string()).or_default();
            sum.chance = if sum.chance < 0.0 { pool.chance } else { sum.chance.min(pool.chance) };
            sum.chance_max = sum.chance_max.max(pool.chance);
            sum.tried += f64::from(u8::from(pool.tried));
            sum.errors += f64::from(u8::from(!pool.error.is_empty()));
            sum.returned += f64::from(u8::from(pool.tried && pool.returned));
            // Counts convert to double exactly.
            sum.items += pool.items.len() as f64;
            for item in &pool.items {
                sum.dropped += f64::from(u8::from(item.dropped));
                if item.level != drop.stage {
                    self.level_mismatches = up(self.level_mismatches, "level mismatches")?;
                }
            }
            if pool.tried && pool.pool.contains("Pool_Money_1or2") {
                let n = self.quantities.entry(pool.items.len()).or_insert(0);
                *n = up(*n, "money quantities")?;
            }
        }
        Ok(())
    }

    /// The kills added.
    pub fn kills(&self) -> u32 {
        self.kills
    }

    /// The batch's rows, keyed `loot.batch.<tag>.*`, in the C++ order.
    pub fn rows(&self, tag: &str) -> Vec<(String, f64)> {
        let key = format!("loot.batch.{tag}");
        let mut rows = vec![(format!("{key}.n"), f64::from(self.kills))];
        for (stage, n) in &self.stages {
            rows.push((format!("{key}.stage.{stage}"), f64::from(*n)));
        }
        if let (Some((low, _)), Some((high, _))) = (self.stages.iter().next(), self.stages.iter().next_back()) {
            rows.push((format!("{key}.stage_min"), f64::from(*low)));
            rows.push((format!("{key}.stage_max"), f64::from(*high)));
        }
        rows.push((format!("{key}.level_mismatch"), f64::from(self.level_mismatches)));
        for (name, sum) in &self.sums {
            rows.push((format!("{key}.{name}.chance"), sum.chance));
            rows.push((format!("{key}.{name}.chance_max"), sum.chance_max));
            rows.push((format!("{key}.{name}.tried"), sum.tried));
            rows.push((format!("{key}.{name}.true"), sum.returned));
            rows.push((format!("{key}.{name}.items"), sum.items));
            rows.push((format!("{key}.{name}.dropped"), sum.dropped));
            if sum.errors > 0.0 {
                rows.push((format!("{key}.{name}.errors"), sum.errors));
            }
        }
        for (quantity, n) in &self.quantities {
            rows.push((format!("{key}.Pool_Money_1or2.qty.{quantity}"), f64::from(*n)));
        }
        rows
    }
}

/// The `rollpool` step's inventories as JSON (C++ `rollPoolSteps`): each object's class name and `bDropOnDeath`.
pub fn inventories_json(vm: &mut Vm, inventories: &[Value]) -> Result<String, VmError> {
    let mut out = String::from("[");
    for (i, v) in inventories.iter().enumerate() {
        let Some(o) = object_of(v) else {
            return Err(error("combat-run: a spawned inventory that is not an object"));
        };
        let flag = get(vm, o, "bDropOnDeath")?.is_some_and(|f| truth(vm, &f));
        if i > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            "{{\"class\":{},\"drop_on_death\":{}}}",
            json_quote(&crate::hit::access::class_name(vm, o)),
            yes(flag)
        ));
    }
    out.push(']');
    Ok(out)
}

#[cfg(test)]
mod tests;
