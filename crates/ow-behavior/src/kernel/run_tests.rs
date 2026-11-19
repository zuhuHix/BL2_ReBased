//! Tests of running behavior lists outside the kernel (decision 2026-11-19-54) on invented classes, structs and
//! natives (AGENTS.md rule 9). The toy `ApplyBehavior` is a native that logs what it got; the game's is script, which
//! these rules only call. AI-assisted (Claude Code).

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::registry::AttributeSystem;
use ow_core::{Missing, ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_package::testkit::reflect::{toy_array_of, toy_struct};
use ow_package::testkit::script::{CPF_OUT, FunctionSpec, Param, ScriptToy};
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ClassId, FuncId, ObjRef, PropAddr, StructId};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::{StructValue, Value};
use ow_vm::api::{FunctionDecl, NativeCall, NativePack, Vm};
use ow_vm::error::VmError;

use super::*;
use crate::kernel::natives::{OUTPUT_LINK, Pack, collect};

const NATIVE: u32 = 0x400 | 0x20000;

static NEXT: AtomicU32 = AtomicU32::new(0);

struct Toy {
    vm: Vm,
    dir: PathBuf,
    behavior: ClassId,
    info: StructId,
    params: StructId,
    context: StructId,
    sm_entry: StructId,
    log: Rc<RefCell<Vec<String>>>,
    local: Rc<Cell<bool>>,
}

fn param(vm: &mut Vm, name: &str, kind: PropKind, flags: u64) -> PropertyDecl {
    PropertyDecl::new(vm.space.names.intern(name), kind).with_flags(PropertyDecl::PARAM | flags)
}

fn declare(vm: &mut Vm, path: &str, owner: ClassId, params: Vec<PropertyDecl>, result: Option<PropKind>) -> FuncId {
    let key = format!("{path}()");
    let mut d = FunctionDecl::native(ObjectPath::parse(path).unwrap(), key)
        .with_owner(owner)
        .with_params(params);
    if let Some(k) = result {
        d = d.with_result(
            PropertyDecl::new(vm.space.names.intern("ReturnValue"), k)
                .with_flags(PropertyDecl::PARAM | PropertyDecl::OUT | PropertyDecl::RETURN),
        );
    }
    vm.declare_function(d).unwrap()
}

fn name_of(vm: &Vm, o: Option<ObjRef>) -> String {
    match o {
        Some(o) => vm.space.names.text(vm.space.heap.get(o).unwrap().name).into_owned(),
        None => "-".to_string(),
    }
}

fn read(vm: &Vm, v: &Value, field: &str) -> Value {
    struct_field(vm, v, field).cloned().unwrap()
}

fn with(vm: &Vm, v: &Value, field: &str, x: Value) -> Value {
    let s = v.as_struct().unwrap();
    let layout = vm.space.classes.struct_layout(s.ty).unwrap();
    let (entry, _) = layout.field(vm.space.names.find(field).unwrap()).unwrap();
    let mut out = StructValue::clone(s);
    out.fields[entry as usize] = x;
    Value::Struct(Rc::new(out))
}

/// The invented package: the classes a script dispatch reaches (`find_method` reads package code only) and the structs
/// their parameters name. `ToyBehavior.ApplyBehavior` and `ToyController.IsLocalController` are natives here, given
/// bodies by the test.
fn package() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&[
        "Class",
        "Function",
        "ScriptStruct",
        "IntProperty",
        "FloatProperty",
        "BoolProperty",
        "NameProperty",
        "ByteProperty",
        "ObjectProperty",
        "ArrayProperty",
        "StructProperty",
    ]);
    let object = t.class("Object", 0);
    let info = toy_struct(&mut t, object, "BehaviorKernelInfo", None);
    t.prop("Bool", info, "bIsInitialRunOfThisBehavior", 0, 0);
    t.prop("Float", info, "NextExecutionDelayTime", 0, 0);
    t.prop("Int", info, "ToyCounter", 0, 0);
    let params = toy_struct(&mut t, object, "BehaviorParameters", None);
    t.prop("Int", params, "ToyTag", 0, 0);
    let context = toy_struct(&mut t, object, "BehaviorContextData", None);
    t.prop("Byte", context, "BehaviorContext", 0, 0);
    t.prop("Name", context, "InstancedDataContextName", 0, 0);
    t.prop("Object", context, "ContextObject", 0, object);
    let entry = toy_struct(&mut t, object, "SMBehavior", None);
    toy_array_of(&mut t, entry, "Behaviors", "Object", object);
    for f in ["PlayServer", "PlayClient", "PlayLocal"] {
        t.prop("Bool", entry, f, 0, 0);
    }
    let base = t.class("BehaviorBase", object);
    let behavior = t.class("ToyBehavior", base);
    let with_ref = |kind: &str, name: &str, flags: u64, type_ref: i32| Param { flags, type_ref, ..Param::new(kind, name) };
    let apply = [
        with_ref("Object", "SelfObject", 0, object),
        with_ref("Struct", "KernelInfo", CPF_OUT, info),
        with_ref("Object", "MyInstigatorObject", 0, object),
        with_ref("Object", "OtherEventParticipantObject", 0, object),
        with_ref("Struct", "EventData", 0, params),
    ];
    let spec = FunctionSpec::new(NATIVE, "ApplyBehavior");
    t.function(behavior, "ApplyBehavior", &apply, None, &[], &spec, |_| None);
    let world = t.class("ToyWorldInfo", object);
    t.prop("Byte", world, "NetMode", 0, 0);
    let controller = t.class("ToyController", object);
    let spec = FunctionSpec::new(NATIVE, "IsLocalController");
    t.function(controller, "IsLocalController", &[], Some(("Bool", 0)), &[], &spec, |_| None);
    let pawn = t.class("ToyPawn", object);
    t.prop("Object", pawn, "WorldInfo", 0, world);
    t.prop("Object", pawn, "Controller", 0, controller);
    t.class("InstanceDataHelper", object);
    // Holds the structs no function names, so their layouts load with it.
    let holder = t.class("ToyHolder", object);
    t.prop("Struct", holder, "Context", 0, context);
    t.prop("Struct", holder, "Entry", 0, entry);
    t.build()
}

fn toy(mode: ParityMode) -> Toy {
    let dir = std::env::temp_dir().join(format!("ow-behavior-run-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let bytes = package();
    let defs = DefStore::from_synthetic(&dir, &[("Engine.upk", &bytes)], IdentityPolicy::FirstInScanOrder, mode).unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), mode);
    let object = vm.find_class("Engine.Object").unwrap();
    let base = vm.find_class("Engine.BehaviorBase").unwrap();
    let behavior = vm.find_class("Engine.ToyBehavior").unwrap();
    let controller = vm.find_class("Engine.ToyController").unwrap();
    let helper = vm.find_class("Engine.InstanceDataHelper").unwrap();
    let apply = vm.find_method(behavior, "ApplyBehavior", None).unwrap().unwrap();
    let is_local = vm.find_method(controller, "IsLocalController", None).unwrap().unwrap();
    let kind_of = |vm: &Vm, i: usize| vm.function(apply).unwrap().params[i].kind.clone();
    let (info, params) = match (kind_of(&vm, 1), kind_of(&vm, 4)) {
        (PropKind::Struct(a), PropKind::Struct(b)) => (a, b),
        other => panic!("{other:?}"),
    };
    let holder = vm.find_class("Engine.ToyHolder").unwrap();
    let struct_of = |vm: &Vm, name: &str| match vm.space.prop(holder, name).and_then(|p| vm.space.classes.decl(p)).map(|d| d.kind.clone()) {
        Some(PropKind::Struct(s)) => s,
        other => panic!("no struct {name}: {other:?}"),
    };
    let context = struct_of(&vm, "Context");
    let sm_entry = struct_of(&vm, "Entry");
    let sm = vm
        .space
        .classes
        .define(
            "GearboxFramework.SpecialMoveComponent",
            Some(object),
            vec![PropertyDecl::new(vm.space.names.intern("SMI"), PropKind::Object { class: object })],
        )
        .unwrap();
    let kernel = vm.space.classes.define("GearboxFramework.BehaviorKernel", Some(object), vec![]).unwrap();
    let o = PropKind::Object { class: object };
    let ps = vec![
        param(&mut vm, "Behaviors", PropKind::Array(Box::new(PropKind::Object { class: base })), PropertyDecl::OUT),
        param(&mut vm, "SelfObject", o.clone(), 0),
        param(&mut vm, "MyInstigatorObject", o.clone(), PropertyDecl::OPTIONAL),
        param(&mut vm, "OtherEventParticipantObject", o.clone(), PropertyDecl::OPTIONAL),
        param(&mut vm, "EventData", PropKind::Struct(params), PropertyDecl::OPTIONAL),
    ];
    declare(&mut vm, RUN_BEHAVIORS, base, ps, None);
    let resolver = |vm: &mut Vm, all: bool| {
        let mut ps = vec![
            param(vm, "DebugCaller", o.clone(), 0),
            param(vm, "ContextData", PropKind::Struct(context), 0),
            param(vm, "SelfObject", o.clone(), 0),
            param(vm, "MyInstigatorObject", o.clone(), 0),
            param(vm, "OtherEventParticipantObject", o.clone(), 0),
            param(vm, "EventData", PropKind::Struct(params), PropertyDecl::OPTIONAL),
        ];
        if all {
            ps.push(param(vm, "Contexts", PropKind::Array(Box::new(o.clone())), PropertyDecl::OUT));
        }
        ps
    };
    let ps = resolver(&mut vm, false);
    declare(&mut vm, STATIC_CONTEXT, base, ps, Some(o.clone()));
    let ps = resolver(&mut vm, true);
    declare(&mut vm, STATIC_ALL_CONTEXTS, base, ps, Some(PropKind::Bool));
    let ps = vec![
        param(&mut vm, "ContextObj", o.clone(), 0),
        param(&mut vm, "DataName", PropKind::Name, 0),
        param(&mut vm, "DefaultValue", o.clone(), 0),
    ];
    declare(&mut vm, FIRST_INSTANCE_DATA[0], helper, ps, Some(o.clone()));
    let ps = vec![param(&mut vm, "SMBehaviors", PropKind::Array(Box::new(PropKind::Struct(sm_entry))), 0)];
    declare(&mut vm, SM_RUN_BEHAVIORS, sm, ps, None);
    let ps = vec![param(&mut vm, "Data", PropKind::Struct(sm_entry), 0)];
    declare(&mut vm, SM_CAN_PLAY, sm, ps, Some(PropKind::Bool));
    let ps = vec![
        param(&mut vm, "KernelInfo", PropKind::Struct(info), PropertyDecl::OUT),
        param(&mut vm, "OutputLinkId", PropKind::Int, 0),
    ];
    declare(&mut vm, OUTPUT_LINK, kernel, ps, None);
    // The toy behavior's ApplyBehavior: logs what it got and counts in the record; Linker selects output link 7,
    // Waiter asks to wait.
    let log = Rc::new(RefCell::new(Vec::new()));
    let sink = log.clone();
    let key = vm.function(apply).unwrap().native_key.clone().unwrap();
    vm.register_native(
        &key,
        Rc::new(move |c: &mut NativeCall<'_>| -> Result<Value, VmError> {
            let me = name_of(c.vm, c.this);
            let record = c.arg(1)?.clone();
            let line = format!(
                "{me} self={} inst={} other={} init={:?} counter={:?} tag={:?}",
                name_of(c.vm, object_of(c.arg(0)?)),
                name_of(c.vm, object_of(c.arg(2)?)),
                name_of(c.vm, object_of(c.arg(3)?)),
                read(c.vm, &record, "bIsInitialRunOfThisBehavior"),
                read(c.vm, &record, "ToyCounter"),
                read(c.vm, c.arg(4)?, "ToyTag"),
            );
            sink.borrow_mut().push(line);
            let Value::Int(k) = read(c.vm, &record, "ToyCounter") else { unreachable!() };
            let mut next = with(c.vm, &record, "ToyCounter", Value::Int(k + 1));
            if me == "Linker" {
                let f = c.vm.find_function(OUTPUT_LINK)?;
                c.vm.call(f, None, vec![next.clone(), Value::Int(7)])?;
            }
            if me == "Waiter" {
                next = with(c.vm, &next, "NextExecutionDelayTime", Value::Float(1.0));
            }
            c.set_out(1, next)?;
            Ok(Value::None)
        }),
    )
    .unwrap();
    let local = Rc::new(Cell::new(false));
    let answer = local.clone();
    let key = vm.function(is_local).unwrap().native_key.clone().unwrap();
    vm.register_native(
        &key,
        Rc::new(move |_c: &mut NativeCall<'_>| -> Result<Value, VmError> { Ok(Value::Bool(answer.get())) }),
    )
    .unwrap();
    Pack.register(&mut vm).unwrap();
    Toy { vm, dir, behavior, info, params, context, sm_entry, log, local }
}

impl Drop for Toy {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

impl Toy {
    fn make(&mut self, class: &str, name: &str) -> ObjRef {
        let c = self.vm.find_class(class).unwrap();
        self.vm.space.alloc(c, name, None, Origin::Runtime).unwrap()
    }

    fn behaviors(&mut self, names: &[Option<&str>]) -> Value {
        let class = self.behavior;
        let items = names
            .iter()
            .map(|n| Value::Object(n.map(|n| self.vm.space.alloc(class, n, None, Origin::Runtime).unwrap())))
            .collect();
        Value::Array(Rc::new(items))
    }

    fn tag(&self, k: i32) -> Value {
        let zero = self.vm.space.classes.default_value(&PropKind::Struct(self.params)).unwrap();
        with(&self.vm, &zero, "ToyTag", Value::Int(k))
    }

    fn call(&mut self, path: &str, this: Option<ObjRef>, args: Vec<Value>) -> Result<ow_vm::api::CallResult, VmError> {
        let f = self.vm.find_function(path).unwrap();
        self.vm.call(f, this, args)
    }

    fn set(&mut self, o: ObjRef, field: &str, v: Value) {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, field).unwrap();
        self.vm.space.write(o, PropAddr::scalar(p), v).unwrap();
    }
}

/// Index order, None entries skipped, one record for the walk (only `bIsInitialRunOfThisBehavior` set; each behavior
/// sees what the previous one wrote), the arguments and a copy of the event data to each, nothing returned.
#[test]
fn run_behaviors_walks_the_list_with_one_shared_record() {
    let mut t = toy(ParityMode::Off);
    let me = t.make("Engine.Object", "Me");
    let inst = t.make("Engine.Object", "Inst");
    let list = t.behaviors(&[Some("A"), None, Some("B")]);
    let tag = t.tag(5);
    let r = t
        .call(RUN_BEHAVIORS, None, vec![list, Value::Object(Some(me)), Value::Object(Some(inst)), Value::Object(None), tag])
        .unwrap();
    assert_eq!(r.ret, Value::None);
    assert_eq!(
        *t.log.borrow(),
        vec![
            "A self=Me inst=Inst other=- init=Bool(true) counter=Int(0) tag=Int(5)".to_string(),
            "B self=Me inst=Inst other=- init=Bool(true) counter=Int(1) tag=Int(5)".to_string(),
        ]
    );
    // A second call starts a new record.
    let list = t.behaviors(&[Some("C")]);
    t.call(RUN_BEHAVIORS, None, vec![list, Value::Object(Some(me))]).unwrap();
    assert!(t.log.borrow()[2].starts_with("C self=Me inst=- other=- init=Bool(true) counter=Int(0)"), "{:?}", t.log.borrow());
    let _ = t.info;
}

/// An empty or None list does nothing.
#[test]
fn an_empty_or_none_list_does_nothing() {
    let mut t = toy(ParityMode::Off);
    for list in [Value::Array(Rc::new(Vec::new())), Value::None, t.behaviors(&[None, None])] {
        t.call(RUN_BEHAVIORS, None, vec![list, Value::Object(None)]).unwrap();
    }
    assert!(t.log.borrow().is_empty());
}

/// An output link a behavior selects leads nowhere: an outer collector frame gets nothing from the walk (the note's
/// section 5 warning), while a link selected directly in that frame is collected.
#[test]
fn output_links_selected_inside_the_walk_are_dropped() {
    let mut t = toy(ParityMode::Off);
    let list = t.behaviors(&[Some("Linker"), Some("B")]);
    let record = t.vm.space.classes.default_value(&PropKind::Struct(t.info)).unwrap();
    let f = t.vm.find_function(RUN_BEHAVIORS).unwrap();
    let link = t.vm.find_function(OUTPUT_LINK).unwrap();
    let (ran, ids) = collect(&mut t.vm, |vm| {
        vm.call(f, None, vec![list, Value::Object(None)])?;
        vm.call(link, None, vec![record, Value::Int(3)])
    })
    .unwrap();
    ran.unwrap();
    assert_eq!(ids, vec![3], "only the link selected outside the walk");
    assert_eq!(t.log.borrow().len(), 2, "the walk went on past the link");
}

/// A wait request outside a thread is open question 1: `NotYet { NotRead }`.
#[test]
fn a_wait_request_outside_a_thread_is_not_yet() {
    let mut t = toy(ParityMode::Off);
    let list = t.behaviors(&[Some("Waiter"), Some("B")]);
    match t.call(RUN_BEHAVIORS, None, vec![list, Value::Object(None)]).unwrap_err() {
        VmError::NotYet(n) => {
            assert_eq!(n.missing, Missing::NotRead);
            assert!(n.what.contains("open question 1"), "{n}");
        }
        e => panic!("expected NotYet, got {e}"),
    }
    assert_eq!(t.log.borrow().len(), 1, "B did not run");
}

/// Reference binds none of these (R3): the C++ has no such rule.
#[test]
fn reference_binds_nothing() {
    let off = toy(ParityMode::Off);
    let reference = toy(ParityMode::Reference);
    for path in [RUN_BEHAVIORS, STATIC_CONTEXT, STATIC_ALL_CONTEXTS, FIRST_INSTANCE_DATA[0], SM_RUN_BEHAVIORS, SM_CAN_PLAY] {
        let key = format!("{path}()");
        assert!(off.vm.has_native(&key), "{path} in Off");
        assert!(!reference.vm.has_native(&key), "{path} in Reference");
    }
}

/// The resolver: the selector picks self, instigator, other or the struct's object; 3 and others give nothing; a name
/// on an object without instance data gives nothing. The "all" form writes its array and refuses a non-empty one.
#[test]
fn the_static_context_resolvers_pick_by_selector() {
    let mut t = toy(ParityMode::Off);
    let [me, inst, other, own] = ["Me", "Inst", "Other", "Own"].map(|n| t.make("Engine.Object", n));
    let zero = t.vm.space.classes.default_value(&PropKind::Struct(t.context)).unwrap();
    let data = |t: &Toy, selector: u8, name: &str| {
        let d = with(&t.vm, &zero, "BehaviorContext", Value::Byte(selector));
        let d = with(&t.vm, &d, "ContextObject", Value::Object(Some(own)));
        let n = t.vm.space.names.find(name).unwrap_or_else(|| t.vm.space.names.find("None").unwrap());
        with(&t.vm, &d, "InstancedDataContextName", Value::Name(n))
    };
    t.vm.space.names.intern("None");
    t.vm.space.names.intern("ToyData");
    let objects = || vec![Value::Object(Some(me)), Value::Object(Some(inst)), Value::Object(Some(other))];
    for (selector, want) in [(0, Some(me)), (1, Some(inst)), (2, Some(other)), (3, None), (4, Some(own)), (9, None)] {
        let mut args = vec![Value::Object(None), data(&t, selector, "None")];
        args.extend(objects());
        assert_eq!(t.call(STATIC_CONTEXT, None, args.clone()).unwrap().ret, Value::Object(want), "selector {selector}");
        let empty = t.vm.space.classes.default_value(&PropKind::Struct(t.params)).unwrap();
        args.push(empty);
        args.push(Value::Array(Rc::new(Vec::new())));
        let r = t.call(STATIC_ALL_CONTEXTS, None, args).unwrap();
        assert_eq!(r.ret, Value::Bool(want.is_some()));
        assert_eq!(r.outs[6], Value::Array(Rc::new(want.into_iter().map(|o| Value::Object(Some(o))).collect())));
    }
    let mut args = vec![Value::Object(None), data(&t, 0, "ToyData")];
    args.extend(objects());
    assert_eq!(t.call(STATIC_CONTEXT, None, args).unwrap().ret, Value::Object(None), "no instance data");
    let mut args = vec![Value::Object(None), data(&t, 0, "None")];
    args.extend(objects());
    args.push(t.vm.space.classes.default_value(&PropKind::Struct(t.params)).unwrap());
    args.push(Value::Array(Rc::new(vec![Value::Object(Some(own))])));
    assert!(matches!(t.call(STATIC_ALL_CONTEXTS, None, args).unwrap_err(), VmError::NotYet(_)));
}

/// `GetFirstInstanceDataObject` answers the default without a name, a context, or instance data on the context.
#[test]
fn first_instance_data_answers_the_default() {
    let mut t = toy(ParityMode::Off);
    let ctx = t.make("Engine.Object", "Ctx");
    let fallback = t.make("Engine.Object", "Fallback");
    let none = t.vm.space.names.intern("None");
    let name = t.vm.space.names.intern("ToyData");
    for (c, n) in [(None, name), (Some(ctx), none), (Some(ctx), name)] {
        let r = t
            .call(FIRST_INSTANCE_DATA[0], None, vec![Value::Object(c), Value::Name(n), Value::Object(Some(fallback))])
            .unwrap();
        assert_eq!(r.ret, Value::Object(Some(fallback)));
    }
}

/// `CanPlayBehaviorData` and `SpecialMoveComponent.RunBehaviors`: no owner interface plays nothing; the side test
/// (server when not a client, client when one, local when the owner's controller is local); the entries that pass run
/// with the owner actor as self and the other arguments empty.
#[test]
fn the_special_move_runs_the_entries_its_side_allows() {
    let mut t = toy(ParityMode::Off);
    let component = t.make("GearboxFramework.SpecialMoveComponent", "SpecialMoveComponent0");
    let pawn = t.make("Engine.ToyPawn", "Maya");
    let world = t.make("Engine.ToyWorldInfo", "World");
    let controller = t.make("Engine.ToyController", "PC");
    t.set(pawn, "WorldInfo", Value::Object(Some(world)));
    let zero = t.vm.space.classes.default_value(&PropKind::Struct(t.sm_entry)).unwrap();
    let entry = |t: &mut Toy, names: &[&str], server: bool, client: bool, local: bool| {
        let list = t.behaviors(&names.iter().map(|n| Some(*n)).collect::<Vec<_>>());
        let e = with(&t.vm, &zero, "Behaviors", list);
        let e = with(&t.vm, &e, "PlayServer", Value::Bool(server));
        let e = with(&t.vm, &e, "PlayClient", Value::Bool(client));
        with(&t.vm, &e, "PlayLocal", Value::Bool(local))
    };
    let server = entry(&mut t, &["S"], true, false, false);
    let client = entry(&mut t, &["C"], false, true, false);
    let local = entry(&mut t, &["L"], false, false, true);
    let can = |t: &mut Toy, e: &Value| t.call(SM_CAN_PLAY, Some(component), vec![e.clone()]).unwrap().ret;
    assert_eq!(can(&mut t, &server), Value::Bool(false), "no owner interface");
    let list = Value::Array(Rc::new(vec![server.clone(), client.clone(), local.clone()]));
    t.call(SM_RUN_BEHAVIORS, Some(component), vec![list.clone()]).unwrap();
    assert!(t.log.borrow().is_empty(), "no owner interface: nothing runs");
    t.set(component, "SMI", Value::Object(Some(pawn)));
    assert_eq!(can(&mut t, &server), Value::Bool(true));
    assert_eq!(can(&mut t, &client), Value::Bool(false));
    assert_eq!(can(&mut t, &local), Value::Bool(false), "no controller");
    t.set(pawn, "Controller", Value::Object(Some(controller)));
    assert_eq!(can(&mut t, &local), Value::Bool(false), "not a local controller");
    t.local.set(true);
    assert_eq!(can(&mut t, &local), Value::Bool(true));
    t.set(world, "NetMode", Value::Byte(NM_CLIENT));
    assert_eq!(can(&mut t, &server), Value::Bool(false));
    assert_eq!(can(&mut t, &client), Value::Bool(true));
    t.set(world, "NetMode", Value::Byte(0));
    t.local.set(false);
    t.call(SM_RUN_BEHAVIORS, Some(component), vec![list]).unwrap();
    assert_eq!(
        *t.log.borrow(),
        vec!["S self=Maya inst=- other=- init=Bool(true) counter=Int(0) tag=Int(0)".to_string()]
    );
}
