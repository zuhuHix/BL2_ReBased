//! `GetParticleEffect` on an invented `GearboxFramework.upk` in a synthetic store (AGENTS.md rule 9; this lane's decision
//! 2026-11-19-188). Loading `ToyHolder` makes its references (the collection, four evaluators and three particle
//! systems) store handles that are not loaded; a test loads the ones it needs as its own set-up step. The evaluators'
//! `Evaluate` is a test native whose answer comes from the evaluator's `Mode` and `Answer`. Expected values are
//! written by hand from NATIVE_GET_PARTICLE_EFFECT.md sections 2 and 4. AI-assisted (Claude Code).

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::{Missing, NotYet, ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_object::ids::{ObjRef, PropAddr, StructId};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_package::testkit::reflect::{toy_array_of, toy_struct};
use ow_package::testkit::script::{Asm, DEFAULT_FLAGS, FunctionSpec, Param, ScriptToy};
use ow_vm::api::{NativeCall, NativeFn, Vm};
use ow_vm::error::VmError;

use super::*;

static NEXT: AtomicU32 = AtomicU32::new(0);

/// The handles the holder names, in this order.
const HANDLES: [&str; 12] = ["Coll", "E1", "E2", "E3", "E4", "FxA", "FxB", "FxD", "Bad", "Aee", "Attr", "Res"];

/// `Mode` of a test evaluator: answer `Answer`; write `Hits` then refuse; refuse without a write; answer an int.
const ANSWER: i32 = 0;
const WRITE_THEN_REFUSE: i32 = 1;
const REFUSE: i32 = 2;
const NOT_A_BOOL: i32 = 3;

const REFUSAL: &str = "the test evaluator's rule";
const TEST_RECORD: &str = "a test record";

fn package() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&[
        "Class",
        "Function",
        "ScriptStruct",
        "BoolProperty",
        "IntProperty",
        "ObjectProperty",
        "StructProperty",
        "ArrayProperty",
    ]);
    let object = t.class("Object", 0);
    let system = t.class("ParticleSystem", object);
    let evaluator = t.class("ExpressionEvaluator", object);
    t.prop("Int", evaluator, "Mode", 0, 0);
    t.prop("Bool", evaluator, "Answer", 0, 0);
    t.prop("Int", evaluator, "Hits", 0, 0);
    let context = |object| [Param { type_ref: object, ..Param::new("Object", "ContextObject") }];
    native(&mut t, evaluator, "Evaluate", &context(object), Some(("Bool", 0)));
    let unbound = t.class("UnboundEvaluator", evaluator);
    native(&mut t, unbound, "Evaluate", &context(object), Some(("Bool", 0)));
    // A script evaluator: `Items.AddItem(1)`, then `return Refuse()`, a native the test binds to a NotYet. Its only
    // write is the script's array change (#573 r1, item 1).
    let scripted = t.class("ScriptEvaluator", evaluator);
    let items = toy_array_of(&mut t, scripted, "Items", "Int", 0);
    let refuse = t.function(scripted, "Refuse", &[], Some(("Bool", 0)), &[], &FunctionSpec::new(NATIVE, "Refuse"), |_| None);
    t.function(scripted, "Evaluate", &context(object), Some(("Bool", 0)), &[], &FunctionSpec::new(DEFAULT_FLAGS, "Evaluate"), |_| {
        let mut a = Asm::new();
        a.stmt();
        a.raw(&[0x55]).instance(items).w(0).int_const(1).raw(&[0x16]);
        a.stmt().raw(&[0x04]).final_function(refuse.function, |_| {});
        a.end();
        Some(a)
    });
    // An attribute evaluator (its Evaluate is the inherited test native), an attribute definition with its two chains,
    // and a value resolver class nothing implements (#573 r1, item 2).
    let aee = t.class("AttributeExpressionEvaluator", evaluator);
    let attribute = t.class("AttributeDefinition", object);
    let data = toy_struct(&mut t, aee, "AttributeExpressionData", None);
    t.prop("Object", data, "AttributeOperand1", 0, attribute);
    t.prop("Object", data, "AttributeOperand2", 0, attribute);
    t.prop("Struct", aee, "Expression", 0, data);
    toy_array_of(&mut t, attribute, "ContextResolverChain", "Object", object);
    toy_array_of(&mut t, attribute, "ValueResolverChain", "Object", object);
    let unknown = t.class("UnknownValueResolver", object);
    let collection = t.class("EffectCollectionDefinition", object);
    let entry = toy_struct(&mut t, collection, "ConditionalParticleEffectData", None);
    t.prop("Object", entry, "Expression", 0, evaluator);
    t.prop("Object", entry, "ParticleEffect", 0, system);
    let inner = t.p.next_export();
    t.prop("Array", collection, "ParticleEffects", 0, inner + 1);
    t.prop("Struct", inner, "ParticleEffects", 0, entry);
    native(&mut t, collection, "GetParticleEffect", &context(object), Some(("Object", system)));
    // A malformed collection: ParticleEffects is an int.
    let bad = t.class("BadCollection", object);
    t.prop("Int", bad, "ParticleEffects", 0, 0);
    let holder = t.class("Holder", object);
    for n in HANDLES {
        t.prop("Object", holder, n, 0, object);
    }
    let mut empty = 0i32.to_le_bytes().to_vec();
    empty.extend_from_slice(&t.none);
    let classes = [collection, evaluator, evaluator, evaluator, evaluator, system, system, system, bad, aee, attribute, unknown];
    let names = ["ToyColl", "ToyE1", "ToyE2", "ToyE3", "ToyE4", "ToyFxA", "ToyFxB", "ToyFxD", "ToyBad", "ToyAee", "ToyAttr", "ToyRes"];
    let exports: Vec<i32> = classes.iter().zip(names).map(|(&c, n)| t.p.add_export(c, n, empty.clone())).collect();
    let mut h = 0i32.to_le_bytes().to_vec();
    for (n, e) in HANDLES.iter().zip(&exports) {
        h.extend(t.p.object_tag(n, *e));
    }
    h.extend_from_slice(&t.none);
    t.p.add_export(holder, "ToyHolder", h);
    t.p.build()
}

/// Each `Evaluate` call: the evaluator and its context.
type Calls = Rc<RefCell<Vec<(ObjRef, Option<ObjRef>)>>>;

struct Fx {
    vm: Vm,
    dir: PathBuf,
    /// The holder's handles, in [`HANDLES`] order.
    h: Vec<ObjRef>,
    calls: Calls,
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn read(vm: &Vm, o: ObjRef, name: &str) -> Value {
    crate::equip::read_prop(vm, o, name).unwrap()
}

/// `FUNC_Native | FUNC_Public`.
const NATIVE: u32 = 0x400 | 0x20000;

fn native(t: &mut ScriptToy, owner: i32, name: &str, params: &[Param], result: Option<(&str, i32)>) {
    t.function(owner, name, params, result, &[], &FunctionSpec::new(NATIVE, name), |_| None);
}

fn fx_mode(mode: ParityMode) -> Fx {
    let dir = std::env::temp_dir().join(format!(
        "ow-inventory-particle-effect-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let bytes = package();
    let defs = DefStore::from_synthetic(&dir, &[("GearboxFramework.upk", &bytes)], IdentityPolicy::FirstInScanOrder, mode)
        .unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), mode);
    let path = ObjectPath::parse("GearboxFramework.ToyHolder").unwrap();
    let holder = vm.defs.find(&mut vm.space, &path).unwrap().unwrap();
    let h: Vec<ObjRef> = HANDLES
        .iter()
        .map(|n| match read(&vm, holder, n) {
            Value::Object(Some(o)) => o,
            other => panic!("{n}: {other:?}"),
        })
        .collect();
    assert!(h.iter().all(|&o| vm.defs.is_unloaded(&vm.space, o)), "every handle starts unloaded");
    let calls: Calls = Rc::default();
    let seen = calls.clone();
    let evaluate: NativeFn = Rc::new(move |c: &mut NativeCall<'_>| {
        let me = c.this.unwrap();
        let context = match c.arg(0)? {
            Value::Object(o) => *o,
            other => panic!("{other:?}"),
        };
        seen.borrow_mut().push((me, context));
        let mode = match read(c.vm, me, "Mode") {
            Value::Int(m) => m,
            other => panic!("{other:?}"),
        };
        match mode {
            ANSWER => Ok(read(c.vm, me, "Answer")),
            WRITE_THEN_REFUSE => {
                let class = c.vm.space.heap.get(me)?.class;
                let p = c.vm.space.prop(class, "Hits").unwrap();
                c.vm.space.write(me, PropAddr::scalar(p), Value::Int(1))?;
                Err(NotYet::not_read(REFUSAL, TEST_RECORD).into())
            }
            REFUSE => Err(NotYet::not_read(REFUSAL, TEST_RECORD).into()),
            NOT_A_BOOL => Ok(Value::Int(1)),
            other => panic!("mode {other}"),
        }
    });
    vm.register_native("ExpressionEvaluator.Evaluate(object)", evaluate).unwrap();
    let refuse: NativeFn = Rc::new(|_c: &mut NativeCall<'_>| Err(NotYet::not_read(REFUSAL, TEST_RECORD).into()));
    vm.register_native("ScriptEvaluator.Refuse()", refuse).unwrap();
    register(&mut vm).unwrap();
    Fx { vm, dir, h, calls }
}

fn fx() -> Fx {
    fx_mode(ParityMode::Off)
}

impl Fx {
    fn handle(&self, name: &str) -> ObjRef {
        self.h[HANDLES.iter().position(|n| *n == name).unwrap()]
    }

    /// The handle `name` loaded (the test's own set-up step).
    fn load(&mut self, name: &str) -> ObjRef {
        let o = self.handle(name);
        self.vm.defs.load(&mut self.vm.space, o).unwrap()
    }

    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap();
        self.vm.space.write(o, PropAddr::scalar(p), v).unwrap();
    }

    /// An evaluator, loaded, with its mode and answer.
    fn evaluator(&mut self, name: &str, mode: i32, answer: bool) -> ObjRef {
        let e = self.load(name);
        self.set(e, "Mode", Value::Int(mode));
        self.set(e, "Answer", Value::Bool(answer));
        e
    }

    fn entry_type(&self, collection: ObjRef) -> StructId {
        let class = self.vm.space.heap.get(collection).unwrap().class;
        let p = self.vm.space.prop(class, "ParticleEffects").unwrap();
        match &self.vm.space.classes.decl(p).unwrap().kind {
            PropKind::Array(inner) => match **inner {
                PropKind::Struct(t) => t,
                _ => panic!(),
            },
            _ => panic!(),
        }
    }

    /// The collection loaded, holding `entries` (Expression, ParticleEffect) in order.
    fn collection(&mut self, entries: &[(Option<ObjRef>, Option<ObjRef>)]) -> ObjRef {
        let c = self.load("Coll");
        let ty = self.entry_type(c);
        let (e, p) = (self.vm.space.names.intern("Expression"), self.vm.space.names.intern("ParticleEffect"));
        let layout = self.vm.space.classes.struct_layout(ty).unwrap();
        let (ei, pi) = (layout.field(e).unwrap().0 as usize, layout.field(p).unwrap().0 as usize);
        let values = entries
            .iter()
            .map(|&(x, fx)| {
                let mut v = self.vm.space.classes.struct_defaults(ty).unwrap();
                let s = v.struct_mut().unwrap();
                s.fields[ei] = Value::Object(x);
                s.fields[pi] = Value::Object(fx);
                v
            })
            .collect();
        self.set(c, "ParticleEffects", Value::array(values));
        c
    }

    fn counters(&self) -> (u64, u64, u64) {
        (self.vm.space.heap.committed_writes(), self.vm.rng.draws(), (self.vm.defs.load_transitions() + self.vm.class_code_loads()))
    }

    fn run(&mut self, collection: ObjRef, context: Option<ObjRef>) -> Result<Option<ObjRef>, VmError> {
        get_particle_effect(&mut self.vm, collection, context)
    }

    fn called(&self) -> Vec<ObjRef> {
        self.calls.borrow().iter().map(|c| c.0).collect()
    }
}

fn not_yet(e: VmError) -> NotYet {
    match e {
        VmError::NotYet(n) => n,
        other => panic!("expected a NotYet, found {other:?}"),
    }
}

fn script_message(e: VmError) -> String {
    match e {
        VmError::Script { function, message } => {
            assert_eq!(function, GET_PARTICLE_EFFECT);
            message
        }
        other => panic!("expected a script error, found {other:?}"),
    }
}

/// Section 2: the first entry that passes, in array order; later entries are not asked.
#[test]
fn the_first_passing_entry_gives_its_effect_in_array_order() {
    let mut f = fx();
    let (a, b, d) = (f.handle("FxA"), f.handle("FxB"), f.handle("FxD"));
    let e1 = f.evaluator("E1", ANSWER, false);
    let e2 = f.evaluator("E2", ANSWER, true);
    let e3 = f.evaluator("E3", ANSWER, true);
    let c = f.collection(&[(Some(e1), Some(a)), (Some(e2), Some(b)), (Some(e3), Some(d)), (None, Some(d))]);
    let weapon = f.handle("Bad");
    assert_eq!(f.run(c, Some(weapon)).unwrap(), Some(b));
    assert_eq!(*f.calls.borrow(), vec![(e1, Some(weapon)), (e2, Some(weapon))], "the context is the argument");
}

/// Section 4, "not a stand-in": the default entry comes only after every earlier entry answered false.
#[test]
fn the_default_entry_comes_only_after_every_earlier_entry_is_evaluated() {
    let mut f = fx();
    let (a, d) = (f.handle("FxA"), f.handle("FxD"));
    let es: Vec<ObjRef> = ["E1", "E2", "E3"].iter().map(|n| f.evaluator(n, ANSWER, false)).collect();
    let mut entries: Vec<(Option<ObjRef>, Option<ObjRef>)> = es.iter().map(|&e| (Some(e), Some(a))).collect();
    entries.push((None, Some(d)));
    let c = f.collection(&entries);
    assert_eq!(f.run(c, None).unwrap(), Some(d));
    assert_eq!(f.called(), es);
    assert_eq!(f.calls.borrow()[0].1, None, "a None argument is passed as it is");
}

/// Section 2: no entry passing, or an empty array, gives None; a passing entry may hold no effect.
#[test]
fn no_passing_entry_or_an_empty_array_gives_none() {
    let mut f = fx();
    let a = f.handle("FxA");
    let e1 = f.evaluator("E1", ANSWER, false);
    let c = f.collection(&[(Some(e1), Some(a))]);
    assert_eq!(f.run(c, None).unwrap(), None);
    let c = f.collection(&[]);
    assert_eq!(f.run(c, None).unwrap(), None);
    let c = f.collection(&[(None, None), (None, Some(a))]);
    assert_eq!(f.run(c, None).unwrap(), None, "the first entry passes, with no effect");
}

/// Section 2: the effect handle is returned as stored, and the native loads and writes nothing of its own.
#[test]
fn the_effect_handle_is_returned_unloaded_and_nothing_is_written_or_loaded() {
    let mut f = fx();
    let b = f.handle("FxB");
    let e1 = f.evaluator("E1", ANSWER, true);
    let c = f.collection(&[(Some(e1), Some(b))]);
    // #518's three only: the code loads this run makes are lane COUNTERS-MISS-CLASS-CODE-LOADS's finding, pinned in the
    // ignored test below.
    let three = |f: &Fx| (f.vm.space.heap.committed_writes(), f.vm.rng.draws(), f.vm.defs.load_transitions());
    let before = three(&f);
    assert_eq!(f.run(c, None).unwrap(), Some(b));
    assert!(f.vm.defs.is_unloaded(&f.vm.space, b), "the effect is not loaded");
    assert_eq!(three(&f), before);
}

/// Lane COUNTERS-MISS-CLASS-CODE-LOADS's finding: on this fixture the same run loads class code (the evaluator
/// dispatch loads `GearboxFramework.ExpressionEvaluator`'s and `GearboxFramework.Object`'s code, three code loads),
/// which #518's three counters do not see. Ignored until the hub routes it; run with `--ignored` to see it.
#[test]
#[ignore = "finding: GetParticleEffect's evaluator dispatch loads cold class code (lane COUNTERS-MISS-CLASS-CODE-LOADS)"]
fn the_effect_lookup_loads_no_class_code() {
    let mut f = fx();
    let b = f.handle("FxB");
    let e1 = f.evaluator("E1", ANSWER, true);
    let c = f.collection(&[(Some(e1), Some(b))]);
    let before = f.vm.class_code_loads();
    assert_eq!(f.run(c, None).unwrap(), Some(b));
    assert_eq!(f.vm.class_code_loads(), before, "no class code loaded");
}

/// Preflight 1: an unloaded collection refuses without loading it.
#[test]
fn an_unloaded_collection_refuses_without_a_load() {
    let mut f = fx();
    let c = f.handle("Coll");
    let before = f.counters();
    let n = not_yet(f.run(c, None).unwrap_err());
    assert_eq!((n.what.as_ref(), n.missing, n.record), (COLLECTION_NOT_LOADED, Missing::NotRead, RECORD));
    assert!(f.vm.defs.is_unloaded(&f.vm.space, c));
    assert_eq!(f.counters(), before);
}

/// Preflight 3: an unloaded evaluator up to the first entry with no Expression refuses before any evaluator runs.
#[test]
fn an_unloaded_reachable_evaluator_refuses_before_any_evaluation() {
    let mut f = fx();
    let (a, d) = (f.handle("FxA"), f.handle("FxD"));
    let e1 = f.evaluator("E1", ANSWER, true);
    let e2 = f.handle("E2");
    let c = f.collection(&[(Some(e1), Some(a)), (Some(e2), Some(a)), (None, Some(d))]);
    let before = f.counters();
    let n = not_yet(f.run(c, None).unwrap_err());
    assert_eq!((n.what.as_ref(), n.missing, n.record), (EVALUATOR_NOT_LOADED, Missing::NotRead, RECORD));
    assert!(f.called().is_empty(), "E1 would pass, but the preflight comes first");
    assert!(f.vm.defs.is_unloaded(&f.vm.space, e2));
    assert_eq!(f.counters(), before);
}

/// Preflight 3: entries after the first with no Expression can never be reached, so they are not checked.
#[test]
fn an_unloaded_evaluator_after_the_default_entry_is_not_checked() {
    let mut f = fx();
    let (a, d) = (f.handle("FxA"), f.handle("FxD"));
    let e1 = f.evaluator("E1", ANSWER, false);
    let e4 = f.handle("E4");
    let c = f.collection(&[(Some(e1), Some(a)), (None, Some(d)), (Some(e4), Some(a))]);
    assert_eq!(f.run(c, None).unwrap(), Some(d));
    assert!(f.vm.defs.is_unloaded(&f.vm.space, e4));
}

/// Preflight 2: ParticleEffects declared as something else is a script error naming the field.
#[test]
fn a_malformed_declaration_is_a_script_error() {
    let mut f = fx();
    let bad = f.load("Bad");
    let m = script_message(f.run(bad, None).unwrap_err());
    assert!(m.contains("ParticleEffects"), "{m}");
}

/// The error rule: an evaluator's NotYet after a counter moved takes the mark once, kind and record kept.
#[test]
fn an_evaluator_refusal_after_a_write_is_marked_once() {
    let mut f = fx();
    let a = f.handle("FxA");
    let e1 = f.evaluator("E1", WRITE_THEN_REFUSE, false);
    let c = f.collection(&[(Some(e1), Some(a))]);
    let n = not_yet(f.run(c, None).unwrap_err());
    assert_eq!(n.what, format!("{}{REFUSAL}", ow_core::notyet::PARTIAL_WRITES_PREFIX));
    assert_eq!((n.missing, n.record), (Missing::NotRead, TEST_RECORD));
}

/// The error rule (#573 r1, item 1): a script evaluator that changes an array and then refuses. The array change
/// moves #518's write counter only with #518's VM instrumentation (copied here from #518), so the refusal is marked.
#[test]
fn a_script_evaluator_that_changes_an_array_then_refuses_is_marked() {
    let mut f = fx();
    let a = f.handle("FxA");
    let class = f.vm.find_class("GearboxFramework.ScriptEvaluator").unwrap();
    let e = f.vm.instantiate(class, "Scripted").unwrap();
    let c = f.collection(&[(Some(e), Some(a))]);
    let n = not_yet(f.run(c, None).unwrap_err());
    assert_eq!(n.what, format!("{}{REFUSAL}", ow_core::notyet::PARTIAL_WRITES_PREFIX));
    assert_eq!((n.missing, n.record), (Missing::NotRead, TEST_RECORD));
    assert_eq!(read(&f.vm, e, "Items"), Value::array(vec![Value::Int(1)]), "the script's array change happened");
}

/// The error rule: with no counter moved, the evaluator's NotYet passes through unchanged.
#[test]
fn an_evaluator_refusal_with_no_write_passes_through_unchanged() {
    let mut f = fx();
    let a = f.handle("FxA");
    let e1 = f.evaluator("E1", ANSWER, false);
    let e2 = f.evaluator("E2", REFUSE, false);
    let c = f.collection(&[(Some(e1), Some(a)), (Some(e2), Some(a))]);
    let n = not_yet(f.run(c, None).unwrap_err());
    assert_eq!((n.what.as_ref(), n.missing, n.record), (REFUSAL, Missing::NotRead, TEST_RECORD));
}

/// The walk: an Evaluate that answers something other than a bool is a script error.
#[test]
fn a_non_bool_answer_is_a_script_error() {
    let mut f = fx();
    let a = f.handle("FxA");
    let e1 = f.evaluator("E1", NOT_A_BOOL, false);
    let c = f.collection(&[(Some(e1), Some(a))]);
    let m = script_message(f.run(c, None).unwrap_err());
    assert!(m.contains("returned Int"), "{m}");
}

/// The error rule: an unbound evaluator passes through unchanged.
#[test]
fn an_unbound_evaluator_passes_through_unchanged() {
    let mut f = fx();
    let a = f.handle("FxA");
    let e1 = f.load("E1");
    let unbound = f.vm.find_class("GearboxFramework.UnboundEvaluator").unwrap();
    let u = f.vm.instantiate(unbound, "Unbound").unwrap();
    let c = f.collection(&[(Some(u), Some(a)), (Some(e1), Some(a))]);
    match f.run(c, None).unwrap_err() {
        VmError::UnboundNative(key) => assert_eq!(key, "UnboundEvaluator.Evaluate(object)"),
        other => panic!("{other:?}"),
    }
}

/// The binding: called through the VM as the script calls it.
#[test]
fn the_bound_native_answers_through_the_vm() {
    let mut f = fx();
    let b = f.handle("FxB");
    let e1 = f.evaluator("E1", ANSWER, true);
    let c = f.collection(&[(Some(e1), Some(b))]);
    let class = f.vm.space.heap.get(c).unwrap().class;
    let func = f.vm.find_method(class, "GetParticleEffect", None).unwrap().unwrap();
    let weapon = f.handle("Bad");
    let r = f.vm.call(func, Some(c), vec![Value::Object(Some(weapon))]).unwrap().ret;
    assert_eq!(r, Value::Object(Some(b)));
    assert_eq!(*f.calls.borrow(), vec![(e1, Some(weapon))]);
}

/// Reference keeps the stub policy: nothing is bound.
#[test]
fn reference_binds_nothing() {
    let f = fx_mode(ParityMode::Reference);
    assert!(!f.vm.has_native(GET_PARTICLE_EFFECT));
}

impl Fx {
    /// The attribute evaluator loaded, answering `answer`, its first operand `operand`.
    fn attribute_evaluator(&mut self, operand: ObjRef, answer: bool) -> ObjRef {
        let e = self.evaluator("Aee", ANSWER, answer);
        let mut expression = read(&self.vm, e, "Expression");
        let ty = expression.as_struct().unwrap().ty;
        let id = self.vm.space.names.intern("AttributeOperand1");
        let at = self.vm.space.classes.struct_layout(ty).unwrap().field(id).unwrap().0 as usize;
        expression.struct_mut().unwrap().fields[at] = Value::Object(Some(operand));
        self.set(e, "Expression", expression);
        e
    }
}

/// Preflight rule 4 (#573 r1, item 2): an attribute evaluator's cold operand refuses before any load or evaluation.
#[test]
fn a_cold_operand_refuses_before_any_load() {
    let mut f = fx();
    let (attr, a) = (f.handle("Attr"), f.handle("FxA"));
    let e = f.attribute_evaluator(attr, true);
    let c = f.collection(&[(Some(e), Some(a))]);
    let before = f.counters();
    let n = not_yet(f.run(c, None).unwrap_err());
    assert_eq!(n.missing, Missing::NotRead);
    assert!(n.what.starts_with("the attribute definition (") && n.what.ends_with("is not loaded (a load is a write)"), "{}", n.what);
    assert!(!ow_core::notyet::is_partial_writes(&n.what));
    assert!(f.called().is_empty(), "no evaluator ran");
    assert!(f.vm.defs.is_unloaded(&f.vm.space, attr));
    assert_eq!(f.counters(), before);
}

/// Preflight rule 4: with the operand and its resolver resident (a harness preload), a value resolver class nothing
/// implements is refused by its name before any evaluation, with no partial-writes mark.
#[test]
fn an_unimplemented_resolver_of_a_resident_operand_refuses_before_any_evaluation() {
    let mut f = fx();
    let a = f.handle("FxA");
    let attr = f.load("Attr");
    let res = f.load("Res");
    f.set(attr, "ValueResolverChain", Value::array(vec![Value::Object(Some(res))]));
    let e = f.attribute_evaluator(attr, true);
    let c = f.collection(&[(Some(e), Some(a))]);
    let before = f.counters();
    let n = not_yet(f.run(c, None).unwrap_err());
    assert_eq!(n.what, "value resolver class GearboxFramework.UnknownValueResolver");
    assert_eq!(n.missing, Missing::NotRead);
    assert!(f.called().is_empty(), "no evaluator ran");
    assert_eq!(f.counters(), before);
}
