//! Tests of the spawn's component update (lane IMPL-SPAWN-COMPONENT-ATTACH, NATIVE_SPAWN_COMPONENT_ATTACH.md and the
//! hub's final rules on #605 r1) on invented classes with the game's class and field names, defined straight into the
//! class table (AGENTS.md rule 9). Every value is invented. The spawn-level cases are in `crate::world::tests`.
//! AI-assisted (Claude Code).

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

use super::*;
use crate::actors::instancing::Plan as Instancing;

struct Fx {
    vm: Vm,
    actor: ClassId,
    ai: ClassId,
    willow_ai: ClassId,
    other_ai: ClassId,
    primitive: ClassId,
    bare: ClassId,
}

/// `Actor` with `Components` and `AllComponents`; `ActorComponent` with the six fields; the two AI component classes, an
/// invented third `AIComponent` subclass and a primitive component under it; and a component class that declares none
/// of the fields. The scene layer is added unless `layer` is false.
fn fx(layer: bool) -> Fx {
    let mut s = ObjectSpace::new();
    let object = s.classes.define("Core.Object", None, vec![]).unwrap();
    let obj = PropKind::Object { class: object };
    let d = |s: &mut ObjectSpace, n: &str, k: PropKind| PropertyDecl::new(s.names.intern(n), k);
    let arrays = vec![
        d(&mut s, "Components", PropKind::Array(Box::new(obj.clone()))),
        d(&mut s, "AllComponents", PropKind::Array(Box::new(obj.clone()))),
        d(&mut s, "AIComponent", obj.clone()),
    ];
    let actor = s.classes.define("Engine.Actor", Some(object), arrays).unwrap();
    let mut fields = vec![d(&mut s, "Owner", obj.clone()), d(&mut s, "Scene", obj.clone())];
    for b in ["bAttached", "bNeedsReattach", "bNeedsUpdateTransform", "bSkipChildComponentUpdate"] {
        fields.push(d(&mut s, b, PropKind::Bool));
    }
    let component = s.classes.define("Engine.ActorComponent", Some(object), fields).unwrap();
    let ai = s.classes.define("GearboxFramework.AIComponent", Some(component), vec![]).unwrap();
    let willow_ai = s.classes.define("WillowGame.WillowAIComponent", Some(ai), vec![]).unwrap();
    let other_ai = s.classes.define("Test.ToyAIComponent", Some(ai), vec![]).unwrap();
    let primitive = s.classes.define("Engine.PrimitiveComponent", Some(component), vec![]).unwrap();
    let bare = s.classes.define("Test.BareComponent", Some(object), vec![]).unwrap();
    let mut vm = Vm::new(s, DefStore::new_empty(), Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
    if layer {
        assert!(add_empty_scene(&mut vm).unwrap());
        assert!(!add_empty_scene(&mut vm).unwrap(), "a second add keeps the one layer");
    }
    Fx { vm, actor, ai, willow_ai, other_ai, primitive, bare }
}

impl Fx {
    fn make(&mut self, class: ClassId) -> ObjRef {
        self.vm.space.alloc(class, "Toy", None, Origin::Runtime).unwrap()
    }

    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let c = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(c, name).unwrap();
        self.vm.space.write(o, PropAddr::scalar(p), v).unwrap();
    }

    fn get(&self, o: ObjRef, name: &str) -> Value {
        let c = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(c, name).unwrap();
        self.vm.space.read(o, PropAddr::scalar(p)).unwrap().clone()
    }

    fn components(&mut self, o: ObjRef, list: Vec<Option<ObjRef>>) {
        self.set(o, "Components", Value::array(list.into_iter().map(Value::Object).collect()));
    }

    /// A template component of `class` with both update flags set, as a template may hold them.
    fn component(&mut self, class: ClassId) -> ObjRef {
        let c = self.make(class);
        self.set(c, "bNeedsReattach", Value::Bool(true));
        self.set(c, "bNeedsUpdateTransform", Value::Bool(true));
        c
    }

    /// The instancing plan of `source` whose template components are `originals`.
    fn instancing(&self, source: ObjRef, originals: Vec<ObjRef>) -> Instancing {
        Instancing { template: source, originals, inherited: Vec::new() }
    }

    /// The spawn's own half (`apply_plan`, by hand): a new actor with the source's slots and a copy of each original,
    /// re-pointed in the actor's `Components` and `AIComponent`. Answers the actor and the (original, copy) pairs.
    fn make_copies(&mut self, source: ObjRef, originals: &[ObjRef]) -> (ObjRef, Vec<(ObjRef, ObjRef)>) {
        let actor = self.make(self.actor);
        let slots = self.vm.space.heap.get(source).unwrap().slots.clone();
        self.vm.space.heap.get_mut(actor).unwrap().slots = slots;
        let mut pairs = Vec::new();
        for &o in originals {
            let class = self.vm.space.heap.get(o).unwrap().class;
            let copy = self.vm.space.alloc(class, "ToyCopy", Some(actor), Origin::Runtime).unwrap();
            let slots = self.vm.space.heap.get(o).unwrap().slots.clone();
            self.vm.space.heap.get_mut(copy).unwrap().slots = slots;
            pairs.push((o, copy));
        }
        let repoint = |v: Value| match v {
            Value::Object(Some(r)) => Value::Object(Some(pairs.iter().find(|(o, _)| *o == r).map_or(r, |p| p.1))),
            other => other,
        };
        let list = self.get(actor, "Components").elements().unwrap().iter().cloned().map(repoint).collect();
        self.set(actor, "Components", Value::array(list));
        let ai = repoint(self.get(actor, "AIComponent"));
        self.set(actor, "AIComponent", ai);
        (actor, pairs)
    }

    /// #518's three counters.
    fn counters(&self) -> (u64, u64, u64) {
        (self.vm.space.heap.committed_writes(), self.vm.defs.load_transitions(), self.vm.rng.draws())
    }

    /// The plan for `source` must be the `NotYet` of `missing` with `what`, and leave the counters as they were.
    fn refuses(&self, source: ObjRef, instancing: &Instancing, missing: Missing, what: &str) {
        let before = self.counters();
        match plan(&self.vm, source, instancing) {
            Err(VmError::NotYet(n)) => {
                assert_eq!(n.missing, missing, "{n}");
                assert!(n.what.contains(what), "{n}");
                assert_eq!(n.record, RECORD);
            }
            other => panic!("expected the NotYet {what:?}, got {other:?}"),
        }
        assert_eq!(self.counters(), before, "a refusal writes, loads and draws nothing");
    }
}

/// The generic attach on every entry and the AI attach entry on the two exact AI classes, written to the copies in
/// `Components` order: `Owner` the actor, both flags and the child-update bit false, `Scene` at its zero value; the
/// two AI copies `bAttached` and in `AllComponents` once each, in order; the primitive copy's `bAttached` unchanged. A
/// None entry is skipped, and the templates keep their own values.
#[test]
fn every_copy_gets_its_owner_and_the_ai_copies_attach() {
    let mut x = fx(true);
    let source = x.make(x.actor);
    let (ai, prim, willow) = (x.component(x.ai), x.component(x.primitive), x.component(x.willow_ai));
    x.components(source, vec![Some(willow), None, Some(prim), Some(ai)]);
    let inst = x.instancing(source, vec![willow, prim, ai]);
    let p = plan(&x.vm, source, &inst).unwrap();
    let (actor, pairs) = x.make_copies(source, &[willow, prim, ai]);
    let before = x.counters();
    apply(&mut x.vm, actor, &p, &pairs).unwrap();
    let copy = |o: ObjRef| pairs.iter().find(|(a, _)| *a == o).unwrap().1;
    for o in [ai, prim, willow] {
        let c = copy(o);
        assert_eq!(x.get(c, "Owner"), Value::Object(Some(actor)));
        for f in ["bNeedsReattach", "bNeedsUpdateTransform", "bSkipChildComponentUpdate"] {
            assert_eq!(x.get(c, f), Value::Bool(false), "{f}");
        }
        assert_eq!(x.get(c, "Scene"), Value::Object(None), "the empty layer leaves Scene at its zero value");
        assert_eq!(x.get(o, "Owner"), Value::Object(None), "the template keeps its own");
        assert_eq!(x.get(o, "bNeedsReattach"), Value::Bool(true), "the template keeps its own");
    }
    assert_eq!(x.get(copy(willow), "bAttached"), Value::Bool(true));
    assert_eq!(x.get(copy(ai), "bAttached"), Value::Bool(true));
    assert_eq!(x.get(copy(prim), "bAttached"), Value::Bool(false), "registration is not modelled");
    assert_eq!(
        x.get(actor, "AllComponents"),
        Value::array(vec![Value::Object(Some(copy(willow))), Value::Object(Some(copy(ai)))]),
        "the AI copies, in Components order, once each"
    );
    assert_eq!(x.get(source, "AllComponents"), Value::array(vec![]), "the source keeps its own");
    let after = x.counters();
    assert!(after.0 > before.0, "the step writes");
    assert_eq!((after.1, after.2), (before.1, before.2), "nothing is loaded or drawn");
}

/// The mind's `AIComponent` slot and its `Components` entry name the same template component: after the spawn both
/// name the same copy, the one that got the actor as `Owner`.
#[test]
fn the_ai_component_slot_holds_the_same_attached_copy() {
    let mut x = fx(true);
    let source = x.make(x.actor);
    let willow = x.component(x.willow_ai);
    x.components(source, vec![Some(willow)]);
    x.set(source, "AIComponent", Value::Object(Some(willow)));
    let p = plan(&x.vm, source, &x.instancing(source, vec![willow])).unwrap();
    let (actor, pairs) = x.make_copies(source, &[willow]);
    apply(&mut x.vm, actor, &p, &pairs).unwrap();
    let Value::Object(Some(slot)) = x.get(actor, "AIComponent") else { panic!("no AIComponent") };
    assert_eq!(x.get(actor, "Components"), Value::array(vec![Value::Object(Some(slot))]));
    assert_ne!(slot, willow);
    assert_eq!(x.get(slot, "Owner"), Value::Object(Some(actor)));
    assert_eq!(x.get(slot, "bAttached"), Value::Bool(true));
}

/// No components, or only None entries: nothing to attach, so the layer is not asked for (a bare VM passes), and
/// nothing is written.
#[test]
fn no_components_asks_for_no_layer() {
    let mut x = fx(false);
    let source = x.make(x.actor);
    for list in [vec![], vec![None, None]] {
        x.components(source, list);
        let p = plan(&x.vm, source, &x.instancing(source, vec![])).unwrap();
        let (actor, pairs) = x.make_copies(source, &[]);
        let before = x.counters();
        apply(&mut x.vm, actor, &p, &pairs).unwrap();
        assert_eq!(x.counters(), before);
    }
}

/// The refusals, each with no write, load or draw (and so, at the spawn, before any actor is made):
/// - an entry that is not one of this spawn's copies (not in the instancing plan);
/// - no scene layer, with a component to attach (LayerAbsent);
/// - a non-empty `AllComponents`;
/// - an attached component;
/// - a stand-in component with no slots (not loaded);
/// - an `AIComponent` subclass other than the two read.
#[test]
fn the_refusals_write_nothing() {
    let mut x = fx(true);
    let source = x.make(x.actor);
    let stray = x.component(x.ai);
    x.components(source, vec![Some(stray)]);
    x.refuses(source, &x.instancing(source, vec![]), Missing::NotRead, NOT_A_COPY);

    let mut x = fx(false);
    let source = x.make(x.actor);
    let ai = x.component(x.ai);
    x.components(source, vec![Some(ai)]);
    x.refuses(source, &x.instancing(source, vec![ai]), Missing::LayerAbsent(SCENE_LAYER), NO_LAYER);

    let mut x = fx(true);
    let source = x.make(x.actor);
    let (ai, other) = (x.component(x.ai), x.component(x.ai));
    x.set(source, "AllComponents", Value::array(vec![Value::Object(Some(other))]));
    x.components(source, vec![Some(ai)]);
    x.refuses(source, &x.instancing(source, vec![ai]), Missing::NotRead, PRE_PASS);

    let mut x = fx(true);
    let source = x.make(x.actor);
    let ai = x.component(x.ai);
    x.set(ai, "bAttached", Value::Bool(true));
    x.components(source, vec![Some(ai)]);
    x.refuses(source, &x.instancing(source, vec![ai]), Missing::NotRead, ATTACHED);

    let mut x = fx(true);
    let source = x.make(x.actor);
    let stand_in = x.vm.space.alloc_stand_in(x.ai, "ToyStandIn", Origin::Resource { package: 0, index: -1 }).unwrap();
    x.components(source, vec![Some(stand_in)]);
    x.refuses(source, &x.instancing(source, vec![stand_in]), Missing::NotRead, "is not loaded");

    let mut x = fx(true);
    let source = x.make(x.actor);
    let toy = x.component(x.other_ai);
    x.components(source, vec![Some(toy)]);
    x.refuses(source, &x.instancing(source, vec![toy]), Missing::NotRead, OTHER_AI_CLASS);
}

/// A component listed twice, directly or as an inherited reference next to the own sub-object it maps to, refuses
/// with `NotYet { NotRead }` (the hub's ruling on #610 r1), with nothing written.
#[test]
fn a_component_listed_twice_is_refused() {
    let mut x = fx(true);
    let source = x.make(x.actor);
    let ai = x.component(x.ai);
    x.components(source, vec![Some(ai), None, Some(ai)]);
    x.refuses(source, &x.instancing(source, vec![ai]), Missing::NotRead, LISTED_TWICE);

    let mut x = fx(true);
    let source = x.make(x.actor);
    let (parents, own) = (x.component(x.ai), x.component(x.ai));
    x.components(source, vec![Some(parents), Some(own)]);
    let inst = Instancing { template: source, originals: vec![own], inherited: vec![(parents, own)] };
    x.refuses(source, &inst, Missing::NotRead, LISTED_TWICE);
}

/// An inherited reference (a parent class default's component) maps to the template's own sub-object, whose copy is
/// written.
#[test]
fn an_inherited_reference_maps_to_its_stand_in() {
    let mut x = fx(true);
    let source = x.make(x.actor);
    let (parents, own) = (x.component(x.ai), x.component(x.ai));
    x.components(source, vec![Some(parents)]);
    let inst = Instancing { template: source, originals: vec![own], inherited: vec![(parents, own)] };
    let p = plan(&x.vm, source, &inst).unwrap();
    let (actor, pairs) = x.make_copies(source, &[own]);
    apply(&mut x.vm, actor, &p, &pairs).unwrap();
    assert_eq!(x.get(pairs[0].1, "Owner"), Value::Object(Some(actor)));
}

/// The refusals come in the note's order: an attached component is refused before the missing layer (step 3 before
/// step 4), and the plan is decided whole before any write.
#[test]
fn an_attached_component_is_refused_before_the_missing_layer() {
    let mut x = fx(false);
    let source = x.make(x.actor);
    let (first, attached) = (x.component(x.ai), x.component(x.ai));
    x.set(attached, "bAttached", Value::Bool(true));
    x.components(source, vec![Some(first), Some(attached)]);
    x.refuses(source, &x.instancing(source, vec![first, attached]), Missing::NotRead, ATTACHED);
    assert_eq!(x.get(first, "Owner"), Value::Object(None));
}

/// Malformed classes are `VmError::Script` naming the field: a source class without `Components`, and a component
/// class without `Owner`. Nothing is written.
#[test]
fn undeclared_fields_are_script_errors() {
    let mut x = fx(true);
    let bare_actor_class = x.vm.space.classes.define("Test.BareActor", None, vec![]).unwrap();
    let bare_actor = x.make(bare_actor_class);
    let before = x.counters();
    let e = plan(&x.vm, bare_actor, &x.instancing(bare_actor, vec![])).unwrap_err();
    assert!(matches!(&e, VmError::Script { message, .. } if message.contains("declares no Components")), "{e:?}");
    assert_eq!(x.counters(), before);
    let source = x.make(x.actor);
    let bare = x.make(x.bare);
    x.components(source, vec![Some(bare)]);
    let before = x.counters();
    let e = plan(&x.vm, source, &x.instancing(source, vec![bare])).unwrap_err();
    assert!(matches!(&e, VmError::Script { message, .. } if message.contains("declares no Owner")), "{e:?}");
    assert_eq!(x.counters(), before);
}
