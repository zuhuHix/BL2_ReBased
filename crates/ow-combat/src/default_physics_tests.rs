//! `SetDefaultPhysics` past its gates on invented packages (AGENTS.md rule 9; NATIVE_SET_DEFAULT_PHYSICS_AND_SPAWN_BALANCE.md
//! section 6, tests 1 to 4). The class and field names are the packages'; every value is invented. `Physics` is a byte
//! of an invented `EPhysics` whose names are the engine's, so the rule names its modes. AI-assisted (Claude Code).

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
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_package::testkit::script::{FunctionSpec, Param, ScriptToy};
use ow_package::testkit::{Export, ints};
use ow_vm::api::{StubPolicy, Vm};
use ow_vm::error::VmError;
use ow_world::world::set_physics::{BASE_SEARCH, NAV_MESH};
use ow_world::world::{BalanceState, World};

use super::*;

static NEXT: AtomicU32 = AtomicU32::new(0);
const NATIVE: u32 = 0x400 | 0x20000;
const STATIC_NATIVE: u32 = 0x400 | 0x2000 | 0x20000;
const PHYSICS: [&str; 13] = [
    "PHYS_None",
    "PHYS_Walking",
    "PHYS_Falling",
    "PHYS_Swimming",
    "PHYS_Flying",
    "PHYS_Rotating",
    "PHYS_Projectile",
    "PHYS_Interpolating",
    "PHYS_Spider",
    "PHYS_Ladder",
    "PHYS_RigidBody",
    "PHYS_SoftBody",
    "PHYS_NavMeshWalking",
];
const NONE: u8 = 0;
const WALKING: u8 = 1;
const FALLING: u8 = 2;
const FLYING: u8 = 4;
const NAV_MESH_WALKING: u8 = 12;

fn engine() -> Vec<u8> {
    let mut t = ScriptToy::new(&["Byte", "Bool", "Object", "Float", "Struct"]);
    let object = t.class("Object", 0);
    let actor = t.class("Actor", object);
    let mut payload = ints(&[0]);
    payload.extend_from_slice(&t.none);
    payload.extend(ints(&[0, PHYSICS.len() as i32]));
    for v in PHYSICS {
        payload.extend_from_slice(&t.p.fname(v, 0));
    }
    let mut e = Export::new(t.import("Enum"), "EPhysics", payload);
    e.outer = actor;
    let physics = t.p.add_export_with(e);
    t.prop("Byte", actor, "Physics", 0, physics);
    for f in ["Base", "PhysicsVolume", "CollisionComponent"] {
        t.prop("Object", actor, f, 0, object);
    }
    for f in ["bCollideWorld", "bCollideActors", "bBlockActors", "bIgnoreEncroachers", "bHardAttach", "bTearOff"] {
        t.prop("Bool", actor, f, 0, 0);
    }
    t.prop("Byte", actor, "Role", 0, 0);
    let spec = FunctionSpec::new(NATIVE, "SetPhysics");
    t.function(actor, "SetPhysics", &[Param::new("Byte", "NewPhysics")], None, &[], &spec, |_| None);
    let pawn = t.class("Pawn", actor);
    for f in ["bNotifyStopFalling", "bNotifyBeginFalling", "bNotifyBeginWalking", "bNotifyStopWalking"] {
        t.prop("Bool", pawn, f, 0, 0);
    }
    t.class("Controller", actor);
    t.build()
}

fn willow() -> Vec<u8> {
    let mut t = ScriptToy::new(&["Byte", "Bool", "Object", "Float", "Struct"]);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let object = t.p.add_import("Core", "Class", engine, "Object");
    let pawn = t.p.add_import("Core", "Class", engine, "Pawn");
    let controller = t.p.add_import("Core", "Class", engine, "Controller");
    let willow_pawn = t.class("WillowPawn", pawn);
    t.prop("Byte", willow_pawn, "bCloaked", 0, 0);
    for f in ["BodyClass", "SMComponent"] {
        t.prop("Object", willow_pawn, f, 0, object);
    }
    let shield_ref = ow_package::testkit::reflect::toy_struct(&mut t, willow_pawn, "ToyShieldRef", None);
    t.prop("Struct", willow_pawn, "ShieldArmor", 0, shield_ref);
    for f in ["ShieldVar", "ShieldMaxVar"] {
        t.prop("Float", willow_pawn, f, 0, 0);
    }
    let can = FunctionSpec::new(STATIC_NATIVE, "StaticCanCloak");
    t.function(willow_pawn, "StaticCanCloak", &[], Some(("Bool", 0)), &[], &can, |_| None);
    let set = FunctionSpec::new(STATIC_NATIVE, "StaticSetCanCloak");
    t.function(willow_pawn, "StaticSetCanCloak", &[Param::new("Bool", "bCanCloak")], None, &[], &set, |_| None);
    let ai = t.class("WillowAIPawn", willow_pawn);
    for f in ["AIClass", "MyWillowMind"] {
        t.prop("Object", ai, f, 0, object);
    }
    let class = t.class("AIClassDefinition", object);
    t.prop("Bool", class, "bIsTurret", 0, 0);
    t.prop("Byte", class, "Physics", 0, 0);
    let mind = t.class("WillowMind", controller);
    t.prop("Object", mind, "WillowNav", 0, object);
    let nav = t.class("WillowNavigationHandle", object);
    t.prop("Byte", nav, "FlightMode", 0, 0);
    let body = t.class("BodyClassDefinition", object);
    for f in ["bCloakUseParentData", "bCloakingRequiresShields"] {
        t.prop("Bool", body, f, 0, 0);
    }
    t.prop("Object", body, "AnimationParentBody", 0, object);
    // The spawn's balance pick (note test 5): balances pointing at archetypes, all exports of this package.
    let balance = t.class("AIPawnBalanceDefinition", object);
    t.prop("Object", balance, "AIPawnArchetype", 0, object);
    let empty = |t: &ScriptToy| {
        let mut p = ints(&[0]);
        p.extend_from_slice(&t.none);
        p
    };
    let mut archetypes = Vec::new();
    for name in ["Pawn_One", "Pawn_Two", "Pawn_Unpicked"] {
        let e = empty(&t);
        archetypes.push(t.p.add_export(ai, name, e));
    }
    for (name, archetype) in [("Balance_One", 0), ("Balance_TwoA", 1), ("Balance_TwoB", 1)] {
        let mut p = ints(&[0]);
        p.extend(t.p.object_tag("AIPawnArchetype", archetypes[archetype]));
        p.extend_from_slice(&t.none);
        t.p.add_export(balance, name, p);
    }
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

fn fx() -> Fx {
    let dir = std::env::temp_dir().join(format!("ow-combat-sdp-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let (e, w) = (engine(), willow());
    let defs = DefStore::from_synthetic(
        &dir,
        &[("Engine.upk", e.as_slice()), ("WillowGame.upk", w.as_slice())],
        IdentityPolicy::FirstInScanOrder,
        ParityMode::Off,
    )
    .unwrap();
    let vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off)
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
    let world = World::new(vm, BalanceState::new(snapshot), &[&crate::hit::Pack]).unwrap();
    Fx { world, dir }
}

impl Fx {
    fn vm(&mut self) -> &mut Vm {
        &mut self.world.vm
    }
    fn make(&mut self, path: &str) -> ObjRef {
        let vm = self.vm();
        let c = vm.find_class(path).unwrap();
        ow_vm::interp::instantiate_from_defaults(vm, c).unwrap()
    }
    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let vm = self.vm();
        let c = vm.space.heap.get(o).unwrap().class;
        let p = vm.space.prop(c, name).unwrap_or_else(|| panic!("no {name}"));
        *vm.space.slot_mut(o, PropAddr::scalar(p)).unwrap() = v;
    }
    fn get(&mut self, o: ObjRef, name: &str) -> Value {
        let vm = self.vm();
        let c = vm.space.heap.get(o).unwrap().class;
        let p = vm.space.prop(c, name).unwrap();
        vm.space.read(o, PropAddr::scalar(p)).unwrap().clone()
    }
    fn writes(&mut self) -> u64 {
        self.vm().space.heap.committed_writes()
    }
    /// An AI pawn in `physics` with an `SMComponent`, a class (`turret`, `class_physics`), and its archetype recorded
    /// (a second pawn, `bCollideWorld` true, its three flags equal to the pawn's).
    fn ai(&mut self, physics: u8, turret: bool, class_physics: u8) -> ObjRef {
        let pawn = self.make("WillowGame.WillowAIPawn");
        let sm = self.make("Engine.Object");
        let class = self.make("WillowGame.AIClassDefinition");
        self.set(pawn, "Physics", Value::Byte(physics));
        self.set(pawn, "SMComponent", Value::Object(Some(sm)));
        self.set(pawn, "AIClass", Value::Object(Some(class)));
        self.set(class, "bIsTurret", Value::Bool(turret));
        self.set(class, "Physics", Value::Byte(class_physics));
        let archetype = self.make("WillowGame.WillowAIPawn");
        self.set(archetype, "bCollideWorld", Value::Bool(true));
        crate::hit::with_state(self.vm(), |s| s.archetypes.insert(pawn, archetype)).unwrap();
        pawn
    }
    /// A mind (with a navigation handle in `flight`, or none) for `pawn`.
    fn mind(&mut self, pawn: ObjRef, flight: Option<u8>) {
        let mind = self.make("WillowGame.WillowMind");
        self.set(pawn, "MyWillowMind", Value::Object(Some(mind)));
        if let Some(f) = flight {
            let nav = self.make("WillowGame.WillowNavigationHandle");
            self.set(nav, "FlightMode", Value::Byte(f));
            self.set(mind, "WillowNav", Value::Object(Some(nav)));
        }
    }
    fn run(&mut self, pawn: ObjRef) -> Result<(), VmError> {
        crate::hit::pools::set_default_physics(self.vm(), pawn)
    }
    fn not_read(&mut self, pawn: ObjRef) -> String {
        let before = self.writes();
        let what = match self.run(pawn) {
            Err(VmError::NotYet(n)) => {
                assert_eq!(n.missing, Missing::NotRead, "{n}");
                assert!(!ow_core::notyet::is_partial_writes(&n.to_string()), "{n}");
                n.what.to_string()
            }
            other => panic!("{other:?}"),
        };
        assert_eq!(self.writes(), before, "nothing written");
        what
    }
}

// Tests 1 and 2: the default physics by the rule, set through `SetPhysics`, and `bCollideWorld` copied.
#[test]
fn the_default_physics_and_bcollideworld() {
    // No AIClass: falling.
    let mut x = fx();
    let pawn = x.ai(NONE, false, NONE);
    x.set(pawn, "AIClass", Value::Object(None));
    x.run(pawn).unwrap();
    assert_eq!((x.get(pawn, "Physics"), x.get(pawn, "bCollideWorld")), (Value::Byte(FALLING), Value::Bool(true)));
    // No mind: the class's Physics.
    let mut x = fx();
    let pawn = x.ai(NONE, false, FLYING);
    x.run(pawn).unwrap();
    assert_eq!(x.get(pawn, "Physics"), Value::Byte(FLYING), "the class's");
    // A handle with FlightMode 2: flying.
    let mut x = fx();
    let pawn = x.ai(NONE, false, WALKING);
    x.mind(pawn, Some(2));
    x.run(pawn).unwrap();
    assert_eq!(x.get(pawn, "Physics"), Value::Byte(FLYING), "a flying handle");
    // The class's Physics flying, FlightMode 0: flying.
    let mut x = fx();
    let pawn = x.ai(NONE, false, FLYING);
    x.mind(pawn, Some(0));
    x.run(pawn).unwrap();
    assert_eq!(x.get(pawn, "Physics"), Value::Byte(FLYING), "a flying class");
    assert_eq!(x.get(pawn, "bCollideWorld"), Value::Bool(true));
}

// Test 1, the modes `SetPhysics` refuses: a turret (PHYS_None), the slice's case (a mind with no handle, the class
// PHYS_None, the pawn walking) and the nav-mesh cases; each refused before any write.
#[test]
fn the_modes_set_physics_refuses_are_refused_before_any_write() {
    let mut x = fx();
    let pawn = x.ai(FALLING, true, FLYING);
    assert_eq!(x.not_read(pawn), BASE_SEARCH, "a turret asks for PHYS_None");
    let mut x = fx();
    let pawn = x.ai(WALKING, false, NONE);
    x.mind(pawn, None);
    assert_eq!(x.not_read(pawn), BASE_SEARCH, "the slice's case");
    assert_eq!(x.get(pawn, "bCollideWorld"), Value::Bool(false));
    for physics in [NONE, NAV_MESH_WALKING] {
        let mut x = fx();
        let pawn = x.ai(physics, false, WALKING);
        x.mind(pawn, Some(0));
        assert_eq!(x.not_read(pawn), NAV_MESH, "a walking handle asks for PHYS_NavMeshWalking (from {physics})");
    }
}

// Test 3: the collision copy.
#[test]
fn the_collision_copy() {
    // Uncloaked, equal flags: nothing (and query 2 not read: no BodyClass, which would answer false).
    let mut x = fx();
    let pawn = x.ai(NONE, false, FLYING);
    x.run(pawn).unwrap();
    // Uncloaked, a flag differs: SetCollision's NotRead.
    let mut x = fx();
    let pawn = x.ai(NONE, false, FLYING);
    x.set(pawn, "bBlockActors", Value::Bool(true));
    assert_eq!(x.not_read(pawn), SET_COLLISION);
    // Cloaked, a body class needing shields, max 50: current 0 copies (refused), current 10 skips (answers).
    for (current, copied) in [(0.0f32, true), (10.0, false)] {
        let mut x = fx();
        let pawn = x.ai(NONE, false, FLYING);
        let body = x.make("WillowGame.BodyClassDefinition");
        x.set(body, "bCloakingRequiresShields", Value::Bool(true));
        x.set(pawn, "BodyClass", Value::Object(Some(body)));
        x.set(pawn, "bCloaked", Value::Byte(1));
        x.set(pawn, "ShieldMaxVar", Value::Float(50.0));
        x.set(pawn, "ShieldVar", Value::Float(current));
        x.set(pawn, "bBlockActors", Value::Bool(true));
        if copied {
            assert_eq!(x.not_read(pawn), SET_COLLISION, "current {current}: copied");
        } else {
            x.run(pawn).unwrap();
            assert_eq!(x.get(pawn, "bBlockActors"), Value::Bool(true), "current {current}: skipped");
        }
    }
    // Cloaked with the can-cloak flag clear: copied.
    let mut x = fx();
    let pawn = x.ai(NONE, false, FLYING);
    let body = x.make("WillowGame.BodyClassDefinition");
    x.set(pawn, "BodyClass", Value::Object(Some(body)));
    x.set(pawn, "bCloaked", Value::Byte(1));
    x.set(pawn, "bBlockActors", Value::Bool(true));
    x.run(pawn).unwrap();
    crate::hit::with_state(x.vm(), |s| s.set_can_cloak(false)).unwrap();
    x.set(pawn, "Physics", Value::Byte(NONE));
    assert_eq!(x.not_read(pawn), SET_COLLISION, "can-cloak clear: copied");
    // bCloakUseParentData: the parent's requirement counts.
    let mut x = fx();
    let pawn = x.ai(NONE, false, FLYING);
    let (child, parent) = (x.make("WillowGame.BodyClassDefinition"), x.make("WillowGame.BodyClassDefinition"));
    x.set(child, "bCloakUseParentData", Value::Bool(true));
    x.set(child, "AnimationParentBody", Value::Object(Some(parent)));
    x.set(parent, "bCloakingRequiresShields", Value::Bool(true));
    x.set(pawn, "BodyClass", Value::Object(Some(child)));
    x.set(pawn, "bCloaked", Value::Byte(1));
    x.set(pawn, "ShieldMaxVar", Value::Float(50.0));
    x.set(pawn, "bBlockActors", Value::Bool(true));
    assert_eq!(x.not_read(pawn), SET_COLLISION, "the parent requires shields, none left: copied");
}

// Test 4: no archetype recorded.
#[test]
fn no_archetype_is_not_read() {
    let mut x = fx();
    let pawn = x.ai(NONE, false, FLYING);
    crate::hit::with_state(x.vm(), |s| s.archetypes.remove(&pawn)).unwrap();
    assert_eq!(x.not_read(pawn), NO_ARCHETYPE);
}

// The can-cloak natives through the VM.
#[test]
fn the_can_cloak_flag_is_read_and_written_through_the_natives() {
    let mut x = fx();
    let vm = x.vm();
    let can = vm.find_function(CAN_CLOAK).unwrap();
    let set = vm.find_function(SET_CAN_CLOAK).unwrap();
    assert_eq!(vm.call(can, None, vec![]).unwrap().ret, Value::Bool(true), "initially true");
    vm.call(set, None, vec![Value::Bool(false)]).unwrap();
    assert_eq!(vm.call(can, None, vec![]).unwrap().ret, Value::Bool(false));
}

// Test 5's harness rows: one balance pointing at the archetype is picked and logged; none, or two, is recorded as the
// step's refusal.
#[test]
fn the_balance_pick_is_by_the_archetype_pointer() {
    let mut x = fx();
    let pawn = x.make("WillowGame.WillowAIPawn");
    crate::spawn_balance::pick(x.vm(), pawn, "WillowGame", "WillowGame.Pawn_One").unwrap();
    let picked = match ow_world::spawn_balance::pending(x.vm(), pawn).unwrap() {
        Some(Ok(b)) => b,
        other => panic!("{other:?}"),
    };
    assert!(crate::hit::access::path_of(x.vm(), Some(picked)).ends_with("Balance_One"));
    let line = crate::spawn_balance::pick_line("WillowGame.Pawn_One", "Balance_One");
    assert!(x.vm().log.iter().any(|l| l == &line), "logged: {:?}", x.vm().log);
    for (archetype, why) in [("WillowGame.Pawn_Unpicked", "no AIPawnBalanceDefinition"), ("WillowGame.Pawn_Two", "2 AIPawnBalanceDefinitions")] {
        let pawn = x.make("WillowGame.WillowAIPawn");
        crate::spawn_balance::pick(x.vm(), pawn, "WillowGame", archetype).unwrap();
        match ow_world::spawn_balance::pending(x.vm(), pawn).unwrap() {
            Some(Err(text)) => assert!(text.contains(why) && text.contains(archetype), "{text}"),
            other => panic!("{archetype}: {other:?}"),
        }
    }
}
