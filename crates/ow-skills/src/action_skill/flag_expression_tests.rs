//! Tests of `FlagExpressionEvaluator.Evaluate` in Off (lane IMPL-FLAG-EXPRESSION-EVALUATE), on invented classes with the
//! note's class and field names, defined by hand, and a small invented package for an unloaded handle (AGENTS.md rule
//! 9). AI-assisted (Claude Code).

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::notyet::Missing;
use ow_core::{ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ClassId, ObjRef, PropAddr, StructId};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_package::testkit::script::ScriptToy;
use ow_package::testkit::{Export, ints};
use ow_vm::api::{FunctionDecl, Vm};
use ow_vm::error::VmError;

use super::*;

static NEXT: AtomicU32 = AtomicU32::new(0);

/// A context resolver that hands its source on unchanged, registered for the invented class `Toy.PassContextResolver`.
struct Pass;

impl ow_attr::api::ContextResolver for Pass {
    fn resolve(
        &self,
        _sys: &AttributeSystem,
        _resolver: ObjRef,
        _attr: ObjRef,
        source: Option<ObjRef>,
        _cx: &mut ow_attr::api::EvalCx<'_>,
    ) -> Result<Option<ObjRef>, ow_attr::api::AttrError> {
        Ok(source)
    }
}

thread_local! {
    /// What `Toy.FixedContextResolver` answers on this test's thread.
    static FIXED: Cell<Option<ObjRef>> = const { Cell::new(None) };
}

/// A context resolver answering [`FIXED`], registered for `Toy.FixedContextResolver`.
struct Fixed;

impl ow_attr::api::ContextResolver for Fixed {
    fn resolve(
        &self,
        _sys: &AttributeSystem,
        _resolver: ObjRef,
        _attr: ObjRef,
        _source: Option<ObjRef>,
        _cx: &mut ow_attr::api::EvalCx<'_>,
    ) -> Result<Option<ObjRef>, ow_attr::api::AttrError> {
        Ok(FIXED.with(Cell::get))
    }
}

/// A context resolver that always answers None, registered for `Toy.NoneContextResolver`.
struct NoneAnswer;

impl ow_attr::api::ContextResolver for NoneAnswer {
    fn resolve(
        &self,
        _sys: &AttributeSystem,
        _resolver: ObjRef,
        _attr: ObjRef,
        _source: Option<ObjRef>,
        _cx: &mut ow_attr::api::EvalCx<'_>,
    ) -> Result<Option<ObjRef>, ow_attr::api::AttrError> {
        Ok(None)
    }
}

struct Fx {
    vm: Vm,
    conditional: StructId,
    flag_entry: StructId,
    time: ObjRef,
    dir: PathBuf,
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// `ToyFlag.upk`: a holder whose `Held` names `Thing_0`, an export the store keeps unloaded until it is loaded, and
/// whose `Broken` names `Broken_0`, an export whose archetype is `Other_0` (an object of an unrelated class), so its
/// load fails with the store's `NotYet`, and whose `Cold` names `GearboxFramework.Evaluator_0` ([`gearbox_package`];
/// review r1 of #586).
fn toy_package() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class", "ObjectProperty"]);
    let gearbox = t.p.add_import("Core", "Package", 0, "GearboxFramework");
    let gearbox_object = t.p.add_import("Core", "Class", gearbox, "Object");
    let cold = t.p.add_import("GearboxFramework", "FlagExpressionEvaluator", gearbox, "Evaluator_0");
    let object = t.class("Object", 0);
    let thing = t.class("Thing", object);
    let holder = t.class("Holder", object);
    let other = t.class("Other", object);
    t.prop("Object", holder, "Held", 0, object);
    t.prop("Object", holder, "Broken", 0, object);
    t.prop("Object", holder, "Cold", 0, gearbox_object);
    let holder_ref = t.p.next_export();
    let (held, unrelated, broken) = (holder_ref + 1, holder_ref + 2, holder_ref + 3);
    let mut payload = ints(&[0]);
    payload.extend(t.p.object_tag("Held", held));
    payload.extend(t.p.object_tag("Broken", broken));
    payload.extend(t.p.object_tag("Cold", cold));
    payload.extend_from_slice(&t.none);
    t.p.add_export_with(Export::new(holder, "Holder_0", payload));
    let mut payload = ints(&[0]);
    payload.extend_from_slice(&t.none);
    t.p.add_export_with(Export::new(thing, "Thing_0", payload.clone()));
    t.p.add_export_with(Export::new(other, "Other_0", payload.clone()));
    let mut e = Export::new(thing, "Broken_0", payload.clone());
    e.archetype = unrelated;
    t.p.add_export_with(e);
    t.build()
}

/// `GearboxFramework.upk`: the class `FlagExpressionEvaluator` (no fields of its own) and `Evaluator_0`, an export of
/// it with no tagged fields, which `ToyFlag`'s holder names (review r1 of #586: a receiver handle).
fn gearbox_package() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class", "ObjectProperty"]);
    let object = t.class("Object", 0);
    let evaluator = t.class("FlagExpressionEvaluator", object);
    let mut payload = ints(&[0]);
    payload.extend_from_slice(&t.none);
    t.p.add_export_with(Export::new(evaluator, "Evaluator_0", payload));
    t.build()
}

fn fx() -> Fx {
    fx_with(true)
}

fn fx_with(world_info: bool) -> Fx {
    fx_full(world_info, None)
}

fn fx_full(world_info: bool, missing: Option<&str>) -> Fx {
    fx_all(world_info, missing, false)
}

/// The fixture; `world_info`: whether a `WorldInfo` is placed (the world's time); `missing`: an interface class left
/// out of the loaded classes (its subclasses then derive from `Core.Object`); `package_evaluator`: the evaluator class
/// is left to `GearboxFramework.upk` (a cold receiver's class) instead of defined here.
fn fx_all(world_info: bool, missing: Option<&str>, package_evaluator: bool) -> Fx {
    let mut s = ObjectSpace::new();
    let object = s.classes.define("Core.Object", None, vec![]).unwrap();
    let d = |s: &mut ObjectSpace, n: &str, k: PropKind| PropertyDecl::new(s.names.intern(n), k);
    let obj = PropKind::Object { class: object };
    let byte = PropKind::Byte { enum_: None };
    let objs = PropKind::Array(Box::new(obj.clone()));
    // The flag structs.
    let decls = vec![
        d(&mut s, "Value", byte.clone()),
        d(&mut s, "Raised", byte.clone()),
        d(&mut s, "Lowered", byte.clone()),
        d(&mut s, "RaisedTime", PropKind::Float),
        d(&mut s, "LoweredTime", PropKind::Float),
        d(&mut s, "Duration", PropKind::Float),
    ];
    let flag = s.classes.define_struct("Core.Object.Flag", None, decls).unwrap();
    let decls = vec![d(&mut s, "DynamicFlag", PropKind::Struct(flag)), d(&mut s, "LastEvaluatedExpressionTime", PropKind::Float)];
    let flag_entry = s.classes.define_struct("GearboxFramework.RuleEngine.DynamicFlagInstanceData", None, decls).unwrap();
    let decls = vec![
        d(&mut s, "FlagEvalType", byte.clone()),
        d(&mut s, "FlagDefinition", obj.clone()),
        d(&mut s, "TimeSeconds", PropKind::Float),
    ];
    let conditional = s.classes.define_struct("GearboxFramework.Rule.FlagEvalConditional", None, decls).unwrap();
    // The actors, the store and the evaluator classes.
    let actor = s.classes.define("Engine.Actor", Some(object), vec![]).unwrap();
    let decls = vec![d(&mut s, "TimeSeconds", PropKind::Float)];
    s.classes.define("Engine.WorldInfo", Some(actor), decls).unwrap();
    let decls = vec![d(&mut s, "Controller", obj.clone()), d(&mut s, "DrivenVehicle", obj.clone())];
    let pawn = s.classes.define("Engine.Pawn", Some(actor), decls).unwrap();
    s.classes.define("GearboxFramework.GearboxPawn", Some(pawn), vec![]).unwrap();
    // The fixture makes the mind derive from both interfaces (rule engine owner, flag provider) so the class tables
    // answer them.
    let interface_class = |s: &mut ObjectSpace, path: &str, parent: ClassId| {
        if missing == Some(path) { parent } else { s.classes.define(path, Some(parent), vec![]).unwrap() }
    };
    let owner = interface_class(&mut s, "GearboxFramework.IRuleEngineOwner", object);
    let decls = vec![d(&mut s, "MyRuleEngine", obj.clone())];
    s.classes.define("Toy.OtherRuleEngineOwner", Some(owner), decls).unwrap();
    s.classes.define("GearboxFramework.RuleEngine", Some(object), vec![]).unwrap();
    let provider = interface_class(&mut s, "GearboxFramework.IFlagProvider", owner);
    let decls = vec![
        d(&mut s, "DynamicFlagDefinitions", objs.clone()),
        d(&mut s, "DynamicFlags", PropKind::Array(Box::new(PropKind::Struct(flag_entry)))),
        d(&mut s, "MyRuleEngine", obj.clone()),
    ];
    s.classes.define("GearboxFramework.GearboxMind", Some(provider), decls).unwrap();
    let decls = vec![
        d(&mut s, "EvaluationExpression", obj.clone()),
        d(&mut s, "ContextResolverChain", objs.clone()),
        d(&mut s, "ValueResolver", obj.clone()),
    ];
    s.classes.define("GearboxFramework.FlagDefinition", Some(object), decls).unwrap();
    let value_resolver = s.classes.define("GearboxFramework.FlagValueResolver", Some(object), vec![]).unwrap();
    s.classes.define("GearboxFramework.RuleEngineFlagValueResolver", Some(value_resolver), vec![]).unwrap();
    let decls = vec![d(&mut s, "CheckRate", PropKind::Float), d(&mut s, "FunctionCall", PropKind::Str)];
    s.classes.define("GearboxFramework.ObjectFunctionFlagValueResolver", Some(value_resolver), decls).unwrap();
    s.classes.define("Toy.OtherFlagValueResolver", Some(value_resolver), vec![]).unwrap();
    s.classes.define("Toy.PassContextResolver", Some(object), vec![]).unwrap();
    s.classes.define("Toy.FixedContextResolver", Some(object), vec![]).unwrap();
    s.classes.define("Toy.NoneContextResolver", Some(object), vec![]).unwrap();
    let expression = s.classes.define("Engine.ExpressionEvaluator", Some(object), vec![]).unwrap();
    let decls = vec![
        d(&mut s, "FlagChain", PropKind::Array(Box::new(PropKind::Struct(conditional)))),
        d(&mut s, "FlagChainOperator", byte),
    ];
    if !package_evaluator {
        s.classes.define("GearboxFramework.FlagExpressionEvaluator", Some(expression), decls).unwrap();
    }
    let dir = std::env::temp_dir().join(format!("ow-skills-flag-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let (package, gearbox) = (toy_package(), gearbox_package());
    let defs = DefStore::from_synthetic(
        &dir,
        &[("ToyFlag.upk", package.as_slice()), ("GearboxFramework.upk", gearbox.as_slice())],
        IdentityPolicy::FirstInScanOrder,
        ParityMode::Off,
    )
    .unwrap();
    let attrs = AttributeSystem::new_empty()
        .with_context_resolver("Toy.PassContextResolver", Pass)
        .unwrap()
        .with_context_resolver("Toy.FixedContextResolver", Fixed)
        .unwrap()
        .with_context_resolver("Toy.NoneContextResolver", NoneAnswer)
        .unwrap();
    let mut vm = Vm::new(s, defs, Rc::new(attrs), ParityMode::Off);
    let actors = vm.add_service(ow_world::actors::Actors::new(actor)).unwrap();
    let wi_class = vm.space.classes.by_path("Engine.WorldInfo").unwrap();
    let time = vm.space.alloc(wi_class, "WorldInfo_0", None, Origin::Runtime).unwrap();
    if world_info {
        actors.borrow_mut().place_world_info(time).unwrap();
    }
    Fx { vm, conditional, flag_entry, time, dir }
}

impl Fx {
    fn class(&self, path: &str) -> ClassId {
        self.vm.space.classes.by_path(path).unwrap()
    }

    fn obj(&mut self, path: &str, name: &str) -> ObjRef {
        let c = self.class(path);
        self.vm.space.alloc(c, name, None, Origin::Runtime).unwrap()
    }

    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let c = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(c, name).unwrap_or_else(|| panic!("no {name}"));
        self.vm.space.write(o, PropAddr::scalar(p), v).unwrap();
    }

    fn get(&self, o: ObjRef, name: &str) -> Value {
        let c = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(c, name).unwrap();
        self.vm.space.read(o, PropAddr::scalar(p)).unwrap().clone()
    }

    fn set_time(&mut self, t: f32) {
        let wi = self.time;
        self.set(wi, "TimeSeconds", Value::Float(t));
    }

    /// An evaluator of `conditionals` (type, definition) with `operator`.
    fn evaluator(&mut self, conditionals: &[(u8, Option<ObjRef>)], operator: u8) -> ObjRef {
        let e = self.obj("GearboxFramework.FlagExpressionEvaluator", "Evaluator");
        let mut items = Vec::new();
        for &(kind, definition) in conditionals {
            let mut v = self.vm.space.classes.struct_defaults(self.conditional).unwrap();
            let s = v.struct_mut().unwrap();
            s.fields[0] = Value::Byte(kind);
            s.fields[1] = Value::Object(definition);
            items.push(v);
        }
        // Through the slot: the unloaded-handle case names an object of the toy package's own `Object` class, which the
        // typed write would refuse.
        let class = self.vm.space.heap.get(e).unwrap().class;
        let p = self.vm.space.prop(class, "FlagChain").unwrap();
        *self.vm.space.slot_mut(e, PropAddr::scalar(p)).unwrap() = Value::array(items);
        self.set(e, "FlagChainOperator", Value::Byte(operator));
        e
    }

    /// A flag definition whose context chain is one pass-through resolver.
    fn definition(&mut self, name: &str) -> ObjRef {
        let f = self.obj("GearboxFramework.FlagDefinition", name);
        let pass = self.obj("Toy.PassContextResolver", "Pass");
        self.set(f, "ContextResolverChain", Value::array(vec![Value::Object(Some(pass))]));
        f
    }

    /// A computed flag: its `EvaluationExpression` is `expression`, no value resolver.
    fn computed(&mut self, expression: ObjRef) -> ObjRef {
        let f = self.definition("Computed");
        self.set(f, "EvaluationExpression", Value::Object(Some(expression)));
        f
    }

    /// A flag answered by the function `function` on the context through `ObjectFunctionFlagValueResolver`.
    fn function_flag(&mut self, function: &str, rate: f32) -> ObjRef {
        let r = self.obj("GearboxFramework.ObjectFunctionFlagValueResolver", "Resolver");
        self.set(r, "CheckRate", Value::Float(rate));
        self.set(r, "FunctionCall", Value::Str(function.into()));
        let f = self.definition("FunctionFlag");
        self.set(f, "ValueResolver", Value::Object(Some(r)));
        f
    }

    /// A pawn whose controller is a mind, the pawn answering `IFlagProvider` with its controller (as the pawn rule
    /// does), and a native `Engine.Pawn.IsToyHeld` answering `answer` and counting its calls.
    fn pawn_with_mind(&mut self, answer: Rc<Cell<bool>>, calls: Rc<Cell<u32>>) -> (ObjRef, ObjRef) {
        let mind = self.obj("GearboxFramework.GearboxMind", "Mind");
        let pawn = self.obj("Engine.Pawn", "Pawn");
        self.set(pawn, "Controller", Value::Object(Some(mind)));
        let pawn_class = self.class("Engine.Pawn");
        self.vm
            .register_interface_rule(
                pawn_class,
                Rc::new(move |vm: &mut Vm, this: ObjRef, _iface| {
                    let c = vm.space.heap.get(this)?.class;
                    let p = vm.space.prop(c, "Controller").unwrap();
                    Ok(match vm.space.read(this, PropAddr::scalar(p))? {
                        Value::Object(o) => *o,
                        _ => None,
                    })
                }),
            )
            .unwrap();
        let decl = FunctionDecl::native(ObjectPath::parse("Engine.Pawn.IsToyHeld").unwrap(), "Pawn.IsToyHeld()")
            .with_owner(pawn_class)
            .with_result(PropertyDecl::new(self.vm.space.names.intern("ReturnValue"), PropKind::Bool));
        self.vm.declare_function(decl).unwrap();
        self.vm
            .register_native(
                "Pawn.IsToyHeld()",
                Rc::new(move |_c| {
                    calls.set(calls.get() + 1);
                    Ok(Value::Bool(answer.get()))
                }),
            )
            .unwrap();
        (pawn, mind)
    }

    fn eval(&mut self, e: ObjRef, context: Option<ObjRef>) -> Result<bool, VmError> {
        let start = WriteCounters::of(&self.vm);
        evaluate(&mut self.vm, Some(e), context).map_err(|err| start.mark(&self.vm, err))
    }

    /// The toy package's holder's handle in field `name` (still unloaded, or failed, as the test left it).
    fn held(&mut self, name: &str) -> ObjRef {
        let path = ObjectPath::parse("ToyFlag.Holder_0").unwrap();
        let holder = self.vm.defs.find(&mut self.vm.space, &path).unwrap().unwrap();
        let v = self.get(holder, name);
        let Value::Object(Some(o)) = v else { panic!("a held object, got {v:?}") };
        o
    }

    /// The toy package's unloaded `Evaluator_0` (its holder's `Cold`).
    fn cold_evaluator(&mut self) -> ObjRef {
        self.held("Cold")
    }

    /// Writes `v` into `o`'s field `name` through the slot (a toy-package object the typed write would refuse).
    fn set_slot(&mut self, o: ObjRef, name: &str, v: Value) {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap();
        *self.vm.space.slot_mut(o, PropAddr::scalar(p)).unwrap() = v;
    }

    /// A flag definition answered by `RuleEngineFlagValueResolver`, its context chain `chain`.
    fn store_flag(&mut self, chain: Vec<Value>) -> ObjRef {
        let r = self.obj("GearboxFramework.RuleEngineFlagValueResolver", "StoreResolver");
        let f = self.obj("GearboxFramework.FlagDefinition", "StoreFlag");
        self.set(f, "ContextResolverChain", Value::array(chain));
        self.set(f, "ValueResolver", Value::Object(Some(r)));
        f
    }

    /// Sets entry `index` of `mind`'s store: the flag's `Value`, `RaisedTime` and `Duration`.
    fn set_entry(&mut self, mind: ObjRef, index: usize, value: bool, raised: f32, duration: f32) {
        let mut entries = self.get(mind, "DynamicFlags").elements().unwrap().to_vec();
        let flag = entries[index].struct_mut().unwrap().fields[0].struct_mut().unwrap();
        flag.fields[0] = Value::Byte(u8::from(value));
        flag.fields[3] = Value::Float(raised);
        flag.fields[5] = Value::Float(duration);
        self.set(mind, "DynamicFlags", Value::array(entries));
    }

    /// The flag `Value` of entry `index` of `mind`'s store.
    fn entry_value(&self, mind: ObjRef, index: usize) -> Value {
        let entries = self.get(mind, "DynamicFlags");
        entries.elements().unwrap()[index].as_struct().unwrap().fields[0].as_struct().unwrap().fields[0].clone()
    }
}

fn not_read(r: Result<bool, VmError>, words: &str) -> String {
    match r {
        Err(VmError::NotYet(n)) => {
            assert_eq!(n.missing, Missing::NotRead, "{n}");
            assert!(n.what.contains(words), "{n}");
            n.what.into_owned()
        }
        other => panic!("expected NotRead, got {other:?}"),
    }
}

#[test]
fn an_empty_chain_is_true() {
    let mut x = fx();
    let e = x.evaluator(&[], 1);
    assert!(x.eval(e, None).unwrap());
}

/// `CEOP_Or` returns at its first true conditional, and `CEOP_And` at its first false one: the second conditional's
/// function is never called. A None definition is false.
#[test]
fn or_and_and_exit_early() {
    let mut x = fx();
    let (answer, calls) = (Rc::new(Cell::new(true)), Rc::new(Cell::new(0)));
    let (pawn, _) = x.pawn_with_mind(answer.clone(), calls.clone());
    x.set_time(5.0);
    let empty = x.evaluator(&[], 1);
    let truthy = x.computed(empty);
    let held = x.function_flag("IsToyHeld", 0.2);
    let or = x.evaluator(&[(0, Some(truthy)), (0, Some(held))], 0);
    assert!(x.eval(or, Some(pawn)).unwrap());
    assert_eq!(calls.get(), 0, "Or: the second conditional is never reached");
    let and = x.evaluator(&[(1, Some(truthy)), (0, Some(held))], 1);
    assert!(!x.eval(and, Some(pawn)).unwrap());
    assert_eq!(calls.get(), 0, "And: the second conditional is never reached");
    // A skipped branch is never checked: a timed test after the short circuit does not refuse.
    let skipped = x.evaluator(&[(1, Some(truthy)), (2, Some(held))], 1);
    assert!(!x.eval(skipped, Some(pawn)).unwrap(), "the timed test is never reached");
    let none = x.evaluator(&[(0, None)], 1);
    assert!(!x.eval(none, Some(pawn)).unwrap(), "a None definition is false");
    // Step 1 comes before the type check (the accepted note, e2724bc9): a None definition of a timed type is false.
    let none_timed = x.evaluator(&[(2, None)], 1);
    assert!(!x.eval(none_timed, Some(pawn)).unwrap(), "a None definition is false before the type check");
    // Or with a false first conditional goes on: the function runs, and sets the flag in the mind's store.
    let or = x.evaluator(&[(1, Some(truthy)), (0, Some(held))], 0);
    assert!(x.eval(or, Some(pawn)).unwrap());
    assert_eq!(calls.get(), 1);
}

/// `IsTrue` and `IsFalse` of a function-call flag: the first lookup adds the definition and a zeroed entry to the
/// mind's store (a write); the function runs once `CheckRate` has passed since the flag last changed and sets it; a
/// changed flag stamps its time. Types 2 and 3 are `NotRead`.
#[test]
fn the_flag_tests_through_the_minds_store() {
    let mut x = fx();
    let (answer, calls) = (Rc::new(Cell::new(true)), Rc::new(Cell::new(0)));
    let (pawn, mind) = x.pawn_with_mind(answer.clone(), calls.clone());
    let held = x.function_flag("IsToyHeld", 0.2);
    // Below the check rate on a fresh entry (times 0): the function is not called and the flag stays false.
    x.set_time(0.1);
    let is_true = x.evaluator(&[(0, Some(held))], 1);
    assert!(!x.eval(is_true, Some(pawn)).unwrap());
    assert_eq!(calls.get(), 0);
    let defs = x.get(mind, "DynamicFlagDefinitions");
    assert_eq!(defs.elements().unwrap(), &[Value::Object(Some(held))], "the definition was added");
    assert_eq!(x.get(mind, "DynamicFlags").elements().unwrap().len(), 1, "with one entry");
    // Past the check rate: the function sets the flag true, raised now.
    x.set_time(1.0);
    assert!(x.eval(is_true, Some(pawn)).unwrap());
    assert_eq!(calls.get(), 1);
    let is_false = x.evaluator(&[(1, Some(held))], 1);
    assert!(!x.eval(is_false, Some(pawn)).unwrap(), "IsFalse of a true flag; not called again within the rate");
    assert_eq!(calls.get(), 1);
    assert_eq!(x.get(mind, "DynamicFlags").elements().unwrap().len(), 1, "one entry, found again");
    for kind in [2u8, 3] {
        let timed = x.evaluator(&[(kind, Some(held))], 1);
        not_read(x.eval(timed, Some(pawn)), "the timed flag tests");
    }
}

/// The interface checks (lane FIX-FLAG-IMPLEMENTS-NOTYET, review r1 of #586): a loaded class that implements
/// `IFlagProvider` is read as the store; a loaded class that does not answers no (no store, false); an interface
/// class that is not loaded is `NotRead` before any write (no counter moves, nothing is marked), for either
/// interface, where the path may still ask for it, and only with a context.
#[test]
fn an_unloaded_interface_class_is_not_read_before_any_write() {
    // Loaded and implemented: the mind's store is read (the definition is added there).
    let mut x = fx();
    let mind = x.obj("GearboxFramework.GearboxMind", "Mind");
    let f = x.store_flag(vec![]);
    let e = x.evaluator(&[(0, Some(f))], 1);
    assert!(!x.eval(e, Some(mind)).unwrap(), "a zeroed entry");
    assert_eq!(x.get(mind, "DynamicFlagDefinitions").elements().unwrap(), &[Value::Object(Some(f))]);
    // Loaded and not implemented: no store, false, nothing refused.
    let plain = x.obj("Toy.PassContextResolver", "Plain");
    assert!(!x.eval(e, Some(plain)).unwrap(), "a loaded class that does not implement it answers no");

    for missing in ["GearboxFramework.IRuleEngineOwner", "GearboxFramework.IFlagProvider"] {
        let mut x = fx_full(true, Some(missing));
        let mind = x.obj("GearboxFramework.GearboxMind", "Mind");
        let f = x.store_flag(vec![]);
        let e = x.evaluator(&[(0, Some(f))], 1);
        let start = WriteCounters::of(&x.vm);
        let text = not_read(x.eval(e, Some(mind)), &format!("the interface class {missing} is not loaded (a load is a write)"));
        assert!(!text.starts_with("stopped after partial writes: "), "{text}");
        assert!(!start.moved(&x.vm), "{missing}: write-free");
        assert_eq!(x.get(mind, "DynamicFlagDefinitions").elements().unwrap().len(), 0, "{missing}: the store untouched");
        // With no context no interface question is reached: false, as before.
        assert!(!x.eval(e, None).unwrap(), "{missing}: no context");
        // A loaded receiver whose path never asks for the missing interface answers as before: an empty chain, a
        // short-circuited And after a None definition, and (IFlagProvider only) a definition with neither a value
        // resolver nor an expression.
        let empty = x.evaluator(&[], 1);
        assert!(x.eval(empty, Some(mind)).unwrap(), "{missing}: an empty chain is true");
        let and = x.evaluator(&[(0, None), (0, Some(f))], 1);
        assert!(!x.eval(and, Some(mind)).unwrap(), "{missing}: the And stops at the None definition");
        let bare = x.definition("Bare");
        let plain = x.evaluator(&[(0, Some(bare))], 1);
        let before = WriteCounters::of(&x.vm);
        let r = x.eval(plain, Some(mind));
        if missing.ends_with("IFlagProvider") {
            // Review r1 of #630, item 2: a reached context resolver may set a ValueResolver, so with IFlagProvider not
            // loaded the bare definition (one pass-through resolver) is refused before it runs.
            let text = not_read(r, MUTABLE_PATH);
            assert!(!text.starts_with("stopped after partial writes: "), "{text}");
            assert!(!before.moved(&x.vm), "{missing}: the mutable-path refusal is write-free");
            // Review r1 of #630, item 1: an And whose first conditional is IsTrue on a loaded definition with no
            // context resolver and neither field is false with nothing run, so the second (a store flag) is never
            // reached and asks for nothing.
            let quiet = x.obj("GearboxFramework.FlagDefinition", "Quiet");
            let and = x.evaluator(&[(0, Some(quiet)), (0, Some(f))], 1);
            let start = WriteCounters::of(&x.vm);
            assert!(!x.eval(and, Some(mind)).unwrap(), "the And stops at the proven false");
            assert!(!start.moved(&x.vm), "nothing ran");
            // The same definition answering IsFalse is the loop's own refusal, not the provider's.
            let and = x.evaluator(&[(1, Some(quiet)), (0, Some(f))], 1);
            not_read(x.eval(and, Some(mind)), "IsFalse with neither an EvaluationExpression nor a ValueResolver");
        } else {
            // The rule engine search asks for IRuleEngineOwner before the definition's load.
            not_read(r, "is not loaded (a load is a write)");
            assert!(!before.moved(&x.vm), "{missing}: the bare refusal is write-free");
        }
    }
}

/// Review r1 of #586, item 2: a receiver handle that is not loaded yet needs both interface classes before its load,
/// so a refusal leaves it unloaded and moves no counter.
#[test]
fn a_cold_receiver_stays_unloaded_on_an_interface_refusal() {
    for missing in ["GearboxFramework.IRuleEngineOwner", "GearboxFramework.IFlagProvider"] {
        let mut x = fx_all(true, Some(missing), true);
        let mind = x.obj("GearboxFramework.GearboxMind", "Mind");
        let cold = x.cold_evaluator();
        assert!(x.vm.defs.is_unloaded(&x.vm.space, cold));
        let start = WriteCounters::of(&x.vm);
        let text = not_read(x.eval(cold, Some(mind)), &format!("the interface class {missing} is not loaded"));
        assert!(!text.starts_with("stopped after partial writes: "), "{text}");
        assert!(!start.moved(&x.vm), "{missing}: write-free");
        assert!(x.vm.defs.is_unloaded(&x.vm.space, cold), "{missing}: the receiver was not loaded");
    }
    // With every interface loaded, the guard passes and the handle is loaded; the invented class declares no
    // FlagChain, so the read after the load is the Script error.
    let mut x = fx_all(true, None, true);
    let mind = x.obj("GearboxFramework.GearboxMind", "Mind");
    let cold = x.cold_evaluator();
    match x.eval(cold, Some(mind)).unwrap_err() {
        VmError::Script { message, .. } => assert!(message.contains("declares no FlagChain"), "{message}"),
        other => panic!("{other:?}"),
    }
    assert!(!x.vm.defs.is_unloaded(&x.vm.space, cold), "loaded once the interfaces are there");
}

/// A reached unloaded flag definition is loaded (a write), and a refusal before any write is not marked: an unread
/// value resolver class on runtime objects.
#[test]
fn the_refusals_come_before_any_write() {
    let mut x = fx();
    let lazy = x.held("Held");
    assert!(x.vm.defs.is_unloaded(&x.vm.space, lazy));
    // A reached handle is loaded when first used (the accepted note, section 6): here the toy package's object, which
    // declares no ContextResolverChain, so the read after the load is a Script error, not a NotYet.
    let e = x.evaluator(&[(0, Some(lazy))], 1);
    let pawn = x.obj("Engine.Pawn", "Pawn");
    match x.eval(e, Some(pawn)).unwrap_err() {
        VmError::Script { message, .. } => assert!(message.contains("declares no ContextResolverChain"), "{message}"),
        other => panic!("{other:?}"),
    }
    assert!(!x.vm.defs.is_unloaded(&x.vm.space, lazy), "the reached handle was loaded");
    let other = x.obj("Toy.OtherFlagValueResolver", "Other");
    let f = x.definition("OtherFlag");
    x.set(f, "ValueResolver", Value::Object(Some(other)));
    let e = x.evaluator(&[(0, Some(f))], 1);
    let text = not_read(x.eval(e, Some(pawn)), "the value resolver class Toy.OtherFlagValueResolver is not read");
    assert!(!text.starts_with("stopped after partial writes: "), "{text}");
}

/// After a write, a refusal is marked: the first lookup adds the entry, then the function's result is a fraction,
/// which the port does not take (its own choice).
#[test]
fn a_refusal_after_a_write_is_marked() {
    let mut x = fx();
    let (answer, calls) = (Rc::new(Cell::new(true)), Rc::new(Cell::new(0)));
    let (pawn, _) = x.pawn_with_mind(answer, calls);
    // Rebind the counted function's answer to a fraction through a second function.
    let pawn_class = x.class("Engine.Pawn");
    let decl = FunctionDecl::native(ObjectPath::parse("Engine.Pawn.ToyFraction").unwrap(), "Pawn.ToyFraction()")
        .with_owner(pawn_class)
        .with_result(PropertyDecl::new(x.vm.space.names.intern("ReturnValue"), PropKind::Float));
    x.vm.declare_function(decl).unwrap();
    x.vm.register_native("Pawn.ToyFraction()", Rc::new(|_c| Ok(Value::Float(0.5)))).unwrap();
    let f = x.function_flag("ToyFraction", 0.2);
    x.set_time(1.0);
    let e = x.evaluator(&[(0, Some(f))], 1);
    let text = not_read(x.eval(e, Some(pawn)), "a fractional function-call flag result (the port takes only an integral");
    assert!(text.starts_with("stopped after partial writes: "), "{text}");
    let _ = x.flag_entry;
}

/// The definition path's cases (the accepted note, section 4.0): a None context answers false; an empty
/// `ContextResolverChain` keeps the context; with neither an expression nor a value resolver `IsTrue` is false and
/// `IsFalse` is `NotRead`.
#[test]
fn the_definition_path_cases() {
    let mut x = fx();
    let pawn = x.obj("Engine.Pawn", "Pawn");
    let empty = x.evaluator(&[], 1);
    let computed = x.computed(empty);
    let e = x.evaluator(&[(0, Some(computed))], 1);
    assert!(!x.eval(e, None).unwrap(), "a None context answers false");
    let bare = x.obj("GearboxFramework.FlagDefinition", "NoChain");
    x.set(bare, "EvaluationExpression", Value::Object(Some(empty)));
    let e = x.evaluator(&[(0, Some(bare))], 1);
    assert!(x.eval(e, Some(pawn)).unwrap(), "an empty chain keeps the context");
    let nothing = x.definition("Nothing");
    let e = x.evaluator(&[(0, Some(nothing))], 1);
    assert!(!x.eval(e, Some(pawn)).unwrap(), "IsTrue with neither: false");
    let e = x.evaluator(&[(1, Some(nothing))], 1);
    not_read(x.eval(e, Some(pawn)), "IsFalse with neither an EvaluationExpression nor a ValueResolver");
}

/// Branch S (the note's store path): with a rule engine `R` found for the context, a definition lacking a context
/// resolver or a value resolver is "a rule engine's own flag store", `NotRead`, before any write. `R` is found on a
/// rule engine owner (its `MyRuleEngine`), on a pawn's controller, on the controller of the vehicle a pawn drives, on
/// and on a `RuleEngine` context; an owner other than a mind is `NotRead` (the port's choice).
#[test]
fn the_store_path_with_a_rule_engine() {
    let mut x = fx();
    let engine = x.obj("GearboxFramework.RuleEngine", "Engine");
    let mind = x.obj("GearboxFramework.GearboxMind", "Mind");
    x.set(mind, "MyRuleEngine", Value::Object(Some(engine)));
    let empty = x.evaluator(&[], 1);
    // A computed flag with a context chain but no value resolver.
    let computed = x.computed(empty);
    let e = x.evaluator(&[(0, Some(computed))], 1);
    let text = not_read(x.eval(e, Some(mind)), "a rule engine's own flag store");
    assert!(!text.starts_with("stopped after partial writes: "), "{text}");
    // The owner lookup through a pawn's controller.
    let pawn = x.obj("GearboxFramework.GearboxPawn", "Pawn");
    x.set(pawn, "Controller", Value::Object(Some(mind)));
    not_read(x.eval(e, Some(pawn)), "a rule engine's own flag store");
    // With no controller, the controller of the vehicle the pawn drives.
    let driver = x.obj("GearboxFramework.GearboxPawn", "Driver");
    let vehicle = x.obj("GearboxFramework.GearboxPawn", "Vehicle");
    x.set(vehicle, "Controller", Value::Object(Some(mind)));
    x.set(driver, "DrivenVehicle", Value::Object(Some(vehicle)));
    not_read(x.eval(e, Some(driver)), "a rule engine's own flag store");
    // A RuleEngine context is its own R.
    not_read(x.eval(e, Some(engine)), "a rule engine's own flag store");
    // A mind with no rule engine gives no R: the definition path runs.
    let free = x.obj("GearboxFramework.GearboxMind", "FreeMind");
    assert!(x.eval(e, Some(free)).unwrap());
    // An owner other than a mind is the port's own refusal, before the definition is loaded: not marked.
    let other = x.obj("Toy.OtherRuleEngineOwner", "Other");
    let lazy = x.held("Held");
    let e = x.evaluator(&[(0, Some(lazy))], 1);
    let text = not_read(x.eval(e, Some(other)), "only GearboxMind's is read");
    assert!(!text.starts_with("stopped after partial writes: "), "{text}");
    assert!(x.vm.defs.is_unloaded(&x.vm.space, lazy), "R comes before the definition's load");
}

/// The hand-off: a computed flag that also has a value resolver is `NotRead` once its expression has answered.
#[test]
fn a_computed_flag_with_a_value_resolver_is_not_read() {
    let mut x = fx();
    let pawn = x.obj("Engine.Pawn", "Pawn");
    let empty = x.evaluator(&[], 1);
    let f = x.computed(empty);
    let r = x.obj("GearboxFramework.RuleEngineFlagValueResolver", "StoreResolver");
    x.set(f, "ValueResolver", Value::Object(Some(r)));
    let e = x.evaluator(&[(0, Some(f))], 1);
    not_read(x.eval(e, Some(pawn)), "handing a computed answer to its value resolver");
}

/// The context chain: a None entry is skipped; a resolver's answer is the new context, and `RuleEngineFlagValueResolver`
/// reads the mind's store there (adding the definition, then reading the flag); a resolver answering None ends the
/// chain and the context so far stays (here a pawn with no controller: no store, false, the mind untouched).
#[test]
fn the_context_chain_and_the_minds_store() {
    let mut x = fx();
    let pawn = x.obj("Engine.Pawn", "Pawn");
    let mind = x.obj("GearboxFramework.GearboxMind", "Mind");
    let fixed = x.obj("Toy.FixedContextResolver", "Fixed");
    FIXED.with(|c| c.set(Some(mind)));
    let f = x.store_flag(vec![Value::Object(None), Value::Object(Some(fixed))]);
    let e = x.evaluator(&[(0, Some(f))], 1);
    assert!(!x.eval(e, Some(pawn)).unwrap(), "a zeroed entry is false");
    assert_eq!(x.get(mind, "DynamicFlagDefinitions").elements().unwrap(), &[Value::Object(Some(f))]);
    x.set_entry(mind, 0, true, 0.0, 0.0);
    assert!(x.eval(e, Some(pawn)).unwrap(), "the mind's flag, read through the chain");
    // A resolver answering None ends the chain: the later resolver would have answered another mind, whose store
    // would then have gained the definition (review r2 of #535: this fails if the chain went on past the None).
    let none = x.obj("Toy.NoneContextResolver", "None");
    let other = x.obj("GearboxFramework.GearboxMind", "Other");
    let g = x.store_flag(vec![Value::Object(Some(none)), Value::Object(Some(fixed))]);
    let e = x.evaluator(&[(0, Some(g))], 1);
    FIXED.with(|c| c.set(Some(other)));
    assert!(!x.eval(e, Some(pawn)).unwrap(), "the pawn stays the context: no store");
    assert_eq!(x.get(other, "DynamicFlagDefinitions").elements().unwrap().len(), 0, "the later resolver never ran");
    assert_eq!(x.get(mind, "DynamicFlagDefinitions").elements().unwrap().len(), 1, "the mind is untouched");
}

/// A resolver's answer is loaded when reached (note section 6), not refused: the answer here is the toy package's
/// unloaded object. It is no flag provider, so the function flag's `IFlagProvider` query answers None and the flag is
/// false, with the answer loaded.
#[test]
fn a_resolvers_answer_is_loaded() {
    let mut x = fx();
    let pawn = x.obj("Engine.Pawn", "Pawn");
    let lazy = x.held("Held");
    let fixed = x.obj("Toy.FixedContextResolver", "Fixed");
    FIXED.with(|c| c.set(Some(lazy)));
    let f = x.function_flag("IsToyHeld", 0.2);
    x.set(f, "ContextResolverChain", Value::array(vec![Value::Object(Some(fixed))]));
    let e = x.evaluator(&[(0, Some(f))], 1);
    // ObjectFunctionFlagValueResolver asks the answer for IFlagProvider; the toy object answers with itself only if
    // it is one, so the plain answer is None and the flag is false, with the answer loaded.
    assert!(!x.eval(e, Some(pawn)).unwrap());
    assert!(!x.vm.defs.is_unloaded(&x.vm.space, lazy), "the answer was loaded");
}

/// A timed flag expires when read: a raised flag with a `Duration` is true within it and false after it, the store's
/// entry lowered (a write).
#[test]
fn a_timed_flag_expires_when_read() {
    let mut x = fx();
    let pawn = x.obj("Engine.Pawn", "Pawn");
    let mind = x.obj("GearboxFramework.GearboxMind", "Mind");
    let fixed = x.obj("Toy.FixedContextResolver", "Fixed");
    FIXED.with(|c| c.set(Some(mind)));
    let f = x.store_flag(vec![Value::Object(Some(fixed))]);
    let e = x.evaluator(&[(0, Some(f))], 1);
    assert!(!x.eval(e, Some(pawn)).unwrap());
    x.set_entry(mind, 0, true, 1.0, 2.0);
    x.set_time(2.5);
    assert!(x.eval(e, Some(pawn)).unwrap(), "within the duration");
    assert_eq!(x.entry_value(mind, 0), Value::Byte(1));
    x.set_time(5.0);
    assert!(!x.eval(e, Some(pawn)).unwrap(), "past the duration");
    assert_eq!(x.entry_value(mind, 0), Value::Byte(0), "the entry was lowered");
}

/// The world's time is `LayerAbsent` with no `WorldInfo` (the map load), marked here since the lookup wrote first.
/// (The definition store's own `LayerAbsent` cannot arise through a `Vm`, which always has a store.)
#[test]
fn no_world_time_is_layer_absent() {
    let mut x = fx_with(false);
    let (answer, calls) = (Rc::new(Cell::new(true)), Rc::new(Cell::new(0)));
    let (pawn, _) = x.pawn_with_mind(answer, calls);
    let f = x.function_flag("IsToyHeld", 0.2);
    let e = x.evaluator(&[(0, Some(f))], 1);
    match x.eval(e, Some(pawn)).unwrap_err() {
        VmError::NotYet(n) => {
            assert!(matches!(n.missing, Missing::LayerAbsent(_)), "{n}");
            assert!(n.what.starts_with("stopped after partial writes: "), "{n}");
        }
        other => panic!("{other:?}"),
    }
}

/// A load alone is a write for the mark: an unloaded value resolver is loaded, then its class is not read (marked).
/// A handle whose load failed is not loaded again, so a retry moves no counter and its refusal is not marked.
#[test]
fn a_load_is_a_write_and_a_failed_retry_is_not() {
    let mut x = fx();
    let pawn = x.obj("Engine.Pawn", "Pawn");
    let lazy = x.held("Held");
    let f = x.definition("LazyResolver");
    x.set_slot(f, "ValueResolver", Value::Object(Some(lazy)));
    let e = x.evaluator(&[(0, Some(f))], 1);
    let text = not_read(x.eval(e, Some(pawn)), "the value resolver class");
    assert!(text.starts_with("stopped after partial writes: "), "{text}");
    let broken = x.held("Broken");
    let e = x.evaluator(&[(0, Some(broken))], 1);
    let first = match x.eval(e, Some(pawn)).unwrap_err() {
        VmError::NotYet(n) => n,
        other => panic!("{other:?}"),
    };
    assert!(first.what.starts_with("stopped after partial writes: "), "the failed load is a transition: {first}");
    let again = match x.eval(e, Some(pawn)).unwrap_err() {
        VmError::NotYet(n) => n,
        other => panic!("{other:?}"),
    };
    assert!(!again.what.starts_with("stopped after partial writes: "), "a retry of a failed handle: {again}");
    assert_eq!(again.missing, first.missing);
}

// ---- Lane FIX-FLAG-PROVIDER-ORDER --------------------------------------------------------------------------------

/// Finding 1: on an Or chain whose first conditional's context resolver would load a cold answer (a write), the second
/// conditional's missing-`IFlagProvider` refusal comes first: no counter moves and the cold answer stays cold.
#[test]
fn the_provider_refusal_comes_before_an_earlier_conditionals_resolver() {
    let mut x = fx_full(true, Some("GearboxFramework.IFlagProvider"));
    let pawn = x.obj("Engine.Pawn", "Pawn");
    let lazy = x.held("Held");
    let fixed = x.obj("Toy.FixedContextResolver", "Fixed");
    FIXED.with(|c| c.set(Some(lazy)));
    // First: a definition with neither field (no store needed), whose chain answers the cold object.
    let first = x.definition("First");
    x.set(first, "ContextResolverChain", Value::array(vec![Value::Object(Some(fixed))]));
    // Second: a store flag, which needs IFlagProvider.
    let second = x.store_flag(vec![]);
    let e = x.evaluator(&[(0, Some(first)), (0, Some(second))], 0);
    let start = WriteCounters::of(&x.vm);
    let text = not_read(x.eval(e, Some(pawn)), "the interface class GearboxFramework.IFlagProvider is not loaded");
    assert!(!text.starts_with("stopped after partial writes: "), "{text}");
    assert!(!start.moved(&x.vm), "no counter moved");
    assert!(x.vm.defs.is_unloaded(&x.vm.space, lazy), "the cold answer stays cold");
    // A chain that refuses at its first conditional with no side effect (a timed test) reports that, not the provider.
    let timed = x.evaluator(&[(2, Some(first)), (0, Some(second))], 0);
    not_read(x.eval(timed, Some(pawn)), "the timed flag tests");
}

/// Review r1 of #630, item 2: on an Or chain, the first conditional's context resolver could set the second
/// definition's `ValueResolver` (neither definition has one before it runs), so the second's provider check would come
/// after the resolver's writes. With `IFlagProvider` not loaded the chain is refused before the first write: the
/// resolver never runs (its cold answer stays cold). With it loaded, the same chain runs and answers false.
#[test]
fn a_reached_context_resolver_needs_the_provider_before_it_runs() {
    let mut x = fx_full(true, Some("GearboxFramework.IFlagProvider"));
    let pawn = x.obj("Engine.Pawn", "Pawn");
    let lazy = x.held("Held");
    let fixed = x.obj("Toy.FixedContextResolver", "Fixed");
    FIXED.with(|c| c.set(Some(lazy)));
    let first = x.definition("First");
    x.set(first, "ContextResolverChain", Value::array(vec![Value::Object(Some(fixed))]));
    let second = x.obj("GearboxFramework.FlagDefinition", "Second");
    let e = x.evaluator(&[(0, Some(first)), (0, Some(second))], 0);
    let start = WriteCounters::of(&x.vm);
    let text = not_read(x.eval(e, Some(pawn)), MUTABLE_PATH);
    assert!(!text.starts_with("stopped after partial writes: "), "{text}");
    assert!(!start.moved(&x.vm), "no counter moved");
    assert!(x.vm.defs.is_unloaded(&x.vm.space, lazy), "the resolver never ran");

    let mut x = fx();
    let pawn = x.obj("Engine.Pawn", "Pawn");
    let first = x.definition("First");
    let second = x.obj("GearboxFramework.FlagDefinition", "Second");
    let e = x.evaluator(&[(0, Some(first)), (0, Some(second))], 0);
    assert!(!x.eval(e, Some(pawn)).unwrap(), "IFlagProvider loaded: the chain runs, false");
}

/// Finding 3: a loaded computed definition with a known rule engine and no value resolver reports the rule engine's own
/// store before the provider guard (`IFlagProvider` missing), write-free.
#[test]
fn the_store_path_is_reported_before_the_provider_guard() {
    let mut x = fx_full(true, Some("GearboxFramework.IFlagProvider"));
    let engine = x.obj("GearboxFramework.RuleEngine", "Engine");
    let mind = x.obj("GearboxFramework.GearboxMind", "Mind");
    x.set(mind, "MyRuleEngine", Value::Object(Some(engine)));
    let empty = x.evaluator(&[], 1);
    let computed = x.computed(empty);
    let e = x.evaluator(&[(0, Some(computed))], 1);
    let start = WriteCounters::of(&x.vm);
    let text = not_read(x.eval(e, Some(mind)), "a rule engine's own flag store");
    assert!(!text.starts_with("stopped after partial writes: "), "{text}");
    assert!(!start.moved(&x.vm), "write-free");
}

