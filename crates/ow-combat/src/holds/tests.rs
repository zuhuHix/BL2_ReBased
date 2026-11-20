//! Tests of the hold kinds on invented classes and values (AGENTS.md rule 9), in both parity modes. The classes are
//! defined straight into the class table at the paths the natives read (`GearboxFramework.AIComponent`,
//! `Engine.Controller`), with only the properties they read. AI-assisted (Claude Code).

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::registry::AttributeSystem;
use ow_core::{Missing, ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ClassId, FuncId, ObjRef, StructId};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_package::testkit::script::ScriptToy;
use ow_vm::api::{FunctionDecl, NativeCall, Vm};
use ow_vm::error::VmError;

use super::*;
use crate::hit::access::{array, field, get, set};

static NEXT: AtomicU32 = AtomicU32::new(0);

/// An invented world: a component class with the four lists and an `Owner`, a controller class, a pawn class with
/// the two motion vectors, a plain class, and the thirteen natives declared at their installed paths.
struct Env {
    vm: Vm,
    dir: PathBuf,
    component: ClassId,
    controller: ClassId,
    pawn: ClassId,
    thing: ClassId,
    vector: StructId,
    natives: Vec<(String, FuncId)>,
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

impl Env {
    fn new(mode: ParityMode) -> Env {
        let dir = std::env::temp_dir().join(format!(
            "ow-combat-holds-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let core = {
            let mut t = ScriptToy::new(&[]);
            t.class("Object", 0);
            t.build()
        };
        let defs = DefStore::from_synthetic(
            &dir,
            &[("Core.upk", core.as_slice())],
            IdentityPolicy::FirstInScanOrder,
            mode,
        )
        .expect("store");
        let mut vm = Vm::new(
            ObjectSpace::new(),
            defs,
            Rc::new(AttributeSystem::new_empty()),
            mode,
        );
        let core_object = vm.find_class("Core.Object").unwrap();
        let root = vm
            .space
            .classes
            .define("Toy.ToyObject", Some(core_object), vec![])
            .unwrap();
        let o = PropKind::Object { class: root };
        let axes = ["X", "Y", "Z"].map(|n| decl(&mut vm, n, PropKind::Float)).to_vec();
        let vector = vm
            .space
            .classes
            .define_struct("Toy.ToyObject.Vector", None, axes)
            .unwrap();
        let pair = vec![
            decl(&mut vm, "HoldName", PropKind::Name),
            decl(&mut vm, "Tag", o.clone()),
        ];
        let hold = vm
            .space
            .classes
            .define_struct("Toy.ToyObject.AIHoldData", None, pair)
            .unwrap();
        let list = PropKind::Array(Box::new(PropKind::Struct(hold)));
        let mut props = vec![("Owner", o.clone())];
        for (_, name) in KINDS {
            props.push((name, list.clone()));
        }
        let component = class(&mut vm, COMPONENT, root, &props);
        let controller = class(&mut vm, "Engine.Controller", root, &[("Pawn", o.clone())]);
        let pawn = class(
            &mut vm,
            "Toy.ToyPawn",
            root,
            &[
                ("Velocity", PropKind::Struct(vector)),
                ("Acceleration", PropKind::Struct(vector)),
            ],
        );
        let thing = class(&mut vm, "Toy.ToyThing", root, &[]);
        let mut natives = Vec::new();
        for (kind, _) in KINDS {
            for (name, ret) in [
                (format!("Hold{kind}"), None),
                (format!("Release{kind}"), None),
                (format!("{kind}OnHold"), Some(PropKind::Bool)),
            ] {
                let path = format!("{COMPONENT}.{name}");
                let params = if ret.is_none() {
                    vec![
                        decl(&mut vm, "HoldName", PropKind::Name),
                        decl(&mut vm, "Tag", o.clone()),
                    ]
                } else {
                    vec![]
                };
                let f = declare(&mut vm, component, &path, params, ret);
                natives.push((name, f));
            }
        }
        let f = declare(&mut vm, root, GET_AI_COMPONENT, vec![], Some(o));
        natives.push(("GetAIComponent".to_string(), f));
        Env {
            vm,
            dir,
            component,
            controller,
            pawn,
            thing,
            vector,
            natives,
        }
    }

    fn native(&self, name: &str) -> FuncId {
        self.natives.iter().find(|(n, _)| n == name).expect(name).1
    }

    fn make(&mut self, class: ClassId, name: &str) -> ObjRef {
        self.vm
            .space
            .alloc(class, name, None, Origin::Runtime)
            .unwrap()
    }

    fn call(&mut self, name: &str, this: ObjRef, args: Vec<Value>) -> Result<Value, VmError> {
        let f = self.native(name);
        Ok(self.vm.call(f, Some(this), args)?.ret)
    }

    fn hold(&mut self, verb: &str, this: ObjRef, reason: &str, tag: Option<ObjRef>) -> Result<Value, VmError> {
        let n = Value::Name(self.vm.space.names.intern(reason));
        self.call(verb, this, vec![n, Value::Object(tag)])
    }

    /// The (name, tag) pairs of `component`'s `list`.
    fn pairs(&mut self, component: ObjRef, list: &str) -> Vec<(String, Option<ObjRef>)> {
        let items = array(&mut self.vm, component, list).unwrap();
        items
            .iter()
            .map(|v| {
                let name = match field(&self.vm, v, "HoldName") {
                    Some(Value::Name(n)) => self.vm.space.names.text(n).into_owned(),
                    other => panic!("{other:?}"),
                };
                let tag = match field(&self.vm, v, "Tag") {
                    Some(Value::Object(o)) => o,
                    other => panic!("{other:?}"),
                };
                (name, tag)
            })
            .collect()
    }

    fn vector(&self, v: [f32; 3]) -> Value {
        Value::structure(self.vector, v.iter().map(|x| Value::Float(*x)).collect())
    }
}

fn decl(vm: &mut Vm, name: &str, kind: PropKind) -> PropertyDecl {
    PropertyDecl::new(vm.space.names.intern(name), kind)
}

fn class(vm: &mut Vm, path: &str, base: ClassId, props: &[(&str, PropKind)]) -> ClassId {
    let props = props.iter().map(|(n, k)| decl(vm, n, k.clone())).collect();
    vm.space.classes.define(path, Some(base), props).unwrap()
}

fn declare(
    vm: &mut Vm,
    owner: ClassId,
    path: &str,
    params: Vec<PropertyDecl>,
    result: Option<PropKind>,
) -> FuncId {
    let mut d = FunctionDecl::native(ObjectPath::parse(path).unwrap(), path).with_owner(owner);
    d.params = params
        .into_iter()
        .map(|p| p.with_flags(PropertyDecl::PARAM))
        .collect();
    d.result = result.map(|k| decl(vm, "ReturnValue", k).with_flags(PropertyDecl::RETURN));
    vm.declare_function(d).unwrap()
}

fn not_read(e: &VmError) -> bool {
    matches!(e, VmError::NotYet(n) if n.missing == Missing::NotRead)
}

/// Each kind: a None tag does nothing, a pair is appended once, a release of a pair not there or with a None tag does
/// nothing, the pair's release empties the list; the other lists are untouched.
#[test]
fn each_kind_keeps_name_and_tag_pairs_on_its_own_list() {
    let mut e = Env::new(ParityMode::Off);
    register(&mut e.vm).unwrap();
    for (kind, list) in KINDS {
        let component = e.make(e.component, "ToyComponent_0");
        let (t, u) = (e.make(e.thing, "ToyTag_0"), e.make(e.thing, "ToyTag_1"));
        let on = format!("{kind}OnHold");
        let (hold, release) = (format!("Hold{kind}"), format!("Release{kind}"));
        assert_eq!(e.call(&on, component, vec![]).unwrap(), Value::Bool(false), "{kind}");
        e.hold(&hold, component, "Safe", None).unwrap();
        assert!(e.pairs(component, list).is_empty(), "{kind}: a None tag holds nothing");
        e.hold(&hold, component, "Safe", Some(t)).unwrap();
        e.hold(&hold, component, "Safe", Some(t)).unwrap();
        e.hold(&hold, component, "Safe", Some(u)).unwrap();
        e.hold(&hold, component, "Other", Some(u)).unwrap();
        assert_eq!(
            e.pairs(component, list),
            [
                ("Safe".to_string(), Some(t)),
                ("Safe".to_string(), Some(u)),
                ("Other".to_string(), Some(u))
            ],
            "{kind}: appended at the end, the same pair not twice"
        );
        assert_eq!(e.call(&on, component, vec![]).unwrap(), Value::Bool(true), "{kind}");
        for (_, other) in KINDS.iter().filter(|(k, _)| *k != kind) {
            assert!(e.pairs(component, other).is_empty(), "{kind} left {other} alone");
        }
        e.hold(&release, component, "Other", None).unwrap();
        e.hold(&release, component, "Missing", Some(u)).unwrap();
        assert_eq!(e.pairs(component, list).len(), 3, "{kind}: nothing released");
        for (name, tag) in [("Other", u), ("Safe", u), ("Safe", t)] {
            e.hold(&release, component, name, Some(tag)).unwrap();
        }
        assert!(e.pairs(component, list).is_empty(), "{kind}");
        assert_eq!(e.call(&on, component, vec![]).unwrap(), Value::Bool(false), "{kind}");
    }
}

/// Whether the entries after a removed one keep their order is not read: a release before the last entry is `NotYet`
/// and leaves the list as it was.
#[test]
fn a_release_before_the_last_entry_is_not_yet_and_changes_nothing() {
    let mut e = Env::new(ParityMode::Off);
    register(&mut e.vm).unwrap();
    let component = e.make(e.component, "ToyComponent_0");
    let t = e.make(e.thing, "ToyTag_0");
    e.hold("HoldGod", component, "First", Some(t)).unwrap();
    e.hold("HoldGod", component, "Second", Some(t)).unwrap();
    let err = e.hold("ReleaseGod", component, "First", Some(t)).unwrap_err();
    assert!(not_read(&err), "{err}");
    assert!(err.to_string().contains("NATIVE_AI_HOLDS.md"), "{err}");
    assert_eq!(e.pairs(component, "GodHolds").len(), 2);
}

/// The known append precedes the unread movement lookup. A duplicate skips it; release has no motion effect.
#[test]
fn movement_appends_before_refusing_unresolved_side_effect() {
    let mut e = Env::new(ParityMode::Off);
    register(&mut e.vm).unwrap();
    let component = e.make(e.component, "ToyComponent");
    let controller = e.make(e.controller, "ToyController");
    let pawn = e.make(e.pawn, "ToyPawn");
    let tag = e.make(e.thing, "ToyTag");
    set(&mut e.vm, component, "Owner", Value::Object(Some(controller))).unwrap();
    set(&mut e.vm, controller, "Pawn", Value::Object(Some(pawn))).unwrap();
    let motion = e.vector([10.0, -20.0, 30.0]);
    set(&mut e.vm, pawn, "Velocity", motion.clone()).unwrap();
    set(&mut e.vm, pawn, "Acceleration", motion.clone()).unwrap();
    let err = e.hold("HoldMovement", component, "Safe", Some(tag)).unwrap_err();
    assert!(not_read(&err), "{err}");
    assert_eq!(e.pairs(component, "MovementHolds"), [("Safe".into(), Some(tag))]);
    assert_eq!(get(&mut e.vm, pawn, "Velocity").unwrap(), Some(motion.clone()));
    assert_eq!(get(&mut e.vm, pawn, "Acceleration").unwrap(), Some(motion));
    e.hold("HoldMovement", component, "Safe", Some(tag)).unwrap();
    e.hold("ReleaseMovement", component, "Safe", Some(tag)).unwrap();
    assert!(e.pairs(component, "MovementHolds").is_empty());
}

/// `WillowAIComponent` keeps the base routines of the four kinds (NATIVE_AI_HOLDS.md, read 2026-11-19; decision
/// 2026-11-19-43): its holds follow the rule, as an `AIComponent`'s do.
#[test]
fn willow_components_keep_the_four_kinds_routines() {
    let mut e = Env::new(ParityMode::Off);
    register(&mut e.vm).unwrap();
    let base = e.component;
    let willow = class(&mut e.vm, WILLOW_COMPONENT, base, &[]);
    let component = e.make(willow, "ToyWillowComponent");
    let tag = e.make(e.thing, "ToyTag");
    for (kind, list) in KINDS {
        if kind == "Movement" {
            continue; // its side effect on an owned component is the NotYet of movement_appends_before_refusing...
        }
        e.hold(&format!("Hold{kind}"), component, "Safe", Some(tag)).unwrap();
        assert_eq!(e.pairs(component, list), vec![("Safe".to_string(), Some(tag))], "{kind}");
        assert_eq!(e.call(&format!("{kind}OnHold"), component, vec![]).unwrap(), Value::Bool(true), "{kind}");
        e.hold(&format!("Release{kind}"), component, "Safe", Some(tag)).unwrap();
        assert!(e.pairs(component, list).is_empty(), "{kind}");
    }
    // Movement with no owner: no side effect, the pair kept.
    e.hold("HoldMovement", component, "Safe", Some(tag)).unwrap();
    assert_eq!(e.call("MovementOnHold", component, vec![]).unwrap(), Value::Bool(true));
}

/// Any other subclass's table was not read: refused for every kind before anything changes.
#[test]
fn subclass_virtuals_are_refused_for_all_kinds_before_mutation() {
    let mut e = Env::new(ParityMode::Off);
    register(&mut e.vm).unwrap();
    let base = e.component;
    let subclass = class(&mut e.vm, "WillowGame.ToyOtherAIComponent", base, &[]);
    let component = e.make(subclass, "ToyOtherComponent");
    let tag = e.make(e.thing, "ToyTag");
    for (kind, list) in KINDS {
        for verb in [format!("Hold{kind}"), format!("Release{kind}")] {
            let err = e.hold(&verb, component, "Safe", Some(tag)).unwrap_err();
            assert!(not_read(&err), "{err}");
        }
        let err = e.call(&format!("{kind}OnHold"), component, vec![]).unwrap_err();
        assert!(not_read(&err), "{err}");
        assert!(e.pairs(component, list).is_empty());
    }
}

/// Every heap object's class, slots and origin, and the store's load state: equal before and after means nothing was
/// written, made or loaded (#518's counters are not on main yet).
fn fingerprint(vm: &Vm) -> String {
    vm.space
        .heap
        .handles()
        .into_iter()
        .map(|h| {
            let o = vm.space.heap.get(h).unwrap();
            format!("{h:?} {:?} {:?} {:?} {}\n", o.class, o.origin, o.slots, vm.defs.is_unloaded(&vm.space, h))
        })
        .collect()
}

/// An invented `GearboxMind` (with `AIComponent`) and a `WillowMind` under it, at the paths the native reads.
fn minds(e: &mut Env) -> (ClassId, ClassId) {
    let root = e.vm.space.classes.by_path("Toy.ToyObject").unwrap();
    let o = PropKind::Object { class: root };
    let gearbox = class(&mut e.vm, MIND, root, &[("AIComponent", o)]);
    let willow = class(&mut e.vm, "WillowGame.WillowMind", gearbox, &[]);
    (gearbox, willow)
}

/// `GetAIComponent` in Off (NATIVE_GEARBOXMIND_GETAICOMPONENT.md section 3): the mind's own `AIComponent` as it is,
/// on a `GearboxMind` and on a `WillowMind` alike; None stays None and nothing is made; a stand-in handle is returned
/// as the handle, unread. Each call leaves every heap object and the store as they were.
#[test]
fn get_ai_component_answers_the_mind_s_own_component() {
    let mut e = Env::new(ParityMode::Off);
    register(&mut e.vm).unwrap();
    let (gearbox, willow) = minds(&mut e);
    let component = e.make(e.component, "ToyComponent_0");
    let stand_in = e.vm.space.alloc_stand_in(e.component, "ToyStandIn", Origin::Resource { package: 0, index: -1 }).unwrap();
    for (mind_class, value) in [
        (gearbox, Some(component)),
        (willow, Some(component)),
        (willow, None),
        (gearbox, Some(stand_in)),
    ] {
        let mind = e.make(mind_class, "ToyMind_0");
        set(&mut e.vm, mind, "AIComponent", Value::Object(value)).unwrap();
        let before = fingerprint(&e.vm);
        assert_eq!(e.call("GetAIComponent", mind, vec![]).unwrap(), Value::Object(value));
        assert_eq!(fingerprint(&e.vm), before, "nothing written, made or loaded");
    }
}

/// A call with no receiver, or on a class outside `GearboxMind` (here a controller), is a `VmError::Script`. A mind
/// class without `AIComponent`, or a value of another kind, cannot be built here: every `GearboxMind` subclass
/// inherits the object field, and the heap refuses a write of another kind.
#[test]
fn get_ai_component_refuses_malformed_calls_as_script_errors() {
    let mut e = Env::new(ParityMode::Off);
    register(&mut e.vm).unwrap();
    minds(&mut e);
    let f = e.native("GetAIComponent");
    let err = e.vm.call(f, None, vec![]).unwrap_err();
    assert!(matches!(err, VmError::Script { .. }), "no receiver: {err:?}");
    let controller = e.make(e.controller, "ToyController");
    let err = e.vm.call(f, Some(controller), vec![]).unwrap_err();
    assert!(matches!(&err, VmError::Script { message, .. } if message.contains("is not a GearboxMind")), "{err:?}");
}

/// Under Reference this pack binds nothing (the `hit.rs` stand-in answers there); a key bound first keeps its binding.
#[test]
fn reference_binds_nothing_and_a_key_bound_first_is_kept() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let mut e = Env::new(mode);
        let taken = e.native("DialogOnHold");
        let key = e.vm.function(taken).unwrap().native_key.clone().unwrap();
        e.vm
            .register_native(&key, Rc::new(|_c: &mut NativeCall<'_>| Ok(Value::Bool(true))))
            .unwrap();
        register(&mut e.vm).unwrap();
        if mode == ParityMode::Reference {
            for (name, f) in e.natives.clone() {
                if name == "DialogOnHold" {
                    continue;
                }
                let key = e.vm.function(f).unwrap().native_key.clone().unwrap();
                assert!(!e.vm.has_native(&key), "{name}: unbound under Reference");
            }
        }
        let component = e.make(e.component, "ToyComponent_0");
        assert_eq!(e.call("DialogOnHold", component, vec![]).unwrap(), Value::Bool(true), "{mode:?}: kept");
    }
}

#[test]
fn callers_and_missing_layout_errors_are_not_silently_accepted() {
    let mut e = Env::new(ParityMode::Off);
    register(&mut e.vm).unwrap();
    let f = e.native("GodOnHold");
    assert!(e.vm.call(f, None, vec![]).is_err());
    let component = e.make(e.component, "ToyOne");
    let other = e.make(e.component, "ToyTwo");
    let tag = e.make(e.thing, "ToyTag");
    e.hold("HoldGod", component, "Safe", Some(tag)).unwrap();
    assert_eq!(e.call("GodOnHold", other, vec![]).unwrap(), Value::Bool(false));
    // Omitted VM arguments receive defaults; a default null tag is a no-op.
    e.call("HoldGod", component, vec![]).unwrap();
    assert_eq!(e.pairs(component, "GodHolds").len(), 1);
}
