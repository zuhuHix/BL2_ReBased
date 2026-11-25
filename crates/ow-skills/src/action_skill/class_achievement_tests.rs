//! NATIVE_UNLOCK_CLASS_ACHIEVEMENT.md test 11, the action-skill pack's half (lane IMPL-UNLOCKPLAYERCLASSACHIEVEMENT):
//! the pack binds `ow_inventory::globals::class_achievement`'s rule for link 9's world, keep-existing beside the
//! inventory's registration in either order, and nothing under Reference. One invented `WillowGame` package over an
//! invented `Engine` (AGENTS.md rule 9): `WillowPlayerController` with `PlayerClass` and the native. The rule itself is
//! tested in `ow-inventory`.

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_balance::types::{
    AwesomeLevel, BalanceSnapshot, ExpLevel, GameStage, LevelCapUpgrades, OpLevel, PlayerCount, Playthrough,
};
use ow_core::ParityMode;
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_inventory::globals::class_achievement::{KEY, PATH};
use ow_object::heap::ObjectSpace;
use ow_object::value::Value;
use ow_package::testkit::script::{FunctionSpec, Param, ScriptToy};
use ow_vm::api::{NativePack, StubPolicy, Vm};
use ow_world::world::{BalanceState, World};

const NATIVE: u32 = 0x400 | 0x20000;
static NEXT: AtomicU32 = AtomicU32::new(0);
const IMPORTS: [&str; 6] = ["Class", "Function", "ObjectProperty", "BoolProperty", "IntProperty", "ByteProperty"];

fn engine() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let object = t.class("Object", 0);
    // The world's set-up looks these up.
    let actor = t.class("Actor", object);
    t.class("Pawn", actor);
    t.build()
}

fn willow_game() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let object = t.p.add_import("Core", "Class", engine, "Object");
    let controller = t.class("WillowPlayerController", object);
    t.prop("Object", controller, "PlayerClass", 0, object);
    let params = [
        Param::new("Byte", "AchievementUnlockType"),
        Param::new("Int", "IntData"),
        Param::new("Bool", "bCanBeGreater"),
        Param::new("Bool", "bCanBeLess"),
    ];
    let name = "UnlockPlayerClassAchievementIfConditionsMet";
    t.function(controller, name, &params, Some(("Bool", 0)), &[], &FunctionSpec::new(NATIVE | 0x20, name), |_| None);
    t.build()
}

/// Where the inventory's own registration runs.
#[derive(Clone, Copy, PartialEq)]
enum Inventory {
    Absent,
    Before,
    After,
}

struct Fx {
    world: World,
    dir: PathBuf,
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn fx(mode: ParityMode, inventory: Inventory) -> Fx {
    let dir = std::env::temp_dir().join(format!(
        "ow-skills-class-achievement-{}-{}",
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
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), mode)
        .with_stub_policy(StubPolicy::refuse_all());
    vm.find_class("WillowGame.WillowPlayerController").unwrap();
    if inventory == Inventory::Before {
        ow_inventory::globals::class_achievement::register(&mut vm).unwrap();
    }
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
    let packs: [&dyn NativePack; 1] = [&crate::action_skill::Pack];
    let mut world = World::new(vm, BalanceState::new(snapshot), &packs).unwrap();
    if inventory == Inventory::After {
        ow_inventory::globals::class_achievement::register(&mut world.vm).unwrap();
    }
    Fx { world, dir }
}

impl Fx {
    /// The native on a fresh controller (its `PlayerClass` None): false when bound.
    fn call(&mut self) -> Result<Value, ow_vm::error::VmError> {
        let c = self.world.vm.find_class("WillowGame.WillowPlayerController").unwrap();
        let pc = ow_vm::interp::instantiate_from_defaults(&mut self.world.vm, c).unwrap();
        let f = self.world.vm.find_function(PATH).unwrap();
        let args = vec![Value::Byte(2), Value::Int(1), Value::Bool(true)];
        Ok(self.world.vm.call(f, Some(pc), args)?.ret)
    }
}

/// Test 11: the pack alone, and with the inventory's registration before or after it, binds the key once (no
/// duplicate registration) and the call answers the rule's false.
#[test]
fn either_order_keeps_one_binding() {
    for order in [Inventory::Absent, Inventory::Before, Inventory::After] {
        let mut x = fx(ParityMode::Off, order);
        assert_eq!(x.world.vm.package_bindings(KEY), 1);
        assert!(!x.world.vm.native_keys().any(|k| k == KEY));
        assert_eq!(x.call().unwrap(), Value::Bool(false));
    }
}

/// Test 11: under Reference the pack binds nothing new.
#[test]
fn reference_binds_nothing_new() {
    let mut x = fx(ParityMode::Reference, Inventory::Absent);
    assert!(!x.world.vm.has_native(KEY));
    assert!(x.call().is_err());
}
