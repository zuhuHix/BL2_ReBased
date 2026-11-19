//! Tests of the per-actor component copy (decision 2026-11-19-44) on invented classes and objects (AGENTS.md rule 9).

use std::rc::Rc;

use ow_attr::AttributeSystem;
use ow_core::ParityMode;
use ow_defs::DefStore;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_vm::api::Vm;

use super::instance_components;

/// A VM with `Core.Object`, `Core.Component`, `Test.ToyComponent` (a `Peer` component field and a `Nested` plain
/// object field), and `Test.ToyActor` (`Comp`, `Other`, a `Components` array, all of component kind, and `Plain`, of
/// object kind). `with_component` false leaves `Core.Component` out (the toy component then extends `Core.Object`).
fn vm(with_component: bool) -> Vm {
    let mut space = ObjectSpace::new();
    let object = space.classes.define("Core.Object", None, vec![]).unwrap();
    let base = if with_component {
        space.classes.define("Core.Component", Some(object), vec![]).unwrap()
    } else {
        object
    };
    let n = |space: &mut ObjectSpace, s: &str| space.names.intern(s);
    let peer = PropertyDecl::new(n(&mut space, "Peer"), PropKind::Object { class: base });
    let nested = PropertyDecl::new(n(&mut space, "Nested"), PropKind::Object { class: object });
    let toy = space.classes.define("Test.ToyComponent", Some(base), vec![peer, nested]).unwrap();
    let decls = vec![
        PropertyDecl::new(n(&mut space, "Comp"), PropKind::Object { class: toy }),
        PropertyDecl::new(n(&mut space, "Other"), PropKind::Object { class: toy }),
        PropertyDecl::new(n(&mut space, "Components"), PropKind::Array(Box::new(PropKind::Object { class: base }))),
        PropertyDecl::new(n(&mut space, "Plain"), PropKind::Object { class: object }),
    ];
    space.classes.define("Test.ToyActor", Some(object), decls).unwrap();
    Vm::new(space, DefStore::new_empty(), Rc::new(AttributeSystem::new_empty()), ParityMode::Off)
}

fn make(vm: &mut Vm, class: &str, name: &str, outer: Option<ObjRef>) -> ObjRef {
    let c = vm.find_class(class).unwrap();
    vm.space.alloc(c, name, outer, Origin::Runtime).unwrap()
}

fn set(vm: &mut Vm, o: ObjRef, name: &str, v: Value) {
    let class = vm.space.heap.get(o).unwrap().class;
    let p = vm.space.prop(class, name).unwrap();
    vm.space.write(o, PropAddr::scalar(p), v).unwrap();
}

fn get(vm: &Vm, o: ObjRef, name: &str) -> Value {
    let class = vm.space.heap.get(o).unwrap().class;
    let p = vm.space.prop(class, name).unwrap();
    vm.space.read(o, PropAddr::scalar(p)).unwrap().clone()
}

fn obj(o: ObjRef) -> Value {
    Value::Object(Some(o))
}

/// A template with two components of its own (one pointing at the other, one with a nested sub-object), a component
/// of another archetype and a non-component object of its own. The actor made from its slots.
struct Fx {
    vm: Vm,
    template: ObjRef,
    a: ObjRef,
    b: ObjRef,
    nested: ObjRef,
    foreign: ObjRef,
    plain: ObjRef,
    actor: ObjRef,
}

fn fx(with_component: bool) -> Fx {
    let mut vm = vm(with_component);
    let template = make(&mut vm, "Test.ToyActor", "Default__ToyActor", None);
    let a = make(&mut vm, "Test.ToyComponent", "CompA", Some(template));
    let b = make(&mut vm, "Test.ToyComponent", "CompB", Some(template));
    let nested = make(&mut vm, "Core.Object", "NestedThing", Some(a));
    let other_archetype = make(&mut vm, "Test.ToyActor", "Elsewhere", None);
    let foreign = make(&mut vm, "Test.ToyComponent", "CompF", Some(other_archetype));
    let plain = make(&mut vm, "Core.Object", "PlainThing", Some(template));
    set(&mut vm, a, "Peer", obj(b));
    set(&mut vm, a, "Nested", obj(nested));
    set(&mut vm, template, "Comp", obj(a));
    set(&mut vm, template, "Other", obj(foreign));
    set(&mut vm, template, "Components", Value::Array(Rc::new(vec![obj(a), obj(b), obj(foreign)])));
    set(&mut vm, template, "Plain", obj(plain));
    let c = vm.find_class("Test.ToyActor").unwrap();
    let actor = vm.instantiate(c, "").unwrap();
    let slots = vm.space.heap.get(template).unwrap().slots.clone();
    vm.space.heap.get_mut(actor).unwrap().slots = slots;
    Fx { vm, template, a, b, nested, foreign, plain, actor }
}

/// The template's own components are copied, outered to the actor, with the same class, name and values; every
/// reference to them (the named field, the `Components` array, a copy's own field) is re-pointed; another archetype's
/// component, a non-component object and a nested sub-object stay shared; the template is not changed.
#[test]
fn the_templates_own_components_are_copied_and_re_pointed() {
    let mut x = fx(true);
    let before = x.vm.space.heap.get(x.template).unwrap().slots.clone();
    let pairs = instance_components(&mut x.vm, x.template, x.actor).unwrap();
    assert_eq!(pairs.iter().map(|p| p.0).collect::<Vec<_>>(), vec![x.a, x.b], "slot order, each once");
    let (ca, cb) = (pairs[0].1, pairs[1].1);
    for (original, copy) in &pairs {
        let (o, c) = (x.vm.space.heap.get(*original).unwrap(), x.vm.space.heap.get(*copy).unwrap());
        assert_ne!(original, copy);
        assert_eq!(c.outer, Some(x.actor));
        assert_eq!((c.class, c.name), (o.class, o.name));
    }
    assert_eq!(get(&x.vm, x.actor, "Comp"), obj(ca));
    assert_eq!(get(&x.vm, x.actor, "Components"), Value::Array(Rc::new(vec![obj(ca), obj(cb), obj(x.foreign)])));
    assert_eq!(get(&x.vm, x.actor, "Other"), obj(x.foreign), "another archetype's component stays shared");
    assert_eq!(get(&x.vm, x.actor, "Plain"), obj(x.plain), "a non-component sub-object stays shared");
    assert_eq!(get(&x.vm, ca, "Peer"), obj(cb), "a copy's reference to a sibling is re-pointed");
    assert_eq!(get(&x.vm, ca, "Nested"), obj(x.nested), "a nested sub-object stays shared (UNVERIFIED)");
    assert_eq!(x.vm.space.heap.get(x.template).unwrap().slots, before, "the template is not changed");
    assert_eq!(get(&x.vm, x.a, "Peer"), obj(x.b), "nor its components");
}

/// Without `Core.Component` nothing counts as a component: nothing is copied, the actor keeps the shared references.
#[test]
fn without_the_component_class_nothing_is_copied() {
    let mut x = fx(false);
    let before = x.vm.space.heap.get(x.actor).unwrap().slots.clone();
    assert!(instance_components(&mut x.vm, x.template, x.actor).unwrap().is_empty());
    assert_eq!(x.vm.space.heap.get(x.actor).unwrap().slots, before);
}

/// A second actor from the same template gets copies of its own, distinct from the first actor's.
#[test]
fn each_actor_gets_its_own_copies() {
    let mut x = fx(true);
    let first = instance_components(&mut x.vm, x.template, x.actor).unwrap();
    let class = x.vm.space.heap.get(x.actor).unwrap().class;
    let second_actor = x.vm.instantiate(class, "").unwrap();
    let slots = x.vm.space.heap.get(x.template).unwrap().slots.clone();
    x.vm.space.heap.get_mut(second_actor).unwrap().slots = slots;
    let second = instance_components(&mut x.vm, x.template, second_actor).unwrap();
    assert_eq!(second.len(), 2);
    for (f, s) in first.iter().zip(&second) {
        assert_eq!(f.0, s.0);
        assert_ne!(f.1, s.1);
        assert_eq!(x.vm.space.heap.get(s.1).unwrap().outer, Some(second_actor));
    }
    assert_eq!(get(&x.vm, x.actor, "Comp"), obj(first[0].1), "the first actor keeps its own");
}

/// The template is matched by path: a component outered to another heap object with the template's name and class
/// (both with no outer, as the store's `Default__` export handle and the class default object are) is the template's
/// own; one outered to an object of the same name but another class is not.
#[test]
fn the_template_is_matched_by_path() {
    let mut x = fx(true);
    let twin = make(&mut x.vm, "Test.ToyActor", "Default__ToyActor", None);
    let impostor = make(&mut x.vm, "Core.Object", "Default__ToyActor", None);
    let by_path = make(&mut x.vm, "Test.ToyComponent", "CompT", Some(twin));
    let not_it = make(&mut x.vm, "Test.ToyComponent", "CompI", Some(impostor));
    set(&mut x.vm, x.actor, "Comp", obj(by_path));
    set(&mut x.vm, x.actor, "Other", obj(not_it));
    let pairs = instance_components(&mut x.vm, x.template, x.actor).unwrap();
    assert_eq!(pairs.iter().map(|p| p.0).collect::<Vec<_>>(), vec![by_path, x.a, x.b]);
    assert_eq!(get(&x.vm, x.actor, "Comp"), obj(pairs[0].1));
    assert_eq!(get(&x.vm, x.actor, "Other"), obj(not_it));
}

/// A component reached only through another component (the template's `CompB.Peer`) is copied too, and the copy of
/// the one that holds it points at the copy (review r1 of #435): a write through it leaves the template as it was.
#[test]
fn a_component_reached_through_another_is_copied() {
    let mut x = fx(true);
    let deep = make(&mut x.vm, "Test.ToyComponent", "CompDeep", Some(x.template));
    set(&mut x.vm, x.b, "Peer", obj(deep));
    let pairs = instance_components(&mut x.vm, x.template, x.actor).unwrap();
    assert_eq!(pairs.iter().map(|p| p.0).collect::<Vec<_>>(), vec![x.a, x.b, deep]);
    let (cb, cdeep) = (pairs[1].1, pairs[2].1);
    assert_eq!(get(&x.vm, cb, "Peer"), obj(cdeep));
    assert_eq!(x.vm.space.heap.get(cdeep).unwrap().outer, Some(x.actor));
    set(&mut x.vm, cdeep, "Peer", obj(x.a));
    assert_eq!(get(&x.vm, deep, "Peer"), Value::Object(None), "the template's component is not written");
}
