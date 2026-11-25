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
use ow_object::layout::{PropKind, PropertyDecl};
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
/// The fixture's `Owner` declarations.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Owners {
    /// `ActorComponent` declares none; `SkeletalMeshComponent` declares one to `Engine.Actor`.
    Plain,
    /// `ActorComponent` declares `Owner` as an int.
    Mistyped,
    /// `SkeletalMeshComponent` declares `Owner` to `ToyOwnerActor`, a subclass of `Engine.Actor` never instantiated, so
    /// the class table holds it only as a reserved name until something defines it (review r1 of #524, item 3).
    Reserved,
    /// As [`Owners::Reserved`], with `ToyOwnerActor` also declaring a property of a kind the port cannot lay out: its
    /// ancestry is readable from the class records, its layout is not (review r2 of #524, item 1).
    Unsupported,
}

/// [`engine`] with the `Owner` declarations of `owners`.
fn engine_with(owners: Owners, attached: Attached) -> Vec<u8> {
    let mistyped = owners == Owners::Mistyped;
    let mut t =
        ScriptToy::with_imports(&["Class", "ObjectProperty", "IntProperty", "ToyUnsupportedProperty", "BoolProperty"]);
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
    // `Controller`'s rule reads these (lane IMPL-GEARBOXMIND-QUERYINTERFACE).
    for field in ["Pawn", "PlayerReplicationInfo"] {
        t.prop("Object", controller, field, 0, object);
    }
    real_class(&mut t, "Vehicle", pawn, object, 2, &[]);
    // `ActorComponent` without `Owner` (for the missing-declaration case), `SkeletalMeshComponent` under it declaring
    // `Owner`, a subclass of it and another component class (lane IMPL-LINK11-QUERY-INTERFACE).
    let actor_component = real_class(&mut t, "ActorComponent", object, object, 2, &[]);
    if mistyped {
        t.prop("Int", actor_component, "Owner", 0, 0);
    }
    // `bAttached`, declared on `ActorComponent` (NATIVE_WILLOWAICOMPONENT_QUERYINTERFACE.md r1), as `attached` says.
    match attached {
        Attached::Bool => t.prop("Bool", actor_component, "bAttached", 0, 0),
        Attached::Int => t.prop("Int", actor_component, "bAttached", 0, 0),
        Attached::Missing => 0,
    };
    let component = real_class(&mut t, "SkeletalMeshComponent", actor_component, object, 2, &[]);
    let owner_target = if matches!(owners, Owners::Reserved | Owners::Unsupported) {
        let target = real_class(&mut t, "ToyOwnerActor", actor, object, 2, &[]);
        if owners == Owners::Unsupported {
            t.prop("ToyUnsupported", target, "Odd", 0, 0);
        }
        target
    } else {
        actor
    };
    t.prop("Object", component, "Owner", 0, owner_target);
    real_class(&mut t, "ToySubMesh", component, object, 2, &[]);
    real_class(&mut t, "ToyPlainComponent", actor_component, object, 2, &[]);
    real_class(&mut t, "ToyAnsweringController", controller, object, 2, &[]);
    real_class(&mut t, "ToyAnsweringMesh", component, object, 2, &[]);
    // A holder whose `Held` names a pawn export, which the store keeps as an unloaded handle until it is loaded.
    // It also names a skeletal mesh export, an unloaded receiver for the component rule (review r1 of #524, item 1).
    let holder = real_class(&mut t, "ToyHolder", object, object, 2, &[]);
    t.prop("Object", holder, "Held", 0, actor);
    t.prop("Object", holder, "HeldMesh", 0, component);
    let held = t.p.next_export() + 1;
    let mut payload = ints(&[0]);
    payload.extend(t.p.object_tag("Held", held));
    payload.extend(t.p.object_tag("HeldMesh", held + 1));
    payload.extend_from_slice(&t.none);
    t.p.add_export_with(Export::new(holder, "ToyHolder_0", payload));
    let mut payload = ints(&[0]);
    payload.extend_from_slice(&t.none);
    t.p.add_export_with(Export::new(pawn, "ToyLazyPawn_0", payload));
    let mut payload = ints(&[0]);
    payload.extend_from_slice(&t.none);
    t.p.add_export_with(Export::new(component, "ToyLazyMesh_0", payload));
    t.build()
}

/// How the invented `ActorComponent` declares `bAttached`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Attached {
    Bool,
    Int,
    Missing,
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
        Env::with(mode, Owners::Plain)
    }

    fn with(mode: ParityMode, owners: Owners) -> Env {
        Env::build(mode, owners, Attached::Bool)
    }

    /// The world with `bAttached` on the AI components declared as `attached` says.
    fn with_attached(attached: Attached) -> Env {
        Env::build(ParityMode::Off, Owners::Plain, attached)
    }

    fn build(mode: ParityMode, owners: Owners, attached: Attached) -> Env {
        let dir = std::env::temp_dir().join(format!(
            "ow-combat-pawn-interface-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let engine = engine_with(owners, attached);
        let defs =
            DefStore::from_synthetic(&dir, &[("Engine.upk", engine.as_slice())], IdentityPolicy::FirstInScanOrder, mode)
                .unwrap();
        let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), mode);
        vm.defs.code_mut().1.load(&PackageName::parse("Engine").unwrap()).unwrap();
        let class = |vm: &mut Vm, path: &str| vm.find_class(path).unwrap();
        let (object, pawn, controller) =
            (class(&mut vm, "Engine.Object"), class(&mut vm, "Engine.Pawn"), class(&mut vm, "Engine.Controller"));
        let vehicle = class(&mut vm, "Engine.Vehicle");
        let mind = vm.space.classes.define("GearboxFramework.GearboxMind", Some(controller), vec![]).unwrap();
        let my_pawn = PropertyDecl::new(vm.space.names.intern("MyWillowPawn"), PropKind::Object { class: object });
        vm.space.classes.define("WillowGame.WillowMind", Some(mind), vec![my_pawn]).unwrap();
        let classes = &mut vm.space.classes;
        classes.define("WillowGame.WillowPlayerController", Some(controller), vec![]).unwrap();
        classes.define("WillowGame.WillowAIPawn", Some(pawn), vec![]).unwrap();
        classes.define("WillowGame.WillowVehicle", Some(vehicle), vec![]).unwrap();
        let master = classes.define("WillowGame.IPlayerMaster", Some(object), vec![]).unwrap();
        classes.define("Toy.IToyPlayerMasterChild", Some(master), vec![]).unwrap();
        classes.define("GearboxFramework.IAIInterface", Some(object), vec![]).unwrap();
        // The AI components (lane IMPL-WILLOWAICOMPONENT-QUERYINTERFACE), at their installed paths under the decoded
        // `ActorComponent` (whose `bAttached` they inherit): `Owner` to `Engine.Actor` on `AIComponent`,
        // `WillowAIComponent` under it, and a subclass of that.
        let (actor, actor_component) = (class(&mut vm, "Engine.Actor"), class(&mut vm, "Engine.ActorComponent"));
        let decls = vec![ow_object::layout::PropertyDecl::new(
            vm.space.names.intern("Owner"),
            ow_object::layout::PropKind::Object { class: actor },
        )];
        let classes = &mut vm.space.classes;
        let ai = classes.define(AI_COMPONENT, Some(actor_component), decls).unwrap();
        let willow = classes.define(WILLOW_AI_COMPONENT, Some(ai), vec![]).unwrap();
        classes.define("WillowGame.ToySubWillowAIComponent", Some(willow), vec![]).unwrap();
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

/// A `NotYet { NotRead }` of the read-only dispatch's own, met through `Controller`'s read-only rule, naming `words`
/// (its record is the dispatch's since review r2 of #648).
fn not_read_gm(r: Result<Option<ObjRef>, VmError>, words: &str) {
    match r {
        Err(VmError::NotYet(n)) => {
            assert_eq!(n.missing, Missing::NotRead, "{n}");
            assert_eq!(n.record, ow_vm::api::DISPATCH_RECORD);
            assert!(n.what.contains(words), "{n}");
        }
        other => panic!("expected NotYet naming {words:?}, got {other:?}"),
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
    // A mind answers by `Controller`'s rule now (lane IMPL-GEARBOXMIND-QUERYINTERFACE; its own tests are in
    // `read_only_tests`): this hand-defined one lists nothing and has no pawn, so it declines, and so does the pawn.
    assert_eq!(e.ask(pawn).unwrap(), None);
    assert_eq!(e.ask(mind).unwrap(), None);
    assert_eq!(e.vm.query_interface(mind, ai).unwrap(), None);
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
    // The vehicle's controller (the mind) declines, so the pawn goes on to its mesh, whose answer is still open.
    not_read(e.ask(lone), "goes to its Mesh, a Engine.ToySubMesh");
    // The read-only chain (lane IMPL-GEARBOXMIND-QUERYINTERFACE) reaching a `SkeletalMeshComponent`, whose component
    // rule (#524) has no read-only form: `NotPorted` (the hub's ruling in review r1 of #602, item 1).
    let held = e.make("Engine.Pawn", "Held_0");
    let skeletal = e.make("Engine.SkeletalMeshComponent", "Skeletal_0");
    e.link(held, "Mesh", skeletal);
    let asker = e.make("Engine.Controller", "Asker_0");
    e.link(asker, "Pawn", held);
    let iface = e.iface;
    match read_only::query_read_only(&e.vm, asker, Some(iface)) {
        Err(VmError::NotYet(n)) => {
            assert_eq!(n.missing, Missing::NotPorted, "{n}");
            assert!(n.what.contains("goes to its pawn, a Engine.Pawn"), "{n}");
            assert!(n.what.contains("Engine.SkeletalMeshComponent's QueryInterface rule is not ported read-only"), "{n}");
        }
        other => panic!("expected NotPorted, got {other:?}"),
    }
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
    assert_eq!(e.ask(pawn).unwrap(), None, "the mind's Controller rule declines; the pawn has no mesh");
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
    // The controller answers by `Controller`'s read-only rule (lane IMPL-GEARBOXMIND-QUERYINTERFACE), which reads only
    // interface entries resolved at set-up: cold, it is `NotRead`; resolved, the answer is the controller.
    not_read_gm(e.ask(mesh), "the interface table of Engine.ToyThingController has an entry not resolved at set-up");
    let class = e.vm.space.heap.get(controller).unwrap().class;
    e.vm.resolve_interface_tables(class);
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
    let mut e = Env::with(ParityMode::Off, Owners::Mistyped);
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

/// The holder's lazy handles: (the pawn, the skeletal mesh), both still unloaded.
fn lazy_handles(e: &mut Env) -> (ObjRef, ObjRef) {
    let path = ow_core::ObjectPath::parse("Engine.ToyHolder_0").unwrap();
    let holder = e.vm.defs.find(&mut e.vm.space, &path).unwrap().unwrap();
    let pawn = crate::hit::access::object(&mut e.vm, holder, "Held").unwrap().expect("a held pawn");
    let mesh = crate::hit::access::object(&mut e.vm, holder, "HeldMesh").unwrap().expect("a held mesh");
    for o in [pawn, mesh] {
        assert!(e.vm.defs.is_unloaded(&e.vm.space, o), "the fixture's handles are lazy");
    }
    (pawn, mesh)
}

/// Review r1 of #524, item 1: the unloaded-receiver check runs only after the plain answer returns None. An unloaded
/// exact-class receiver whose class derives from the asked class answers itself; a plain `NotYet` passes through; a
/// plain None meets the check. Nothing is loaded.
#[test]
fn an_unloaded_receiver_is_checked_only_after_a_plain_none() {
    let mut e = Env::new(ParityMode::Off);
    let (_, mesh) = lazy_handles(&mut e);
    let base = e.vm.find_class("Engine.ActorComponent").unwrap();
    assert_eq!(e.vm.query_interface(mesh, base).unwrap(), Some(mesh), "the plain answer");
    not_read(e.ask(mesh), "ActorComponent.QueryInterface: the component Engine.ToyLazyMesh_0 is an unloaded definition handle");
    let class = e.vm.find_class("Engine.SkeletalMeshComponent").unwrap();
    e.vm.space.classes.mark_native(class).unwrap();
    match e.ask(mesh).unwrap_err() {
        VmError::NotYet(n) => assert_eq!(n.record, "decision 2026-11-15-08 (engine-native stand-in classes)"),
        other => panic!("expected the plain answer's NotYet, got {other:?}"),
    }
    assert!(e.vm.defs.is_unloaded(&e.vm.space, mesh), "still not loaded");
}

/// Review r1 of #524, item 2: forwarding loads nothing. A mesh forwards to a loaded pawn with no controller whose
/// `DrivenVehicle` is an unloaded handle: the pawn rule's next read needs it, so the answer is a typed `NotRead`, and the
/// vehicle stays unloaded.
#[test]
fn owner_forwarding_never_loads_a_definition() {
    let mut e = Env::new(ParityMode::Off);
    let (vehicle, _) = lazy_handles(&mut e);
    let pawn = e.make("Engine.Pawn", "Pawn_0");
    e.link(pawn, "DrivenVehicle", vehicle);
    let mesh = e.make("Engine.SkeletalMeshComponent", "Mesh_0");
    e.link(mesh, "Owner", pawn);
    not_read(e.ask(mesh), "the pawn QueryInterface rule: reading Controller of Engine.ToyLazyPawn_0, an unloaded definition handle");
    assert!(e.vm.defs.is_unloaded(&e.vm.space, vehicle), "the vehicle stays unloaded");
}

/// Review r1 of #524, item 3: an `Owner` declared to a subclass of `Engine.Actor` that nothing instantiates is a valid
/// declaration, its ancestry read from the class metadata at registration: `Owner` None answers None, never a Script
/// error.
#[test]
fn an_owner_declared_to_an_uninstantiated_actor_subclass_is_valid() {
    let mut e = Env::with(ParityMode::Off, Owners::Reserved);
    let mesh = e.make("Engine.SkeletalMeshComponent", "Mesh_0");
    assert_eq!(e.ask(mesh).unwrap(), None);
}

/// Review r2 of #524, item 1: the `Owner` target's ancestry is read from the class records alone, so a target whose
/// layout the port cannot build does not fail the registration, and `Owner` None still answers None.
#[test]
fn an_owner_target_with_an_unsupported_layout_is_judged_from_its_class_records() {
    let mut e = Env::with(ParityMode::Off, Owners::Unsupported);
    let target = e.vm.space.classes.by_path("Engine.ToyOwnerActor");
    assert!(
        target.is_none_or(|t| !e.vm.space.classes.get_layout(t).is_some_and(|l| l.is_defined())),
        "the target's layout was never built"
    );
    let mesh = e.make("Engine.SkeletalMeshComponent", "Mesh_0");
    assert_eq!(e.ask(mesh).unwrap(), None);
    // The fixture is what it claims: building the target's layout fails.
    assert!(e.vm.find_class("Engine.ToyOwnerActor").is_err(), "the target's layout is unsupported");
}

/// Review r2 of #524, item 2: a refusal names its object best effort. A component whose outer chain is longer than
/// `PathName`'s limit, with an unloaded `Owner`, still gives the typed `NotRead`, naming the component by its own name.
#[test]
fn a_refusal_names_a_deep_component_by_its_leaf_name() {
    let mut e = Env::new(ParityMode::Off);
    let (lazy, _) = lazy_handles(&mut e);
    let class = e.vm.find_class("Engine.Object").unwrap();
    let mut outer = e.vm.space.alloc(class, "Link_0", None, ow_object::heap::Origin::Runtime).unwrap();
    for i in 1..80 {
        outer = e.vm.space.alloc(class, &format!("Link_{i}"), Some(outer), ow_object::heap::Origin::Runtime).unwrap();
    }
    let mesh_class = e.vm.find_class("Engine.SkeletalMeshComponent").unwrap();
    let mesh = e.vm.space.alloc(mesh_class, "DeepMesh_0", Some(outer), ow_object::heap::Origin::Runtime).unwrap();
    let defaults = e.vm.defs.class_default(&mut e.vm.space, "Engine.SkeletalMeshComponent").unwrap();
    let slots = e.vm.space.heap.get(defaults).unwrap().slots.clone();
    e.vm.space.heap.get_mut(mesh).unwrap().slots = slots;
    e.link(mesh, "Owner", lazy);
    assert!(ow_vm::path_name::path_name(&e.vm, mesh).is_err(), "the chain is past PathName's limit");
    not_read(e.ask(mesh), "of DeepMesh_0 is an unloaded definition handle");
}

// ---------------------------------------------------------------------------------------------------------------
// The AI components (NATIVE_WILLOWAICOMPONENT_QUERYINTERFACE.md, with the hub's ruling on an unattached component;
// lane IMPL-WILLOWAICOMPONENT-QUERYINTERFACE)
// ---------------------------------------------------------------------------------------------------------------

/// #518's counters: heap committed writes, generator draws and definition-store load transitions.
fn counters(vm: &Vm) -> (u64, u64, u64) {
    (vm.space.heap.committed_writes(), vm.rng.draws(), (vm.defs.load_transitions() + vm.class_code_loads()))
}

impl Env {
    /// The question, checked write-, load- and draw-free.
    fn ask_counted(&mut self, o: ObjRef) -> Result<Option<ObjRef>, VmError> {
        let before = counters(&self.vm);
        let r = self.ask(o);
        assert_eq!(counters(&self.vm), before, "no write, draw or load: {r:?}");
        r
    }

    fn attach(&mut self, o: ObjRef, attached: bool) {
        set(&mut self.vm, o, "bAttached", Value::Bool(attached)).unwrap();
    }
}

fn ai_not_read(r: Result<Option<ObjRef>, VmError>, words: &str) {
    match r {
        Err(VmError::NotYet(n)) => {
            assert_eq!(n.missing, Missing::NotRead, "{n}");
            assert_eq!(n.record, AI_COMPONENT_RECORD, "{n}");
            assert!(n.what.contains(words), "{n}");
        }
        other => panic!("expected NotYet NotRead naming {words:?}, got {other:?}"),
    }
}

/// A spawned mind's component (#605 sets its `Owner`): both exact classes take `ActorComponent`'s rule and answer
/// what their `Owner` answers, attached or not (`bAttached` is read only when `Owner` is None). A subclass keeps the
/// open answer, even with an `Owner` that would answer.
#[test]
fn an_owned_ai_component_answers_its_owner_s_answer() {
    let mut e = Env::new(ParityMode::Off);
    let pawn = e.make("Engine.Pawn", "Pawn_0");
    let controller = e.make("Engine.ToyThingController", "Controller_0");
    e.link(pawn, "Controller", controller);
    // The controller answers by `Controller`'s read-only rule (#602), which reads only interface entries resolved at
    // set-up, so they are resolved first (hub ruling at landing, 2026-11-20).
    let class = e.vm.space.heap.get(controller).unwrap().class;
    e.vm.resolve_interface_tables(class);
    for path in [AI_COMPONENT, WILLOW_AI_COMPONENT] {
        let component = e.make(path, "AIComponent_0");
        e.link(component, "Owner", pawn);
        for attached in [false, true] {
            e.attach(component, attached);
            assert_eq!(e.ask_counted(component).unwrap(), Some(controller), "{path}, attached {attached}");
        }
    }
    let sub = e.make("WillowGame.ToySubWillowAIComponent", "Sub_0");
    e.link(sub, "Owner", pawn);
    not_read(e.ask_counted(sub), "what a WillowGame.ToySubWillowAIComponent answers to QueryInterface is not read");
}

/// The rest of the note's component cases, for both exact classes: a class the component derives from is its plain
/// answer (itself) before `Owner` is read; an `Owner` that declines gives none; a question that comes back to the
/// component (its owner pawn asks its `Mesh`, the component) gives none at once.
#[test]
fn an_ai_component_s_plain_answer_a_declining_owner_and_a_question_back() {
    let mut e = Env::new(ParityMode::Off);
    let base = e.vm.find_class("Engine.ActorComponent").unwrap();
    for path in [AI_COMPONENT, WILLOW_AI_COMPONENT] {
        let component = e.make(path, "AIComponent_0");
        let before = counters(&e.vm);
        assert_eq!(e.vm.query_interface(component, base).unwrap(), Some(component), "{path}: the plain answer");
        assert_eq!(counters(&e.vm), before);
        let pawn = e.make("Engine.Pawn", "Pawn_0");
        e.link(component, "Owner", pawn);
        assert_eq!(e.ask_counted(component).unwrap(), None, "{path}: the owner declines");
        e.link(pawn, "Mesh", component);
        assert_eq!(e.ask_counted(component).unwrap(), None, "{path}: the question back gets none");
        assert!(!e.vm.interface_asked(component, e.iface), "the chain is unwound");
    }
}

/// A map-placed mind's component (no spawn sets `Owner`): `Owner` None on an unattached AI component is the hub's
/// `NotYet { NotRead }`; an attached one answers none. `ActorComponent` and `SkeletalMeshComponent` keep "`Owner` None
/// answers none" with `bAttached` false: the ruling does not reach them.
#[test]
fn an_unowned_ai_component_is_refused_unless_attached() {
    let mut e = Env::new(ParityMode::Off);
    let mesh = e.make("Engine.SkeletalMeshComponent", "Mesh_0");
    e.attach(mesh, false);
    assert_eq!(e.ask_counted(mesh).unwrap(), None, "SkeletalMeshComponent: Owner None answers none");
    let mut e = Env::with(ParityMode::Off, Owners::Reserved);
    let mesh = e.make("Engine.SkeletalMeshComponent", "Mesh_0");
    e.attach(mesh, false);
    assert_eq!(e.ask_counted(mesh).unwrap(), None, "SkeletalMeshComponent (another Owner target): none");
    for path in [AI_COMPONENT, WILLOW_AI_COMPONENT] {
        let component = e.make(path, "AIComponent_0");
        e.attach(component, false);
        ai_not_read(
            e.ask_counted(component),
            "has no Owner and is not attached: the attach step that sets Owner is not ported (asked for Engine.IToyThing)",
        );
        e.attach(component, true);
        assert_eq!(e.ask_counted(component).unwrap(), None, "{path}: attached, Owner None answers none");
    }
}

/// `bAttached` undeclared or not a bool is a `VmError::Script`, and only once `Owner` reads None: an owned component
/// still answers its owner's answer.
#[test]
fn a_missing_or_mistyped_b_attached_is_a_script_error_only_when_unowned() {
    for attached in [Attached::Missing, Attached::Int] {
        let mut e = Env::with_attached(attached);
        for path in [AI_COMPONENT, WILLOW_AI_COMPONENT] {
            let component = e.make(path, "AIComponent_0");
            assert_eq!(
                script_text(e.ask_counted(component)),
                format!("ActorComponent.QueryInterface: {path}.bAttached is not declared as a bool"),
                "{attached:?}"
            );
            let owner = e.make("Engine.ToyThingPawn", "Owner_0");
            e.link(component, "Owner", owner);
            assert_eq!(e.ask_counted(component).unwrap(), Some(owner), "{attached:?}: Owner set, bAttached not read");
        }
        // The other component classes never read it.
        let mesh = e.make("Engine.SkeletalMeshComponent", "Mesh_0");
        assert_eq!(e.ask_counted(mesh).unwrap(), None, "{attached:?}");
    }
}
