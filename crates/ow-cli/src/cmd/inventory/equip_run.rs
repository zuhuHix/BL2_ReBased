//! Owner: lane FIX-EQUIP-NATIVES. The installed equip run (Rust-only): `<package> --inventory-move --equip-run
//! <weapons file> --cooked <dir> --output <file>`. AI-assisted (Claude Code).
//!
//! It runs the installed script a looted gun goes through, on a gameplay world, and reports where each phase stops:
//! the lane's stop list (decision 2026-11-19-03). The weapons file is `--script-diff`'s (`<weapon type> <level>
//! <balance> [<Slot>=<part>]...`). One world over the cooked folder: W5-WORLD's packs, then `ow_combat::hit::Pack`
//! (the Off pool natives `--combat-run` binds: `ShutDown`, the idle timer) and `ow_inventory::manager::Pack`, in Off
//! `ow_skills::action_skill::Pack` (its `AttributeExpressionEvaluator.Evaluate`, which the muzzle-flash collections'
//! evaluators reach; lane IMPL-GET-PARTICLE-EFFECT), a `WorldInfo` from its class defaults placed first, with the VM's default stub policy (`Refuse`, nothing allowed), so a native the
//! script reaches that nothing binds stops the phase with the VM's `UnboundNative` text. No native is bound here.
//!
//! Per weapon (each phase runs even when an earlier one stopped, so one run lists every phase's stop):
//! - **make**: a `WillowWeapon` from its class defaults with the line's definition data and `ExpLevel`, as
//!   `--script-diff` makes it, and in Off with its own copy of each component the class default owns (a logged,
//!   UNVERIFIED harness input: the spawn path's instancing, NATIVE_SET_SKELETAL_MESH.md section 4); its owner is a `WillowPlayerPawn` (`Instigator` and `Owner`) whose `Controller` is a
//!   `WillowPlayerController` (whose `Pawn` is the pawn) and whose `InvManager` is a `WillowInventoryManager` (whose
//!   `Owner` is the pawn), each from its class defaults. These links stand in for the pickup and `AddInventory`
//!   (lane H-INTERACT, the host side), which the run does not play: a harness choice, not a rule. The controller
//!   also gets its accuracy pool: the Off `ResourcePoolManager.CreatePool` native on Maya's class's
//!   `AccuracyPoolDefinition` ([`ACCURACY_POOL`]), written to `ResourcePoolManager` and `AccuracyPool`. The game makes
//!   that pool in the player's own bring-up, which no script of the equip path reaches and no record reads (W5-MAPLOAD
//!   and the pools lanes); without it the weapon's accuracy effects find no pool and fall back to the controller
//!   source, where `WeaponResourcePoolAttributeContextResolver` is not read. A harness stand-in, not a rule.
//!   In Off it also preloads the muzzle-flash collections ([`preload_muzzle_flashes`], lane IMPL-GET-PARTICLE-EFFECT):
//!   the native loads nothing, as #518 counts a lazy load as a write, so the harness loads them, the way #528 preloads
//!   the body class and #541 the unlock data. A logged harness input, not a rule. Then the weapon type's
//!   `GestaltMesh` ([`preload_gestalt_mesh`], lane HARNESS-LINK8-GESTALT-PRELOAD): `BuildCompositeMesh`'s preflight
//!   refuses a cold gestalt definition, so the harness loads it, an UNVERIFIED stand-in for the game keeping it
//!   resident. Then that definition's `GestaltSkeletalMesh` ([`preload_gestalt_skeletal_mesh`], lane
//!   LINK8-NEWMESH-PRELOAD): `SetSkeletalMesh` refuses an unloaded new mesh, so the harness loads it, an UNVERIFIED
//!   stand-in for the mesh being resident with its definition. Then the Material part's `Material`
//!   ([`preload_material_part_material`], lane LINK-8-STARTUP-MATERIAL-PRELOAD): `BuildWeaponMaterial` refuses a parent
//!   material that is not resident, and the game loads those materials at boot (Startup exports, SLICE_NPC_ASSETS.md),
//!   so the harness loads it, an UNVERIFIED stand-in; then [`preload_material_classes`] loads the material classes'
//!   code and the `MaterialClass` default object `BuildWeaponMaterial` needs ready (boot-loaded script classes in
//!   the game; a harness set-up load). Each preload logs its line before it runs and its outcome after
//!   (": loaded", or ": failed (...)" with the error kept).
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
    let mut harness: Vec<String> = Vec::new();
    for (n, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        weapons += 1;
        let request = diff_request(line).map_err(|e| format!("line {}: {e}", n + 1))?;
        // The weapon's harness lines (review r1 of #632): its set-up's preloads change what `init` sees, so each
        // weapon's line and the summary carry them, also when the set-up fails.
        let log_before = world.vm.log.len();
        let result = run_weapon(&mut world.vm, &request);
        let own = harness_lines(&world.vm, log_before);
        for h in &own {
            if !harness.contains(h) {
                harness.push(h.clone());
            }
        }
        let listed = own.iter().map(|h| quote(h)).collect::<Vec<_>>().join(",");
        let run = match result {
            Ok(run) => run,
            Err(e) => return Err(failed_set_up(&output, lines, n + 1, &request.weapon_type, &e, &listed)),
        };
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
            "{{\"line\":{},\"weapon_type\":{},{},\"records_after_equip\":{},\"records_after_unequip\":{},\"harness\":[{listed}]}}\n",
            n + 1,
            quote(&request.weapon_type),
            fields.join(","),
            run.after_equip,
            run.after_unequip,
        ));
    }
    std::fs::write(&output, lines).map_err(|e| format!("{output}: {e}"))?;
    out.push_str(&summary(weapons, &passed, &stops, &harness));
    let all = passed.iter().all(|p| *p == weapons);
    Ok(if all { 0 } else { 1 })
}

/// A weapon whose set-up failed (review r1 of #632): the lines so far, then its own line (the set-up error under `make`
/// and its harness lines), written to `output` as a best effort; a failed write is ignored (review r2 of #632). The
/// answer is always the original set-up error, "line N: ...", byte for byte.
fn failed_set_up(output: &str, mut lines: String, line: usize, weapon_type: &str, e: &str, listed: &str) -> String {
    lines.push_str(&format!(
        "{{\"line\":{line},\"weapon_type\":{},\"make\":{},\"harness\":[{listed}]}}\n",
        quote(weapon_type),
        quote(e),
    ));
    let _ = std::fs::write(output, &lines);
    format!("line {line}: {e}")
}

/// The `harness input:` lines `vm` logged since `from`, in order.
fn harness_lines(vm: &Vm, from: usize) -> Vec<String> {
    vm.log[from.min(vm.log.len())..].iter().filter(|l| l.starts_with("harness input:")).cloned().collect()
}

/// The summary line: weapons, passes per phase, each phase's stops with their weapon counts, and the distinct harness
/// lines the weapons' set-ups logged, in first-seen order (review r1 of #632).
fn summary(weapons: usize, passed: &[usize], stops: &[BTreeMap<String, usize>], harness: &[String]) -> String {
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
    let harness: Vec<String> = harness.iter().map(|h| quote(h)).collect();
    format!("{{\"weapons\":{weapons},{},\"harness\":[{}]}}\n", phases.join(","), harness.join(","))
}

/// The run's world (module documentation).
fn run_world(cooked: &Path, mode: ParityMode) -> Result<ow_world::world::World, String> {
    run_world_typed(cooked, mode, true).map_err(|e| e.text())
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

/// `equip` adds `ow_skills::action_skill::Pack` in Off: `--equip-run` and link 8 only (review r1 of #573, item 3).
fn run_world_typed(cooked: &Path, mode: ParityMode, equip: bool) -> Result<ow_world::world::World, SetupError> {
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
    let skills = ow_skills::action_skill::Pack;
    let mut packs: Vec<&dyn ow_vm::api::NativePack> = vec![&combat, &inventory];
    // The evaluators the muzzle-flash collections hold (lane IMPL-GET-PARTICLE-EFFECT): the equip run's world only, in
    // Off only.
    if equip && mode == ParityMode::Off {
        packs.push(&skills);
    }
    let world = ow_world::world::World::new(vm, ow_world::world::BalanceState::new(snapshot), &packs[..]).map_err(
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

/// A world before its `WorldInfo` is placed, with no object made yet: `before` runs on the VM first, right after the
/// core natives are registered (the card's class setup, review r2 of #521, item 5). `equip` adds
/// `ow_skills::action_skill::Pack` in Off, as [`run_world_typed`] does: [`equip_world`]'s (link 8, and links 6 and 7
/// through [`pickup_world`]; #573, lane IMPL-LINK67-PICKUP-GUN-INIT).
fn bare_world_t(
    cooked: &Path,
    mode: ParityMode,
    equip: bool,
    before: impl FnOnce(&mut Vm) -> Result<(), VmError>,
) -> Result<ow_world::world::World, VmError> {
    use ow_balance::types::{
        AwesomeLevel, BalanceSnapshot, ExpLevel, GameStage, LevelCapUpgrades, OpLevel, PlayerCount,
        Playthrough,
    };
    let defs = DefStore::open(cooked.to_path_buf(), IdentityPolicy::FirstInScanOrder, mode)?;
    let attrs = AttributeSystem::standard()?;
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(attrs), mode);
    ow_vm::natives_core::register(&mut vm)?;
    before(&mut vm)?;
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
    let skills = ow_skills::action_skill::Pack;
    let mut packs: Vec<&dyn ow_vm::api::NativePack> = vec![&combat, &inventory];
    // The evaluators the muzzle-flash collections hold (lane IMPL-GET-PARTICLE-EFFECT): link 8's world only, in Off.
    if equip && mode == ParityMode::Off {
        packs.push(&skills);
    }
    ow_world::world::World::new(vm, ow_world::world::BalanceState::new(snapshot), &packs[..]).map_err(|e| match e {
        ow_world::world::WorldError::Vm(v) => v,
        other => script(error_text(&other)),
    })
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

/// The line logged for the harness input of [`own_components`], before it runs; its outcome follows as
/// `<line>: <n> component(s)` or `<line>: failed (<error>)`.
pub(crate) const WEAPON_COMPONENTS_INPUT: &str =
    "harness input: the weapon's components instanced through the spawn path (UNVERIFIED harness input)";

/// A harness input, **UNVERIFIED** (the hub's ruling on #583; NATIVE_SET_SKELETAL_MESH.md section 4): in the game every
/// spawned weapon owns its mesh components, but the run makes its weapon from the class defaults, which leaves the
/// shared template components in its slots. So the weapon gets its own copy of each component `template` (the class
/// default) owns, in the spawn path's order (`ow_world::actors::spawn`, #435): the plan first
/// (`plan_components_tracked`, whose lazy loads are writes), then `make` makes the object, then `apply_plan`. The line
/// [`WEAPON_COMPONENTS_INPUT`] is logged before the plan, and its outcome after. `wrote` is set once the plan loaded
/// something or the object exists. Answers the made object.
fn own_components(
    vm: &mut Vm,
    template: ObjRef,
    wrote: &mut bool,
    make: impl FnOnce(&mut Vm) -> Result<ObjRef, VmError>,
) -> Result<ObjRef, VmError> {
    vm.log.push(WEAPON_COMPONENTS_INPUT.to_string());
    let outcome = (|| {
        let mut loaded = false;
        let plan = ow_world::actors::plan_components_tracked(vm, template, &mut loaded);
        *wrote |= loaded;
        let plan = plan?;
        let made = make(vm)?;
        *wrote = true;
        let pairs = ow_world::actors::apply_plan(vm, &plan, made)?;
        Ok((made, pairs.len()))
    })();
    match outcome {
        Ok((made, n)) => {
            vm.log.push(format!("{WEAPON_COMPONENTS_INPUT}: {n} component(s)"));
            Ok(made)
        }
        Err(e) => {
            vm.log.push(format!("{WEAPON_COMPONENTS_INPUT}: failed ({e})"));
            Err(e)
        }
    }
}

fn make_t(vm: &mut Vm, class: &str) -> Result<ObjRef, VmError> {
    let c = vm.find_class(class)?;
    instantiate_from_defaults(vm, c)
}

fn run_weapon(vm: &mut Vm, r: &DiffRequest) -> Result<EquipRun, String> {
    let weapon = prepare_equip(vm, r).map_err(|e| vm_text(&e))?;
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

/// Link 8's weapon (lane SLICE-GATE-DRIVERS) as [`prepare_line_typed`] makes it, with `wrote` kept for the make's
/// mark (review r2 of #506), on [`WeaponRoute::Link8`] (#577's manufacturer inputs). `--slice-gate`'s link 8 then
/// runs [`preload_muzzle_flashes`], as [`prepare_equip`] does for `--equip-run`.
pub(crate) fn prepare_link8_typed(vm: &mut Vm, line: &str, wrote: &mut bool) -> Result<ObjRef, VmError> {
    let r = diff_request(line).map_err(script)?;
    prepare(vm, &r, wrote, WeaponRoute::Link8)
}

/// One weapon of a weapons-file line, as the run makes it: the weapon, its owner and the owner's accuracy pool (module
/// documentation), with its `VmError` kept, and `wrote` set once its first mutation succeeded (the weapon's creation;
/// lane IMPL-PICKUP, review r3 of #452), on [`WeaponRoute::Pickup`]: the pickup links' held weapon. Link 8 has
/// [`prepare_link8_typed`].
pub(crate) fn prepare_line_typed(vm: &mut Vm, line: &str, wrote: &mut bool) -> Result<ObjRef, VmError> {
    let r = diff_request(line).map_err(script)?;
    prepare(vm, &r, wrote, WeaponRoute::Pickup)
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

/// The `SetBaseValue` harness preload of `--slice-gate`'s link 8 (the hub's ruling on #572, narrowed in its review r2;
/// lane IMPL-SET-BASE-VALUE): only the **destination** attributes the weapon type's `AnimThresholdValues` hand to
/// `AttributeInitializationDefinition.SetBaseValue`, which the game keeps resident, and the resolvers each destination
/// needs: its context resolvers, and its value resolvers up to the chain's first None (where the set stops). Each
/// unloaded one is loaded here, before the run writes anything, and logged as an UNVERIFIED harness input with its
/// outcome; a failed load is logged and left. Every other input (a `BaseValue`'s attribute or definition, their
/// resolvers) is not preloaded: an unloaded one reaches the native and refuses, so the slice shows it.
pub(crate) fn preload_threshold_attributes(vm: &mut Vm, weapon: ObjRef) -> Result<(), VmError> {
    let Some(data) = ow_inventory::equip::read_prop(vm, weapon, "DefinitionData") else { return Ok(()) };
    let Value::Object(Some(weapon_type)) = ow_attr::eval::field(&vm.space, &data, "WeaponTypeDefinition")?.clone() else {
        return Ok(());
    };
    let Some(Value::Array(entries)) = ow_inventory::equip::read_prop(vm, weapon_type, "AnimThresholdValues") else {
        return Ok(());
    };
    let mut destinations: Vec<ObjRef> = Vec::new();
    for entry in entries.iter() {
        if let Value::Object(Some(attr)) = ow_attr::eval::field(&vm.space, entry, "Attribute")?.clone()
            && !destinations.contains(&attr)
        {
            destinations.push(attr);
        }
    }
    for attr in destinations {
        if !preload(vm, attr) {
            continue;
        }
        let mut resolvers: Vec<ObjRef> = Vec::new();
        if let Some(Value::Array(rs)) = ow_inventory::equip::read_prop(vm, attr, "ContextResolverChain") {
            resolvers.extend(rs.iter().filter_map(|r| match r {
                Value::Object(Some(r)) => Some(*r),
                _ => None,
            }));
        }
        if let Some(Value::Array(rs)) = ow_inventory::equip::read_prop(vm, attr, "ValueResolverChain") {
            for r in rs.iter() {
                match r {
                    Value::Object(Some(r)) => resolvers.push(*r),
                    _ => break,
                }
            }
        }
        for r in resolvers {
            preload(vm, r);
        }
    }
    Ok(())
}

/// Loads `o` when it is an unloaded handle, logging the harness input and its outcome; whether `o` is readable after.
fn preload(vm: &mut Vm, o: ObjRef) -> bool {
    if !vm.defs.is_unloaded(&vm.space, o) {
        return true;
    }
    let path = object_path(vm, o);
    vm.log.push(format!("harness input: preload {path} (a SetBaseValue definition the game keeps resident; UNVERIFIED)"));
    let loaded = vm.defs.ensure_loaded(&mut vm.space, o);
    vm.log.extend(vm.defs.take_log());
    match loaded {
        Ok(()) => {
            vm.log.push(format!("harness input: preload {path}: loaded"));
            true
        }
        Err(e) => {
            vm.log.push(format!("harness input: preload {path}: failed ({e})"));
            false
        }
    }
}

/// `o`'s dotted path from its outer chain (names only), for a harness log line.
fn object_path(vm: &Vm, o: ObjRef) -> String {
    let mut parts = Vec::new();
    let mut cursor = Some(o);
    while let Some(x) = cursor {
        let Ok(obj) = vm.space.heap.get(x) else { break };
        parts.push(vm.space.names.text(obj.name).to_string());
        cursor = obj.outer;
    }
    parts.reverse();
    parts.join(".")
}

/// The installed globals definition the pickup world's globals hold (NATIVE_WILLOW_GLOBALS.md section 2: the
/// configured `DefaultGlobalsDefinitionName`).
pub(crate) const GLOBALS_DEFINITION: &str = "GD_Globals.General.Globals";

/// `--slice-gate`'s links 6 and 7's world: [`equip_world`] plus the pickup set-up (the hub's ruling,
/// NATIVE_LINK67_PICKUP_GUN_INIT.md section 3; lane IMPL-LINK67-PICKUP-GUN-INIT). It is world set-up, done before the
/// run writes anything, and it keeps its errors typed (review r1 of #476):
/// - [`equip_world`]: the card's class set-up, the action-skill pack, the `WorldInfo`, and the world's one
///   `WillowGlobals` holding the installed [`GLOBALS_DEFINITION`] (NATIVE_WILLOW_GLOBALS.md section 6.1). The pickup
///   set-up checks the globals' declared schemas (lane IMPL-WILLOW-GLOBALS) and uses that definition;
/// - the weapon class's default object, loaded here, so the preflight reads it without a load;
/// - an empty component registry layer (`ow_inventory::anim_tree::ComponentRegistry`), so `SetAnimTreeTemplate`'s
///   registry walk finds nothing (the hub's ruling on NATIVE_SET_ANIM_TREE_TEMPLATE.md section 5, UNVERIFIED);
/// - an empty scene layer (`ow_inventory::attach_mesh::SceneLayer`), so `Actor.AttachComponent` makes only its
///   script-visible writes and registers nothing (the hub's ruling on NATIVE_ATTACH_MESH_TO_PICKUP.md section 5, a port
///   Off answer, UNVERIFIED);
/// - a game info ([`pickup_game_info`], lane LINK-6-PICKUP-GAMEINFO), so `DroppedPickup.Pickup.ValidTouch`'s
///   `WorldInfo.Game.PickupQuery` runs instead of reading through None.
///
/// The other links' worlds are unchanged.
pub(crate) fn pickup_world(
    cooked: &Path,
) -> Result<(ow_world::world::World, crate::cmd::slice_pickup::PickupSetup), VmError> {
    // The hub's ruling (NATIVE_LINK67_PICKUP_GUN_INIT.md section 3; lane IMPL-LINK67-PICKUP-GUN-INIT): link 8's world,
    // with the card's class set-up, the action-skill pack and its globals (one object holding the installed
    // definition), then the pickup's own set-up on top.
    let mut world = equip_world(cooked)?;
    let vm = &mut world.vm;
    let defaults = vm.defs.class_default(&mut vm.space, crate::cmd::slice_pickup::WEAPON_CLASS);
    vm.log.extend(vm.defs.take_log());
    let weapon_defaults = defaults.map_err(VmError::from)?;
    let globals = ow_inventory::globals::world_globals(vm)?;
    ow_inventory::globals::globals_schemas(vm, globals, "the pickup world's set-up")?;
    let definition = match ow_inventory::equip::read_prop(vm, globals, "TheGlobalsDefinition") {
        Some(Value::Object(Some(d))) => d,
        other => return Err(script(format!("the equip world's globals hold {other:?}, not a definition"))),
    };
    let definition_class = vm.space.heap.get(definition)?.class;
    ow_inventory::anim_tree::add_empty_registry(&mut world.vm)?;
    ow_inventory::attach_mesh::add_empty_scene(&mut world.vm)?;
    particle_preload(&mut world.vm, definition)?;
    pickup_game_info(&mut world)?;
    // Link 6's take (lane IMPL-UPDATEAMMOCOUNTS, the hub's two rulings): the eight ammo definitions preloaded, and
    // GetSaveRotation's at-rest answer turned on, both UNVERIFIED harness inputs.
    preload_ammo_definitions(&mut world.vm, definition)?;
    ow_inventory::pickup_take::enable_at_rest_body_pose(&mut world.vm)?;
    world.vm.log.push(AT_REST_INPUT.to_string());
    // Link 6 after SetBase (lane IMPL-LINK6-AFTER-TAKE, the hub's ruling on NATIVE_ACTOR_SETBASE.md 4.2): the audio
    // stand-in turned on (it logs its own harness line), UNVERIFIED.
    ow_inventory::link6::enable_audio_stand_in(&mut world.vm)?;
    Ok((world, crate::cmd::slice_pickup::PickupSetup { weapon_defaults, definition_class }))
}

/// The log line of the at-rest ruling's set-up.
pub(crate) const AT_REST_INPUT: &str = "harness input: the pickup world turns on GetSaveRotation's at-rest answer (the \
     hub's ruling on NATIVE_PICKUP_SAVE_ROTATION.md section 5; UNVERIFIED)";

/// The log line of one ammo-definition preload.
pub(crate) fn ammo_preload_line(path: &str) -> String {
    format!("harness input: set-up preload: {path}{AMMO_PRELOAD_TAIL}")
}

/// The tail of [`ammo_preload_line`].
pub(crate) const AMMO_PRELOAD_TAIL: &str = " (UNVERIFIED: an ammo definition the game has resident)";

/// The hub's ruling for `UpdateAmmoCounts` (lane IMPL-UPDATEAMMOCOUNTS): the eight ammo definitions the globals
/// definition lists (`ow_inventory::pickup_take::AMMO`'s fields, in that order) are loaded through the store at the
/// pickup world's set-up, each logged before its load; one already resident is neither loaded nor logged, and a
/// None field is skipped. Nothing else: the native's preflight reads only their residency, and nothing they
/// reference is loaded. The native itself still refuses a definition the store has not loaded and never loads one.
/// Answers the number loaded.
pub(crate) fn preload_ammo_definitions(vm: &mut Vm, definition: ObjRef) -> Result<usize, VmError> {
    let class = vm.space.heap.get(definition)?.class;
    let mut loaded = 0;
    for (field, _, _) in ow_inventory::pickup_take::AMMO {
        let Some(p) = vm.space.prop(class, field) else {
            continue;
        };
        let Value::Object(Some(ammo)) = vm.space.read(definition, PropAddr::scalar(p))?.clone() else {
            continue;
        };
        if !vm.defs.is_unloaded(&vm.space, ammo) {
            continue;
        }
        let path = vm.defs.handle_path(ammo).unwrap_or_else(|| format!("{ammo:?}"));
        vm.log.push(ammo_preload_line(&path));
        let r = vm.defs.ensure_loaded(&mut vm.space, ammo);
        vm.log.extend(vm.defs.take_log());
        r.map_err(VmError::from)?;
        loaded += 1;
    }
    Ok(loaded)
}

/// The game-info class of [`pickup_game_info`].
pub(crate) const GAME_INFO_CLASS: &str = "WillowGame.WillowGameInfo";
/// The log line of [`pickup_game_info`].
pub(crate) const GAME_INFO_INPUT: &str = "harness input: the pickup world's WorldInfo.Game is a class-default \
     WillowGame.WillowGameInfo with authority (an UNVERIFIED stand-in, as the combat run's deaths install one: this \
     world spawns no game info)";

/// The pickup world's game info (lane LINK-6-PICKUP-GAMEINFO), world set-up before the run writes anything and
/// logged as a harness input. The world spawns no game info, so its `WorldInfo.Game` was None and
/// `DroppedPickup.Pickup.ValidTouch`'s last test, `WorldInfo.Game.PickupQuery(Other, self)`, read through None and
/// answered false silently: no touch registered (decision 2026-11-25-41, lane LINK-6-OWNER-CHECK). Like the combat
/// run's deaths (`ow_combat::death::enable_deaths`), the world gets a plain class-default `WillowGameInfo` with
/// `Role` authority and its `WorldInfo`, written into the `WorldInfo`'s `Game`. In the game, the level's game mode
/// spawns its own game info; this one is a stand-in, UNVERIFIED as the game's. A world whose `Game` is set already
/// is left as it is.
pub(crate) fn pickup_game_info(world: &mut ow_world::world::World) -> Result<(), VmError> {
    let info = world
        .actors()
        .try_borrow()
        .map_err(|_| VmError::ServiceBorrowed("Actors"))?
        .world_info()
        .ok_or_else(|| script("the pickup world has no WorldInfo for its game info".to_string()))?;
    let vm = &mut world.vm;
    let class = vm.space.heap.get(info)?.class;
    let game_prop = vm.space.prop(class, "Game").ok_or_else(|| script("the WorldInfo declares no Game".to_string()))?;
    if !matches!(vm.space.read(info, PropAddr::scalar(game_prop))?, Value::Object(None)) {
        return Ok(());
    }
    let game = make_t(vm, GAME_INFO_CLASS)?;
    write_t(vm, game, "Role", Value::Byte(ow_world::actors::ROLE_AUTHORITY))?;
    write_t(vm, game, "WorldInfo", Value::Object(Some(info)))?;
    write_t(vm, info, "Game", Value::Object(Some(game)))?;
    vm.log.push(GAME_INFO_INPUT.to_string());
    Ok(())
}

/// The allowed preload of NATIVE_WILLOW_GLOBALS.md section 6.1, step 2, for a world's set-up, before the run writes
/// anything: the installed [`GLOBALS_DEFINITION`] found and loaded, then written into `globals`'
/// `TheGlobalsDefinition`. The holder's declaration is checked first, with the preflight's own validator, so a
/// malformed schema is the note's `VmError::Script`, not a failed write (review r2 of #476). The first load's
/// challenge numbering is not run here or anywhere (section 6.1). Used by the
/// Fire driver for link 12 (`cmd/slice.rs`, lane IMPL-LINK12-UNLOCK-ACHIEVEMENT).
pub(crate) fn preload_globals_definition(vm: &mut Vm, globals: ObjRef, what: &str) -> Result<ObjRef, VmError> {
    let definition = find_object_t(vm, GLOBALS_DEFINITION)?;
    let loaded = vm.defs.ensure_loaded(&mut vm.space, definition);
    vm.log.extend(vm.defs.take_log());
    loaded.map_err(VmError::from)?;
    ow_inventory::globals::globals_schemas(vm, globals, what)?;
    write_t(vm, globals, "TheGlobalsDefinition", Value::Object(Some(definition)))?;
    Ok(definition)
}

/// The prefix of the particle preload's log lines.
pub(crate) const PARTICLE_INPUT: &str = "harness input: SpawnPickupParticles preload";
/// The prefix of the particle engine flags' log line.
pub(crate) const PARTICLE_FLAGS: &str = "harness input: ParticleSystemComponent engine flags";

/// `SpawnPickupParticles`' harness inputs (hub ruling 1 on #581, NATIVE_SPAWN_PICKUP_PARTICLES.md section 4; lane
/// IMPL-SPAWN-PICKUP-PARTICLES), world set-up before `make`, each an UNVERIFIED stand-in for packages the game has
/// loaded already and logged: the globals definition's `PartPickup` template, loaded here; the
/// `ParticleSystemComponent` class with its code and its default object, recorded for the native's preflight
/// (`ow_inventory::attach_mesh::pickup_particles::ParticlePreload`).
fn particle_preload(vm: &mut Vm, definition: ObjRef) -> Result<(), VmError> {
    use ow_inventory::attach_mesh::pickup_particles as pp;
    match ow_inventory::equip::read_prop(vm, definition, "PartPickup") {
        Some(Value::Object(Some(template))) => {
            let loaded = vm.defs.ensure_loaded(&mut vm.space, template);
            vm.log.extend(vm.defs.take_log());
            loaded.map_err(VmError::from)?;
            let path = ow_attr::eval::class_path(&vm.space, template);
            vm.log.push(format!("{PARTICLE_INPUT}: the globals definition's PartPickup ({path}) loaded at set-up (UNVERIFIED)"));
        }
        other => vm.log.push(format!("{PARTICLE_INPUT}: the globals definition's PartPickup is {other:?}, nothing loaded")),
    }
    let class = vm.find_class(pp::COMPONENT_CLASS)?;
    let defaults = vm.defs.class_default(&mut vm.space, pp::COMPONENT_CLASS);
    vm.log.extend(vm.defs.take_log());
    let defaults = defaults.map_err(VmError::from)?;
    pp::add_particle_preload(vm, pp::ParticlePreload { class, defaults })?;
    vm.log.push(format!("{PARTICLE_INPUT}: {} class and default object loaded at set-up (UNVERIFIED)", pp::COMPONENT_CLASS));
    // The engine flags SetTemplate and ActivateSystem test (hub ruling 1 on #597, NATIVE_PARTICLE_ACTIVATION.md
    // section 7; lane IMPL-PARTICLE-ACTIVATION): the hub's slice values, an UNVERIFIED stand-in for the game's.
    let flags = ow_inventory::attach_mesh::particle_activation::ParticleEngineFlags::SLICE;
    ow_inventory::attach_mesh::particle_activation::add_particle_engine_flags(vm, flags)?;
    vm.log.push(format!(
        "{PARTICLE_FLAGS}: GIsGame {}, GIsEditor {}, particles allowed {}, game-thread LOD {}, force lowest LOD {}, LOD bias {}, system detail mode {} (UNVERIFIED)",
        flags.is_game, flags.is_editor, flags.particles_allowed, flags.game_thread_lod, flags.force_lowest, flags.lod_bias, flags.system_detail_mode
    ));
    Ok(())
}

/// The run's world in Off, typed ([`SetupError`]): a setup `NotYet` stays a `NotYet` (review r2 of #470, item 4).
pub(crate) fn gate_world_typed(cooked: &Path) -> Result<ow_world::world::World, SetupError> {
    run_world_typed(cooked, ParityMode::Off, false)
}

/// The run's world in Off with the world's one globals object, for `--slice-gate`'s link 8 (lane IMPL-CACHE-WEAPON-CARD-BODY,
/// NATIVE_CACHE_WEAPON_CARD_BODY.md section 4; NATIVE_WILLOW_GLOBALS.md section 6.1). World set-up, before the run
/// writes anything, its errors typed: a `WillowGlobals` from its class defaults whose `TheGlobalsDefinition` is the
/// installed [`GLOBALS_DEFINITION`], loaded here (the allowed preload), recorded as the world's globals. Unlike
/// [`pickup_world`] it checks only the holder's declaration it writes, not the pickup route's schemas (section 6.2,
/// step 4, which the card does not read; review of #491).
///
/// The card's class setup (`equip::card::setup`) runs first, before the world exists and before any object is made
/// (review r2 of #521, item 5). From there the three counts of `equip::card::load_mark` (heap writes, random draws,
/// definition loads) are tracked from the first allocation, the `WorldInfo`'s default instantiation included: a
/// refusal after any of them moved is marked "stopped after partial writes".
pub(crate) fn equip_world(cooked: &Path) -> Result<ow_world::world::World, VmError> {
    let mut world = bare_world_t(cooked, ParityMode::Off, true, ow_inventory::equip::card::setup)?;
    let before = ow_inventory::equip::card::load_mark(&world.vm);
    match equip_world_objects(&mut world) {
        Ok(()) => Ok(world),
        Err(e) if ow_inventory::equip::card::load_mark(&world.vm) == before => Err(e),
        Err(e) => Err(ow_world::pools::after_partial_writes(e)),
    }
}

/// [`equip_world`]'s objects: the `WorldInfo`, then the globals holding the installed definition.
fn equip_world_objects(world: &mut ow_world::world::World) -> Result<(), VmError> {
    let info = make_t(&mut world.vm, "Engine.WorldInfo")?;
    world.actors().try_borrow_mut().map_err(|_| VmError::ServiceBorrowed("Actors"))?.place_world_info(info)?;
    let globals = make_t(&mut world.vm, "WillowGame.WillowGlobals")?;
    equip_globals(&mut world.vm, globals)?;
    world.query().try_borrow_mut().map_err(|_| VmError::ServiceBorrowed("WorldQuery"))?.set_globals(globals);
    Ok(())
}

/// [`equip_world`]'s writes after the globals' allocation: the installed definition, loaded, held by `globals`.
fn equip_globals(vm: &mut Vm, globals: ObjRef) -> Result<(), VmError> {
    let definition = find_object_t(vm, GLOBALS_DEFINITION)?;
    let loaded = vm.defs.ensure_loaded(&mut vm.space, definition);
    vm.log.extend(vm.defs.take_log());
    loaded.map_err(VmError::from)?;
    ow_inventory::globals::declared_reference(
        vm,
        globals,
        "TheGlobalsDefinition",
        Some(ow_inventory::globals::GEARBOX_GLOBALS_DEFINITION),
        "the equip world's set-up",
    )?;
    write_t(vm, globals, "TheGlobalsDefinition", Value::Object(Some(definition)))
}

/// The gun's init before its drop, on links 6 and 7 (lane IMPL-LINK67-PICKUP-GUN-INIT, NATIVE_LINK67_PICKUP_GUN_INIT.md
/// section 3), in link 8's order: its five harness preloads ([`preload_muzzle_flashes`], [`preload_gestalt_mesh`],
/// [`preload_gestalt_skeletal_mesh`], [`preload_material_part_material`], [`preload_material_classes`]), the skeletal
/// mesh layer ([`install_mesh_layer`], once per world), the threshold preload ([`preload_threshold_attributes`]), then
/// the installed `WillowWeapon.InitializeInternal(false)` (the `init` phase). Each preload is an UNVERIFIED stand-in
/// and logs its harness lines, as link 8's do. It runs inside `make`'s mark, after the weapon exists, so a refusal
/// here is marked "stopped after partial writes" by the caller (link 8's threshold preload runs after its mark: the
/// prefix differs there, as the note says).
pub(crate) fn pickup_gun_init(vm: &mut Vm, weapon: ObjRef) -> Result<(), VmError> {
    for (_, step) in GUN_INIT_STEPS {
        step(vm, weapon)?;
    }
    Ok(())
}

/// One step of [`pickup_gun_init`].
pub(crate) type GunInitStep = fn(&mut Vm, ObjRef) -> Result<(), VmError>;

/// [`pickup_gun_init`]'s steps, in order, named (review of #761: a test pins the order).
pub(crate) const GUN_INIT_STEPS: [(&str, GunInitStep); 8] = [
    ("muzzle flashes", preload_muzzle_flashes),
    ("gestalt mesh", preload_gestalt_mesh),
    ("gestalt skeletal mesh", preload_gestalt_skeletal_mesh),
    ("material part material", preload_material_part_material),
    ("material classes", preload_material_classes),
    ("mesh layer", install_mesh_layer),
    ("threshold attributes", preload_threshold_attributes),
    ("init", init_phase),
];

/// The installed `WillowWeapon.InitializeInternal(false)` (the `init` phase), its error kept.
pub(crate) fn init_phase(vm: &mut Vm, weapon: ObjRef) -> Result<(), VmError> {
    match run_phase_typed(vm, "init", weapon) {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// One weapon of a weapons-file line with no owner (`--slice-gate`'s link 7 decoy, lane IMPL-PICKUP), typed as
/// [`prepare_line_typed`].
pub(crate) fn prepare_unowned_typed(vm: &mut Vm, line: &str, wrote: &mut bool) -> Result<ObjRef, VmError> {
    let r = diff_request(line).map_err(script)?;
    weapon_of(vm, &r, wrote, WeaponRoute::PickupDecoy)
}

/// Which run a weapon is made for (review r1 of #577). The manufacturer inputs are set on every route but none: links 6
/// and 7's guns get them as link 8's does (lane IMPL-LINK67-PICKUP-GUN-INIT, NATIVE_LINK67_PICKUP_GUN_INIT.md section
/// 3), since their init reads the same definition data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WeaponRoute {
    /// `--equip-run`.
    EquipRun,
    /// `--slice-gate`'s link 8 (`prepare_link8_typed`).
    Link8,
    /// The pickup links' held weapon (`prepare_line_typed`).
    Pickup,
    /// Link 7's unowned decoy (`prepare_unowned_typed`).
    PickupDecoy,
}

impl WeaponRoute {
    /// Whether the Off harness sets the definition data's manufacturer inputs ([`manufacturer_inputs`]).
    pub(crate) fn manufacturer_inputs(self) -> bool {
        matches!(self, WeaponRoute::EquipRun | WeaponRoute::Link8 | WeaponRoute::Pickup | WeaponRoute::PickupDecoy)
    }
}

fn prepare(vm: &mut Vm, r: &DiffRequest, wrote: &mut bool, route: WeaponRoute) -> Result<ObjRef, VmError> {
    let weapon = weapon_of(vm, r, wrote, route)?;
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

/// [`prepare`] and then the equip run's four Off harness preloads, in order: the muzzle flashes
/// ([`preload_muzzle_flashes`]), the gestalt definition ([`preload_gestalt_mesh`]), its skeletal mesh
/// ([`preload_gestalt_skeletal_mesh`]) and the Material part's material ([`preload_material_part_material`]). The
/// equip run's own (`--equip-run`; review r1 of #573, item 3); link 8's driver in the slice gate calls the same four
/// itself. The pickup links prepare their weapons with [`prepare`], then [`pickup_gun_init`] in `make`.
fn prepare_equip(vm: &mut Vm, r: &DiffRequest) -> Result<ObjRef, VmError> {
    let weapon = prepare(vm, r, &mut false, WeaponRoute::EquipRun)?;
    preload_muzzle_flashes(vm, weapon)?;
    preload_gestalt_mesh(vm, weapon)?;
    preload_gestalt_skeletal_mesh(vm, weapon)?;
    preload_material_part_material(vm, weapon)?;
    preload_material_classes(vm, weapon)?;
    install_mesh_layer(vm, weapon)?;
    Ok(weapon)
}

/// The text of [`preload_material_part_material`]'s harness lines.
pub(crate) const MATERIAL_INPUT: &str = "harness input: preload the Material part's Material";

/// The package a Material-part material must come from to be preloaded (review of #712).
pub(crate) const STARTUP_PACKAGE: &str = "Startup";

/// The Off harness preload of lane LINK-8-STARTUP-MATERIAL-PRELOAD: the weapon's `DefinitionData.MaterialPartDefinition`'s
/// `Material`, when the part is set and resident, the material is one of the store's unloaded handles, **and its export
/// is in the `Startup` package** (`DefStore::handle_source`), is loaded through the store. `BuildWeaponMaterial`'s
/// preflight (NATIVE_WEAPON_VISUALS.md section 1.1, step 3) refuses a parent material that is not resident, because a
/// lazy load is a write. What is recorded (review of #712): link 8's `Mati_BanditCommonAR` is Startup export 23880
/// (read 2026-11-25 from the installed `Startup.upk`), and SLICE_NPC_ASSETS.md records the pistol's
/// `Mati_MaliwanUncommon` as Startup export 23971. That the game loads `Startup` at boot is decision 2026-11-25-36's
/// statement and no verification record's, so this load is an **UNVERIFIED stand-in** for the material being resident.
/// A material from any other package is skipped with a logged line, and the native's refusal stands. Only that one
/// object is loaded: not the part, nor the material's parent chain. It is logged before it runs (rule 7) and its
/// outcome after; a failed load keeps its error. No part, a cold part, no material, or one already loaded: nothing.
/// Outside Off, nothing. `BuildWeaponMaterial`'s own refusal is unchanged.
pub(crate) fn preload_material_part_material(vm: &mut Vm, weapon: ObjRef) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    let data = ow_inventory::equip::read_prop(vm, weapon, "DefinitionData")
        .ok_or_else(|| script("the weapon has no DefinitionData".into()))?;
    let Ok(Value::Object(Some(part))) = ow_attr::eval::field(&vm.space, &data, "MaterialPartDefinition").cloned() else {
        return Ok(());
    };
    if vm.defs.is_unloaded(&vm.space, part) {
        return Ok(());
    }
    let Some(Value::Object(Some(material))) = ow_inventory::equip::read_prop(vm, part, "Material") else {
        return Ok(());
    };
    // A material already resident is not loaded again, but its graph is still walked (review of #712, delta r1).
    if !vm.defs.is_unloaded(&vm.space, material) {
        return preload_material_graph(vm, material);
    }
    let name = ow_vm::path_name::path_name(vm, material).unwrap_or_else(|_| format!("{material:?}"));
    let export = match vm.defs.handle_source(material) {
        Some((package, index)) if package.eq_ignore_ascii_case(STARTUP_PACKAGE) => index,
        other => {
            let from = other.map_or_else(|| "an unknown package".to_string(), |(p, i)| format!("{p} export {i}"));
            vm.log.push(format!(
                "{MATERIAL_INPUT} {name}: skipped ({from}, not a Startup export; BuildWeaponMaterial's refusal stands)"
            ));
            return Ok(());
        }
    };
    vm.log.push(format!(
        "{MATERIAL_INPUT} {name} (Startup export {export}; BuildWeaponMaterial loads nothing: UNVERIFIED stand-in for \
         the material being resident)"
    ));
    let loaded = vm.defs.ensure_loaded(&mut vm.space, material);
    vm.log.extend(vm.defs.take_log());
    match &loaded {
        Ok(()) => vm.log.push(format!("{MATERIAL_INPUT} {name}: loaded")),
        Err(e) => vm.log.push(format!("{MATERIAL_INPUT} {name}: failed ({e})")),
    }
    loaded.map_err(VmError::from)?;
    preload_material_graph(vm, material)
}

/// How many objects [`preload_material_graph`] visits at most (the hub's ruling on #712: a total cap).
pub(crate) const MATERIAL_WALK_CAP: usize = 64;

/// The rest of [`preload_material_part_material`] (the hub's rulings on #712's parent chain and on a `Material`'s
/// expressions). From the loaded material, the walk visits what the glow read (`GetScalarParameterValue`) may read:
/// - each visited object's `Parent`;
/// - for an object that declares `Expressions` (a `Material`), each entry, in order. A None entry is skipped, as the
///   glow read skips it.
///
/// An object is loaded **only if it is a Startup export** (`DefStore::handle_source`), by the same rule as the material
/// itself. In the game `Startup` is resident from boot (decision 2026-11-25-36, UNVERIFIED), so any Startup export the
/// walk reaches is resident there. An object already resident is visited with no load. A branch stops, with a logged
/// line, at a None `Parent` (or an object that declares none) and at the first object that is not a Startup export,
/// which is left for the native to refuse. Each object is visited once. After [`MATERIAL_WALK_CAP`] visits the walk
/// stops, and that is logged. Each load is logged before and after; a failed load keeps its error. It is an
/// **UNVERIFIED stand-in**, as the material's load is.
fn preload_material_graph(vm: &mut Vm, material: ObjRef) -> Result<(), VmError> {
    // Each entry: the object and how the walk reached it (the material itself and its parents, or an expression).
    let mut queue = std::collections::VecDeque::from([(material, "parent")]);
    let mut seen: Vec<ObjRef> = vec![material];
    let mut visits = 0usize;
    while let Some((current, via)) = queue.pop_front() {
        if visits == MATERIAL_WALK_CAP {
            vm.log.push(format!("{MATERIAL_INPUT}: the material walk stopped at its cap of {MATERIAL_WALK_CAP} objects"));
            return Ok(());
        }
        visits += 1;
        if vm.defs.is_unloaded(&vm.space, current) {
            let name = ow_vm::path_name::path_name(vm, current).unwrap_or_else(|_| format!("{current:?}"));
            let export = match vm.defs.handle_source(current) {
                Some((package, index)) if package.eq_ignore_ascii_case(STARTUP_PACKAGE) => index,
                other => {
                    let from = other.map_or_else(|| "an unknown package".to_string(), |(p, i)| format!("{p} export {i}"));
                    vm.log.push(format!(
                        "{MATERIAL_INPUT}: the walk stops at {name} ({from}, not a Startup export; the glow read's \
                         refusal stands)"
                    ));
                    continue;
                }
            };
            vm.log.push(format!("{MATERIAL_INPUT}: {via} {name} (Startup export {export}; UNVERIFIED stand-in)"));
            let loaded = vm.defs.ensure_loaded(&mut vm.space, current);
            vm.log.extend(vm.defs.take_log());
            match &loaded {
                Ok(()) => vm.log.push(format!("{MATERIAL_INPUT}: {via} {name}: loaded")),
                Err(e) => vm.log.push(format!("{MATERIAL_INPUT}: {via} {name}: failed ({e})")),
            }
            loaded.map_err(VmError::from)?;
        }
        let mut next: Vec<(ObjRef, &'static str)> = Vec::new();
        match ow_inventory::equip::read_prop(vm, current, "Parent") {
            Some(Value::Object(Some(parent))) => next.push((parent, "parent")),
            _ if via == "parent" => {
                let here = ow_vm::path_name::path_name(vm, current).unwrap_or_else(|_| format!("{current:?}"));
                vm.log.push(format!("{MATERIAL_INPUT}: the parent chain ends at {here} (no Parent)"));
            }
            _ => {}
        }
        if let Some(list) = ow_inventory::equip::read_prop(vm, current, "Expressions")
            && let Some(entries) = list.elements()
        {
            next.extend(entries.iter().filter_map(|e| match e {
                Value::Object(Some(o)) => Some((*o, "expression")),
                _ => None,
            }));
        }
        for (o, how) in next {
            if !seen.contains(&o) {
                seen.push(o);
                queue.push_back((o, how));
            }
        }
    }
    Ok(())
}

/// The text of [`preload_material_classes`]'s harness lines.
pub(crate) const MATERIAL_CLASS_INPUT: &str = "harness input: set-up load of the material classes";

/// The Off harness set-up step after [`preload_material_part_material`] (lane LINK-8-STARTUP-MATERIAL-PRELOAD, the
/// hub's extension). `BuildWeaponMaterial`'s step 4 (NATIVE_WEAPON_VISUALS.md section 1.1) asks, with no load, for
/// `Engine.MaterialInstanceConstant` and the weapon type's `MaterialClass` to be ready (a defined layout and loaded
/// code) and for that class's default object to be resident. Engine and WillowGame script classes are loaded at boot in
/// the game (they live in the startup and native script packages), so the harness loads, in this order:
/// 1. `MaterialInstanceConstant`'s class code, then `Engine.MaterialExpressionScalarParameter`'s
///    ([`SCALAR_EXPRESSION_CLASS`]) when the material's resident `Parent` chain holds a `Material` with expressions (the
///    glow read's class test; the hub's ruling);
/// 2. the `MaterialClass`'s class code;
/// 3. that class's default object (`DefStore::class_default`).
///
/// Steps 2 and 3 run only for a `MaterialClass` whose layout is defined and that is a child of
/// `MaterialInstanceConstant` (review of #712); any other class is skipped with a logged line and left to the native's
/// own refusal. Each step is skipped when already so. It is a harness set-up load, not a native stand-in (**UNVERIFIED** that the game
/// has them resident at that point), and each logs its line before it runs and its outcome after; a failure keeps its
/// error. A weapon type that is unset or not resident, or a `MaterialClass` that is None: only step 1 (the native
/// answers those cases itself). Outside Off, nothing. `BuildWeaponMaterial`'s refusals are unchanged.
pub(crate) fn preload_material_classes(vm: &mut Vm, weapon: ObjRef) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    let mic = ow_inventory::equip::weapon_material::MATERIAL_INSTANCE_CONSTANT;
    class_code_load(vm, mic)?;
    // The hub's ruling: the glow read's scalar-expression class, only when the material walk met a Material with
    // expressions (another Engine script class, loaded at boot in the game: UNVERIFIED).
    if material_chain_has_expressions(vm, weapon) {
        class_code_load(vm, SCALAR_EXPRESSION_CLASS)?;
    }
    let data = ow_inventory::equip::read_prop(vm, weapon, "DefinitionData")
        .ok_or_else(|| script("the weapon has no DefinitionData".into()))?;
    let Ok(Value::Object(Some(weapon_type))) = ow_attr::eval::field(&vm.space, &data, "WeaponTypeDefinition").cloned()
    else {
        return Ok(());
    };
    if vm.defs.is_unloaded(&vm.space, weapon_type) {
        return Ok(());
    }
    let Some(Value::Class(Some(class))) = ow_inventory::equip::read_prop(vm, weapon_type, "MaterialClass") else {
        return Ok(());
    };
    // Steps 2 and 3 only for a MaterialClass of the family (review of #712): its layout defined and a child of
    // MaterialInstanceConstant. Any other class is skipped with a line and left to the native's own refusal.
    let defined = vm.space.classes.get_layout(class).filter(|l| l.is_defined()).map(|l| l.path().as_str().to_string());
    let in_family =
        vm.space.classes.by_path(mic).is_some_and(|family| vm.space.classes.is_child_of(class, family));
    let Some(path) = defined.filter(|_| in_family) else {
        let shown = vm.space.classes.get_layout(class).map_or_else(|| format!("{class:?}"), |l| l.path().as_str().to_string());
        vm.log.push(format!(
            "{MATERIAL_CLASS_INPUT}: skipped {shown} (no defined layout under {mic}; BuildWeaponMaterial's own refusal stands)"
        ));
        return Ok(());
    };
    class_code_load(vm, &path)?;
    let resident = vm
        .space
        .heap
        .handles()
        .into_iter()
        .any(|o| vm.space.heap.get(o).is_ok_and(|x| x.class == class && matches!(x.origin, ow_object::heap::Origin::Default { .. })));
    if resident {
        return Ok(());
    }
    vm.log.push(format!("{MATERIAL_CLASS_INPUT}: the default object of {path} (loaded at boot in the game: UNVERIFIED)"));
    let built = vm.defs.class_default(&mut vm.space, &path);
    vm.log.extend(vm.defs.take_log());
    match &built {
        Ok(_) => vm.log.push(format!("{MATERIAL_CLASS_INPUT}: the default object of {path}: loaded")),
        Err(e) => vm.log.push(format!("{MATERIAL_CLASS_INPUT}: the default object of {path}: failed ({e})")),
    }
    built.map(|_| ()).map_err(VmError::from)
}

/// The class the glow read's `Material` walk tests expressions against (NATIVE_INITMESHES_MATERIAL_GLOW.md section
/// 4.2, by fit).
pub(crate) const SCALAR_EXPRESSION_CLASS: &str = "Engine.MaterialExpressionScalarParameter";

/// Whether the Material-part material's resident `Parent` chain (the material included) holds an object whose
/// `Expressions` has an entry that is set (`Object(Some)`; None entries do not count), read only. [`preload_material_part_material`]'s walk has loaded what it could by then.
fn material_chain_has_expressions(vm: &Vm, weapon: ObjRef) -> bool {
    let Some(data) = ow_inventory::equip::read_prop(vm, weapon, "DefinitionData") else {
        return false;
    };
    let Ok(Value::Object(Some(part))) = ow_attr::eval::field(&vm.space, &data, "MaterialPartDefinition").cloned() else {
        return false;
    };
    if vm.defs.is_unloaded(&vm.space, part) {
        return false;
    }
    let mut cursor = match ow_inventory::equip::read_prop(vm, part, "Material") {
        Some(Value::Object(Some(m))) => Some(m),
        _ => None,
    };
    let mut seen: Vec<ObjRef> = Vec::new();
    while let Some(o) = cursor {
        if seen.contains(&o) || seen.len() >= MATERIAL_WALK_CAP || vm.defs.is_unloaded(&vm.space, o) {
            return false;
        }
        seen.push(o);
        if ow_inventory::equip::read_prop(vm, o, "Expressions")
            .and_then(|v| v.elements().map(|e| e.iter().any(|x| matches!(x, Value::Object(Some(_))))))
            .unwrap_or(false)
        {
            return true;
        }
        cursor = match ow_inventory::equip::read_prop(vm, o, "Parent") {
            Some(Value::Object(Some(p))) => Some(p),
            _ => None,
        };
    }
    false
}

/// One class-code step of [`preload_material_classes`]: `path`'s layout and code through `Vm::find_class`, logged
/// before and after, unless the class is already defined with its code loaded.
fn class_code_load(vm: &mut Vm, path: &str) -> Result<(), VmError> {
    let ready = vm.space.classes.by_path(path).is_some_and(|c| {
        vm.space.classes.get_layout(c).is_some_and(|l| l.is_defined()) && vm.class_code_loaded(c)
    });
    if ready {
        return Ok(());
    }
    vm.log.push(format!("{MATERIAL_CLASS_INPUT}: the class code of {path} (loaded at boot in the game: UNVERIFIED)"));
    let found = vm.find_class(path);
    vm.log.extend(vm.defs.take_log());
    match &found {
        Ok(_) => vm.log.push(format!("{MATERIAL_CLASS_INPUT}: the class code of {path}: loaded")),
        Err(e) => vm.log.push(format!("{MATERIAL_CLASS_INPUT}: the class code of {path}: failed ({e})")),
    }
    found.map(|_| ())
}

/// The text of [`install_mesh_layer`]'s line.
pub(crate) const LOD_LAYER_LINE: &str = "harness input: the skeletal mesh data layer reads a mesh's LOD models from its own package \
     export (ow_assets::skeletal_lods; a fitted layout checked by structural oracles, UNVERIFIED)";

/// Lane SKELMESH-LOD-LAYER: in Off, the equip world gets the skeletal mesh data layer, as the combat run's does
/// (`combat::install_skeleton_source`, logged as a harness input there), so `SetSkeletalMesh`'s LOD row can ask
/// it for the new mesh's LOD count. The count is read from the player's own packages, not from a stand-in. The
/// install is logged once; then the count the layer gives for the weapon's resident gestalt mesh (the mesh
/// `SetSkeletalMesh` is handed) is logged, a read with no write. A world that has the layer already gets no second
/// install. Reference gets nothing.
pub(crate) fn install_mesh_layer(vm: &mut Vm, weapon: ObjRef) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    if vm.service::<ow_world::world::skeletal::SkeletalMeshes>().is_none() {
        crate::cmd::combat::install_skeleton_source(vm, &crate::cmd::combat::psk_roots())?;
        vm.log.push(LOD_LAYER_LINE.to_string());
    }
    let Some(mesh) = gestalt_skeletal_mesh(vm, weapon) else {
        return Ok(());
    };
    if vm.defs.is_unloaded(&vm.space, mesh) {
        return Ok(());
    }
    let name = ow_vm::path_name::path_name(vm, mesh).unwrap_or_else(|_| format!("{mesh:?}"));
    // A refusal or a parse failure is logged, not raised: `SetSkeletalMesh`'s LOD row asks again and reports it.
    let shown = match ow_world::world::skeletal::lod_count(vm, mesh) {
        Ok(Some(n)) => match ow_world::world::skeletal::lod_trailing_counts(vm, mesh) {
            Ok(Some(t)) => format!("{n} LOD model(s), trailing counts {t:?}"),
            Ok(None) => format!("{n} LOD model(s), trailing counts not read"),
            Err(e) => format!("{n} LOD model(s), trailing counts not read ({e})"),
        },
        Ok(None) => "no answer".to_string(),
        Err(e) => format!("no answer ({e})"),
    };
    vm.log.push(format!("{LOD_LAYER_LINE}: {name} has {shown}"));
    Ok(())
}

/// The weapon type's gestalt definition's `GestaltSkeletalMesh`, by reads only (the path of
/// [`preload_gestalt_skeletal_mesh`]); `None` where that path ends.
fn gestalt_skeletal_mesh(vm: &Vm, weapon: ObjRef) -> Option<ObjRef> {
    let data = ow_inventory::equip::read_prop(vm, weapon, "DefinitionData")?;
    let Ok(Value::Object(Some(weapon_type))) = ow_attr::eval::field(&vm.space, &data, "WeaponTypeDefinition").cloned()
    else {
        return None;
    };
    let Some(Value::Object(Some(gestalt))) = ow_inventory::equip::read_prop(vm, weapon_type, "GestaltMesh") else {
        return None;
    };
    if vm.defs.is_unloaded(&vm.space, gestalt) {
        return None;
    }
    match ow_inventory::equip::read_prop(vm, gestalt, "GestaltSkeletalMesh") {
        Some(Value::Object(Some(mesh))) => Some(mesh),
        _ => None,
    }
}

/// The text of [`preload_gestalt_mesh`]'s harness lines.
pub(crate) const GESTALT_INPUT: &str = "harness input: preload the weapon type's GestaltMesh";

/// The Off harness preload of lane HARNESS-LINK8-GESTALT-PRELOAD (the hub's ruling on stop 3 of ANALYST-LINK8-NEXT):
/// the weapon's `DefinitionData.WeaponTypeDefinition`'s `GestaltMesh`, when it is set and one of the store's unloaded
/// handles, is loaded through the store. `BuildCompositeMesh`'s preflight (NATIVE_BUILD_COMPOSITE_MESH.md section 4,
/// step 3) refuses a gestalt definition that is not resident, because a lazy load is a write; in the game it is
/// resident with the weapon type, which this load stands in for (**UNVERIFIED**). It is logged before it runs (rule 7)
/// and its outcome after; a failed load keeps its error. No type, no `GestaltMesh`, or one already loaded: nothing.
/// Outside Off, nothing.
pub(crate) fn preload_gestalt_mesh(vm: &mut Vm, weapon: ObjRef) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    let data = ow_inventory::equip::read_prop(vm, weapon, "DefinitionData")
        .ok_or_else(|| script("the weapon has no DefinitionData".into()))?;
    let Ok(Value::Object(Some(weapon_type))) = ow_attr::eval::field(&vm.space, &data, "WeaponTypeDefinition").cloned()
    else {
        return Ok(());
    };
    let Some(Value::Object(Some(gestalt))) = ow_inventory::equip::read_prop(vm, weapon_type, "GestaltMesh") else {
        return Ok(());
    };
    if !vm.defs.is_unloaded(&vm.space, gestalt) {
        return Ok(());
    }
    let name = ow_vm::path_name::path_name(vm, gestalt).unwrap_or_else(|_| format!("{gestalt:?}"));
    vm.log.push(format!(
        "{GESTALT_INPUT} {name} (BuildCompositeMesh loads nothing; the game keeps it resident: UNVERIFIED stand-in)"
    ));
    let loaded = vm.defs.ensure_loaded(&mut vm.space, gestalt);
    vm.log.extend(vm.defs.take_log());
    match &loaded {
        Ok(()) => vm.log.push(format!("{GESTALT_INPUT} {name}: loaded")),
        Err(e) => vm.log.push(format!("{GESTALT_INPUT} {name}: failed ({e})")),
    }
    loaded.map_err(VmError::from)
}

/// The text of [`preload_gestalt_skeletal_mesh`]'s harness lines.
pub(crate) const NEW_MESH_INPUT: &str = "harness input: preload the gestalt definition's GestaltSkeletalMesh";

/// The Off harness preload of lane LINK8-NEWMESH-PRELOAD, run right after [`preload_gestalt_mesh`]: the weapon type's
/// gestalt definition's `GestaltSkeletalMesh`, when the definition is resident and the mesh is one of the store's
/// unloaded handles, is loaded through the store. `BuildCompositeMesh`'s gestalt path answers that shared mesh
/// (NATIVE_BUILD_COMPOSITE_MESH.md section 2), and `InitMeshes` hands it to `SetSkeletalMesh`, whose rule refuses an
/// unloaded new mesh because a lazy load is a write (NATIVE_SKELMESH_MESH_CHANGE.md row 6). In the game the mesh is
/// resident with the definition that references it, which this load stands in for (**UNVERIFIED stand-in**). It is
/// logged before it runs (rule 7) and its outcome after; a failed load keeps its error. No type, no gestalt
/// definition, a cold definition, no mesh, or a mesh already loaded: nothing. Outside Off, nothing.
pub(crate) fn preload_gestalt_skeletal_mesh(vm: &mut Vm, weapon: ObjRef) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    let data = ow_inventory::equip::read_prop(vm, weapon, "DefinitionData")
        .ok_or_else(|| script("the weapon has no DefinitionData".into()))?;
    let Ok(Value::Object(Some(weapon_type))) = ow_attr::eval::field(&vm.space, &data, "WeaponTypeDefinition").cloned()
    else {
        return Ok(());
    };
    let Some(Value::Object(Some(gestalt))) = ow_inventory::equip::read_prop(vm, weapon_type, "GestaltMesh") else {
        return Ok(());
    };
    if vm.defs.is_unloaded(&vm.space, gestalt) {
        return Ok(());
    }
    let Some(Value::Object(Some(mesh))) = ow_inventory::equip::read_prop(vm, gestalt, "GestaltSkeletalMesh") else {
        return Ok(());
    };
    if !vm.defs.is_unloaded(&vm.space, mesh) {
        return Ok(());
    }
    let name = ow_vm::path_name::path_name(vm, mesh).unwrap_or_else(|_| format!("{mesh:?}"));
    vm.log.push(format!(
        "{NEW_MESH_INPUT} {name} (SetSkeletalMesh loads nothing; the game has it resident with the gestalt definition: \
         UNVERIFIED stand-in)"
    ));
    let loaded = vm.defs.ensure_loaded(&mut vm.space, mesh);
    vm.log.extend(vm.defs.take_log());
    match &loaded {
        Ok(()) => vm.log.push(format!("{NEW_MESH_INPUT} {name}: loaded")),
        Err(e) => vm.log.push(format!("{NEW_MESH_INPUT} {name}: failed ({e})")),
    }
    loaded.map_err(VmError::from)
}

/// The definition fields whose `MuzzleFlashPSTemplates` the weapon's client initialisation asks, in its order
/// (NATIVE_GET_PARTICLE_EFFECT.md section 1): the barrel part's, then the weapon type's.
const MUZZLE_FLASH_HOLDERS: [&str; 2] = ["BarrelPartDefinition", "WeaponTypeDefinition"];

/// The Off harness preload of lane IMPL-GET-PARTICLE-EFFECT (the hub's ruling): for each set field of
/// [`MUZZLE_FLASH_HOLDERS`] whose `MuzzleFlashPSTemplates` is set, that collection is loaded, then the entries'
/// `Expression`s in array order up to the first entry with none (the native's preflight boundary; review r1 of #573,
/// item 4), and for an `AttributeExpressionEvaluator` its operands' attribute definitions and their chains' resolver
/// objects (item 2: the native's preflight checks them resident). Each load is logged as a harness input before it
/// runs (rule 7). `GetParticleEffect` itself loads nothing and refuses a cold handle. The particle systems are not
/// loaded (the native returns them as stored). Outside Off, nothing.
pub(crate) fn preload_muzzle_flashes(vm: &mut Vm, weapon: ObjRef) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    let data = ow_inventory::equip::read_prop(vm, weapon, "DefinitionData")
        .ok_or_else(|| script("the weapon has no DefinitionData".into()))?;
    let mut collections = Vec::new();
    for field in MUZZLE_FLASH_HOLDERS {
        let Some(Value::Object(Some(holder))) = ow_attr::eval::field(&vm.space, &data, field).ok().cloned() else {
            continue;
        };
        if let Some(Value::Object(Some(c))) = ow_inventory::equip::read_prop(vm, holder, "MuzzleFlashPSTemplates") {
            collections.push(c);
        }
    }
    for c in collections {
        preload_flash(vm, c, "a muzzle-flash collection")?;
        let entries = ow_inventory::equip::read_prop(vm, c, "ParticleEffects").unwrap_or(Value::None);
        let mut evaluators = Vec::new();
        for e in entries.elements().unwrap_or(&[]) {
            match ow_attr::eval::field(&vm.space, e, "Expression") {
                Ok(Value::Object(Some(o))) => evaluators.push(*o),
                _ => break,
            }
        }
        for e in evaluators {
            preload_flash(vm, e, "its evaluator")?;
            preload_operands(vm, e)?;
        }
    }
    Ok(())
}

/// The operands of an `AttributeExpressionEvaluator` `e` ([`preload_muzzle_flashes`]): each set operand's attribute
/// definition, then the resolver objects of its context and value chains. Any other evaluator class, nothing.
fn preload_operands(vm: &mut Vm, e: ObjRef) -> Result<(), VmError> {
    let Some(class) = vm.space.classes.by_path(ow_inventory::equip::particle_effect::ATTRIBUTE_EVALUATOR) else {
        return Ok(());
    };
    if !vm.space.classes.is_child_of(vm.space.heap.get(e)?.class, class) {
        return Ok(());
    }
    let Some(expression) = ow_inventory::equip::read_prop(vm, e, "Expression") else {
        return Ok(());
    };
    for name in ["AttributeOperand1", "AttributeOperand2"] {
        let Ok(Value::Object(Some(attr))) = ow_attr::eval::field(&vm.space, &expression, name).cloned() else {
            continue;
        };
        preload_flash(vm, attr, "an evaluator's attribute")?;
        for chain in ["ContextResolverChain", "ValueResolverChain"] {
            let resolvers: Vec<ObjRef> = ow_inventory::equip::read_prop(vm, attr, chain)
                .as_ref()
                .and_then(Value::elements)
                .unwrap_or(&[])
                .iter()
                .filter_map(|v| match v {
                    Value::Object(Some(o)) => Some(*o),
                    _ => None,
                })
                .collect();
            for r in resolvers {
                preload_flash(vm, r, "its resolver")?;
            }
        }
    }
    Ok(())
}

/// One logged harness load (rule 7: labelled before it runs, so a failed load still shows).
fn preload_flash(vm: &mut Vm, o: ObjRef, what: &str) -> Result<(), VmError> {
    let name = ow_vm::path_name::path_name(vm, o).unwrap_or_else(|_| format!("{o:?}"));
    vm.log.push(format!("harness input: preload {what} {name} (GetParticleEffect loads nothing)"));
    let loaded = vm.defs.ensure_loaded(&mut vm.space, o);
    vm.log.extend(vm.defs.take_log());
    if let Err(e) = &loaded {
        vm.log.push(format!("harness input: preload {name}: failed ({e})"));
    }
    loaded.map_err(VmError::from)
}

/// Off only (lane IMPL-GET-EXP-LEVEL; the hub's ruling on NATIVE_GET_EXP_LEVEL_FROM_MANUFACTURER_DATA.md section 3):
/// the definition data's `ManufacturerDefinition` and `ManufacturerGradeIndex`, as the game's spawn sets them (the
/// candidate expansion: the balance's manufacturer, the grade index word the level). The manufacturer is the balance's
/// first listed one (`GD_Manufacturers.Manufacturers.Bandit` for link 8's rifle). Each is logged as an UNVERIFIED
/// harness input. A balance listing no manufacturer leaves both as they are.
fn manufacturer_inputs(vm: &mut Vm, data: &mut Value, balance: ObjRef, level: i32) -> Result<(), VmError> {
    let first = ow_inventory::equip::read_prop(vm, balance, "Manufacturers")
        .and_then(|list| list.elements().and_then(|e| e.first().cloned()))
        .and_then(|entry| ow_combat::hit::access::field(vm, &entry, "Manufacturer"));
    let Some(Value::Object(Some(manufacturer))) = first else {
        return Ok(());
    };
    set_field(vm, data, "ManufacturerDefinition", Value::Object(Some(manufacturer))).map_err(script)?;
    set_field(vm, data, "ManufacturerGradeIndex", Value::Int(level)).map_err(script)?;
    let path = ow_vm::path_name::path_name(vm, manufacturer).unwrap_or_default();
    vm.log.push(format!(
        "harness input: DefinitionData.ManufacturerDefinition {path}, ManufacturerGradeIndex {level} (UNVERIFIED)"
    ));
    Ok(())
}

/// The weapon of a request, its definition data and level set, with no owner. `wrote` is set once the weapon exists.
/// In Off it gets its own components, in the spawn path's order ([`own_components`]).
fn weapon_of(vm: &mut Vm, r: &DiffRequest, wrote: &mut bool, route: WeaponRoute) -> Result<ObjRef, VmError> {
    let weapon = if vm.parity == ParityMode::Off {
        let defaults = vm.defs.class_default(&mut vm.space, WEAPON);
        vm.log.extend(vm.defs.take_log());
        let defaults = defaults.map_err(VmError::from)?;
        own_components(vm, defaults, wrote, |vm| make_t(vm, WEAPON))?
    } else {
        let weapon = make_t(vm, WEAPON)?;
        *wrote = true;
        weapon
    };
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
    if vm.parity == ow_core::ParityMode::Off && route.manufacturer_inputs() {
        manufacturer_inputs(vm, &mut data, balance, r.level)?;
    }
    write_t(vm, weapon, "DefinitionData", data)?;
    write_t(vm, weapon, "ExpLevel", Value::Int(r.level))?;
    Ok(weapon)
}

#[cfg(test)]
#[path = "equip_run_gestalt_tests.rs"]
mod gestalt_tests;

#[cfg(test)]
#[path = "equip_run_game_info_tests.rs"]
mod game_info_tests;

#[cfg(test)]
#[path = "equip_run_material_tests.rs"]
mod material_tests;

#[cfg(test)]
#[path = "equip_run_ammo_tests.rs"]
mod ammo_tests;

#[cfg(test)]
mod tests {
    use super::*;

    /// Every route sets the manufacturer inputs (lane IMPL-LINK67-PICKUP-GUN-INIT, NATIVE_LINK67_PICKUP_GUN_INIT.md
    /// section 3; before it, review r1 of #577 kept them to link 8 and `--equip-run`): the pickup links' held weapon and
    /// link 7's decoy run the same init as link 8's gun.
    #[test]
    fn every_route_sets_the_manufacturer_inputs() {
        assert!(WeaponRoute::EquipRun.manufacturer_inputs());
        assert!(WeaponRoute::Link8.manufacturer_inputs());
        assert!(WeaponRoute::Pickup.manufacturer_inputs());
        assert!(WeaponRoute::PickupDecoy.manufacturer_inputs());
    }

    /// Invented packages for the weapon preparation paths (AGENTS.md rule 9): `WillowGame.upk` declares the weapon
    /// (its `DefinitionData` with the five fields the paths write or keep), the owner classes and a balance class whose
    /// `Manufacturers` lists structs with a `Manufacturer`; `GD_ToyGun.upk` holds a weapon type, a manufacturer and a
    /// balance listing it.
    fn toy_gun_vm(mode: ParityMode) -> (Vm, std::path::PathBuf) {
        use ow_package::testkit::ints;
        use ow_package::testkit::reflect::{toy_array_of, toy_struct};
        use ow_package::testkit::script::ScriptToy;
        let core = {
            let mut t = ScriptToy::new(&[]);
            t.class("Object", 0);
            t.build()
        };
        let willow = {
            let mut t = ScriptToy::with_imports(&[
                "Class",
                "ObjectProperty",
                "IntProperty",
                "StructProperty",
                "ArrayProperty",
                "ScriptStruct",
            ]);
            let core = t.p.add_import("Core", "Package", 0, "Core");
            let object = t.p.add_import("Core", "Class", core, "Object");
            let weapon = t.class("WillowWeapon", object);
            let data = toy_struct(&mut t, weapon, "WeaponDefinitionData", None);
            for name in ["WeaponTypeDefinition", "BalanceDefinition", "ManufacturerDefinition"] {
                t.prop("Object", data, name, 0, object);
            }
            for name in ["GameStage", "ManufacturerGradeIndex"] {
                t.prop("Int", data, name, 0, 0);
            }
            t.prop("Struct", weapon, "DefinitionData", 0, data);
            t.prop("Int", weapon, "ExpLevel", 0, 0);
            t.prop("Object", weapon, "Instigator", 0, object);
            t.prop("Object", weapon, "Owner", 0, object);
            let pawn = t.class("WillowPlayerPawn", object);
            t.prop("Object", pawn, "Controller", 0, object);
            t.prop("Object", pawn, "InvManager", 0, object);
            let controller = t.class("WillowPlayerController", object);
            t.prop("Object", controller, "Pawn", 0, object);
            let manager = t.class("WillowInventoryManager", object);
            t.prop("Object", manager, "Owner", 0, object);
            t.class("WeaponTypeDefinition", object);
            t.class("ManufacturerDefinition", object);
            let balance = t.class("WeaponBalanceDefinition", object);
            let entry = toy_struct(&mut t, balance, "ManufacturerEntry", None);
            t.prop("Object", entry, "Manufacturer", 0, object);
            toy_array_of(&mut t, balance, "Manufacturers", "Struct", entry);
            t.build()
        };
        let gd = {
            let mut t = ScriptToy::with_imports(&["Class"]);
            let willow = t.p.add_import("Core", "Package", 0, "WillowGame");
            let weapon_type = t.p.add_import("Core", "Class", willow, "WeaponTypeDefinition");
            let maker = t.p.add_import("Core", "Class", willow, "ManufacturerDefinition");
            let balance = t.p.add_import("Core", "Class", willow, "WeaponBalanceDefinition");
            let mut empty = ints(&[0]);
            empty.extend_from_slice(&t.none);
            t.p.add_export(weapon_type, "Type_A", empty.clone());
            let m = t.p.add_export(maker, "Maker_A", empty);
            let mut element = t.p.object_tag("Manufacturer", m);
            element.extend_from_slice(&t.none);
            let mut payload = ints(&[0]);
            payload.extend(t.p.array_tag("Manufacturers", 1, &element));
            payload.extend_from_slice(&t.none);
            t.p.add_export(balance, "Balance_A", payload);
            t.build()
        };
        let dir = std::env::temp_dir().join(format!("ow-cli-toy-gun-{}-{mode:?}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let files: Vec<(&str, &[u8])> =
            vec![("Core.upk", core.as_slice()), ("WillowGame.upk", willow.as_slice()), ("GD_ToyGun.upk", gd.as_slice())];
        let defs = DefStore::from_synthetic(&dir, &files, IdentityPolicy::FirstInScanOrder, mode).unwrap();
        let vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), mode);
        (vm, dir)
    }

    const TOY_LINE: &str = "GD_ToyGun.Type_A 7 GD_ToyGun.Balance_A";

    /// The definition data's manufacturer inputs of the newest toy weapon: (`ManufacturerDefinition` set,
    /// `ManufacturerGradeIndex`).
    fn manufacturer_fields(vm: &mut Vm) -> (bool, i32) {
        let class = vm.find_class(WEAPON).unwrap();
        let weapon = vm.space.heap.live(&vm.space.classes, class).last().expect("a toy weapon");
        let data = ow_inventory::equip::read_prop(vm, weapon, "DefinitionData").unwrap();
        let maker = matches!(ow_combat::hit::access::field(vm, &data, "ManufacturerDefinition"), Some(Value::Object(Some(_))));
        let index = match ow_combat::hit::access::field(vm, &data, "ManufacturerGradeIndex") {
            Some(Value::Int(i)) => i,
            other => panic!("{other:?}"),
        };
        (maker, index)
    }

    /// The real preparation paths on invented data (review r2 of #577; lane IMPL-LINK67-PICKUP-GUN-INIT). In Off every
    /// path sets `ManufacturerDefinition` (the balance's manufacturer) and `ManufacturerGradeIndex` (the level): link
    /// 7's decoy (`prepare_unowned_typed`), the pickup links' held weapon (`prepare_line_typed`) and link 8's
    /// (`prepare_link8_typed`). The held weapon's and link 8's paths go on to the owners' accuracy pool, which the toy
    /// data does not hold, so they stop there, after the weapon's definition data is written. Reference sets none.
    #[test]
    fn every_path_sets_the_manufacturer_fields_in_off() {
        let (mut vm, dir) = toy_gun_vm(ParityMode::Off);
        prepare_unowned_typed(&mut vm, TOY_LINE, &mut false).expect("the decoy needs no owner");
        assert_eq!(manufacturer_fields(&mut vm), (true, 7), "link 7's decoy");
        let held = prepare_line_typed(&mut vm, TOY_LINE, &mut false);
        assert!(held.is_err(), "the toy data holds no accuracy pool");
        assert_eq!(manufacturer_fields(&mut vm), (true, 7), "the pickup links' held weapon");
        let _ = prepare_link8_typed(&mut vm, TOY_LINE, &mut false);
        assert_eq!(manufacturer_fields(&mut vm), (true, 7), "link 8 sets both in Off");
        drop(vm);
        let _ = std::fs::remove_dir_all(&dir);
        // Reference: no route sets them, link 7's decoy route included (review of #761).
        let (mut vm, dir) = toy_gun_vm(ParityMode::Reference);
        let _ = prepare_unowned_typed(&mut vm, TOY_LINE, &mut false);
        assert_eq!(manufacturer_fields(&mut vm), (false, 0), "Reference's decoy leaves them");
        let _ = prepare_link8_typed(&mut vm, TOY_LINE, &mut false);
        assert_eq!(manufacturer_fields(&mut vm), (false, 0), "Reference leaves them");
        drop(vm);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Review r1 of #599, item 2: on an invented template that owns one component, the made object's slot names a
    /// copy outered to it, not the template's component; the harness line is logged before, with its outcome.
    #[test]
    fn own_components_copies_the_templates_component_and_logs_the_input() {
        use ow_object::heap::Origin;
        use ow_object::layout::{PropKind, PropertyDecl};
        let mut space = ObjectSpace::new();
        let object = space.classes.define("Core.Object", None, vec![]).unwrap();
        let base = space.classes.define("Core.Component", Some(object), vec![]).unwrap();
        let toy = space.classes.define("Test.ToyMeshComponent", Some(base), vec![]).unwrap();
        let decls = vec![PropertyDecl::new(space.names.intern("Mesh"), PropKind::Object { class: toy })];
        let weapon_class = space.classes.define("Test.ToyWeapon", Some(object), decls).unwrap();
        let mut vm = Vm::new(space, DefStore::new_empty(), Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
        let template = vm.space.alloc(weapon_class, "Default__ToyWeapon", None, Origin::Runtime).unwrap();
        let component = vm.space.alloc(toy, "MeshComp", Some(template), Origin::Runtime).unwrap();
        write_t(&mut vm, template, "Mesh", Value::Object(Some(component))).unwrap();
        let mut wrote = false;
        let weapon = own_components(&mut vm, template, &mut wrote, |vm| {
            let made = vm.instantiate(weapon_class, "ToyWeapon_0")?;
            let slots = vm.space.heap.get(template)?.slots.clone();
            vm.space.heap.get_mut(made)?.slots = slots;
            Ok(made)
        })
        .unwrap();
        assert!(wrote);
        let p = vm.space.prop(weapon_class, "Mesh").unwrap();
        let Value::Object(Some(copy)) = vm.space.read(weapon, PropAddr::scalar(p)).unwrap().clone() else {
            panic!("a component")
        };
        assert_ne!(copy, component, "the weapon's own copy");
        assert_eq!(vm.space.heap.get(copy).unwrap().outer, Some(weapon), "outered to the weapon");
        let p = vm.space.prop(weapon_class, "Mesh").unwrap();
        assert_eq!(vm.space.read(template, PropAddr::scalar(p)).unwrap(), &Value::Object(Some(component)), "template kept");
        let lines: Vec<&String> = vm.log.iter().filter(|l| l.starts_with(WEAPON_COMPONENTS_INPUT)).collect();
        assert_eq!(lines.len(), 2, "{:?}", vm.log);
        assert_eq!(lines[0], WEAPON_COMPONENTS_INPUT, "logged before it runs");
        assert_eq!(lines[1], &format!("{WEAPON_COMPONENTS_INPUT}: 1 component(s)"));
    }

    /// Review r2 of #632: a failed set-up whose partial write also fails (the output is a directory) still answers the
    /// set-up error, byte for byte; with a writable output the partial lines are written.
    #[test]
    fn a_failed_set_up_answers_its_own_error_even_when_the_write_fails() {
        let dir = std::env::temp_dir().join(format!("ow-cli-equip-run-failed-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let as_dir = dir.to_string_lossy().into_owned();
        let e = failed_set_up(&as_dir, String::new(), 3, "Toy.WT", "the toy set-up refused", "\"harness input: H\"");
        assert_eq!(e, "line 3: the toy set-up refused");
        assert!(dir.is_dir(), "the directory is left as it was");
        let file = dir.join("out.jsonl");
        let e = failed_set_up(&file.to_string_lossy(), "{\"line\":1}\n".into(), 3, "Toy.WT", "X", "");
        assert_eq!(e, "line 3: X");
        let written = std::fs::read_to_string(&file).unwrap();
        assert_eq!(written, "{\"line\":1}\n{\"line\":3,\"weapon_type\":\"Toy.WT\",\"make\":\"X\",\"harness\":[]}\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_summary_counts_passes_and_groups_stops_per_phase() {
        let mut stops: [BTreeMap<String, usize>; PHASES.len()] = Default::default();
        stops[0].insert("unbound native X".into(), 2);
        stops[2].insert("NotYet Y".into(), 1);
        let s = summary(2, &[0, 2, 1, 2], &stops, &["harness input: H".to_string()]);
        assert_eq!(
            s,
            "{\"weapons\":2,\"init\":{\"passed\":0,\"stops\":[{\"stop\":\"unbound native X\",\"weapons\":2}]},\
             \"bases\":{\"passed\":2,\"stops\":[]},\"equip\":{\"passed\":1,\"stops\":[{\"stop\":\"NotYet Y\",\
             \"weapons\":1}]},\"unequip\":{\"passed\":2,\"stops\":[]},\"harness\":[\"harness input: H\"]}\n"
        );
    }
}
