//! Tests of `Controller`'s `QueryInterface` rule and the read-only dispatch, on invented classes and objects (AGENTS.md
//! rule 9), one per case of NATIVE_GEARBOXMIND_QUERYINTERFACE.md section 5. AI-assisted (Claude Code).
//!
//! The classes are decoded class exports of an invented `Engine.upk`, so their interface tables are read as the
//! installed ones are. `ToyObjects.upk` holds a pawn export whose `DrivenVehicle` names a vehicle export, so reading
//! it from the loaded pawn gives the store's unloaded handle. The production [`register`] binds `Engine.Controller`,
//! `Engine.PlayerController` and `Engine.Pawn`; the toy mind (`MyWillowPawn`), the toy `WillowAIPawn` and a class
//! with no read-only form are registered by the tests. Every class's table is resolved at set-up except
//! `ColdController`'s. Each refusal checks that the heap and the interface tables are as they were.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::registry::AttributeSystem;
use ow_core::{Missing, PackageName, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_object::ids::{ClassId, ObjRef, PropAddr};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_package::testkit::reflect::{ClassBodySpec, class_body};
use ow_package::testkit::script::ScriptToy;
use ow_package::testkit::{Export, ints};
use ow_vm::api::{InterfaceAsking, Vm};
use ow_vm::error::VmError;

use super::*;
use crate::hit::access::set;
use crate::pawn_interface::{pawn_answer, register, willow_ai_pawn_answer};

static NEXT: AtomicU32 = AtomicU32::new(0);

const INTERFACE: u32 = 2 | 0x4000;
/// A native class's flags (the fitted native bit, `ow_object::class_body::CLASS_NATIVE`).
const NATIVE: u32 = 2 | ow_object::class_body::CLASS_NATIVE;

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

/// The invented `Engine.upk` (module notes).
fn engine() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class", "ObjectProperty"]);
    let object = real_class(&mut t, "Object", 0, 1, 2, &[]);
    let thing = real_class(&mut t, "IToyThing", object, object, INTERFACE, &[]);
    let world = real_class(&mut t, "IWorldBody", object, object, INTERFACE, &[]);
    real_class(&mut t, "ISoundBehavior", object, object, INTERFACE, &[]);
    let other = real_class(&mut t, "IOther", object, object, INTERFACE, &[]);
    let controller = real_class(&mut t, "Controller", object, object, 2, &[]);
    for field in ["Pawn", "PlayerReplicationInfo"] {
        t.prop("Object", controller, field, 0, object);
    }
    real_class(&mut t, "ListingController", controller, object, 2, &[thing, world]);
    real_class(&mut t, "OtherController", controller, object, 2, &[other]);
    real_class(&mut t, "ColdController", controller, object, 2, &[other]);
    real_class(&mut t, "PlayerController", controller, object, 2, &[]);
    // Lane FOLLOWUP-602-CLASS-POLICY: a native class under `Controller` whose answer was not read, listing the
    // interface, and its script child; and a native class for the accessor-apart case.
    let native = real_class(&mut t, "NativeController", controller, object, NATIVE, &[thing]);
    real_class(&mut t, "ScriptUnderNative", native, object, 2, &[]);
    real_class(&mut t, "AccessorController", controller, object, NATIVE, &[thing]);
    let mind = real_class(&mut t, "ToyMind", controller, object, 2, &[]);
    t.prop("Object", mind, "MyWillowPawn", 0, object);
    real_class(&mut t, "BareController", object, object, 2, &[thing]);
    let pawn = real_class(&mut t, "Pawn", object, object, 2, &[]);
    for field in ["Controller", "DrivenVehicle", "Mesh"] {
        t.prop("Object", pawn, field, 0, object);
    }
    real_class(&mut t, "ThingPawn", pawn, object, 2, &[thing]);
    real_class(&mut t, "WorldPawn", pawn, object, 2, &[world]);
    real_class(&mut t, "ToyAIPawn", pawn, object, 2, &[]);
    real_class(&mut t, "EchoPawn", pawn, object, 2, &[]);
    real_class(&mut t, "Vehicle", pawn, object, 2, &[]);
    real_class(&mut t, "ToyPRI", object, object, 2, &[thing]);
    real_class(&mut t, "ToyNotPorted", object, object, 2, &[]);
    t.build()
}

/// `ToyObjects.upk`: `Holder`, a `ToyAIPawn` whose `DrivenVehicle` names `Vehicle_1`.
fn objects() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class"]);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let vehicle = t.p.add_import("Core", "Class", engine, "Vehicle");
    let ai_pawn = t.p.add_import("Core", "Class", engine, "ToyAIPawn");
    let mut v = ints(&[0]);
    v.extend_from_slice(&t.none);
    let v = t.p.add_export(vehicle, "Vehicle_1", v);
    let mut holder = ints(&[0]);
    holder.extend(t.p.tag("DrivenVehicle", "ObjectProperty", &ints(&[v])));
    holder.extend_from_slice(&t.none);
    t.p.add_export(ai_pawn, "Holder", holder);
    t.p.build()
}

/// The classes the tests resolve at set-up (all but `ColdController`).
const RESOLVED: [&str; 21] = [
    "Engine.NativeController",
    "Engine.ScriptUnderNative",
    "Engine.AccessorController",
    "Engine.Object",
    "Engine.IToyThing",
    "Engine.IWorldBody",
    "Engine.ISoundBehavior",
    "Engine.IOther",
    "Engine.Controller",
    "Engine.ListingController",
    "Engine.OtherController",
    "Engine.PlayerController",
    "Engine.ToyMind",
    "Engine.BareController",
    "Engine.Pawn",
    "Engine.ThingPawn",
    "Engine.WorldPawn",
    "Engine.ToyAIPawn",
    "Engine.EchoPawn",
    "Engine.ToyPRI",
    "Engine.ToyNotPorted",
];

struct Env {
    vm: Vm,
    dir: PathBuf,
    /// What `EchoPawn`'s rule got back from its controller, each time it asked.
    echoed: Rc<RefCell<Vec<Option<ObjRef>>>>,
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

impl Env {
    fn new(mode: ParityMode) -> Env {
        Env::new_with(mode, false)
    }

    /// With `production`, the production classes the fixture's package lacks are defined by hand before
    /// [`register`] runs (`AIController`, `GearboxMind`, `WillowMind` with `MyWillowPawn`, `WillowPlayerController`),
    /// so every binding is present.
    fn new_with(mode: ParityMode, production: bool) -> Env {
        let dir = std::env::temp_dir().join(format!(
            "ow-combat-controller-interface-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let (engine, objects) = (engine(), objects());
        let defs = DefStore::from_synthetic(
            &dir,
            &[("Engine.upk", engine.as_slice()), ("ToyObjects.upk", objects.as_slice())],
            IdentityPolicy::FirstInScanOrder,
            mode,
        )
        .unwrap();
        let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), mode);
        vm.defs.code_mut().1.load(&PackageName::parse("Engine").unwrap()).unwrap();
        for path in RESOLVED.iter().chain(["Engine.ColdController", "Engine.Vehicle"].iter()) {
            vm.find_class(path).unwrap();
        }
        let object = vm.find_class("Engine.Object").unwrap();
        vm.space.classes.define("WillowGame.IPlayerMaster", Some(object), vec![]).unwrap();
        if production {
            let controller = vm.find_class("Engine.Controller").unwrap();
            let player = vm.find_class("Engine.PlayerController").unwrap();
            let c = &mut vm.space.classes;
            let ai = c.define("Engine.AIController", Some(controller), vec![]).unwrap();
            let gearbox = c.define("GearboxFramework.GearboxMind", Some(ai), vec![]).unwrap();
            let decl = PropertyDecl::new(vm.space.names.intern("MyWillowPawn"), PropKind::Object { class: object });
            let c = &mut vm.space.classes;
            c.define("WillowGame.WillowMind", Some(gearbox), vec![decl]).unwrap();
            c.define("WillowGame.WillowPlayerController", Some(player), vec![]).unwrap();
        }
        let echoed = Rc::new(RefCell::new(Vec::new()));
        if mode == ParityMode::Off {
            register(&mut vm).unwrap();
            let c = vm.find_class("Engine.ToyMind").unwrap();
            vm.register_read_only_interface_rule(c, controller_rule("MyWillowPawn")).unwrap();
            let c = vm.find_class("Engine.ToyAIPawn").unwrap();
            vm.register_interface_rule(c, Rc::new(willow_ai_pawn_answer)).unwrap();
            vm.register_read_only_form(c, Rc::new(willow_ai_pawn_read_only)).unwrap();
            let c = vm.find_class("Engine.BareController").unwrap();
            vm.register_read_only_interface_rule(c, controller_rule("Pawn")).unwrap();
            let c = vm.find_class("Engine.ToyNotPorted").unwrap();
            vm.register_interface_rule(c, Rc::new(pawn_answer)).unwrap();
            // `EchoPawn` asks its controller the interface in its `Mesh`-free form: `IOther` when asked `IToyThing`,
            // else the same interface, and notes what it got (the guard tests).
            let c = vm.find_class("Engine.EchoPawn").unwrap();
            let (thing, other) = (vm.find_class("Engine.IToyThing").unwrap(), vm.find_class("Engine.IOther").unwrap());
            let noted = echoed.clone();
            vm.register_read_only_interface_rule(
                c,
                Rc::new(move |vm: &Vm, asking: &mut InterfaceAsking, this: ObjRef, iface: ClassId| {
                    let back = if iface == thing { other } else { iface };
                    let controller = field(vm, this, "Controller", "the echo pawn")?.expect("the echo pawn's controller");
                    let got = vm.query_interface_read_only(asking, controller, back)?;
                    noted.borrow_mut().push(got);
                    Ok(None)
                }),
            )
            .unwrap();
        }
        for path in RESOLVED {
            let c = vm.find_class(path).unwrap();
            vm.resolve_interface_tables(c);
        }
        Env { vm, dir, echoed }
    }

    fn class(&mut self, path: &str) -> ClassId {
        self.vm.find_class(path).unwrap()
    }

    fn make(&mut self, path: &str) -> ObjRef {
        let class = self.class(path);
        let name = format!("{}_{}", path.rsplit('.').next().unwrap(), NEXT.fetch_add(1, Ordering::Relaxed));
        self.vm.instantiate(class, &name).unwrap()
    }

    fn link(&mut self, o: ObjRef, field: &str, to: ObjRef) {
        assert!(set(&mut self.vm, o, field, Value::Object(Some(to))).unwrap(), "{field} declared");
    }

    /// `o` asked `iface` through the read-only entry, with [`counters`] unchanged around it in every case, an answer
    /// or a refusal (review r1 of #602, item 3).
    fn ask(&mut self, o: ObjRef, iface: &str) -> Result<Option<ObjRef>, VmError> {
        let i = self.class(iface);
        let before = counters(&self.vm);
        let r = query_read_only(&self.vm, o, Some(i));
        assert_eq!(counters(&self.vm), before, "no write, draw, load or class: {r:?}");
        r
    }

    fn state(&self) -> Counts {
        counters(&self.vm)
    }
}

/// #518's counters (heap writes, draws, load transitions), the class table's length and the name table's length.
type Counts = (u64, u64, u64, usize, usize);

fn counters(vm: &Vm) -> Counts {
    let space = &vm.space;
    (space.heap.committed_writes(), vm.rng.draws(), vm.defs.load_transitions(), space.classes.len(), space.names.len())
}

fn not_yet(r: Result<Option<ObjRef>, VmError>, missing: Missing, words: &[&str]) {
    match r {
        Err(VmError::NotYet(n)) => {
            assert_eq!(n.missing, missing, "{n}");
            for w in words {
                assert!(n.what.contains(w), "{w:?} in {n}");
            }
        }
        other => panic!("expected NotYet {missing:?} naming {words:?}, got {other:?}"),
    }
}

fn script(r: Result<Option<ObjRef>, VmError>, words: &str) {
    match r {
        Err(VmError::Script { message, .. }) => assert!(message.contains(words), "{words:?} in {message}"),
        other => panic!("expected a script error naming {words:?}, got {other:?}"),
    }
}

/// Steps 3, 4 and 5, and a controller with no pawn: itself when its class lists `I`; else its pawn's answer; with no
/// pawn none (its `PlayerReplicationInfo` is not asked); with a declining pawn, its `PlayerReplicationInfo`'s.
#[test]
fn the_branches_itself_its_pawn_none_and_its_replication_info() {
    let mut e = Env::new(ParityMode::Off);
    let listing = e.make("Engine.ListingController");
    assert_eq!(e.ask(listing, "Engine.IToyThing").unwrap(), Some(listing), "step 3");
    let c = e.make("Engine.Controller");
    let thing_pawn = e.make("Engine.ThingPawn");
    e.link(c, "Pawn", thing_pawn);
    assert_eq!(e.ask(c, "Engine.IToyThing").unwrap(), Some(thing_pawn), "step 4");
    let lone = e.make("Engine.Controller");
    let pri = e.make("Engine.ToyPRI");
    e.link(lone, "PlayerReplicationInfo", pri);
    assert_eq!(e.ask(lone, "Engine.IToyThing").unwrap(), None, "no pawn: none, the PRI not asked");
    let c = e.make("Engine.Controller");
    let plain = e.make("Engine.Pawn");
    e.link(c, "Pawn", plain);
    e.link(c, "PlayerReplicationInfo", pri);
    assert_eq!(e.ask(c, "Engine.IToyThing").unwrap(), Some(pri), "step 5");
    assert_eq!(e.ask(c, "Engine.IOther").unwrap(), None, "nobody lists it");
    // The ordinary dispatch (the script native's) gives the same answers.
    let i = e.class("Engine.IToyThing");
    let before = e.state();
    assert_eq!(e.vm.query_interface(c, i).unwrap(), Some(pri));
    assert_eq!(e.vm.query_interface(listing, i).unwrap(), Some(listing));
    assert_eq!(e.state(), before, "the ordinary dispatch writes, draws and loads nothing either");
}

/// Step 2: `IWorldBody` goes to the pawn first, and does not fall through: a declining pawn gives none although the
/// controller lists it; a pawn that lists it answers; with no pawn, step 3 answers. `ISoundBehavior` likewise.
#[test]
fn the_two_named_interfaces_go_to_the_pawn_and_do_not_fall_through() {
    let mut e = Env::new(ParityMode::Off);
    let c = e.make("Engine.ListingController");
    assert_eq!(e.ask(c, "Engine.IWorldBody").unwrap(), Some(c), "no pawn: step 3");
    let plain = e.make("Engine.Pawn");
    e.link(c, "Pawn", plain);
    assert_eq!(e.ask(c, "Engine.IWorldBody").unwrap(), None, "the pawn declined: still none");
    assert_eq!(e.ask(c, "Engine.IToyThing").unwrap(), Some(c), "another interface: step 3 as before");
    let world = e.make("Engine.WorldPawn");
    e.link(c, "Pawn", world);
    assert_eq!(e.ask(c, "Engine.IWorldBody").unwrap(), Some(world));
    assert_eq!(e.ask(c, "Engine.ISoundBehavior").unwrap(), None, "the pawn declines ISoundBehavior");
}

/// Section 3: a mind registered with `MyWillowPawn` asks that pawn, not the one in `Pawn`.
#[test]
fn a_mind_reads_my_willow_pawn() {
    let mut e = Env::new(ParityMode::Off);
    let mind = e.make("Engine.ToyMind");
    let (mine, other) = (e.make("Engine.ThingPawn"), e.make("Engine.ThingPawn"));
    e.link(mind, "MyWillowPawn", mine);
    e.link(mind, "Pawn", other);
    assert_eq!(e.ask(mind, "Engine.IToyThing").unwrap(), Some(mine));
}

/// Fields are read only when a step reaches them: a controller that answers itself at step 3 has no `Pawn` or
/// `PlayerReplicationInfo` at all, or a malformed one; the same fields reached at steps 4 and 5 are script errors.
#[test]
fn fields_are_checked_only_when_reached() {
    let mut e = Env::new(ParityMode::Off);
    let bare = e.make("Engine.BareController");
    assert_eq!(e.ask(bare, "Engine.IToyThing").unwrap(), Some(bare), "no fields declared, never read");
    script(e.ask(bare, "Engine.IOther"), "does not declare Pawn");
    let c = e.make("Engine.ListingController");
    let pri = e.class("Engine.Controller");
    let p = e.vm.space.prop(pri, "PlayerReplicationInfo").unwrap();
    let sp = &mut e.vm.space;
    *sp.heap.slot_mut(&sp.classes, c, PropAddr::scalar(p)).unwrap() = Value::Int(3);
    assert_eq!(e.ask(c, "Engine.IToyThing").unwrap(), Some(c), "a malformed PRI is never read at step 3");
    let plain = e.make("Engine.Pawn");
    e.link(c, "Pawn", plain);
    script(e.ask(c, "Engine.IOther"), "PlayerReplicationInfo holds");
    let q = e.vm.space.prop(pri, "Pawn").unwrap();
    let sp = &mut e.vm.space;
    *sp.heap.slot_mut(&sp.classes, c, PropAddr::scalar(q)).unwrap() = Value::Int(4);
    script(e.ask(c, "Engine.IOther"), "Pawn holds");
}

/// A cold interface-table entry, while the controller's class and `I` are both loaded: `NotRead`, the entry still
/// unresolved and nothing cached.
#[test]
fn a_cold_entry_is_not_read_and_stays_cold() {
    let mut e = Env::new(ParityMode::Off);
    let cold = e.make("Engine.ColdController");
    let class = e.class("Engine.ColdController");
    let left = e.vm.unresolved_interface_entries(class);
    assert!(left > 0, "the fixture's entry is cold");
    let before = e.state();
    not_yet(e.ask(cold, "Engine.IToyThing"), Missing::NotRead, &["Engine.ColdController", "not resolved at set-up"]);
    assert_eq!(e.state(), before, "nothing written or resolved");
    assert_eq!(e.vm.unresolved_interface_entries(class), left);
}

/// Nested refusals, each typed and changing nothing: a loaded `WillowAIPawn` (the toy one) whose `DrivenVehicle` is
/// the store's unloaded handle, reached through step 4; the same pawn asked `IPlayerMaster`; and an unloaded handle
/// asked directly as the pawn.
#[test]
fn nested_refusals_keep_their_kind_and_change_nothing() {
    let mut e = Env::new(ParityMode::Off);
    let path = ow_core::ObjectPath::parse("ToyObjects.Holder").unwrap();
    let holder = e.vm.defs.find(&mut e.vm.space, &path).unwrap().unwrap();
    e.vm.defs.ensure_loaded(&mut e.vm.space, holder).unwrap();
    let class = e.vm.space.heap.get(holder).unwrap().class;
    let p = e.vm.space.prop(class, "DrivenVehicle").unwrap();
    let Value::Object(Some(vehicle)) = e.vm.space.read(holder, PropAddr::scalar(p)).unwrap().clone() else {
        panic!("the holder names its vehicle")
    };
    assert!(e.vm.defs.is_unloaded(&e.vm.space, vehicle), "a genuinely unloaded handle");
    let c = e.make("Engine.Controller");
    e.link(c, "Pawn", holder);
    let before = e.state();
    not_yet(
        e.ask(c, "Engine.IToyThing"),
        Missing::NotRead,
        &["goes to its pawn, a Engine.ToyAIPawn", "reads its driven vehicle's Controller", "unloaded Engine.Vehicle"],
    );
    assert_eq!(e.state(), before);
    assert!(e.vm.defs.is_unloaded(&e.vm.space, vehicle), "still unloaded");
    let runtime = e.make("Engine.ToyAIPawn");
    e.link(c, "Pawn", runtime);
    let before = e.state();
    not_yet(e.ask(c, "WillowGame.IPlayerMaster"), Missing::NotRead, &["goes to its pawn", "IPlayerMaster"]);
    assert_eq!(e.state(), before);
    e.link(c, "Pawn", vehicle);
    let before = e.state();
    not_yet(e.ask(c, "Engine.IToyThing"), Missing::NotRead, &["goes to its pawn", "unloaded Engine.Vehicle"]);
    assert_eq!(e.state(), before);
}

/// A delegate whose class answers by a rule with no read-only form: `NotPorted`, named with the step.
#[test]
fn a_delegate_with_no_read_only_form_is_not_ported() {
    let mut e = Env::new(ParityMode::Off);
    let c = e.make("Engine.Controller");
    let (plain, odd) = (e.make("Engine.Pawn"), e.make("Engine.ToyNotPorted"));
    e.link(c, "Pawn", plain);
    e.link(c, "PlayerReplicationInfo", odd);
    let before = e.state();
    not_yet(
        e.ask(c, "Engine.IToyThing"),
        Missing::NotPorted,
        &["goes to its PlayerReplicationInfo", "Engine.ToyNotPorted's QueryInterface rule is not ported read-only"],
    );
    assert_eq!(e.state(), before);
}

/// The guard, by the controller alone: a question that comes back to the controller on the same interface gets none
/// (the dispatch guard), and so does one on another interface (the rule's own guard), although the controller lists
/// that one and answers it when asked directly.
#[test]
fn the_guard_is_keyed_by_the_controller_alone() {
    let mut e = Env::new(ParityMode::Off);
    let c = e.make("Engine.OtherController");
    let echo = e.make("Engine.EchoPawn");
    e.link(c, "Pawn", echo);
    e.link(echo, "Controller", c);
    assert_eq!(e.ask(c, "Engine.IOther").unwrap(), Some(c), "asked directly: itself");
    e.echoed.borrow_mut().clear();
    assert_eq!(e.ask(c, "Engine.IToyThing").unwrap(), None);
    assert_eq!(*e.echoed.borrow(), [None], "the pawn asked the controller IOther and got none");
    e.echoed.borrow_mut().clear();
    assert_eq!(e.ask(c, "Engine.ISoundBehavior").unwrap(), None);
    assert_eq!(*e.echoed.borrow(), [None], "the same interface back: none");
}

/// `Engine.PlayerController` answers `NotRead`, its subclasses too; `None` passed directly is a caller error; a world
/// with no code layer is `LayerAbsent`; Reference registers none of it.
#[test]
fn player_controllers_none_no_code_layer_and_reference() {
    let mut e = Env::new(ParityMode::Off);
    let pc = e.make("Engine.PlayerController");
    not_yet(e.ask(pc, "Engine.IToyThing"), Missing::NotRead, &["PlayerController's own QueryInterface answer"]);
    let c = e.make("Engine.Controller");
    let before = e.state();
    script(query_read_only(&e.vm, c, None), "no interface class");
    assert_eq!(e.state(), before);

    let mut r = Env::new(ParityMode::Reference);
    let class = r.class("Engine.Controller");
    assert!(r.vm.interface_rule_for(class).is_none());
    assert!(!r.vm.has_read_only_interface_rule(class));

    // No code layer: a world whose code index holds no class, the controller defined by hand.
    let dir = std::env::temp_dir().join(format!("ow-combat-no-code-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let mut empty = ScriptToy::with_imports(&["Class"]);
    let bytes = empty.p.build();
    let defs = DefStore::from_synthetic(&dir, &[("Toy.upk", bytes.as_slice())], IdentityPolicy::FirstInScanOrder, ParityMode::Off)
        .unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
    let object = vm.space.classes.define("Toy.Object", None, vec![]).unwrap();
    let decl = |vm: &mut Vm, n: &str| PropertyDecl::new(vm.space.names.intern(n), PropKind::Object { class: object });
    let fields = vec![decl(&mut vm, "Pawn"), decl(&mut vm, "PlayerReplicationInfo")];
    let controller = vm.space.classes.define("Toy.Controller", Some(object), fields).unwrap();
    let iface = vm.space.classes.define("Toy.IThing", Some(object), vec![]).unwrap();
    vm.register_read_only_interface_rule(controller, controller_rule("Pawn")).unwrap();
    let c = vm.instantiate(controller, "C").unwrap();
    let before = counters(&vm);
    not_yet(query_read_only(&vm, c, Some(iface)), Missing::LayerAbsent("the code layer (package class exports)"), &["no code layer"]);
    assert_eq!(counters(&vm), before);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The harness set-up: resolves the cold entry of a controller it reaches, logs each class it resolved and each set-up
/// class, and leaves an unloaded object it meets unloaded; after it, the question that was `NotRead` answers.
#[test]
fn the_set_up_resolves_what_the_rule_reaches_and_loads_no_object() {
    let mut e = Env::new(ParityMode::Off);
    let cold = e.make("Engine.ColdController");
    not_yet(e.ask(cold, "Engine.IOther"), Missing::NotRead, &["not resolved at set-up"]);
    let path = ow_core::ObjectPath::parse("ToyObjects.Holder").unwrap();
    let holder = e.vm.defs.find(&mut e.vm.space, &path).unwrap().unwrap();
    e.vm.defs.ensure_loaded(&mut e.vm.space, holder).unwrap();
    e.link(cold, "Pawn", holder);
    e.link(holder, "Controller", cold);
    let class = e.vm.space.heap.get(holder).unwrap().class;
    let p = e.vm.space.prop(class, "DrivenVehicle").unwrap();
    let Value::Object(Some(vehicle)) = e.vm.space.read(holder, PropAddr::scalar(p)).unwrap().clone() else {
        panic!("the holder names its vehicle")
    };
    let lines = prepare(&mut e.vm, &[holder]).unwrap();
    assert!(lines.iter().all(|l| l.starts_with("harness input: ")), "{lines:?}");
    assert_eq!(lines.iter().filter(|l| l.contains("interface set-up class")).count(), SETUP_CLASSES.len(), "{lines:?}");
    assert!(lines.iter().any(|l| l.contains("interface tables of Engine.ColdController: 1 entries resolved")), "{lines:?}");
    assert!(lines.iter().any(|l| l.contains("an unloaded Engine.Vehicle is left unloaded")), "{lines:?}");
    assert!(e.vm.defs.is_unloaded(&e.vm.space, vehicle), "the set-up loads no object");
    assert_eq!(e.ask(cold, "Engine.IOther").unwrap(), Some(cold));
}

// ---- Lane FOLLOWUP-602-CLASS-POLICY --------------------------------------------------------------------------------

/// The ordinary dispatch's answer, with the counters unchanged around it.
fn ask_ordinary(e: &mut Env, o: ObjRef, iface: &str) -> Result<Option<ObjRef>, VmError> {
    let i = e.class(iface);
    let before = counters(&e.vm);
    let r = e.vm.query_interface(o, i);
    assert_eq!(counters(&e.vm), before, "no write, draw, load, class or name: {r:?}");
    r
}

/// The holder's `DrivenVehicle`: the store's unloaded `Vehicle_1` handle.
fn cold_vehicle(e: &mut Env) -> ObjRef {
    let path = ow_core::ObjectPath::parse("ToyObjects.Holder").unwrap();
    let holder = e.vm.defs.find(&mut e.vm.space, &path).unwrap().unwrap();
    e.vm.defs.ensure_loaded(&mut e.vm.space, holder).unwrap();
    let class = e.vm.space.heap.get(holder).unwrap().class;
    let p = e.vm.space.prop(class, "DrivenVehicle").unwrap();
    let Value::Object(Some(vehicle)) = e.vm.space.read(holder, PropAddr::scalar(p)).unwrap().clone() else {
        panic!("the holder names its vehicle")
    };
    assert!(e.vm.defs.is_unloaded(&e.vm.space, vehicle));
    vehicle
}

/// Review point 1: every production binding present. The exact `Controller` and `WillowMind` rules, the
/// `PlayerController` `NotYet` and `WillowPlayerController`'s open answer each sit on their own class; `AIController`
/// and `GearboxMind` have none and take `Controller`'s. On both dispatch paths, a `WillowMind` reads `MyWillowPawn`
/// (never `Pawn`, which names another pawn), a `GearboxMind` reads `Pawn`, and a `PlayerController` is refused.
#[test]
fn every_binding_present_resolves_to_the_nearest_rule_on_both_paths() {
    let mut e = Env::new_with(ParityMode::Off, true);
    for (path, own) in [
        ("Engine.Controller", true),
        ("WillowGame.WillowMind", true),
        ("Engine.PlayerController", true),
        ("Engine.AIController", false),
        ("GearboxFramework.GearboxMind", false),
    ] {
        let c = e.class(path);
        assert_eq!(e.vm.has_read_only_interface_rule(c), own, "{path}: read-only form");
        assert_eq!(e.vm.has_interface_rule(c), own, "{path}: ordinary rule");
    }
    let wpc = e.class("WillowGame.WillowPlayerController");
    assert!(e.vm.has_interface_rule(wpc) && e.vm.has_read_only_interface_rule(wpc), "the open answer, kept");
    let (a, b) = (e.make("Engine.WorldPawn"), e.make("Engine.WorldPawn"));
    let mind = e.make("WillowGame.WillowMind");
    e.link(mind, "Pawn", a);
    e.link(mind, "MyWillowPawn", b);
    assert_eq!(e.ask(mind, "Engine.IWorldBody").unwrap(), Some(b), "read-only: MyWillowPawn");
    assert_eq!(ask_ordinary(&mut e, mind, "Engine.IWorldBody").unwrap(), Some(b), "ordinary: MyWillowPawn");
    let gearbox = e.make("GearboxFramework.GearboxMind");
    e.link(gearbox, "Pawn", a);
    assert_eq!(e.ask(gearbox, "Engine.IWorldBody").unwrap(), Some(a), "read-only: Pawn");
    assert_eq!(ask_ordinary(&mut e, gearbox, "Engine.IWorldBody").unwrap(), Some(a), "ordinary: Pawn");
    let player = e.make("Engine.PlayerController");
    not_yet(e.ask(player, "Engine.IToyThing"), Missing::NotRead, &["PlayerController"]);
    not_yet(ask_ordinary(&mut e, player, "Engine.IToyThing"), Missing::NotRead, &["PlayerController"]);
    // Registering again keeps every existing binding.
    register(&mut e.vm).unwrap();
    assert_eq!(e.ask(mind, "Engine.IWorldBody").unwrap(), Some(b));
}

/// Review point 2: a native class under `Controller` whose own answer was not read, though its class lists the
/// interface, and its script child, are `NotRead` naming the native class, on both paths, before anything is read.
#[test]
fn an_unread_native_class_and_its_script_child_are_not_read() {
    let mut e = Env::new(ParityMode::Off);
    for path in ["Engine.NativeController", "Engine.ScriptUnderNative"] {
        let c = e.make(path);
        let words = ["the interface answer of Engine.NativeController", "own answer was not read", path];
        not_yet(e.ask(c, "Engine.IToyThing"), Missing::NotRead, &words);
        not_yet(ask_ordinary(&mut e, c, "Engine.IToyThing"), Missing::NotRead, &words);
    }
    // A script-only class under a read one inherits the rule: the toy `ListingController` answers itself.
    let listing = e.make("Engine.ListingController");
    assert_eq!(e.ask(listing, "Engine.IToyThing").unwrap(), Some(listing));
}

/// Review point 2: an unknown pawn accessor is kept apart from an unknown interface answer. Under a policy that reads
/// `AccessorController`'s answer but not its accessor, a plain hit answers (the pawn is never needed), and each step
/// that reads the pawn is `NotRead` naming the accessor.
#[test]
fn an_unread_pawn_accessor_is_named_only_when_a_step_reads_the_pawn() {
    let mut e = Env::new(ParityMode::Off);
    let class = e.class("Engine.AccessorController");
    let policy = ClassPolicy { interface_answer: &["Engine.AccessorController"], pawn_accessor: &["Engine.Controller"] };
    e.vm.register_read_only_interface_rule(class, controller_rule_with("Pawn", policy)).unwrap();
    let c = e.make("Engine.AccessorController");
    let pawn = e.make("Engine.ThingPawn");
    e.link(c, "Pawn", pawn);
    assert_eq!(e.ask(c, "Engine.IToyThing").unwrap(), Some(c), "step 3 never reads the pawn");
    for iface in ["Engine.IOther", "Engine.IWorldBody"] {
        let r = e.ask(c, iface);
        not_yet(r, Missing::NotRead, &["the pawn accessor of Engine.AccessorController", "own accessor was not read"]);
    }
}

/// Review point 3: field shape and residency only where the ordered rule reaches them. A plain hit answers over a
/// cold, or a malformed, pawn it never reads; the reached branch refuses the cold pawn; a reserved but undefined
/// interface is `NotRead` before any step.
#[test]
fn the_rule_reads_the_pawn_only_on_the_branch_that_reaches_it() {
    let mut e = Env::new(ParityMode::Off);
    let vehicle = cold_vehicle(&mut e);
    let c = e.make("Engine.ListingController");
    e.link(c, "Pawn", vehicle);
    assert_eq!(e.ask(c, "Engine.IToyThing").unwrap(), Some(c), "a plain hit, the cold pawn skipped");
    assert_eq!(ask_ordinary(&mut e, c, "Engine.IToyThing").unwrap(), Some(c));
    not_yet(e.ask(c, "Engine.IOther"), Missing::NotRead, &["goes to its pawn", "unloaded Engine.Vehicle"]);
    assert!(e.vm.defs.is_unloaded(&e.vm.space, vehicle), "still cold");
    let controller = e.class("Engine.Controller");
    let p = e.vm.space.prop(controller, "Pawn").unwrap();
    let sp = &mut e.vm.space;
    *sp.heap.slot_mut(&sp.classes, c, PropAddr::scalar(p)).unwrap() = Value::Int(7);
    assert_eq!(e.ask(c, "Engine.IToyThing").unwrap(), Some(c), "a malformed pawn never read");
    script(e.ask(c, "Engine.IOther"), "Pawn holds");
    let reserved = e.vm.space.classes.declare("Engine.IReserved").unwrap();
    let before = counters(&e.vm);
    let r = query_read_only(&e.vm, c, Some(reserved));
    not_yet(r, Missing::NotRead, &["Engine.IReserved", "reserved but not defined"]);
    assert_eq!(counters(&e.vm), before);
}

