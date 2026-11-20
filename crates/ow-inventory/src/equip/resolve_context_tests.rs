//! Tests of the shared `AttributeDefinitionBase.ResolveContext` rule (lane IMPL-RESOLVE-CONTEXT, NATIVE_RESOLVE_CONTEXT.md)
//! on two invented packages built with `ow_package::testkit` (AGENTS.md rule 9). Every value is invented. Every
//! refusal before the walk also checks #518's counters (heap committed writes, store load transitions, generator draws)
//! unchanged.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_attr::api::{AttrError, ContextResolver, EngineQuery, WorldParts};
use ow_core::{Missing, NotYet, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_package::testkit::{Export, ints};
use ow_package::testkit::script::{FunctionSpec, Param, ScriptToy};
use ow_vm::api::{StubPolicy, Vm, WorldHooks};
use ow_vm::error::VmError;
use ow_world::pools::Counters;

use super::*;
use ow_attr::resolve_native::UNLOADED;

const NATIVE: u32 = 0x400 | 0x20000;
static NEXT: AtomicU32 = AtomicU32::new(0);

/// The invented `Engine`:
/// - `Object`, `Actor`, `Pawn`, `Controller` (with `Pawn`), and `ToyMarker` (an object with a `Touched` int);
/// - `AttributeDefinitionBase` (its `ContextResolverChain`, and the native `ResolveContext` with the game's shape),
///   `AttributeDefinition` and `AttributeDefinitionMultiContext`;
/// - `ToyBadChainDefinition`, a child of the base whose `ContextResolverChain` is an int (malformed);
/// - `PawnAttributeContextResolver`, `ToyUnreadResolver` (a class `ow-attr` does not register), and two resolvers the
///   tests register: `ToyWritingResolver` (writes, then answers its source) and `ToyRefusingResolver` (a NotYet);
/// - `ToyHolder_0`, whose `HeldDef` and `HeldResolver` name an `AttributeDefinition` export and a Pawn resolver
///   export, which the store keeps as unloaded handles.
fn engine() -> Vec<u8> {
    let mut t = ScriptToy::new(&["Object", "Array", "Int"]);
    let object = t.class("Object", 0);
    let actor = t.class("Actor", object);
    let pawn = t.class("Pawn", actor);
    let controller = t.class("Controller", actor);
    t.prop("Object", controller, "Pawn", 0, pawn);
    let marker = t.class("ToyMarker", object);
    t.prop("Int", marker, "Touched", 0, 0);
    let base = t.class("AttributeDefinitionBase", object);
    let array = t.p.next_export();
    t.prop("Array", base, "ContextResolverChain", 0, array + 1);
    t.prop("Object", array, "ContextResolverChain", 0, object);
    let source = Param { type_ref: object, ..Param::new("Object", "ContextSource") };
    t.function(base, "ResolveContext", &[source], Some(("Object", object)), &[], &FunctionSpec::new(NATIVE, "ResolveContext"), |_| None);
    let attribute = t.class("AttributeDefinition", base);
    t.class("AttributeDefinitionMultiContext", base);
    let bad = t.class("ToyBadChainDefinition", base);
    t.prop("Int", bad, "ContextResolverChain", 0, 0);
    let pawn_resolver = t.class("PawnAttributeContextResolver", object);
    t.class("ToyUnreadResolver", object);
    t.class("ToyWritingResolver", object);
    t.class("ToyRefusingResolver", object);
    let holder = t.class("ToyHolder", object);
    t.prop("Object", holder, "HeldDef", 0, attribute);
    t.prop("Object", holder, "HeldResolver", 0, object);
    let held = t.p.next_export() + 1;
    let mut payload = ints(&[0]);
    payload.extend(t.p.object_tag("HeldDef", held));
    payload.extend(t.p.object_tag("HeldResolver", held + 1));
    payload.extend_from_slice(&t.none);
    t.p.add_export_with(Export::new(holder, "ToyHolder_0", payload));
    for (class, name) in [(attribute, "ToyLazyDefinition_0"), (pawn_resolver, "ToyLazyResolver_0")] {
        let mut payload = ints(&[0]);
        payload.extend_from_slice(&t.none);
        t.p.add_export_with(Export::new(class, name, payload));
    }
    t.p.build()
}

/// The invented `GearboxFramework`: `NoContextNeededAttributeContextResolver`, and `FlagDefinition` with its own
/// `ContextResolverChain`, outside `AttributeDefinitionBase` (as in the installed packages).
fn gearbox() -> Vec<u8> {
    let mut t = ScriptToy::new(&["Object", "Array"]);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let object = t.p.add_import("Core", "Class", engine, "Object");
    t.class("NoContextNeededAttributeContextResolver", object);
    let flag = t.class("FlagDefinition", object);
    let array = t.p.next_export();
    t.prop("Array", flag, "ContextResolverChain", 0, array + 1);
    t.prop("Object", array, "ContextResolverChain", 0, object);
    t.p.build()
}

/// Writes one invented int on the context's `ToyMarker` (a write mid-walk), then answers its source.
struct Writing;

impl ContextResolver for Writing {
    fn resolve(
        &self,
        _sys: &AttributeSystem,
        _resolver: ObjRef,
        _attr: ObjRef,
        source: Option<ObjRef>,
        cx: &mut ow_attr::api::EvalCx<'_>,
    ) -> Result<Option<ObjRef>, AttrError> {
        let parts = cx.world.parts();
        let space = parts.space;
        let marker = space.classes.by_path("Engine.ToyMarker").expect("marker class");
        let m = space.alloc(marker, "ToyMarker_written", None, Origin::Runtime)?;
        let p = space.prop(marker, "Touched").expect("Touched");
        space.write(m, PropAddr::scalar(p), Value::Int(7))?;
        Ok(source)
    }
}

/// A resolver that refuses: `NotYet { NotRead }`.
struct Refusing;

impl ContextResolver for Refusing {
    fn resolve(
        &self,
        _sys: &AttributeSystem,
        _resolver: ObjRef,
        _attr: ObjRef,
        _source: Option<ObjRef>,
        _cx: &mut ow_attr::api::EvalCx<'_>,
    ) -> Result<Option<ObjRef>, AttrError> {
        Err(NotYet::not_read("an invented refusal mid-walk", "toy").into())
    }
}

/// World hooks with one invented globals object.
struct Hooks {
    globals: ObjRef,
}

impl WorldHooks for Hooks {
    fn globals(&mut self, _w: WorldParts<'_>) -> Result<ObjRef, AttrError> {
        Ok(self.globals)
    }
    fn game_info(&mut self, _w: WorldParts<'_>) -> Result<ObjRef, AttrError> {
        Err(AttrError::BadData { reason: "no game info in this fixture".into() })
    }
    fn engine_query(&mut self, _q: EngineQuery, _source: ObjRef, _w: WorldParts<'_>) -> Result<Option<ObjRef>, AttrError> {
        Err(AttrError::BadData { reason: "no engine queries in this fixture".into() })
    }
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
    fx_with(mode, true)
}

/// [`fx`], with the shared rule bound or not.
fn fx_with(mode: ParityMode, bind: bool) -> Fx {
    let dir = std::env::temp_dir().join(format!("ow-inventory-resolve-context-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let (e, g) = (engine(), gearbox());
    let defs = DefStore::from_synthetic(&dir, &[("Engine.upk", e.as_slice()), ("GearboxFramework.upk", g.as_slice())], IdentityPolicy::FirstInScanOrder, mode)
        .unwrap();
    let attrs = AttributeSystem::standard()
        .unwrap()
        .with_context_resolver("Engine.ToyWritingResolver", Writing)
        .unwrap()
        .with_context_resolver("Engine.ToyRefusingResolver", Refusing)
        .unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(attrs), mode).with_stub_policy(StubPolicy::refuse_all());
    for c in ["Engine.AttributeDefinitionBase", "Engine.Pawn", "Engine.Controller", "Engine.ToyMarker", "GearboxFramework.FlagDefinition"] {
        vm.find_class(c).unwrap();
    }
    let object = vm.find_class("Engine.Object").unwrap();
    let globals = vm.space.alloc(object, "ToyGlobals", None, Origin::Runtime).unwrap();
    vm.set_world_hooks(Rc::new(RefCell::new(Hooks { globals }))).unwrap();
    if bind {
        bind_resolve_context(&mut vm).unwrap();
    }
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
    /// A definition of `class` whose chain is `chain` (None entries allowed).
    fn definition_of(&mut self, class: &str, chain: &[Option<ObjRef>]) -> ObjRef {
        let d = self.obj(class);
        let items = chain.iter().map(|o| Value::Object(*o)).collect();
        self.set(d, "ContextResolverChain", Value::Array(Rc::new(items)));
        d
    }
    fn definition(&mut self, chain: &[Option<ObjRef>]) -> ObjRef {
        self.definition_of("Engine.AttributeDefinition", chain)
    }
    fn resolver(&mut self, class: &str) -> ObjRef {
        self.obj(&format!("Engine.{class}"))
    }
    /// The native through the VM (the script call's path), on `this` with `source`.
    fn call(&mut self, this: ObjRef, source: Value) -> Result<Value, VmError> {
        self.call_on(Some(this), source)
    }
    /// [`Fx::call`] with any receiver, None included.
    fn call_on(&mut self, this: Option<ObjRef>, source: Value) -> Result<Value, VmError> {
        let f = self.vm.find_function(RESOLVE_CONTEXT_FUNCTION)?;
        Ok(self.vm.call(f, this, vec![source])?.ret)
    }
    /// The holder's two lazy handles: the definition and the resolver.
    fn lazy(&mut self) -> (ObjRef, ObjRef) {
        let path = ow_core::ObjectPath::parse("Engine.ToyHolder_0").unwrap();
        let holder = self.vm.defs.find(&mut self.vm.space, &path).unwrap().unwrap();
        let (Value::Object(Some(d)), Value::Object(Some(r))) = (self.read(holder, "HeldDef"), self.read(holder, "HeldResolver")) else {
            panic!("the holder names both exports")
        };
        assert!(self.vm.defs.is_unloaded(&self.vm.space, d) && self.vm.defs.is_unloaded(&self.vm.space, r), "both lazy");
        (d, r)
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

fn script_text(r: Result<impl std::fmt::Debug, VmError>) -> String {
    match r {
        Err(VmError::Script { message, .. }) => message,
        other => panic!("expected a script error, got {other:?}"),
    }
}

/// The note's tests 1 to 5: the walk through the bound native.
#[test]
fn the_walk_answers_as_the_note_reads_it() {
    let mut fx = fx(ParityMode::Off);
    let pawn = fx.obj("Engine.Pawn");
    let controller = fx.obj("Engine.Controller");
    fx.set(controller, "Pawn", Value::Object(Some(pawn)));
    let p = fx.resolver("PawnAttributeContextResolver");
    let d = fx.definition(&[Some(p)]);
    // 1 and 2: a pawn gives itself, a controller its Pawn.
    assert_eq!(fx.call(d, Value::Object(Some(pawn))).unwrap(), Value::Object(Some(pawn)));
    assert_eq!(fx.call(d, Value::Object(Some(controller))).unwrap(), Value::Object(Some(pawn)));
    // 3: a second resolver that answers None (the Pawn resolver on a plain object) ends the walk with None.
    let plain = fx.obj("Engine.ToyMarker");
    let w = fx.resolver("ToyWritingResolver");
    let d3 = fx.definition(&[Some(w), Some(p)]);
    assert_eq!(fx.call(d3, Value::Object(Some(plain))).unwrap(), Value::Object(None));
    // 4: an empty chain and a chain of None entries give None, even with a source.
    let empty = fx.definition(&[]);
    let nones = fx.definition(&[None, None]);
    assert_eq!(fx.call(empty, Value::Object(Some(pawn))).unwrap(), Value::Object(None));
    assert_eq!(fx.call(nones, Value::Object(Some(pawn))).unwrap(), Value::Object(None));
    // 5: a None entry after an answering resolver leaves that answer.
    let d5 = fx.definition(&[Some(p), None]);
    assert_eq!(fx.call(d5, Value::Object(Some(pawn))).unwrap(), Value::Object(Some(pawn)));
}

/// Hub ruling 4: a None source. The Pawn resolver gives None; NoContextNeeded gives the engine globals object.
#[test]
fn a_none_source_follows_each_resolver_s_rule() {
    let mut fx = fx(ParityMode::Off);
    let p = fx.resolver("PawnAttributeContextResolver");
    let d = fx.definition(&[Some(p)]);
    assert_eq!(fx.call(d, Value::Object(None)).unwrap(), Value::Object(None));
    let n = fx.obj("GearboxFramework.NoContextNeededAttributeContextResolver");
    let d = fx.definition(&[Some(n)]);
    let globals = match fx.call(d, Value::Object(None)).unwrap() {
        Value::Object(Some(g)) => g,
        other => panic!("{other:?}"),
    };
    assert_eq!(fx.vm.space.names.text(fx.vm.space.heap.get(globals).unwrap().name), "ToyGlobals");
}

/// Hub ruling 1: once the walk starts, a NotYet is terminal, prefixed once, its kind kept. A writing step (a stand-in
/// for a scripted resolver) and then a refusing step: the prefixed refusal, and the write stays.
#[test]
fn a_not_yet_inside_the_walk_is_terminal() {
    let mut fx = fx(ParityMode::Off);
    let pawn = fx.obj("Engine.Pawn");
    let (w, r) = (fx.resolver("ToyWritingResolver"), fx.resolver("ToyRefusingResolver"));
    let d = fx.definition(&[Some(w), Some(r)]);
    let before = Counters::read(&fx.vm);
    let text = not_read(fx.call(d, Value::Object(Some(pawn))));
    assert!(text.starts_with("stopped after partial writes: "), "{text}");
    assert_eq!(text.matches("stopped after partial writes").count(), 1, "{text}");
    assert!(before.moved(&fx.vm), "the writing step wrote");
}

/// R1 (test 6): an unloaded definition handle, still unloaded afterwards, nothing counted.
#[test]
fn an_unloaded_definition_is_r1_and_stays_unloaded() {
    let mut fx = fx(ParityMode::Off);
    let (d, _) = fx.lazy();
    let pawn = fx.obj("Engine.Pawn");
    let before = Counters::read(&fx.vm);
    let text = not_read(fx.call_on(Some(d), Value::Object(Some(pawn))));
    assert!(text.contains("the definition") && text.contains(UNLOADED) && !text.contains("partial writes"), "{text}");
    assert!(fx.vm.defs.is_unloaded(&fx.vm.space, d), "not loaded");
    assert_eq!(Counters::read(&fx.vm), before, "nothing counted");
}

/// R2 (test 7) and hub ruling 3: an unloaded resolver entry is R2, also behind an unregistered loaded one (R2 runs over
/// every entry before R3). Nothing loaded or counted.
#[test]
fn an_unloaded_resolver_is_r2_before_any_r3() {
    let mut fx = fx(ParityMode::Off);
    let (_, lazy) = fx.lazy();
    let pawn = fx.obj("Engine.Pawn");
    let unread = fx.resolver("ToyUnreadResolver");
    for chain in [vec![Some(lazy)], vec![Some(unread), Some(lazy)]] {
        let d = fx.definition(&chain);
        let before = Counters::read(&fx.vm);
        let text = not_read(fx.call_on(Some(d), Value::Object(Some(pawn))));
        assert!(text.contains("the context resolver") && text.contains(UNLOADED), "{text}");
        assert!(fx.vm.defs.is_unloaded(&fx.vm.space, lazy), "not loaded");
        assert_eq!(Counters::read(&fx.vm), before, "nothing counted");
    }
}

/// R3 (test 8): a resolver class `ow-attr` does not register is the registry's own refusal, before any resolver runs:
/// the writing resolver before it has not written.
#[test]
fn an_unregistered_resolver_class_is_r3_before_the_walk() {
    let mut fx = fx(ParityMode::Off);
    let pawn = fx.obj("Engine.Pawn");
    let (w, unread) = (fx.resolver("ToyWritingResolver"), fx.resolver("ToyUnreadResolver"));
    let d = fx.definition(&[Some(w), Some(unread)]);
    let before = Counters::read(&fx.vm);
    let text = not_read(fx.call_on(Some(d), Value::Object(Some(pawn))));
    assert!(text.contains("context resolver class Engine.ToyUnreadResolver") && !text.contains("partial writes"), "{text}");
    assert_eq!(Counters::read(&fx.vm), before, "nothing ran");
}

/// Test 9: no receiver, a receiver outside the base class (a `FlagDefinition`, which the installed packages place under
/// `GBXDefinition`, not `AttributeDefinitionBase`), a non-object argument and a malformed chain are script errors,
/// before anything runs.
#[test]
fn malformed_calls_are_script_errors() {
    let mut fx = fx(ParityMode::Off);
    let pawn = fx.obj("Engine.Pawn");
    let p = fx.resolver("PawnAttributeContextResolver");
    let flag = fx.definition_of("GearboxFramework.FlagDefinition", &[Some(p)]);
    let d = fx.definition(&[Some(p)]);
    let bad = fx.obj("Engine.ToyBadChainDefinition");
    let cases: [(Option<ObjRef>, Value, &str); 4] = [
        (None, Value::Object(Some(pawn)), "without a definition"),
        (Some(flag), Value::Object(Some(pawn)), "not an Engine.AttributeDefinitionBase"),
        (Some(d), Value::Int(3), "ContextSource holds"),
        (Some(bad), Value::Object(Some(pawn)), "ContextResolverChain is not declared as an array"),
    ];
    for (this, source, want) in cases {
        let before = Counters::read(&fx.vm);
        let text = script_text(fx.call_on(this, source));
        assert!(text.contains(want), "{text}");
        assert_eq!(Counters::read(&fx.vm), before, "nothing counted: {want}");
    }
}

/// Hub ruling 4: a multi-context receiver runs the same base walk (pending the note's final text).
#[test]
fn a_multi_context_receiver_runs_the_base_walk() {
    let mut fx = fx(ParityMode::Off);
    let pawn = fx.obj("Engine.Pawn");
    let p = fx.resolver("PawnAttributeContextResolver");
    let d = fx.definition_of("Engine.AttributeDefinitionMultiContext", &[Some(p)]);
    assert_eq!(fx.call(d, Value::Object(Some(pawn))).unwrap(), Value::Object(Some(pawn)));
}

/// Test 10: Reference binds nothing.
#[test]
fn reference_binds_nothing() {
    let fx = fx(ParityMode::Reference);
    assert!(!fx.vm.has_native(RESOLVE_CONTEXT));
}

/// Hub ruling 2: the shared rule and the equip pack register in both orders; each keeps the binding made first, and
/// both give the shared rule (R1 on an unloaded definition).
#[test]
fn the_equip_pack_and_the_shared_rule_register_in_either_order() {
    for equip_first in [true, false] {
        let mut fx = fx_with(ParityMode::Off, false);
        assert!(!fx.vm.has_native(RESOLVE_CONTEXT));
        if equip_first {
            register(&mut fx.vm).unwrap();
            bind_resolve_context(&mut fx.vm).unwrap();
        } else {
            bind_resolve_context(&mut fx.vm).unwrap();
            register(&mut fx.vm).unwrap();
        }
        assert!(fx.vm.has_native(RESOLVE_CONTEXT));
        let (d, _) = fx.lazy();
        let pawn = fx.obj("Engine.Pawn");
        let text = not_read(fx.call(d, Value::Object(Some(pawn))));
        assert!(text.contains(UNLOADED), "{equip_first}: {text}");
    }
}
