//! `InitPlayerSkillTree` on an invented `WillowGame.upk` (AGENTS.md rule 9; decision 2026-11-19-88). Expected answers
//! are written by hand from NATIVE_INIT_PLAYER_SKILL_TREE.md section 5. AI-assisted (Claude Code).

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_attr::api::{AttrError, EngineQuery, WorldParts};
use ow_core::Missing;
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_package::testkit::script::{FunctionSpec, ScriptToy};
use ow_vm::api::WorldHooks;

use super::*;

const NATIVE: u32 = 0x400 | 0x20000;
static NEXT: AtomicU32 = AtomicU32::new(0);

/// `WillowGame.upk`: the controller (with `PlayerSkillTree`, `PlayerClass` and the native), the tree class, the player
/// class definition (with `SkillTreePath`, unless `no_path`), the globals and the pawn data manager. `odd_tree`
/// declares `PlayerSkillTree` as a string instead (P1's data error); `tree_of_manager` declares it as an object of the
/// manager's class (a field the tree class cannot be assigned to). `ToyHolder`'s `Target` names `ToyClass`, a player
/// class definition that stays an unloaded store handle when the holder is loaded.
fn package(odd_tree: bool, no_path: bool, tree_of_manager: bool) -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class", "Function", "ObjectProperty", "StrProperty"]);
    let object = t.class("Object", 0);
    let tree = t.class("PlayerSkillTree", object);
    let class_def = t.class("PlayerClassDefinition", object);
    if !no_path {
        t.prop("Str", class_def, "SkillTreePath", 0, 0);
    }
    let controller = t.class("WillowPlayerController", object);
    let manager = t.class("WillowPlayerPawnDataManager", object);
    if odd_tree {
        t.prop("Str", controller, "PlayerSkillTree", 0, 0);
    } else if tree_of_manager {
        t.prop("Object", controller, "PlayerSkillTree", 0, manager);
    } else {
        t.prop("Object", controller, "PlayerSkillTree", 0, tree);
    }
    t.prop("Object", controller, "PlayerClass", 0, class_def);
    let spec = FunctionSpec::new(NATIVE, "InitPlayerSkillTree");
    t.function(controller, "InitPlayerSkillTree", &[], None, &[], &spec, |_| None);
    // COLD-CLASS-AUDIT: subclasses no fixture call warms.
    t.class("ToyColdController", controller);
    t.class("ToyColdClassDefinition", class_def);
    let globals = t.class("WillowGlobals", object);
    t.prop("Object", globals, "ThePlayerPawnDataManager", 0, manager);
    let holder = t.class("Holder", object);
    t.prop("Object", holder, "Target", 0, class_def);
    let mut d = 0i32.to_le_bytes().to_vec();
    d.extend_from_slice(&t.none);
    let toy_class = t.p.add_export(class_def, "ToyClass", d);
    let mut h = 0i32.to_le_bytes().to_vec();
    h.extend(t.p.object_tag("Target", toy_class));
    h.extend_from_slice(&t.none);
    t.p.add_export(holder, "ToyHolder", h);
    t.p.build()
}

/// Toy world hooks: only the globals object (or none, for P4's world `NotYet`).
struct GlobalsOnly(Option<ObjRef>);
impl WorldHooks for GlobalsOnly {
    fn globals(&mut self, _: WorldParts<'_>) -> Result<ObjRef, AttrError> {
        self.0.ok_or_else(|| {
            AttrError::NotYet(ow_core::NotYet::layer_absent("the toy world's globals", "a globals object", "toy"))
        })
    }
    fn game_info(&mut self, _: WorldParts<'_>) -> Result<ObjRef, AttrError> {
        Err(AttrError::BadData { reason: "no game info".into() })
    }
    fn engine_query(&mut self, _: EngineQuery, _: ObjRef, _: WorldParts<'_>) -> Result<Option<ObjRef>, AttrError> {
        Err(AttrError::BadData { reason: "no engine query".into() })
    }
}

struct Fx {
    vm: Vm,
    dir: PathBuf,
    controller: ObjRef,
    globals: ObjRef,
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn fx_with(mode: ParityMode, odd_tree: bool, with_globals: bool) -> Fx {
    fx_full(mode, odd_tree, with_globals, false)
}

fn fx_full(mode: ParityMode, odd_tree: bool, with_globals: bool, no_path: bool) -> Fx {
    fx_all(mode, odd_tree, with_globals, no_path, false)
}

fn fx_all(mode: ParityMode, odd_tree: bool, with_globals: bool, no_path: bool, tree_of_manager: bool) -> Fx {
    let dir = std::env::temp_dir().join(format!(
        "ow-skills-init-tree-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let bytes = package(odd_tree, no_path, tree_of_manager);
    let defs = DefStore::from_synthetic(&dir, &[("WillowGame.upk", &bytes)], IdentityPolicy::FirstInScanOrder, mode).unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), mode);
    let class = vm.find_class("WillowGame.WillowGlobals").unwrap();
    let globals = ow_vm::interp::instantiate_from_defaults(&mut vm, class).unwrap();
    let hooks = GlobalsOnly(with_globals.then_some(globals));
    vm.set_world_hooks(Rc::new(RefCell::new(hooks))).unwrap();
    let class = vm.find_class("WillowGame.WillowPlayerController").unwrap();
    let controller = ow_vm::interp::instantiate_from_defaults(&mut vm, class).unwrap();
    Pack.register(&mut vm).unwrap();
    Fx { vm, dir, controller, globals }
}

fn fx() -> Fx {
    fx_with(ParityMode::Off, false, true)
}

impl Fx {
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

    fn make(&mut self, class: &str) -> ObjRef {
        let c = self.vm.find_class(class).unwrap();
        ow_vm::interp::instantiate_from_defaults(&mut self.vm, c).unwrap()
    }

    /// A manager in the globals and a player class whose `SkillTreePath` is `path`.
    fn ready(&mut self, path: &str) -> (ObjRef, ObjRef) {
        let manager = self.make("WillowGame.WillowPlayerPawnDataManager");
        let g = self.globals;
        self.set(g, "ThePlayerPawnDataManager", Value::Object(Some(manager)));
        let class = self.make("WillowGame.PlayerClassDefinition");
        self.set(class, "SkillTreePath", Value::Str(path.into()));
        let c = self.controller;
        self.set(c, "PlayerClass", Value::Object(Some(class)));
        (manager, class)
    }

    fn call(&mut self) -> Result<Value, VmError> {
        let f = self.vm.find_function(PATH)?;
        Ok(self.vm.call(f, Some(self.controller), Vec::new())?.ret)
    }

    fn requests(&mut self) -> Vec<SkillTreeRequest> {
        pending(&mut self.vm).unwrap().borrow().requests.clone()
    }

    fn tree(&self) -> Option<ObjRef> {
        match self.get(self.controller, "PlayerSkillTree") {
            Value::Object(o) => o,
            other => panic!("{other:?}"),
        }
    }

    /// The live `PlayerSkillTree` objects.
    fn objects(&mut self) -> usize {
        let c = self.vm.find_class(TREE_CLASS).unwrap();
        self.vm.space.heap.live(&self.vm.space.classes, c).count()
    }
}

/// The slice minimum: a new tree, outer the controller, in `PlayerSkillTree`; one pending request with all seven
/// arguments for a dotted path; nothing delivered.
#[test]
fn a_new_tree_and_one_pending_request_for_a_dotted_path() {
    let mut x = fx();
    let (manager, _) = x.ready("GD_Siren_Skills.SkillTree");
    x.call().unwrap();
    let tree = x.tree().expect("a tree");
    let o = x.vm.space.heap.get(tree).unwrap();
    assert_eq!(o.outer, Some(x.controller), "outer the controller");
    assert_eq!(Some(o.class), x.vm.space.classes.by_path(TREE_CLASS));
    let c = x.controller;
    assert_eq!(
        x.requests(),
        [SkillTreeRequest {
            path: "GD_Siren_Skills.SkillTree".into(),
            player_context: c,
            payload: None,
            instigator: c,
            int_param: 0,
            float_param: 0.0,
            event: EVENT.into(),
            manager,
        }]
    );
}

/// The four request cases in Off (A1, A2): a dotless path, the empty string included, records nothing; no manager or
/// no player class makes no request. The tree is made in every case.
#[test]
fn no_request_without_a_dot_a_manager_or_a_player_class() {
    for path in ["NoDotHere", ""] {
        let mut x = fx();
        x.ready(path);
        x.call().unwrap();
        assert!(x.tree().is_some(), "{path:?}: the tree is made");
        assert!(x.requests().is_empty(), "{path:?}: nothing recorded");
    }
    let mut x = fx();
    x.ready("A.B");
    let g = x.globals;
    x.set(g, "ThePlayerPawnDataManager", Value::Object(None));
    x.call().unwrap();
    assert!(x.tree().is_some() && x.requests().is_empty(), "no manager: no request");
    let mut x = fx();
    x.ready("A.B");
    let c = x.controller;
    x.set(c, "PlayerClass", Value::Object(None));
    x.call().unwrap();
    assert!(x.tree().is_some() && x.requests().is_empty(), "no player class: no request");
}

/// P2: an existing tree is the named `NotYet` before any write (no object made, no request).
#[test]
fn an_existing_tree_is_refused_before_any_write() {
    let mut x = fx();
    x.ready("A.B");
    let old = x.make(TREE_CLASS);
    let c = x.controller;
    x.set(c, "PlayerSkillTree", Value::Object(Some(old)));
    let before = x.objects();
    let VmError::NotYet(n) = x.call().unwrap_err() else { panic!("a NotYet") };
    assert_eq!(n.missing, Missing::NotRead);
    assert_eq!(n.what, EXISTING_TREE, "the note's exact text");
    assert_eq!(
        n.what,
        "InitPlayerSkillTree: the reset of an existing skill tree (NATIVE_SKILLS.md section 3.1 is not ported)"
    );
    assert!(!n.what.contains(PARTIAL_WRITES));
    assert_eq!(x.tree(), Some(old));
    assert_eq!(x.objects(), before, "no object made");
    assert!(x.requests().is_empty());
}

/// P4: a world without globals answers its own `NotYet` (layer absent), unchanged, before any write.
#[test]
fn a_world_without_globals_answers_its_own_not_yet_before_any_write() {
    let mut x = fx_with(ParityMode::Off, false, false);
    let before = x.objects();
    let VmError::NotYet(n) = x.call().unwrap_err() else { panic!("a NotYet") };
    assert!(matches!(n.missing, Missing::LayerAbsent(_)), "{n}");
    assert!(!n.what.contains(PARTIAL_WRITES), "{n}");
    assert_eq!(x.tree(), None);
    assert_eq!(x.objects(), before);
}

/// P1: `PlayerSkillTree` declared as something other than an object property is a data error, before any write.
#[test]
fn a_tree_field_of_another_kind_is_a_data_error() {
    let mut x = fx_with(ParityMode::Off, true, true);
    let before = x.objects();
    let e = x.call().unwrap_err();
    assert!(matches!(e, VmError::Script { .. }), "{e:?}");
    assert_eq!(x.objects(), before);
}

/// A `NotYet` after a write gets the prefix once and keeps its kind and record; other errors pass unchanged.
#[test]
fn a_not_yet_after_a_write_is_marked_once() {
    let n = after_write(ow_core::NotYet::not_read("Toy.Thing", "TOY.md").into());
    let VmError::NotYet(n) = n else { panic!() };
    assert_eq!(n.what, format!("{PARTIAL_WRITES}: Toy.Thing"));
    assert_eq!((n.missing, n.record), (Missing::NotRead, "TOY.md"));
    let VmError::NotYet(again) = after_write(VmError::NotYet(n)) else { panic!() };
    assert_eq!(again.what.matches(PARTIAL_WRITES).count(), 1);
    let other = after_write(bad("toy"));
    assert!(matches!(other, VmError::Script { .. }));
}

/// Reference binds nothing (architecture R3).
#[test]
fn reference_binds_nothing() {
    let mut x = fx_with(ParityMode::Reference, false, true);
    let id = x.vm.find_function(PATH).unwrap();
    let f = x.vm.function(id).unwrap();
    let key = f.native_key.clone().unwrap();
    assert!(!x.vm.has_native(&key));
    let o = fx();
    assert!(o.vm.has_native(&key));
}

/// P5b is read only for a request (the note at 094415f3): a player class whose class declares no `SkillTreePath` is
/// not read without a manager (no request, the tree made), and is a data error before any write with one.
#[test]
fn the_skill_tree_path_is_read_only_for_a_request() {
    let mut x = fx_full(ParityMode::Off, false, true, true);
    let class = x.make("WillowGame.PlayerClassDefinition");
    let c = x.controller;
    x.set(c, "PlayerClass", Value::Object(Some(class)));
    x.call().unwrap();
    assert!(x.tree().is_some() && x.requests().is_empty(), "no manager: SkillTreePath is not read");
    let mut x = fx_full(ParityMode::Off, false, true, true);
    let manager = x.make("WillowGame.WillowPlayerPawnDataManager");
    let g = x.globals;
    x.set(g, "ThePlayerPawnDataManager", Value::Object(Some(manager)));
    let class = x.make("WillowGame.PlayerClassDefinition");
    let c = x.controller;
    x.set(c, "PlayerClass", Value::Object(Some(class)));
    let before = x.objects();
    let e = x.call().unwrap_err();
    assert!(matches!(e, VmError::Script { .. }), "{e:?}");
    assert_eq!(x.objects(), before, "refused before the tree is allocated");
    assert_eq!(x.tree(), None);
}

/// An unloaded player class (review r1 of #475): P5b's read would need a load, a write, so it is a named `NotYet`
/// before any write, and the handle stays unloaded.
#[test]
fn an_unloaded_player_class_is_a_not_yet_before_any_write() {
    let mut x = fx();
    x.ready("A.B");
    let path = ow_core::ObjectPath::parse("WillowGame.ToyHolder").unwrap();
    let holder = x.vm.defs.find(&mut x.vm.space, &path).unwrap().unwrap();
    let Value::Object(Some(handle)) = x.get(holder, "Target") else { panic!() };
    assert!(x.vm.defs.is_unloaded(&x.vm.space, handle));
    let c = x.controller;
    x.set(c, "PlayerClass", Value::Object(Some(handle)));
    let before = x.objects();
    let VmError::NotYet(n) = x.call().unwrap_err() else { panic!("a NotYet") };
    assert!(n.what.contains("the player class") && n.what.contains("is not loaded"), "{n}");
    assert!(!n.what.starts_with(PARTIAL_WRITES), "{n}");
    assert!(x.vm.defs.is_unloaded(&x.vm.space, handle), "nothing loaded");
    assert_eq!((x.objects(), x.tree()), (before, None));
    assert!(x.requests().is_empty());
}

/// A tree field whose declared class cannot hold the tree (review r1 of #475): the heap's typed `WrongClass` error
/// after P3 and before W1, so no tree is allocated.
#[test]
fn a_tree_field_of_an_incompatible_class_is_refused_before_the_tree_is_made() {
    let mut x = fx_all(ParityMode::Off, false, true, false, true);
    let before = x.objects();
    let e = x.call().unwrap_err();
    assert!(matches!(e, VmError::Heap(ow_object::heap::HeapError::WrongClass { .. })), "{e:?}");
    assert_eq!(x.objects(), before, "no tree allocated");
}

/// A borrowed request queue (review r1 of #475): refused before W1, so a retry once the borrow is gone makes the tree
/// and queues the request.
#[test]
fn a_borrowed_queue_is_refused_before_the_tree_is_made() {
    let mut x = fx();
    x.ready("A.B");
    let queue = pending(&mut x.vm).unwrap();
    let before = x.objects();
    {
        let _held = queue.borrow();
        let e = x.call().unwrap_err();
        assert!(matches!(e, VmError::ServiceBorrowed(_)), "{e:?}");
        assert_eq!((x.objects(), x.tree()), (before, None), "nothing made");
    }
    x.call().unwrap();
    assert!(x.tree().is_some());
    assert_eq!(x.requests().len(), 1, "the retry queues the request");
}

/// P3 answers setup's own result (review r2 of #475): a lookup that failed at setup is returned as it was, and no
/// second lookup runs in the preflight.
#[test]
fn p3_answers_the_setup_lookup_error_unchanged() {
    let mut x = fx();
    x.ready("A.B");
    let original = VmError::Script { function: "toy-setup".into(), message: "the toy lookup failed".into() };
    let s = x.vm.service::<TreeClassSetup>().unwrap();
    s.borrow_mut().0 = Err(original.clone());
    let before = x.objects();
    assert_eq!(x.call().unwrap_err(), original);
    assert_eq!((x.objects(), x.tree()), (before, None), "nothing made");
}

/// P4 comes before P5a (review r2 of #475): with no globals, the world's own `NotYet` comes out first, whatever the
/// player class holds.
#[test]
fn no_globals_comes_out_before_the_player_class_is_read() {
    let mut x = fx_with(ParityMode::Off, false, false);
    let class = x.make("WillowGame.PlayerClassDefinition");
    let c = x.controller;
    x.set(c, "PlayerClass", Value::Object(Some(class)));
    let VmError::NotYet(n) = x.call().unwrap_err() else { panic!("a NotYet") };
    assert!(matches!(n.missing, Missing::LayerAbsent(_)), "{n}");
    assert_eq!(n.what, "the toy world's globals", "the world's NotYet, unchanged");
}

/// COLD-CLASS-AUDIT (lane COLD-CLASS-AUDIT-SKILLS): `InitPlayerSkillTree` on a controller, and with a player class, whose
/// classes have a layout but no loaded code (the fixture warms its classes through `find_class`): the existing-tree
/// refusal loads no class code, and leaves #518's counters and the name table unchanged.
#[test]
fn a_cold_controller_and_player_class_load_no_code() {
    let mut x = fx();
    x.ready("A.B");
    let cold = |x: &mut Fx, class: &str| {
        let vm = &mut x.vm;
        let (code, store) = vm.defs.code_mut();
        let key = code.find_class(store, class).unwrap();
        let c = code.define_layout(store, &mut vm.space.names, &mut vm.space.classes, key).unwrap();
        assert!(!vm.class_code_loaded(c), "{class} starts cold");
        vm.space.alloc(c, "Cold", None, ow_object::heap::Origin::Runtime).unwrap()
    };
    let controller = cold(&mut x, "WillowGame.ToyColdController");
    let class = cold(&mut x, "WillowGame.ToyColdClassDefinition");
    x.set(class, "SkillTreePath", Value::Str("A.B".into()));
    x.set(controller, "PlayerClass", Value::Object(Some(class)));
    let old = x.make(TREE_CLASS);
    x.set(controller, "PlayerSkillTree", Value::Object(Some(old)));
    let marks = |vm: &Vm| (vm.class_code_loads(), vm.space.heap.committed_writes(), vm.rng.draws(), vm.defs.load_transitions(), vm.space.names.len());
    let before = marks(&x.vm);
    let f = x.vm.find_function(PATH).unwrap();
    let r = x.vm.call(f, Some(controller), Vec::new());
    assert!(matches!(&r, Err(VmError::NotYet(n)) if n.what == EXISTING_TREE), "{r:?}");
    assert_eq!(marks(&x.vm), before, "no code load, write, draw, store load or name");
}
