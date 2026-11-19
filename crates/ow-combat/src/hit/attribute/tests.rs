//! Invented packages and values only (decision 2026-11-19-58).
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_attr::api::{AttrError, ValueResolver};
use ow_balance::types::*;
use ow_core::{Missing, ParityMode};
use ow_defs::{DefStore, identity::IdentityPolicy};
use ow_object::heap::ObjectSpace;
use ow_package::testkit::Export;
use ow_package::testkit::script::{Asm, CPF_OPT, CPF_OUT, FunctionSpec, Param, ScriptToy};
use ow_vm::api::StubPolicy;
use ow_world::world::{BalanceState, World};

use super::*;
use crate::hit::access::{get, set_field, set_required};

const NATIVE: u32 = 0x400 | 0x20000;
const SCRIPT: u32 = 0x1 | 0x2 | 0x20000;
static NEXT: AtomicU32 = AtomicU32::new(0);

/// `Engine`, invented:
/// - `Pawn`, with a plain float attribute `Toughness`, an object `LastContext` and an object `Attr`, a float `Seen`, and
///   a script function `Probe()` that runs `Seen = Attr.GetValue(Self, LastContext)`: the out argument aliases the
///   pawn's own `LastContext`;
/// - the two attribute classes with their chains, and the native `GetValue` with the game's parameter shape;
/// - `PawnAttributeContextResolver`, `ObjectPropertyAttributeValueResolver`, `ToyUnreadResolver` (a class `ow-attr`
///   does not implement) and `ToyProbeResolver` (a value resolver the tests register; [`Probe`]).
fn engine() -> Vec<u8> {
    let mut t = ScriptToy::new(&["Float", "Object", "Name", "Array"]);
    let object = t.class("Object", 0);
    let actor = t.class("Actor", object);
    let pawn = t.class("Pawn", actor);
    // A plain float attribute (the package flag bit 62), as the value resolver reads one.
    t.prop("Float", pawn, "Toughness", 1 << 62, 0);
    let last = t.prop("Object", pawn, "LastContext", 0, object);
    let attr_prop = t.prop("Object", pawn, "Attr", 0, object);
    let seen = t.prop("Float", pawn, "Seen", 0, 0);
    let base = t.class("AttributeDefinitionBase", object);
    for chain in ["ContextResolverChain", "ValueResolverChain"] {
        let array = t.p.next_export();
        t.prop("Array", base, chain, 0, array + 1);
        t.prop("Object", array, chain, 0, object);
    }
    let attribute = t.class("AttributeDefinition", base);
    // Object parameters carry their class (ScriptToy reads an untyped one as no parameter).
    let source = Param { type_ref: object, ..Param::new("Object", "ContextSource") };
    let out = Param { flags: CPF_OUT | CPF_OPT, type_ref: object, ..Param::new("Object", "ResolvedContext") };
    let optional =
        Param { flags: CPF_OPT, type_ref: object, ..Param::new("Object", "OptionalOverrideContextSource") };
    let get_value = t
        .function(
            attribute,
            "GetValue",
            &[source, out, optional],
            Some(("Float", 0)),
            &[],
            &FunctionSpec::new(NATIVE, "GetValue"),
            |_| None,
        )
        .function;
    t.function(pawn, "Probe", &[], None, &[], &FunctionSpec::new(SCRIPT, "Probe"), |_| {
        let mut a = Asm::new();
        // Seen = Attr.GetValue(Self, LastContext) with the override omitted (EmptyParmValue).
        a.stmt().raw(&[0x0F]).instance(seen).context(
            |a| {
                a.instance(attr_prop);
            },
            |a| {
                a.final_function(get_value, |a| {
                    a.self_().instance(last).raw(&[0x4A]);
                });
            },
        );
        a.stmt().return_nothing();
        a.end();
        Some(a)
    });
    t.class("PawnAttributeContextResolver", object);
    t.class("ToyUnreadResolver", object);
    t.class("ToyProbeResolver", object);
    let property = t.class("ObjectPropertyAttributeValueResolver", object);
    t.prop("Name", property, "PropertyName", 0, 0);
    t.p.build()
}

/// `GearboxFramework`, invented: a `RandomAttributeValueResolver` with the fields the library's random rule reads (the
/// integer flag, three init-data bounds and the `SavedRandomValues` cache), on toy struct types.
fn gearbox() -> Vec<u8> {
    let mut t = ScriptToy::new(&["Float", "Object", "Name", "Array", "Bool", "Struct"]);
    // Under the invented Engine's root class, so an Engine attribute's chain can hold it.
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let object = t.p.add_import("Core", "Class", engine, "Object");
    let random = t.class("RandomAttributeValueResolver", object);
    let strukt = |t: &mut ScriptToy, name: &str| {
        let mut e = Export::new(t.import("ScriptStruct"), name, vec![0u8; 16]);
        e.outer = random;
        t.p.add_export_with(e)
    };
    let data = strukt(&mut t, "ToyInitData");
    for f in ["BaseValueConstant", "BaseValueScaleConstant"] {
        t.prop("Float", data, f, 0, 0);
    }
    for f in ["BaseValueAttribute", "InitializationDefinition"] {
        t.prop("Object", data, f, 0, object);
    }
    let saved = strukt(&mut t, "ToySavedValue");
    t.prop("Name", saved, "ContextName", 0, 0);
    t.prop("Float", saved, "SavedValue", 0, 0);
    t.prop("Float", saved, "ExpirationTime", 0, 0);
    t.prop("Bool", random, "bUseIntegerRandomization", 0, 0);
    for f in ["LowerBound", "UpperBound", "ValueLifetime"] {
        t.prop("Struct", random, f, 0, data);
    }
    let array = t.p.next_export();
    t.prop("Array", random, "SavedRandomValues", 0, array + 1);
    t.prop("Struct", array, "SavedRandomValues", 0, saved);
    t.p.build()
}

/// `ToyProbeResolver`'s invented value: 1.0 when the context's `LastContext` already names the context, else 0.0. It
/// shows whether `ResolvedContext` was written before the value chain ran.
struct Probe;

impl ValueResolver for Probe {
    fn value(
        &self,
        _sys: &AttributeSystem,
        _resolver: ObjRef,
        context: ObjRef,
        _previous: f32,
        cx: &mut EvalCx<'_>,
    ) -> Result<f32, AttrError> {
        let space = cx.world.parts().space;
        let class = space.heap.get(context)?.class;
        let p = space.prop(class, "LastContext").ok_or(AttrError::BadData { reason: "no LastContext".into() })?;
        let v = space.read(context, ow_object::ids::PropAddr::scalar(p))?.clone();
        Ok(if v == Value::Object(Some(context)) { 1.0 } else { 0.0 })
    }
}

struct Fx {
    world: World,
    _dir: crate::run::tests::Dir,
}

impl Fx {
    fn new(mode: ParityMode) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "ow-attr-getvalue-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let (engine, gearbox) = (engine(), gearbox());
        let defs = DefStore::from_synthetic(
            &dir,
            &[("Engine.upk", &engine), ("GearboxFramework.upk", &gearbox)],
            IdentityPolicy::FirstInScanOrder,
            mode,
        )
        .unwrap();
        let attrs = AttributeSystem::standard().unwrap().with_value_resolver("Engine.ToyProbeResolver", Probe).unwrap();
        let vm = Vm::new(ObjectSpace::new(), defs, Rc::new(attrs), mode).with_stub_policy(StubPolicy::refuse_all());
        let snapshot = BalanceSnapshot::new(
            Playthrough::NORMAL,
            OpLevel::NONE,
            PlayerCount::SINGLE,
            ExpLevel::new(1),
            GameStage(1),
            AwesomeLevel::new(0),
            LevelCapUpgrades::ALL,
        )
        .unwrap();
        let world = World::new(vm, BalanceState::new(snapshot), &[&crate::hit::Pack]).unwrap();
        Self { world, _dir: crate::run::tests::Dir(dir) }
    }
    fn vm(&mut self) -> &mut Vm {
        &mut self.world.vm
    }
    fn make_in(&mut self, path: &str) -> ObjRef {
        let class = self.vm().find_class(path).unwrap();
        ow_vm::interp::instantiate_from_defaults(self.vm(), class).unwrap()
    }
    fn make(&mut self, class: &str) -> ObjRef {
        self.make_in(&format!("Engine.{class}"))
    }
    fn set(&mut self, o: ObjRef, name: &str, value: Value) {
        set_required(self.vm(), o, name, value, "test").unwrap();
    }
    fn value(&mut self, o: ObjRef, name: &str) -> Value {
        get(self.vm(), o, name).unwrap().unwrap()
    }
    fn pawn(&mut self, toughness: f32) -> ObjRef {
        let p = self.make("Pawn");
        self.set(p, "Toughness", Value::Float(toughness));
        p
    }
    fn reader(&mut self, property: &str) -> ObjRef {
        let reader = self.make("ObjectPropertyAttributeValueResolver");
        let name = self.vm().space.names.intern(property);
        self.set(reader, "PropertyName", Value::Name(name));
        reader
    }
    /// An attribute whose context chain is one `context` resolver and whose value chain is `values`.
    fn attribute_with(&mut self, context: &str, values: Vec<ObjRef>) -> ObjRef {
        let resolver = self.make(context);
        let a = self.make("AttributeDefinition");
        self.set(a, "ContextResolverChain", Value::Array(Rc::new(vec![Value::Object(Some(resolver))])));
        let chain = values.into_iter().map(|v| Value::Object(Some(v))).collect();
        self.set(a, "ValueResolverChain", Value::Array(Rc::new(chain)));
        a
    }
    /// An attribute whose context chain is `context` and whose value chain reads `Toughness`.
    fn attribute(&mut self, context: &str) -> ObjRef {
        let reader = self.reader("Toughness");
        self.attribute_with(context, vec![reader])
    }
    /// An invented random resolver: a float draw between 0 and 10, kept for 5 seconds.
    fn random(&mut self) -> ObjRef {
        let r = self.make_in("GearboxFramework.RandomAttributeValueResolver");
        for (field, constant) in [("LowerBound", 0.0), ("UpperBound", 10.0), ("ValueLifetime", 5.0)] {
            let vm = self.vm();
            let mut data = get(vm, r, field).unwrap().unwrap();
            set_field(vm, &mut data, "BaseValueConstant", Value::Float(constant));
            set_field(vm, &mut data, "BaseValueScaleConstant", Value::Float(1.0));
            self.set(r, field, data);
        }
        r
    }
    /// `GetValue(source, ResolvedContext, override)` from the host: the result and the out context.
    fn get_value(&mut self, a: ObjRef, source: Value, over: Value) -> Result<(Value, Value), VmError> {
        self.get_value_out(a, source, Value::Object(None), over)
    }
    fn get_value_out(&mut self, a: ObjRef, source: Value, out: Value, over: Value) -> Result<(Value, Value), VmError> {
        let f = self.vm().find_function(GET_VALUE)?;
        let r = self.vm().call(f, Some(a), vec![source, out, over])?;
        Ok((r.ret, r.outs[1].clone()))
    }
    /// The pawn's script `Probe()`: `Seen = Attr.GetValue(Self, LastContext)`.
    fn probe(&mut self, pawn: ObjRef) -> Result<(), VmError> {
        let f = self.vm().find_function("Engine.Pawn.Probe")?;
        self.vm().call(f, Some(pawn), vec![]).map(|_| ())
    }
}

fn not_read(r: Result<impl std::fmt::Debug, VmError>) -> String {
    match r {
        Err(VmError::NotYet(n)) => {
            assert_eq!(n.missing, Missing::NotRead, "{n}");
            n.to_string()
        }
        other => panic!("expected a NotYet, got {other:?}"),
    }
}

/// Off: the library's rule through the script call. A pawn resolves to itself and reads its `Toughness`, written to
/// `ResolvedContext`; a source that does not resolve gives 0.0 and a None context; the override is tried first and
/// the source is the silent fallback. Each answer equals `AttributeSystem::value_of` on the same arguments.
#[test]
fn get_value_runs_the_librarys_rule_and_writes_the_resolved_context() {
    let mut fx = Fx::new(ParityMode::Off);
    let a = fx.attribute("PawnAttributeContextResolver");
    let (p, q) = (fx.pawn(7.5), fx.pawn(2.25));
    let plain = fx.make("Object");
    let none = Value::Object(None);
    let cases = [
        (Some(p), None, 7.5, Some(p)),
        (Some(plain), None, 0.0, None),
        (None, None, 0.0, None),
        (Some(plain), Some(q), 2.25, Some(q)),
        (Some(p), Some(plain), 7.5, Some(p)),
        (Some(p), Some(q), 2.25, Some(q)),
    ];
    for (source, over, value, context) in cases {
        let (ret, out) = fx.get_value(a, Value::Object(source), over.map_or(none.clone(), |o| Value::Object(Some(o)))).unwrap();
        assert_eq!(ret, Value::Float(value), "{source:?} {over:?}");
        assert_eq!(out, Value::Object(context), "{source:?} {over:?}");
        let attrs = fx.vm().attrs.clone();
        let lib = attrs.value_of(a, source, over, &mut EvalCx::new(fx.vm())).unwrap();
        assert_eq!(lib.map(|v| (v.value, v.context)), context.map(|c| (value, c)), "the library agrees");
    }
}

/// Off: a resolver class the library does not implement is refused by the preflight, `NotYet { NotRead }` without the
/// terminal prefix; an argument that is not an object reference is an error. Reference binds nothing (the C++ has no
/// such native).
#[test]
fn get_value_passes_the_librarys_not_yet_and_refuses_a_bad_argument() {
    let mut fx = Fx::new(ParityMode::Off);
    let a = fx.attribute("ToyUnreadResolver");
    let p = fx.pawn(1.0);
    let text = not_read(fx.get_value(a, Value::Object(Some(p)), Value::Object(None)));
    assert!(!text.contains("stopped after partial writes"), "{text}");
    let a = fx.attribute("PawnAttributeContextResolver");
    let e = fx.get_value(a, Value::Float(3.0), Value::Object(None));
    assert!(format!("{e:?}").contains("not an object reference"), "{e:?}");

    let mut fx = Fx::new(ParityMode::Reference);
    let f = fx.vm().find_function(GET_VALUE).unwrap();
    let key = fx.vm().function(f).unwrap().native_key.clone().unwrap();
    assert_eq!(key, "AttributeDefinition.GetValue(object,object,object)");
    assert!(!fx.vm().has_native(&key), "Reference binds nothing");
}

/// Off (review r1 of #442, item 1): a random resolver followed by a resolver class the library does not implement is
/// refused before the random resolver runs. Called from script with the out argument aliasing the pawn's
/// `LastContext`: the attribute generator, the random resolver's `SavedRandomValues` and `LastContext` are untouched,
/// and the refusal carries no terminal prefix. The same random resolver alone does draw and save (so the check means
/// something).
#[test]
fn get_value_refuses_an_unread_chain_before_any_effect() {
    let mut fx = Fx::new(ParityMode::Off);
    let random = fx.random();
    let unread = fx.make("ToyUnreadResolver");
    let a = fx.attribute_with("PawnAttributeContextResolver", vec![random, unread]);
    let p = fx.pawn(1.0);
    let sentinel = fx.make("Object");
    fx.set(p, "Attr", Value::Object(Some(a)));
    fx.set(p, "LastContext", Value::Object(Some(sentinel)));
    let (rng, cache) = (fx.vm().rng.clone(), fx.value(random, "SavedRandomValues"));
    let text = not_read(fx.probe(p));
    assert!(text.contains("ToyUnreadResolver") && !text.contains("stopped after partial writes"), "{text}");
    assert_eq!(fx.vm().rng, rng, "no draw");
    assert_eq!(fx.value(random, "SavedRandomValues"), cache, "no saved value");
    assert_eq!(fx.value(p, "LastContext"), Value::Object(Some(sentinel)), "the out destination is untouched");
    assert_eq!(fx.value(p, "Seen"), Value::Float(0.0));
    // The random resolver alone draws and saves.
    let a = fx.attribute_with("PawnAttributeContextResolver", vec![random]);
    fx.set(p, "Attr", Value::Object(Some(a)));
    fx.probe(p).unwrap();
    assert_ne!(fx.vm().rng, rng, "a draw");
    assert_eq!(fx.value(random, "SavedRandomValues").elements().map(<[Value]>::len), Some(1), "a saved value");
    assert_eq!(fx.value(p, "LastContext"), Value::Object(Some(p)));
}

/// Off (item 2): the resolved context is written to `ResolvedContext` before the value chain runs. The out argument
/// aliases the pawn's `LastContext`, and the probe resolver answers 1.0 only when `LastContext` already names the
/// context. A refusal from the value chain after that write is terminal: its text starts "stopped after partial
/// writes", its kind is kept, and `LastContext` keeps the written context.
#[test]
fn get_value_writes_the_context_before_the_value_chain_and_a_later_refusal_is_terminal() {
    let mut fx = Fx::new(ParityMode::Off);
    let probe = fx.make("ToyProbeResolver");
    let a = fx.attribute_with("PawnAttributeContextResolver", vec![probe]);
    let p = fx.pawn(1.0);
    let sentinel = fx.make("Object");
    fx.set(p, "Attr", Value::Object(Some(a)));
    fx.set(p, "LastContext", Value::Object(Some(sentinel)));
    fx.probe(p).unwrap();
    assert_eq!(fx.value(p, "Seen"), Value::Float(1.0), "the value chain saw the written context");
    assert_eq!(fx.value(p, "LastContext"), Value::Object(Some(p)));
    // A reader of a property the pawn does not have: the library's NotYet, found only by evaluating.
    let missing = fx.reader("NoSuchProperty");
    let a = fx.attribute_with("PawnAttributeContextResolver", vec![missing]);
    fx.set(p, "Attr", Value::Object(Some(a)));
    fx.set(p, "LastContext", Value::Object(Some(sentinel)));
    let text = not_read(fx.probe(p));
    assert!(text.starts_with("stopped after partial writes: "), "{text}");
    assert_eq!(fx.value(p, "LastContext"), Value::Object(Some(p)), "the write is not put back");
}

/// Off (item 3): a supplied `ResolvedContext` that is not an object reference is a data error before anything is
/// evaluated: no draw.
#[test]
fn get_value_refuses_a_non_object_out_argument_before_evaluating() {
    let mut fx = Fx::new(ParityMode::Off);
    let random = fx.random();
    let a = fx.attribute_with("PawnAttributeContextResolver", vec![random]);
    let p = fx.pawn(1.0);
    let rng = fx.vm().rng.clone();
    let e = fx.get_value_out(a, Value::Object(Some(p)), Value::Float(2.0), Value::Object(None));
    assert!(format!("{e:?}").contains("ResolvedContext holds"), "{e:?}");
    assert_eq!(fx.vm().rng, rng, "no draw");
}
