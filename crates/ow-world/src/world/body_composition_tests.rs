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
use ow_package::testkit::reflect::{toy_array_of, toy_struct};
use ow_package::testkit::script::{FunctionSpec, ScriptToy};
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
    /// The rendering hooks' classes (lane IMPL-BODYCOMP-RENDER-HOOKS): a plain `Engine.ActorComponent`, an
    /// `Engine.PlayerController`, a package component that implements `WillowGame.IGearLikenessConsumer` (by the
    /// structural rule: these synthetic class bodies do not decode), and a cylinder that declares the deferred
    /// reattach's fields (the package's `PrimitiveComponent` does not, so `SetOwnerNoSee`'s planner refuses it).
    actor_component: ClassId,
    player_controller: ClassId,
    likeness: ClassId,
    reattach_cylinder: ClassId,
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
    env_with(true)
}

/// The fixture; with `reattach`, the pawn class has the canonical `WillowPawn.ReattachWeapons` (a hand-declared class
/// gets it through `Vm::declare_method`).
fn env_with(reattach: bool) -> Env {
    env_opts(reattach, true, false)
}

/// [`env_with`]; without `head_mesh` the pawn class declares no `HeadMesh`, and with `controller_int` its
/// `Controller` is declared as an int (the rendering hooks' malformed-field tests).
fn env_opts(reattach: bool, head_mesh: bool, controller_int: bool) -> Env {
    // The attach's tree-copy check (lane IMPL-COPY-ANIM-TREE) comes before the lookups; this fixture's tree has no
    // copy fields, so these tests skip it (the check has its own tests in anim_tree_copy_tests.rs).
    crate::world::anim_tree_copy::skip_attach_check_for_tests(true);
    let dir = std::env::temp_dir().join(format!("ow-world-bodycomp-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let core = {
        let mut t = ScriptToy::new(&[]);
        t.class("Object", 0);
        t.build()
    };
    let engine = {
        let mut t = ScriptToy::with_imports(&[
            "Class",
            "ObjectProperty",
            "BoolProperty",
            "FloatProperty",
            "ScriptStruct",
            "NameProperty",
            "IntProperty",
            "ArrayProperty",
            "StructProperty",
            "ByteProperty",
        ]);
        let core = t.p.add_import("Core", "Package", 0, "Core");
        let object = t.p.add_import("Core", "Class", core, "Object");
        let component = t.class("ActorComponent", object);
        t.prop("Object", component, "Owner", 0, object);
        let primitive = t.class("PrimitiveComponent", component);
        for flag in FLAGS {
            t.prop("Bool", primitive, flag, 0, 0);
        }
        t.prop("Object", primitive, "LightEnvironment", 0, object);
        t.prop("Float", primitive, "Scale", 0, 0);
        // The rendering hooks' flags (NATIVE_BODYCOMP_RENDER_HOOKS.md: hook 2's and hook 3's).
        for flag in ["bOwnerNoSee", "bAcceptsLights"] {
            t.prop("Bool", primitive, flag, 0, 0);
        }
        let controller = t.class("Controller", object);
        let player_controller = t.class("PlayerController", controller);
        t.prop("Object", player_controller, "Player", 0, object);
        let cylinder = t.class("CylinderComponent", primitive);
        t.prop("Float", cylinder, "CollisionRadius", 0, 0);
        let mesh = t.class("MeshComponent", primitive);
        let skeletal = t.class("SkeletalMeshComponent", mesh);
        t.prop("Object", skeletal, "SkeletalMesh", 0, object);
        t.prop("Bool", skeletal, "bSkipAllUpdateWhenPhysicsAsleep", 0, 0);
        // The animation classes the lookups read (NATIVE_ANIM_NODE_LOOKUP.md), with only the fields they read.
        let node = t.class("AnimNode", object);
        t.prop("Name", node, "NodeName", 0, 0);
        t.prop("Int", node, "TickArrayIndex", 0, 0);
        // The fields the attach's tree copy and initialisation read (review r1 of #618, item 3), so a test can run
        // the copy's own check here.
        toy_array_of(&mut t, node, "ParentNodes", "Object", node);
        t.prop("Object", node, "SkelComponent", 0, skeletal);
        for name in ["NodeTickTag", "NodeInitTag"] {
            t.prop("Int", node, name, 0, 0);
        }
        for name in ["bRelevant", "bJustBecameRelevant", "bCallScriptEventOnInit"] {
            t.prop("Bool", node, name, 0, 0);
        }
        t.prop("Float", node, "NodeTotalWeight", 0, 0);
        let blend = t.class("AnimNodeBlendBase", node);
        let child = toy_struct(&mut t, blend, "AnimBlendChild", None);
        t.prop("Object", child, "Anim", 0, node);
        t.prop("Float", child, "Weight", 0, 0);
        t.prop("Bool", child, "bIsAdditive", 0, 0);
        t.prop("Float", child, "AdditiveWeightScale", 0, 0);
        toy_array_of(&mut t, blend, "Children", "Struct", child);
        t.class("AnimNodeBlendPerBone", blend);
        let slot = t.class("AnimNodeSlot", blend);
        // The slot's and the aim-offset node's fields the copy's per-node initialisation reads (review r2 of #618).
        toy_array_of(&mut t, slot, "TargetWeight", "Float", 0);
        t.prop("Object", slot, "SynchNode", 0, node);
        let aim = t.class("AnimNodeAimOffset", blend);
        t.prop("Object", aim, "TemplateNode", 0, aim);
        toy_array_of(&mut t, aim, "Profiles", "Object", object);
        let control = t.class("SkelControlBase", object);
        t.prop("Name", control, "ControlName", 0, 0);
        t.prop("Object", control, "NextControl", 0, control);
        t.prop("Int", control, "ControlTickTag", 0, 0);
        t.prop("Bool", control, "bPostPhysicsController", 0, 0);
        t.class("SkelControlLeftHandGripWeapon", control);
        let tree = t.class("AnimTree", blend);
        let head = toy_struct(&mut t, tree, "SkelControlListHead", None);
        t.prop("Object", head, "ControlHead", 0, control);
        toy_array_of(&mut t, tree, "SkelControlLists", "Struct", head);
        t.prop("Object", skeletal, "AnimTreeTemplate", 0, tree);
        // The copy's tree and component fields (review r1 of #618, item 3).
        t.prop("Object", tree, "AnimTreeTemplate", 0, tree);
        for name in ["bParentNodeArrayBuilt", "bRebuildAnimTickArray"] {
            t.prop("Bool", tree, name, 0, 0);
        }
        let group = toy_struct(&mut t, tree, "AnimGroup", None);
        toy_array_of(&mut t, group, "SeqNodes", "Object", node);
        t.prop("Object", group, "SynchMaster", 0, node);
        t.prop("Object", group, "NotifyMaster", 0, node);
        toy_array_of(&mut t, tree, "AnimGroups", "Struct", group);
        toy_array_of(&mut t, tree, "RootMorphNodes", "Object", node);
        toy_array_of(&mut t, tree, "AnimTickArray", "Object", node);
        t.prop("Object", skeletal, "Animations", 0, node);
        // The initialisation's own gate (review r1 of #631, item 1).
        t.prop("Object", skeletal, "ParentAnimComponent", 0, skeletal);
        toy_array_of(&mut t, skeletal, "AnimTickArray", "Object", node);
        for name in ["TickTag", "InitTag"] {
            t.prop("Int", skeletal, name, 0, 0);
        }
        for name in ["bAnimTreeInitialised", "bUpdateComposeSkeletonPasses"] {
            t.prop("Bool", skeletal, name, 0, 0);
        }
        toy_array_of(&mut t, skeletal, "MorphSets", "Object", object);
        toy_array_of(&mut t, skeletal, "SkelControlIndex", "Byte", 0);
        toy_array_of(&mut t, skeletal, "PostPhysSkelControlIndex", "Byte", 0);
        toy_array_of(&mut t, skeletal, "SkelControlTickArray", "Object", control);
        toy_array_of(&mut t, skeletal, "AnimTickRelevancyArray", "Int", 0);
        toy_array_of(&mut t, skeletal, "AnimTickWeightsArray", "Float", 0);
        let holder = t.class("ToyTemplateHolder", object);
        for name in ["Skel", "Cyl", "Cycle", "SkelTree"] {
            t.prop("Object", holder, name, 0, object);
        }
        t.build()
    };
    let templates = {
        let mut t = ScriptToy::with_imports(&["Class"]);
        let engine = t.p.add_import("Core", "Package", 0, "Engine");
        let skeletal = t.p.add_import("Core", "Class", engine, "SkeletalMeshComponent");
        let cylinder = t.p.add_import("Core", "Class", engine, "CylinderComponent");
        let holder = t.p.add_import("Core", "Class", engine, "ToyTemplateHolder");
        let anim_tree = t.p.add_import("Core", "Class", engine, "AnimTree");
        let component = |t: &mut ScriptToy, class: i32, name: &str| {
            let mut payload = ow_package::testkit::ints(&[0, 0]);
            payload.extend_from_slice(&t.none);
            t.p.add_export(class, name, payload)
        };
        let skel = component(&mut t, skeletal, "SkelTemplate");
        let cyl = component(&mut t, cylinder, "CylTemplate");
        // An export whose archetype is itself: the Off store's load refuses it (archetype chain cycle).
        let cycle_ref = t.p.next_export();
        let mut cycle_payload = ow_package::testkit::ints(&[0, 0]);
        cycle_payload.extend_from_slice(&t.none);
        let mut cycle = ow_package::testkit::Export::new(cylinder, "CycleTemplate", cycle_payload);
        cycle.archetype = cycle_ref;
        t.p.add_export_with(cycle);
        // A skeletal template whose AnimTreeTemplate is a store handle of its own.
        let tree = component(&mut t, anim_tree, "TreeTemplate");
        let mut payload = ow_package::testkit::ints(&[0, 0]);
        payload.extend(t.p.object_tag("AnimTreeTemplate", tree));
        payload.extend_from_slice(&t.none);
        let skel_tree = t.p.add_export(skeletal, "SkelTreeTemplate", payload);
        let mut payload = ow_package::testkit::ints(&[0]);
        for (name, r) in [("Skel", skel), ("Cyl", cyl), ("Cycle", cycle_ref), ("SkelTree", skel_tree)] {
            payload.extend(t.p.object_tag(name, r));
        }
        payload.extend_from_slice(&t.none);
        t.p.add_export(holder, "Holder", payload);
        t.build()
    };
    // The gear-likeness interface and one component that defines its function (hook 1).
    let willow = {
        let mut t = ScriptToy::with_imports(&["Class", "Function"]);
        let core = t.p.add_import("Core", "Package", 0, "Core");
        let object = t.p.add_import("Core", "Class", core, "Object");
        let engine = t.p.add_import("Core", "Package", 0, "Engine");
        let component = t.p.add_import("Core", "Class", engine, "ActorComponent");
        let iface = t.class("IGearLikenessConsumer", object);
        let likeness = t.class("ToyLikenessComponent", component);
        for owner in [iface, likeness] {
            t.function(owner, "ToyRefresh", &[], None, &[], &FunctionSpec::new(0x400 | 0x20000, "ToyRefresh"), |_| None);
        }
        t.build()
    };
    let files: Vec<(&str, &[u8])> = vec![
        ("Core.upk", core.as_slice()),
        ("Engine.upk", engine.as_slice()),
        ("GD_ToyBody.upk", templates.as_slice()),
        ("WillowGame.upk", willow.as_slice()),
    ];
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
    let cylinder = vm.find_class(CYLINDER).unwrap();
    let skeletal = vm.find_class(SKELETAL).unwrap();
    vm.find_class(PRIMITIVE).unwrap();
    vm.find_class(MESH_COMPONENT).unwrap();
    // The rendering hooks' set-up classes (defined at set-up, as the game has them loaded).
    vm.find_class(crate::world::render_hooks::GEAR_LIKENESS).unwrap();
    let likeness = vm.find_class("WillowGame.ToyLikenessComponent").unwrap();
    let player_controller = vm.find_class("Engine.PlayerController").unwrap();
    let actor_component = vm.find_class("Engine.ActorComponent").unwrap();
    let scene = strukt(&mut vm, "Test.ToyScenePointer", &[("Ptr", PropKind::Int)]);
    let reattach_cylinder = class(&mut vm, "Test.ToyReattachCylinder", cylinder, &[
        ("Scene", PropKind::Struct(scene)),
        ("bNeedsReattach", PropKind::Bool),
        ("bNeedsUpdateTransform", PropKind::Bool),
    ]);
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
        ("AimOffsetNodeName", PropKind::Name),
        ("HeadLookAtName", PropKind::Name),
    ]);
    let mut pawn_props = vec![
        ("BodyClass", PropKind::Object { class: body }),
        ("InstanceDataState", PropKind::Struct(set)),
        ("ReplicatedInstanceDataState", PropKind::Struct(rep)),
        ("CylinderComponent", o.clone()),
        ("Mesh", o.clone()),
        ("HeadMesh", o.clone()),
        ("LightEnvironment", o.clone()),
        // The rest of the made-Mesh step (NATIVE_MESH_REST.md).
        ("MeshBodyCompositionIndex", PropKind::Byte { enum_: None }),
        ("ArmBoneBlend", o.clone()),
        ("AimOffset", o.clone()),
        ("LeftHandIK", o.clone()),
        ("HeadLookAt", o.clone()),
        ("bUsingFootIK", PropKind::Bool),
        ("SlotNodes", PropKind::Array(Box::new(o.clone()))),
        ("RepMeshScale", PropKind::Float),
        ("Weapon", o.clone()),
        ("OffHandWeapon", o.clone()),
        ("Controller", if controller_int { PropKind::Int } else { o.clone() }),
    ];
    if !head_mesh {
        pawn_props.retain(|(n, _)| *n != "HeadMesh");
    }
    let pawn = class(&mut vm, crate::world::pawn_meshes::WILLOW_PAWN, actor, &pawn_props);
    let path = "WillowGame.WillowPawn.InitializePawnMeshes";
    let d = FunctionDecl::native(ObjectPath::parse(path).unwrap(), "WillowPawn.InitializePawnMeshes()").with_owner(pawn);
    let wrapper = vm.declare_function(d).unwrap();
    if reattach {
        declare_reattach(&mut vm, pawn, REATTACH_WEAPONS);
    }
    crate::world::pawn_meshes::register(&mut vm).unwrap();
    Env {
        vm,
        dir,
        object,
        pawn,
        body,
        cylinder,
        skeletal,
        union,
        body_union,
        component_data,
        composition,
        wrapper,
        actor_component,
        player_controller,
        likeness,
        reattach_cylinder,
    }
}

/// `ReattachWeapons` at `path`, a method of the hand-declared `class`.
fn declare_reattach(vm: &mut Vm, class: ClassId, path: &str) {
    let d = FunctionDecl::native(ObjectPath::parse(path).unwrap(), path).with_owner(class);
    let f = vm.declare_function(d).unwrap();
    vm.declare_method(class, "ReattachWeapons", None, f).unwrap();
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
    let pawn = e.pawn;
    pawn_of(e, pawn, attachments)
}

/// [`pawn_with`] for a pawn of `class` (the pawn class or a subclass).
fn pawn_of(e: &mut Env, class: ClassId, attachments: Vec<Value>) -> ObjRef {
    let body = make(&mut e.vm, e.body, "BodyClass_Test");
    let comp = value(&e.vm, e.composition, vec![("Attachments", Value::array(attachments))]);
    put(&mut e.vm, body, "BodyComposition", comp);
    let p = make(&mut e.vm, class, "Pawn_Test");
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
/// A child named `CustomHead` becomes `HeadMesh`. The light environment is not written (ruling 4, review r1 of #542).
#[test]
fn the_owner_retry_and_head_mesh() {
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
    assert_eq!(prop(&e.vm, m0, "LightEnvironment"), Value::Object(None), "no light environment write");
    assert_eq!(prop(&e.vm, m1, "LightEnvironment"), Value::Object(None));
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

/// Maya's order: attachment 0 (`Mesh`, a skeletal component whose loaded tree has a kept `AimOffset` node) is the
/// animation-node `LayerAbsent`, found before any write, so attachment 13's cylinder (later in the list) is not written
/// either.
#[test]
fn the_new_mesh_s_animation_nodes_are_layer_absent_and_write_free() {
    let mut e = env();
    let t = Anim::new(&mut e);
    let aim = t.node(&mut e, "WillowAnimNodeAimOffset", "AimOffset", 1);
    t.children(&mut e, t.root, &[aim]);
    let skel = skel_with(&mut e, Some(t.root));
    let cyl = make(&mut e.vm, e.cylinder, "Cyl_Template");
    let a0 = attachment(&mut e, A { template: Some(skel), parent: MESH_ID, index: 1, ..A::default() });
    let a1 = attachment(&mut e, A { template: Some(cyl), parent: CYLINDER_ID, collision: 2, index: 2, ..A::default() });
    let p = pawn_with(&mut e, vec![a0, a1]);
    aim_offset_named(&mut e, p, "AimOffset");
    let (missing, what) = refused(&mut e, p);
    assert!(matches!(missing, Missing::LayerAbsent(l) if l == ANIM_LAYER), "{missing:?}");
    assert_eq!(what, ANIM_NODES);
    assert_eq!(prop(&e.vm, p, "CylinderComponent"), Value::Object(None));
}

/// Hub, 20:03Z: a pawn that already holds a `CylinderComponent` (stock pawn defaults give one) meets the release of the
/// old one, `NotRead`, before any write. With an old `Mesh` set, that release comes before attachment 0's
/// animation-node `LayerAbsent` (section 4.5.1, review r1 of #542, item 2).
#[test]
fn releasing_the_old_cylinder_or_mesh_is_not_read() {
    let mut e = env();
    let cyl = make(&mut e.vm, e.cylinder, "Cyl_Template");
    let a = attachment(&mut e, A { template: Some(cyl), parent: CYLINDER_ID, collision: 2, ..A::default() });
    let p = pawn_with(&mut e, vec![a]);
    let old = make(&mut e.vm, e.cylinder, "OldCylinder");
    put(&mut e.vm, p, "CylinderComponent", Value::Object(Some(old)));
    let (missing, what) = refused(&mut e, p);
    assert_eq!(missing, Missing::NotRead);
    assert_eq!(what, releasing_old("CylinderComponent"));
    assert_eq!(prop(&e.vm, p, "CylinderComponent"), Value::Object(Some(old)), "untouched");

    let mut e = env();
    let skel = make(&mut e.vm, e.skeletal, "Skel_Template");
    let a0 = attachment(&mut e, A { template: Some(skel), parent: MESH_ID, ..A::default() });
    let p = pawn_with(&mut e, vec![a0]);
    let old = make(&mut e.vm, e.skeletal, "OldMesh");
    put(&mut e.vm, p, "Mesh", Value::Object(Some(old)));
    let (missing, what) = refused(&mut e, p);
    assert_eq!(missing, Missing::NotRead);
    assert_eq!(what, releasing_old("Mesh"));
}

/// Review r1 of #542, item 4: a second `CylinderComponent` attachment would release the first one's component, so it
/// is the release `NotRead` too, before any write.
#[test]
fn a_second_cylinder_attachment_is_a_release() {
    let mut e = env();
    let c1 = make(&mut e.vm, e.cylinder, "Cyl_1");
    let c2 = make(&mut e.vm, e.cylinder, "Cyl_2");
    let a0 = attachment(&mut e, A { template: Some(c1), parent: CYLINDER_ID, ..A::default() });
    let a1 = attachment(&mut e, A { template: Some(c2), parent: CYLINDER_ID, index: 2, ..A::default() });
    let p = pawn_with(&mut e, vec![a0, a1]);
    let (missing, what) = refused(&mut e, p);
    assert_eq!(missing, Missing::NotRead);
    assert_eq!(what, releasing_old("CylinderComponent"));
}

/// The rendering hooks (lane IMPL-BODYCOMP-RENDER-HOOKS, which replaced ruling 4's refusal of every made component):
/// a made cylinder on a pawn with no controller and no light environment passes all three and is written; a body with
/// only switches and custom entries still writes.
#[test]
fn a_made_component_passes_the_rendering_hooks_when_none_acts() {
    let mut e = env();
    let cyl = make(&mut e.vm, e.cylinder, "Cyl_Template");
    let a = attachment(&mut e, A { template: Some(cyl), parent: CYLINDER_ID, collision: 2, ..A::default() });
    let p = pawn_with(&mut e, vec![a]);
    run(&mut e, p).unwrap();
    let made = made_of(&e, &entries(&e, p)[0]);
    assert_eq!(prop(&e.vm, p, "CylinderComponent"), Value::Object(Some(made)));
    assert_eq!(prop(&e.vm, made, "bOwnerNoSee"), Value::Bool(false), "no hook wrote");
    assert_eq!(prop(&e.vm, made, "LightEnvironment"), Value::Object(None), "no hook wrote");

    let mut e = env();
    let s = attachment(&mut e, A { ty: SWITCH, index: 1, ..A::default() });
    let h = attachment(&mut e, A { ty: HEAD, index: 2, ..A::default() });
    let p = pawn_with(&mut e, vec![s, h]);
    run(&mut e, p).unwrap();
    assert_eq!(entries(&e, p).len(), 2, "switches and custom entries still write");
}

/// Review r1 of #542, item 7: the wrapper's root-motion check runs after the build's preflight succeeds and before any
/// write; with a component the build refuses first.
#[test]
fn the_root_motion_check_follows_the_preflight() {
    let mut e = env();
    let s = attachment(&mut e, A { ty: SWITCH, ..A::default() });
    let p = pawn_with(&mut e, vec![s]);
    let Value::Object(Some(body)) = prop(&e.vm, p, "BodyClass") else { panic!() };
    put(&mut e.vm, body, "UseRootMotion", Value::Bool(true));
    let (missing, what) = refused(&mut e, p);
    assert_eq!(missing, Missing::NotRead);
    assert_eq!(what, crate::world::pawn_meshes::ROOT_MOTION);

    let mut e = env();
    let skel = make(&mut e.vm, e.skeletal, "Skel_Template");
    let a = attachment(&mut e, A { template: Some(skel), ..A::default() });
    let p = pawn_with(&mut e, vec![a]);
    stop_at_light(&mut e, p, skel);
    let Value::Object(Some(body)) = prop(&e.vm, p, "BodyClass") else { panic!() };
    put(&mut e.vm, body, "UseRootMotion", Value::Bool(true));
    let (_, what) = refused(&mut e, p);
    assert_eq!(what, crate::world::render_hooks::ARRAY_CHECK, "the preflight's refusal comes first");
}

/// Review r1 of #542, item 5: a replicated state without the packing's fields is the build's shape error before any
/// write, even for a body whose walk makes only a switch.
#[test]
fn the_packing_fields_are_probed_before_any_write() {
    let mut e = env();
    let s = attachment(&mut e, A { ty: SWITCH, ..A::default() });
    let p = pawn_with(&mut e, vec![s]);
    // A replicated state with the fields the walk reads but not the packing's two.
    let short = strukt(&mut e.vm, "Test.ShortReplicatedState", &[
        ("RemovedBodyCompositionPartsBitField", PropKind::Int),
        ("bNeedToApplyThisState", PropKind::Bool),
    ]);
    let composition = prop(&e.vm, prop_object(&e, p, "BodyClass"), "BodyComposition");
    let inputs = Inputs {
        owner: Some(p),
        composition,
        set: prop(&e.vm, p, "InstanceDataState"),
        replicated: e.vm.space.classes.struct_defaults(short).unwrap(),
        mode: 0,
    };
    let before = Counters::read(&e.vm);
    let err = build(&mut e.vm, inputs).unwrap_err();
    assert!(matches!(&err, VmError::Script { message, .. } if message.contains("SwitchStateBitField")), "{err:?}");
    assert_eq!(Counters::read(&e.vm), before, "write-free");
    assert!(entries(&e, p).is_empty());
}

fn prop_object(e: &Env, o: ObjRef, name: &str) -> ObjRef {
    let Value::Object(Some(x)) = prop(&e.vm, o, name) else { panic!("{name}") };
    x
}

/// The preflight's other refusals, each its named kind and text, write-free.
#[test]
fn the_named_refusals_are_write_free() {
    for case in ["rebuild", "client", "template", "hit", "actor socket", "socket tables", "parent link", "other target"] {
        let mut e = env();
        // The later attachments' refusals need the earlier made component past the rendering hooks.
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

/// The holder's three template handles, unloaded: `Skel`, `Cyl`, `Cycle`.
fn store_templates(e: &mut Env) -> (ObjRef, ObjRef, ObjRef) {
    let path = ObjectPath::parse("GD_ToyBody.Holder").unwrap();
    let holder = e.vm.defs.find(&mut e.vm.space, &path).unwrap().unwrap();
    let get = |e: &Env, n: &str| prop_object(e, holder, n);
    let t = (get(e, "Skel"), get(e, "Cyl"), get(e, "Cycle"));
    for h in [t.0, t.1, t.2] {
        assert!(e.vm.defs.is_unloaded(&e.vm.space, h), "a store handle, not loaded");
    }
    t
}

/// Review r1 of #542, item 8: a skeletal parent whose template the store has not loaded cannot give its mesh asset
/// without a load, so a child with a socket is the named `NotRead`, write-free, and nothing is loaded.
#[test]
fn an_unloaded_skeletal_parent_template_is_not_read() {
    let mut e = env();
    let (skel, _, _) = store_templates(&mut e);
    let cyl = make(&mut e.vm, e.cylinder, "Cyl");
    let a0 = attachment(&mut e, A { template: Some(skel), ..A::default() });
    let a1 = attachment(&mut e, A { base: 0, template: Some(cyl), socket: "Spine3", index: 2, ..A::default() });
    let p = pawn_with(&mut e, vec![a0, a1]);
    let (missing, what) = refused(&mut e, p);
    assert_eq!(missing, Missing::NotRead);
    assert_eq!(what, lazy_template("GD_ToyBody.SkelTemplate"));
    assert!(e.vm.defs.is_unloaded(&e.vm.space, skel), "nothing loaded");
}

/// Review r1 of #542, item 8: a template the store has not loaded is loaded by the writes, and that load moves
/// `DefStore::load_transitions` (a counted write).
#[test]
fn a_template_loaded_by_the_writes_is_counted() {
    let mut e = env();
    let (_, cyl, _) = store_templates(&mut e);
    let a = attachment(&mut e, A { template: Some(cyl), ..A::default() });
    let p = pawn_with(&mut e, vec![a]);
    let loads = e.vm.defs.load_transitions();
    run(&mut e, p).unwrap();
    assert!(!e.vm.defs.is_unloaded(&e.vm.space, cyl), "loaded by the writes");
    assert!(e.vm.defs.load_transitions() > loads, "the load is counted");
    let made = made_of(&e, &entries(&e, p)[0]);
    assert_ne!(made, cyl);
}

/// Review r1 of #542, item 6: a template whose load already failed is refused in the preflight, before any write.
#[test]
fn a_failed_template_is_refused_before_any_write() {
    let mut e = env();
    let (_, _, cycle) = store_templates(&mut e);
    assert!(e.vm.defs.ensure_loaded(&mut e.vm.space, cycle).is_err(), "the cycle export's load fails");
    assert!(e.vm.defs.is_failed(&e.vm.space, cycle));
    let a = attachment(&mut e, A { template: Some(cycle), ..A::default() });
    let p = pawn_with(&mut e, vec![a]);
    let (missing, what) = refused(&mut e, p);
    assert_eq!(missing, Missing::NotRead);
    assert_eq!(what, failed_template("GD_ToyBody.CycleTemplate"));
}

// ---------------------------------------------------------------------------------------------------------------
// The animation-node lookups (NATIVE_ANIM_NODE_LOOKUP.md at b09720f6, lane IMPL-ANIM-NODE-LOOKUP)
// ---------------------------------------------------------------------------------------------------------------

use crate::world::anim_node_lookup::{Settled, check};

/// An invented template tree, made of runtime objects (loaded, so the template settles what it can).
struct Anim {
    root: ObjRef,
}

impl Anim {
    /// A root `AnimTree` with no name and saved tick index 0.
    fn new(e: &mut Env) -> Self {
        let class = e.vm.find_class("Engine.AnimTree").unwrap();
        Anim { root: make(&mut e.vm, class, "AnimTree_Toy") }
    }

    /// A node of class `class` (an engine class, or one of the invented game classes), named `name` ("None" for
    /// none), at saved tick index `tick`.
    fn node(&self, e: &mut Env, class: &str, node_name: &str, tick: i32) -> ObjRef {
        let class = anim_class(e, class);
        let n = make(&mut e.vm, class, "Node");
        let v = name(&mut e.vm, node_name);
        put(&mut e.vm, n, "NodeName", v);
        put(&mut e.vm, n, "TickArrayIndex", Value::Int(tick));
        n
    }

    /// `parent.Children`, one child per node, in order.
    fn children(&self, e: &mut Env, parent: ObjRef, nodes: &[ObjRef]) {
        let s = e.vm.space.classes.struct_by_path("Engine.AnimNodeBlendBase.AnimBlendChild").unwrap();
        let list = nodes.iter().map(|n| value(&e.vm, s, vec![("Anim", Value::Object(Some(*n)))])).collect();
        put(&mut e.vm, parent, "Children", Value::array(list));
    }

    /// One control list per chain, each control of the class and name given, linked by `NextControl`.
    fn controls(&self, e: &mut Env, chains: &[&[(&str, &str)]]) {
        let s = e.vm.space.classes.struct_by_path("Engine.AnimTree.SkelControlListHead").unwrap();
        let mut heads = Vec::new();
        for chain in chains {
            let mut list: Vec<ObjRef> = Vec::new();
            for (class, control_name) in *chain {
                let class = anim_class(e, class);
                let c = make(&mut e.vm, class, "Control");
                let v = name(&mut e.vm, control_name);
                put(&mut e.vm, c, "ControlName", v);
                if let Some(&prev) = list.last() {
                    put(&mut e.vm, prev, "NextControl", Value::Object(Some(c)));
                }
                list.push(c);
            }
            heads.push(value(&e.vm, s, vec![("ControlHead", Value::Object(list.first().copied()))]));
        }
        put(&mut e.vm, self.root, "SkelControlLists", Value::array(heads));
    }
}

/// The engine class `short`, or one of the invented game classes (defined on first use under the class the game's
/// extends).
fn anim_class(e: &mut Env, short: &str) -> ClassId {
    if let Some(c) = e.vm.space.classes.by_path(&format!("WillowGame.{short}")) {
        return c;
    }
    let base = match short {
        "WillowAnimNodeAimOffset" => "Engine.AnimNodeAimOffset",
        "AnimNodeSpecialMoveBlend" => "Engine.AnimNodeSlot",
        "WillowSkelControl_LookAtActor" | "WillowSkelControl_FootPlacement" => "Engine.SkelControlBase",
        engine => return e.vm.find_class(&format!("Engine.{engine}")).unwrap(),
    };
    let base = e.vm.find_class(base).unwrap();
    class(&mut e.vm, &format!("WillowGame.{short}"), base, &[])
}

fn body_of(e: &Env, p: ObjRef) -> ObjRef {
    let Value::Object(Some(body)) = prop(&e.vm, p, "BodyClass") else { panic!("no body class") };
    body
}

/// Sets the pawn's body class's `AimOffsetNodeName` (`HeadLookAtName` stays None).
fn aim_offset_named(e: &mut Env, p: ObjRef, aim: &str) {
    let body = body_of(e, p);
    let v = name(&mut e.vm, aim);
    put(&mut e.vm, body, "AimOffsetNodeName", v);
}

/// A pawn with one `Mesh` attachment from `template`, its body class naming `AimOffset` (Maya's).
fn mesh_pawn(e: &mut Env, template: ObjRef) -> ObjRef {
    let a = attachment(e, A { template: Some(template), parent: MESH_ID, ..A::default() });
    let p = pawn_with(e, vec![a]);
    aim_offset_named(e, p, "AimOffset");
    p
}

fn skel_with(e: &mut Env, tree: Option<ObjRef>) -> ObjRef {
    let skel = make(&mut e.vm, e.skeletal, "Skel_Template");
    put(&mut e.vm, skel, "AnimTreeTemplate", Value::Object(tree));
    skel
}

/// #529's refusal, text and layer unchanged.
fn is_anim_refusal(missing: &Missing, what: &str) -> bool {
    matches!(missing, Missing::LayerAbsent(l) if *l == ANIM_LAYER) && what == ANIM_NODES
}

/// Rule 5: a template with no tree settles all four lookups as None, `bUsingFootIK` false and `SlotNodes` empty. The
/// rest of the made-`Mesh` step (NATIVE_MESH_REST.md) then passes its preflight, and the walk reaches the rendering
/// hooks: here hook 3's array check (the pawn's light environment set, the template's `bAcceptsLights` set), write-free.
#[test]
fn no_tree_settles_everything_and_the_mesh_step_reaches_the_rendering_hooks() {
    let mut e = env();
    let skel = skel_with(&mut e, None);
    let p = mesh_pawn(&mut e, skel);
    stop_at_light(&mut e, p, skel);
    let body = body_of(&e, p);
    assert_eq!(check(&e.vm, skel, Some(body), None).unwrap(), Settled::default());
    let (missing, what) = refused(&mut e, p);
    assert!(is_light_stop(&missing, &what), "{missing:?} {what}");
    assert_eq!(prop(&e.vm, p, "Mesh"), Value::Object(None), "untouched");
}

/// Lane FIX-BODY-COMPOSITION-LOOKUP-STALE: the lookups search the planned copy, so a tree with no planned copy (this
/// fixture switches the copy's check off: its classes declare none of the copy's fields) is #529's refusal unchanged,
/// whatever the tree holds, write-free. The rules themselves are tested on the copy's fixture
/// (anim_tree_copy_tests.rs, "the body composition's lookups over the copy").
#[test]
fn a_tree_with_no_planned_copy_is_the_refusal() {
    let mut e = env();
    let t = Anim::new(&mut e);
    let arm = t.node(&mut e, "AnimNode", "ArmBoneBlend", 1);
    let blend = t.node(&mut e, "AnimNodeBlendBase", "UBMoveBlend", 2);
    let leaf = t.node(&mut e, "AnimNode", "Leaf", 3);
    t.children(&mut e, t.root, &[arm, blend]);
    t.children(&mut e, blend, &[leaf]);
    let foot = ("WillowSkelControl_FootPlacement", "WillowFootIK");
    t.controls(&mut e, &[&[("SkelControlBase", "LeftHandGrip")], &[foot, foot]]);
    let skel = skel_with(&mut e, Some(t.root));
    let p = mesh_pawn(&mut e, skel);
    let body = body_of(&e, p);
    let before = Counters::read(&e.vm);
    match check(&e.vm, skel, Some(body), None) {
        Err(VmError::NotYet(n)) => assert!(is_anim_refusal(&n.missing, &n.what), "{n}"),
        other => panic!("expected #529's refusal, got {other:?}"),
    }
    assert_eq!(Counters::read(&e.vm), before, "the check writes nothing");
    let (missing, what) = refused(&mut e, p);
    assert!(is_anim_refusal(&missing, &what), "{missing:?} {what}");
}

/// Review r1 of #561, item 8: with the attach's tree-copy check on (lane IMPL-COPY-ANIM-TREE), a tree the copy refuses
/// stops at the copy's own check, before the lookups, and nothing is written. Since review r1 of #618 the fixture
/// declares the copy's fields, so the refusal is the copy's first unread step (its parent node array not built), not a
/// missing field.
#[test]
fn with_the_copy_check_on_the_copy_refuses_before_the_lookups() {
    let mut e = env();
    crate::world::anim_tree_copy::skip_attach_check_for_tests(false);
    let t = Anim::new(&mut e);
    let arm = t.node(&mut e, "AnimNode", "ArmBoneBlend", 1);
    t.children(&mut e, t.root, &[arm]);
    let skel = skel_with(&mut e, Some(t.root));
    let p = mesh_pawn(&mut e, skel);
    let before = Counters::read(&e.vm);
    match run(&mut e, p).unwrap_err() {
        VmError::NotYet(n) => assert_eq!(&*n.what, crate::world::anim_tree_copy::PARENTS, "{n}"),
        other => panic!("expected the copy's refusal, got {other:?}"),
    }
    assert_eq!(Counters::read(&e.vm), before, "write-free");
    assert!(entries(&e, p).is_empty());
    crate::world::anim_tree_copy::skip_attach_check_for_tests(true);
}

/// The copy's check on (its switches off) for the length of `f`: the fixture declares the copy's fields (review r1 of
/// #618, item 3), so the attach's planned copy is the lookups' input, as in production.
fn with_copy<T>(f: impl FnOnce() -> T) -> T {
    crate::world::anim_tree_copy::skip_attach_check_for_tests(false);
    let out = f();
    crate::world::anim_tree_copy::skip_attach_check_for_tests(true);
    out
}

/// The attach's planned copy of `skel`'s tree, with the copy's check on.
fn planned(e: &Env, skel: ObjRef) -> Option<crate::world::anim_tree_copy::Plan> {
    with_copy(|| crate::world::anim_tree_copy::attach_plan(&e.vm, skel, None).unwrap())
}

/// Review r1 of #618, item 3: with the copy's check on, a tree the copy can plan (a root with no children, its parent
/// array built) goes through `attach_plan` and the lookups over the planned copy, past the made `Mesh`'s rest (#570),
/// to the next refusal of the walk, the rendering hooks, with every counter unchanged.
#[test]
fn with_the_copy_check_on_a_plannable_tree_passes_the_lookups() {
    let mut e = env();
    let t = Anim::new(&mut e);
    put(&mut e.vm, t.root, "bParentNodeArrayBuilt", Value::Bool(true));
    let skel = skel_with(&mut e, Some(t.root));
    let p = mesh_pawn(&mut e, skel);
    stop_at_light(&mut e, p, skel);
    let (missing, what) = with_copy(|| refused(&mut e, p));
    assert!(is_light_stop(&missing, &what), "{missing:?} {what}");
}

/// With no planned copy, a kept find and a slot node are #529's refusal too (as before this lane), write-free.
#[test]
fn a_kept_find_or_a_slot_node_is_the_refusal() {
    // (node class, node name, control class, control name, head look-at name)
    let cases: [(&str, &str, &str, &str, &str); 5] = [
        ("AnimNodeBlendPerBone", "ArmBoneBlend", "SkelControlBase", "Other", "None"),
        ("WillowAnimNodeAimOffset", "AimOffset", "SkelControlBase", "Other", "None"),
        ("AnimNode", "Other", "SkelControlLeftHandGripWeapon", "LeftHandGrip", "None"),
        ("AnimNode", "Other", "WillowSkelControl_LookAtActor", "HeadLook", "HeadLook"),
        ("AnimNodeSpecialMoveBlend", "SpecialMoveBlend", "SkelControlBase", "Other", "None"),
    ];
    for (node_class, node_name, control_class, control_name, head) in cases {
        let mut e = env();
        let t = Anim::new(&mut e);
        let n = t.node(&mut e, node_class, node_name, 1);
        t.children(&mut e, t.root, &[n]);
        t.controls(&mut e, &[&[(control_class, control_name)]]);
        let skel = skel_with(&mut e, Some(t.root));
        let p = mesh_pawn(&mut e, skel);
        let body = body_of(&e, p);
        let v = name(&mut e.vm, head);
        put(&mut e.vm, body, "HeadLookAtName", v);
        let (missing, what) = refused(&mut e, p);
        assert!(is_anim_refusal(&missing, &what), "{node_class} {control_class}: {missing:?} {what}");
        assert_eq!(prop(&e.vm, p, "Mesh"), Value::Object(None));
    }
}

/// Rule 2: an unloaded template component is #529's refusal with nothing loaded; so is a loaded template whose tree
/// is an unloaded handle.
#[test]
fn an_unloaded_template_or_tree_is_the_refusal_with_nothing_loaded() {
    let mut e = env();
    let (skel, _, _) = store_templates(&mut e);
    let p = mesh_pawn(&mut e, skel);
    let (missing, what) = refused(&mut e, p);
    assert!(is_anim_refusal(&missing, &what), "{missing:?} {what}");
    assert!(e.vm.defs.is_unloaded(&e.vm.space, skel), "nothing loaded");

    let mut e = env();
    let path = ObjectPath::parse("GD_ToyBody.Holder").unwrap();
    let holder = e.vm.defs.find(&mut e.vm.space, &path).unwrap().unwrap();
    let skel_tree = prop_object(&e, holder, "SkelTree");
    e.vm.defs.ensure_loaded(&mut e.vm.space, skel_tree).unwrap();
    let tree = prop_object(&e, skel_tree, "AnimTreeTemplate");
    assert!(e.vm.defs.is_unloaded(&e.vm.space, tree), "the tree is a handle the store has not loaded");
    let p = mesh_pawn(&mut e, skel_tree);
    let (missing, what) = refused(&mut e, p);
    assert!(is_anim_refusal(&missing, &what), "{missing:?} {what}");
    assert!(e.vm.defs.is_unloaded(&e.vm.space, tree), "nothing loaded");
}

/// Section 4.5.1's order: with an old `Mesh` set, its release is refused before the lookups, even on a tree that
/// would be the refusal.
#[test]
fn the_old_mesh_release_comes_before_the_lookups() {
    let mut e = env();
    let t = Anim::new(&mut e);
    let n = t.node(&mut e, "AnimNodeSlot", "Slot", 1);
    t.children(&mut e, t.root, &[n]);
    let skel = skel_with(&mut e, Some(t.root));
    let p = mesh_pawn(&mut e, skel);
    let old = make(&mut e.vm, e.skeletal, "OldMesh");
    put(&mut e.vm, p, "Mesh", Value::Object(Some(old)));
    let (missing, what) = refused(&mut e, p);
    assert_eq!(missing, Missing::NotRead);
    assert_eq!(what, releasing_old("Mesh"));
}

/// Review r1 of #557, item 1: a control reached from two lists' chains is not a loop. It keeps its first place, and
/// the lookups still find it.
#[test]
fn a_control_shared_by_two_lists_is_not_a_loop() {
    let mut e = env();
    let t = Anim::new(&mut e);
    put(&mut e.vm, t.root, "bParentNodeArrayBuilt", Value::Bool(true));
    t.controls(&mut e, &[&[("WillowSkelControl_FootPlacement", "WillowFootIK")]]);
    let lists = prop(&e.vm, t.root, "SkelControlLists");
    let head = lists.elements().unwrap()[0].clone();
    put(&mut e.vm, t.root, "SkelControlLists", Value::array(vec![head.clone(), head]));
    let skel = skel_with(&mut e, Some(t.root));
    let p = mesh_pawn(&mut e, skel);
    let body = body_of(&e, p);
    let copy = planned(&e, skel);
    assert_eq!(check(&e.vm, skel, Some(body), copy.as_ref()).unwrap(), Settled { foot_ik: true, ..Settled::default() });
    stop_at_light(&mut e, p, skel);
    let (missing, what) = with_copy(|| refused(&mut e, p));
    assert!(is_light_stop(&missing, &what), "{missing:?} {what}");
}

/// `check` on its own, with #518's counters read around it: a refusal moves none of them.
fn check_refused(e: &Env, skel: ObjRef, body: Option<ObjRef>) -> (Missing, String) {
    let copy = planned(e, skel);
    let before = Counters::read(&e.vm);
    let err = check(&e.vm, skel, body, copy.as_ref()).unwrap_err();
    let VmError::NotYet(n) = err else { panic!("{err:?}") };
    assert_eq!(Counters::read(&e.vm), before, "write-free: {}", n.what);
    (n.missing, n.what.to_string())
}

/// Review r1 of #557, item 2: the AimOffset lookup with no body class is `NotRead`. The pawn wrapper's gate needs a
/// loaded body class, so this is reached only through `check` itself.
#[test]
fn no_body_class_is_not_read() {
    let mut e = env();
    let t = Anim::new(&mut e);
    put(&mut e.vm, t.root, "bParentNodeArrayBuilt", Value::Bool(true));
    let skel = skel_with(&mut e, Some(t.root));
    let (missing, what) = check_refused(&e, skel, None);
    assert_eq!(missing, Missing::NotRead);
    assert_eq!(what, crate::world::anim_node_lookup::NO_BODY_CLASS);
}

/// Review r1 of #557, items 2 and 3: an unloaded body class is #529's refusal with nothing loaded; one whose load
/// failed is `NotRead`, naming it. Store handles stand in for the body class (only their state is read first).
#[test]
fn an_unloaded_or_failed_body_class_is_refused_with_nothing_loaded() {
    let mut e = env();
    let t = Anim::new(&mut e);
    put(&mut e.vm, t.root, "bParentNodeArrayBuilt", Value::Bool(true));
    let skel = skel_with(&mut e, Some(t.root));
    let (_, cyl, cycle) = store_templates(&mut e);
    let (missing, what) = check_refused(&e, skel, Some(cyl));
    assert!(is_anim_refusal(&missing, &what), "{missing:?} {what}");
    assert!(e.vm.defs.is_unloaded(&e.vm.space, cyl), "nothing loaded");
    assert!(e.vm.defs.ensure_loaded(&mut e.vm.space, cycle).is_err(), "the cycle export's load fails");
    let (missing, what) = check_refused(&e, skel, Some(cycle));
    assert_eq!(missing, Missing::NotRead);
    assert_eq!(what, crate::world::anim_node_lookup::failed_body_class("GD_ToyBody.CycleTemplate"));
}

/// Review r1 of #557, item 2: an unloaded node inside a loaded tree is #529's refusal with nothing loaded, through the
/// pawn wrapper.
#[test]
fn an_unloaded_node_in_a_loaded_tree_is_the_refusal() {
    let mut e = env();
    let path = ObjectPath::parse("GD_ToyBody.Holder").unwrap();
    let holder = e.vm.defs.find(&mut e.vm.space, &path).unwrap().unwrap();
    let skel_tree = prop_object(&e, holder, "SkelTree");
    e.vm.defs.ensure_loaded(&mut e.vm.space, skel_tree).unwrap();
    let handle = prop_object(&e, skel_tree, "AnimTreeTemplate");
    assert!(e.vm.defs.is_unloaded(&e.vm.space, handle));
    let t = Anim::new(&mut e);
    t.children(&mut e, t.root, &[handle]);
    let skel = skel_with(&mut e, Some(t.root));
    let p = mesh_pawn(&mut e, skel);
    let (missing, what) = refused(&mut e, p);
    assert!(is_anim_refusal(&missing, &what), "{missing:?} {what}");
    assert!(e.vm.defs.is_unloaded(&e.vm.space, handle), "nothing loaded");
}

// The rest of the made-Mesh step (NATIVE_MESH_REST.md, lane IMPL-MESH-REST)
// ---------------------------------------------------------------------------------------------------------------

/// A cylinder at attachment 0 and the `Mesh` (no tree) at attachment 1, on a body that uses the mesh for collision.
/// The pawn's animation fields and `SlotNodes` start set, so the writes are seen to clear them.
fn mesh_rest_pawn(e: &mut Env) -> (ObjRef, ObjRef) {
    let cyl = make(&mut e.vm, e.cylinder, "Cyl_Template");
    let skel = skel_with(e, None);
    put(&mut e.vm, skel, "Scale", Value::Float(1.25));
    let a0 = attachment(e, A { template: Some(cyl), parent: CYLINDER_ID, ..A::default() });
    let a1 = attachment(e, A { template: Some(skel), parent: MESH_ID, index: 2, ..A::default() });
    let p = pawn_with(e, vec![a0, a1]);
    aim_offset_named(e, p, "AimOffset");
    let body = body_of(e, p);
    put(&mut e.vm, body, "bUseMeshForCollision", Value::Bool(true));
    let object = e.object;
    let stale = make(&mut e.vm, object, "Stale");
    for name in &ANIM_FIELDS[..4] {
        put(&mut e.vm, p, name, Value::Object(Some(stale)));
    }
    put(&mut e.vm, p, "bUsingFootIK", Value::Bool(true));
    put(&mut e.vm, p, "SlotNodes", Value::array(vec![Value::Object(Some(stale))]));
    (p, skel)
}

/// Section 3's writes on authority, checked by their final values: `Mesh`, the index as a byte, the flag on the new component (a layout fit,
/// UNVERIFIED), the four animation fields None, `bUsingFootIK` as settled, `SlotNodes` empty, `RepMeshScale` from the
/// component's `Scale`, and `CollisionComponent` = the mesh. `ReattachWeapons` with no weapon held and no override
/// (the canonical `WillowPawn.ReattachWeapons`) runs nothing.
#[test]
fn the_rest_of_the_mesh_step_sets_its_fields_on_authority() {
    let mut e = env();
    let (p, skel) = mesh_rest_pawn(&mut e);
    run(&mut e, p).unwrap();
    let list = entries(&e, p);
    assert_eq!(list.len(), 2);
    let cyl = made_of(&e, &list[0]);
    let mesh = made_of(&e, &list[1]);
    assert_ne!(mesh, skel, "a new component");
    assert_eq!(prop(&e.vm, p, "Mesh"), Value::Object(Some(mesh)));
    assert_eq!(prop(&e.vm, p, "MeshBodyCompositionIndex"), Value::Byte(1), "the attachment's index");
    assert_eq!(prop(&e.vm, mesh, SKIP_FLAG), Value::Bool(true));
    assert_eq!(prop(&e.vm, skel, SKIP_FLAG), Value::Bool(false), "the template is untouched");
    for name in &ANIM_FIELDS[..4] {
        assert_eq!(prop(&e.vm, p, name), Value::Object(None), "{name}");
    }
    assert_eq!(prop(&e.vm, p, "bUsingFootIK"), Value::Bool(false), "no tree: no foot IK");
    assert_eq!(prop(&e.vm, p, "SlotNodes"), Value::array(Vec::new()));
    assert_eq!(prop(&e.vm, p, "RepMeshScale"), Value::Float(1.25));
    assert_eq!(prop(&e.vm, p, "CylinderComponent"), Value::Object(Some(cyl)));
    assert_eq!(prop(&e.vm, p, "CollisionComponent"), Value::Object(Some(mesh)), "the mesh is used for collision");
}

/// Review r2 of #618, on the merged path (#570's write path): a `Mesh` whose tree the attach copies (the root, a slot
/// node, and under it a node named `AimOffset` of the kept class). The lookups over the planned copy and the copy's map
/// write the copies: `AimOffset` is the copied aim-offset node (outered to the made `Mesh`) and `SlotNodes` holds the
/// copied slot. A refusal of the same lookups (the template component already initialised, so the copy has no tick
/// list) writes nothing.
#[test]
fn the_lookups_write_the_copies_of_aim_offset_and_the_slot_nodes() {
    let build = |e: &mut Env| -> (ObjRef, ObjRef, ObjRef, ObjRef) {
        let t = Anim::new(e);
        put(&mut e.vm, t.root, "bParentNodeArrayBuilt", Value::Bool(true));
        let slot = t.node(e, "AnimNodeSlot", "Slot", 1);
        let aim = t.node(e, "WillowAnimNodeAimOffset", "AimOffset", 2);
        t.children(e, t.root, &[slot]);
        t.children(e, slot, &[aim]);
        let s = e.vm.space.classes.struct_by_path("Engine.AnimNodeBlendBase.AnimBlendChild").unwrap();
        let child = value(&e.vm, s, vec![("Anim", Value::Object(Some(aim))), ("Weight", Value::Float(1.0))]);
        put(&mut e.vm, slot, "Children", Value::array(vec![child]));
        put(&mut e.vm, slot, "ParentNodes", Value::array(vec![Value::Object(Some(t.root))]));
        put(&mut e.vm, aim, "ParentNodes", Value::array(vec![Value::Object(Some(slot))]));
        let (p, skel) = mesh_rest_pawn(e);
        put(&mut e.vm, skel, "AnimTreeTemplate", Value::Object(Some(t.root)));
        (p, skel, slot, aim)
    };
    let mut e = env();
    let (p, _, slot, aim) = build(&mut e);
    with_copy(|| run(&mut e, p)).unwrap();
    let mesh = made_of(&e, &entries(&e, p)[1]);
    let Value::Object(Some(copy)) = prop(&e.vm, p, "AimOffset") else { panic!("AimOffset set") };
    assert_ne!(copy, aim, "the copy, not the template node");
    assert_eq!(e.vm.space.heap.get(copy).unwrap().outer, Some(mesh), "outered to the made Mesh");
    let Value::Object(Some(_)) = prop(&e.vm, p, "Mesh") else { panic!("Mesh set") };
    let slots = prop(&e.vm, p, "SlotNodes");
    let slots = slots.elements().unwrap();
    assert_eq!(slots.len(), 1, "the one slot node");
    let Value::Object(Some(slot_copy)) = slots[0] else { panic!() };
    assert_ne!(slot_copy, slot);
    assert_eq!(e.vm.space.heap.get(slot_copy).unwrap().outer, Some(mesh));
    assert_eq!(prop(&e.vm, p, "ArmBoneBlend"), Value::Object(None), "no ArmBoneBlend node");

    let mut e = env();
    let (p, skel, _, _) = build(&mut e);
    put(&mut e.vm, skel, "bAnimTreeInitialised", Value::Bool(true));
    let (missing, what) = with_copy(|| refused(&mut e, p));
    assert_eq!((missing, what.as_str()), (Missing::NotRead, crate::world::anim_node_lookup::NO_TICK_LIST));
}

/// Off authority (the plan's authority, #529 section 7 step 3), `RepMeshScale` is neither required nor written.
#[test]
fn off_authority_the_mesh_scale_is_not_replicated() {
    let mut e = env();
    let (p, _) = mesh_rest_pawn(&mut e);
    put(&mut e.vm, p, "Role", Value::Byte(2));
    put(&mut e.vm, p, "RepMeshScale", Value::Float(9.0));
    run(&mut e, p).unwrap();
    let mesh = made_of(&e, &entries(&e, p)[1]);
    assert_eq!(prop(&e.vm, p, "Mesh"), Value::Object(Some(mesh)));
    assert_eq!(prop(&e.vm, p, "RepMeshScale"), Value::Float(9.0), "untouched");
}

/// `ReattachWeapons` with a held weapon (either hand) needs the attach to the third-person mesh, which is not read:
/// `NotRead`, before any write.
#[test]
fn a_held_weapon_is_not_read_and_write_free() {
    for hand in ["Weapon", "OffHandWeapon"] {
        let mut e = env();
        let (p, _) = mesh_rest_pawn(&mut e);
        let object = e.object;
        let gun = make(&mut e.vm, object, "Gun");
        put(&mut e.vm, p, hand, Value::Object(Some(gun)));
        let (missing, what) = refused(&mut e, p);
        assert_eq!((missing, what.as_str()), (Missing::NotRead, HELD_WEAPON), "{hand}");
        assert_eq!(prop(&e.vm, p, "Mesh"), Value::Object(None));
    }
}

/// Two `Mesh` attachments: the second would release the first one's component, so it is the release `NotRead`,
/// before any write.
#[test]
fn a_second_mesh_attachment_is_a_release() {
    let mut e = env();
    let s1 = skel_with(&mut e, None);
    let s2 = skel_with(&mut e, None);
    let a0 = attachment(&mut e, A { template: Some(s1), parent: MESH_ID, ..A::default() });
    let a1 = attachment(&mut e, A { template: Some(s2), parent: MESH_ID, index: 2, ..A::default() });
    let p = pawn_with(&mut e, vec![a0, a1]);
    let (missing, what) = refused(&mut e, p);
    assert_eq!(missing, Missing::NotRead);
    assert_eq!(what, releasing_old("Mesh"));
}

/// The kind checks: each destination must take its write; a missing field or another kind is `VmError::Script`.
#[test]
fn the_flag_and_the_mesh_scale_are_kind_checked() {
    let e = env();
    let skeletal = e.skeletal;
    let pawn = e.pawn;
    let cylinder = e.cylinder;
    assert!(require_kind(&e.vm, skeletal, SKIP_FLAG, Want::Bool).is_ok());
    assert!(require_kind(&e.vm, pawn, "RepMeshScale", Want::Float).is_ok());
    assert!(require_kind(&e.vm, pawn, "Mesh", Want::Takes(skeletal)).is_ok());
    let cases = [
        (pawn, "Role", Want::Float),
        (skeletal, "Scale", Want::Bool),
        (pawn, "NoSuchField", Want::Float),
        (pawn, "BodyClass", Want::Takes(cylinder)),
        (pawn, "SlotNodes", Want::Byte),
    ];
    for (class, name, want) in cases {
        let err = require_kind(&e.vm, class, name, want).unwrap_err();
        assert!(matches!(err, VmError::Script { .. }), "{name}: {err:?}");
    }
}

/// Section 4, preflight 4.1: a pawn class with no `ReattachWeapons` at all is refused like an override (review r1 of
/// #570, item 2), write-free.
#[test]
fn no_reattach_weapons_is_refused_like_an_override() {
    let mut e = env_with(false);
    let (p, _) = mesh_rest_pawn(&mut e);
    let (missing, what) = refused(&mut e, p);
    assert_eq!((missing, what.as_str()), (Missing::NotRead, REATTACH_OVERRIDE));
    assert_eq!(prop(&e.vm, p, "Mesh"), Value::Object(None));
}

/// A `ReattachWeapons` that is not `WillowPawn`'s (here the only one, a subclass's own) is raised before a held
/// weapon, write-free (review r1 of #570, item 4). The fixture's base pawn has none, as `Vm::declare_method` refuses
/// a registration that would hide an inherited method (review r2). A state-scoped override goes through Off's state
/// frames, which only package states have; that walk is `ow_vm::states::find_loaded_event`'s test.
#[test]
fn an_override_is_refused_before_a_held_weapon() {
    let mut e = env_with(false);
    let pawn = e.pawn;
    let sub = class(&mut e.vm, "WillowGame.ToyPawnSub", pawn, &[]);
    declare_reattach(&mut e.vm, sub, "WillowGame.ToyPawnSub.ReattachWeapons");
    let skel = skel_with(&mut e, None);
    let a = attachment(&mut e, A { template: Some(skel), parent: MESH_ID, ..A::default() });
    let p = pawn_of(&mut e, sub, vec![a]);
    aim_offset_named(&mut e, p, "AimOffset");
    let object = e.object;
    let gun = make(&mut e.vm, object, "Gun");
    put(&mut e.vm, p, "Weapon", Value::Object(Some(gun)));
    let (missing, what) = refused(&mut e, p);
    assert_eq!((missing, what.as_str()), (Missing::NotRead, REATTACH_OVERRIDE));
    assert_eq!(prop(&e.vm, p, "Mesh"), Value::Object(None));
}

/// A pawn whose heap names a state but which has no state frame yet: the dispatch would make the frame (a write and
/// a state read), so the preflight refuses with this port's `NotRead`, before the held weapon and write-free.
#[test]
fn a_dispatch_that_needs_a_read_is_refused_before_a_held_weapon() {
    let mut e = env();
    let (p, _) = mesh_rest_pawn(&mut e);
    let state = e.vm.space.names.intern("Aiming");
    e.vm.space.heap.get_mut(p).unwrap().state = Some(state);
    let object = e.object;
    let gun = make(&mut e.vm, object, "Gun");
    put(&mut e.vm, p, "OffHandWeapon", Value::Object(Some(gun)));
    let (missing, what) = refused(&mut e, p);
    assert_eq!((missing, what.as_str()), (Missing::NotRead, REATTACH_UNREAD));
}

/// Review r1 of #570, item 3: on authority a component class whose `Scale` is not a float is `VmError::Script` in the
/// preflight, before the mesh is made or any pawn field written.
#[test]
fn a_scale_that_is_not_a_float_fails_before_any_write() {
    let mut e = env();
    let skeletal = e.skeletal;
    let odd = class(&mut e.vm, "Engine.ToyIntScaleMeshComponent", skeletal, &[("Scale", PropKind::Int)]);
    let skel = make(&mut e.vm, odd, "Odd_Template");
    let p = mesh_pawn(&mut e, skel);
    let before = Counters::read(&e.vm);
    let err = run(&mut e, p).unwrap_err();
    assert!(matches!(err, VmError::Script { .. }), "{err:?}");
    assert_eq!(Counters::read(&e.vm), before, "write-free");
    assert!(entries(&e, p).is_empty());
    assert_eq!(prop(&e.vm, p, "Mesh"), Value::Object(None));
}

// ---------------------------------------------------------------------------------------------------------------
// The rendering hooks (NATIVE_BODYCOMP_RENDER_HOOKS.md section 4, lane IMPL-BODYCOMP-RENDER-HOOKS). Numbers are the
// note's test list. Each refusal goes through `refused`: no heap write, no draw, no load transition, no entry.
// ---------------------------------------------------------------------------------------------------------------

use crate::world::render_hooks::{
    ARRAY_CHECK, GEAR_INTERFACE, GEAR_LIKENESS, GEAR_REFRESH, GEAR_UNRESOLVED, NOT_SETTLED, PAWN_TEST, RECORD as HOOKS_RECORD,
    RENDER_LAYER as HOOKS_LAYER,
};

/// Hook 3's array-check stop on the made component from `template`: the pawn's `LightEnvironment` set and the
/// template's `bAcceptsLights` set.
fn stop_at_light(e: &mut Env, p: ObjRef, template: ObjRef) {
    let light = make(&mut e.vm, e.object, "Light_Test");
    put(&mut e.vm, p, "LightEnvironment", Value::Object(Some(light)));
    put(&mut e.vm, template, "bAcceptsLights", Value::Bool(true));
}

fn is_light_stop(missing: &Missing, what: &str) -> bool {
    *missing == Missing::NotRead && what == ARRAY_CHECK
}

/// A player controller for `p`; with `local`, listed as the world's local player (an `Actors` service is added).
fn controller_for(e: &mut Env, p: ObjRef, local: bool) -> ObjRef {
    let pc_class = e.player_controller;
    let pc = make(&mut e.vm, pc_class, "PC_Test");
    put(&mut e.vm, p, "Controller", Value::Object(Some(pc)));
    if e.vm.service::<crate::actors::Actors>().is_none() {
        let actor = e.vm.space.classes.by_path("Engine.Actor").unwrap();
        e.vm.add_service(crate::actors::Actors::new(actor)).unwrap();
    }
    if local {
        e.vm.service::<crate::actors::Actors>().unwrap().borrow_mut().add_local_player(pc).unwrap();
    }
    pc
}

/// A pawn with one made attachment from `template` (identifier None, collision 0).
fn one_made(e: &mut Env, template: ObjRef) -> ObjRef {
    let a = attachment(e, A { template: Some(template), ..A::default() });
    pawn_with(e, vec![a])
}

fn hooks_not_read(e: &mut Env, p: ObjRef, want: &str) {
    let (missing, what) = refused(e, p);
    assert_eq!((missing, what.as_str()), (Missing::NotRead, want));
}

/// Test 1 (and hook 1's no on the walk): Engine components do not implement the interface; nothing is written by the
/// hooks. The made cylinder of [`a_made_component_passes_the_rendering_hooks_when_none_acts`] covers the write side.
#[test]
fn hook_1_an_engine_component_does_not_implement_the_gear_likeness_interface() {
    let e = env();
    for class in [e.cylinder, e.skeletal, e.actor_component, e.reattach_cylinder] {
        crate::world::render_hooks::gear_likeness(&e.vm, class).unwrap();
    }
}

/// Test 2: a made component whose class implements it is the refresh's `LayerAbsent`, before any write.
#[test]
fn hook_1_an_implementer_is_the_gear_likeness_layer_absent() {
    let mut e = env();
    let likeness = e.likeness;
    let t = make(&mut e.vm, likeness, "Likeness_Template");
    let p = one_made(&mut e, t);
    let before = Counters::read(&e.vm);
    let VmError::NotYet(n) = run(&mut e, p).unwrap_err() else { panic!() };
    assert_eq!(Counters::read(&e.vm), before, "write-free");
    assert!(matches!(&n.missing, Missing::LayerAbsent(l) if &**l == HOOKS_LAYER), "{:?}", n.missing);
    assert_eq!((&*n.what, n.record), (GEAR_REFRESH, HOOKS_RECORD));
}

/// Test 3, on decoded class bodies (a WillowGame package built with `Declarations`): an implementing class whose interface entry is not resolved yet (`NotRead`: resolving it would load), the
/// same class once the entry is resolved (the refresh's `LayerAbsent`), and a class with no entries (nothing).
#[test]
fn hook_1_reads_only_resolved_interface_entries() {
    use ow_object::class_body::CLASS_INTERFACE;
    use ow_package::testkit::reflect::{ClassBodySpec, Declarations, class_body};
    let dir = std::env::temp_dir().join(format!("ow-world-gearlike-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let mut d = Declarations::with_imports(&["Class", "Function", "StructProperty"]);
    let object_body = class_body(&mut d.p, &ClassBodySpec::new(0, 1, 2));
    let obj = d.p.add_export(0, "Object", object_body);
    d.default_object(obj, "Object");
    let iface = d.real_class("IGearLikenessConsumer", obj, obj, 2 | CLASS_INTERFACE, &[], None);
    d.real_class("ToyConsumer", obj, obj, 2, &[iface], None);
    d.real_class("ToyPlain", obj, obj, 2, &[], None);
    let bytes = d.build();
    let defs = DefStore::from_synthetic(&dir, &[("WillowGame.upk", &bytes)], IdentityPolicy::FirstInScanOrder, ParityMode::Off)
        .expect("store");
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
    let consumer = vm.find_class("WillowGame.ToyConsumer").unwrap();
    let plain = vm.find_class("WillowGame.ToyPlain").unwrap();
    let not_read = |r: Result<(), VmError>, want: &str| {
        let VmError::NotYet(n) = r.unwrap_err() else { panic!() };
        assert_eq!((n.missing, &*n.what, n.record), (Missing::NotRead, want, HOOKS_RECORD));
    };
    let gear = vm.find_class(GEAR_LIKENESS).unwrap();
    not_read(crate::world::render_hooks::gear_likeness(&vm, consumer), GEAR_UNRESOLVED);
    assert!(vm.implements(consumer, gear), "the entry resolved (as a loaded pawn class's would be)");
    let VmError::NotYet(n) = crate::world::render_hooks::gear_likeness(&vm, consumer).unwrap_err() else { panic!() };
    assert_eq!(&*n.what, GEAR_REFRESH);
    crate::world::render_hooks::gear_likeness(&vm, plain).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

/// Test 3, the fixture's side: a store whose interface class is not defined at set-up is `NotRead` on the walk.
#[test]
fn hook_1_the_interface_not_defined_at_set_up_is_not_read() {
    let mut e = env();
    let cyl = make(&mut e.vm, e.cylinder, "Cyl_Template");
    let p = one_made(&mut e, cyl);
    // A hand class of the same path cannot stand in: `by_path` must find no class. So the lookup is asked directly on
    // a fresh store with no WillowGame package.
    let dir = std::env::temp_dir().join(format!("ow-world-gearless-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let core = {
        let mut t = ScriptToy::new(&[]);
        t.class("Object", 0);
        t.build()
    };
    let defs = DefStore::from_synthetic(&dir, &[("Core.upk", core.as_slice())], IdentityPolicy::FirstInScanOrder, ParityMode::Off)
        .expect("store");
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
    let object = vm.find_class("Core.Object").unwrap();
    let VmError::NotYet(n) = crate::world::render_hooks::gear_likeness(&vm, object).unwrap_err() else { panic!() };
    assert_eq!((n.missing, &*n.what), (Missing::NotRead, GEAR_INTERFACE));
    let _ = std::fs::remove_dir_all(&dir);
    run(&mut e, p).unwrap();
}

/// Test 4: hook 1 comes before the custom-head step, so its refusal masks a pawn class that declares no `HeadMesh`.
#[test]
fn hook_1_is_reported_before_the_custom_head_declaration() {
    let mut e = env_opts(true, false, false);
    let likeness = e.likeness;
    let t = make(&mut e.vm, likeness, "Likeness_Template");
    let a = attachment(&mut e, A { template: Some(t), data_name: CUSTOM_HEAD_NAME, ..A::default() });
    let p = pawn_with(&mut e, vec![a]);
    let (_, what) = refused(&mut e, p);
    assert_eq!(what, GEAR_REFRESH);
    // Without hook 1's refusal the missing declaration is the custom-head step's script error.
    let cyl = make(&mut e.vm, e.cylinder, "Cyl_Template");
    let a = attachment(&mut e, A { template: Some(cyl), data_name: CUSTOM_HEAD_NAME, ..A::default() });
    let p = pawn_with(&mut e, vec![a]);
    match run(&mut e, p).unwrap_err() {
        VmError::Script { message, .. } => assert!(message.contains("HeadMesh"), "{message}"),
        other => panic!("{other:?}"),
    }
}

/// Test 5: a local player's pawn, `bOwnerNoSee` already true: the setter would do nothing, so the pawn test does not
/// decide; nothing, and the component is written.
#[test]
fn hook_2_a_flag_already_set_is_nothing() {
    let mut e = env();
    let rc = e.reattach_cylinder;
    let t = make(&mut e.vm, rc, "Cyl_Template");
    put(&mut e.vm, t, "bOwnerNoSee", Value::Bool(true));
    let p = one_made(&mut e, t);
    controller_for(&mut e, p, true);
    run(&mut e, p).unwrap();
    let made = made_of(&e, &entries(&e, p)[0]);
    assert_eq!(prop(&e.vm, made, "bOwnerNoSee"), Value::Bool(true));
    assert_eq!(prop(&e.vm, made, "bNeedsReattach"), Value::Bool(false), "no setter ran");
}

/// Test 6: a local player's pawn, `bOwnerNoSee` clear: the pawn test decides, its `NotRead` before any write and
/// ahead of the setter's deferred-reattach refusal (the attach sets `Owner`).
#[test]
fn hook_2_a_clear_flag_is_the_pawn_test_s_not_read() {
    let mut e = env();
    let rc = e.reattach_cylinder;
    let t = make(&mut e.vm, rc, "Cyl_Template");
    let p = one_made(&mut e, t);
    controller_for(&mut e, p, true);
    hooks_not_read(&mut e, p, PAWN_TEST);
}

/// Test 7: no controller, a controller that is not a `PlayerController`, a non-local one, and a made object that is
/// not a `PrimitiveComponent`: the gate fails whatever the pawn test says, so nothing. Mode 3 never reaches the hooks
/// (the walk refuses every mode other than 0 first, `MENU_MODES`).
#[test]
fn hook_2_a_failing_known_gate_is_nothing() {
    let rc_flag_clear = |e: &mut Env| {
        let rc = e.reattach_cylinder;
        make(&mut e.vm, rc, "Cyl_Template")
    };
    // No controller.
    let mut e = env();
    let t = rc_flag_clear(&mut e);
    let p = one_made(&mut e, t);
    run(&mut e, p).unwrap();
    // A plain controller.
    let mut e = env();
    let t = rc_flag_clear(&mut e);
    let p = one_made(&mut e, t);
    let c = e.vm.find_class("Engine.Controller").unwrap();
    let ai = make(&mut e.vm, c, "AI_Test");
    put(&mut e.vm, p, "Controller", Value::Object(Some(ai)));
    run(&mut e, p).unwrap();
    // A player controller that is not local (no Player).
    let mut e = env();
    let t = rc_flag_clear(&mut e);
    let p = one_made(&mut e, t);
    controller_for(&mut e, p, false);
    run(&mut e, p).unwrap();
    // A local player, but the made object is not a PrimitiveComponent.
    let mut e = env();
    let ac = e.actor_component;
    let t = make(&mut e.vm, ac, "Plain_Template");
    let p = one_made(&mut e, t);
    controller_for(&mut e, p, true);
    run(&mut e, p).unwrap();
}

/// Test 8: the setter's planner keeps its order: a component class that declares no deferred-reattach fields is the
/// planner's script error even when the flag already equals the value, before any write.
#[test]
fn hook_2_a_malformed_reattach_declaration_is_the_planner_s_error() {
    let mut e = env();
    let cyl = make(&mut e.vm, e.cylinder, "Cyl_Template");
    put(&mut e.vm, cyl, "bOwnerNoSee", Value::Bool(true));
    let p = one_made(&mut e, cyl);
    controller_for(&mut e, p, true);
    let before = Counters::read(&e.vm);
    match run(&mut e, p).unwrap_err() {
        VmError::Script { message, .. } => assert!(message.contains("Scene"), "{message}"),
        other => panic!("{other:?}"),
    }
    assert_eq!(Counters::read(&e.vm), before, "write-free");
    assert!(entries(&e, p).is_empty());
}

/// Test 9: no pawn `LightEnvironment`, a non-mesh component with one set, or a mesh template whose `bAcceptsLights` is
/// clear: nothing, and the light environment is not written.
#[test]
fn hook_3_nothing_without_a_light_environment_a_mesh_or_accepted_lights() {
    // No light environment (the default).
    let mut e = env();
    let skel = make(&mut e.vm, e.skeletal, "Skel_Template");
    put(&mut e.vm, skel, "bAcceptsLights", Value::Bool(true));
    let p = one_made(&mut e, skel);
    run(&mut e, p).unwrap();
    // A cylinder (not a MeshComponent) with lights accepted and a light environment set.
    let mut e = env();
    let cyl = make(&mut e.vm, e.cylinder, "Cyl_Template");
    let p = one_made(&mut e, cyl);
    stop_at_light(&mut e, p, cyl);
    run(&mut e, p).unwrap();
    let made = made_of(&e, &entries(&e, p)[0]);
    assert_eq!(prop(&e.vm, made, "LightEnvironment"), Value::Object(None));
    // A mesh whose bAcceptsLights is clear.
    let mut e = env();
    let skel = make(&mut e.vm, e.skeletal, "Skel_Template");
    let p = one_made(&mut e, skel);
    stop_at_light(&mut e, p, skel);
    put(&mut e.vm, skel, "bAcceptsLights", Value::Bool(false));
    run(&mut e, p).unwrap();
    let made = made_of(&e, &entries(&e, p)[0]);
    assert_eq!(prop(&e.vm, made, "LightEnvironment"), Value::Object(None));
}

/// Test 10: a mesh template with `bAcceptsLights` set reaches the array check: its `NotRead`, before any write.
#[test]
fn hook_3_accepted_lights_reach_the_array_check() {
    let mut e = env();
    let skel = make(&mut e.vm, e.skeletal, "Skel_Template");
    let p = one_made(&mut e, skel);
    stop_at_light(&mut e, p, skel);
    hooks_not_read(&mut e, p, ARRAY_CHECK);
}

/// Test 11: a template the store has not loaded, on a path that needs its values, is the not-settled `NotRead`, with
/// nothing loaded: hook 2 (a local player's cylinder) and hook 3 (a skeletal template with a light environment).
#[test]
fn an_unloaded_template_on_a_hook_path_is_not_settled() {
    let mut e = env();
    let (_, cyl, _) = store_templates(&mut e);
    let p = one_made(&mut e, cyl);
    controller_for(&mut e, p, true);
    hooks_not_read(&mut e, p, NOT_SETTLED);
    assert!(e.vm.defs.is_unloaded(&e.vm.space, cyl), "nothing loaded");

    let mut e = env();
    let (skel, _, _) = store_templates(&mut e);
    let p = one_made(&mut e, skel);
    let light = make(&mut e.vm, e.object, "Light_Test");
    put(&mut e.vm, p, "LightEnvironment", Value::Object(Some(light)));
    hooks_not_read(&mut e, p, NOT_SETTLED);
    assert!(e.vm.defs.is_unloaded(&e.vm.space, skel), "nothing loaded");
}

/// Test 12: a pawn `Controller` not declared as an object is a script error naming it, before any write.
#[test]
fn a_malformed_controller_field_is_a_script_error() {
    let mut e = env_opts(true, true, true);
    let cyl = make(&mut e.vm, e.cylinder, "Cyl_Template");
    let p = one_made(&mut e, cyl);
    let before = Counters::read(&e.vm);
    match run(&mut e, p).unwrap_err() {
        VmError::Script { message, .. } => assert!(message.contains("Controller"), "{message}"),
        other => panic!("{other:?}"),
    }
    assert_eq!(Counters::read(&e.vm), before, "write-free");
}
