//! Tests of the spawn extent's class list for `WillowPickup` (lane PLACEMENT-WILLOWPICKUP-EXTENT), on two invented
//! packages built with `ow_package::testkit` (AGENTS.md rule 9). `Engine` has `Actor` (`CollisionComponent`,
//! `Components`), `PrimitiveComponent` (`bAttached`, `CollideActors`) and `CylinderComponent` (`CollisionRadius`,
//! `CollisionHeight`). `WillowGame` has `WillowPickup` and a subclass of it. Every value is invented. AI-assisted
//! (Claude Code).

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::ParityMode;
use ow_core::notyet::Missing;
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_object::ids::ObjRef;
use ow_object::value::Value;
use ow_core::ObjectPath;
use ow_package::testkit::ints;
use ow_package::testkit::reflect::toy_array_of;
use ow_package::testkit::script::ScriptToy;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use super::{BOUNDING_CYLINDER_OF, NoLevel, extent, install_collision, load_extent_components};
use crate::actors::set_field;

static NEXT: AtomicU32 = AtomicU32::new(0);

const IMPORTS: [&str; 5] = ["Class", "ObjectProperty", "BoolProperty", "FloatProperty", "ArrayProperty"];

fn engine() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let object = t.class("Object", 0);
    let actor = t.class("Actor", object);
    let primitive = t.class("PrimitiveComponent", object);
    t.prop("Bool", primitive, "bAttached", 0, 0);
    t.prop("Bool", primitive, "CollideActors", 0, 0);
    t.prop("Bool", primitive, "BulletListener", 0, 0);
    let cylinder = t.class("CylinderComponent", primitive);
    t.prop("Float", cylinder, "CollisionRadius", 0, 0);
    t.prop("Float", cylinder, "CollisionHeight", 0, 0);
    t.prop("Object", actor, "CollisionComponent", 0, primitive);
    toy_array_of(&mut t, actor, "Components", "Object", primitive);
    t.build()
}

fn willow_game() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let actor = t.p.add_import("Core", "Class", engine, "Actor");
    let primitive = t.p.add_import("Core", "Class", engine, "PrimitiveComponent");
    let pickup = t.class("WillowPickup", actor);
    t.class("ToyPickupSubclass", pickup);
    // A pickup archetype whose one `Components` entry is a primitive export: loading the archetype leaves it lazy.
    let mut empty = ints(&[0]);
    empty.extend_from_slice(&t.none);
    let part = t.p.add_export(primitive, "ToyPickupPart", empty);
    let mut arch = ints(&[0]);
    arch.extend(t.p.array_tag("Components", 1, &ints(&[part])));
    arch.extend_from_slice(&t.none);
    t.p.add_export(pickup, "ToyPickupArchetype", arch);
    t.build()
}

struct Fx {
    vm: Vm,
    dir: PathBuf,
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn fx() -> Fx {
    let dir = std::env::temp_dir().join(format!(
        "ow-world-placement-pickup-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let (e, w) = (engine(), willow_game());
    let files = [("Engine.upk", e.as_slice()), ("WillowGame.upk", w.as_slice())];
    let defs = DefStore::from_synthetic(&dir, &files, IdentityPolicy::FirstInScanOrder, ParityMode::Off).unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
    for c in ["Engine.CylinderComponent", "WillowGame.ToyPickupSubclass"] {
        vm.find_class(c).unwrap();
    }
    Fx { vm, dir }
}

impl Fx {
    /// The class default of `path`: the spawn's template when there is no archetype.
    fn template(&mut self, path: &str) -> ObjRef {
        self.vm.defs.class_default(&mut self.vm.space, path).unwrap()
    }

    fn make(&mut self, path: &str) -> ObjRef {
        let c = self.vm.find_class(path).unwrap();
        ow_vm::interp::instantiate_from_defaults(&mut self.vm, c).unwrap()
    }

    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        set_field(&mut self.vm.space, o, name, v).unwrap();
    }
}

fn not_read(r: Result<[f32; 3], VmError>) -> String {
    match r {
        Err(VmError::NotYet(n)) => {
            assert_eq!(n.missing, Missing::NotRead);
            n.what.into_owned()
        }
        other => panic!("expected NotYet, got {other:?}"),
    }
}

/// A `WillowPickup` template with no cylinder `CollisionComponent` takes `Actor`'s rule, as
/// `Actor.GetBoundingCylinder` does (#595): with no qualifying `Components` entry, (0, 0, 0). Before this lane it
/// refused `NotRead`.
#[test]
fn a_willow_pickup_template_takes_the_actor_rule() {
    let mut x = fx();
    let pickup = x.template("WillowGame.WillowPickup");
    assert_eq!(extent(&x.vm, pickup).unwrap(), [0.0; 3]);
    // An unattached primitive entry is skipped: still (0, 0, 0).
    let entry = x.make("Engine.PrimitiveComponent");
    x.set(entry, "CollideActors", Value::Bool(true));
    x.set(pickup, "Components", Value::array(vec![Value::Object(Some(entry))]));
    assert_eq!(extent(&x.vm, pickup).unwrap(), [0.0; 3]);
    // An attached, colliding entry reads the skip flag (lane IMPL-SKIP-FLAG): clear, it reaches the bounds step,
    // refusal L's `LayerAbsent`, as for `Actor`; set, it is skipped.
    x.set(entry, "bAttached", Value::Bool(true));
    match extent(&x.vm, pickup) {
        Err(VmError::NotYet(n)) => {
            assert_eq!(n.what, crate::world::bounding_cylinder::REFUSAL_L, "{n}");
            assert_eq!(n.missing, Missing::LayerAbsent(crate::world::bounding_cylinder::BOUNDS_LAYER), "{n}");
        }
        other => panic!("expected L, got {other:?}"),
    }
    x.set(entry, "BulletListener", Value::Bool(true));
    assert_eq!(extent(&x.vm, pickup).unwrap(), [0.0; 3]);
}

/// A cylinder `CollisionComponent` still comes first (the spawn's own step 1), for a pickup as for any class.
#[test]
fn a_cylinder_collision_component_still_comes_first() {
    let mut x = fx();
    let pickup = x.template("WillowGame.WillowPickup");
    let cylinder = x.make("Engine.CylinderComponent");
    x.set(cylinder, "CollisionRadius", Value::Float(12.5));
    x.set(cylinder, "CollisionHeight", Value::Float(7.0));
    x.set(pickup, "CollisionComponent", Value::Object(Some(cylinder)));
    assert_eq!(extent(&x.vm, pickup).unwrap(), [12.5, 12.5, 7.0]);
}

/// The class list is by exact class: a subclass of `WillowPickup` is not assumed to keep its parent's body.
#[test]
fn a_willow_pickup_subclass_is_still_not_read() {
    let mut x = fx();
    let sub = x.template("WillowGame.ToyPickupSubclass");
    assert_eq!(not_read(extent(&x.vm, sub)), format!("{BOUNDING_CYLINDER_OF} WillowGame.ToyPickupSubclass"));
}


/// Review r1 of #606: the encroachment check calls `extent` on the live actor, after its step 3.2 refused any
/// colliding primitive. A live pickup whose primitives all have `CollideActors` clear gets (0, 0, 0), even with
/// `bAttached` set (the entries are skipped either way).
#[test]
fn a_live_pickup_with_no_colliding_primitive_gets_a_zero_extent() {
    let mut x = fx();
    let pickup = x.make("WillowGame.WillowPickup");
    let entries: Vec<Value> = (0..2)
        .map(|i| {
            let e = x.make("Engine.PrimitiveComponent");
            x.set(e, "bAttached", Value::Bool(i == 0));
            Value::Object(Some(e))
        })
        .collect();
    x.set(pickup, "Components", Value::array(entries));
    assert_eq!(extent(&x.vm, pickup).unwrap(), [0.0; 3]);
}

/// Review r1 of #606: a pickup archetype's lazy primitive entry is loaded by `load_extent_components`, which reports
/// the write; the extent then reads the loaded entry.
#[test]
fn a_pickup_archetypes_lazy_primitive_entry_is_loaded_and_reported() {
    let mut x = fx();
    install_collision(&mut x.vm, Rc::new(NoLevel)).unwrap();
    let path = ObjectPath::parse("WillowGame.ToyPickupArchetype").unwrap();
    let arch = x.vm.defs.find(&mut x.vm.space, &path).unwrap().unwrap();
    let entries = crate::actors::objects(&x.vm.space, arch, "Components").unwrap();
    let part = entries.into_iter().flatten().next().expect("the archetype's entry");
    assert!(x.vm.defs.is_unloaded(&x.vm.space, part), "the entry starts lazy");
    assert!(load_extent_components(&mut x.vm, arch).unwrap(), "a load is a write");
    assert!(!x.vm.defs.is_unloaded(&x.vm.space, part), "the entry is loaded");
    assert_eq!(extent(&x.vm, arch).unwrap(), [0.0; 3]);
}
