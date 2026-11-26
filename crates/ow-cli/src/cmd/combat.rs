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
        // The engine object's PhysX level (NATIVE_GETPHYSXLEVEL_AND_AFTER.md 2.A; lane SOCKETS-PUBLISH-AND-PHYSX-LEVEL):
        // a harness setting, the shipped default 0. UNVERIFIED: the game's value is the player's own setting.
        ow_world::world::physx_level::set_physx_level(&mut world.vm, ow_world::world::physx_level::HARNESS_PHYSX_LEVEL)?;
        // GetMaterial, SetParent, SetVectorParameterValue and GetInstancedMaterialInstanceConstant (#744), which
        // maya:8's player part reaches; ow-world's placeholder for the last left (lane PLACEHOLDER-SHADOWS).
        // Keep-existing, Off only.
        ow_inventory::equip::material_instances::register(&mut world.vm)?;
        // Actor.AttachComponent (NATIVE_ATTACH_MESH_TO_PICKUP.md part 1, UNVERIFIED; lane
        // FIX-IS-PLAYER-PAWN-AND-UNBOUND-KEYS): the player part's arms attach. The one rule is ow-inventory's,
        // bound as the cast's worlds bind it (Engine's function only, keep-existing); it needs the scene layer the
        // combat run installs. Off only.
        ow_inventory::attach_mesh::bind_engine_attach(&mut world.vm)?;
    }
    Ok(world)
}

/// Maya's skeletal mesh, which the slice harness preloads (lane SLICE-HARNESS-MAYA-MESH, hub ruling): in the game it is
/// loaded with her pawn.
pub(crate) const MAYA_MESH_PATH: &str = "Char_Siren.Mesh.Skel_SirenBody";
/// The body-composition walk's cap on objects visited ([`preload_body_composition`]), as link 8's material walk's.
pub(crate) const BODY_WALK_CAP: usize = 64;
/// The skeleton source's export roots (`;`-separated), else [`PSK_ROOTS_DEFAULT`].
pub(crate) const PSK_ROOTS_VAR: &str = "OPENWILLOW_PSK_EXPORTS";
/// The default export root, under ignored `local/` of the repository ow-cli was built from (review r1 of #592, item
/// 1: not the folder it runs from): UModel `.psk` exports in the file-package layout
/// (`<file package>/SkeletalMesh3/<mesh>.psk`, what `skeletal_source::PskExports` reads).
pub(crate) const PSK_ROOTS_DEFAULT: &str = "local/external/psk";

pub(crate) fn psk_roots() -> Vec<std::path::PathBuf> {
    parse_psk_roots(std::env::var(PSK_ROOTS_VAR).ok().as_deref())
}

/// The repository root: two levels above this crate's manifest (`crates/ow-cli`).
pub(crate) fn repo_root() -> std::path::PathBuf {
    let crate_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    crate_dir.parent().and_then(std::path::Path::parent).unwrap_or(crate_dir).to_path_buf()
}

/// The roots named by `value` (`;`-separated, blanks dropped), else [`PSK_ROOTS_DEFAULT`] under [`repo_root`].
pub(crate) fn parse_psk_roots(value: Option<&str>) -> Vec<std::path::PathBuf> {
    let named: Vec<std::path::PathBuf> =
        value.unwrap_or("").split(';').map(str::trim).filter(|p| !p.is_empty()).map(std::path::PathBuf::from).collect();
    if named.is_empty() { vec![repo_root().join(PSK_ROOTS_DEFAULT)] } else { named }
}

/// Installs `skeletal_source::PskExports` over `roots` as the VM's skeleton source, logged as a harness input
/// (UNVERIFIED: a stand-in for the engine's own mesh data). Each root is logged as an absolute path, with "(missing)"
/// when it is not a folder (review r1 of #592, item 1), so a later "no reference skeleton" is not read as a data gap;
/// the install itself goes ahead either way.
pub(crate) fn install_skeleton_source(vm: &mut Vm, roots: &[std::path::PathBuf]) -> Result<(), VmError> {
    let shown: Vec<String> = roots
        .iter()
        .map(|r| {
            let full = std::path::absolute(r).unwrap_or_else(|_| r.clone());
            let missing = if full.is_dir() { "" } else { " (missing)" };
            format!("{}{missing}", full.display())
        })
        .collect();
    vm.log.push(format!(
        "harness input: the skeleton source over UModel .psk exports in {} (file-package layout; UNVERIFIED \
         stand-in for the engine's mesh data); a mesh with no export is read from its own package export \
         (ow_assets::skeletal_lods, a fitted layout checked by structural oracles, UNVERIFIED)",
        shown.join(";")
    ));
    let source = crate::cmd::skeletal_source::PskExports::new(roots.to_vec());
    ow_world::world::skeletal::SkeletalMeshes::install(vm, Box::new(source))
}

/// The aim-offset profiles reader (lane IMPL-AIMOFFSET-PROFILES): `ow_assets::aim_profiles` as ow-world's
/// [`ow_world::world::aim_profiles::ProfileSource`], which the crate edges keep ow-world from calling directly.
struct AssetProfiles;

impl ow_world::world::aim_profiles::ProfileSource for AssetProfiles {
    fn profiles_in(
        &self,
        package: &ow_package::package::Package,
        index: i32,
        property_offset: usize,
    ) -> Result<ow_world::world::aim_profiles::SourceProfiles, String> {
        let read = ow_assets::aim_profiles::read_export(package, index, property_offset).map_err(|e| e.to_string())?;
        let profiles = read.profiles.map(|p| {
            p.into_iter()
                .map(|x| ow_world::world::aim_profiles::SourceProfile {
                    name: x.profile_name,
                    bones: x.components.into_iter().map(|c| c.bone_name).collect(),
                })
                .collect()
        });
        Ok((profiles, read.tags_end))
    }
}

/// Installs [`AssetProfiles`] as the VM's profiles reader, logged as a harness input (a fitted layout checked by
/// structural oracles, UNVERIFIED; decision 2026-11-25-72).
pub(crate) fn install_profiles_reader(vm: &mut Vm) -> Result<(), VmError> {
    vm.log.push(
        "harness input: the aim-offset profiles reader over the node's own export (ow_assets::aim_profiles, a fitted \
         layout checked by structural oracles, UNVERIFIED)"
            .to_string(),
    );
    ow_world::world::aim_profiles::AimProfiles::install(vm, Box::new(AssetProfiles))
}

/// Loads the skeletal mesh at `path` (`what` names it in the log) through the store, logged as a harness input
/// (UNVERIFIED stand-in for the streaming load of the pawn's package). A path no package holds is logged and skipped; a
/// load that fails is the step's error.
pub(crate) fn preload_mesh(vm: &mut Vm, path: &str, what: &str) -> Result<(), VmError> {
    let parsed = ObjectPath::parse(path)
        .map_err(|e| VmError::Script { function: "combat-run".into(), message: e.to_string() })?;
    vm.log.push(format!("harness input: preload {path} ({what}; UNVERIFIED stand-in for the streaming load)"));
    let found = vm.defs.find(&mut vm.space, &parsed);
    vm.log.extend(vm.defs.take_log());
    let mesh = match found {
        Ok(Some(m)) => m,
        Ok(None) => {
            vm.log.push(format!("harness input: preload {path}: found nothing"));
            return Ok(());
        }
        Err(e) => {
            vm.log.push(format!("harness input: preload {path}: failed ({e})"));
            return Err(VmError::Def(e));
        }
    };
    let loaded = vm.defs.ensure_loaded(&mut vm.space, mesh);
    vm.log.extend(vm.defs.take_log());
    match loaded {
        Ok(()) => {
            vm.log.push(format!("harness input: preload {path}: loaded"));
            Ok(())
        }
        Err(e) => {
            vm.log.push(format!("harness input: preload {path}: failed ({e})"));
            Err(VmError::Def(e))
        }
    }
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
    let package_of = file_package;
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

/// The log line of one anim-set preload (lane PRELOAD-MAYA-ANIMSETS).
pub(crate) fn anim_set_preload_line(path: &str) -> String {
    format!("harness input: set-up preload: {path} (UNVERIFIED: the game streams it with the character)")
}

/// The anim sets of Maya's mesh component and of her body class's weapon holds, and their sequences (lane
/// PRELOAD-MAYA-ANIMSETS, the hub's rulings on the lookups of NATIVE_UPDATEANIMATIONS_TREE_TICKS.md and
/// NATIVE_ANIM_NODE_INIT.md): in the game they are resident when she spawns, because they stream with her character.
/// The port loads **exactly** these sets, in this order, each once, then each set's `Sequences`, through the store,
/// each logged as a harness input ([`anim_set_preload_line`]), an UNVERIFIED stand-in for that streaming load:
/// 1. the sets the archetype's `Mesh` component lists in `AnimSets`;
/// 2. the weapon holds' sets ([`hold_anim_sets`]): what `BodyClassDefinition.GetWeaponHoldAnimSets` can return for her
///    holds, which `WillowPawn.UpdateAnimSets` appends to that list.
///
/// Nothing else is loaded: not a hold, a body class or a sequence's own references. A field the object does not
/// have, a None entry, or an object that is not resident is skipped and logged; a load that fails is the step's error.
/// The function checks no parity mode: it runs wherever the `maya` step's harness inputs run.
pub(crate) fn preload_mesh_anim_sets(vm: &mut Vm, archetype: ObjRef) -> Result<(), VmError> {
    let mut sets: Vec<ObjRef> = Vec::new();
    match object_slot(vm, archetype, "Mesh")? {
        None => vm.log.push("harness input: set-up preload: no Mesh component on the archetype".to_string()),
        Some(mesh) if !present(vm, mesh)? => vm.log.push(format!(
            "harness input: set-up preload: the Mesh component {} is not resident; skipped",
            object_path(vm, mesh)
        )),
        Some(mesh) => sets.extend(objects_of(vm, mesh, "AnimSets")?),
    }
    match object_slot(vm, archetype, "BodyClass")? {
        None => vm.log.push("harness input: set-up preload: no BodyClass on the archetype, so no weapon-hold sets".to_string()),
        Some(body) => {
            for set in hold_anim_sets(vm, body)? {
                if !sets.contains(&set) {
                    sets.push(set);
                }
            }
        }
    }
    for set in sets {
        preload_one(vm, set)?;
        // A set the store keeps as a stand-in (or whose load failed earlier) has no Sequences to read (review of #754).
        if !present(vm, set)? {
            vm.log.push(format!("harness input: set-up preload: the anim set {} is not resident; skipped", object_path(vm, set)));
            continue;
        }
        for sequence in objects_of(vm, set, "Sequences")? {
            preload_one(vm, sequence)?;
        }
    }
    Ok(())
}

/// Whether `o` can be read: neither a handle the store has not loaded (or whose load failed) nor a stand-in with fewer
/// slots than its class lays out (`ow_world::world::weapon_hold`'s residency test; review of #754: a stand-in would
/// fail the read with `SlotMissing`).
fn present(vm: &Vm, o: ObjRef) -> Result<bool, VmError> {
    let x = vm.space.heap.get(o)?;
    let slotless = vm.space.classes.get_layout(x.class).is_some_and(|l| x.slots.len() < l.slot_count() as usize);
    Ok(!(vm.defs.is_unloaded(&vm.space, o) || vm.defs.is_failed(&vm.space, o) || slotless))
}

/// The anim sets `BodyClassDefinition.GetWeaponHoldAnimSets` can return for the body class `body`, read only (the script
/// read of `GetWeaponHoldAnimSets` and `BodyWeaponHoldDefinition.GetAnimSets`): for `body`, then each class up its
/// `WeaponActionParentClass` chain, every hold in `WeaponHoldDefs` and its `DefaultWeaponHoldDef`; for each hold, up
/// its `ParentDefinition` chain from the farthest, the sets its `AnimSetList` names. In that order, each once. A body
/// class or hold that is not resident (a handle not loaded, a failed load, or a stand-in without its slots) is skipped
/// and logged (holds are not preloaded); a chain that comes back to a visited object stops there, logged. The
/// first-person holds (`GetFirstPersonWeaponHoldAnimSets`, the arms) are not read.
///
/// The class order (the body class first, then up its parents) is this mirror's own: the native's recursion takes the
/// parent class first. The set of objects loaded is the same, only their order differs (review of #754).
pub(crate) fn hold_anim_sets(vm: &mut Vm, body: ObjRef) -> Result<Vec<ObjRef>, VmError> {
    let mut out: Vec<ObjRef> = Vec::new();
    let mut classes: Vec<ObjRef> = Vec::new();
    let mut cursor = Some(body);
    while let Some(class) = cursor {
        if classes.contains(&class) {
            vm.log.push(format!(
                "harness input: set-up preload: the body-class chain comes back to {}; stopped",
                object_path(vm, class)
            ));
            break;
        }
        classes.push(class);
        if !present(vm, class)? {
            vm.log.push(format!("harness input: set-up preload: the body class {} is not resident; skipped", object_path(vm, class)));
            break;
        }
        let mut holds = objects_of(vm, class, "WeaponHoldDefs")?;
        if let Some(d) = object_slot(vm, class, "DefaultWeaponHoldDef")?
            && !holds.contains(&d)
        {
            holds.push(d);
        }
        for hold in holds {
            // The hold's chain, farthest parent first, as GetAnimSets appends the parent's sets before its own.
            let mut chain: Vec<ObjRef> = Vec::new();
            let mut h = Some(hold);
            while let Some(x) = h {
                if chain.contains(&x) {
                    vm.log.push(format!(
                        "harness input: set-up preload: the weapon-hold chain comes back to {}; stopped",
                        object_path(vm, x)
                    ));
                    break;
                }
                if !present(vm, x)? {
                    vm.log.push(format!("harness input: set-up preload: the weapon hold {} is not resident; skipped", object_path(vm, x)));
                    break;
                }
                chain.push(x);
                h = object_slot(vm, x, "ParentDefinition")?;
            }
            for x in chain.into_iter().rev() {
                for set in objects_of(vm, x, "AnimSetList")? {
                    if !out.contains(&set) {
                        out.push(set);
                    }
                }
            }
        }
        cursor = object_slot(vm, class, "WeaponActionParentClass")?;
    }
    Ok(out)
}

/// The objects array property `name` of the resident `o` holds, None entries left out (empty when not declared).
fn objects_of(vm: &Vm, o: ObjRef, name: &str) -> Result<Vec<ObjRef>, VmError> {
    let class = vm.space.heap.get(o)?.class;
    let Some(p) = vm.space.prop(class, name) else {
        return Ok(Vec::new());
    };
    let v = vm.space.read(o, ow_object::ids::PropAddr::scalar(p))?;
    Ok(v.elements()
        .unwrap_or(&[])
        .iter()
        .filter_map(|e| match e {
            Value::Object(Some(x)) => Some(*x),
            _ => None,
        })
        .collect())
}

/// Loads `o` through the store when it is a handle not loaded yet, logged before the load ([`anim_set_preload_line`]).
fn preload_one(vm: &mut Vm, o: ObjRef) -> Result<(), VmError> {
    if !vm.defs.is_unloaded(&vm.space, o) {
        return Ok(());
    }
    let path = vm.defs.handle_path(o).unwrap_or_else(|| object_path(vm, o));
    vm.log.push(anim_set_preload_line(&path));
    let loaded = vm.defs.ensure_loaded(&mut vm.space, o);
    vm.log.extend(vm.defs.take_log());
    if let Err(e) = loaded {
        vm.log.push(format!("harness input: set-up preload: {path}: failed ({e})"));
        return Err(VmError::from(e));
    }
    Ok(())
}

/// The package file whose export `o` is (or will be, for a store handle), from its heap origin.
pub(crate) fn file_package(vm: &Vm, o: ObjRef) -> Option<ow_package::PackageId> {
    use ow_object::heap::Origin;
    match vm.space.heap.get(o).map(|x| x.origin) {
        Ok(Origin::Export { package, .. } | Origin::Resource { package, .. }) => Some(package),
        _ => None,
    }
}

/// `o`'s object path, for the log.
fn object_path(vm: &Vm, o: ObjRef) -> String {
    use ow_object::heap::Origin;
    match vm.space.heap.get(o).map(|x| x.origin) {
        Ok(Origin::Export { package, index } | Origin::Resource { package, index }) => {
            ow_defs::identity::full_path(vm.defs.packages().get(package), index).unwrap_or_else(|_| format!("{o:?}"))
        }
        _ => format!("{o:?}"),
    }
}

/// Object property `name` of `o`, when its class declares it and it holds an object.
fn object_slot(vm: &Vm, o: ObjRef, name: &str) -> Result<Option<ObjRef>, VmError> {
    let class = vm.space.heap.get(o)?.class;
    let Some(p) = vm.space.prop(class, name) else {
        return Ok(None);
    };
    Ok(match vm.space.read(o, ow_object::ids::PropAddr::scalar(p))? {
        Value::Object(r) => *r,
        _ => None,
    })
}

/// Struct field `name` of `v`, when `v` is a struct that has it.
fn struct_field(vm: &Vm, v: &Value, name: &str) -> Option<Value> {
    let s = v.as_struct()?;
    let id = vm.space.names.find(name)?;
    let (index, _) = vm.space.classes.struct_layout(s.ty)?.field(id)?;
    s.fields.get(usize::try_from(index).ok()?).cloned()
}

/// The body class's attachment templates in list order (`BodyComposition.Attachments[i].Data.ComponentData.Component`,
/// None entries left out), or `None` when the body class is not shaped so.
fn attachment_templates(vm: &Vm, body: ObjRef) -> Result<Option<Vec<ObjRef>>, VmError> {
    let class = vm.space.heap.get(body)?.class;
    let Some(p) = vm.space.prop(class, "BodyComposition") else {
        return Ok(None);
    };
    let composition = vm.space.read(body, ow_object::ids::PropAddr::scalar(p))?.clone();
    let Some(list) = struct_field(vm, &composition, "Attachments") else {
        return Ok(None);
    };
    let Some(items) = list.elements() else {
        return Ok(None);
    };
    let mut out = Vec::new();
    for item in items {
        let component = struct_field(vm, item, "Data")
            .and_then(|d| struct_field(vm, &d, "ComponentData"))
            .and_then(|c| struct_field(vm, &c, "Component"));
        match component {
            Some(Value::Object(Some(t))) => {
                if !out.contains(&t) {
                    out.push(t);
                }
            }
            Some(Value::Object(None)) => {}
            _ => return Ok(None),
        }
    }
    Ok(Some(out))
}

/// The body-composition set-up walk (lane LINK1-HEAD-MESH, the hub's ruling on #722's next stop; the pattern of link
/// 8's material walk, decision 2026-11-25-47). From the archetype's `BodyClass`: the component templates its body
/// composition makes (the attachment list), and each template's `SkeletalMesh`. An object is loaded through the store
/// **only if it is an export of the archetype's own package file**, the pawn's streaming package; one already resident
/// is visited with no load. Anything else stops its branch with a logged line and is left for the native to refuse.
/// Each object is visited once, up to [`BODY_WALK_CAP`] (logged if hit). Each load is logged before and after; a failed
/// load is the step's error, kept. An UNVERIFIED stand-in, as the archetype's preload: the streaming load makes the
/// package resident, and which exports it loads is not read. No native refusal changes.
pub(crate) fn preload_body_composition(vm: &mut Vm, archetype: ObjRef) -> Result<(), VmError> {
    let Some(home) = file_package(vm, archetype) else {
        vm.log.push("harness input: body-composition walk skipped (the archetype is not a package export)".to_string());
        return Ok(());
    };
    let Some(body) = object_slot(vm, archetype, "BodyClass")? else {
        vm.log.push("harness input: body-composition walk skipped (the archetype has no BodyClass)".to_string());
        return Ok(());
    };
    let package = vm.defs.packages().get(home).name.clone();
    vm.log.push(format!(
        "harness input: body-composition walk from {}: its templates and their meshes, loading only exports of \
         {package} (UNVERIFIED stand-in for the streaming load)",
        object_path(vm, body)
    ));
    let mut walk = BodyWalk { home, package, visited: Vec::new(), loaded: 0 };
    if walk.visit(vm, body)? {
        match attachment_templates(vm, body)? {
            None => vm.log.push("harness input: body-composition walk: the body class has no attachment list".to_string()),
            Some(templates) => walk_templates(vm, &mut walk, &templates)?,
        }
    }
    walk_weapon_holds(vm, &mut walk, body)?;
    vm.log.push(format!("harness input: body-composition walk: {} loaded, {} visited", walk.loaded, walk.visited.len()));
    Ok(())
}

/// The walk's state: the home package file, the objects visited and the loads made.
pub(crate) struct BodyWalk {
    pub(crate) home: ow_package::PackageId,
    pub(crate) package: String,
    pub(crate) visited: Vec<ObjRef>,
    pub(crate) loaded: usize,
}

impl BodyWalk {
    /// Visits `o` once: `Ok(true)` when it is resident afterwards (loaded here or before), so its branch goes on.
    fn visit(&mut self, vm: &mut Vm, o: ObjRef) -> Result<bool, VmError> {
        if self.visited.contains(&o) {
            return Ok(!vm.defs.is_unloaded(&vm.space, o));
        }
        if self.visited.len() >= BODY_WALK_CAP {
            return Ok(false);
        }
        self.visited.push(o);
        let path = object_path(vm, o);
        if file_package(vm, o) != Some(self.home) {
            let at = file_package(vm, o).map_or_else(|| "no package".to_string(), |p| vm.defs.packages().get(p).name.clone());
            // The home file's own copy of the path, if it holds one (the hub's ruling: the game, which loads only the
            // pawn's package, has that copy; `DefStore::load_export_of_file`, an UNVERIFIED store rule).
            vm.log.push(format!(
                "harness input: body-composition walk: the store holds {path} from {at}; asking {} for its own copy \
                 (UNVERIFIED store rule)",
                self.package
            ));
            let copy = vm.defs.load_export_of_file(&mut vm.space, o, self.home);
            vm.log.extend(vm.defs.take_log());
            return match copy {
                Ok(ow_defs::store::FileCopy::Loaded) => {
                    vm.log.push(format!("harness input: preload {path}: loaded {}'s own copy", self.package));
                    self.loaded += 1;
                    Ok(true)
                }
                Ok(ow_defs::store::FileCopy::NotInFile) => {
                    vm.log.push(format!(
                        "harness input: body-composition walk: {path} is not an export of {} ({at}); its branch stops",
                        self.package
                    ));
                    Ok(false)
                }
                Err(e) => {
                    vm.log.push(format!("harness input: preload {path}: failed ({e})"));
                    Err(VmError::from(e))
                }
            };
        }
        if !vm.defs.is_unloaded(&vm.space, o) {
            return Ok(true);
        }
        vm.log.push(format!("harness input: preload {path} (an export of {}; UNVERIFIED stand-in)", self.package));
        let loaded = vm.defs.ensure_loaded(&mut vm.space, o);
        vm.log.extend(vm.defs.take_log());
        match loaded {
            Ok(()) => {
                vm.log.push(format!("harness input: preload {path}: loaded"));
                self.loaded += 1;
                Ok(true)
            }
            Err(e) => {
                vm.log.push(format!("harness input: preload {path}: failed ({e})"));
                Err(VmError::from(e))
            }
        }
    }
}

/// The weapon-hold part of the walk (lane IMPL-GETWEAPONHOLDDEF, the hub's ruling: `GetWeaponHoldDef` reads them and
/// loads nothing, NATIVE_GET_WEAPON_HOLD_DEF.md section 3). From `body`, and then along each class's
/// `WeaponActionParentClass`: the body class itself, its `WeaponHoldDefs` entries in order and its
/// `DefaultWeaponHoldDef`, each by [`BodyWalk::visit`], so the same rule holds: only exports of the pawn's package
/// file (or that file's own copy of the path) are loaded, and the rest stays for the native to refuse. A class whose
/// visit leaves it unloaded stops its branch. UNVERIFIED stand-in for the streaming load, as the rest of the walk.
pub(crate) fn walk_weapon_holds(vm: &mut Vm, walk: &mut BodyWalk, body: ObjRef) -> Result<(), VmError> {
    let mut bodies = vec![body];
    let mut next = 0;
    while let Some(&b) = bodies.get(next) {
        next += 1;
        if walk.visited.len() >= BODY_WALK_CAP {
            vm.log.push(format!("harness input: body-composition walk: stopped at the cap of {BODY_WALK_CAP} objects"));
            break;
        }
        if !walk.visit(vm, b)? {
            continue;
        }
        let class = vm.space.heap.get(b)?.class;
        let holds: Vec<ObjRef> = match vm.space.prop(class, "WeaponHoldDefs") {
            Some(p) => vm
                .space
                .read(b, ow_object::ids::PropAddr::scalar(p))?
                .elements()
                .map(|items| items.iter().filter_map(|v| if let Value::Object(Some(o)) = v { Some(*o) } else { None }).collect())
                .unwrap_or_default(),
            None => Vec::new(),
        };
        for h in holds.into_iter().chain(object_slot(vm, b, "DefaultWeaponHoldDef")?) {
            walk.visit(vm, h)?;
        }
        if let Some(parent) = object_slot(vm, b, "WeaponActionParentClass")?
            && !bodies.contains(&parent)
        {
            bodies.push(parent);
        }
    }
    Ok(())
}

/// The walk over `templates` and each one's `SkeletalMesh` (module function so the tests can drive it).
pub(crate) fn walk_templates(vm: &mut Vm, walk: &mut BodyWalk, templates: &[ObjRef]) -> Result<(), VmError> {
    for &t in templates {
        if walk.visited.len() >= BODY_WALK_CAP {
            vm.log.push(format!("harness input: body-composition walk: stopped at the cap of {BODY_WALK_CAP} objects"));
            break;
        }
        if walk.visit(vm, t)?
            && let Some(mesh) = object_slot(vm, t, "SkeletalMesh")?
        {
            walk.visit(vm, mesh)?;
        }
    }
    Ok(())
}

/// The six Off harness inputs of the `maya` step, in order, each logged with its outcome (rule 7; decisions
/// 2026-11-19-41 and 2026-11-19-46, lane IMPL-MESH-REST for the tree, and lane SLICE-HARNESS-MAYA-MESH for steps 3
/// and 4, lane IMPL-BODYCOMP-SOCKET-BONE-TABLES for step 5). Outside Off, or for a controller that already has a `PlayerClass`, nothing.
/// 1. **Preload:** `class`'s own `PawnArchetypePath` is loaded through the definition store, an UNVERIFIED stand-in for
///    the streaming load NATIVE_PLAYER_CLASS.md does not settle. A missing, non-string or unparsable path is skipped
///    (and logged), and a load that finds nothing is logged; the class select then runs either way. After the
///    archetype's same-package references, the body-composition walk (lane LINK1-HEAD-MESH): its body class's component
///    templates and their meshes, exports of the pawn's package file only ([`preload_body_composition`]), the same
///    UNVERIFIED stand-in.
/// 1a. **Owner-test set-up** (lane ROUTE-A, NATIVE_SPECIALMOVE_OWNER.md route (a)): `SpecialMoveInterface` loaded and
///    the interface entries on the archetype's class chain resolved ([`owner_setup`]), UNVERIFIED stand-ins for the
///    game having them loaded and resolved.
/// 2. **Tree preload:** Maya's animation tree and the nodes and controls it reaches ([`preload_player_tree`]).
/// 3. **Skeleton source:** ow-cli's `.psk` source installed as the skeletal mesh data layer's source, once per VM
///    ([`install_skeleton_source`]), an UNVERIFIED stand-in for the engine's own mesh data.
/// 4. **Mesh preload:** Maya's skeletal mesh [`MAYA_MESH_PATH`] loaded through the definition store ([`preload_mesh`]),
///    an UNVERIFIED stand-in for the streaming load that brings it with her pawn.
/// 5. **Socket preload:** the mesh's unloaded socket objects loaded through the store
///    (`crate::cmd::mesh_sockets::preload_mesh_sockets`), an UNVERIFIED stand-in for the same streaming load (hub
///    ruling).
/// 6. **Class select:** the controller's installed `ServerPlayerSelectClass(<Maya's class path>)`.
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
    // The pawn archetype's class, for the owner test's set-up below (lane ROUTE-A).
    let mut pawn_class = None;
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
                                    pawn_class = Some(vm.space.heap.get(archetype)?.class);
                                    load_same_package_references(vm, archetype)?;
                                    // The body composition's templates and their meshes (lane LINK1-HEAD-MESH).
                                    preload_body_composition(vm, archetype)?;
                                    // Her mesh component's anim sets and their sequences (lane
                                    // PRELOAD-MAYA-ANIMSETS, hub ruling).
                                    preload_mesh_anim_sets(vm, archetype)?;
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
    owner_setup::special_move_owner_setup(vm, pawn_class)?;
    preload_player_tree(vm)?;
    // Maya's mesh data (lane SLICE-HARNESS-MAYA-MESH, hub ruling), before the class select spawns her pawn: the
    // skeleton source (once per VM), then the mesh. Both are logged inside the `maya` step, so the slice gate's harness list for
    // the link shows them.
    if vm.service::<ow_world::world::skeletal::SkeletalMeshes>().is_none() {
        install_skeleton_source(vm, &psk_roots())?;
    }
    if vm.service::<ow_world::world::aim_profiles::AimProfiles>().is_none() {
        install_profiles_reader(vm)?;
    }
    preload_mesh(vm, MAYA_MESH_PATH, "Maya's mesh")?;
    crate::cmd::mesh_sockets::preload_mesh_sockets(vm, MAYA_MESH_PATH)?;
    gear_likeness_setup(vm)?;
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

/// The made `Mesh`'s class, whose interface entries hook 1 reads (lane HARNESS-GEARLIKENESS-SETUP).
pub(crate) const MAYA_MESH_CLASS: &str = "Engine.SkeletalMeshComponent";

/// The body composition's hook 1 set-up (lane HARNESS-GEARLIKENESS-SETUP, the hub's call on #657's probes), inside the
/// `maya` step so the link's harness list shows it: the gear-likeness interface is loaded
/// ([`crate::cmd::slice_gate::preload_class`], an UNVERIFIED stand-in for the game having it loaded), then the
/// interface entries on the made `Mesh`'s class chain are resolved ([`crate::cmd::slice_gate::resolve_interface`]).
/// The rule itself (ow_world::world::render_hooks) is unchanged and still loads nothing.
fn gear_likeness_setup(vm: &mut Vm) -> Result<(), VmError> {
    let (line, outcome) = crate::cmd::slice_gate::preload_class(vm, ow_world::world::render_hooks::GEAR_LIKENESS);
    vm.log.push(line);
    outcome?;
    let (line, outcome) =
        crate::cmd::slice_gate::resolve_interface(vm, MAYA_MESH_CLASS, ow_world::world::render_hooks::GEAR_LIKENESS);
    vm.log.push(line);
    outcome
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
pub mod owner_setup;

/// A shadow the slice worlds have by design (lane PLACEHOLDER-SHADOWS): `ResolveContext` and `AddAttributeModifier` are
/// one wrapper each (`ow_inventory::equip::natives`), which the equip pack binds VM-wide and the action skill pack
/// (`ow-skills/src/action_skill.rs`) binds through `ow_combat::hit::bind_fn`, keep-existing in either order. The
/// helper is `#[track_caller]`, so the audit names the action skill file, not `hit.rs` (review of #748). The pack that
/// gives up the key would have bound the same function, so nothing is lost. Exactly those two keys, between exactly
/// those two files.
///
/// **A third, seen once the helper is `#[track_caller]`** (lane SHADOW-AUDIT-FOLLOWUPS): `Actor.Trace`, which
/// `ow-combat/src/go_down.rs` and `ow-skills/src/action_skill.rs` both bind through the helper to the one
/// implementation, `ow_world::world::actor_trace::trace`. Before, both sides were recorded as `hit.rs`, the blind spot
/// decision 2026-11-26-07 stated. Exactly that key (by its function's native key) between exactly those two files.
#[cfg(test)]
pub(crate) fn shared_wrapper_shadow(s: &ow_vm::api::NativeShadow) -> bool {
    use ow_inventory::equip::natives::{ADD_MODIFIER, RESOLVE_CONTEXT};
    let file = |f: &str, tail: &str| f.replace('\\', "/").ends_with(tail);
    let wrappers = (s.key == RESOLVE_CONTEXT || s.key == ADD_MODIFIER)
        && file(s.bound_by, "ow-inventory/src/equip/natives.rs")
        && file(s.checked_by, "ow-skills/src/action_skill.rs");
    let trace = s.key == ACTOR_TRACE_KEY
        && file(s.bound_by, "ow-combat/src/go_down.rs")
        && file(s.checked_by, "ow-skills/src/action_skill.rs");
    wrappers || trace
}

/// `Actor.Trace`'s native key, as the shadow audit records it.
#[cfg(test)]
pub(crate) const ACTOR_TRACE_KEY: &str = "Actor.Trace(vector,vector,vector,vector,bool,vector,tracehitinfo,int,bool,object)";

#[cfg(test)]
mod tests;
