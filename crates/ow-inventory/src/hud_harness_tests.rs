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
    for b in ["bStatic", "bNoDelete", "bDeleteMe", "bPendingDelete", "bCollideWorld", "bCollideWhenPlacing", "bCollideActors", "bNoEncroachCheck"] {
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
    for b in ["AbsoluteTranslation", "AbsoluteRotation", "AbsoluteScale"] {
        t.prop("Bool", primitive, b, 0, 0);
    }
    let cylinder = t.class("CylinderComponent", primitive);
    t.prop("Float", cylinder, "CollisionRadius", 0, 0);
    t.prop("Float", cylinder, "CollisionHeight", 0, 0);
    t.class("SkeletalMeshComponent", primitive);
    let pawn = t.class("Pawn", actor);
    t.prop("Float", pawn, "BaseEyeHeight", 0, 0);
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
    /// A placed actor whose collision is a cylinder of `radius` and half height `half` at `at`.
    fn cylinder_actor(&mut self, at: [f32; 3], radius: f32, half: f32) -> ObjRef {
        let a = self.obj("Engine.Actor");
        self.vector(a, "Location", at);
        let c = self.obj("Engine.CylinderComponent");
        self.set(c, "CollisionRadius", Value::Float(radius));
        self.set(c, "CollisionHeight", Value::Float(half));
        self.set(c, "Owner", Value::Object(Some(a)));
        self.set(a, "CollisionComponent", Value::Object(Some(c)));
        self.w.actors().borrow_mut().place(a).unwrap();
        a
    }
}

fn not_yet(r: Result<impl std::fmt::Debug, VmError>) -> (Missing, String) {
    match r {
        Err(VmError::NotYet(n)) => (n.missing.clone(), n.what.to_string()),
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

// Test 4: the line check under NoLevel.
#[test]
fn the_line_check_under_no_level() {
    let mut x = fx();
    let pawn = x.pawn;
    let target = x.obj("WillowGame.WillowPickup");
    let (from, to) = ([125.0, 0.0, 0.0], [0.0, 0.0, 75.0]);
    // Off: the headless LayerAbsent.
    let mut off = HarnessHost::for_world(&x.w.vm);
    assert!(matches!(not_yet(off.line_blocker_for(&x.w.vm, from, to, pawn, target)).0, Missing::LayerAbsent(_)));
    crate::pickup_targeting::enable_no_level_line_check(x.vm()).unwrap();
    assert!(x.w.vm.log.iter().any(|l| l == crate::pickup_targeting::NO_LEVEL_LINE_INPUT));
    assert!(matches!(crate::pickup_targeting::enable_no_level_line_check(x.vm()), Err(VmError::Registration { .. })), "twice");
    // On, no collision service: still the headless LayerAbsent.
    let mut host = HarnessHost::for_world(&x.w.vm);
    assert!(matches!(not_yet(host.line_blocker_for(&x.w.vm, from, to, pawn, target)).0, Missing::LayerAbsent(_)));
    ow_world::placement::install_collision(x.vm(), Rc::new(ow_world::placement::NoLevel)).unwrap();
    // NoLevel, a cylinder 45 to the side (radius 20) and one well above: their bounds boxes are disjoint from the
    // line's box, so cleared; no hit, one line.
    x.cylinder_actor([60.0, 45.0, 0.0], 20.0, 30.0);
    x.cylinder_actor([60.0, 0.0, 500.0], 20.0, 30.0);
    // The target, its Base and the owner's pawn are ignored even on the line.
    let base = x.cylinder_actor([125.0, 0.0, 0.0], 50.0, 50.0);
    x.set(target, "Base", Value::Object(Some(base)));
    // The owner's pawn, placed on the line with collision of its own.
    let own = x.obj("Engine.CylinderComponent");
    x.set(own, "CollisionRadius", Value::Float(50.0));
    x.set(own, "CollisionHeight", Value::Float(50.0));
    x.set(own, "Owner", Value::Object(Some(pawn)));
    x.set(pawn, "CollisionComponent", Value::Object(Some(own)));
    x.vector(pawn, "Location", [0.0, 0.0, 75.0]);
    x.w.actors().borrow_mut().place(pawn).unwrap();
    let mut host = HarnessHost::for_world(&x.w.vm);
    assert_eq!(host.line_blocker_for(&x.w.vm, from, to, pawn, target).unwrap(), None);
    assert_eq!(host.inputs, [LINE_INPUT]);
    // A cylinder whose bounds box meets the line's box: NotRead.
    let mut x2 = fx();
    crate::pickup_targeting::enable_no_level_line_check(x2.vm()).unwrap();
    ow_world::placement::install_collision(x2.vm(), Rc::new(ow_world::placement::NoLevel)).unwrap();
    let (pawn2, target2) = (x2.pawn, x2.obj("WillowGame.WillowPickup"));
    // (Its bounds by the rule: the toy actor's own body answers a point, on the line here.)
    x2.cylinder_actor([60.0, 0.0, 30.0], 20.0, 30.0);
    let mut host = HarnessHost::for_world(&x2.w.vm);
    let (missing, what) = not_yet(host.line_blocker_for(&x2.w.vm, from, to, pawn2, target2));
    assert_eq!(missing, Missing::NotRead);
    assert!(what.starts_with(LINE_ACTOR), "{what}");
    // Another actor holding collision whose bounds box meets the line's box (here on the line itself): NotRead.
    let mut x3 = fx();
    crate::pickup_targeting::enable_no_level_line_check(x3.vm()).unwrap();
    ow_world::placement::install_collision(x3.vm(), Rc::new(ow_world::placement::NoLevel)).unwrap();
    let (pawn3, target3) = (x3.pawn, x3.obj("WillowGame.WillowPickup"));
    let far = x3.obj("Engine.Actor");
    x3.vector(far, "Location", [60.0, 0.0, 30.0]);
    let mesh = x3.obj("Engine.SkeletalMeshComponent");
    x3.set(far, "CollisionComponent", Value::Object(Some(mesh)));
    x3.w.actors().borrow_mut().place(far).unwrap();
    let mut host = HarnessHost::for_world(&x3.w.vm);
    assert!(not_yet(host.line_blocker_for(&x3.w.vm, from, to, pawn3, target3)).1.starts_with(LINE_ACTOR));
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
