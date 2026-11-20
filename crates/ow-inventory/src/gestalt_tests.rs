//! Tests of `BuildCompositeMesh` and `InitGestaltMeshDataFromOther` in Off (lane IMPL-GESTALT-MESH-LAYER) on an
//! invented `Engine.upk` in a synthetic store (AGENTS.md rule 9). The class and field names are the ones the natives
//! read; every object and value is invented. Loading `ToyHolder` makes its references store handles that are not
//! loaded, which is how the residency refusals are reached. AI-assisted (Claude Code, Opus 5.5).

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::{Missing, NotYet, ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_object::ids::{NameId, ObjRef, PropAddr};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_package::testkit::reflect::{property_payload, toy_struct};
use ow_package::testkit::script::ScriptToy;
use ow_package::testkit::{Export, ints};
use ow_vm::api::{FunctionDecl, Vm};
use ow_vm::error::VmError;

use super::*;

static NEXT: AtomicU32 = AtomicU32::new(0);

const LOADED: [&str; 12] = [
    "ToyType", "ToyGestalt", "ToyPartA", "ToyPartB", "ToyPartC", "ToyMeshA", "ToyMeshB", "ToyShared", "ToyComp1",
    "ToyComp2", "ToyWeapon", "ToyMeshObj",
];
const UNLOADED: [&str; 4] = ["UnType", "UnPart", "UnComp", "UnMesh"];

fn package() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&[
        "Class",
        "ScriptStruct",
        "BoolProperty",
        "NameProperty",
        "ObjectProperty",
        "StructProperty",
        "ArrayProperty",
    ]);
    let object = t.class("Object", 0);
    let ty = t.class("ToyWeaponType", object);
    t.prop("Object", ty, "GestaltMesh", 0, object);
    let gestalt = t.class("ToyGestaltDefinition", object);
    t.prop("Object", gestalt, "GestaltSkeletalMesh", 0, object);
    let part = t.class("ToyPart", object);
    t.prop("Bool", part, "bIsGestaltMode", 0, 0);
    t.prop("Object", part, "NongestaltSkeletalMesh", 0, object);
    t.prop("Name", part, "GestaltModeSkeletalMeshName", 0, 0);
    let payload = property_payload(&t.none, 2, 0, &ints(&[0]));
    let mut e = Export::new(t.import("NameProperty"), "AdditionalGestaltModeSkeletalMeshNames", payload);
    e.outer = part;
    t.p.add_export_with(e);
    let mesh = t.class("ToyMesh", object);
    let component = t.class("SkeletalMeshComponent", object);
    let container = toy_struct(&mut t, component, "GestaltDataContainer", None);
    let inner = t.p.next_export();
    t.prop("Array", container, "PartMeshNames", 0, inner + 1);
    t.prop("Name", inner, "PartMeshNames", 0, 0);
    t.prop("Struct", component, "GestaltData", 0, container);
    let weapon = t.class("WillowWeapon", object);
    let data = toy_struct(&mut t, weapon, "WeaponDefinitionData", None);
    t.prop("Object", data, "WeaponTypeDefinition", 0, object);
    for f in WEAPON_ALL_PARTS {
        t.prop("Object", data, f, 0, object);
    }
    t.prop("Struct", weapon, "DefinitionData", 0, data);
    let holder = t.class("Holder", object);
    for n in UNLOADED {
        t.prop("Object", holder, n, 0, object);
    }
    let mut d = 0i32.to_le_bytes().to_vec();
    d.extend_from_slice(&t.none);
    let classes = [ty, gestalt, part, part, part, mesh, mesh, mesh, component, component, weapon, mesh];
    for (n, c) in LOADED.iter().zip(classes) {
        t.p.add_export(c, n, d.clone());
    }
    let mut h = 0i32.to_le_bytes().to_vec();
    for (n, c) in UNLOADED.iter().zip([ty, part, component, mesh]) {
        let x = t.p.add_export(c, n, d.clone());
        h.extend(t.p.object_tag(n, x));
    }
    h.extend_from_slice(&t.none);
    t.p.add_export(holder, "ToyHolder", h);
    t.p.build()
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

fn fx() -> Fx {
    let dir = std::env::temp_dir().join(format!("ow-inventory-gestalt-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let bytes = package();
    let defs = DefStore::from_synthetic(&dir, &[("Engine.upk", &bytes)], IdentityPolicy::FirstInScanOrder, ParityMode::Off).unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
    for n in LOADED.iter().chain(["ToyHolder"].iter()) {
        let path = ObjectPath::parse(&format!("Engine.{n}")).unwrap();
        assert!(vm.defs.find(&mut vm.space, &path).unwrap().is_some(), "{n}");
    }
    let mut x = Fx { vm, dir };
    for n in UNLOADED {
        let o = x.handle(n);
        assert!(x.vm.defs.is_unloaded(&x.vm.space, o), "{n} is an unloaded handle");
    }
    for n in LOADED {
        let o = x.o(n);
        assert!(!x.vm.defs.is_unloaded(&x.vm.space, o), "{n} is loaded");
    }
    x
}

impl Fx {
    /// A loaded export.
    fn o(&mut self, n: &str) -> ObjRef {
        let path = ObjectPath::parse(&format!("Engine.{n}")).unwrap();
        self.vm.defs.find(&mut self.vm.space, &path).unwrap().unwrap()
    }

    /// The holder's reference `n`: an unloaded handle.
    fn handle(&mut self, n: &str) -> ObjRef {
        let holder = self.o("ToyHolder");
        match crate::equip::read_prop(&self.vm, holder, n).unwrap() {
            Value::Object(Some(o)) => o,
            other => panic!("{other:?}"),
        }
    }

    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        self.set_at(o, name, 0, v);
    }

    fn set_at(&mut self, o: ObjRef, name: &str, i: u32, v: Value) {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap();
        self.vm.space.write(o, PropAddr::element(p, i), v).unwrap();
    }

    fn name(&mut self, s: &str) -> NameId {
        self.vm.space.names.intern(s)
    }

    /// Definition data with `fields` set and the rest at their defaults.
    fn data(&mut self, fields: Vec<(&str, Value)>) -> Value {
        let ty = self.vm.space.classes.struct_by_path("Engine.WillowWeapon.WeaponDefinitionData").unwrap();
        let mut v = self.vm.space.classes.struct_defaults(ty).unwrap();
        for (n, x) in fields {
            let id = self.vm.space.names.find(n).unwrap();
            let at = self.vm.space.classes.struct_layout(ty).unwrap().field(id).unwrap().0;
            v.struct_mut().unwrap().fields[at as usize] = x;
        }
        v
    }

    /// The gestalt type: `ToyType` whose `ToyGestalt` has the shared mesh.
    fn gestalt_type(&mut self) -> ObjRef {
        let (ty, g, shared) = (self.o("ToyType"), self.o("ToyGestalt"), self.o("ToyShared"));
        self.set(ty, "GestaltMesh", some(g));
        self.set(g, "GestaltSkeletalMesh", some(shared));
        ty
    }

    /// A heap fingerprint: every slot of every loaded object, to show a call wrote nothing.
    fn snapshot(&mut self) -> Vec<Vec<Value>> {
        LOADED.iter().map(|n| { let o = self.o(n); self.vm.space.heap.get(o).unwrap().slots.clone() }).collect()
    }
}

fn some(o: ObjRef) -> Value {
    Value::Object(Some(o))
}

fn not_yet(e: VmError) -> NotYet {
    match e {
        VmError::NotYet(n) => n,
        other => panic!("expected a NotYet, got {other:?}"),
    }
}

fn layer(e: VmError) -> (String, &'static str) {
    let n = not_yet(e);
    let Missing::LayerAbsent(l) = n.missing else { panic!("not LayerAbsent: {n:?}") };
    (n.what.to_string(), l)
}

fn not_read(e: VmError) -> String {
    let n = not_yet(e);
    assert_eq!(n.missing, Missing::NotRead, "{n:?}");
    n.what.to_string()
}

fn script(e: VmError) -> String {
    match e {
        VmError::Script { message, .. } => message,
        other => panic!("expected a data error, got {other:?}"),
    }
}

/// NATIVE_BUILD_COMPOSITE_MESH.md section 2 step 1: no type answers None, and the component is not looked at.
#[test]
fn no_type_answers_none_without_checking_the_component() {
    let mut x = fx();
    let data = x.data(vec![]);
    assert_eq!(build_composite_mesh(&x.vm, None, &data).unwrap(), None);
}

/// The gestalt path's name list (section 2 step 3), in walk order: a gestalt-mode part's own name and its set
/// additional names; a non-gestalt part's mesh handle name (unloaded here, not loaded by the read); a part with
/// neither gives nothing. Then the answer refuses the absent layer, and nothing is written.
#[test]
fn the_gestalt_path_plans_the_names_then_refuses_the_layer_without_writing() {
    let mut x = fx();
    let ty = x.gestalt_type();
    let (a, b, c, un_mesh) = (x.o("ToyPartA"), x.o("ToyPartB"), x.o("ToyPartC"), x.handle("UnMesh"));
    let (barrel, extra) = (x.name("Toy_Barrel"), x.name("Toy_Extra"));
    x.set(a, "bIsGestaltMode", Value::Bool(true));
    x.set(a, "GestaltModeSkeletalMeshName", Value::Name(barrel));
    x.set_at(a, "AdditionalGestaltModeSkeletalMeshNames", 1, Value::Name(extra));
    x.set(b, "NongestaltSkeletalMesh", some(un_mesh));
    let data = x.data(vec![
        ("WeaponTypeDefinition", some(ty)),
        ("BodyPartDefinition", some(a)),
        ("BarrelPartDefinition", some(c)),
        ("SightPartDefinition", some(b)),
    ]);
    let (g, shared) = (x.o("ToyGestalt"), x.o("ToyShared"));
    let un = x.name("UnMesh");
    assert_eq!(
        composite_plan(&x.vm, &data).unwrap(),
        Some(CompositePlan::Gestalt { definition: g, mesh: shared, names: vec![barrel, extra, un] }),
    );
    assert!(x.vm.defs.is_unloaded(&x.vm.space, un_mesh), "the mesh handle's name is read without a load");
    let comp = x.o("ToyComp1");
    let before = x.snapshot();
    let (what, l) = layer(build_composite_mesh(&x.vm, Some(comp), &data).unwrap_err());
    assert_eq!((what.as_str(), l), (GESTALT_DATA, GESTALT_LAYER));
    assert_eq!(x.snapshot(), before, "a refusal before any write");
}

/// The name parts are not walked (section 2 step 2).
#[test]
fn the_name_parts_are_not_walked() {
    let mut x = fx();
    let ty = x.gestalt_type();
    let a = x.o("ToyPartA");
    let n = x.name("Toy_Prefix");
    x.set(a, "bIsGestaltMode", Value::Bool(true));
    x.set(a, "GestaltModeSkeletalMeshName", Value::Name(n));
    let data = x.data(vec![("WeaponTypeDefinition", some(ty)), ("PrefixPartDefinition", some(a)), ("TitlePartDefinition", some(a))]);
    let Some(CompositePlan::Gestalt { names, .. }) = composite_plan(&x.vm, &data).unwrap() else { panic!() };
    assert!(names.is_empty());
}

/// The non-gestalt path (section 2 step 4): zero meshes None, one mesh that mesh, two the merge layer. A gestalt
/// definition without a shared mesh is the non-gestalt path. The component is never checked there (None passes).
#[test]
fn the_non_gestalt_path_answers_none_one_mesh_or_refuses_the_merge() {
    let mut x = fx();
    let (ty, g) = (x.o("ToyType"), x.o("ToyGestalt"));
    x.set(ty, "GestaltMesh", some(g));
    let (a, b, c) = (x.o("ToyPartA"), x.o("ToyPartB"), x.o("ToyPartC"));
    let (ma, mb) = (x.o("ToyMeshA"), x.o("ToyMeshB"));
    x.set(a, "NongestaltSkeletalMesh", some(ma));
    x.set(b, "NongestaltSkeletalMesh", some(mb));
    // A gestalt-mode part's mesh is not collected.
    x.set(c, "NongestaltSkeletalMesh", some(mb));
    x.set(c, "bIsGestaltMode", Value::Bool(true));
    let t = ("WeaponTypeDefinition", some(ty));
    let zero = x.data(vec![t.clone(), ("GripPartDefinition", some(c))]);
    assert_eq!(build_composite_mesh(&x.vm, None, &zero).unwrap(), None);
    let one = x.data(vec![t.clone(), ("GripPartDefinition", some(c)), ("BarrelPartDefinition", some(a))]);
    assert_eq!(build_composite_mesh(&x.vm, None, &one).unwrap(), Some(ma));
    let two = x.data(vec![t, ("BodyPartDefinition", some(a)), ("BarrelPartDefinition", some(b))]);
    let (what, l) = layer(build_composite_mesh(&x.vm, None, &two).unwrap_err());
    assert_eq!((what.as_str(), l), (MERGE, MERGE_LAYER));
}

/// Residency (section 4, checks 2 to 4): an unloaded type or part is `NotRead`, and the walk's refusal comes before
/// the component's data error.
#[test]
fn an_unloaded_definition_refuses_not_read_before_the_component_is_checked() {
    let mut x = fx();
    let un_type = x.handle("UnType");
    let data = x.data(vec![("WeaponTypeDefinition", some(un_type))]);
    assert_eq!(not_read(build_composite_mesh(&x.vm, None, &data).unwrap_err()), NOT_LOADED);
    let ty = x.gestalt_type();
    let un_part = x.handle("UnPart");
    let data = x.data(vec![("WeaponTypeDefinition", some(ty)), ("StockPartDefinition", some(un_part))]);
    assert_eq!(not_read(build_composite_mesh(&x.vm, None, &data).unwrap_err()), NOT_LOADED);
    assert!(x.vm.defs.is_unloaded(&x.vm.space, un_part), "nothing was loaded");
}

/// The component on the gestalt path (section 4, check 5): None and a wrong class are data errors, an unloaded
/// handle is `NotRead`.
#[test]
fn the_gestalt_path_checks_the_component() {
    let mut x = fx();
    let ty = x.gestalt_type();
    let data = x.data(vec![("WeaponTypeDefinition", some(ty))]);
    assert!(script(build_composite_mesh(&x.vm, None, &data).unwrap_err()).contains("component is None"));
    let mesh = x.o("ToyMeshObj");
    assert!(script(build_composite_mesh(&x.vm, Some(mesh), &data).unwrap_err()).contains(COMPONENT_CLASS));
    let un = x.handle("UnComp");
    assert_eq!(not_read(build_composite_mesh(&x.vm, Some(un), &data).unwrap_err()), COMPONENT_NOT_LOADED);
}

/// A part whose class lacks a field the walk reads is a data error naming it (section 4, check 4).
#[test]
fn a_part_without_a_read_field_is_a_data_error() {
    let mut x = fx();
    let ty = x.gestalt_type();
    let not_a_part = x.o("ToyMeshObj");
    let data = x.data(vec![("WeaponTypeDefinition", some(ty)), ("BodyPartDefinition", some(not_a_part))]);
    assert!(script(build_composite_mesh(&x.vm, None, &data).unwrap_err()).contains("bIsGestaltMode"));
}

/// Check 4 of the fixed copy note (#591 r1): a loaded stand-in whose slots the store has not filled is `NotRead`, and
/// a `GestaltData` value of the wrong shape is the layer's `LayerAbsent`; nothing is written either way.
#[test]
fn the_copy_needs_both_gestalt_slots_held_whole() {
    let mut x = fx();
    let (c1, c2) = (x.o("ToyComp1"), x.o("ToyComp2"));
    let names = x.vm.space.names.intern("Toy_Part");
    let class = x.vm.space.heap.get(c2).unwrap().class;
    let p = x.vm.space.prop(class, "GestaltData").unwrap();
    let mut whole = x.vm.space.read(c2, PropAddr::scalar(p)).unwrap().clone();
    whole.struct_mut().unwrap().fields[0] = Value::array(vec![Value::Name(names)]);
    x.vm.space.write(c2, PropAddr::scalar(p), whole.clone()).unwrap();
    let (what, _) = layer(init_gestalt_mesh_data(&x.vm, c1, Some(c2)).unwrap_err());
    assert_eq!(what, INIT_GESTALT_DATA, "both whole: the absent layer");
    // A malformed nested array element: the slot is not held whole.
    let mut bad = whole;
    bad.struct_mut().unwrap().fields[0] = Value::array(vec![Value::Int(3)]);
    // Placed in the slot directly, past the heap's write checks (a fixture of a malformed decode).
    *x.vm.space.slot_mut(c2, PropAddr::scalar(p)).unwrap() = bad;
    let before = x.snapshot();
    let (what, l) = layer(init_gestalt_mesh_data(&x.vm, c1, Some(c2)).unwrap_err());
    assert_eq!((what.as_str(), l), (INIT_GESTALT_DATA, GESTALT_LAYER));
    assert_eq!(x.snapshot(), before, "nothing written");
    // A stand-in with no slots.
    x.vm.space.heap.get_mut(c2).unwrap().slots.clear();
    assert_eq!(not_read(init_gestalt_mesh_data(&x.vm, c1, Some(c2)).unwrap_err()), NOT_DECODED);
    assert_eq!(not_read(init_gestalt_mesh_data(&x.vm, c2, Some(c1)).unwrap_err()), NOT_DECODED);
    // The self-copy answers before check 4, so even the stand-in succeeds with nothing written.
    init_gestalt_mesh_data(&x.vm, c2, Some(c2)).unwrap();
    assert!(x.vm.space.heap.get(c2).unwrap().slots.is_empty());
}

/// NATIVE_INIT_GESTALT_MESH_DATA.md section 5: the receiver, the argument, the self-copy, then the layer.
#[test]
fn the_copy_checks_both_components_answers_a_self_copy_and_refuses_the_layer() {
    let mut x = fx();
    let (c1, c2, mesh, un) = (x.o("ToyComp1"), x.o("ToyComp2"), x.o("ToyMeshObj"), x.handle("UnComp"));
    assert_eq!(not_read(init_gestalt_mesh_data(&x.vm, un, Some(c1)).unwrap_err()), RECEIVER_NOT_LOADED);
    assert!(script(init_gestalt_mesh_data(&x.vm, mesh, Some(c1)).unwrap_err()).contains("GestaltData"));
    assert!(script(init_gestalt_mesh_data(&x.vm, c1, None).unwrap_err()).contains("ComponentToCopyFrom is None"));
    assert!(script(init_gestalt_mesh_data(&x.vm, c1, Some(mesh)).unwrap_err()).contains(COMPONENT_CLASS));
    assert_eq!(not_read(init_gestalt_mesh_data(&x.vm, c1, Some(un)).unwrap_err()), ARGUMENT_NOT_LOADED);
    let before = x.snapshot();
    init_gestalt_mesh_data(&x.vm, c1, Some(c1)).unwrap();
    let (what, l) = layer(init_gestalt_mesh_data(&x.vm, c2, Some(c1)).unwrap_err());
    assert_eq!((what.as_str(), l), (INIT_GESTALT_DATA, GESTALT_LAYER));
    assert_eq!(x.snapshot(), before, "nothing written");
}

/// Both natives bound in Off and called through the VM; Reference binds neither.
#[test]
fn off_binds_both_natives_and_reference_binds_none() {
    let mut x = fx();
    register(&mut x.vm).unwrap();
    assert!(x.vm.has_native(BUILD_COMPOSITE_MESH) && x.vm.has_native(INIT_GESTALT_MESH_DATA));
    let object = x.vm.space.classes.by_path("Engine.Object").unwrap();
    let weapon = x.vm.space.classes.by_path("Engine.WillowWeapon").unwrap();
    let component = x.vm.space.classes.by_path("Engine.SkeletalMeshComponent").unwrap();
    let data_ty = x.vm.space.classes.struct_by_path("Engine.WillowWeapon.WeaponDefinitionData").unwrap();
    let param = |vm: &mut Vm, n: &str, k: PropKind| PropertyDecl::new(vm.space.names.intern(n), k).with_flags(PropertyDecl::PARAM);
    let params = vec![param(&mut x.vm, "Comp", PropKind::Object { class: object }), param(&mut x.vm, "Data", PropKind::Struct(data_ty))];
    let path = ObjectPath::parse("Engine.WillowWeapon.BuildCompositeMesh").unwrap();
    let f = x.vm.declare_function(FunctionDecl::native(path, BUILD_COMPOSITE_MESH).with_owner(weapon).with_params(params)).unwrap();
    let w = x.o("ToyWeapon");
    let data = x.data(vec![]);
    assert_eq!(x.vm.call(f, Some(w), vec![Value::Object(None), data]).unwrap().ret, Value::Object(None));
    let params = vec![param(&mut x.vm, "ComponentToCopyFrom", PropKind::Object { class: component })];
    let path = ObjectPath::parse("Engine.SkeletalMeshComponent.InitGestaltMeshDataFromOther").unwrap();
    let f = x.vm.declare_function(FunctionDecl::native(path, INIT_GESTALT_MESH_DATA).with_owner(component).with_params(params)).unwrap();
    let c = x.o("ToyComp1");
    x.vm.call(f, Some(c), vec![some(c)]).unwrap();
    let mut y = fx();
    y.vm.parity = ParityMode::Reference;
    register(&mut y.vm).unwrap();
    assert!(!y.vm.has_native(BUILD_COMPOSITE_MESH) && !y.vm.has_native(INIT_GESTALT_MESH_DATA));
}
