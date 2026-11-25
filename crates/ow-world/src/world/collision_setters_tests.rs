//! 2.D to 2.H of NATIVE_LINK1_TAIL_SETTERS.md on invented classes (AGENTS.md rule 9: invented objects and values,
//! the class and field names the packages'). Every refusal leaves #518's counters, `class_code_loads` and the name
//! table unchanged.

use std::rc::Rc;

use ow_attr::AttributeSystem;
use ow_core::notyet::Missing;
use ow_core::{ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ClassId, FuncId, ObjRef, PropAddr, StructId};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_vm::api::{FunctionDecl, Vm};
use ow_vm::error::VmError;

use super::*;

struct Fx {
    vm: Vm,
    fns: [FuncId; 5],
    primitive: ClassId,
    skeletal: ClassId,
    actor: ClassId,
    scene: StructId,
}

fn decl(vm: &mut Vm, name: &str, kind: PropKind) -> PropertyDecl {
    PropertyDecl::new(vm.space.names.intern(name), kind)
}

fn fx() -> Fx {
    let mut space = ObjectSpace::new();
    let object = space.classes.define("Core.Object", None, vec![]).unwrap();
    let mut vm = Vm::new(space, DefStore::new_empty(), Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
    let o = PropKind::Object { class: object };
    let flags = vec![
        decl(&mut vm, "bStatic", PropKind::Bool),
        decl(&mut vm, "bTickIsDisabled", PropKind::Bool),
        decl(&mut vm, "bDeleteMe", PropKind::Bool),
    ];
    let actor = vm.space.classes.define("Engine.Actor", Some(object), flags).unwrap();
    let dummy = vec![decl(&mut vm, "Dummy", PropKind::Int)];
    let scene = vm.space.classes.define_struct("Core.Object.Pointer", None, dummy).unwrap();
    let mut props = vec![
        decl(&mut vm, "RBChannel", PropKind::Byte { enum_: None }),
        decl(&mut vm, "BodyInstance", o.clone()),
        decl(&mut vm, "Owner", PropKind::Object { class: actor }),
        decl(&mut vm, "Scene", PropKind::Struct(scene)),
    ];
    for b in [
        "BlockRigidBody",
        "bNotifyRigidBodyCollision",
        "BlockZeroExtent",
        "BlockNonZeroExtent",
        "CollideActors",
        "BlockActors",
        "AlwaysCheckCollision",
        "bNeedsReattach",
    ] {
        props.push(decl(&mut vm, b, PropKind::Bool));
    }
    let primitive = vm.space.classes.define("Engine.PrimitiveComponent", Some(object), props).unwrap();
    let assets = vec![decl(&mut vm, "PhysicsAssetInstance", o)];
    let skeletal = vm.space.classes.define("Engine.SkeletalMeshComponent", Some(primitive), assets).unwrap();
    let mut fns = Vec::new();
    for (name, key, params) in [
        ("SetRBChannel", SET_RB_CHANNEL, vec![("Channel", PropKind::Byte { enum_: None }, false)]),
        ("SetBlockRigidBody", SET_BLOCK_RIGID_BODY, vec![("b", PropKind::Bool, false)]),
        (
            "SetActorCollision",
            SET_ACTOR_COLLISION,
            vec![("C", PropKind::Bool, false), ("B", PropKind::Bool, false), ("A", PropKind::Bool, true)],
        ),
        ("SetTraceBlocking", SET_TRACE_BLOCKING, vec![("Z", PropKind::Bool, false), ("NZ", PropKind::Bool, false)]),
        ("SetNotifyRigidBodyCollision", SET_NOTIFY_RIGID_BODY_COLLISION, vec![("b", PropKind::Bool, false)]),
    ] {
        let params = params
            .into_iter()
            .map(|(n, k, opt)| {
                let f = PropertyDecl::PARAM | if opt { PropertyDecl::OPTIONAL } else { 0 };
                decl(&mut vm, n, k).with_flags(f)
            })
            .collect();
        let path = ObjectPath::parse(&format!("Engine.PrimitiveComponent.{name}")).unwrap();
        fns.push(vm.declare_function(FunctionDecl::native(path, key).with_owner(primitive).with_params(params)).unwrap());
    }
    register(&mut vm).unwrap();
    Fx { vm, fns: fns.try_into().unwrap(), primitive, skeletal, actor, scene }
}

const RB: usize = 0;
const BLOCK_RB: usize = 1;
const ACTOR: usize = 2;
const TRACE: usize = 3;
const NOTIFY: usize = 4;

impl Fx {
    fn make(&mut self, class: ClassId, name: &str) -> ObjRef {
        self.vm.space.alloc(class, name, None, Origin::Runtime).unwrap()
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

    fn call(&mut self, which: usize, o: ObjRef, args: Vec<Value>) -> Result<(), VmError> {
        let f = self.fns[which];
        self.vm.call(f, Some(o), args).map(|_| ())
    }

    fn refusal(&mut self, which: usize, o: ObjRef, args: Vec<Value>) -> ow_core::NotYet {
        let before = self.marks();
        let e = self.call(which, o, args).unwrap_err();
        assert_eq!(self.marks(), before, "a refusal writes nothing");
        match e {
            VmError::NotYet(n) => n,
            other => panic!("expected NotYet, got {other:?}"),
        }
    }

    /// Every field of `o` (for "no other field changes").
    fn snapshot(&self, o: ObjRef) -> Vec<Value> {
        self.vm.space.heap.get(o).unwrap().slots.clone()
    }

    fn bools(&self, o: ObjRef, names: &[&str]) -> Vec<bool> {
        names.iter().map(|n| self.get(o, n) == Value::Bool(true)).collect()
    }
}

/// 2.D, 2.E and 2.H: with no bodies the field write only; a `SkeletalMeshComponent` with a `PhysicsAssetInstance`, or a
/// plain component with a `BodyInstance`: the `LayerAbsent`, nothing written.
#[test]
fn the_body_setters_write_or_need_the_physics_layer() {
    let mut x = fx();
    let c = x.make(x.primitive, "Comp");
    let before = x.snapshot(c);
    x.call(RB, c, vec![Value::Byte(1)]).unwrap();
    assert_eq!(x.get(c, "RBChannel"), Value::Byte(1));
    let changed: Vec<usize> = before.iter().zip(x.snapshot(c)).enumerate().filter(|(_, (a, b))| *a != b).map(|(i, _)| i).collect();
    assert_eq!(changed.len(), 1, "RBChannel only");
    for (which, field) in [(BLOCK_RB, "BlockRigidBody"), (NOTIFY, "bNotifyRigidBodyCollision")] {
        x.call(which, c, vec![Value::Bool(true)]).unwrap();
        assert_eq!(x.get(c, field), Value::Bool(true));
        x.call(which, c, vec![Value::Bool(false)]).unwrap();
        assert_eq!(x.get(c, field), Value::Bool(false), "cleared");
    }
    let body = x.make(x.actor, "ToyBody");
    let plain = x.make(x.primitive, "WithBody");
    x.set(plain, "BodyInstance", Value::Object(Some(body)));
    let mesh = x.make(x.skeletal, "WithAsset");
    x.set(mesh, "PhysicsAssetInstance", Value::Object(Some(body)));
    for o in [plain, mesh] {
        for (which, arg) in [(RB, Value::Byte(2)), (BLOCK_RB, Value::Bool(true)), (NOTIFY, Value::Bool(true))] {
            let n = x.refusal(which, o, vec![arg]);
            assert!(matches!(n.missing, Missing::LayerAbsent(_)), "{n}");
            assert_eq!(&*n.what, RB_FILTER);
        }
    }
    // A skeletal mesh component's BodyInstance is not its bodies' field: with no PhysicsAssetInstance, the write.
    let mesh = x.make(x.skeletal, "BodyOnly");
    x.set(mesh, "BodyInstance", Value::Object(Some(body)));
    x.call(RB, mesh, vec![Value::Byte(1)]).unwrap();
    assert_eq!(x.get(mesh, "RBChannel"), Value::Byte(1));
}

/// 2.G: all four combinations, and no other field changes.
#[test]
fn set_trace_blocking_writes_the_two_bits() {
    let mut x = fx();
    let c = x.make(x.primitive, "Comp");
    for (z, nz) in [(false, false), (true, false), (false, true), (true, true)] {
        let before = x.snapshot(c);
        x.call(TRACE, c, vec![Value::Bool(z), Value::Bool(nz)]).unwrap();
        assert_eq!(x.bools(c, &["BlockZeroExtent", "BlockNonZeroExtent"]), [z, nz]);
        let changed = before.iter().zip(x.snapshot(c)).filter(|(a, b)| *a != b).count();
        assert!(changed <= 2);
    }
}

/// 2.F tests 1, 2, 4 and 5: no change (two plain writes, no reattach); a change with an ordinary owner
/// (`bNeedsReattach` too); the third argument omitted; no change with all three set and an owner (no owner step).
#[test]
fn set_actor_collision_writes() {
    let mut x = fx();
    let owner = x.make(x.actor, "Pawn");
    let c = x.make(x.primitive, "Mesh");
    x.set(c, "Owner", Value::Object(Some(owner)));
    x.call(ACTOR, c, vec![Value::Bool(false), Value::Bool(true), Value::Bool(true)]).unwrap();
    assert_eq!(x.bools(c, &["AlwaysCheckCollision", "CollideActors", "BlockActors", "bNeedsReattach"]), [true, false, true, false]);
    // Maya's (true, false), the third omitted: a change with an ordinary owner.
    x.call(ACTOR, c, vec![Value::Bool(true), Value::Bool(false)]).unwrap();
    assert_eq!(x.bools(c, &["AlwaysCheckCollision", "CollideActors", "BlockActors", "bNeedsReattach"]), [false, true, false, true]);
    // No change, all three set and an owner: the plain writes, no owner step.
    x.set(c, "bNeedsReattach", Value::Bool(false));
    x.call(ACTOR, c, vec![Value::Bool(true), Value::Bool(true), Value::Bool(true)]).unwrap();
    assert_eq!(x.bools(c, &["AlwaysCheckCollision", "CollideActors", "BlockActors", "bNeedsReattach"]), [true, true, true, false]);
}

/// 2.F tests 3, 6 and 7: a change with all three set and an owner is the owner step's `NotRead`; a change with a
/// `bStatic` owner is `REATTACH_OWNER`; with no owner and a non-zero `Scene`, `REATTACH_SCENE`; each with nothing
/// written (`AlwaysCheckCollision` included).
#[test]
fn set_actor_collision_refusals_are_all_or_nothing() {
    let mut x = fx();
    let owner = x.make(x.actor, "Pawn");
    let c = x.make(x.primitive, "Mesh");
    x.set(c, "Owner", Value::Object(Some(owner)));
    let n = x.refusal(ACTOR, c, vec![Value::Bool(true), Value::Bool(true), Value::Bool(true)]);
    assert_eq!((n.missing, &*n.what), (Missing::NotRead, OWNER_STEP));
    x.set(owner, "bStatic", Value::Bool(true));
    let n = x.refusal(ACTOR, c, vec![Value::Bool(true), Value::Bool(false), Value::Bool(true)]);
    assert_eq!(&*n.what, super::super::component_setters::REATTACH_OWNER);
    assert_eq!(x.get(c, "AlwaysCheckCollision"), Value::Bool(false), "not written");
    let lone = x.make(x.primitive, "Lone");
    x.set(lone, "Scene", Value::structure(x.scene, vec![Value::Int(7)]));
    let n = x.refusal(ACTOR, lone, vec![Value::Bool(true), Value::Bool(false)]);
    assert_eq!(&*n.what, super::super::component_setters::REATTACH_SCENE);
}

/// A malformed declaration is `VmError::Script`, nothing written.
#[test]
fn malformed_declarations_are_script_errors() {
    let mut x = fx();
    let odd = decl(&mut x.vm, "RBChannel", PropKind::Int);
    let class = x.vm.space.classes.define("Test.OddComponent", Some(x.primitive), vec![odd]).unwrap();
    let c = x.make(class, "Odd");
    let before = x.marks();
    assert!(matches!(x.call(RB, c, vec![Value::Byte(1)]), Err(VmError::Script { .. })));
    assert_eq!(x.marks(), before);
    let odd = decl(&mut x.vm, "BodyInstance", PropKind::Int);
    let class = x.vm.space.classes.define("Test.OddBody", Some(x.primitive), vec![odd]).unwrap();
    let c = x.make(class, "OddBody");
    let before = x.marks();
    assert!(matches!(x.call(BLOCK_RB, c, vec![Value::Bool(true)]), Err(VmError::Script { .. })));
    assert_eq!(x.marks(), before);
}
