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

/// `ToyFlag.upk`: a holder whose `Held` names `Thing_0`, an export the store keeps unloaded until it is loaded.
fn toy_package() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class", "ObjectProperty"]);
    let object = t.class("Object", 0);
    let thing = t.class("Thing", object);
    let holder = t.class("Holder", object);
    t.prop("Object", holder, "Held", 0, object);
    let held = t.p.next_export() + 1;
    let mut payload = ints(&[0]);
    payload.extend(t.p.object_tag("Held", held));
    payload.extend_from_slice(&t.none);
    t.p.add_export_with(Export::new(holder, "Holder_0", payload));
    let mut payload = ints(&[0]);
    payload.extend_from_slice(&t.none);
    t.p.add_export_with(Export::new(thing, "Thing_0", payload));
    t.build()
}

fn fx() -> Fx {
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
    s.classes.define("Engine.Pawn", Some(actor), decls).unwrap();
    // The fixture makes the mind derive from the flag-provider interface so the class tables answer it.
    let provider = s.classes.define("GearboxFramework.IFlagProvider", Some(object), vec![]).unwrap();
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
    let expression = s.classes.define("Engine.ExpressionEvaluator", Some(object), vec![]).unwrap();
    let decls = vec![
        d(&mut s, "FlagChain", PropKind::Array(Box::new(PropKind::Struct(conditional)))),
        d(&mut s, "FlagChainOperator", byte),
    ];
    s.classes.define("GearboxFramework.FlagExpressionEvaluator", Some(expression), decls).unwrap();
    let dir = std::env::temp_dir().join(format!("ow-skills-flag-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let package = toy_package();
    let defs = DefStore::from_synthetic(&dir, &[("ToyFlag.upk", package.as_slice())], IdentityPolicy::FirstInScanOrder, ParityMode::Off)
        .unwrap();
    let mut vm = Vm::new(s, defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
    let actors = vm.add_service(ow_world::actors::Actors::new(actor)).unwrap();
    let wi_class = vm.space.classes.by_path("Engine.WorldInfo").unwrap();
    let time = vm.space.alloc(wi_class, "WorldInfo_0", None, Origin::Runtime).unwrap();
    actors.borrow_mut().place_world_info(time).unwrap();
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

    /// A computed flag: its `EvaluationExpression` is `expression`, no resolvers.
    fn computed(&mut self, expression: ObjRef) -> ObjRef {
        let f = self.obj("GearboxFramework.FlagDefinition", "Computed");
        self.set(f, "EvaluationExpression", Value::Object(Some(expression)));
        f
    }

    /// A flag answered by the function `function` on the context through `ObjectFunctionFlagValueResolver`.
    fn function_flag(&mut self, function: &str, rate: f32) -> ObjRef {
        let r = self.obj("GearboxFramework.ObjectFunctionFlagValueResolver", "Resolver");
        self.set(r, "CheckRate", Value::Float(rate));
        self.set(r, "FunctionCall", Value::Str(function.into()));
        let f = self.obj("GearboxFramework.FlagDefinition", "FunctionFlag");
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
        let mut run = Run::default();
        evaluate(&mut self.vm, Some(e), context, &mut run).map_err(|err| run.mark(err))
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
    let none = x.evaluator(&[(0, None)], 1);
    assert!(!x.eval(none, Some(pawn)).unwrap(), "a None definition is false");
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

/// The refusals before any write: an unloaded flag definition (nothing loaded), an unread value resolver class, and a
/// store path when the context has a rule engine.
#[test]
fn the_refusals_come_before_any_write() {
    let mut x = fx();
    let path = ObjectPath::parse("ToyFlag.Holder_0").unwrap();
    let holder = x.vm.defs.find(&mut x.vm.space, &path).unwrap().unwrap();
    let Value::Object(Some(lazy)) = x.get(holder, "Held") else { panic!("a held object") };
    assert!(x.vm.defs.is_unloaded(&x.vm.space, lazy));
    let e = x.evaluator(&[(0, Some(lazy))], 1);
    let text = not_read(x.eval(e, None), "the unloaded flag definition");
    assert!(!text.starts_with("stopped after partial writes"), "{text}");
    assert!(x.vm.defs.is_unloaded(&x.vm.space, lazy), "nothing loaded");
    let other = x.obj("Toy.OtherFlagValueResolver", "Other");
    let f = x.obj("GearboxFramework.FlagDefinition", "OtherFlag");
    x.set(f, "ValueResolver", Value::Object(Some(other)));
    let e = x.evaluator(&[(0, Some(f))], 1);
    match x.eval(e, None).unwrap_err() {
        VmError::NotYet(n) => assert!(n.what.contains("the value resolver class Toy.OtherFlagValueResolver is not read"), "{n}"),
        other => panic!("{other:?}"),
    }
    let engine = x.obj("GearboxFramework.GearboxMind", "Engine");
    let mind = x.obj("GearboxFramework.GearboxMind", "Mind");
    x.set(mind, "MyRuleEngine", Value::Object(Some(engine)));
    let empty = x.evaluator(&[], 1);
    let computed = x.computed(empty);
    let e = x.evaluator(&[(0, Some(computed))], 1);
    // A GearboxMind context is not a rule engine owner in this fixture (no IRuleEngineOwner class): no store path.
    assert!(x.eval(e, Some(mind)).unwrap());
}

/// After a write, a refusal is marked: the first lookup adds the entry, then the function's result is a fraction,
/// whose rounding is not read.
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
    let text = not_read(x.eval(e, Some(pawn)), "the rounding of a fractional function-call flag result");
    assert!(text.starts_with("stopped after partial writes: "), "{text}");
    let _ = x.flag_entry;
}
