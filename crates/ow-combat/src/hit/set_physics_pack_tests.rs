//! NATIVE_ACTOR_SET_PHYSICS.md section 4, tests 15 and 16 (lane IMPL-ACTOR-SETPHYSICS): the hit pack's
//! `Actor.SetPhysics` byte-write stand-in is Reference-only. In Off a world gets `ow_world::world::set_physics`'s
//! rule whether the hit pack is registered before the world's natives or after them; in Reference both orders get the
//! stand-in's plain byte write. One invented `Engine` package (AGENTS.md rule 9): `Object` and `Actor` (`Physics`,
//! `Base`, `PhysicsVolume`, `CollisionComponent`, the native `SetPhysics(byte)`).

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_balance::types::{
    AwesomeLevel, BalanceSnapshot, ExpLevel, GameStage, LevelCapUpgrades, OpLevel, PlayerCount, Playthrough,
};
use ow_core::{Missing, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_object::ids::PropAddr;
use ow_object::value::Value;
use ow_package::testkit::script::{FunctionSpec, Param, ScriptToy};
use ow_vm::api::{NativePack, StubPolicy, Vm};
use ow_vm::error::VmError;
use ow_world::world::{BalanceState, World};

static NEXT: AtomicU32 = AtomicU32::new(0);

fn engine() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class", "Function", "ByteProperty", "ObjectProperty"]);
    let object = t.class("Object", 0);
    let actor = t.class("Actor", object);
    t.prop("Byte", actor, "Physics", 0, 0);
    for f in ["Base", "PhysicsVolume", "CollisionComponent"] {
        t.prop("Object", actor, f, 0, object);
    }
    let spec = FunctionSpec::new(0x400 | 0x20000, "SetPhysics");
    t.function(actor, "SetPhysics", &[Param::new("Byte", "NewPhysics")], None, &[], &spec, |_| None);
    t.build()
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

/// A world in `mode` with the hit pack registered before the world's natives (`before`) or after them.
fn fx(mode: ParityMode, before: bool) -> Fx {
    let dir = std::env::temp_dir().join(format!("ow-combat-setphys-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let e = engine();
    let defs = DefStore::from_synthetic(&dir, &[("Engine.upk", e.as_slice())], IdentityPolicy::FirstInScanOrder, mode).unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), mode)
        .with_stub_policy(StubPolicy::refuse_all());
    vm.find_class("Engine.Actor").unwrap();
    if before {
        crate::hit::Pack.register(&mut vm).unwrap();
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
    let packs: Vec<&dyn NativePack> = if before { vec![] } else { vec![&crate::hit::Pack] };
    let world = World::new(vm, BalanceState::new(snapshot), &packs).unwrap();
    Fx { world, dir }
}

impl Fx {
    /// SetPhysics(mode) on a fresh actor (Physics 0): the answer, and Physics afterwards.
    fn set_physics(&mut self, mode: u8) -> (Result<Value, VmError>, Value) {
        let vm = &mut self.world.vm;
        let c = vm.find_class("Engine.Actor").unwrap();
        let actor = ow_vm::interp::instantiate_from_defaults(vm, c).unwrap();
        let f = vm.find_function("Engine.Actor.SetPhysics").unwrap();
        let r = vm.call(f, Some(actor), vec![Value::Byte(mode)]).map(|r| r.ret);
        let p = vm.space.prop(c, "Physics").unwrap();
        (r, vm.space.read(actor, PropAddr::scalar(p)).unwrap().clone())
    }
}

/// Test 15: in Off, both orders get the rule: 14 on an `Engine.Actor` writes Physics; 1 (walking) is the rule's
/// base-search `NotRead` with Physics unchanged, where the stand-in would write it.
#[test]
fn off_gets_the_rule_in_either_order() {
    for before in [true, false] {
        let mut x = fx(ParityMode::Off, before);
        let (r, physics) = x.set_physics(14);
        assert_eq!((r.unwrap(), physics), (Value::None, Value::Byte(14)), "before {before}");
        let (r, physics) = x.set_physics(1);
        match r {
            Err(VmError::NotYet(n)) => {
                // The base search's refusal in a world with no collision service (lane IMPL-LINK34-AFTER-MIND).
                assert_eq!(n.what.as_ref(), ow_world::world::set_physics::BASE_TRACE);
                assert!(matches!(n.missing, Missing::LayerAbsent(_)), "{n}");
            }
            other => panic!("before {before}: expected the rule's refusal, got {other:?}"),
        }
        assert_eq!(physics, Value::Byte(0), "before {before}: nothing written");
    }
}

/// Tests 15 and 16: in Reference, both orders get the stand-in's plain byte write (1 is written too).
#[test]
fn reference_keeps_the_stand_in_in_either_order() {
    for before in [true, false] {
        let mut x = fx(ParityMode::Reference, before);
        let (r, physics) = x.set_physics(1);
        assert_eq!((r.unwrap(), physics), (Value::None, Value::Byte(1)), "before {before}");
    }
}
