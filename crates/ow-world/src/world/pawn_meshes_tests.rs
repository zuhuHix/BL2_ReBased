//! `InitializePawnMeshes` on invented classes (AGENTS.md rule 9: invented classes and values; the class and field
//! names are the packages'). Each test names the module documentation's step it checks. Graded by the `VmError`
//! variant and the `NotYet` kind; the declaration errors by their exact text.

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
    player: ClassId,
    body: ClassId,
    composition: StructId,
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

/// The invented classes. `with_helper` defines `Engine.InstanceDataHelper`; `attachments_kind` is
/// `BodyComposition.Attachments`' kind (the broken-declaration case passes a non-array).
fn env(with_helper: bool, attachments_kind: Option<PropKind>) -> Env {
    let dir = std::env::temp_dir().join(format!("ow-world-pawnmesh-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
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
    if with_helper {
        class(&mut vm, HELPER, object, &[]);
    }
    let data = decl(&mut vm, "Data", PropKind::Array(Box::new(PropKind::Int)));
    let set = vm.space.classes.define_struct("Engine.IInstanceData.InstanceDataSet", None, vec![data]).unwrap();
    let bits = decl(&mut vm, "SwitchStateBitField", PropKind::Int);
    let rep = vm.space.classes.define_struct("Engine.IInstanceData.ReplicatedInstanceDataState", None, vec![bits]).unwrap();
    let attachments = decl(&mut vm, "Attachments", attachments_kind.unwrap_or(PropKind::Array(Box::new(PropKind::Int))));
    let max = decl(&mut vm, "MaxExpectedComponents", PropKind::Int);
    let composition =
        vm.space.classes.define_struct("WillowGame.BodyClassDefinition.BodyComposition", None, vec![attachments, max]).unwrap();
    let body = class(&mut vm, "WillowGame.BodyClassDefinition", object, &[
        ("BodyComposition", PropKind::Struct(composition)),
        ("UseRootMotion", PropKind::Bool),
        ("UseRootRotation", PropKind::Bool),
    ]);
    let base = class(&mut vm, "Engine.Pawn", object, &[("Mesh", o.clone())]);
    let pawn = class(&mut vm, WILLOW_PAWN, base, &[
        ("BodyClass", PropKind::Object { class: body }),
        ("InstanceDataState", PropKind::Struct(set)),
        ("ReplicatedInstanceDataState", PropKind::Struct(rep)),
    ]);
    let player = class(&mut vm, "WillowGame.WillowPlayerPawn", pawn, &[]);
    Env { vm, dir, object, pawn, player, body, composition }
}

fn native(vm: &mut Vm, path: &str, owner: ClassId) -> FuncId {
    let key = format!("{}()", path.split_once('.').map_or(path, |(_, rest)| rest));
    let d = FunctionDecl::native(ObjectPath::parse(path).unwrap(), key).with_owner(owner);
    vm.declare_function(d).unwrap()
}

fn make(vm: &mut Vm, c: ClassId, name: &str) -> ObjRef {
    vm.space.alloc(c, name, None, Origin::Runtime).unwrap()
}

fn set(vm: &mut Vm, o: ObjRef, name: &str, v: Value) {
    let class = vm.space.heap.get(o).unwrap().class;
    let p = vm.space.prop(class, name).unwrap();
    vm.space.write(o, PropAddr::scalar(p), v).unwrap();
}

/// A body class whose composition holds `n` invented attachments.
fn body(e: &mut Env, n: i32) -> ObjRef {
    let b = make(&mut e.vm, e.body, "BodyClass_Test");
    let list = Value::array((0..n).map(Value::Int).collect());
    set(&mut e.vm, b, "BodyComposition", Value::structure(e.composition, vec![list, Value::Int(0)]));
    b
}

/// The two functions, bound by `register`.
fn functions(e: &mut Env) -> (FuncId, FuncId) {
    let (pawn, player) = (e.pawn, e.player);
    let a = native(&mut e.vm, "WillowGame.WillowPawn.InitializePawnMeshes", pawn);
    let b = native(&mut e.vm, "WillowGame.WillowPlayerPawn.InitializePawnMeshes", player);
    register(&mut e.vm).unwrap();
    (a, b)
}

/// Step 2: a class table without the helper class is the named `NotRead` (a port state), under both keys.
#[test]
fn no_helper_class_is_not_read() {
    let mut e = env(false, None);
    let (a, b) = functions(&mut e);
    let p = make(&mut e.vm, e.player, "Maya_1");
    let bc = body(&mut e, 3);
    set(&mut e.vm, p, "BodyClass", Value::Object(Some(bc)));
    for f in [a, b] {
        let err = e.vm.call(f, Some(p), vec![]).unwrap_err();
        let VmError::NotYet(n) = &err else { panic!("{err:?}") };
        assert!(matches!(n.missing, Missing::NotRead), "{n:?}");
        assert_eq!(n.what, NO_HELPER);
    }
}

/// Steps 3 and 4: BodyClass None and an empty attachment list answer nothing and write nothing, under both keys.
#[test]
fn the_empty_cases_do_nothing() {
    let mut e = env(true, None);
    let (a, b) = functions(&mut e);
    let p = make(&mut e.vm, e.player, "Maya_2");
    for f in [a, b] {
        assert_eq!(e.vm.call(f, Some(p), vec![]).unwrap().ret, Value::None, "BodyClass None");
    }
    let empty = body(&mut e, 0);
    set(&mut e.vm, p, "BodyClass", Value::Object(Some(empty)));
    for f in [a, b] {
        assert_eq!(e.vm.call(f, Some(p), vec![]).unwrap().ret, Value::None, "an empty list");
    }
}

/// Step 5: a non-empty list is the note's `LayerAbsent`, unprefixed, and the heap is unchanged.
#[test]
fn a_non_empty_list_is_layer_absent_and_writes_nothing() {
    let mut e = env(true, None);
    let (a, b) = functions(&mut e);
    let p = make(&mut e.vm, e.player, "Maya_3");
    let bc = body(&mut e, 18);
    set(&mut e.vm, p, "BodyClass", Value::Object(Some(bc)));
    for f in [a, b] {
        let before = (e.vm.space.heap.len(), e.vm.space.heap.get(p).unwrap().slots.clone(), e.vm.space.heap.get(bc).unwrap().slots.clone());
        let err = e.vm.call(f, Some(p), vec![]).unwrap_err();
        let VmError::NotYet(n) = &err else { panic!("{err:?}") };
        assert!(matches!(n.missing, Missing::LayerAbsent(l) if l == LAYER), "{n:?}");
        assert_eq!(n.what, BUILD);
        assert_eq!(n.record, RECORD);
        assert!(!ow_core::notyet::is_partial_writes(&n.what), "no prefix of its own");
        let after = (e.vm.space.heap.len(), e.vm.space.heap.get(p).unwrap().slots.clone(), e.vm.space.heap.get(bc).unwrap().slots.clone());
        assert_eq!(before, after, "nothing written");
    }
}

/// Step 4: an `Attachments` that is not an array is the exact `VmError::Script` text, before any write.
#[test]
fn a_broken_field_declaration_is_its_exact_error() {
    let mut e = env(true, Some(PropKind::Int));
    let (a, _) = functions(&mut e);
    let p = make(&mut e.vm, e.pawn, "Pawn_4");
    let bc = make(&mut e.vm, e.body, "BodyClass_Broken");
    set(&mut e.vm, bc, "BodyComposition", Value::structure(e.composition, vec![Value::Int(2), Value::Int(0)]));
    set(&mut e.vm, p, "BodyClass", Value::Object(Some(bc)));
    let err = e.vm.call(a, Some(p), vec![]).unwrap_err();
    let VmError::Script { function, message } = &err else { panic!("{err:?}") };
    assert_eq!(function, UNIT);
    assert_eq!(message, &no_attachments("WillowGame.BodyClassDefinition"));
}

/// Step 1: a receiver that is not a WillowPawn is a script error.
#[test]
fn a_receiver_that_is_not_a_willow_pawn_is_refused() {
    let mut e = env(true, None);
    let (a, _) = functions(&mut e);
    let o = make(&mut e.vm, e.object, "NotAPawn");
    let err = e.vm.call(a, Some(o), vec![]).unwrap_err();
    let VmError::Script { message, .. } = &err else { panic!("{err:?}") };
    assert_eq!(message, &not_a_pawn("Core.Object"));
}

/// First-wins: a key bound before keeps its binding.
#[test]
fn an_earlier_binding_keeps_its_key() {
    let mut e = env(true, None);
    let pawn = e.pawn;
    let f = native(&mut e.vm, "WillowGame.WillowPawn.InitializePawnMeshes", pawn);
    e.vm.register_native(KEYS[0], Rc::new(|_| Ok(Value::Bool(true)))).unwrap();
    register(&mut e.vm).unwrap();
    let p = make(&mut e.vm, e.pawn, "Pawn_5");
    assert_eq!(e.vm.call(f, Some(p), vec![]).unwrap().ret, Value::Bool(true));
}

/// The lazy cases' packages (invented).
/// - A toy WillowGame whose `BodyClassDefinition` declares an invented `NextBody`.
/// - The bodies: two body classes, `Holder` and `Target`. `Holder`'s `NextBody` names `Target`, so reading it from the
///   loaded holder gives `Target` as the store's unloaded handle (the slice's case: Maya's body class at link 1's
///   stop). With `nested`, both sit under a `Package` export `GD_ToyBody` of a file `Startup.upk` (a package cooked into
///   another file); otherwise they are the file `GD_ToyBody.upk`'s own exports.
/// - With `helper_package`, a toy `Engine.upk` declares `InstanceDataHelper`, a class the packages hold but the class
///   table has not loaded.
fn lazy_vm(nested: bool, helper_package: bool) -> (Vm, PathBuf, ObjRef, ClassId, ClassId) {
    let dir = std::env::temp_dir().join(format!("ow-world-pawnmesh-lazy-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let core = {
        let mut t = ScriptToy::new(&[]);
        t.class("Object", 0);
        t.build()
    };
    let willow = {
        let mut t = ScriptToy::with_imports(&["Class", "ObjectProperty"]);
        let core = t.p.add_import("Core", "Package", 0, "Core");
        let object = t.p.add_import("Core", "Class", core, "Object");
        let definition = t.class("BodyClassDefinition", object);
        t.prop("Object", definition, "NextBody", 0, object);
        t.build()
    };
    let engine = {
        let mut t = ScriptToy::with_imports(&["Class"]);
        let core = t.p.add_import("Core", "Package", 0, "Core");
        let object = t.p.add_import("Core", "Class", core, "Object");
        t.class("InstanceDataHelper", object);
        t.build()
    };
    let bodies = {
        let mut t = ScriptToy::with_imports(&["Class", "Package"]);
        let willow = t.p.add_import("Core", "Package", 0, "WillowGame");
        let definition = t.p.add_import("Core", "Class", willow, "BodyClassDefinition");
        let outer = if nested {
            let mut payload = ow_package::testkit::ints(&[0]);
            payload.extend_from_slice(&t.none);
            t.p.add_export_with(ow_package::testkit::Export::new(t.import("Package"), "GD_ToyBody", payload))
        } else {
            0
        };
        let export = |t: &mut ScriptToy, name: &str, payload: Vec<u8>| {
            let mut e = ow_package::testkit::Export::new(definition, name, payload);
            e.outer = outer;
            t.p.add_export_with(e)
        };
        let mut target = ow_package::testkit::ints(&[0]);
        target.extend_from_slice(&t.none);
        let target = export(&mut t, "Target", target);
        let mut holder = ow_package::testkit::ints(&[0]);
        holder.extend(t.p.tag("NextBody", "ObjectProperty", &ow_package::testkit::ints(&[target])));
        holder.extend_from_slice(&t.none);
        export(&mut t, "Holder", holder);
        t.build()
    };
    let bodies_file = if nested { "Startup.upk" } else { "GD_ToyBody.upk" };
    let mut files: Vec<(&str, &[u8])> =
        vec![("Core.upk", core.as_slice()), ("WillowGame.upk", willow.as_slice()), (bodies_file, bodies.as_slice())];
    if helper_package {
        files.push(("Engine.upk", engine.as_slice()));
    }
    let defs = DefStore::from_synthetic(&dir, &files, IdentityPolicy::FirstInScanOrder, ParityMode::Off).expect("store");
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
    let object = vm.find_class("Core.Object").unwrap();
    let body = vm.find_class("WillowGame.BodyClassDefinition").unwrap();
    if !helper_package {
        class(&mut vm, HELPER, object, &[]);
    }
    let pawn = class(&mut vm, WILLOW_PAWN, object, &[("BodyClass", PropKind::Object { class: body })]);
    let player = class(&mut vm, "WillowGame.WillowPlayerPawn", pawn, &[]);
    let path = ObjectPath::parse("GD_ToyBody.Holder").unwrap();
    let holder = vm.defs.find(&mut vm.space, &path).unwrap().unwrap();
    let class_id = vm.space.heap.get(holder).unwrap().class;
    let p = vm.space.prop(class_id, "NextBody").unwrap();
    let Value::Object(Some(target)) = vm.space.read(holder, PropAddr::scalar(p)).unwrap().clone() else {
        panic!("the holder names its target")
    };
    assert!(vm.defs.is_unloaded(&vm.space, target), "a genuinely unloaded handle");
    (vm, dir, target, pawn, player)
}

/// Step 3, the slice's stop: a `BodyClass` the store has not loaded is the named `NotRead`, with the record and the
/// handle's full path as the store names it, no prefix, and the handle still unloaded. Both a plain file and a package
/// cooked into another file (review r2 of #522: no `Startup.` prefix).
#[test]
fn an_unloaded_body_class_is_not_read_and_stays_unloaded() {
    for nested in [false, true] {
        let (mut vm, dir, target, pawn, player) = lazy_vm(nested, false);
        let a = native(&mut vm, "WillowGame.WillowPawn.InitializePawnMeshes", pawn);
        let b = native(&mut vm, "WillowGame.WillowPlayerPawn.InitializePawnMeshes", player);
        register(&mut vm).unwrap();
        let maya = make(&mut vm, player, "Maya_6");
        set(&mut vm, maya, "BodyClass", Value::Object(Some(target)));
        for f in [a, b] {
            let err = vm.call(f, Some(maya), vec![]).unwrap_err();
            let VmError::NotYet(n) = &err else { panic!("{err:?}") };
            assert!(matches!(n.missing, Missing::NotRead), "{n:?}");
            assert_eq!(n.what, lazy_body_class("GD_ToyBody.Target"), "nested {nested}");
            assert_eq!(n.record, RECORD);
            assert!(!ow_core::notyet::is_partial_writes(&n.what));
            assert!(vm.defs.is_unloaded(&vm.space, target), "nothing was loaded");
        }
        drop(vm);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// Step 2 with the helper held by a package but not in the class table (review r2 of #522): still the named `NotRead`,
/// the class is not loaded by the lookup, and the lazy body class stays unloaded.
#[test]
fn a_helper_only_in_a_package_is_not_loaded_by_the_lookup() {
    let (mut vm, dir, target, pawn, player) = lazy_vm(false, true);
    let a = native(&mut vm, "WillowGame.WillowPawn.InitializePawnMeshes", pawn);
    register(&mut vm).unwrap();
    assert!(vm.space.classes.by_path(HELPER).is_none(), "the helper is not in the class table");
    let maya = make(&mut vm, player, "Maya_7");
    set(&mut vm, maya, "BodyClass", Value::Object(Some(target)));
    let err = vm.call(a, Some(maya), vec![]).unwrap_err();
    let VmError::NotYet(n) = &err else { panic!("{err:?}") };
    assert!(matches!(n.missing, Missing::NotRead), "{n:?}");
    assert_eq!(n.what, NO_HELPER);
    assert!(vm.space.classes.by_path(HELPER).is_none(), "the lookup loaded nothing");
    assert!(vm.defs.is_unloaded(&vm.space, target), "the body class was not loaded either");
    drop(vm);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Step 2 before step 3 (review r2 of #522): a missing helper is the named `NotRead` on a pawn with no BodyClass too.
#[test]
fn a_missing_helper_wins_over_a_none_body_class() {
    let mut e = env(false, None);
    let (a, b) = functions(&mut e);
    let p = make(&mut e.vm, e.player, "Maya_8");
    for f in [a, b] {
        let err = e.vm.call(f, Some(p), vec![]).unwrap_err();
        let VmError::NotYet(n) = &err else { panic!("{err:?}") };
        assert_eq!(n.what, NO_HELPER);
    }
}

/// Step 5 needs only the attachment list (review r2 of #522): with the fields the note does not name left out of the
/// classes (no `Mesh`, `InstanceDataState`, `ReplicatedInstanceDataState`, `UseRootMotion` or `UseRootRotation`), a
/// non-empty list is still the `LayerAbsent`.
#[test]
fn the_layer_absent_does_not_need_the_unrelated_fields() {
    let dir = std::env::temp_dir().join(format!("ow-world-pawnmesh-bare-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
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
    class(&mut vm, HELPER, object, &[]);
    let attachments = decl(&mut vm, "Attachments", PropKind::Array(Box::new(PropKind::Int)));
    let composition = vm.space.classes.define_struct("WillowGame.BodyClassDefinition.BodyComposition", None, vec![attachments]).unwrap();
    let body = class(&mut vm, "WillowGame.BodyClassDefinition", object, &[("BodyComposition", PropKind::Struct(composition))]);
    let pawn = class(&mut vm, WILLOW_PAWN, object, &[("BodyClass", PropKind::Object { class: body })]);
    let f = native(&mut vm, "WillowGame.WillowPawn.InitializePawnMeshes", pawn);
    register(&mut vm).unwrap();
    let bc = make(&mut vm, body, "BodyClass_Bare");
    set(&mut vm, bc, "BodyComposition", Value::structure(composition, vec![Value::array(vec![Value::Int(1)])]));
    let p = make(&mut vm, pawn, "Pawn_9");
    set(&mut vm, p, "BodyClass", Value::Object(Some(bc)));
    let err = vm.call(f, Some(p), vec![]).unwrap_err();
    let VmError::NotYet(n) = &err else { panic!("{err:?}") };
    assert!(matches!(n.missing, Missing::LayerAbsent(_)), "{n:?}");
    assert_eq!(n.what, BUILD);
    drop(vm);
    let _ = std::fs::remove_dir_all(&dir);
}
