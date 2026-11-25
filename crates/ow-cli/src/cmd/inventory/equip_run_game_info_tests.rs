//! Tests of the pickup world's game-info stand-in (lane LINK-6-PICKUP-GAMEINFO, `pickup_game_info`) on invented
//! packages (AGENTS.md rule 9; every value invented). AI-assisted (Claude Code).

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_core::ParityMode;
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_package::testkit::script::ScriptToy;
use ow_vm::api::Vm;
use ow_vm::error::VmError;
use ow_world::world::World;

use super::*;

static NEXT: AtomicU32 = AtomicU32::new(0);

/// `Engine.upk`: `Object`, `Actor` (`Role`, `WorldInfo`), `WorldInfo` (`Game`) and `GameInfo`.
fn engine() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class", "ByteProperty", "ObjectProperty"]);
    let object = t.class("Object", 0);
    let actor = t.class("Actor", object);
    t.prop("Byte", actor, "Role", 0, 0);
    t.prop("Object", actor, "WorldInfo", 0, object);
    let info = t.class("WorldInfo", actor);
    t.prop("Object", info, "Game", 0, object);
    t.class("GameInfo", actor);
    t.p.build()
}

/// `WillowGame.upk`: `WillowGameInfo` under `GameInfo`.
fn willow() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class"]);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let game = t.p.add_import("Core", "Class", engine, "GameInfo");
    t.class("WillowGameInfo", game);
    t.p.build()
}

struct Toy {
    world: World,
    dir: PathBuf,
}

impl Drop for Toy {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// A world with the two packages and, when `place` is set, a placed `WorldInfo`.
fn toy(place: bool) -> Toy {
    use ow_balance::types::{
        AwesomeLevel, BalanceSnapshot, ExpLevel, GameStage, LevelCapUpgrades, OpLevel, PlayerCount, Playthrough,
    };
    let dir = std::env::temp_dir().join(format!("ow-cli-game-info-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let (e, w) = (engine(), willow());
    let files: [(&str, &[u8]); 2] = [("Engine.upk", &e), ("WillowGame.upk", &w)];
    let defs = DefStore::from_synthetic(&dir, &files, IdentityPolicy::FirstInScanOrder, ParityMode::Off).unwrap();
    let vm = Vm::new(ObjectSpace::new(), defs, Rc::new(ow_attr::AttributeSystem::new_empty()), ParityMode::Off);
    let snapshot = BalanceSnapshot::new(
        Playthrough::NORMAL,
        OpLevel::NONE,
        PlayerCount::SINGLE,
        ExpLevel::new(8),
        GameStage(8),
        AwesomeLevel::new(0),
        LevelCapUpgrades::NONE,
    )
    .unwrap();
    let mut world = World::new(vm, ow_world::world::BalanceState::new(snapshot), &[]).unwrap();
    if place {
        let class = world.vm.find_class("Engine.WorldInfo").unwrap();
        let info = ow_vm::interp::instantiate_from_defaults(&mut world.vm, class).unwrap();
        world.actors().try_borrow_mut().unwrap().place_world_info(info).unwrap();
    }
    Toy { world, dir }
}

fn get(vm: &Vm, o: ObjRef, name: &str) -> Value {
    let class = vm.space.heap.get(o).unwrap().class;
    let p = vm.space.prop(class, name).unwrap();
    vm.space.read(o, PropAddr::scalar(p)).unwrap().clone()
}

fn info(t: &Toy) -> ObjRef {
    t.world.actors().try_borrow().unwrap().world_info().unwrap()
}

/// A `WorldInfo` with no `Game` gets a class-default `WillowGameInfo` with `Role` authority and its `WorldInfo`, written
/// into the `WorldInfo`'s `Game`, and the harness line is logged once.
#[test]
fn a_world_with_no_game_gets_the_stand_in() {
    let mut t = toy(true);
    let wi = info(&t);
    pickup_game_info(&mut t.world).unwrap();
    let Value::Object(Some(game)) = get(&t.world.vm, wi, "Game") else {
        panic!("the WorldInfo's Game is set")
    };
    assert_eq!(ow_attr::eval::class_path(&t.world.vm.space, game), GAME_INFO_CLASS);
    assert_eq!(get(&t.world.vm, game, "Role"), Value::Byte(ow_world::actors::ROLE_AUTHORITY));
    assert_eq!(get(&t.world.vm, game, "WorldInfo"), Value::Object(Some(wi)));
    assert_eq!(t.world.vm.log.iter().filter(|l| l.as_str() == GAME_INFO_INPUT).count(), 1);
}

/// A `WorldInfo` whose `Game` is set already is left as it is: no object made, no write, no line.
#[test]
fn a_world_with_a_game_is_left_as_it_is() {
    let mut t = toy(true);
    let wi = info(&t);
    pickup_game_info(&mut t.world).unwrap();
    let first = get(&t.world.vm, wi, "Game");
    let writes = t.world.vm.space.heap.committed_writes();
    let objects = t.world.vm.space.heap.handles().len();
    pickup_game_info(&mut t.world).unwrap();
    assert_eq!(get(&t.world.vm, wi, "Game"), first);
    assert_eq!(t.world.vm.space.heap.committed_writes(), writes, "nothing written");
    assert_eq!(t.world.vm.space.heap.handles().len(), objects, "nothing made");
    assert_eq!(t.world.vm.log.iter().filter(|l| l.as_str() == GAME_INFO_INPUT).count(), 1, "logged once");
}

/// A world with no placed `WorldInfo` is an error of the set-up, with nothing made.
#[test]
fn a_world_with_no_world_info_is_an_error() {
    let mut t = toy(false);
    let objects = t.world.vm.space.heap.handles().len();
    match pickup_game_info(&mut t.world) {
        Err(VmError::Script { message, .. }) => assert!(message.contains("no WorldInfo"), "{message}"),
        other => panic!("{other:?}"),
    }
    assert_eq!(t.world.vm.space.heap.handles().len(), objects);
}

/// The harness line reads as one sentence: single spaces between words.
#[test]
fn the_harness_line_has_single_spaces() {
    assert!(!GAME_INFO_INPUT.contains("  "), "{GAME_INFO_INPUT}");
}
