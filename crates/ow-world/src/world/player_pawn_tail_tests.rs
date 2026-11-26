//! NATIVE_LINK1_RBPHYSICS_AND_AFTER.md 2.A, 2.B, 2.D, 2.E and 2.F on invented classes (AGENTS.md rule 9: invented
//! objects and values, the class and field names the packages'). Every refusal leaves #518's counters,
//! `class_code_loads` and the name table unchanged; `InitializeRBPhysics` never writes.

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
    rb: FuncId,
    providers: FuncId,
    linear: FuncId,
    mic: FuncId,
    hook: FuncId,
    player: ClassId,
    component: ClassId,
    color: StructId,
}

fn decl(vm: &mut Vm, name: &str, kind: PropKind) -> PropertyDecl {
    PropertyDecl::new(vm.space.names.intern(name), kind)
}

fn fx_in(mode: ParityMode) -> Fx {
    let mut space = ObjectSpace::new();
    let object = space.classes.define("Core.Object", None, vec![]).unwrap();
    let mut vm = Vm::new(space, DefStore::new_empty(), Rc::new(AttributeSystem::new_empty()), mode);
    let o = PropKind::Object { class: object };
    let props = vec![
        decl(&mut vm, "bDeleteMe", PropKind::Bool),
        decl(&mut vm, "Physics", PropKind::Byte { enum_: None }),
        decl(&mut vm, "Components", PropKind::Array(Box::new(o.clone()))),
        decl(&mut vm, "CollisionComponent", o.clone()),
        decl(&mut vm, "bPushesRigidBodies", PropKind::Bool),
    ];
    let actor = vm.space.classes.define("Engine.Actor", Some(object), props).unwrap();
    let pawn = vm.space.classes.define("WillowGame.WillowPawn", Some(actor), vec![]).unwrap();
    let props = vec![decl(&mut vm, "ImmuneToFallingDamage", PropKind::Bool), decl(&mut vm, "bPushesSomething", PropKind::Bool)];
    let player = vm.space.classes.define("WillowGame.WillowPlayerPawn", Some(pawn), props).unwrap();
    let props = vec![
        decl(&mut vm, "Owner", PropKind::Object { class: actor }),
        decl(&mut vm, "bAttached", PropKind::Bool),
        decl(&mut vm, "CollideActors", PropKind::Bool),
    ];
    let component = vm.space.classes.define("Engine.PrimitiveComponent", Some(object), props).unwrap();
    let mesh = vm.space.classes.define("Engine.MeshComponent", Some(component), vec![]).unwrap();
    let bytes: Vec<_> = ["B", "G", "R", "A"].iter().map(|n| decl(&mut vm, n, PropKind::Byte { enum_: None })).collect();
    let color = vm.space.classes.define_struct("Core.Object.Color", None, bytes).unwrap();
    let floats: Vec<_> = ["R", "G", "B", "A"].iter().map(|n| decl(&mut vm, n, PropKind::Float)).collect();
    let linear = vm.space.classes.define_struct("Core.Object.LinearColor", None, floats).unwrap();
    let declare = |vm: &mut Vm, path: &str, key: &str, owner: ClassId, params: Vec<PropertyDecl>, result: Option<PropertyDecl>| {
        let mut d = FunctionDecl::native(ObjectPath::parse(path).unwrap(), key).with_owner(owner).with_params(params);
        d.result = result;
        vm.declare_function(d).unwrap()
    };
    let rb = declare(&mut vm, "WillowGame.WillowPawn.InitializeRBPhysics", INITIALIZE_RB_PHYSICS, pawn, vec![], None);
    let providers =
        declare(&mut vm, "WillowGame.WillowPawn.InitializeBehaviorProviders", INITIALIZE_BEHAVIOR_PROVIDERS, pawn, vec![], None);
    let c = decl(&mut vm, "C", PropKind::Struct(color)).with_flags(PropertyDecl::PARAM);
    let ret = decl(&mut vm, "ReturnValue", PropKind::Struct(linear));
    let linear_fn = declare(&mut vm, "WillowGame.WillowPlayerPawn.GetLinearColor", GET_LINEAR_COLOR, player, vec![c], Some(ret));
    let i = decl(&mut vm, "ElementIndex", PropKind::Int).with_flags(PropertyDecl::PARAM);
    let mic = declare(&mut vm, "Engine.MeshComponent.GetInstancedMaterialInstanceConstant", GET_INSTANCED_MIC, mesh, vec![i], None);
    let hook = declare(&mut vm, "WillowGame.WillowPlayerPawn.OnNativeApplyBodyClassStartingValuesHook", APPLY_HOOK, player, vec![], None);
    crate::actors::add_empty_scene(&mut vm).unwrap();
    if mode != ParityMode::Reference {
        register(&mut vm).unwrap();
    }
    Fx { vm, rb, providers, linear: linear_fn, mic, hook, player, component, color }
}

fn fx() -> Fx {
    fx_in(ParityMode::Off)
}

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

    /// A pawn with `components`, the first one its `CollisionComponent` when `collision` is set.
    fn pawn(&mut self, components: &[Option<ObjRef>]) -> ObjRef {
        let c = self.player;
        let p = self.make(c, "Pawn_0");
        self.set(p, "Components", Value::array(components.iter().map(|c| Value::Object(*c)).collect()));
        p
    }

    /// A colliding component owned by `owner`, recorded under it when `recorded`.
    fn component(&mut self, name: &str, owner: Option<ObjRef>, recorded: bool) -> ObjRef {
        let class = self.component;
        let c = self.make(class, name);
        self.set(c, "CollideActors", Value::Bool(true));
        self.set(c, "Owner", Value::Object(owner));
        if recorded && let Some(o) = owner {
            crate::actors::publish_attach(&mut self.vm, c, o).unwrap();
        }
        c
    }

    /// `InitializeRBPhysics` on `pawn`: it never writes.
    fn rb(&mut self, pawn: ObjRef) -> Result<(), VmError> {
        let before = self.marks();
        let out = self.vm.call(self.rb, Some(pawn), vec![]).map(|_| ());
        assert_eq!(self.marks(), before, "InitializeRBPhysics writes nothing");
        out
    }
}

fn layer(e: VmError) -> String {
    match e {
        VmError::NotYet(n) => {
            assert!(matches!(n.missing, Missing::LayerAbsent(_)), "{n}");
            n.what.to_string()
        }
        other => panic!("expected NotYet, got {other:?}"),
    }
}

/// 2.A tests 1 and 6: all-None components with `bPushesRigidBodies` clear is nothing (no write, no prefix); `bDeleteMe`
/// set is nothing whatever the components.
#[test]
fn nothing_to_set_up_is_nothing() {
    let mut x = fx();
    let p = x.pawn(&[None, None]);
    x.rb(p).unwrap();
    let c = x.component("Attached", Some(p), true);
    x.set(p, "Components", Value::array(vec![Value::Object(Some(c))]));
    x.set(p, "bDeleteMe", Value::Bool(true));
    x.rb(p).unwrap();
}

/// 2.A tests 2, 3 and 9: a component in the attach record under the pawn is the physics `LayerAbsent`; one not in
/// the record with `bAttached` false is refusal L's convention; one recorded under another owner is not attached, so
/// refusal L's convention too.
#[test]
fn an_attached_component_is_the_physics_layer_and_an_unknown_one_refusal_l() {
    let mut x = fx();
    let p = x.pawn(&[]);
    let c = x.component("Mesh", Some(p), true);
    x.set(p, "Components", Value::array(vec![Value::Object(Some(c))]));
    assert_eq!(layer(x.rb(p).unwrap_err()), RIGID_BODY);
    let p = x.pawn(&[]);
    let c = x.component("Unrecorded", Some(p), false);
    x.set(p, "Components", Value::array(vec![Value::Object(Some(c))]));
    assert_eq!(layer(x.rb(p).unwrap_err()), super::super::bounding_cylinder::REFUSAL_L);
    let other = x.pawn(&[]);
    let p = x.pawn(&[]);
    let c = x.component("OtherOwners", Some(other), true);
    x.set(c, "Owner", Value::Object(Some(other)));
    x.set(p, "Components", Value::array(vec![Value::Object(Some(c))]));
    assert_eq!(layer(x.rb(p).unwrap_err()), super::super::bounding_cylinder::REFUSAL_L);
}

/// 2.A tests 4 and 5: with `PHYS_RigidBody`, a recorded component that is not the `CollisionComponent` is skipped by
/// the filter; `bPushesRigidBodies` set with the `CollisionComponent` in the record is the physics `LayerAbsent`.
#[test]
fn the_rigid_body_filter_and_the_pushing_body() {
    let mut x = fx();
    let p = x.pawn(&[]);
    let c = x.component("NotTheCollision", Some(p), true);
    x.set(p, "Components", Value::array(vec![Value::Object(Some(c))]));
    x.set(p, "Physics", Value::Byte(PHYS_RIGID_BODY));
    x.rb(p).unwrap();
    let p = x.pawn(&[]);
    let cc = x.component("Collision", Some(p), true);
    x.set(p, "CollisionComponent", Value::Object(Some(cc)));
    x.set(p, "bPushesRigidBodies", Value::Bool(true));
    assert_eq!(layer(x.rb(p).unwrap_err()), RIGID_BODY);
    // Clear, with no pushing body: nothing (the same-value write is skipped).
    x.set(p, "bPushesRigidBodies", Value::Bool(false));
    x.rb(p).unwrap();
}

/// 2.A tests 7 and 8: a lazy component is the not-loaded `NotRead`; a template receiver with an entry whose
/// `bAttached` is set is the physics `LayerAbsent` (and with it clear, nothing).
#[test]
fn a_lazy_component_and_a_template_receiver() {
    let mut x = fx();
    let class = x.component;
    let lazy = x.vm.space.alloc_stand_in(class, "Lazy", Origin::Default { class }).unwrap();
    let p = x.pawn(&[Some(lazy)]);
    match x.rb(p) {
        Err(VmError::NotYet(n)) => assert_eq!((n.missing, &*n.what), (Missing::NotRead, NOT_LOADED)),
        other => panic!("{other:?}"),
    }
    let player = x.player;
    let template = x.vm.space.alloc(player, "Default__Pawn", None, Origin::Default { class: player }).unwrap();
    let entry = x.vm.space.alloc(class, "TemplateMesh", None, Origin::Default { class }).unwrap();
    x.set(template, "Components", Value::array(vec![Value::Object(Some(entry))]));
    x.rb(template).unwrap();
    x.set(entry, "bAttached", Value::Bool(true));
    assert_eq!(layer(x.rb(template).unwrap_err()), RIGID_BODY);
}

/// 2.B and 2.E: `NotRead`, nothing written.
#[test]
fn the_provider_list_and_the_instanced_material_are_not_read() {
    let mut x = fx();
    let p = x.pawn(&[]);
    let c = x.component("Mesh", None, false);
    let before = x.marks();
    match x.vm.call(x.providers, Some(p), vec![]) {
        Err(VmError::NotYet(n)) => assert_eq!((n.missing, &*n.what), (Missing::NotRead, PROVIDER_LIST)),
        other => panic!("{other:?}"),
    }
    match x.vm.call(x.mic, Some(c), vec![Value::Int(0)]) {
        Err(VmError::NotYet(n)) => assert_eq!(&*n.what, GET_MATERIAL),
        other => panic!("{other:?}"),
    }
    assert_eq!(x.marks(), before);
}

/// 2.D: (255, 0, 128, 255) and zeros; the f32 multiply, not the divide (byte 3's bits).
#[test]
fn get_linear_color_is_an_f32_multiply() {
    let mut x = fx();
    let p = x.pawn(&[]);
    let color = |x: &Fx, r: u8, g: u8, b: u8, a: u8| Value::structure(x.color, vec![Value::Byte(b), Value::Byte(g), Value::Byte(r), Value::Byte(a)]);
    let rgba = |v: &Value| -> Vec<f32> {
        v.as_struct().unwrap().fields.iter().map(|f| match f {
            Value::Float(x) => *x,
            other => panic!("{other:?}"),
        }).collect()
    };
    let before = x.marks();
    let c = color(&x, 255, 0, 128, 255);
    let out = x.vm.call(x.linear, Some(p), vec![c]).unwrap().ret;
    assert_eq!(rgba(&out), [1.0, 0.0, 0.501_960_8, 1.0]);
    let c = color(&x, 0, 0, 0, 0);
    assert_eq!(rgba(&x.vm.call(x.linear, Some(p), vec![c]).unwrap().ret), [0.0; 4]);
    assert_eq!(x.marks(), before, "nothing written");
    assert_eq!(byte_to_linear(3).to_bits(), 0x3C40_C0C2, "the multiply");
    assert_ne!((3.0f32 / 255.0).to_bits(), 0x3C40_C0C2, "the divide differs");
    assert_eq!((3.0f32 / 255.0).to_bits(), 0x3C40_C0C1);
}

/// 2.F: `ImmuneToFallingDamage` set, and no other field changed.
#[test]
fn the_hook_sets_immune_to_falling_damage_only() {
    let mut x = fx();
    let p = x.pawn(&[]);
    let before = x.vm.space.heap.get(p).unwrap().slots.clone();
    x.vm.call(x.hook, Some(p), vec![]).unwrap();
    assert_eq!(x.get(p, "ImmuneToFallingDamage"), Value::Bool(true));
    let after = x.vm.space.heap.get(p).unwrap().slots.clone();
    assert_eq!(before.iter().zip(&after).filter(|(a, b)| a != b).count(), 1);
}

/// Reference binds none of the five keys.
#[test]
fn reference_binds_nothing() {
    let x = fx_in(ParityMode::Reference);
    for key in [INITIALIZE_RB_PHYSICS, INITIALIZE_BEHAVIOR_PROVIDERS, GET_LINEAR_COLOR, GET_INSTANCED_MIC, APPLY_HOOK] {
        assert!(!x.vm.has_native(key), "{key}");
    }
}
