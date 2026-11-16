//! Tests of the VM adapter on invented classes and objects (AGENTS.md rule 9), in both parity modes. AI-assisted
//! (Claude Code).
//!
//! The classes are defined straight into the class table at the paths the adapter reads (`Engine.Pawn`,
//! `WillowGame.WillowMind`, the iterator classes, ...), with only the properties it reads; every value is invented.

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::registry::AttributeSystem;
use ow_combat::hit::access::{get, set};
use ow_core::{DetMap, Missing, ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ClassId, ObjRef, StructId};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_package::testkit::Export;
use ow_package::testkit::script::ScriptToy;
use ow_vm::api::{FunctionDecl, NativePack, Vm};

use super::*;

static NEXT: AtomicU32 = AtomicU32::new(0);

pub(crate) struct ToyHost {
    pub(crate) exposure: DetMap<ObjRef, f32>,
    pub(crate) registry: Vec<ObjRef>,
}

impl Host for ToyHost {
    fn exposure(&mut self, _vm: &mut Vm, _viewer: ObjRef, target: ObjRef) -> Result<f32, VmError> {
        Ok(self.exposure.get(&target).copied().unwrap_or(0.0))
    }
    fn targetables(&mut self, _vm: &mut Vm) -> Result<Vec<ObjRef>, VmError> {
        Ok(self.registry.clone())
    }
    fn can_be_targeted(&mut self, _vm: &mut Vm, _actor: ObjRef) -> Result<bool, VmError> {
        Ok(true)
    }
}

pub(crate) struct Env {
    pub(crate) vm: Vm,
    pub(crate) dir: PathBuf,
    pub(crate) mind: ObjRef,
    pub(crate) component: ObjRef,
    pub(crate) ai: ObjRef,
    pub(crate) player: ObjRef,
    pub(crate) globals: ObjRef,
    pub(crate) aidef: ObjRef,
    pub(crate) targeting: ObjRef,
    pub(crate) vector: StructId,
    pub(crate) controller_class: ClassId,
    pub(crate) tree_node: StructId,
    pub(crate) object: ClassId,
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

pub(crate) fn decl(vm: &mut Vm, name: &str, kind: PropKind) -> PropertyDecl {
    PropertyDecl::new(vm.space.names.intern(name), kind)
}

pub(crate) fn class(vm: &mut Vm, path: &str, base: ClassId, props: &[(&str, PropKind)]) -> ClassId {
    let props = props.iter().map(|(n, k)| decl(vm, n, k.clone())).collect();
    vm.space.classes.define(path, Some(base), props).unwrap()
}

pub(crate) fn make(vm: &mut Vm, c: ClassId, name: &str) -> ObjRef {
    vm.space.alloc(c, name, None, Origin::Runtime).unwrap()
}

pub(crate) fn vec3(env_vector: StructId, v: [f32; 3]) -> Value {
    Value::structure(env_vector, v.iter().map(|x| Value::Float(*x)).collect())
}

/// An invented world: an AI pawn at the origin facing +X with a mind and a component bound to an invented AI and
/// targeting definition (the live iterator weights), and a player 3,000 uu in front, enemies by default opinion.
pub(crate) fn env(mode: ParityMode) -> Env {
    env_with(mode, &[])
}

/// [`env`] with more invented packages in the store.
pub(crate) fn env_with(mode: ParityMode, extra: &[(&str, Vec<u8>)]) -> Env {
    let dir = std::env::temp_dir().join(format!("ow-ai-vm-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let core = {
        let mut t = ScriptToy::new(&[]);
        t.class("Object", 0);
        t.build()
    };
    // A toy WillowGame with only the threat enum, in an order unlike the game's (GetThreatLevel answers by name).
    let willowgame = {
        let mut t = ScriptToy::new(&[]);
        let core = t.p.add_import("Core", "Package", 0, "Core");
        let object = t.p.add_import("Core", "Class", core, "Object");
        let component = t.class("WillowAIComponent", object);
        let values = ["ETL_SeenInThreatRadius", "ETL_NoThreat", "ETL_InThreatRadius", "ETL_Provoked"];
        let mut payload = 0i32.to_le_bytes().to_vec();
        payload.extend_from_slice(&t.none);
        payload.extend(0i32.to_le_bytes());
        payload.extend((values.len() as i32).to_le_bytes());
        for v in values {
            payload.extend(t.p.fname(v, 0));
        }
        let mut e = Export::new(t.import("Enum"), "EThreatLevel", payload);
        e.outer = component;
        t.p.add_export_with(e);
        t.build()
    };
    let mut files: Vec<(String, &[u8])> = vec![("Core.upk".into(), core.as_slice()), ("WillowGame.upk".into(), willowgame.as_slice())];
    for (name, bytes) in extra {
        files.push((format!("{name}.upk"), bytes.as_slice()));
    }
    let files: Vec<(&str, &[u8])> = files.iter().map(|(n, b)| (n.as_str(), *b)).collect();
    let defs = DefStore::from_synthetic(&dir, &files, IdentityPolicy::FirstInScanOrder, mode).expect("store");
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), mode);
    // Rooted at the toy Core's Object, so a package class (an action) and these classes share one root.
    let core_object = vm.find_class("Core.Object").unwrap();
    let object = vm.space.classes.define("Toy.ToyObject", Some(core_object), vec![]).unwrap();
    let o = PropKind::Object { class: object };
    let (x, y, z) = (decl(&mut vm, "X", PropKind::Float), decl(&mut vm, "Y", PropKind::Float), decl(&mut vm, "Z", PropKind::Float));
    let vector = vm.space.classes.define_struct("Toy.ToyObject.Vector", None, vec![x, y, z]).unwrap();
    let (p, w, r) = (decl(&mut vm, "Pitch", PropKind::Int), decl(&mut vm, "Yaw", PropKind::Int), decl(&mut vm, "Roll", PropKind::Int));
    let rotator = vm.space.classes.define_struct("Toy.ToyObject.Rotator", None, vec![p, w, r]).unwrap();
    let pawn = class(&mut vm, "Engine.Pawn", object, &[
        ("Location", PropKind::Struct(vector)),
        ("Rotation", PropKind::Struct(rotator)),
        ("Controller", o.clone()),
        ("Allegiance", o.clone()),
        ("bCloaked", PropKind::Bool),
        ("bHidden", PropKind::Bool),
        ("NumAICurrentlyTargeting", PropKind::Int),
        ("CylinderComponent", o.clone()),
        ("DrivenVehicle", o.clone()),
    ]);
    let cylinder = class(&mut vm, "Toy.ToyCylinder", object, &[("CollisionRadius", PropKind::Float)]);
    let allegiance = class(&mut vm, "Toy.ToyAllegiance", object, &[
        ("SelfOpinion", PropKind::Byte { enum_: None }),
        ("DefaultOpinion", PropKind::Byte { enum_: None }),
    ]);
    let mind_class = class(&mut vm, "WillowGame.WillowMind", object, &[
        ("Pawn", o.clone()),
        ("AIComponent", o.clone()),
        ("SightViewCone", PropKind::Float),
        ("MinAggroTimeToConsiderAwareOfTarget", PropKind::Float),
        ("bProvoked", PropKind::Bool),
        ("ProvokedLocation", PropKind::Struct(vector)),
    ]);
    let component_class = class(&mut vm, "WillowGame.WillowAIComponent", object, &[
        ("UpdateRate", PropKind::Float),
        ("AIDef", o.clone()),
        ("TargetingDef", o.clone()),
        ("CurrentTarget", o.clone()),
        ("ScriptedTarget", o.clone()),
        ("Holds", PropKind::Array(Box::new(o.clone()))),
        ("LastStateUpdateTime", PropKind::Float),
        ("LastTargetUpdateTime", PropKind::Float),
        ("LastAttackedTime", PropKind::Float),
        ("LastHurtTime", PropKind::Float),
        ("LastAttacker", o.clone()),
        ("MyWillowMind", o.clone()),
    ]);
    let controller_class = class(&mut vm, "Engine.Controller", object, &[("Pawn", o.clone())]);
    let range = {
        let (lo, hi) = (decl(&mut vm, "MinVal", PropKind::Float), decl(&mut vm, "MaxVal", PropKind::Float));
        vm.space.classes.define_struct("Toy.ToyObject.ToyRange", None, vec![lo, hi]).unwrap()
    };
    let node_fields: Vec<(&str, PropKind)> = vec![
        ("Type", PropKind::Byte { enum_: None }),
        ("Children", PropKind::Array(Box::new(PropKind::Int))),
        ("Resource", o.clone()),
        ("CanRunIf", o.clone()),
        ("TimedStart", PropKind::Struct(range)),
        ("TimedReset", PropKind::Struct(range)),
        ("TimedFail", PropKind::Struct(range)),
        ("bStopIfCannotRun", PropKind::Bool),
        ("bInterrupt", PropKind::Bool),
        ("Weights", PropKind::Array(Box::new(PropKind::Float))),
        ("LoopCountMax", PropKind::Array(Box::new(PropKind::Int))),
        ("Conditions", PropKind::Array(Box::new(o.clone()))),
        ("Action", PropKind::Object { class: core_object }),
    ];
    let node_fields = node_fields.into_iter().map(|(n, k)| decl(&mut vm, n, k)).collect();
    let tree_node = vm.space.classes.define_struct("Toy.ToyObject.ToyTreeNode", None, node_fields).unwrap();
    let aidef_class = class(&mut vm, "Toy.ToyAIDefinition", object, &[
        ("TargetSearchRadius", PropKind::Float),
        ("NodeList", PropKind::Array(Box::new(PropKind::Struct(tree_node)))),
    ]);
    let targeting_class = class(&mut vm, "Toy.ToyTargetingDefinition", object, &[
        ("bOverrideSearchRadius", PropKind::Bool),
        ("SearchRadius", PropKind::Float),
        ("CanTargetIf", o.clone()),
        ("StopTargetingIf", o.clone()),
        ("CanTargetFriendliesIf", o.clone()),
        ("bCanTargetFriendlies", PropKind::Bool),
        ("Knowledge", PropKind::Array(Box::new(o.clone()))),
        ("Prioritization", PropKind::Array(Box::new(o.clone()))),
    ]);
    let globals_class = class(&mut vm, "Toy.ToyGlobals", object, &[("AICloseThreatRadius", PropKind::Float)]);
    let mut knowledge = Vec::new();
    for path in [
        "WillowGame.WillowTI_CalcThreat",
        "WillowGame.WillowTI_CalcCanFire",
        "WillowGame.WillowTI_CalcExposure",
        "WillowGame.WillowTI_CalcDistances",
    ] {
        let c = class(&mut vm, path, object, &[]);
        knowledge.push(Value::Object(Some(make(&mut vm, c, "ToyKnowledge"))));
    }
    knowledge.push(Value::Object(None));
    let mut prioritization = Vec::new();
    for (path, weight, max) in [
        ("GearboxFramework.TargetIteratorPrioritizeNearbyTarget", 1.0f32, Some(4000.0f32)),
        ("GearboxFramework.TargetIteratorPrioritizeExposedTargets", 1.0, None),
        ("GearboxFramework.TargetIteratorPrioritizePreviousTarget", 0.5, None),
    ] {
        let mut props = vec![("Weight", PropKind::Float)];
        if max.is_some() {
            props.push(("MaxDistance", PropKind::Float));
        }
        let c = class(&mut vm, path, object, &props);
        let it = make(&mut vm, c, "ToyPrioritizer");
        set(&mut vm, it, "Weight", Value::Float(weight)).unwrap();
        if let Some(m) = max {
            set(&mut vm, it, "MaxDistance", Value::Float(m)).unwrap();
        }
        prioritization.push(Value::Object(Some(it)));
    }
    let ai = make(&mut vm, pawn, "ToyBeast_0");
    let player = make(&mut vm, pawn, "ToyPlayer_0");
    let controller = make(&mut vm, object, "ToyController_0");
    let mind = make(&mut vm, mind_class, "ToyMind_0");
    let component = make(&mut vm, component_class, "ToyComponent_0");
    let aidef = make(&mut vm, aidef_class, "ToyAIDef");
    let targeting = make(&mut vm, targeting_class, "ToyTargeting");
    let globals = make(&mut vm, globals_class, "ToyGlobals");
    let cyl = make(&mut vm, cylinder, "ToyCylinder_0");
    let (beasts, players) = (make(&mut vm, allegiance, "ToyBeasts"), make(&mut vm, allegiance, "ToyPlayers"));
    for a in [beasts, players] {
        set(&mut vm, a, "SelfOpinion", Value::Byte(2)).unwrap();
        set(&mut vm, a, "DefaultOpinion", Value::Byte(0)).unwrap();
    }
    let sets: Vec<(ObjRef, &str, Value)> = vec![
        (ai, "Controller", Value::Object(Some(mind))),
        (ai, "Allegiance", Value::Object(Some(beasts))),
        (ai, "CylinderComponent", Value::Object(Some(cyl))),
        (cyl, "CollisionRadius", Value::Float(90.0)),
        (player, "Controller", Value::Object(Some(controller))),
        (player, "Allegiance", Value::Object(Some(players))),
        (player, "Location", vec3(vector, [3000.0, 0.0, 0.0])),
        (mind, "Pawn", Value::Object(Some(ai))),
        (mind, "AIComponent", Value::Object(Some(component))),
        (mind, "SightViewCone", Value::Float(0.0)),
        (mind, "MinAggroTimeToConsiderAwareOfTarget", Value::Float(0.1)),
        (component, "UpdateRate", Value::Float(0.25)),
        (component, "AIDef", Value::Object(Some(aidef))),
        (component, "TargetingDef", Value::Object(Some(targeting))),
        (aidef, "TargetSearchRadius", Value::Float(4000.0)),
        (targeting, "Knowledge", Value::Array(Rc::new(knowledge))),
        (targeting, "Prioritization", Value::Array(Rc::new(prioritization))),
        (globals, "AICloseThreatRadius", Value::Float(700.0)),
    ];
    for (o, name, v) in sets {
        assert!(set(&mut vm, o, name, v).unwrap(), "{name}");
    }
    Env { vm, dir, mind, component, ai, player, globals, aidef, targeting, vector, controller_class, tree_node, object }
}

pub(crate) fn host(e: &Env, exposure: f32) -> ToyHost {
    let mut map = DetMap::new();
    map.insert(e.player, exposure);
    ToyHost { exposure: map, registry: vec![e.player, e.ai] }
}

/// Advances the VM's world time to `t` and ticks the mind.
pub(crate) fn tick_at(e: &mut Env, h: &mut ToyHost, t: f32) -> TickReport {
    let dt = t - e.vm.clock.time_seconds();
    e.vm.clock.advance(dt);
    tick_mind(&mut e.vm, h, e.mind, e.globals).unwrap()
}

#[test]
fn the_settings_come_from_the_definitions() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let mut e = env(mode);
        let s = settings(&mut e.vm, e.mind, e.globals).unwrap();
        assert_eq!((s.update_rate, s.search_radius, s.close_threat_radius), (0.25, 4000.0, 700.0));
        assert_eq!((s.collision_radius, s.min_aggro_time, s.sight_view_cone), (Some(90.0), 0.1, 0.0));
        assert_eq!((s.seen_time, s.provoked_time), (2.5, 10.0), "not a den definition: the read defaults");
        assert_eq!(s.knowledge.len(), 5);
        assert_eq!(s.knowledge[4], Knowledge::Empty);
        assert_eq!(s.prioritization[2], (Prioritizer::Previous, 0.5));
        // A component whose definitions the AI start did not bind: NotYet.
        set(&mut e.vm, e.component, "AIDef", Value::Object(None)).unwrap();
        let err = settings(&mut e.vm, e.mind, e.globals).unwrap_err();
        assert!(matches!(err, VmError::NotYet(n) if n.missing == Missing::NotRead), "{mode:?}");
        set(&mut e.vm, e.component, "AIDef", Value::Object(Some(e.aidef))).unwrap();
    }
}

/// The sight rule on the VM: the target is picked one update after the exposure, and the script-visible fields are
/// written: the component's target and times, the mind's flag and location, the player's counter.
#[test]
fn a_tick_writes_what_script_reads() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let mut e = env(mode);
        let mut h = host(&e, 1.0);
        assert!(!tick_at(&mut e, &mut h, 0.125).target_update, "{mode:?}: before UpdateRate");
        let first = tick_at(&mut e, &mut h, 0.25);
        assert_eq!(first.added, vec![e.player], "the AI's own pawn is not a target");
        assert_eq!(get(&mut e.vm, e.component, "CurrentTarget").unwrap(), Some(Value::Object(None)));
        tick_at(&mut e, &mut h, 0.5);
        assert_eq!(get(&mut e.vm, e.component, "CurrentTarget").unwrap(), Some(Value::Object(Some(e.player))));
        assert_eq!(get(&mut e.vm, e.component, "LastStateUpdateTime").unwrap(), Some(Value::Float(0.5)));
        assert_eq!(get(&mut e.vm, e.mind, "bProvoked").unwrap(), Some(Value::Bool(true)));
        assert_eq!(get(&mut e.vm, e.player, "NumAICurrentlyTargeting").unwrap(), Some(Value::Int(1)));
        let s = state(&e.vm, e.mind).unwrap();
        assert_eq!(s.record(e.player).unwrap().threat, ThreatLevel::SeenInThreatRadius);
        // Script clears bProvoked (Action_Patrol.SetMoveNode): read back before the next tick.
        set(&mut e.vm, e.mind, "bProvoked", Value::Bool(false)).unwrap();
        set(&mut e.vm, e.ai, "Location", vec3(e.vector, [100.0, 0.0, 0.0])).unwrap();
        tick_at(&mut e, &mut h, 0.75);
        assert!(!state(&e.vm, e.mind).unwrap().provoked);
    }
}

/// `WillowMind.NotifyAttackedBy` as a native: without the host it is `NotYet`; with it the attacker is a level-3
/// target and the component's `Last*` fields are written.
#[test]
fn the_attacked_native_provokes() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let mut e = env(mode);
        let mind_class = e.vm.space.heap.get(e.mind).unwrap().class;
        let mut d = FunctionDecl::native(ObjectPath::parse(NOTIFY_ATTACKED_BY).unwrap(), NOTIFY_ATTACKED_BY).with_owner(mind_class);
        d.params = vec![
            decl(&mut e.vm, "Attacker", PropKind::Object { class: mind_class }).with_flags(PropertyDecl::PARAM),
            decl(&mut e.vm, "bDamaged", PropKind::Bool).with_flags(PropertyDecl::PARAM),
        ];
        let f = e.vm.declare_function(d).unwrap();
        Pack.register(&mut e.vm).unwrap();
        set(&mut e.vm, e.player, "Location", vec3(e.vector, [-9000.0, 0.0, 0.0])).unwrap();
        let args = vec![Value::Object(Some(e.player)), Value::Bool(true)];
        let err = e.vm.call(f, Some(e.mind), args.clone()).unwrap_err();
        assert!(matches!(err, VmError::NotYet(n) if matches!(n.missing, Missing::LayerAbsent(_))), "{mode:?}");
        let h: Rc<RefCell<dyn Host>> = Rc::new(RefCell::new(host(&e, 0.0)));
        install(&mut e.vm, h, e.globals).unwrap();
        e.vm.clock.advance(2.0);
        e.vm.call(f, Some(e.mind), args).unwrap();
        let s = state(&e.vm, e.mind).unwrap();
        let r = s.record(e.player).unwrap();
        assert_eq!((r.threat, r.aggro_time, r.forget_time), (ThreatLevel::Provoked, 2.0, 12.0));
        assert_eq!(get(&mut e.vm, e.component, "LastAttacker").unwrap(), Some(Value::Object(Some(e.player))));
        assert_eq!(get(&mut e.vm, e.component, "LastHurtTime").unwrap(), Some(Value::Float(2.0)));
        // Aware 0.1 s after the stamp, not before.
        assert!(!threat::is_aware(&s, 2.05, e.player).unwrap());
        assert!(threat::is_aware(&s, 2.2, e.player).unwrap());
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Review r1 of PR #357
// ---------------------------------------------------------------------------------------------------------------

/// A tick whose settings read fails (`TargetingDef` swapped to None) keeps the held state: the records, the target
/// and their times are all there for the next tick.
#[test]
fn a_failed_settings_read_keeps_the_state() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let mut e = env(mode);
        let mut h = host(&e, 1.0);
        tick_at(&mut e, &mut h, 0.25);
        tick_at(&mut e, &mut h, 0.5);
        let before = state(&e.vm, e.mind).unwrap();
        assert_eq!(before.current, Some(e.player));
        set(&mut e.vm, e.component, "TargetingDef", Value::Object(None)).unwrap();
        e.vm.clock.advance(0.25);
        let err = tick_mind(&mut e.vm, &mut h, e.mind, e.globals).unwrap_err();
        assert!(matches!(err, VmError::NotYet(_)), "{mode:?}");
        assert_eq!(state(&e.vm, e.mind).as_ref(), Some(&before), "{mode:?}: nothing lost");
        set(&mut e.vm, e.component, "TargetingDef", Value::Object(Some(e.targeting))).unwrap();
        tick_at(&mut e, &mut h, 1.0);
        assert_eq!(state(&e.vm, e.mind).unwrap().current, Some(e.player));
    }
}

/// G26 step 2 in the adapter: a controller is tested as its pawn (`NotYet` without one); a pawn whose controller has
/// a pawn is tested as that pawn. And the idle counter of an actor without one is 1.
#[test]
fn the_adapter_tests_a_controller_as_its_pawn() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let mut e = env(mode);
        let mut h = host(&e, 0.0);
        let controller = make(&mut e.vm, e.controller_class, "ToyPlayerController_0");
        let mut senses = VmSenses { vm: &mut e.vm, host: &mut h };
        assert_eq!(senses.tested_actor(controller).unwrap(), controller, "{mode:?}: no pawn: tested as itself");
        set(senses.vm, controller, "Pawn", Value::Object(Some(e.player))).unwrap();
        assert_eq!(senses.tested_actor(controller).unwrap(), e.player);
        assert_eq!(senses.tested_actor(e.ai).unwrap(), e.ai, "the mind's pawn is the AI pawn");
        assert_eq!(senses.tested_actor(e.player).unwrap(), e.player, "a controller with no Pawn property");
        let s = settings(senses.vm, e.mind, e.globals).unwrap();
        let c = Component::new(e.ai, s);
        assert!(super::super::targets::valid_target(&c, &mut senses, Some(controller)).unwrap());
        assert_eq!(senses.targeted_by(controller).unwrap(), 1, "no counter: 1");
        assert_eq!(senses.targeted_by(e.player).unwrap(), 0);
    }
}

/// `GetThreatLevel` on a component the adapter has not ticked: built from its settings, so a Friendly target answers
/// `ETL_SeenInThreatRadius` and an enemy `ETL_NoThreat` (no record), each as the installed enum's byte.
#[test]
fn get_threat_level_without_state_keeps_the_friendly_rule() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let mut e = env(mode);
        let component_class = e.vm.space.heap.get(e.component).unwrap().class;
        let mut d = FunctionDecl::native(ObjectPath::parse(GET_THREAT).unwrap(), GET_THREAT).with_owner(component_class);
        d.params = vec![decl(&mut e.vm, "Target", PropKind::Object { class: component_class }).with_flags(PropertyDecl::PARAM)];
        let f = e.vm.declare_function(d).unwrap();
        Pack.register(&mut e.vm).unwrap();
        let h: Rc<RefCell<dyn Host>> = Rc::new(RefCell::new(host(&e, 0.0)));
        install(&mut e.vm, h, e.globals).unwrap();
        set(&mut e.vm, e.component, "MyWillowMind", Value::Object(Some(e.mind))).unwrap();
        assert!(state(&e.vm, e.mind).is_none());
        let enemy = e.vm.call(f, Some(e.component), vec![Value::Object(Some(e.player))]).unwrap().ret;
        assert_eq!(enemy, Value::Byte(1), "{mode:?}: ETL_NoThreat in the toy order");
        let beasts = get(&mut e.vm, e.ai, "Allegiance").unwrap().unwrap();
        set(&mut e.vm, e.player, "Allegiance", beasts).unwrap();
        let friend = e.vm.call(f, Some(e.component), vec![Value::Object(Some(e.player))]).unwrap().ret;
        assert_eq!(friend, Value::Byte(0), "{mode:?}: ETL_SeenInThreatRadius in the toy order");
    }
}
