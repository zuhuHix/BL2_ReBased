//! NATIVE_LINK1_RBPHYSICS_AND_AFTER.md 2.A, 2.B, 2.D, 2.E and 2.F on invented classes (AGENTS.md rule 9: invented
//! objects and values, the class and field names the packages'). Every refusal leaves #518's counters,
//! `class_code_loads` and the name table unchanged. `InitializeRBPhysics`' class table (NATIVE_LINK1_RIGID_BODY_LAYER.md
//! section 5's tests) writes only on a live receiver, and only when nothing refuses.

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
    linear: FuncId,
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
    // The component classes: each of the note's table native (the class table's mark), plus a toy native class the
    // note did not read and a script subclass of the cylinder.
    let props = vec![decl(&mut vm, "Owner", PropKind::Object { class: actor }), decl(&mut vm, "bAttached", PropKind::Bool)];
    let base = vm.space.classes.define("Engine.ActorComponent", Some(object), props).unwrap();
    let props = vec![decl(&mut vm, "CollideActors", PropKind::Bool)];
    let component = vm.space.classes.define("Engine.PrimitiveComponent", Some(base), props).unwrap();
    let mesh_component = vm.space.classes.define("Engine.MeshComponent", Some(component), vec![]).unwrap();
    let cylinder = vm.space.classes.define("Engine.CylinderComponent", Some(component), vec![]).unwrap();
    let light = vm.space.classes.define("Engine.DynamicLightEnvironmentComponent", Some(base), vec![]).unwrap();
    let dummy = vec![decl(&mut vm, "Dummy", PropKind::Int)];
    let pointer = vm.space.classes.define_struct("Core.Object.Pointer", None, dummy).unwrap();
    let member = decl(&mut vm, "Component", PropKind::Object { class: base });
    let attachment = vm.space.classes.define_struct("Engine.SkeletalMeshComponent.Attachment", None, vec![member]).unwrap();
    let cloth = vec![decl(&mut vm, "bClothMetal", PropKind::Bool)];
    let mesh = vm.space.classes.define("Engine.SkeletalMesh", Some(object), cloth).unwrap();
    let props = vec![
        decl(&mut vm, "bDisableAllRigidBody", PropKind::Bool),
        decl(&mut vm, "bHasPhysicsAssetInstance", PropKind::Bool),
        decl(&mut vm, "bSkelCompFixed", PropKind::Bool),
        decl(&mut vm, "bEnableSoftBodySimulation", PropKind::Bool),
        decl(&mut vm, "bUseSingleBodyPhysics", PropKind::Int),
        decl(&mut vm, "ClothSim", PropKind::Struct(pointer)),
        decl(&mut vm, "ClothMeshIndexData", PropKind::Array(Box::new(PropKind::Int))),
        decl(&mut vm, "SkeletalMesh", PropKind::Object { class: mesh }),
        decl(&mut vm, "Attachments", PropKind::Array(Box::new(PropKind::Struct(attachment)))),
    ];
    let skeletal = vm.space.classes.define("Engine.SkeletalMeshComponent", Some(mesh_component), props).unwrap();
    let gear = vm.space.classes.define("WillowGame.GearLikenessMeshComponent", Some(skeletal), vec![]).unwrap();
    let toy = vm.space.classes.define("Test.ToyNativeComponent", Some(base), vec![]).unwrap();
    vm.space.classes.define("Test.ToyScriptCylinder", Some(cylinder), vec![]).unwrap();
    for c in [base, component, cylinder, light, skeletal, gear, toy] {
        vm.space.classes.mark_native(c).unwrap();
    }
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
    let c = decl(&mut vm, "C", PropKind::Struct(color)).with_flags(PropertyDecl::PARAM);
    let ret = decl(&mut vm, "ReturnValue", PropKind::Struct(linear));
    let linear_fn = declare(&mut vm, "WillowGame.WillowPlayerPawn.GetLinearColor", GET_LINEAR_COLOR, player, vec![c], Some(ret));
    let hook = declare(&mut vm, "WillowGame.WillowPlayerPawn.OnNativeApplyBodyClassStartingValuesHook", APPLY_HOOK, player, vec![], None);
    crate::actors::add_empty_scene(&mut vm).unwrap();
    if mode != ParityMode::Reference {
        register(&mut vm).unwrap();
    }
    Fx { vm, rb, linear: linear_fn, hook, player, component, color }
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

    /// `InitializeRBPhysics` on `pawn`: a refusal writes nothing (every refusal comes before the first write).
    fn rb(&mut self, pawn: ObjRef) -> Result<(), VmError> {
        let before = self.marks();
        let out = self.vm.call(self.rb, Some(pawn), vec![]).map(|_| ());
        if out.is_err() {
            assert_eq!(self.marks(), before, "a refusal writes nothing");
        }
        out
    }

    /// `InitializeRBPhysics` on `pawn`, answering, with the count of its committed writes; nothing else is counted.
    fn rb_writes(&mut self, pawn: ObjRef) -> u64 {
        let before = self.marks();
        self.rb(pawn).unwrap();
        let after = self.marks();
        assert_eq!((after.1, after.2, after.3, after.4), (before.1, before.2, before.3, before.4), "no draw, load or name");
        after.0 - before.0
    }

    /// A component of the class at `path` owned by `owner`, in the attach record under it when `recorded`.
    fn part(&mut self, path: &str, name: &str, owner: ObjRef, recorded: bool) -> ObjRef {
        let class = self.vm.space.classes.by_path(path).unwrap();
        let c = self.make(class, name);
        self.set(c, "Owner", Value::Object(Some(owner)));
        if recorded {
            crate::actors::publish_attach(&mut self.vm, c, owner).unwrap();
        }
        c
    }

    /// A skeletal mesh asset with its cloth bool.
    fn mesh(&mut self, cloth: bool) -> ObjRef {
        let class = self.vm.space.classes.by_path("Engine.SkeletalMesh").unwrap();
        let m = self.make(class, "Skel_Toy");
        self.set(m, "bClothMetal", Value::Bool(cloth));
        m
    }

    /// `parent`'s `Attachments`, one entry per child.
    fn attach(&mut self, parent: ObjRef, children: &[ObjRef]) {
        let ty = self.vm.space.classes.struct_by_path("Engine.SkeletalMeshComponent.Attachment").unwrap();
        let list = children.iter().map(|c| Value::structure(ty, vec![Value::Object(Some(*c))])).collect();
        self.set(parent, "Attachments", Value::array(list));
    }

    fn fixed(&self, o: ObjRef) -> bool {
        matches!(self.get(o, "bSkelCompFixed"), Value::Bool(true))
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

/// 2.A tests 2, 3 and 9, with the class table: a `PrimitiveComponent` in the attach record under the pawn is nothing
/// (it has no shape; before the table, the physics `LayerAbsent`); one not in the record with `bAttached` false is
/// refusal L's convention; one recorded under another owner is not attached, so refusal L's convention too.
#[test]
fn an_attached_primitive_is_nothing_and_an_unknown_one_refusal_l() {
    let mut x = fx();
    let p = x.pawn(&[]);
    let c = x.component("Mesh", Some(p), true);
    x.set(p, "Components", Value::array(vec![Value::Object(Some(c))]));
    assert_eq!(x.rb_writes(p), 0);
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
        Err(VmError::NotYet(n)) => {
            assert_eq!(n.missing, Missing::NotRead);
            assert!(n.what.starts_with(&format!("{NOT_LOADED}: ")), "{}", n.what);
            assert!(n.what.ends_with("Lazy"), "names the component: {}", n.what);
        }
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

/// Reference binds none of the four keys through the world pack's own gate (review of #742, item 4); Off binds them all,
/// and not `InitializeBehaviorProviders`, whose rule the kernel pack binds (#744's landing check).
#[test]
fn reference_binds_nothing() {
    let keys = [INITIALIZE_RB_PHYSICS, GET_LINEAR_COLOR, APPLY_HOOK];
    let mut r = fx_in(ParityMode::Reference);
    ow_vm::api::NativePack::register(&crate::world::Pack, &mut r.vm).unwrap();
    for key in keys {
        assert!(!r.vm.has_native(key), "{key}");
    }
    let x = fx();
    for key in keys {
        assert!(x.vm.has_native(key), "{key}");
    }
    assert!(!x.vm.has_native("WillowPawn.InitializeBehaviorProviders()"), "the kernel pack's key");
    assert!(!x.vm.has_native("MeshComponent.GetInstancedMaterialInstanceConstant(int)"), "the inventory pack's key");
}

/// Review of #742, item 1: on a live receiver only the attach record counts. An unrecorded entry whose `bAttached` is
/// set is refusal L's convention (its text and layer), not the physics `LayerAbsent`.
#[test]
fn a_live_entry_s_own_b_attached_is_not_read() {
    let mut x = fx();
    let p = x.pawn(&[]);
    let c = x.component("Unrecorded", Some(p), false);
    x.set(c, "bAttached", Value::Bool(true));
    x.set(p, "Components", Value::array(vec![Value::Object(Some(c))]));
    match x.rb(p) {
        Err(VmError::NotYet(n)) => {
            assert_eq!(&*n.what, super::super::bounding_cylinder::REFUSAL_L);
            assert!(matches!(n.missing, Missing::LayerAbsent(l) if l == super::super::bounding_cylinder::BOUNDS_LAYER), "{n}");
        }
        other => panic!("{other:?}"),
    }
}

/// Review of #742, items 2 and 3: the entry checks run in one pass before the first answer. A recorded mesh (whose
/// answer would be the physics `LayerAbsent`) followed by an entry whose class declares no `Owner` is the
/// `VmError::Script` of the second entry, on a live receiver.
#[test]
fn the_entry_checks_run_before_the_first_answer() {
    let mut x = fx();
    let p = x.pawn(&[]);
    let mesh = x.component("Mesh", Some(p), true);
    let object = x.vm.space.classes.by_path("Core.Object").unwrap();
    let ownerless = x.vm.space.classes.define("Test.OwnerlessComponent", Some(object), vec![]).unwrap();
    let stray = x.make(ownerless, "Stray");
    x.set(p, "Components", Value::array(vec![Value::Object(Some(mesh)), Value::Object(Some(stray))]));
    match x.rb(p) {
        Err(VmError::Script { message, .. }) => assert!(message.contains("Owner"), "{message}"),
        other => panic!("{other:?}"),
    }
}

// ---- The class table (lane IMPL-LINK1-RIGID-BODY-CLASS-TABLE, NATIVE_LINK1_RIGID_BODY_LAYER.md section 5) --------

fn not_read_what(e: VmError) -> (String, &'static str) {
    match e {
        VmError::NotYet(n) => {
            assert_eq!(n.missing, Missing::NotRead, "{n}");
            (n.what.to_string(), n.record)
        }
        other => panic!("expected NotYet, got {other:?}"),
    }
}

/// A pawn whose `Components` are `parts` (each `(class path, name)`, recorded under it).
fn pawn_with(x: &mut Fx, parts: &[(&str, &str)]) -> (ObjRef, Vec<ObjRef>) {
    let p = x.pawn(&[]);
    let made: Vec<ObjRef> = parts.iter().map(|(path, name)| x.part(path, name, p, true)).collect();
    x.set(p, "Components", Value::array(made.iter().map(|c| Value::Object(Some(*c))).collect()));
    (p, made)
}

/// Test 1: an attached `CylinderComponent` and an attached `DynamicLightEnvironmentComponent`: nothing written, no
/// refusal. A script subclass of the cylinder takes its first native class's answer, nothing.
#[test]
fn a_cylinder_and_a_light_environment_are_nothing() {
    let mut x = fx();
    let (p, _) = pawn_with(&mut x, &[
        ("Engine.CylinderComponent", "Cylinder"),
        ("Engine.DynamicLightEnvironmentComponent", "Light"),
        ("Test.ToyScriptCylinder", "ScriptCylinder"),
    ]);
    assert_eq!(x.rb_writes(p), 0);
}

/// Test 2: an attached `SkeletalMeshComponent` with every flag off and `ClothSim` zero: `bSkelCompFixed` true and
/// `ClothMeshIndexData` emptied. With the pawn `PHYS_RigidBody` and the mesh its `CollisionComponent`: false.
#[test]
fn a_plain_skeletal_entry_is_fixed_and_its_cloth_index_emptied() {
    let mut x = fx();
    let (p, made) = pawn_with(&mut x, &[("Engine.SkeletalMeshComponent", "Body")]);
    let body = made[0];
    let m = x.mesh(false);
    x.set(body, "SkeletalMesh", Value::Object(Some(m)));
    x.set(body, "ClothMeshIndexData", Value::array(vec![Value::Int(3), Value::Int(4)]));
    assert_eq!(x.rb_writes(p), 2, "bSkelCompFixed and ClothMeshIndexData");
    assert!(x.fixed(body));
    assert_eq!(x.get(body, "ClothMeshIndexData"), Value::array(Vec::new()));
    x.set(p, "Physics", Value::Byte(PHYS_RIGID_BODY));
    x.set(p, "CollisionComponent", Value::Object(Some(body)));
    x.rb(p).unwrap();
    assert!(!x.fixed(body), "not fixed under PHYS_RigidBody");
}

/// Tests 3 and 4: `bHasPhysicsAssetInstance` with a mesh, `bUseSingleBodyPhysics` 1, or `bEnableSoftBodySimulation`:
/// the physics `LayerAbsent`, nothing written.
#[test]
fn the_body_making_flags_are_the_physics_layer() {
    for (name, v) in [
        ("bHasPhysicsAssetInstance", Value::Bool(true)),
        ("bUseSingleBodyPhysics", Value::Int(1)),
        ("bEnableSoftBodySimulation", Value::Bool(true)),
    ] {
        let mut x = fx();
        let (p, made) = pawn_with(&mut x, &[("Engine.SkeletalMeshComponent", "Body")]);
        let m = x.mesh(false);
        x.set(made[0], "SkeletalMesh", Value::Object(Some(m)));
        x.set(made[0], name, v);
        assert_eq!(layer(x.rb(p).unwrap_err()), RIGID_BODY, "{name}");
        assert!(!x.fixed(made[0]), "{name}");
    }
}

/// Test 5: `bDisableAllRigidBody` set: nothing, `bSkelCompFixed` unchanged, and its attachments are not visited.
#[test]
fn disable_all_rigid_body_is_nothing() {
    let mut x = fx();
    let (p, made) = pawn_with(&mut x, &[("Engine.SkeletalMeshComponent", "Body")]);
    let head = x.part("WillowGame.GearLikenessMeshComponent", "Head", p, true);
    x.attach(made[0], &[head]);
    x.set(made[0], "bDisableAllRigidBody", Value::Bool(true));
    assert_eq!(x.rb_writes(p), 0);
    assert!(!x.fixed(made[0]) && !x.fixed(head));
}

/// Test 6: a skeletal entry with an attached `Attachments` component of a skeletal subclass: both written. With the
/// attachment's `bHasPhysicsAssetInstance` set and a mesh: the `LayerAbsent`, and neither written.
#[test]
fn an_attached_attachment_takes_the_same_rule() {
    let mut x = fx();
    let (p, made) = pawn_with(&mut x, &[("Engine.SkeletalMeshComponent", "Body")]);
    let head = x.part("WillowGame.GearLikenessMeshComponent", "Head", p, true);
    x.attach(made[0], &[head]);
    x.rb(p).unwrap();
    assert!(x.fixed(made[0]) && x.fixed(head));
    let mut x = fx();
    let (p, made) = pawn_with(&mut x, &[("Engine.SkeletalMeshComponent", "Body")]);
    let head = x.part("WillowGame.GearLikenessMeshComponent", "Head", p, true);
    x.attach(made[0], &[head]);
    let m = x.mesh(false);
    x.set(head, "SkeletalMesh", Value::Object(Some(m)));
    x.set(head, "bHasPhysicsAssetInstance", Value::Bool(true));
    assert_eq!(layer(x.rb(p).unwrap_err()), RIGID_BODY);
    assert!(!x.fixed(made[0]) && !x.fixed(head), "the up-front preflight");
}

/// Test 7: an attachment whose component is not in the attach record is not visited.
#[test]
fn an_unrecorded_attachment_is_not_visited() {
    let mut x = fx();
    let (p, made) = pawn_with(&mut x, &[("Engine.SkeletalMeshComponent", "Body")]);
    let head = x.part("WillowGame.GearLikenessMeshComponent", "Head", p, false);
    x.set(head, "bUseSingleBodyPhysics", Value::Int(1));
    x.attach(made[0], &[head]);
    x.rb(p).unwrap();
    assert!(x.fixed(made[0]));
    assert!(!x.fixed(head), "not visited");
}

/// Test 8: an attached component of an unread class (a toy native component class) is the `NotRead` naming it, as is
/// an attachment of that class; nothing is written.
#[test]
fn an_unread_class_is_not_read() {
    let mut x = fx();
    let (p, _) = pawn_with(&mut x, &[("Engine.SkeletalMeshComponent", "Body"), ("Test.ToyNativeComponent", "Toy")]);
    let (what, record) = not_read_what(x.rb(p).unwrap_err());
    assert_eq!((what.as_str(), record), (format!("{UNREAD_CLASS} Test.ToyNativeComponent").as_str(), RECORD_LAYER));
    let mut x = fx();
    let (p, made) = pawn_with(&mut x, &[("Engine.SkeletalMeshComponent", "Body")]);
    let toy = x.part("Test.ToyNativeComponent", "Toy", p, true);
    x.attach(made[0], &[toy]);
    assert_eq!(not_read_what(x.rb(p).unwrap_err()).0, format!("{UNREAD_CLASS} Test.ToyNativeComponent"));
    assert!(!x.fixed(made[0]));
}

/// Test 9: the mesh's cloth bool set with `ClothSim` zero is the cloth `NotRead`. With `ClothSim` not zero there is
/// no cloth step: the writes, and `ClothMeshIndexData` is kept.
#[test]
fn the_cloth_bool_with_no_cloth_sim_is_not_read() {
    let mut x = fx();
    let (p, made) = pawn_with(&mut x, &[("Engine.SkeletalMeshComponent", "Body")]);
    let m = x.mesh(true);
    x.set(made[0], "SkeletalMesh", Value::Object(Some(m)));
    assert_eq!(not_read_what(x.rb(p).unwrap_err()), (CLOTH.to_string(), RECORD_LAYER));
    let ty = x.vm.space.classes.struct_by_path("Core.Object.Pointer").unwrap();
    x.set(made[0], "ClothSim", Value::structure(ty, vec![Value::Int(7)]));
    x.set(made[0], "ClothMeshIndexData", Value::array(vec![Value::Int(1)]));
    assert_eq!(x.rb_writes(p), 1, "bSkelCompFixed only");
    assert_eq!(x.get(made[0], "ClothMeshIndexData"), Value::array(vec![Value::Int(1)]));
}

/// Test 10: a `GearLikenessMeshComponent` with no `SkeletalMesh`: the writes, no cloth step, and
/// `bHasPhysicsAssetInstance` alone (no mesh) makes no body.
#[test]
fn a_mesh_less_gear_likeness_component_takes_the_writes() {
    let mut x = fx();
    let (p, made) = pawn_with(&mut x, &[("WillowGame.GearLikenessMeshComponent", "Gear")]);
    x.set(made[0], "bHasPhysicsAssetInstance", Value::Bool(true));
    assert_eq!(x.rb_writes(p), 2);
    assert!(x.fixed(made[0]));
}

/// Test 11: the pushing body first. An attached skeletal entry with every flag off, and `bPushesRigidBodies` set with
/// an attached `CollisionComponent`: the physics `LayerAbsent`, and `bSkelCompFixed` not written.
#[test]
fn the_pushing_body_refuses_before_the_skeletal_writes() {
    let mut x = fx();
    let (p, made) = pawn_with(&mut x, &[("Engine.SkeletalMeshComponent", "Body"), ("Engine.CylinderComponent", "Cylinder")]);
    x.set(p, "CollisionComponent", Value::Object(Some(made[1])));
    x.set(p, "bPushesRigidBodies", Value::Bool(true));
    assert_eq!(layer(x.rb(p).unwrap_err()), RIGID_BODY);
    assert!(!x.fixed(made[0]));
}

/// Test 12: a live entry not in the attach record (`bAttached` false), after a skeletal entry that would be written:
/// refusal L's `LayerAbsent`, nothing written.
#[test]
fn an_unrecorded_live_entry_is_refusal_l_before_any_write() {
    let mut x = fx();
    let (p, made) = pawn_with(&mut x, &[("Engine.SkeletalMeshComponent", "Body")]);
    let stray = x.part("Engine.CylinderComponent", "Stray", p, false);
    x.set(p, "Components", Value::array(vec![Value::Object(Some(made[0])), Value::Object(Some(stray))]));
    assert_eq!(layer(x.rb(p).unwrap_err()), super::super::bounding_cylinder::REFUSAL_L);
    assert!(!x.fixed(made[0]));
}

/// Test 13: a template receiver whose skeletal entry has `bAttached` set: the physics `LayerAbsent`, and the
/// template's `bSkelCompFixed` unchanged.
#[test]
fn a_template_receiver_keeps_the_physics_layer() {
    let mut x = fx();
    let player = x.player;
    let skeletal = x.vm.space.classes.by_path("Engine.SkeletalMeshComponent").unwrap();
    let template = x.vm.space.alloc(player, "Default__Pawn", None, Origin::Default { class: player }).unwrap();
    let entry = x.vm.space.alloc(skeletal, "TemplateBody", None, Origin::Default { class: skeletal }).unwrap();
    x.set(template, "Components", Value::array(vec![Value::Object(Some(entry))]));
    x.set(entry, "bAttached", Value::Bool(true));
    assert_eq!(layer(x.rb(template).unwrap_err()), RIGID_BODY);
    assert!(!x.fixed(entry));
}

/// The preflight's residency: an unloaded attachment component and an unloaded mesh (when its cloth bool is read) are
/// the not-loaded `NotRead`, nothing written or loaded.
#[test]
fn an_unloaded_attachment_or_mesh_is_not_read() {
    let mut x = fx();
    let (p, made) = pawn_with(&mut x, &[("Engine.SkeletalMeshComponent", "Body")]);
    let gear = x.vm.space.classes.by_path("WillowGame.GearLikenessMeshComponent").unwrap();
    let lazy = x.vm.space.alloc_stand_in(gear, "LazyHead", Origin::Default { class: gear }).unwrap();
    x.attach(made[0], &[lazy]);
    assert!(not_read_what(x.rb(p).unwrap_err()).0.starts_with(&format!("{NOT_LOADED}: ")));
    assert!(!x.fixed(made[0]));
    let mut x = fx();
    let (p, made) = pawn_with(&mut x, &[("Engine.SkeletalMeshComponent", "Body")]);
    let class = x.vm.space.classes.by_path("Engine.SkeletalMesh").unwrap();
    let lazy = x.vm.space.alloc_stand_in(class, "LazyMesh", Origin::Default { class }).unwrap();
    x.set(made[0], "SkeletalMesh", Value::Object(Some(lazy)));
    assert!(not_read_what(x.rb(p).unwrap_err()).0.ends_with("LazyMesh"));
    assert!(!x.fixed(made[0]));
}

/// The declarations: a skeletal class missing one of the rule's fields is a data error naming it, before any write.
#[test]
fn a_skeletal_class_missing_a_field_is_a_data_error() {
    let mut x = fx();
    let mesh_component = x.vm.space.classes.by_path("Engine.MeshComponent").unwrap();
    let bare = x.vm.space.classes.define("WillowGame.CustomSkeletalMeshComponent", Some(mesh_component), vec![]).unwrap();
    x.vm.space.classes.mark_native(bare).unwrap();
    let (p, made) = pawn_with(&mut x, &[("Engine.SkeletalMeshComponent", "Body"), ("WillowGame.CustomSkeletalMeshComponent", "Bare")]);
    match x.rb(p) {
        Err(VmError::Script { message, .. }) => assert!(message.contains("bDisableAllRigidBody"), "{message}"),
        other => panic!("{other:?}"),
    }
    assert!(!x.fixed(made[0]));
}
