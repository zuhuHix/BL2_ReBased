//! `InitializeBodyComposition` on invented classes and structs (AGENTS.md rule 9: invented values; the class, struct,
//! field and enum names are the packages'), through the pawn wrapper (`InitializePawnMeshes`) and the script native.
//! Graded by the `VmError` variant and the `NotYet` kind. The note is NATIVE_INITIALIZE_BODY_COMPOSITION.md at 87c8b9cd.

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::notyet::Missing;
use ow_core::{ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ClassId, FuncId, ObjRef, PropAddr, StructId};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_package::testkit::script::ScriptToy;
use ow_vm::api::{FunctionDecl, Vm};
use ow_vm::error::VmError;

use super::*;

static NEXT: AtomicU32 = AtomicU32::new(0);

struct Env {
    vm: Vm,
    dir: PathBuf,
    object: ClassId,
    pawn: ClassId,
    body: ClassId,
    cylinder: ClassId,
    skeletal: ClassId,
    union: StructId,
    body_union: StructId,
    component_data: StructId,
    composition: StructId,
    wrapper: FuncId,
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn decl(vm: &mut Vm, name: &str, kind: PropKind) -> PropertyDecl {
    PropertyDecl::new(vm.space.names.intern(name), kind)
}

fn class(vm: &mut Vm, path: &str, base: ClassId, props: &[(&str, PropKind)]) -> ClassId {
    let props = props.iter().map(|(n, k)| decl(vm, n, k.clone())).collect();
    vm.space.classes.define(path, Some(base), props).unwrap()
}

fn strukt(vm: &mut Vm, path: &str, props: &[(&str, PropKind)]) -> StructId {
    let props = props.iter().map(|(n, k)| decl(vm, n, k.clone())).collect();
    vm.space.classes.define_struct(path, None, props).unwrap()
}

/// The type enum: the note's names, at the positions the note gives for the ones it numbers.
const TYPES: [&str; 13] = [
    "IDT_Bool", "IDT_Int", "IDT_Float", "IDT_Vector", "IDT_Object", "IDT_ActorComponent", "IDT_String", "IDT_ActorTemplate",
    "IDT_Actor", "IDT_Switch", "IDT_PickupTemplate", "IDT_CustomHead", "IDT_CustomSkin",
];
const COMPONENT: u8 = 5;
const SWITCH: u8 = 9;
const HEAD: u8 = 11;
const ACTOR_TEMPLATE: u8 = 7;

const FLAGS: [&str; 5] = ["CollideActors", "BlockActors", "BlockRigidBody", "BlockNonZeroExtent", "BlockZeroExtent"];

fn env() -> Env {
    let dir = std::env::temp_dir().join(format!("ow-world-bodycomp-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let core = {
        let mut t = ScriptToy::new(&[]);
        t.class("Object", 0);
        t.build()
    };
    let files: Vec<(&str, &[u8])> = vec![("Core.upk", core.as_slice())];
    let defs = DefStore::from_synthetic(&dir, &files, IdentityPolicy::FirstInScanOrder, ParityMode::Off).expect("store");
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
    let object = vm.find_class("Core.Object").unwrap();
    let o = PropKind::Object { class: object };
    let names = TYPES.iter().map(|n| vm.space.names.intern(n)).collect();
    let types = vm.space.classes.define_enum("Engine.IInstanceData.EInstanceDataType", names).unwrap();
    let component_data = strukt(&mut vm, "Engine.IInstanceData.ComponentData", &[
        ("Component", o.clone()),
        ("CollisionType", PropKind::Byte { enum_: None }),
        ("MeshSocketName", PropKind::Name),
        ("HitRegionDefinition", o.clone()),
    ]);
    let union = strukt(&mut vm, "Engine.IInstanceData.InstanceDataUnion", &[
        ("Name", PropKind::Name),
        ("Type", PropKind::Byte { enum_: Some(types) }),
        ("Replication", PropKind::Byte { enum_: None }),
        ("MaxSwitchValue", PropKind::Byte { enum_: None }),
        ("BitsRequiredForMaxSwitchValue", PropKind::Byte { enum_: None }),
        ("DeleteByOwner", PropKind::Bool),
        ("Int", PropKind::Int),
        ("Float", PropKind::Float),
        ("Object", o.clone()),
        ("ComponentData", PropKind::Struct(component_data)),
        ("BodyCompositionIndex", PropKind::Int),
    ]);
    let body_union = strukt(&mut vm, "Engine.InstanceDataHelper.BodyInstanceDataUnion", &[
        ("Data", PropKind::Struct(union)),
        ("ParentAttachmentIdentifier", PropKind::Name),
        ("AttachmentBaseInstanceDataIndex", PropKind::Int),
        ("SwitchIdentifier", PropKind::Byte { enum_: None }),
    ]);
    let composition = strukt(&mut vm, "Engine.InstanceDataHelper.BodyCompositionData", &[
        ("Attachments", PropKind::Array(Box::new(PropKind::Struct(body_union)))),
        ("MaxExpectedComponents", PropKind::Byte { enum_: None }),
    ]);
    let set = strukt(&mut vm, "Engine.IInstanceData.InstanceDataSet", &[("Data", PropKind::Array(Box::new(PropKind::Struct(union))))]);
    let rep = strukt(&mut vm, "Engine.IInstanceData.ReplicatedInstanceDataState", &[
        ("SwitchStateBitField", PropKind::Int),
        ("ActiveSwitchValues", PropKind::Int),
        ("RemovedBodyCompositionPartsBitField", PropKind::Int),
        ("bNeedToApplyThisState", PropKind::Bool),
    ]);
    class(&mut vm, crate::world::pawn_meshes::HELPER, object, &[]);
    let component = class(&mut vm, "Engine.ActorComponent", object, &[("Owner", o.clone())]);
    let mut prim_props: Vec<(&str, PropKind)> = FLAGS.iter().map(|f| (*f, PropKind::Bool)).collect();
    prim_props.push(("LightEnvironment", o.clone()));
    let primitive = class(&mut vm, PRIMITIVE, component, &prim_props);
    let cylinder = class(&mut vm, CYLINDER, primitive, &[("CollisionRadius", PropKind::Float)]);
    let mesh = class(&mut vm, MESH_COMPONENT, primitive, &[]);
    let skeletal = class(&mut vm, SKELETAL, mesh, &[("SkeletalMesh", o.clone())]);
    let actor = class(&mut vm, "Engine.Actor", object, &[
        ("Role", PropKind::Byte { enum_: None }),
        ("bNetDirty", PropKind::Bool),
        ("bForceNetUpdate", PropKind::Bool),
        ("bCollideActors", PropKind::Bool),
        ("bBlockActors", PropKind::Bool),
        ("bPushedByEncroachers", PropKind::Bool),
        ("CollisionComponent", o.clone()),
        ("Components", PropKind::Array(Box::new(o.clone()))),
    ]);
    let body = class(&mut vm, "WillowGame.BodyClassDefinition", object, &[
        ("BodyComposition", PropKind::Struct(composition)),
        ("UseRootMotion", PropKind::Bool),
        ("UseRootRotation", PropKind::Bool),
        ("bUseMeshForCollision", PropKind::Bool),
    ]);
    let pawn = class(&mut vm, crate::world::pawn_meshes::WILLOW_PAWN, actor, &[
        ("BodyClass", PropKind::Object { class: body }),
        ("InstanceDataState", PropKind::Struct(set)),
        ("ReplicatedInstanceDataState", PropKind::Struct(rep)),
        ("CylinderComponent", o.clone()),
        ("HeadMesh", o.clone()),
        ("LightEnvironment", o.clone()),
    ]);
    let path = "WillowGame.WillowPawn.InitializePawnMeshes";
    let d = FunctionDecl::native(ObjectPath::parse(path).unwrap(), "WillowPawn.InitializePawnMeshes()").with_owner(pawn);
    let wrapper = vm.declare_function(d).unwrap();
    crate::world::pawn_meshes::register(&mut vm).unwrap();
    Env { vm, dir, object, pawn, body, cylinder, skeletal, union, body_union, component_data, composition, wrapper }
}

fn make(vm: &mut Vm, c: ClassId, name: &str) -> ObjRef {
    vm.space.alloc(c, name, None, Origin::Runtime).unwrap()
}

fn prop(vm: &Vm, o: ObjRef, name: &str) -> Value {
    let class = vm.space.heap.get(o).unwrap().class;
    let p = vm.space.prop(class, name).unwrap();
    vm.space.read(o, PropAddr::scalar(p)).unwrap().clone()
}

fn put(vm: &mut Vm, o: ObjRef, name: &str, v: Value) {
    let class = vm.space.heap.get(o).unwrap().class;
    let p = vm.space.prop(class, name).unwrap();
    vm.space.write(o, PropAddr::scalar(p), v).unwrap();
}

/// A struct value of `s` with `fields` set, the rest at their defaults.
fn value(vm: &Vm, s: StructId, fields: Vec<(&str, Value)>) -> Value {
    let mut v = vm.space.classes.struct_defaults(s).unwrap();
    for (n, x) in fields {
        set_field(vm, &mut v, n, x, "test").unwrap();
    }
    v
}

fn name(vm: &mut Vm, s: &str) -> Value {
    Value::Name(vm.space.names.intern(s))
}

/// One attachment.
struct A<'a> {
    ty: u8,
    base: i32,
    socket: &'a str,
    parent: &'a str,
    data_name: &'a str,
    template: Option<ObjRef>,
    collision: u8,
    switch_id: u8,
    int: i32,
    bits: u8,
    hit_region: Option<ObjRef>,
    /// `Data.BodyCompositionIndex`: the packages hold the attachment's index + 1.
    index: i32,
}

impl Default for A<'_> {
    fn default() -> Self {
        A {
            ty: COMPONENT,
            base: -1,
            socket: "None",
            parent: "None",
            data_name: "None",
            template: None,
            collision: 0,
            switch_id: 0,
            int: 0,
            bits: 1,
            hit_region: None,
            index: 1,
        }
    }
}

fn attachment(e: &mut Env, a: A<'_>) -> Value {
    let socket = name(&mut e.vm, a.socket);
    let parent = name(&mut e.vm, a.parent);
    let entry_name = name(&mut e.vm, a.data_name);
    let cd = value(&e.vm, e.component_data, vec![
        ("Component", Value::Object(a.template)),
        ("CollisionType", Value::Byte(a.collision)),
        ("MeshSocketName", socket),
        ("HitRegionDefinition", Value::Object(a.hit_region)),
    ]);
    let object = e.object;
    let marker = make(&mut e.vm, object, "DataObject");
    let data = value(&e.vm, e.union, vec![
        ("Name", entry_name),
        ("Type", Value::Byte(a.ty)),
        ("Replication", Value::Byte(2)),
        ("MaxSwitchValue", Value::Byte(1)),
        ("BitsRequiredForMaxSwitchValue", Value::Byte(a.bits)),
        ("DeleteByOwner", Value::Bool(true)),
        ("Int", Value::Int(a.int)),
        ("Float", Value::Float(3.0)),
        ("Object", Value::Object(Some(marker))),
        ("ComponentData", cd),
        ("BodyCompositionIndex", Value::Int(a.index)),
    ]);
    value(&e.vm, e.body_union, vec![
        ("Data", data),
        ("ParentAttachmentIdentifier", parent),
        ("AttachmentBaseInstanceDataIndex", Value::Int(a.base)),
        ("SwitchIdentifier", Value::Byte(a.switch_id)),
    ])
}

/// A pawn (Role 3) whose body class holds `attachments`.
fn pawn_with(e: &mut Env, attachments: Vec<Value>) -> ObjRef {
    let body = make(&mut e.vm, e.body, "BodyClass_Test");
    let comp = value(&e.vm, e.composition, vec![("Attachments", Value::array(attachments))]);
    put(&mut e.vm, body, "BodyComposition", comp);
    let p = make(&mut e.vm, e.pawn, "Pawn_Test");
    put(&mut e.vm, p, "Role", Value::Byte(3));
    put(&mut e.vm, p, "BodyClass", Value::Object(Some(body)));
    p
}

fn run(e: &mut Env, p: ObjRef) -> Result<(), VmError> {
    let w = e.wrapper;
    e.vm.call(w, Some(p), vec![]).map(|_| ())
}

fn entries(e: &Env, p: ObjRef) -> Vec<Value> {
    field(&e.vm, &prop(&e.vm, p, "InstanceDataState"), "Data", "set").unwrap().elements().unwrap().to_vec()
}

fn f(e: &Env, v: &Value, n: &str) -> Value {
    field(&e.vm, v, n, "test").unwrap()
}

fn made_of(e: &Env, entry: &Value) -> ObjRef {
    let Value::Object(Some(made)) = f(e, &f(e, entry, "ComponentData"), "Component") else { panic!("no component") };
    made
}

fn rep(e: &Env, p: ObjRef) -> (Value, Value) {
    let r = prop(&e.vm, p, "ReplicatedInstanceDataState");
    (f(e, &r, "SwitchStateBitField"), f(e, &r, "ActiveSwitchValues"))
}

/// A refusal of the preflight: its kind and text, with no counter moved and no entry written.
fn refused(e: &mut Env, p: ObjRef) -> (Missing, String) {
    let before = Counters::read(&e.vm);
    let err = run(e, p).unwrap_err();
    let VmError::NotYet(n) = err else { panic!("{err:?}") };
    assert!(!ow_core::notyet::is_partial_writes(&n.what), "{}", n.what);
    assert_eq!(Counters::read(&e.vm), before, "write-free: {}", n.what);
    assert!(entries(e, p).is_empty());
    (n.missing, n.what.to_string())
}

/// Attachment 13's shape: a baseless cylinder, identifier `CylinderComponent`, collision 2. The component is made from
/// the template and owned by the pawn; the entry follows 4.1's table (Name, Type, DeleteByOwner, ComponentData with the
/// new component, BodyCompositionIndex copied; the rest zero); collision 2 sets the five component flags and the
/// owner's three; the actor attach sets `Owner` and `Components`; `PostInitBodyComposition` sets `CylinderComponent`
/// and `CollisionComponent`; then the authority flags.
#[test]
fn a_cylinder_is_made_entered_collided_attached_and_posted() {
    let mut e = env();
    let template = make(&mut e.vm, e.cylinder, "Cylinder_Template");
    put(&mut e.vm, template, "CollisionRadius", Value::Float(34.0));
    let a = attachment(&mut e, A { template: Some(template), parent: CYLINDER_ID, collision: 2, index: 14, ..A::default() });
    let p = pawn_with(&mut e, vec![a]);
    run(&mut e, p).unwrap();
    let list = entries(&e, p);
    assert_eq!(list.len(), 1);
    let entry = &list[0];
    assert_eq!(f(&e, entry, "Type"), Value::Byte(COMPONENT));
    assert_eq!(f(&e, entry, "BodyCompositionIndex"), Value::Int(14), "copied from Data");
    assert_eq!(f(&e, entry, "DeleteByOwner"), Value::Bool(true), "copied");
    for (n, zero) in [("Replication", Value::Byte(0)), ("Int", Value::Int(0)), ("Float", Value::Float(0.0)), ("Object", Value::Object(None)), ("BitsRequiredForMaxSwitchValue", Value::Byte(0))] {
        assert_eq!(f(&e, entry, n), zero, "{n} is zero");
    }
    let made = made_of(&e, entry);
    assert_ne!(made, template, "a new component, not the template");
    assert_eq!(e.vm.space.heap.get(made).unwrap().outer, Some(p), "outered to the pawn");
    assert_eq!(prop(&e.vm, made, "CollisionRadius"), Value::Float(34.0), "the template's properties");
    for flag in FLAGS {
        assert_eq!(prop(&e.vm, made, flag), Value::Bool(true), "{flag}");
    }
    for flag in ["bCollideActors", "bBlockActors", "bPushedByEncroachers"] {
        assert_eq!(prop(&e.vm, p, flag), Value::Bool(true), "{flag}");
    }
    assert_eq!(prop(&e.vm, made, "Owner"), Value::Object(Some(p)));
    assert_eq!(prop(&e.vm, p, "Components"), Value::array(vec![Value::Object(Some(made))]), "attached to the pawn");
    assert_eq!(prop(&e.vm, p, "CylinderComponent"), Value::Object(Some(made)));
    assert_eq!(prop(&e.vm, p, "CollisionComponent"), Value::Object(Some(made)));
    assert_eq!(prop(&e.vm, p, "bNetDirty"), Value::Bool(true));
    assert_eq!(prop(&e.vm, p, "bForceNetUpdate"), Value::Bool(true));
}

/// With the body class's `bUseMeshForCollision` set, the cylinder does not become `CollisionComponent`.
#[test]
fn a_mesh_for_collision_body_keeps_collision_component() {
    let mut e = env();
    let template = make(&mut e.vm, e.cylinder, "Cylinder_Template");
    let a = attachment(&mut e, A { template: Some(template), parent: CYLINDER_ID, ..A::default() });
    let p = pawn_with(&mut e, vec![a]);
    let Value::Object(Some(body)) = prop(&e.vm, p, "BodyClass") else { panic!() };
    put(&mut e.vm, body, "bUseMeshForCollision", Value::Bool(true));
    run(&mut e, p).unwrap();
    let made = made_of(&e, &entries(&e, p)[0]);
    assert_eq!(prop(&e.vm, p, "CylinderComponent"), Value::Object(Some(made)));
    assert_eq!(prop(&e.vm, p, "CollisionComponent"), Value::Object(None));
}

/// The collision table for types 3, 1 and 8: what each sets and clears.
#[test]
fn the_collision_table_sets_and_clears_by_type() {
    for (ty, want, owner) in [
        (3u8, [true, true, false, false, true], [true, true, false]),
        (1, [false, false, false, true, true], [false, false, false]),
        (8, [false, true, true, true, true], [false, false, false]),
    ] {
        let mut e = env();
        let template = make(&mut e.vm, e.cylinder, "Cylinder_Template");
        for flag in FLAGS {
            put(&mut e.vm, template, flag, Value::Bool(true));
        }
        let a = attachment(&mut e, A { template: Some(template), collision: ty, ..A::default() });
        let p = pawn_with(&mut e, vec![a]);
        run(&mut e, p).unwrap();
        let made = made_of(&e, &entries(&e, p)[0]);
        for (flag, w) in FLAGS.iter().zip(want) {
            assert_eq!(prop(&e.vm, made, flag), Value::Bool(w), "type {ty}: {flag}");
        }
        for (flag, w) in ["bCollideActors", "bBlockActors", "bPushedByEncroachers"].iter().zip(owner) {
            assert_eq!(prop(&e.vm, p, flag), Value::Bool(w), "type {ty}: {flag}");
        }
    }
}

/// A switch entry is the whole `Data` with `Float` 0 (`Object`, `Replication` and the index copied). A component under
/// it whose identifier differs from the switch's `Int` is not made; one that matches is made, with the switch's parent
/// (the pawn) as its parent. The packing sets the switch's bit.
#[test]
fn a_switch_is_copied_and_gates_its_components() {
    let mut e = env();
    let t1 = make(&mut e.vm, e.cylinder, "Cylinder_T1");
    let t2 = make(&mut e.vm, e.cylinder, "Cylinder_T2");
    let s = attachment(&mut e, A { ty: SWITCH, int: 1, index: 1, ..A::default() });
    let waits = attachment(&mut e, A { base: 0, template: Some(t1), switch_id: 2, index: 2, ..A::default() });
    let goes = attachment(&mut e, A { base: 0, template: Some(t2), switch_id: 1, index: 3, ..A::default() });
    let p = pawn_with(&mut e, vec![s.clone(), waits, goes]);
    run(&mut e, p).unwrap();
    let list = entries(&e, p);
    assert_eq!(list.len(), 2, "the switch and the matching component");
    let mut want = f(&e, &s, "Data");
    set_field(&e.vm, &mut want, "Float", Value::Float(0.0), "t").unwrap();
    assert_eq!(list[0], want, "the whole Data, Float 0");
    assert_eq!(f(&e, &list[1], "BodyCompositionIndex"), Value::Int(3));
    let made = made_of(&e, &list[1]);
    assert_eq!(prop(&e.vm, p, "Components"), Value::array(vec![Value::Object(Some(made))]), "the switch's parent: the pawn");
    assert_eq!(rep(&e, p), (Value::Int(1), Value::Int(1)));
}

/// The packing (section 5): a switch that is not made (removed) uses up its position without a bit or an offset; a width
/// of 0 packs its value at the current offset without moving it; values are not masked to their width.
#[test]
fn the_switch_packing_follows_the_note() {
    let mut e = env();
    let removed = attachment(&mut e, A { ty: SWITCH, int: 1, index: 1, ..A::default() });
    let zero_width = attachment(&mut e, A { ty: SWITCH, int: 4, bits: 0, index: 2, ..A::default() });
    let wide = attachment(&mut e, A { ty: SWITCH, int: 3, bits: 1, index: 3, ..A::default() });
    let next = attachment(&mut e, A { ty: SWITCH, int: 1, bits: 2, index: 4, ..A::default() });
    let p = pawn_with(&mut e, vec![removed, zero_width, wide, next]);
    let mut r = prop(&e.vm, p, "ReplicatedInstanceDataState");
    set_field(&e.vm, &mut r, "RemovedBodyCompositionPartsBitField", Value::Int(1), "t").unwrap();
    put(&mut e.vm, p, "ReplicatedInstanceDataState", r);
    run(&mut e, p).unwrap();
    assert_eq!(entries(&e, p).len(), 3, "the removed switch makes no entry");
    // Positions 1, 2, 3 set (0 used up). Values: 4 at offset 0 (width 0, offset stays 0), 3 at offset 0 (width 1, not
    // masked), 1 at offset 1 (width 2): 4 | 3 | 2 = 7.
    assert_eq!(rep(&e, p), (Value::Int(0b1110), Value::Int(7)));
}

/// More than 32 switches: the packing stops after the 32nd.
#[test]
fn the_switch_packing_stops_after_32() {
    let mut e = env();
    let mut list = Vec::new();
    for i in 0..34 {
        list.push(attachment(&mut e, A { ty: SWITCH, bits: 0, index: i + 1, ..A::default() }));
    }
    let p = pawn_with(&mut e, list);
    run(&mut e, p).unwrap();
    assert_eq!(entries(&e, p).len(), 34);
    assert_eq!(rep(&e, p), (Value::Int(-1), Value::Int(0)));
}

/// Ruling 2: a custom head entry has Name, Type and the index, every other field zero (Replication and Object
/// included); it counts as not made, so a component based on it is not made.
#[test]
fn a_custom_head_entry_is_appended_and_counts_as_not_made() {
    let mut e = env();
    let t = make(&mut e.vm, e.cylinder, "T");
    let h = attachment(&mut e, A { ty: HEAD, data_name: "CustomHead", index: 1, ..A::default() });
    let child = attachment(&mut e, A { base: 0, template: Some(t), index: 2, ..A::default() });
    let p = pawn_with(&mut e, vec![h, child]);
    run(&mut e, p).unwrap();
    let list = entries(&e, p);
    assert_eq!(list.len(), 1);
    assert_eq!(f(&e, &list[0], "Type"), Value::Byte(HEAD));
    assert_eq!(f(&e, &list[0], "BodyCompositionIndex"), Value::Int(1));
    assert_eq!(f(&e, &list[0], "Replication"), Value::Byte(0));
    assert_eq!(f(&e, &list[0], "Object"), Value::Object(None));
    assert_eq!(f(&e, &list[0], "Float"), Value::Float(0.0));
    assert_eq!(prop(&e.vm, p, "HeadMesh"), Value::Object(None), "no component, no HeadMesh");
}

/// A skeletal parent with a socket but no mesh asset answers false: the owner retry attaches the child to the pawn.
/// A child named `CustomHead` becomes `HeadMesh`; a mesh component gets the pawn's `LightEnvironment`.
#[test]
fn the_owner_retry_head_mesh_and_light_environment() {
    let mut e = env();
    let skel = make(&mut e.vm, e.skeletal, "Skel_Template");
    let head = make(&mut e.vm, e.skeletal, "Head_Template");
    let base = attachment(&mut e, A { template: Some(skel), index: 1, ..A::default() });
    let h = attachment(&mut e, A { base: 0, template: Some(head), socket: "Root", data_name: CUSTOM_HEAD_NAME, index: 2, ..A::default() });
    let p = pawn_with(&mut e, vec![base, h]);
    let object = e.object;
    let light = make(&mut e.vm, object, "Light");
    put(&mut e.vm, p, "LightEnvironment", Value::Object(Some(light)));
    run(&mut e, p).unwrap();
    let list = entries(&e, p);
    let (m0, m1) = (made_of(&e, &list[0]), made_of(&e, &list[1]));
    assert_eq!(prop(&e.vm, p, "Components"), Value::array(vec![Value::Object(Some(m0)), Value::Object(Some(m1))]));
    assert_eq!(prop(&e.vm, p, "HeadMesh"), Value::Object(Some(m1)));
    assert_eq!(prop(&e.vm, m0, "LightEnvironment"), Value::Object(Some(light)));
    assert_eq!(prop(&e.vm, m1, "LightEnvironment"), Value::Object(Some(light)));
}

/// A component attachment with no template makes nothing.
#[test]
fn a_missing_template_makes_nothing() {
    let mut e = env();
    let a = attachment(&mut e, A::default());
    let p = pawn_with(&mut e, vec![a]);
    run(&mut e, p).unwrap();
    assert!(entries(&e, p).is_empty());
}

/// Maya's order: attachment 0 (`Mesh`, a skeletal component) is the animation-node `LayerAbsent`, found before any
/// write, so attachment 13's cylinder (later in the list) is not written either.
#[test]
fn the_new_mesh_s_animation_nodes_are_layer_absent_and_write_free() {
    let mut e = env();
    let skel = make(&mut e.vm, e.skeletal, "Skel_Template");
    let cyl = make(&mut e.vm, e.cylinder, "Cyl_Template");
    let a0 = attachment(&mut e, A { template: Some(skel), parent: MESH_ID, index: 1, ..A::default() });
    let a1 = attachment(&mut e, A { template: Some(cyl), parent: CYLINDER_ID, collision: 2, index: 2, ..A::default() });
    let p = pawn_with(&mut e, vec![a1.clone(), a0]);
    // The cylinder first in the list still waits for the whole preflight.
    let (missing, what) = refused(&mut e, p);
    assert!(matches!(missing, Missing::LayerAbsent(l) if l == ANIM_LAYER), "{missing:?}");
    assert_eq!(what, ANIM_NODES);
    assert_eq!(prop(&e.vm, p, "CylinderComponent"), Value::Object(None));
}

/// The preflight's other refusals, each its named kind and text, write-free.
#[test]
fn the_named_refusals_are_write_free() {
    for case in ["rebuild", "client", "template", "hit", "actor socket", "socket tables", "parent link", "other target"] {
        let mut e = env();
        let skel = make(&mut e.vm, e.skeletal, "Skel_Template");
        let cyl = make(&mut e.vm, e.cylinder, "Cyl_Template");
        let object = e.object;
        let thing = make(&mut e.vm, object, "Thing");
        let (list, want_kind, want): (Vec<Value>, &str, String) = match case {
            "rebuild" | "client" => (vec![attachment(&mut e, A { template: Some(cyl), ..A::default() })], "NotRead", String::new()),
            "template" => (vec![attachment(&mut e, A { ty: ACTOR_TEMPLATE, ..A::default() })], "NotRead", OBJECT_TEMPLATES.into()),
            "hit" => (vec![attachment(&mut e, A { template: Some(cyl), hit_region: Some(thing), ..A::default() })], "NotRead", HIT_REGION.into()),
            "actor socket" => (vec![attachment(&mut e, A { template: Some(cyl), socket: "Hips", ..A::default() })], "NotRead", ACTOR_SOCKETS.into()),
            "socket tables" => {
                put(&mut e.vm, skel, "SkeletalMesh", Value::Object(Some(thing)));
                let a0 = attachment(&mut e, A { template: Some(skel), ..A::default() });
                let a1 = attachment(&mut e, A { base: 0, template: Some(cyl), socket: "Spine3", ..A::default() });
                (vec![a0, a1], "LayerAbsent", socket_tables(&crate::world::pawn_meshes::handle_path(&e.vm, thing)))
            }
            "parent link" => {
                let head = make(&mut e.vm, e.skeletal, "Head_Template");
                let a0 = attachment(&mut e, A { template: Some(skel), ..A::default() });
                let a1 = attachment(&mut e, A { base: 0, template: Some(head), parent: PARENT_ID, ..A::default() });
                (vec![a0, a1], "NotRead", PARENT_LINK.into())
            }
            _ => {
                let a0 = attachment(&mut e, A { template: Some(cyl), ..A::default() });
                let a1 = attachment(&mut e, A { base: 0, template: Some(skel), ..A::default() });
                (vec![a0, a1], "NotRead", other_target(CYLINDER))
            }
        };
        let p = pawn_with(&mut e, list);
        let want = match case {
            "rebuild" => {
                let mut set = prop(&e.vm, p, "InstanceDataState");
                let union = e.union;
                let zero = e.vm.space.classes.struct_defaults(union).unwrap();
                set_field(&e.vm, &mut set, "Data", Value::array(vec![zero]), "t").unwrap();
                put(&mut e.vm, p, "InstanceDataState", set);
                REBUILD.to_string()
            }
            "client" => {
                put(&mut e.vm, p, "Role", Value::Byte(2));
                let mut r = prop(&e.vm, p, "ReplicatedInstanceDataState");
                set_field(&e.vm, &mut r, "bNeedToApplyThisState", Value::Bool(true), "t").unwrap();
                put(&mut e.vm, p, "ReplicatedInstanceDataState", r);
                CLIENT_APPLY.to_string()
            }
            _ => want,
        };
        let before = Counters::read(&e.vm);
        let err = run(&mut e, p).unwrap_err();
        let VmError::NotYet(n) = &err else { panic!("{case}: {err:?}") };
        match want_kind {
            "LayerAbsent" => assert!(matches!(&n.missing, Missing::LayerAbsent(l) if *l == MESH_LAYER), "{case}: {n:?}"),
            _ => assert_eq!(n.missing, Missing::NotRead, "{case}"),
        }
        assert_eq!(n.what, want, "{case}");
        assert_eq!(Counters::read(&e.vm), before, "{case}: write-free");
    }
}

/// The tracker: a `NotYet` after a counter moved is marked, kind and record kept; with no write since, it is plain.
#[test]
fn a_refusal_after_a_write_is_marked() {
    let mut e = env();
    let before = Counters::read(&e.vm);
    let plain = after_writes(&e.vm, before, NotYet::not_read("x", RECORD).into());
    let VmError::NotYet(n) = &plain else { panic!() };
    assert!(!ow_core::notyet::is_partial_writes(&n.what), "no write, no prefix");
    let object = e.object;
    make(&mut e.vm, object, "Written");
    let marked = after_writes(&e.vm, before, NotYet::layer_absent("y", "z", RECORD).into());
    let VmError::NotYet(n) = &marked else { panic!() };
    assert!(ow_core::notyet::is_partial_writes(&n.what), "a write since: marked");
    assert!(matches!(n.missing, Missing::LayerAbsent(_)), "kind kept");
    assert_eq!(n.record, RECORD);
}

/// The script native, for callers outside the pawn wrapper: `NotYet { NotPorted }`.
#[test]
fn the_script_native_is_not_ported() {
    let mut e = env();
    let helper = e.vm.space.classes.by_path(crate::world::pawn_meshes::HELPER).unwrap();
    let d = FunctionDecl::native(
        ObjectPath::parse(FUNCTION).unwrap(),
        "InstanceDataHelper.InitializeBodyComposition(object,struct,struct,struct,byte)",
    )
    .with_owner(helper)
    .with_static(true);
    let fun = e.vm.declare_function(d).unwrap();
    register(&mut e.vm).unwrap();
    let err = e.vm.call(fun, None, vec![]).unwrap_err();
    let VmError::NotYet(n) = &err else { panic!("{err:?}") };
    assert_eq!(n.missing, Missing::NotPorted);
    assert_eq!(n.what, SCRIPT_CALL);
}
