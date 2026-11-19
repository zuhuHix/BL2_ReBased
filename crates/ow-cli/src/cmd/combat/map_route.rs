//! Owner: lane IMPL-SLICE-MAP-WORLD (lane 4 of docs/rust-port/SLICE_MAPLOAD.md, first PR). The combat run's world
//! on the **map route**: Off only, behind `--map <name>` (`--combat-run`) and `--slice-gate ... --map <name>`. The old
//! route stays the default. AI-assisted (Claude Code). Every rule here is **UNVERIFIED** in the running game.
//!
//! [`build`] follows SLICE_MAPLOAD.md section 3 on one `World`:
//! 1. [`super::world`], the combat run's world, as today.
//! 2. **The globals natives' owners**, before any script: `ow_inventory::globals::register` (`GetWillowGlobals`,
//!    `GetGearboxGlobals`, `GetGlobalsDefinition`) and `ow_behavior::kernel::natives::register_kernel_getter`
//!    (`GetBehaviorKernel`). The adopted bridge binds none of the four (step 8).
//! 3. **The globals** (a harness input, labelled): one `WillowGlobals` from its class defaults, its
//!    `TheGlobalsDefinition` the installed [`GLOBALS_DEFINITION`], recorded with `WorldQuery::set_globals`; then, as
//!    the old route does, the pawn data manager's globals set-up. Where the engine makes its globals object was not
//!    read.
//! 4. **The engine's local player** (a harness input): one [`LOCAL_PLAYER_CLASS`] from class defaults, with no actor,
//!    standing for what engine start-up makes before any map (NATIVE_MAP_LOAD.md section 6.1).
//! 5. **`load_map`**, then **`bring_up_map_for_play`** (lane 2): the configured default game from the install's
//!    `DefaultGame.ini`, no URL options, every actor routed. The bring-up counts its failures and goes on (its own
//!    rule); the count and the game info's own stops are logged as harness lines (SLICE_MAPLOAD.md Q4: whether a link
//!    may pass over an incomplete bring-up is the maintainer's call).
//! 6. **The local-player spawn** (lane 3) with the map's name as the URL text, the empty net id and
//!    `ROLE_SimulatedProxy`. A `Login` that answers None with an error text is this route's failure, the text kept.
//! 7. **Adopt**: [`super::MissionBridge::adopt`] over the same stock mission as the old route.
//!
//! Each step's refusal comes back as it is (a `NotYet` stays one), so the slice gate grades the link's `setup` stop by
//! its type.

use std::path::Path;

use ow_core::{ObjectPath, ParityMode};
use ow_object::ids::PropAddr;
use ow_object::value::Value;
use ow_vm::error::VmError;
use ow_world::actors::ROLE_SIMULATED_PROXY;
use ow_world::world::World;
use ow_world::world::mapload::bringup::{BringUpOptions, RouteMode, bring_up_map_for_play};
use ow_world::world::mapload::game_info::{GameInfoInput, installed_default_game};
use ow_world::world::mapload::player::{NetIdAnswer, PlayActorInput, PlayActorOutcome, make_local_player, spawn_play_actor};

use super::MissionBridge;

/// The installed globals definition (NATIVE_WILLOW_GLOBALS.md section 2: the configured
/// `DefaultGlobalsDefinitionName`), as the pickup world uses it.
pub const GLOBALS_DEFINITION: &str = "GD_Globals.General.Globals";
/// The engine's local player class (the installed `BaseEngine.ini`'s `LocalPlayerClassName`).
pub const LOCAL_PLAYER_CLASS: &str = "Engine.LocalPlayer";

/// A harness line, to the VM's log and to `notes`.
fn note(world: &mut World, notes: &mut Vec<String>, line: String) {
    notes.push(line.clone());
    world.vm.log.push(line);
}

fn script(message: String) -> VmError {
    VmError::Script { function: "the map route".into(), message }
}

/// The world's one globals object (module documentation, step 3).
fn set_up_globals(world: &mut World, notes: &mut Vec<String>) -> Result<(), VmError> {
    note(world, notes, format!(
        "harness input: the world's WillowGlobals from class defaults, TheGlobalsDefinition {GLOBALS_DEFINITION} (the \
         map route's globals set-up, SLICE_MAPLOAD.md section 3 step 2; UNVERIFIED)"
    ));
    let vm = &mut world.vm;
    let class = vm.find_class("WillowGame.WillowGlobals")?;
    let globals = ow_vm::interp::instantiate_from_defaults(vm, class)?;
    let path = ObjectPath::parse(GLOBALS_DEFINITION).map_err(|e| script(format!("{GLOBALS_DEFINITION}: {e}")))?;
    let found = vm.defs.find(&mut vm.space, &path);
    vm.log.extend(vm.defs.take_log());
    let definition = found?.ok_or_else(|| script(format!("{GLOBALS_DEFINITION}: no package holds it")))?;
    let loaded = vm.defs.ensure_loaded(&mut vm.space, definition);
    vm.log.extend(vm.defs.take_log());
    loaded?;
    ow_inventory::globals::globals_schemas(vm, globals, "the map route's globals set-up")?;
    let p = vm
        .space
        .prop(class, "TheGlobalsDefinition")
        .ok_or_else(|| script("WillowGlobals declares no TheGlobalsDefinition".into()))?;
    vm.space.write(globals, PropAddr::scalar(p), Value::Object(Some(definition)))?;
    world.query().try_borrow_mut().map_err(|_| VmError::ServiceBorrowed("WorldQuery"))?.set_globals(globals);
    // Off: the pawn data manager's globals set-up, as the old route runs it after the bridge made the globals
    // (NATIVE_PAWN_DATA.md; its timing is the note's open question 1, UNVERIFIED).
    ow_world::world::pawn_data::set_up(&mut world.vm, globals)?;
    Ok(())
}

/// The combat run's world on the map route, and the adopted bridge (module documentation). `cooked` is the install's
/// `WillowGame/CookedPCConsole`; the install root is two folders up.
///
/// Each harness line goes to the VM's log and to `notes`, so a caller still has them when a step refuses.
pub fn build(cooked: &Path, map: &str, notes: &mut Vec<String>) -> Result<(World, MissionBridge), VmError> {
    let mut world = super::world(cooked, ParityMode::Off)?;
    ow_inventory::globals::register(&mut world.vm)?;
    ow_behavior::kernel::natives::register_kernel_getter(&mut world.vm)?;
    set_up_globals(&mut world, notes)?;
    note(&mut world, notes, format!(
        "harness input: one {LOCAL_PLAYER_CLASS} from class defaults in GamePlayers, no actor (engine start-up's \
         local player, NATIVE_MAP_LOAD.md section 6.1; UNVERIFIED)"
    ));
    let player = make_local_player(&mut world, LOCAL_PLAYER_CLASS)?;
    let name = ow_core::PackageName::parse(map).map_err(|e| script(format!("{map}: {e}")))?;
    ow_world::world::mapload::load_map(&mut world, &name).map_err(|e| match e {
        ow_world::world::mapload::MapError::Vm(v) => v,
        other => script(other.to_string()),
    })?;
    let root = cooked
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| script(format!("{} is not <install>/WillowGame/CookedPCConsole", cooked.display())))?;
    let default_game = installed_default_game(root)?;
    let input = GameInfoInput { options: "", default_game: default_game.as_deref() };
    let up = bring_up_map_for_play(&mut world, &input, BringUpOptions { reset_time: true, mode: RouteMode::EveryActor })?;
    let game = up.game_info.game;
    let on_game: Vec<String> = up
        .bring_up
        .failures
        .iter()
        .filter(|f| f.actor == game)
        .map(|f| format!("{}: {}", f.step, f.text.chars().take(160).collect::<String>()))
        .collect();
    note(&mut world, notes, format!(
        "harness input: map route {map}: game info {}, bring-up failures {} (counted and gone past, SLICE_MAPLOAD.md \
         Q4); on the game info: {}",
        up.game_info.class,
        up.bring_up.failures.len(),
        if on_game.is_empty() { "none".to_string() } else { on_game.join(" | ") }
    ));
    note(&mut world, notes, format!(
        "harness input: the local-player spawn with URL text {map:?}, the empty net id and remote role \
         ROLE_SimulatedProxy (port answers, UNVERIFIED)"
    ));
    let players = [player];
    let given = PlayActorInput {
        url: map,
        game_players: &players,
        net_id: NetIdAnswer::Empty,
        remote_role: Some(ROLE_SIMULATED_PROXY),
    };
    match spawn_play_actor(&mut world, player, &given)? {
        PlayActorOutcome::Spawned(_) => {}
        PlayActorOutcome::LoginFailed { error } => return Err(script(format!("Login failed: {error}"))),
        other => return Err(script(format!("the local-player spawn answered {other:?}"))),
    }
    let bridge = MissionBridge::adopt(&mut world)?;
    Ok((world, bridge))
}
