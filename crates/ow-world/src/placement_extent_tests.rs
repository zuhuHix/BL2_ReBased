//! The spawn's extent (lane IMPL-LINK9-EXTENT, NATIVE_SPAWN_EXTENT.md) on invented classes with the game's class and
//! field names (AGENTS.md rule 9). AI-assisted (Claude Code).

use std::rc::Rc;

use ow_attr::AttributeSystem;
use ow_core::{Missing, ParityMode};
use ow_defs::DefStore;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ClassId, ObjRef, PropAddr};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use super::{BOUNDING_CYLINDER_OF, EXTENT_RECORD, box_cylinder, extent};
use crate::world::bounding_cylinder::{BOUNDS_LAYER, COMPONENT_CHECK_OF, REFUSAL_L, SKIP_RECORD};

struct Fx {
    vm: Vm,
}

fn vm_of(s: ObjectSpace) -> Vm {
    Vm::new(s, DefStore::new_empty(), Rc::new(AttributeSystem::new_empty()), ParityMode::Off)
}

fn fx() -> Fx {
    let mut s = ObjectSpace::new();
    let object = s.classes.define("Core.Object", None, vec![]).unwrap();
    let obj = PropKind::Object { class: object };
    let d = |s: &mut ObjectSpace, n: &str, k: PropKind| PropertyDecl::new(s.names.intern(n), k);
    let decls = vec![
        d(&mut s, "bAttached", PropKind::Bool),
        d(&mut s, "CollideActors", PropKind::Bool),
        d(&mut s, "BulletListener", PropKind::Bool),
    ];
    let primitive = s.classes.define("Engine.PrimitiveComponent", Some(object), decls).unwrap();
    // A primitive class whose per-component check was not read (lane IMPL-SKIP-FLAG).
    s.classes.define("Engine.SkeletalMeshComponent", Some(primitive), vec![]).unwrap();
    let decls = vec![d(&mut s, "CollisionRadius", PropKind::Float), d(&mut s, "CollisionHeight", PropKind::Float)];
    s.classes.define("Engine.CylinderComponent", Some(primitive), decls).unwrap();
    s.classes.define("Test.AudioComponent", Some(object), vec![]).unwrap();
    let decls = vec![
        d(&mut s, "CollisionComponent", obj.clone()),
        d(&mut s, "Components", PropKind::Array(Box::new(obj.clone()))),
    ];
    let actor = s.classes.define("Engine.Actor", Some(object), decls).unwrap();
    let decls = vec![d(&mut s, "CylinderComponent", obj)];
    let pawn = s.classes.define("Engine.Pawn", Some(actor), decls).unwrap();
    let willow_pawn = s.classes.define("WillowGame.WillowPawn", Some(pawn), vec![]).unwrap();
    s.classes.define("WillowGame.WillowAIPawn", Some(willow_pawn), vec![]).unwrap();
    s.classes.define("WillowGame.WillowPlayerPawn", Some(willow_pawn), vec![]).unwrap();
    s.classes.define("Test.OtherActor", Some(actor), vec![]).unwrap();
    Fx { vm: vm_of(s) }
}

impl Fx {
    fn class(&self, p: &str) -> ClassId {
        self.vm.space.classes.by_path(p).unwrap()
    }

    fn obj(&mut self, class: &str) -> ObjRef {
        let c = self.class(class);
        self.vm.space.alloc(c, "Test", None, Origin::Runtime).unwrap()
    }

    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let c = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(c, name).unwrap();
        self.vm.space.write(o, PropAddr::scalar(p), v).unwrap();
    }

    fn cylinder(&mut self, r: f32, h: f32) -> ObjRef {
        let c = self.obj("Engine.CylinderComponent");
        self.set(c, "CollisionRadius", Value::Float(r));
        self.set(c, "CollisionHeight", Value::Float(h));
        c
    }

    fn primitive(&mut self, attached: bool, collides: bool) -> ObjRef {
        let c = self.obj("Engine.PrimitiveComponent");
        self.set(c, "bAttached", Value::Bool(attached));
        self.set(c, "CollideActors", Value::Bool(collides));
        c
    }

    fn components(&mut self, o: ObjRef, list: Vec<Option<ObjRef>>) {
        self.set(o, "Components", Value::array(list.into_iter().map(Value::Object).collect()));
    }
}

fn not_read(e: VmError) -> String {
    match e {
        VmError::NotYet(n) => {
            assert_eq!((n.missing, n.record), (Missing::NotRead, EXTENT_RECORD));
            n.what.into_owned()
        }
        other => panic!("expected NotYet, got {other:?}"),
    }
}

/// A cylinder `CollisionComponent` gives its radius and height, on any class.
#[test]
fn a_cylinder_collision_component_gives_its_extent() {
    let mut x = fx();
    let t = x.obj("Test.OtherActor");
    let c = x.cylinder(30.0, 60.0);
    x.set(t, "CollisionComponent", Value::Object(Some(c)));
    assert_eq!(extent(&x.vm, t).unwrap(), [30.0, 30.0, 60.0]);
}

/// The pawns' rule (section 3): a separate cylinder `CylinderComponent`, on each of the four classes.
#[test]
fn a_pawn_s_separate_cylinder_component_gives_its_extent() {
    for class in ["Engine.Pawn", "WillowGame.WillowPawn", "WillowGame.WillowAIPawn", "WillowGame.WillowPlayerPawn"] {
        let mut x = fx();
        let t = x.obj(class);
        let c = x.cylinder(20.0, 44.0);
        x.set(t, "CylinderComponent", Value::Object(Some(c)));
        assert_eq!(extent(&x.vm, t).unwrap(), [20.0, 20.0, 44.0], "{class}");
    }
}

/// Link 9's template (section 4): a player pawn with no collision cylinder and only unattached components (and one
/// that is not a primitive) has the zero extent; a `CylinderComponent` that is not a cylinder falls to the same rule.
#[test]
fn a_pawn_without_a_cylinder_and_unattached_components_has_the_zero_extent() {
    let mut x = fx();
    let t = x.obj("WillowGame.WillowPlayerPawn");
    let (a, b, audio) = (x.primitive(false, true), x.primitive(false, false), x.obj("Test.AudioComponent"));
    x.components(t, vec![Some(a), None, Some(b), Some(audio)]);
    assert_eq!(extent(&x.vm, t).unwrap(), [0.0; 3]);
    let not_a_cylinder = x.primitive(false, true);
    x.set(t, "CylinderComponent", Value::Object(Some(not_a_cylinder)));
    assert_eq!(extent(&x.vm, t).unwrap(), [0.0; 3]);
    // An attached primitive that does not collide does not qualify either.
    let quiet = x.primitive(true, false);
    x.components(t, vec![Some(quiet)]);
    assert_eq!(extent(&x.vm, t).unwrap(), [0.0; 3]);
}

/// `Actor`'s rule on an attached colliding primitive reads the skip flag (lane IMPL-SKIP-FLAG,
/// NATIVE_COMPONENT_SKIP_FLAG.md section 5, test 6): set, the entry is skipped and the extent stays (0, 0, 0); clear,
/// the component check passes a `PrimitiveComponent` and the bounds step is refusal L's `LayerAbsent`; a class whose
/// check was not read is the check's `NotRead`. Nothing changes on any path.
#[test]
fn an_attached_colliding_primitive_reads_the_skip_flag() {
    let mut x = fx();
    let t = x.obj("Engine.Actor");
    let p = x.primitive(true, true);
    x.components(t, vec![Some(p)]);
    let marks = |x: &Fx| (x.vm.space.heap.committed_writes(), x.vm.rng.draws(), x.vm.defs.load_transitions(), x.vm.space.names.len());
    let before = marks(&x);
    match extent(&x.vm, t).unwrap_err() {
        VmError::NotYet(n) => {
            assert_eq!(n.missing, Missing::LayerAbsent(BOUNDS_LAYER), "{n}");
            assert_eq!((n.what.as_ref(), n.record), (REFUSAL_L, EXTENT_RECORD));
        }
        other => panic!("expected L, got {other:?}"),
    }
    assert_eq!(marks(&x), before);
    let skel = x.obj("Engine.SkeletalMeshComponent");
    x.set(skel, "bAttached", Value::Bool(true));
    x.set(skel, "CollideActors", Value::Bool(true));
    x.components(t, vec![Some(skel)]);
    let before = marks(&x);
    match extent(&x.vm, t).unwrap_err() {
        VmError::NotYet(n) => {
            assert_eq!(n.missing, Missing::NotRead, "{n}");
            assert_eq!(n.what, format!("{COMPONENT_CHECK_OF} Engine.SkeletalMeshComponent (not read)"));
            assert_eq!(n.record, SKIP_RECORD);
        }
        other => panic!("expected the component check, got {other:?}"),
    }
    assert_eq!(marks(&x), before);
    for c in [p, skel] {
        x.set(c, "BulletListener", Value::Bool(true));
    }
    x.components(t, vec![Some(p), Some(skel)]);
    let before = marks(&x);
    assert_eq!(extent(&x.vm, t).unwrap(), [0.0; 3], "bullet listeners are skipped");
    assert_eq!(marks(&x), before);
}

/// Any class other than the five read is a named `NotYet`, even a subclass of `Actor`.
#[test]
fn another_class_s_rule_is_not_read() {
    let mut x = fx();
    let t = x.obj("Test.OtherActor");
    assert_eq!(not_read(extent(&x.vm, t).unwrap_err()), format!("{BOUNDING_CYLINDER_OF} Test.OtherActor"));
}

/// The box's cylinder (section 2, step 3): radius the horizontal half-diagonal, half height half the height.
#[test]
fn the_box_cylinder_is_the_half_diagonal_and_half_height() {
    assert_eq!(box_cylinder(&[]), [0.0; 3]);
    assert_eq!(box_cylinder(&[([0.0; 3], [3.0, 4.0, 5.0])]), [5.0, 5.0, 5.0]);
    // Two boxes: x from -1 to 5, y from -4 to 4, z from 0 to 2: half sizes 3, 4 and 1.
    let union = box_cylinder(&[([0.0, 0.0, 1.0], [1.0, 4.0, 1.0]), ([4.0, 0.0, 1.0], [1.0, 1.0, 0.5])]);
    assert_eq!(union, [5.0, 5.0, 1.0]);
}

/// A loaded stand-in with no slots among the `Components` is a named `NotYet`, not a read of missing slots.
#[test]
fn a_stand_in_primitive_entry_is_not_read() {
    let mut x = fx();
    let t = x.obj("Engine.Actor");
    let c = x.class("Engine.PrimitiveComponent");
    let stand_in = x.vm.space.alloc_stand_in(c, "StandIn", Origin::Resource { package: 0, index: -1 }).unwrap();
    x.components(t, vec![Some(stand_in)]);
    let what = not_read(extent(&x.vm, t).unwrap_err());
    assert!(what.contains("a stand-in with no slots"), "{what}");
}

/// A field the classes do not declare is bad data, a `VmError::Script` naming it (NATIVE_SPAWN_EXTENT.md section 5).
#[test]
fn a_field_the_classes_do_not_declare_is_a_script_error() {
    let mut s = ObjectSpace::new();
    let object = s.classes.define("Core.Object", None, vec![]).unwrap();
    let primitive = s.classes.define("Engine.PrimitiveComponent", Some(object), vec![]).unwrap();
    let obj = PropKind::Object { class: object };
    let decls = vec![
        PropertyDecl::new(s.names.intern("CollisionComponent"), obj.clone()),
        PropertyDecl::new(s.names.intern("Components"), PropKind::Array(Box::new(obj))),
    ];
    let actor = s.classes.define("Engine.Actor", Some(object), decls).unwrap();
    let mut vm = vm_of(s);
    let t = vm.space.alloc(actor, "Test", None, Origin::Runtime).unwrap();
    let c = vm.space.alloc(primitive, "Test", None, Origin::Runtime).unwrap();
    let p = vm.space.prop(actor, "Components").unwrap();
    vm.space.write(t, PropAddr::scalar(p), Value::array(vec![Value::Object(Some(c))])).unwrap();
    match extent(&vm, t).unwrap_err() {
        VmError::Script { message, .. } => assert!(message.contains("declares no bAttached"), "{message}"),
        other => panic!("expected a Script error, got {other:?}"),
    }
}
