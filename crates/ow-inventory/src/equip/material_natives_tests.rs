//! `SetMaterial` and `GetScalarParameterValue` in Off (NATIVE_INITMESHES_MATERIAL_GLOW.md section 5's tests 1 to 14)
//! on invented packages in a synthetic store (AGENTS.md rule 9):
//! - `Engine`: the material classes (`MaterialInterface`, `MaterialInstance`, `MaterialInstanceConstant` with `Parent`,
//!   `ReentrantFlag` and `ScalarParameterValues`, `Material` with `Expressions`, the expression classes, an unlisted
//!   `ToyTimeVarying`); `MeshComponent` with the deferred reattach's fields and `Materials`, and two malformed mesh
//!   classes; the two natives; and lazy exports reached through a holder;
//! - `WillowGame`: a toy weapon with `FirstPersonMesh` and `ThirdPersonMesh` (5a).
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
use ow_world::world::component_setters::{REATTACH_OWNER, REATTACH_SCENE};

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
const OUT: u64 = 0x100;
const SET_PATH: &str = "Engine.MeshComponent.SetMaterial";
const GET_PATH: &str = "Engine.MaterialInterface.GetScalarParameterValue";

fn engine() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let object = t.class("Object", 0);
    let interface = t.class("MaterialInterface", object);
    let instance = t.class("MaterialInstance", interface);
    let mic = t.class("MaterialInstanceConstant", instance);
    t.prop("Object", mic, "Parent", 0, interface);
    t.prop("Bool", mic, "ReentrantFlag", 0, 0);
    let entry = toy_struct(&mut t, mic, "ScalarParameterValue", None);
    t.prop("Name", entry, "ParameterName", 0, 0);
    t.prop("Float", entry, "ParameterValue", 0, 0);
    toy_array_of(&mut t, mic, "ScalarParameterValues", "Struct", entry);
    t.class("ToyTimeVarying", instance);
    let expression = t.class("MaterialExpression", object);
    let scalar = t.class("MaterialExpressionScalarParameter", expression);
    t.prop("Name", scalar, "ParameterName", 0, 0);
    t.prop("Float", scalar, "DefaultValue", 0, 0);
    // A scalar-expression subclass that shadows ParameterName with an int (review of #718).
    let int_name = t.class("ToyIntNameScalar", scalar);
    t.prop("Int", int_name, "ParameterName", 0, 0);
    let other = t.class("MaterialExpressionToyVector", expression);
    t.prop("Name", other, "ParameterName", 0, 0);
    let material = t.class("Material", interface);
    toy_array_of(&mut t, material, "Expressions", "Object", expression);
    let get = [Param::new("Name", "ParameterName"), Param { flags: OUT, ..Param::new("Float", "OutValue") }];
    t.function(interface, "GetScalarParameterValue", &get, Some(("Bool", 0)), &[], &FunctionSpec::new(NATIVE, "GetScalarParameterValue"), |_| None);
    // The mesh component with the deferred reattach's fields, and two malformed ones.
    let scene = toy_struct(&mut t, object, "ToyScenePointer", None);
    t.prop("Int", scene, "Ptr", 0, 0);
    let mesh = t.class("MeshComponent", object);
    t.prop("Object", mesh, "Owner", 0, object);
    t.prop("Struct", mesh, "Scene", 0, scene);
    t.prop("Bool", mesh, "bNeedsReattach", 0, 0);
    toy_array_of(&mut t, mesh, "Materials", "Object", interface);
    let set = [Param::new("Int", "ElementIndex"), Param { type_ref: interface, ..Param::new("Object", "Material") }];
    t.function(mesh, "SetMaterial", &set, None, &[], &FunctionSpec::new(NATIVE, "SetMaterial"), |_| None);
    let bad_list = t.class("ToyIntMaterialsMesh", object);
    t.prop("Object", bad_list, "Owner", 0, object);
    t.prop("Struct", bad_list, "Scene", 0, scene);
    t.prop("Bool", bad_list, "bNeedsReattach", 0, 0);
    toy_array_of(&mut t, bad_list, "Materials", "Int", 0);
    let bad_flag = t.class("ToyIntFlagMesh", object);
    t.prop("Object", bad_flag, "Owner", 0, object);
    t.prop("Struct", bad_flag, "Scene", 0, scene);
    t.prop("Int", bad_flag, "bNeedsReattach", 0, 0);
    toy_array_of(&mut t, bad_flag, "Materials", "Object", interface);
    let bad_owner = t.class("ToyIntOwnerMesh", object);
    t.prop("Int", bad_owner, "Owner", 0, 0);
    t.prop("Struct", bad_owner, "Scene", 0, scene);
    t.prop("Bool", bad_owner, "bNeedsReattach", 0, 0);
    toy_array_of(&mut t, bad_owner, "Materials", "Object", interface);
    let bad_scene = t.class("ToyIntSceneMesh", object);
    t.prop("Object", bad_scene, "Owner", 0, object);
    t.prop("Int", bad_scene, "Scene", 0, 0);
    t.prop("Bool", bad_scene, "bNeedsReattach", 0, 0);
    toy_array_of(&mut t, bad_scene, "Materials", "Object", interface);
    // Lazy exports: a MaterialInstanceConstant and a scalar expression, reached through a holder.
    let holder = t.class("ToyHolder", object);
    t.prop("Object", holder, "Mic", 0, mic);
    t.prop("Object", holder, "Expr", 0, scalar);
    let mut empty = ints(&[0]);
    empty.extend_from_slice(&t.none);
    let lazy_mic = t.p.add_export(mic, "Mati_Lazy", empty.clone());
    let lazy_expr = t.p.add_export(scalar, "Expr_Lazy", empty);
    let mut h = ints(&[0]);
    h.extend(t.p.object_tag("Mic", lazy_mic));
    h.extend(t.p.object_tag("Expr", lazy_expr));
    h.extend_from_slice(&t.none);
    t.p.add_export(holder, "ToyHolderObject", h);
    t.build()
}

fn willow() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let object = t.p.add_import("Core", "Class", engine, "Object");
    let mesh = t.p.add_import("Core", "Class", engine, "MeshComponent");
    let weapon = t.class("ToyWeapon", object);
    // The owner rule's flags (#714, `component_setters::OWNER_FLAGS`).
    for flag in ["bStatic", "bTickIsDisabled", "bDeleteMe"] {
        t.prop("Bool", weapon, flag, 0, 0);
    }
    t.prop("Object", weapon, "FirstPersonMesh", 0, mesh);
    t.prop("Object", weapon, "ThirdPersonMesh", 0, mesh);
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

fn fx_in(mode: ParityMode) -> Fx {
    let dir = std::env::temp_dir().join(format!("ow-inventory-glow-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let (e, w) = (engine(), willow());
    let files: Vec<(&str, &[u8])> = vec![("Engine.upk", &e), ("WillowGame.upk", &w)];
    let defs = DefStore::from_synthetic(&dir, &files, IdentityPolicy::FirstInScanOrder, mode).unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), mode);
    // Every class but the scalar expression, which a test warms by making one (test 10 needs it cold).
    for c in [
        "Engine.MaterialInterface",
        "Engine.MaterialInstance",
        "Engine.MaterialInstanceConstant",
        "Engine.ToyTimeVarying",
        "Engine.Material",
        "Engine.MaterialExpressionToyVector",
        "Engine.MeshComponent",
        "Engine.ToyIntMaterialsMesh",
        "Engine.ToyIntFlagMesh",
        "Engine.ToyIntOwnerMesh",
        "Engine.ToyIntSceneMesh",
        "WillowGame.ToyWeapon",
    ] {
        vm.find_class(c).unwrap();
    }
    register(&mut vm).unwrap();
    Fx { vm, dir }
}

fn fx() -> Fx {
    fx_in(ParityMode::Off)
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

    /// A `ScalarParameterValues` entry.
    fn entry(&mut self, name: &str, value: f32) -> Value {
        let ty = self.vm.space.classes.struct_by_path("Engine.MaterialInstanceConstant.ScalarParameterValue").unwrap();
        let mut e = self.vm.space.classes.struct_defaults(ty).unwrap();
        let layout = self.vm.space.classes.struct_layout(ty).unwrap();
        let (n, v) = (self.vm.space.names.find("ParameterName").unwrap(), self.vm.space.names.find("ParameterValue").unwrap());
        let (at_n, at_v) = (layout.field(n).unwrap().0 as usize, layout.field(v).unwrap().0 as usize);
        let name = self.name(name);
        let fields = &mut e.struct_mut().unwrap().fields;
        fields[at_n] = name;
        fields[at_v] = Value::Float(value);
        e
    }

    /// A `MaterialInstanceConstant` with `entries` and `parent`.
    fn mic(&mut self, entries: &[(&str, f32)], parent: Option<ObjRef>) -> ObjRef {
        let m = self.obj("Engine.MaterialInstanceConstant");
        let list = entries.iter().map(|(n, v)| self.entry(n, *v)).collect();
        self.set(m, "ScalarParameterValues", Value::array(list));
        self.set(m, "Parent", Value::Object(parent));
        m
    }

    /// A mesh component with `owner` and a `Scene` pointer `scene`.
    fn mesh(&mut self, owner: Option<ObjRef>, scene: i32) -> ObjRef {
        let m = self.obj("Engine.MeshComponent");
        self.set(m, "Owner", Value::Object(owner));
        self.set_scene(m, scene);
        m
    }

    fn set_scene(&mut self, m: ObjRef, scene: i32) {
        let ty = self.vm.space.classes.struct_by_path("Engine.Object.ToyScenePointer").unwrap();
        let mut s = self.vm.space.classes.struct_defaults(ty).unwrap();
        s.struct_mut().unwrap().fields[0] = Value::Int(scene);
        self.set(m, "Scene", s);
    }

    fn set_material(&mut self, mesh: ObjRef, index: i32, material: Option<ObjRef>) -> Result<Value, VmError> {
        let f = self.vm.find_function(SET_PATH).unwrap();
        Ok(self.vm.call(f, Some(mesh), vec![Value::Int(index), Value::Object(material)])?.ret)
    }

    /// The glow read: the answer and the out value, the out variable starting at `prior`.
    fn scalar(&mut self, material: ObjRef, name: &str, prior: f32) -> Result<(Value, Value), VmError> {
        let f = self.vm.find_function(GET_PATH).unwrap();
        let name = self.name(name);
        let r = self.vm.call(f, Some(material), vec![name, Value::Float(prior)])?;
        Ok((r.ret, r.outs[1].clone()))
    }

    /// The holder's reference `name`: a store handle, not loaded.
    fn handle(&mut self, name: &str) -> ObjRef {
        let path = ObjectPath::parse("Engine.ToyHolderObject").unwrap();
        let holder = self.vm.defs.find(&mut self.vm.space, &path).unwrap().unwrap();
        match self.get(holder, name) {
            Value::Object(Some(o)) => {
                assert!(self.vm.defs.is_unloaded(&self.vm.space, o));
                o
            }
            other => panic!("{other:?}"),
        }
    }

    /// #518's counters, the class-code loads and the name table.
    fn marks(&self) -> (u64, u64, u64, u64, usize) {
        let vm = &self.vm;
        (vm.space.heap.committed_writes(), vm.rng.draws(), vm.defs.load_transitions(), vm.class_code_loads(), vm.space.names.len())
    }
}

/// A refusal of kind `NotRead` with `what`, and nothing moved.
fn refused(x: &mut Fx, run: impl FnOnce(&mut Fx) -> Result<Value, VmError>, what: &str) {
    let before = x.marks();
    match run(x) {
        Err(VmError::NotYet(n)) => {
            assert_eq!(n.missing, Missing::NotRead, "{n}");
            assert_eq!(&*n.what, what, "{n}");
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(x.marks(), before, "nothing written, drawn, loaded or interned: {what}");
}

#[test]
fn both_natives_bind_on_the_package_s_keys() {
    let mut x = fx();
    for (path, key) in [(SET_PATH, SET_MATERIAL), (GET_PATH, GET_SCALAR_PARAMETER_VALUE)] {
        let f = x.vm.find_function(path).unwrap();
        assert_eq!(x.vm.function(f).unwrap().native_key.as_deref(), Some(key), "{path}");
        assert!(x.vm.has_native(key), "{key}");
    }
}

/// Tests 1 and 2: a negative index, and an entry that already holds the material (even with an `Owner`): nothing.
#[test]
fn a_negative_index_or_the_same_entry_is_nothing() {
    let mut x = fx();
    let m = x.mic(&[], None);
    let mesh = x.mesh(None, 0);
    let before = x.marks();
    assert_eq!(x.set_material(mesh, -1, Some(m)).unwrap(), Value::None);
    assert_eq!(x.marks(), before);
    let owner = x.obj("WillowGame.ToyWeapon");
    let held = x.mesh(Some(owner), 0);
    x.set(held, "Materials", Value::array(vec![Value::Object(Some(m))]));
    let before = x.marks();
    x.set_material(held, 0, Some(m)).unwrap();
    assert_eq!(x.marks(), before, "no reattach refusal and no write");
}

/// Test 3: index 2 on an empty `Materials`, no `Owner`, no `Scene`: (None, None, M), and `bNeedsReattach`.
#[test]
fn a_past_the_end_index_grows_the_list_with_none() {
    let mut x = fx();
    let m = x.mic(&[], None);
    let mesh = x.mesh(None, 0);
    x.set_material(mesh, 2, Some(m)).unwrap();
    assert_eq!(x.get(mesh, "Materials"), Value::array(vec![Value::Object(None), Value::Object(None), Value::Object(Some(m))]));
    assert_eq!(x.get(mesh, "bNeedsReattach"), Value::Bool(true));
}

/// Test 4, on the deferred reattach as #714's owner rule reads it: a static `Owner` (not deleted) is `REATTACH_OWNER`;
/// no `Owner` with a `Scene` is `REATTACH_SCENE`; nothing written. A dynamic `Owner` (the three flags clear) passes
/// the owner rule, so the entry and `bNeedsReattach` are written.
#[test]
fn the_deferred_reattach_refuses_before_any_write() {
    let mut x = fx();
    let m = x.mic(&[], None);
    let owner = x.obj("WillowGame.ToyWeapon");
    x.set(owner, "bStatic", Value::Bool(true));
    let with_owner = x.mesh(Some(owner), 0);
    refused(&mut x, |x| x.set_material(with_owner, 0, Some(m)), REATTACH_OWNER);
    let dynamic = x.obj("WillowGame.ToyWeapon");
    let with_dynamic = x.mesh(Some(dynamic), 0);
    x.set_material(with_dynamic, 0, Some(m)).unwrap();
    assert_eq!(x.get(with_dynamic, "Materials"), Value::array(vec![Value::Object(Some(m))]));
    assert_eq!(x.get(with_dynamic, "bNeedsReattach"), Value::Bool(true));
    let with_scene = x.mesh(None, 7);
    refused(&mut x, |x| x.set_material(with_scene, 0, Some(m)), REATTACH_SCENE);
    assert_eq!(x.get(with_scene, "Materials"), Value::array(vec![]));
}

/// Test 5: `Materials` not an array of objects, `bNeedsReattach` not a bool, `Owner` not an object, or `Scene` not a
/// struct: a script error naming it, even for a negative index.
#[test]
fn malformed_declarations_are_script_errors_first() {
    for (class, needle) in [
        ("Engine.ToyIntMaterialsMesh", "Materials"),
        ("Engine.ToyIntFlagMesh", "bNeedsReattach"),
        ("Engine.ToyIntOwnerMesh", "Owner"),
        ("Engine.ToyIntSceneMesh", "Scene"),
    ] {
        let mut x = fx();
        let mesh = x.obj(class);
        let f = x.vm.find_function(SET_PATH).unwrap();
        let before = x.marks();
        match x.vm.call(f, Some(mesh), vec![Value::Int(-1), Value::Object(None)]) {
            Err(VmError::Script { message, .. }) => assert!(message.contains(needle), "{message}"),
            other => panic!("{class}: {other:?}"),
        }
        assert_eq!(x.marks(), before, "{class}");
    }
}

/// Test 5a: link 8's call shape, a toy weapon's two meshes then the glow read on the same instance.
#[test]
fn link_8_s_call_shape_two_meshes_then_the_glow() {
    // (i) No Owner and no Scene on either mesh.
    let mut x = fx();
    let parent = x.mic(&[("p_EmissiveScale", 2.5)], None);
    let m = x.mic(&[], Some(parent));
    let weapon = x.obj("WillowGame.ToyWeapon");
    let (first, third) = (x.mesh(None, 0), x.mesh(None, 0));
    x.set(weapon, "FirstPersonMesh", Value::Object(Some(first)));
    x.set(weapon, "ThirdPersonMesh", Value::Object(Some(third)));
    for mesh in [first, third] {
        x.set_material(mesh, 0, Some(m)).unwrap();
        assert_eq!(x.get(mesh, "Materials"), Value::array(vec![Value::Object(Some(m))]));
        assert_eq!(x.get(mesh, "bNeedsReattach"), Value::Bool(true));
    }
    assert_eq!(x.scalar(m, "p_EmissiveScale", 1.0).unwrap(), (Value::Bool(true), Value::Float(2.5)));
    let orphan = x.mic(&[], None);
    assert_eq!(x.scalar(orphan, "p_EmissiveScale", 1.0).unwrap(), (Value::Bool(false), Value::Float(1.0)));
    // (ii) The first mesh with a static Owner (#714's owner rule): the first call refuses, which stops the script, so
    // `InitMeshes` makes no second call; the test makes only the first. Neither mesh is written: the
    // `ThirdPersonMesh` is untouched.
    let mut x = fx();
    let m = x.mic(&[], None);
    let weapon = x.obj("WillowGame.ToyWeapon");
    x.set(weapon, "bStatic", Value::Bool(true));
    let (first, third) = (x.mesh(Some(weapon), 0), x.mesh(None, 0));
    refused(&mut x, |x| x.set_material(first, 0, Some(m)), REATTACH_OWNER);
    assert_eq!(x.get(first, "Materials"), Value::array(vec![]));
    assert_eq!(x.get(third, "Materials"), Value::array(vec![]), "the ThirdPersonMesh is untouched");
}

/// Test 6: an entry for the name: true and the out value; the first of two wins; a different number does not match.
#[test]
fn an_instance_s_own_entry_answers_first_wins() {
    let mut x = fx();
    let m = x.mic(&[("p_Glow", 3.0), ("p_Glow", 4.0)], None);
    assert_eq!(x.scalar(m, "p_Glow", 0.0).unwrap(), (Value::Bool(true), Value::Float(3.0)));
    let numbered = x.mic(&[("p_Glow_2", 9.0)], None);
    assert_eq!(x.scalar(numbered, "p_Glow", 0.5).unwrap(), (Value::Bool(false), Value::Float(0.5)));
}

/// Tests 7 to 9: the parent answers, the flag is unchanged; a cycle is false with no loop; a set flag is false.
#[test]
fn the_parent_chain_answers_and_its_guard_writes_nothing() {
    let mut x = fx();
    let parent = x.mic(&[("p_Glow", 7.0)], None);
    let m = x.mic(&[], Some(parent));
    let before = x.marks();
    assert_eq!(x.scalar(m, "p_Glow", 0.0).unwrap(), (Value::Bool(true), Value::Float(7.0)));
    assert_eq!(x.get(m, "ReentrantFlag"), Value::Bool(false), "the flag is unchanged");
    assert_eq!(x.marks().0, before.0, "the walk writes nothing (the out value goes to the host)");
    // A cycle.
    let a = x.mic(&[], None);
    let b = x.mic(&[], Some(a));
    x.set(a, "Parent", Value::Object(Some(b)));
    assert_eq!(x.scalar(a, "p_Glow", 0.25).unwrap(), (Value::Bool(false), Value::Float(0.25)));
    // A set flag.
    let flagged = x.mic(&[("p_Glow", 1.0)], None);
    x.set(flagged, "ReentrantFlag", Value::Bool(true));
    assert_eq!(x.scalar(flagged, "p_Glow", 0.0).unwrap().0, Value::Bool(false));
}

/// Test 10: a `Material`'s expressions. A scalar expression with the name answers its default; a non-scalar one with
/// the name is skipped; a None entry is skipped. With the scalar class cold and an entry to test: the class
/// `NotRead`; with only None entries: false, and no class check.
#[test]
fn a_material_answers_from_its_scalar_expressions() {
    // The scalar class cold: only None entries, then a non-None one.
    let mut x = fx();
    let material = x.obj("Engine.Material");
    x.set(material, "Expressions", Value::array(vec![Value::Object(None)]));
    assert_eq!(x.scalar(material, "p_Glow", 0.5).unwrap(), (Value::Bool(false), Value::Float(0.5)));
    let vector = x.obj("Engine.MaterialExpressionToyVector");
    x.set(material, "Expressions", Value::array(vec![Value::Object(None), Value::Object(Some(vector))]));
    let f = x.vm.find_function(GET_PATH).unwrap();
    let name = x.name("p_Glow");
    refused(&mut x, |x| x.vm.call(f, Some(material), vec![name, Value::Float(0.0)]).map(|r| r.ret), &expression_class_not_ready());
    // The scalar class ready.
    let mut x = fx();
    let material = x.obj("Engine.Material");
    let vector = x.obj("Engine.MaterialExpressionToyVector");
    let glow = x.name("p_Glow");
    x.set(vector, "ParameterName", glow.clone());
    let scalar = x.obj(SCALAR_EXPRESSION);
    x.set(scalar, "ParameterName", glow);
    x.set(scalar, "DefaultValue", Value::Float(6.0));
    let list = vec![Value::Object(None), Value::Object(Some(vector)), Value::Object(Some(scalar))];
    x.set(material, "Expressions", Value::array(list));
    assert_eq!(x.scalar(material, "p_Glow", 0.0).unwrap(), (Value::Bool(true), Value::Float(6.0)));
    // A parent Material answers for an instance.
    let m = x.mic(&[], Some(material));
    assert_eq!(x.scalar(m, "p_Glow", 0.0).unwrap(), (Value::Bool(true), Value::Float(6.0)));
}

/// Test 11: no match anywhere: false, the out variable keeps its prior value.
#[test]
fn no_match_keeps_the_out_value() {
    let mut x = fx();
    let base = x.obj("Engine.MaterialInterface");
    let m = x.mic(&[("p_Other", 1.0)], Some(base));
    assert_eq!(x.scalar(m, "p_Glow", 4.5).unwrap(), (Value::Bool(false), Value::Float(4.5)));
    let instance = x.obj("Engine.MaterialInstance");
    assert_eq!(x.scalar(instance, "p_Glow", 4.5).unwrap(), (Value::Bool(false), Value::Float(4.5)));
}

/// Test 12: a lazy `Parent`, or a lazy expression: the not-loaded `NotRead`, nothing loaded.
#[test]
fn a_lazy_parent_or_expression_is_not_loaded() {
    let mut x = fx();
    let lazy = x.handle("Mic");
    let m = x.mic(&[], Some(lazy));
    let f = x.vm.find_function(GET_PATH).unwrap();
    let name = x.name("p_Glow");
    refused(&mut x, |x| x.vm.call(f, Some(m), vec![name.clone(), Value::Float(0.0)]).map(|r| r.ret), SCALAR_NOT_LOADED);
    assert!(x.vm.defs.is_unloaded(&x.vm.space, lazy));
    let expr = x.handle("Expr");
    let material = x.obj("Engine.Material");
    x.set(material, "Expressions", Value::array(vec![Value::Object(Some(expr))]));
    refused(&mut x, |x| x.vm.call(f, Some(material), vec![name, Value::Float(0.0)]).map(|r| r.ret), SCALAR_NOT_LOADED);
    assert!(x.vm.defs.is_unloaded(&x.vm.space, expr));
}

/// Test 13: a receiver of an unlisted first native class: the class `NotRead`.
#[test]
fn an_unlisted_material_class_is_not_read() {
    let mut x = fx();
    let m = x.obj("Engine.ToyTimeVarying");
    let f = x.vm.find_function(GET_PATH).unwrap();
    let name = x.name("p_Glow");
    refused(&mut x, |x| x.vm.call(f, Some(m), vec![name, Value::Float(0.0)]).map(|r| r.ret), &scalar_lookup_of("Engine.ToyTimeVarying"));
}

/// Test 14: Reference binds neither key.
#[test]
fn reference_binds_nothing() {
    let x = fx_in(ParityMode::Reference);
    assert!(!x.vm.has_native(SET_MATERIAL) && !x.vm.has_native(GET_SCALAR_PARAMETER_VALUE));
}

/// Review of #718: an expression whose `ParameterName` is not a name (here a scalar-expression subclass that shadows it
/// with an int) is a script error naming the field, before any write.
#[test]
fn a_parameter_name_that_is_not_a_name_is_a_script_error() {
    let mut x = fx();
    let _ready = x.obj(SCALAR_EXPRESSION);
    let odd = x.obj("Engine.ToyIntNameScalar");
    let material = x.obj("Engine.Material");
    x.set(material, "Expressions", Value::array(vec![Value::Object(Some(odd))]));
    let _ = x.name("p_Glow"); // interned before the marks: the call's own name argument
    let before = x.marks();
    match x.scalar(material, "p_Glow", 0.0) {
        Err(VmError::Script { message, .. }) => assert!(message.contains("ParameterName"), "{message}"),
        other => panic!("{other:?}"),
    }
    assert_eq!(x.marks(), before);
}
