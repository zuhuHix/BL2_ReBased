//! NATIVE_PAWN_INTERACTION_PROXY.md section 7, tests 1 to 5, on two invented packages (AGENTS.md rule 9; every value
//! is invented, the class and field names are the packages'):
//! - `Engine`: `Actor` (the fields `SetBase`, `SetHardAttach` and `SetCollisionSize` read, `Role` with an `ENetRole`
//!   enum, the three natives, and spy events `Detach`, `Attach`, `BaseChange`, `CollisionChanged`), `Trigger`,
//!   `PrimitiveComponent`, `CylinderComponent`;
//! - `WillowGame`: `InteractionProxy` (a spy `BaseChange` that, when asked to, marks a base-less proxy deleted as the
//!   script's destroy would), `PawnInteractionProxy`, `WillowWeapon`.
//!
//! The toy class bodies do not decode, so each toy class is its own first native class. AI-assisted (Claude Code).

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_balance::types::{
    AwesomeLevel, BalanceSnapshot, ExpLevel, GameStage, LevelCapUpgrades, OpLevel, PlayerCount, Playthrough,
};
use ow_core::{Missing, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_package::testkit::reflect::{toy_array_of, toy_struct};
use ow_package::testkit::script::{CPF_OPT, FunctionSpec, Param, ScriptToy};
use ow_package::testkit::{Export, ints};
use ow_vm::api::{NativeCall, StubPolicy, Vm};
use ow_vm::error::VmError;

use super::*;
use crate::world::{BalanceState, World};

const NATIVE_FLAGS: u32 = 0x400 | 0x20000;
static NEXT: AtomicU32 = AtomicU32::new(0);
const IMPORTS: [&str; 12] = [
    "Class",
    "Function",
    "Enum",
    "ObjectProperty",
    "BoolProperty",
    "ByteProperty",
    "NameProperty",
    "ScriptStruct",
    "StructProperty",
    "FloatProperty",
    "IntProperty",
    "ArrayProperty",
];
const ROLES: [&str; 5] = ["ROLE_None", "ROLE_SimulatedProxy", "ROLE_AutonomousProxy", "ROLE_Authority", "ROLE_MAX"];
const AUTHORITY: u8 = 3;
const AUTONOMOUS: u8 = 2;
const SIMULATED: u8 = 1;
const EVENTS: [&str; 5] = [
    "Engine.Actor.Detach",
    "Engine.Actor.Attach",
    "Engine.Actor.BaseChange",
    "Engine.Actor.CollisionChanged",
    "WillowGame.InteractionProxy.BaseChange",
];

fn native(t: &mut ScriptToy, owner: i32, name: &str, params: &[Param]) {
    t.function(owner, name, params, None, &[], &FunctionSpec::new(NATIVE_FLAGS, name), |_| None);
}

fn opt(kind: &str, name: &str, type_ref: i32) -> Param {
    Param { flags: CPF_OPT, type_ref, ..Param::new(kind, name) }
}

fn engine() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let object = t.class("Object", 0);
    let vector = toy_struct(&mut t, object, "Vector", None);
    for f in ["X", "Y", "Z"] {
        t.prop("Float", vector, f, 0, 0);
    }
    let rotator = toy_struct(&mut t, object, "Rotator", None);
    for f in ["Pitch", "Yaw", "Roll"] {
        t.prop("Int", rotator, f, 0, 0);
    }
    let actor = t.class("Actor", object);
    let mut payload = ints(&[0]);
    payload.extend_from_slice(&t.none);
    payload.extend(ints(&[0, ROLES.len() as i32]));
    for v in ROLES {
        payload.extend_from_slice(&t.p.fname(v, 0));
    }
    let mut e = Export::new(t.import("Enum"), "ENetRole", payload);
    e.outer = actor;
    let roles = t.p.add_export_with(e);
    t.prop("Byte", actor, "Role", 0, roles);
    for b in ["bStatic", "bMovable", "bDeleteMe", "bWorldGeometry", "bHardAttach", "bNetDirty", "bTickIsDisabled"] {
        t.prop("Bool", actor, b, 0, 0);
    }
    t.prop("Object", actor, "Base", 0, actor);
    t.prop("Object", actor, "BaseSkelComponent", 0, object);
    t.prop("Name", actor, "BaseBoneName", 0, 0);
    t.prop("Object", actor, "CollisionComponent", 0, object);
    for f in ["Location", "RelativeLocation", "DrawScale3D", "PrePivot"] {
        t.prop("Struct", actor, f, 0, vector);
    }
    for f in ["Rotation", "RelativeRotation"] {
        t.prop("Struct", actor, f, 0, rotator);
    }
    t.prop("Float", actor, "DrawScale", 0, 0);
    toy_array_of(&mut t, actor, "Attached", "Object", actor);
    native(&mut t, actor, "SetBase", &[opt("Object", "NewBase", actor), opt("Struct", "NewFloor", vector), opt("Object", "SkelComp", object), opt("Name", "AttachName", 0)]);
    native(&mut t, actor, "SetCollisionSize", &[Param::new("Float", "NewRadius"), Param::new("Float", "NewHeight")]);
    native(&mut t, actor, "SetHardAttach", &[opt("Bool", "bNewHardAttach", 0)]);
    native(&mut t, actor, "Detach", &[opt("Object", "Other", actor)]);
    native(&mut t, actor, "Attach", &[opt("Object", "Other", actor)]);
    native(&mut t, actor, "BaseChange", &[]);
    native(&mut t, actor, "CollisionChanged", &[]);
    t.class("Trigger", actor);
    let primitive = t.class("PrimitiveComponent", object);
    t.prop("Object", primitive, "Owner", 0, actor);
    let pointer = toy_struct(&mut t, object, "Pointer", None);
    t.prop("Int", pointer, "Dummy", 0, 0);
    t.prop("Struct", primitive, "Scene", 0, pointer);
    t.prop("Bool", primitive, "bNeedsReattach", 0, 0);
    let cylinder = t.class("CylinderComponent", primitive);
    t.prop("Float", cylinder, "CollisionRadius", 0, 0);
    t.prop("Float", cylinder, "CollisionHeight", 0, 0);
    t.p.build()
}

fn willow() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let actor = t.p.add_import("Core", "Class", engine, "Actor");
    let proxy = t.class("InteractionProxy", actor);
    native(&mut t, proxy, "BaseChange", &[]);
    t.class("PawnInteractionProxy", proxy);
    t.class("WillowWeapon", actor);
    t.p.build()
}

struct Fx {
    w: World,
    dir: PathBuf,
    /// The spies' calls, in order: (receiver, event).
    calls: Rc<RefCell<Vec<(ObjRef, String)>>>,
    /// Whether `InteractionProxy.BaseChange` marks a base-less proxy deleted.
    destroying: Rc<RefCell<bool>>,
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn fx_in(mode: ParityMode) -> Fx {
    let dir = std::env::temp_dir().join(format!("ow-world-proxy-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let (e, w) = (engine(), willow());
    let defs = DefStore::from_synthetic(&dir, &[("Engine.upk", e.as_slice()), ("WillowGame.upk", w.as_slice())], IdentityPolicy::FirstInScanOrder, mode)
        .unwrap();
    let vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), mode).with_stub_policy(StubPolicy::refuse_all());
    let snapshot = BalanceSnapshot::new(
        Playthrough::NORMAL,
        OpLevel::NONE,
        PlayerCount::SINGLE,
        ExpLevel::new(1),
        GameStage(1),
        AwesomeLevel::new(0),
        LevelCapUpgrades::ALL,
    )
    .unwrap();
    let mut world = World::new(vm, BalanceState::new(snapshot), &[]).unwrap();
    let calls = Rc::new(RefCell::new(Vec::new()));
    let destroying = Rc::new(RefCell::new(false));
    for path in EVENTS {
        let f = world.vm.find_function(path).unwrap();
        let key = world.vm.function(f).unwrap().native_key.clone().unwrap();
        let name = path.rsplit('.').next().unwrap().to_string();
        let (seen, destroy) = (calls.clone(), destroying.clone());
        let proxy_event = path.starts_with("WillowGame");
        world
            .vm
            .register_native(
                &key,
                Rc::new(move |c: &mut NativeCall<'_>| {
                    let me = c.this.unwrap();
                    seen.borrow_mut().push((me, name.clone()));
                    if proxy_event && *destroy.borrow() {
                        let class = c.vm.space.heap.get(me)?.class;
                        let base = c.vm.space.prop(class, "Base").unwrap();
                        if c.vm.space.read(me, PropAddr::scalar(base))? == &Value::Object(None) {
                            let p = c.vm.space.prop(class, "bDeleteMe").unwrap();
                            c.vm.space.write(me, PropAddr::scalar(p), Value::Bool(true))?;
                        }
                    }
                    Ok(Value::None)
                }),
            )
            .unwrap();
    }
    Fx { w: world, dir, calls, destroying }
}

fn fx() -> Fx {
    fx_in(ParityMode::Off)
}

impl Fx {
    fn vm(&mut self) -> &mut Vm {
        &mut self.w.vm
    }
    fn obj(&mut self, class: &str) -> ObjRef {
        let vm = self.vm();
        let c = vm.find_class(class).unwrap();
        let o = ow_vm::interp::instantiate_from_defaults(vm, c).unwrap();
        if vm.space.prop(c, "bMovable").is_some() {
            self.set(o, "bMovable", Value::Bool(true));
            self.set(o, "DrawScale", Value::Float(1.0));
            let ones = self.vector([1.0, 1.0, 1.0]);
            self.set(o, "DrawScale3D", ones);
        }
        o
    }
    fn vector(&mut self, v: [f32; 3]) -> Value {
        let vm = self.vm();
        let ty = vm.space.classes.struct_by_path("Engine.Object.Vector").unwrap();
        Value::structure(ty, v.iter().map(|x| Value::Float(*x)).collect())
    }
    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let vm = self.vm();
        let class = vm.space.heap.get(o).unwrap().class;
        let p = vm.space.prop(class, name).unwrap_or_else(|| panic!("no {name}"));
        vm.space.write(o, PropAddr::scalar(p), v).unwrap();
    }
    fn get(&mut self, o: ObjRef, name: &str) -> Value {
        let vm = self.vm();
        let class = vm.space.heap.get(o).unwrap().class;
        let p = vm.space.prop(class, name).unwrap();
        vm.space.read(o, PropAddr::scalar(p)).unwrap().clone()
    }
    fn counters(&self) -> (u64, u64, u64, usize) {
        let vm = &self.w.vm;
        (vm.space.heap.committed_writes(), vm.rng.draws(), vm.defs.load_transitions(), vm.space.names.len())
    }
    fn call(&mut self, path: &str, this: ObjRef, args: Vec<Value>) -> Result<Value, VmError> {
        let vm = self.vm();
        let f = vm.find_function(path).unwrap();
        Ok(vm.call(f, Some(this), args)?.ret)
    }
    fn events(&self) -> Vec<(ObjRef, String)> {
        self.calls.borrow().clone()
    }
    fn size(&mut self, actor: ObjRef) -> Result<Value, VmError> {
        self.call("Engine.Actor.SetCollisionSize", actor, vec![Value::Float(80.0), Value::Float(78.0)])
    }
    fn hard(&mut self, actor: ObjRef, value: bool) -> Result<Value, VmError> {
        self.call("Engine.Actor.SetHardAttach", actor, vec![Value::Bool(value)])
    }
    fn base(&mut self, actor: ObjRef, base: Option<ObjRef>) -> Result<Value, VmError> {
        self.call("Engine.Actor.SetBase", actor, vec![Value::Object(base)])
    }
}

fn not_yet(r: Result<Value, VmError>) -> (Missing, String) {
    match r {
        Err(VmError::NotYet(n)) => (n.missing.clone(), n.what.to_string()),
        other => panic!("{other:?}"),
    }
}

/// A toy actor whose `CollisionComponent` is a cylinder with no owner and a zero scene (the reattach's no-owner case).
fn sized(x: &mut Fx, class: &str) -> (ObjRef, ObjRef) {
    let a = x.obj(class);
    let c = x.obj("Engine.CylinderComponent");
    x.set(a, "CollisionComponent", Value::Object(Some(c)));
    (a, c)
}

// Test 1: SetCollisionSize.
#[test]
fn set_collision_size_writes_in_order_under_the_stand_in() {
    let mut x = fx();
    let (a, c) = sized(&mut x, "Engine.Actor");
    // No collision service and no setting: the touch update's LayerAbsent, nothing written.
    let before = x.counters();
    assert_eq!(not_yet(x.size(a)), (Missing::LayerAbsent(TOUCH_LAYER), TOUCH_UPDATE.to_string()));
    assert_eq!(x.counters(), before);
    // The setting on: the sizes, the reattach, the event, bNetDirty, and one line.
    enable_nothing_touched(x.vm()).unwrap();
    assert!(x.w.vm.log.iter().any(|l| l == NOTHING_TOUCHED_INPUT));
    let logged = x.w.vm.log.len();
    x.size(a).unwrap();
    assert_eq!((x.get(c, "CollisionRadius"), x.get(c, "CollisionHeight")), (Value::Float(80.0), Value::Float(78.0)));
    assert_eq!(x.get(c, "bNeedsReattach"), Value::Bool(true));
    assert_eq!(x.get(a, "bNetDirty"), Value::Bool(true));
    assert_eq!(x.events(), [(a, "CollisionChanged".to_string())]);
    assert_eq!(&x.w.vm.log[logged..], [NOTHING_TOUCHED_LINE.to_string()], "one line");
    // A non-cylinder CollisionComponent: no size write, the rest.
    let b = x.obj("Engine.Actor");
    let other = x.obj("Engine.PrimitiveComponent");
    x.set(b, "CollisionComponent", Value::Object(Some(other)));
    x.size(b).unwrap();
    assert_eq!(x.get(b, "bNetDirty"), Value::Bool(true));
    assert_eq!(x.get(other, "bNeedsReattach"), Value::Bool(false), "no reattach on a non-cylinder");
    // A collision service: the overlap rule's NotRead, nothing written.
    crate::placement::install_collision(x.vm(), Rc::new(crate::placement::NoLevel)).unwrap();
    let (d, _) = sized(&mut x, "Engine.Actor");
    let before = x.counters();
    assert_eq!(not_yet(x.size(d)), (Missing::NotRead, TOUCH_RULE.to_string()));
    assert_eq!(x.counters(), before);
    // The setting is Off only and turned on once.
    assert!(matches!(enable_nothing_touched(x.vm()), Err(VmError::Registration { .. })), "twice");
    let mut r = fx_in(ParityMode::Reference);
    assert!(matches!(enable_nothing_touched(r.vm()), Err(VmError::Registration { .. })), "Off only");
}

#[test]
fn set_collision_size_refusals_write_nothing() {
    let mut x = fx();
    enable_nothing_touched(x.vm()).unwrap();
    // The reattach's owner rule refuses a static owner, before any write.
    let (a, c) = sized(&mut x, "Engine.Actor");
    let owner = x.obj("Engine.Actor");
    x.set(owner, "bStatic", Value::Bool(true));
    x.set(c, "Owner", Value::Object(Some(owner)));
    let before = x.counters();
    assert_eq!(not_yet(x.size(a)).0, Missing::NotRead);
    assert_eq!(x.counters(), before);
    // A mistyped argument: VmError::Script.
    let (b, _) = sized(&mut x, "Engine.Actor");
    assert!(matches!(
        x.call("Engine.Actor.SetCollisionSize", b, vec![Value::Int(1), Value::Float(2.0)]),
        Err(VmError::Script { .. })
    ));
}

// Test 2: SetHardAttach.
#[test]
fn set_hard_attach_with_and_without_a_base() {
    let mut x = fx();
    let a = x.obj("Engine.Actor");
    // The same value: nothing.
    let before = x.counters();
    x.hard(a, false).unwrap();
    assert_eq!(x.counters(), before);
    // A change with no Base: the flag.
    x.hard(a, true).unwrap();
    assert_eq!(x.get(a, "bHardAttach"), Value::Bool(true));
    // A toy Trigger and a toy InteractionProxy with no Base: the key is bound, the flag set.
    for class in ["Engine.Trigger", "WillowGame.InteractionProxy"] {
        let t = x.obj(class);
        x.hard(t, true).unwrap();
        assert_eq!(x.get(t, "bHardAttach"), Value::Bool(true), "{class}");
    }
    // A change with a Base: SetBase(None), the flag, then SetBase(old) (here an identity: the receiver sits on the
    // base, so the hard attach takes the zero pose).
    let (r, base) = (x.obj("Engine.Actor"), x.obj("Engine.Actor"));
    x.base(r, Some(base)).unwrap();
    x.calls.borrow_mut().clear();
    x.hard(r, true).unwrap();
    assert_eq!(x.get(r, "bHardAttach"), Value::Bool(true));
    assert_eq!(x.get(r, "Base"), Value::Object(Some(base)), "rebased");
    let order: Vec<String> = x.events().into_iter().map(|(_, e)| e).collect();
    assert_eq!(order, ["Detach", "BaseChange", "Attach", "BaseChange"], "SetBase(None) then SetBase(old)");
}

#[test]
fn set_hard_attach_refusals_and_the_prefix() {
    let mut x = fx();
    // A class outside the four (a toy WillowWeapon, on SetBase's list): the unresolved NotRead, nothing written.
    let w = x.obj("WillowGame.WillowWeapon");
    let before = x.counters();
    assert_eq!(not_yet(x.hard(w, true)), (Missing::NotRead, HARD_ATTACH_UNRESOLVED.to_string()));
    assert_eq!(x.counters(), before);
    // A refusal in the first SetBase(None)'s preflight (the old base not loaded): nothing written, no prefix.
    let r = x.obj("Engine.Actor");
    let class = x.w.vm.find_class("Engine.Actor").unwrap();
    let lazy = x.w.vm.space.alloc_stand_in(class, "Lazy", ow_object::heap::Origin::Default { class }).unwrap();
    x.set(r, "Base", Value::Object(Some(lazy)));
    let before = x.counters();
    let (missing, what) = not_yet(x.hard(r, true));
    assert_eq!(missing, Missing::NotRead);
    assert!(!what.starts_with("stopped after partial writes"), "{what}");
    assert_eq!(x.counters(), before);
    // A toy Trigger with a Base (set by hand): a Trigger is not on SetBase's list, so the first SetBase(None) is
    // already refused, before any write and with no prefix (the note's test 2 expected the second; see the decision).
    let (t, base) = (x.obj("Engine.Trigger"), x.obj("Engine.Actor"));
    x.set(t, "Base", Value::Object(Some(base)));
    let before = x.counters();
    let (missing, what) = not_yet(x.hard(t, true));
    assert_eq!((missing, what.as_str()), (Missing::NotRead, crate::world::set_base::NOT_RESOLVED));
    assert_eq!(x.counters(), before);
    // The second SetBase refused: a proxy one unit off its base, hard-attaching as the authority, gets the full
    // inverse's NotRead there, prefixed; the first SetBase(None)'s writes and the flag are kept.
    let (q, base) = (x.obj("WillowGame.PawnInteractionProxy"), x.obj("Engine.Actor"));
    x.set(q, "Role", Value::Byte(AUTHORITY));
    let off = x.vector([1.0, 0.0, 0.0]);
    x.set(q, "Location", off);
    x.base(q, Some(base)).unwrap();
    let (missing, what) = not_yet(x.hard(q, true));
    assert_eq!(missing, Missing::NotRead);
    assert!(what.contains(crate::world::set_base::HARD_ATTACH) && what.starts_with("stopped after partial writes"), "{what}");
    assert_eq!((x.get(q, "Base"), x.get(q, "bHardAttach")), (Value::Object(None), Value::Bool(true)), "the first writes kept");
    // A toy InteractionProxy going from hard-attached to not: its BaseChange destroys it; the NotRead, prefixed, and
    // no second SetBase. (An InteractionProxy is not on SetBase's list either, so a PawnInteractionProxy runs this.)
    let (p, base) = (x.obj("WillowGame.PawnInteractionProxy"), x.obj("Engine.Actor"));
    x.set(p, "Role", Value::Byte(SIMULATED));
    x.base(p, Some(base)).unwrap();
    x.set(p, "bHardAttach", Value::Bool(true));
    *x.destroying.borrow_mut() = true;
    x.calls.borrow_mut().clear();
    let (missing, what) = not_yet(x.hard(p, false));
    assert_eq!(missing, Missing::NotRead);
    assert!(what.contains(DESTROYED) && what.starts_with("stopped after partial writes"), "{what}");
    assert_eq!(x.get(p, "Base"), Value::Object(None), "no second SetBase");
    assert!(!x.events().iter().any(|(_, e)| e == "Attach"), "no Attach");
}

// Test 3: SetBase on a toy PawnInteractionProxy, the identity case.
#[test]
fn set_base_on_a_proxy_the_identity_case() {
    for role in [AUTHORITY, AUTONOMOUS] {
        let mut x = fx();
        let (p, maya) = (x.obj("WillowGame.PawnInteractionProxy"), x.obj("Engine.Actor"));
        x.set(p, "bHardAttach", Value::Bool(true));
        x.set(p, "Role", Value::Byte(role));
        x.base(p, Some(maya)).unwrap();
        assert_eq!(x.get(p, "Base"), Value::Object(Some(maya)), "role {role}");
        let zero = x.vector([0.0, 0.0, 0.0]);
        assert_eq!(x.get(p, "RelativeLocation"), zero);
        assert_eq!(crate::actors::objects(&x.w.vm.space, maya, "Attached").unwrap(), [Some(p)]);
        let order: Vec<String> = x.events().into_iter().map(|(_, e)| e).collect();
        assert_eq!(order, ["Attach", "BaseChange"]);
    }
    // A simulated proxy: attached with no pose computation.
    let mut x = fx();
    let (p, maya) = (x.obj("WillowGame.PawnInteractionProxy"), x.obj("Engine.Actor"));
    x.set(p, "bHardAttach", Value::Bool(true));
    x.set(p, "Role", Value::Byte(SIMULATED));
    let away = x.vector([5.0, 0.0, 0.0]);
    x.set(p, "Location", away);
    x.base(p, Some(maya)).unwrap();
    let zero = x.vector([0.0, 0.0, 0.0]);
    assert_eq!(x.get(p, "RelativeLocation"), zero, "no pose written");
}

#[test]
fn set_base_on_a_proxy_off_identity_is_the_hard_attach_not_read() {
    use crate::world::set_base::HARD_ATTACH;
    let cases: [(&str, fn(&mut Fx, ObjRef, ObjRef)); 4] = [
        ("one unit away", |x, p, _| {
            let v = x.vector([1.0, 0.0, 0.0]);
            x.set(p, "Location", v);
        }),
        ("a turned base", |x, p, m| {
            let mut r = x.get(m, "Rotation");
            r.struct_mut().unwrap().fields[1] = Value::Int(100);
            x.set(m, "Rotation", r.clone());
            x.set(p, "Rotation", r);
        }),
        ("DrawScale 2", |x, _, m| x.set(m, "DrawScale", Value::Float(2.0))),
        ("a PrePivot", |x, _, m| {
            let v = x.vector([0.0, 0.0, 3.0]);
            x.set(m, "PrePivot", v);
        }),
    ];
    for (label, setup) in cases {
        let mut x = fx();
        let (p, maya) = (x.obj("WillowGame.PawnInteractionProxy"), x.obj("Engine.Actor"));
        x.set(p, "bHardAttach", Value::Bool(true));
        x.set(p, "Role", Value::Byte(AUTHORITY));
        setup(&mut x, p, maya);
        let before = x.counters();
        assert_eq!(not_yet(x.base(p, Some(maya))), (Missing::NotRead, HARD_ATTACH.to_string()), "{label}");
        assert_eq!(x.counters(), before, "{label}: nothing written");
    }
}

// Test 4: InteractionProxy.BaseChange after SetBase(None) destroys the proxy (the spy's destroy).
#[test]
fn base_change_after_set_base_none_destroys_the_proxy() {
    let mut x = fx();
    let (p, maya) = (x.obj("WillowGame.PawnInteractionProxy"), x.obj("Engine.Actor"));
    x.set(p, "bHardAttach", Value::Bool(true));
    x.set(p, "Role", Value::Byte(AUTHORITY));
    x.base(p, Some(maya)).unwrap();
    *x.destroying.borrow_mut() = true;
    x.base(p, None).unwrap();
    assert_eq!(x.get(p, "bDeleteMe"), Value::Bool(true));
}

// Test 5: Reference binds neither key.
#[test]
fn reference_binds_neither() {
    let mut x = fx_in(ParityMode::Reference);
    for path in ["Engine.Actor.SetCollisionSize", "Engine.Actor.SetHardAttach"] {
        let f = x.vm().find_function(path).unwrap();
        assert!(!x.w.vm.is_bound(f), "{path}");
    }
}
