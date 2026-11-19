//! Owner: lane W6-COMBAT. Commands: `--combat-run`, the port of its branch in `src/cli.cpp`. AI-assisted (Claude
//! Code).
//!
//! `<package> --combat-run --cooked <dir> <step>...` spawns pawns from their stock archetypes on a world over the
//! cooked folder, applies the steps and prints the JSON report of `ow_combat::run` (one line), exit 0 when no step
//! failed and 1 when one did. The source package loads first, as the C++ loads it before it reads the mode; the
//! package argument is any package (the archetypes name their own). Errors before the run print `ow-package:
//! <message>` with exit 1; a `NotYet` before the run exits 3.
//!
//! **The mission bridge** (lane W6-COMBAT-RUN2). A run with a `maya` step is built over W6-MISSION's bridge, as the
//! C++ `combatRun`: the stock Fire mission of `Startup` gives the bridge its curve and globals (nothing of it is
//! played), and [`MissionBridge`] hands the run the bridge's player (`ow_combat::run::PlayerBridge`).
//!
//! **The world.** A definition store over the cooked folder (first holder in scan order), the standard attribute
//! system, the Core natives, and `World::new` with the combat pack (`ow_combat::hit::Pack`) and the skill manager
//! pack (`ow_skills::action_skill::manager::Pack`, Off only) after the world's own packs. Its balance state is the first playthrough, no OP level, one player, level and stage 1 (the C++ run has no
//! playthrough input and reads the first playthrough's rules).
//!
//! **The stub policy** (architecture section 3.8). `ParityMode::Off`: `Refuse` with nothing allowed, so a native the
//! chain reaches that nothing binds stops the step with the VM's `UnboundNative` text. `ParityMode::Reference`:
//! `LogAndZero`, which is what the C++ run does with every unbound native (logged as `UNIMPLEMENTED <key>`, counted in
//! the report's `stubs`, a zero result): the reference stand-in, so that a real-package run compares with the C++.
//!
//! `mode` is the run's parity mode, read once by `main`; it goes to the definition store and the VM.
//!
//! **The map route** (lane IMPL-SLICE-MAP-WORLD, Off only): `<package> --combat-run --cooked <dir> --map <name>
//! <step>...` builds the world by loading the map and bringing it up for play, spawns the local player's play actor,
//! and adopts the bridge from that world ([`map_route::build`], [`MissionBridge::adopt`]). Without `--map` nothing
//! changes. Under Reference `--map` is refused (`NotYet`, exit 3).

use std::io::Write as _;
use std::path::Path;
use std::process::ExitCode;
use std::rc::Rc;

use ow_attr::AttributeSystem;
use ow_balance::types::{
    AwesomeLevel, BalanceSnapshot, ExpLevel, GameStage, LevelCapUpgrades, OpLevel, PlayerCount,
    Playthrough,
};
use ow_behavior::provider::BehaviorError;
use ow_combat::run::{BridgeGain, PlayerBridge};
use ow_core::{ObjectPath, PackageName, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_mission::bridge::{Gain, MissionScript, PLAYER_CLASS_PATH};
use ow_mission::tracker::MissionSystem;
use ow_object::heap::ObjectSpace;
use ow_object::ids::ObjRef;
use ow_object::value::Value;
use ow_package::package::Package;
use ow_vm::api::{NativePack, StubPolicy, Vm};
use ow_vm::error::VmError;
use ow_vm::interp::on_vm_stack;
use ow_world::world::{BalanceState, World};

use crate::args::{Command, USAGE};
use crate::cmd::NOT_PORTED;
use crate::cmd::properties::error_text;

/// Why the command failed before or around the run.
enum Failure {
    Message(String),
    NotYet(String),
}

fn vm_failure(e: VmError) -> Failure {
    match e {
        VmError::NotYet(n) => Failure::NotYet(n.to_string()),
        other => Failure::Message(ow_combat::hit::access::error_text(&other)),
    }
}

pub fn run(_command: &Command, args: &[String], mode: ParityMode) -> ExitCode {
    let args = args.to_vec();
    let outcome = on_vm_stack(move || execute(&args, mode));
    let result = match outcome {
        Ok(r) => r,
        Err(e) => Err(Failure::Message(e.to_string())),
    };
    match result {
        Ok((text, failed)) => {
            // Ignore a failed write (a closed pipe), as the C++ binary does.
            let _ = writeln!(std::io::stdout().lock(), "{text}");
            ExitCode::from(if failed { 1 } else { 0 })
        }
        Err(Failure::Message(message)) => {
            eprintln!("ow-package: {message}");
            ExitCode::from(1)
        }
        Err(Failure::NotYet(message)) => {
            eprintln!("ow-package (Rust): {message}");
            ExitCode::from(NOT_PORTED)
        }
    }
}

/// The world of a run over `cooked` (module documentation).
pub fn world(cooked: &Path, mode: ParityMode) -> Result<World, VmError> {
    let defs = DefStore::open(cooked.to_path_buf(), IdentityPolicy::FirstInScanOrder, mode)
        .map_err(VmError::Def)?;
    let attrs = AttributeSystem::standard().map_err(VmError::Attr)?;
    let policy = match mode {
        ParityMode::Off => StubPolicy::refuse_all(),
        ParityMode::Reference => StubPolicy::LogAndZero,
    };
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(attrs), mode).with_stub_policy(policy);
    ow_vm::natives_core::register(&mut vm)?;
    let snapshot = BalanceSnapshot::new(
        Playthrough::NORMAL,
        OpLevel::new(0).map_err(|e| VmError::Script {
            function: "combat-run".into(),
            message: e.to_string(),
        })?,
        PlayerCount::SINGLE,
        ExpLevel::new(1),
        GameStage(1),
        AwesomeLevel::new(0),
        LevelCapUpgrades::ALL,
    )
    .map_err(|e| VmError::Script {
        function: "combat-run".into(),
        message: e.to_string(),
    })?;
    let pack = ow_combat::hit::Pack;
    // The skill manager natives (FIX-SKILLMGR-B): bound in Off only, so a Reference run is unchanged.
    let manager = ow_skills::action_skill::manager::Pack;
    // The player pawn data manager natives (FIX-PAWN-DATA-B): bound in Off only, too.
    let pawn_data = ow_world::world::pawn_data::Pack;
    // InitPlayerSkillTree (IMPL-INIT-PLAYER-SKILL-TREE, decision 2026-11-19-88): bound in Off only, too.
    let skill_tree = ow_skills::tree::init::Pack;
    let packs: [&dyn NativePack; 4] = [&pack, &manager, &pawn_data, &skill_tree];
    let mut world = World::new(vm, BalanceState::new(snapshot), &packs).map_err(|e| match e {
        ow_world::world::WorldError::Vm(v) => v,
        other => VmError::Script {
            function: "combat-run".into(),
            message: other.to_string(),
        },
    })?;
    // The engine object with an empty online subsystem slot (decision 2026-11-19-112). Port Off answer: the port has
    // no online subsystem; UNVERIFIED against a logged-in game. Off only, so a Reference run is unchanged.
    if mode == ParityMode::Off {
        ow_world::world::online::register_offline_engine(&mut world.vm)?;
    }
    Ok(world)
}

/// The mission bridge's player for the run (module documentation).
pub struct MissionBridge {
    script: MissionScript,
}

/// A bridge error as a VM error (its `NotYet` stays one).
fn mission_error(e: BehaviorError) -> VmError {
    match e {
        BehaviorError::Vm(v) => v,
        other => VmError::Script {
            function: "mission script bridge".into(),
            message: other.to_string(),
        },
    }
}

impl MissionBridge {
    /// Builds the bridge on `world` (which must have no actors yet) over the C++ mission
    /// (`ow_combat::run::BRIDGE_MISSION`).
    pub fn new(world: &mut World) -> Result<MissionBridge, VmError> {
        let (package, path) = ow_combat::run::BRIDGE_MISSION;
        let package = PackageName::parse(package).map_err(|e| VmError::Script {
            function: "combat-run".into(),
            message: e.to_string(),
        })?;
        let mission = MissionSystem::new(&mut world.vm, &package, path).map_err(mission_error)?;
        let script = MissionScript::new(world, &mission).map_err(mission_error)?;
        // Off: the globals set-up step for the pawn data manager, where the bridge made the globals and before the
        // player's script runs (NATIVE_PAWN_DATA.md; its timing is the note's open question 1, UNVERIFIED).
        if world.vm.parity == ParityMode::Off {
            let globals = world.query().borrow().globals();
            if let Some(globals) = globals {
                ow_world::world::pawn_data::set_up(&mut world.vm, globals)?;
            }
        }
        Ok(MissionBridge { script })
    }

    /// The bridge adopted from a map world over the same mission as [`MissionBridge::new`]
    /// (`MissionScript::adopt`; the map route made the globals and ran their pawn data set-up before).
    pub fn adopt(world: &mut World) -> Result<MissionBridge, VmError> {
        let (package, path) = ow_combat::run::BRIDGE_MISSION;
        let package = PackageName::parse(package).map_err(|e| VmError::Script {
            function: "combat-run".into(),
            message: e.to_string(),
        })?;
        let mission = MissionSystem::new(&mut world.vm, &package, path).map_err(mission_error)?;
        let script = MissionScript::adopt(world, &mission).map_err(mission_error)?;
        Ok(MissionBridge { script })
    }
}

impl PlayerBridge for MissionBridge {
    fn player_controller(&self) -> ObjRef {
        self.script.player_controller()
    }

    fn player_pawn(&self, vm: &mut Vm) -> Result<ObjRef, VmError> {
        self.script.player_pawn(vm).map_err(mission_error)
    }

    fn set_player_experience(&self, vm: &mut Vm, level: i32, experience: i64) -> Result<(), VmError> {
        self.script
            .set_player_experience(vm, level, experience)
            .map_err(mission_error)
    }

    fn update_experience_pool(&self, vm: &mut Vm) -> Result<(), VmError> {
        self.script.update_experience_pool(vm).map_err(mission_error)
    }

    /// The C++ kinds by name; the bridge produces no health gains (its module documentation).
    fn take_gains(&self) -> Vec<BridgeGain> {
        self.script
            .take_gains()
            .into_iter()
            .map(|g| match g {
                Gain::Experience(n) => BridgeGain::new("experience", n, 0.0, 0.0),
                Gain::Level(n) => BridgeGain::new("level", n, 0.0, 0.0),
                Gain::SkillPoints(n) => BridgeGain::new("skill_points", n, 0.0, 0.0),
                _ => BridgeGain::new("unknown", 0, 0.0, 0.0),
            })
            .collect()
    }

    fn player_level(&self, vm: &Vm) -> Result<i32, VmError> {
        self.script.player_level(vm).map_err(mission_error)
    }

    fn experience_pool(&self) -> f32 {
        self.script.experience_pool()
    }

    fn errors(&self) -> Vec<String> {
        self.script.errors()
    }

    /// The player's class by its stock path (the definition store's first holder; the C++ copies it from the
    /// mission's package, `Startup`).
    fn player_class(&self, vm: &mut Vm) -> Result<Option<ObjRef>, VmError> {
        let path = ObjectPath::parse(PLAYER_CLASS_PATH).map_err(|e| VmError::Script {
            function: "combat-run".into(),
            message: e.to_string(),
        })?;
        let found = vm.defs.find(&mut vm.space, &path);
        vm.log.extend(vm.defs.take_log());
        found.map_err(VmError::Def)
    }

    /// Off: `ServerPlayerSelectClass(<Maya's class path>)` on the controller, once (a labelled harness input: the
    /// class-select call NATIVE_PLAYER_CLASS.md section 1 names as the other route to Maya's class, decision
    /// 2026-11-19-41). The installed script then finds the class (`GBXDefinition.GetDefinition`) and switches to it.
    /// A controller that already has a `PlayerClass` is left as it is.
    fn select_player_class(&self, vm: &mut Vm) -> Result<(), VmError> {
        let controller = self.player_controller();
        if vm.parity != ParityMode::Off || has_player_class(vm, controller)? {
            return Ok(());
        }
        // Reading the class's PawnArchetypePath loads the class through the store, before the class select: so the
        // installed GetDefinition then finds it in memory (review r1 of #431; decision 2026-11-19-46).
        vm.log.push(format!(
            "harness input: load {PLAYER_CLASS_PATH} (to read its PawnArchetypePath; an UNVERIFIED harness load)"
        ));
        let class = match self.player_class(vm) {
            Ok(class) => class,
            Err(e) => {
                vm.log.push(format!("harness input: load {PLAYER_CLASS_PATH}: failed ({e})"));
                return Err(e);
            }
        };
        apply_class_inputs(vm, controller, class)
    }
}

/// Whether `controller`'s `PlayerClass` is set.
fn has_player_class(vm: &Vm, controller: ObjRef) -> Result<bool, VmError> {
    let class = vm.space.heap.get(controller)?.class;
    Ok(vm
        .space
        .prop(class, "PlayerClass")
        .map(|p| vm.space.read(controller, ow_object::ids::PropAddr::scalar(p)).map(|v| !matches!(v, Value::Object(None))))
        .transpose()?
        .unwrap_or(false))
}

/// The player's animation tree template, which the made `Mesh`'s tree copy reads (NATIVE_COPY_ANIM_TREE.md).
pub(crate) const PLAYER_TREE_PATH: &str = "GD_PlayerShared.Character.AnimTree_Player";

/// A harness input (the hub's ruling for lane IMPL-MESH-REST): loads [`PLAYER_TREE_PATH`] and the nodes and controls
/// it reaches (`Children[].Anim`, `SkelControlLists[].ControlHead`, `NextControl`), as [`apply_class_inputs`] loads
/// the pawn archetype. In the game the tree arrives with the streamed player package; this stands in for that load
/// (UNVERIFIED). The natives load nothing: the tree copy's preflight refuses an unloaded node.
fn preload_player_tree(vm: &mut Vm) -> Result<(), VmError> {
    let path = ObjectPath::parse(PLAYER_TREE_PATH).expect("a valid path");
    vm.log.push(format!("harness input: preload {PLAYER_TREE_PATH} (UNVERIFIED stand-in for the streaming load)"));
    let found = vm.defs.find(&mut vm.space, &path);
    vm.log.extend(vm.defs.take_log());
    let tree = match found {
        Ok(Some(tree)) => tree,
        Ok(None) => {
            vm.log.push(format!("harness input: preload {PLAYER_TREE_PATH}: found nothing"));
            return Ok(());
        }
        Err(e) => {
            vm.log.push(format!("harness input: preload {PLAYER_TREE_PATH}: failed ({e})"));
            return Err(VmError::Def(e));
        }
    };
    let mut seen = vec![tree];
    let mut next = 0;
    while next < seen.len() {
        let o = seen[next];
        next += 1;
        if let Err(e) = vm.defs.ensure_loaded(&mut vm.space, o) {
            vm.log.extend(vm.defs.take_log());
            vm.log.push(format!("harness input: preload {PLAYER_TREE_PATH}: failed ({e})"));
            return Err(VmError::Def(e));
        }
        let class = vm.space.heap.get(o)?.class;
        for name in ["Children", "SkelControlLists", "NextControl"] {
            let Some(p) = vm.space.prop(class, name) else { continue };
            let mut found = Vec::new();
            objects_in(vm.space.read(o, ow_object::ids::PropAddr::scalar(p))?, &mut found);
            for r in found {
                if !seen.contains(&r) {
                    seen.push(r);
                }
            }
        }
    }
    vm.log.extend(vm.defs.take_log());
    vm.log.push(format!("harness input: preload {PLAYER_TREE_PATH}: loaded {} objects", seen.len()));
    Ok(())
}

/// Every object a value holds, through structs and arrays.
fn objects_in(v: &Value, out: &mut Vec<ObjRef>) {
    match v {
        Value::Object(Some(o)) => out.push(*o),
        Value::Struct(s) => s.fields.iter().for_each(|f| objects_in(f, out)),
        Value::Array(a) => a.iter().for_each(|e| objects_in(e, out)),
        _ => {}
    }
}

/// The rest of the streaming load's stand-in (hub ruling, lane SPAWN-COMPLETE, option (a)): the preloaded archetype's
/// object references that live in the archetype's own package file are loaded too, one level deep, through the store,
/// so the loads are the store's tracked ones. The game's reason: the whole streaming package is resident, so Maya's
/// `BodyClass` (`GD_Siren_Streaming.BodyClass_Siren`, the same package as `GD_Siren_Streaming.Pawn_Siren`) is loaded
/// by the time `ApplyCharacterClassDefaults` reads it off the archetype and hands it to `BodySelectClass`. An UNVERIFIED
/// stand-in, like the preload it extends: which exports the game's streaming load makes resident is not read.
///
/// Only object-valued slots are read (arrays and structs are not walked). A reference to another package is left alone.
/// Each load is logged. A fresh failure is the step's error, and a handle whose load already failed returns that same
/// error again, its kind kept (review r2 of #528, as the spawn does), so a retry never reaches the class select.
/// Returns the object paths loaded, in slot order.
pub(crate) fn load_same_package_references(vm: &mut Vm, archetype: ObjRef) -> Result<Vec<String>, VmError> {
    use ow_object::heap::Origin;
    let package_of = |vm: &Vm, o: ObjRef| match vm.space.heap.get(o).map(|x| x.origin) {
        Ok(Origin::Export { package, .. } | Origin::Resource { package, .. }) => Some(package),
        _ => None,
    };
    let Some(home) = package_of(vm, archetype) else {
        return Ok(Vec::new());
    };
    let mut refs: Vec<ObjRef> = Vec::new();
    for v in &vm.space.heap.get(archetype)?.slots {
        if let Value::Object(Some(r)) = v
            && !refs.contains(r)
        {
            refs.push(*r);
        }
    }
    let mut done = Vec::new();
    for r in refs {
        if package_of(vm, r) != Some(home) || !vm.defs.is_unloaded(&vm.space, r) {
            continue;
        }
        let path = match vm.space.heap.get(r).map(|x| x.origin) {
            Ok(Origin::Export { package, index } | Origin::Resource { package, index }) => {
                ow_defs::identity::full_path(vm.defs.packages().get(package), index).unwrap_or_else(|_| format!("{r:?}"))
            }
            _ => format!("{r:?}"),
        };
        vm.log.push(format!("harness input: preload {path} (same package as the archetype; UNVERIFIED stand-in)"));
        let loaded = vm.defs.ensure_loaded(&mut vm.space, r);
        vm.log.extend(vm.defs.take_log());
        loaded.map_err(VmError::from)?;
        done.push(path);
    }
    Ok(done)
}

/// The three Off harness inputs of the `maya` step, in order, each logged with its outcome (rule 7; decisions
/// 2026-11-19-41 and 2026-11-19-46, and lane IMPL-MESH-REST for the tree). Outside Off, or for a controller that
/// already has a `PlayerClass`, nothing.
/// 1. **Preload:** `class`'s own `PawnArchetypePath` is loaded through the definition store, an UNVERIFIED stand-in for
///    the streaming load NATIVE_PLAYER_CLASS.md does not settle. A missing, non-string or unparsable path is skipped
///    (and logged), and a load that finds nothing is logged; the class select then runs either way.
/// 2. **Tree preload:** Maya's animation tree and the nodes and controls it reaches ([`preload_player_tree`]).
/// 3. **Class select:** the controller's installed `ServerPlayerSelectClass(<Maya's class path>)`.
pub(crate) fn apply_class_inputs(vm: &mut Vm, controller: ObjRef, class: Option<ObjRef>) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off || has_player_class(vm, controller)? {
        return Ok(());
    }
    // The write state is carried through both inputs (review r1 of #528, item 1): a refusal after a preload or any
    // other write of the step, the class select's included, is marked "stopped after partial writes", its kind kept;
    // one with no write since the step began is returned as it is.
    let before = ow_world::pools::Counters::read(vm);
    class_inputs(vm, controller, class).map_err(|e| ow_world::pools::after_counted_writes(vm, before, e))
}

fn class_inputs(vm: &mut Vm, controller: ObjRef, class: Option<ObjRef>) -> Result<(), VmError> {
    match class {
        None => vm.log.push(format!("harness input: preload skipped (no class at {PLAYER_CLASS_PATH})")),
        Some(definition) => {
            let dclass = vm.space.heap.get(definition)?.class;
            let path = vm
                .space
                .prop(dclass, "PawnArchetypePath")
                .map(|p| vm.space.read(definition, ow_object::ids::PropAddr::scalar(p)).cloned())
                .transpose()?;
            match path {
                None => vm.log.push("harness input: preload skipped (the class has no PawnArchetypePath)".to_string()),
                Some(Value::Str(text)) => match ObjectPath::parse(&text) {
                    Err(_) => vm.log.push(format!("harness input: preload skipped ({text:?} is no object path)")),
                    Ok(path) => {
                        // The attempt is labelled before it runs, so a load that fails still shows (rule 7).
                        vm.log.push(format!(
                            "harness input: preload {text} (UNVERIFIED stand-in for the streaming load)"
                        ));
                        let loaded = vm.defs.find(&mut vm.space, &path);
                        vm.log.extend(vm.defs.take_log());
                        match loaded {
                            Ok(found) => {
                                vm.log.push(format!(
                                    "harness input: preload {text}: {}",
                                    if found.is_some() { "loaded" } else { "found nothing" }
                                ));
                                if let Some(archetype) = found {
                                    load_same_package_references(vm, archetype)?;
                                }
                            }
                            Err(e) => {
                                vm.log.push(format!("harness input: preload {text}: failed ({e})"));
                                return Err(VmError::from(e));
                            }
                        }
                    }
                },
                Some(other) => vm.log.push(format!(
                    "harness input: preload skipped (PawnArchetypePath holds {}, not a string)",
                    other.kind_name()
                )),
            }
        }
    }
    preload_player_tree(vm)?;
    let own = vm.space.heap.get(controller)?.class;
    let state = vm.space.heap.get(controller)?.state;
    let Some(f) = vm.find_method(own, "ServerPlayerSelectClass", state)? else {
        return Err(VmError::Script {
            function: "combat-run".into(),
            message: "the player's controller has no ServerPlayerSelectClass".into(),
        });
    };
    vm.log.push(format!("harness input: ServerPlayerSelectClass({PLAYER_CLASS_PATH})"));
    vm.call(f, Some(controller), vec![Value::Str(PLAYER_CLASS_PATH.into())])?;
    Ok(())
}

/// The `--map <name>` option right after `--cooked <dir>` (module documentation): the map's name and the steps after
/// it; `None` without the option. A missing name is the usage message; any mode but Off is `NotYet`.
fn map_option(args: &[String], mode: ParityMode) -> Result<Option<(&str, &[String])>, Failure> {
    if args.get(4).map(String::as_str) != Some("--map") {
        return Ok(None);
    }
    let map = args.get(5).ok_or_else(|| Failure::Message(USAGE.to_string()))?;
    if mode != ParityMode::Off {
        return Err(Failure::NotYet(
            "--combat-run --map: the map route runs in Off only (the C++ has no map load)".to_string(),
        ));
    }
    Ok(Some((map.as_str(), &args[6..])))
}

fn execute(args: &[String], mode: ParityMode) -> Result<(String, bool), Failure> {
    let source = args
        .first()
        .ok_or_else(|| Failure::Message(USAGE.to_string()))?;
    Package::load(Path::new(source)).map_err(|e| Failure::Message(error_text(&e)))?;
    if args.len() < 4 || args[2] != "--cooked" {
        return Err(Failure::Message(USAGE.to_string()));
    }
    if let Some((map, steps)) = map_option(args, mode)? {
        let (world, bridge) = map_route::build(Path::new(&args[3]), map, &mut Vec::new()).map_err(vm_failure)?;
        let bridge: Rc<dyn PlayerBridge> = Rc::new(bridge);
        return ow_combat::run::combat_run_with(world, steps, Some(bridge)).map_err(vm_failure);
    }
    let steps = &args[4..];
    let mut world = world(Path::new(&args[3]), mode).map_err(vm_failure)?;
    let bridge: Option<Rc<dyn PlayerBridge>> = if ow_combat::run::wants_bridge(steps) {
        Some(Rc::new(MissionBridge::new(&mut world).map_err(vm_failure)?))
    } else {
        None
    };
    ow_combat::run::combat_run_with(world, steps, bridge).map_err(vm_failure)
}

pub mod map_route;

#[cfg(test)]
mod tests;
