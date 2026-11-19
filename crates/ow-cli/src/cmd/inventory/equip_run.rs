//! Owner: lane FIX-EQUIP-NATIVES. The installed equip run (Rust-only): `<package> --inventory-move --equip-run
//! <weapons file> --cooked <dir> --output <file>`. AI-assisted (Claude Code).
//!
//! It runs the installed script a looted gun goes through, on a gameplay world, and reports where each phase stops:
//! the lane's stop list (decision 2026-11-19-03). The weapons file is `--script-diff`'s (`<weapon type> <level>
//! <balance> [<Slot>=<part>]...`). One world over the cooked folder: W5-WORLD's packs, then `ow_combat::hit::Pack`
//! (the Off pool natives `--combat-run` binds: `ShutDown`, the idle timer) and `ow_inventory::manager::Pack`, a `WorldInfo` from its class defaults placed first, with the VM's default stub policy (`Refuse`, nothing allowed), so a native the
//! script reaches that nothing binds stops the phase with the VM's `UnboundNative` text. No native is bound here.
//!
//! Per weapon (each phase runs even when an earlier one stopped, so one run lists every phase's stop):
//! - **make**: a `WillowWeapon` from its class defaults with the line's definition data and `ExpLevel`, as
//!   `--script-diff` makes it; its owner is a `WillowPlayerPawn` (`Instigator` and `Owner`) whose `Controller` is a
//!   `WillowPlayerController` (whose `Pawn` is the pawn) and whose `InvManager` is a `WillowInventoryManager` (whose
//!   `Owner` is the pawn), each from its class defaults. These links stand in for the pickup and `AddInventory`
//!   (lane H-INTERACT, the host side), which the run does not play: a harness choice, not a rule. The controller
//!   also gets its accuracy pool: the Off `ResourcePoolManager.CreatePool` native on Maya's class's
//!   `AccuracyPoolDefinition` ([`ACCURACY_POOL`]), written to `ResourcePoolManager` and `AccuracyPool`. The game makes
//!   that pool in the player's own bring-up, which no script of the equip path reaches and no record reads (W5-MAPLOAD
//!   and the pools lanes); without it the weapon's accuracy effects find no pool and fall back to the controller
//!   source, where `WeaponResourcePoolAttributeContextResolver` is not read. A harness stand-in, not a rule.
//! - **init**: the installed `WillowWeapon.InitializeInternal(false)`, the spawn's script (bases, ammo, the attribute
//!   slots, the weapon's own effects and slot modifiers, the card).
//! - **bases**: `CalculateWeaponBaseValues` (the crate's override), so the equip phases start from computed bases
//!   when `init` stopped early.
//! - **equip**: the installed `WillowWeapon.ApplyAllExternalAttributeEffects`, what `WeaponEquipping.BeginState`
//!   calls (after `ActivateAttributeSlotSkill`, not run here).
//! - **unequip**: the installed `WillowWeapon.RemoveAllExternalAttributeEffects`.
//!
//! One JSON line per weapon goes to `--output` (each phase's error or null, and the external records after equip
//! and after unequip), and stdout gets a summary: the weapon count, each phase's pass count, and each phase's stops
//! with how many weapons stopped there. Exit 0 when every phase of every weapon passed, else 1. Output names game
//! objects: write it under ignored `local/` only.

use std::collections::BTreeMap;
use std::path::Path;
use std::rc::Rc;

use ow_attr::AttributeSystem;
use ow_core::ParityMode;
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;
use ow_vm::interp::instantiate_from_defaults;

use super::{DiffRequest, diff_request, set_field, vm_text};
use crate::args::USAGE;
use crate::cmd::properties::error_text;
use crate::json::quote;

/// The phases, in run order.
pub const PHASES: [&str; 4] = ["init", "bases", "equip", "unequip"];

const WEAPON: &str = "WillowGame.WillowWeapon";
const PAWN: &str = "WillowGame.WillowPlayerPawn";
const CONTROLLER: &str = "WillowGame.WillowPlayerController";
const MANAGER: &str = "WillowGame.WillowInventoryManager";
const INITIALIZE: &str = "WillowGame.WillowWeapon.InitializeInternal";
const EQUIP: &str = "WillowGame.WillowWeapon.ApplyAllExternalAttributeEffects";
const UNEQUIP: &str = "WillowGame.WillowWeapon.RemoveAllExternalAttributeEffects";
const CREATE_POOL: &str = "Engine.ResourcePoolManager.CreatePool";
/// Maya's class's `AccuracyPoolDefinition` (`GD_Siren.Character.CharClass_Siren`, read with `--def-dump`).
pub const ACCURACY_POOL: &str = "D_Resourcepools.PlayerPools.AccuracyPool";

/// The command (module documentation).
pub(super) fn equip_run(args: &[String], mode: ParityMode, out: &mut String) -> Result<u8, String> {
    let mut list = None;
    let mut cooked = None;
    let mut output = None;
    let mut i = 2;
    while i < args.len() {
        let value = args.get(i + 1).cloned().ok_or_else(|| USAGE.to_string());
        match args[i].as_str() {
            "--equip-run" => list = Some(value?),
            "--cooked" => cooked = Some(value?),
            "--output" => output = Some(value?),
            _ => return Err(USAGE.to_string()),
        }
        i += 2;
    }
    let (Some(list), Some(cooked), Some(output)) = (list, cooked, output) else {
        return Err(USAGE.to_string());
    };
    let text = std::fs::read_to_string(&list).map_err(|e| format!("{list}: {e}"))?;
    let mut world = run_world(Path::new(&cooked), mode)?;
    let mut weapons = 0usize;
    let mut passed = [0usize; PHASES.len()];
    let mut stops: [BTreeMap<String, usize>; PHASES.len()] = Default::default();
    let mut lines = String::new();
    for (n, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        weapons += 1;
        let request = diff_request(line).map_err(|e| format!("line {}: {e}", n + 1))?;
        let run = run_weapon(&mut world.vm, &request).map_err(|e| format!("line {}: {e}", n + 1))?;
        let mut fields = Vec::new();
        for (k, phase) in PHASES.iter().enumerate() {
            match &run.errors[k] {
                None => passed[k] += 1,
                Some(e) => *stops[k].entry(e.clone()).or_default() += 1,
            }
            fields.push(format!(
                "{}:{}",
                quote(phase),
                run.errors[k].as_deref().map_or_else(|| "null".into(), quote)
            ));
        }
        lines.push_str(&format!(
            "{{\"line\":{},\"weapon_type\":{},{},\"records_after_equip\":{},\"records_after_unequip\":{}}}\n",
            n + 1,
            quote(&request.weapon_type),
            fields.join(","),
            run.after_equip,
            run.after_unequip,
        ));
    }
    std::fs::write(&output, lines).map_err(|e| format!("{output}: {e}"))?;
    out.push_str(&summary(weapons, &passed, &stops));
    let all = passed.iter().all(|p| *p == weapons);
    Ok(if all { 0 } else { 1 })
}

/// The summary line: weapons, passes per phase, and each phase's stops with their weapon counts.
fn summary(weapons: usize, passed: &[usize], stops: &[BTreeMap<String, usize>]) -> String {
    let mut phases = Vec::new();
    for (k, phase) in PHASES.iter().enumerate() {
        let listed: Vec<String> = stops[k]
            .iter()
            .map(|(e, c)| format!("{{\"stop\":{},\"weapons\":{c}}}", quote(e)))
            .collect();
        phases.push(format!(
            "{}:{{\"passed\":{},\"stops\":[{}]}}",
            quote(phase),
            passed[k],
            listed.join(",")
        ));
    }
    format!("{{\"weapons\":{weapons},{}}}\n", phases.join(","))
}

/// The run's world (module documentation).
fn run_world(cooked: &Path, mode: ParityMode) -> Result<ow_world::world::World, String> {
    run_world_typed(cooked, mode).map_err(|e| e.text())
}

/// Why the run's world could not be set up, typed (review r2 of #470, item 4): a `NotYet` of any layer, or any other
/// VM error, stays a [`VmError`], so a caller grading by variant keeps it; the rest is text.
#[derive(Debug)]
pub(crate) enum SetupError {
    Vm(VmError),
    Other(String),
}

impl SetupError {
    /// The text the untyped callers have always printed.
    pub(crate) fn text(&self) -> String {
        match self {
            SetupError::Vm(e) => vm_text(e),
            SetupError::Other(t) => t.clone(),
        }
    }
}

impl From<VmError> for SetupError {
    fn from(e: VmError) -> SetupError {
        SetupError::Vm(e)
    }
}

/// A layer's error as a [`SetupError`]: its `NotYet` (`as_not_yet`) as `VmError::NotYet`, anything else as its text.
fn setup_error<E: std::error::Error>(e: E, as_not_yet: impl FnOnce(&E) -> Option<ow_core::NotYet>) -> SetupError {
    match as_not_yet(&e) {
        Some(n) => SetupError::Vm(VmError::NotYet(n)),
        None => SetupError::Other(error_text(&e)),
    }
}

fn run_world_typed(cooked: &Path, mode: ParityMode) -> Result<ow_world::world::World, SetupError> {
    use ow_balance::types::{
        AwesomeLevel, BalanceError, BalanceSnapshot, ExpLevel, GameStage, LevelCapUpgrades, OpLevel, PlayerCount,
        Playthrough,
    };
    let def = |e: ow_defs::store::DefError| {
        setup_error(e, |e| match e {
            ow_defs::store::DefError::NotYet(n) => Some(n.clone()),
            _ => None,
        })
    };
    let attr = |e: ow_attr::api::AttrError| {
        setup_error(e, |e| match e {
            ow_attr::api::AttrError::NotYet(n) => Some(n.clone()),
            _ => None,
        })
    };
    let balance = |e: BalanceError| {
        setup_error(e, |e| match e {
            BalanceError::NotYet(n) => Some(n.clone()),
            _ => None,
        })
    };
    let defs = DefStore::open(cooked.to_path_buf(), IdentityPolicy::FirstInScanOrder, mode).map_err(def)?;
    let attrs = AttributeSystem::standard().map_err(attr)?;
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(attrs), mode);
    ow_vm::natives_core::register(&mut vm)?;
    let snapshot = BalanceSnapshot::new(
        Playthrough::NORMAL,
        OpLevel::new(0).map_err(balance)?,
        PlayerCount::SINGLE,
        ExpLevel::new(1),
        GameStage(1),
        AwesomeLevel::new(0),
        LevelCapUpgrades::ALL,
    )
    .map_err(balance)?;
    let combat = ow_combat::hit::Pack;
    let inventory = ow_inventory::manager::Pack;
    let packs: [&dyn ow_vm::api::NativePack; 2] = [&combat, &inventory];
    let world = ow_world::world::World::new(vm, ow_world::world::BalanceState::new(snapshot), &packs).map_err(
        |e| match e {
            ow_world::world::WorldError::Vm(v) => SetupError::Vm(v),
            other => SetupError::Other(error_text(&other)),
        },
    )?;
    place_world_info(world)
}

/// The level's `WorldInfo`, from its class defaults (as `--combat-run` places one without its bridge): the pool
/// native spawns its manager only in a world that has one.
fn place_world_info(mut world: ow_world::world::World) -> Result<ow_world::world::World, SetupError> {
    let class = world.vm.find_class("Engine.WorldInfo")?;
    let info = instantiate_from_defaults(&mut world.vm, class)?;
    world
        .actors()
        .try_borrow_mut()
        .map_err(|_| SetupError::Other("the actor list is borrowed".to_string()))?
        .place_world_info(info)?;
    Ok(world)
}

/// [`run_world`] with its errors typed, for the Off pickup gate's set-up only (reviews r1 and r2 of #476): the shared
/// runner keeps its own diagnostics.
fn run_world_t(cooked: &Path, mode: ParityMode) -> Result<ow_world::world::World, VmError> {
    use ow_balance::types::{
        AwesomeLevel, BalanceSnapshot, ExpLevel, GameStage, LevelCapUpgrades, OpLevel, PlayerCount,
        Playthrough,
    };
    let defs = DefStore::open(cooked.to_path_buf(), IdentityPolicy::FirstInScanOrder, mode)?;
    let attrs = AttributeSystem::standard()?;
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(attrs), mode);
    ow_vm::natives_core::register(&mut vm)?;
    let balance = |e: &dyn std::error::Error| script(error_text(e));
    let snapshot = BalanceSnapshot::new(
        Playthrough::NORMAL,
        OpLevel::new(0).map_err(|e| balance(&e))?,
        PlayerCount::SINGLE,
        ExpLevel::new(1),
        GameStage(1),
        AwesomeLevel::new(0),
        LevelCapUpgrades::ALL,
    )
    .map_err(|e| balance(&e))?;
    let combat = ow_combat::hit::Pack;
    let inventory = ow_inventory::manager::Pack;
    let packs: [&dyn ow_vm::api::NativePack; 2] = [&combat, &inventory];
    let world = ow_world::world::World::new(vm, ow_world::world::BalanceState::new(snapshot), &packs).map_err(|e| match e {
        ow_world::world::WorldError::Vm(v) => v,
        other => script(error_text(&other)),
    })?;
    place_world_info_t(world)
}

/// [`place_world_info`] with its errors typed (the pickup gate's set-up).
fn place_world_info_t(mut world: ow_world::world::World) -> Result<ow_world::world::World, VmError> {
    let info = make_t(&mut world.vm, "Engine.WorldInfo")?;
    world
        .actors()
        .try_borrow_mut()
        .map_err(|_| VmError::ServiceBorrowed("Actors"))?
        .place_world_info(info)?;
    Ok(world)
}

/// What one weapon's run saw.
struct EquipRun {
    /// Each phase's error, in [`PHASES`] order.
    errors: [Option<String>; PHASES.len()],
    /// `ExternalAttributeModifiers`' length after equip and after unequip.
    after_equip: usize,
    after_unequip: usize,
}

fn records(vm: &Vm, weapon: ObjRef) -> usize {
    match ow_inventory::equip::read_prop(vm, weapon, "ExternalAttributeModifiers") {
        Some(Value::Array(items)) => items.len(),
        _ => 0,
    }
}

/// Runs one script function on the weapon; the phase's typed error, if any.
fn phase(vm: &mut Vm, path: &str, weapon: ObjRef, args: Vec<Value>) -> Option<ow_vm::error::VmError> {
    let f = match vm.find_function(path) {
        Ok(f) => f,
        Err(e) => return Some(e),
    };
    vm.call(f, Some(weapon), args).err()
}

/// The controller's accuracy pool (module documentation): `CreatePool(Mgr, controller, ACCURACY_POOL, true)`, the
/// manager written to `ResourcePoolManager` and the reference to `AccuracyPool`.
fn accuracy_pool(vm: &mut Vm, controller: ObjRef) -> Result<(), VmError> {
    let def = find_object_t(vm, ACCURACY_POOL)?;
    let f = vm.find_function(CREATE_POOL)?;
    let made = vm.call(
        f,
        None,
        vec![
            Value::Object(None),
            Value::Object(Some(controller)),
            Value::Object(Some(def)),
            Value::Bool(true),
        ],
    )?;
    let manager = made.outs.first().cloned().ok_or_else(|| script("the accuracy pool: CreatePool gave no manager".into()))?;
    write_t(vm, controller, "ResourcePoolManager", manager)?;
    write_t(vm, controller, "AccuracyPool", made.ret)
}

/// The installed object at `path`, its lookup's error kept typed (review r3 of #467): a definition store `NotYet` stays
/// a `NotYet` (`VmError::from`), with its kind and record; an object no package holds is a script error of the run.
fn find_object_t(vm: &mut Vm, path: &str) -> Result<ObjRef, VmError> {
    let p = ow_core::ObjectPath::parse(path).map_err(|e| script(format!("{path}: {e}")))?;
    let found = vm.defs.find(&mut vm.space, &p);
    vm.log.extend(vm.defs.take_log());
    found.map_err(VmError::from)?.ok_or_else(|| script(format!("{path}: no package holds it")))
}

/// A preparation error that is not the VM's own, as a script error of the run.
fn script(message: String) -> VmError {
    VmError::Script { function: "equip-run".into(), message }
}

fn write_t(vm: &mut Vm, object: ObjRef, name: &str, v: Value) -> Result<(), VmError> {
    let class = vm.space.heap.get(object)?.class;
    let p = vm
        .space
        .prop(class, name)
        .ok_or_else(|| script(format!("{} declares no {name}", ow_attr::eval::class_path(&vm.space, object))))?;
    Ok(vm.space.write(object, PropAddr::scalar(p), v)?)
}

fn make_t(vm: &mut Vm, class: &str) -> Result<ObjRef, VmError> {
    let c = vm.find_class(class)?;
    instantiate_from_defaults(vm, c)
}

fn run_weapon(vm: &mut Vm, r: &DiffRequest) -> Result<EquipRun, String> {
    let weapon = prepare(vm, r, &mut false).map_err(|e| vm_text(&e))?;
    let init = run_phase(vm, "init", weapon);
    let bases = run_phase(vm, "bases", weapon);
    let equip = run_phase(vm, "equip", weapon);
    let after_equip = records(vm, weapon);
    let unequip = run_phase(vm, "unequip", weapon);
    let after_unequip = records(vm, weapon);
    Ok(EquipRun {
        errors: [init, bases, equip, unequip],
        after_equip,
        after_unequip,
    })
}

/// One weapon of a weapons-file line, as the run makes it: the weapon, its owner and the owner's accuracy pool (module
/// documentation), with its `VmError` kept, and `wrote` set once its first mutation succeeded (the weapon's creation;
/// lane IMPL-PICKUP, review r3 of #452). Shared with `--slice-gate`'s link 8 (lane SLICE-GATE-DRIVERS), which marks a
/// failure after that write (review r2 of #506).
pub(crate) fn prepare_line_typed(vm: &mut Vm, line: &str, wrote: &mut bool) -> Result<ObjRef, VmError> {
    let r = diff_request(line).map_err(script)?;
    prepare(vm, &r, wrote)
}

/// Phase `name` of [`PHASES`] on `weapon`: its error text, if any; an unknown name is an error text.
pub(crate) fn run_phase(vm: &mut Vm, name: &str, weapon: ObjRef) -> Option<String> {
    run_phase_typed(vm, name, weapon).map(|e| vm_text(&e))
}

/// [`run_phase`] with the error kept as the VM gave it, so a caller grades it by its variant and formats its text only
/// when it reports (review r3 of #468); an unknown name is a script error.
pub(crate) fn run_phase_typed(vm: &mut Vm, name: &str, weapon: ObjRef) -> Option<ow_vm::error::VmError> {
    match name {
        "init" => phase(vm, INITIALIZE, weapon, vec![Value::Bool(false)]),
        "bases" => phase(vm, ow_inventory::equip::CALCULATE_WEAPON_BASE_VALUES, weapon, Vec::new()),
        "equip" => phase(vm, EQUIP, weapon, Vec::new()),
        "unequip" => phase(vm, UNEQUIP, weapon, Vec::new()),
        other => Some(ow_vm::error::VmError::Script {
            function: "equip-run".into(),
            message: format!("{other}: not a phase of the equip run"),
        }),
    }
}

/// `ExternalAttributeModifiers`' length on `weapon` (0 when it holds no array).
pub(crate) fn external_records(vm: &Vm, weapon: ObjRef) -> usize {
    records(vm, weapon)
}

/// The run's world (module documentation), for `--slice-gate`.
pub(crate) fn gate_world(cooked: &Path) -> Result<ow_world::world::World, String> {
    run_world(cooked, ParityMode::Off)
}

/// The installed globals definition the pickup world's globals hold (NATIVE_WILLOW_GLOBALS.md section 2: the
/// configured `DefaultGlobalsDefinitionName`).
pub(crate) const GLOBALS_DEFINITION: &str = "GD_Globals.General.Globals";

/// [`gate_world`] with the world's one globals object, for `--slice-gate`'s links 6 and 7 (lane IMPL-WILLOW-GLOBALS,
/// decision 2026-11-19-94; NATIVE_WILLOW_GLOBALS.md section 6.1). It is world set-up, done before the run writes
/// anything, and it keeps its errors typed (review r1 of #476):
/// - a `WillowGlobals` from its class defaults, whose `TheGlobalsDefinition` is the installed [`GLOBALS_DEFINITION`],
///   loaded here (the allowed preload), recorded as the world's globals (`WorldQuery::set_globals`);
/// - the weapon class's default object, loaded here, so the preflight reads it without a load;
/// - an empty component registry layer (`ow_inventory::anim_tree::ComponentRegistry`), so `SetAnimTreeTemplate`'s
///   registry walk finds nothing (the hub's ruling on NATIVE_SET_ANIM_TREE_TEMPLATE.md section 5, UNVERIFIED).
///
/// The other links' worlds are unchanged.
pub(crate) fn pickup_world(
    cooked: &Path,
) -> Result<(ow_world::world::World, crate::cmd::slice_pickup::PickupSetup), VmError> {
    let mut world = run_world_t(cooked, ParityMode::Off)?;
    let vm = &mut world.vm;
    let defaults = vm.defs.class_default(&mut vm.space, crate::cmd::slice_pickup::WEAPON_CLASS);
    vm.log.extend(vm.defs.take_log());
    let weapon_defaults = defaults.map_err(VmError::from)?;
    let globals = make_t(vm, "WillowGame.WillowGlobals")?;
    let definition = find_object_t(vm, GLOBALS_DEFINITION)?;
    let loaded = vm.defs.ensure_loaded(&mut vm.space, definition);
    vm.log.extend(vm.defs.take_log());
    loaded.map_err(VmError::from)?;
    let definition_class = vm.space.heap.get(definition)?.class;
    // The holder's declaration first, with the preflight's own validator, so a malformed schema is the note's
    // VmError::Script, not a failed write (review r2 of #476).
    ow_inventory::globals::globals_schemas(vm, globals, "the pickup world's set-up")?;
    write_t(vm, globals, "TheGlobalsDefinition", Value::Object(Some(definition)))?;
    world.query().try_borrow_mut().map_err(|_| VmError::ServiceBorrowed("WorldQuery"))?.set_globals(globals);
    ow_inventory::anim_tree::add_empty_registry(&mut world.vm)?;
    Ok((world, crate::cmd::slice_pickup::PickupSetup { weapon_defaults, definition_class }))
}

/// [`gate_world`], typed ([`SetupError`]): a setup `NotYet` stays a `NotYet` (review r2 of #470, item 4).
pub(crate) fn gate_world_typed(cooked: &Path) -> Result<ow_world::world::World, SetupError> {
    run_world_typed(cooked, ParityMode::Off)
}

/// One weapon of a weapons-file line with no owner (`--slice-gate`'s link 7 decoy, lane IMPL-PICKUP), typed as
/// [`prepare_line_typed`].
pub(crate) fn prepare_unowned_typed(vm: &mut Vm, line: &str, wrote: &mut bool) -> Result<ObjRef, VmError> {
    let r = diff_request(line).map_err(script)?;
    weapon_of(vm, &r, wrote)
}

fn prepare(vm: &mut Vm, r: &DiffRequest, wrote: &mut bool) -> Result<ObjRef, VmError> {
    let weapon = weapon_of(vm, r, wrote)?;
    let pawn = make_t(vm, PAWN)?;
    let controller = make_t(vm, CONTROLLER)?;
    let manager = make_t(vm, MANAGER)?;
    write_t(vm, pawn, "Controller", Value::Object(Some(controller)))?;
    write_t(vm, controller, "Pawn", Value::Object(Some(pawn)))?;
    write_t(vm, pawn, "InvManager", Value::Object(Some(manager)))?;
    write_t(vm, manager, "Owner", Value::Object(Some(pawn)))?;
    write_t(vm, weapon, "Instigator", Value::Object(Some(pawn)))?;
    write_t(vm, weapon, "Owner", Value::Object(Some(pawn)))?;
    accuracy_pool(vm, controller)?;
    Ok(weapon)
}

/// The weapon of a request, its definition data and level set, with no owner. `wrote` is set once the weapon exists.
fn weapon_of(vm: &mut Vm, r: &DiffRequest, wrote: &mut bool) -> Result<ObjRef, VmError> {
    let weapon = make_t(vm, WEAPON)?;
    *wrote = true;
    let weapon_type = find_object_t(vm, &r.weapon_type)?;
    let balance = find_object_t(vm, &r.balance)?;
    let mut data = ow_inventory::equip::read_prop(vm, weapon, "DefinitionData")
        .ok_or_else(|| script("the weapon has no DefinitionData".into()))?;
    set_field(vm, &mut data, "WeaponTypeDefinition", Value::Object(Some(weapon_type))).map_err(script)?;
    set_field(vm, &mut data, "BalanceDefinition", Value::Object(Some(balance))).map_err(script)?;
    set_field(vm, &mut data, "GameStage", Value::Int(r.level)).map_err(script)?;
    for (field, part) in &r.parts {
        let o = find_object_t(vm, part)?;
        set_field(vm, &mut data, field, Value::Object(Some(o))).map_err(script)?;
    }
    write_t(vm, weapon, "DefinitionData", data)?;
    write_t(vm, weapon, "ExpLevel", Value::Int(r.level))?;
    Ok(weapon)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_summary_counts_passes_and_groups_stops_per_phase() {
        let mut stops: [BTreeMap<String, usize>; PHASES.len()] = Default::default();
        stops[0].insert("unbound native X".into(), 2);
        stops[2].insert("NotYet Y".into(), 1);
        let s = summary(2, &[0, 2, 1, 2], &stops);
        assert_eq!(
            s,
            "{\"weapons\":2,\"init\":{\"passed\":0,\"stops\":[{\"stop\":\"unbound native X\",\"weapons\":2}]},\
             \"bases\":{\"passed\":2,\"stops\":[]},\"equip\":{\"passed\":1,\"stops\":[{\"stop\":\"NotYet Y\",\
             \"weapons\":1}]},\"unequip\":{\"passed\":2,\"stops\":[]}}\n"
        );
    }
}
