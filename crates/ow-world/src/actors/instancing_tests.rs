//! Tests of the per-actor component copy (decision 2026-11-19-44) on invented classes and objects (AGENTS.md rule 9).

use std::rc::Rc;

use ow_attr::AttributeSystem;
use ow_core::{Missing, ParityMode};
use ow_defs::DefStore;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::{DelegateValue, StructValue, Value};
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use super::{instance_components, plan_components};

/// A VM with `Core.Object`, `Core.Component`, `Test.ToyComponent` (a `Peer` component field and a `Thing` field of
/// plain object type), the struct `Test.ToyBox` (`Held`, an object, and `Cb`, a delegate) and `Test.ToyActor` (`Comp`,
/// `Other` and a `Components` array of component type, `Plain` of object type, `Cb` a delegate and `Box` a
/// `ToyBox`). `with_component` false leaves `Core.Component` out (the toy component then extends `Core.Object`).
fn vm(with_component: bool) -> Vm {
    let mut space = ObjectSpace::new();
    let object = space.classes.define("Core.Object", None, vec![]).unwrap();
    let base = if with_component {
        space.classes.define("Core.Component", Some(object), vec![]).unwrap()
    } else {
        object
    };
    let peer = PropertyDecl::new(space.names.intern("Peer"), PropKind::Object { class: base });
    let thing = PropertyDecl::new(space.names.intern("Thing"), PropKind::Object { class: object });
    let toy = space.classes.define("Test.ToyComponent", Some(base), vec![peer, thing]).unwrap();
    let held = PropertyDecl::new(space.names.intern("Held"), PropKind::Object { class: object });
    let cb = PropertyDecl::new(space.names.intern("Cb"), PropKind::Delegate);
    let toy_box = space.classes.define_struct("Test.ToyBox", None, vec![held, cb]).unwrap();
    let decls = vec![
        PropertyDecl::new(space.names.intern("Comp"), PropKind::Object { class: toy }),
        PropertyDecl::new(space.names.intern("Other"), PropKind::Object { class: toy }),
        PropertyDecl::new(space.names.intern("Components"), PropKind::Array(Box::new(PropKind::Object { class: base }))),
        PropertyDecl::new(space.names.intern("Plain"), PropKind::Object { class: object }),
        PropertyDecl::new(space.names.intern("Cb"), PropKind::Delegate),
        PropertyDecl::new(space.names.intern("Box"), PropKind::Struct(toy_box)),
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

fn delegate(vm: &mut Vm, o: ObjRef) -> Value {
    let function = vm.space.names.intern("OnToyEvent");
    Value::Delegate(Some(DelegateValue { object: Some(o), function }))
}

fn with_box_field(vm: &Vm, o: ObjRef, at: usize, v: Value) -> Value {
    let s = get(vm, o, "Box");
    let mut fields = StructValue::clone(s.as_struct().unwrap());
    fields.fields[at] = v;
    Value::Struct(Rc::new(fields))
}

/// A template with two components of its own (`CompA.Peer` is `CompB`) and a component of another archetype.
struct Fx {
    vm: Vm,
    template: ObjRef,
    a: ObjRef,
    b: ObjRef,
    foreign: ObjRef,
}

fn fx(with_component: bool) -> Fx {
    let mut vm = vm(with_component);
    let template = make(&mut vm, "Test.ToyActor", "ToyArchetype", None);
    let a = make(&mut vm, "Test.ToyComponent", "CompA", Some(template));
    let b = make(&mut vm, "Test.ToyComponent", "CompB", Some(template));
    let other_archetype = make(&mut vm, "Test.ToyActor", "Elsewhere", None);
    let foreign = make(&mut vm, "Test.ToyComponent", "CompF", Some(other_archetype));
    set(&mut vm, a, "Peer", obj(b));
    set(&mut vm, template, "Comp", obj(a));
    set(&mut vm, template, "Other", obj(foreign));
    set(&mut vm, template, "Components", Value::Array(Rc::new(vec![obj(a), obj(b), obj(foreign)])));
    Fx { vm, template, a, b, foreign }
}

impl Fx {
    /// A new actor of the template's class with the template's slots, as the spawn makes it.
    fn actor(&mut self) -> ObjRef {
        let c = self.vm.space.heap.get(self.template).unwrap().class;
        let actor = self.vm.instantiate(c, "").unwrap();
        let slots = self.vm.space.heap.get(self.template).unwrap().slots.clone();
        self.vm.space.heap.get_mut(actor).unwrap().slots = slots;
        actor
    }
}

fn refused(e: VmError, what: &str) {
    match e {
        VmError::NotYet(n) => {
            assert_eq!(n.missing, Missing::NotRead);
            assert!(n.what.contains(what), "{n}");
        }
        e => panic!("expected NotYet naming {what:?}, got {e}"),
    }
}

/// The template's own components are copied, outered to the actor, with the same class, name and values; every
/// reference to them (the named field, the `Components` array, a copy's own field) is re-pointed; another archetype's
/// component stays shared; the template is not changed.
#[test]
fn the_templates_own_components_are_copied_and_re_pointed() {
    let mut x = fx(true);
    let before = x.vm.space.heap.get(x.template).unwrap().slots.clone();
    let actor = x.actor();
    let pairs = instance_components(&mut x.vm, x.template, actor).unwrap();
    assert_eq!(pairs.iter().map(|p| p.0).collect::<Vec<_>>(), vec![x.a, x.b], "discovery order, each once");
    let (ca, cb) = (pairs[0].1, pairs[1].1);
    for (original, copy) in &pairs {
        let (o, c) = (x.vm.space.heap.get(*original).unwrap(), x.vm.space.heap.get(*copy).unwrap());
        assert_ne!(original, copy);
        assert_eq!(c.outer, Some(actor));
        assert_eq!((c.class, c.name), (o.class, o.name));
    }
    assert_eq!(get(&x.vm, actor, "Comp"), obj(ca));
    assert_eq!(get(&x.vm, actor, "Components"), Value::Array(Rc::new(vec![obj(ca), obj(cb), obj(x.foreign)])));
    assert_eq!(get(&x.vm, actor, "Other"), obj(x.foreign), "another archetype's component stays shared");
    assert_eq!(get(&x.vm, ca, "Peer"), obj(cb), "a copy's reference to a sibling is re-pointed");
    assert_eq!(x.vm.space.heap.get(x.template).unwrap().slots, before, "the template is not changed");
    assert_eq!(get(&x.vm, x.a, "Peer"), obj(x.b), "nor its components");
}

/// Without `Core.Component` in the class table nothing is a component: the template's sub-objects then count as
/// non-component sub-objects, which Off refuses rather than share (review r2 of #435, item 7: no lookup error turns
/// into a spawn with shared references).
#[test]
fn without_the_component_class_the_sub_objects_are_refused() {
    let mut x = fx(false);
    refused(plan_components(&mut x.vm, x.template).unwrap_err(), "not a component");
}

/// A second actor from the same template gets copies of its own, distinct from the first actor's.
#[test]
fn each_actor_gets_its_own_copies() {
    let mut x = fx(true);
    let first_actor = x.actor();
    let first = instance_components(&mut x.vm, x.template, first_actor).unwrap();
    let second_actor = x.actor();
    let second = instance_components(&mut x.vm, x.template, second_actor).unwrap();
    assert_eq!(second.len(), 2);
    for (f, s) in first.iter().zip(&second) {
        assert_eq!(f.0, s.0);
        assert_ne!(f.1, s.1);
        assert_eq!(x.vm.space.heap.get(s.1).unwrap().outer, Some(second_actor));
    }
    assert_eq!(get(&x.vm, first_actor, "Comp"), obj(first[0].1), "the first actor keeps its own");
}

/// A component reached only through another component (the template's `CompB.Peer`) is copied too, and the copy of
/// the one that holds it points at the copy (review r1 of #435): a write through it leaves the template as it was.
#[test]
fn a_component_reached_through_another_is_copied() {
    let mut x = fx(true);
    let deep = make(&mut x.vm, "Test.ToyComponent", "CompDeep", Some(x.template));
    set(&mut x.vm, x.b, "Peer", obj(deep));
    let actor = x.actor();
    let pairs = instance_components(&mut x.vm, x.template, actor).unwrap();
    assert_eq!(pairs.iter().map(|p| p.0).collect::<Vec<_>>(), vec![x.a, x.b, deep]);
    let (cb, cdeep) = (pairs[1].1, pairs[2].1);
    assert_eq!(get(&x.vm, cb, "Peer"), obj(cdeep));
    assert_eq!(x.vm.space.heap.get(cdeep).unwrap().outer, Some(actor));
    set(&mut x.vm, cdeep, "Peer", obj(x.a));
    assert_eq!(get(&x.vm, deep, "Peer"), Value::Object(None), "the template's component is not written");
}

/// Discovery goes by the object's run-time class and owner, not the field's declared type (review r2 of #435, item
/// 2): a template component held only through an `Object`-typed field, or inside a struct, is copied and re-pointed.
#[test]
fn a_component_held_through_an_object_field_or_a_struct_is_copied() {
    let mut x = fx(true);
    let via_object = make(&mut x.vm, "Test.ToyComponent", "CompViaObject", Some(x.template));
    let in_box = make(&mut x.vm, "Test.ToyComponent", "CompInBox", Some(x.template));
    set(&mut x.vm, x.a, "Thing", obj(via_object));
    let boxed = with_box_field(&x.vm, x.template, 0, obj(in_box));
    set(&mut x.vm, x.template, "Box", boxed);
    let actor = x.actor();
    let pairs = instance_components(&mut x.vm, x.template, actor).unwrap();
    let copy_of = |o: ObjRef| pairs.iter().find(|p| p.0 == o).map(|p| p.1).unwrap();
    assert_eq!(get(&x.vm, copy_of(x.a), "Thing"), obj(copy_of(via_object)));
    assert_eq!(get(&x.vm, actor, "Box").as_struct().unwrap().fields[0], obj(copy_of(in_box)));
}

/// Delegates bound to a copied component are rebound to the copy, also inside a struct (review r2 of #435, item 1).
#[test]
fn delegates_bound_to_a_copied_component_are_rebound() {
    let mut x = fx(true);
    let bound = delegate(&mut x.vm, x.a);
    set(&mut x.vm, x.template, "Cb", bound.clone());
    let in_box = delegate(&mut x.vm, x.b);
    let boxed = with_box_field(&x.vm, x.template, 1, in_box);
    set(&mut x.vm, x.template, "Box", boxed);
    let actor = x.actor();
    let pairs = instance_components(&mut x.vm, x.template, actor).unwrap();
    let (ca, cb) = (pairs[0].1, pairs[1].1);
    let target = |v: &Value| match v {
        Value::Delegate(Some(d)) => d.object,
        other => panic!("{other:?}"),
    };
    assert_eq!(target(&get(&x.vm, actor, "Cb")), Some(ca));
    assert_eq!(target(&get(&x.vm, actor, "Box").as_struct().unwrap().fields[1]), Some(cb));
    assert_eq!(get(&x.vm, x.template, "Cb"), bound, "the template's delegate is not changed");
}

/// What Off cannot copy or re-point correctly is refused before anything is made (review r2 of #435): a reference
/// back to the template, a delegate bound to it, a non-component sub-object of the template, a sub-object of one of
/// its components, and a component owned by another class default object.
#[test]
fn what_off_cannot_copy_is_refused_before_anything_is_made() {
    type Setup = fn(&mut Fx);
    let cases: [(Setup, &str); 5] = [
        (|x| set(&mut x.vm, x.template, "Plain", obj(x.template)), "back to the template"),
        (
            |x| {
                let d = delegate(&mut x.vm, x.template);
                set(&mut x.vm, x.template, "Cb", d);
            },
            "back to the template",
        ),
        (
            |x| {
                let plain = make(&mut x.vm, "Core.Object", "PlainThing", Some(x.template));
                set(&mut x.vm, x.template, "Plain", obj(plain));
            },
            "not a component",
        ),
        (
            |x| {
                let nested = make(&mut x.vm, "Core.Object", "NestedThing", Some(x.a));
                set(&mut x.vm, x.a, "Thing", obj(nested));
            },
            "nested instancing",
        ),
        (
            |x| {
                let parent = make(&mut x.vm, "Test.ToyActor", "Default__ToyParent", None);
                let inherited = make(&mut x.vm, "Test.ToyComponent", "CompInherited", Some(parent));
                set(&mut x.vm, x.template, "Other", obj(inherited));
            },
            "another class default object",
        ),
    ];
    for (setup, what) in cases {
        let mut x = fx(true);
        setup(&mut x);
        let count = x.vm.space.heap.len();
        refused(plan_components(&mut x.vm, x.template).unwrap_err(), what);
        assert_eq!(x.vm.space.heap.len(), count, "{what}: nothing made");
    }
}

/// Ownership is by handle, never by name (review r2 of #435, item 3): a component owned by another object of the
/// same name and class (another package's archetype of the same path) is not the template's, and stays shared.
#[test]
fn ownership_is_by_handle_not_by_name() {
    let mut x = fx(true);
    let twin = make(&mut x.vm, "Test.ToyActor", "ToyArchetype", None);
    let twins = make(&mut x.vm, "Test.ToyComponent", "CompTwin", Some(twin));
    set(&mut x.vm, x.template, "Other", obj(twins));
    let actor = x.actor();
    let pairs = instance_components(&mut x.vm, x.template, actor).unwrap();
    assert!(pairs.iter().all(|p| p.0 != twins), "the twin's component is not the template's");
    assert_eq!(get(&x.vm, actor, "Other"), obj(twins));
}

/// Nested sub-objects are refused whatever order they are met in (review r3 of #435, item 1): a template slot that
/// names a component's sub-object before the component itself, and a deeper descendant, are refused on their whole
/// outer chain.
#[test]
fn a_nested_sub_object_met_first_or_deeper_is_refused() {
    for depth in [1, 2] {
        let mut x = fx(true);
        let mut nested = make(&mut x.vm, "Test.ToyComponent", "NestedComp", Some(x.a));
        if depth == 2 {
            nested = make(&mut x.vm, "Test.ToyComponent", "DeeperComp", Some(nested));
        }
        // `Comp` (the first slot) names the nested object; CompA is reached only later, through `Components`.
        set(&mut x.vm, x.template, "Comp", obj(nested));
        let count = x.vm.space.heap.len();
        refused(plan_components(&mut x.vm, x.template).unwrap_err(), "nested instancing");
        assert_eq!(x.vm.space.heap.len(), count, "depth {depth}: nothing made");
    }
}

/// A template component whose properties are not loaded (a stand-in with no slots) is refused, never copied as an
/// object with no slots (review r3 of #435, item 5).
#[test]
fn a_component_stand_in_is_refused() {
    let mut x = fx(true);
    let class = x.vm.find_class("Test.ToyComponent").unwrap();
    let stand_in = x.vm.space.alloc_stand_in(class, "CompStandIn", Origin::Resource { package: 0, index: 1 }).unwrap();
    x.vm.space.heap.get_mut(stand_in).unwrap().outer = Some(x.template);
    let components = Value::Array(Rc::new(vec![obj(x.a), obj(x.b), obj(stand_in)]));
    set(&mut x.vm, x.template, "Components", components);
    refused(plan_components(&mut x.vm, x.template).unwrap_err(), "not loaded");
}
