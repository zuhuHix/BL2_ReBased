//! `SetBaseValue` on an invented class set (AGENTS.md rule 9; this lane's decision 2026-11-19-NNN). Expected values are
//! written by hand from NATIVE_ATTRIBUTES.md section 1 (the evaluator and SetAttributeBaseValue) and section 3 (set
//! base on a stack attribute writes the base companion and recomputes the value from the stack).

use std::cell::Cell;
use std::rc::Rc;

use ow_attr::AttributeSystem;
use ow_attr::api::{AttrError, ContextResolver};
use ow_core::{ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ObjRef, PropAddr, StructId};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_vm::api::{FunctionDecl, Vm};
use ow_vm::error::VmError;

use super::*;

const PATH: &str = "Engine.AttributeInitializationDefinition.SetBaseValue";

/// A toy context resolver: the context is the source itself.
struct Same;
impl ContextResolver for Same {
    fn resolve(
        &self,
        _: &AttributeSystem,
        _: ObjRef,
        _: ObjRef,
        source: Option<ObjRef>,
        _: &mut EvalCx<'_>,
    ) -> Result<Option<ObjRef>, AttrError> {
        Ok(source)
    }
}

struct Toy {
    vm: Vm,
    aid: StructId,
}

fn decl(s: &mut ObjectSpace, name: &str, kind: PropKind, flags: u64) -> PropertyDecl {
    PropertyDecl::new(s.names.intern(name), kind).with_flags(flags)
}

impl Toy {
    /// `Test.Target` holds `Damage` (a float stack attribute with the notify flag, its base and stack) and a plain
    /// float `Level`. Attribute definitions read a property of their context through
    /// `ObjectPropertyAttributeValueResolver`, with `Test.Same` as the context chain.
    fn new(mode: ParityMode) -> Toy {
        let mut s = ObjectSpace::new();
        let object = s.classes.define("Core.Object", None, vec![]).unwrap();
        let obj = PropKind::Object { class: object };
        let objs = PropKind::Array(Box::new(obj.clone()));
        let fields = vec![
            decl(&mut s, "BaseValueConstant", PropKind::Float, 0),
            decl(&mut s, "BaseValueAttribute", obj.clone(), 0),
            decl(&mut s, "InitializationDefinition", obj.clone(), 0),
            decl(&mut s, "BaseValueScaleConstant", PropKind::Float, 0),
        ];
        let aid = s
            .classes
            .define_struct("Engine.AttributeInitializationDefinition.AttributeInitializationData", None, fields)
            .unwrap();
        let modifier_props =
            vec![decl(&mut s, "Type", PropKind::Byte { enum_: None }, 0), decl(&mut s, "Value", PropKind::Float, 0)];
        let modifier = s.classes.define("Core.AttributeModifier", Some(object), modifier_props).unwrap();
        let chains = vec![
            decl(&mut s, "ContextResolverChain", objs.clone(), 0),
            decl(&mut s, "ValueResolverChain", objs.clone(), 0),
        ];
        let base = s.classes.define("Engine.AttributeDefinitionBase", Some(object), chains).unwrap();
        let attribute = s.classes.define("Engine.AttributeDefinition", Some(base), vec![]).unwrap();
        s.classes.define("Engine.AttributeDefinitionMultiContext", Some(attribute), vec![]).unwrap();
        // The definition's structs, with the fields the evaluator reads (invented layout; review r1 of #572).
        let toggle = |s: &mut ObjectSpace, path: &str, flags: &[&str], datas: &[&str]| {
            let mut fields: Vec<PropertyDecl> = flags.iter().map(|f| decl(s, f, PropKind::Bool, 0)).collect();
            fields.extend(datas.iter().map(|f| decl(s, f, PropKind::Struct(aid), 0)));
            s.classes.define_struct(path, None, fields).unwrap()
        };
        let formula = toggle(&mut s, "Test.ValueFormula", &["bEnabled"], &[]);
        let conditional = toggle(&mut s, "Test.Conditional", &["bEnabled"], &[]);
        let variance =
            toggle(&mut s, "Test.Variance", &["bEnabled", "bUseIntegerRandomization"], &["LowerBound", "UpperBound"]);
        let range = toggle(&mut s, "Test.Range", &["bEnableMinValueRestriction", "bEnableMaxValueRestriction"], &[]);
        let definition_props = vec![
            decl(&mut s, "ValueFormula", PropKind::Struct(formula), 0),
            decl(&mut s, "ConditionalInitialization", PropKind::Struct(conditional), 0),
            decl(&mut s, "RandomVariance", PropKind::Struct(variance), 0),
            decl(&mut s, "RangeRestriction", PropKind::Struct(range), 0),
            decl(&mut s, "BaseValueMode", PropKind::Byte { enum_: None }, 0),
            decl(&mut s, "RoundingMode", PropKind::Byte { enum_: None }, 0),
        ];
        s.classes.define("Engine.AttributeInitializationDefinition", Some(object), definition_props).unwrap();
        let named = vec![decl(&mut s, "PropertyName", PropKind::Name, 0)];
        s.classes.define("Engine.ObjectPropertyAttributeValueResolver", Some(object), named).unwrap();
        s.classes.define("Test.Same", Some(object), vec![]).unwrap();
        let stack = PropKind::Array(Box::new(PropKind::Object { class: modifier }));
        let target_props = vec![
            decl(&mut s, "Damage", PropKind::FloatAttribute, PropertyDecl::STACK_ATTRIBUTE | PropertyDecl::NOTIFY),
            decl(&mut s, "DamageBaseValue", PropKind::FloatAttribute, PropertyDecl::PLAIN_ATTRIBUTE),
            decl(&mut s, "DamageModifierStack", stack, 0),
            decl(&mut s, "Level", PropKind::Float, PropertyDecl::PLAIN_ATTRIBUTE),
        ];
        let target = s.classes.define("Test.Target", Some(object), target_props).unwrap();
        let (v, b, k) = (s.names.intern("Damage"), s.names.intern("DamageBaseValue"), s.names.intern("DamageModifierStack"));
        s.classes.link_attribute(target, v, b, k).unwrap();
        let attrs = AttributeSystem::standard().unwrap().with_context_resolver("Test.Same", Same).unwrap();
        let mut vm = Vm::new(s, DefStore::new_empty(), Rc::new(attrs), mode);
        let mut f = FunctionDecl::native(ObjectPath::parse(PATH).unwrap(), SET_BASE_VALUE);
        f.is_static = true;
        let param = |vm: &mut Vm, n: &str, k: PropKind, flags: u64| {
            PropertyDecl::new(vm.space.names.intern(n), k).with_flags(PropertyDecl::PARAM | flags)
        };
        f.params = vec![
            param(&mut vm, "DestAttribute", obj.clone(), 0),
            param(&mut vm, "BaseValue", PropKind::Struct(aid), PropertyDecl::OUT),
            param(&mut vm, "ValueContextSource", obj.clone(), 0),
            param(&mut vm, "AttributeContextSource", obj.clone(), 0),
        ];
        f.result = Some(PropertyDecl::new(vm.space.names.intern("ReturnValue"), PropKind::Bool).with_flags(PropertyDecl::RETURN));
        vm.declare_function(f).unwrap();
        register(&mut vm).unwrap();
        Toy { vm, aid }
    }

    fn obj(&mut self, class: &str, name: &str) -> ObjRef {
        let c = self.vm.space.classes.by_path(class).unwrap();
        self.vm.space.alloc(c, name, None, Origin::Runtime).unwrap()
    }

    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap();
        self.vm.space.write(o, PropAddr::scalar(p), v).unwrap();
    }

    fn get(&self, o: ObjRef, name: &str) -> Value {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap();
        self.vm.space.read(o, PropAddr::scalar(p)).unwrap().clone()
    }

    /// An attribute definition reading the context's property `property`.
    fn attribute(&mut self, name: &str, property: &str) -> ObjRef {
        let a = self.obj("Engine.AttributeDefinition", name);
        let same = self.obj("Test.Same", &format!("{name}.Same"));
        let reader = self.obj("Engine.ObjectPropertyAttributeValueResolver", &format!("{name}.Reader"));
        let n = self.vm.space.names.intern(property);
        self.set(reader, "PropertyName", Value::Name(n));
        self.set(a, "ContextResolverChain", Value::Array(Rc::new(vec![Value::Object(Some(same))])));
        self.set(a, "ValueResolverChain", Value::Array(Rc::new(vec![Value::Object(Some(reader))])));
        a
    }

    fn data(&mut self, constant: f32, attribute: Option<ObjRef>, scale: f32) -> Value {
        let ty = self.aid;
        let mut v = self.vm.space.classes.struct_defaults(ty).unwrap();
        for (name, x) in [
            ("BaseValueConstant", Value::Float(constant)),
            ("BaseValueAttribute", Value::Object(attribute)),
            ("BaseValueScaleConstant", Value::Float(scale)),
        ] {
            let id = self.vm.space.names.find(name).unwrap();
            let at = self.vm.space.classes.struct_layout(ty).unwrap().field(id).unwrap().0;
            v.struct_mut().unwrap().fields[at as usize] = x;
        }
        v
    }

    /// An initialisation definition with only random variance enabled, between 0 and 10.
    fn variance_definition(&mut self) -> ObjRef {
        let d = self.obj("Engine.AttributeInitializationDefinition", "Test.Variance");
        let low = self.data(0.0, None, 1.0);
        let high = self.data(10.0, None, 1.0);
        let class = self.vm.space.heap.get(d).unwrap().class;
        let p = self.vm.space.prop(class, "RandomVariance").unwrap();
        let mut v = self.vm.space.read(d, PropAddr::scalar(p)).unwrap().clone();
        let ty = match &v {
            Value::Struct(st) => st.ty,
            other => panic!("{other:?}"),
        };
        for (name, x) in [("bEnabled", Value::Bool(true)), ("LowerBound", low), ("UpperBound", high)] {
            let id = self.vm.space.names.find(name).unwrap();
            let at = self.vm.space.classes.struct_layout(ty).unwrap().field(id).unwrap().0;
            v.struct_mut().unwrap().fields[at as usize] = x;
        }
        self.set(d, "RandomVariance", v);
        d
    }

    fn with_definition(&mut self, mut data: Value, definition: ObjRef) -> Value {
        let id = self.vm.space.names.find("InitializationDefinition").unwrap();
        let at = self.vm.space.classes.struct_layout(self.aid).unwrap().field(id).unwrap().0;
        data.struct_mut().unwrap().fields[at as usize] = Value::Object(Some(definition));
        data
    }

    fn counters(&self) -> (u64, u64, u64) {
        (self.vm.space.heap.committed_writes(), self.vm.rng.draws(), self.vm.defs.load_transitions())
    }

    fn call(&mut self, args: Vec<Value>) -> Result<Value, VmError> {
        let f = self.vm.find_function(PATH)?;
        Ok(self.vm.call(f, None, args)?.ret)
    }
}

/// The record: the data evaluated with the value context (constant 4 times scale 2 = 8), then set as the base of
/// the destination attribute on the attribute context. The value is recomputed from the empty stack, and the call
/// answers true.
#[test]
fn the_evaluated_value_becomes_the_base() {
    let mut t = Toy::new(ParityMode::Off);
    let damage = t.attribute("Test.DamageAttr", "Damage");
    let target = t.obj("Test.Target", "Test.Gun");
    t.set(target, "Damage", Value::Float(1.0));
    t.set(target, "DamageBaseValue", Value::Float(1.0));
    let data = t.data(4.0, None, 2.0);
    let r = t.call(vec![Value::Object(Some(damage)), data, Value::Object(None), Value::Object(Some(target))]).unwrap();
    assert_eq!(r, Value::Bool(true));
    assert_eq!(t.get(target, "DamageBaseValue"), Value::Float(8.0));
    assert_eq!(t.get(target, "Damage"), Value::Float(8.0));
}

/// The data is evaluated with `ValueContextSource`, not the attribute context: its `BaseValueAttribute` reads the
/// value source's `Level` (3, times scale 2 = 6) while the base is set on the attribute context.
#[test]
fn the_value_context_and_the_attribute_context_are_separate() {
    let mut t = Toy::new(ParityMode::Off);
    let damage = t.attribute("Test.DamageAttr", "Damage");
    let level = t.attribute("Test.LevelAttr", "Level");
    let source = t.obj("Test.Target", "Test.Source");
    t.set(source, "Level", Value::Float(3.0));
    let target = t.obj("Test.Target", "Test.Gun");
    t.set(target, "Level", Value::Float(50.0));
    let data = t.data(0.0, Some(level), 2.0);
    let r = t.call(vec![Value::Object(Some(damage)), data, Value::Object(Some(source)), Value::Object(Some(target))]);
    assert_eq!(r.unwrap(), Value::Bool(true));
    assert_eq!(t.get(target, "DamageBaseValue"), Value::Float(6.0));
    assert_eq!(t.get(source, "DamageBaseValue"), Value::Float(0.0), "the value context is not written");
}

/// The record: false when `DestAttribute` is None, and nothing is written.
#[test]
fn no_destination_answers_false() {
    let mut t = Toy::new(ParityMode::Off);
    let target = t.obj("Test.Target", "Test.Gun");
    let data = t.data(4.0, None, 2.0);
    let before = t.vm.space.heap.committed_writes();
    let r = t.call(vec![Value::Object(None), data, Value::Object(None), Value::Object(Some(target))]).unwrap();
    assert_eq!(r, Value::Bool(false));
    assert_eq!(t.vm.space.heap.committed_writes(), before);
    assert_eq!(t.get(target, "DamageBaseValue"), Value::Float(0.0));
}

/// The record's set-base calls the attribute change notification (the script default `bSuppressNotify` false;
/// UNVERIFIED): the property carries the notify flag, so the class's override runs once.
#[test]
fn the_change_notification_runs() {
    let mut t = Toy::new(ParityMode::Off);
    let calls = Rc::new(Cell::new(0));
    let seen = calls.clone();
    t.vm
        .register_attribute_notify(
            "Test.Target",
            Rc::new(move |_vm, _owner, _prop, _forms| {
                seen.set(seen.get() + 1);
                Ok(())
            }),
        )
        .unwrap();
    let damage = t.attribute("Test.DamageAttr", "Damage");
    let target = t.obj("Test.Target", "Test.Gun");
    let data = t.data(4.0, None, 1.0);
    t.call(vec![Value::Object(Some(damage)), data, Value::Object(None), Value::Object(Some(target))]).unwrap();
    assert_eq!(calls.get(), 1);
}

/// A `NotYet` met after the set committed a write carries the partial-writes mark once, with its kind kept: the
/// notification refuses after the base was written.
#[test]
fn a_not_yet_after_a_write_is_marked() {
    let mut t = Toy::new(ParityMode::Off);
    t.vm
        .register_attribute_notify(
            "Test.Target",
            Rc::new(|_vm, _owner, _prop, _forms| Err(ow_core::NotYet::not_read("the toy notification", "TOY.md").into())),
        )
        .unwrap();
    let damage = t.attribute("Test.DamageAttr", "Damage");
    let target = t.obj("Test.Target", "Test.Gun");
    let data = t.data(4.0, None, 1.0);
    let e = t.call(vec![Value::Object(Some(damage)), data, Value::Object(None), Value::Object(Some(target))]).unwrap_err();
    match e {
        VmError::NotYet(n) => {
            assert!(ow_core::notyet::is_partial_writes(&n.what), "{}", n.what);
            assert_eq!(n.what.matches("stopped after partial writes").count(), 1);
            assert_eq!(n.missing, ow_core::Missing::NotRead);
        }
        other => panic!("{other:?}"),
    }
}

/// Reference binds nothing (the C++ has no SetBaseValue).
#[test]
fn reference_binds_nothing() {
    let t = Toy::new(ParityMode::Reference);
    assert!(!t.vm.has_native(SET_BASE_VALUE));
}

/// The preflight loads nothing: an unloaded destination handle (the world fixture's lazy `Probe_LazyArch`) refuses
/// `NotYet { NotRead }` unmarked, and stays unloaded.
#[test]
fn an_unloaded_destination_refuses_without_loading() {
    crate::world::tests::run(|| {
        let mut x = crate::world::tests::fx_in(true, ParityMode::Off);
        x.class("AttributeInitializationDefinition");
        let path = ObjectPath::parse("Engine.ToyLazyHolder").unwrap();
        let vm = &mut x.w.vm;
        let holder = vm.defs.find(&mut vm.space, &path).unwrap().unwrap();
        let arch = crate::actors::object(&x.w.vm.space, holder, "Arch").unwrap().unwrap();
        assert!(x.w.vm.defs.is_unloaded(&x.w.vm.space, arch));
        let ty = x.w.vm.space.classes.struct_by_path("Engine.Object.AttributeInitializationData");
        let data = Value::structure(
            ty.unwrap(),
            vec![Value::Float(3.0), Value::Object(None), Value::Object(None), Value::Float(2.0)],
        );
        let loads = x.w.vm.defs.load_transitions();
        let e = x
            .call("AttributeInitializationDefinition.SetBaseValue", None, vec![Value::Object(Some(arch)), data, Value::Object(None), Value::Object(None)])
            .unwrap_err();
        match e {
            VmError::NotYet(n) => {
                assert_eq!(n.missing, ow_core::Missing::NotRead);
                assert_eq!(n.what, UNLOADED);
            }
            other => panic!("{other:?}"),
        }
        assert!(x.w.vm.defs.is_unloaded(&x.w.vm.space, arch), "nothing was loaded");
        assert_eq!(x.w.vm.defs.load_transitions(), loads);
    });
}

/// Review r1 of #572, case 1: a loaded multi-context destination with variance on refuses in the preflight, before
/// the evaluation draws. The refusal is unmarked, and the heap, RNG and load counters are unchanged. The control
/// (the same data on a plain destination) does draw, so the test would catch a draw before the refusal.
#[test]
fn a_multi_context_destination_refuses_before_the_variance_draw() {
    let mut t = Toy::new(ParityMode::Off);
    let definition = t.variance_definition();
    let multi = t.obj("Engine.AttributeDefinitionMultiContext", "Test.Multi");
    let target = t.obj("Test.Target", "Test.Gun");
    let base = t.data(4.0, None, 1.0);
    let data = t.with_definition(base, definition);
    let before = t.counters();
    let e = t
        .call(vec![Value::Object(Some(multi)), data.clone(), Value::Object(None), Value::Object(Some(target))])
        .unwrap_err();
    match &e {
        VmError::NotYet(n) => {
            assert!(!ow_core::notyet::is_partial_writes(&n.what), "{}", n.what);
            assert_eq!(n.missing, ow_core::Missing::NotRead);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(t.counters(), before, "nothing written, drawn or loaded");
    // The control: a plain destination draws the variance.
    let damage = t.attribute("Test.DamageAttr", "Damage");
    let draws = t.vm.rng.draws();
    t.call(vec![Value::Object(Some(damage)), data, Value::Object(None), Value::Object(Some(target))]).unwrap();
    assert!(t.vm.rng.draws() > draws, "the control draws");
}

/// Review r1 of #572, case 2: a loaded destination whose value resolver is still an unloaded handle refuses in the
/// preflight, without loading it. The refusal is unmarked, the counters are unchanged, and the handle stays unloaded.
#[test]
fn a_lazy_value_resolver_refuses_without_loading() {
    crate::world::tests::run(|| {
        let mut x = crate::world::tests::fx_in(true, ParityMode::Off);
        x.class("AttributeInitializationDefinition");
        let path = ObjectPath::parse("Engine.Toy_LazyAttr").unwrap();
        let vm = &mut x.w.vm;
        let attr = vm.defs.find(&mut vm.space, &path).unwrap().unwrap();
        let class = x.w.vm.space.heap.get(attr).unwrap().class;
        let p = x.w.vm.space.prop(class, "ValueResolverChain").unwrap();
        let reader = match x.w.vm.space.read(attr, PropAddr::scalar(p)).unwrap() {
            Value::Array(a) => match a.first() {
                Some(Value::Object(Some(r))) => *r,
                other => panic!("{other:?}"),
            },
            other => panic!("{other:?}"),
        };
        assert!(x.w.vm.defs.is_unloaded(&x.w.vm.space, reader), "the fixture's resolver is lazy");
        let ty = x.w.vm.space.classes.struct_by_path("Engine.Object.AttributeInitializationData").unwrap();
        let data = Value::structure(ty, vec![Value::Float(3.0), Value::Object(None), Value::Object(None), Value::Float(1.0)]);
        let before = (x.w.vm.space.heap.committed_writes(), x.w.vm.rng.draws(), x.w.vm.defs.load_transitions());
        let e = x
            .call("AttributeInitializationDefinition.SetBaseValue", None, vec![Value::Object(Some(attr)), data, Value::Object(None), Value::Object(None)])
            .unwrap_err();
        match &e {
            VmError::NotYet(n) => {
                assert!(n.what.starts_with(ow_attr::eval::PREFLIGHT_UNLOADED), "{}", n.what);
                assert_eq!(n.missing, ow_core::Missing::NotRead);
            }
            other => panic!("{other:?}"),
        }
        let after = (x.w.vm.space.heap.committed_writes(), x.w.vm.rng.draws(), x.w.vm.defs.load_transitions());
        assert_eq!(after, before, "nothing written, drawn or loaded");
        assert!(x.w.vm.defs.is_unloaded(&x.w.vm.space, reader), "the resolver is still unloaded");
    });
}
