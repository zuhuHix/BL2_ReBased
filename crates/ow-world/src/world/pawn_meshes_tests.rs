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

/// The invented classes. `with_helper` defines `Engine.InstanceDataHelper`; `mesh_kind` is `Pawn.Mesh`'s kind (the
/// broken-declaration case passes a non-object).
fn env(with_helper: bool, mesh_kind: Option<PropKind>) -> Env {
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
    let attachments = decl(&mut vm, "Attachments", PropKind::Array(Box::new(PropKind::Int)));
    let max = decl(&mut vm, "MaxExpectedComponents", PropKind::Int);
    let composition =
        vm.space.classes.define_struct("WillowGame.BodyClassDefinition.BodyComposition", None, vec![attachments, max]).unwrap();
    let body = class(&mut vm, "WillowGame.BodyClassDefinition", object, &[
        ("BodyComposition", PropKind::Struct(composition)),
        ("UseRootMotion", PropKind::Bool),
        ("UseRootRotation", PropKind::Bool),
    ]);
    let base = class(&mut vm, "Engine.Pawn", object, &[("Mesh", mesh_kind.unwrap_or(o.clone()))]);
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

/// Steps 3, 4 and 6: the three empty cases answer nothing and write nothing, under both keys.
#[test]
fn the_three_empty_cases_do_nothing() {
    // No helper class.
    let mut e = env(false, None);
    let (a, b) = functions(&mut e);
    let p = make(&mut e.vm, e.player, "Maya_1");
    let bc = body(&mut e, 3);
    set(&mut e.vm, p, "BodyClass", Value::Object(Some(bc)));
    for f in [a, b] {
        assert_eq!(e.vm.call(f, Some(p), vec![]).unwrap().ret, Value::None, "no helper");
    }
    // BodyClass None, then an empty attachment list.
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

/// Step 7: a non-empty list is the note's `LayerAbsent`, unprefixed, and the heap is unchanged.
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

/// Step 7's field preflight: a mistyped `Mesh` is the exact `VmError::Script` text, before any write.
#[test]
fn a_broken_field_declaration_is_its_exact_error() {
    let mut e = env(true, Some(PropKind::Int));
    let (a, _) = functions(&mut e);
    let p = make(&mut e.vm, e.pawn, "Pawn_4");
    let bc = body(&mut e, 2);
    set(&mut e.vm, p, "BodyClass", Value::Object(Some(bc)));
    let err = e.vm.call(a, Some(p), vec![]).unwrap_err();
    let VmError::Script { message, .. } = &err else { panic!("{err:?}") };
    assert_eq!(message, &wrong_field(WILLOW_PAWN, "Mesh", "an object reference"));
}

/// Step 1: a receiver that is not a WillowPawn is a script error.
#[test]
fn a_receiver_that_is_not_a_willow_pawn_is_refused() {
    let mut e = env(true, None);
    let (a, _) = functions(&mut e);
    let o = make(&mut e.vm, e.object, "NotAPawn");
    let err = e.vm.call(a, Some(o), vec![]).unwrap_err();
    assert!(matches!(err, VmError::Script { .. }), "{err:?}");
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
