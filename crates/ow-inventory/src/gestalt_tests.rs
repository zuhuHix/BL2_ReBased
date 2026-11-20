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
use ow_package::testkit::reflect::{property_payload, toy_array_of, toy_struct};
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
const UNLOADED: [&str; 5] = ["UnType", "UnPart", "UnComp", "UnMesh", "UnGestalt"];

/// The package; without `gestalt_data` the component class declares no `GestaltData`.
/// A fixture variant with one member declared with the wrong kind (review r1 of #613, item 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mis {
    No,
    /// The container's `ReferencePoseBounds` declared as an int.
    BoundsInt,
    /// The gestalt definition's `GestaltInfos` declared as an array of ints.
    InfosInt,
}

fn package_mis(gestalt_data: bool, mis: Mis) -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&[
        "Class",
        "ScriptStruct",
        "BoolProperty",
        "NameProperty",
        "ObjectProperty",
        "StructProperty",
        "ArrayProperty",
        "IntProperty",
        "FloatProperty",
    ]);
    let object = t.class("Object", 0);
    let ty = t.class("ToyWeaponType", object);
    t.prop("Object", ty, "GestaltMesh", 0, object);
    let gestalt = t.class("ToyGestaltDefinition", object);
    t.prop("Object", gestalt, "GestaltSkeletalMesh", 0, object);
    let bare = t.class("ToyBareGestaltDefinition", object);
    t.prop("Object", bare, "GestaltSkeletalMesh", 0, object);
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
    // The container's members in the note's declared order (NATIVE_GESTALT_DATA_LAYER.md section 1), with invented
    // struct names for the nested structs.
    let vector = toy_struct(&mut t, component, "ToyVector", None);
    for a in ["X", "Y", "Z"] {
        t.prop("Float", vector, a, 0, 0);
    }
    let bounds = toy_struct(&mut t, component, "ToyBounds", None);
    t.prop("Struct", bounds, "Origin", 0, vector);
    t.prop("Struct", bounds, "BoxExtent", 0, vector);
    t.prop("Float", bounds, "SphereRadius", 0, 0);
    let fragment = toy_struct(&mut t, component, "ToyFragment", None);
    for f in ["MaterialIndex", "FirstIndex", "NumPrimitives"] {
        t.prop("Int", fragment, f, 0, 0);
    }
    let lod = toy_struct(&mut t, component, "ToyLODModel", None);
    toy_array_of(&mut t, lod, "GestaltFragments", "Struct", fragment);
    let remap = toy_struct(&mut t, component, "ToySocketRemap", None);
    t.prop("Name", remap, "OriginalSocketName", 0, 0);
    t.prop("Name", remap, "MangledSocketName", 0, 0);
    let accessory = toy_struct(&mut t, component, "ToyAccessory", None);
    for f in ["TransformIndex", "OriginalBoneIndex", "GPUBoneIndex"] {
        t.prop("Int", accessory, f, 0, 0);
    }
    let container = toy_struct(&mut t, component, "GestaltDataContainer", None);
    toy_array_of(&mut t, container, "PartMeshNames", "Name", 0);
    // Declared as the mesh class, so a planned value holding another class's object is refused by the heap's rules.
    t.prop("Object", container, "SkeletalMesh", 0, mesh);
    toy_array_of(&mut t, container, "LODModelData", "Struct", lod);
    toy_array_of(&mut t, container, "SocketMappings", "Struct", remap);
    toy_array_of(&mut t, container, "GestaltAccessoryList", "Struct", accessory);
    t.prop("Object", container, "GestaltPartMatricesContainer", 0, object);
    if mis == Mis::BoundsInt {
        t.prop("Int", container, "ReferencePoseBounds", 0, 0);
    } else {
        t.prop("Struct", container, "ReferencePoseBounds", 0, bounds);
    }
    // The gestalt definition's three arrays (the note's section 6, check 2).
    let part_ty = toy_struct(&mut t, gestalt, "ToyGestaltPart", None);
    t.prop("Name", part_ty, "SkeletalMeshFragmentName", 0, 0);
    for f in ["MaterialIndex", "FirstIndex", "NumPrimitives"] {
        t.prop("Int", part_ty, f, 0, 0);
    }
    let info = toy_struct(&mut t, gestalt, "ToyGestaltInfo", None);
    toy_array_of(&mut t, info, "Parts", "Struct", part_ty);
    let socket = toy_struct(&mut t, gestalt, "ToyGestaltSocket", None);
    for f in ["SkeletalMeshFragmentName", "OriginalSocketName", "MangledSocketName"] {
        t.prop("Name", socket, f, 0, 0);
    }
    let part_bounds = toy_struct(&mut t, gestalt, "ToyGestaltPartBounds", None);
    t.prop("Name", part_bounds, "SkeletalMeshFragmentName", 0, 0);
    t.prop("Struct", part_bounds, "ReferencePoseBounds", 0, bounds);
    if mis == Mis::InfosInt {
        toy_array_of(&mut t, gestalt, "GestaltInfos", "Int", 0);
    } else {
        toy_array_of(&mut t, gestalt, "GestaltInfos", "Struct", info);
    }
    toy_array_of(&mut t, gestalt, "GestaltSocketMappings", "Struct", socket);
    toy_array_of(&mut t, gestalt, "GestaltPartBounds", "Struct", part_bounds);
    // The component's own mesh, which the copy must leave alone (lane FOLLOWUP-GESTALT-OPERAND-ORDER).
    t.prop("Object", component, "SkeletalMesh", 0, mesh);
    if gestalt_data {
        t.prop("Struct", component, "GestaltData", 0, container);
        // A class that is not a component but declares a same-named struct field (lane FOLLOWUP-GESTALT-OPERAND-ORDER).
        let lookalike = t.class("ToyLookalike", object);
        t.prop("Struct", lookalike, "GestaltData", 0, container);
    } else {
        // Without the base class's field: a component subclass that declares it, a valid receiver (review r1 of #634).
        let declaring = t.class("ToyDeclaringComponent", component);
        t.prop("Struct", declaring, "GestaltData", 0, container);
    }
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
    for (n, c) in UNLOADED.iter().zip([ty, part, component, mesh, gestalt]) {
        let x = t.p.add_export(c, n, d.clone());
        h.extend(t.p.object_tag(n, x));
    }
    h.extend_from_slice(&t.none);
    t.p.add_export(holder, "ToyHolder", h);
    // The serialized gestalt definitions: one whole, one whose first part holds a delegate tag the walker leaves raw,
    // and one of a class without the arrays.
    // With `InfosInt` the serialized definitions would not decode, so they are left out.
    let serialized: &[(&str, bool)] = if mis == Mis::InfosInt { &[] } else { &[("ToyGestaltFull", false), ("ToyGestaltRaw", true)] };
    for &(name, raw) in serialized {
        let payload = full_definition(&mut t, raw);
        t.p.add_export(gestalt, name, payload);
    }
    t.p.add_export(bare, "ToyGestaltBare", d.clone());
    t.p.build()
}

/// The invented contents of `ToyGestaltFull` (rule 9): two gestalt infos, four socket mappings, two part bounds.
pub(crate) const FULL_PARTS: [&[(&str, i32, i32, i32)]; 2] =
    [&[("Toy_A", 1, 0, 10), ("Toy_B", 2, 30, 5), ("Toy_C", 3, 45, 7)], &[("Toy_B", 4, 0, 3)]];
pub(crate) const FULL_SOCKETS: [(&str, &str, &str); 4] = [
    ("Toy_B", "SockB1", "MangB1"),
    ("Toy_A", "SockA1", "MangA1"),
    ("Toy_A", "SockA2", "MangA2"),
    ("Toy_Z", "SockZ", "MangZ"),
];
/// Name, origin, extent, radius.
pub(crate) const FULL_BOUNDS: [(&str, [f32; 3], [f32; 3], f32); 2] =
    [("Toy_A", [1.0, 2.0, 3.0], [1.0, 1.0, 1.0], 2.0), ("Toy_B", [4.0, 0.0, 0.0], [2.0, 1.0, 1.0], 3.0)];

/// A tagged struct element: `tags`, then `None`.
fn element(t: &mut ScriptToy, tags: Vec<Vec<u8>>) -> Vec<u8> {
    let mut out: Vec<u8> = tags.concat();
    out.extend_from_slice(&t.none);
    out
}

fn vector_body(t: &mut ScriptToy, v: [f32; 3]) -> Vec<u8> {
    let tags = vec![t.p.float_tag("X", v[0]), t.p.float_tag("Y", v[1]), t.p.float_tag("Z", v[2])];
    element(t, tags)
}

/// `ToyGestaltFull`'s payload; with `raw`, its first part also holds a `DelegateProperty` tag.
fn full_definition(t: &mut ScriptToy, raw: bool) -> Vec<u8> {
    let mut infos = Vec::new();
    for (i, parts) in FULL_PARTS.iter().enumerate() {
        let mut body = Vec::new();
        for (k, (n, m, f, c)) in parts.iter().enumerate() {
            let mut tags = vec![
                t.p.name_tag("SkeletalMeshFragmentName", n),
                t.p.int_tag("MaterialIndex", *m),
                t.p.int_tag("FirstIndex", *f),
                t.p.int_tag("NumPrimitives", *c),
            ];
            if raw && i == 0 && k == 0 {
                tags.push(t.p.tag("ToySpare", "DelegateProperty", &[0u8; 12]));
            }
            body.extend(element(t, tags));
        }
        let parts_tag = t.p.array_tag("Parts", i32::try_from(parts.len()).unwrap(), &body);
        infos.extend(element(t, vec![parts_tag]));
    }
    let mut sockets = Vec::new();
    for (n, o, m) in FULL_SOCKETS {
        let tags = vec![
            t.p.name_tag("SkeletalMeshFragmentName", n),
            t.p.name_tag("OriginalSocketName", o),
            t.p.name_tag("MangledSocketName", m),
        ];
        sockets.extend(element(t, tags));
    }
    let mut bounds = Vec::new();
    for (n, o, e, r) in FULL_BOUNDS {
        let origin = vector_body(t, o);
        let extent = vector_body(t, e);
        let body_tags = vec![
            t.p.struct_tag("Origin", "ToyVector", &origin),
            t.p.struct_tag("BoxExtent", "ToyVector", &extent),
            t.p.float_tag("SphereRadius", r),
        ];
        let body = element(t, body_tags);
        let tags = vec![t.p.name_tag("SkeletalMeshFragmentName", n), t.p.struct_tag("ReferencePoseBounds", "ToyBounds", &body)];
        bounds.extend(element(t, tags));
    }
    let mut out = 0i32.to_le_bytes().to_vec();
    out.extend(t.p.array_tag("GestaltInfos", 2, &infos));
    out.extend(t.p.array_tag("GestaltSocketMappings", 4, &sockets));
    out.extend(t.p.array_tag("GestaltPartBounds", 2, &bounds));
    out.extend_from_slice(&t.none);
    out
}

pub(crate) struct Fx {
    pub(crate) vm: Vm,
    dir: PathBuf,
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

pub(crate) fn fx() -> Fx {
    fx_with(true)
}

fn fx_with(gestalt_data: bool) -> Fx {
    fx_mis_with(gestalt_data, Mis::No)
}

/// [`fx`] on the variant `mis`.
pub(crate) fn fx_mis(mis: Mis) -> Fx {
    fx_mis_with(true, mis)
}

fn fx_mis_with(gestalt_data: bool, mis: Mis) -> Fx {
    let dir = std::env::temp_dir().join(format!("ow-inventory-gestalt-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let bytes = package_mis(gestalt_data, mis);
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
    pub(crate) fn o(&mut self, n: &str) -> ObjRef {
        let path = ObjectPath::parse(&format!("Engine.{n}")).unwrap();
        self.vm.defs.find(&mut self.vm.space, &path).unwrap().unwrap()
    }

    /// The holder's reference `n`: an unloaded handle.
    pub(crate) fn handle(&mut self, n: &str) -> ObjRef {
        let holder = self.o("ToyHolder");
        match crate::equip::read_prop(&self.vm, holder, n).unwrap() {
            Value::Object(Some(o)) => o,
            other => panic!("{other:?}"),
        }
    }

    pub(crate) fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        self.set_at(o, name, 0, v);
    }

    fn set_at(&mut self, o: ObjRef, name: &str, i: u32, v: Value) {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap();
        self.vm.space.write(o, PropAddr::element(p, i), v).unwrap();
    }

    pub(crate) fn name(&mut self, s: &str) -> NameId {
        self.vm.space.names.intern(s)
    }

    /// Definition data with `fields` set and the rest at their defaults.
    pub(crate) fn data(&mut self, fields: Vec<(&str, Value)>) -> Value {
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
    pub(crate) fn gestalt_type(&mut self) -> ObjRef {
        let (ty, g, shared) = (self.o("ToyType"), self.o("ToyGestalt"), self.o("ToyShared"));
        self.set(ty, "GestaltMesh", some(g));
        self.set(g, "GestaltSkeletalMesh", some(shared));
        ty
    }

    /// A heap fingerprint: every slot of every loaded object, to show a call wrote nothing.
    pub(crate) fn snapshot(&mut self) -> Vec<Vec<Value>> {
        LOADED.iter().map(|n| { let o = self.o(n); self.vm.space.heap.get(o).unwrap().slots.clone() }).collect()
    }
}

pub(crate) fn some(o: ObjRef) -> Value {
    Value::Object(Some(o))
}

pub(crate) fn not_yet(e: VmError) -> NotYet {
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

pub(crate) fn not_read(e: VmError) -> String {
    let n = not_yet(e);
    assert_eq!(n.missing, Missing::NotRead, "{n:?}");
    n.what.to_string()
}

pub(crate) fn script(e: VmError) -> String {
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
    assert_eq!(build_composite_mesh(&mut x.vm, None, &data).unwrap(), None);
}

/// The gestalt path's name list (section 2 step 3), in walk order: a gestalt-mode part's own name and its set
/// additional names; a non-gestalt part's mesh handle name (unloaded here, not loaded by the read); a part with
/// neither gives nothing. Then the fill writes them into the component's `GestaltData` and the shared mesh is the
/// answer (lane IMPL-GESTALT-DATA-LAYER; the definition here has empty arrays).
#[test]
fn the_gestalt_path_plans_the_names_then_fills_the_component() {
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
    assert_eq!(build_composite_mesh(&mut x.vm, Some(comp), &data).unwrap(), Some(shared));
    let v = crate::equip::read_prop(&x.vm, comp, "GestaltData").unwrap();
    let names = ow_attr::eval::field(&x.vm.space, &v, "PartMeshNames").unwrap().clone();
    assert_eq!(names, Value::array(vec![Value::Name(barrel), Value::Name(extra), Value::Name(un)]));
    assert_eq!(ow_attr::eval::field(&x.vm.space, &v, "SkeletalMesh").unwrap(), &some(shared));
    assert!(x.vm.defs.is_unloaded(&x.vm.space, un_mesh), "still no load");
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
    assert_eq!(build_composite_mesh(&mut x.vm, None, &zero).unwrap(), None);
    let one = x.data(vec![t.clone(), ("GripPartDefinition", some(c)), ("BarrelPartDefinition", some(a))]);
    assert_eq!(build_composite_mesh(&mut x.vm, None, &one).unwrap(), Some(ma));
    let two = x.data(vec![t, ("BodyPartDefinition", some(a)), ("BarrelPartDefinition", some(b))]);
    let (what, l) = layer(build_composite_mesh(&mut x.vm, None, &two).unwrap_err());
    assert_eq!((what.as_str(), l), (MERGE, MERGE_LAYER));
}

/// Residency (section 4, checks 2 to 4): an unloaded type or part is `NotRead`, and the walk's refusal comes before
/// the component's data error.
#[test]
fn an_unloaded_definition_refuses_not_read_before_the_component_is_checked() {
    let mut x = fx();
    let un_type = x.handle("UnType");
    let data = x.data(vec![("WeaponTypeDefinition", some(un_type))]);
    assert_eq!(not_read(build_composite_mesh(&mut x.vm, None, &data).unwrap_err()), NOT_LOADED);
    let ty = x.gestalt_type();
    let un_part = x.handle("UnPart");
    let data = x.data(vec![("WeaponTypeDefinition", some(ty)), ("StockPartDefinition", some(un_part))]);
    assert_eq!(not_read(build_composite_mesh(&mut x.vm, None, &data).unwrap_err()), NOT_LOADED);
    assert!(x.vm.defs.is_unloaded(&x.vm.space, un_part), "nothing was loaded");
}

/// The component on the gestalt path (section 4, check 5): None and a wrong class are data errors, an unloaded
/// handle is `NotRead`.
#[test]
fn the_gestalt_path_checks_the_component() {
    let mut x = fx();
    let ty = x.gestalt_type();
    let data = x.data(vec![("WeaponTypeDefinition", some(ty))]);
    assert!(script(build_composite_mesh(&mut x.vm, None, &data).unwrap_err()).contains("component is None"));
    let mesh = x.o("ToyMeshObj");
    assert!(script(build_composite_mesh(&mut x.vm, Some(mesh), &data).unwrap_err()).contains(COMPONENT_CLASS));
    let un = x.handle("UnComp");
    assert_eq!(not_read(build_composite_mesh(&mut x.vm, Some(un), &data).unwrap_err()), COMPONENT_NOT_LOADED);
}

/// A part whose class lacks a field the walk reads is a data error naming it (section 4, check 4).
#[test]
fn a_part_without_a_read_field_is_a_data_error() {
    let mut x = fx();
    let ty = x.gestalt_type();
    let not_a_part = x.o("ToyMeshObj");
    let data = x.data(vec![("WeaponTypeDefinition", some(ty)), ("BodyPartDefinition", some(not_a_part))]);
    assert!(script(build_composite_mesh(&mut x.vm, None, &data).unwrap_err()).contains("bIsGestaltMode"));
}

/// Check 4 of the copy with the layer (NATIVE_GESTALT_DATA_LAYER.md section 6, F1): both whole copies the value; a
/// malformed slot is a data error (F1(iii)); a stand-in is F1(ii)'s `NotRead`; a loaded definition with unfilled
/// slots is F1(i)'s `NOT_DECODED`. Nothing is written on a refusal.
#[test]
fn the_copy_needs_both_gestalt_slots_held_whole() {
    let mut x = fx();
    let (c1, c2) = (x.o("ToyComp1"), x.o("ToyComp2"));
    let names = x.vm.space.names.intern("Toy_Part");
    let class = x.vm.space.heap.get(c2).unwrap().class;
    let p = x.vm.space.prop(class, "GestaltData").unwrap();
    let mut whole = x.vm.space.read(c2, PropAddr::scalar(p)).unwrap().clone();
    let at = part_mesh_names(&x, &whole);
    whole.struct_mut().unwrap().fields[at] = Value::array(vec![Value::Name(names)]);
    x.vm.space.write(c2, PropAddr::scalar(p), whole.clone()).unwrap();
    init_gestalt_mesh_data(&mut x.vm, Some(c1), Some(c2)).unwrap();
    assert_eq!(x.vm.space.read(c1, PropAddr::scalar(p)).unwrap(), &whole, "both whole: the value is copied");
    // A malformed nested array element: F1(iii), a data error.
    let mut bad = whole;
    bad.struct_mut().unwrap().fields[at] = Value::array(vec![Value::Int(3)]);
    // Placed in the slot directly, past the heap's write checks (a fixture of a malformed decode).
    *x.vm.space.slot_mut(c2, PropAddr::scalar(p)).unwrap() = bad;
    // Through `refused_copy` (review r1 of #634): the counters, the slots and the name table unchanged.
    assert_eq!(script(refused_copy(&mut x, Some(c1), Some(c2))), crate::gestalt_data::INIT_MISMATCH);
    // A real stand-in (slotless, not a store definition, never loads): F1(ii)'s NotRead (the hub's ruling on #603),
    // as the receiver or as the argument.
    let class = x.vm.space.heap.get(c1).unwrap().class;
    let ghost = x.vm.space.alloc_stand_in(class, "ToyGhostComp", ow_object::heap::Origin::Resource { package: 0, index: 1 }).unwrap();
    assert!(x.vm.space.heap.get(ghost).unwrap().slots.is_empty() && !x.vm.defs.is_loaded(&x.vm.space, ghost));
    for (this, from) in [(c1, ghost), (ghost, c1)] {
        let e = quiet(&mut x.vm, |vm| init_gestalt_mesh_data(vm, Some(this), Some(from)));
        assert_eq!(not_read(e), crate::gestalt_data::INIT_STAND_IN);
    }
    // The self-copy answers before check 4, so even the stand-in succeeds with nothing written.
    init_gestalt_mesh_data(&mut x.vm, Some(ghost), Some(ghost)).unwrap();
    assert!(x.vm.space.heap.get(ghost).unwrap().slots.is_empty());
    // A definition, not a stand-in: a loaded store export whose slots were never filled is NOT_DECODED (the store
    // holds the data), as the receiver and as the argument. The slots are cleared directly (a fixture of that state).
    assert!(x.vm.defs.is_loaded(&x.vm.space, c2), "c2 is a store definition");
    x.vm.space.heap.get_mut(c2).unwrap().slots.clear();
    assert_eq!(not_read(init_gestalt_mesh_data(&mut x.vm, Some(c1), Some(c2)).unwrap_err()), NOT_DECODED);
    assert_eq!(not_read(init_gestalt_mesh_data(&mut x.vm, Some(c2), Some(c1)).unwrap_err()), NOT_DECODED);
}

/// The slot index of `PartMeshNames` in a `GestaltData` value.
pub(crate) fn part_mesh_names(x: &Fx, v: &Value) -> usize {
    let ty = v.as_struct().unwrap().ty;
    let id = x.vm.space.names.find("PartMeshNames").unwrap();
    x.vm.space.classes.struct_layout(ty).unwrap().field(id).unwrap().0 as usize
}

/// Runs one refused native call and checks #518's counters (heap writes, draws, load transitions) did not move.
pub(crate) fn quiet<T: std::fmt::Debug>(vm: &mut Vm, f: impl FnOnce(&mut Vm) -> Result<T, VmError>) -> VmError {
    let mark = crate::equip::card::load_mark(vm);
    let e = f(vm).unwrap_err();
    assert_eq!(crate::equip::card::load_mark(vm), mark, "#518's counters moved on a refusal: {e}");
    e
}

/// Review r1 of #596, item 5: an unloaded gestalt definition, and an unloaded part on the non-gestalt path, are
/// `NotRead`; a stand-in definition is the defined [`STAND_IN`] refusal, not a heap error (item 2).
#[test]
fn unloaded_and_stand_in_definitions_refuse_not_read() {
    let mut x = fx();
    let (ty, un_gestalt) = (x.o("ToyType"), x.handle("UnGestalt"));
    x.set(ty, "GestaltMesh", some(un_gestalt));
    let data = x.data(vec![("WeaponTypeDefinition", some(ty))]);
    assert_eq!(not_read(quiet(&mut x.vm, |vm| build_composite_mesh(vm, None, &data))), NOT_LOADED);
    x.set(ty, "GestaltMesh", Value::Object(None));
    let un_part = x.handle("UnPart");
    let data = x.data(vec![("WeaponTypeDefinition", some(ty)), ("GripPartDefinition", some(un_part))]);
    assert_eq!(not_read(quiet(&mut x.vm, |vm| build_composite_mesh(vm, None, &data))), NOT_LOADED, "the non-gestalt path");
    let part_a = x.o("ToyPartA");
    let part_class = x.vm.space.heap.get(part_a).unwrap().class;
    let ghost = x.vm.space.alloc_stand_in(part_class, "ToyGhostPart", ow_object::heap::Origin::Resource { package: 0, index: 1 }).unwrap();
    let data = x.data(vec![("WeaponTypeDefinition", some(ty)), ("GripPartDefinition", some(ghost))]);
    assert_eq!(not_read(quiet(&mut x.vm, |vm| build_composite_mesh(vm, None, &data))), STAND_IN);
    let type_class = x.vm.space.heap.get(ty).unwrap().class;
    let ghost_type = x.vm.space.alloc_stand_in(type_class, "ToyGhostType", ow_object::heap::Origin::Resource { package: 0, index: 1 }).unwrap();
    let data = x.data(vec![("WeaponTypeDefinition", some(ghost_type))]);
    assert_eq!(not_read(quiet(&mut x.vm, |vm| build_composite_mesh(vm, None, &data))), STAND_IN);
}

/// Review r1 of #596, item 5: a field holding the wrong kind of value is a data error naming it. The values are
/// placed in the slots directly, past the heap's write checks (a fixture of malformed data).
#[test]
fn a_field_of_the_wrong_kind_is_a_data_error_naming_it() {
    let put = |x: &mut Fx, o: ObjRef, name: &str, v: Value| {
        let class = x.vm.space.heap.get(o).unwrap().class;
        let p = x.vm.space.prop(class, name).unwrap();
        *x.vm.space.slot_mut(o, PropAddr::scalar(p)).unwrap() = v;
    };
    let mut x = fx();
    let ty = x.o("ToyType");
    put(&mut x, ty, "GestaltMesh", Value::Int(1));
    let data = x.data(vec![("WeaponTypeDefinition", some(ty))]);
    let e = quiet(&mut x.vm, |vm| build_composite_mesh(vm, None, &data));
    assert!(matches!(e, VmError::Script { .. }) && e.to_string().contains("GestaltMesh"), "{e}");
    for (field, v) in [("GestaltModeSkeletalMeshName", Value::Int(2)), ("NongestaltSkeletalMesh", Value::Int(3))] {
        let mut x = fx();
        let ty = x.gestalt_type();
        let a = x.o("ToyPartA");
        put(&mut x, a, field, v);
        let data = x.data(vec![("WeaponTypeDefinition", some(ty)), ("BodyPartDefinition", some(a))]);
        let e = quiet(&mut x.vm, |vm| build_composite_mesh(vm, None, &data));
        assert!(matches!(e, VmError::Script { .. }) && e.to_string().contains(field), "{field}: {e}");
    }
    // A bad field of D itself (item 3): a data error, not an attribute error.
    let mut x = fx();
    let ty = x.o("ToyType");
    let mut data = x.data(vec![("WeaponTypeDefinition", some(ty))]);
    data.struct_mut().unwrap().fields.truncate(1);
    let e = quiet(&mut x.vm, |vm| build_composite_mesh(vm, None, &data));
    assert!(matches!(e, VmError::Script { .. }) && e.to_string().contains("WeaponDefinitionData.WeaponTypeDefinition"), "{e}");
}

/// Review r1 of #596, item 5: a component of the right class whose class declares no `GestaltData` is a data error.
#[test]
fn a_component_class_without_gestalt_data_is_a_data_error() {
    let mut x = fx_with(false);
    let ty = x.gestalt_type();
    let data = x.data(vec![("WeaponTypeDefinition", some(ty))]);
    let c = x.o("ToyComp1");
    assert!(script(quiet(&mut x.vm, |vm| build_composite_mesh(vm, Some(c), &data))).contains("GestaltData"));
    let c2 = x.o("ToyComp2");
    // The receiver without the field (review r1 of #634: through `refused_copy`, the name table included).
    assert!(script(refused_copy(&mut x, Some(c), Some(c2))).contains("GestaltData"));
    // A valid receiver (a component subclass that declares the field) with a resident component that has none.
    let class = x.vm.find_class("Engine.ToyDeclaringComponent").unwrap();
    let declaring = ow_vm::interp::instantiate_from_defaults(&mut x.vm, class).unwrap();
    let e = script(refused_copy(&mut x, Some(declaring), Some(c2)));
    // The argument's class is named, so a receiver-side refusal cannot pass here (review r2 of #634, item 2).
    assert!(e.contains("Engine.SkeletalMeshComponent declares no GestaltData"), "the argument's field: {e}");
}

/// A runtime `ToyLookalike`: not a component, but its class declares a struct `GestaltData`.
fn lookalike(x: &mut Fx) -> ObjRef {
    let class = x.vm.find_class("Engine.ToyLookalike").unwrap();
    ow_vm::interp::instantiate_from_defaults(&mut x.vm, class).unwrap()
}

/// One refused copy: #518's counters ([`quiet`]), the loaded objects' slots and the name table's size all unchanged.
fn refused_copy(x: &mut Fx, this: Option<ObjRef>, from: Option<ObjRef>) -> VmError {
    let (before, names) = (x.snapshot(), x.vm.space.names.len());
    let e = quiet(&mut x.vm, |vm| init_gestalt_mesh_data(vm, this, from));
    assert_eq!(x.snapshot(), before, "nothing written: {e}");
    assert_eq!(x.vm.space.names.len(), names, "no name interned: {e}");
    e
}

/// Lane FOLLOWUP-GESTALT-OPERAND-ORDER: one order on each operand, the receiver first. None is a data error; an
/// unloaded handle is `NotRead` before its class is read (an unloaded mesh handle is `NotRead`, not a class error);
/// a resident object of another class is a data error, also when its class declares a same-named `GestaltData`.
#[test]
fn each_operand_refuses_in_one_order() {
    let mut x = fx();
    let (c1, mesh, un, un_mesh) = (x.o("ToyComp1"), x.o("ToyMeshObj"), x.handle("UnComp"), x.handle("UnMesh"));
    let look = lookalike(&mut x);
    // The receiver, with a good argument.
    assert_eq!(script(refused_copy(&mut x, None, Some(c1))), RECEIVER_NONE);
    assert_eq!(not_read(refused_copy(&mut x, Some(un), Some(c1))), RECEIVER_NOT_LOADED);
    assert_eq!(not_read(refused_copy(&mut x, Some(un_mesh), Some(c1))), RECEIVER_NOT_LOADED, "residency before class");
    for wrong in [mesh, look] {
        let e = script(refused_copy(&mut x, Some(wrong), Some(c1)));
        assert!(e.contains("the receiver is a") && e.contains(COMPONENT_CLASS), "{e}");
    }
    // The argument, with a good receiver.
    assert_eq!(script(refused_copy(&mut x, Some(c1), None)), "InitGestaltMeshDataFromOther: ComponentToCopyFrom is None");
    assert_eq!(not_read(refused_copy(&mut x, Some(c1), Some(un))), ARGUMENT_NOT_LOADED);
    assert_eq!(not_read(refused_copy(&mut x, Some(c1), Some(un_mesh))), ARGUMENT_NOT_LOADED, "residency before class");
    for wrong in [mesh, look] {
        let e = script(refused_copy(&mut x, Some(c1), Some(wrong)));
        assert!(e.contains("ComponentToCopyFrom is a") && e.contains(COMPONENT_CLASS), "{e}");
    }
    // The receiver is checked whole before the argument: a bad receiver wins over a bad argument.
    assert_eq!(script(refused_copy(&mut x, None, Some(un))), RECEIVER_NONE);
    assert_eq!(not_read(refused_copy(&mut x, Some(un), None)), RECEIVER_NOT_LOADED);
    for o in [un, un_mesh] {
        assert!(x.vm.defs.is_unloaded(&x.vm.space, o), "never loaded");
    }
}

/// The note's test 5: a self-copy succeeds with nothing written, nothing loaded or drawn and no name interned. Then
/// the layer's refusals (F1, the hub's ruling on #603: the copy's slot checks replaced the note's pre-layer
/// `LayerAbsent`) are exact, with nothing changed, and a copy between two whole components is one whole-slot write:
/// the receiver keeps its identity and its own `SkeletalMesh`, and its `GestaltData` is the argument's value, the
/// objects inside it the same objects.
#[test]
fn the_self_copy_the_layer_refusals_and_one_whole_slot_write() {
    let mut x = fx();
    let (c1, c2, mesh_a, mesh_b) = (x.o("ToyComp1"), x.o("ToyComp2"), x.o("ToyMeshA"), x.o("ToyMeshB"));
    let (mark, names, before) = (crate::equip::card::load_mark(&x.vm), x.vm.space.names.len(), x.snapshot());
    init_gestalt_mesh_data(&mut x.vm, Some(c1), Some(c1)).unwrap();
    assert_eq!((crate::equip::card::load_mark(&x.vm), x.vm.space.names.len(), x.snapshot()), (mark, names, before));
    // The layer's refusals, exactly: a stand-in (F1(ii)) and an undecoded store definition (F1(i)), on either operand.
    let class = x.vm.space.heap.get(c1).unwrap().class;
    let ghost = x.vm.space.alloc_stand_in(class, "ToyGhostComp", ow_object::heap::Origin::Resource { package: 0, index: 1 }).unwrap();
    for (this, from) in [(c1, ghost), (ghost, c1)] {
        let n = not_yet(refused_copy(&mut x, Some(this), Some(from)));
        assert_eq!((n.missing, n.what.as_ref(), n.record), (Missing::NotRead, crate::gestalt_data::INIT_STAND_IN, crate::gestalt_data::DATA_RECORD));
    }
    // The whole-slot write. Each component holds its own mesh; c1's GestaltData names mesh A.
    x.set(c1, "SkeletalMesh", some(mesh_a));
    x.set(c2, "SkeletalMesh", some(mesh_b));
    let p = x.vm.space.prop(class, "GestaltData").unwrap();
    let mut value = x.vm.space.read(c1, PropAddr::scalar(p)).unwrap().clone();
    let at = {
        let ty = value.as_struct().unwrap().ty;
        let id = x.vm.space.names.find("SkeletalMesh").unwrap();
        x.vm.space.classes.struct_layout(ty).unwrap().field(id).unwrap().0 as usize
    };
    value.struct_mut().unwrap().fields[at] = some(mesh_a);
    x.vm.space.write(c1, PropAddr::scalar(p), value.clone()).unwrap();
    let writes = x.vm.space.heap.committed_writes();
    let (draws, loads) = (x.vm.rng.draws(), x.vm.defs.load_transitions());
    init_gestalt_mesh_data(&mut x.vm, Some(c2), Some(c1)).unwrap();
    assert_eq!(x.vm.space.heap.committed_writes(), writes + 1, "one whole-slot write");
    assert_eq!((x.vm.rng.draws(), x.vm.defs.load_transitions()), (draws, loads), "no draw, no load");
    assert_eq!(x.vm.space.read(c2, PropAddr::scalar(p)).unwrap(), &value, "the argument's value, its objects the same");
    assert_eq!(x.o("ToyComp2"), c2, "the receiver keeps its identity");
    assert_eq!(crate::equip::read_prop(&x.vm, c2, "SkeletalMesh"), Some(some(mesh_b)), "its own mesh unchanged");
    assert_eq!(crate::equip::read_prop(&x.vm, c1, "SkeletalMesh"), Some(some(mesh_a)), "the argument unchanged");
    // The undecoded store definition, last (its slots are cleared directly, a fixture of that state).
    x.vm.space.heap.get_mut(c2).unwrap().slots.clear();
    for (this, from) in [(c1, c2), (c2, c1)] {
        let n = not_yet(refused_copy(&mut x, Some(this), Some(from)));
        assert_eq!((n.missing, n.what.as_ref(), n.record), (Missing::NotRead, NOT_DECODED, INIT_RECORD));
    }
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

/// Declares `InitGestaltMeshDataFromOther` on the fixture's component class, as the package's native.
fn declare_init(x: &mut Fx) -> ow_object::ids::FuncId {
    let component = x.vm.space.classes.by_path("Engine.SkeletalMeshComponent").unwrap();
    let p = PropertyDecl::new(x.vm.space.names.intern("ComponentToCopyFrom"), PropKind::Object { class: component })
        .with_flags(PropertyDecl::PARAM);
    let path = ObjectPath::parse("Engine.SkeletalMeshComponent.InitGestaltMeshDataFromOther").unwrap();
    x.vm.declare_function(FunctionDecl::native(path, INIT_GESTALT_MESH_DATA).with_owner(component).with_params(vec![p])).unwrap()
}

/// Review r1 of #634, item 1: through the registered native, the receiver is answered whole before argument 0 is read
/// or converted: an unloaded receiver with a raw int argument is `NotRead`, and a None receiver is `RECEIVER_NONE`.
#[test]
fn the_native_answers_the_receiver_before_converting_its_argument() {
    let mut x = fx();
    register(&mut x.vm).unwrap();
    let f = declare_init(&mut x);
    let un = x.handle("UnComp");
    let mark = crate::equip::card::load_mark(&x.vm);
    assert_eq!(not_read(x.vm.call(f, Some(un), vec![Value::Int(1)]).unwrap_err()), RECEIVER_NOT_LOADED);
    assert_eq!(script(x.vm.call(f, None, vec![Value::Int(1)]).unwrap_err()), RECEIVER_NONE);
    assert_eq!(crate::equip::card::load_mark(&x.vm), mark, "nothing written, drawn or loaded");
    assert!(x.vm.defs.is_unloaded(&x.vm.space, un));
    // With a valid receiver the argument's shape is then the error.
    let c = x.o("ToyComp1");
    assert!(matches!(x.vm.call(f, Some(c), vec![Value::Int(1)]).unwrap_err(), VmError::Script { .. }));
}

/// Review r1 of #634, item 2: `register` keeps an existing handler per key and binds the other key.
#[test]
fn register_keeps_an_existing_handler_per_key() {
    for pre in [INIT_GESTALT_MESH_DATA, BUILD_COMPOSITE_MESH] {
        let mut x = fx();
        x.vm.register_native(pre, Rc::new(|_c: &mut ow_vm::api::NativeCall<'_>| Ok(Value::Int(7)))).unwrap();
        register(&mut x.vm).unwrap();
        assert!(x.vm.has_native(BUILD_COMPOSITE_MESH) && x.vm.has_native(INIT_GESTALT_MESH_DATA), "{pre}");
        if pre == INIT_GESTALT_MESH_DATA {
            let f = declare_init(&mut x);
            let c = x.o("ToyComp1");
            assert_eq!(x.vm.call(f, Some(c), vec![some(c)]).unwrap().ret, Value::Int(7), "the earlier handler kept");
        } else {
            // The pre-bound Build handler is the one that answers (review r2 of #634, item 1).
            let object = x.vm.space.classes.by_path("Engine.Object").unwrap();
            let weapon = x.vm.space.classes.by_path("Engine.WillowWeapon").unwrap();
            let data_ty = x.vm.space.classes.struct_by_path("Engine.WillowWeapon.WeaponDefinitionData").unwrap();
            let param = |vm: &mut Vm, n: &str, k: PropKind| {
                PropertyDecl::new(vm.space.names.intern(n), k).with_flags(PropertyDecl::PARAM)
            };
            let params =
                vec![param(&mut x.vm, "Comp", PropKind::Object { class: object }), param(&mut x.vm, "Data", PropKind::Struct(data_ty))];
            let path = ObjectPath::parse("Engine.WillowWeapon.BuildCompositeMesh").unwrap();
            let decl = FunctionDecl::native(path, BUILD_COMPOSITE_MESH).with_owner(weapon).with_params(params);
            let f = x.vm.declare_function(decl).unwrap();
            let w = x.o("ToyWeapon");
            let data = x.data(vec![]);
            let got = x.vm.call(f, Some(w), vec![Value::Object(None), data]).unwrap().ret;
            assert_eq!(got, Value::Int(7), "the earlier Build handler kept");
        }
        register(&mut x.vm).unwrap();
    }
}
