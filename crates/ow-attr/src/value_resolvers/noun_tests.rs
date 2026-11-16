//! `NounAttributeValueResolver` on a synthetic world (rule 9: invented objects and numbers; the class and field
//! names are the packages' names, so the resolver reads the shapes it reads from reflection).

use ow_core::{Missing, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::Origin;
use ow_object::ids::{ClassId, ObjRef, PropAddr, StructId};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_package::store::PackageStore;

use super::testworld::{Fx, system};
use crate::api::{AttrError, EvalCx, InitData};

/// The extra classes: the starting-value struct and an AI class holding a list of them, the noun store's entry
/// struct, a mind (an `AIClass` and a `NounState`) and a context with neither.
struct Noun {
    fx: Fx,
    start: StructId,
    state: StructId,
    ai_class: ClassId,
    mind: ClassId,
    plain: ClassId,
}

fn fixture() -> Noun {
    let mut fx = Fx::new();
    let object = fx.c.object;
    let aid = fx.c.aid;
    let space = &mut fx.w.space;
    let d = |space: &mut ow_object::heap::ObjectSpace, n: &str, k: PropKind| {
        PropertyDecl::new(space.names.intern(n), k)
    };
    let fields = vec![
        d(space, "Attribute", PropKind::Object { class: object }),
        d(space, "BaseValue", PropKind::Struct(aid)),
    ];
    let start = space
        .classes
        .define_struct(
            "Engine.AttributeInitializationDefinition.AttributeBaseValueData",
            None,
            fields,
        )
        .unwrap();
    let fields = vec![
        d(space, "NounName", PropKind::Name),
        d(space, "Value", PropKind::Float),
        d(space, "BaseValue", PropKind::Float),
        d(
            space,
            "ModifierStack",
            PropKind::Array(Box::new(PropKind::Object { class: object })),
        ),
        d(space, "ReplicationStrategy", PropKind::Byte { enum_: None }),
        d(space, "IndexInReplicationArray", PropKind::Int),
    ];
    let state = space
        .classes
        .define_struct(
            "GearboxFramework.INounAttributeProvider.NounAttributeState",
            None,
            fields,
        )
        .unwrap();
    let decls = vec![d(
        space,
        "AttributeStartingValues",
        PropKind::Array(Box::new(PropKind::Struct(start))),
    )];
    let ai_class = space
        .classes
        .define("Test.AIClassDefinition", Some(object), decls)
        .unwrap();
    let decls = vec![
        d(space, "AIClass", PropKind::Object { class: object }),
        d(
            space,
            "NounState",
            PropKind::Array(Box::new(PropKind::Struct(state))),
        ),
    ];
    let mind = space
        .classes
        .define("Test.WillowMind", Some(object), decls)
        .unwrap();
    let plain = space
        .classes
        .define("Test.NotAProvider", Some(object), vec![])
        .unwrap();
    Noun {
        fx,
        start,
        state,
        ai_class,
        mind,
        plain,
    }
}

impl Noun {
    /// An attribute named `name` whose one value resolver is a Noun resolver with `default` as its `DefaultValue`,
    /// the resolver a subobject of the attribute (the installed shape); the context is the source (no-context chain).
    fn attr(&mut self, name: &str, default: Value) -> ObjRef {
        let ctx = self.fx.no_context();
        let attr = self.fx.attr(name, vec![ctx], vec![]);
        let c = self
            .fx
            .by_path("GearboxFramework.NounAttributeValueResolver");
        let r = self
            .fx
            .w
            .space
            .alloc(
                c,
                "NounAttributeValueResolver_0",
                Some(attr),
                Origin::Runtime,
            )
            .unwrap();
        let p = self.fx.w.space.prop(c, "DefaultValue").unwrap();
        self.fx
            .w
            .space
            .write(r, PropAddr::scalar(p), default)
            .unwrap();
        let p = self
            .fx
            .w
            .space
            .prop(self.fx.c.attribute, "ValueResolverChain")
            .unwrap();
        self.fx
            .w
            .space
            .write(
                attr,
                PropAddr::scalar(p),
                Value::array(vec![Value::Object(Some(r))]),
            )
            .unwrap();
        attr
    }

    /// The one value resolver of `attr`.
    fn resolver_of(&self, attr: ObjRef) -> ObjRef {
        let p = self
            .fx
            .w
            .space
            .prop(self.fx.c.attribute, "ValueResolverChain")
            .unwrap();
        let chain = self.fx.w.space.read(attr, PropAddr::scalar(p)).unwrap();
        let Value::Object(Some(r)) = chain.elements().unwrap()[0] else {
            panic!("no resolver")
        };
        r
    }

    /// An AI class whose starting values are `entries` (attribute, base value data).
    fn ai_class(&mut self, entries: Vec<(ObjRef, Value)>) -> ObjRef {
        let start = self.start;
        let list = entries
            .into_iter()
            .map(|(a, v)| {
                self.fx.st(
                    start,
                    vec![("Attribute", Value::Object(Some(a))), ("BaseValue", v)],
                )
            })
            .collect();
        self.fx.obj(
            self.ai_class,
            "AIClass",
            vec![("AttributeStartingValues", Value::array(list))],
        )
    }

    /// A noun store entry.
    fn noun(&mut self, name: &str, value: f32, base: f32) -> Value {
        let n = self.fx.name(name);
        let state = self.state;
        self.fx.st(
            state,
            vec![
                ("NounName", n),
                ("Value", Value::Float(value)),
                ("BaseValue", Value::Float(base)),
            ],
        )
    }

    /// A mind with `class` as its `AIClass` and `nouns` as its `NounState`.
    fn mind(&mut self, class: Option<ObjRef>, nouns: Vec<Value>) -> ObjRef {
        self.fx.obj(
            self.mind,
            "Mind",
            vec![
                ("AIClass", Value::Object(class)),
                ("NounState", Value::array(nouns)),
            ],
        )
    }

    fn mode(&mut self, mode: ParityMode) {
        self.fx.w.defs = Some(DefStore::new(
            PackageStore::new_empty(),
            IdentityPolicy::FirstInScanOrder,
            mode,
        ));
    }

    fn value(&mut self, attr: ObjRef, source: ObjRef) -> Result<f32, AttrError> {
        let sys = system();
        sys.value_of(attr, Some(source), None, &mut EvalCx::new(&mut self.fx.w))
            .map(|v| v.unwrap().value)
    }

    fn base(&mut self, attr: ObjRef, source: ObjRef) -> Result<f32, AttrError> {
        let sys = system();
        sys.base_value_of(attr, Some(source), None, &mut EvalCx::new(&mut self.fx.w))
            .map(|v| v.unwrap().value)
    }
}

fn missing(e: AttrError) -> Missing {
    match e {
        AttrError::NotYet(n) => n.missing,
        other => panic!("expected NotYet, got {other:?}"),
    }
}

// ----- Reference mode: the C++ reading -----

#[test]
fn reference_reads_the_ai_class_starting_value_for_this_attribute() {
    let mut n = fixture();
    n.mode(ParityMode::Reference);
    let default = n.fx.k(1.0);
    let health = n.attr("Attribute_HealthMultiplier", default);
    let default = n.fx.k(7.0);
    let other = n.attr("Attribute_Other", default);
    // The starting value is init data, evaluated: 3 x 0.5.
    let start = n.fx.data(InitData {
        base_value_scale_constant: 0.5,
        ..InitData::constant(3.0)
    });
    let five = n.fx.k(5.0);
    let class = n.ai_class(vec![(other, five), (health, start)]);
    let mind = n.mind(Some(class), vec![]);
    assert_eq!(n.value(health, mind), Ok(1.5));
    assert_eq!(n.value(other, mind), Ok(5.0));
    // Base and value are the same read (the C++ has one).
    assert_eq!(n.base(health, mind), Ok(1.5));
}

#[test]
fn reference_falls_back_to_the_default_value() {
    let mut n = fixture();
    n.mode(ParityMode::Reference);
    let default = n.fx.data(InitData {
        base_value_scale_constant: 2.0,
        ..InitData::constant(1.25)
    });
    let attr = n.attr("Attribute_Listed_Nowhere", default);
    let default = n.fx.k(9.0);
    let listed = n.attr("Attribute_Listed", default);
    let four = n.fx.k(4.0);
    let class = n.ai_class(vec![(listed, four)]);
    // The class lists another attribute only.
    let mind = n.mind(Some(class), vec![]);
    assert_eq!(n.value(attr, mind), Ok(2.5));
    // No AI class.
    let bare = n.mind(None, vec![]);
    assert_eq!(n.value(listed, bare), Ok(9.0));
    // A context whose class has no AIClass property at all (a weapon, a pawn).
    let plain = n.fx.obj(n.plain, "Weapon", vec![]);
    assert_eq!(n.value(listed, plain), Ok(9.0));
}

#[test]
fn reference_does_not_read_the_noun_store() {
    let mut n = fixture();
    n.mode(ParityMode::Reference);
    let default = n.fx.k(1.0);
    let attr = n.attr("Attribute_HealthMultiplier", default);
    let stored = n.noun("Attribute_HealthMultiplier", 6.0, 6.0);
    let mind = n.mind(None, vec![stored]);
    assert_eq!(n.value(attr, mind), Ok(1.0));
}

#[test]
fn reference_needs_the_attribute_as_the_outer() {
    let mut n = fixture();
    n.mode(ParityMode::Reference);
    let c = n.fx.by_path("GearboxFramework.NounAttributeValueResolver");
    let k = n.fx.k(1.0);
    let loose = n.fx.obj(c, "Loose", vec![("DefaultValue", k)]);
    let ctx = n.fx.no_context();
    let attr = n.fx.attr("Attribute_Loose", vec![ctx], vec![loose]);
    let mind = n.mind(None, vec![]);
    assert_eq!(missing(n.value(attr, mind).unwrap_err()), Missing::NotRead);
}

#[test]
fn reference_needs_the_outer_to_list_the_resolver() {
    let mut n = fixture();
    n.mode(ParityMode::Reference);
    // A owns the resolver but lists none; B lists it but does not own it.
    let ctx = n.fx.no_context();
    let a = n.fx.attr("Attribute_A", vec![ctx], vec![]);
    let c = n.fx.by_path("GearboxFramework.NounAttributeValueResolver");
    let r =
        n.fx.w
            .space
            .alloc(c, "NounAttributeValueResolver_0", Some(a), Origin::Runtime)
            .unwrap();
    let k = n.fx.k(2.0);
    let p = n.fx.w.space.prop(c, "DefaultValue").unwrap();
    n.fx.w.space.write(r, PropAddr::scalar(p), k).unwrap();
    let ctx = n.fx.no_context();
    let b = n.fx.attr("Attribute_B", vec![ctx], vec![r]);
    let three = n.fx.k(3.0);
    let class = n.ai_class(vec![(a, three)]);
    let mind = n.mind(Some(class), vec![]);
    assert_eq!(missing(n.value(b, mind).unwrap_err()), Missing::NotRead);
    assert_eq!(missing(n.base(b, mind).unwrap_err()), Missing::NotRead);
}

// ----- Off mode: the native rule waits for a clean-room implementation -----

fn not_ported_awaiting(r: Result<f32, AttrError>) {
    match r {
        Err(AttrError::NotYet(e)) => {
            assert_eq!(e.missing, Missing::NotPorted);
            assert!(
                e.what.contains("awaiting clean-room implementation"),
                "{}",
                e.what
            );
            assert_eq!(e.record, "NATIVE_NOUN_ATTRIBUTES.md");
        }
        other => panic!("expected NotYet NotPorted, got {other:?}"),
    }
}

#[test]
fn off_is_not_ported_yet_with_or_without_a_store() {
    for with_store in [false, true] {
        let mut n = fixture();
        if with_store {
            n.mode(ParityMode::Off);
        }
        let default = n.fx.k(1.0);
        let attr = n.attr("Attribute_HealthMultiplier", default);
        let four = n.fx.k(4.0);
        let class = n.ai_class(vec![(attr, four)]);
        let mind = n.mind(Some(class), vec![]);
        not_ported_awaiting(n.value(attr, mind));
        not_ported_awaiting(n.base(attr, mind));
    }
}

#[test]
fn writes_are_not_ported_yet_in_either_mode() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let mut n = fixture();
        n.mode(mode);
        let default = n.fx.k(1.0);
        let attr = n.attr("Attribute_HealthMultiplier", default);
        let resolver = n.resolver_of(attr);
        let mind = n.mind(None, vec![]);
        let sys = system();
        let r = sys.value_resolver(&n.fx.w.space, resolver).unwrap();
        let mut cx = EvalCx::new(&mut n.fx.w);
        assert!(matches!(
            r.storage(&sys, resolver, mind, &mut cx),
            Err(AttrError::NotYet(ref e)) if e.missing == Missing::NotPorted
        ));
    }
}
