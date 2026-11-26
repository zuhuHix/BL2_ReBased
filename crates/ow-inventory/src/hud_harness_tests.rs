//! NATIVE_LINK7_HUD_TARGETING.md section 9, tests 1 to 4 and 7 (the hub's four rulings), on invented packages (AGENTS.md
//! rule 9; every value is invented, the class and field names are the packages'):
//! - `Engine`: the actor fields the spawn reads, `Controller` / `PlayerController` (`Pawn`, `myHUD`), `Pawn`
//!   (`BaseEyeHeight`), `WorldInfo` (`Game`), `GameInfo` (`HUDType`), `HUD`, `PrimitiveComponent`, `CylinderComponent`,
//!   `SkeletalMeshComponent`;
//! - `GearboxFramework`: `GearboxGlobalsDefinition`;
//! - `WillowGame`: `WillowHUD`, `WillowPickup`, `WillowGlobals` (`PickupList`, the native `AddPickup`), `GlobalsDefinition`
//!   (`HUDDefinition`), `HUDDefinition`.
//!
//! AI-assisted (Claude Code).

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
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_package::testkit::reflect::{toy_array_of, toy_struct};
use ow_package::testkit::script::{FunctionSpec, Param, ScriptToy};
use ow_vm::api::{StubPolicy, Vm};
use ow_vm::error::VmError;
use ow_world::world::{BalanceState, World};

use super::*;

const NATIVE: u32 = 0x400 | 0x20000;
static NEXT: AtomicU32 = AtomicU32::new(0);
const IMPORTS: [&str; 11] = [
    "Class",
    "Function",
    "ScriptStruct",
    "ObjectProperty",
    "ClassProperty",
    "BoolProperty",
    "IntProperty",
    "FloatProperty",
    "StructProperty",
    "ArrayProperty",
    "ByteProperty",
];

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
    for b in ["bStatic", "bNoDelete", "bDeleteMe", "bPendingDelete", "bCollideWorld", "bCollideWhenPlacing", "bCollideActors", "bNoEncroachCheck", "bWorldGeometry"] {
        t.prop("Bool", actor, b, 0, 0);
    }
    for b in ["Role", "RemoteRole", "Physics"] {
        t.prop("Byte", actor, b, 0, 0);
    }
    for f in ["LifeSpan", "CreationTime"] {
        t.prop("Float", actor, f, 0, 0);
    }
    for o in ["Owner", "Base", "Instigator", "WorldInfo"] {
        t.prop("Object", actor, o, 0, actor);
    }
    t.prop("Object", actor, "CollisionComponent", 0, object);
    t.prop("Struct", actor, "Location", 0, vector);
    t.prop("Struct", actor, "Rotation", 0, rotator);
    for a in ["Attached", "Touching", "Children", "Components", "AllComponents"] {
        toy_array_of(&mut t, actor, a, "Object", object);
    }
    let primitive = t.class("PrimitiveComponent", object);
    t.prop("Object", primitive, "Owner", 0, actor);
    t.prop("Struct", primitive, "Translation", 0, vector);
    // The walkers' flags (NATIVE_LINK7_LINE_CHECK.md section 3; lane IMPL-LINK7-LINE-CHECK).
    for b in ["AbsoluteTranslation", "AbsoluteRotation", "AbsoluteScale", "BulletListener", "BlockZeroExtent", "CollideActors"] {
        t.prop("Bool", primitive, b, 0, 0);
    }
    let cylinder = t.class("CylinderComponent", primitive);
    t.prop("Float", cylinder, "CollisionRadius", 0, 0);
    t.prop("Float", cylinder, "CollisionHeight", 0, 0);
    t.class("SkeletalMeshComponent", primitive);
    t.class("StaticMeshComponent", primitive);
    // Two classes the walk meets as live actors with a wrong or missing declaration (review of #818, item 5).
    let odd = t.class("ToyOddActor", object);
    t.prop("Object", odd, "CollisionComponent", 0, object);
    t.prop("Int", odd, "Components", 0, 0);
    t.class("ToyBareActor", object);
    let pawn = t.class("Pawn", actor);
    t.prop("Float", pawn, "BaseEyeHeight", 0, 0);
    for b in ["bIgnoreAllTraces", "bStationary"] {
        t.prop("Bool", pawn, b, 0, 0);
    }
    t.class("StaticMeshActor", actor);
    t.class("Terrain", actor);
    t.class("ToyPawnChild", pawn);
    t.class("InterpActor", actor);
    let volume = t.class("Volume", actor);
    t.prop("Bool", volume, "bPawnsOnly", 0, 0);
    let controller = t.class("Controller", actor);
    t.prop("Object", controller, "Pawn", 0, pawn);
    let player = t.class("PlayerController", controller);
    let hud = t.class("HUD", actor);
    t.prop("Object", player, "myHUD", 0, hud);
    t.prop("Object", hud, "PlayerOwner", 0, player);
    let game = t.class("GameInfo", actor);
    let class_class = t.import("Class");
    t.prop_tail("Class", game, "HUDType", 0, &ow_package::testkit::ints(&[class_class, hud]));
    let info = t.class("WorldInfo", actor);
    t.prop("Byte", info, "NetMode", 0, 0);
    t.prop("Float", info, "TimeSeconds", 0, 0);
    t.prop("Object", info, "Game", 0, game);
    t.build()
}

fn gearbox() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let object = t.p.add_import("Core", "Class", engine, "Object");
    t.class("GearboxGlobalsDefinition", object);
    t.build()
}

fn willow() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let object = t.p.add_import("Core", "Class", engine, "Object");
    let actor = t.p.add_import("Core", "Class", engine, "Actor");
    let hud = t.p.add_import("Core", "Class", engine, "HUD");
    let player = t.p.add_import("Core", "Class", engine, "PlayerController");
    let gearbox = t.p.add_import("Core", "Package", 0, "GearboxFramework");
    let gearbox_definition = t.p.add_import("Core", "Class", gearbox, "GearboxGlobalsDefinition");
    let hud_definition = t.class("HUDDefinition", object);
    t.prop("Float", hud_definition, "PickupRange", 0, 0);
    let willow_hud = t.class("WillowHUD", hud);
    t.prop("Object", willow_hud, "WPlayerOwner", 0, player);
    t.prop("Object", willow_hud, "HUDDef", 0, hud_definition);
    let pickup = t.class("WillowPickup", actor);
    // A pickup subclass with no read filter row (an unread override), and the Willow pawn classes.
    t.class("ToyPickupChild", pickup);
    let engine_pawn = t.p.add_import("Core", "Class", engine, "Pawn");
    let willow_pawn = t.class("WillowPawn", engine_pawn);
    t.class("WillowPlayerPawn", willow_pawn);
    let definition = t.class("GlobalsDefinition", gearbox_definition);
    t.prop("Object", definition, "HUDDefinition", 0, hud_definition);
    let globals = t.class("WillowGlobals", object);
    t.prop("Object", globals, "TheGlobalsDefinition", 0, gearbox_definition);
    toy_array_of(&mut t, globals, "PickupList", "Object", pickup);
    let param = Param { type_ref: pickup, ..Param::new("Object", "Pickup") };
    t.function(globals, "AddPickup", &[param], None, &[], &FunctionSpec::new(NATIVE, "AddPickup"), |_| None);
    t.build()
}

struct Fx {
    w: World,
    dir: PathBuf,
    controller: ObjRef,
    pawn: ObjRef,
    game: ObjRef,
    globals: ObjRef,
    definition: ObjRef,
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn fx_in(mode: ParityMode) -> Fx {
    let dir = std::env::temp_dir().join(format!("ow-inventory-hud-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let (e, g, w) = (engine(), gearbox(), willow());
    let defs = DefStore::from_synthetic(
        &dir,
        &[("Engine.upk", e.as_slice()), ("GearboxFramework.upk", g.as_slice()), ("WillowGame.upk", w.as_slice())],
        IdentityPolicy::FirstInScanOrder,
        mode,
    )
    .unwrap();
    let vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), mode).with_stub_policy(StubPolicy::refuse_all());
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
    let mut world = World::new(vm, BalanceState::new(snapshot), &[]).unwrap();
    for c in ["Engine.CylinderComponent", "WillowGame.WillowHUD", "WillowGame.GlobalsDefinition", "GearboxFramework.GearboxGlobalsDefinition"] {
        world.vm.find_class(c).unwrap();
    }
    if mode == ParityMode::Off {
        crate::globals::register(&mut world.vm).unwrap();
    }
    let make = |vm: &mut Vm, path: &str| {
        let c = vm.find_class(path).unwrap();
        ow_vm::interp::instantiate_from_defaults(vm, c).unwrap()
    };
    let info = make(&mut world.vm, "Engine.WorldInfo");
    world.actors().borrow_mut().place_world_info(info).unwrap();
    let game = make(&mut world.vm, "Engine.GameInfo");
    let controller = make(&mut world.vm, "Engine.PlayerController");
    let pawn = make(&mut world.vm, "Engine.Pawn");
    let globals = make(&mut world.vm, "WillowGame.WillowGlobals");
    let definition = make(&mut world.vm, "WillowGame.GlobalsDefinition");
    let hud_definition = make(&mut world.vm, "WillowGame.HUDDefinition");
    let mut x = Fx { w: world, dir, controller, pawn, game, globals, definition };
    x.set(info, "Game", Value::Object(Some(game)));
    let hud_class = x.w.vm.find_class("WillowGame.WillowHUD").unwrap();
    x.set(game, "HUDType", Value::Class(Some(hud_class)));
    x.set(controller, "WorldInfo", Value::Object(Some(info)));
    x.set(controller, "Pawn", Value::Object(Some(pawn)));
    x.set(definition, "HUDDefinition", Value::Object(Some(hud_definition)));
    x.set(globals, "TheGlobalsDefinition", Value::Object(Some(definition)));
    x.w.query().borrow_mut().set_globals(globals);
    x
}

fn fx() -> Fx {
    fx_in(ParityMode::Off)
}

impl Fx {
    fn vm(&mut self) -> &mut Vm {
        &mut self.w.vm
    }
    fn obj(&mut self, path: &str) -> ObjRef {
        let vm = self.vm();
        let c = vm.find_class(path).unwrap();
        ow_vm::interp::instantiate_from_defaults(vm, c).unwrap()
    }
    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let vm = self.vm();
        let class = vm.space.heap.get(o).unwrap().class;
        let p = vm.space.prop(class, name).unwrap_or_else(|| panic!("no {name}"));
        vm.space.write(o, PropAddr::scalar(p), v).unwrap();
    }
    fn get(&mut self, o: ObjRef, name: &str) -> Value {
        let vm = self.vm();
        let class = vm.space.heap.get(o).unwrap().class;
        let p = vm.space.prop(class, name).unwrap();
        vm.space.read(o, PropAddr::scalar(p)).unwrap().clone()
    }
    /// Fields of the struct value at `o.name`, set by name.
    fn fields(&mut self, o: ObjRef, name: &str, values: &[(&str, Value)]) {
        let mut s = self.get(o, name);
        {
            let vm = &self.w.vm;
            let st = s.struct_mut().unwrap();
            let layout = vm.space.classes.struct_layout(st.ty).unwrap().clone();
            for (f, v) in values {
                let (i, _) = layout.field(vm.space.names.find(f).unwrap()).unwrap();
                st.fields[i as usize] = v.clone();
            }
        }
        self.set(o, name, s);
    }
    fn vector(&mut self, o: ObjRef, name: &str, v: [f32; 3]) {
        self.fields(o, name, &[("X", Value::Float(v[0])), ("Y", Value::Float(v[1])), ("Z", Value::Float(v[2]))]);
    }
    fn counters(&self) -> (u64, u64, u64) {
        let vm = &self.w.vm;
        (vm.space.heap.committed_writes(), vm.rng.draws(), vm.defs.load_transitions())
    }
    /// #518's three counters and the class-code loads (lane IMPL-LINK7-LINE-CHECK).
    fn marks(&self) -> (u64, u64, u64, u64) {
        let (a, b, c) = self.counters();
        (a, b, c, self.w.vm.class_code_loads())
    }
    /// A placed actor of `class` at `at` whose collision is a colliding, zero-extent-blocking cylinder of `radius` and
    /// half height `half` (its owner's `bCollideActors` set).
    fn colliding(&mut self, class: &str, at: [f32; 3], radius: f32, half: f32) -> (ObjRef, ObjRef) {
        let a = self.obj(class);
        self.vector(a, "Location", at);
        let c = self.obj("Engine.CylinderComponent");
        self.set(c, "CollisionRadius", Value::Float(radius));
        self.set(c, "CollisionHeight", Value::Float(half));
        self.set(c, "Owner", Value::Object(Some(a)));
        for f in ["BlockZeroExtent", "CollideActors"] {
            self.set(c, f, Value::Bool(true));
        }
        self.set(a, "bCollideActors", Value::Bool(true));
        self.set(a, "CollisionComponent", Value::Object(Some(c)));
        self.w.actors().borrow_mut().place(a).unwrap();
        (a, c)
    }
}

fn not_yet(r: Result<impl std::fmt::Debug, VmError>) -> (Missing, String) {
    match r {
        Err(VmError::NotYet(n)) => (n.missing, n.what.to_string()),
        other => panic!("{other:?}"),
    }
}

// Test 1: the harness HUD.
#[test]
fn the_harness_hud_has_its_data_steps_only() {
    let mut x = fx();
    let controller = x.controller;
    // The setting off: the slice makes no HUD (it asks `harness_on`), and the targeting answers its no-HUD LayerAbsent.
    assert!(!harness_on(&x.w.vm));
    assert!(matches!(not_yet(crate::pickup_targeting::hud_of(&x.w.vm, controller)).0, Missing::LayerAbsent(_)));
    enable_hud_targeting_harness(x.vm()).unwrap();
    assert!(harness_on(&x.w.vm));
    assert!(x.w.vm.log.iter().any(|l| l == SETTING_INPUT));
    let hud = harness_hud(x.vm(), controller).unwrap();
    assert_eq!(x.get(controller, "myHUD"), Value::Object(Some(hud)));
    for f in ["PlayerOwner", "WPlayerOwner"] {
        assert_eq!(x.get(hud, f), Value::Object(Some(controller)), "{f}");
    }
    let hud_definition = x.get(x.definition, "HUDDefinition");
    assert_eq!(x.get(hud, "HUDDef"), hud_definition);
    assert_eq!(x.get(hud, "Owner"), Value::Object(Some(controller)));
    assert!(x.w.vm.log.iter().any(|l| l == HUD_INPUT), "one harness line");
    // Made before any begin play: no begin-play event ran on it (the world has not begun play).
    assert!(!x.w.actors().borrow().begun_play());
    // A second set-up: NotRead, the first HUD kept, nothing written.
    let before = x.counters();
    assert_eq!(not_yet(harness_hud(x.vm(), controller)), (Missing::NotRead, SECOND_SET_UP.to_string()));
    assert_eq!(x.counters(), before);
    assert_eq!(x.get(controller, "myHUD"), Value::Object(Some(hud)));
    assert!(matches!(enable_hud_targeting_harness(x.vm()), Err(VmError::Registration { .. })), "twice");
}

#[test]
fn the_harness_hud_refuses_before_any_write() {
    let mut x = fx();
    enable_hud_targeting_harness(x.vm()).unwrap();
    let controller = x.controller;
    // An unloaded HUDDefinition.
    let class = x.w.vm.find_class("WillowGame.HUDDefinition").unwrap();
    let lazy = x.w.vm.space.alloc_stand_in(class, "Lazy", Origin::Default { class }).unwrap();
    let definition = x.definition;
    x.set(definition, "HUDDefinition", Value::Object(Some(lazy)));
    let before = x.counters();
    assert_eq!(not_yet(harness_hud(x.vm(), controller)), (Missing::NotRead, HUD_DEFINITION.to_string()));
    assert_eq!(x.counters(), before);
    // A HUDType that is no WillowHUD, and no game info: the HUD class's NotRead.
    let plain = x.w.vm.find_class("Engine.HUD").unwrap();
    let game = x.game;
    x.set(game, "HUDType", Value::Class(Some(plain)));
    let before = x.counters();
    assert_eq!(not_yet(harness_hud(x.vm(), controller)), (Missing::NotRead, HUD_CLASS.to_string()));
    assert_eq!(x.counters(), before);
    let info = x.get(controller, "WorldInfo");
    let Value::Object(Some(info)) = info else { panic!() };
    x.set(info, "Game", Value::Object(None));
    let before = x.counters();
    assert_eq!(not_yet(harness_hud(x.vm(), controller)), (Missing::NotRead, NO_GAME.to_string()));
    assert_eq!(x.counters(), before, "nothing written");
    assert_eq!(x.get(controller, "myHUD"), Value::Object(None));
}

// Test 2: the registration (ruling 2's fallback form).
#[test]
fn a_dropped_pickup_is_registered_only_with_the_setting() {
    let mut x = fx();
    let p = x.obj("WillowGame.WillowPickup");
    register_pickup(x.vm(), p).unwrap();
    let globals = x.globals;
    assert_eq!(x.get(globals, "PickupList"), Value::array(vec![]), "off: nothing");
    enable_hud_targeting_harness(x.vm()).unwrap();
    register_pickup(x.vm(), p).unwrap();
    assert_eq!(x.get(globals, "PickupList"), Value::array(vec![Value::Object(Some(p))]), "on: once");
    assert_eq!(x.w.vm.log.iter().filter(|l| *l == REGISTER_INPUT).count(), 1);
}

// Test 3: the view.
#[test]
fn the_view_is_the_pawn_s_eye_point_with_the_setting() {
    let mut x = fx();
    let (controller, pawn) = (x.controller, x.pawn);
    x.vector(pawn, "Location", [10.0, 20.0, 30.0]);
    x.set(pawn, "BaseEyeHeight", Value::Float(64.0));
    x.fields(controller, "Rotation", &[("Pitch", Value::Int(100)), ("Yaw", Value::Int(200))]);
    let mut off = HarnessHost::for_world(&x.w.vm);
    assert!(matches!(not_yet(off.view(&x.w.vm, controller)).0, Missing::LayerAbsent(_)), "off: the camera");
    enable_hud_targeting_harness(x.vm()).unwrap();
    let mut host = HarnessHost::for_world(&x.w.vm);
    assert_eq!(host.view(&x.w.vm, controller).unwrap(), View { location: [10.0, 20.0, 94.0], pitch: 100, yaw: 200 });
    assert_eq!(host.inputs, [VIEW_INPUT]);
    // No pawn: the camera's LayerAbsent stays.
    x.set(controller, "Pawn", Value::Object(None));
    assert!(matches!(not_yet(host.view(&x.w.vm, controller)).0, Missing::LayerAbsent(_)));
}

// Test 4: the line check under NoLevel (NATIVE_LINK7_LINE_CHECK.md section 5; lane IMPL-LINK7-LINE-CHECK). The
// segment runs from the view point (0, 0, 75) to 25 short of the target; the HUD, owned by the controller, is the
// source. Invented positions throughout.

/// A world with the setting on, `NoLevel`, the harness HUD owned by the controller, and a target pickup; the host.
fn line_world() -> (Fx, ObjRef, ObjRef, HarnessHost) {
    let mut x = fx();
    crate::pickup_targeting::enable_no_level_line_check(x.vm()).unwrap();
    ow_world::placement::install_collision(x.vm(), Rc::new(ow_world::placement::NoLevel)).unwrap();
    let hud = x.obj("WillowGame.WillowHUD");
    let controller = x.controller;
    x.set(hud, "Owner", Value::Object(Some(controller)));
    x.w.actors().borrow_mut().place(hud).unwrap();
    let target = x.obj("WillowGame.WillowPickup");
    x.vector(target, "Location", [150.0, 0.0, 0.0]);
    let host = HarnessHost::for_world(&x.w.vm);
    (x, hud, target, host)
}

const FROM: [f32; 3] = [0.0, 0.0, 75.0];
const TO: [f32; 3] = [125.0, 0.0, 10.0];

fn line(x: &Fx, host: &mut HarnessHost, hud: ObjRef, target: ObjRef) -> Result<Option<ObjRef>, VmError> {
    let before = x.marks();
    let r = host.line_blocker_for(&x.w.vm, FROM, TO, hud, target);
    assert_eq!(x.marks(), before, "reads only, no load, no class code");
    r
}

#[test]
fn the_line_check_s_settings_and_services() {
    let mut x = fx();
    let target = x.obj("WillowGame.WillowPickup");
    let hud = x.obj("WillowGame.WillowHUD");
    // Off: the headless LayerAbsent.
    let mut off = HarnessHost::for_world(&x.w.vm);
    assert!(matches!(not_yet(off.line_blocker_for(&x.w.vm, FROM, TO, hud, target)).0, Missing::LayerAbsent(_)));
    crate::pickup_targeting::enable_no_level_line_check(x.vm()).unwrap();
    assert!(x.w.vm.log.iter().any(|l| l == crate::pickup_targeting::NO_LEVEL_LINE_INPUT));
    assert!(matches!(crate::pickup_targeting::enable_no_level_line_check(x.vm()), Err(VmError::Registration { .. })), "twice");
    // On, no collision service: still the headless LayerAbsent.
    let mut host = HarnessHost::for_world(&x.w.vm);
    assert!(matches!(not_yet(host.line_blocker_for(&x.w.vm, FROM, TO, hud, target)).0, Missing::LayerAbsent(_)));
    // Section 6, test 8: a service holding geometry with the setting on is LINE_GEOMETRY's NotRead.
    struct WithGeometry;
    impl ow_world::placement::SpawnCollision for WithGeometry {
        fn box_test(&self, _: [f32; 3], _: [f32; 3], _: ow_world::placement::Mode) -> Result<ow_world::placement::Test, VmError> {
            Ok(ow_world::placement::Test::Clear)
        }
        fn box_sweep(&self, _: [f32; 3], _: [f32; 3], _: [f32; 3]) -> Result<ow_world::placement::Sweep, VmError> {
            Ok(ow_world::placement::Sweep::Clear)
        }
        fn point_sweep(&self, _: [f32; 3], _: [f32; 3]) -> Result<ow_world::placement::Sweep, VmError> {
            Ok(ow_world::placement::Sweep::Clear)
        }
    }
    ow_world::placement::install_collision(x.vm(), Rc::new(WithGeometry)).unwrap();
    assert_eq!(not_yet(host.line_blocker_for(&x.w.vm, FROM, TO, hud, target)), (Missing::NotRead, LINE_GEOMETRY.to_string()));
}

/// Section 6, tests 1 and 3: pickups and pawns on the line never block it, whatever their collision or bounds.
#[test]
fn pickups_and_pawns_never_block_the_line() {
    let (mut x, hud, target, mut host) = line_world();
    // The target on the line with collision of its own, and a decoy pickup (a cylinder, then a skeletal mesh).
    let tc = x.obj("Engine.CylinderComponent");
    for f in ["BlockZeroExtent", "CollideActors"] {
        x.set(tc, f, Value::Bool(true));
    }
    x.set(tc, "Owner", Value::Object(Some(target)));
    x.set(target, "CollisionComponent", Value::Object(Some(tc)));
    x.set(target, "bCollideActors", Value::Bool(true));
    x.w.actors().borrow_mut().place(target).unwrap();
    x.colliding("WillowGame.WillowPickup", [60.0, 0.0, 40.0], 100.0, 40.0);
    let (decoy2, _) = x.colliding("WillowGame.WillowPickup", [30.0, 0.0, 60.0], 100.0, 40.0);
    let mesh = x.obj("Engine.SkeletalMeshComponent");
    for f in ["BlockZeroExtent", "CollideActors"] {
        x.set(mesh, f, Value::Bool(true));
    }
    x.set(mesh, "Owner", Value::Object(Some(decoy2)));
    x.set(decoy2, "CollisionComponent", Value::Object(Some(mesh)));
    // A pawn, a stationary pawn, and a WillowPlayerPawn, all on the line.
    x.colliding("Engine.Pawn", [40.0, 0.0, 50.0], 50.0, 50.0);
    let (still, _) = x.colliding("Engine.Pawn", [80.0, 0.0, 40.0], 50.0, 50.0);
    x.set(still, "bStationary", Value::Bool(true));
    x.colliding("WillowGame.WillowPlayerPawn", [100.0, 0.0, 20.0], 50.0, 50.0);
    assert_eq!(line(&x, &mut host, hud, target).unwrap(), None);
    assert_eq!(host.inputs, [LINE_INPUT]);
}

/// Section 6, tests 2, 4, 5 and 6, and (f): what can block is tested, unless the HUD's chain, the target's Base or the
/// bounds pass it over.
#[test]
fn world_geometry_and_movers_are_tested() {
    let refused = |r: Result<Option<ObjRef>, VmError>| -> String {
        let (m, what) = not_yet(r);
        assert_eq!(m, Missing::NotRead);
        assert!(what.starts_with(LINE_ACTOR), "{what}");
        what
    };
    // A pickup with bWorldGeometry set: tested, the hash admission next.
    let (mut x, hud, target, mut host) = line_world();
    let (p, _) = x.colliding("WillowGame.WillowPickup", [60.0, 0.0, 40.0], 20.0, 20.0);
    x.set(p, "bWorldGeometry", Value::Bool(true));
    assert!(refused(line(&x, &mut host, hud, target)).contains(ow_world::trace_filter::HASH_ADMISSION));
    // A mover on the line: tested; with bounds disjoint from the line's box: passed over.
    let (mut x, hud, target, mut host) = line_world();
    x.colliding("Engine.InterpActor", [60.0, 0.0, 40.0], 20.0, 20.0);
    refused(line(&x, &mut host, hud, target));
    let (mut x, hud, target, mut host) = line_world();
    x.colliding("Engine.InterpActor", [60.0, 200.0, 40.0], 20.0, 20.0);
    assert_eq!(line(&x, &mut host, hud, target).unwrap(), None);
    // A world-geometry StaticMeshActor that is the target's Base: kept; not the Base: tested.
    let (mut x, hud, target, mut host) = line_world();
    let (m, _) = x.colliding("Engine.StaticMeshActor", [60.0, 0.0, 40.0], 20.0, 20.0);
    x.set(m, "bWorldGeometry", Value::Bool(true));
    x.set(target, "Base", Value::Object(Some(m)));
    assert_eq!(line(&x, &mut host, hud, target).unwrap(), None);
    x.set(target, "Base", Value::Object(None));
    refused(line(&x, &mut host, hud, target));
    // The HUD's own controller holding a cylinder on the line: the source's Owner chain.
    let (mut x, hud, target, mut host) = line_world();
    let controller = x.controller;
    let c = x.obj("Engine.CylinderComponent");
    for f in ["BlockZeroExtent", "CollideActors"] {
        x.set(c, f, Value::Bool(true));
    }
    x.set(c, "Owner", Value::Object(Some(controller)));
    x.set(controller, "CollisionComponent", Value::Object(Some(c)));
    x.set(controller, "bWorldGeometry", Value::Bool(true));
    x.set(controller, "bCollideActors", Value::Bool(true));
    x.vector(controller, "Location", [60.0, 0.0, 40.0]);
    x.set(c, "CollisionRadius", Value::Float(20.0));
    x.set(c, "CollisionHeight", Value::Float(20.0));
    x.w.actors().borrow_mut().place(controller).unwrap();
    assert_eq!(line(&x, &mut host, hud, target).unwrap(), None);
    // The same controller not the HUD's Owner: tested, the hash admission.
    x.set(hud, "Owner", Value::Object(None));
    assert!(refused(line(&x, &mut host, hud, target)).contains(ow_world::trace_filter::HASH_ADMISSION));
    // A bPawnsOnly volume: the source's volume test.
    let (mut x, hud, target, mut host) = line_world();
    let (v, _) = x.colliding("Engine.Volume", [60.0, 0.0, 40.0], 20.0, 20.0);
    x.set(v, "bPawnsOnly", Value::Bool(true));
    assert!(refused(line(&x, &mut host, hud, target)).contains(ow_world::trace_filter::SOURCE_VOLUME_TEST));
    // (f) A pickup subclass with no read filter row: NotRead, not skipped.
    let (mut x, hud, target, mut host) = line_world();
    x.colliding("WillowGame.ToyPickupChild", [60.0, 0.0, 40.0], 20.0, 20.0);
    assert!(refused(line(&x, &mut host, hud, target)).contains("the trace filter of WillowGame.ToyPickupChild"));
}

/// NATIVE_LINK7_LINE_CHECK.md section 6, tests 4 and 4a (review r1 of #814): a mover whose only blocking primitive is
/// in `Components` is the hash admission's `NotRead`; a NaN bounds centre is malformed; an unloaded `Owner` link that
/// decides a candidate is the Owner-chain link's `NotRead`, nothing loaded; a mistyped `bWorldGeometry` is a script
/// error; a terrain is the terrain handling's `NotRead`; a `Pawn` subclass with no read row is "the trace filter of"
/// it.
#[test]
fn the_fixed_note_s_section_6_4a_cases() {
    use ow_world::trace_filter::{HASH_ADMISSION, OWNER_CHAIN_LINK, TERRAIN_HANDLING};
    // A mover whose CollisionComponent is None and whose only primitive is in Components.
    let (mut x, hud, target, mut host) = line_world();
    let (mover, c) = x.colliding("Engine.InterpActor", [60.0, 0.0, 40.0], 20.0, 20.0);
    x.set(mover, "CollisionComponent", Value::Object(None));
    x.set(mover, "Components", Value::array(vec![Value::Object(Some(c))]));
    assert!(not_yet(line(&x, &mut host, hud, target)).1.contains(HASH_ADMISSION));
    // A NaN bounds centre, far away on Y: malformed, not cleared.
    let (mut x, hud, target, mut host) = line_world();
    let (m, _) = x.colliding("Engine.StaticMeshActor", [f32::NAN, 900.0, 40.0], 20.0, 20.0);
    x.set(m, "bWorldGeometry", Value::Bool(true));
    assert!(not_yet(line(&x, &mut host, hud, target)).1.starts_with(LINE_MALFORMED));
    // The HUD's Owner a slotless stand-in: the chain is undecided, so a world-geometry candidate names the link.
    let (mut x, hud, target, mut host) = line_world();
    let class = x.w.vm.find_class("Engine.Actor").unwrap();
    let lazy = x.w.vm.space.alloc_stand_in(class, "Lazy", Origin::Default { class }).unwrap();
    x.set(hud, "Owner", Value::Object(Some(lazy)));
    let (m, _) = x.colliding("Engine.StaticMeshActor", [60.0, 0.0, 40.0], 20.0, 20.0);
    x.set(m, "bWorldGeometry", Value::Bool(true));
    assert!(not_yet(line(&x, &mut host, hud, target)).1.contains(OWNER_CHAIN_LINK));
    // A mistyped bWorldGeometry (an int in the bool's slot): a script error.
    let (mut x, hud, target, mut host) = line_world();
    let (m, _) = x.colliding("Engine.StaticMeshActor", [60.0, 0.0, 40.0], 20.0, 20.0);
    let p = x.w.vm.space.prop(x.w.vm.space.heap.get(m).unwrap().class, "bWorldGeometry").unwrap();
    *x.w.vm.space.slot_mut(m, PropAddr::scalar(p)).unwrap() = Value::Int(1);
    assert!(matches!(host.line_blocker_for(&x.w.vm, FROM, TO, hud, target), Err(VmError::Script { .. })));
    // A terrain on the line, and a Pawn subclass with no read row.
    let (mut x, hud, target, mut host) = line_world();
    x.colliding("Engine.Terrain", [60.0, 0.0, 40.0], 20.0, 20.0);
    assert!(not_yet(line(&x, &mut host, hud, target)).1.contains(TERRAIN_HANDLING));
    let (mut x, hud, target, mut host) = line_world();
    x.colliding("Engine.ToyPawnChild", [60.0, 0.0, 40.0], 20.0, 20.0);
    assert!(not_yet(line(&x, &mut host, hud, target)).1.contains("the trace filter of Engine.ToyPawnChild"));
}

/// The review of #818, items 1 to 6 and 8: overflowing box limits are malformed, even when another axis would clear;
/// a primitive a terrain lists but an exact pickup owns refuses on the terrain (the hub's ruling: #514's rule, a terrain
/// on either side, the conservative answer, UNVERIFIED); a mover's static mesh, whose
/// bounds the rule does not answer, is the hash admission's refusal; an exact WillowPlayerPawn is passed over whatever
/// the chain says; `Components` and `CollisionComponent` must be declared with their kinds; a slotless entry of a
/// non-primitive class is skipped; an actor whose Owner is the HUD is passed over (the reverse walk).
#[test]
fn the_review_of_818_cases() {
    use ow_world::trace_filter::HASH_ADMISSION;
    // 1. A centre and radius each finite whose sum overflows; the box is far away on Z.
    let (mut x, hud, target, mut host) = line_world();
    let (m, c) = x.colliding("Engine.StaticMeshActor", [3.0e38, 0.0, 9000.0], 20.0, 20.0);
    x.set(m, "bWorldGeometry", Value::Bool(true));
    x.set(c, "CollisionRadius", Value::Float(3.0e38));
    assert!(not_yet(line(&x, &mut host, hud, target)).1.starts_with(LINE_MALFORMED));
    // 2. A terrain lists a cylinder an exact WillowPickup owns: the terrain handling refuses (#514's rule, kept by the
    // hub's ruling on the #818 fix: no note reads the game's answer, so the conservative one, UNVERIFIED).
    let (mut x, hud, target, mut host) = line_world();
    let pickup = x.obj("WillowGame.WillowPickup");
    x.set(pickup, "bCollideActors", Value::Bool(true));
    let (_, c) = x.colliding("Engine.Terrain", [60.0, 0.0, 40.0], 20.0, 20.0);
    x.set(c, "Owner", Value::Object(Some(pickup)));
    let (m, what) = not_yet(line(&x, &mut host, hud, target));
    assert_eq!(m, Missing::NotRead);
    assert!(what.contains(ow_world::trace_filter::TERRAIN_HANDLING), "{what}");
    // 3. A mover on the line whose primitive is a static mesh (bounds not answered): the hash admission, not a layer.
    let (mut x, hud, target, mut host) = line_world();
    let (mover, _) = x.colliding("Engine.InterpActor", [60.0, 0.0, 40.0], 20.0, 20.0);
    let mesh = x.obj("Engine.StaticMeshComponent");
    for f in ["BlockZeroExtent", "CollideActors"] {
        x.set(mesh, f, Value::Bool(true));
    }
    x.set(mesh, "Owner", Value::Object(Some(mover)));
    x.set(mover, "CollisionComponent", Value::Object(Some(mesh)));
    let (m, what) = not_yet(line(&x, &mut host, hud, target));
    assert_eq!(m, Missing::NotRead);
    assert!(what.contains(HASH_ADMISSION), "{what}");
    // 4. The HUD's Owner a slotless stand-in, and an exact WillowPlayerPawn on the line: passed over.
    let (mut x, hud, target, mut host) = line_world();
    let class = x.w.vm.find_class("Engine.Actor").unwrap();
    let lazy = x.w.vm.space.alloc_stand_in(class, "Lazy", Origin::Default { class }).unwrap();
    x.set(hud, "Owner", Value::Object(Some(lazy)));
    x.colliding("WillowGame.WillowPlayerPawn", [60.0, 0.0, 40.0], 50.0, 50.0);
    assert_eq!(line(&x, &mut host, hud, target).unwrap(), None);
    // 5. Components declared as an int, and an actor that declares neither field: data errors.
    for class in ["Engine.ToyOddActor", "Engine.ToyBareActor"] {
        let (mut x, hud, target, mut host) = line_world();
        let odd = x.obj(class);
        x.w.actors().borrow_mut().place(odd).unwrap();
        assert!(matches!(host.line_blocker_for(&x.w.vm, FROM, TO, hud, target), Err(VmError::Script { .. })), "{class}");
    }
    // 6. A slotless entry of a non-primitive class in Components: skipped.
    let (mut x, hud, target, mut host) = line_world();
    let a = x.obj("Engine.Actor");
    let class = x.w.vm.find_class("WillowGame.HUDDefinition").unwrap();
    let slotless = x.w.vm.space.alloc_stand_in(class, "Slotless", Origin::Default { class }).unwrap();
    x.set(a, "Components", Value::array(vec![Value::Object(Some(slotless))]));
    x.w.actors().borrow_mut().place(a).unwrap();
    assert_eq!(line(&x, &mut host, hud, target).unwrap(), None);
    // 8. An actor whose Owner is the HUD (the reverse walk), world geometry on the line: passed over.
    let (mut x, hud, target, mut host) = line_world();
    let (owned, _) = x.colliding("Engine.StaticMeshActor", [60.0, 0.0, 40.0], 20.0, 20.0);
    x.set(owned, "bWorldGeometry", Value::Bool(true));
    x.set(owned, "Owner", Value::Object(Some(hud)));
    assert_eq!(line(&x, &mut host, hud, target).unwrap(), None);
}

/// (a): a primitive held only in `Components` is walked too; (c) malformed values; (d) an unloaded part is named.
#[test]
fn components_malformed_values_and_unloaded_parts() {
    // A world-geometry actor whose primitive is only in Components.
    let (mut x, hud, target, mut host) = line_world();
    let a = x.obj("Engine.Actor");
    x.vector(a, "Location", [60.0, 0.0, 40.0]);
    x.set(a, "bWorldGeometry", Value::Bool(true));
    x.set(a, "bCollideActors", Value::Bool(true));
    let c = x.obj("Engine.CylinderComponent");
    x.set(c, "CollisionRadius", Value::Float(20.0));
    x.set(c, "CollisionHeight", Value::Float(20.0));
    for f in ["BlockZeroExtent", "CollideActors"] {
        x.set(c, f, Value::Bool(true));
    }
    x.set(c, "Owner", Value::Object(Some(a)));
    x.set(a, "Components", Value::array(vec![Value::Object(Some(c))]));
    x.w.actors().borrow_mut().place(a).unwrap();
    assert!(not_yet(line(&x, &mut host, hud, target)).1.starts_with(LINE_ACTOR), "walked from Components");
    // A negative candidate radius, far away: malformed, not cleared.
    let (mut x, hud, target, mut host) = line_world();
    let (m, c) = x.colliding("Engine.StaticMeshActor", [60.0, 900.0, 40.0], 20.0, 20.0);
    x.set(m, "bWorldGeometry", Value::Bool(true));
    x.set(c, "CollisionRadius", Value::Float(-5.0));
    assert!(not_yet(line(&x, &mut host, hud, target)).1.starts_with(LINE_MALFORMED));
    // A NaN endpoint.
    let (x, hud, target, mut host) = line_world();
    let r = host.line_blocker_for(&x.w.vm, [f32::NAN, 0.0, 75.0], TO, hud, target);
    assert_eq!(not_yet(r), (Missing::NotRead, LINE_MALFORMED.to_string()));
    // A live actor's part the store has not loaded: named, nothing loaded.
    let (mut x, hud, target, mut host) = line_world();
    let a = x.obj("Engine.Actor");
    let class = x.w.vm.find_class("Engine.CylinderComponent").unwrap();
    let lazy = x.w.vm.space.alloc_stand_in(class, "Lazy", Origin::Default { class }).unwrap();
    x.set(a, "CollisionComponent", Value::Object(Some(lazy)));
    x.w.actors().borrow_mut().place(a).unwrap();
    assert!(not_yet(line(&x, &mut host, hud, target)).1.starts_with(LINE_UNLOADED));
}

// Test 7: Reference.
#[test]
fn reference_refuses_the_setting() {
    let mut r = fx_in(ParityMode::Reference);
    assert!(matches!(enable_hud_targeting_harness(r.vm()), Err(VmError::Registration { .. })));
    assert!(matches!(crate::pickup_targeting::enable_no_level_line_check(r.vm()), Err(VmError::Registration { .. })));
    let controller = r.controller;
    assert!(matches!(harness_hud(r.vm(), controller), Err(VmError::Registration { .. })));
    assert!(!r.w.vm.log.iter().any(|l| l == SETTING_INPUT));
}

/// Review of #810, item 3: the harness HUD checks its own conditions. With the setting off it answers as the headless
/// world does (the no-HUD `LayerAbsent`), nothing made; in a world that has begun play it is the `NotRead` before any
/// write.
#[test]
fn the_harness_hud_checks_the_setting_and_begun_play_itself() {
    let mut x = fx();
    let controller = x.controller;
    let before = x.counters();
    assert!(matches!(not_yet(harness_hud(x.vm(), controller)).0, Missing::LayerAbsent(_)), "setting off");
    assert_eq!(x.counters(), before);
    assert_eq!(x.get(controller, "myHUD"), Value::Object(None));
    enable_hud_targeting_harness(x.vm()).unwrap();
    x.w.actors().borrow_mut().begin_play();
    let before = x.counters();
    assert_eq!(not_yet(harness_hud(x.vm(), controller)), (Missing::NotRead, BEGUN_PLAY.to_string()));
    assert_eq!(x.counters(), before, "nothing written");
    assert_eq!(x.get(controller, "myHUD"), Value::Object(None));
}

/// Review r2 of #810, item 4: a pawn `Location` without a `Z` field (here a `Rotator` value in its slot) is a data
/// error (`VmError::Script`), as a malformed `Rotation` is.
#[test]
fn a_location_without_z_is_a_data_error() {
    let mut x = fx();
    let (controller, pawn) = (x.controller, x.pawn);
    x.set(pawn, "BaseEyeHeight", Value::Float(64.0));
    let rotation = x.get(controller, "Rotation");
    {
        let vm = x.vm();
        let class = vm.space.heap.get(pawn).unwrap().class;
        let p = vm.space.prop(class, "Location").unwrap();
        *vm.space.slot_mut(pawn, PropAddr::scalar(p)).unwrap() = rotation;
    }
    enable_hud_targeting_harness(x.vm()).unwrap();
    let mut host = HarnessHost::for_world(&x.w.vm);
    match host.view(&x.w.vm, controller) {
        Err(VmError::Script { message, .. }) => assert!(message.contains("Location.X"), "{message}"),
        other => panic!("{other:?}"),
    }
}
