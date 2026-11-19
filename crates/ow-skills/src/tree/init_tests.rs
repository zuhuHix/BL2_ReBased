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
/// declares `PlayerSkillTree` as a string instead (P1's data error).
fn package(odd_tree: bool, no_path: bool) -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class", "Function", "ObjectProperty", "StrProperty"]);
    let object = t.class("Object", 0);
    let tree = t.class("PlayerSkillTree", object);
    let class_def = t.class("PlayerClassDefinition", object);
    if !no_path {
        t.prop("Str", class_def, "SkillTreePath", 0, 0);
    }
    let controller = t.class("WillowPlayerController", object);
    if odd_tree {
        t.prop("Str", controller, "PlayerSkillTree", 0, 0);
    } else {
        t.prop("Object", controller, "PlayerSkillTree", 0, tree);
    }
    t.prop("Object", controller, "PlayerClass", 0, class_def);
    let spec = FunctionSpec::new(NATIVE, "InitPlayerSkillTree");
    t.function(controller, "InitPlayerSkillTree", &[], None, &[], &spec, |_| None);
    let manager = t.class("WillowPlayerPawnDataManager", object);
    let globals = t.class("WillowGlobals", object);
    t.prop("Object", globals, "ThePlayerPawnDataManager", 0, manager);
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
    let dir = std::env::temp_dir().join(format!(
        "ow-skills-init-tree-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let bytes = package(odd_tree, no_path);
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
    assert!(n.what.starts_with("InitPlayerSkillTree: the reset of an existing skill tree"), "{n}");
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
