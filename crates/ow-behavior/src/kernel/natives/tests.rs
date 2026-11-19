//! Tests of the kernel natives pack on invented data (AGENTS.md rule 9), in both parity modes. AI-assisted (Claude
//! Code).
//!
//! The provider comes from the kernel fixtures' invented packages; the natives, the handle struct, the consumer
//! class and the action enum are declared on the VM here (invented paths and values; the toy enum orders its values
//! unlike the game, so a rule that read the action by position would fail).

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::registry::AttributeSystem;
use ow_core::{Missing, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ClassId, ObjRef, PropAddr, StructId};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::prefix::PropertyPrefix;
use ow_object::value::Value;
use ow_vm::api::{FunctionDecl, NativePack, StubPolicy, Vm};
use ow_vm::error::VmError;

use super::*;
use crate::kernel::fixtures::{Seq, Spec, beh, ev, install, provider_package};
use crate::registry::{self, Handler};

static NEXT: AtomicU32 = AtomicU32::new(0);

const PROVIDER: &str = "ToyKernelProvider";
const CONDITIONAL: &str = "ToyConditionalProvider";
const FAILING: &str = "ToyFailingProvider";
const DELAYED: &str = "ToyDelayedProvider";

/// The toy action enum, in an order unlike the game's.
const ACTIONS: [&str; 3] = ["CHANGE_Disable", "CHANGE_Toggle", "CHANGE_Enable"];

struct Toy {
    vm: Vm,
    dir: PathBuf,
    handle: StructId,
    pawn: ClassId,
    bare: ClassId,
    /// What the toy behavior handler saw: `event consumer=<name>`.
    seen: Rc<RefCell<Vec<String>>>,
    /// While set, the failing toy behavior (`Behavior_ToyDifferent`) fails.
    fail: Rc<std::cell::Cell<bool>>,
}

impl Drop for Toy {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// `Main` (enabled on spawn, its event `Go` runs one toy behavior) and `Side` (disabled, mutex-free).
fn spec() -> Spec {
    Spec::new(
        PROVIDER,
        vec![
            Seq {
                name: "Main",
                enabled_on_spawn: Some(true),
                events: vec![ev("Go", (0, 1))],
                behaviors: vec![beh("B", "Behavior_ToySame", (0, 0))],
                links: vec![(0, -1, 0.0)],
                ..Seq::default()
            },
            Seq {
                name: "Side",
                enabled_on_spawn: Some(false),
                events: vec![ev("Go", (0, 1))],
                behaviors: vec![beh("S", "Behavior_ToySame", (0, 0))],
                links: vec![(0, -1, 0.0)],
                ..Seq::default()
            },
        ],
        vec![],
    )
}

/// `Late` (enabled on spawn): its event `Go` runs one toy behavior after a 1 s link delay, so the thread parks.
fn delayed() -> Spec {
    Spec::new(
        DELAYED,
        vec![Seq {
            name: "Late",
            enabled_on_spawn: Some(true),
            events: vec![ev("Go", (0, 1))],
            behaviors: vec![beh("D", "Behavior_ToySame", (0, 0))],
            links: vec![(0, -1, 1.0)],
            ..Seq::default()
        }],
        vec![],
    )
}

/// `Boom` (enabled on spawn): its `OnBehaviorSequenceEnabled` runs the failing toy behavior, so pass 1 fails.
fn failing() -> Spec {
    Spec::new(
        FAILING,
        vec![Seq {
            name: "Boom",
            enabled_on_spawn: Some(true),
            events: vec![ev("OnBehaviorSequenceEnabled", (0, 1))],
            behaviors: vec![beh("X", "Behavior_ToyDifferent", (0, 0))],
            links: vec![(0, -1, 0.0)],
            ..Seq::default()
        }],
        vec![],
    )
}

fn conditional() -> Spec {
    Spec::new(
        CONDITIONAL,
        vec![Seq {
            name: "Gated",
            condition: true,
            ..Seq::default()
        }],
        vec![],
    )
}

fn param(vm: &mut Vm, name: &str, kind: PropKind, optional: bool) -> PropertyDecl {
    let flags = PropertyDecl::PARAM | if optional { PropertyDecl::OPTIONAL } else { 0 };
    PropertyDecl::new(vm.space.names.intern(name), kind).with_flags(flags)
}

fn declare(vm: &mut Vm, path: &str, params: Vec<PropertyDecl>, result: Option<PropertyDecl>) {
    let mut decl = FunctionDecl::native(ObjectPath::parse(path).unwrap(), path).with_static(true);
    decl.params = params;
    decl.result = result;
    vm.declare_function(decl).expect("declared");
}

use ow_core::ObjectPath;

/// A VM in `mode` over the fixtures' packages, with the kernel natives declared (not bound).
fn toy(mode: ParityMode) -> Toy {
    toy_with(mode, true)
}

/// [`toy`], declaring the kernel natives only when `natives` is set.
fn toy_with(mode: ParityMode, natives: bool) -> Toy {
    let dir = std::env::temp_dir().join(format!(
        "ow-behavior-natives-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let files = install(&[
        ("TestBpd", provider_package(&spec())),
        ("TestCond", provider_package(&conditional())),
        ("TestFail", provider_package(&failing())),
        ("TestDelay", provider_package(&delayed())),
    ]);
    let borrowed: Vec<(&str, &[u8])> = files.iter().map(|(n, b)| (n.as_str(), b.as_slice())).collect();
    let defs = DefStore::from_synthetic(&dir, &borrowed, IdentityPolicy::FirstInScanOrder, mode)
        .expect("synthetic store");
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), mode);
    vm.stub_policy = StubPolicy::LogAndZero;
    let pid = PropertyDecl::new(vm.space.names.intern("PID"), PropKind::Int);
    let handle = vm.space.classes.define_struct("ToyKernel.BehaviorConsumerHandle", None, vec![pid]).unwrap();
    let field = PropertyDecl::new(vm.space.names.intern("ConsumerHandle"), PropKind::Struct(handle));
    let pawn = vm.space.classes.define("ToyKernel.ToyPawn", None, vec![field]).unwrap();
    let bare = vm.space.classes.define("ToyKernel.ToyBare", None, vec![]).unwrap();
    let values = ACTIONS.iter().map(|a| vm.space.names.intern(a)).collect();
    let action = vm.space.classes.define_enum("ToyKernel.EChangeStatus", values).unwrap();
    let h = || PropKind::Struct(handle);
    let o = || PropKind::Object { class: bare };
    let decls = [
        (OUTPUT_LINK, vec![("KernelInfo", PropKind::Int, false), ("OutputLinkId", PropKind::Int, false)], None),
        (CONSUMER_HANDLE, vec![], Some(h())),
        (IS_SEQUENCE_ENABLED, vec![("Handle", h(), false), ("Provider", o(), false), ("SequenceName", PropKind::Name, false)], Some(PropKind::Bool)),
        (
            EVENT_FROM_SCRIPT,
            vec![
                ("Handle", h(), false),
                ("Provider", o(), false),
                ("EventName", PropKind::Name, false),
                ("EventOutputToActivate", PropKind::Int, true),
                ("Parameters", PropKind::Int, true),
            ],
            None,
        ),
        (BROADCAST_FROM_SCRIPT, vec![("Handle", h(), false), ("EventName", PropKind::Name, false)], None),
        (REGISTER_CONSUMER, vec![("Consumer", o(), false)], Some(h())),
        (BEGIN_NONDETERMINISTIC, vec![("Handle", h(), false)], None),
        (FORCE_UNREGISTER, vec![("Handle", h(), false)], None),
        (INITIALIZE_PROVIDER, vec![("Handle", h(), false), ("Provider", o(), false)], None),
        (
            CHANGE_SEQUENCE,
            vec![
                ("Handle", h(), false),
                ("Provider", o(), false),
                ("SequenceName", PropKind::Name, false),
                ("Action", PropKind::Byte { enum_: Some(action) }, false),
            ],
            None,
        ),
    ];
    for (path, params, result) in decls.into_iter().filter(|_| natives) {
        // The two provider natives take the handle by reference (NATIVE_BEHAVIOR_PROVIDERS.md: out parameters).
        let by_ref = path == BEGIN_NONDETERMINISTIC || path == FORCE_UNREGISTER;
        let params = params
            .into_iter()
            .map(|(n, k, opt)| {
                let d = param(&mut vm, n, k, opt);
                if by_ref { d.with_flags(PropertyDecl::PARAM | PropertyDecl::OUT) } else { d }
            })
            .collect();
        let result = result.map(|k| {
            PropertyDecl::new(vm.space.names.intern("ReturnValue"), k).with_flags(PropertyDecl::RETURN)
        });
        declare(&mut vm, path, params, result);
    }
    let seen = Rc::new(RefCell::new(Vec::new()));
    let log = seen.clone();
    registry::register(
        &mut vm,
        "WillowGame.Behavior_ToySame",
        Handler::run(move |vm, p, id, event| {
            let consumer = match p.consumer() {
                Some(c) => vm.space.heap.get(c).map(|o| vm.space.names.text(o.name).into_owned()).unwrap_or_default(),
                None => "none".to_string(),
            };
            log.borrow_mut().push(format!("{event} {} consumer={consumer}", p.behavior(id).name));
            Ok(Vec::new())
        }),
    )
    .unwrap();
    let fail = Rc::new(std::cell::Cell::new(false));
    let failing = fail.clone();
    registry::register(
        &mut vm,
        "WillowGame.Behavior_ToyDifferent",
        Handler::run(move |_, _, _, _| {
            if failing.get() {
                Err(crate::provider::BehaviorError::Data("toy failure".to_string()))
            } else {
                Ok(Vec::new())
            }
        }),
    )
    .unwrap();
    Toy { vm, dir, handle, pawn, bare, seen, fail }
}

impl Toy {
    fn call(&mut self, path: &str, this: Option<ObjRef>, args: Vec<Value>) -> Result<Value, VmError> {
        let f = self.vm.find_function(path)?;
        Ok(self.vm.call(f, this, args)?.ret)
    }

    fn handle(&self, pid: i32) -> Value {
        Value::structure(self.handle, vec![Value::Int(pid)])
    }

    fn name(&mut self, s: &str) -> Value {
        Value::Name(self.vm.space.names.intern(s))
    }

    fn object(&mut self, class: ClassId, name: &str) -> ObjRef {
        self.vm.space.alloc(class, name, None, Origin::Runtime).unwrap()
    }

    fn definition(&mut self, file: &str, path: &str) -> ObjRef {
        let name = ow_core::PackageName::parse(file).unwrap();
        let (_, store) = self.vm.defs.code_mut();
        let id = store.load(&name).unwrap();
        let index = store.get(id).find_export(path).unwrap().expect("export");
        self.vm.defs.load_export(&mut self.vm.space, id, index, PropertyPrefix::Ordinary).unwrap()
    }

    fn pid_of(&self, v: &Value) -> i32 {
        match handle_pid(&self.vm, v) {
            Some(p) => p,
            None => panic!("not a handle: {v:?}"),
        }
    }

    fn seen(&self) -> Vec<String> {
        std::mem::take(&mut *self.seen.borrow_mut())
    }
}

fn not_yet(r: Result<Value, VmError>) -> Missing {
    match r {
        Err(VmError::NotYet(n)) => n.missing,
        other => panic!("expected NotYet, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The output-link collector
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn output_links_go_to_the_innermost_running_behavior_in_both_modes() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let mut t = toy(mode);
        Pack.register(&mut t.vm).unwrap();
        let link = |t: &mut Toy, id: i32| t.call(OUTPUT_LINK, None, vec![Value::Int(0), Value::Int(id)]).unwrap();
        // No behavior running: kept nowhere.
        link(&mut t, 9);
        let mut tt = t;
        let ((inner, ()), outer) = collect(&mut tt.vm, |vm| {
            let f = vm.find_function(OUTPUT_LINK).unwrap();
            vm.call(f, None, vec![Value::Int(0), Value::Int(1)]).unwrap();
            let ((), inner) = collect(vm, |vm| {
                for id in [2, 2] {
                    vm.call(f, None, vec![Value::Int(0), Value::Int(id)]).unwrap();
                }
            })
            .unwrap();
            vm.call(f, None, vec![Value::Int(0), Value::Int(3)]).unwrap();
            (inner, ())
        })
        .unwrap();
        assert_eq!(inner, vec![2, 2], "{mode:?}: duplicates kept, the inner run's own");
        assert_eq!(outer, vec![1, 3], "{mode:?}: call order");
    }
}

#[test]
fn collecting_without_the_pack_is_not_yet() {
    let mut t = toy(ParityMode::Off);
    match collect(&mut t.vm, |_| ()) {
        Err(VmError::NotYet(n)) => assert!(matches!(n.missing, Missing::LayerAbsent(_))),
        other => panic!("{other:?}"),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// One owner
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn registering_twice_is_a_no_op_and_another_binder_is_refused() {
    let mut t = toy(ParityMode::Off);
    Pack.register(&mut t.vm).unwrap();
    Pack.register(&mut t.vm).unwrap();
    assert!(t.vm.has_native(OUTPUT_LINK));
    // A pack that bound a kernel key first: the kernel pack refuses to share it.
    let mut t = toy(ParityMode::Off);
    t.vm.register_native(OUTPUT_LINK, Rc::new(|_: &mut NativeCall<'_>| Ok(Value::None))).unwrap();
    assert!(matches!(Pack.register(&mut t.vm), Err(VmError::Registration { .. })));
}

/// The kernel's one-owner check uses the VM's package-qualified identity, case folded (review r1 of #519, item 3): a
/// second function of the same package under another spelling (`PACKA`) with the same key is first-wins, not a
/// refusal, and runs the first binding.
#[test]
fn a_package_spelled_in_another_case_is_one_owner() {
    let mut t = toy(ParityMode::Off);
    Pack.register(&mut t.vm).unwrap();
    for path in ["PackA.ToyCase.Ping", "PACKA.ToyCase.Busy.Ping"] {
        let p = ow_core::ObjectPath::parse(path).unwrap();
        t.vm.declare_function(FunctionDecl::native(p, "ToyCase.Ping()")).unwrap();
    }
    bind(&mut t.vm, "PackA.ToyCase.Ping", Rc::new(|_: &mut NativeCall<'_>| Ok(Value::Int(1)))).unwrap();
    bind(&mut t.vm, "PACKA.ToyCase.Busy.Ping", Rc::new(|_: &mut NativeCall<'_>| Ok(Value::Int(2)))).unwrap();
    let second = t.vm.find_function("PACKA.ToyCase.Busy.Ping").unwrap();
    assert_eq!(t.vm.call(second, None, vec![]).unwrap().ret, Value::Int(1));
}

#[test]
fn a_native_declared_after_the_first_registration_is_bound_by_the_next() {
    let mut t = toy_with(ParityMode::Off, false);
    // Nothing declared: nothing bound, and no error.
    Pack.register(&mut t.vm).unwrap();
    assert!(!t.vm.has_native(OUTPUT_LINK));
    let params = vec![
        param(&mut t.vm, "KernelInfo", PropKind::Int, false),
        param(&mut t.vm, "OutputLinkId", PropKind::Int, false),
    ];
    declare(&mut t.vm, OUTPUT_LINK, params, None);
    Pack.register(&mut t.vm).unwrap();
    assert!(t.vm.has_native(OUTPUT_LINK));
    let ((), ids) = collect(&mut t.vm, |vm| {
        let f = vm.find_function(OUTPUT_LINK).unwrap();
        vm.call(f, None, vec![Value::Int(0), Value::Int(5)]).unwrap();
    })
    .unwrap();
    assert_eq!(ids, vec![5]);
}

#[test]
fn reference_binds_only_what_the_cxx_binds() {
    let mut t = toy(ParityMode::Reference);
    Pack.register(&mut t.vm).unwrap();
    for path in [OUTPUT_LINK, CONSUMER_HANDLE, IS_SEQUENCE_ENABLED, EVENT_FROM_SCRIPT, BROADCAST_FROM_SCRIPT] {
        assert!(t.vm.has_native(path), "{path}");
    }
    for path in [REGISTER_CONSUMER, INITIALIZE_PROVIDER, CHANGE_SEQUENCE, BEGIN_NONDETERMINISTIC, FORCE_UNREGISTER] {
        assert!(!t.vm.has_native(path), "{path}");
    }
    // Unbound there, the stub policy answers, as before the pack.
    let pawn = t.object(t.bare, "Skill_0");
    let got = t.call(REGISTER_CONSUMER, None, vec![Value::Object(Some(pawn))]).unwrap();
    assert_eq!(t.pid_of(&got), 0, "the stub's zero struct");
    assert_eq!(t.vm.log.last().map(String::as_str), Some(format!("UNIMPLEMENTED {REGISTER_CONSUMER}").as_str()));
}

// ---------------------------------------------------------------------------------------------------------------
// The handle
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn the_consumer_handle_is_a_plain_read_of_the_object_s_field_in_off() {
    for mode in [ParityMode::Off] {
        let mut t = toy(mode);
        Pack.register(&mut t.vm).unwrap();
        let pawn = t.object(t.pawn, "Pawn_0");
        let prop = t.vm.space.prop(t.pawn, "ConsumerHandle").unwrap();
        let h = t.handle(7);
        t.vm.space.write(pawn, PropAddr::scalar(prop), h.clone()).unwrap();
        assert_eq!(t.call(CONSUMER_HANDLE, Some(pawn), vec![]).unwrap(), h, "{mode:?}");
        let bare = t.object(t.bare, "Bare_0");
        let err = t.call(CONSUMER_HANDLE, Some(bare), vec![]).unwrap_err().to_string();
        assert!(err.contains("has no property ConsumerHandle"), "{err}");
        assert!(t.call(CONSUMER_HANDLE, None, vec![]).is_err());
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Routing by owner
// ---------------------------------------------------------------------------------------------------------------

/// Two owners, set up in the given order; returns what each kind of handle answers.
fn routed_answers(owners_first: bool) -> Vec<String> {
    let mut t = toy(ParityMode::Off);
    let setup_a = |vm: &mut Vm| {
        claim(vm, 3, "a").unwrap();
        route(vm, "a", IS_SEQUENCE_ENABLED, Rc::new(|_: &mut NativeCall<'_>| Ok(Value::Bool(true)))).unwrap();
    };
    let setup_b = |vm: &mut Vm| {
        claim(vm, 4, "b").unwrap();
        route(vm, "b", BROADCAST_FROM_SCRIPT, Rc::new(|c: &mut NativeCall<'_>| {
            c.vm.log.push("b broadcast".to_string());
            Ok(Value::None)
        }))
        .unwrap();
    };
    if owners_first {
        setup_a(&mut t.vm);
        setup_b(&mut t.vm);
        Pack.register(&mut t.vm).unwrap();
    } else {
        Pack.register(&mut t.vm).unwrap();
        setup_b(&mut t.vm);
        setup_a(&mut t.vm);
    }
    let provider = t.object(t.bare, "AnyProvider");
    let mut out = Vec::new();
    for pid in [3, 4, 5, INVALID_PID] {
        let (h, seq) = (t.handle(pid), t.name("Main"));
        let enabled = t.call(IS_SEQUENCE_ENABLED, None, vec![h.clone(), Value::Object(Some(provider)), seq]);
        let event = t.name("Go");
        let broadcast = t.call(BROADCAST_FROM_SCRIPT, None, vec![h, event]);
        out.push(format!("{pid}: enabled={enabled:?} broadcast={broadcast:?} log={:?}", t.vm.log.last()));
    }
    out
}

#[test]
fn routing_by_owner_does_not_depend_on_the_registration_order() {
    let first = routed_answers(true);
    assert_eq!(first, routed_answers(false));
    // a's handle: a's body; b's handle: b's body, and the VM's unbound behaviour for a native b gave no body; a handle
    // no one owns (5, and the invalid -1): false and nothing.
    assert!(first[0].starts_with("3: enabled=Ok(Bool(true)) broadcast=Ok(None)"), "{}", first[0]);
    assert!(first[0].contains(&format!("UNIMPLEMENTED {BROADCAST_FROM_SCRIPT}")), "{}", first[0]);
    assert!(first[1].contains("b broadcast"), "{}", first[1]);
    assert!(first[1].contains("enabled=Ok(Bool(false))"), "the stub's zero bool: {}", first[1]);
    for line in &first[2..] {
        assert!(line.contains("enabled=Ok(Bool(false)) broadcast=Ok(None)"), "{line}");
    }
}

#[test]
fn an_owner_without_a_body_is_unbound_under_a_refusing_stub_policy() {
    let mut t = toy(ParityMode::Off);
    claim(&mut t.vm, 4, "b").unwrap();
    t.vm.stub_policy = StubPolicy::refuse_all();
    let (h, e) = (t.handle(4), t.name("Go"));
    assert!(matches!(t.call(BROADCAST_FROM_SCRIPT, None, vec![h, e]), Err(VmError::UnboundNative(_))));
}

#[test]
fn claims_are_exclusive_and_allocation_skips_held_handles() {
    let mut t = toy(ParityMode::Off);
    claim(&mut t.vm, 3, "a").unwrap();
    claim(&mut t.vm, 3, "a").unwrap();
    assert!(matches!(claim(&mut t.vm, 3, "b"), Err(VmError::Registration { .. })));
    assert!(matches!(claim(&mut t.vm, INVALID_PID, "a"), Err(VmError::Registration { .. })));
    assert_eq!(allocate(&mut t.vm, "b", 3).unwrap(), 4);
    assert_eq!(allocate(&mut t.vm, "b", 0).unwrap(), 0);
    assert_eq!(owner_of(&t.vm, 4).unwrap(), Some(Owner::Crate("b")));
    assert!(route(&mut t.vm, "a", OUTPUT_LINK, Rc::new(|_: &mut NativeCall<'_>| Ok(Value::None))).is_err());
    let body: NativeFn = Rc::new(|_: &mut NativeCall<'_>| Ok(Value::None));
    route(&mut t.vm, "a", BROADCAST_FROM_SCRIPT, body.clone()).unwrap();
    assert!(route(&mut t.vm, "a", BROADCAST_FROM_SCRIPT, body).is_err());
}

#[test]
fn an_event_on_a_none_provider_fires_nothing_whoever_owns_the_handle_in_off() {
    let mut t = toy(ParityMode::Off);
    claim(&mut t.vm, 3, "a").unwrap();
    route(&mut t.vm, "a", EVENT_FROM_SCRIPT, Rc::new(|_: &mut NativeCall<'_>| panic!("not reached"))).unwrap();
    let (h, e) = (t.handle(3), t.name("Go"));
    assert_eq!(t.call(EVENT_FROM_SCRIPT, None, vec![h, Value::Object(None), e]).unwrap(), Value::None);
}

// ---------------------------------------------------------------------------------------------------------------
// The kernel's own processes (Off)
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn register_consumer_gives_minus_one_for_none_and_the_lowest_free_handle_otherwise() {
    let mut t = toy(ParityMode::Off);
    Pack.register(&mut t.vm).unwrap();
    let none = t.call(REGISTER_CONSUMER, None, vec![Value::Object(None)]).unwrap();
    assert_eq!(t.pid_of(&none), INVALID_PID);
    claim(&mut t.vm, 0, "a").unwrap();
    let skill = t.object(t.bare, "Skill_0");
    let got = t.call(REGISTER_CONSUMER, None, vec![Value::Object(Some(skill))]).unwrap();
    assert_eq!(t.pid_of(&got), 1, "0 is held");
    assert_eq!(owner_of(&t.vm, 1).unwrap(), Some(Owner::Kernel));
    assert_eq!(process_of(&t.vm, skill).unwrap(), Some(1));
    assert_eq!(not_yet(t.call(REGISTER_CONSUMER, None, vec![Value::Object(Some(skill))])), Missing::NotRead);
}

#[test]
fn a_kernel_provider_runs_its_events_with_its_consumer() {
    let mut t = toy(ParityMode::Off);
    Pack.register(&mut t.vm).unwrap();
    let skill = t.object(t.bare, "Skill_0");
    let h = t.call(REGISTER_CONSUMER, None, vec![Value::Object(Some(skill))]).unwrap();
    let definition = t.definition("TestBpd", PROVIDER);
    let p = Value::Object(Some(definition));
    t.call(INITIALIZE_PROVIDER, None, vec![h.clone(), p.clone()]).unwrap();
    let enabled = |t: &mut Toy, s: &str| {
        let n = t.name(s);
        t.call(IS_SEQUENCE_ENABLED, None, vec![h.clone(), p.clone(), n]).unwrap()
    };
    assert_eq!(enabled(&mut t, "Main"), Value::Bool(true), "pass 1: bEnabledOnSpawn");
    assert_eq!(enabled(&mut t, "Side"), Value::Bool(false));
    let go = t.name("Go");
    t.call(EVENT_FROM_SCRIPT, None, vec![h.clone(), p.clone(), go.clone()]).unwrap();
    assert_eq!(t.seen(), ["Go B consumer=Skill_0"]);
    // The action by its enum name (the toy enum: Disable 0, Toggle 1, Enable 2).
    let side = t.name("Side");
    t.call(CHANGE_SEQUENCE, None, vec![h.clone(), p.clone(), side.clone(), Value::Byte(2)]).unwrap();
    assert_eq!(enabled(&mut t, "Side"), Value::Bool(true));
    t.call(BROADCAST_FROM_SCRIPT, None, vec![h.clone(), go.clone()]).unwrap();
    assert_eq!(t.seen(), ["Go B consumer=Skill_0", "Go S consumer=Skill_0"]);
    t.call(CHANGE_SEQUENCE, None, vec![h.clone(), p.clone(), side.clone(), Value::Byte(1)]).unwrap();
    assert_eq!(enabled(&mut t, "Side"), Value::Bool(false), "toggled off");
    t.call(CHANGE_SEQUENCE, None, vec![h.clone(), p.clone(), side.clone(), Value::Byte(0)]).unwrap();
    assert_eq!(enabled(&mut t, "Side"), Value::Bool(false), "disable of a disabled sequence");
    // A provider the consumer has not registered: false; the event fires nothing.
    let other = Value::Object(Some(t.definition("TestCond", CONDITIONAL)));
    let main = t.name("Main");
    assert_eq!(t.call(IS_SEQUENCE_ENABLED, None, vec![h.clone(), other.clone(), main]).unwrap(), Value::Bool(false));
    t.call(EVENT_FROM_SCRIPT, None, vec![h.clone(), other, go]).unwrap();
    assert!(t.seen().is_empty());
}

#[test]
fn what_section_c_does_not_read_is_not_yet() {
    let mut t = toy(ParityMode::Off);
    Pack.register(&mut t.vm).unwrap();
    let skill = t.object(t.bare, "Skill_0");
    let h = t.call(REGISTER_CONSUMER, None, vec![Value::Object(Some(skill))]).unwrap();
    assert_eq!(not_yet(t.call(INITIALIZE_PROVIDER, None, vec![h.clone(), Value::Object(None)])), Missing::NotRead);
    let gated = Value::Object(Some(t.definition("TestCond", CONDITIONAL)));
    assert!(matches!(not_yet(t.call(INITIALIZE_PROVIDER, None, vec![h.clone(), gated])), Missing::LayerAbsent(_)));
    let p = Value::Object(Some(t.definition("TestBpd", PROVIDER)));
    t.call(INITIALIZE_PROVIDER, None, vec![h.clone(), p.clone()]).unwrap();
    assert_eq!(not_yet(t.call(INITIALIZE_PROVIDER, None, vec![h.clone(), p.clone()])), Missing::NotRead);
    let side = t.name("Side");
    let none_provider = vec![h.clone(), Value::Object(None), side.clone(), Value::Byte(2)];
    assert_eq!(not_yet(t.call(CHANGE_SEQUENCE, None, none_provider)), Missing::NotRead);
    let go = t.name("Go");
    // Off reads Parameters (lane IMPL-ACTIVATE-EVENT-PARAMETERS): a value that is not an array is the port's own
    // Script refusal.
    let with_parameters = vec![h.clone(), p.clone(), go, Value::Int(-1), Value::Int(0)];
    match t.call(EVENT_FROM_SCRIPT, None, with_parameters) {
        Err(VmError::Script { message, .. }) => assert!(message.contains("Parameters holds Int, not an array"), "{message}"),
        other => panic!("{other:?}"),
    }
    // A handle no process holds: nothing happens.
    let stranger = t.handle(42);
    t.call(INITIALIZE_PROVIDER, None, vec![stranger, p.clone()]).unwrap();
    // Re-entrant use of a lent provider.
    let pid = t.pid_of(&h);
    let path = stock_path(&t.vm, match p { Value::Object(Some(o)) => o, _ => unreachable!() }).unwrap();
    let (h2, main) = (h.clone(), t.name("Main"));
    let inner = with_provider(&mut t.vm, pid, &path, |vm, _| {
        let f = vm.find_function(IS_SEQUENCE_ENABLED).unwrap();
        vm.call(f, None, vec![h2, p.clone(), main]).map(|r| r.ret)
    })
    .unwrap()
    .expect("registered");
    assert_eq!(not_yet(inner), Missing::NotRead);
}

// ---------------------------------------------------------------------------------------------------------------
// Reference: what the C++ does (review r1 of PR #352)
// ---------------------------------------------------------------------------------------------------------------

fn key_of(t: &mut Toy, path: &str) -> String {
    let f = t.vm.find_function(path).unwrap();
    t.vm.function(f).and_then(|d| d.native_key.clone()).unwrap()
}

/// An unowned handle in Reference: the stub when no crate binds the native in the C++, the one crate's body when one
/// does, `NotYet` when two would. Off keeps the record's false / nothing.
#[test]
fn reference_unowned_handles_take_the_cxx_crate_s_body_or_the_stub() {
    let mut t = toy(ParityMode::Reference);
    Pack.register(&mut t.vm).unwrap();
    let provider = t.object(t.bare, "AnyProvider");
    let enabled = |t: &mut Toy| {
        let (h, seq) = (t.handle(9), t.name("Main"));
        t.call(IS_SEQUENCE_ENABLED, None, vec![h, Value::Object(Some(provider)), seq])
    };
    assert_eq!(enabled(&mut t).unwrap(), Value::Bool(false), "the stub's zero bool");
    let stub = format!("UNIMPLEMENTED {}", key_of(&mut t, IS_SEQUENCE_ENABLED));
    assert_eq!(t.vm.log.last(), Some(&stub));
    // A None provider is not settled before the body in Reference: no body, the stub.
    let (h, go) = (t.handle(9), t.name("Go"));
    t.call(EVENT_FROM_SCRIPT, None, vec![h, Value::Object(None), go]).unwrap();
    let stub = format!("UNIMPLEMENTED {}", key_of(&mut t, EVENT_FROM_SCRIPT));
    assert_eq!(t.vm.log.last(), Some(&stub));
    // One crate binds it in the C++: its body, for every unowned handle.
    fallback(&mut t.vm, "m", IS_SEQUENCE_ENABLED, Rc::new(|_: &mut NativeCall<'_>| Ok(Value::Bool(true)))).unwrap();
    let before = t.vm.log.len();
    assert_eq!(enabled(&mut t).unwrap(), Value::Bool(true));
    assert_eq!(t.vm.log.len(), before, "no stub line");
    assert!(fallback(&mut t.vm, "m", IS_SEQUENCE_ENABLED, Rc::new(|_: &mut NativeCall<'_>| Ok(Value::None))).is_err());
    // Two crates would: no C++ VM holds both.
    fallback(&mut t.vm, "c", IS_SEQUENCE_ENABLED, Rc::new(|_: &mut NativeCall<'_>| Ok(Value::Bool(false)))).unwrap();
    assert_eq!(not_yet(enabled(&mut t)), Missing::NotPorted);
    // Off never runs a fallback.
    let mut t = toy(ParityMode::Off);
    fallback(&mut t.vm, "m", IS_SEQUENCE_ENABLED, Rc::new(|_: &mut NativeCall<'_>| Ok(Value::Bool(true)))).unwrap();
    assert_eq!(enabled(&mut t).unwrap(), Value::Bool(false));
}

/// `GetBehaviorConsumerHandle` in Reference goes through the owner of the object's own handle, else the fallback,
/// else the stub; it is not the plain read (the C++ mission bridge stubs every pawn but the director).
#[test]
fn reference_reads_the_consumer_handle_through_its_owner() {
    let mut t = toy(ParityMode::Reference);
    Pack.register(&mut t.vm).unwrap();
    let prop = t.vm.space.prop(t.pawn, "ConsumerHandle").unwrap();
    let pawn = |t: &mut Toy, name: &str, pid: i32| {
        let o = t.object(t.pawn, name);
        let h = t.handle(pid);
        t.vm.space.write(o, PropAddr::scalar(prop), h).unwrap();
        o
    };
    let (owned, stranger) = (pawn(&mut t, "Owned", 7), pawn(&mut t, "Stranger", 8));
    t.call(CONSUMER_HANDLE, Some(stranger), vec![]).unwrap();
    let stub = format!("UNIMPLEMENTED {}", key_of(&mut t, CONSUMER_HANDLE));
    assert_eq!(t.vm.log.last(), Some(&stub), "no body: the stub, not the plain read");
    claim(&mut t.vm, 7, "a").unwrap();
    route(&mut t.vm, "a", CONSUMER_HANDLE, Rc::new(|_: &mut NativeCall<'_>| Ok(Value::Int(1)))).unwrap();
    fallback(&mut t.vm, "b", CONSUMER_HANDLE, Rc::new(|_: &mut NativeCall<'_>| Ok(Value::Int(2)))).unwrap();
    assert_eq!(t.call(CONSUMER_HANDLE, Some(owned), vec![]).unwrap(), Value::Int(1));
    assert_eq!(t.call(CONSUMER_HANDLE, Some(stranger), vec![]).unwrap(), Value::Int(2));
    let bare = t.object(t.bare, "Bare_0");
    assert_eq!(t.call(CONSUMER_HANDLE, Some(bare), vec![]).unwrap(), Value::Int(2), "no handle field: unowned");
}

/// A pass 1 that fails registers nothing: the provider's slot goes, the sequence reads as not registered, and the
/// call can be made again (review r1 of PR #352).
#[test]
fn a_failed_pass_one_registers_nothing_and_can_be_retried() {
    let mut t = toy(ParityMode::Off);
    Pack.register(&mut t.vm).unwrap();
    let skill = t.object(t.bare, "Skill_0");
    let h = t.call(REGISTER_CONSUMER, None, vec![Value::Object(Some(skill))]).unwrap();
    let p = Value::Object(Some(t.definition("TestFail", FAILING)));
    t.fail.set(true);
    let err = t.call(INITIALIZE_PROVIDER, None, vec![h.clone(), p.clone()]).unwrap_err();
    assert!(err.to_string().contains("toy failure"), "{err}");
    let boom = t.name("Boom");
    assert_eq!(t.call(IS_SEQUENCE_ENABLED, None, vec![h.clone(), p.clone(), boom.clone()]).unwrap(), Value::Bool(false));
    t.fail.set(false);
    t.call(INITIALIZE_PROVIDER, None, vec![h.clone(), p.clone()]).unwrap();
    assert_eq!(t.call(IS_SEQUENCE_ENABLED, None, vec![h, p, boom]).unwrap(), Value::Bool(true));
}

// ---------------------------------------------------------------------------------------------------------------
// The provider natives (NATIVE_BEHAVIOR_PROVIDERS.md section 4; decision 2026-11-19-19)
// ---------------------------------------------------------------------------------------------------------------

impl Toy {
    /// Calls a by-reference handle native and returns (result, the handle as the caller sees it afterwards).
    fn call_by_ref(&mut self, path: &str, handle: Value) -> Result<(Value, Value), VmError> {
        let f = self.vm.find_function(path)?;
        let done = self.vm.call(f, None, vec![handle])?;
        Ok((done.ret, done.outs[0].clone()))
    }
}

/// Begin: nothing changes and the handle is not written, on a registered PID, on -1 and on an unknown PID; the
/// consumer still answers afterwards.
#[test]
fn begin_nondeterministic_changes_nothing_a_script_sees() {
    let mut t = toy(ParityMode::Off);
    Pack.register(&mut t.vm).unwrap();
    let skill = t.object(t.bare, "Skill_0");
    let h = t.call(REGISTER_CONSUMER, None, vec![Value::Object(Some(skill))]).unwrap();
    let pid = t.pid_of(&h);
    for given in [h.clone(), t.handle(INVALID_PID), t.handle(77)] {
        let (ret, after) = t.call_by_ref(BEGIN_NONDETERMINISTIC, given.clone()).unwrap();
        assert_eq!(ret, Value::None);
        assert_eq!(after, given, "the handle is not written");
    }
    let definition = t.definition("TestBpd", PROVIDER);
    let p = Value::Object(Some(definition));
    t.call(INITIALIZE_PROVIDER, None, vec![h.clone(), p.clone()]).unwrap();
    let main = t.name("Main");
    assert_eq!(t.call(IS_SEQUENCE_ENABLED, None, vec![h, p, main]).unwrap(), Value::Bool(true));
    assert_eq!(process_of(&t.vm, skill).unwrap(), Some(pid));
}

/// Force on a kernel consumer: the handle comes back -1; the process and its provider are gone, so nothing of it runs
/// again (its events fire nothing, its sequences read disabled); the PID is kept out of the allocator; a second Force
/// only writes -1. Force on -1 and on an unknown PID: the handle reads -1 and nothing else changes.
#[test]
fn force_unregister_tears_the_consumer_down_and_always_writes_minus_one() {
    let mut t = toy(ParityMode::Off);
    Pack.register(&mut t.vm).unwrap();
    let skill = t.object(t.bare, "Skill_0");
    let h = t.call(REGISTER_CONSUMER, None, vec![Value::Object(Some(skill))]).unwrap();
    let pid = t.pid_of(&h);
    let definition = t.definition("TestBpd", PROVIDER);
    let p = Value::Object(Some(definition));
    t.call(INITIALIZE_PROVIDER, None, vec![h.clone(), p.clone()]).unwrap();
    let (ret, after) = t.call_by_ref(FORCE_UNREGISTER, h.clone()).unwrap();
    assert_eq!(ret, Value::None);
    assert_eq!(t.pid_of(&after), INVALID_PID);
    assert_eq!(process_of(&t.vm, skill).unwrap(), None);
    assert!(with_provider(&mut t.vm, pid, PROVIDER, |_, _| ()).unwrap().is_none(), "the provider is gone");
    // Calls with the old handle find no consumer.
    let (main, go) = (t.name("Main"), t.name("Go"));
    assert_eq!(t.call(IS_SEQUENCE_ENABLED, None, vec![h.clone(), p.clone(), main]).unwrap(), Value::Bool(false));
    t.call(EVENT_FROM_SCRIPT, None, vec![h.clone(), p, go]).unwrap();
    assert!(t.seen().is_empty(), "nothing of the released consumer runs");
    assert_eq!(owner_of(&t.vm, pid).unwrap(), Some(Owner::Released));
    // The released PID is not handed out again (NATIVE_BEHAVIOR_PROVIDERS.md, open question 2).
    let other = t.object(t.bare, "Skill_1");
    let next = t.call(REGISTER_CONSUMER, None, vec![Value::Object(Some(other))]).unwrap();
    assert_ne!(t.pid_of(&next), pid);
    // A second Force with the old handle, and Force on -1 and on an unknown PID: only the -1 write.
    let owners_before = format!("{:?}", owner_of(&t.vm, t.pid_of(&next)).unwrap());
    for given in [h, t.handle(INVALID_PID), t.handle(77)] {
        let (_, after) = t.call_by_ref(FORCE_UNREGISTER, given).unwrap();
        assert_eq!(t.pid_of(&after), INVALID_PID);
    }
    assert_eq!(format!("{:?}", owner_of(&t.vm, t.pid_of(&next)).unwrap()), owners_before, "the other consumer is kept");
    assert_eq!(owner_of(&t.vm, 77).unwrap(), None);
}

/// Force on a handle another crate owns: `NotYet` (that crate's teardown is not ported here), before any write.
#[test]
fn force_unregister_on_a_crate_handle_is_not_yet() {
    let mut t = toy(ParityMode::Off);
    Pack.register(&mut t.vm).unwrap();
    claim(&mut t.vm, 5, "ow-mission").unwrap();
    let h = t.handle(5);
    let f = t.vm.find_function(FORCE_UNREGISTER).unwrap();
    let r = t.vm.call(f, None, vec![h]).map(|d| d.ret);
    assert_eq!(not_yet(r), Missing::NotPorted);
    assert_eq!(owner_of(&t.vm, 5).unwrap(), Some(Owner::Crate("ow-mission")));
}

/// Review follow-up of #405: a consumer whose provider holds a parked (delayed) thread is forced out; ticking past the
/// delay then runs nothing of it. A second consumer of the same provider, not forced, is the control: its parked
/// thread runs on the same tick. The forced process has left the table, so no tick can reach its provider again.
#[test]
fn force_unregister_drops_a_parked_thread_so_a_later_tick_runs_nothing() {
    let mut t = toy(ParityMode::Off);
    Pack.register(&mut t.vm).unwrap();
    let definition = Value::Object(Some(t.definition("TestDelay", DELAYED)));
    let consumer = |t: &mut Toy, name: &str| {
        let o = t.object(t.bare, name);
        let h = t.call(REGISTER_CONSUMER, None, vec![Value::Object(Some(o))]).unwrap();
        t.call(INITIALIZE_PROVIDER, None, vec![h.clone(), definition.clone()]).unwrap();
        h
    };
    let (forced, kept) = (consumer(&mut t, "Skill_F"), consumer(&mut t, "Skill_K"));
    let (forced_pid, kept_pid) = (t.pid_of(&forced), t.pid_of(&kept));
    let go = t.name("Go");
    for h in [&forced, &kept] {
        t.call(EVENT_FROM_SCRIPT, None, vec![h.clone(), definition.clone(), go.clone()]).unwrap();
    }
    assert!(t.seen().is_empty(), "both threads are parked behind the 1 s delay");
    let (_, after) = t.call_by_ref(FORCE_UNREGISTER, forced.clone()).unwrap();
    assert_eq!(t.pid_of(&after), INVALID_PID);
    // Past the delay: the kept consumer's thread runs; the forced one's provider is not there to tick.
    let ticked = with_provider(&mut t.vm, kept_pid, DELAYED, |vm, p| p.tick(vm, 2.0)).unwrap();
    ticked.expect("the kept provider").unwrap();
    assert!(with_provider(&mut t.vm, forced_pid, DELAYED, |vm, p| p.tick(vm, 2.0)).unwrap().is_none());
    assert_eq!(t.seen(), ["Go D consumer=Skill_K"], "only the kept consumer's parked thread ran");
    // Later ticks run nothing more: the forced consumer's thread never runs.
    let ticked = with_provider(&mut t.vm, kept_pid, DELAYED, |vm, p| p.tick(vm, 5.0)).unwrap();
    ticked.expect("the kept provider").unwrap();
    assert!(t.seen().is_empty());
}

// ------------------------------------------------------------------------- script Parameters (IMPL-ACTIVATE-EVENT-PARAMETERS)

/// Defines the invented `BehaviorVariableValue` struct (the note's field names) and returns it.
fn variable_value_struct(t: &mut Toy) -> StructId {
    let bare = t.bare;
    let d = |t: &mut Toy, n: &str, k: PropKind| PropertyDecl::new(t.vm.space.names.intern(n), k);
    let decls = vec![
        d(t, "IntValue", PropKind::Int),
        d(t, "FloatValue", PropKind::Float),
        d(t, "ObjectValue", PropKind::Object { class: bare }),
        d(t, "VariableType", PropKind::Byte { enum_: None }),
    ];
    t.vm.space
        .classes
        .define_struct("GearboxFramework.BehaviorProviderDefinition.BehaviorVariableValue", None, decls)
        .unwrap()
}

fn element(ty: StructId, type_index: u8, object: Option<ObjRef>) -> Value {
    Value::structure(ty, vec![Value::Int(0), Value::Float(0.0), Value::Object(object), Value::Byte(type_index)])
}

/// The array is read by name from the loaded enum (the toy enum orders `BVAR_Object` second, on purpose); a None
/// element is `BVAR_None`; a struct of another type and a type byte past the enum are the port's own Script refusals.
#[test]
fn parameters_are_read_by_enum_name() {
    let mut t = toy(ParityMode::Off);
    let ty = variable_value_struct(&mut t);
    let maya = t.object(t.bare, "Maya");
    let object = crate::kernel::fixtures::VARIABLE_TYPES.iter().position(|n| *n == "BVAR_Object").unwrap() as u8;
    let int = crate::kernel::fixtures::VARIABLE_TYPES.iter().position(|n| *n == "BVAR_Int").unwrap() as u8;
    let arg = Value::array(vec![element(ty, object, Some(maya)), Value::None, element(ty, int, None)]);
    let out = parameters(&mut t.vm, arg).unwrap();
    assert_eq!(out[0], PayloadElement::object(Some(maya)));
    assert_eq!(out[1].var_type, "BVAR_None");
    assert_eq!(out[2].var_type, "BVAR_Int");
    match parameters(&mut t.vm, Value::array(vec![element(ty, 200, None)])) {
        Err(VmError::Script { message, .. }) => assert!(message.contains("outside EBehaviorVariableType"), "{message}"),
        other => panic!("{other:?}"),
    }
    let handle = t.handle;
    match parameters(&mut t.vm, Value::array(vec![Value::structure(handle, vec![Value::Int(1)])])) {
        Err(VmError::Script { message, .. }) => assert!(message.contains("is not a BehaviorVariableValue"), "{message}"),
        other => panic!("{other:?}"),
    }
    assert_eq!(parameters(&mut t.vm, Value::array(vec![])).unwrap(), vec![]);
}

/// Off: the event fires with its Parameters (the toy provider's `Go` runs its behavior); Reference is unchanged.
#[test]
fn the_script_route_fires_with_parameters_in_off_only() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let mut t = toy(mode);
        Pack.register(&mut t.vm).unwrap();
        let ty = variable_value_struct(&mut t);
        let skill = t.object(t.bare, "Skill_0");
        let h = t.call(REGISTER_CONSUMER, None, vec![Value::Object(Some(skill))]).unwrap();
        let definition = t.definition("TestBpd", PROVIDER);
        let p = Value::Object(Some(definition));
        t.call(INITIALIZE_PROVIDER, None, vec![h.clone(), p.clone()]).unwrap();
        let object = crate::kernel::fixtures::VARIABLE_TYPES.iter().position(|n| *n == "BVAR_Object").unwrap() as u8;
        let parameters = Value::array(vec![element(ty, object, Some(skill))]);
        let go = t.name("Go");
        let r = t.call(EVENT_FROM_SCRIPT, None, vec![h.clone(), p.clone(), go, Value::Int(-1), parameters]);
        match mode {
            ParityMode::Off => {
                r.unwrap();
                assert_eq!(t.seen(), ["Go B consumer=Skill_0"]);
            }
            // Reference does not route the call to the kernel rule (as on main): nothing fires.
            ParityMode::Reference => {
                r.unwrap();
                assert!(t.seen().is_empty());
            }
        }
    }
}
