//! Tests of the three `WillowInventory` natives (NATIVE_LOOT_SPAWN.md) on invented classes, objects and numbers
//! (AGENTS.md rule 9), in both parity modes. The toy currency enum's reserved names are invented; the three named
//! values and the 13-entry shape are the note's.

use std::rc::Rc;

use ow_attr::AttributeSystem;
use ow_attr::api::{AttrError, ContextResolver, EvalCx, ValueResolver};
use ow_core::notyet::Missing;
use ow_core::rand::{CrtStream, Rand15};
use ow_core::{ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ClassId, ObjRef, PropAddr, StructId};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_vm::api::{FunctionDecl, StubPolicy, Vm};
use ow_vm::error::VmError;

use super::*;

/// A generator that returns the given values in turn.
struct Seq(Vec<u16>, usize);
impl Rand15 for Seq {
    fn rand15(&mut self) -> u16 {
        self.1 += 1;
        self.0[self.1 - 1]
    }
}

const CURRENCIES: [&str; 14] = [
    "CURRENCY_Credits",
    "CURRENCY_Eridium",
    "CURRENCY_SeraphCrystals",
    "CURRENCY_ToyReserved0",
    "CURRENCY_ToyReserved1",
    "CURRENCY_ToyReserved2",
    "CURRENCY_ToyReserved3",
    "CURRENCY_ToyReserved4",
    "CURRENCY_ToyReserved5",
    "CURRENCY_ToyReserved6",
    "CURRENCY_ToyReserved7",
    "CURRENCY_ToyReserved8",
    "CURRENCY_ToyReserved9",
    "CURRENCY_MAX",
];

struct T {
    vm: Vm,
    aid: StructId,
    wdata: StructId,
    idata: StructId,
    definition: ClassId,
    attribute: ClassId,
    base: ClassId,
    weapon: ClassId,
    item: ClassId,
    /// Toy resolver classes: `Test.Same` (context = source), `Test.Worth` (30 on an inventory) and `Test.Unread`
    /// (registered nowhere).
    same: ClassId,
    worth: ClassId,
    unread: ClassId,
    next: u32,
}

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

/// A toy value resolver: 30 on a `WillowInventory` context, a data error on any other, so a test sees which
/// context the evaluation was given.
struct Worth(ClassId);
impl ValueResolver for Worth {
    fn value(
        &self,
        _: &AttributeSystem,
        _: ObjRef,
        context: ObjRef,
        _: f32,
        cx: &mut EvalCx<'_>,
    ) -> Result<f32, AttrError> {
        let space = cx.world.parts().space;
        let class = space.heap.get(context)?.class;
        if space.classes.is_child_of(class, self.0) {
            Ok(30.0)
        } else {
            Err(AttrError::BadData {
                reason: "Worth: the context is not an inventory".into(),
            })
        }
    }
}

fn decl(space: &mut ObjectSpace, name: &str, kind: PropKind) -> PropertyDecl {
    PropertyDecl::new(space.names.intern(name), kind)
}

fn world(mode: ParityMode) -> T {
    let mut space = ObjectSpace::new();
    let object = space.classes.define("Core.Object", None, vec![]).unwrap();
    let obj = PropKind::Object { class: object };
    let st = |space: &mut ObjectSpace, path: &str, fields: Vec<(&str, PropKind)>| {
        let d = fields.into_iter().map(|(n, k)| decl(space, n, k)).collect();
        space.classes.define_struct(path, None, d).unwrap()
    };
    let aid = st(
        &mut space,
        "Engine.AttributeInitializationDefinition.AttributeInitializationData",
        vec![
            ("BaseValueConstant", PropKind::Float),
            ("BaseValueAttribute", obj.clone()),
            ("InitializationDefinition", obj.clone()),
            ("BaseValueScaleConstant", PropKind::Float),
        ],
    );
    let objs = PropKind::Array(Box::new(obj.clone()));
    let chains = vec![
        decl(&mut space, "ContextResolverChain", objs.clone()),
        decl(&mut space, "ValueResolverChain", objs),
    ];
    let attribute = space
        .classes
        .define("Engine.AttributeDefinition", Some(object), chains)
        .unwrap();
    let same = space.classes.define("Test.Same", Some(object), vec![]).unwrap();
    let worth = space.classes.define("Test.Worth", Some(object), vec![]).unwrap();
    let unread = space.classes.define("Test.Unread", Some(object), vec![]).unwrap();
    let names = CURRENCIES.iter().map(|n| space.names.intern(n)).collect();
    let currency = space
        .classes
        .define_enum("WillowGame.ECurrencyType", names)
        .unwrap();
    let d = vec![
        decl(&mut space, "MonetaryValue", PropKind::Struct(aid)),
        decl(
            &mut space,
            "FormOfCurrency",
            PropKind::Byte {
                enum_: Some(currency),
            },
        ),
    ];
    let definition = space
        .classes
        .define("WillowGame.WillowInventoryDefinition", Some(object), d)
        .unwrap();
    let wdata = st(
        &mut space,
        "WillowGame.WillowWeaponTypes.WeaponDefinitionData",
        vec![
            ("WeaponTypeDefinition", obj.clone()),
            ("ManufacturerDefinition", obj.clone()),
        ],
    );
    let idata = st(
        &mut space,
        "WillowGame.WillowItemTypes.ItemDefinitionData",
        vec![
            ("ItemDefinition", obj.clone()),
            ("ManufacturerDefinition", obj.clone()),
        ],
    );
    let base = space
        .classes
        .define("Engine.WillowInventory", Some(object), vec![])
        .unwrap();
    let d = vec![decl(&mut space, "DefinitionData", PropKind::Struct(wdata))];
    let weapon = space
        .classes
        .define("WillowGame.WillowWeapon", Some(base), d)
        .unwrap();
    let d = vec![decl(&mut space, "DefinitionData", PropKind::Struct(idata))];
    let item = space
        .classes
        .define("WillowGame.WillowItem", Some(base), d)
        .unwrap();
    let attrs = AttributeSystem::new_empty()
        .with_context_resolver("Test.Same", Same)
        .unwrap()
        .with_value_resolver("Test.Worth", Worth(base))
        .unwrap();
    let mut vm = Vm::new(space, DefStore::new_empty(), Rc::new(attrs), mode);
    vm.stub_policy = StubPolicy::refuse_all();
    for (path, result) in [
        (GENERATE_UNIQUE_ID, PropKind::Int),
        (COMPUTE_MONETARY_VALUE, PropKind::Int),
        (GET_MANUFACTURER, obj.clone()),
    ] {
        let key = format!("WillowInventory.{}()", path.rsplit('.').next().unwrap());
        let mut f = FunctionDecl::native(ObjectPath::parse(path).unwrap(), key).with_owner(base);
        f.result =
            Some(decl(&mut vm.space, "ReturnValue", result).with_flags(PropertyDecl::RETURN));
        vm.declare_function(f).unwrap();
    }
    register(&mut vm).unwrap();
    T {
        vm,
        aid,
        wdata,
        idata,
        definition,
        attribute,
        base,
        weapon,
        item,
        same,
        worth,
        unread,
        next: 0,
    }
}

impl T {
    fn obj(&mut self, class: ClassId) -> ObjRef {
        self.next += 1;
        self.vm
            .space
            .alloc(class, &format!("O{}", self.next), None, Origin::Runtime)
            .unwrap()
    }

    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap();
        self.vm.space.write(o, PropAddr::scalar(p), v).unwrap();
    }

    /// A definition worth `value` (times `scale`), in currency `form`, optionally reading an attribute.
    fn definition(&mut self, value: f32, scale: f32, form: u8, attribute: bool) -> ObjRef {
        let d = self.obj(self.definition);
        let a = if attribute {
            Some(self.obj(self.attribute))
        } else {
            None
        };
        let init = Value::structure(
            self.aid,
            vec![
                Value::Float(value),
                Value::Object(a),
                Value::Object(None),
                Value::Float(scale),
            ],
        );
        self.set(d, "MonetaryValue", init);
        self.set(d, "FormOfCurrency", Value::Byte(form));
        d
    }

    /// The `BaseValueAttribute` of a definition made with `attribute: true`.
    fn attribute_of(&mut self, d: ObjRef) -> ObjRef {
        let class = self.vm.space.heap.get(d).unwrap().class;
        let p = self.vm.space.prop(class, "MonetaryValue").unwrap();
        let v = self.vm.space.read(d, PropAddr::scalar(p)).unwrap();
        match ow_attr::eval::init_data(&self.vm.space, &v).unwrap().base_value_attribute {
            Some(a) => a,
            None => panic!("a definition without an attribute"),
        }
    }

    fn weapon(&mut self, definition: Option<ObjRef>, manufacturer: Option<ObjRef>) -> ObjRef {
        let w = self.obj(self.weapon);
        let data = Value::structure(
            self.wdata,
            vec![Value::Object(definition), Value::Object(manufacturer)],
        );
        self.set(w, "DefinitionData", data);
        w
    }

    fn item(&mut self, definition: Option<ObjRef>, manufacturer: Option<ObjRef>) -> ObjRef {
        let i = self.obj(self.item);
        let data = Value::structure(
            self.idata,
            vec![Value::Object(definition), Value::Object(manufacturer)],
        );
        self.set(i, "DefinitionData", data);
        i
    }

    fn call(&mut self, path: &str, this: ObjRef) -> Result<Value, VmError> {
        let f = self.vm.find_function(path)?;
        Ok(self.vm.call(f, Some(this), vec![])?.ret)
    }

    fn value_of(&mut self, value: f32, scale: f32, form: u8) -> Result<Value, VmError> {
        let d = self.definition(value, scale, form, false);
        let w = self.weapon(Some(d), None);
        self.call(COMPUTE_MONETARY_VALUE, w)
    }
}

fn not_read(r: Result<Value, VmError>) -> String {
    match r {
        Err(VmError::NotYet(n)) => {
            assert_eq!(n.missing, Missing::NotRead);
            n.to_string()
        }
        other => panic!("expected NotYet, got {other:?}"),
    }
}

#[test]
fn the_unique_id_is_two_draws_low_half_first_and_never_zero() {
    assert_eq!(unique_id(&mut Seq(vec![0x1234, 0x0005], 0)), 0x0005_1234);
    assert_eq!(unique_id(&mut Seq(vec![0, 0], 0)), 1, "0 becomes 1");
    assert_eq!(
        unique_id(&mut Seq(vec![0, 1], 0)),
        0x1_0000,
        "only 0 is changed"
    );
    let top = unique_id(&mut Seq(vec![32767, 32767], 0));
    assert_eq!(top, 0x7FFF_7FFF);
    assert_eq!(top & (1 << 15 | 1 << 31), 0, "bits 15 and 31 stay clear");
}

#[test]
fn generate_unique_id_draws_two_from_the_loot_stream_only_in_off() {
    let mut t = world(ParityMode::Off);
    let w = t.weapon(None, None);
    let mut expect = t.vm.rng.crt(ID_STREAM).clone();
    let other = t.vm.rng.crt(CrtStream::AttributeVariance).clone();
    for _ in 0..3 {
        let want = unique_id(&mut expect);
        assert_eq!(t.call(GENERATE_UNIQUE_ID, w).unwrap(), Value::Int(want));
    }
    assert_eq!(
        *t.vm.rng.crt(ID_STREAM),
        expect,
        "two draws a call, from LootPool"
    );
    assert_eq!(
        *t.vm.rng.crt(CrtStream::AttributeVariance),
        other,
        "no other stream moves"
    );
    assert_eq!(ID_STREAM, CrtStream::LootPool);
}

#[test]
fn the_monetary_value_truncates_then_caps_by_currency() {
    let mut t = world(ParityMode::Off);
    assert_eq!(
        t.value_of(12.9, 2.0, 0).unwrap(),
        Value::Int(25),
        "12.9 x 2 = 25.8, toward zero"
    );
    assert_eq!(
        t.value_of(-3.7, 1.0, 0).unwrap(),
        Value::Int(-3),
        "negative: toward zero, not raised"
    );
    assert_eq!(t.value_of(1.0e9, 1.0, 0).unwrap(), Value::Int(CREDITS_CAP));
    assert_eq!(t.value_of(600.0, 1.0, 1).unwrap(), Value::Int(ERIDIUM_CAP));
    assert_eq!(
        t.value_of(499.0, 1.0, 1).unwrap(),
        Value::Int(499),
        "under the cap"
    );
    assert_eq!(
        t.value_of(1000.0, 1.0, 2).unwrap(),
        Value::Int(OTHER_CAP),
        "Seraph crystals"
    );
    for reserved in 3..13 {
        assert_eq!(
            t.value_of(2000.0, 1.0, reserved).unwrap(),
            Value::Int(OTHER_CAP),
            "reserved {reserved}"
        );
    }
    assert_eq!(
        t.value_of(2000.0, 1.0, 14).unwrap(),
        Value::Int(0),
        "past the enum's last entry"
    );
    let text = not_read(t.value_of(5.0, 1.0, 13));
    assert!(text.contains("_MAX"), "{text}");
    // An item reads its ItemDefinition the same way.
    let d = t.definition(42.5, 1.0, 0, false);
    let i = t.item(Some(d), None);
    assert_eq!(t.call(COMPUTE_MONETARY_VALUE, i).unwrap(), Value::Int(42));
}

#[test]
fn what_the_note_leaves_open_is_not_yet_and_no_definition_is_zero() {
    let mut t = world(ParityMode::Off);
    let w = t.weapon(None, None);
    assert_eq!(
        t.call(COMPUTE_MONETARY_VALUE, w).unwrap(),
        Value::Int(0),
        "no definition: 0"
    );
    // An attribute the evaluator cannot read answers the evaluator's own NotYet (decision 2026-11-19-45).
    let d = t.definition(5.0, 1.0, 0, true);
    let a = t.attribute_of(d);
    let unread = t.obj(t.unread);
    t.set(a, "ContextResolverChain", Value::array(vec![Value::Object(Some(unread))]));
    let w = t.weapon(Some(d), None);
    let text = not_read(t.call(COMPUTE_MONETARY_VALUE, w));
    assert!(text.contains("context resolver class Test.Unread"), "{text}");
    let text = not_read(t.value_of(f32::NAN, 1.0, 0));
    assert!(text.contains("int range"), "{text}");
    let text = not_read(t.value_of(3.0e9, 1.0, 0));
    assert!(text.contains("int range"), "{text}");
    let bare = t.obj(t.base);
    let text = not_read(t.call(GET_MANUFACTURER, bare));
    assert!(text.contains("no DefinitionData"), "{text}");
    let text = not_read(t.call(COMPUTE_MONETARY_VALUE, bare));
    assert!(text.contains("no DefinitionData"), "{text}");
}

#[test]
fn an_enum_unlike_the_notes_is_not_yet() {
    let names =
        |n: usize| -> Vec<String> { CURRENCIES[..n].iter().map(|s| s.to_string()).collect() };
    assert_eq!(
        currency_cap(&names(13), 1),
        Ok(Some(ERIDIUM_CAP)),
        "13 entries, no _MAX"
    );
    assert_eq!(currency_cap(&names(13), 13), Ok(None));
    assert!(currency_cap(&names(12), 0).is_err(), "12 entries");
    let mut swapped = names(14);
    swapped.swap(0, 1);
    assert!(currency_cap(&swapped, 0).is_err(), "credits not first");
}

#[test]
fn get_manufacturer_reads_the_definition_data_as_it_is() {
    let mut t = world(ParityMode::Off);
    let m = t.obj(t.definition);
    let w = t.weapon(None, Some(m));
    assert_eq!(t.call(GET_MANUFACTURER, w).unwrap(), Value::Object(Some(m)));
    let i = t.item(None, Some(m));
    assert_eq!(t.call(GET_MANUFACTURER, i).unwrap(), Value::Object(Some(m)));
    let none = t.item(None, None);
    assert_eq!(t.call(GET_MANUFACTURER, none).unwrap(), Value::Object(None));
}

#[test]
fn reference_leaves_the_three_unbound() {
    let mut t = world(ParityMode::Reference);
    let w = t.weapon(None, None);
    let before = t.vm.rng.crt(ID_STREAM).clone();
    for path in [GENERATE_UNIQUE_ID, COMPUTE_MONETARY_VALUE, GET_MANUFACTURER] {
        match t.call(path, w) {
            Err(VmError::UnboundNative(key)) => {
                assert!(key.starts_with("WillowInventory."), "{key}")
            }
            other => panic!("{path}: expected UnboundNative, got {other:?}"),
        }
    }
    assert_eq!(*t.vm.rng.crt(ID_STREAM), before, "nothing drawn");
}

#[test]
fn an_attribute_value_is_evaluated_with_the_inventory_as_context() {
    // NATIVE_INVENTORY_VALUE.md section 3 (decision 2026-11-19-45): the base attribute's chains run on the inventory.
    let mut t = world(ParityMode::Off);
    let d = t.definition(5.0, 2.0, 0, true);
    let a = t.attribute_of(d);
    let (same, worth) = (t.obj(t.same), t.obj(t.worth));
    t.set(a, "ContextResolverChain", Value::array(vec![Value::Object(Some(same))]));
    t.set(a, "ValueResolverChain", Value::array(vec![Value::Object(Some(worth))]));
    let w = t.weapon(Some(d), None);
    assert_eq!(t.call(COMPUTE_MONETARY_VALUE, w).unwrap(), Value::Int(60), "30 x 2 on the weapon");
    let i = t.item(Some(d), None);
    assert_eq!(t.call(COMPUTE_MONETARY_VALUE, i).unwrap(), Value::Int(60), "and on an item");
    // An attribute whose context chain is empty resolves no context: the constant stays the base (5 x 2).
    let d = t.definition(5.0, 2.0, 0, true);
    let w = t.weapon(Some(d), None);
    assert_eq!(t.call(COMPUTE_MONETARY_VALUE, w).unwrap(), Value::Int(10));
}
