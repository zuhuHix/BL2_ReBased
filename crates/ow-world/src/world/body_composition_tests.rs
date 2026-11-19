//! `InitializeBodyComposition` on invented classes and structs (AGENTS.md rule 9: invented values; the class, struct,
//! field and enum names are the packages'), through the pawn wrapper (`InitializePawnMeshes`) and the script native.
//! Graded by the `VmError` variant and the `NotYet` kind.

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
    plain_component: ClassId,
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
    let component = class(&mut vm, "Engine.ActorComponent", object, &[]);
    let plain_component = class(&mut vm, "Engine.CylinderComponent", component, &[("CollisionRadius", PropKind::Float)]);
    let skeletal = class(&mut vm, SKELETAL, component, &[("SkeletalMesh", o.clone())]);
    let actor = class(&mut vm, "Engine.Actor", object, &[
        ("Role", PropKind::Byte { enum_: None }),
        ("bNetDirty", PropKind::Bool),
        ("bForceNetUpdate", PropKind::Bool),
        ("Components", PropKind::Array(Box::new(o.clone()))),
    ]);
    let body = class(&mut vm, "WillowGame.BodyClassDefinition", object, &[
        ("BodyComposition", PropKind::Struct(composition)),
        ("UseRootMotion", PropKind::Bool),
        ("UseRootRotation", PropKind::Bool),
    ]);
    let pawn = class(&mut vm, crate::world::pawn_meshes::WILLOW_PAWN, actor, &[
        ("BodyClass", PropKind::Object { class: body }),
        ("InstanceDataState", PropKind::Struct(set)),
        ("ReplicatedInstanceDataState", PropKind::Struct(rep)),
    ]);
    let path = "WillowGame.WillowPawn.InitializePawnMeshes";
    let d = FunctionDecl::native(ObjectPath::parse(path).unwrap(), "WillowPawn.InitializePawnMeshes()").with_owner(pawn);
    let wrapper = vm.declare_function(d).unwrap();
    crate::world::pawn_meshes::register(&mut vm).unwrap();
    Env { vm, dir, object, pawn, body, plain_component, skeletal, union, body_union, component_data, composition, wrapper }
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

/// One attachment: its type, base, socket, parent identifier, template, collision type, switch identifier and Int.
struct A<'a> {
    ty: u8,
    base: i32,
    socket: &'a str,
    parent: &'a str,
    template: Option<ObjRef>,
    collision: u8,
    switch_id: u8,
    int: i32,
}

impl Default for A<'_> {
    fn default() -> Self {
        A { ty: COMPONENT, base: -1, socket: "None", parent: "None", template: None, collision: 0, switch_id: 0, int: 0 }
    }
}

fn attachment(e: &mut Env, a: A<'_>) -> Value {
    let socket = name(&mut e.vm, a.socket);
    let parent = name(&mut e.vm, a.parent);
    let entry_name = name(&mut e.vm, "Part");
    let cd = value(&e.vm, e.component_data, vec![
        ("Component", Value::Object(a.template)),
        ("CollisionType", Value::Byte(a.collision)),
        ("MeshSocketName", socket),
    ]);
    let data = value(&e.vm, e.union, vec![
        ("Name", entry_name),
        ("Type", Value::Byte(a.ty)),
        ("BitsRequiredForMaxSwitchValue", Value::Byte(1)),
        ("Int", Value::Int(a.int)),
        ("Float", Value::Float(3.0)),
        ("ComponentData", cd),
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

fn entries(e: &Env, p: ObjRef) -> Vec<Value> {
    field(&e.vm, &prop(&e.vm, p, "InstanceDataState"), "Data", "set").unwrap().elements().unwrap().to_vec()
}

fn f(e: &Env, v: &Value, n: &str) -> Value {
    field(&e.vm, v, n, "test").unwrap()
}

/// A component attachment with no base and no socket: a new component from the template, owned by the pawn, its entry
/// (type 5, index 1, the new component), and the attach (the pawn's Components); then the authority packing.
#[test]
fn a_component_is_made_entered_and_attached_to_the_pawn() {
    let mut e = env();
    let template = make(&mut e.vm, e.plain_component, "Cylinder_Template");
    put(&mut e.vm, template, "CollisionRadius", Value::Float(34.0));
    let a = attachment(&mut e, A { template: Some(template), ..A::default() });
    let p = pawn_with(&mut e, vec![a]);
    let w = e.wrapper;
    e.vm.call(w, Some(p), vec![]).unwrap();
    let list = entries(&e, p);
    assert_eq!(list.len(), 1);
    assert_eq!(f(&e, &list[0], "Type"), Value::Byte(COMPONENT));
    assert_eq!(f(&e, &list[0], "BodyCompositionIndex"), Value::Int(1));
    let Value::Object(Some(made)) = f(&e, &f(&e, &list[0], "ComponentData"), "Component") else { panic!() };
    assert_ne!(made, template, "a new component, not the template");
    assert_eq!(e.vm.space.heap.get(made).unwrap().outer, Some(p), "owned by the pawn");
    assert_eq!(prop(&e.vm, made, "CollisionRadius"), Value::Float(34.0), "the template's properties");
    assert_eq!(prop(&e.vm, p, "Components"), Value::array(vec![Value::Object(Some(made))]), "attached to the pawn");
    assert_eq!(prop(&e.vm, p, "bNetDirty"), Value::Bool(true));
    assert_eq!(prop(&e.vm, p, "bForceNetUpdate"), Value::Bool(true));
}

/// A switch entry (a copy of Data with Float 0 and its index) and a component under it that is not made (the switch's
/// Int is 0, the component's identifier 1); the authority packing sets the switch's bit.
#[test]
fn a_switch_is_entered_and_its_component_waits() {
    let mut e = env();
    let template = make(&mut e.vm, e.plain_component, "Cylinder_T2");
    let s = attachment(&mut e, A { ty: SWITCH, ..A::default() });
    let c = attachment(&mut e, A { base: 0, template: Some(template), switch_id: 1, ..A::default() });
    let p = pawn_with(&mut e, vec![s, c]);
    let w = e.wrapper;
    e.vm.call(w, Some(p), vec![]).unwrap();
    let list = entries(&e, p);
    assert_eq!(list.len(), 1, "the switched component is not made");
    assert_eq!(f(&e, &list[0], "Type"), Value::Byte(SWITCH));
    assert_eq!(f(&e, &list[0], "Float"), Value::Float(0.0));
    assert_eq!(f(&e, &list[0], "BodyCompositionIndex"), Value::Int(1));
    let rep = prop(&e.vm, p, "ReplicatedInstanceDataState");
    assert_eq!(f(&e, &rep, "SwitchStateBitField"), Value::Int(1));
    assert_eq!(f(&e, &rep, "ActiveSwitchValues"), Value::Int(0));
    assert_eq!(prop(&e.vm, p, "Components"), Value::array(vec![]), "nothing attached");
}

/// Ruling 3: a custom head entry is appended (Name, Replication, Type, index) and nothing is applied.
#[test]
fn a_custom_head_entry_is_appended() {
    let mut e = env();
    let h = attachment(&mut e, A { ty: HEAD, ..A::default() });
    let p = pawn_with(&mut e, vec![h]);
    let w = e.wrapper;
    e.vm.call(w, Some(p), vec![]).unwrap();
    let list = entries(&e, p);
    assert_eq!(list.len(), 1);
    assert_eq!(f(&e, &list[0], "Type"), Value::Byte(HEAD));
    assert_eq!(f(&e, &list[0], "BodyCompositionIndex"), Value::Int(1));
    assert_eq!(f(&e, &list[0], "Float"), Value::Float(0.0), "only the four fields are copied");
    assert_eq!(prop(&e.vm, p, "Components"), Value::array(vec![]));
}

/// A component attachment with no template makes nothing.
#[test]
fn a_missing_template_makes_nothing() {
    let mut e = env();
    let a = attachment(&mut e, A::default());
    let p = pawn_with(&mut e, vec![a]);
    let w = e.wrapper;
    e.vm.call(w, Some(p), vec![]).unwrap();
    assert!(entries(&e, p).is_empty());
}

/// A component with a socket under a skeletal parent is the socket-table `LayerAbsent`, before any write: the counters
/// do not move and there is no prefix.
#[test]
fn a_socket_under_a_skeletal_parent_is_layer_absent_and_write_free() {
    let mut e = env();
    let mesh_template = make(&mut e.vm, e.skeletal, "Skel_Template");
    let gear = make(&mut e.vm, e.plain_component, "Gear_Template");
    let a0 = attachment(&mut e, A { template: Some(mesh_template), ..A::default() });
    let a1 = attachment(&mut e, A { base: 0, socket: "Spine3", template: Some(gear), ..A::default() });
    let p = pawn_with(&mut e, vec![a0, a1]);
    let before = Counters::read(&e.vm);
    let w = e.wrapper;
    let err = e.vm.call(w, Some(p), vec![]).unwrap_err();
    let VmError::NotYet(n) = &err else { panic!("{err:?}") };
    assert!(matches!(n.missing, Missing::LayerAbsent(l) if l == MESH_LAYER), "{n:?}");
    assert_eq!(n.what, socket_tables("None"));
    assert!(!ow_core::notyet::is_partial_writes(&n.what));
    assert_eq!(Counters::read(&e.vm), before, "write-free");
    assert!(entries(&e, p).is_empty());
}

/// The special parent identifier (ruling 2), a collision type other than 0 and an actor socket: each its named
/// `NotRead`, write-free.
#[test]
fn the_named_refusals_are_write_free() {
    for (case, want) in [("special", SPECIAL_REFRESH.to_string()), ("collision", collision_type(2)), ("socket", ACTOR_SOCKETS.to_string())] {
        let mut e = env();
        let t = make(&mut e.vm, e.plain_component, "T");
        let a = match case {
            "special" => A { parent: SPECIAL_PARENT, template: Some(t), ..A::default() },
            "collision" => A { collision: 2, template: Some(t), ..A::default() },
            _ => A { socket: "Hips", template: Some(t), ..A::default() },
        };
        let a = attachment(&mut e, a);
        let p = pawn_with(&mut e, vec![a]);
        let before = Counters::read(&e.vm);
        let w = e.wrapper;
        let err = e.vm.call(w, Some(p), vec![]).unwrap_err();
        let VmError::NotYet(n) = &err else { panic!("{case}: {err:?}") };
        assert_eq!(n.missing, Missing::NotRead, "{case}");
        assert_eq!(n.what, want, "{case}");
        assert_eq!(Counters::read(&e.vm), before, "{case}: write-free");
    }
}

/// An actor-template attachment is the note's `NotRead`.
#[test]
fn object_and_template_attachments_are_not_read() {
    let mut e = env();
    let a = attachment(&mut e, A { ty: ACTOR_TEMPLATE, ..A::default() });
    let p = pawn_with(&mut e, vec![a]);
    let w = e.wrapper;
    let err = e.vm.call(w, Some(p), vec![]).unwrap_err();
    let VmError::NotYet(n) = &err else { panic!("{err:?}") };
    assert_eq!(n.what, OBJECT_TEMPLATES);
}

/// The tracker: a `NotYet` after a counter moved is marked, kind and record kept; with no write since, it is plain.
#[test]
fn a_refusal_after_a_write_is_marked() {
    let mut e = env();
    let before = Counters::read(&e.vm);
    let plain = after_writes(&e.vm, before, NotYet::not_read("x", RECORD).into());
    let VmError::NotYet(n) = &plain else { panic!() };
    assert!(!ow_core::notyet::is_partial_writes(&n.what), "no write, no prefix");
    let o = make(&mut e.vm, e.object, "Written");
    let _ = o;
    let marked = after_writes(&e.vm, before, NotYet::layer_absent("y", "z", RECORD).into());
    let VmError::NotYet(n) = &marked else { panic!() };
    assert!(ow_core::notyet::is_partial_writes(&n.what), "a write since: marked");
    assert!(matches!(n.missing, Missing::LayerAbsent(_)), "kind kept");
    assert_eq!(n.record, RECORD);
}

/// The script native writes its two structs to its out arguments.
#[test]
fn the_script_native_writes_its_out_arguments() {
    let mut e = env();
    let helper = e.vm.space.classes.by_path(crate::world::pawn_meshes::HELPER).unwrap();
    let o = PropKind::Object { class: e.object };
    let set_s = e.vm.space.classes.struct_by_path("Engine.IInstanceData.InstanceDataSet").unwrap();
    let rep_s = e.vm.space.classes.struct_by_path("Engine.IInstanceData.ReplicatedInstanceDataState").unwrap();
    let params = vec![
        decl(&mut e.vm, "Owner", o).with_flags(PropertyDecl::PARAM),
        decl(&mut e.vm, "BodyComposition", PropKind::Struct(e.composition)).with_flags(PropertyDecl::PARAM),
        decl(&mut e.vm, "InstanceDataSet", PropKind::Struct(set_s)).with_flags(PropertyDecl::PARAM | PropertyDecl::OUT),
        decl(&mut e.vm, "ReplicatedInstanceDataState", PropKind::Struct(rep_s)).with_flags(PropertyDecl::PARAM | PropertyDecl::OUT),
        decl(&mut e.vm, "Mode", PropKind::Byte { enum_: None }).with_flags(PropertyDecl::PARAM | PropertyDecl::OPTIONAL),
    ];
    let d = FunctionDecl::native(ObjectPath::parse(FUNCTION).unwrap(), "InstanceDataHelper.InitializeBodyComposition(object,struct,struct,struct,byte)")
        .with_owner(helper)
        .with_static(true)
        .with_params(params);
    let fun = e.vm.declare_function(d).unwrap();
    register(&mut e.vm).unwrap();
    let h = attachment(&mut e, A { ty: HEAD, ..A::default() });
    let p = pawn_with(&mut e, vec![]);
    let comp = value(&e.vm, e.composition, vec![("Attachments", Value::array(vec![h]))]);
    let set = e.vm.space.classes.struct_defaults(set_s).unwrap();
    let rep = e.vm.space.classes.struct_defaults(rep_s).unwrap();
    let r = e.vm.call(fun, None, vec![Value::Object(Some(p)), comp, set, rep, Value::Byte(0)]).unwrap();
    let out_set = &r.outs[2];
    assert_eq!(f(&e, out_set, "Data").elements().unwrap().len(), 1, "the entry, through the out argument");
    assert!(entries(&e, p).is_empty(), "the pawn's own field is not the out argument");
}
