//! Tests of the `ActionSkill` event natives (NATIVE_ACTION_SKILL.md section 9; lane FIX-PHASELOCK-CAST part B) on
//! invented data (AGENTS.md rule 9), in both parity modes. The provider package comes from `ow-behavior`'s invented
//! provider packages (the same file, as `ow-combat`'s tests include it); the kernel natives, the handle struct and the
//! two event natives are declared on the VM here under invented classes.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_behavior::kernel::natives::{self as kernel, INITIALIZE_PROVIDER, REGISTER_CONSUMER};
use ow_behavior::registry::{self, Handler};
use ow_core::{Missing, ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ClassId, ObjRef, StructId};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::prefix::PropertyPrefix;
use ow_object::value::Value;
use ow_vm::api::{FunctionDecl, NativePack, StubPolicy, Vm};
use ow_vm::error::VmError;

use super::EVENT_PATHS;

// The invented provider packages of the `ow-behavior` tests (the same file; it uses only `ow_package::testkit`).
#[allow(dead_code)]
#[path = "../../../ow-behavior/src/kernel/fixtures/packages.rs"]
mod packages;

use packages::{Ev, Seq, Spec, VLink, beh, ev, install, provider_package};

static NEXT: AtomicU32 = AtomicU32::new(0);

const PROVIDER: &str = "ToySkillProvider";
const SECOND: &str = "ToySecondProvider";
const ACTIVATED: &str = EVENT_PATHS[0].0;
const DEACTIVATED: &str = EVENT_PATHS[1].0;

/// One sequence, `Main`: `OnActionSkillActivated` publishes its payload elements 0 and 1 into `Who` and `Whom`,
/// `OnActionSkillDeactivated` element 0 into `Ended`; each runs one toy behavior.
fn spec(name: &'static str) -> Spec {
    Spec::new(
        name,
        vec![Seq {
            name: "Main",
            enabled_on_spawn: Some(true),
            events: vec![
                Ev {
                    vars: (0, 2),
                    ..ev("OnActionSkillActivated", (0, 1))
                },
                Ev {
                    vars: (2, 1),
                    ..ev("OnActionSkillDeactivated", (1, 1))
                },
            ],
            behaviors: vec![beh("B", "Behavior_ToySame", (0, 0))],
            links: vec![(0, -1, 0.0), (0, -1, 0.0)],
            variables: vec![
                ("Who", "BVAR_Object"),
                ("Whom", "BVAR_Object"),
                ("Ended", "BVAR_Object"),
            ],
            vlinks: vec![
                VLink("Pawn", "BVARLINK_Output", 0, (0, 1)),
                VLink("Target", "BVARLINK_Output", 1, (1, 1)),
                VLink("Pawn", "BVARLINK_Output", 0, (2, 1)),
            ],
            linked: vec![0, 1, 2],
            ..Seq::default()
        }],
        vec![],
    )
}

struct Toy {
    vm: Vm,
    dir: PathBuf,
    handle: StructId,
    bare: ClassId,
    /// What the toy behavior saw: `<event> consumer=<name>`.
    seen: Rc<RefCell<Vec<String>>>,
}

impl Drop for Toy {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn declare(vm: &mut Vm, path: &str, params: Vec<(&str, PropKind)>, result: Option<PropKind>) {
    let mut d = FunctionDecl::native(ObjectPath::parse(path).unwrap(), path).with_static(true);
    d.params = params
        .into_iter()
        .map(|(n, k)| {
            PropertyDecl::new(vm.space.names.intern(n), k).with_flags(PropertyDecl::PARAM)
        })
        .collect();
    d.result = result.map(|k| {
        PropertyDecl::new(vm.space.names.intern("ReturnValue"), k).with_flags(PropertyDecl::RETURN)
    });
    vm.declare_function(d).expect("declared");
}

/// A VM in `mode` over the two invented providers, with the kernel natives it needs and the two event natives
/// declared, and the action skill pack registered.
fn toy(mode: ParityMode) -> Toy {
    toy_with(mode, &[], &[])
}

/// [`toy`] with `before` registered before the action skill pack and `after` after it (review follow-up of #407).
fn toy_with(mode: ParityMode, before: &[&dyn NativePack], after: &[&dyn NativePack]) -> Toy {
    let dir = std::env::temp_dir().join(format!(
        "ow-skills-events-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let files = install(&[
        ("TestSkillBpd", provider_package(&spec(PROVIDER))),
        ("TestSecondBpd", provider_package(&spec(SECOND))),
    ]);
    let borrowed: Vec<(&str, &[u8])> = files
        .iter()
        .map(|(n, b)| (n.as_str(), b.as_slice()))
        .collect();
    let defs = DefStore::from_synthetic(&dir, &borrowed, IdentityPolicy::FirstInScanOrder, mode)
        .expect("synthetic store");
    let mut vm = Vm::new(
        ObjectSpace::new(),
        defs,
        Rc::new(AttributeSystem::new_empty()),
        mode,
    );
    vm.stub_policy = StubPolicy::refuse_all();
    let pid = PropertyDecl::new(vm.space.names.intern("PID"), PropKind::Int);
    let handle = vm
        .space
        .classes
        .define_struct("ToySkill.BehaviorConsumerHandle", None, vec![pid])
        .unwrap();
    let bare = vm
        .space
        .classes
        .define("ToySkill.ToyBare", None, vec![])
        .unwrap();
    let h = || PropKind::Struct(handle);
    let o = || PropKind::Object { class: bare };
    declare(
        &mut vm,
        REGISTER_CONSUMER,
        vec![("Consumer", o())],
        Some(h()),
    );
    declare(
        &mut vm,
        INITIALIZE_PROVIDER,
        vec![("Handle", h()), ("Provider", o())],
        None,
    );
    declare(
        &mut vm,
        ACTIVATED,
        vec![("Handle", h()), ("Pawn", o()), ("Target", o())],
        None,
    );
    declare(
        &mut vm,
        DEACTIVATED,
        vec![("Handle", h()), ("Pawn", o())],
        None,
    );
    let seen = Rc::new(RefCell::new(Vec::new()));
    let log = seen.clone();
    registry::register(
        &mut vm,
        "WillowGame.Behavior_ToySame",
        Handler::run(move |vm, p, _id, event| {
            let consumer = match p.consumer() {
                Some(c) => vm
                    .space
                    .heap
                    .get(c)
                    .map(|o| vm.space.names.text(o.name).into_owned())
                    .unwrap_or_default(),
                None => "none".to_string(),
            };
            log.borrow_mut()
                .push(format!("{event} consumer={consumer}"));
            Ok(Vec::new())
        }),
    )
    .unwrap();
    for p in before {
        p.register(&mut vm).unwrap();
    }
    super::Pack.register(&mut vm).unwrap();
    for p in after {
        p.register(&mut vm).unwrap();
    }
    Toy {
        vm,
        dir,
        handle,
        bare,
        seen,
    }
}

impl Toy {
    fn call(&mut self, path: &str, args: Vec<Value>) -> Result<Value, VmError> {
        let f = self.vm.find_function(path)?;
        Ok(self.vm.call(f, None, args)?.ret)
    }

    fn handle(&self, pid: i32) -> Value {
        Value::structure(self.handle, vec![Value::Int(pid)])
    }

    fn object(&mut self, name: &str) -> ObjRef {
        self.vm
            .space
            .alloc(self.bare, name, None, Origin::Runtime)
            .unwrap()
    }

    fn definition(&mut self, file: &str, path: &str) -> Value {
        let name = ow_core::PackageName::parse(file).unwrap();
        let (_, store) = self.vm.defs.code_mut();
        let id = store.load(&name).unwrap();
        let index = store.get(id).find_export(path).unwrap().expect("export");
        let o = self
            .vm
            .defs
            .load_export(&mut self.vm.space, id, index, PropertyPrefix::Ordinary)
            .unwrap();
        Value::Object(Some(o))
    }

    /// A kernel consumer `name` with the providers of `files` (file, provider path) initialized, in order.
    fn consumer(&mut self, name: &str, files: &[(&str, &str)]) -> (Value, ObjRef) {
        let skill = self.object(name);
        let h = self
            .call(REGISTER_CONSUMER, vec![Value::Object(Some(skill))])
            .unwrap();
        for (file, path) in files {
            let p = self.definition(file, path);
            self.call(INITIALIZE_PROVIDER, vec![h.clone(), p]).unwrap();
        }
        (h, skill)
    }

    fn seen(&self) -> Vec<String> {
        std::mem::take(&mut *self.seen.borrow_mut())
    }

    /// The live objects of `Main`'s variables `Who`, `Whom`, `Ended` on the consumer's provider.
    fn live(&mut self, h: &Value) -> Vec<Option<ObjRef>> {
        let pid = kernel::consumer_pid(&self.vm, h).unwrap();
        kernel::with_provider(&mut self.vm, pid, PROVIDER, |_, p| {
            p.variables("Main").iter().map(|v| v.live).collect()
        })
        .unwrap()
        .expect("the provider")
    }
}

fn not_yet(r: Result<Value, VmError>) -> (Missing, String) {
    match r {
        Err(VmError::NotYet(n)) => (n.missing, n.to_string()),
        other => panic!("expected NotYet, got {other:?}"),
    }
}

#[test]
fn the_events_reach_the_consumer_s_provider_with_their_parameters_in_order_in_off() {
    let mut t = toy(ParityMode::Off);
    let (h, _) = t.consumer("Skill_0", &[("TestSkillBpd", PROVIDER)]);
    let (pawn, target) = (t.object("Maya"), t.object("Target"));
    let r = t.call(
        ACTIVATED,
        vec![
            h.clone(),
            Value::Object(Some(pawn)),
            Value::Object(Some(target)),
        ],
    );
    assert_eq!(r.unwrap(), Value::None, "returns nothing");
    assert_eq!(t.seen(), ["OnActionSkillActivated consumer=Skill_0"]);
    assert_eq!(
        t.live(&h),
        [Some(pawn), Some(target), None],
        "pawn, then target"
    );
    t.call(DEACTIVATED, vec![h.clone(), Value::Object(Some(pawn))])
        .unwrap();
    assert_eq!(t.seen(), ["OnActionSkillDeactivated consumer=Skill_0"]);
    assert_eq!(t.live(&h)[2], Some(pawn), "the pawn alone");
    // A None target is passed on as None.
    t.call(
        ACTIVATED,
        vec![h.clone(), Value::Object(Some(pawn)), Value::Object(None)],
    )
    .unwrap();
    assert_eq!(t.live(&h)[..2], [Some(pawn), None]);
    assert_eq!(t.seen().len(), 1);
}

#[test]
fn a_handle_nobody_owns_fires_nothing_in_off() {
    let mut t = toy(ParityMode::Off);
    let (_h, _) = t.consumer("Skill_0", &[("TestSkillBpd", PROVIDER)]);
    let pawn = t.object("Maya");
    for pid in [kernel::INVALID_PID, 7] {
        let h = t.handle(pid);
        let r = t.call(
            ACTIVATED,
            vec![h.clone(), Value::Object(Some(pawn)), Value::Object(None)],
        );
        assert_eq!(r.unwrap(), Value::None, "PID {pid}");
        let r = t.call(DEACTIVATED, vec![h, Value::Object(Some(pawn))]);
        assert_eq!(r.unwrap(), Value::None, "PID {pid}");
    }
    assert!(t.seen().is_empty());
}

#[test]
fn what_section_nine_leaves_open_is_not_yet_in_off() {
    let mut t = toy(ParityMode::Off);
    let pawn = Value::Object(Some(t.object("Maya")));
    // A kernel consumer with no provider, and one with two.
    let (none, _) = t.consumer("Skill_0", &[]);
    let (two, _) = t.consumer(
        "Skill_1",
        &[("TestSkillBpd", PROVIDER), ("TestSecondBpd", SECOND)],
    );
    for (h, n) in [(none, 0), (two, 2)] {
        let (missing, text) = not_yet(t.call(
            ACTIVATED,
            vec![h.clone(), pawn.clone(), Value::Object(None)],
        ));
        assert_eq!(missing, Missing::NotRead);
        assert!(text.contains(&format!("with {n} providers")), "{text}");
        let (missing, _) = not_yet(t.call(DEACTIVATED, vec![h, pawn.clone()]));
        assert_eq!(missing, Missing::NotRead);
    }
    // A handle another crate gave out.
    kernel::claim(&mut t.vm, 40, "ow-toy").unwrap();
    let h = t.handle(40);
    let (missing, text) = not_yet(t.call(DEACTIVATED, vec![h, pawn]));
    assert_eq!(missing, Missing::NotRead);
    assert!(text.contains("ow-toy"), "{text}");
    assert!(t.seen().is_empty(), "nothing fired");
}

#[test]
fn reference_leaves_both_unbound() {
    let mut t = toy(ParityMode::Reference);
    let pawn = Value::Object(Some(t.object("Maya")));
    for (path, args) in [
        (
            ACTIVATED,
            vec![t.handle(0), pawn.clone(), Value::Object(None)],
        ),
        (DEACTIVATED, vec![t.handle(0), pawn]),
    ] {
        match t.call(path, args) {
            Err(VmError::UnboundNative(key)) => {
                assert!(key.contains(path.rsplit('.').next().unwrap()), "{key}")
            }
            other => panic!("{path}: expected UnboundNative, got {other:?}"),
        }
    }
}

/// Review follow-up of #407: ow-combat's pack registered before or after the action skill pack, and the kernel pack
/// both before and after it (the action skill pack registers the kernel pack itself, so that is three times), change
/// nothing: the events still reach the provider with their payload. ow-combat's pack is registered once per world:
/// a second registration is refused (its pools service is added unguarded), which no caller does.
#[test]
fn registering_the_kernel_or_combat_pack_before_and_after_keeps_delivery_in_off() {
    let combat = ow_combat::hit::Pack;
    let kernel_pack = kernel::Pack;
    let (c, k): (&dyn NativePack, &dyn NativePack) = (&combat, &kernel_pack);
    for (name, before, after) in [
        ("ow-combat before", vec![c], vec![]),
        ("ow-combat after", vec![], vec![c]),
        ("kernel before and after", vec![k], vec![k]),
        (
            "kernel and ow-combat before, kernel after",
            vec![k, c],
            vec![k],
        ),
        (
            "kernel before, ow-combat and kernel after",
            vec![k],
            vec![c, k],
        ),
    ] {
        let mut t = toy_with(ParityMode::Off, &before, &after);
        let (h, _) = t.consumer("Skill_0", &[("TestSkillBpd", PROVIDER)]);
        let (pawn, target) = (t.object("Maya"), t.object("Target"));
        t.call(
            ACTIVATED,
            vec![
                h.clone(),
                Value::Object(Some(pawn)),
                Value::Object(Some(target)),
            ],
        )
        .unwrap();
        assert_eq!(
            t.seen(),
            ["OnActionSkillActivated consumer=Skill_0"],
            "{name}"
        );
        assert_eq!(t.live(&h), [Some(pawn), Some(target), None], "{name}");
        t.call(DEACTIVATED, vec![h.clone(), Value::Object(Some(pawn))])
            .unwrap();
        assert_eq!(
            t.seen(),
            ["OnActionSkillDeactivated consumer=Skill_0"],
            "{name}"
        );
    }
}
