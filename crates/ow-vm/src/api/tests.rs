//! Owner: lane W2-VMAPI. Unit tests of the VM contract on a hand-built object space (invented classes and values,
//! rule 9).

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use ow_attr::AttributeSystem;
use ow_attr::api::{AttrError, AttrWorld, EngineQuery, EvalCx, GlobalSlots, NumForms, WorldParts};
use ow_attr::math::PowForm;
use ow_core::rand::{CrtStream, GameRng, Rand15, ScriptStream};
use ow_core::{Missing, NotYet, ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ClassId, ObjRef, PropId};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;

use super::*;

struct Fixture {
    vm: Vm,
    object: ClassId,
    pool: ClassId,
    sub_pool: ClassId,
}

/// Core.Object; Engine.Pool (CurrentValue: float) and a subclass Game.SubPool.
fn fixture(mode: ParityMode) -> Fixture {
    let mut space = ObjectSpace::new();
    let object = space.classes.define("Core.Object", None, vec![]).unwrap();
    let current = space.names.intern("CurrentValue");
    let pool = space
        .classes
        .define(
            "Engine.Pool",
            Some(object),
            vec![PropertyDecl::new(current, PropKind::Float)],
        )
        .unwrap();
    let sub_pool = space
        .classes
        .define("Game.SubPool", Some(pool), vec![])
        .unwrap();
    let vm = Vm::new(
        space,
        DefStore::new_empty(),
        Rc::new(AttributeSystem::new_empty()),
        mode,
    );
    Fixture {
        vm,
        object,
        pool,
        sub_pool,
    }
}

fn path(s: &str) -> ObjectPath {
    ObjectPath::parse(s).unwrap()
}

fn param(vm: &mut Vm, name: &str, kind: PropKind, flags: u64) -> PropertyDecl {
    let n = vm.space.names.intern(name);
    PropertyDecl::new(n, kind).with_flags(PropertyDecl::PARAM | flags)
}

fn ret(vm: &mut Vm, kind: PropKind) -> PropertyDecl {
    let n = vm.space.names.intern("ReturnValue");
    PropertyDecl::new(n, kind).with_flags(PropertyDecl::RETURN)
}

fn native(f: impl Fn(&mut NativeCall<'_>) -> Result<Value, VmError> + 'static) -> NativeFn {
    Rc::new(f)
}

/// `native function int Twice(int A, out int B, optional int C)` on Core.Object.
fn declare_twice(vm: &mut Vm) -> FuncId {
    let a = param(vm, "A", PropKind::Int, 0);
    let b = param(vm, "B", PropKind::Int, PropertyDecl::OUT);
    let c = param(vm, "C", PropKind::Int, PropertyDecl::OPTIONAL);
    let r = ret(vm, PropKind::Int);
    vm.declare_function(
        FunctionDecl::native(path("Core.Object.Twice"), "Object.Twice(int,int,int)")
            .with_params(vec![a, b, c])
            .with_result(r),
    )
    .unwrap()
}

fn int(v: &Value) -> i32 {
    match v {
        Value::Int(i) => *i,
        other => panic!("not an int: {other:?}"),
    }
}

#[test]
fn a_registered_native_runs_with_out_args() {
    let mut fx = fixture(ParityMode::Off);
    let vm = &mut fx.vm;
    let twice = declare_twice(vm);
    vm.register_native(
        "Object.Twice(int,int,int)",
        native(|c| {
            let a = int(c.arg(0)?);
            assert!(c.has(0) && c.has(1));
            assert!(!c.has(2), "the optional parameter was omitted");
            assert_eq!(c.arg(2)?, &Value::None);
            c.set_out(1, Value::Int(a * 2))?;
            Ok(Value::Int(a + 1))
        }),
    )
    .unwrap();
    let r = vm
        .call(twice, None, vec![Value::Int(21), Value::Int(0)])
        .unwrap();
    assert_eq!(r.ret, Value::Int(22));
    assert_eq!(r.outs, vec![Value::Int(21), Value::Int(42), Value::None]);
    assert!(vm.log.is_empty());
}

#[test]
fn argument_errors_are_reported_not_panics() {
    let mut fx = fixture(ParityMode::Off);
    let vm = &mut fx.vm;
    let twice = declare_twice(vm);
    vm.register_native(
        "Object.Twice(int,int,int)",
        native(|c| {
            assert!(matches!(
                c.set_out(0, Value::Int(1)),
                Err(VmError::BadArgument {
                    index: 0,
                    reason: "not an out parameter",
                    ..
                })
            ));
            assert!(matches!(
                c.arg(3),
                Err(VmError::BadArgument { index: 3, .. })
            ));
            assert!(!c.has(3));
            Ok(Value::Int(0))
        }),
    )
    .unwrap();
    vm.call(twice, None, vec![Value::Int(1)]).unwrap();
    let four = vec![Value::Int(1); 4];
    assert!(matches!(
        vm.call(twice, None, four),
        Err(VmError::Script { message, .. }) if message.starts_with("too many arguments")
    ));
}

#[test]
fn a_native_draws_from_the_vm_rng() {
    let mut fx = fixture(ParityMode::Off);
    let vm = &mut fx.vm;
    let r = ret(vm, PropKind::Int);
    let f = vm
        .declare_function(
            FunctionDecl::native(path("Core.Object.Draw"), "Object.Draw()").with_result(r),
        )
        .unwrap();
    vm.register_native(
        "Object.Draw()",
        native(|c| {
            let a = c.vm.rng.crt(CrtStream::AiDecision).rand15();
            let b = c.vm.rng.script(ScriptStream::Rand).rand_limit(100);
            Ok(Value::Int(i32::from(a) * 1000 + b))
        }),
    )
    .unwrap();
    let mut expect = GameRng::from_world_seed(0, ParityMode::Off);
    let a = expect.crt(CrtStream::AiDecision).rand15();
    let b = expect.script(ScriptStream::Rand).rand_limit(100);
    assert_eq!(
        vm.call(f, None, vec![]).unwrap().ret,
        Value::Int(i32::from(a) * 1000 + b)
    );
    // The stream moved: the next draw is the generator's second value.
    let a2 = expect.crt(CrtStream::AiDecision).rand15();
    let b2 = expect.script(ScriptStream::Rand).rand_limit(100);
    assert_eq!(
        vm.call(f, None, vec![]).unwrap().ret,
        Value::Int(i32::from(a2) * 1000 + b2)
    );
    // AttrWorld::rng is the same GameRng stream.
    let next = expect.crt(CrtStream::AttributeVariance).rand15();
    assert_eq!(
        AttrWorld::rng(vm, CrtStream::AttributeVariance)
            .unwrap()
            .rand15(),
        next
    );
}

#[test]
fn the_parity_mode_is_a_value_and_reaches_the_rng() {
    let off = fixture(ParityMode::Off).vm;
    let reference = fixture(ParityMode::Reference).vm;
    assert_eq!(off.parity, ParityMode::Off);
    assert_eq!(reference.parity, ParityMode::Reference);
    assert_eq!(reference.rng.mode(), ParityMode::Reference);
    let seeded = fixture(ParityMode::Off)
        .vm
        .with_rng(GameRng::from_world_seed(7, ParityMode::Off));
    assert_eq!(seeded.rng.world_seed(), 7);
}

#[derive(Debug, Default)]
struct Counter {
    calls: u32,
}

#[test]
fn a_native_uses_a_service() {
    let mut fx = fixture(ParityMode::Off);
    let vm = &mut fx.vm;
    vm.add_service(Counter::default()).unwrap();
    assert!(matches!(
        vm.add_service(Counter::default()),
        Err(VmError::Registration { .. })
    ));
    let f = vm
        .declare_function(FunctionDecl::native(
            path("Core.Object.Count"),
            "Object.Count()",
        ))
        .unwrap();
    vm.register_native(
        "Object.Count()",
        native(|c| {
            let counter =
                c.vm.service::<Counter>()
                    .ok_or(VmError::NotYet(NotYet::layer_absent(
                        "counter", "test", "test",
                    )))?;
            counter.borrow_mut().calls += 1;
            Ok(Value::None)
        }),
    )
    .unwrap();
    vm.call(f, None, vec![]).unwrap();
    vm.call(f, None, vec![]).unwrap();
    assert_eq!(vm.service::<Counter>().unwrap().borrow().calls, 2);
}

#[test]
fn a_service_borrowed_across_a_nested_call_is_refused() {
    let mut fx = fixture(ParityMode::Off);
    let vm = &mut fx.vm;
    vm.add_service(Counter::default()).unwrap();
    let inner = vm
        .declare_function(FunctionDecl::native(
            path("Core.Object.Inner"),
            "Object.Inner()",
        ))
        .unwrap();
    let outer = vm
        .declare_function(FunctionDecl::native(
            path("Core.Object.Outer"),
            "Object.Outer()",
        ))
        .unwrap();
    vm.register_native("Object.Inner()", native(|_| Ok(Value::Int(5))))
        .unwrap();
    vm.register_native(
        "Object.Outer()",
        native(move |c| {
            // Correct: the borrow ends before the nested call.
            {
                let s = c.vm.service::<Counter>().unwrap();
                s.borrow_mut().calls += 1;
            }
            assert_eq!(c.call_script(inner, None, vec![])?.ret, Value::Int(5));
            // Wrong: the borrow is live across it.
            let s = c.vm.service::<Counter>().unwrap();
            let guard = s.borrow_mut();
            let refused = c.call_script(inner, None, vec![]);
            drop(guard);
            assert!(matches!(refused, Err(VmError::ServiceBorrowed(n)) if n.ends_with("Counter")));
            Ok(Value::None)
        }),
    )
    .unwrap();
    vm.call(outer, None, vec![]).unwrap();
}

#[test]
fn the_attribute_system_is_called_from_inside_a_native() {
    let mut fx = fixture(ParityMode::Off);
    let pool = fx.pool;
    let vm = &mut fx.vm;
    let attr = vm.instantiate(pool, "Attr").unwrap();
    let r = ret(vm, PropKind::Float);
    let f = vm
        .declare_function(
            FunctionDecl::native(path("Core.Object.Eval"), "Object.Eval()").with_result(r),
        )
        .unwrap();
    vm.register_native(
        "Object.Eval()",
        native(move |c| {
            // The clone-the-Rc pattern: no borrow of c.vm.attrs is live while the VM is lent.
            let attrs = c.vm.attrs.clone();
            let got = attrs.value_of(attr, Some(attr), None, &mut EvalCx::new(c.vm));
            match got {
                Err(AttrError::NotYet(n)) => {
                    assert_eq!(n.missing, Missing::NotPorted);
                    Err(VmError::Attr(AttrError::NotYet(n)))
                }
                other => panic!("the empty system evaluates nothing yet: {other:?}"),
            }
        }),
    )
    .unwrap();
    assert!(matches!(
        vm.call(f, None, vec![]),
        Err(VmError::Attr(AttrError::NotYet(NotYet {
            missing: Missing::NotPorted,
            ..
        })))
    ));
}

#[test]
fn stub_policies() {
    let mut fx = fixture(ParityMode::Off);
    let vm = &mut fx.vm;
    let twice = declare_twice(vm);
    // The default refuses.
    assert_eq!(vm.stub_policy, StubPolicy::refuse_all());
    assert_eq!(
        vm.call(twice, None, vec![Value::Int(3), Value::Int(9)]),
        Err(VmError::UnboundNative("Object.Twice(int,int,int)".into()))
    );
    assert!(vm.log.is_empty());
    // LogAndZero: logged, zero result, out arguments untouched.
    vm.stub_policy = StubPolicy::LogAndZero;
    let r = vm
        .call(twice, None, vec![Value::Int(3), Value::Int(9)])
        .unwrap();
    assert_eq!(r.ret, Value::Int(0));
    assert_eq!(r.outs, vec![Value::Int(3), Value::Int(9), Value::None]);
    assert_eq!(
        vm.log,
        vec!["UNIMPLEMENTED Object.Twice(int,int,int)".to_string()]
    );
    // Refuse with the key allowed: stubbed as under LogAndZero.
    let mut allow = BTreeSet::new();
    allow.insert("Object.Twice(int,int,int)".to_string());
    vm.stub_policy = StubPolicy::Refuse { allow };
    assert_eq!(vm.call(twice, None, vec![]).unwrap().ret, Value::Int(0));
    assert_eq!(vm.log.len(), 2);
    // A void stub returns None.
    let void = vm
        .declare_function(FunctionDecl::native(
            path("Core.Object.Void"),
            "Object.Void()",
        ))
        .unwrap();
    vm.stub_policy = StubPolicy::LogAndZero;
    assert_eq!(vm.call(void, None, vec![]).unwrap().ret, Value::None);
}

#[test]
fn registration_rules() {
    let mut fx = fixture(ParityMode::Off);
    let vm = &mut fx.vm;
    let twice = declare_twice(vm);
    // Registered after the declaration: the function uses it.
    vm.register_native("Object.Twice(int,int,int)", native(|_| Ok(Value::Int(1))))
        .unwrap();
    assert!(vm.has_native("Object.Twice(int,int,int)"));
    assert!(matches!(
        vm.register_native("Object.Twice(int,int,int)", native(|_| Ok(Value::Int(2)))),
        Err(VmError::Registration {
            reason: "duplicate native registration",
            ..
        })
    ));
    assert_eq!(vm.call(twice, None, vec![]).unwrap().ret, Value::Int(1));
    // Paths are unique, compared case-insensitively.
    assert!(matches!(
        vm.declare_function(FunctionDecl::script(path("core.object.TWICE"))),
        Err(VmError::Registration { .. })
    ));
    assert_eq!(vm.find_function("CORE.Object.twice"), Ok(twice));
    assert!(matches!(
        vm.find_function("Core.Object.Missing"),
        Err(VmError::NotYet(NotYet {
            missing: Missing::LayerAbsent(_),
            ..
        }))
    ));
    assert_eq!(vm.find_class("engine.pool"), Ok(fx.pool));
    assert!(matches!(
        vm.find_class("Engine.Nope"),
        Err(VmError::NotYet(NotYet {
            missing: Missing::LayerAbsent(_),
            ..
        }))
    ));
    // Overrides: only of a declared script function, once.
    assert!(matches!(
        vm.override_script("Core.Object.Twice", native(|_| Ok(Value::None))),
        Err(VmError::Registration { .. })
    ));
    assert!(matches!(
        vm.override_script("Core.Object.Nope", native(|_| Ok(Value::None))),
        Err(VmError::Registration { .. })
    ));
}

#[test]
fn script_functions_run_only_through_an_override() {
    let mut fx = fixture(ParityMode::Off);
    let vm = &mut fx.vm;
    let x = param(vm, "X", PropKind::Int, 0);
    let r = ret(vm, PropKind::Int);
    let script = vm
        .declare_function(
            FunctionDecl::script(path("Game.Thing.Value"))
                .with_params(vec![x])
                .with_result(r),
        )
        .unwrap();
    assert!(matches!(
        vm.call(script, None, vec![Value::Int(1)]),
        Err(VmError::NotYet(NotYet {
            missing: Missing::NotPorted,
            ..
        }))
    ));
    vm.override_script(
        "Game.Thing.Value",
        native(|c| Ok(Value::Int(int(c.arg(0)?) * 10))),
    )
    .unwrap();
    assert_eq!(
        vm.call(script, None, vec![Value::Int(4)]).unwrap().ret,
        Value::Int(40)
    );
    assert!(matches!(
        vm.override_script("Game.Thing.Value", native(|_| Ok(Value::None))),
        Err(VmError::Registration {
            reason: "the script function is already overridden",
            ..
        })
    ));
}

#[test]
fn iterators_bind_by_key() {
    let mut fx = fixture(ParityMode::Off);
    let vm = &mut fx.vm;
    let o = param(vm, "A", PropKind::Int, PropertyDecl::OUT);
    let f = vm
        .declare_function(
            FunctionDecl::native(path("Engine.Actor.AllThings"), "Actor.AllThings(int)")
                .with_params(vec![o]),
        )
        .unwrap();
    assert!(vm.iterator_for(f).is_none());
    let it: IteratorFn =
        Rc::new(|_| Ok(IteratorResult::new(0, vec![Value::Int(1), Value::Int(2)])));
    vm.register_iterator("Actor.AllThings(int)", it.clone())
        .unwrap();
    assert!(matches!(
        vm.register_iterator("Actor.AllThings(int)", it),
        Err(VmError::Registration { .. })
    ));
    assert!(vm.iterator_for(f).is_some());
}

#[test]
fn step_limit_census_and_trace() {
    let mut fx = fixture(ParityMode::Off);
    let vm = &mut fx.vm;
    let inner = vm
        .declare_function(FunctionDecl::native(
            path("Core.Object.Inner"),
            "Object.Inner()",
        ))
        .unwrap();
    let outer = vm
        .declare_function(FunctionDecl::native(
            path("Core.Object.Outer"),
            "Object.Outer()",
        ))
        .unwrap();
    let stub = vm
        .declare_function(FunctionDecl::native(
            path("Core.Object.Stub"),
            "Object.Stub()",
        ))
        .unwrap();
    vm.register_native("Object.Inner()", native(|_| Ok(Value::None)))
        .unwrap();
    vm.register_native(
        "Object.Outer()",
        native(move |c| {
            c.call_script(inner, None, vec![])?;
            c.call_script(inner, None, vec![])?;
            Ok(Value::None)
        }),
    )
    .unwrap();
    vm.census = Some(CallCensus::default());
    vm.trace = Some(Vec::new());
    vm.stub_policy = StubPolicy::LogAndZero;
    vm.call(outer, None, vec![]).unwrap();
    vm.call(stub, None, vec![]).unwrap();
    assert_eq!(
        vm.trace.as_deref().unwrap(),
        [
            "Core.Object.Outer",
            "Core.Object.Inner",
            "Core.Object.Inner",
            "Core.Object.Stub"
        ]
    );
    let census = vm.census.as_ref().unwrap();
    assert_eq!(census.native_calls.get("Core.Object.Inner"), Some(&2));
    assert_eq!(census.native_calls.get("Core.Object.Outer"), Some(&1));
    assert_eq!(census.stub_calls.get("Core.Object.Stub"), Some(&1));
    // Three steps per top-level outer call: nested calls share its budget, a new top-level call starts afresh.
    vm.limits.step_limit = 3;
    vm.call(outer, None, vec![]).unwrap();
    vm.call(outer, None, vec![]).unwrap();
    vm.limits.step_limit = 2;
    assert_eq!(vm.call(outer, None, vec![]), Err(VmError::StepLimit));
    // The failed call left no call in progress: the next one is top-level again.
    vm.limits.step_limit = 3;
    vm.call(outer, None, vec![]).unwrap();
}

#[test]
fn instantiate_names() {
    let mut fx = fixture(ParityMode::Off);
    let named = fx.vm.instantiate(fx.pool, "Pool_7").unwrap();
    let obj = fx.vm.space.heap.get(named).unwrap();
    assert_eq!(fx.vm.space.names.text(obj.name), "Pool_7");
    assert_eq!(obj.origin, Origin::Runtime);
    assert!(matches!(
        fx.vm.instantiate(fx.pool, ""),
        Err(VmError::NotYet(NotYet {
            missing: Missing::NotRead,
            ..
        }))
    ));
    let mut rf = fixture(ParityMode::Reference);
    let o = rf.vm.instantiate(rf.sub_pool, "").unwrap();
    let obj = rf.vm.space.heap.get(o).unwrap();
    assert_eq!(rf.vm.space.names.text(obj.name), "SubPool_0");
}

#[test]
fn world_queries_without_hooks_are_layer_absent() {
    let mut fx = fixture(ParityMode::Off);
    let o = fx.vm.instantiate(fx.object, "O").unwrap();
    let absent = |r: Result<Option<ObjRef>, AttrError>| {
        matches!(
            r,
            Err(AttrError::NotYet(NotYet {
                missing: Missing::LayerAbsent("ow-world"),
                ..
            }))
        )
    };
    // globals() answers an object, not an option; folded in for the one check.
    assert!(absent(fx.vm.globals().map(Some)));
    assert!(absent(fx.vm.game_info()));
    assert!(absent(fx.vm.engine_query(EngineQuery::PlayerController, o)));
    // parts() lends the space and the store at once.
    let mut parts = fx.vm.parts();
    assert!(parts.defs().is_ok());
    assert!(parts.space.heap.contains(o));
}

/// A world hook that answers from objects it made in the space.
struct TestHooks {
    globals: Option<ObjRef>,
    queries: Vec<EngineQuery>,
}

impl WorldHooks for TestHooks {
    fn globals(&mut self, w: WorldParts<'_>) -> Result<ObjRef, AttrError> {
        match self.globals {
            Some(g) => Ok(g),
            None => {
                let c = w.space.classes.by_path("Core.Object").unwrap();
                let g = w.space.alloc(c, "Globals", None, Origin::Runtime)?;
                self.globals = Some(g);
                Ok(g)
            }
        }
    }
    fn game_info(&mut self, w: WorldParts<'_>) -> Result<ObjRef, AttrError> {
        // Invented: this test world's game info is its globals object.
        self.globals(w)
    }
    fn engine_query(
        &mut self,
        q: EngineQuery,
        source: ObjRef,
        _w: WorldParts<'_>,
    ) -> Result<Option<ObjRef>, AttrError> {
        self.queries.push(q);
        Ok(Some(source))
    }
}

#[test]
fn world_queries_go_through_the_hooks() {
    let mut fx = fixture(ParityMode::Off);
    let hooks = Rc::new(RefCell::new(TestHooks {
        globals: None,
        queries: Vec::new(),
    }));
    fx.vm.set_world_hooks(hooks.clone()).unwrap();
    let again: Rc<RefCell<dyn WorldHooks>> = hooks.clone();
    assert!(matches!(
        fx.vm.set_world_hooks(again),
        Err(VmError::Registration { .. })
    ));
    let g = fx.vm.globals().unwrap();
    assert_eq!(fx.vm.globals(), Ok(g));
    assert!(fx.vm.space.heap.contains(g));
    assert_eq!(fx.vm.game_info(), Ok(Some(g)));
    let o = fx.vm.instantiate(fx.object, "O").unwrap();
    assert_eq!(
        fx.vm.engine_query(EngineQuery::PlayerController, o),
        Ok(Some(o))
    );
    assert_eq!(hooks.borrow().queries, vec![EngineQuery::PlayerController]);
    assert!(fx.vm.world_hooks().is_some());
}

#[test]
fn attribute_change_dispatches_the_nearest_override() {
    let mut fx = fixture(ParityMode::Off);
    let current: PropId = fx.vm.space.prop(fx.pool, "CurrentValue").unwrap();
    let pool = fx.vm.instantiate(fx.pool, "P").unwrap();
    let sub = fx.vm.instantiate(fx.sub_pool, "S").unwrap();
    let plain = fx.vm.instantiate(fx.object, "O").unwrap();
    let forms = NumForms {
        pow: PowForm::Single,
        ..NumForms::default()
    };
    // No override in the chain: nothing happens.
    assert_eq!(
        fx.vm.attribute_changed(pool, current, NumForms::default()),
        Ok(())
    );
    // An invented clamp on Engine.Pool, reached from the subclass too; it writes through the VM it is lent.
    fx.vm
        .register_attribute_notify(
            "engine.pool",
            Rc::new(|vm: &mut Vm, owner, prop, forms: NumForms| {
                // The calling evaluation's forms arrive, for a nested EvalCx.
                assert_eq!(forms.pow, PowForm::Single);
                let cx = EvalCx::new(vm).with_forms(forms);
                assert_eq!(cx.forms, forms);
                vm.space.write(owner, prop.into(), Value::Float(1.5))?;
                Ok(())
            }),
        )
        .unwrap();
    assert!(matches!(
        fx.vm
            .register_attribute_notify("Engine.Pool", Rc::new(|_, _, _, _| Ok(()))),
        Err(VmError::Registration { .. })
    ));
    fx.vm.attribute_changed(sub, current, forms).unwrap();
    assert_eq!(
        fx.vm.space.read(sub, current.into()),
        Ok(&Value::Float(1.5))
    );
    assert_eq!(
        fx.vm.space.read(pool, current.into()),
        Ok(&Value::Float(0.0))
    );
    assert_eq!(fx.vm.attribute_changed(plain, current, forms), Ok(()));
    // A stale owner is the heap's error.
    fx.vm.space.free(plain).unwrap();
    assert!(matches!(
        fx.vm.attribute_changed(plain, current, forms),
        Err(AttrError::Heap(_))
    ));
}

#[test]
fn native_packs_register_into_the_vm() {
    struct Pack;
    impl NativePack for Pack {
        fn register(&self, vm: &mut Vm) -> Result<(), VmError> {
            vm.register_native("Object.Twice(int,int,int)", Rc::new(|_| Ok(Value::Int(7))))?;
            vm.add_service(Counter::default())?;
            Ok(())
        }
    }
    let mut fx = fixture(ParityMode::Off);
    let twice = declare_twice(&mut fx.vm);
    Pack.register(&mut fx.vm).unwrap();
    assert!(Pack.register(&mut fx.vm).is_err(), "a pack registers once");
    assert_eq!(fx.vm.call(twice, None, vec![]).unwrap().ret, Value::Int(7));
    assert!(fx.vm.service::<Counter>().is_some());
}

#[test]
fn the_world_owns_the_global_slots() {
    let mut fx = fixture(ParityMode::Off);
    assert_eq!(fx.vm.global_slots, GlobalSlots::default());
    fx.vm.global_slots().set(0, 12.0).unwrap();
    assert_eq!(fx.vm.global_slots.experience_point_test_level, 12.0);
    // One process-wide pair: an evaluation context sees what was set before it.
    let cx = EvalCx::new(&mut fx.vm);
    assert_eq!(cx.world.global_slots().get(0), 12.0);
}
