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
    let mut t = ScriptToy::with_imports(&["Class", "ObjectProperty"]);
    let object = real_class(&mut t, "Object", 0, 1, 2, &[]);
    let thing = real_class(&mut t, "IToyThing", object, object, INTERFACE, &[]);
    let pawn = real_class(&mut t, "Pawn", object, object, 2, &[]);
    for field in ["Controller", "DrivenVehicle", "Mesh"] {
        t.prop("Object", pawn, field, 0, object);
    }
    let thing_pawn = real_class(&mut t, "ToyThingPawn", pawn, object, 2, &[thing]);
    real_class(&mut t, "ToyInheritingPawn", thing_pawn, object, 2, &[]);
    let controller = real_class(&mut t, "Controller", object, object, 2, &[]);
    real_class(&mut t, "Vehicle", pawn, object, 2, &[]);
    let component = real_class(&mut t, "SkeletalMeshComponent", object, object, 2, &[]);
    real_class(&mut t, "ToyAnsweringController", controller, object, 2, &[]);
    real_class(&mut t, "ToyAnsweringMesh", component, object, 2, &[]);
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
        let dir = std::env::temp_dir().join(format!(
            "ow-combat-pawn-interface-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let engine = engine();
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
        classes.define("WillowGame.IPlayerMaster", Some(object), vec![]).unwrap();
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
/// controller, as the Fire dummy's), `WillowPlayerController`'s, a skeletal mesh component's, and any other class's
/// own answer.
#[test]
fn the_open_answers_are_named_not_yets() {
    let mut e = Env::new(ParityMode::Off);
    let pawn = e.make("Engine.Pawn", "Pawn_0");
    let mind = e.make("WillowGame.WillowMind", "Mind_0");
    e.link(pawn, "Controller", mind);
    not_read(e.ask(pawn), "goes to its Controller, a WillowGame.WillowMind: GearboxMind's own QueryInterface answer");
    let player = e.make("WillowGame.WillowPlayerController", "Player_0");
    e.link(pawn, "Controller", player);
    not_read(e.ask(pawn), "WillowPlayerController's QueryInterface answer");
    let plain = e.make("Engine.Controller", "Plain_0");
    e.link(pawn, "Controller", plain);
    not_read(e.ask(pawn), "the QueryInterface answer of Engine.Controller is not read");
    let lone = e.make("Engine.Pawn", "Lone_0");
    let mesh = e.make("Engine.SkeletalMeshComponent", "Mesh_0");
    e.link(lone, "Mesh", mesh);
    not_read(e.ask(lone), "goes to its Mesh, a Engine.SkeletalMeshComponent: what a skeletal mesh component answers");
    let vehicle = e.make("Engine.Vehicle", "Vehicle_0");
    e.link(lone, "DrivenVehicle", vehicle);
    e.link(vehicle, "Controller", plain);
    not_read(e.ask(lone), "goes to its driven vehicle's Controller");
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
