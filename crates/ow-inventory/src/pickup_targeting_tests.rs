//! Tests of the pickup targeting (NATIVE_PICKUP_TARGETING.md; lane IMPL-PICKUP-TARGETING, decision 2026-11-19-81) on
//! two invented packages built with `ow_package::testkit` (AGENTS.md rule 9). `Engine` has the actors, the HUD and the
//! inventory with its native `GetInventoryDefinition`. `WillowGame` has the player pawn and controller (the two events,
//! as natives that record their calls), the Willow HUD with its slots, the globals with their pickup list and native
//! `GetGlobalsDefinition`, the pickup and the two definitions. Every value is invented. The view, the bounds and the
//! line check come from a test [`Host`]; [`Headless`] is tested on its own.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::{Missing, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_object::ids::PropAddr;
use ow_package::testkit::reflect::{toy_array_of, toy_struct};
use ow_package::testkit::script::{Asm, FunctionSpec, Param, ScriptToy};
use ow_vm::api::{NativeCall, StubPolicy};

use super::*;

const NATIVE: u32 = 0x400 | 0x20000;
static NEXT: AtomicU32 = AtomicU32::new(0);

const IMPORTS: [&str; 10] = [
    "Class",
    "Function",
    "ScriptStruct",
    "ObjectProperty",
    "BoolProperty",
    "IntProperty",
    "FloatProperty",
    "StructProperty",
    "ArrayProperty",
    "ByteProperty",
];

fn native(t: &mut ScriptToy, owner: i32, name: &str, params: &[Param], result: Option<(&str, i32)>) {
    t.function(owner, name, params, result, &[], &FunctionSpec::new(NATIVE, name), |_| None);
}

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
    t.prop("Struct", actor, "Location", 0, vector);
    t.prop("Struct", actor, "Rotation", 0, rotator);
    t.prop("Object", actor, "Base", 0, actor);
    t.prop("Bool", actor, "bDeleteMe", 0, 0);
    t.prop("Float", actor, "LifeSpan", 0, 0);
    // The shrink step's reads (NATIVE_LINK7_HUD_TARGETING.md section 7): the HUD's WorldInfo and its NetMode, the
    // owner's Player.
    let world_info = t.class("WorldInfo", actor);
    t.prop("Byte", world_info, "NetMode", 0, 0);
    t.prop("Object", actor, "WorldInfo", 0, world_info);
    let pawn = t.class("Pawn", actor);
    t.prop("Object", pawn, "DrivenVehicle", 0, actor);
    t.prop("Bool", pawn, "bIsInjured", 0, 0);
    t.prop("Bool", pawn, "bIsDead", 0, 0);
    let controller = t.class("Controller", actor);
    t.prop("Object", controller, "Pawn", 0, pawn);
    let player = t.class("PlayerController", controller);
    let hud = t.class("HUD", actor);
    t.prop("Object", player, "myHUD", 0, hud);
    t.prop("Object", player, "Player", 0, object);
    t.prop("Object", hud, "PlayerOwner", 0, player);
    let dropped = t.class("DroppedPickup", actor);
    let inventory = t.class("WillowInventory", actor);
    t.prop("Object", dropped, "Inventory", 0, inventory);
    native(&mut t, inventory, "GetInventoryDefinition", &[], Some(("Object", object)));
    let definition = t.class("WillowInventoryDefinition", object);
    t.prop("Struct", definition, "PickupFlagOffset", 0, vector);
    t.build()
}

fn willow_game() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let object = t.p.add_import("Core", "Class", engine, "Object");
    let actor = t.p.add_import("Core", "Class", engine, "Actor");
    let pawn = t.p.add_import("Core", "Class", engine, "Pawn");
    let player = t.p.add_import("Core", "Class", engine, "PlayerController");
    let hud = t.p.add_import("Core", "Class", engine, "HUD");
    let dropped = t.p.add_import("Core", "Class", engine, "DroppedPickup");
    let inventory = t.p.add_import("Core", "Class", engine, "WillowInventory");
    // The weapon's definition data, with the field GetInventoryDefinition's rule reads for a weapon.
    let weapon = t.class("WillowWeapon", inventory);
    let data = toy_struct(&mut t, weapon, "WeaponDefinitionData", None);
    t.prop("Object", data, "WeaponTypeDefinition", 0, object);
    t.prop("Struct", weapon, "DefinitionData", 0, data);
    // A weapon subclass that overrides the getter in script (review r1 of #467).
    let overriding = t.class("ToyOverridingWeapon", weapon);
    t.function(
        overriding,
        "GetInventoryDefinition",
        &[],
        Some(("Object", object)),
        &[],
        &FunctionSpec::new(0x1 | 0x2 | 0x20000, "GetInventoryDefinition"),
        |_| {
            let mut a = Asm::new();
            a.stmt().return_nothing();
            a.end();
            Some(a)
        },
    );
    // An allowlisted item class declared without DefinitionData.
    t.class("WillowItem", inventory);
    // A player controller with neither handler.
    t.class("ToyMuteController", player);
    let player_pawn = t.class("WillowPlayerPawn", pawn);
    t.class("ToyPlayerPawnSubclass", player_pawn);
    let controller = t.class("WillowPlayerController", player);
    native(&mut t, controller, "SawPickupable", &[Param { type_ref: object, ..Param::new("Object", "Pickup") }], None);
    native(&mut t, controller, "ClearSeenPickupable", &[], None);
    let pickup = t.class("WillowPickup", dropped);
    for b in ["bPickupable", "bHasPickupParticleOffset"] {
        t.prop("Bool", pickup, b, 0, 0);
    }
    t.prop("Float", pickup, "PickupShrinkDuration", 0, 0);
    native(&mut t, pickup, "BeginShrinking", &[], None);
    let hud_definition = t.class("HUDDefinition", object);
    t.prop("Float", hud_definition, "PickupRange", 0, 0);
    let willow_hud = t.class("WillowHUD", hud);
    t.prop("Object", willow_hud, "WPlayerOwner", 0, player);
    t.prop("Object", willow_hud, "HUDDef", 0, hud_definition);
    for o in ["CachedHoverPickup", "CachedBestPickup", "ClosestPickup"] {
        t.prop("Object", willow_hud, o, 0, pickup);
    }
    t.prop("Object", willow_hud, "SavedLookAtInventory", 0, actor);
    t.prop("Int", willow_hud, "MaxNumPickups", 0, 0);
    let globals = t.class("WillowGlobals", object);
    toy_array_of(&mut t, globals, "PickupList", "Object", pickup);
    t.prop("Object", globals, "TheGlobalsDefinition", 0, object);
    t.prop("Object", globals, "ToyDefinition", 0, object);
    native(&mut t, globals, "GetGlobalsDefinition", &[], Some(("Object", object)));
    let gd = t.class("GlobalsDefinition", object);
    t.prop("Object", gd, "PartItemHover", 0, object);
    t.prop("Float", gd, "HoverWidthPickup", 0, 0);
    t.prop("Float", gd, "ItemHoverProjectionHeight", 0, 0);
    t.build()
}

/// What the event natives saw (each with `S` as the HUD held it at the call), and whether `SawPickupable` refuses.
#[derive(Default)]
struct Events {
    seen: Vec<(String, Option<ObjRef>)>,
    refuse: bool,
    /// Whether the toy `BeginShrinking` refuses (review of #810, item 1).
    refuse_shrink: bool,
    hud: Option<ObjRef>,
}

/// The test's view, bounds and line check, recording which pickups reached the bounds query.
struct Fixed {
    view: View,
    height: f32,
    blocker: Option<ObjRef>,
    traced: u32,
    bounded: Vec<ObjRef>,
}

impl Host for Fixed {
    fn view(&mut self, _: &Vm, _: ObjRef) -> Result<View, VmError> {
        Ok(self.view)
    }
    fn bounds_height(&mut self, _: &Vm, p: ObjRef) -> Result<f32, VmError> {
        self.bounded.push(p);
        Ok(self.height)
    }
    fn line_blocker(&mut self, _: &Vm, _: [f32; 3], _: [f32; 3]) -> Result<Option<ObjRef>, VmError> {
        self.traced += 1;
        Ok(self.blocker)
    }
}

/// The toy globals and their definition; `resolved` counts the calls (the pass asks only when it reaches the list).
struct ToyGlobals {
    globals: ObjRef,
    definition: ObjRef,
    resolved: std::cell::Cell<u32>,
}

impl Globals for ToyGlobals {
    fn resolve(&self, _: &Vm) -> Result<(ObjRef, ObjRef), VmError> {
        self.resolved.set(self.resolved.get() + 1);
        Ok((self.globals, self.definition))
    }
}

struct Fx {
    vm: Vm,
    dir: PathBuf,
    events: Rc<RefCell<Events>>,
    globals: ObjRef,
    gd: ObjRef,
    hud: ObjRef,
    hud_definition: ObjRef,
    controller: ObjRef,
    pawn: ObjRef,
    host: Fixed,
    provider: ToyGlobals,
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn make(vm: &mut Vm, class: &str) -> ObjRef {
    let c = vm.find_class(class).unwrap();
    ow_vm::interp::instantiate_from_defaults(vm, c).unwrap()
}

fn reported_now(vm: &Vm, hud: Option<ObjRef>) -> Option<ObjRef> {
    hud.and_then(|h| object_field(vm, h, "CachedBestPickup").unwrap())
}

/// The world: Maya's controller and pawn and her HUD (owner, recipient and definition set), the globals with an empty
/// pickup list, PickupRange 300, HoverWidthPickup 100 (a capture radius of 10), ItemHoverProjectionHeight 20, the
/// pickup default's PickupShrinkDuration 5. The view looks along +X from the origin; bounds 40 high.
fn fx() -> Fx {
    fx_with(true)
}

/// The fixture; with `warm_player_pawn` false, `WillowPlayerPawn` is never looked up and the controller's pawn is a
/// cold `ToyPlayerPawnSubclass` (its layout, and so its parents', through the code index; no class code loaded), for
/// the cold-pawn case of #701 (lane COLD-LOAD-FIXES).
fn fx_with(warm_player_pawn: bool) -> Fx {
    let dir = std::env::temp_dir().join(format!(
        "ow-inventory-targeting-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let (e, w) = (engine(), willow_game());
    let defs = DefStore::from_synthetic(
        &dir,
        &[("Engine.upk", e.as_slice()), ("WillowGame.upk", w.as_slice())],
        IdentityPolicy::FirstInScanOrder,
        ParityMode::Off,
    )
    .unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off)
        .with_stub_policy(StubPolicy::refuse_all());
    for c in ["Engine.WillowInventory", "WillowGame.WillowPlayerController", "WillowGame.WillowGlobals", PICKUP] {
        vm.find_class(c).unwrap();
    }
    let events = Rc::new(RefCell::new(Events::default()));
    let e1 = events.clone();
    vm.register_native(
        "WillowPlayerController.SawPickupable(object)",
        Rc::new(move |c: &mut NativeCall<'_>| {
            if e1.borrow().refuse {
                return Err(NotYet::not_read(format!("the toy SawPickupable ({PARTIAL_WRITES} inside)"), "TOY.md").into());
            }
            let p = object_of(c.arg(0)?, "test")?.unwrap();
            let s = reported_now(c.vm, e1.borrow().hud);
            e1.borrow_mut().seen.push((format!("Saw {p:?}"), s));
            Ok(Value::None)
        }),
    )
    .unwrap();
    let e2 = events.clone();
    vm.register_native(
        "WillowPlayerController.ClearSeenPickupable()",
        Rc::new(move |c: &mut NativeCall<'_>| {
            let s = reported_now(c.vm, e2.borrow().hud);
            e2.borrow_mut().seen.push(("Clear".into(), s));
            Ok(Value::None)
        }),
    )
    .unwrap();
    let e3 = events.clone();
    vm.register_native(
        "WillowPickup.BeginShrinking()",
        Rc::new(move |c: &mut NativeCall<'_>| {
            if e3.borrow().refuse_shrink {
                return Err(NotYet::not_read("the toy BeginShrinking", "TOY.md").into());
            }
            let reported = reported_now(c.vm, e3.borrow().hud);
            e3.borrow_mut().seen.push((format!("Shrink {:?}", c.this.unwrap()), reported));
            Ok(Value::None)
        }),
    )
    .unwrap();
    let globals = make(&mut vm, "WillowGame.WillowGlobals");
    let gd = make(&mut vm, "WillowGame.GlobalsDefinition");
    let hud = make(&mut vm, "WillowGame.WillowHUD");
    let hud_definition = make(&mut vm, "WillowGame.HUDDefinition");
    let controller = make(&mut vm, "WillowGame.WillowPlayerController");
    let pawn = if warm_player_pawn {
        make(&mut vm, "WillowGame.WillowPlayerPawn")
    } else {
        let (code, store) = vm.defs.code_mut();
        let key = code.find_class(store, "WillowGame.ToyPlayerPawnSubclass").unwrap();
        let c = code.define_layout(store, &mut vm.space.names, &mut vm.space.classes, key).unwrap();
        vm.space.alloc(c, "ColdPawn", None, ow_object::heap::Origin::Runtime).unwrap()
    };
    events.borrow_mut().hud = Some(hud);
    let mut x = Fx {
        vm,
        dir,
        events,
        globals,
        gd,
        hud,
        hud_definition,
        controller,
        pawn,
        host: Fixed {
            view: View { location: [0.0; 3], pitch: 0, yaw: 0 },
            height: 40.0,
            blocker: None,
            traced: 0,
            bounded: Vec::new(),
        },
        provider: ToyGlobals { globals, definition: gd, resolved: std::cell::Cell::new(0) },
    };
    x.set(gd, "HoverWidthPickup", Value::Float(100.0));
    x.set(gd, "ItemHoverProjectionHeight", Value::Float(20.0));
    x.set(hud_definition, "PickupRange", Value::Float(300.0));
    x.set(controller, "Pawn", Value::Object(Some(pawn)));
    x.set(controller, "myHUD", Value::Object(Some(hud)));
    x.set(hud, "PlayerOwner", Value::Object(Some(controller)));
    x.set(hud, "WPlayerOwner", Value::Object(Some(controller)));
    x.set(hud, "HUDDef", Value::Object(Some(hud_definition)));
    // The shrink step: a server world and a limit of 200 (the installed class default; invented here).
    let info = x.obj("Engine.WorldInfo");
    x.set(hud, "WorldInfo", Value::Object(Some(info)));
    x.set(hud, "MaxNumPickups", Value::Int(200));
    let defaults = x.vm.defs.class_default(&mut x.vm.space, PICKUP).unwrap();
    x.set(defaults, "PickupShrinkDuration", Value::Float(5.0));
    x
}

impl Fx {
    fn obj(&mut self, class: &str) -> ObjRef {
        make(&mut self.vm, class)
    }

    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap();
        self.vm.space.write(o, PropAddr::scalar(p), v).unwrap();
    }

    fn get(&self, o: ObjRef, name: &str) -> Value {
        required(&self.vm, o, name, "test").unwrap()
    }

    fn slot(&self, name: &str) -> Option<ObjRef> {
        object_field(&self.vm, self.hud, name).unwrap()
    }

    /// `H` and `S`.
    fn slots(&self) -> (Option<ObjRef>, Option<ObjRef>) {
        (self.slot("CachedHoverPickup"), self.slot("CachedBestPickup"))
    }

    fn set_slots(&mut self, hover: Option<ObjRef>, reported: Option<ObjRef>) {
        self.set(self.hud, "CachedHoverPickup", Value::Object(hover));
        self.set(self.hud, "CachedBestPickup", Value::Object(reported));
    }

    /// `o`'s vector `name` with X, Y and Z set to `at`, field by name (the layout's order is the package's).
    fn vector(&self, o: ObjRef, name: &str, at: [f32; 3]) -> Value {
        let mut v = self.get(o, name);
        let ty = v.as_struct().unwrap().ty;
        for (axis, x) in ["X", "Y", "Z"].into_iter().zip(at) {
            let id = self.vm.space.names.find(axis).unwrap();
            let (index, _) = self.vm.space.classes.struct_layout(ty).unwrap().field(id).unwrap();
            v.struct_mut().unwrap().fields[usize::try_from(index).unwrap()] = Value::Float(x);
        }
        assert_eq!(vector_of(&self.vm, &v).unwrap(), at);
        v
    }

    /// A pickable pickup at `at` holding an item whose definition has `offset` as its `PickupFlagOffset`.
    fn pickup(&mut self, at: [f32; 3], offset: [f32; 3]) -> ObjRef {
        let p = self.obj(PICKUP);
        let v = self.vector(p, "Location", at);
        self.set(p, "Location", v);
        self.set(p, "bPickupable", Value::Bool(true));
        let item = self.obj("WillowGame.WillowWeapon");
        let definition = self.obj("Engine.WillowInventoryDefinition");
        let o = self.vector(definition, "PickupFlagOffset", offset);
        self.set(definition, "PickupFlagOffset", o);
        self.set_definition(item, Some(definition));
        self.set(p, "Inventory", Value::Object(Some(item)));
        let mut list = match self.get(self.globals, "PickupList") {
            Value::Array(a) => a.as_ref().clone(),
            other => panic!("{other:?}"),
        };
        list.push(Value::Object(Some(p)));
        self.set(self.globals, "PickupList", Value::array(list));
        p
    }

    /// `item`'s `DefinitionData.WeaponTypeDefinition`.
    fn set_definition(&mut self, item: ObjRef, definition: Option<ObjRef>) {
        let mut data = self.get(item, "DefinitionData");
        let ty = data.as_struct().unwrap().ty;
        let id = self.vm.space.names.find("WeaponTypeDefinition").unwrap();
        let (index, _) = self.vm.space.classes.struct_layout(ty).unwrap().field(id).unwrap();
        data.struct_mut().unwrap().fields[usize::try_from(index).unwrap()] = Value::Object(definition);
        self.set(item, "DefinitionData", data);
    }

    fn tick(&mut self) -> Result<Outcome, VmError> {
        tick(&mut self.vm, self.hud, &self.provider, &mut self.host)
    }

    fn seen(&self) -> Vec<(String, Option<ObjRef>)> {
        self.events.borrow().seen.clone()
    }
}

fn not_yet(r: Result<Outcome, VmError>) -> ow_core::NotYet {
    match r {
        Err(VmError::NotYet(n)) => n,
        other => panic!("expected NotYet, got {other:?}"),
    }
}

/// Link 7's shape (section 11): the aimed gun 150 ahead and a nearer decoy 90 degrees off. The decoy fails the 0.9
/// cone; the aimed gun wins, becomes `H` and is reported once, with `S` still empty while the handler runs. The next
/// tick keeps it and raises nothing.
#[test]
fn the_aimed_pickup_wins_over_the_nearer_decoy_and_is_reported_once() {
    let mut x = fx();
    // A capture radius of 100, so the decoy 45 from the ray would score if the cone did not stop it first.
    x.set(x.gd, "HoverWidthPickup", Value::Float(1000.0));
    let decoy = x.pickup([0.0, 45.0, 0.0], [0.0; 3]);
    let aimed = x.pickup([150.0, 0.0, 0.0], [0.0; 3]);
    assert_eq!(x.tick().unwrap(), Outcome::Hover { winner: aimed, reported: true });
    assert_eq!(x.host.bounded, vec![aimed], "the decoy fails the cone before its bounds are asked");
    assert_eq!(x.slots(), (Some(aimed), Some(aimed)));
    assert_eq!(x.seen(), vec![(format!("Saw {aimed:?}"), None)], "raised before S is written");
    assert_eq!(x.slot("ClosestPickup"), Some(decoy), "the nearest counted pickup with an inventory");
    let item = object_field(&x.vm, aimed, "Inventory").unwrap();
    assert_eq!(x.slot("SavedLookAtInventory"), item);
    assert_eq!(x.tick().unwrap(), Outcome::Hover { winner: aimed, reported: false });
    assert_eq!(x.seen().len(), 1, "not raised again for the same pickup");
}

/// The ranking divides by the distance: of two pickups the ray passes through, the nearer is reported.
#[test]
fn the_nearer_of_two_hit_pickups_wins() {
    let mut x = fx();
    let _far = x.pickup([250.0, 0.0, 0.0], [0.0; 3]);
    let near = x.pickup([120.0, 0.0, 0.0], [0.0; 3]);
    assert_eq!(x.tick().unwrap(), Outcome::Hover { winner: near, reported: true });
}

/// A ray passing the segment further off than the capture radius (10) gives no score: no winner. The current hover
/// pickup's radius is 1.5 times larger (15), so a miss of 12 keeps it. A capture radius of 0 scores 0.
#[test]
fn the_capture_radius_bounds_the_score_and_the_current_pickup_gets_more() {
    let mut x = fx();
    let p = x.pickup([150.0, 12.0, 0.0], [0.0; 3]);
    // The 12 off is inside the 0.9 cone (cosine about 0.997), outside the radius 10.
    assert_eq!(x.tick().unwrap(), Outcome::NoWinner { cleared: false });
    x.set_slots(Some(p), None);
    assert_eq!(x.tick().unwrap(), Outcome::Hover { winner: p, reported: true });
    assert_eq!(score(Some(0.0), 0.0), 0.0);
    assert_eq!(score(Some(5.0), -1.0), 0.0);
    assert_eq!(score(None, 10.0), 0.0);
    assert_eq!(score(Some(2.5), 10.0), 0.75);
}

/// Out of PickupRange with no hover particle: not scored, and a pickup reported earlier is cleared (no winner), the
/// handler running while `S` still holds it. With a hover particle the range does not gate scoring: the pickup is `H`,
/// nothing is raised, `S` keeps its old value and the seen-item slot is emptied.
#[test]
fn the_range_gates_scoring_without_a_hover_particle_and_the_event_always() {
    let mut x = fx();
    let far = x.pickup([400.0, 0.0, 0.0], [0.0; 3]);
    let earlier = x.obj(PICKUP);
    x.set_slots(None, Some(earlier));
    assert_eq!(x.tick().unwrap(), Outcome::NoWinner { cleared: true });
    assert_eq!(x.slots(), (None, None));
    assert_eq!(x.seen(), vec![("Clear".to_string(), Some(earlier))], "raised before S is emptied");
    let particle = x.obj("Engine.Actor");
    x.set(x.gd, "PartItemHover", Value::Object(Some(particle)));
    x.set_slots(None, Some(earlier));
    assert_eq!(x.tick().unwrap(), Outcome::Hover { winner: far, reported: false });
    assert_eq!(x.slots(), (Some(far), Some(earlier)));
    assert_eq!(x.slot("SavedLookAtInventory"), None);
    assert_eq!(x.seen().len(), 1, "no ClearSeenPickupable out of range");
}

/// The line check runs only beyond a quarter of PickupRange (75) horizontally; a hit other than the winner or its
/// base refuses the winner.
#[test]
fn the_line_check_runs_beyond_a_quarter_of_the_range() {
    let mut x = fx();
    let near = x.pickup([60.0, 0.0, 0.0], [0.0; 3]);
    let wall = x.obj("Engine.Actor");
    x.host.blocker = Some(wall);
    assert_eq!(x.tick().unwrap(), Outcome::Hover { winner: near, reported: true });
    assert_eq!(x.host.traced, 0, "no trace within 75");

    let mut x = fx();
    let far = x.pickup([150.0, 0.0, 0.0], [0.0; 3]);
    let wall = x.obj("Engine.Actor");
    x.host.blocker = Some(wall);
    assert_eq!(x.tick().unwrap(), Outcome::NoWinner { cleared: false });
    assert_eq!(x.host.traced, 1);
    let base = x.obj("Engine.Actor");
    x.set(far, "Base", Value::Object(Some(base)));
    x.host.blocker = Some(base);
    assert_eq!(x.tick().unwrap(), Outcome::Hover { winner: far, reported: true });
    x.host.blocker = Some(far);
    assert_eq!(x.tick().unwrap(), Outcome::Hover { winner: far, reported: false });
}

/// The 0.5 view check is three-dimensional without a hover particle: a pickup well below the view whose tall segment
/// the ray still crosses is left unchanged. With a hover particle the check is horizontal and passes.
#[test]
fn the_half_view_check_is_three_dimensional_without_a_hover_particle() {
    let mut x = fx();
    x.host.height = 0.0;
    x.set(x.gd, "ItemHoverProjectionHeight", Value::Float(400.0));
    let low = x.pickup([40.0, 0.0, -250.0], [0.0; 3]);
    assert_eq!(x.tick().unwrap(), Outcome::Unchanged { winner: low });
    assert_eq!(x.slots(), (None, None));
    let particle = x.obj("Engine.Actor");
    x.set(x.gd, "PartItemHover", Value::Object(Some(particle)));
    assert_eq!(x.tick().unwrap(), Outcome::Hover { winner: low, reported: true });
}

/// A winner whose item has no definition changes neither `H` nor `S`.
#[test]
fn a_winner_without_a_definition_changes_nothing() {
    let mut x = fx();
    let p = x.pickup([150.0, 0.0, 0.0], [0.0; 3]);
    let item = object_field(&x.vm, p, "Inventory").unwrap().unwrap();
    x.set_definition(item, None);
    let earlier = x.obj(PICKUP);
    x.set_slots(Some(earlier), Some(earlier));
    assert_eq!(x.tick().unwrap(), Outcome::Unchanged { winner: p });
    assert_eq!(x.slots(), (Some(earlier), Some(earlier)));
    assert!(x.seen().is_empty());
}

/// Suspension and the pawn test: the controller's `Dead` state gives no winner; a dead pawn gives none; a pawn of
/// another class (a subclass too) or a driven vehicle is `NotYet { NotRead }` with nothing written.
#[test]
fn suspension_and_the_pawn_test() {
    let mut x = fx();
    let _p = x.pickup([150.0, 0.0, 0.0], [0.0; 3]);
    let dead = x.vm.space.names.intern("Dead");
    x.vm.space.heap.get_mut(x.controller).unwrap().state = Some(dead);
    assert_eq!(x.tick().unwrap(), Outcome::NoWinner { cleared: false });
    x.vm.space.heap.get_mut(x.controller).unwrap().state = None;
    x.set(x.pawn, "bIsDead", Value::Bool(true));
    assert_eq!(x.tick().unwrap(), Outcome::NoWinner { cleared: false });
    x.set(x.pawn, "bIsDead", Value::Bool(false));
    let vehicle = x.obj("Engine.Actor");
    x.set(x.pawn, "DrivenVehicle", Value::Object(Some(vehicle)));
    assert!(not_yet(x.tick()).what.contains("driven vehicle"));
    let other = x.obj("WillowGame.ToyPlayerPawnSubclass");
    x.set(x.controller, "Pawn", Value::Object(Some(other)));
    let n = not_yet(x.tick());
    assert_eq!(n.missing, Missing::NotRead);
    assert!(n.what.contains("open question 1"), "{n}");
    assert!(x.seen().is_empty());
}

/// No owner or no `HUDDef`: the pass ends, only `ClosestPickup` emptied; no pawn: no event, `H` and `S` unchanged.
#[test]
fn idle_and_no_pawn() {
    let mut x = fx();
    let p = x.pickup([150.0, 0.0, 0.0], [0.0; 3]);
    x.set_slots(Some(p), Some(p));
    x.set(x.hud, "ClosestPickup", Value::Object(Some(p)));
    x.set(x.hud, "HUDDef", Value::Object(None));
    assert_eq!(x.tick().unwrap(), Outcome::Idle);
    assert_eq!(x.slot("ClosestPickup"), None);
    x.set(x.hud, "HUDDef", Value::Object(Some(x.hud_definition)));
    x.set(x.hud, "PlayerOwner", Value::Object(None));
    assert_eq!(x.tick().unwrap(), Outcome::Idle);
    x.set(x.hud, "PlayerOwner", Value::Object(Some(x.controller)));
    x.set(x.controller, "Pawn", Value::Object(None));
    assert_eq!(x.tick().unwrap(), Outcome::NoPawn);
    assert_eq!(x.slots(), (Some(p), Some(p)));
    assert!(x.seen().is_empty());
}

/// `WPlayerOwner` None is refused only when the tick raises an event: a no-event tick passes, a reporting tick and a
/// clearing tick are `NotYet { NotRead }` with nothing written.
#[test]
fn a_none_recipient_is_refused_only_when_an_event_is_raised() {
    let mut x = fx();
    x.set(x.hud, "WPlayerOwner", Value::Object(None));
    let p = x.pickup([150.0, 0.0, 0.0], [0.0; 3]);
    x.set_slots(Some(p), Some(p));
    assert_eq!(x.tick().unwrap(), Outcome::Hover { winner: p, reported: false }, "already S: no event");
    x.set_slots(None, None);
    let n = not_yet(x.tick());
    assert!(n.missing == Missing::NotRead && n.what.contains("WPlayerOwner"), "{n}");
    assert_eq!(x.slots(), (None, None), "nothing written");
    x.set(p, "bPickupable", Value::Bool(false));
    let earlier = x.obj(PICKUP);
    x.set_slots(None, Some(earlier));
    assert!(not_yet(x.tick()).what.contains("WPlayerOwner"), "a clear needs the recipient too");
    x.set_slots(None, None);
    assert_eq!(x.tick().unwrap(), Outcome::NoWinner { cleared: false }, "no event, no refusal");
}

/// The particle offset: the definition's `PickupFlagOffset` turned by the pickup's rotation; a non-zero one sets
/// `bHasPickupParticleOffset`, written only after the plan.
#[test]
fn the_particle_offset_is_turned_and_flagged() {
    let mut x = fx();
    assert_eq!(rotate([10.0, 0.0, 0.0], 0, 0, 0), [10.0, 0.0, 0.0]);
    let quarter = rotate([10.0, 0.0, 0.0], 0, 16384, 0);
    assert!(quarter[0].abs() < 1e-4 && (quarter[1] - 10.0).abs() < 1e-4, "{quarter:?}");
    // The offset lifts the centre to 30: the segment spans 15..50, above the ray at z = 0, so no winner.
    let p = x.pickup([150.0, 0.0, -50.0], [0.0, 0.0, 0.0]);
    let q = x.pickup([150.0, 0.0, -50.0], [0.0, 0.0, 80.0]);
    x.set(p, "bPickupable", Value::Bool(false));
    assert_eq!(x.tick().unwrap(), Outcome::NoWinner { cleared: false });
    assert_eq!(x.get(q, "bHasPickupParticleOffset"), Value::Bool(true));
    let (offset, nonzero) = particle_offset(&mut x.vm, q).unwrap();
    assert!(nonzero && offset == [0.0, 0.0, 80.0], "{offset:?}");
}

/// NATIVE_LINK7_HUD_TARGETING.md test 4, the shrink step: three pickups with `LifeSpan` 20, 18 and 5 and the duration
/// 15; with `MaxNumPickups` 1 `BeginShrinking` is raised on the 18, after the other writes; with 2 nothing; in a
/// client world, or with no `WorldInfo`, nothing; an owner with a `Player` is the split-screen `NotRead` before any write.
#[test]
fn the_shrink_step_raises_begin_shrinking_on_the_shortest_counted() {
    let shrinks = |x: &Fx| x.seen().into_iter().filter(|(e, _)| e.starts_with("Shrink")).map(|(e, _)| e).collect::<Vec<_>>();
    let setup = |max: i32| {
        let mut x = fx();
        let defaults = x.vm.defs.class_default(&mut x.vm.space, PICKUP).unwrap();
        x.set(defaults, "PickupShrinkDuration", Value::Float(15.0));
        let mut made = Vec::new();
        for (i, life) in [20.0f32, 18.0, 5.0].into_iter().enumerate() {
            let p = x.pickup([150.0, 40.0 * i as f32 + 500.0, 0.0], [0.0; 3]);
            x.set(p, "LifeSpan", Value::Float(life));
            made.push(p);
        }
        x.set(x.hud, "MaxNumPickups", Value::Int(max));
        (x, made)
    };
    let (mut x, made) = setup(1);
    x.tick().unwrap();
    assert_eq!(shrinks(&x), [format!("Shrink {:?}", made[1])], "the 18");
    let (mut x, _) = setup(2);
    x.tick().unwrap();
    assert!(shrinks(&x).is_empty(), "two counted, not above 2");
    let (mut x, _) = setup(1);
    let info = x.obj("Engine.WorldInfo");
    x.set(info, "NetMode", Value::Byte(3));
    x.set(x.hud, "WorldInfo", Value::Object(Some(info)));
    x.tick().unwrap();
    assert!(shrinks(&x).is_empty(), "a client world");
    let (mut x, _) = setup(1);
    x.set(x.hud, "WorldInfo", Value::Object(None));
    x.tick().unwrap();
    assert!(shrinks(&x).is_empty(), "no WorldInfo");
    let (mut x, made) = setup(1);
    let player = x.obj("Engine.Actor");
    x.set(x.controller, "Player", Value::Object(Some(player)));
    x.set(x.hud, "ClosestPickup", Value::Object(Some(made[2])));
    let n = not_yet(x.tick());
    assert_eq!((n.missing, n.what.as_ref()), (Missing::NotRead, SPLIT_SCREEN));
    assert_eq!(x.slot("ClosestPickup"), Some(made[2]), "nothing written");
}

/// The headless world: the view, the bounds and the line check are each `NotYet { LayerAbsent }`, before any write;
/// [`hud_of`] answers no HUD as a missing layer.
#[test]
fn the_headless_world_names_each_missing_layer() {
    let mut x = fx();
    let p = x.pickup([150.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
    let hud = x.hud;
    let n = not_yet(tick(&mut x.vm, hud, &x.provider, &mut Headless));
    assert_eq!(n.missing, Missing::LayerAbsent("a camera (the headless world has none)"));
    assert_eq!(x.provider.resolved.get(), 0, "the camera refuses before the globals are resolved");
    let n = not_yet(tick(&mut x.vm, hud, &x.provider, &mut BoundsOnly(x.host.view)));
    assert_eq!(n.missing, Missing::LayerAbsent("component bounds (the headless world has none)"));
    let n = not_yet(tick(&mut x.vm, hud, &x.provider, &mut TraceOnly(x.host.view)));
    assert_eq!(n.missing, Missing::LayerAbsent("collision (the headless world has none)"));
    assert!(!n.what.contains(PARTIAL_WRITES));
    assert_eq!(x.get(p, "bHasPickupParticleOffset"), Value::Bool(false), "nothing written");
    assert_eq!(hud_of(&x.vm, x.controller).unwrap(), hud);
    x.set(x.controller, "myHUD", Value::Object(None));
    match hud_of(&x.vm, x.controller).unwrap_err() {
        VmError::NotYet(n) => assert_eq!(n.missing, Missing::LayerAbsent("a HUD (the headless world spawns none)")),
        e => panic!("{e}"),
    }
}

/// A host with a view and bounds but the headless line check.
struct TraceOnly(View);
impl Host for TraceOnly {
    fn view(&mut self, _: &Vm, _: ObjRef) -> Result<View, VmError> {
        Ok(self.0)
    }
    fn bounds_height(&mut self, _: &Vm, _: ObjRef) -> Result<f32, VmError> {
        Ok(40.0)
    }
    fn line_blocker(&mut self, vm: &Vm, from: [f32; 3], to: [f32; 3]) -> Result<Option<ObjRef>, VmError> {
        Headless.line_blocker(vm, from, to)
    }
}

/// A host with a view but the headless bounds.
struct BoundsOnly(View);
impl Host for BoundsOnly {
    fn view(&mut self, _: &Vm, _: ObjRef) -> Result<View, VmError> {
        Ok(self.0)
    }
    fn bounds_height(&mut self, vm: &Vm, p: ObjRef) -> Result<f32, VmError> {
        Headless.bounds_height(vm, p)
    }
    fn line_blocker(&mut self, _: &Vm, _: [f32; 3], _: [f32; 3]) -> Result<Option<ObjRef>, VmError> {
        Ok(None)
    }
}

/// A `NotYet` from an event handler comes after writes: marked "stopped after partial writes", its kind and record
/// kept; the writes before it stay (`H`), and `S` is not written.
#[test]
fn a_refused_event_after_a_write_is_marked() {
    let mut x = fx();
    let p = x.pickup([150.0, 0.0, 0.0], [0.0; 3]);
    x.events.borrow_mut().refuse = true;
    let n = not_yet(x.tick());
    assert!(n.what.starts_with(PARTIAL_WRITES), "marked although its text mentions the prefix: {n}");
    assert_eq!((n.missing, n.record), (Missing::NotRead, "TOY.md"));
    assert_eq!(x.slots(), (Some(p), None));
}

/// Section 4's cutoffs: a crossing gives 0, a closest point outside a piece, parallel lines and a near-zero piece give
/// no `d`; both ends of a piece count as inside.
#[test]
fn the_segment_distance_and_its_cutoffs() {
    let ray = ([0.0; 3], [100.0, 0.0, 0.0]);
    assert_eq!(segment_distance([10.0, 0.0, -5.0], [10.0, 0.0, 5.0], ray.0, ray.1), Some(0.0));
    let d = segment_distance([10.0, 3.0, -5.0], [10.0, 3.0, 5.0], ray.0, ray.1).unwrap();
    assert!((d - 3.0).abs() < 1e-5);
    assert_eq!(segment_distance([10.0, 0.0, 10.0], [10.0, 0.0, 20.0], ray.0, ray.1), None, "outside the segment");
    assert_eq!(segment_distance([0.0, 1.0, 0.0], [10.0, 1.0, 0.0], ray.0, ray.1), None, "parallel");
    assert_eq!(segment_distance([10.0, 0.0, 0.0], [10.0, 0.0, 0.00009], ray.0, ray.1), None, "near-zero segment");
    assert_eq!(segment_distance([10.0, 0.0, 0.0], [10.0, 0.0, 5.0], ray.0, ray.1), Some(0.0), "the bottom end counts");
}

/// Section 3's cone cutoff: a pickup straight above the view point fails the cone (its horizontal offset is not
/// normalised); the view direction comes from the sine table.
#[test]
fn a_pickup_straight_above_fails_the_cone_and_the_view_uses_the_table() {
    let mut x = fx();
    let _above = x.pickup([0.0, 0.0, 100.0], [0.0; 3]);
    assert_eq!(x.tick().unwrap(), Outcome::NoWinner { cleared: false });
    assert!(cosine([1.0, 0.0, 0.0], [0.00005, 0.0, 0.0]) < 1e-4);
    let quarter = view_direction(&View { location: [0.0; 3], pitch: 0, yaw: 16384 });
    assert!(quarter[0].abs() < 1e-6 && (quarter[1] - 1.0).abs() < 1e-6, "{quarter:?}");
    // Units below one table step (4) give the same direction.
    assert_eq!(view_direction(&View { location: [0.0; 3], pitch: 0, yaw: 3 }), [1.0, 0.0, 0.0]);
}

/// An exact tie (two pickups mirrored across the ray, bit-identical values) keeps the earlier list entry.
#[test]
fn an_exact_tie_keeps_the_earlier_pickup() {
    for first_left in [true, false] {
        let mut x = fx();
        let (a, b) = ([150.0, 5.0, 0.0], [150.0, -5.0, 0.0]);
        let (first, second) = if first_left { (a, b) } else { (b, a) };
        let earlier = x.pickup(first, [0.0; 3]);
        let _later = x.pickup(second, [0.0; 3]);
        assert_eq!(x.tick().unwrap(), Outcome::Hover { winner: earlier, reported: true });
    }
}

/// The hover bonus decides a win: a farther pickup that is `H` beats a nearer one only by its 0.25 (both are hit
/// dead on, so both score 1).
#[test]
fn the_hover_bonus_decides_a_win() {
    let mut x = fx();
    let near = x.pickup([150.0, 0.0, 0.0], [0.0; 3]);
    let far = x.pickup([160.0, 0.0, 0.0], [0.0; 3]);
    assert_eq!(x.tick().unwrap(), Outcome::Hover { winner: near, reported: true });
    x.set_slots(Some(far), Some(near));
    assert_eq!(x.tick().unwrap(), Outcome::Hover { winner: far, reported: true }, "1.25 / 160 > 1 / 150");
}

/// The getters are answered by field reads only: an inventory whose class overrides `GetInventoryDefinition` in
/// script is a named `NotYet { NotRead }` in the plan, with nothing written.
#[test]
fn a_script_override_of_a_getter_is_refused_in_the_plan() {
    let mut x = fx();
    let p = x.pickup([150.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
    let item = x.obj("WillowGame.ToyOverridingWeapon");
    x.set(p, "Inventory", Value::Object(Some(item)));
    let n = not_yet(x.tick());
    assert!(n.missing == Missing::NotRead && n.what.contains("ToyOverridingWeapon"), "{n}");
    assert_eq!(x.slot("ClosestPickup"), None);
    assert_eq!(x.get(p, "bHasPickupParticleOffset"), Value::Bool(false), "nothing written");
}

/// The planned event's handler is resolved and checked in the plan: a recipient without `SawPickupable` is an
/// ordinary error before any write.
#[test]
fn a_missing_handler_is_found_before_any_write() {
    let mut x = fx();
    let p = x.pickup([150.0, 0.0, 0.0], [0.0; 3]);
    let mute = x.obj("WillowGame.ToyMuteController");
    x.set(x.hud, "WPlayerOwner", Value::Object(Some(mute)));
    x.set(x.hud, "ClosestPickup", Value::Object(Some(p)));
    let e = x.tick().unwrap_err();
    assert!(matches!(&e, VmError::Script { message, .. } if message.contains("SawPickupable")), "{e}");
    assert_eq!(x.slot("ClosestPickup"), Some(p), "nothing written");
    assert_eq!(x.slots(), (None, None));
}

/// The shrink step comes after the winner's checks: with a long-lived pickup and a winner beyond a quarter of the
/// range, the missing collision answers first.
#[test]
fn the_shrink_refusal_comes_after_the_line_check() {
    let mut x = fx();
    let p = x.pickup([150.0, 0.0, 0.0], [0.0; 3]);
    x.set(p, "LifeSpan", Value::Float(30.0));
    let hud = x.hud;
    let n = not_yet(tick(&mut x.vm, hud, &x.provider, &mut TraceOnly(x.host.view)));
    assert_eq!(n.missing, Missing::LayerAbsent("collision (the headless world has none)"));
}

/// An out-of-range winner: the seen-item slot ends empty after `H` is updated (section 7, both writes planned).
#[test]
fn an_out_of_range_winner_ends_with_the_seen_slot_empty() {
    let mut x = fx();
    let particle = x.obj("Engine.Actor");
    x.set(x.gd, "PartItemHover", Value::Object(Some(particle)));
    let far = x.pickup([400.0, 0.0, 0.0], [0.0; 3]);
    assert_eq!(x.tick().unwrap(), Outcome::Hover { winner: far, reported: false });
    assert_eq!(x.slot("CachedHoverPickup"), Some(far));
    assert_eq!(x.slot("SavedLookAtInventory"), None);
}

/// The Off world's globals: the world's `WillowGlobals` and its `TheGlobalsDefinition` when that holds a
/// `GlobalsDefinition`; none held is a named `NotYet { NotRead }` (the getter's first load was not read).
#[test]
fn the_world_globals_read_the_definition_field() {
    let mut x = fx();
    let q = ow_world::query::install(&mut x.vm).unwrap();
    q.borrow_mut().set_globals(x.globals);
    let n = match WorldGlobals::prepare(&mut x.vm).resolve(&x.vm).unwrap_err() {
        VmError::NotYet(n) => n,
        e => panic!("{e}"),
    };
    assert!(n.missing == Missing::NotRead && n.what.contains("TheGlobalsDefinition"), "{n}");
    x.set(x.globals, "TheGlobalsDefinition", Value::Object(Some(x.gd)));
    assert_eq!(WorldGlobals::prepare(&mut x.vm).resolve(&x.vm).unwrap(), (x.globals, x.gd));
}

/// The planned `SawPickupable` call is built and checked in the plan with its exact argument (review r3 of #467); the
/// commit makes that call.
#[test]
fn the_handler_call_is_checked_with_its_exact_argument() {
    let mut x = fx();
    let p = x.pickup([150.0, 0.0, 0.0], [0.0; 3]);
    let checked = handler(&mut x.vm, Some(x.controller), "SawPickupable", Some(p)).unwrap();
    assert_eq!(checked.args, vec![Value::Object(Some(p))]);
    let none = handler(&mut x.vm, Some(x.controller), "ClearSeenPickupable", None).unwrap();
    assert!(none.args.is_empty());
}

/// On an allowlisted class the missing layout is a named `NotYet { NotRead }` (NATIVE_INVENTORY_DEFINITION.md, as
/// #462), not an ordinary error.
#[test]
fn a_missing_definition_layout_is_not_yet() {
    let mut x = fx();
    let p = x.pickup([150.0, 0.0, 0.0], [0.0; 3]);
    let bare = x.obj("WillowGame.WillowItem");
    x.set(p, "Inventory", Value::Object(Some(bare)));
    let n = not_yet(x.tick());
    assert!(n.missing == Missing::NotRead && n.what.contains("without DefinitionData"), "{n}");
}

/// A `NotYet` from the definition store keeps its variant (`VmError::from`, review r3 of #467).
#[test]
fn a_definition_store_not_yet_keeps_its_variant() {
    let n = ow_core::NotYet::not_read("a lazy handle", "NOTE.md");
    match VmError::from(ow_defs::store::DefError::NotYet(n)) {
        VmError::NotYet(n) => assert_eq!(n.record, "NOTE.md"),
        e => panic!("{e}"),
    }
}

/// COLD-CLASS-AUDIT (lane COLD-CLASS-AUDIT-WORLD-INVENTORY): the pawn test's refusal with a pawn whose class has a
/// layout and no loaded code, `WillowPlayerPawn` warm (the fixture's `make` looks it up). The refusal loads no class
/// code and leaves #518's counters and the name table unchanged (lane COLD-LOAD-FIXES: `suspended` no longer interns
/// "Dead").
#[test]
fn the_pawn_test_on_a_cold_pawn_class_loads_no_code() {
    let mut x = fx();
    let _p = x.pickup([150.0, 0.0, 0.0], [0.0; 3]);
    let vm = &mut x.vm;
    let (code, store) = vm.defs.code_mut();
    let key = code.find_class(store, "WillowGame.ToyPlayerPawnSubclass").unwrap();
    let c = code.define_layout(store, &mut vm.space.names, &mut vm.space.classes, key).unwrap();
    assert!(!vm.class_code_loaded(c), "starts cold");
    let other = vm.space.alloc(c, "Cold", None, ow_object::heap::Origin::Runtime).unwrap();
    x.set(x.controller, "Pawn", Value::Object(Some(other)));
    let before = cold_marks(&x.vm);
    let n = not_yet(x.tick());
    assert!(n.what.contains("open question 1"), "{n}");
    assert_eq!(cold_marks(&x.vm), before, "no code load, write, draw, store load or name");
}

/// Code loads, #518's three counters and the name table (lane COLD-LOAD-FIXES).
fn cold_marks(vm: &Vm) -> (u64, u64, u64, u64, usize) {
    (vm.class_code_loads(), vm.space.heap.committed_writes(), vm.rng.draws(), vm.defs.load_transitions(), vm.space.names.len())
}

/// Lane COLD-LOAD-FIXES, the case #701 listed as not reached: `WillowPlayerPawn` itself cold (never looked up), the
/// pawn a cold subclass. `suspended` compares the class through the class table by path, so the refusal loads no
/// class code and interns nothing.
#[test]
fn the_pawn_test_with_the_player_pawn_class_cold_loads_no_code() {
    let mut x = fx_with(false);
    let player_pawn = x.vm.space.classes.by_path("WillowGame.WillowPlayerPawn").expect("the parent's layout");
    assert!(!x.vm.class_code_loaded(player_pawn), "WillowPlayerPawn starts cold");
    let _p = x.pickup([150.0, 0.0, 0.0], [0.0; 3]);
    let before = cold_marks(&x.vm);
    let n = not_yet(x.tick());
    assert!(n.what.contains("open question 1"), "{n}");
    assert_eq!(cold_marks(&x.vm), before, "no code load, write, draw, store load or name");
    assert!(!x.vm.class_code_loaded(player_pawn), "still cold");
}

/// Review of #810, item 1: `BeginShrinking` comes after the nearest-pickup and particle-offset writes and before the
/// seen-item, hover and reporting writes. Raised with a winner to report: before `SawPickupable`, with `S` still
/// empty. Refused: the refusal is prefixed (`ClosestPickup` was written before it), `SawPickupable` has not run and
/// `S` and `H` are unchanged.
#[test]
fn begin_shrinking_comes_before_the_report() {
    let setup = || {
        let mut x = fx();
        let defaults = x.vm.defs.class_default(&mut x.vm.space, PICKUP).unwrap();
        x.set(defaults, "PickupShrinkDuration", Value::Float(15.0));
        let aimed = x.pickup([150.0, 0.0, 0.0], [0.0; 3]);
        let far = x.pickup([150.0, 600.0, 0.0], [0.0; 3]);
        for p in [aimed, far] {
            x.set(p, "LifeSpan", Value::Float(20.0));
        }
        x.set(x.hud, "MaxNumPickups", Value::Int(1));
        (x, aimed)
    };
    let (mut x, aimed) = setup();
    assert_eq!(x.tick().unwrap(), Outcome::Hover { winner: aimed, reported: true });
    let seen = x.seen();
    assert_eq!(seen.len(), 2, "{seen:?}");
    assert!(seen[0].0.starts_with("Shrink") && seen[0].1.is_none(), "the shrink first, S still empty: {seen:?}");
    assert!(seen[1].0.starts_with("Saw"), "{seen:?}");
    let (mut x, _) = setup();
    x.events.borrow_mut().refuse_shrink = true;
    let n = not_yet(x.tick());
    assert!(n.what.starts_with(PARTIAL_WRITES) && n.what.contains("the toy BeginShrinking"), "{n}");
    assert!(x.seen().is_empty(), "SawPickupable has not run");
    assert_eq!((x.slot("CachedHoverPickup"), x.slot("CachedBestPickup")), (None, None), "H and S unchanged");
}

/// Review of #810, item 2: the plan reads only. A class default that is not built, or a handler whose class code is not
/// loaded, is a named `NotYet { NotRead }` with nothing written, loaded or drawn, and so no prefix.
#[test]
fn the_plan_loads_and_builds_nothing() {
    let counters = |x: &Fx| (x.vm.space.heap.committed_writes(), x.vm.rng.draws(), x.vm.defs.load_transitions(), x.vm.class_code_loads());
    // The pickup class's default freed: not built.
    let mut x = fx();
    x.pickup([150.0, 0.0, 0.0], [0.0; 3]);
    let defaults = x.vm.defs.class_default(&mut x.vm.space, PICKUP).unwrap();
    x.vm.space.free(defaults).unwrap();
    let before = counters(&x);
    let n = not_yet(x.tick());
    assert!(n.what.contains("class default") && !n.what.starts_with(PARTIAL_WRITES), "{n}");
    assert_eq!(counters(&x), before, "nothing built or loaded");
    // A recipient whose class code is not loaded: the handler's lookup is the NotRead, nothing loaded.
    let mut x = fx();
    x.pickup([150.0, 0.0, 0.0], [0.0; 3]);
    let cold = {
        let (code, store) = x.vm.defs.code_mut();
        let key = code.find_class(store, "WillowGame.ToyMuteController").unwrap();
        code.define_layout(store, &mut x.vm.space.names, &mut x.vm.space.classes, key).unwrap()
    };
    let recipient = x.vm.space.alloc(cold, "Cold", None, ow_object::heap::Origin::Runtime).unwrap();
    x.set(x.hud, "WPlayerOwner", Value::Object(Some(recipient)));
    let before = counters(&x);
    let n = not_yet(x.tick());
    assert!(n.what.contains("is not loaded") && !n.what.starts_with(PARTIAL_WRITES), "{n}");
    assert_eq!(counters(&x), before, "no class code loaded");
}

/// Review of #810, item 4: whether shrinking is on is decided before `MaxNumPickups` is read, so a client HUD whose class
/// does not declare it answers nothing; an enabled one needs it (a data error).
#[test]
fn a_client_hud_needs_no_max_num_pickups() {
    let mut x = fx();
    let plain = x.obj("Engine.HUD");
    let info = x.obj("Engine.WorldInfo");
    x.set(info, "NetMode", Value::Byte(3));
    x.set(plain, "WorldInfo", Value::Object(Some(info)));
    let h = hud_state(&x.vm, plain).unwrap_or(HudState { hud: plain, owner: None, definition: None, recipient: None, hover: None, reported: None });
    let owner = x.controller;
    assert_eq!(shrink_step(&mut x.vm, &h, owner, &[]).unwrap().map(|s| s.0), None, "a client: nothing, no MaxNumPickups read");
    x.set(info, "NetMode", Value::Byte(0));
    assert!(matches!(shrink_step(&mut x.vm, &h, owner, &[]), Err(VmError::Script { .. })), "enabled: MaxNumPickups needed");
}

/// Review r2 of #810, item 3: the pickup default and the LifeSpans are read only once shrinking is on. A client HUD (and
/// one with no WorldInfo) passes with the default not built, with and without pickups, and builds nothing; an enabled
/// one still needs the default (the named NotRead).
#[test]
fn a_disabled_shrink_needs_no_pickup_default() {
    for pickups in [false, true] {
        let mut x = fx();
        if pickups {
            x.pickup([150.0, 0.0, 0.0], [0.0; 3]);
        }
        let defaults = x.vm.defs.class_default(&mut x.vm.space, PICKUP).unwrap();
        x.vm.space.free(defaults).unwrap();
        let info = x.obj("Engine.WorldInfo");
        x.set(info, "NetMode", Value::Byte(3));
        x.set(x.hud, "WorldInfo", Value::Object(Some(info)));
        x.tick().unwrap();
        x.set(x.hud, "WorldInfo", Value::Object(None));
        x.tick().unwrap();
        x.set(info, "NetMode", Value::Byte(0));
        x.set(x.hud, "WorldInfo", Value::Object(Some(info)));
        let n = not_yet(x.tick());
        assert!(n.what.contains("class default"), "enabled: the default is needed ({pickups}): {n}");
    }
}
