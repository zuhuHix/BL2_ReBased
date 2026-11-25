//! `BuildWeaponMaterial` in Off (NATIVE_WEAPON_VISUALS.md section 1.1.2, the note's tests 1 to 9, and the hub's points
//! on #698's review) on two invented packages in a synthetic store (AGENTS.md rule 9):
//! - `Engine`: `Object`, `MaterialInterface`, `MaterialInstanceConstant` (`Parent`), `ToyMIC` and `ToyColdMIC` under it,
//!   `ToyNotMIC` outside the family, `SkeletalMeshComponent`, and a parent material export `Mati_Toy`;
//! - `WillowGame`: `WillowWeapon` with the static native and its `WeaponDefinitionData`, `WeaponTypeDefinition`
//!   (`MaterialClass`), the part classes with `Material` and `MaterialVectorParameterValues`, and a holder whose
//!   references stay store handles.
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
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ObjRef, StructId};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_package::testkit::ints;
use ow_package::testkit::reflect::{toy_array_of, toy_struct};
use ow_package::testkit::script::{FunctionSpec, Param, ScriptToy};
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use super::*;

static NEXT: AtomicU32 = AtomicU32::new(0);
const IMPORTS: [&str; 10] = [
    "Class",
    "Function",
    "ScriptStruct",
    "ObjectProperty",
    "ClassProperty",
    "ArrayProperty",
    "StructProperty",
    "NameProperty",
    "FloatProperty",
    "IntProperty",
];
const PATH: &str = "WillowGame.WillowWeapon.BuildWeaponMaterial";

fn engine() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let object = t.class("Object", 0);
    let interface = t.class("MaterialInterface", object);
    let mic = t.class("MaterialInstanceConstant", interface);
    t.prop("Object", mic, "Parent", 0, interface);
    t.class("ToyMIC", mic);
    t.class("ToyColdMIC", mic);
    let not_mic = t.class("ToyNotMIC", interface);
    t.prop("Object", not_mic, "Parent", 0, interface);
    t.class("SkeletalMeshComponent", object);
    let mut empty = ints(&[0]);
    empty.extend_from_slice(&t.none);
    t.p.add_export(mic, "Mati_Toy", empty);
    t.build()
}

/// The `WillowGame` package; with `mistyped`, `MaterialVectorParameterValues` is an array of ints.
fn willow(mistyped: bool) -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let object = t.p.add_import("Core", "Class", engine, "Object");
    let interface = t.p.add_import("Core", "Class", engine, "MaterialInterface");
    let mic = t.p.add_import("Core", "Class", engine, "MaterialInstanceConstant");
    let part = t.class("WillowInventoryPartDefinition", object);
    t.prop("Object", part, "Material", 0, mic);
    if mistyped {
        toy_array_of(&mut t, part, "MaterialVectorParameterValues", "Int", 0);
    } else {
        let vpv = toy_struct(&mut t, part, "VectorParameterValue", None);
        t.prop("Name", vpv, "ParameterName", 0, 0);
        t.prop("Float", vpv, "ParameterValue", 0, 0);
        toy_array_of(&mut t, part, "MaterialVectorParameterValues", "Struct", vpv);
    }
    let weapon_part = t.class("WeaponPartDefinition", part);
    let name_part = t.class("WeaponNamePartDefinition", part);
    let weapon_type = t.class("WeaponTypeDefinition", object);
    t.prop_tail("Class", weapon_type, "MaterialClass", 0, &ints(&[object, interface]));
    let weapon = t.class("WillowWeapon", object);
    let data = toy_struct(&mut t, weapon, "WeaponDefinitionData", None);
    t.prop("Object", data, "WeaponTypeDefinition", 0, weapon_type);
    for f in crate::equip::WEAPON_PART_FIELDS {
        t.prop("Object", data, f, 0, weapon_part);
    }
    for f in ["PrefixPartDefinition", "TitlePartDefinition"] {
        t.prop("Object", data, f, 0, name_part);
    }
    let params = [
        Param { type_ref: object, ..Param::new("Object", "Weapon") },
        Param { type_ref: object, ..Param::new("Object", "Mesh") },
        Param { type_ref: data, ..Param::new("Struct", "WeaponData") },
    ];
    let spec = FunctionSpec::new(0x400 | 0x20000 | 0x2000, "BuildWeaponMaterial");
    t.function(weapon, "BuildWeaponMaterial", &params, Some(("Object", object)), &[], &spec, |_| None);
    // Store handles: a Material part and a material, held by a holder export.
    let holder = t.class("ToyHolder", object);
    t.prop("Object", holder, "Part", 0, weapon_part);
    t.prop("Object", holder, "Mat", 0, mic);
    let mut empty = ints(&[0]);
    empty.extend_from_slice(&t.none);
    let stored_part = t.p.add_export(weapon_part, "ToyStoredPart", empty.clone());
    let stored_mat = t.p.add_export(mic, "ToyStoredMat", empty);
    let mut h = ints(&[0]);
    h.extend(t.p.object_tag("Part", stored_part));
    h.extend(t.p.object_tag("Mat", stored_mat));
    h.extend_from_slice(&t.none);
    t.p.add_export(holder, "ToyHolderObject", h);
    t.build()
}

struct Fx {
    vm: Vm,
    dir: PathBuf,
    data: StructId,
    weapon: ObjRef,
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn fx_with(mode: ParityMode, mistyped: bool) -> Fx {
    let dir = std::env::temp_dir().join(format!("ow-inventory-bwm-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let (e, w) = (engine(), willow(mistyped));
    let files: Vec<(&str, &[u8])> = vec![("Engine.upk", &e), ("WillowGame.upk", &w)];
    let defs = DefStore::from_synthetic(&dir, &files, IdentityPolicy::FirstInScanOrder, mode).unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), mode);
    for c in [
        "Engine.MaterialInstanceConstant",
        "Engine.ToyMIC",
        "Engine.ToyNotMIC",
        "Engine.SkeletalMeshComponent",
        "WillowGame.WeaponPartDefinition",
        "WillowGame.WeaponNamePartDefinition",
        "WillowGame.WeaponTypeDefinition",
        "WillowGame.WillowWeapon",
    ] {
        vm.find_class(c).unwrap();
    }
    // The material class's default object, as the weapon's set-up would have it (the construction copies it).
    vm.defs.class_default(&mut vm.space, "Engine.ToyMIC").unwrap();
    register(&mut vm).unwrap();
    let data = vm.space.classes.struct_by_path("WillowGame.WillowWeapon.WeaponDefinitionData").unwrap();
    let weapon = {
        let c = vm.space.classes.by_path("WillowGame.WillowWeapon").unwrap();
        ow_vm::interp::instantiate_from_defaults(&mut vm, c).unwrap()
    };
    Fx { vm, dir, data, weapon }
}

fn fx() -> Fx {
    fx_with(ParityMode::Off, false)
}

impl Fx {
    fn obj(&mut self, class: &str) -> ObjRef {
        let c = self.vm.find_class(class).unwrap();
        ow_vm::interp::instantiate_from_defaults(&mut self.vm, c).unwrap()
    }

    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap();
        self.vm.space.write(o, ow_object::ids::PropAddr::scalar(p), v).unwrap();
    }

    fn get(&self, o: ObjRef, name: &str) -> Value {
        crate::equip::read_prop(&self.vm, o, name).unwrap()
    }

    fn export(&mut self, path: &str) -> ObjRef {
        let path = ObjectPath::parse(path).unwrap();
        self.vm.defs.find(&mut self.vm.space, &path).unwrap().unwrap()
    }

    /// The holder's reference `name`: a store handle, not loaded.
    fn handle(&mut self, name: &str) -> ObjRef {
        let holder = self.export("WillowGame.ToyHolderObject");
        match self.get(holder, name) {
            Value::Object(Some(o)) => {
                assert!(self.vm.defs.is_unloaded(&self.vm.space, o));
                o
            }
            other => panic!("{other:?}"),
        }
    }

    /// A weapon type whose `MaterialClass` is the class at `class` (None for no class).
    fn weapon_type(&mut self, class: Option<&str>) -> ObjRef {
        let t = self.obj("WillowGame.WeaponTypeDefinition");
        let c = class.map(|p| self.vm.space.classes.by_path(p).unwrap());
        self.set(t, "MaterialClass", Value::Class(c));
        t
    }

    /// A weapon part of `class` with `Material` `material` and one vector parameter when `entry`.
    fn part(&mut self, class: &str, material: Option<ObjRef>, entry: bool) -> ObjRef {
        let p = self.obj(class);
        self.set(p, "Material", Value::Object(material));
        if entry {
            let owner = self.vm.space.classes.by_path("WillowGame.WillowInventoryPartDefinition").unwrap();
            let p_decl = self.vm.space.prop(owner, "MaterialVectorParameterValues").unwrap();
            let PropKind::Array(inner) = self.vm.space.classes.decl(p_decl).unwrap().kind.clone() else { panic!() };
            let PropKind::Struct(ty) = *inner else { panic!() };
            let mut e = self.vm.space.classes.struct_defaults(ty).unwrap();
            let name = Value::Name(self.vm.space.names.intern("p_EmissiveColor"));
            let id = self.vm.space.names.find("ParameterName").unwrap();
            let at = self.vm.space.classes.struct_layout(ty).unwrap().field(id).unwrap().0;
            e.struct_mut().unwrap().fields[at as usize] = name;
            self.set(p, "MaterialVectorParameterValues", Value::array(vec![e]));
        }
        p
    }

    /// The definition data with `fields`.
    fn data(&self, fields: &[(&str, Option<ObjRef>)]) -> Value {
        let mut v = self.vm.space.classes.struct_defaults(self.data).unwrap();
        let layout = self.vm.space.classes.struct_layout(self.data).unwrap();
        for (name, o) in fields {
            let id = self.vm.space.names.find(name).unwrap();
            let at = layout.field(id).unwrap().0;
            v.struct_mut().unwrap().fields[at as usize] = Value::Object(*o);
        }
        v
    }

    fn call(&mut self, weapon: Option<ObjRef>, mesh: Option<ObjRef>, data: Value) -> Result<Value, VmError> {
        let f = self.vm.find_function(PATH).unwrap();
        Ok(self.vm.call(f, None, vec![Value::Object(weapon), Value::Object(mesh), data])?.ret)
    }

    /// #518's counters, the class-code loads, the name table and the heap.
    fn state(&self) -> (u64, u64, u64, u64, usize, String) {
        let vm = &self.vm;
        (
            vm.space.heap.committed_writes(),
            vm.rng.draws(),
            vm.defs.load_transitions(),
            vm.class_code_loads(),
            vm.space.names.len(),
            format!("{:?}", vm.space.heap),
        )
    }

    /// A preflight answer or refusal with nothing changed.
    fn same(&mut self, weapon: Option<ObjRef>, mesh: Option<ObjRef>, data: Value) -> Result<Value, VmError> {
        let before = self.state();
        let r = self.call(weapon, mesh, data);
        assert_eq!(self.state(), before, "nothing changed: {r:?}");
        r
    }

    fn objects(&self) -> usize {
        self.vm.space.heap.handles().len()
    }
}

fn not_yet(r: Result<Value, VmError>) -> (Missing, String) {
    match r {
        Err(VmError::NotYet(n)) => (n.missing, n.what.to_string()),
        other => panic!("expected NotYet, got {other:?}"),
    }
}

/// Test 1: no `WeaponTypeDefinition`: None, nothing created (and no weapon needed).
#[test]
fn no_weapon_type_answers_none() {
    let mut x = fx();
    let data = x.data(&[]);
    assert_eq!(x.same(None, None, data).unwrap(), Value::Object(None));
}

/// Test 2 (with the hub's point 4: a mesh component passed, as `InitMeshes` always does): a Material part with a
/// `Material` and a `ToyMIC` class: one new object of that class, its outer the weapon, its `Parent` that material; the
/// heap grows by one object, the class's code and default untouched.
#[test]
fn a_material_part_s_material_is_the_parent() {
    let mut x = fx();
    let weapon_type = x.weapon_type(Some("Engine.ToyMIC"));
    let mat = x.export("Engine.Mati_Toy");
    let part = x.part("WillowGame.WeaponPartDefinition", Some(mat), false);
    let mesh = x.obj("Engine.SkeletalMeshComponent");
    let data = x.data(&[("WeaponTypeDefinition", Some(weapon_type)), ("MaterialPartDefinition", Some(part))]);
    let (objects, loads) = (x.objects(), x.vm.class_code_loads());
    let weapon = x.weapon;
    let Value::Object(Some(instance)) = x.call(Some(weapon), Some(mesh), data).unwrap() else { panic!("an instance") };
    assert_eq!(x.objects(), objects + 1, "one new object");
    assert_eq!(x.vm.class_code_loads(), loads, "no class code loaded");
    let o = x.vm.space.heap.get(instance).unwrap();
    assert_eq!((o.class, o.outer), (x.vm.space.classes.by_path("Engine.ToyMIC").unwrap(), Some(weapon)));
    assert_eq!(x.get(instance, "Parent"), Value::Object(Some(mat)));
}

/// Test 3, and a Material part whose `Material` is None (the hub's point 4): no mesh component: `Parent` None.
#[test]
fn no_material_and_no_mesh_gives_no_parent() {
    let mut x = fx();
    let weapon_type = x.weapon_type(Some("Engine.ToyMIC"));
    let weapon = x.weapon;
    let empty = x.part("WillowGame.WeaponPartDefinition", None, false);
    for part in [None, Some(empty)] {
        let data = x.data(&[("WeaponTypeDefinition", Some(weapon_type)), ("MaterialPartDefinition", part)]);
        let Value::Object(Some(instance)) = x.call(Some(weapon), None, data).unwrap() else { panic!() };
        assert_eq!(x.get(instance, "Parent"), Value::Object(None), "{part:?}");
    }
}

/// Test 4: no Material part's material, a mesh component passed: the material-0 `NotRead`, nothing created.
#[test]
fn a_mesh_without_a_material_part_is_not_read() {
    let mut x = fx();
    let weapon_type = x.weapon_type(Some("Engine.ToyMIC"));
    let mesh = x.obj("Engine.SkeletalMeshComponent");
    let weapon = x.weapon;
    let empty = x.part("WillowGame.WeaponPartDefinition", None, false);
    for part in [None, Some(empty)] {
        let data = x.data(&[("WeaponTypeDefinition", Some(weapon_type)), ("MaterialPartDefinition", part)]);
        assert_eq!(not_yet(x.same(Some(weapon), Some(mesh), data)), (Missing::NotRead, MESH_MATERIAL.to_string()));
    }
}

/// Test 5, and the hub's point 1: `MaterialClass` None, a class outside the family, and a cold `MaterialClass` (its
/// layout defined, its code not loaded) are each `NotRead` naming it, with no class-code load.
#[test]
fn the_class_refusals() {
    let mut x = fx();
    let weapon = x.weapon;
    let none = x.weapon_type(None);
    let data = x.data(&[("WeaponTypeDefinition", Some(none))]);
    assert_eq!(not_yet(x.same(Some(weapon), None, data)), (Missing::NotRead, NO_CLASS.to_string()));
    let not_mic = x.weapon_type(Some("Engine.ToyNotMIC"));
    let data = x.data(&[("WeaponTypeDefinition", Some(not_mic))]);
    assert_eq!(not_yet(x.same(Some(weapon), None, data)), (Missing::NotRead, outside_family("Engine.ToyNotMIC")));
    let cold = {
        let vm = &mut x.vm;
        let (code, store) = vm.defs.code_mut();
        let key = code.find_class(store, "Engine.ToyColdMIC").unwrap();
        code.define_layout(store, &mut vm.space.names, &mut vm.space.classes, key).unwrap()
    };
    assert!(!x.vm.class_code_loaded(cold));
    let t = x.obj("WillowGame.WeaponTypeDefinition");
    x.set(t, "MaterialClass", Value::Class(Some(cold)));
    let data = x.data(&[("WeaponTypeDefinition", Some(t))]);
    assert_eq!(not_yet(x.same(Some(weapon), None, data)), (Missing::NotRead, class_not_ready("Engine.ToyColdMIC")));
    assert!(!x.vm.class_code_loaded(cold), "still cold");
}

/// The hub's point 1: a ready class whose default object is not built yet is refused too (the construction would
/// build it, a load).
#[test]
fn a_class_whose_default_is_not_built_is_not_ready() {
    let mut x = fx();
    let weapon = x.weapon;
    x.vm.find_class("Engine.ToyColdMIC").unwrap();
    let t = x.weapon_type(Some("Engine.ToyColdMIC"));
    let class = x.vm.space.classes.by_path("Engine.ToyColdMIC").unwrap();
    assert!(!x.vm.space.heap.handles().into_iter().any(|o| matches!(x.vm.space.heap.get(o).unwrap().origin, Origin::Default { class: c } if c == class)));
    let data = x.data(&[("WeaponTypeDefinition", Some(t))]);
    assert_eq!(not_yet(x.same(Some(weapon), None, data)), (Missing::NotRead, class_not_ready("Engine.ToyColdMIC")));
}

/// Test 6: an elemental part with one `p_EmissiveColor` entry: the setter's `NotRead`, nothing created.
#[test]
fn a_vector_parameter_is_not_read() {
    let mut x = fx();
    let weapon_type = x.weapon_type(Some("Engine.ToyMIC"));
    let elemental = x.part("WillowGame.WeaponPartDefinition", None, true);
    let weapon = x.weapon;
    let data = x.data(&[("WeaponTypeDefinition", Some(weapon_type)), ("ElementalPartDefinition", Some(elemental))]);
    assert_eq!(not_yet(x.same(Some(weapon), None, data)), (Missing::NotRead, VECTOR_SETTER.to_string()));
}

/// The hub's point 4: a name part carrying an entry, and a part whose array is empty, refuse nothing.
#[test]
fn name_parts_and_empty_arrays_refuse_nothing() {
    let mut x = fx();
    let weapon_type = x.weapon_type(Some("Engine.ToyMIC"));
    let prefix = x.part("WillowGame.WeaponNamePartDefinition", None, true);
    let body = x.part("WillowGame.WeaponPartDefinition", None, false);
    let weapon = x.weapon;
    let data = x.data(&[
        ("WeaponTypeDefinition", Some(weapon_type)),
        ("PrefixPartDefinition", Some(prefix)),
        ("BodyPartDefinition", Some(body)),
    ]);
    assert!(matches!(x.call(Some(weapon), None, data).unwrap(), Value::Object(Some(_))));
}

/// Test 7: a lazy Material part, or a lazy `Material`: the "not loaded" `NotRead`, nothing loaded.
#[test]
fn lazy_parts_and_materials_are_not_read() {
    let mut x = fx();
    let weapon_type = x.weapon_type(Some("Engine.ToyMIC"));
    let weapon = x.weapon;
    let lazy_part = x.handle("Part");
    let data = x.data(&[("WeaponTypeDefinition", Some(weapon_type)), ("MaterialPartDefinition", Some(lazy_part))]);
    assert_eq!(not_yet(x.same(Some(weapon), None, data)), (Missing::NotRead, NOT_LOADED.to_string()));
    let lazy_mat = x.handle("Mat");
    let part = x.part("WillowGame.WeaponPartDefinition", Some(lazy_mat), false);
    let data = x.data(&[("WeaponTypeDefinition", Some(weapon_type)), ("MaterialPartDefinition", Some(part))]);
    assert_eq!(not_yet(x.same(Some(weapon), None, data)), (Missing::NotRead, NOT_LOADED.to_string()));
    for o in [lazy_part, lazy_mat] {
        assert!(x.vm.defs.is_unloaded(&x.vm.space, o), "still unloaded");
    }
}

/// Test 8: `MaterialVectorParameterValues` not an array of `VectorParameterValue`: a script error naming it.
#[test]
fn a_mistyped_vector_parameter_array_is_a_script_error() {
    let mut x = fx_with(ParityMode::Off, true);
    let weapon_type = x.weapon_type(Some("Engine.ToyMIC"));
    let body = x.obj("WillowGame.WeaponPartDefinition");
    let weapon = x.weapon;
    let data = x.data(&[("WeaponTypeDefinition", Some(weapon_type)), ("BodyPartDefinition", Some(body))]);
    match x.same(Some(weapon), None, data) {
        Err(VmError::Script { message, .. }) => assert!(message.contains("MaterialVectorParameterValues"), "{message}"),
        other => panic!("{other:?}"),
    }
}

/// The hub's point 3: no weapon passed (the game's default outer): `NotRead`, nothing created.
#[test]
fn no_weapon_is_not_read() {
    let mut x = fx();
    let weapon_type = x.weapon_type(Some("Engine.ToyMIC"));
    let data = x.data(&[("WeaponTypeDefinition", Some(weapon_type))]);
    assert_eq!(not_yet(x.same(None, None, data)), (Missing::NotRead, NO_WEAPON.to_string()));
}

/// Test 9: Reference binds nothing.
#[test]
fn reference_binds_nothing() {
    let x = fx_with(ParityMode::Reference, false);
    assert!(!x.vm.has_native(KEY));
}
