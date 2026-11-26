//! NATIVE_BOUNDING_CYLINDER_AND_AFTER.md section 9, tests 2 to 9, on invented classes (AGENTS.md rule 9: the class
//! and field names are the packages'; every value is invented). AI-assisted (Claude Code).

use std::rc::Rc;

use ow_attr::AttributeSystem;
use ow_core::{Missing, ParityMode};
use ow_defs::DefStore;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ClassId, ObjRef, PropAddr, StructId};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use super::*;

struct Fx {
    vm: Vm,
    reference: StructId,
    shake: StructId,
}

fn decl(vm: &mut Vm, name: &str, kind: PropKind) -> PropertyDecl {
    PropertyDecl::new(vm.space.names.intern(name), kind)
}

fn fx_in(mode: ParityMode) -> Fx {
    let mut space = ObjectSpace::new();
    let object = space.classes.define("Core.Object", None, vec![]).unwrap();
    let mut vm = Vm::new(space, DefStore::new_empty(), Rc::new(AttributeSystem::new_empty()), mode);
    let o = PropKind::Object { class: object };
    let byte = PropKind::Byte { enum_: None };
    let props = vec![
        decl(&mut vm, "PoolManager", o.clone()),
        decl(&mut vm, "PoolIndexInManager", byte.clone()),
        decl(&mut vm, "PoolGUID", byte.clone()),
        decl(&mut vm, "Data", o.clone()),
    ];
    let reference = vm.space.classes.define_struct("Engine.Actor.ResourcePoolReference", None, props).unwrap();
    let props = vec![decl(&mut vm, "Owner", o.clone()), decl(&mut vm, "Base", o.clone())];
    let actor = vm.space.classes.define("Engine.Actor", Some(object), props).unwrap();
    let props = vec![decl(&mut vm, "bAttached", PropKind::Bool), decl(&mut vm, "Owner", o.clone())];
    vm.space.classes.define("Engine.PrimitiveComponent", Some(object), props).unwrap();
    let props = vec![
        decl(&mut vm, "HealthPool", PropKind::Struct(reference)),
        decl(&mut vm, "HealthVar", PropKind::Float),
        decl(&mut vm, "DrivenVehicle", o.clone()),
    ];
    let pawn = vm.space.classes.define("Engine.Pawn", Some(actor), props).unwrap();
    let props = vec![
        decl(&mut vm, "bGibbed", PropKind::Bool),
        decl(&mut vm, "bDidServerGibPawn", PropKind::Bool),
        decl(&mut vm, "MyDeathDef", o.clone()),
        decl(&mut vm, "ShieldArmor", PropKind::Struct(reference)),
        decl(&mut vm, "ShieldVar", PropKind::Int),
        decl(&mut vm, "ShieldMaxVar", PropKind::Int),
        decl(&mut vm, "BodyClass", o.clone()),
    ];
    let willow = vm.space.classes.define("WillowGame.WillowPawn", Some(pawn), props).unwrap();
    let props = vec![decl(&mut vm, "SpawnParent", o.clone())];
    vm.space.classes.define("WillowGame.WillowAIPawn", Some(willow), props).unwrap();
    let props = vec![decl(&mut vm, "bOnlyDisplayParentInfoWhileAttached", PropKind::Bool)];
    vm.space.classes.define("WillowGame.BodyClassDefinition", Some(object), props).unwrap();
    let props = vec![decl(&mut vm, "ResourcePools", o.clone()).with_array_dim(i32::from(ow_world::pools::POOL_SLOTS))];
    vm.space.classes.define("Engine.ResourcePoolManager", Some(object), props).unwrap();
    let props = vec![
        decl(&mut vm, "PoolGUID", byte),
        decl(&mut vm, "CurrentValue", PropKind::Float),
        decl(&mut vm, "MaxValue", PropKind::Float),
    ];
    vm.space.classes.define("Engine.ResourcePool", Some(object), props).unwrap();
    let floats: Vec<_> = ["OffsetTime", "RotTime", "FOVTime"].iter().map(|n| decl(&mut vm, n, PropKind::Float)).collect();
    let shake = vm.space.classes.define_struct("GearboxFramework.GearboxPlayerController.GearboxViewShakeInfo", None, floats).unwrap();
    let props = vec![decl(&mut vm, "bUsingGamepad", PropKind::Bool)];
    vm.space.classes.define("Engine.PlayerInput", Some(object), props).unwrap();
    let props = vec![
        decl(&mut vm, "PlayerInput", o.clone()),
        decl(&mut vm, "ForceFeedbackManager", o.clone()),
        decl(&mut vm, "PlayerReplicationInfo", o.clone()),
    ];
    let pc = vm.space.classes.define("Engine.PlayerController", Some(actor), props).unwrap();
    let props = vec![
        decl(&mut vm, "CurrentViewShake", PropKind::Struct(shake)),
        decl(&mut vm, "ShakeOffsetLength", PropKind::Float),
        decl(&mut vm, "ShakeRotLength", PropKind::Float),
        decl(&mut vm, "ShakeFOVLength", PropKind::Float),
    ];
    vm.space.classes.define("WillowGame.WillowPlayerController", Some(pc), props).unwrap();
    let props = vec![decl(&mut vm, "Scale", PropKind::Float)];
    vm.space.classes.define("Test.ToyWaveform", Some(object), props).unwrap();
    let props = vec![decl(&mut vm, "GlobalCombatMusicManager", o.clone())];
    vm.space.classes.define("WillowGame.WillowGameInfo", Some(actor), props).unwrap();
    ow_world::actors::add_empty_scene(&mut vm).unwrap();
    Fx { vm, reference, shake }
}

fn fx() -> Fx {
    fx_in(ParityMode::Off)
}

impl Fx {
    fn class(&self, path: &str) -> ClassId {
        self.vm.space.classes.by_path(path).unwrap()
    }

    fn make(&mut self, path: &str) -> ObjRef {
        let c = self.class(path);
        let o = self.vm.space.alloc(c, "Toy", None, Origin::Runtime).unwrap();
        // An invalid pool reference where the class holds one (index and GUID 255).
        for field in ["HealthPool", "ShieldArmor"] {
            if self.vm.space.prop(c, field).is_some() {
                let v = ow_world::pools::ref_value(&self.vm.space, self.reference, ow_world::pools::PoolRef::INVALID).unwrap();
                self.set(o, field, v);
            }
        }
        o
    }

    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap();
        self.vm.space.write(o, PropAddr::scalar(p), v).unwrap();
    }

    fn get(&self, o: ObjRef, name: &str) -> Value {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap();
        self.vm.space.read(o, PropAddr::scalar(p)).unwrap().clone()
    }

    fn marks(&self) -> (u64, u64, u64, u64, usize) {
        let vm = &self.vm;
        (vm.space.heap.committed_writes(), vm.rng.draws(), vm.defs.load_transitions(), vm.class_code_loads(), vm.space.names.len())
    }
}

fn not_read(e: VmError) -> String {
    match e {
        VmError::NotYet(n) => {
            assert_eq!(n.missing, Missing::NotRead, "{n}");
            n.what.to_string()
        }
        other => panic!("expected NotYet, got {other:?}"),
    }
}

/// Test 2: `TraceComponent`.
#[test]
fn trace_component_s_cases() {
    let mut x = fx();
    let actor = x.make("Engine.Actor");
    let c = x.make("Engine.PrimitiveComponent");
    x.set(c, "Owner", Value::Object(Some(actor)));
    assert!(!trace_answer(&mut x.vm, actor, None).unwrap(), "no component");
    assert!(!trace_answer(&mut x.vm, actor, Some(c)).unwrap(), "bAttached clear");
    x.set(c, "bAttached", Value::Bool(true));
    match trace_answer(&mut x.vm, actor, Some(c)).unwrap_err() {
        VmError::NotYet(n) => assert_eq!(n.what.as_ref(), ow_world::world::bounding_cylinder::REFUSAL_L, "unknown: refusal L"),
        other => panic!("{other:?}"),
    }
    ow_world::actors::publish_attach(&mut x.vm, c, actor).unwrap();
    let other = x.make("Engine.Actor");
    let before = x.marks();
    // Review of #804, item 1: recorded under another owner is refusal L, as GetBoundingCylinder answers it.
    match trace_answer(&mut x.vm, other, Some(c)).unwrap_err() {
        VmError::NotYet(n) => assert_eq!(n.what.as_ref(), ow_world::world::bounding_cylinder::REFUSAL_L, "another owner"),
        other => panic!("{other:?}"),
    }
    match trace_answer(&mut x.vm, actor, Some(c)).unwrap_err() {
        VmError::NotYet(n) => {
            assert!(matches!(n.missing, Missing::LayerAbsent(_)), "{n}");
            assert_eq!(n.what.as_ref(), TRACE_LINE_CHECK);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(x.marks(), before, "write-free");
    // The hub's ruling: with the setting on, the attached component is a logged miss.
    enable_trace_component_miss(&mut x.vm).unwrap();
    let logged = x.vm.log.len();
    assert!(!trace_answer(&mut x.vm, actor, Some(c)).unwrap());
    assert_eq!(x.vm.log[logged..], [TRACE_MISS_LINE.to_string()]);
    assert!(!trace_answer(&mut x.vm, actor, None).unwrap(), "unchanged without a component");
    assert_eq!(x.vm.log.len(), logged + 1, "only the attached case logs");
    // Review of #804, item 1: a stale entry (an Owner write after publication, even of the same owner) is refusal L,
    // also with the miss setting on, and logs nothing.
    x.set(c, "Owner", Value::Object(Some(actor)));
    match trace_answer(&mut x.vm, actor, Some(c)).unwrap_err() {
        VmError::NotYet(n) => assert_eq!(n.what.as_ref(), ow_world::world::bounding_cylinder::REFUSAL_L, "stale"),
        other => panic!("{other:?}"),
    }
    assert_eq!(x.vm.log.len(), logged + 1, "the stale refusal logs nothing");
    let mut r = fx_in(ParityMode::Reference);
    assert!(matches!(enable_trace_component_miss(&mut r.vm), Err(VmError::Registration { .. })));
}

/// Test 3: `TryFullBodyGib`.
#[test]
fn try_full_body_gib_s_cases() {
    let mut x = fx();
    let pawn = x.make("WillowGame.WillowPawn");
    assert!(!try_full_body_gib(&x.vm, pawn).unwrap(), "no MyDeathDef: false");
    let death = x.make("Core.Object");
    x.set(pawn, "MyDeathDef", Value::Object(Some(death)));
    x.set(pawn, "HealthVar", Value::Float(50.0));
    assert!(!try_full_body_gib(&x.vm, pawn).unwrap(), "alive: false");
    x.set(pawn, "HealthVar", Value::Float(0.0));
    assert_eq!(not_read(try_full_body_gib(&x.vm, pawn).unwrap_err()), GIB_TEST, "dead: the gib test");
    x.set(pawn, "bDidServerGibPawn", Value::Bool(true));
    assert_eq!(not_read(try_full_body_gib(&x.vm, pawn).unwrap_err()), THE_GIB);
    x.set(pawn, "bGibbed", Value::Bool(true));
    assert!(try_full_body_gib(&x.vm, pawn).unwrap(), "gibbed: true");
}

/// Test 4: `GetParent`.
#[test]
fn get_parent_s_cases() {
    let mut x = fx();
    let pawn = x.make("WillowGame.WillowPawn");
    assert_eq!(get_parent(&x.vm, pawn).unwrap(), None);
    let owner = x.make("WillowGame.WillowPawn");
    x.set(pawn, "Owner", Value::Object(Some(owner)));
    assert_eq!(get_parent(&x.vm, pawn).unwrap(), Some(owner), "a WillowPawn owner");
    let thing = x.make("Engine.Actor");
    x.set(pawn, "Owner", Value::Object(Some(thing)));
    assert_eq!(get_parent(&x.vm, pawn).unwrap(), None, "a non-pawn owner");
    let ai = x.make("WillowGame.WillowAIPawn");
    let parent = x.make("WillowGame.WillowPawn");
    x.set(ai, "SpawnParent", Value::Object(Some(parent)));
    assert_eq!(get_parent(&x.vm, ai).unwrap(), Some(parent), "SpawnParent");
    x.set(ai, "SpawnParent", Value::Object(None));
    let body = x.make("WillowGame.BodyClassDefinition");
    x.set(body, "bOnlyDisplayParentInfoWhileAttached", Value::Bool(true));
    x.set(ai, "BodyClass", Value::Object(Some(body)));
    let base = x.make("WillowGame.WillowPawn");
    x.set(ai, "Base", Value::Object(Some(base)));
    assert_eq!(get_parent(&x.vm, ai).unwrap(), Some(base), "the body flag with a pawn Base");
    let car = x.make("Engine.Pawn");
    x.set(pawn, "DrivenVehicle", Value::Object(Some(car)));
    assert_eq!(not_read(get_parent(&x.vm, pawn).unwrap_err()), PARENT_VEHICLE);
}

/// Test 5: the shield getters: an invalid reference, the two plain fields as floats; a valid pool, its values.
#[test]
fn the_shield_getters_read_the_pool_or_the_plain_fields() {
    let mut x = fx();
    let pawn = x.make("WillowGame.WillowPawn");
    x.set(pawn, "ShieldVar", Value::Int(7));
    x.set(pawn, "ShieldMaxVar", Value::Int(40));
    assert_eq!(shield(&x.vm, pawn).unwrap(), (7.0, 40.0));
    let manager = x.make("Engine.ResourcePoolManager");
    let pool = x.make("Engine.ResourcePool");
    x.set(pool, "PoolGUID", Value::Byte(3));
    x.set(pool, "CurrentValue", Value::Float(12.0));
    x.set(pool, "MaxValue", Value::Float(90.0));
    x.set(manager, "ResourcePools", Value::Object(Some(pool)));
    let r = ow_world::pools::PoolRef::new(Some(manager), 0, 3, None);
    let v = ow_world::pools::ref_value(&x.vm.space, x.reference, r).unwrap();
    x.set(pawn, "ShieldArmor", v);
    let before = x.marks();
    assert_eq!(shield(&x.vm, pawn).unwrap(), (12.0, 90.0));
    assert_eq!(x.marks(), before, "write-free");
    // Review of #804, item 2: a manager the store has not loaded is the NotRead, before the resolve reads it.
    x.vm.space.heap.get_mut(manager).unwrap().slots.clear();
    let text = not_read(shield(&x.vm, pawn).unwrap_err());
    assert!(text.contains("ResourcePoolManager") && text.contains("not loaded"), "{text}");
}

/// Test 6: `ShakeView` copies the struct and sets the three lengths; another receiver is the `NotRead`.
#[test]
fn shake_view_copies_the_info() {
    let mut x = fx();
    let pc = x.make("WillowGame.WillowPlayerController");
    let info = Value::structure(x.shake, vec![Value::Float(0.25), Value::Float(0.5), Value::Float(0.75)]);
    shake_view(&mut x.vm, pc, &info).unwrap();
    assert_eq!(x.get(pc, "CurrentViewShake"), info);
    assert_eq!(x.get(pc, "ShakeOffsetLength"), Value::Float(0.25));
    assert_eq!(x.get(pc, "ShakeRotLength"), Value::Float(0.5));
    assert_eq!(x.get(pc, "ShakeFOVLength"), Value::Float(0.75));
    let plain = x.make("Engine.PlayerController");
    assert_eq!(not_read(shake_view(&mut x.vm, plain, &info).unwrap_err()), SHAKE_OTHER);
}

/// Test 7: `DoForceFeedbackWaveform`'s gates; with a manager the `NotRead`, nothing written.
#[test]
fn force_feedback_s_gates() {
    let mut x = fx();
    let pc = x.make("Engine.PlayerController");
    let input = x.make("Engine.PlayerInput");
    x.set(pc, "PlayerInput", Value::Object(Some(input)));
    let manager = x.make("Core.Object");
    let info = x.make("Core.Object");
    x.set(pc, "ForceFeedbackManager", Value::Object(Some(manager)));
    x.set(pc, "PlayerReplicationInfo", Value::Object(Some(info)));
    let before = x.marks();
    force_feedback(&x.vm, pc, None).unwrap();
    assert_eq!(x.marks(), before, "no gamepad: nothing");
    x.set(input, "bUsingGamepad", Value::Bool(true));
    x.set(pc, "ForceFeedbackManager", Value::Object(None));
    force_feedback(&x.vm, pc, None).unwrap();
    x.set(pc, "ForceFeedbackManager", Value::Object(Some(manager)));
    x.set(pc, "PlayerReplicationInfo", Value::Object(None));
    force_feedback(&x.vm, pc, None).unwrap();
    x.set(pc, "PlayerReplicationInfo", Value::Object(Some(info)));
    let waveform = x.make("Test.ToyWaveform");
    let before = x.marks();
    assert_eq!(not_read(force_feedback(&x.vm, pc, Some(waveform)).unwrap_err()), FEEDBACK_PLAY);
    assert_eq!(x.marks(), before, "nothing written: decided before the Scale write");
    assert_eq!(x.get(waveform, "Scale"), Value::Float(0.0));
    let bare = x.make("Core.Object");
    assert!(matches!(force_feedback(&x.vm, pc, Some(bare)), Err(VmError::Script { .. })), "a waveform with no Scale");
}

/// Test 8: `GetCombatMusicManager`.
#[test]
fn combat_music_manager_s_cases() {
    let mut x = fx();
    let game = x.make("WillowGame.WillowGameInfo");
    let before = x.marks();
    assert_eq!(not_read(combat_music_manager(&x.vm, game).unwrap_err()), MUSIC_LAZY);
    assert_eq!(x.marks(), before);
    let manager = x.make("Core.Object");
    x.set(game, "GlobalCombatMusicManager", Value::Object(Some(manager)));
    assert_eq!(combat_music_manager(&x.vm, game).unwrap(), manager);
}

/// Test 9: the declarations: a receiver whose class lacks the read fields is a data error.
#[test]
fn undeclared_fields_are_script_errors() {
    let mut x = fx();
    let thing = x.make("Engine.Actor");
    assert!(matches!(try_full_body_gib(&x.vm, thing), Err(VmError::Script { .. })));
    assert!(matches!(shield(&x.vm, thing), Err(VmError::Script { .. })));
    assert!(matches!(force_feedback(&x.vm, thing, None), Err(VmError::Script { .. })));
    assert!(matches!(combat_music_manager(&x.vm, thing), Err(VmError::Script { .. })));
}
