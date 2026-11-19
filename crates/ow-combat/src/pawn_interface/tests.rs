//! Tests of `Pawn`'s and `WillowAIPawn`'s `QueryInterface` answers, one per branch of NATIVE_PAWN_AI_INTERFACE.md
//! section 2, on invented classes and objects (AGENTS.md rule 9). AI-assisted (Claude Code).
//!
//! The pawn, controller, vehicle and mesh classes and the interface `IToyThing` are decoded class exports of an
//! invented `Engine.upk` (so the interface tables are read, as the installed ones are); the classes whose answers the
//! note leaves open, and `WillowAIPawn`, are defined by hand at their installed paths, over those. A "read" answer is
//! stood in for by a rule a test registers on an invented class (`ToyAnswering*`), since the note reads none.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::registry::AttributeSystem;
use ow_core::{Missing, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_object::ids::{ClassId, ObjRef};
use ow_object::value::Value;
use ow_core::PackageName;
use ow_package::testkit::reflect::{ClassBodySpec, class_body};
use ow_package::testkit::script::ScriptToy;
use ow_package::testkit::{Export, ints};
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use super::*;
use crate::hit::access::set;

static NEXT: AtomicU32 = AtomicU32::new(0);

const INTERFACE: u32 = 2 | 0x4000;

/// A class export with a decodable body naming `table`'s interfaces, and its default object.
fn real_class(t: &mut ScriptToy, name: &str, super_ref: i32, within: i32, flags: u32, table: &[i32]) -> i32 {
    let at = t.p.next_export();
    let spec = ClassBodySpec {
        flags,
        interfaces: table.iter().map(|&c| (c, 0)).collect(),
        ..ClassBodySpec::new(super_ref, within, at + 1)
    };
    let body = class_body(&mut t.p, &spec);
    let mut export = Export::new(0, name, body);
    export.super_ = super_ref;
    assert_eq!(t.p.add_export_with(export), at);
    let mut default = ints(&[0]);
    default.extend_from_slice(&t.none);
    t.p.add_export_with(Export::new(at, &format!("Default__{name}"), default));
    at
}

/// The invented `Engine.upk`: `Object`; the interface `IToyThing`; `Pawn` with `Controller`, `DrivenVehicle` and
/// `Mesh`; `ToyThingPawn` (a pawn listing `IToyThing`) and `ToyInheritingPawn` (its subclass, which lists nothing);
/// `Controller` and `Vehicle` (a pawn), each with `Controller` where the rule reads it; `SkeletalMeshComponent`; and the
/// classes that stand in for read answers: `ToyAnsweringController` and `ToyAnsweringMesh`.
fn engine() -> Vec<u8> {
    engine_with(false)
}

/// [`engine`]; with `mistyped`, `ActorComponent` declares `Owner` as an int (the mistyped-`Owner` case).
fn engine_with(mistyped: bool) -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class", "ObjectProperty", "IntProperty"]);
    let object = real_class(&mut t, "Object", 0, 1, 2, &[]);
    let thing = real_class(&mut t, "IToyThing", object, object, INTERFACE, &[]);
    // `Engine.Actor`, the class a component's `Owner` must reference (lane IMPL-LINK11-QUERY-INTERFACE).
    let actor = real_class(&mut t, "Actor", object, object, 2, &[]);
    let pawn = real_class(&mut t, "Pawn", actor, object, 2, &[]);
    for field in ["Controller", "DrivenVehicle", "Mesh"] {
        t.prop("Object", pawn, field, 0, object);
    }
    let thing_pawn = real_class(&mut t, "ToyThingPawn", pawn, object, 2, &[thing]);
    real_class(&mut t, "ToyInheritingPawn", thing_pawn, object, 2, &[]);
    let controller = real_class(&mut t, "Controller", actor, object, 2, &[]);
    // A controller listing the interface, for the direct mesh query's owner forwarding.
    real_class(&mut t, "ToyThingController", controller, object, 2, &[thing]);
    real_class(&mut t, "Vehicle", pawn, object, 2, &[]);
    // `ActorComponent` without `Owner` (for the missing-declaration case), `SkeletalMeshComponent` under it declaring
    // `Owner`, a subclass of it and another component class (lane IMPL-LINK11-QUERY-INTERFACE).
    let actor_component = real_class(&mut t, "ActorComponent", object, object, 2, &[]);
    if mistyped {
        t.prop("Int", actor_component, "Owner", 0, 0);
    }
    let component = real_class(&mut t, "SkeletalMeshComponent", actor_component, object, 2, &[]);
    t.prop("Object", component, "Owner", 0, actor);
    real_class(&mut t, "ToySubMesh", component, object, 2, &[]);
    real_class(&mut t, "ToyPlainComponent", actor_component, object, 2, &[]);
    real_class(&mut t, "ToyAnsweringController", controller, object, 2, &[]);
    real_class(&mut t, "ToyAnsweringMesh", component, object, 2, &[]);
    // A holder whose `Held` names a pawn export, which the store keeps as an unloaded handle until it is loaded.
    let holder = real_class(&mut t, "ToyHolder", object, object, 2, &[]);
    t.prop("Object", holder, "Held", 0, actor);
    let held = t.p.next_export() + 1;
    let mut payload = ints(&[0]);
    payload.extend(t.p.object_tag("Held", held));
    payload.extend_from_slice(&t.none);
    t.p.add_export_with(Export::new(holder, "ToyHolder_0", payload));
    let mut payload = ints(&[0]);
    payload.extend_from_slice(&t.none);
    t.p.add_export_with(Export::new(pawn, "ToyLazyPawn_0", payload));
    t.build()
}

struct Env {
    vm: Vm,
    dir: PathBuf,
    iface: ClassId,
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

impl Env {
    /// The invented world in `mode`, with the hand-defined classes and (in Off) the pawn rules registered.
    fn new(mode: ParityMode) -> Env {
        Env::with(mode, false)
    }

    fn with(mode: ParityMode, mistyped: bool) -> Env {
        let dir = std::env::temp_dir().join(format!(
            "ow-combat-pawn-interface-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let engine = engine_with(mistyped);
        let defs =
            DefStore::from_synthetic(&dir, &[("Engine.upk", engine.as_slice())], IdentityPolicy::FirstInScanOrder, mode)
                .unwrap();
        let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), mode);
        vm.defs.code_mut().1.load(&PackageName::parse("Engine").unwrap()).unwrap();
        let class = |vm: &mut Vm, path: &str| vm.find_class(path).unwrap();
        let (object, pawn, controller) =
            (class(&mut vm, "Engine.Object"), class(&mut vm, "Engine.Pawn"), class(&mut vm, "Engine.Controller"));
        let vehicle = class(&mut vm, "Engine.Vehicle");
        let classes = &mut vm.space.classes;
        let mind = classes.define("GearboxFramework.GearboxMind", Some(controller), vec![]).unwrap();
        classes.define("WillowGame.WillowMind", Some(mind), vec![]).unwrap();
        classes.define("WillowGame.WillowPlayerController", Some(controller), vec![]).unwrap();
        classes.define("WillowGame.WillowAIPawn", Some(pawn), vec![]).unwrap();
        classes.define("WillowGame.WillowVehicle", Some(vehicle), vec![]).unwrap();
        let master = classes.define("WillowGame.IPlayerMaster", Some(object), vec![]).unwrap();
        classes.define("Toy.IToyPlayerMasterChild", Some(master), vec![]).unwrap();
        classes.define("GearboxFramework.IAIInterface", Some(object), vec![]).unwrap();
        register(&mut vm).unwrap();
        let iface = class(&mut vm, "Engine.IToyThing");
        Env { vm, dir, iface }
    }

    fn make(&mut self, path: &str, name: &str) -> ObjRef {
        let class = self.vm.find_class(path).unwrap();
        self.vm.instantiate(class, name).unwrap()
    }

    fn link(&mut self, o: ObjRef, field: &str, to: ObjRef) {
        set(&mut self.vm, o, field, Value::Object(Some(to))).unwrap();
    }

    /// Registers on `path` a stand-in for a read answer: `answer` for every question, and each question noted.
    fn answers(&mut self, path: &str, answer: Option<ObjRef>) -> Rc<RefCell<Vec<ObjRef>>> {
        let asked = Rc::new(RefCell::new(Vec::new()));
        let noted = asked.clone();
        let class = self.vm.find_class(path).unwrap();
        self.vm
            .register_interface_rule(
                class,
                Rc::new(move |_vm: &mut Vm, this, _iface| {
                    noted.borrow_mut().push(this);
                    Ok(answer)
                }),
            )
            .unwrap();
        asked
    }

    fn ask(&mut self, o: ObjRef) -> Result<Option<ObjRef>, VmError> {
        self.vm.query_interface(o, self.iface)
    }
}

fn not_read(r: Result<Option<ObjRef>, VmError>, words: &str) {
    match r {
        Err(VmError::NotYet(n)) => {
            assert_eq!(n.missing, Missing::NotRead, "{n}");
            assert_eq!(n.record, RECORD);
            assert!(n.to_string().contains(words), "{n}");
        }
        other => panic!("expected NotYet naming {words:?}, got {other:?}"),
    }
}

/// Step 1: a pawn whose class lists the interface answers itself, and so does one that only inherits it, before any
/// controller is asked (theirs here would be NotYet).
#[test]
fn a_pawn_whose_class_lists_the_interface_answers_itself_inherited_too() {
    let mut e = Env::new(ParityMode::Off);
    let mind = e.make("WillowGame.WillowMind", "Mind_0");
    for class in ["Engine.ToyThingPawn", "Engine.ToyInheritingPawn"] {
        let pawn = e.make(class, "Pawn_0");
        e.link(pawn, "Controller", mind);
        assert_eq!(e.ask(pawn).unwrap(), Some(pawn), "{class}");
    }
}

/// Step 2, a set controller: its answer; when it declines, the driven vehicle's controller is not asked and the `Mesh`
/// is; with no mesh, none.
#[test]
fn a_set_controller_answers_and_a_declining_one_leads_to_the_mesh_not_the_vehicle() {
    let mut e = Env::new(ParityMode::Off);
    let answer = e.make("Engine.Object", "Answer_0");
    let pawn = e.make("Engine.Pawn", "Pawn_0");
    let controller = e.make("Engine.ToyAnsweringController", "Controller_0");
    e.link(pawn, "Controller", controller);
    let asked = e.answers("Engine.ToyAnsweringController", Some(answer));
    assert_eq!(e.ask(pawn).unwrap(), Some(answer));
    assert_eq!(*asked.borrow(), [controller]);

    let mut e = Env::new(ParityMode::Off);
    let (pawn, controller) = (e.make("Engine.Pawn", "Pawn_0"), e.make("Engine.ToyAnsweringController", "Declines"));
    let vehicle = e.make("Engine.Vehicle", "Vehicle_0");
    let vehicle_controller = e.make("Engine.ToyAnsweringController", "WouldAnswer");
    e.link(pawn, "Controller", controller);
    e.link(pawn, "DrivenVehicle", vehicle);
    e.link(vehicle, "Controller", vehicle_controller);
    let asked = e.answers("Engine.ToyAnsweringController", None);
    // No mesh: none, and only the pawn's own controller was asked.
    assert_eq!(e.ask(pawn).unwrap(), None);
    assert_eq!(*asked.borrow(), [controller], "the vehicle's controller is not asked");
    let mesh_answer = e.make("Engine.Object", "MeshAnswer");
    let mesh = e.make("Engine.ToyAnsweringMesh", "Mesh_0");
    e.link(pawn, "Mesh", mesh);
    let mesh_asked = e.answers("Engine.ToyAnsweringMesh", Some(mesh_answer));
    assert_eq!(e.ask(pawn).unwrap(), Some(mesh_answer));
    assert_eq!(*mesh_asked.borrow(), [mesh]);
    assert_eq!(*asked.borrow(), [controller, controller], "still never the vehicle's");
}

/// Step 2, no controller: the driven vehicle's controller; a vehicle with no controller leads on to the mesh; nothing
/// set at all answers none.
#[test]
fn with_no_controller_the_driven_vehicle_s_controller_then_the_mesh_then_none() {
    let mut e = Env::new(ParityMode::Off);
    let answer = e.make("Engine.Object", "Answer_0");
    let pawn = e.make("Engine.Pawn", "Pawn_0");
    assert_eq!(e.ask(pawn).unwrap(), None, "nothing to ask: none");
    let vehicle = e.make("Engine.Vehicle", "Vehicle_0");
    e.link(pawn, "DrivenVehicle", vehicle);
    let mesh = e.make("Engine.ToyAnsweringMesh", "Mesh_0");
    e.link(pawn, "Mesh", mesh);
    let mesh_asked = e.answers("Engine.ToyAnsweringMesh", None);
    assert_eq!(e.ask(pawn).unwrap(), None, "a vehicle with no controller: on to the mesh, which declines");
    assert_eq!(*mesh_asked.borrow(), [mesh]);
    let vehicle_controller = e.make("Engine.ToyAnsweringController", "Driver");
    e.link(vehicle, "Controller", vehicle_controller);
    let asked = e.answers("Engine.ToyAnsweringController", Some(answer));
    assert_eq!(e.ask(pawn).unwrap(), Some(answer));
    assert_eq!(*asked.borrow(), [vehicle_controller]);
}

/// The answers the note leaves open are named NotYets in Off, never a guess: `GearboxMind`'s (a `WillowMind`
/// controller, as the Fire dummy's), `WillowPlayerController`'s for `IAIInterface`, and a skeletal mesh component's.
/// They are bound as rules, so a direct question meets the same refusal as a handed-on one (review r1 of #456, item
/// 3); a class with no open answer answers by the one dispatch, its plain table here.
#[test]
fn the_open_answers_are_named_not_yets() {
    let mut e = Env::new(ParityMode::Off);
    let ai = e.vm.find_class(AI_INTERFACE).unwrap();
    let pawn = e.make("Engine.Pawn", "Pawn_0");
    let mind = e.make("WillowGame.WillowMind", "Mind_0");
    e.link(pawn, "Controller", mind);
    not_read(e.ask(pawn), "goes to its Controller, a WillowGame.WillowMind: GearboxMind's own QueryInterface answer");
    not_read(e.ask(mind), "GearboxMind's own QueryInterface answer");
    not_read(e.vm.query_interface(mind, ai), "GearboxMind's own QueryInterface answer");
    let player = e.make("WillowGame.WillowPlayerController", "Player_0");
    e.link(pawn, "Controller", player);
    not_read(e.vm.query_interface(pawn, ai), "WillowPlayerController's QueryInterface answer for IAIInterface");
    not_read(e.vm.query_interface(player, ai), "WillowPlayerController's QueryInterface answer for IAIInterface");
    assert_eq!(e.ask(player).unwrap(), None, "another interface: the plain answer");
    assert_eq!(e.ask(pawn).unwrap(), None, "so the pawn goes on to its mesh, unset: none");
    let plain = e.make("Engine.Controller", "Plain_0");
    e.link(pawn, "Controller", plain);
    assert_eq!(e.vm.query_interface(pawn, ai).unwrap(), None, "a plain controller: its plain answer");
    let lone = e.make("Engine.Pawn", "Lone_0");
    let mesh = e.make("Engine.ToySubMesh", "Mesh_0");
    e.link(lone, "Mesh", mesh);
    // A skeletal mesh component class other than exactly SkeletalMeshComponent keeps the open answer.
    not_read(e.ask(lone), "goes to its Mesh, a Engine.ToySubMesh: what a Engine.ToySubMesh answers");
    not_read(e.ask(mesh), "what a Engine.ToySubMesh answers");
    let vehicle = e.make("Engine.Vehicle", "Vehicle_0");
    e.link(lone, "DrivenVehicle", vehicle);
    e.link(vehicle, "Controller", mind);
    not_read(e.ask(lone), "goes to its driven vehicle's Controller, a WillowGame.WillowMind");
}

/// A question asked twice: a controller whose answer asks the pawn back gets none for it at once, and the pawn takes
/// the controller's none and goes on to its mesh.
#[test]
fn a_question_asked_twice_gets_none_and_the_asker_goes_on() {
    let mut e = Env::new(ParityMode::Off);
    let pawn = e.make("Engine.Pawn", "Pawn_0");
    let controller = e.make("Engine.ToyAnsweringController", "AsksBack");
    let (mesh, mesh_answer) = (e.make("Engine.ToyAnsweringMesh", "Mesh_0"), e.make("Engine.Object", "MeshAnswer"));
    e.link(pawn, "Controller", controller);
    e.link(pawn, "Mesh", mesh);
    let inner = Rc::new(RefCell::new(Vec::new()));
    let seen = inner.clone();
    let class = e.vm.find_class("Engine.ToyAnsweringController").unwrap();
    e.vm
        .register_interface_rule(
            class,
            Rc::new(move |vm: &mut Vm, _this, iface| {
                let back = vm.query_interface(pawn, iface)?;
                seen.borrow_mut().push(back);
                Ok(back)
            }),
        )
        .unwrap();
    e.answers("Engine.ToyAnsweringMesh", Some(mesh_answer));
    assert_eq!(e.ask(pawn).unwrap(), Some(mesh_answer));
    assert_eq!(*inner.borrow(), [None], "the inner question to the pawn got none");
    assert!(!e.vm.interface_asked(pawn, e.iface), "the chain is unwound");
}

/// `WillowAIPawn`: `Pawn`'s rule first; only when it declines and the asked class is `IPlayerMaster`, the player
/// master step, which is NotYet (the field is not identified). Any other class answers as `Pawn` does.
#[test]
fn willow_ai_pawn_adds_only_the_player_master_step() {
    let mut e = Env::new(ParityMode::Off);
    let pawn = e.make("WillowGame.WillowAIPawn", "AIPawn_0");
    assert_eq!(e.ask(pawn).unwrap(), None, "Pawn's rule: nothing to ask");
    let master = e.vm.find_class("WillowGame.IPlayerMaster").unwrap();
    not_read(e.vm.query_interface(pawn, master), "WillowAIPawn's QueryInterface(IPlayerMaster)");
    // Exactly IPlayerMaster (review r1 of #456, item 4): an interface derived from it gets Pawn's none.
    let child = e.vm.find_class("Toy.IToyPlayerMasterChild").unwrap();
    assert_eq!(e.vm.query_interface(pawn, child).unwrap(), None);
    let answer = e.make("Engine.Object", "Answer_0");
    let controller = e.make("Engine.ToyAnsweringController", "Controller_0");
    e.link(pawn, "Controller", controller);
    e.answers("Engine.ToyAnsweringController", Some(answer));
    assert_eq!(e.vm.query_interface(pawn, master).unwrap(), Some(answer), "Pawn's rule answered first");
}

/// `WillowVehicle`'s own rule is not implemented: NotYet, not `Pawn`'s rule.
#[test]
fn a_willow_vehicle_is_not_yet() {
    let mut e = Env::new(ParityMode::Off);
    let vehicle = e.make("WillowGame.WillowVehicle", "Vehicle_0");
    not_read(e.ask(vehicle), "WillowVehicle's own QueryInterface");
}

/// Registering again keeps the first rule; Reference registers nothing, so a pawn gets the plain answer there (the
/// native's use of the rule is `ow_vm`'s `query_interface_takes_a_registered_class_answer`).
#[test]
fn registering_twice_keeps_the_rule_and_reference_registers_none() {
    let mut e = Env::new(ParityMode::Off);
    let pawn = e.make("Engine.Pawn", "Pawn_0");
    let mind = e.make("WillowGame.WillowMind", "Mind_0");
    e.link(pawn, "Controller", mind);
    register(&mut e.vm).unwrap();
    not_read(e.ask(pawn), "GearboxMind's own");
    let mut r = Env::new(ParityMode::Reference);
    let pawn = r.make("Engine.Pawn", "Pawn_0");
    let mind = r.make("WillowGame.WillowMind", "Mind_0");
    r.link(pawn, "Controller", mind);
    let class = r.vm.find_class(PAWN).unwrap();
    assert!(r.vm.interface_rule_for(class).is_none());
    assert_eq!(r.ask(pawn).unwrap(), None, "Reference: the plain answer");
}

// ---------------------------------------------------------------------------------------------------------------
// A component's answer (NATIVE_PAWN_AI_INTERFACE.md section 6, with the hub's rulings on review of #512; lane
// IMPL-LINK11-QUERY-INTERFACE)
// ---------------------------------------------------------------------------------------------------------------

/// Pawn first, with the mesh's ownership known and no controller: the pawn asks its mesh, the mesh's plain answer is
/// none, its `Owner` is the pawn, which is still answering the same question, so the VM's guard gives none at once.
/// The pawn's answer is none, and nothing is written.
#[test]
fn a_pawn_s_mesh_owned_by_the_pawn_meets_the_cycle_guard() {
    let mut e = Env::new(ParityMode::Off);
    let pawn = e.make("Engine.Pawn", "Pawn_0");
    let mesh = e.make("Engine.SkeletalMeshComponent", "Mesh_0");
    e.link(pawn, "Mesh", mesh);
    e.link(mesh, "Owner", pawn);
    let before: Vec<_> = [pawn, mesh].iter().map(|&o| e.vm.space.heap.get(o).unwrap().slots.clone()).collect();
    assert_eq!(e.ask(pawn).unwrap(), None);
    let after: Vec<_> = [pawn, mesh].iter().map(|&o| e.vm.space.heap.get(o).unwrap().slots.clone()).collect();
    assert_eq!(before, after, "no writes");
}

/// A direct mesh query (the note's section 6.3, case 2): the mesh's `Owner` is a pawn whose `Controller` lists the
/// interface, so the pawn answers through its controller before any cycle, and the mesh answers that controller.
/// `Owner` None answers none.
#[test]
fn a_direct_mesh_query_forwards_to_its_owner() {
    let mut e = Env::new(ParityMode::Off);
    let pawn = e.make("Engine.Pawn", "Pawn_0");
    let controller = e.make("Engine.ToyThingController", "Controller_0");
    e.link(pawn, "Controller", controller);
    let mesh = e.make("Engine.SkeletalMeshComponent", "Mesh_0");
    e.link(mesh, "Owner", pawn);
    assert_eq!(e.ask(mesh).unwrap(), Some(controller));
    set(&mut e.vm, mesh, "Owner", Value::Object(None)).unwrap();
    assert_eq!(e.ask(mesh).unwrap(), None);
}

/// The exact-class test: a `SkeletalMeshComponent` subclass and another component class get the open `NotRead`, not
/// the rule, even with an `Owner` that would answer.
#[test]
fn other_component_classes_get_the_open_answer() {
    let mut e = Env::new(ParityMode::Off);
    let owner = e.make("Engine.ToyThingPawn", "Owner_0");
    let sub = e.make("Engine.ToySubMesh", "Sub_0");
    e.link(sub, "Owner", owner);
    not_read(
        e.ask(sub),
        "what a Engine.ToySubMesh answers to QueryInterface is not read (asked of Engine.ToySubMesh for Engine.IToyThing)",
    );
    let plain = e.make("Engine.ToyPlainComponent", "Plain_0");
    not_read(e.ask(plain), "what a Engine.ToyPlainComponent answers to QueryInterface is not read");
}

/// A `NotYet` from the plain answer passes through unchanged, before `Owner` is read: the receiver's class is made an
/// engine-native stand-in, whose derivation the plain answer cannot decide.
#[test]
fn a_not_yet_from_the_plain_answer_passes_through() {
    let mut e = Env::new(ParityMode::Off);
    let owner = e.make("Engine.ToyThingPawn", "Owner_0");
    let mesh = e.make("Engine.SkeletalMeshComponent", "Mesh_0");
    e.link(mesh, "Owner", owner);
    let class = e.vm.find_class("Engine.SkeletalMeshComponent").unwrap();
    e.vm.space.classes.mark_native(class).unwrap();
    match e.ask(mesh).unwrap_err() {
        VmError::NotYet(n) => {
            // Unchanged: the VM's own kind, record and text, with no step prefix of this rule's.
            assert_eq!(n.missing, Missing::NotRead);
            assert_eq!(n.record, "decision 2026-11-15-08 (engine-native stand-in classes)");
            assert!(n.what.starts_with("whether a class derives from another through the engine-native class"), "{n}");
        }
        other => panic!("expected the plain answer's NotYet, got {other:?}"),
    }
}

fn script_text(r: Result<Option<ObjRef>, VmError>) -> String {
    match r.unwrap_err() {
        VmError::Script { message, .. } => message,
        other => panic!("expected a Script error, got {other:?}"),
    }
}

/// `Owner` missing and mistyped: the note's two `VmError::Script` texts.
#[test]
fn a_missing_or_mistyped_owner_is_a_script_error() {
    let mut e = Env::new(ParityMode::Off);
    let component = e.make("Engine.ActorComponent", "Component_0");
    assert_eq!(script_text(e.ask(component)), "ActorComponent.QueryInterface: Engine.ActorComponent declares no Owner");
    let mut e = Env::with(ParityMode::Off, true);
    let component = e.make("Engine.ActorComponent", "Component_0");
    assert_eq!(
        script_text(e.ask(component)),
        "ActorComponent.QueryInterface: Engine.ActorComponent.Owner is not an object reference to Engine.Actor"
    );
}

/// An unloaded `Owner`: `NotYet { NotRead }` under this note, and nothing loaded (a load is a store write).
#[test]
fn an_unloaded_owner_is_not_read_and_not_loaded() {
    let mut e = Env::new(ParityMode::Off);
    let path = ow_core::ObjectPath::parse("Engine.ToyHolder_0").unwrap();
    let holder = e.vm.defs.find(&mut e.vm.space, &path).unwrap().unwrap();
    let lazy = crate::hit::access::object(&mut e.vm, holder, "Held").unwrap().expect("a held pawn");
    assert!(e.vm.defs.is_unloaded(&e.vm.space, lazy), "the fixture's pawn is a lazy handle");
    let mesh = e.make("Engine.SkeletalMeshComponent", "Mesh_0");
    e.link(mesh, "Owner", lazy);
    not_read(e.ask(mesh), "ActorComponent.QueryInterface: the Owner Engine.ToyLazyPawn_0 of");
    assert!(e.vm.defs.is_unloaded(&e.vm.space, lazy), "still not loaded");
}

/// Reference registers none of it: a mesh gives its plain answer.
#[test]
fn reference_keeps_the_plain_component_answer() {
    let mut e = Env::new(ParityMode::Reference);
    let owner = e.make("Engine.ToyThingPawn", "Owner_0");
    let mesh = e.make("Engine.SkeletalMeshComponent", "Mesh_0");
    e.link(mesh, "Owner", owner);
    assert_eq!(e.ask(mesh).unwrap(), None);
}
