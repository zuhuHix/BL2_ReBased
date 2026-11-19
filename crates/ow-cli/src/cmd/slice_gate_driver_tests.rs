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

/// `Engine.upk`: `Actor` with the fields the spawn reads, `WorldInfo`, a toy component class, `ToyPawn` and
/// `ToyTarget` (each with a component field), `ToyController`, `ToyBlocked` (its defaults collide with the world, so
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
    t.prop("Struct", actor, "Rotation", 0, rotator);
    let world = t.class("WorldInfo", actor);
    t.prop("Byte", world, "NetMode", 0, 0);
    t.prop("Float", world, "TimeSeconds", 0, 0);
    let core_pkg = t.p.add_import("Core", "Package", 0, "Core");
    let component = t.p.add_import("Core", "Class", core_pkg, "Component");
    let toy_component = t.class("ToyComponent", component);
    t.prop("Int", toy_component, "Payload", 0, 0);
    for name in ["ToyPawn", "ToyTarget"] {
        let c = t.class(name, actor);
        t.prop("Object", c, "Comp", 0, toy_component);
    }
    t.class("ToyController", actor);
    let blocked = t.class("ToyBlocked", actor);
    let mut defaults = ow_package::testkit::ints(&[0]);
    defaults.extend(t.p.bool_tag("bCollideWorld", true));
    defaults.extend_from_slice(&t.none);
    t.p.add_export(blocked, "Default__ToyBlocked", defaults);
    t.class("Thing", object);
    t.p.build()
}

struct Toy {
    world: World,
    dir: PathBuf,
    /// The component each spawnable class's defaults hold, outered to that class default.
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
    let (c, e) = (core(), engine());
    let files: [(&str, &[u8]); 2] = [("Core.upk", &c), ("Engine.upk", &e)];
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
    for name in ["ToyPawn", "ToyTarget"] {
        let vm = &mut world.vm;
        let cdo = vm.defs.class_default(&mut vm.space, &format!("Engine.{name}")).unwrap();
        let component_class = vm.find_class("Engine.ToyComponent").unwrap();
        let component = vm.space.alloc(component_class, "ToyComponent0", Some(cdo), Origin::Runtime).unwrap();
        let p = vm.space.prop(vm.space.heap.get(cdo).unwrap().class, "Comp").unwrap();
        vm.space.write(cdo, PropAddr::scalar(p), Value::Object(Some(component))).unwrap();
        templates.push((cdo, component));
    }
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
        drive_phaselock(&mut r, &mut self.world.vm, grading, classes, move |_vm, actors| {
            got.set(Some(actors));
            Ok(())
        });
        (r, seen.get())
    }
}

const ACTORS: [&str; 3] = ["Engine.ToyPawn", "Engine.ToyController", "Engine.ToyTarget"];

/// All three actors are spawned and registered, each with its own component (not the class default's), and the cast
/// runs on them.
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
    assert!(listed.contains(&pawn) && listed.contains(&controller) && listed.contains(&target));
    for (actor, (_, template)) in [pawn, target].into_iter().zip(t.templates.clone()) {
        let own = t.comp(actor);
        assert_ne!(own, template, "not the class default's component");
        assert_eq!(t.world.vm.space.heap.get(own).unwrap().outer, Some(actor));
    }
}

/// A refusal before the first spawn: the spawn's `NotYet` (graded from its variant) is the link's stop, without the
/// partial-writes mark, and the cast does not run.
#[test]
fn a_refusal_before_the_first_spawn_stops_the_link_unmarked() {
    let mut t = toy();
    let before = t.listed().len();
    let (r, actors) = t.drive(&["Engine.ToyBlocked", "Engine.ToyController", "Engine.ToyTarget"], Grading::Strict);
    let (outcome, stop) = r.stop.expect("a stop");
    assert_eq!(outcome, Outcome::NotYet);
    assert_eq!(stop.step, "spawn");
    assert!(!stop.text.contains(PARTIAL_WRITES), "{}", stop.text);
    assert!(actors.is_none(), "the cast did not run");
    assert_eq!(t.listed().len(), before, "nothing registered");
}

/// A `NotYet` after one spawn succeeded is marked as after partial writes, its kind kept; the earlier actor stays
/// registered, and the cast does not run.
#[test]
fn a_not_yet_after_a_committed_spawn_is_marked() {
    let mut t = toy();
    let before = t.listed().len();
    let mut r = Ran::default();
    drive_phaselock(&mut r, &mut t.world.vm, Grading::Strict, &["Engine.ToyPawn", "Engine.ToyBlocked", "Engine.ToyTarget"], |_, _| {
        panic!("the cast must not run")
    });
    let (outcome, stop) = r.stop.expect("a stop");
    assert_eq!(outcome, Outcome::NotYet);
    assert!(stop.text.starts_with(PARTIAL_WRITES), "{}", stop.text);
    assert_eq!(t.listed().len(), before + 1, "the pawn stays registered");
    // The helper alone keeps the NotYet's kind.
    match phaselock_actors(&mut t.world.vm, &["Engine.ToyPawn", "Engine.ToyBlocked", "Engine.ToyTarget"]) {
        Err(VmError::NotYet(n)) => {
            assert!(matches!(n.missing, Missing::LayerAbsent(_)), "{n}");
            assert!(n.what.starts_with(PARTIAL_WRITES), "{n}");
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
        assert!(r.error_lines.is_empty(), "{grading:?}: not an error line gone past");
        assert!(actors.is_none(), "{grading:?}: the cast did not run");
    }
}
