//! `SetActiveProfileByName` / `ByIndex` (NATIVE_AIMOFFSET_SET_PROFILE.md section 4) over an invented `Engine.upk`
//! (AGENTS.md rule 9: invented objects and values, the class and field names the packages'). Every refusal leaves #518's
//! counters, `class_code_loads` and the name table unchanged.

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::notyet::Missing;
use ow_core::{ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_package::testkit::ints;
use ow_package::testkit::reflect::{toy_array_of, toy_struct};
use ow_package::testkit::script::{Asm, FunctionSpec, Param, ScriptToy};
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use super::*;

static NEXT: AtomicU32 = AtomicU32::new(0);
const NATIVE: u32 = 0x400 | 0x20000;
const SCRIPT: u32 = 0x1 | 0x2 | 0x20000;

/// The package: `AnimNode` (`SkelComponent`), `AnimNodeAimOffset` (the fields, the `AimOffsetProfile` struct and the
/// two natives), `WillowAnimNodeAimOffset` below it, a toy mesh component and mesh, and `ToyPawn` with `AimOffset` and
/// the script `SetAimOffsetProfile(N) { AimOffset.SetActiveProfileByName(N); }` (the shape of
/// `WillowPawn.SetAimOffsetProfile` with an `AimOffset` set). Exports:
/// - `Template_0`: a `WillowAnimNodeAimOffset` with profiles `Default`, `Soldier`, `Merc`, `Unarmed` (maya:8's first
///   four);
/// - `Skipped_0`: one whose `Profiles` tag says two elements and holds garbage, so the store skips it;
/// - `Copy_0`: one whose `TemplateNode` names `LazyTemplate_0`, which loading `Copy_0` leaves an unloaded handle.
fn package() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&[
        "Class",
        "Function",
        "ScriptStruct",
        "ObjectProperty",
        "NameProperty",
        "IntProperty",
        "ArrayProperty",
        "StructProperty",
        "ByteProperty",
    ]);
    let object = t.class("Object", 0);
    let node = t.class("AnimNode", object);
    let mesh_comp = t.class("ToyMeshComponent", object);
    let mesh = t.class("ToyMesh", object);
    t.prop("Object", mesh_comp, "SkeletalMesh", 0, mesh);
    t.prop("Object", node, "SkelComponent", 0, mesh_comp);
    let aim = t.class("AnimNodeAimOffset", node);
    let profile = toy_struct(&mut t, aim, "AimOffsetProfile", None);
    t.prop("Name", profile, "ProfileName", 0, 0);
    t.prop("Object", aim, "TemplateNode", 0, aim);
    toy_array_of(&mut t, aim, "Profiles", "Struct", profile);
    t.prop("Int", aim, "CurrentProfileIndex", 0, 0);
    toy_array_of(&mut t, aim, "RequiredBones", "Byte", 0);
    toy_array_of(&mut t, aim, "AimCpntIndexLUT", "Byte", 0);
    let by_name = t
        .function(aim, "SetActiveProfileByName", &[Param::new("Name", "ProfileName")], None, &[],
            &FunctionSpec::new(NATIVE, "SetActiveProfileByName"), |_| None)
        .function;
    t.function(aim, "SetActiveProfileByIndex", &[Param::new("Int", "ProfileIndex")], None, &[],
        &FunctionSpec::new(NATIVE, "SetActiveProfileByIndex"), |_| None);
    let willow = t.class("WillowAnimNodeAimOffset", aim);
    let pawn = t.class("ToyPawn", object);
    let aim_field = t.prop("Object", pawn, "AimOffset", 0, aim);
    t.function(pawn, "SetAimOffsetProfile", &[Param::new("Name", "N")], None, &[],
        &FunctionSpec::new(SCRIPT, "SetAimOffsetProfile"), |ids| {
            let n = ids.children.iter().find(|(c, _)| c == "N").unwrap().1;
            let mut a = Asm::new();
            a.stmt().context(|a| { a.instance(aim_field); }, |a| { a.final_function(by_name, |a| { a.local(n); }); });
            a.stmt().return_nothing();
            a.end();
            Some(a)
        });
    let none = t.none;
    let profiles = |t: &mut ScriptToy, names: &[&str]| {
        let mut body = Vec::new();
        for n in names {
            body.extend(t.p.name_tag("ProfileName", n));
            body.extend_from_slice(&none);
        }
        t.p.array_tag("Profiles", i32::try_from(names.len()).unwrap(), &body)
    };
    let mut p = ints(&[0]);
    p.extend(profiles(&mut t, &["Default", "Soldier", "Merc", "Unarmed"]));
    p.extend_from_slice(&none);
    t.p.add_export(willow, "Template_0", p);
    let mut p = ints(&[0]);
    p.extend(t.p.array_tag("Profiles", 2, &ints(&[5])));
    p.extend_from_slice(&none);
    t.p.add_export(willow, "Skipped_0", p);
    let mut p = ints(&[0]);
    p.extend_from_slice(&none);
    let lazy = t.p.add_export(willow, "LazyTemplate_0", p);
    let mut p = ints(&[0]);
    p.extend(t.p.object_tag("TemplateNode", lazy));
    p.extend_from_slice(&none);
    t.p.add_export(willow, "Copy_0", p);
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

fn fx_in(mode: ParityMode) -> Fx {
    let dir = std::env::temp_dir().join(format!("ow-world-aimoffset-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let bytes = package();
    let defs = DefStore::from_synthetic(&dir, &[("Engine.upk", &bytes)], IdentityPolicy::FirstInScanOrder, mode).unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), mode);
    for c in ["Engine.WillowAnimNodeAimOffset", "Engine.ToyPawn", "Engine.ToyMeshComponent", "Engine.ToyMesh"] {
        vm.find_class(c).unwrap();
    }
    if mode != ParityMode::Reference {
        register(&mut vm).unwrap();
    }
    Fx { vm, dir }
}

fn fx() -> Fx {
    fx_in(ParityMode::Off)
}

impl Fx {
    fn obj(&mut self, class: &str, name: &str) -> ObjRef {
        let c = self.vm.space.classes.by_path(&format!("Engine.{class}")).unwrap();
        self.vm.space.alloc(c, name, None, Origin::Runtime).unwrap()
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

    fn export(&mut self, name: &str) -> ObjRef {
        let path = ObjectPath::parse(&format!("Engine.{name}")).unwrap();
        self.vm.defs.find(&mut self.vm.space, &path).unwrap().unwrap()
    }

    /// A node with its own profiles `names`, `CurrentProfileIndex` 0 and both run-time arrays non-empty.
    fn node(&mut self, label: &str, names: &[&str]) -> ObjRef {
        let n = self.obj("WillowAnimNodeAimOffset", label);
        let class = self.vm.space.heap.get(n).unwrap().class;
        let p = self.vm.space.prop(class, "Profiles").unwrap();
        let Some(PropKind::Array(inner)) = self.vm.space.classes.decl(p).map(|d| d.kind.clone()) else { panic!() };
        let PropKind::Struct(ty) = *inner else { panic!() };
        let items = names.iter().map(|s| Value::structure(ty, vec![Value::Name(self.vm.space.names.intern(s))])).collect();
        self.set(n, "Profiles", Value::array(items));
        self.set(n, "RequiredBones", Value::array(vec![Value::Byte(1)]));
        self.set(n, "AimCpntIndexLUT", Value::array(vec![Value::Byte(2)]));
        n
    }

    fn marks(&self) -> (u64, u64, u64, u64, usize) {
        let vm = &self.vm;
        (vm.space.heap.committed_writes(), vm.rng.draws(), vm.defs.load_transitions(), vm.class_code_loads(), vm.space.names.len())
    }

    fn call(&mut self, node: ObjRef, key: &str, arg: Value) -> Result<(), VmError> {
        let f = self.vm.find_function(&format!("Engine.AnimNodeAimOffset.{key}"))?;
        self.vm.call(f, Some(node), vec![arg]).map(|_| ())
    }

    fn by_name(&mut self, node: ObjRef, name: &str) -> Result<(), VmError> {
        let n = Value::Name(self.vm.space.names.intern(name));
        self.call(node, "SetActiveProfileByName", n)
    }

    fn by_index(&mut self, node: ObjRef, i: i32) -> Result<(), VmError> {
        self.call(node, "SetActiveProfileByIndex", Value::Int(i))
    }

    fn current(&self, node: ObjRef) -> i32 {
        match self.get(node, "CurrentProfileIndex") {
            Value::Int(i) => i,
            other => panic!("{other:?}"),
        }
    }

    fn arrays_empty(&self, node: ObjRef) -> bool {
        ["RequiredBones", "AimCpntIndexLUT"].iter().all(|f| self.get(node, f).elements().is_some_and(<[Value]>::is_empty))
    }

    /// A call expected to change nothing.
    fn nothing(&mut self, f: impl FnOnce(&mut Fx) -> Result<(), VmError>) {
        let before = self.marks();
        f(self).unwrap();
        assert_eq!(self.marks(), before, "nothing written");
    }

    fn refusal(&mut self, f: impl FnOnce(&mut Fx) -> Result<(), VmError>) -> ow_core::NotYet {
        let before = self.marks();
        let e = f(self).unwrap_err();
        assert_eq!(self.marks(), before, "a refusal writes nothing");
        match e {
            VmError::NotYet(n) => n,
            other => panic!("expected NotYet, got {other:?}"),
        }
    }
}

/// The keys the package's declarations give are the bound keys.
#[test]
fn the_declarations_give_the_bound_keys() {
    let mut x = fx();
    for (name, key) in [("SetActiveProfileByName", BY_NAME), ("SetActiveProfileByIndex", BY_INDEX)] {
        let f = x.vm.find_function(&format!("Engine.AnimNodeAimOffset.{name}")).unwrap();
        assert_eq!(x.vm.function(f).unwrap().native_key.as_deref(), Some(key));
        assert!(x.vm.has_native(key));
    }
}

/// Test 1: own profiles `A`, `B`, `C`, no `TemplateNode`, no `SkelComponent`, the two run-time arrays seeded non-empty:
/// by name `C` gives 2 and empties them; by name `Z` does nothing.
#[test]
fn by_name_on_the_node_s_own_profiles() {
    let mut x = fx();
    let n = x.node("Node_0", &["A", "B", "C"]);
    x.by_name(n, "C").unwrap();
    assert_eq!(x.current(n), 2);
    assert!(x.arrays_empty(n));
    x.vm.space.names.intern("Z");
    x.nothing(|x| x.by_name(n, "Z"));
    assert_eq!(x.current(n), 2);
}

/// Test 2: with `TemplateNode` set, the template's list is used: `Y` gives 1; `A` (only in the node's own) nothing.
#[test]
fn by_name_uses_the_template_s_profiles() {
    let mut x = fx();
    let template = x.node("Template_1", &["X", "Y"]);
    let n = x.node("Node_1", &["A", "B", "C"]);
    x.set(n, "TemplateNode", Value::Object(Some(template)));
    x.vm.space.names.intern("A");
    x.nothing(|x| x.by_name(n, "A"));
    x.by_name(n, "Y").unwrap();
    assert_eq!(x.current(n), 1);
    assert_eq!(x.current(template), 0, "the template is not written");
}

/// Test 3: by index: the current index, -1 and the list's length do nothing; 1 gives 1.
#[test]
fn by_index_in_range_and_changed_only() {
    let mut x = fx();
    let n = x.node("Node_2", &["A", "B", "C"]);
    x.nothing(|x| x.by_index(n, 0));
    x.nothing(|x| x.by_index(n, -1));
    x.nothing(|x| x.by_index(n, 3));
    x.by_index(n, 1).unwrap();
    assert_eq!(x.current(n), 1);
}

/// Test 4: a change on a node whose `SkelComponent` holds a mesh is the rebuild's `NotRead`, before any write. A
/// component with no mesh is the routine's gate: the change goes through.
#[test]
fn a_change_with_a_mesh_is_the_rebuild_s_not_read() {
    let mut x = fx();
    let n = x.node("Node_3", &["A", "B"]);
    let comp = x.obj("ToyMeshComponent", "Comp_0");
    x.set(n, "SkelComponent", Value::Object(Some(comp)));
    x.by_name(n, "B").unwrap();
    assert_eq!(x.current(n), 1, "no mesh: the change goes through");
    let mesh = x.obj("ToyMesh", "Mesh_0");
    x.set(comp, "SkeletalMesh", Value::Object(Some(mesh)));
    x.vm.space.names.intern("A");
    let e = x.refusal(|x| x.by_name(n, "A"));
    assert_eq!((e.missing, &*e.what), (Missing::NotRead, REBUILD));
    assert_eq!(x.current(n), 1);
    // A change that is no change is still nothing, mesh or not.
    x.nothing(|x| x.by_index(n, 1));
    // Any mesh set, not resident included, is the rebuild's NotRead without being resolved.
    let class = x.vm.space.classes.by_path("Engine.ToyMesh").unwrap();
    let lazy_mesh = x.vm.space.alloc_stand_in(class, "LazyMesh", Origin::Resource { package: 0, index: 1 }).unwrap();
    x.set(comp, "SkeletalMesh", Value::Object(Some(lazy_mesh)));
    let e = x.refusal(|x| x.by_name(n, "A"));
    assert_eq!((e.missing, &*e.what), (Missing::NotRead, REBUILD));
}

/// Test 5, today's store behaviour (the interim rule): an empty `Profiles` on the list's owner is the profiles'
/// `LayerAbsent`, nothing written, never "no match". Both the export whose `Profiles` tag the store skipped (the class
/// default's empty array is left) and a node whose list is simply empty; as the receiver or as a copy's template.
#[test]
fn an_empty_profile_list_is_layer_absent() {
    let mut x = fx();
    let skipped = x.export("Skipped_0");
    assert!(x.vm.defs.take_log().iter().any(|l| l.contains("defaults: skipped") && l.contains("Profiles")));
    let empty = x.node("Empty_0", &[]);
    x.vm.space.names.intern("Unarmed");
    for owner in [skipped, empty] {
        let copy = x.node("Copy_N", &["Own"]);
        x.set(copy, "TemplateNode", Value::Object(Some(owner)));
        for n in [owner, copy] {
            let e = x.refusal(|x| x.by_name(n, "Unarmed"));
            assert!(matches!(e.missing, Missing::LayerAbsent(_)), "{e}");
            assert_eq!(&*e.what, PROFILES);
            let e = x.refusal(|x| x.by_index(n, 0));
            assert_eq!(&*e.what, PROFILES, "by index too");
        }
    }
}

/// Test 6: a lazy `TemplateNode`: the not-loaded `NotRead`, nothing loaded.
#[test]
fn a_lazy_template_is_not_read() {
    let mut x = fx();
    let copy = x.export("Copy_0");
    let Value::Object(Some(lazy)) = x.get(copy, "TemplateNode") else { panic!() };
    assert!(x.vm.defs.is_unloaded(&x.vm.space, lazy));
    x.vm.space.names.intern("Unarmed");
    let e = x.refusal(|x| x.by_name(copy, "Unarmed"));
    assert_eq!((e.missing, &*e.what), (Missing::NotRead, TEMPLATE_NOT_LOADED));
    assert!(x.vm.defs.is_unloaded(&x.vm.space, lazy), "nothing loaded");
}

/// Test 7: malformed declarations (`CurrentProfileIndex` not an int, `Profiles` not an array, `SkelComponent` not an
/// object reference, `RequiredBones` or `AimCpntIndexLUT` not a byte array) and a receiver that is no aim-offset node:
/// `VmError::Script`, nothing written. A lazy `SkelComponent` on a change: the not-loaded `NotRead`.
#[test]
fn malformed_declarations_are_script_errors() {
    let mut x = fx();
    let aim = x.vm.space.classes.by_path("Engine.AnimNodeAimOffset").unwrap();
    let object = x.vm.space.classes.by_path("Engine.Object").unwrap();
    let cases = [
        ("CurrentProfileIndex", PropKind::Float),
        ("Profiles", PropKind::Int),
        ("SkelComponent", PropKind::Int),
        ("RequiredBones", PropKind::Array(Box::new(PropKind::Int))),
        ("AimCpntIndexLUT", PropKind::Int),
    ];
    for (field, kind) in cases {
        let decl = PropertyDecl::new(x.vm.space.names.intern(field), kind);
        let class = x.vm.space.classes.define(&format!("Test.Odd{field}"), Some(aim), vec![decl]).unwrap();
        let n = x.vm.space.alloc(class, "Odd", None, Origin::Runtime).unwrap();
        let before = x.marks();
        match x.by_index(n, 1) {
            Err(VmError::Script { message, .. }) => assert!(message.contains(field), "{message}"),
            other => panic!("{other:?}"),
        }
        assert_eq!(x.marks(), before, "nothing written");
    }
    let class = x.vm.space.classes.define("Test.NotANode", Some(object), vec![]).unwrap();
    let stray = x.vm.space.alloc(class, "Stray", None, Origin::Runtime).unwrap();
    match x.by_index(stray, 1) {
        Err(VmError::Script { message, .. }) => assert!(message.contains("is not an Engine.AnimNodeAimOffset"), "{message}"),
        other => panic!("{other:?}"),
    }
    let n = x.node("Node_7", &["A", "B"]);
    let comp_class = x.vm.space.classes.by_path("Engine.ToyMeshComponent").unwrap();
    let lazy = x.vm.space.alloc_stand_in(comp_class, "LazyComp", Origin::Resource { package: 0, index: 1 }).unwrap();
    x.set(n, "SkelComponent", Value::Object(Some(lazy)));
    let e = x.refusal(|x| x.by_index(n, 1));
    assert_eq!((e.missing, &*e.what), (Missing::NotRead, COMPONENT_NOT_LOADED));
    x.nothing(|x| x.by_index(n, 0));
}

/// Test 8, maya:8's shape: a `WillowAnimNodeAimOffset` copy (its own `Profiles` empty, as the copy fix-up leaves it)
/// whose `TemplateNode` is a loaded template with `Default`, `Soldier`, `Merc`, `Unarmed`, `CurrentProfileIndex` 0,
/// called through the pawn's script `SetAimOffsetProfile('Unarmed')`: 3 with no mesh, the rebuild's `NotRead` with one.
#[test]
fn maya_s_unarmed_profile_through_the_pawn_s_script() {
    for with_mesh in [false, true] {
        let mut x = fx();
        let template = x.export("Template_0");
        let copy = x.node("AimOffset_Copy", &[]);
        x.set(copy, "TemplateNode", Value::Object(Some(template)));
        if with_mesh {
            let comp = x.obj("ToyMeshComponent", "Comp_1");
            let mesh = x.obj("ToyMesh", "Skel_Toy");
            x.set(comp, "SkeletalMesh", Value::Object(Some(mesh)));
            x.set(copy, "SkelComponent", Value::Object(Some(comp)));
        }
        let pawn = x.obj("ToyPawn", "Pawn_0");
        x.set(pawn, "AimOffset", Value::Object(Some(copy)));
        let f = x.vm.find_function("Engine.ToyPawn.SetAimOffsetProfile").unwrap();
        let unarmed = Value::Name(x.vm.space.names.intern("Unarmed"));
        let out = x.vm.call(f, Some(pawn), vec![unarmed]);
        if with_mesh {
            match out {
                Err(VmError::NotYet(n)) => assert_eq!((n.missing, &*n.what), (Missing::NotRead, REBUILD)),
                other => panic!("{other:?}"),
            }
            assert_eq!(x.current(copy), 0);
        } else {
            out.unwrap();
            assert_eq!(x.current(copy), 3);
        }
    }
}

/// Test 9: Reference binds nothing (the world pack in Reference).
#[test]
fn reference_binds_nothing() {
    let mut r = fx_in(ParityMode::Reference);
    ow_vm::api::NativePack::register(&crate::world::Pack, &mut r.vm).unwrap();
    assert!(!r.vm.has_native(BY_NAME) && !r.vm.has_native(BY_INDEX));
}
