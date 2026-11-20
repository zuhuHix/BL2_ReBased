//! `SetBaseValue` on an invented class set (AGENTS.md rule 9; this lane's decision 2026-11-19-187). Expected values are
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

/// The world fixture's `Toy_LazyAttr`, loaded, with a resolving context chain (a runtime `PawnAttributeContextResolver`,
/// review r2 of #572), its lazy value resolver, and a valid target (an `AttrTarget` pawn with the plain attribute the
/// resolver names).
fn lazy_destination(x: &mut crate::world::tests::Fx) -> (ObjRef, ObjRef, ObjRef) {
    // The fixture world has no resolvers registered; these tests use the standard set.
    x.w.vm.attrs = Rc::new(AttributeSystem::standard().unwrap());
    x.class("AttributeInitializationDefinition");
    let path = ObjectPath::parse("Engine.Toy_LazyAttr").unwrap();
    let vm = &mut x.w.vm;
    let attr = vm.defs.find(&mut vm.space, &path).unwrap().unwrap();
    let class = vm.space.heap.get(attr).unwrap().class;
    let p = vm.space.prop(class, "ValueResolverChain").unwrap();
    let reader = match vm.space.read(attr, PropAddr::scalar(p)).unwrap() {
        Value::Array(a) => match a.first() {
            Some(Value::Object(Some(r))) => *r,
            other => panic!("{other:?}"),
        },
        other => panic!("{other:?}"),
    };
    let resolver_class = x.class("PawnAttributeContextResolver");
    let vm = &mut x.w.vm;
    let resolver = vm.space.alloc(resolver_class, "Toy.PawnResolver", None, Origin::Runtime).unwrap();
    let chain = vm.space.prop(class, "ContextResolverChain").unwrap();
    vm.space.write(attr, PropAddr::scalar(chain), Value::Array(Rc::new(vec![Value::Object(Some(resolver))]))).unwrap();
    let target_class = x.class("AttrTarget");
    let target = x.w.vm.space.alloc(target_class, "Toy.Target", None, Origin::Runtime).unwrap();
    (attr, reader, target)
}

fn fixture_data(x: &crate::world::tests::Fx, constant: f32, attribute: Option<ObjRef>, definition: Option<ObjRef>) -> Value {
    let ty = x.w.vm.space.classes.struct_by_path("Engine.Object.AttributeInitializationData").unwrap();
    Value::structure(ty, vec![Value::Float(constant), Value::Object(attribute), Value::Object(definition), Value::Float(1.0)])
}

fn counters_of(x: &crate::world::tests::Fx) -> (u64, u64, u64) {
    (x.w.vm.space.heap.committed_writes(), x.w.vm.rng.draws(), x.w.vm.defs.load_transitions())
}

/// Review r1 of #572, case 2 (made to reach the resolver in review r2): a loaded destination whose context chain
/// resolves to a valid target and whose value resolver is still an unloaded handle refuses in the preflight, without
/// loading it. The refusal is unmarked, the counters are unchanged, and the handle stays unloaded.
#[test]
fn a_lazy_value_resolver_refuses_without_loading() {
    crate::world::tests::run(|| {
        let mut x = crate::world::tests::fx_in(true, ParityMode::Off);
        let (attr, reader, target) = lazy_destination(&mut x);
        assert!(x.w.vm.defs.is_unloaded(&x.w.vm.space, reader), "the fixture's resolver is lazy");
        let data = fixture_data(&x, 3.0, None, None);
        let before = counters_of(&x);
        let e = x
            .call(
                "AttributeInitializationDefinition.SetBaseValue",
                None,
                vec![Value::Object(Some(attr)), data, Value::Object(None), Value::Object(Some(target))],
            )
            .unwrap_err();
        match &e {
            VmError::NotYet(n) => {
                assert!(n.what.starts_with(ow_attr::eval::PREFLIGHT_UNLOADED), "{}", n.what);
                assert_eq!(n.missing, ow_core::Missing::NotRead);
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(counters_of(&x), before, "nothing written, drawn or loaded");
        assert!(x.w.vm.defs.is_unloaded(&x.w.vm.space, reader), "the resolver is still unloaded");
    });
}

/// The control of the test above (review r2 of #572): the same destination and target, set through `ow-attr`'s
/// `set_base` with no preflight, does reach the resolver: it loads it and sets the target's attribute. So the
/// preflight's refusal is what keeps the load from happening.
#[test]
fn without_the_preflight_the_set_reaches_and_loads_the_resolver() {
    crate::world::tests::run(|| {
        let mut x = crate::world::tests::fx_in(true, ParityMode::Off);
        let (attr, reader, target) = lazy_destination(&mut x);
        let loads = x.w.vm.defs.load_transitions();
        let attrs = x.w.vm.attrs.clone();
        let set = attrs.set_base(attr, Some(target), 3.0, false, &mut EvalCx::new(&mut x.w.vm)).unwrap();
        assert!(set, "the set reaches the target");
        assert!(x.w.vm.defs.load_transitions() > loads, "the resolver was loaded");
        assert!(!x.w.vm.defs.is_unloaded(&x.w.vm.space, reader));
        let class = x.w.vm.space.heap.get(target).unwrap().class;
        let p = x.w.vm.space.prop(class, "Toughness").unwrap();
        assert_eq!(x.w.vm.space.read(target, PropAddr::scalar(p)).unwrap(), &Value::Float(3.0));
    });
}

/// Review r2 of #572: the preflight checks only reachable branches. A conditional initialisation whose first entry
/// has no expressions (it always holds) never reaches its default, so an unloaded attribute there does not refuse. The
/// control (the same first entry with one expression, which may not hold) does reach the default and refuses.
#[test]
fn an_unreachable_conditional_branch_is_not_checked() {
    crate::world::tests::run(|| {
        let mut x = crate::world::tests::fx_in(true, ParityMode::Off);
        x.class("AttributeInitializationDefinition");
        let path = ObjectPath::parse("Engine.Toy_AttrHolder").unwrap();
        let vm = &mut x.w.vm;
        let holder = vm.defs.find(&mut vm.space, &path).unwrap().unwrap();
        let unreached = crate::actors::object(&x.w.vm.space, holder, "Attr").unwrap().unwrap();
        assert!(x.w.vm.defs.is_unloaded(&x.w.vm.space, unreached));
        let init_ty = x.w.vm.space.classes.struct_by_path("Engine.Object.AttributeInitializationData").unwrap();
        // The definition's shape, as runtime declarations (invented).
        let s = &mut x.w.vm.space;
        let d = |s: &mut ObjectSpace, n: &str, k: PropKind| PropertyDecl::new(s.names.intern(n), k);
        let object = s.classes.by_path("Engine.Object").unwrap();
        let expr_fields = vec![
            d(s, "AttributeOperand1", PropKind::Object { class: object }),
            d(s, "ComparisonOperator", PropKind::Byte { enum_: None }),
            d(s, "Operand2Usage", PropKind::Byte { enum_: None }),
            d(s, "AttributeOperand2", PropKind::Object { class: object }),
            d(s, "ConstantOperand2", PropKind::Float),
        ];
        let expr = s.classes.define_struct("Test.Expr", None, expr_fields).unwrap();
        let entry_fields = vec![
            d(s, "Expressions", PropKind::Array(Box::new(PropKind::Struct(expr)))),
            d(s, "BaseValueIfTrue", PropKind::Struct(init_ty)),
        ];
        let entry = s.classes.define_struct("Test.CondEntry", None, entry_fields).unwrap();
        let toggle = |s: &mut ObjectSpace, path: &str, names: &[&str]| {
            let fields = names.iter().map(|n| PropertyDecl::new(s.names.intern(n), PropKind::Bool)).collect();
            s.classes.define_struct(path, None, fields).unwrap()
        };
        let formula = toggle(s, "Test.Formula", &["bEnabled"]);
        let variance = toggle(s, "Test.Variance", &["bEnabled"]);
        let range = toggle(s, "Test.Range", &["bEnableMinValueRestriction", "bEnableMaxValueRestriction"]);
        let cond_fields = vec![
            d(s, "bEnabled", PropKind::Bool),
            d(s, "ConditionalExpressionList", PropKind::Array(Box::new(PropKind::Struct(entry)))),
            d(s, "DefaultBaseValue", PropKind::Struct(init_ty)),
        ];
        let cond = s.classes.define_struct("Test.Cond", None, cond_fields).unwrap();
        let def_props = vec![
            d(s, "ValueFormula", PropKind::Struct(formula)),
            d(s, "ConditionalInitialization", PropKind::Struct(cond)),
            d(s, "RandomVariance", PropKind::Struct(variance)),
            d(s, "RangeRestriction", PropKind::Struct(range)),
        ];
        let def_class = s.classes.define("Test.CondDefinition", Some(object), def_props).unwrap();
        let init = |constant: f32, attribute: Option<ObjRef>| {
            Value::structure(init_ty, vec![Value::Float(constant), Value::Object(attribute), Value::Object(None), Value::Float(1.0)])
        };
        let off = |ty| Value::structure(ty, vec![Value::Bool(false)]);
        let build = |x: &mut crate::world::tests::Fx, expressions: Vec<Value>, name: &str| -> ObjRef {
            let s = &mut x.w.vm.space;
            let first = Value::structure(entry, vec![Value::Array(Rc::new(expressions)), init(5.0, None)]);
            let conditional = Value::structure(cond, vec![Value::Bool(true), Value::Array(Rc::new(vec![first])), init(0.0, Some(unreached))]);
            let o = s.alloc(def_class, name, None, Origin::Runtime).unwrap();
            for (prop, v) in [
                ("ValueFormula", off(formula)),
                ("ConditionalInitialization", conditional),
                ("RandomVariance", off(variance)),
                ("RangeRestriction", Value::structure(range, vec![Value::Bool(false), Value::Bool(false)])),
            ] {
                let p = s.prop(def_class, prop).unwrap();
                s.write(o, PropAddr::scalar(p), v).unwrap();
            }
            o
        };
        let attr_class = x.class("AttributeDefinition");
        let dest = x.w.vm.space.alloc(attr_class, "Toy.Dest", None, Origin::Runtime).unwrap();
        let always = build(&mut x, Vec::new(), "Toy.Always");
        let data = init_with(init_ty, always);
        let attrs = x.w.vm.attrs.clone();
        let before = counters_of(&x);
        attrs.preflight_set_base_value(dest, &data, &mut EvalCx::new(&mut x.w.vm)).unwrap();
        assert_eq!(counters_of(&x), before);
        // The control: one expression with no first operand (its second is not checked), so the entry may fail and
        // the default is reached: the unloaded attribute there refuses.
        let e = Value::structure(expr, vec![Value::Object(None), Value::Byte(0), Value::Byte(0), Value::Object(Some(unreached)), Value::Float(1.0)]);
        let maybe = build(&mut x, vec![e], "Toy.Maybe");
        let data = init_with(init_ty, maybe);
        let refused = attrs.preflight_set_base_value(dest, &data, &mut EvalCx::new(&mut x.w.vm)).unwrap_err();
        assert!(format!("{refused:?}").contains(ow_attr::eval::PREFLIGHT_UNLOADED), "{refused:?}");
        assert!(x.w.vm.defs.is_unloaded(&x.w.vm.space, unreached), "nothing was loaded");
    });
}

/// An `InitData` with no constant or attribute and `definition` as its initialisation definition.
fn init_with(_ty: StructId, definition: ObjRef) -> ow_attr::api::InitData {
    ow_attr::api::InitData {
        base_value_constant: 0.0,
        base_value_attribute: None,
        initialization_definition: Some(definition),
        base_value_scale_constant: 1.0,
    }
}
