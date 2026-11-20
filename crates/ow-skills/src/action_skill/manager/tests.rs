//! Tests of the skill manager natives on two invented packages (AGENTS.md rule 9): `Engine` (the actor fields the
//! world's spawn reads, `WorldInfo`) and `WillowGame` (`WillowGameInfo`, `SkillEffectManager`, `Skill`, a
//! controller), the second importing the first. The natives are reached through the VM by their function paths, as
//! the installed script reaches them.

use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_balance::types::{
    AwesomeLevel, BalanceSnapshot, ExpLevel, GameStage, LevelCapUpgrades, OpLevel, PlayerCount,
    Playthrough,
};
use ow_combat::hit::access::get;
use ow_core::notyet::Missing;
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_package::testkit::reflect::{toy_array_of, toy_struct};
use ow_package::testkit::script::{FunctionSpec, Param, ScriptToy};
use ow_vm::api::StubPolicy;
use ow_world::world::{BalanceState, World};

use super::*;

const NATIVE: u32 = 0x400 | 0x20000;
static NEXT: AtomicU32 = AtomicU32::new(0);

const IMPORTS: [&str; 10] = [
    "Class",
    "Function",
    "ScriptStruct",
    "IntProperty",
    "FloatProperty",
    "BoolProperty",
    "ByteProperty",
    "ObjectProperty",
    "ArrayProperty",
    "StructProperty",
];

/// `Engine`: `Object` with `Vector` and `Rotator`, `Actor` with the fields the spawn reads and writes, `WorldInfo`.
fn engine() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let object = t.class("Object", 0);
    let vector = toy_struct(&mut t, object, "Vector", None);
    for f in ["X", "Y", "Z"] {
        t.prop("Float", vector, f, 0, 0);
    }
    let rotator = toy_struct(&mut t, object, "Rotator", None);
    for f in ["Pitch", "Yaw", "Roll"] {
        t.prop("Int", rotator, f, 0, 0);
    }
    let actor = t.class("Actor", object);
    for b in [
        "bStatic",
        "bNoDelete",
        "bDeleteMe",
        "bPendingDelete",
        "bCollideWorld",
        "bCollideWhenPlacing",
        "bCollideActors",
        "bShouldBaseAtStartup",
    ] {
        t.prop("Bool", actor, b, 0, 0);
    }
    t.prop("Byte", actor, "Role", 0, 0);
    t.prop("Float", actor, "CreationTime", 0, 0);
    for o in ["Owner", "Base", "Instigator", "WorldInfo"] {
        t.prop("Object", actor, o, 0, actor);
    }
    t.prop("Struct", actor, "Location", 0, vector);
    // The spawn's component update (lane IMPL-SPAWN-COMPONENT-ATTACH) reads both arrays.
    for a in ["Components", "AllComponents"] {
        ow_package::testkit::reflect::toy_array_of(&mut t, actor, a, "Object", object);
    }
    t.prop("Struct", actor, "Rotation", 0, rotator);
    let world = t.class("WorldInfo", actor);
    t.prop("Float", world, "TimeSeconds", 0, 0);
    t.prop("Byte", world, "NetMode", 0, 0);
    t.build()
}

/// `WillowGame`: `WillowGameInfo` (`GlobalSkillManager`, native `GetSkillManager()`), `SkillEffectManager`
/// (`ActiveSkills`, native `DeactivateAllSkillsForInstigator(Controller)`), `Skill` (`SkillInstigator`) and
/// `ToyController`.
fn willow_game() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let object = t.p.add_import("Core", "Class", engine, "Object");
    let actor = t.p.add_import("Core", "Class", engine, "Actor");
    let controller = t.class("ToyController", actor);
    let skill = t.class("Skill", object);
    t.prop("Object", skill, "SkillInstigator", 0, controller);
    let manager = t.class("SkillEffectManager", actor);
    toy_array_of(&mut t, manager, "ActiveSkills", "Object", skill);
    t.function(
        manager,
        "DeactivateAllSkillsForInstigator",
        &[Param {
            type_ref: controller,
            ..Param::new("Object", "Instigator")
        }],
        None,
        &[],
        &FunctionSpec::new(NATIVE, "DeactivateAllSkillsForInstigator"),
        |_| None,
    );
    let game = t.class("WillowGameInfo", actor);
    t.prop("Object", game, "GlobalSkillManager", 0, manager);
    t.function(
        game,
        "GetSkillManager",
        &[],
        Some(("Object", manager)),
        &[],
        &FunctionSpec::new(NATIVE, "GetSkillManager"),
        |_| None,
    );
    t.build()
}

struct Fx {
    world: World,
    game: ObjRef,
    dir: std::path::PathBuf,
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// A world over the two packages in `mode`, with the pack registered, a game info of `Role` `role`, and (with
/// `world_info`) a placed `WorldInfo`.
fn fx(mode: ParityMode, role: u8, world_info: bool) -> Fx {
    let dir = std::env::temp_dir().join(format!(
        "ow-skill-manager-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let (e, w) = (engine(), willow_game());
    let defs = DefStore::from_synthetic(
        &dir,
        &[("Engine.upk", e.as_slice()), ("WillowGame.upk", w.as_slice())],
        IdentityPolicy::FirstInScanOrder,
        mode,
    )
    .unwrap();
    let vm = Vm::new(
        ObjectSpace::new(),
        defs,
        Rc::new(AttributeSystem::new_empty()),
        mode,
    )
    .with_stub_policy(StubPolicy::refuse_all());
    let snapshot = BalanceSnapshot::new(
        Playthrough::NORMAL,
        OpLevel::NONE,
        PlayerCount::SINGLE,
        ExpLevel::new(1),
        GameStage(1),
        AwesomeLevel::new(0),
        LevelCapUpgrades::ALL,
    )
    .unwrap();
    let mut world = World::new(vm, BalanceState::new(snapshot), &[&Pack]).unwrap();
    if world_info {
        let wi = make(&mut world.vm, "Engine.WorldInfo");
        world.actors().borrow_mut().place_world_info(wi).unwrap();
    }
    let game = make(&mut world.vm, "WillowGame.WillowGameInfo");
    set_required(&mut world.vm, game, "Role", Value::Byte(role), "test").unwrap();
    // An invented game info location, so the spawn's explicit (0, 0, 0) is told apart from the spawner's.
    let mut at = get(&mut world.vm, game, "Location").unwrap().unwrap();
    assert!(ow_combat::hit::access::set_field(&world.vm, &mut at, "X", Value::Float(12.5)));
    set_required(&mut world.vm, game, "Location", at, "test").unwrap();
    Fx { world, game, dir }
}

fn make(vm: &mut Vm, class: &str) -> ObjRef {
    let c = vm.find_class(class).unwrap();
    ow_vm::interp::instantiate_from_defaults(vm, c).unwrap()
}

impl Fx {
    fn vm(&mut self) -> &mut Vm {
        &mut self.world.vm
    }

    fn call(&mut self, path: &str, this: ObjRef, args: Vec<Value>) -> Result<Value, VmError> {
        let f = self.vm().find_function(path)?;
        Ok(self.vm().call(f, Some(this), args)?.ret)
    }

    fn get_manager(&mut self) -> Result<Option<ObjRef>, VmError> {
        let game = self.game;
        match self.call(NATIVE_PATHS[0], game, Vec::new())? {
            Value::Object(o) => Ok(o),
            other => panic!("GetSkillManager answered {other:?}"),
        }
    }

    fn value(&mut self, o: ObjRef, name: &str) -> Value {
        get(self.vm(), o, name).unwrap().unwrap()
    }

    fn actor_count(&mut self) -> usize {
        self.world.actors().borrow().list().len()
    }
}

fn not_yet(r: Result<impl std::fmt::Debug, VmError>) -> NotYet {
    match r {
        Err(VmError::NotYet(n)) => n,
        other => panic!("expected NotYet, got {other:?}"),
    }
}

#[test]
fn the_first_call_spawns_the_manager_owned_by_the_game_info_and_later_calls_return_it() {
    let mut x = fx(ParityMode::Off, 3, true);
    let before = x.actor_count();
    let m = x.get_manager().unwrap().expect("a manager");
    assert_eq!(x.actor_count(), before + 1, "spawned into the level list");
    let class = x.vm().space.heap.get(m).unwrap().class;
    assert_eq!(class, x.vm().find_class("WillowGame.SkillEffectManager").unwrap());
    let game = x.game;
    assert_eq!(x.value(m, "Owner"), Value::Object(Some(game)));
    assert_eq!(x.value(game, "GlobalSkillManager"), Value::Object(Some(m)));
    // At (0, 0, 0), not at the game info's invented location.
    let at = x.value(m, "Location");
    assert_eq!(ow_combat::hit::access::field(x.vm(), &at, "X"), Some(Value::Float(0.0)));
    assert_eq!(x.value(m, "ActiveSkills").elements().map(<[Value]>::len), Some(0));

    let again = x.get_manager().unwrap();
    assert_eq!(again, Some(m), "the same manager");
    assert_eq!(x.actor_count(), before + 1, "nothing more is spawned");
}

#[test]
fn a_stored_manager_is_returned_as_it_is_without_a_spawn() {
    let mut x = fx(ParityMode::Off, 3, true);
    let stored = make(x.vm(), "WillowGame.SkillEffectManager");
    // Marked destroyed: the native does not check.
    x.vm().space.heap.get_mut(stored).unwrap().destroyed = true;
    let game = x.game;
    set_required(x.vm(), game, "GlobalSkillManager", Value::Object(Some(stored)), "test").unwrap();
    let before = x.actor_count();
    assert_eq!(x.get_manager().unwrap(), Some(stored));
    assert_eq!(x.actor_count(), before);
}

#[test]
fn a_game_info_that_is_not_the_authority_answers_none_and_spawns_nothing() {
    for role in [0, 1, 2, 4] {
        let mut x = fx(ParityMode::Off, role, true);
        let before = x.actor_count();
        assert_eq!(x.get_manager().unwrap(), None, "Role {role}");
        assert_eq!(x.actor_count(), before);
        let game = x.game;
        assert_eq!(x.value(game, "GlobalSkillManager"), Value::Object(None));
    }
}

#[test]
fn a_world_that_cannot_spawn_answers_not_yet_not_none() {
    let mut x = fx(ParityMode::Off, 3, false);
    let n = not_yet(x.get_manager());
    assert!(matches!(n.missing, Missing::LayerAbsent(_)), "{n:?}");
    let game = x.game;
    assert_eq!(x.value(game, "GlobalSkillManager"), Value::Object(None), "nothing stored");
}

#[test]
fn deactivate_all_is_a_no_op_without_a_skill_of_the_instigator() {
    let mut x = fx(ParityMode::Off, 3, true);
    let m = x.get_manager().unwrap().unwrap();
    let pc = make(x.vm(), "WillowGame.ToyController");
    let path = NATIVE_PATHS[1];
    // The fresh manager: no active skills.
    assert_eq!(x.call(path, m, vec![Value::Object(Some(pc))]).unwrap(), Value::None);
    // An empty entry and another controller's skill.
    let other = make(x.vm(), "WillowGame.ToyController");
    let theirs = make(x.vm(), "WillowGame.Skill");
    set_required(x.vm(), theirs, "SkillInstigator", Value::Object(Some(other)), "test").unwrap();
    let list = Value::array(vec![Value::Object(None), Value::Object(Some(theirs))]);
    set_required(x.vm(), m, "ActiveSkills", list, "test").unwrap();
    assert_eq!(x.call(path, m, vec![Value::Object(Some(pc))]).unwrap(), Value::None);
}

#[test]
fn deactivate_all_on_a_skill_of_the_instigator_is_not_yet() {
    let mut x = fx(ParityMode::Off, 3, true);
    let m = x.get_manager().unwrap().unwrap();
    let pc = make(x.vm(), "WillowGame.ToyController");
    let mine = make(x.vm(), "WillowGame.Skill");
    set_required(x.vm(), mine, "SkillInstigator", Value::Object(Some(pc)), "test").unwrap();
    let list = Value::array(vec![Value::Object(Some(mine))]);
    set_required(x.vm(), m, "ActiveSkills", list.clone(), "test").unwrap();
    let n = not_yet(x.call(NATIVE_PATHS[1], m, vec![Value::Object(Some(pc))]));
    assert_eq!(n.missing, Missing::NotRead);
    assert_eq!(x.value(m, "ActiveSkills"), list, "nothing changed");
}

#[test]
fn under_reference_both_natives_stay_unbound() {
    let mut x = fx(ParityMode::Reference, 3, true);
    let before = x.actor_count();
    let got = x.get_manager();
    assert!(
        matches!(got, Err(VmError::UnboundNative { .. })),
        "the stub policy answers it: {got:?}"
    );
    assert_eq!(x.actor_count(), before, "nothing spawned");
    let m = make(x.vm(), "WillowGame.SkillEffectManager");
    let got = x.call(NATIVE_PATHS[1], m, vec![Value::Object(None)]);
    assert!(matches!(got, Err(VmError::UnboundNative { .. })), "{got:?}");
}
