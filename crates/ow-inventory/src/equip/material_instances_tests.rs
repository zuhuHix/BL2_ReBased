//! The material natives of NATIVE_INITIALIZE_BEHAVIOR_PROVIDERS.md section 3 in Off, on invented packages in a
//! synthetic store (AGENTS.md rule 9):
//! - `Engine`: `LinearColor` and `Guid`; the material classes (`MaterialInterface`, `MaterialInstance`,
//!   `MaterialInstanceConstant` with `Parent` and `VectorParameterValues`, an unlisted `ToyTimeVarying`, `Material`);
//!   `MeshComponent` with the deferred reattach's fields and `Materials`, `SkeletalMeshComponent` with `SkeletalMesh`,
//!   `StaticMeshComponent`; `SkeletalMesh` with `Materials`; the five natives; a lazy mesh reached through a holder;
//! - `GearboxFramework`: `GearboxSkeletalMeshComponent`;
//! - `WillowGame`: a toy owner with the owner rule's flags.
//!
//! Every value is invented; every call goes through the VM. AI-assisted (Claude Code).

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::notyet::Missing;
use ow_core::{ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_package::testkit::ints;
use ow_package::testkit::reflect::{toy_array_of, toy_struct};
use ow_package::testkit::script::{FunctionSpec, Param, ScriptToy};
use ow_vm::api::Vm;
use ow_vm::error::VmError;
use ow_world::world::component_setters::REATTACH_OWNER;

use super::*;

static NEXT: AtomicU32 = AtomicU32::new(0);
const IMPORTS: [&str; 10] = [
    "Class",
    "Function",
    "ScriptStruct",
    "ObjectProperty",
    "ArrayProperty",
    "StructProperty",
    "NameProperty",
    "FloatProperty",
    "IntProperty",
    "BoolProperty",
];
const NATIVE: u32 = 0x400 | 0x20000;
const GET_PATH: &str = "Engine.MeshComponent.GetMaterial";
const SET_MATERIAL_PATH: &str = "Engine.MeshComponent.SetMaterial";
const INSTANCED_PATH: &str = "Engine.MeshComponent.GetInstancedMaterialInstanceConstant";
const PARENT_PATH: &str = "Engine.MaterialInstance.SetParent";
const VECTOR_PATH: &str = "Engine.MaterialInstance.SetVectorParameterValue";
const MIC: &str = "Engine.MaterialInstanceConstant";

fn engine() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let object = t.class("Object", 0);
    let color = toy_struct(&mut t, object, "LinearColor", None);
    for c in ["R", "G", "B", "A"] {
        t.prop("Float", color, c, 0, 0);
    }
    let guid = toy_struct(&mut t, object, "Guid", None);
    for c in ["A", "B", "C", "D"] {
        t.prop("Int", guid, c, 0, 0);
    }
    let interface = t.class("MaterialInterface", object);
    let instance = t.class("MaterialInstance", interface);
    let mic = t.class("MaterialInstanceConstant", instance);
    t.prop("Object", mic, "Parent", 0, interface);
    let entry = toy_struct(&mut t, mic, "VectorParameterValue", None);
    t.prop("Name", entry, "ParameterName", 0, 0);
    t.prop("Struct", entry, "ParameterValue", 0, color);
    t.prop("Struct", entry, "ExpressionGUID", 0, guid);
    toy_array_of(&mut t, mic, "VectorParameterValues", "Struct", entry);
    t.class("ToyTimeVarying", instance);
    t.class("Material", interface);
    let parent = [Param { type_ref: interface, ..Param::new("Object", "NewParent") }];
    t.function(instance, "SetParent", &parent, None, &[], &FunctionSpec::new(NATIVE, "SetParent"), |_| None);
    let vector = [Param::new("Name", "ParameterName"), Param { type_ref: color, ..Param::new("Struct", "Value") }];
    t.function(instance, "SetVectorParameterValue", &vector, None, &[], &FunctionSpec::new(NATIVE, "SetVectorParameterValue"), |_| None);
    let scene = toy_struct(&mut t, object, "ToyScenePointer", None);
    t.prop("Int", scene, "Ptr", 0, 0);
    let mesh = t.class("MeshComponent", object);
    t.prop("Object", mesh, "Owner", 0, object);
    t.prop("Struct", mesh, "Scene", 0, scene);
    t.prop("Bool", mesh, "bNeedsReattach", 0, 0);
    toy_array_of(&mut t, mesh, "Materials", "Object", interface);
    let set = [Param::new("Int", "ElementIndex"), Param { type_ref: interface, ..Param::new("Object", "Material") }];
    t.function(mesh, "SetMaterial", &set, None, &[], &FunctionSpec::new(NATIVE, "SetMaterial"), |_| None);
    let index = [Param::new("Int", "ElementIndex")];
    t.function(mesh, "GetMaterial", &index, Some(("Object", interface)), &[], &FunctionSpec::new(NATIVE, "GetMaterial"), |_| None);
    t.function(
        mesh,
        "GetInstancedMaterialInstanceConstant",
        &index,
        Some(("Object", mic)),
        &[],
        &FunctionSpec::new(NATIVE, "GetInstancedMaterialInstanceConstant"),
        |_| None,
    );
    let skeletal_mesh = t.class("SkeletalMesh", object);
    toy_array_of(&mut t, skeletal_mesh, "Materials", "Object", interface);
    let skeletal = t.class("SkeletalMeshComponent", mesh);
    t.prop("Object", skeletal, "SkeletalMesh", 0, skeletal_mesh);
    t.class("StaticMeshComponent", mesh);
    // A lazy mesh, reached through a holder.
    let holder = t.class("ToyHolder", object);
    t.prop("Object", holder, "Mesh", 0, skeletal_mesh);
    let mut empty = ints(&[0]);
    empty.extend_from_slice(&t.none);
    let lazy = t.p.add_export(skeletal_mesh, "Mesh_Lazy", empty);
    let mut h = ints(&[0]);
    h.extend(t.p.object_tag("Mesh", lazy));
    h.extend_from_slice(&t.none);
    t.p.add_export(holder, "ToyHolderObject", h);
    t.build()
}

fn gearbox() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let skeletal = t.p.add_import("Core", "Class", engine, "SkeletalMeshComponent");
    t.class("GearboxSkeletalMeshComponent", skeletal);
    t.build()
}

fn willow() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let object = t.p.add_import("Core", "Class", engine, "Object");
    let owner = t.class("ToyOwner", object);
    for flag in ["bStatic", "bTickIsDisabled", "bDeleteMe"] {
        t.prop("Bool", owner, flag, 0, 0);
    }
    t.build()
}

struct Fx {
    vm: Vm,
    dir: PathBuf,
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// A VM over the three packages in `mode`, every class loaded, both material modules registered. With `warm`, one
/// `MaterialInstanceConstant` is made, so its default object is built (the creation branch's readiness).
fn fx_with(mode: ParityMode, warm: bool) -> Fx {
    let dir = std::env::temp_dir().join(format!("ow-inventory-mic-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let (e, g, w) = (engine(), gearbox(), willow());
    let files: Vec<(&str, &[u8])> = vec![("Engine.upk", &e), ("GearboxFramework.upk", &g), ("WillowGame.upk", &w)];
    let defs = DefStore::from_synthetic(&dir, &files, IdentityPolicy::FirstInScanOrder, mode).unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), mode);
    for c in [
        "Engine.MaterialInterface",
        "Engine.MaterialInstance",
        MIC,
        "Engine.ToyTimeVarying",
        "Engine.Material",
        "Engine.MeshComponent",
        "Engine.SkeletalMeshComponent",
        "Engine.StaticMeshComponent",
        "Engine.SkeletalMesh",
        "GearboxFramework.GearboxSkeletalMeshComponent",
        "WillowGame.ToyOwner",
    ] {
        vm.find_class(c).unwrap();
    }
    super::super::material_natives::register(&mut vm).unwrap();
    register(&mut vm).unwrap();
    let mut x = Fx { vm, dir };
    if warm {
        x.obj(MIC);
    }
    x
}

fn fx() -> Fx {
    fx_with(ParityMode::Off, true)
}

impl Fx {
    fn obj(&mut self, class: &str) -> ObjRef {
        let c = self.vm.find_class(class).unwrap();
        ow_vm::interp::instantiate_from_defaults(&mut self.vm, c).unwrap()
    }

    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap();
        self.vm.space.write(o, PropAddr::scalar(p), v).unwrap();
    }

    fn get(&self, o: ObjRef, name: &str) -> Value {
        crate::equip::read_prop(&self.vm, o, name).unwrap()
    }

    fn name(&mut self, text: &str) -> Value {
        Value::Name(self.vm.space.names.intern(text))
    }

    fn list(items: &[Option<ObjRef>]) -> Value {
        Value::array(items.iter().map(|i| Value::Object(*i)).collect())
    }

    /// A component of `class` with its own `Materials` and a mesh whose `Materials` is `mesh` (no mesh for `None`).
    fn component(&mut self, class: &str, own: &[Option<ObjRef>], mesh: Option<&[Option<ObjRef>]>) -> ObjRef {
        let c = self.obj(class);
        self.set(c, "Materials", Self::list(own));
        if let Some(entries) = mesh {
            let m = self.obj("Engine.SkeletalMesh");
            self.set(m, "Materials", Self::list(entries));
            self.set(c, "SkeletalMesh", Value::Object(Some(m)));
        }
        c
    }

    fn call(&mut self, path: &str, this: ObjRef, args: Vec<Value>) -> Result<Value, VmError> {
        let f = self.vm.find_function(path).unwrap();
        Ok(self.vm.call(f, Some(this), args)?.ret)
    }

    fn get_material(&mut self, c: ObjRef, index: i32) -> Result<Value, VmError> {
        self.call(GET_PATH, c, vec![Value::Int(index)])
    }

    fn instanced(&mut self, c: ObjRef, index: i32) -> Result<Value, VmError> {
        self.call(INSTANCED_PATH, c, vec![Value::Int(index)])
    }

    fn set_parent(&mut self, m: ObjRef, parent: Option<ObjRef>) -> Result<Value, VmError> {
        self.call(PARENT_PATH, m, vec![Value::Object(parent)])
    }

    fn color(&self, rgba: [f32; 4]) -> Value {
        let ty = self.vm.space.classes.struct_by_path("Engine.Object.LinearColor").unwrap();
        let layout = self.vm.space.classes.struct_layout(ty).unwrap();
        let mut v = layout.zero().clone();
        // By name: the toy package's field order is not R, G, B, A.
        for (c, x) in ["R", "G", "B", "A"].into_iter().zip(rgba) {
            let at = layout.field(self.vm.space.names.find(c).unwrap()).unwrap().0 as usize;
            v.struct_mut().unwrap().fields[at] = Value::Float(x);
        }
        v
    }

    fn set_vector(&mut self, m: ObjRef, name: &str, rgba: [f32; 4]) -> Result<Value, VmError> {
        let name = self.name(name);
        let color = self.color(rgba);
        self.call(VECTOR_PATH, m, vec![name, color])
    }

    /// The `VectorParameterValues` entries as (name, R, G, B, A).
    fn vectors(&self, m: ObjRef) -> Vec<(String, [f32; 4])> {
        let list = self.get(m, "VectorParameterValues");
        list.elements()
            .unwrap()
            .iter()
            .map(|e| {
                let f = |n: &str| ow_attr::eval::field(&self.vm.space, e, n).unwrap().clone();
                let name = match f("ParameterName") {
                    Value::Name(n) => self.vm.space.names.text(n).into_owned(),
                    other => panic!("{other:?}"),
                };
                let value = f("ParameterValue");
                let c = |n: &str| match ow_attr::eval::field(&self.vm.space, &value, n).unwrap() {
                    Value::Float(x) => *x,
                    other => panic!("{other:?}"),
                };
                (name, [c("R"), c("G"), c("B"), c("A")])
            })
            .collect()
    }

    fn lazy_mesh(&mut self) -> ObjRef {
        let path = ObjectPath::parse("Engine.ToyHolderObject").unwrap();
        let holder = self.vm.defs.find(&mut self.vm.space, &path).unwrap().unwrap();
        match self.get(holder, "Mesh") {
            Value::Object(Some(o)) => {
                assert!(self.vm.defs.is_unloaded(&self.vm.space, o));
                o
            }
            other => panic!("{other:?}"),
        }
    }

    /// #518's counters, the class-code loads, the name table and the heap's object count.
    fn marks(&self) -> (u64, u64, u64, u64, usize, usize) {
        let vm = &self.vm;
        (
            vm.space.heap.committed_writes(),
            vm.rng.draws(),
            vm.defs.load_transitions(),
            vm.class_code_loads(),
            vm.space.names.len(),
            vm.space.heap.handles().len(),
        )
    }
}

/// A `NotRead` with `what`, and nothing moved (no write, draw, load or intern, no object made).
fn refused(x: &mut Fx, run: impl FnOnce(&mut Fx) -> Result<Value, VmError>, what: &str) {
    let before = x.marks();
    match run(x) {
        Err(VmError::NotYet(n)) => {
            assert_eq!(n.missing, Missing::NotRead, "{n}");
            assert_eq!(&*n.what, what, "{n}");
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(x.marks(), before, "nothing moved: {what}");
}

fn object(v: Value) -> ObjRef {
    match v {
        Value::Object(Some(o)) => o,
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_five_keys_bind_in_off_and_none_of_the_new_ones_in_reference() {
    let mut x = fx();
    for (path, key) in [
        (GET_PATH, GET_MATERIAL),
        (PARENT_PATH, SET_PARENT),
        (VECTOR_PATH, SET_VECTOR_PARAMETER_VALUE),
        (INSTANCED_PATH, GET_INSTANCED),
    ] {
        let f = x.vm.find_function(path).unwrap();
        assert_eq!(x.vm.function(f).unwrap().native_key.as_deref(), Some(key), "{path}");
        assert!(x.vm.has_native(key), "{key}");
    }
    let x = fx_with(ParityMode::Reference, false);
    for key in [GET_MATERIAL, SET_PARENT, SET_VECTOR_PARAMETER_VALUE, GET_INSTANCED] {
        assert!(!x.vm.has_native(key), "{key}");
    }
}

// ---- 3.1 GetMaterial ----------------------------------------------------------------------------------------------

#[test]
fn get_material_override_then_mesh_then_none() {
    let mut x = fx();
    let (a, b, m) = (x.obj("Engine.Material"), x.obj("Engine.Material"), x.obj("Engine.Material"));
    let c = x.component("Engine.SkeletalMeshComponent", &[Some(a), None], Some(&[Some(m), Some(b)]));
    let before = x.marks();
    // An override entry wins.
    assert_eq!(x.get_material(c, 0).unwrap(), Value::Object(Some(a)));
    // A None override entry falls through to the mesh's entry.
    assert_eq!(x.get_material(c, 1).unwrap(), Value::Object(Some(b)));
    // Past both lists, and a negative index: None.
    assert_eq!(x.get_material(c, 2).unwrap(), Value::Object(None));
    assert_eq!(x.get_material(c, -1).unwrap(), Value::Object(None));
    assert_eq!(x.marks(), before, "nothing written");
    // No mesh: None.
    let bare = x.component("Engine.SkeletalMeshComponent", &[None], None);
    assert_eq!(x.get_material(bare, 0).unwrap(), Value::Object(None));
    // The Gearbox component keeps the rule.
    let g = x.component("GearboxFramework.GearboxSkeletalMeshComponent", &[], Some(&[Some(m)]));
    let before_g = x.marks();
    assert_eq!(x.get_material(g, 0).unwrap(), Value::Object(Some(m)));
    assert_eq!(x.marks(), before_g, "nothing written");
}

#[test]
fn get_material_on_another_class_or_an_unloaded_mesh_is_not_read() {
    let mut x = fx();
    let s = x.component("Engine.StaticMeshComponent", &[], None);
    refused(&mut x, |x| x.get_material(s, 0), &get_material_for("Engine.StaticMeshComponent"));
    let lazy = x.lazy_mesh();
    let c = x.component("Engine.SkeletalMeshComponent", &[], None);
    x.set(c, "SkeletalMesh", Value::Object(Some(lazy)));
    refused(&mut x, |x| x.get_material(c, 0), &not_loaded("MeshComponent.GetMaterial"));
    assert!(x.vm.defs.is_unloaded(&x.vm.space, lazy), "still unloaded");
    // An override entry answers without reading the mesh.
    let a = x.obj("Engine.Material");
    x.set(c, "Materials", Fx::list(&[Some(a)]));
    assert_eq!(x.get_material(c, 0).unwrap(), Value::Object(Some(a)));
}

// ---- 3.2 SetParent ------------------------------------------------------------------------------------------------

#[test]
fn set_parent_writes_only_a_new_parent() {
    let mut x = fx();
    let (m, p) = (x.obj(MIC), x.obj("Engine.Material"));
    let before = x.marks();
    x.set_parent(m, Some(p)).unwrap();
    assert_eq!(x.get(m, "Parent"), Value::Object(Some(p)));
    assert_eq!(x.marks().0, before.0 + 1, "one write");
    let before = x.marks();
    x.set_parent(m, Some(p)).unwrap();
    assert_eq!(x.marks(), before, "the same parent writes nothing");
    let tv = x.obj("Engine.ToyTimeVarying");
    refused(&mut x, |x| x.set_parent(tv, Some(p)), &set_parent_for("Engine.ToyTimeVarying"));
}

// ---- 3.3 GetInstancedMaterialInstanceConstant ---------------------------------------------------------------------

#[test]
fn an_instance_the_component_owns_is_returned_with_nothing_written() {
    let mut x = fx();
    let c = x.component("Engine.SkeletalMeshComponent", &[], None);
    let m = x.obj(MIC);
    x.vm.space.heap.get_mut(m).unwrap().outer = Some(c);
    x.set(c, "Materials", Fx::list(&[Some(m)]));
    let before = x.marks();
    assert_eq!(x.instanced(c, 0).unwrap(), Value::Object(Some(m)));
    assert_eq!(x.marks(), before);
}

#[test]
fn another_owner_s_instance_or_a_plain_material_gets_a_new_instance() {
    for plain in [false, true] {
        let mut x = fx();
        let c = x.component("Engine.SkeletalMeshComponent", &[], None);
        let elsewhere = x.obj("WillowGame.ToyOwner");
        let m = if plain { x.obj("Engine.Material") } else { x.obj(MIC) };
        x.vm.space.heap.get_mut(m).unwrap().outer = Some(elsewhere);
        x.set(c, "Materials", Fx::list(&[Some(m)]));
        let made = object(x.instanced(c, 0).unwrap());
        assert_ne!(made, m);
        let o = x.vm.space.heap.get(made).unwrap();
        assert_eq!(o.outer, Some(c), "the component is its Outer");
        assert_eq!(x.vm.space.classes.get_layout(o.class).unwrap().path().as_str(), MIC);
        assert_eq!(x.get(made, "Parent"), Value::Object(Some(m)));
        assert_eq!(x.get(c, "Materials"), Fx::list(&[Some(made)]));
        assert_eq!(x.get(c, "bNeedsReattach"), Value::Bool(true));
    }
}

#[test]
fn an_index_past_the_end_grows_the_list() {
    let mut x = fx();
    let mat = x.obj("Engine.Material");
    let c = x.component("Engine.SkeletalMeshComponent", &[], Some(&[None, None, Some(mat)]));
    let made = object(x.instanced(c, 2).unwrap());
    assert_eq!(x.get(c, "Materials"), Fx::list(&[None, None, Some(made)]));
    assert_eq!(x.get(made, "Parent"), Value::Object(Some(mat)), "the mesh's entry is the parent");
}

#[test]
fn a_static_owner_is_the_reattach_refusal_with_no_object_made() {
    let mut x = fx();
    let owner = x.obj("WillowGame.ToyOwner");
    x.set(owner, "bStatic", Value::Bool(true));
    let c = x.component("Engine.SkeletalMeshComponent", &[], None);
    x.set(c, "Owner", Value::Object(Some(owner)));
    refused(&mut x, |x| x.instanced(c, 0), REATTACH_OWNER);
}

#[test]
fn a_negative_index_still_makes_an_instance_and_leaves_materials() {
    let mut x = fx();
    let a = x.obj("Engine.Material");
    let c = x.component("Engine.SkeletalMeshComponent", &[Some(a)], None);
    let made = object(x.instanced(c, -1).unwrap());
    assert_eq!(x.vm.space.heap.get(made).unwrap().outer, Some(c));
    assert_eq!(x.get(made, "Parent"), Value::Object(None));
    assert_eq!(x.get(c, "Materials"), Fx::list(&[Some(a)]), "unchanged");
}

#[test]
fn the_instance_class_not_ready_is_not_read() {
    let mut x = fx_with(ParityMode::Off, false);
    let c = x.component("Engine.SkeletalMeshComponent", &[], None);
    refused(&mut x, |x| x.instanced(c, 0), &instance_class_not_ready());
    // A component of another class is GetMaterial's refusal, first.
    let mut x = fx();
    let s = x.component("Engine.StaticMeshComponent", &[], None);
    refused(&mut x, |x| x.instanced(s, 0), &get_material_for("Engine.StaticMeshComponent"));
}

// ---- 3.4 SetVectorParameterValue ----------------------------------------------------------------------------------

#[test]
fn set_vector_appends_updates_the_first_match_and_skips_the_same_value() {
    let mut x = fx();
    let m = x.obj(MIC);
    x.set_vector(m, "PrimaryColor", [0.1, 0.2, 0.3, 1.0]).unwrap();
    assert_eq!(x.vectors(m), vec![("PrimaryColor".to_string(), [0.1, 0.2, 0.3, 1.0])]);
    // The appended entry's ExpressionGUID is zero.
    let list = x.get(m, "VectorParameterValues");
    let guid = ow_attr::eval::field(&x.vm.space, &list.elements().unwrap()[0], "ExpressionGUID").unwrap().clone();
    let ty = x.vm.space.classes.struct_by_path("Engine.Object.Guid").unwrap();
    assert_eq!(&guid, x.vm.space.classes.struct_layout(ty).unwrap().zero());
    // Two entries of one name: the first is updated.
    x.set_vector(m, "Secondary", [0.0; 4]).unwrap();
    let doubled = {
        let mut l = list.elements().unwrap().to_vec();
        l.push(l[0].clone());
        l
    };
    x.set(m, "VectorParameterValues", Value::array(doubled));
    x.set_vector(m, "PrimaryColor", [0.5, 0.5, 0.5, 0.5]).unwrap();
    let v = x.vectors(m);
    assert_eq!(v[0], ("PrimaryColor".to_string(), [0.5; 4]));
    assert_eq!(v[1], ("PrimaryColor".to_string(), [0.1, 0.2, 0.3, 1.0]), "the second is untouched");
    // The same value: no write.
    let before = x.marks();
    x.set_vector(m, "PrimaryColor", [0.5, 0.5, 0.5, 0.5]).unwrap();
    assert_eq!(x.marks(), before);
}

#[test]
fn set_vector_with_a_nan_always_writes_and_other_classes_are_not_read() {
    let mut x = fx();
    let m = x.obj(MIC);
    x.set_vector(m, "ThirdColor", [f32::NAN, 0.0, 0.0, 1.0]).unwrap();
    let before = x.marks();
    x.set_vector(m, "ThirdColor", [f32::NAN, 0.0, 0.0, 1.0]).unwrap();
    assert_eq!(x.marks().0, before.0 + 1, "a NaN never compares equal");
    let tv = x.obj("Engine.ToyTimeVarying");
    let name = x.name("ThirdColor");
    let color = x.color([0.0; 4]);
    refused(&mut x, |x| x.call(VECTOR_PATH, tv, vec![name, color]), &set_vector_for("Engine.ToyTimeVarying"));
}

// ---- 3.5 The arms' two instances ----------------------------------------------------------------------------------

/// The arms material on `maya:8`'s path: both `Materials` lists empty. The first instance takes `GetMaterial(0)`'s
/// None (no write), the second takes the first as its parent (one write); each is set at index 0.
#[test]
fn the_arms_two_instances() {
    let mut x = fx();
    let arms = x.component("GearboxFramework.GearboxSkeletalMeshComponent", &[], Some(&[]));
    let first = x.obj(MIC);
    let parent = x.get_material(arms, 0).unwrap();
    assert_eq!(parent, Value::Object(None));
    let before = x.marks();
    x.call(PARENT_PATH, first, vec![parent]).unwrap();
    assert_eq!(x.marks(), before, "a None parent onto a None Parent writes nothing");
    x.call(SET_MATERIAL_PATH, arms, vec![Value::Int(0), Value::Object(Some(first))]).unwrap();
    let second = x.obj(MIC);
    let before = x.marks();
    x.set_parent(second, Some(first)).unwrap();
    assert_eq!(x.marks().0, before.0 + 1, "one Parent write");
    x.call(SET_MATERIAL_PATH, arms, vec![Value::Int(0), Value::Object(Some(second))]).unwrap();
    assert_eq!(x.get(arms, "Materials"), Fx::list(&[Some(second)]));
    assert_eq!(x.get(second, "Parent"), Value::Object(Some(first)));
}
