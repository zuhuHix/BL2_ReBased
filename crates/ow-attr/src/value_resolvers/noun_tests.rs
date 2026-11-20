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
use crate::api::{
    AddModifierOutcome, AttrError, EffectData, EvalCx, InitData, ProbeOp, Refusal, RemoveModifierOutcome,
};
use crate::stack::{CombineForm, Combined, Modifier, ModifierId, ModifierType, NumKind, combine};

/// The extra classes: the starting-value struct and an AI class holding a list of them, the noun store's entry
/// struct, a mind (an `AIClass` and a `NounState`), a context with neither, and the modifier class.
struct Noun {
    fx: Fx,
    start: StructId,
    state: StructId,
    ai_class: ClassId,
    mind: ClassId,
    plain: ClassId,
    modifier: ClassId,
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
    let decls = vec![
        d(space, "Type", PropKind::Byte { enum_: None }),
        d(space, "Value", PropKind::Float),
    ];
    let modifier = space
        .classes
        .define("Core.AttributeModifier", Some(object), decls)
        .unwrap();
    Noun {
        fx,
        start,
        state,
        ai_class,
        mind,
        plain,
        modifier,
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

// ----- Off mode: the native rule (NATIVE_NOUN_ATTRIBUTES.md), implemented from the note alone -----

const SCALE: u8 = 0;
const PRE: u8 = 1;
const POST: u8 = 2;

/// One `NounState` element as the tests read it back.
#[derive(Debug, Clone, PartialEq)]
struct Entry {
    name: String,
    value: f32,
    base: f32,
    stack: Vec<Option<ObjRef>>,
    strategy: u8,
    slot: i32,
}

impl Noun {
    /// The one value resolver of `attr` gets `field` = `v`.
    fn set_resolver(&mut self, attr: ObjRef, field: &str, v: Value) {
        let r = self.resolver_of(attr);
        let c = self.fx.w.space.heap.get(r).unwrap().class;
        let p = self.fx.w.space.prop(c, field).unwrap();
        self.fx.w.space.write(r, PropAddr::scalar(p), v).unwrap();
    }

    /// An `AttributeModifier` of type `kind` and value `value`.
    fn modifier(&mut self, kind: u8, value: f32) -> ObjRef {
        self.fx.obj(
            self.modifier,
            "AttributeModifier",
            vec![("Type", Value::Byte(kind)), ("Value", Value::Float(value))],
        )
    }

    /// A noun store element with every field given.
    fn full_noun(&mut self, e: &Entry) -> Value {
        let n = self.fx.name(&e.name);
        let stack = Value::array(e.stack.iter().map(|m| Value::Object(*m)).collect());
        let state = self.state;
        self.fx.st(
            state,
            vec![
                ("NounName", n),
                ("Value", Value::Float(e.value)),
                ("BaseValue", Value::Float(e.base)),
                ("ModifierStack", stack),
                ("ReplicationStrategy", Value::Byte(e.strategy)),
                ("IndexInReplicationArray", Value::Int(e.slot)),
            ],
        )
    }

    /// The `NounState` of `ctx`, element by element.
    fn entries(&self, ctx: ObjRef) -> Vec<Entry> {
        let space = &self.fx.w.space;
        let class = space.heap.get(ctx).unwrap().class;
        let p = space.prop(class, "NounState").unwrap();
        let items = space.read(ctx, PropAddr::scalar(p)).unwrap().clone();
        let f = |e: &Value, n: &str| crate::eval::field(space, e, n).unwrap().clone();
        items
            .elements()
            .unwrap()
            .iter()
            .map(|e| Entry {
                name: match f(e, "NounName") {
                    Value::Name(n) => space.names.text(n).into_owned(),
                    other => panic!("{other:?}"),
                },
                value: crate::eval::number("Value", &f(e, "Value")).unwrap(),
                base: crate::eval::number("BaseValue", &f(e, "BaseValue")).unwrap(),
                stack: f(e, "ModifierStack")
                    .elements()
                    .unwrap()
                    .iter()
                    .map(|m| crate::eval::object("m", m).unwrap())
                    .collect(),
                strategy: match f(e, "ReplicationStrategy") {
                    Value::Byte(b) => b,
                    other => panic!("{other:?}"),
                },
                slot: match f(e, "IndexInReplicationArray") {
                    Value::Int(i) => i,
                    other => panic!("{other:?}"),
                },
            })
            .collect()
    }

    fn add(
        &mut self,
        attr: ObjRef,
        ctx: ObjRef,
        m: ObjRef,
    ) -> Result<AddModifierOutcome, AttrError> {
        let sys = system();
        sys.add_modifier(attr, Some(ctx), m, false, &mut EvalCx::new(&mut self.fx.w))
    }

    fn remove(
        &mut self,
        attr: ObjRef,
        ctx: ObjRef,
        m: ObjRef,
    ) -> Result<RemoveModifierOutcome, AttrError> {
        let sys = system();
        sys.remove_modifier(attr, Some(ctx), m, false, &mut EvalCx::new(&mut self.fx.w))
    }

    fn set_base(&mut self, attr: ObjRef, ctx: ObjRef, v: f32) -> Result<bool, AttrError> {
        let sys = system();
        sys.set_base(attr, Some(ctx), v, false, &mut EvalCx::new(&mut self.fx.w))
    }

    /// A fresh entry as section 4 creates it.
    fn created(name: &str, d: f32) -> Entry {
        Entry {
            name: name.to_string(),
            value: d,
            base: d,
            stack: vec![],
            strategy: 0,
            slot: -1,
        }
    }
}

/// An Off-mode fixture (a store in Off mode).
fn off() -> Noun {
    let mut n = fixture();
    n.mode(ParityMode::Off);
    n
}

#[test]
fn off_first_get_creates_the_entry_from_the_default() {
    // Section 4 "A created entry", section 5 "Get"; checklist 3 and 4.
    for with_store in [false, true] {
        let mut n = fixture();
        if with_store {
            n.mode(ParityMode::Off);
        }
        let default = n.fx.data(InitData {
            base_value_scale_constant: 0.5,
            ..InitData::constant(3.0)
        });
        let attr = n.attr("Attribute_HealthMultiplier", default);
        let mind = n.mind(None, vec![]);
        assert_eq!(n.value(attr, mind), Ok(1.5));
        assert_eq!(
            n.entries(mind),
            vec![Noun::created("Attribute_HealthMultiplier", 1.5)]
        );
        // The second read finds the entry; get-base is its BaseValue.
        assert_eq!(n.base(attr, mind), Ok(1.5));
        assert_eq!(n.entries(mind).len(), 1);
    }
}

#[test]
fn off_get_base_alone_creates_the_entry_too() {
    // Section 4 table: "yes, by reads and writes alike"; section 5 "Get-base".
    let mut n = off();
    let default = n.fx.k(2.0);
    let attr = n.attr("Attribute_A", default);
    let mind = n.mind(None, vec![]);
    assert_eq!(n.base(attr, mind), Ok(2.0));
    assert_eq!(n.entries(mind), vec![Noun::created("Attribute_A", 2.0)]);
}

#[test]
fn off_reads_an_existing_entry_and_never_reevaluates_the_default() {
    // Section 5 "Get", "Get-base"; the last bullet of section 5 (`d` is never evaluated again).
    let mut n = off();
    let default = n.fx.k(1.0);
    let attr = n.attr("Attribute_HealthMultiplier", default);
    let stored = n.noun("Attribute_HealthMultiplier", 6.0, 4.0);
    let mind = n.mind(None, vec![stored]);
    assert_eq!(n.value(attr, mind), Ok(6.0));
    assert_eq!(n.base(attr, mind), Ok(4.0));
    // A created entry keeps its `d` when DefaultValue changes later.
    let fresh = n.mind(None, vec![]);
    assert_eq!(n.value(attr, fresh), Ok(1.0));
    let nine = n.fx.k(9.0);
    n.set_resolver(attr, "DefaultValue", nine);
    assert_eq!(n.value(attr, fresh), Ok(1.0));
    assert_eq!(n.entries(fresh).len(), 1);
}

#[test]
fn off_ignores_the_previous_chain_value() {
    // Section 2 and section 5 "Get": the chain's previous value is ignored; checklist 4.
    let mut n = off();
    let default = n.fx.k(2.0);
    let attr = n.attr("Attribute_A", default);
    let r = n.resolver_of(attr);
    let k = n.fx.constant(50.0);
    let p =
        n.fx.w
            .space
            .prop(n.fx.c.attribute, "ValueResolverChain")
            .unwrap();
    let chain = Value::array(vec![Value::Object(Some(k)), Value::Object(Some(r))]);
    n.fx.w
        .space
        .write(attr, PropAddr::scalar(p), chain)
        .unwrap();
    let mind = n.mind(None, vec![]);
    assert_eq!(n.value(attr, mind), Ok(2.0));
}

#[test]
fn off_key_is_the_outer_name_first_match_number_included() {
    // Section 3 (name equality, the number included) and section 4 "entry" (the first match in array order).
    let mut n = off();
    let default = n.fx.k(1.0);
    let attr = n.attr("Attribute_X", default);
    let first = n.noun("Attribute_X", 5.0, 5.0);
    let second = n.noun("Attribute_X", 8.0, 8.0);
    let mind = n.mind(None, vec![first, second]);
    assert_eq!(n.value(attr, mind), Ok(5.0));
    // `Attribute_X_1` is another name (the same base with a number): no match, so an entry is created.
    let numbered = n.noun("Attribute_X_1", 7.0, 7.0);
    let other = n.mind(None, vec![numbered]);
    assert_eq!(n.value(attr, other), Ok(1.0));
    let names: Vec<String> = n.entries(other).into_iter().map(|e| e.name).collect();
    assert_eq!(names, ["Attribute_X_1", "Attribute_X"]);
}

#[test]
fn off_key_comes_from_any_outer() {
    // Section 3: the key is the outer's name; Off does not need the outer to be the attribute.
    let mut n = off();
    let c = n.fx.by_path("GearboxFramework.NounAttributeValueResolver");
    let holder = n.fx.obj(n.plain, "Attribute_Elsewhere", vec![]);
    let k = n.fx.k(4.0);
    let r =
        n.fx.w
            .space
            .alloc(c, "Noun_0", Some(holder), Origin::Runtime)
            .unwrap();
    let p = n.fx.w.space.prop(c, "DefaultValue").unwrap();
    n.fx.w.space.write(r, PropAddr::scalar(p), k).unwrap();
    let ctx = n.fx.no_context();
    let attr = n.fx.attr("Attribute_Listing", vec![ctx], vec![r]);
    let mind = n.mind(None, vec![]);
    assert_eq!(n.value(attr, mind), Ok(4.0));
    assert_eq!(
        n.entries(mind),
        vec![Noun::created("Attribute_Elsewhere", 4.0)]
    );
}

#[test]
fn off_two_outers_with_one_name_share_one_entry() {
    // Section 6, first bullet.
    let mut n = off();
    let one = n.fx.k(1.0);
    let a = n.attr("Attribute_Shared", one);
    let two = n.fx.k(2.0);
    let b = n.attr("Attribute_Shared", two);
    let mind = n.mind(None, vec![]);
    assert_eq!(n.set_base(a, mind, 6.0), Ok(true));
    assert_eq!(n.value(b, mind), Ok(6.0));
    assert_eq!(n.entries(mind).len(), 1);
}

#[test]
fn off_no_outer_reads_zero_and_writes_false() {
    // Section 3 (no outer: as not a provider), section 4 table row 1; checklist 1.
    let mut n = off();
    let c = n.fx.by_path("GearboxFramework.NounAttributeValueResolver");
    let k = n.fx.k(3.0);
    let loose = n.fx.obj(c, "Loose", vec![("DefaultValue", k)]);
    let ctx = n.fx.no_context();
    let attr = n.fx.attr("Attribute_Loose", vec![ctx], vec![loose]);
    let mind = n.mind(None, vec![]);
    let m = n.modifier(PRE, 1.0);
    assert_eq!(n.value(attr, mind), Ok(0.0));
    assert_eq!(n.base(attr, mind), Ok(0.0));
    let refused = AddModifierOutcome::Refused(Refusal::NotWritable);
    assert_eq!(n.add(attr, mind, m), Ok(refused));
    let refused = RemoveModifierOutcome::Refused(Refusal::NotWritable);
    assert_eq!(n.remove(attr, mind, m), Ok(refused));
    assert_eq!(n.set_base(attr, mind, 5.0), Ok(false));
    assert!(n.entries(mind).is_empty());
}

#[test]
fn off_a_context_that_is_not_a_provider_reads_zero_not_the_default() {
    // Section 4 table row 1, section 6 last bullet; checklist 2. The strategy does not matter there.
    for strategy in [0, 1, 2] {
        let mut n = off();
        let default = n.fx.k(999.0);
        let attr = n.attr("Attribute_A", default);
        n.set_resolver(attr, "ReplicationStrategy", Value::Byte(strategy));
        let controller = n.fx.obj(n.plain, "Controller", vec![]);
        let m = n.modifier(PRE, 1.0);
        assert_eq!(n.value(attr, controller), Ok(0.0));
        assert_eq!(n.base(attr, controller), Ok(0.0));
        let refused = AddModifierOutcome::Refused(Refusal::NotWritable);
        assert_eq!(n.add(attr, controller, m), Ok(refused));
        let refused = RemoveModifierOutcome::Refused(Refusal::NotWritable);
        assert_eq!(n.remove(attr, controller, m), Ok(refused));
        assert_eq!(n.set_base(attr, controller, 5.0), Ok(false));
    }
}

#[test]
fn off_the_default_is_evaluated_before_the_entry_is_appended() {
    // Section 4, "Two orderings change behaviour", first bullet.
    let mut n = off();
    let three = n.fx.k(3.0);
    let b = n.attr("Attribute_B", three);
    let reads_b = n.fx.of_attr(b);
    let a = n.attr("Attribute_A", reads_b);
    let mind = n.mind(None, vec![]);
    assert_eq!(n.value(a, mind), Ok(3.0));
    assert_eq!(
        n.entries(mind),
        vec![
            Noun::created("Attribute_B", 3.0),
            Noun::created("Attribute_A", 3.0)
        ]
    );
}

#[test]
fn off_a_fresh_context_starts_empty() {
    // Section 6, second bullet.
    let mut n = off();
    let default = n.fx.k(1.0);
    let attr = n.attr("Attribute_A", default);
    let first = n.mind(None, vec![]);
    let second = n.mind(None, vec![]);
    assert_eq!(n.set_base(attr, first, 7.0), Ok(true));
    assert_eq!(n.value(attr, second), Ok(1.0));
    assert_eq!(n.value(attr, first), Ok(7.0));
}

#[test]
fn off_add_appends_recomputes_and_refuses_the_same_object() {
    // Section 5 "Add a modifier" (identity, not equal values); checklist 5.
    let mut n = off();
    let default = n.fx.k(10.0);
    let attr = n.attr("Attribute_A", default);
    let mind = n.mind(None, vec![]);
    let m = n.modifier(PRE, 2.0);
    let twin = n.modifier(PRE, 2.0);
    // A write on a fresh provider creates the entry first.
    assert_eq!(n.add(attr, mind, m), Ok(AddModifierOutcome::Added));
    assert_eq!(n.value(attr, mind), Ok(12.0));
    assert_eq!(n.base(attr, mind), Ok(10.0));
    assert_eq!(n.add(attr, mind, m), Ok(AddModifierOutcome::Duplicate));
    assert_eq!(n.entries(mind)[0].stack, vec![Some(m)]);
    assert_eq!(n.add(attr, mind, twin), Ok(AddModifierOutcome::Added));
    assert_eq!(n.value(attr, mind), Ok(14.0));
    assert_eq!(n.entries(mind)[0].stack, vec![Some(m), Some(twin)]);
}

#[test]
fn off_remove_takes_every_copy_and_refuses_an_absent_one() {
    // Section 5 "Remove a modifier"; checklist 5.
    let mut n = off();
    let default = n.fx.k(1.0);
    let attr = n.attr("Attribute_A", default);
    let m = n.modifier(PRE, 1.0);
    let other = n.modifier(POST, 3.0);
    let absent = n.modifier(PRE, 1.0);
    let held = Entry {
        name: "Attribute_A".to_string(),
        value: 0.0,
        base: 10.0,
        stack: vec![Some(m), Some(other), Some(m)],
        strategy: 0,
        slot: -1,
    };
    let held = n.full_noun(&held);
    let mind = n.mind(None, vec![held]);
    let before = n.entries(mind);
    assert_eq!(
        n.remove(attr, mind, absent),
        Ok(RemoveModifierOutcome::NotPresent)
    );
    assert_eq!(n.entries(mind), before);
    assert_eq!(
        n.remove(attr, mind, m),
        Ok(RemoveModifierOutcome::Removed { count: 2 })
    );
    assert_eq!(n.entries(mind)[0].stack, vec![Some(other)]);
    assert_eq!(n.value(attr, mind), Ok(13.0));
}

#[test]
fn off_remove_on_a_fresh_provider_creates_the_entry_and_answers_false() {
    // Section 4 table (writes create the entry too) and section 5 "Remove": none was there.
    let mut n = off();
    let default = n.fx.k(5.0);
    let attr = n.attr("Attribute_A", default);
    let mind = n.mind(None, vec![]);
    let m = n.modifier(PRE, 1.0);
    assert_eq!(
        n.remove(attr, mind, m),
        Ok(RemoveModifierOutcome::NotPresent)
    );
    assert_eq!(n.entries(mind), vec![Noun::created("Attribute_A", 5.0)]);
}

#[test]
fn off_set_base_creates_from_the_default_then_overwrites_the_base() {
    // Section 5 "Set-base(x)"; checklist 5.
    let mut n = off();
    let default = n.fx.k(5.0);
    let attr = n.attr("Attribute_A", default);
    let mind = n.mind(None, vec![]);
    assert_eq!(n.set_base(attr, mind, 8.0), Ok(true));
    assert_eq!(n.entries(mind), vec![Noun::created("Attribute_A", 8.0)]);
    // With a stack, set-base recomputes the value.
    let m = n.modifier(POST, 0.5);
    assert_eq!(n.add(attr, mind, m), Ok(AddModifierOutcome::Added));
    assert_eq!(n.set_base(attr, mind, 2.0), Ok(true));
    assert_eq!(n.base(attr, mind), Ok(2.0));
    assert_eq!(n.value(attr, mind), Ok(2.5));
}

#[test]
fn off_recompute_is_the_single_precision_formula() {
    // Section 5 "Recompute": (base + pre) * ((1 + up) / (1 - down)) + post, down the zero-or-negative scales, sums
    // in stack order, other types skipped, no truncation, no clamp.
    let mut n = off();
    let default = n.fx.k(0.0);
    let attr = n.attr("Attribute_A", default);
    let mind = n.mind(None, vec![]);
    assert_eq!(n.set_base(attr, mind, 1.1), Ok(true));
    let mods = [
        (PRE, 0.3_f32),
        (SCALE, 0.7),
        (SCALE, -0.3),
        (SCALE, 0.0),
        (POST, 0.1),
        (7, 100.0),
    ];
    for (kind, v) in mods {
        let m = n.modifier(kind, v);
        assert_eq!(n.add(attr, mind, m), Ok(AddModifierOutcome::Added));
    }
    let (base, pre, up, down, post) = (1.1_f32, 0.3_f32, 0.7_f32, -0.3_f32 + 0.0, 0.1_f32);
    let expected = (base + pre) * ((1.0 + up) / (1.0 - down)) + post;
    let got = n.value(attr, mind).unwrap();
    assert_eq!(got.to_bits(), expected.to_bits());
    // The world's default combine form is WeaponGen; the noun keeps single precision anyway. These inputs tell the
    // two forms apart, so the check above is not vacuous.
    let as_mods: Vec<Modifier> = [
        (PRE, 0.3_f32),
        (SCALE, 0.7),
        (SCALE, -0.3),
        (SCALE, 0.0),
        (POST, 0.1),
    ]
    .iter()
    .enumerate()
    .map(|(i, (k, v))| Modifier {
        kind: ModifierType::from_raw(i64::from(*k)).unwrap(),
        value: *v,
        id: ModifierId(u64::try_from(i).unwrap()),
    })
    .collect();
    let wide = combine(base, &as_mods, CombineForm::WeaponGen, NumKind::Float);
    assert_ne!(wide, Combined::Float(expected));
    // No clamp and no truncation: a negative fractional result stays as it is.
    assert_eq!(n.set_base(attr, mind, -5.25), Ok(true));
    let expected = (-5.25_f32 + pre) * ((1.0 + up) / (1.0 - down)) + post;
    assert_eq!(n.value(attr, mind).unwrap().to_bits(), expected.to_bits());
    assert!(expected < 0.0 && expected.fract() != 0.0);
}

#[test]
fn off_writes_raise_no_change_notification() {
    // Section 5, "No operation raises the attribute change notification".
    let mut n = off();
    let default = n.fx.k(1.0);
    let attr = n.attr("Attribute_A", default);
    let mind = n.mind(None, vec![]);
    let m = n.modifier(PRE, 1.0);
    assert_eq!(n.add(attr, mind, m), Ok(AddModifierOutcome::Added));
    assert_eq!(
        n.remove(attr, mind, m),
        Ok(RemoveModifierOutcome::Removed { count: 1 })
    );
    assert_eq!(n.set_base(attr, mind, 3.0), Ok(true));
    assert_eq!(n.fx.w.notified, 0);
}

#[test]
fn off_a_bad_modifier_leaves_the_entry_as_it_was() {
    // Section 5 reads each modifier's type at the recompute; a modifier object without `Type` is a data error here,
    // and nothing is written.
    let mut n = off();
    let default = n.fx.k(1.0);
    let attr = n.attr("Attribute_A", default);
    let mind = n.mind(None, vec![]);
    assert_eq!(n.set_base(attr, mind, 4.0), Ok(true));
    let before = n.entries(mind);
    let not_a_modifier = n.fx.obj(n.plain, "NotAModifier", vec![]);
    assert!(matches!(
        n.add(attr, mind, not_a_modifier),
        Err(AttrError::BadData { .. })
    ));
    assert_eq!(n.entries(mind), before);
}

#[test]
fn off_class_values_arrive_only_through_set_base() {
    // Checklist 8: the AI class's starting value is not read by the Off rule; the first get gives `d`, and the
    // class value arrives once the script's set-base has run.
    let mut n = off();
    let default = n.fx.k(1.0);
    let attr = n.attr("Attribute_HealthMultiplier", default);
    let four = n.fx.k(4.0);
    let class = n.ai_class(vec![(attr, four)]);
    let mind = n.mind(Some(class), vec![]);
    assert_eq!(n.value(attr, mind), Ok(1.0));
    assert_eq!(n.set_base(attr, mind, 4.0), Ok(true));
    assert_eq!(n.value(attr, mind), Ok(4.0));
}

#[test]
fn off_applied_effects_stay_on_the_noun_stack_until_removed() {
    // Section 5 "Add" / "Remove" through AttributeEffect: the noun's stack holds the modifier object the apply made,
    // so it is not freed after the call (the `Storage::Noun` addition).
    let mut n = off();
    let default = n.fx.k(10.0);
    let attr = n.attr("Attribute_A", default);
    let mind = n.mind(None, vec![]);
    let sys = system();
    let effect = EffectData::new(Some(attr), ModifierType::PreAdd, InitData::constant(2.5));
    let applied = sys
        .apply_effects(&[effect], Some(mind), None, &mut EvalCx::new(&mut n.fx.w))
        .unwrap();
    assert_eq!(applied.0.len(), 1);
    let m = applied.0[0].modifier;
    assert!(!n.fx.w.space.heap.get(m).unwrap().destroyed);
    assert_eq!(n.entries(mind)[0].stack, vec![Some(m)]);
    assert_eq!(n.value(attr, mind), Ok(12.5));
    sys.remove_effects(applied, &mut EvalCx::new(&mut n.fx.w))
        .unwrap();
    assert_eq!(n.value(attr, mind), Ok(10.0));
    assert!(n.entries(mind)[0].stack.is_empty());
}

#[test]
fn off_a_replicated_strategy_on_an_unread_class_is_not_read() {
    // Section 10.5, preflight step 3: the fixture's mind (`Test.WillowMind`) is not a class the note reads, so its
    // arrays and change call are unknown for strategies 1 and 2. Nothing is created.
    for strategy in [1, 2] {
        let mut n = off();
        let default = n.fx.k(0.0);
        let attr = n.attr("IsPhaselocked", default);
        n.set_resolver(attr, "ReplicationStrategy", Value::Byte(strategy));
        let mind = n.mind(None, vec![]);
        let m = n.modifier(PRE, 1.0);
        assert_eq!(
            missing(n.value(attr, mind).unwrap_err()),
            Missing::NotRead
        );
        assert_eq!(missing(n.base(attr, mind).unwrap_err()), Missing::NotRead);
        assert_eq!(
            missing(n.add(attr, mind, m).unwrap_err()),
            Missing::NotRead
        );
        assert_eq!(
            missing(n.set_base(attr, mind, 1.0).unwrap_err()),
            Missing::NotRead
        );
        assert!(n.entries(mind).is_empty());
    }
}

#[test]
fn off_a_strategy_outside_the_table_is_not_read() {
    // Section 1: the enum has 0 to 2 (then the MAX sentinel); the table has no row for anything else.
    let mut n = off();
    let default = n.fx.k(0.0);
    let attr = n.attr("Attribute_A", default);
    n.set_resolver(attr, "ReplicationStrategy", Value::Byte(3));
    let mind = n.mind(None, vec![]);
    assert_eq!(missing(n.value(attr, mind).unwrap_err()), Missing::NotRead);
}

#[test]
fn off_an_entry_with_a_replicated_slot_on_an_unread_class_reads_but_is_not_written() {
    // Section 10.5, preflight step 3: a write to a slotted entry on an unread class needs its replicated array and
    // change call (not read). Reads are the entry's own fields.
    let mut n = off();
    let default = n.fx.k(0.0);
    let attr = n.attr("Attribute_A", default);
    let slotted = Entry {
        name: "Attribute_A".to_string(),
        value: 5.0,
        base: 5.0,
        stack: vec![],
        strategy: 2,
        slot: 0,
    };
    let slotted = n.full_noun(&slotted);
    let mind = n.mind(None, vec![slotted]);
    assert_eq!(n.value(attr, mind), Ok(5.0));
    let before = n.entries(mind);
    let m = n.modifier(PRE, 1.0);
    assert_eq!(
        missing(n.add(attr, mind, m).unwrap_err()),
        Missing::NotRead
    );
    assert_eq!(
        missing(n.set_base(attr, mind, 1.0).unwrap_err()),
        Missing::NotRead
    );
    assert_eq!(n.entries(mind), before);
}

// ----- Reference mode has no writes -----

#[test]
fn reference_writes_are_not_ported() {
    let mut n = fixture();
    n.mode(ParityMode::Reference);
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

/// The read-only probe (lane IMPL-APPLY-SLOT-EFFECT-MODIFIERS, review r1 of #483; lane FIX-483-FOLLOWUP): the chain
/// check of an add on a context with no entry validates the default without evaluating it, and appends nothing.
#[test]
fn off_the_chain_check_does_not_create_a_missing_entry() {
    let mut n = off();
    let default = n.fx.data(InitData::constant(3.0));
    let attr = n.attr("Attribute_HealthMultiplier", default);
    let mind = n.mind(None, vec![]);
    let sys = system();
    sys.check_modifier_chain(attr, mind, ProbeOp::Add, &mut EvalCx::new(&mut n.fx.w)).unwrap();
    assert!(n.entries(mind).is_empty(), "nothing appended");
}

/// The probe refuses an entry with a replicated slot when the operation would rewrite it, as the write would be
/// refused, so a preflight sees it: an add always, a removal only of a modifier the entry's stack holds (lane
/// FIX-483-FOLLOWUP, F6: removing an absent modifier changes nothing, so it is not refused).
#[test]
fn off_the_chain_check_refuses_a_replicated_slot_it_would_rewrite() {
    let mut n = off();
    let default = n.fx.data(InitData::constant(3.0));
    let attr = n.attr("Attribute_HealthMultiplier", default);
    let held = n.modifier(2, 1.0);
    let absent = n.modifier(2, 1.0);
    let e = Entry { slot: 2, stack: vec![Some(held)], ..Noun::created("Attribute_HealthMultiplier", 1.0) };
    let noun = n.full_noun(&e);
    let mind = n.mind(None, vec![noun]);
    let sys = system();
    for (op, refused) in [(ProbeOp::Add, true), (ProbeOp::Remove(held), true), (ProbeOp::Remove(absent), false)] {
        let r = sys.check_modifier_chain(attr, mind, op, &mut EvalCx::new(&mut n.fx.w));
        assert_eq!(matches!(r, Err(AttrError::NotYet(_))), refused, "{op:?}: {r:?}");
        if !refused {
            r.unwrap();
        }
        assert_eq!(n.entries(mind), vec![e.clone()], "unchanged");
    }
}

#[path = "noun_replication_tests.rs"]
mod replication;

// ----- The shared add of the script native (lane IMPL-ADD-ATTRIBUTE-MODIFIER, NATIVE_ADD_ATTRIBUTE_MODIFIER.md
// section 3): the rule's own cases on this hand world. The unloaded-handle cases (A0 to A2, an unloaded default) need
// a store with packages and are `ow-inventory`'s tests of the wrapper. -----

impl Noun {
    fn add_native(&mut self, attr: ObjRef, ctx: ObjRef, m: ObjRef) -> Result<bool, crate::add_native::AddError> {
        self.add_native_with(attr, ctx, m, false)
    }
    /// [`Noun::add_native`] with `bSuppressNotify` given.
    fn add_native_with(&mut self, attr: ObjRef, ctx: ObjRef, m: ObjRef, suppress: bool) -> Result<bool, crate::add_native::AddError> {
        system().add_modifier_native(attr, ctx, m, suppress, &mut EvalCx::new(&mut self.fx.w))
    }
    /// The world generator's draws (#518's generator-draw count; review r1 of #616, item 6).
    fn draws(&self) -> u64 {
        self.fx.w.rng.as_ref().map_or(0, ow_core::rand::CrtRand::draws)
    }
    fn writes(&self) -> u64 {
        self.fx.w.space.heap.committed_writes()
    }
    /// `attr`'s value chain set to `chain`.
    fn set_chain(&mut self, attr: ObjRef, chain: Vec<Option<ObjRef>>) {
        let p = self.fx.w.space.prop(self.fx.c.attribute, "ValueResolverChain").unwrap();
        let v = Value::array(chain.into_iter().map(Value::Object).collect());
        self.fx.w.space.write(attr, PropAddr::scalar(p), v).unwrap();
    }
}

/// A value resolver class whose probe passes and whose storage lookup refuses `LayerAbsent` (review r1 of #616, item
/// 3): the write path refuses something the probe cannot see.
struct LateRefusal;

const LATE_LAYER: &str = "a toy layer";

impl crate::api::ValueResolver for LateRefusal {
    fn value(&self, _: &crate::AttributeSystem, _: ObjRef, _: ObjRef, _: f32, _: &mut EvalCx<'_>) -> Result<f32, AttrError> {
        Ok(0.0)
    }
    fn storage(&self, _: &crate::AttributeSystem, _: ObjRef, _: ObjRef, _: &mut EvalCx<'_>) -> Result<Option<crate::api::Storage>, AttrError> {
        Err(ow_core::NotYet::layer_absent("the toy resolver's write", LATE_LAYER, "test").into())
    }
    fn probe_storage(&self, _: &crate::AttributeSystem, _: ObjRef, _: ObjRef, _: ProbeOp, _: &mut EvalCx<'_>) -> Result<(), AttrError> {
        Ok(())
    }
}

/// Review r1 of #616, item 3: a `NotYet` inside the write is terminal. The chain is a noun resolver (which makes its
/// entry and adds the modifier) then [`LateRefusal`], whose probe passes: the second entry's storage refuses after the
/// first wrote. The answer carries the partial-writes prefix exactly once and keeps its kind, and nothing is put back.
#[test]
fn a_not_yet_inside_the_write_is_terminal() {
    let mut n = off();
    let default = n.fx.k(10.0);
    let attr = n.attr("Attribute_A", default);
    let noun = n.resolver_of(attr);
    let object = n.fx.c.object;
    let class = n.fx.w.space.classes.define("Test.ToyLateRefusal", Some(object), vec![]).unwrap();
    let late = n.fx.obj(class, "ToyLateRefusal_0", vec![]);
    n.set_chain(attr, vec![Some(noun), Some(late)]);
    let mind = n.mind(None, vec![]);
    let m = n.modifier(POST, 1.0);
    let sys = system().with_value_resolver("Test.ToyLateRefusal", LateRefusal).unwrap();
    let before = n.writes();
    let (missing, text) = refused(sys.add_modifier_native(attr, mind, m, false, &mut EvalCx::new(&mut n.fx.w)));
    assert_eq!(missing, Missing::LayerAbsent(LATE_LAYER), "the kind is kept");
    assert!(text.starts_with("stopped after partial writes: "), "{text}");
    assert_eq!(text.matches("stopped after partial writes").count(), 1, "{text}");
    assert!(n.writes() > before, "the first entry wrote");
    assert_eq!(n.entries(mind)[0].stack, vec![Some(m)], "nothing put back");
}

fn refused(r: Result<bool, crate::add_native::AddError>) -> (Missing, String) {
    match r {
        Err(crate::add_native::AddError::Attr(AttrError::NotYet(n))) => (n.missing, n.to_string()),
        other => panic!("expected a NotYet, got {other:?}"),
    }
}

/// Tests 2a and 3: a strategy-0 noun on a mind gets its entry made and the modifier on its stack (true); the same
/// modifier again answers false and changes nothing.
#[test]
fn the_shared_add_puts_a_noun_modifier_on_a_fresh_entry_once() {
    let mut n = off();
    let default = n.fx.k(10.0);
    let attr = n.attr("Attribute_A", default);
    let mind = n.mind(None, vec![]);
    let m = n.modifier(POST, 1.0);
    assert!(n.add_native(attr, mind, m).unwrap());
    let e = n.entries(mind);
    assert_eq!(e.len(), 1);
    assert_eq!((e[0].base, e[0].stack.clone()), (10.0, vec![Some(m)]));
    let before = n.writes();
    assert!(!n.add_native(attr, mind, m).unwrap(), "the same modifier twice");
    assert_eq!(n.writes(), before, "nothing changed");
}

/// Test 4: a constant-resolver attribute answers false and changes nothing.
#[test]
fn the_shared_add_on_a_constant_attribute_is_false() {
    let mut n = off();
    let attr = n.fx.constant_attr("Attribute_K", 3.0);
    let mind = n.mind(None, vec![]);
    let m = n.modifier(PRE, 1.0);
    let before = n.writes();
    assert!(!n.add_native(attr, mind, m).unwrap());
    assert_eq!(n.writes(), before);
}

/// Test 5: an empty chain, and a None first entry, answer false with nothing written.
#[test]
fn the_shared_add_on_an_empty_chain_or_a_none_first_entry_is_false() {
    let mut n = off();
    let default = n.fx.k(10.0);
    let attr = n.attr("Attribute_A", default);
    let mind = n.mind(None, vec![]);
    let m = n.modifier(PRE, 1.0);
    let r = n.resolver_of(attr);
    for chain in [vec![], vec![None, Some(r)]] {
        n.set_chain(attr, chain);
        let before = n.writes();
        assert!(!n.add_native(attr, mind, m).unwrap());
        assert_eq!(n.writes(), before);
        assert!(n.entries(mind).is_empty(), "no entry made");
    }
}

/// A3: a value resolver class the registry does not register refuses before anything is written.
#[test]
fn the_shared_add_refuses_an_unregistered_resolver_class_before_writing() {
    let mut n = off();
    let default = n.fx.k(10.0);
    let attr = n.attr("Attribute_A", default);
    let mind = n.mind(None, vec![]);
    let m = n.modifier(PRE, 1.0);
    let noun = n.resolver_of(attr);
    let unknown = n.fx.obj(n.plain, "NotAResolver", vec![]);
    n.set_chain(attr, vec![Some(noun), Some(unknown)]);
    let before = n.writes();
    let (missing, text) = refused(n.add_native(attr, mind, m));
    assert_eq!(missing, Missing::NotRead);
    assert!(text.contains("value resolver class") && !text.contains("partial writes"), "{text}");
    assert_eq!(n.writes(), before);
    assert!(n.entries(mind).is_empty(), "the first entry did not write");
}

/// Test 9: a two-entry chain whose second entry refuses in its storage probe (a replicated strategy, which main's noun
/// resolver does not port) is refused before the first entry writes.
#[test]
fn the_shared_add_refuses_a_probe_refusal_before_the_first_entry_writes() {
    let mut n = off();
    let default = n.fx.k(10.0);
    let first = n.attr("Attribute_A", default.clone());
    let second = n.attr("Attribute_B", default);
    n.set_resolver(second, "ReplicationStrategy", Value::Byte(1));
    let (r1, r2) = (n.resolver_of(first), n.resolver_of(second));
    n.set_chain(first, vec![Some(r1), Some(r2)]);
    let mind = n.mind(None, vec![]);
    let m = n.modifier(PRE, 1.0);
    let before = (n.writes(), n.draws());
    let (_, text) = refused(n.add_native(first, mind, m));
    assert!(text.contains("ReplicationStrategy 1") && !text.contains("partial writes"), "{text}");
    assert_eq!((n.writes(), n.draws()), before);
    assert!(n.entries(mind).is_empty(), "the first entry did not write");
}

/// Test 8: a missing noun entry whose default has variance: the add succeeds, and the draw happens in the write. The
/// load-free probe alone draws nothing.
#[test]
fn a_default_with_variance_draws_in_the_write_not_in_the_probe() {
    let mut n = off();
    let v = n.fx.variance(2.0, 4.0, true);
    let def = n.fx.definition(vec![("RandomVariance", v)]);
    let default = n.fx.of_def(def);
    let attr = n.attr("Attribute_V", default);
    let mind = n.mind(None, vec![]);
    let m = n.modifier(PRE, 1.0);
    let before = (n.writes(), n.draws());
    let probe = crate::eval::load_free(crate::add_native::UNIT, || {
        system().check_modifier_chain(attr, mind, ProbeOp::Add, &mut EvalCx::new(&mut n.fx.w))
    });
    probe.unwrap();
    assert_eq!((n.writes(), n.draws()), before, "the probe writes and draws nothing");
    assert!(n.add_native(attr, mind, m).unwrap());
    assert!(n.draws() > before.1, "the write drew");
}

/// Test 2b: a replicated strategy mirrors the value into the replicated slot and sets `bNetDirty` (review r1 of #616,
/// item 7: the slot, the count and the flag are asserted). It needs #523's mirror (noun replication), which this
/// branch's base does not have; it was run against a local merge of main, which has it.
#[test]
#[ignore = "needs #523 (noun replication): on main a replicated strategy is NotYet { NotPorted } in the probe"]
fn the_shared_add_on_a_replicated_noun_mirrors_it() {
    let mut n = off();
    // A pawn provider with #523's replicated arrays (section 10.2's shape, invented values).
    let (object, state) = (n.fx.c.object, n.state);
    let space = &mut n.fx.w.space;
    let d = |space: &mut ow_object::heap::ObjectSpace, name: &str, k: PropKind| PropertyDecl::new(space.names.intern(name), k);
    let fields = vec![d(space, "NounName", PropKind::Name), d(space, "Value", PropKind::Float), d(space, "bOnlyRelevantToOwner", PropKind::Bool)];
    let rep = space.classes.define_struct("GearboxFramework.INounAttributeProvider.ReplicatedNounAttributeState", None, fields).unwrap();
    let decls = vec![
        d(space, "Role", PropKind::Byte { enum_: None }),
        d(space, "bNetDirty", PropKind::Bool),
        d(space, "NounState", PropKind::Array(Box::new(PropKind::Struct(state)))),
        d(space, "ReplicatedNouns", PropKind::Struct(rep)).with_array_dim(20),
        d(space, "NumReplicatedNouns", PropKind::Int),
    ];
    let pawn = space.classes.define("WillowGame.WillowPawn", Some(object), decls).unwrap();
    let default = n.fx.k(10.0);
    let attr = n.attr("Attribute_R", default);
    n.set_resolver(attr, "ReplicationStrategy", Value::Byte(1));
    let ctx = n.fx.obj(pawn, "Provider", vec![("Role", Value::Byte(3)), ("NounState", Value::array(vec![]))]);
    let m = n.modifier(PRE, 1.0);
    assert!(n.add_native(attr, ctx, m).unwrap());
    let read = |n: &Noun, name: &str, i: u32| {
        let space = &n.fx.w.space;
        let p = space.prop(pawn, name).unwrap();
        space.read(ctx, PropAddr::element(p, i)).unwrap().clone()
    };
    let e = n.entries(ctx);
    assert_eq!((e.len(), e[0].slot, e[0].stack.clone()), (1, 0, vec![Some(m)]), "one entry, slot 0, the modifier on it");
    assert_eq!(read(&n, "NumReplicatedNouns", 0), Value::Int(1));
    let slot = read(&n, "ReplicatedNouns", 0);
    let space = &n.fx.w.space;
    assert_eq!(crate::eval::field(space, &slot, "NounName").unwrap(), &Value::Name(space.names.find("Attribute_R").unwrap()));
    assert_eq!(crate::eval::number("Value", crate::eval::field(space, &slot, "Value").unwrap()).unwrap(), e[0].value, "the slot mirrors the entry");
    assert_eq!(read(&n, "bNetDirty", 0), Value::Bool(true));
}

/// Test 1: a property (stack) attribute: the modifier joins the property's stack and the value is recomputed. The
/// change notification is the existing storage rule (`storage::notify`), not changed by this lane.
#[test]
fn the_shared_add_puts_a_property_modifier_on_its_stack() {
    let mut n = off();
    let weapon = n.fx.weapon(None, None);
    let space = &mut n.fx.w.space;
    let class = space.heap.get(weapon).unwrap().class;
    let base = space.prop(class, "InstantHitDamageBaseValue").unwrap();
    space.write(weapon, PropAddr::scalar(base), Value::Float(5.0)).unwrap();
    let attr = n.fx.property_attr("Attribute_D", weapon, "InstantHitDamage");
    let m = n.modifier(PRE, 2.0);
    assert!(n.add_native(attr, weapon, m).unwrap());
    let space = &n.fx.w.space;
    let stack = space.prop(class, "InstantHitDamageModifierStack").unwrap();
    assert_eq!(space.read(weapon, PropAddr::scalar(stack)).unwrap().elements().unwrap(), &[Value::Object(Some(m))]);
    let value = space.prop(class, "InstantHitDamage").unwrap();
    assert_eq!(space.read(weapon, PropAddr::scalar(value)).unwrap(), &Value::Float(7.0));
    // The change notification fires once (review r1 of #616, item 5) ...
    assert_eq!(n.fx.w.notified, 1, "notified");
    // ... and not with bSuppressNotify.
    let m2 = n.modifier(PRE, 1.0);
    assert!(n.add_native_with(attr, weapon, m2, true).unwrap());
    assert_eq!(n.fx.w.notified, 1, "suppressed");
}
