//! NATIVE_GETPHYSXLEVEL_AND_AFTER.md section 5's tests 1 to 5 and 8, on invented classes (AGENTS.md rule 9: invented
//! objects and values, the class and field names the packages'). Tests 6 and 7 (the installed
//! `SetupSkeletalMeshPhysicsForPhysXEffects`' "greater than 0" gate) are script; the installed slice gate runs it.

use std::rc::Rc;

use ow_attr::AttributeSystem;
use ow_core::notyet::Missing;
use ow_core::{ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ClassId, FuncId, ObjRef, PropAddr};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_vm::api::{FunctionDecl, Vm};
use ow_vm::error::VmError;

use super::*;

struct Fx {
    vm: Vm,
    level: FuncId,
    set_has: FuncId,
    force_field: FuncId,
    skeletal: ClassId,
}

fn decl(vm: &mut Vm, name: &str, kind: PropKind) -> PropertyDecl {
    PropertyDecl::new(vm.space.names.intern(name), kind)
}

fn fx_in(mode: ParityMode) -> Fx {
    let mut space = ObjectSpace::new();
    let object = space.classes.define("Core.Object", None, vec![]).unwrap();
    let mut vm = Vm::new(space, DefStore::new_empty(), Rc::new(AttributeSystem::new_empty()), mode);
    let props = vec![decl(&mut vm, "PhysXLevel", PropKind::Int)];
    let engine = vm.space.classes.define("Engine.Engine", Some(object), props).unwrap();
    vm.space.classes.define("Test.EngineWithNoLevel", Some(object), vec![]).unwrap();
    let props = vec![decl(&mut vm, "PhysXLevel", PropKind::Float)];
    vm.space.classes.define("Test.EngineWithAFloatLevel", Some(object), props).unwrap();
    let props = vec![decl(&mut vm, "bHasPhysicsAssetInstance", PropKind::Bool), decl(&mut vm, "bSkelCompFixed", PropKind::Bool)];
    let skeletal = vm.space.classes.define("Engine.SkeletalMeshComponent", Some(object), props).unwrap();
    let field = vm.space.classes.define("Engine.NxForceFieldComponent", Some(object), vec![]).unwrap();
    let declare = |vm: &mut Vm, path: &str, key: &str, owner: ClassId, params: Vec<PropertyDecl>, result: Option<PropertyDecl>| {
        let mut d = FunctionDecl::native(ObjectPath::parse(path).unwrap(), key).with_owner(owner).with_params(params);
        d.result = result;
        vm.declare_function(d).unwrap()
    };
    let ret = decl(&mut vm, "ReturnValue", PropKind::Int);
    let level = declare(&mut vm, "Engine.Engine.GetPhysXLevel", GET_PHYSX_LEVEL, engine, vec![], Some(ret));
    let params = vec![
        decl(&mut vm, "bHasInstance", PropKind::Bool).with_flags(PropertyDecl::PARAM),
        decl(&mut vm, "bUseCurrentPosition", PropKind::Bool).with_flags(PropertyDecl::PARAM),
    ];
    let set_has = declare(
        &mut vm,
        "Engine.SkeletalMeshComponent.SetHasPhysicsAssetInstance",
        SET_HAS_PHYSICS_ASSET_INSTANCE,
        skeletal,
        params,
        None,
    );
    let force_field = declare(&mut vm, "Engine.NxForceFieldComponent.DoInitRBPhys", DO_INIT_RB_PHYS, field, vec![], None);
    if mode != ParityMode::Reference {
        register(&mut vm).unwrap();
    }
    Fx { vm, level, set_has, force_field, skeletal }
}

fn fx() -> Fx {
    fx_in(ParityMode::Off)
}

impl Fx {
    fn make(&mut self, path: &str) -> ObjRef {
        let class = self.vm.space.classes.by_path(path).unwrap();
        self.vm.space.alloc(class, "Toy", None, Origin::Runtime).unwrap()
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

    /// An engine object of the class at `path`, registered as the world's.
    fn engine(&mut self, path: &str) -> ObjRef {
        let e = self.make(path);
        self.vm.add_service(EngineObject(e)).unwrap();
        e
    }

    fn marks(&self) -> (u64, u64, u64, u64, usize) {
        let vm = &self.vm;
        (vm.space.heap.committed_writes(), vm.rng.draws(), vm.defs.load_transitions(), vm.class_code_loads(), vm.space.names.len())
    }

    /// Calls `f` and checks nothing was written, drawn, loaded or interned.
    fn call(&mut self, f: FuncId, this: Option<ObjRef>, args: Vec<Value>) -> Result<Value, VmError> {
        let before = self.marks();
        let out = self.vm.call(f, this, args).map(|r| r.ret);
        assert_eq!(self.marks(), before, "nothing written, drawn, loaded or interned");
        out
    }

    fn level(&mut self) -> Result<Value, VmError> {
        let f = self.level;
        self.call(f, None, vec![])
    }

    fn set_has(&mut self, c: ObjRef, want: bool) -> Result<Value, VmError> {
        let f = self.set_has;
        self.call(f, Some(c), vec![Value::Bool(want), Value::Bool(false)])
    }
}

fn not_yet(r: Result<Value, VmError>) -> (Missing, String) {
    match r {
        Err(VmError::NotYet(n)) => {
            assert_eq!(n.record, RECORD);
            (n.missing, n.what.to_string())
        }
        other => panic!("expected NotYet, got {other:?}"),
    }
}

fn script_error(r: Result<Value, VmError>, has: &str) {
    match r {
        Err(VmError::Script { message, .. }) => assert!(message.contains(has), "{message}"),
        other => panic!("expected a data error naming {has:?}, got {other:?}"),
    }
}

/// Tests 1 and 2: the engine object's `PhysXLevel` as stored, 0 then 2; nothing written.
#[test]
fn get_physx_level_reads_the_engine_object() {
    let mut x = fx();
    let e = x.engine("Engine.Engine");
    assert_eq!(x.level().unwrap(), Value::Int(0));
    x.set(e, "PhysXLevel", Value::Int(2));
    assert_eq!(x.level().unwrap(), Value::Int(2));
}

/// Test 3: no engine object is the `LayerAbsent` naming it.
#[test]
fn no_engine_object_is_layer_absent() {
    let mut x = fx();
    let (missing, what) = not_yet(x.level());
    assert!(matches!(missing, Missing::LayerAbsent(l) if l == ENGINE_LAYER), "{missing:?}");
    assert_eq!(what, NO_ENGINE);
}

/// Test 4: an engine class with no `PhysXLevel`, or with a float one, is a data error.
#[test]
fn an_undeclared_or_float_level_is_a_data_error() {
    let mut x = fx();
    x.engine("Test.EngineWithNoLevel");
    script_error(x.level(), "declares no PhysXLevel");
    let mut x = fx();
    x.engine("Test.EngineWithAFloatLevel");
    script_error(x.level(), "not an int");
}

/// Test 5: the harness setting at 1 writes the field and logs one harness line naming it; 3 (and -1) is refused with
/// nothing written; a world with no engine object sets nothing; Reference refuses; a float field refuses before the
/// write.
#[test]
fn the_harness_setting_writes_0_to_2_and_logs_it() {
    let mut x = fx();
    let e = x.engine("Engine.Engine");
    let lines = x.vm.log.len();
    assert!(set_physx_level(&mut x.vm, 1).unwrap());
    assert_eq!(x.get(e, "PhysXLevel"), Value::Int(1));
    let logged: Vec<&String> = x.vm.log[lines..].iter().collect();
    assert_eq!(logged.len(), 1);
    assert!(logged[0].starts_with("harness input: the engine object's PhysXLevel = 1 ("), "{}", logged[0]);
    assert!(logged[0].contains("UNVERIFIED: the player's setting; shipped default 0"), "{}", logged[0]);
    for bad in [3, -1] {
        let before = x.marks();
        assert!(matches!(set_physx_level(&mut x.vm, bad), Err(VmError::Registration { .. })), "{bad}");
        assert_eq!(x.marks(), before, "{bad}: nothing written");
        assert_eq!(x.get(e, "PhysXLevel"), Value::Int(1));
    }
    let mut x = fx();
    assert!(!set_physx_level(&mut x.vm, HARNESS_PHYSX_LEVEL).unwrap(), "no engine object: nothing set");
    let mut x = fx();
    x.engine("Test.EngineWithAFloatLevel");
    let before = x.marks();
    assert!(matches!(set_physx_level(&mut x.vm, 0), Err(VmError::Script { .. })));
    assert_eq!(x.marks(), before);
    let mut x = fx_in(ParityMode::Reference);
    assert!(matches!(set_physx_level(&mut x.vm, 0), Err(VmError::Registration { .. })));
}

/// Test 8: `SetHasPhysicsAssetInstance(true)` with the flag set writes nothing; with it clear, the physics
/// `LayerAbsent`, the flag still clear. True to false is the tear-down `NotRead`; an unloaded component the not-loaded
/// `NotRead`.
#[test]
fn set_has_physics_asset_instance_refuses_the_instance() {
    let mut x = fx();
    let c = x.make("Engine.SkeletalMeshComponent");
    let (missing, what) = not_yet(x.set_has(c, true));
    assert!(matches!(missing, Missing::LayerAbsent(l) if l == RIGID_LAYER), "{missing:?}");
    assert_eq!(what, ASSET_INSTANCE);
    assert_eq!(x.get(c, "bHasPhysicsAssetInstance"), Value::Bool(false));
    assert_eq!(x.set_has(c, false).unwrap(), Value::None, "clear and asked clear: nothing");
    x.set(c, "bHasPhysicsAssetInstance", Value::Bool(true));
    assert_eq!(x.set_has(c, true).unwrap(), Value::None, "set and asked set: nothing");
    assert_eq!(not_yet(x.set_has(c, false)), (Missing::NotRead, TEAR_DOWN.to_string()));
    let skeletal = x.skeletal;
    let lazy = x.vm.space.alloc_stand_in(skeletal, "Lazy", Origin::Default { class: skeletal }).unwrap();
    assert_eq!(not_yet(x.set_has(lazy, true)), (Missing::NotRead, NOT_LOADED.to_string()));
}

/// 2.C: `DoInitRBPhys` is the physics `LayerAbsent`, nothing written.
#[test]
fn do_init_rb_phys_is_the_physics_layer() {
    let mut x = fx();
    let c = x.make("Engine.NxForceFieldComponent");
    let f = x.force_field;
    let (missing, what) = not_yet(x.call(f, Some(c), vec![]));
    assert!(matches!(missing, Missing::LayerAbsent(l) if l == RIGID_LAYER), "{missing:?}");
    assert_eq!(what, FORCE_FIELD);
}
