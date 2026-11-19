//! Invented package and values only (decision 2026-11-19-58).
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_balance::types::*;
use ow_core::{Missing, ParityMode};
use ow_defs::{DefStore, identity::IdentityPolicy};
use ow_object::heap::ObjectSpace;
use ow_package::testkit::script::{CPF_OPT, CPF_OUT, FunctionSpec, Param, ScriptToy};
use ow_vm::api::StubPolicy;
use ow_world::world::{BalanceState, World};

use super::*;
use crate::hit::access::set_required;

const NATIVE: u32 = 0x400 | 0x20000;
static NEXT: AtomicU32 = AtomicU32::new(0);

/// `Engine`: an invented `Pawn` with a plain float attribute `Toughness`, the two attribute classes with their chains and the native
/// `GetValue` with the game's parameter shape, `PawnAttributeContextResolver`, `ObjectPropertyAttributeValueResolver`
/// and `ToyUnreadResolver`, a context resolver class `ow-attr` does not implement.
fn package() -> Vec<u8> {
    let mut t = ScriptToy::new(&["Float", "Object", "Name", "Array"]);
    let object = t.class("Object", 0);
    let actor = t.class("Actor", object);
    let pawn = t.class("Pawn", actor);
    // A plain float attribute (the package flag bit 62), as the value resolver reads one.
    t.prop("Float", pawn, "Toughness", 1 << 62, 0);
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
    t.function(
        attribute,
        "GetValue",
        &[source, out, optional],
        Some(("Float", 0)),
        &[],
        &FunctionSpec::new(NATIVE, "GetValue"),
        |_| None,
    );
    t.class("PawnAttributeContextResolver", object);
    t.class("ToyUnreadResolver", object);
    let property = t.class("ObjectPropertyAttributeValueResolver", object);
    t.prop("Name", property, "PropertyName", 0, 0);
    t.p.build()
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
        let bytes = package();
        let defs =
            DefStore::from_synthetic(&dir, &[("Engine.upk", &bytes)], IdentityPolicy::FirstInScanOrder, mode).unwrap();
        let vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::standard().unwrap()), mode)
            .with_stub_policy(StubPolicy::refuse_all());
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
    fn make(&mut self, class: &str) -> ObjRef {
        let class = self.vm().find_class(&format!("Engine.{class}")).unwrap();
        ow_vm::interp::instantiate_from_defaults(self.vm(), class).unwrap()
    }
    fn set(&mut self, o: ObjRef, name: &str, value: Value) {
        set_required(self.vm(), o, name, value, "test").unwrap();
    }
    fn pawn(&mut self, toughness: f32) -> ObjRef {
        let p = self.make("Pawn");
        self.set(p, "Toughness", Value::Float(toughness));
        p
    }
    /// An attribute whose context chain is `context` and whose value chain reads `Toughness`.
    fn attribute(&mut self, context: &str) -> ObjRef {
        let resolver = self.make(context);
        let reader = self.make("ObjectPropertyAttributeValueResolver");
        let name = self.vm().space.names.intern("Toughness");
        self.set(reader, "PropertyName", Value::Name(name));
        let a = self.make("AttributeDefinition");
        self.set(a, "ContextResolverChain", Value::Array(Rc::new(vec![Value::Object(Some(resolver))])));
        self.set(a, "ValueResolverChain", Value::Array(Rc::new(vec![Value::Object(Some(reader))])));
        a
    }
    /// `GetValue(source, ResolvedContext, override)`: the result and the out context.
    fn get_value(&mut self, a: ObjRef, source: Value, over: Value) -> Result<(Value, Value), VmError> {
        let f = self.vm().find_function(GET_VALUE)?;
        let r = self.vm().call(f, Some(a), vec![source, Value::Object(None), over])?;
        Ok((r.ret, r.outs[1].clone()))
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

/// Off: a resolver class the library does not implement is the library's `NotYet { NotRead }`, passed through; an
/// argument that is not an object reference is an error. Reference binds nothing (the C++ has no such native).
#[test]
fn get_value_passes_the_librarys_not_yet_and_refuses_a_bad_argument() {
    let mut fx = Fx::new(ParityMode::Off);
    let a = fx.attribute("ToyUnreadResolver");
    let p = fx.pawn(1.0);
    match fx.get_value(a, Value::Object(Some(p)), Value::Object(None)) {
        Err(VmError::NotYet(n)) => assert_eq!(n.missing, Missing::NotRead, "{n}"),
        other => panic!("expected the library's NotYet, got {other:?}"),
    }
    let a = fx.attribute("PawnAttributeContextResolver");
    let e = fx.get_value(a, Value::Float(3.0), Value::Object(None));
    assert!(format!("{e:?}").contains("not an object reference"), "{e:?}");

    let mut fx = Fx::new(ParityMode::Reference);
    let f = fx.vm().find_function(GET_VALUE).unwrap();
    let key = fx.vm().function(f).unwrap().native_key.clone().unwrap();
    assert_eq!(key, "AttributeDefinition.GetValue(object,object,object)");
    assert!(!fx.vm().has_native(&key), "Reference binds nothing");
}
