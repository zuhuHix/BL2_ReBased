//! Tests of link 9's driver (decision 2026-11-19-70; review r2 of #454) on invented packages (AGENTS.md rule 9): the
//! world spawns the cast's three actors with their own components, and a spawn refusal stops the link before the cast.
//! AI-assisted (Claude Code).

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_core::{Missing, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_package::testkit::reflect::toy_struct;
use ow_package::testkit::script::ScriptToy;
use ow_vm::api::Vm;
use ow_vm::error::VmError;
use ow_world::world::World;

use super::*;

static NEXT: AtomicU32 = AtomicU32::new(0);

/// `Core.upk`: `Core.Object` and `Core.Component`.
fn core() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class"]);
    let object = t.class("Object", 0);
    t.class("Component", object);
    t.p.build()
}

/// `Engine.upk`: `Actor` with the fields the spawn reads, `WorldInfo`, a toy component class, `ToyPawn`,
/// `ToyController` and `ToyTarget` (each with a component field), `ToyBlocked` (its defaults collide with the world, so
/// its spawn needs the placement search, a `NotYet`) and `Thing` (not an actor: the world refuses to spawn it).
fn engine() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&[
        "Class",
        "ScriptStruct",
        "IntProperty",
        "FloatProperty",
        "BoolProperty",
        "ByteProperty",
        "ObjectProperty",
        "StructProperty",
        "ClassProperty",
        "ArrayProperty",
    ]);
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
    for b in [
        "bStatic",
        "bNoDelete",
        "bDeleteMe",
        "bPendingDelete",
        "bCollideWorld",
        "bCollideWhenPlacing",
        "bCollideActors",
        "bShouldBaseAtStartup",
    ] {
        t.prop("Bool", actor, b, 0, 0);
    }
    t.prop("Byte", actor, "Role", 0, 0);
    t.prop("Float", actor, "CreationTime", 0, 0);
    for o in ["Owner", "Base", "Instigator", "WorldInfo"] {
        t.prop("Object", actor, o, 0, actor);
    }
    t.prop("Struct", actor, "Location", 0, vector);
    // The spawn's component update (lane IMPL-SPAWN-COMPONENT-ATTACH) reads both arrays.
    for a in ["Components", "AllComponents"] {
        ow_package::testkit::reflect::toy_array_of(&mut t, actor, a, "Object", object);
    }
    t.prop("Struct", actor, "Rotation", 0, rotator);
    let world = t.class("WorldInfo", actor);
    t.prop("Byte", world, "NetMode", 0, 0);
    t.prop("Float", world, "TimeSeconds", 0, 0);
    let core_pkg = t.p.add_import("Core", "Package", 0, "Core");
    let component = t.p.add_import("Core", "Class", core_pkg, "Component");
    let toy_component = t.class("ToyComponent", component);
    t.prop("Int", toy_component, "Payload", 0, 0);
    for name in ["ToyPawn", "ToyController", "ToyTarget"] {
        let c = t.class(name, actor);
        t.prop("Object", c, "Comp", 0, toy_component);
    }
    let blocked = t.class("ToyBlocked", actor);
    let mut defaults = ow_package::testkit::ints(&[0]);
    defaults.extend(t.p.bool_tag("bCollideWorld", true));
    defaults.extend_from_slice(&t.none);
    t.p.add_export(blocked, "Default__ToyBlocked", defaults);
    t.class("Thing", object);
    // Lane IMPL-LINK9-TARGET-MIND: a controller class with the mind's link field, a toy mind, a target that can hold a
    // controller (no ControllerClass: the fallback WillowGame.WillowMind) and one whose default names ToyMind.
    let controller = t.class("Controller", actor);
    t.prop("Object", controller, "Pawn", 0, actor);
    let toy_mind = t.class("ToyMind", controller);
    t.prop("Object", toy_mind, "MyWillowPawn", 0, actor);
    let minded = t.class("ToyMindedTarget", actor);
    t.prop("Object", minded, "Controller", 0, controller);
    let named = t.class("ToyNamedTarget", actor);
    t.prop("Object", named, "Controller", 0, controller);
    t.prop("Object", named, "MyWillowMind", 0, controller);
    let class_class = t.import("Class");
    t.prop_tail("Class", named, "ControllerClass", 0, &ow_package::testkit::ints(&[class_class, controller]));
    let mut defaults = ow_package::testkit::ints(&[0]);
    defaults.extend(t.p.tag("ControllerClass", "ClassProperty", &ow_package::testkit::ints(&[toy_mind])));
    defaults.extend_from_slice(&t.none);
    t.p.add_export(named, "Default__ToyNamedTarget", defaults);
    // Review r1 of #579: a target whose MyWillowMind holds only a ToyMind, narrower than the fallback WillowMind.
    let narrow = t.class("ToyNarrowTarget", actor);
    t.prop("Object", narrow, "Controller", 0, controller);
    t.prop("Object", narrow, "MyWillowMind", 0, toy_mind);
    t.p.build()
}

/// `WillowGame.upk`: `WillowMind`, a `Controller` (the target mind's fallback class).
fn willow() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class"]);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let controller = t.p.add_import("Core", "Class", engine, "Controller");
    t.class("WillowMind", controller);
    t.p.build()
}

/// `GearboxFramework.upk`: `IFlagProvider`, the interface the harness preloads with the mind.
fn gearbox() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class"]);
    let core = t.p.add_import("Core", "Package", 0, "Core");
    let object = t.p.add_import("Core", "Class", core, "Object");
    t.class("IFlagProvider", object);
    t.p.build()
}

struct Toy {
    world: World,
    dir: PathBuf,
    /// The component each spawnable class's defaults hold, outered to that class default: pawn, controller, target.
    templates: Vec<(ObjRef, ObjRef)>,
}

impl Drop for Toy {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn toy() -> Toy {
    use ow_balance::types::{
        AwesomeLevel, BalanceSnapshot, ExpLevel, GameStage, LevelCapUpgrades, OpLevel, PlayerCount, Playthrough,
    };
    let dir = std::env::temp_dir().join(format!("ow-cli-l9-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let (c, e, w, g) = (core(), engine(), willow(), gearbox());
    let files: [(&str, &[u8]); 4] =
        [("Core.upk", &c), ("Engine.upk", &e), ("WillowGame.upk", &w), ("GearboxFramework.upk", &g)];
    let defs = DefStore::from_synthetic(&dir, &files, IdentityPolicy::FirstInScanOrder, ParityMode::Off).unwrap();
    let vm = Vm::new(ObjectSpace::new(), defs, Rc::new(ow_attr::AttributeSystem::new_empty()), ParityMode::Off);
    let snapshot = BalanceSnapshot::new(
        Playthrough::NORMAL,
        OpLevel::NONE,
        PlayerCount::SINGLE,
        ExpLevel::new(8),
        GameStage(8),
        AwesomeLevel::new(0),
        LevelCapUpgrades::NONE,
    )
    .unwrap();
    let mut world = World::new(vm, ow_world::world::BalanceState::new(snapshot), &[]).unwrap();
    let class = world.vm.find_class("Engine.WorldInfo").unwrap();
    let info = ow_vm::interp::instantiate_from_defaults(&mut world.vm, class).unwrap();
    world.actors().try_borrow_mut().unwrap().place_world_info(info).unwrap();
    let mut templates = Vec::new();
    for name in ["ToyPawn", "ToyController", "ToyTarget"] {
        let vm = &mut world.vm;
        let cdo = vm.defs.class_default(&mut vm.space, &format!("Engine.{name}")).unwrap();
        let component_class = vm.find_class("Engine.ToyComponent").unwrap();
        let component = vm.space.alloc(component_class, "ToyComponent0", Some(cdo), Origin::Runtime).unwrap();
        let p = vm.space.prop(vm.space.heap.get(cdo).unwrap().class, "Comp").unwrap();
        vm.space.write(cdo, PropAddr::scalar(p), Value::Object(Some(component))).unwrap();
        templates.push((cdo, component));
    }
    // The blocked class's default is built here too, as the others are: a spawn that builds a cold class default
    // marks its later refusals (review r1 of #514, item 3), and these tests are about the driver's own marking.
    let vm = &mut world.vm;
    vm.defs.class_default(&mut vm.space, "Engine.ToyBlocked").unwrap();
    Toy { world, dir, templates }
}

impl Toy {
    fn listed(&self) -> Vec<ObjRef> {
        self.world.actors().borrow().list().to_vec()
    }

    fn comp(&self, o: ObjRef) -> ObjRef {
        let space = &self.world.vm.space;
        let p = space.prop(space.heap.get(o).unwrap().class, "Comp").unwrap();
        match space.read(o, PropAddr::scalar(p)).unwrap() {
            Value::Object(Some(c)) => *c,
            other => panic!("{other:?}"),
        }
    }

    /// Runs link 9's driver on `classes` with a cast that only records the actors it was given.
    fn drive(&mut self, classes: &[&str], grading: Grading) -> (Ran, Option<CastActors>) {
        let mut r = Ran::default();
        let seen: Rc<Cell<Option<CastActors>>> = Rc::new(Cell::new(None));
        let got = seen.clone();
        drive_phaselock(
            &mut r,
            &mut self.world.vm,
            grading,
            classes,
            move |_vm, actors| {
                got.set(Some(actors));
                actors.0.ok_or_else(|| VmError::Script { function: "toy".into(), message: "no pawn".into() })
            },
            |_, _, _| false,
        );
        (r, seen.get())
    }
}

const ACTORS: [&str; 3] = ["Engine.ToyPawn", "Engine.ToyController", "Engine.ToyTarget"];

/// All three actors are spawned and registered, and each cast slot holds the expected class's actor, a distinct one,
/// with its own component (not the class default's, outered to that actor); the cast runs on them (review r3 of #454:
/// a swap of any two slots fails).
#[test]
fn the_world_spawns_the_cast_s_actors_with_their_own_components() {
    let mut t = toy();
    let before = t.listed().len();
    let (r, actors) = t.drive(&ACTORS, Grading::Strict);
    assert!(r.stop.is_none(), "{:?}", r.stop);
    assert_eq!(r.steps, ["spawn", "cast"]);
    let (Some(pawn), Some(controller), Some(target)) = actors.expect("the cast ran") else {
        panic!("three actors");
    };
    let listed = t.listed();
    assert_eq!(listed.len(), before + 3);
    let slots = [pawn, controller, target];
    let mut components = Vec::new();
    for (i, ((actor, class), (_, template))) in slots.into_iter().zip(ACTORS).zip(t.templates.clone()).enumerate() {
        let want = t.world.vm.find_class(class).unwrap();
        assert_eq!(t.world.vm.space.heap.get(actor).unwrap().class, want, "slot {i} holds a {class}");
        assert!(listed.contains(&actor), "slot {i}: registered in the world");
        assert!(!slots[..i].contains(&actor), "slot {i}: a distinct actor");
        let own = t.comp(actor);
        assert_ne!(own, template, "slot {i}: not the class default's component");
        assert_eq!(t.world.vm.space.heap.get(own).unwrap().outer, Some(actor), "slot {i}: its own component");
        assert!(!components.contains(&own), "slot {i}: a component no other slot shares");
        components.push(own);
    }
}

/// Builds `Engine.ToyBlocked`'s class default object, so its spawn's refusal writes nothing first (review r1 of #528,
/// item 4: building a cold class default is a write, and a refusal after it is marked).
fn warm_blocked(vm: &mut ow_vm::api::Vm) {
    vm.defs.class_default(&mut vm.space, "Engine.ToyBlocked").unwrap();
}

/// A refusal before the first spawn, with nothing written (the class default already built): the spawn's `NotYet`
/// (graded from its variant) is the link's stop, without the partial-writes mark, and the cast does not run.
#[test]
fn a_refusal_before_the_first_spawn_stops_the_link_unmarked() {
    let mut t = toy();
    warm_blocked(&mut t.world.vm);
    let before = t.listed().len();
    let (r, actors) = t.drive(&["Engine.ToyBlocked", "Engine.ToyController", "Engine.ToyTarget"], Grading::Strict);
    let (outcome, stop) = r.stop.expect("a stop");
    assert_eq!(outcome, Outcome::NotYet);
    assert_eq!(stop.step, "spawn");
    assert!(!stop.text.contains(PARTIAL_WRITES), "{}", stop.text);
    assert!(actors.is_none(), "the cast did not run");
    assert_eq!(t.listed().len(), before, "nothing registered");
}

/// A `NotYet` after one spawn succeeded is marked as after partial writes, once, its kind and record kept; the earlier
/// actor stays registered, and the cast does not run.
#[test]
fn a_not_yet_after_a_committed_spawn_is_marked() {
    // The unmarked NotYet of the blocked spawn, from a fresh world: its record is the one the marked stop must keep.
    let mut fresh = toy();
    warm_blocked(&mut fresh.world.vm);
    let original = match phaselock_actors(&mut fresh.world.vm, &["Engine.ToyBlocked", "Engine.ToyController", "Engine.ToyTarget"]) {
        Err(VmError::NotYet(n)) => n,
        other => panic!("{other:?}"),
    };
    assert!(!original.record.is_empty());
    let mut t = toy();
    let before = t.listed().len();
    let mut r = Ran::default();
    drive_phaselock(
        &mut r,
        &mut t.world.vm,
        Grading::Strict,
        &["Engine.ToyPawn", "Engine.ToyBlocked", "Engine.ToyTarget"],
        |_, _| panic!("the cast must not run"),
        |_, _, _| panic!("no evidence without a cast"),
    );
    let (outcome, stop) = r.stop.expect("a stop");
    assert_eq!(outcome, Outcome::NotYet);
    assert!(stop.text.starts_with(PARTIAL_WRITES), "{}", stop.text);
    assert_eq!(stop.text.matches(PARTIAL_WRITES).count(), 1, "exactly one prefix: {}", stop.text);
    assert_eq!(stop.native, format!("{PARTIAL_WRITES}: {}", original.what), "the subject, prefixed once");
    assert_eq!(stop.record, original.record, "the original record");
    assert_eq!(t.listed().len(), before + 1, "the pawn stays registered");
    // The helper alone keeps the NotYet's kind.
    match phaselock_actors(&mut t.world.vm, &["Engine.ToyPawn", "Engine.ToyBlocked", "Engine.ToyTarget"]) {
        Err(VmError::NotYet(n)) => {
            assert!(matches!(n.missing, Missing::LayerAbsent(_)), "{n}");
            assert_eq!(n.what, format!("{PARTIAL_WRITES}: {}", original.what), "{n}");
            assert_eq!(n.record, original.record);
        }
        other => panic!("{other:?}"),
    }
}

/// An ordinary spawn error (the world refuses a class that is not an actor) is a hard stop under both gradings, so the
/// row never looks as if the cast had run.
#[test]
fn an_ordinary_spawn_error_is_a_hard_stop_in_both_gradings() {
    for grading in [Grading::Strict, Grading::PastErrors] {
        let mut t = toy();
        let (r, actors) = t.drive(&["Engine.Thing", "Engine.ToyController", "Engine.ToyTarget"], grading);
        let (outcome, stop) = r.stop.expect("a hard stop");
        assert_eq!(outcome, Outcome::Error, "{grading:?}");
        assert_eq!(stop.step, "spawn");
        assert!(stop.text.contains(REFUSED_THING), "{grading:?}: the world's refusal, not another error: {}", stop.text);
        assert!(r.error_lines.is_empty(), "{grading:?}: not an error line gone past");
        assert!(actors.is_none(), "{grading:?}: the cast did not run");
    }
}

/// The refusal `phaselock_actors` reports when the world's spawn answers no actor for `Engine.Thing`.
const REFUSED_THING: &str = "the world's spawn refused a Engine.Thing";

/// The grading is typed (review r3 of #454): an error that is not a `VmError::NotYet` is an error even when its text
/// reads like a `NotYet`'s display, from the cast under strict grading and from `typed_stop` itself; a real `NotYet` is
/// not yet, its subject and record read from the value even when the record's text holds another phrase.
#[test]
fn the_grading_follows_the_error_s_variant_not_its_text() {
    let looks = ow_core::NotYet::not_ported("Toy.Native", "TOY_RECORD.md").to_string();
    let fake = VmError::Script { function: "toy".into(), message: looks.clone() };
    let (outcome, stop) = typed_stop("cast", &fake);
    assert_eq!(outcome, Outcome::Error, "{}", stop.text);
    assert!(stop.text.contains(&looks));
    let mut t = toy();
    let mut r = Ran::default();
    let message = looks.clone();
    drive_phaselock(
        &mut r,
        &mut t.world.vm,
        Grading::Strict,
        &ACTORS,
        move |_, _| Err(VmError::Script { function: "toy".into(), message }),
        |_, _, _| panic!("no evidence after a cast error"),
    );
    let (outcome, stop) = r.stop.expect("a stop");
    assert_eq!((outcome, stop.step.as_str()), (Outcome::Error, "cast"), "{}", stop.text);
    // A real NotYet whose record names a higher-priority phrase keeps its own subject and record.
    const TRICKY: &str = "TOY_RECORD.md, not read from native code or script yet";
    assert!(TRICKY.contains(&Missing::NotRead.to_string()), "the record holds the higher-priority phrase");
    let real = VmError::NotYet(ow_core::NotYet::not_ported("Toy.Native", TRICKY));
    let (outcome, stop) = typed_stop("cast", &real);
    assert_eq!(outcome, Outcome::NotYet);
    assert_eq!(stop.native, "Toy.Native");
    assert_eq!(stop.record, TRICKY);
}

// ---- Link 8's make (review r2 of #506) ---------------------------------------------------------------------------

fn toy_vm() -> Vm {
    Vm::new(ObjectSpace::new(), DefStore::new_empty(), Rc::new(ow_attr::AttributeSystem::new_empty()), ParityMode::Off)
}

fn toy_not_yet() -> VmError {
    VmError::NotYet(ow_core::NotYet::not_read("Toy.Lookup", "TOY_RECORD.md"))
}

fn what(e: &VmError) -> String {
    match e {
        VmError::NotYet(n) => n.what.to_string(),
        other => panic!("expected NotYet, got {other:?}"),
    }
}

#[test]
fn a_make_failure_after_the_weapon_is_allocated_is_marked_once() {
    let mut vm = toy_vm();
    let e = make_marked(&mut vm, |_, wrote| -> Result<(), VmError> {
        *wrote = true;
        Err(ow_world::pools::after_partial_writes(toy_not_yet()))
    })
    .unwrap_err();
    let text = what(&e);
    assert!(text.starts_with("stopped after partial writes: "), "{text}");
    assert_eq!(text.matches("partial writes").count(), 1, "marked once: {text}");
    assert_eq!(classify_error("make", &e).0, Outcome::NotYet, "graded by its variant");
    let e = make_marked(&mut vm, |_, wrote| -> Result<(), VmError> {
        *wrote = true;
        Err(toy_not_yet())
    })
    .unwrap_err();
    assert_eq!(what(&e).matches("partial writes").count(), 1);
}

#[test]
fn a_make_failure_before_any_write_is_not_marked() {
    let mut vm = toy_vm();
    let e = make_marked(&mut vm, |_, _| -> Result<(), VmError> { Err(toy_not_yet()) }).unwrap_err();
    assert_eq!(what(&e), "Toy.Lookup");
    assert_eq!(classify_error("make", &e).0, Outcome::NotYet);
}

/// Link 9's evidence (lane LINK9-EVIDENCE-CHECK) on invented classes: the skill's `LiftedPawn` must be the cast's
/// target and its `CurrentState` the value named `EPLS_Intro`, read by name (the toy enum puts it at 2, not the
/// installed 1). Unset fields, another target, another state, no target and an enum-less `CurrentState` are no
/// evidence. Reads only.
#[test]
fn link_9_s_evidence_is_the_skill_s_lift_writes() {
    use ow_object::layout::{PropKind, PropertyDecl};
    let mut vm = Vm::new(
        ObjectSpace::new(),
        DefStore::new_empty(),
        Rc::new(ow_attr::AttributeSystem::new_empty()),
        ParityMode::Off,
    );
    let object = vm.space.classes.define("Core.Object", None, vec![]).unwrap();
    let names = ["EPLS_None", "EPLS_Locked", "EPLS_Intro"].iter().map(|n| vm.space.names.intern(n)).collect();
    let states = vm.space.classes.define_enum("WillowGame.LiftActionSkill.EPhaselockState", names).unwrap();
    let decl = |vm: &mut Vm, n: &str, k: PropKind| PropertyDecl::new(vm.space.names.intern(n), k);
    let props = vec![
        decl(&mut vm, "CurrentState", PropKind::Byte { enum_: Some(states) }),
        decl(&mut vm, "LiftedPawn", PropKind::Object { class: object }),
    ];
    let skill_class = vm.space.classes.define("WillowGame.LiftActionSkill", Some(object), props).unwrap();
    let props = vec![
        decl(&mut vm, "CurrentState", PropKind::Byte { enum_: None }),
        decl(&mut vm, "LiftedPawn", PropKind::Object { class: object }),
    ];
    let plain_class = vm.space.classes.define("Test.EnumlessSkill", Some(object), props).unwrap();
    let skill = vm.space.alloc(skill_class, "Skill", None, Origin::Runtime).unwrap();
    let target = vm.space.alloc(object, "Target", None, Origin::Runtime).unwrap();
    let other = vm.space.alloc(object, "Other", None, Origin::Runtime).unwrap();
    let put = |vm: &mut Vm, o: ObjRef, n: &str, v: Value| {
        let class = vm.space.heap.get(o).unwrap().class;
        let p = vm.space.prop(class, n).unwrap();
        vm.space.write(o, PropAddr::scalar(p), v).unwrap();
    };
    assert!(!phaselock_evident(&vm, skill, Some(target)), "unset");
    put(&mut vm, skill, "LiftedPawn", Value::Object(Some(target)));
    assert!(!phaselock_evident(&vm, skill, Some(target)), "the state still EPLS_None");
    put(&mut vm, skill, "CurrentState", Value::Byte(1));
    assert!(!phaselock_evident(&vm, skill, Some(target)), "value 1 is EPLS_Locked here: read by name");
    put(&mut vm, skill, "CurrentState", Value::Byte(2));
    assert!(phaselock_evident(&vm, skill, Some(target)), "both writes");
    assert!(!phaselock_evident(&vm, skill, Some(other)), "another target");
    assert!(!phaselock_evident(&vm, skill, None), "no target");
    let heap = format!("{:?}", vm.space.heap);
    let _ = phaselock_evident(&vm, skill, Some(target));
    assert_eq!(format!("{:?}", vm.space.heap), heap, "reads only");
    let plain = vm.space.alloc(plain_class, "Plain", None, Origin::Runtime).unwrap();
    put(&mut vm, plain, "LiftedPawn", Value::Object(Some(target)));
    put(&mut vm, plain, "CurrentState", Value::Byte(2));
    assert!(!phaselock_evident(&vm, plain, Some(target)), "a CurrentState with no enum");
}

/// An invented skill class with the two fields link 9's evidence reads, on `vm`; `EPLS_Intro` at position 2. With
/// `fold`, the name table first holds the spelling `epls_intro` (review r1 of #568: the evidence finds the value by
/// name, not by the stored spelling).
fn toy_skill_class(vm: &mut Vm, fold: bool) -> (ow_object::ids::ClassId, u8) {
    use ow_object::layout::{PropKind, PropertyDecl};
    if fold {
        vm.space.names.intern("epls_intro");
    }
    let names = ["EPLS_None", "EPLS_Locked", "EPLS_Intro"].iter().map(|n| vm.space.names.intern(n)).collect();
    let states = vm.space.classes.define_enum("WillowGame.ToyLiftActionSkill.EPhaselockState", names).unwrap();
    let props = vec![
        PropertyDecl::new(vm.space.names.intern("CurrentState"), PropKind::Byte { enum_: Some(states) }),
        PropertyDecl::new(vm.space.names.intern("LiftedPawn"), PropKind::Object { class: ow_object::ids::ClassId::new(0) }),
    ];
    (vm.space.classes.define("WillowGame.ToyLiftActionSkill", None, props).unwrap(), 2)
}

fn lift(vm: &mut Vm, skill: ObjRef, target: Option<ObjRef>, state: u8) {
    let class = vm.space.heap.get(skill).unwrap().class;
    let p = vm.space.prop(class, "LiftedPawn").unwrap();
    vm.space.write(skill, PropAddr::scalar(p), Value::Object(target)).unwrap();
    let p = vm.space.prop(class, "CurrentState").unwrap();
    vm.space.write(skill, PropAddr::scalar(p), Value::Byte(state)).unwrap();
}

/// Review r1 of #568, at the driver: the evidence is asked about the exact receiver the cast returns, and only after a
/// cast that returned `Ok`. A cast that lifts the target on the skill it returns is evident; one that returns another
/// object is not; a cast that errors is never asked (also under past-errors grading, where no stop is recorded).
#[test]
fn the_driver_asks_the_evidence_about_the_cast_s_own_receiver_only_after_success() {
    for (case, want) in [("right receiver", true), ("wrong receiver", false)] {
        let mut t = toy();
        let (class, intro) = toy_skill_class(&mut t.world.vm, false);
        let mut r = Ran::default();
        drive_phaselock(
            &mut r,
            &mut t.world.vm,
            Grading::Strict,
            &ACTORS,
            move |vm, actors| {
                let skill = vm.space.alloc(class, "Skill", None, Origin::Runtime)?;
                let other = vm.space.alloc(class, "Other", None, Origin::Runtime)?;
                lift(vm, skill, actors.2, intro);
                Ok(if case == "right receiver" { skill } else { other })
            },
            |vm, skill, (_, _, target)| phaselock_evident(vm, skill, target),
        );
        assert!(r.stop.is_none(), "{case}: {:?}", r.stop);
        assert_eq!(r.evident, want, "{case}");
    }
    let mut t = toy();
    let (class, intro) = toy_skill_class(&mut t.world.vm, false);
    let asked = Rc::new(Cell::new(false));
    let flag = asked.clone();
    let mut r = Ran::default();
    drive_phaselock(
        &mut r,
        &mut t.world.vm,
        Grading::PastErrors,
        &ACTORS,
        move |vm, actors| {
            let skill = vm.space.alloc(class, "Skill", None, Origin::Runtime)?;
            lift(vm, skill, actors.2, intro);
            Err(VmError::Script { function: "toy".into(), message: "an ordinary cast error".into() })
        },
        move |_, _, _| {
            flag.set(true);
            true
        },
    );
    assert!(r.stop.is_none(), "past-errors grading goes on past the error");
    assert_eq!(r.error_lines.len(), 1);
    assert!(!asked.get() && !r.evident, "a cast that errored is never evidence");
}

/// Review r1 of #568: `EPLS_Intro` found by name when the name table's first spelling is `epls_intro`.
#[test]
fn link_9_s_evidence_finds_the_state_by_name_whatever_its_spelling() {
    let mut vm = Vm::new(ObjectSpace::new(), DefStore::new_empty(), Rc::new(ow_attr::AttributeSystem::new_empty()), ParityMode::Off);
    let (class, intro) = toy_skill_class(&mut vm, true);
    assert_eq!(vm.space.names.text(vm.space.names.find("EPLS_Intro").unwrap()), "epls_intro", "the first spelling kept");
    let skill = vm.space.alloc(class, "Skill", None, Origin::Runtime).unwrap();
    let target = vm.space.alloc(class, "Target", None, Origin::Runtime).unwrap();
    lift(&mut vm, skill, Some(target), intro);
    assert!(phaselock_evident(&vm, skill, Some(target)));
}

/// Lane IMPL-LINK9-TARGET-MIND: the spawned target gets a mind that possesses it, through the world's own spawn. With
/// no ControllerClass the mind is the fallback `WillowGame.WillowMind`; a target whose default names `ToyMind` gets a
/// `ToyMind`, linked both ways (`Controller`, `MyWillowMind`; `Pawn`, `MyWillowPawn`). The mind is registered in the
/// world and the harness line is logged. Link 9's set-up runs first (it resolves the fallback, review r2 of #579).
/// A target that cannot hold a controller gets none.
#[test]
fn the_cast_s_target_gets_a_mind_that_possesses_it() {
    for (target, mind_class) in [("Engine.ToyMindedTarget", "WillowGame.WillowMind"), ("Engine.ToyNamedTarget", "Engine.ToyMind")] {
        let mut t = toy();
        phaselock_setup(&mut t.world.vm, &mut Vec::new()).unwrap();
        let before = t.listed().len();
        let (r, actors) = t.drive(&["Engine.ToyPawn", "Engine.ToyController", target], Grading::Strict);
        assert!(r.stop.is_none(), "{target}: {:?}", r.stop);
        let (_, _, Some(pawn)) = actors.expect("the cast ran") else { panic!() };
        let Value::Object(Some(mind)) = read(&t, pawn, "Controller") else { panic!("{target}: no controller") };
        let want = t.world.vm.find_class(mind_class).unwrap();
        assert_eq!(t.world.vm.space.heap.get(mind).unwrap().class, want, "{target}");
        assert_eq!(read(&t, mind, "Pawn"), Value::Object(Some(pawn)), "{target}: the mind possesses the pawn");
        assert!(t.listed().contains(&mind), "{target}: spawned by the world");
        assert_eq!(t.listed().len(), before + 4, "{target}: three actors and the mind");
        assert_eq!(r.harness.len(), 1, "{target}: {:?}", r.harness);
        assert!(r.harness[0].starts_with(TARGET_MIND_INPUT) && r.harness[0].contains(mind_class) && r.harness[0].contains("UNVERIFIED"));
        if target == "Engine.ToyNamedTarget" {
            assert_eq!(read(&t, pawn, "MyWillowMind"), Value::Object(Some(mind)));
            assert_eq!(read(&t, mind, "MyWillowPawn"), Value::Object(Some(pawn)));
        }
    }
    // A target class with no Controller field: no mind, no harness line.
    let mut t = toy();
    let before = t.listed().len();
    let (r, _) = t.drive(&ACTORS, Grading::Strict);
    assert!(r.stop.is_none());
    assert_eq!(t.listed().len(), before + 3);
    assert!(r.harness.is_empty(), "{:?}", r.harness);
}

fn read(t: &Toy, o: ObjRef, n: &str) -> Value {
    let space = &t.world.vm.space;
    let p = space.prop(space.heap.get(o).unwrap().class, n).unwrap();
    space.read(o, PropAddr::scalar(p)).unwrap().clone()
}

fn write(t: &mut Toy, o: ObjRef, n: &str, v: Value) {
    let space = &mut t.world.vm.space;
    let p = space.prop(space.heap.get(o).unwrap().class, n).unwrap();
    space.write(o, PropAddr::scalar(p), v).unwrap();
}

fn spawner(t: &Toy) -> ObjRef {
    t.world.actors().borrow().world_info().unwrap()
}

/// Review r1 of #579, item 1: a target whose `MyWillowMind` cannot hold the mind's class is refused before anything
/// is written. No mind is spawned, `Controller` stays None, and the stop is a typed error at the spawn that names the
/// slot.
#[test]
fn a_target_slot_that_cannot_hold_the_mind_leaves_nothing_half_linked() {
    let mut t = toy();
    phaselock_setup(&mut t.world.vm, &mut Vec::new()).unwrap();
    let before = t.listed().len();
    let (r, _) = t.drive(&["Engine.ToyPawn", "Engine.ToyController", "Engine.ToyNarrowTarget"], Grading::Strict);
    let Some((outcome, stop)) = &r.stop else { panic!("no stop") };
    assert_eq!(*outcome, Outcome::Error);
    assert_eq!(stop.step, "spawn");
    assert!(stop.text.contains("MyWillowMind cannot hold a WillowGame.WillowMind"), "{}", stop.text);
    assert_eq!(t.listed().len(), before + 3, "the mind was never spawned");
    let target = *t.listed().last().unwrap();
    let narrow = t.world.vm.find_class("Engine.ToyNarrowTarget").unwrap();
    assert_eq!(t.world.vm.space.heap.get(target).unwrap().class, narrow);
    assert_eq!(read(&t, target, "Controller"), Value::Object(None), "no half link");
    assert_eq!(read(&t, target, "MyWillowMind"), Value::Object(None));
}

/// Review r1 of #579, item 1: an already-possessed target keeps its mind, read as `adopt` reads it: a set
/// `Controller`, else a set `MyWillowMind`. Nothing is spawned or written, and no harness line is logged.
#[test]
fn an_already_possessed_target_keeps_its_mind() {
    let mut t = toy();
    let (_, actors) = t.drive(&["Engine.ToyPawn", "Engine.ToyController", "Engine.ToyNamedTarget"], Grading::Strict);
    let (_, _, Some(target)) = actors.unwrap() else { panic!() };
    let Value::Object(Some(mind)) = read(&t, target, "Controller") else { panic!() };
    let (listed, logged, at) = (t.listed().len(), t.world.vm.log.len(), spawner(&t));
    assert_eq!(target_mind(&mut t.world.vm, at, target).unwrap(), Some(mind), "a set Controller");
    // Controller cleared, MyWillowMind still set: possessed, as adopt treats it.
    write(&mut t, target, "Controller", Value::Object(None));
    assert_eq!(target_mind(&mut t.world.vm, at, target).unwrap(), Some(mind), "a set MyWillowMind");
    assert_eq!(read(&t, target, "Controller"), Value::Object(None), "nothing written");
    assert_eq!(t.listed().len(), listed, "nothing spawned");
    assert_eq!(t.world.vm.log.len(), logged, "no harness line");
}

/// Review r1 of #579, items 2 and 3: a set-up class is reported whatever happens to it. An absent class is not in the
/// packages, and nothing is loaded. A class that is only declared (`by_path` knows it, its layout is not defined) is
/// loaded and reported as a write. Asked again, it is already loaded.
#[test]
fn a_set_up_class_is_resolved_and_reported_whatever_its_state() {
    let mut t = toy();
    let vm = &mut t.world.vm;
    let (line, outcome) = preload_class(vm, "GearboxFramework.INoSuchInterface");
    assert!(outcome.is_ok());
    assert!(line.starts_with(TARGET_FLAG_PROVIDER_INPUT) && line.ends_with("not in the packages, nothing loaded"), "{line}");
    let id = vm.space.classes.declare(TARGET_FLAG_PROVIDER).unwrap();
    assert!(!vm.space.classes.get_layout(id).unwrap().is_defined(), "declared only");
    let (line, outcome) = preload_class(vm, TARGET_FLAG_PROVIDER);
    assert!(outcome.is_ok());
    assert!(line.contains("loaded at set-up (a write") && line.contains("UNVERIFIED"), "{line}");
    assert!(vm.space.classes.get_layout(vm.space.classes.by_path(TARGET_FLAG_PROVIDER).unwrap()).unwrap().is_defined());
    let (line, outcome) = preload_class(vm, TARGET_FLAG_PROVIDER);
    assert!(outcome.is_ok());
    assert!(line.ends_with("layout ready, code ready, nothing loaded"), "{line}");
}

/// Review r2 of #579, item 2: a class whose layout another load defined, with no code loaded, gets its code loaded by
/// the set-up step, and the line says so (layout ready, code loaded at set-up), so registration later loads nothing.
#[test]
fn a_set_up_class_with_a_ready_layout_still_gets_its_code() {
    let mut t = toy();
    let vm = &mut t.world.vm;
    let id = {
        let (code, store) = vm.defs.code_mut();
        let key = code.find_class(store, TARGET_FLAG_PROVIDER).unwrap();
        code.define_layout(store, &mut vm.space.names, &mut vm.space.classes, key).unwrap()
    };
    assert!(vm.space.classes.get_layout(id).unwrap().is_defined(), "layout defined");
    assert!(!vm.class_code_loaded(id), "no code yet");
    let (line, outcome) = preload_class(vm, TARGET_FLAG_PROVIDER);
    assert!(outcome.is_ok());
    assert!(line.contains("layout ready, code loaded at set-up (a write"), "{line}");
    assert!(vm.class_code_loaded(id), "the code is loaded at set-up");
    assert!(class_ready(vm, id));
}

/// Review r2 of #579, item 1: with no set-up, the fallback `WillowGame.WillowMind` is cold, and the target's mind
/// refuses before anything is loaded. A target whose `MyWillowMind` slot is also too narrow gets the same refusal:
/// the fallback is never loaded to reach the slot check.
#[test]
fn a_cold_fallback_mind_is_refused_with_nothing_loaded() {
    for target in ["Engine.ToyMindedTarget", "Engine.ToyNarrowTarget"] {
        let mut t = toy();
        let before = t.listed().len();
        let (r, _) = t.drive(&["Engine.ToyPawn", "Engine.ToyController", target], Grading::Strict);
        let Some((outcome, stop)) = &r.stop else { panic!("{target}: no stop") };
        assert_eq!(*outcome, Outcome::Error, "{target}");
        assert!(stop.text.contains("was not resolved at link 9's set-up"), "{target}: {}", stop.text);
        let vm = &t.world.vm;
        let cold = vm.space.classes.by_path(TARGET_MIND_FALLBACK).is_none_or(|c| !class_ready(vm, c));
        assert!(cold, "{target}: the fallback was not loaded");
        assert_eq!(t.listed().len(), before + 3, "{target}: no mind spawned");
    }
}

/// Review r1 of #579, item 3: link 9's set-up resolves each of its classes and reports one line per class, in
/// order. The toy packages hold only the fallback mind and `IFlagProvider` of them, so the rest are absent. The pawn
/// rules then find nothing to load.
#[test]
fn link_9_s_set_up_reports_every_class_it_resolves() {
    let mut t = toy();
    let mut harness = Vec::new();
    phaselock_setup(&mut t.world.vm, &mut harness).unwrap();
    let classes = phaselock_setup_classes();
    assert_eq!(harness.len(), classes.len(), "{harness:?}");
    for (line, path) in harness.iter().zip(&classes) {
        assert!(line.starts_with(&format!("{TARGET_FLAG_PROVIDER_INPUT} {path}: ")), "{line}");
    }
    assert_eq!(classes[0], TARGET_MIND_FALLBACK);
    assert!(harness[0].contains("loaded at set-up"), "{}", harness[0]);
    assert!(harness[1].contains("loaded at set-up"), "{}", harness[1]);
    assert!(classes.contains(&ow_combat::pawn_interface::PLAYER_MASTER_INTERFACE));
    // Lane HARNESS-LINK9-SETUP: the rule-engine owner interface is resolved too, right after the flag store's; the
    // toy packages lack it, so its line says nothing was loaded.
    assert_eq!(classes[2], TARGET_RULE_ENGINE_OWNER);
    assert!(harness[2].ends_with("not in the packages, nothing loaded"), "{}", harness[2]);
}

/// Review r1 of #579, item 4: a NotYet from the mind is marked once, at the start, with its kind and record kept, even
/// when its text already mentions the marker.
#[test]
fn a_mind_not_yet_is_marked_once_at_the_start() {
    let inside = ow_core::NotYet::not_read(format!("a rule ({PARTIAL_WRITES} inside)"), "R.md");
    let marked = inside.after_partial_writes();
    assert!(marked.what.starts_with(PARTIAL_WRITES), "{}", marked.what);
    assert_eq!(marked.what.matches(PARTIAL_WRITES).count(), 2, "the leading mark, and the text's own");
    assert_eq!(marked.record, "R.md");
    assert!(matches!(marked.missing, Missing::NotRead));
}

// ---- Lane HARNESS-GEARLIKENESS-SETUP: the set-up interface check (resolve_interface) ------------------------------

static GEAR_NEXT: AtomicU32 = AtomicU32::new(0);

/// A WillowGame package with decoded class bodies (`Declarations`): the gear-likeness interface, another interface,
/// a component class whose only entry is that other interface, and one that implements the gear-likeness interface.
fn decoded_gear_vm() -> (Vm, PathBuf) {
    use ow_object::class_body::CLASS_INTERFACE;
    use ow_package::testkit::reflect::{ClassBodySpec, Declarations, class_body};
    let dir = std::env::temp_dir().join(format!("ow-cli-gearlike-{}-{}", std::process::id(), GEAR_NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let mut d = Declarations::with_imports(&["Class", "Function", "StructProperty"]);
    let object_body = class_body(&mut d.p, &ClassBodySpec::new(0, 1, 2));
    let obj = d.p.add_export(0, "Object", object_body);
    d.default_object(obj, "Object");
    let gear = d.real_class("IGearLikenessConsumer", obj, obj, 2 | CLASS_INTERFACE, &[], None);
    let other = d.real_class("IToyOther", obj, obj, 2 | CLASS_INTERFACE, &[], None);
    d.real_class("ToyMesh", obj, obj, 2, &[other], None);
    d.real_class("ToyLikeness", obj, obj, 2, &[gear], None);
    let bytes = d.build();
    let defs = DefStore::from_synthetic(&dir, &[("WillowGame.upk", &bytes)], IdentityPolicy::FirstInScanOrder, ParityMode::Off)
        .expect("store");
    let vm = Vm::new(ObjectSpace::new(), defs, Rc::new(ow_attr::AttributeSystem::new_empty()), ParityMode::Off);
    (vm, dir)
}

fn counters(vm: &Vm) -> (u64, u64, u64) {
    (vm.space.heap.committed_writes() as u64, vm.defs.load_transitions() as u64, vm.rng.draws() as u64)
}

/// A decoded class whose entry is another interface: the answer (no) is read from the packages' interface lists, its
/// entry resolved at set-up (a labelled write). Afterwards hook 1's read-only check answers with no write, no load and
/// no draw, and a second set-up check resolves nothing.
#[test]
fn the_set_up_interface_check_reads_the_packages_lists_and_hook_1_then_loads_nothing() {
    use ow_world::world::render_hooks::{GEAR_LIKENESS, GEAR_UNRESOLVED, gear_likeness};
    let (mut vm, dir) = decoded_gear_vm();
    let mesh = vm.find_class("WillowGame.ToyMesh").unwrap();
    vm.find_class(GEAR_LIKENESS).unwrap();
    let Err(VmError::NotYet(n)) = gear_likeness(&vm, mesh) else { panic!("an unresolved entry refuses first") };
    assert_eq!(&*n.what, GEAR_UNRESOLVED);
    let (line, outcome) = resolve_interface(&mut vm, "WillowGame.ToyMesh", GEAR_LIKENESS);
    assert!(outcome.is_ok());
    assert!(line.starts_with(SETUP_INTERFACE_INPUT) && line.contains(": no, read from the packages' interface lists;"), "{line}");
    assert!(line.contains("resolved at set-up (a write; UNVERIFIED stand-in for when"), "{line}");
    let before = counters(&vm);
    gear_likeness(&vm, mesh).unwrap();
    assert_eq!(counters(&vm), before, "hook 1 reads only");
    let (line, _) = resolve_interface(&mut vm, "WillowGame.ToyMesh", GEAR_LIKENESS);
    assert!(line.ends_with("already resolved, nothing resolved"), "{line}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// An implementer answers yes, read the same way, and hook 1 then refuses with the refresh's `LayerAbsent`.
#[test]
fn the_set_up_interface_check_reads_an_implementer_s_yes() {
    use ow_world::world::render_hooks::{GEAR_LIKENESS, GEAR_REFRESH, gear_likeness};
    let (mut vm, dir) = decoded_gear_vm();
    let (line, outcome) = resolve_interface(&mut vm, "WillowGame.ToyLikeness", GEAR_LIKENESS);
    assert!(outcome.is_ok());
    assert!(line.contains(": yes, read from the packages' interface lists;"), "{line}");
    let like = vm.space.classes.by_path("WillowGame.ToyLikeness").unwrap();
    let Err(VmError::NotYet(n)) = gear_likeness(&vm, like) else { panic!() };
    assert!(matches!(n.missing, Missing::LayerAbsent(_)) && &*n.what == GEAR_REFRESH, "{n:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Bodies that do not decode (the toy's `ScriptToy` classes) answer by the structural fallback, labelled an
/// UNVERIFIED stand-in; a class the packages do not hold resolves nothing.
#[test]
fn the_set_up_interface_check_labels_a_structural_answer_and_reports_an_absent_class() {
    let mut t = toy();
    let vm = &mut t.world.vm;
    let (line, outcome) = resolve_interface(vm, "Engine.ToyPawn", TARGET_FLAG_PROVIDER);
    assert!(outcome.is_ok());
    assert!(line.contains("by the structural fallback") && line.contains("UNVERIFIED stand-in"), "{line}");
    let before = counters(vm);
    let (line, outcome) = resolve_interface(vm, "Engine.NoSuchComponent", TARGET_FLAG_PROVIDER);
    assert!(outcome.is_ok());
    assert!(line.ends_with("not in the packages, nothing resolved"), "{line}");
    assert_eq!(counters(vm), before, "nothing loaded");
}
