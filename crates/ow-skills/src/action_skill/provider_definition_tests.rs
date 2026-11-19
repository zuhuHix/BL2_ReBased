//! Tests of `ActionSkill.GetBehaviorProviderDefinition` in Off (lane IMPL-GET-BEHAVIOR-PROVIDER-DEFINITION), on
//! invented classes with the note's class and field names, defined by hand, and a small invented package for an
//! unloaded handle (AGENTS.md rule 9). AI-assisted (Claude Code).

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::notyet::Missing;
use ow_core::{ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ClassId, ObjRef, PropAddr};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_package::testkit::script::ScriptToy;
use ow_package::testkit::{Export, ints};
use ow_vm::api::{FunctionDecl, Vm};
use ow_vm::error::VmError;

use super::*;

static NEXT: AtomicU32 = AtomicU32::new(0);

struct Fx {
    vm: Vm,
    skill: ClassId,
    dir: PathBuf,
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// `ToyProvider.upk`: a skill-like holder `Holder_0` whose `BehaviorProviderDefinition` names `Definition_0` and whose
/// `Next` names `Holder_1`, both exports the store keeps unloaded until they are loaded.
fn toy_package() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class", "ObjectProperty"]);
    let object = t.class("Object", 0);
    let definition = t.class("Definition", object);
    let holder = t.class("Holder", object);
    t.prop("Object", holder, "BehaviorProviderDefinition", 0, object);
    t.prop("Object", holder, "Next", 0, object);
    // `next_export()` is the reference of the export added next.
    let first = t.p.next_export();
    let (held, next) = (first + 1, first + 2);
    let mut payload = ints(&[0]);
    payload.extend(t.p.object_tag("BehaviorProviderDefinition", held));
    payload.extend(t.p.object_tag("Next", next));
    payload.extend_from_slice(&t.none);
    t.p.add_export_with(Export::new(holder, "Holder_0", payload));
    let mut payload = ints(&[0]);
    payload.extend_from_slice(&t.none);
    t.p.add_export_with(Export::new(definition, "Definition_0", payload.clone()));
    t.p.add_export_with(Export::new(holder, "Holder_1", payload));
    t.build()
}

fn fx(parity: ParityMode) -> Fx {
    let mut s = ObjectSpace::new();
    let object = s.classes.define("Core.Object", None, vec![]).unwrap();
    let field = PropertyDecl::new(s.names.intern("BehaviorProviderDefinition"), PropKind::Object { class: object });
    let actor = s.classes.define("Engine.Actor", Some(object), vec![]).unwrap();
    let skill = s.classes.define("WillowGame.ActionSkill", Some(actor), vec![field]).unwrap();
    s.classes.define("WillowGame.LiftActionSkill", Some(skill), vec![]).unwrap();
    s.classes.define("GearboxFramework.BehaviorProviderDefinition", Some(object), vec![]).unwrap();
    let wrong = PropertyDecl::new(s.names.intern("BehaviorProviderDefinition"), PropKind::Int);
    s.classes.define("Toy.WrongKind", Some(object), vec![wrong]).unwrap();
    s.classes.define("Toy.NoField", Some(object), vec![]).unwrap();
    let dir = std::env::temp_dir().join(format!("ow-skills-gbpd-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let package = toy_package();
    let defs = DefStore::from_synthetic(&dir, &[("ToyProvider.upk", package.as_slice())], IdentityPolicy::FirstInScanOrder, parity)
        .unwrap();
    let mut vm = Vm::new(s, defs, Rc::new(AttributeSystem::new_empty()), parity);
    let decl = FunctionDecl::native(ObjectPath::parse(GET_BEHAVIOR_PROVIDER_DEFINITION).unwrap(), "ActionSkill.GetBehaviorProviderDefinition()")
        .with_owner(skill)
        .with_result(PropertyDecl::new(vm.space.names.intern("ReturnValue"), PropKind::Object { class: object }));
    vm.declare_function(decl).unwrap();
    register(&mut vm).unwrap();
    Fx { vm, skill, dir }
}

impl Fx {
    fn obj(&mut self, path: &str, name: &str) -> ObjRef {
        let c = self.vm.space.classes.by_path(path).unwrap();
        self.vm.space.alloc(c, name, None, Origin::Runtime).unwrap()
    }

    fn set(&mut self, o: ObjRef, v: Option<ObjRef>) {
        let c = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(c, "BehaviorProviderDefinition").unwrap();
        self.vm.space.write(o, PropAddr::scalar(p), Value::Object(v)).unwrap();
    }

    fn call(&mut self, me: ObjRef) -> Result<Value, VmError> {
        let f = self.vm.find_function(GET_BEHAVIOR_PROVIDER_DEFINITION).unwrap();
        self.vm.call(f, Some(me), Vec::new()).map(|r| r.ret)
    }
}

/// The skill's own field, as stored, through the bound native; a subclass's call answers the same way; None is None.
#[test]
fn the_answer_is_the_skills_own_field() {
    let mut x = fx(ParityMode::Off);
    let lift = x.obj("WillowGame.LiftActionSkill", "ActionSkill_Phaselock");
    let definition = x.obj("GearboxFramework.BehaviorProviderDefinition", "BehaviorProviderDefinition_0");
    x.set(lift, Some(definition));
    assert_eq!(x.call(lift).unwrap(), Value::Object(Some(definition)));
    let bare = x.obj("WillowGame.ActionSkill", "Bare");
    assert_eq!(x.call(bare).unwrap(), Value::Object(None), "None, with no fallback");
    let _ = x.skill;
}

/// The stored reference is returned without loading what it names, and nothing is written.
#[test]
fn the_named_definition_stays_unloaded_and_nothing_is_written() {
    let mut x = fx(ParityMode::Off);
    let holder = x.vm.defs.find(&mut x.vm.space, &ObjectPath::parse("ToyProvider.Holder_0").unwrap()).unwrap().unwrap();
    let before = format!("{:?}", x.vm.space.heap);
    let answer = provider_definition(&x.vm, holder).unwrap().expect("a definition");
    assert!(x.vm.defs.is_unloaded(&x.vm.space, answer), "not loaded");
    assert_eq!(format!("{:?}", x.vm.space.heap), before, "nothing written");
}

/// The port's own refusals: a class without the field, or with a field of another kind, is a data error; an unloaded
/// receiver is `NotRead`.
#[test]
fn the_refusals() {
    let mut x = fx(ParityMode::Off);
    let none = x.obj("Toy.NoField", "NoField");
    match provider_definition(&x.vm, none) {
        Err(VmError::Script { message, .. }) => assert!(message.contains("declares no BehaviorProviderDefinition"), "{message}"),
        other => panic!("{other:?}"),
    }
    let wrong = x.obj("Toy.WrongKind", "Wrong");
    match provider_definition(&x.vm, wrong) {
        Err(VmError::Script { message, .. }) => assert!(message.contains("is not an object reference"), "{message}"),
        other => panic!("{other:?}"),
    }
    // An unloaded receiver whose class declares the field: `Holder_0.Next`, still a handle.
    let holder = x.vm.defs.find(&mut x.vm.space, &ObjectPath::parse("ToyProvider.Holder_0").unwrap()).unwrap().unwrap();
    let c = x.vm.space.heap.get(holder).unwrap().class;
    let p = x.vm.space.prop(c, "Next").unwrap();
    let Value::Object(Some(lazy)) = x.vm.space.read(holder, PropAddr::scalar(p)).unwrap().clone() else { panic!("Next") };
    assert!(x.vm.defs.is_unloaded(&x.vm.space, lazy));
    // Main has no #518 load counter yet: the heap's Debug text and the handle's unloaded state stand in for it.
    let before = format!("{:?}", x.vm.space.heap);
    match provider_definition(&x.vm, lazy) {
        Err(VmError::NotYet(n)) => {
            assert_eq!(n.missing, Missing::NotRead, "{n}");
            assert_eq!(n.what, "GetBehaviorProviderDefinition: an unloaded receiver", "unmarked");
        }
        other => panic!("{other:?}"),
    }
    assert!(x.vm.defs.is_unloaded(&x.vm.space, lazy), "still not loaded");
    assert_eq!(format!("{:?}", x.vm.space.heap), before, "nothing written");
}

/// Reference binds nothing: the declaration stays unbound.
#[test]
fn reference_binds_nothing() {
    let mut x = fx(ParityMode::Reference);
    let f = x.vm.find_function(GET_BEHAVIOR_PROVIDER_DEFINITION).unwrap();
    let key = x.vm.function(f).and_then(|d| d.native_key.clone()).unwrap();
    assert!(!x.vm.has_native(&key));
    let _ = x.obj("WillowGame.ActionSkill", "Bare");
}

#[test]
fn off_binds_the_native() {
    let mut x = fx(ParityMode::Off);
    let f = x.vm.find_function(GET_BEHAVIOR_PROVIDER_DEFINITION).unwrap();
    let key = x.vm.function(f).and_then(|d| d.native_key.clone()).unwrap();
    assert!(x.vm.has_native(&key));
}

/// Review r1 of #560, item 1: a stand-in with no slots, whose class has some, is the same unmarked refusal, not a heap
/// error; nothing is written. The subclass's call finds the declaration on `ActionSkill`.
#[test]
fn a_slotless_stand_in_is_an_unloaded_receiver() {
    let mut x = fx(ParityMode::Off);
    let skill = x.skill;
    let stand_in = x.vm.space.alloc_stand_in(skill, "ActionSkill_StandIn", Origin::Default { class: skill }).unwrap();
    assert!(!x.vm.defs.is_unloaded(&x.vm.space, stand_in), "the store does not report it");
    let before = format!("{:?}", x.vm.space.heap);
    match provider_definition(&x.vm, stand_in) {
        Err(VmError::NotYet(n)) => {
            assert_eq!(n.missing, Missing::NotRead, "{n}");
            assert_eq!(n.what, "GetBehaviorProviderDefinition: an unloaded receiver", "unmarked");
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(format!("{:?}", x.vm.space.heap), before, "nothing written");
    let lift = x.obj("WillowGame.LiftActionSkill", "Lift");
    assert!(crate::action_skill::method_of(&mut x.vm, lift, "GetBehaviorProviderDefinition").unwrap().is_some());
}
