//! `SetDefaultStance`, `ApplyFlagInitializationData` and `ApplyPlayThroughBasedPlayerAttributeEffects` on invented classes (AGENTS.md rule 9; NATIVE_MAYA9_AND_AFTER.md
//! section 6, tests 1 to 6, 8 and 9; lane IMPL-MAYA9-NATIVES). The class and field names are the packages' names the
//! natives read; every value is invented. AI-assisted (Claude Code).

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::registry::AttributeSystem;
use ow_core::{Missing, ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ClassId, FuncId, ObjRef, PropAddr};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_package::testkit::script::ScriptToy;
use ow_vm::api::{FunctionDecl, NativePack, Vm};
use ow_vm::error::VmError;

use super::*;

static NEXT: AtomicU32 = AtomicU32::new(0);

thread_local! {
    /// Each stance event the native dispatched to the kernel, (event, provider path), in order (review of #781).
    pub(super) static FIRED: std::cell::RefCell<Vec<(String, String)>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn fired() -> Vec<(String, String)> {
    FIRED.with(|f| f.borrow().clone())
}

struct Env {
    vm: Vm,
    _dir: PathBuf,
    pawn: ClassId,
    body: ClassId,
    stance: ClassId,
    aim: ClassId,
    control: ClassId,
    bare: ClassId,
    mesh: ClassId,
    tree: ClassId,
    ai: ClassId,
    set_default: FuncId,
    pawn_default: FuncId,
}

fn decl(vm: &mut Vm, name: &str, kind: PropKind) -> PropertyDecl {
    PropertyDecl::new(vm.space.names.intern(name), kind)
}

fn class(vm: &mut Vm, path: &str, base: Option<ClassId>, props: &[(&str, PropKind)]) -> ClassId {
    let props = props.iter().map(|(n, k)| decl(vm, n, k.clone())).collect();
    vm.space.classes.define(path, base, props).unwrap()
}

fn env() -> Env {
    let dir = std::env::temp_dir().join(format!("ow-combat-stance-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let core = {
        let mut t = ScriptToy::new(&[]);
        t.class("Object", 0);
        t.build()
    };
    let defs = DefStore::from_synthetic(&dir, &[("Core.upk", &core)], IdentityPolicy::FirstInScanOrder, ParityMode::Off)
        .unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
    let bare = class(&mut vm, "Toy.Bare", None, &[]);
    let o = PropKind::Object { class: bare };
    let roles = ["ROLE_None", "ROLE_SimulatedProxy", "ROLE_AutonomousProxy", "ROLE_Authority"]
        .map(|n| vm.space.names.intern(n))
        .to_vec();
    let role = vm.space.classes.define_enum("Engine.Actor.ENetRole", roles).unwrap();
    let physics = ["PHYS_None", "PHYS_Walking", "PHYS_Ladder"].map(|n| vm.space.names.intern(n)).to_vec();
    let physics = vm.space.classes.define_enum("Engine.Actor.EPhysics", physics).unwrap();
    let pid = decl(&mut vm, "PID", PropKind::Int);
    let handle = vm.space.classes.define_struct("Toy.BehaviorConsumerHandle", None, vec![pid]).unwrap();
    let stance = class(&mut vm, "WillowGame.StanceTypeDefinition", Some(bare), &[
        ("bCanUseAimOffsets", PropKind::Bool),
        ("bAllowLeftHandIK", PropKind::Bool),
        ("AimOffsetName", PropKind::Name),
        ("BehaviorProviderDefinition", o.clone()),
    ]);
    let body = class(&mut vm, "WillowGame.BodyClassDefinition", Some(bare), &[
        ("RunStance", o.clone()),
        ("PatrolStance", o.clone()),
        ("SprintStance", o.clone()),
        ("InjuredStance", o.clone()),
        ("CrouchStance", o.clone()),
        ("AnimationParentBody", o.clone()),
        ("bUseParentStances", PropKind::Bool),
        ("AimOffsetNodeName", PropKind::Name),
    ]);
    let aim = class(&mut vm, "WillowGame.WillowAnimNodeAimOffset", Some(bare), &[("NodeName", PropKind::Name)]);
    let control = class(&mut vm, "Engine.SkelControlBase", Some(bare), &[
        ("StrengthTarget", PropKind::Float),
        ("ControlStrength", PropKind::Float),
        ("BlendTimeToGo", PropKind::Float),
        ("BlendInTime", PropKind::Float),
        ("BlendOutTime", PropKind::Float),
        ("bPropagateSetActive", PropKind::Bool),
        ("NextControl", o.clone()),
    ]);
    let pawn_fields: Vec<(&str, PropKind)> = vec![
        ("BodyClass", o.clone()),
        ("bIsInjured", PropKind::Bool),
        ("bIsCrouched", PropKind::Bool),
        ("bIsSprinting", PropKind::Int),
        ("bNetDirty", PropKind::Bool),
        ("bCanCrouch", PropKind::Bool),
        ("Mesh", o.clone()),
        ("Role", PropKind::Byte { enum_: Some(role) }),
        ("Physics", PropKind::Byte { enum_: Some(physics) }),
        ("AimOffset", o.clone()),
        ("LeftHandIK", o.clone()),
        ("LeftHandIKHolds", PropKind::Array(Box::new(o.clone()))),
        ("Controller", o.clone()),
        ("CurrentStance", o.clone()),
        ("OverrideStance", o.clone()),
        ("ConsumerHandle", PropKind::Struct(handle)),
    ];
    let willow_pawn = class(&mut vm, "WillowGame.WillowPawn", Some(bare), &pawn_fields);
    let pawn = class(&mut vm, "WillowGame.WillowPlayerPawn", Some(willow_pawn), &[]);
    let ai = class(&mut vm, "WillowGame.WillowAIPawn", Some(willow_pawn), &[]);
    // A mesh with an anim tree, for the aim-offset lookup (FindAnimNode's reads).
    let mesh = class(&mut vm, "Toy.SkeletalMeshComponent", Some(bare), &[
        ("Animations", o.clone()),
        ("AnimTickArray", PropKind::Array(Box::new(o.clone()))),
    ]);
    let tree = class(&mut vm, "Engine.AnimTree", Some(bare), &[("NodeName", PropKind::Name)]);
    vm.add_service(ow_world::world::tree_lookup::TreeLookups::default()).unwrap();
    let pawn_default = vm
        .declare_function(
            FunctionDecl::native(ObjectPath::parse(PAWN_SET_DEFAULT_STANCE).unwrap(), PAWN_SET_DEFAULT_STANCE)
                .with_owner(willow_pawn),
        )
        .unwrap();
    let set_default = vm
        .declare_function(
            FunctionDecl::native(ObjectPath::parse(PLAYER_SET_DEFAULT_STANCE).unwrap(), PLAYER_SET_DEFAULT_STANCE)
                .with_owner(pawn),
        )
        .unwrap();
    // The kernel's RegisterBehaviorConsumer, so a test can make a kernel-owned consumer.
    let consumer = decl(&mut vm, "Consumer", PropKind::Object { class: bare }).with_flags(PropertyDecl::PARAM);
    let result = decl(&mut vm, "ReturnValue", PropKind::Struct(handle));
    let path = ow_behavior::kernel::natives::REGISTER_CONSUMER;
    vm.declare_function(
        FunctionDecl::native(ObjectPath::parse(path).unwrap(), path)
            .with_static(true)
            .with_params(vec![consumer])
            .with_result(result),
    )
    .unwrap();
    ow_behavior::kernel::natives::Pack.register(&mut vm).unwrap();
    register(&mut vm).unwrap();
    FIRED.with(|f| f.borrow_mut().clear());
    Env { vm, _dir: dir, pawn, body, stance, aim, control, bare, mesh, tree, ai, set_default, pawn_default }
}

impl Env {
    fn make(&mut self, c: ClassId, name: &str) -> ObjRef {
        self.vm.space.alloc(c, name, None, Origin::Runtime).unwrap()
    }
    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap();
        *self.vm.space.slot_mut(o, PropAddr::scalar(p)).unwrap() = v;
    }
    fn get(&self, o: ObjRef, name: &str) -> Value {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap();
        self.vm.space.read(o, PropAddr::scalar(p)).unwrap().clone()
    }
    fn name(&mut self, s: &str) -> Value {
        Value::Name(self.vm.space.names.intern(s))
    }
    fn stance(&mut self, name: &str, aim: bool) -> ObjRef {
        let s = self.make(self.stance, name);
        self.set(s, "bCanUseAimOffsets", Value::Bool(aim));
        self.set(s, "bAllowLeftHandIK", Value::Bool(true));
        s
    }
    /// A body class with its five stances (all aim-capable), node name `Aim`.
    fn body(&mut self, name: &str) -> (ObjRef, [ObjRef; 5]) {
        let b = self.make(self.body, name);
        let mut five = [b; 5];
        for (i, f) in FIVE.iter().enumerate() {
            let s = self.stance(&format!("{name}_{f}"), true);
            self.set(b, f, Value::Object(Some(s)));
            five[i] = s;
        }
        let aim = self.name("Aim");
        self.set(b, "AimOffsetNodeName", aim);
        (b, five)
    }
    /// An authority player pawn on `body`, with a mesh, an `AimOffset` node already named `Aim`, a handle no one owns,
    /// and an archetype whose `bCanCrouch` is true.
    fn pawn(&mut self, body: ObjRef) -> ObjRef {
        let p = self.make(self.pawn, "Maya");
        let mesh = self.make(self.mesh, "Mesh");
        let node = self.make(self.aim, "AimNode");
        let aim = self.name("Aim");
        self.set(node, "NodeName", aim);
        self.set(p, "BodyClass", Value::Object(Some(body)));
        self.set(p, "Mesh", Value::Object(Some(mesh)));
        self.set(p, "AimOffset", Value::Object(Some(node)));
        self.set(p, "Role", Value::Byte(3));
        self.set(p, "LeftHandIKHolds", Value::array(vec![]));
        let handle = self.vm.space.classes.struct_by_path("Toy.BehaviorConsumerHandle").unwrap();
        self.set(p, "ConsumerHandle", Value::structure(handle, vec![Value::Int(-1)]));
        let archetype = self.make(self.pawn, "Pawn_Toy");
        self.set(archetype, "bCanCrouch", Value::Bool(true));
        self.vm.space.heap.get_mut(p).unwrap().archetype = Some(archetype);
        p
    }
    fn run(&mut self, p: ObjRef) -> Result<Value, VmError> {
        Ok(self.vm.call(self.set_default, Some(p), vec![])?.ret)
    }
    fn writes(&self) -> u64 {
        self.vm.space.heap.committed_writes()
    }
}

// 1, with the hub's ruling: an event on a provider the consumer never registered answers nothing.
#[test]
fn the_run_stance_is_set_with_its_dirty_bit_and_the_archetype_s_crouch() {
    let mut e = env();
    let (b, five) = e.body("Body");
    let p = e.pawn(b);
    e.run(p).unwrap();
    assert_eq!(e.get(p, "CurrentStance"), Value::Object(Some(five[0])));
    assert_eq!(e.get(p, "bNetDirty"), Value::Bool(true));
    assert_eq!(e.get(p, "bCanCrouch"), Value::Bool(true), "the archetype's");
    assert_eq!(e.get(p, "OverrideStance"), Value::Object(None));
}

#[test]
fn a_kernel_consumer_without_the_provider_answers_nothing_and_a_crate_consumer_is_not_read() {
    // Maya's case: the kernel registered the pawn, not the stance's provider.
    let mut e = env();
    let (b, five) = e.body("Body");
    let p = e.pawn(b);
    let provider = e.make(e.bare, "Provider");
    e.set(five[0], "BehaviorProviderDefinition", Value::Object(Some(provider)));
    let register = e.vm.find_function(ow_behavior::kernel::natives::REGISTER_CONSUMER).unwrap();
    let handle = e.vm.call(register, None, vec![Value::Object(Some(p))]).unwrap().ret;
    let pid = ow_behavior::kernel::natives::consumer_pid(&e.vm, &handle).unwrap();
    assert_eq!(ow_behavior::kernel::natives::owner_of(&e.vm, pid).unwrap(), Some(ow_behavior::kernel::natives::Owner::Kernel));
    e.set(p, "ConsumerHandle", handle);
    e.run(p).unwrap();
    assert_eq!(e.get(p, "CurrentStance"), Value::Object(Some(five[0])));
    assert_eq!(e.get(p, "bCanCrouch"), Value::Bool(true), "the writes after the event ran");
    assert_eq!(fired(), [(ENTER.to_string(), ow_behavior::provider::stock_path(&e.vm, provider).unwrap())], "enter once, no exit");
    // A crate-owned consumer: the typed NotRead in the preflight, nothing written (review of #781, item 1).
    let mut e = env();
    let (b, five) = e.body("Body");
    let p = e.pawn(b);
    let provider = e.make(e.bare, "Provider");
    e.set(five[0], "BehaviorProviderDefinition", Value::Object(Some(provider)));
    let pid = ow_behavior::kernel::natives::allocate(&mut e.vm, "ow-combat", 7).unwrap();
    let handle = e.vm.space.classes.struct_by_path("Toy.BehaviorConsumerHandle").unwrap();
    e.set(p, "ConsumerHandle", Value::structure(handle, vec![Value::Int(pid)]));
    let before = e.writes();
    match e.run(p) {
        Err(VmError::NotYet(n)) => {
            assert_eq!((n.missing, &*n.what), (Missing::NotRead, CRATE_OWNED));
            assert!(!ow_core::notyet::is_partial_writes(&n.to_string()), "{n}");
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(e.writes(), before, "nothing written");
    assert_eq!(e.get(p, "CurrentStance"), Value::Object(None));
    assert!(fired().is_empty());
}

/// Review of #781, item 2: the positive event case. With a current stance and a new one, both with providers, on a
/// kernel consumer: one OnExitStance on the old provider, then one OnEnterStance on the new one; with no current
/// stance, OnEnterStance once and no OnExitStance. (The dispatch is recorded where the native hands the event to the
/// kernel; registering a provider on the process needs the kernel's own provider fixtures.)
#[test]
fn the_events_go_once_each_to_the_kernel_in_order() {
    let mut e = env();
    let (b, five) = e.body("Body");
    let p = e.pawn(b);
    let (old, new) = (e.make(e.bare, "OldProvider"), e.make(e.bare, "NewProvider"));
    e.set(five[1], "BehaviorProviderDefinition", Value::Object(Some(old)));
    e.set(five[0], "BehaviorProviderDefinition", Value::Object(Some(new)));
    e.set(p, "CurrentStance", Value::Object(Some(five[1])));
    let register = e.vm.find_function(ow_behavior::kernel::natives::REGISTER_CONSUMER).unwrap();
    let handle = e.vm.call(register, None, vec![Value::Object(Some(p))]).unwrap().ret;
    e.set(p, "ConsumerHandle", handle);
    e.run(p).unwrap();
    let path = |o| ow_behavior::provider::stock_path(&e.vm, o).unwrap();
    assert_eq!(fired(), [(EXIT.to_string(), path(old)), (ENTER.to_string(), path(new))]);
    assert_eq!(e.get(p, "CurrentStance"), Value::Object(Some(five[0])));
}

/// Review of #781, item 2: an archetype whose `bCanCrouch` is false gives false.
#[test]
fn the_archetype_s_crouch_false_is_copied() {
    let mut e = env();
    let (b, _) = e.body("Body");
    let p = e.pawn(b);
    e.set(p, "bCanCrouch", Value::Bool(true));
    let archetype = e.vm.space.heap.get(p).unwrap().archetype.unwrap();
    e.set(archetype, "bCanCrouch", Value::Bool(false));
    e.run(p).unwrap();
    assert_eq!(e.get(p, "bCanCrouch"), Value::Bool(false));
}

/// Review of #781, item 2: an aim-offset node name that differs looks the node up on the mesh's tree. Found and a
/// `WillowAnimNodeAimOffset`: kept; found of another class, or absent: None.
#[test]
fn a_differing_aim_offset_name_is_looked_up_on_the_mesh() {
    for (wanted, want_found, aim_class) in [("Wanted", true, true), ("Wanted", false, false), ("Absent", false, true)] {
        let mut e = env();
        let (b, five) = e.body("Body");
        let p = e.pawn(b);
        let Value::Object(Some(mesh)) = e.get(p, "Mesh") else { panic!() };
        let tree = e.vm.space.alloc(e.tree, "Tree", Some(mesh), Origin::Runtime).unwrap();
        let class = if aim_class {
            e.aim
        } else {
            class(&mut e.vm, "Toy.OtherAnimNode", Some(e.bare), &[("NodeName", PropKind::Name)])
        };
        let node = e.vm.space.alloc(class, "Node", Some(mesh), Origin::Runtime).unwrap();
        let (none, named) = (Value::Name(ow_object::ids::NameId::NONE), e.name("Wanted"));
        e.set(tree, "NodeName", none);
        e.set(node, "NodeName", named);
        e.set(mesh, "Animations", Value::Object(Some(tree)));
        let ticks = vec![tree, node];
        e.set(mesh, "AnimTickArray", Value::array(ticks.iter().map(|&o| Value::Object(Some(o))).collect()));
        ow_world::world::tree_lookup::record_copy(&e.vm, tree, [node]).unwrap();
        let n = e.name(wanted);
        e.set(five[0], "AimOffsetName", n);
        e.run(p).unwrap();
        let got = e.get(p, "AimOffset");
        if want_found {
            assert_eq!(got, Value::Object(Some(node)), "{wanted}: found");
        } else {
            assert_eq!(got, Value::Object(None), "{wanted} aim class {aim_class}: None");
        }
    }
}

/// Review of #781, item 3 (the hub's ruling): `WillowPawn.SetDefaultStance` dispatches on the receiver's class.
#[test]
fn the_willow_pawn_body_dispatches_on_the_receiver() {
    let mut e = env();
    let (b, five) = e.body("Body");
    let p = e.pawn(b);
    e.vm.call(e.pawn_default, Some(p), vec![]).unwrap();
    assert_eq!(e.get(p, "CurrentStance"), Value::Object(Some(five[0])), "a player pawn gets the player rule");
    let ai = e.make(e.ai, "Enemy");
    match e.vm.call(e.pawn_default, Some(ai), vec![]) {
        Err(VmError::NotYet(n)) => assert_eq!((n.missing, &*n.what), (Missing::NotRead, AI_PAWN)),
        other => panic!("{other:?}"),
    }
    let other = class(&mut e.vm, "Toy.OtherPawn", Some(e.bare), &[]);
    let o = e.make(other, "Other");
    let before = e.writes();
    assert_eq!(e.vm.call(e.pawn_default, Some(o), vec![]).unwrap().ret, Value::None);
    assert_eq!(e.writes(), before, "nothing");
}

// 2.
#[test]
fn the_pick_follows_injured_then_crouched_then_sprinting_and_the_parent() {
    for (field, flag, at) in [("bIsInjured", Value::Bool(true), 3), ("bIsCrouched", Value::Bool(true), 4), ("bIsSprinting", Value::Int(1), 2)] {
        let mut e = env();
        let (b, five) = e.body("Body");
        let p = e.pawn(b);
        e.set(p, field, flag);
        e.run(p).unwrap();
        assert_eq!(e.get(p, "CurrentStance"), Value::Object(Some(five[at])), "{field}");
    }
    let mut e = env();
    let (child, _) = e.body("Child");
    let (parent, parent_five) = e.body("Parent");
    e.set(child, "bUseParentStances", Value::Bool(true));
    e.set(child, "AnimationParentBody", Value::Object(Some(parent)));
    let p = e.pawn(child);
    e.run(p).unwrap();
    assert_eq!(e.get(p, "CurrentStance"), Value::Object(Some(parent_five[0])), "the parent's run stance");
}

// 3.
#[test]
fn the_same_stance_or_a_held_override_writes_nothing() {
    let mut e = env();
    let (b, five) = e.body("Body");
    let p = e.pawn(b);
    e.set(p, "CurrentStance", Value::Object(Some(five[0])));
    let before = e.writes();
    e.run(p).unwrap();
    assert_eq!(e.writes(), before, "the same stance");
    let mut e = env();
    let (b, _) = e.body("Body");
    let p = e.pawn(b);
    let held = e.stance("Held", true);
    e.set(p, "OverrideStance", Value::Object(Some(held)));
    let before = e.writes();
    e.run(p).unwrap();
    assert_eq!(e.writes(), before, "an override held");
    assert_eq!(e.get(p, "CurrentStance"), Value::Object(None));
}

// 4.
#[test]
fn an_override_stance_is_held_and_clears_crouch() {
    let mut e = env();
    let (b, _) = e.body("Body");
    let p = e.pawn(b);
    let over = e.stance("Over", true);
    set_stance(&mut e.vm, p, b, over).unwrap();
    assert_eq!(e.get(p, "OverrideStance"), Value::Object(Some(over)));
    assert_eq!(e.get(p, "CurrentStance"), Value::Object(Some(over)));
    assert_eq!(e.get(p, "bCanCrouch"), Value::Bool(false), "an override");
}

// 5.
#[test]
fn the_aim_test_turning_false_with_a_controller_is_not_read() {
    let mut e = env();
    let (b, five) = e.body("Body");
    let p = e.pawn(b);
    let controller = e.make(e.bare, "Controller");
    e.set(p, "Controller", Value::Object(Some(controller)));
    e.set(p, "CurrentStance", Value::Object(Some(five[1])));
    e.set(five[0], "bCanUseAimOffsets", Value::Bool(false));
    let before = e.writes();
    match e.run(p) {
        Err(VmError::NotYet(n)) => assert_eq!((n.missing, &*n.what), (Missing::NotRead, ROTATION)),
        other => panic!("{other:?}"),
    }
    assert_eq!(e.writes(), before, "nothing written");
}

// 6.
#[test]
fn the_left_hand_ik_control_is_activated_and_propagates() {
    for (blend_in, propagate) in [(2.0f32, true), (0.0, false)] {
        let mut e = env();
        let (b, _) = e.body("Body");
        let p = e.pawn(b);
        let ik = e.make(e.control, "LeftHandIK");
        let next = e.make(e.control, "Next");
        for c in [ik, next] {
            e.set(c, "StrengthTarget", Value::Float(0.0));
            e.set(c, "ControlStrength", Value::Float(0.25));
            e.set(c, "BlendInTime", Value::Float(blend_in));
        }
        e.set(ik, "bPropagateSetActive", Value::Bool(propagate));
        e.set(ik, "NextControl", Value::Object(Some(next)));
        e.set(p, "LeftHandIK", Value::Object(Some(ik)));
        e.run(p).unwrap();
        assert_eq!(e.get(ik, "StrengthTarget"), Value::Float(1.0));
        if blend_in > 0.0 {
            assert_eq!(e.get(ik, "BlendTimeToGo"), Value::Float(0.75 * 2.0));
            assert_eq!(e.get(ik, "ControlStrength"), Value::Float(0.25));
            assert_eq!(e.get(next, "StrengthTarget"), Value::Float(1.0), "propagated");
        } else {
            assert_eq!(e.get(ik, "BlendTimeToGo"), Value::Float(0.0));
            assert_eq!(e.get(ik, "ControlStrength"), Value::Float(1.0));
            assert_eq!(e.get(next, "StrengthTarget"), Value::Float(0.0), "not propagated");
        }
    }
}

// 8.
#[test]
fn flag_initialization_is_nothing_when_empty_and_not_read_for_an_entry() {
    let mut e = env();
    let entry = e.vm.space.classes.define_struct("Toy.FlagInitData", None, vec![]).unwrap();
    let params = vec![
        decl(&mut e.vm, "FlagsToSet", PropKind::Array(Box::new(PropKind::Struct(entry))))
            .with_flags(PropertyDecl::PARAM),
        decl(&mut e.vm, "ContextSource", PropKind::Object { class: e.bare }).with_flags(PropertyDecl::PARAM),
    ];
    let f = e
        .vm
        .declare_function(
            FunctionDecl::native(
                ObjectPath::parse(crate::maya9::APPLY_FLAG_INITIALIZATION).unwrap(),
                crate::maya9::APPLY_FLAG_INITIALIZATION,
            )
            .with_static(true)
            .with_params(params),
        )
        .unwrap();
    crate::maya9::register(&mut e.vm).unwrap();
    let context = e.make(e.bare, "Context");
    let draws = e.vm.rng.draws();
    let r = e.vm.call(f, None, vec![Value::array(vec![]), Value::Object(Some(context))]).unwrap();
    assert_eq!(r.ret, Value::None);
    let one = Value::array(vec![Value::structure(entry, vec![])]);
    let before = e.writes();
    match e.vm.call(f, None, vec![one, Value::Object(Some(context))]) {
        Err(VmError::NotYet(n)) => assert_eq!((n.missing, &*n.what), (Missing::NotRead, crate::maya9::FLAG_ENTRY)),
        other => panic!("{other:?}"),
    }
    assert_eq!((e.writes(), e.vm.rng.draws()), (before, draws), "nothing written or drawn");
}

// 9.
#[test]
fn playthrough_effects_are_nothing_at_one_or_below_and_not_read_above() {
    let mut e = env();
    let gri_class = class(&mut e.vm, "WillowGame.WillowGameReplicationInfo", Some(e.bare), &[("CurrentPlaythrough", PropKind::Int)]);
    let o = PropKind::Object { class: e.bare };
    let world_class = class(&mut e.vm, "Engine.WorldInfo", Some(e.bare), &[("GRI", o.clone())]);
    let params = vec![decl(&mut e.vm, "Target", o).with_flags(PropertyDecl::PARAM)];
    let path = crate::maya9::APPLY_PLAYTHROUGH_EFFECTS;
    let f = e
        .vm
        .declare_function(FunctionDecl::native(ObjectPath::parse(path).unwrap(), path).with_static(true).with_params(params))
        .unwrap();
    crate::maya9::register(&mut e.vm).unwrap();
    let target = e.make(e.bare, "Target");
    let arg = || vec![Value::Object(Some(target))];
    // No world: the world layer is absent.
    match e.vm.call(f, None, arg()) {
        Err(VmError::NotYet(n)) => assert!(matches!(n.missing, Missing::LayerAbsent(_)), "{n}"),
        other => panic!("{other:?}"),
    }
    let world = e.make(world_class, "WorldInfo");
    let actors = e.vm.add_service(ow_world::actors::Actors::new(e.bare)).unwrap();
    actors.borrow_mut().place_world_info(world).unwrap();
    // No GRI, a None argument: nothing.
    assert_eq!(e.vm.call(f, None, arg()).unwrap().ret, Value::None);
    assert_eq!(e.vm.call(f, None, vec![Value::Object(None)]).unwrap().ret, Value::None);
    let gri = e.make(gri_class, "GRI");
    e.set(world, "GRI", Value::Object(Some(gri)));
    for playthrough in [0, 1] {
        e.set(gri, "CurrentPlaythrough", Value::Int(playthrough));
        assert_eq!(e.vm.call(f, None, arg()).unwrap().ret, Value::None, "{playthrough}");
    }
    e.set(gri, "CurrentPlaythrough", Value::Int(2));
    let before = e.writes();
    match e.vm.call(f, None, arg()) {
        Err(VmError::NotYet(n)) => {
            assert_eq!((n.missing.clone(), &*n.what), (Missing::NotRead, crate::maya9::PLAYTHROUGH_TABLE))
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(e.writes(), before, "reads only");
}
