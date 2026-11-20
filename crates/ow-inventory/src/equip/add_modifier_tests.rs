//! Tests of the shared `AttributeDefinitionBase.AddAttributeModifier` wrapper (lane IMPL-ADD-ATTRIBUTE-MODIFIER,
//! NATIVE_ADD_ATTRIBUTE_MODIFIER.md section 3) through the VM, on two invented packages built with
//! `ow_package::testkit` (AGENTS.md rule 9). These are the cases that need a store with packages: the receiver and
//! argument checks, the None cases, and every unloaded-handle refusal (A1, A0, A2, an unloaded default). The rule's
//! other cases are `ow-attr`'s (`value_resolvers::noun_tests`). Every refusal checks #518's counters unchanged and the
//! handle still unloaded.

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_attr::add_native::UNLOADED;
use ow_core::{Missing, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_package::testkit::script::{FunctionSpec, Param, ScriptToy};
use ow_package::testkit::{Export, ints};
use ow_vm::api::{StubPolicy, Vm};
use ow_vm::error::VmError;
use ow_world::pools::Counters;

use super::*;

const NATIVE: u32 = 0x400 | 0x20000;
static NEXT: AtomicU32 = AtomicU32::new(0);

/// The invented `Engine`: `Object`, `Pawn`, `ToyMind` (a noun store `NounState` of `NounAttributeState`),
/// `AttributeModifier` (`Type`, `Value`), the definition classes (both chains, the native with the game's shape),
/// `ToyBadChainDefinition` (an int `ValueResolverChain`), `ToyIntChainDefinition` (an array-of-ints `ValueResolverChain`), `AttributeInitializationDefinition`,
/// `ObjectPropertyAttributeValueResolver` (`PropertyName`), and `ToyHolder_0`, whose fields name four exports the
/// store keeps as unloaded handles: a definition, a value resolver, a pawn and an initialization definition.
fn engine() -> Vec<u8> {
    let mut t = ScriptToy::new(&["Object", "Array", "Int", "Bool", "Float", "Name", "Byte", "Struct"]);
    let object = t.class("Object", 0);
    let pawn = t.class("Pawn", object);
    let mind = t.class("ToyMind", object);
    let state = {
        let mut e = Export::new(t.import("ScriptStruct"), "NounAttributeState", vec![0u8; 16]);
        e.outer = mind;
        t.p.add_export_with(e)
    };
    t.prop("Name", state, "NounName", 0, 0);
    t.prop("Float", state, "Value", 0, 0);
    t.prop("Float", state, "BaseValue", 0, 0);
    let stack = t.p.next_export();
    t.prop("Array", state, "ModifierStack", 0, stack + 1);
    t.prop("Object", stack, "ModifierStack", 0, object);
    t.prop("Byte", state, "ReplicationStrategy", 0, 0);
    t.prop("Int", state, "IndexInReplicationArray", 0, 0);
    let nouns = t.p.next_export();
    t.prop("Array", mind, "NounState", 0, nouns + 1);
    t.prop("Struct", nouns, "NounState", 0, state);
    let modifier = t.class("AttributeModifier", object);
    t.prop("Byte", modifier, "Type", 0, 0);
    t.prop("Float", modifier, "Value", 0, 0);
    let base = t.class("AttributeDefinitionBase", object);
    for chain in ["ContextResolverChain", "ValueResolverChain"] {
        let array = t.p.next_export();
        t.prop("Array", base, chain, 0, array + 1);
        t.prop("Object", array, chain, 0, object);
    }
    let params = [
        Param { type_ref: object, ..Param::new("Object", "Context") },
        Param { type_ref: modifier, ..Param::new("Object", "Modifier") },
        Param::new("Bool", "bSuppressNotify"),
    ];
    t.function(base, "AddAttributeModifier", &params, Some(("Bool", 0)), &[], &FunctionSpec::new(NATIVE, "AddAttributeModifier"), |_| None);
    let attribute = t.class("AttributeDefinition", base);
    let bad = t.class("ToyBadChainDefinition", base);
    t.prop("Int", bad, "ValueResolverChain", 0, 0);
    let int_chain = t.class("ToyIntChainDefinition", base);
    for chain in ["ContextResolverChain", "ValueResolverChain"] {
        let array = t.p.next_export();
        t.prop("Array", int_chain, chain, 0, array + 1);
        t.prop(if chain == "ValueResolverChain" { "Int" } else { "Object" }, array, chain, 0, if chain == "ValueResolverChain" { 0 } else { object });
    }
    let init = t.class("AttributeInitializationDefinition", object);
    let property = t.class("ObjectPropertyAttributeValueResolver", object);
    t.prop("Name", property, "PropertyName", 0, 0);
    t.class("ToyUnreadValueResolver", object);
    let holder = t.class("ToyHolder", object);
    let fields = [("HeldDef", attribute), ("HeldResolver", property), ("HeldPawn", pawn), ("HeldInit", init), ("HeldBroken", pawn)];
    for (f, class) in fields {
        t.prop("Object", holder, f, 0, class);
    }
    let held = t.p.next_export() + 1;
    let mut payload = ints(&[0]);
    for (i, (f, _)) in fields.iter().enumerate() {
        payload.extend(t.p.object_tag(f, held + i as i32));
    }
    payload.extend_from_slice(&t.none);
    t.p.add_export_with(Export::new(holder, "ToyHolder_0", payload));
    for (class, name) in [(attribute, "ToyLazyDefinition_0"), (property, "ToyLazyResolver_0"), (pawn, "ToyLazyPawn_0"), (init, "ToyLazyInit_0")] {
        let mut payload = ints(&[0]);
        payload.extend_from_slice(&t.none);
        t.p.add_export_with(Export::new(class, name, payload));
    }
    // A pawn export whose payload is cut short, so its load fails (review r2 of #616).
    t.p.add_export_with(Export::new(pawn, "ToyBrokenPawn_0", vec![1, 2]));
    t.p.build()
}

/// The invented `GearboxFramework`: `NounAttributeValueResolver` with its `DefaultValue` (an
/// `AttributeInitializationData`-shaped struct) and `ReplicationStrategy`.
fn gearbox() -> Vec<u8> {
    let mut t = ScriptToy::new(&["Object", "Float", "Byte", "Struct"]);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let object = t.p.add_import("Core", "Class", engine, "Object");
    let noun = t.class("NounAttributeValueResolver", object);
    let data = {
        let mut e = Export::new(t.import("ScriptStruct"), "ToyInitData", vec![0u8; 16]);
        e.outer = noun;
        t.p.add_export_with(e)
    };
    for f in ["BaseValueConstant", "BaseValueScaleConstant"] {
        t.prop("Float", data, f, 0, 0);
    }
    for f in ["BaseValueAttribute", "InitializationDefinition"] {
        t.prop("Object", data, f, 0, object);
    }
    t.prop("Struct", noun, "DefaultValue", 0, data);
    t.prop("Byte", noun, "ReplicationStrategy", 0, 0);
    t.p.build()
}

struct Fx {
    vm: Vm,
    dir: PathBuf,
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn fx(mode: ParityMode) -> Fx {
    let dir = std::env::temp_dir().join(format!("ow-inventory-add-modifier-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let (e, g) = (engine(), gearbox());
    let defs = DefStore::from_synthetic(&dir, &[("Engine.upk", e.as_slice()), ("GearboxFramework.upk", g.as_slice())], IdentityPolicy::FirstInScanOrder, mode)
        .unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::standard().unwrap()), mode)
        .with_stub_policy(StubPolicy::refuse_all());
    for c in ["Engine.AttributeDefinitionBase", "Engine.Pawn", "Engine.ToyMind", "GearboxFramework.NounAttributeValueResolver"] {
        vm.find_class(c).unwrap();
    }
    bind_add_modifier(&mut vm).unwrap();
    Fx { vm, dir }
}

impl Fx {
    fn obj(&mut self, class: &str) -> ObjRef {
        let c = self.vm.find_class(class).unwrap();
        ow_vm::interp::instantiate_from_defaults(&mut self.vm, c).unwrap()
    }
    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap();
        self.vm.space.write(o, PropAddr::scalar(p), v).unwrap();
    }
    fn read(&self, o: ObjRef, name: &str) -> Value {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap();
        self.vm.space.read(o, PropAddr::scalar(p)).unwrap().clone()
    }
    fn definition(&mut self, chain: &[Option<ObjRef>]) -> ObjRef {
        let d = self.obj("Engine.AttributeDefinition");
        self.set(d, "ValueResolverChain", Value::Array(Rc::new(chain.iter().map(|o| Value::Object(*o)).collect())));
        d
    }
    fn property_resolver(&mut self) -> ObjRef {
        let r = self.obj("Engine.ObjectPropertyAttributeValueResolver");
        let n = self.vm.space.names.intern("NoSuchAttribute");
        self.set(r, "PropertyName", Value::Name(n));
        r
    }
    fn modifier(&mut self) -> ObjRef {
        self.obj("Engine.AttributeModifier")
    }
    /// The four lazy handles: (definition, value resolver, pawn, initialization definition).
    fn lazy(&mut self) -> [ObjRef; 4] {
        let path = ow_core::ObjectPath::parse("Engine.ToyHolder_0").unwrap();
        let holder = self.vm.defs.find(&mut self.vm.space, &path).unwrap().unwrap();
        let out = ["HeldDef", "HeldResolver", "HeldPawn", "HeldInit"].map(|f| match self.read(holder, f) {
            Value::Object(Some(o)) => o,
            other => panic!("{f}: {other:?}"),
        });
        for o in out {
            assert!(self.vm.defs.is_unloaded(&self.vm.space, o), "lazy");
        }
        out
    }
    fn call(&mut self, this: Option<ObjRef>, args: Vec<Value>) -> Result<Value, VmError> {
        let f = self.vm.find_function(ADD_MODIFIER_FUNCTION)?;
        Ok(self.vm.call(f, this, args)?.ret)
    }
    fn add(&mut self, this: ObjRef, context: Option<ObjRef>, modifier: Option<ObjRef>) -> Result<Value, VmError> {
        self.call(Some(this), vec![Value::Object(context), Value::Object(modifier), Value::Bool(false)])
    }
}

fn not_read(r: Result<Value, VmError>) -> String {
    match r {
        Err(VmError::NotYet(n)) => {
            assert_eq!(n.missing, Missing::NotRead, "{n}");
            n.to_string()
        }
        other => panic!("expected a NotYet, got {other:?}"),
    }
}

fn script_text(r: Result<Value, VmError>) -> String {
    match r {
        Err(VmError::Script { message, .. }) => message,
        other => panic!("expected a script error, got {other:?}"),
    }
}

/// Test 3's None cases: a None modifier or a None context answers false at once, the definition not read, so an
/// unloaded definition does not refuse and stays unloaded. A non-bool flag is false, not an error.
#[test]
fn none_arguments_answer_false_before_the_definition_is_read() {
    let mut fx = fx(ParityMode::Off);
    let [lazy_def, ..] = fx.lazy();
    let pawn = fx.obj("Engine.Pawn");
    let m = fx.modifier();
    let before = Counters::read(&fx.vm);
    assert_eq!(fx.add(lazy_def, Some(pawn), None).unwrap(), Value::Bool(false));
    assert_eq!(fx.add(lazy_def, None, Some(m)).unwrap(), Value::Bool(false));
    assert_eq!(Counters::read(&fx.vm), before, "nothing read or written");
    assert!(fx.vm.defs.is_unloaded(&fx.vm.space, lazy_def));
    // A non-bool flag is not an error (an empty chain, so the answer is the chain's false).
    let d = fx.definition(&[]);
    let flag = fx.call(Some(d), vec![Value::Object(Some(pawn)), Value::Object(Some(m)), Value::Int(3)]);
    assert_eq!(flag.unwrap(), Value::Bool(false));
}

/// A1, A0 and A2, each before any write, the handle still unloaded; and the mixed chain (an unregistered loaded
/// resolver, then an unloaded one) gives A2.
#[test]
fn unloaded_definition_context_and_resolver_refuse_before_any_write() {
    let mut fx = fx(ParityMode::Off);
    let [lazy_def, lazy_resolver, lazy_pawn, _] = fx.lazy();
    let pawn = fx.obj("Engine.Pawn");
    let m = fx.modifier();
    let r = fx.property_resolver();
    let d = fx.definition(&[Some(r)]);
    let unread = fx.obj("Engine.ToyUnreadValueResolver");
    let mixed = fx.definition(&[Some(unread), Some(lazy_resolver)]);
    let only_lazy = fx.definition(&[Some(lazy_resolver)]);
    // A handle whose load already failed keeps its full path (review r2 of #616).
    let path = ow_core::ObjectPath::parse("Engine.ToyHolder_0").unwrap();
    let holder = fx.vm.defs.find(&mut fx.vm.space, &path).unwrap().unwrap();
    let Value::Object(Some(broken)) = fx.read(holder, "HeldBroken") else { panic!("the holder names the broken export") };
    assert!(fx.vm.defs.ensure_loaded(&mut fx.vm.space, broken).is_err(), "the cut payload fails to load");
    assert!(fx.vm.defs.is_failed(&fx.vm.space, broken) && fx.vm.defs.is_unloaded(&fx.vm.space, broken));
    let cases = [
        // The full object path, from the store's record (review r1 of #616).
        (lazy_def, Some(pawn), "the definition Engine.ToyLazyDefinition_0 ", lazy_def),
        (d, Some(lazy_pawn), "the context Engine.ToyLazyPawn_0 ", lazy_pawn),
        (only_lazy, Some(pawn), "the value resolver Engine.ToyLazyResolver_0 ", lazy_resolver),
        (mixed, Some(pawn), "the value resolver Engine.ToyLazyResolver_0 ", lazy_resolver),
        (d, Some(broken), "the context Engine.ToyBrokenPawn_0 ", broken),
    ];
    for (this, context, want, handle) in cases {
        let before = Counters::read(&fx.vm);
        let text = not_read(fx.add(this, context, Some(m)));
        assert!(text.contains(want) && text.contains(UNLOADED) && !text.contains("partial writes"), "{text}");
        assert_eq!(Counters::read(&fx.vm), before, "{want}: nothing counted");
        assert!(fx.vm.defs.is_unloaded(&fx.vm.space, handle), "{want}: still unloaded");
    }
}

/// Test 7: a missing noun entry whose default's `InitializationDefinition` is an unloaded handle is refused in the
/// load-free probe: nothing loaded, no entry made.
#[test]
fn an_unloaded_default_is_refused_in_the_probe_and_not_loaded() {
    let mut fx = fx(ParityMode::Off);
    let [.., lazy_init] = fx.lazy();
    let d = fx.obj("Engine.AttributeDefinition");
    let class = fx.vm.find_class("GearboxFramework.NounAttributeValueResolver").unwrap();
    let noun = fx.vm.space.alloc(class, "NounAttributeValueResolver_0", Some(d), Origin::Runtime).unwrap();
    let mut default = fx.read(noun, "DefaultValue");
    let ty = default.as_struct().unwrap().ty;
    let id = fx.vm.space.names.find("InitializationDefinition").unwrap();
    let (index, _) = fx.vm.space.classes.struct_layout(ty).and_then(|l| l.field(id)).unwrap();
    default.struct_mut().unwrap().fields[usize::try_from(index).unwrap()] = Value::Object(Some(lazy_init));
    fx.set(noun, "DefaultValue", default);
    fx.set(d, "ValueResolverChain", Value::Array(Rc::new(vec![Value::Object(Some(noun))])));
    let mind = fx.obj("Engine.ToyMind");
    let m = fx.modifier();
    let before = Counters::read(&fx.vm);
    let text = not_read(fx.add(d, Some(mind), Some(m)));
    assert!(text.contains("a default value's handle") && text.contains(UNLOADED), "{text}");
    assert_eq!(Counters::read(&fx.vm), before, "nothing counted");
    assert!(fx.vm.defs.is_unloaded(&fx.vm.space, lazy_init), "not loaded");
    assert_eq!(fx.read(mind, "NounState").elements().map(<[Value]>::len), Some(0), "no entry made");
}

/// Receiver, argument shapes and a malformed chain: script errors before anything is read or written.
#[test]
fn malformed_calls_are_script_errors() {
    let mut fx = fx(ParityMode::Off);
    let pawn = fx.obj("Engine.Pawn");
    let m = fx.modifier();
    let r = fx.property_resolver();
    let d = fx.definition(&[Some(r)]);
    let bad = fx.obj("Engine.ToyBadChainDefinition");
    // An empty array declared as ints: malformed by its declared type (review r1 of #616).
    let int_chain = fx.obj("Engine.ToyIntChainDefinition");
    // The argument-order ruling (review r2 of #616): a wrongly typed argument is a script error even when the other is
    // None, so neither (None, int) nor (int, None) answers false early.
    let cases: [(Option<ObjRef>, Vec<Value>, &str); 8] = [
        (Some(d), vec![Value::Object(None), Value::Int(1), Value::Bool(false)], "Modifier holds"),
        (Some(d), vec![Value::Int(1), Value::Object(None), Value::Bool(false)], "Context holds"),
        (None, vec![Value::Object(Some(pawn)), Value::Object(Some(m)), Value::Bool(false)], "without a definition"),
        (Some(pawn), vec![Value::Object(Some(pawn)), Value::Object(Some(m)), Value::Bool(false)], "not an Engine.AttributeDefinitionBase"),
        (Some(d), vec![Value::Int(1), Value::Object(Some(m)), Value::Bool(false)], "Context holds"),
        (Some(d), vec![Value::Object(Some(pawn)), Value::Int(1), Value::Bool(false)], "Modifier holds"),
        (Some(bad), vec![Value::Object(Some(pawn)), Value::Object(Some(m)), Value::Bool(false)], "ValueResolverChain is not declared"),
        (Some(int_chain), vec![Value::Object(Some(pawn)), Value::Object(Some(m)), Value::Bool(false)], "ValueResolverChain is not declared"),
    ];
    for (this, args, want) in cases {
        let before = Counters::read(&fx.vm);
        let text = script_text(fx.call(this, args));
        assert!(text.contains(want), "{text}");
        assert_eq!(Counters::read(&fx.vm), before, "{want}");
    }
}

/// Reference binds nothing.
#[test]
fn reference_binds_no_add_modifier() {
    let fx = fx(ParityMode::Reference);
    assert!(!fx.vm.has_native(ADD_MODIFIER));
}
