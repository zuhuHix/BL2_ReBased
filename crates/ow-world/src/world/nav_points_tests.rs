//! `WorldInfo.AllNavigationPoints` on an invented `Engine.upk` (AGENTS.md rule 9). The package has an actor base with
//! `bDeleteMe`, a `WorldInfo` with `NavigationPointList`, a `NavigationPoint` with `nextNavigationPoint`, two
//! subclasses (`PlayerStart`, `Teleporter`) and a plain actor. It also has the native iterator and one script function
//! on the `WorldInfo`:
//!
//! - `Walk()`: `foreach AllNavigationPoints(WalkClass, Cursor) { Seen(Cursor); }`, the shape of `ChoosePlayerStart`'s
//!   loop (a call per yield), with `Seen` a test native that logs its argument.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::ParityMode;
use ow_core::notyet::Missing;
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_package::testkit::script::{Asm, CPF_OUT, FunctionSpec, Param, ScriptToy};
use ow_vm::api::{NativeCall, StubPolicy, Vm};
use ow_vm::error::VmError;

use crate::actors::Actors;

const NATIVE_ITERATOR: u32 = 0x400 | 0x20000 | 0x4;
const NATIVE: u32 = 0x400 | 0x20000 | 0x1;
const SCRIPT: u32 = 0x1 | 0x2 | 0x20000;
static NEXT: AtomicU32 = AtomicU32::new(0);

fn le(words: &[i32]) -> Vec<u8> {
    words.iter().flat_map(|w| w.to_le_bytes()).collect()
}

fn package() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&[
        "Class",
        "Function",
        "ScriptStruct",
        "BoolProperty",
        "ObjectProperty",
        "ClassProperty",
    ]);
    let object = t.class("Object", 0);
    let actor = t.class("Actor", object);
    t.prop("Bool", actor, "bDeleteMe", 0, 0);
    t.prop("Bool", actor, "bPendingDelete", 0, 0);
    let nav = t.class("NavigationPoint", actor);
    t.prop("Object", nav, "nextNavigationPoint", 0, nav);
    t.class("PlayerStart", nav);
    t.class("Teleporter", nav);
    t.class("Prop", actor);
    let wi = t.class("WorldInfo", actor);
    t.prop("Object", wi, "NavigationPointList", 0, nav);
    let cursor = t.prop("Object", wi, "Cursor", 0, nav);
    let class_import = t.import("Class");
    let walk_class = t.prop_tail("Class", wi, "WalkClass", 0, &le(&[class_import, nav]));
    let iterator_ids = t
        .function(
            wi,
            "AllNavigationPoints",
            &[
                Param::new("Class", "BaseClass"),
                Param { flags: CPF_OUT, type_ref: nav, ..Param::new("Object", "N") },
            ],
            None,
            &[],
            &FunctionSpec::new(NATIVE_ITERATOR, "AllNavigationPoints"),
            |_| None,
        );
    let iterator = iterator_ids.function;
    // `ScriptToy::function` writes one type word; a class parameter carries two (the class import, then the meta
    // class), so the parameter's payload is rewritten in `prop_tail`'s layout.
    let mut payload = le(&[0]);
    payload.extend_from_slice(&t.none);
    payload.extend_from_slice(&le(&[0, 1]));
    payload.extend_from_slice(&0x80u64.to_le_bytes());
    payload.extend_from_slice(&t.none);
    payload.extend_from_slice(&le(&[0, class_import, nav]));
    t.p.set_payload(iterator_ids.get("BaseClass"), payload);
    let seen = t
        .function(
            wi,
            "Seen",
            &[Param { type_ref: nav, ..Param::new("Object", "P") }],
            None,
            &[],
            &FunctionSpec::new(NATIVE, "Seen"),
            |_| None,
        )
        .function;
    t.function(wi, "Walk", &[], None, &[], &FunctionSpec::new(SCRIPT, "Walk"), |_| {
        let mut a = Asm::new();
        a.stmt().raw(&[0x2F]).final_function(iterator, |a| {
            a.instance(walk_class).instance(cursor);
        });
        let end = a.bytes.len();
        a.w(0);
        a.stmt().final_function(seen, |a| {
            a.instance(cursor);
        });
        a.stmt().raw(&[0x31]);
        a.patch(end);
        a.stmt().raw(&[0x30]);
        a.stmt().raw(&[0x04, 0x0B]);
        a.end();
        Some(a)
    });
    t.p.build()
}

struct Fx {
    vm: Vm,
    wi: ObjRef,
    seen: Rc<RefCell<Vec<ObjRef>>>,
    dir: PathBuf,
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn fixture_in(mode: ParityMode) -> Fx {
    let dir = std::env::temp_dir().join(format!(
        "ow-world-navpoints-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let bytes = package();
    let defs = DefStore::from_synthetic(&dir, &[("Engine.upk", &bytes)], IdentityPolicy::FirstInScanOrder, mode).unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), mode)
        .with_stub_policy(StubPolicy::refuse_all());
    let seen = Rc::new(RefCell::new(Vec::new()));
    let sink = seen.clone();
    vm.register_native(
        "WorldInfo.Seen(object)",
        Rc::new(move |c: &mut NativeCall<'_>| {
            match c.arg(0)? {
                Value::Object(Some(o)) => sink.borrow_mut().push(*o),
                other => panic!("Seen got {other:?}"),
            }
            Ok(Value::None)
        }),
    )
    .unwrap();
    let actor = vm.find_class("Engine.Actor").unwrap();
    let wi_class = vm.find_class("Engine.WorldInfo").unwrap();
    let wi = vm.space.alloc(wi_class, "ToyWorldInfo", None, Origin::Runtime).unwrap();
    let actors = vm.add_service(Actors::new(actor)).unwrap();
    actors.borrow_mut().place_world_info(wi).unwrap();
    if mode == ParityMode::Off {
        super::register(&mut vm).unwrap();
    }
    Fx { vm, wi, seen, dir }
}

fn fixture() -> Fx {
    fixture_in(ParityMode::Off)
}

impl Fx {
    /// A placed actor of `class`.
    fn point(&mut self, class: &str, name: &str) -> ObjRef {
        let o = self.unplaced(class, name);
        self.vm.service::<Actors>().unwrap().borrow_mut().place(o).unwrap();
        o
    }

    fn unplaced(&mut self, class: &str, name: &str) -> ObjRef {
        let c = self.vm.find_class(&format!("Engine.{class}")).unwrap();
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

    /// `NavigationPointList` = the first of `chain`, each linked to the next.
    fn link(&mut self, chain: &[ObjRef]) {
        let wi = self.wi;
        self.set(wi, "NavigationPointList", Value::Object(chain.first().copied()));
        for pair in chain.windows(2) {
            self.set(pair[0], "nextNavigationPoint", Value::Object(Some(pair[1])));
        }
    }

    /// `Walk()` over `class` (`None`: a None `WalkClass`); the points the body saw.
    fn walk(&mut self, class: Option<&str>) -> Result<Vec<ObjRef>, VmError> {
        let c = class.map(|c| self.vm.find_class(&format!("Engine.{c}")).unwrap());
        let wi = self.wi;
        self.set(wi, "WalkClass", Value::Class(c));
        self.seen.borrow_mut().clear();
        let f = self.vm.find_function("Engine.WorldInfo.Walk")?;
        self.vm.call(f, Some(wi), vec![])?;
        Ok(self.seen.borrow().clone())
    }
}

fn expect_not_read(r: Result<Vec<ObjRef>, VmError>, text: &str) {
    match r {
        Err(VmError::NotYet(n)) => {
            assert_eq!(n.missing, Missing::NotRead);
            assert_eq!(n.what, text);
            assert_eq!(n.record, super::RECORD);
        }
        other => panic!("expected NotYet {text:?}, got {other:?}"),
    }
}

/// The declaration's own key is the one bound.
#[test]
fn the_toy_declaration_carries_the_installed_key() {
    let mut x = fixture();
    let f = x.vm.find_function("Engine.WorldInfo.AllNavigationPoints").unwrap();
    assert_eq!(x.vm.function(f).unwrap().native_key.as_deref(), Some(super::KEY));
}

/// List order, not placement order, and the subclass filter: the base class yields every point, `PlayerStart` only
/// the player starts, `Teleporter` only the teleporter. The script body runs once per yield.
#[test]
fn the_walk_follows_the_list_and_keeps_the_class_and_its_subclasses() {
    let mut x = fixture();
    let a = x.point("PlayerStart", "StartA");
    let t = x.point("Teleporter", "Tele");
    let n = x.point("NavigationPoint", "Plain");
    let b = x.point("PlayerStart", "StartB");
    x.point("Prop", "NotAPoint");
    x.link(&[b, n, t, a]);
    assert_eq!(x.walk(Some("NavigationPoint")).unwrap(), vec![b, n, t, a]);
    assert_eq!(x.walk(Some("PlayerStart")).unwrap(), vec![b, a]);
    assert_eq!(x.walk(Some("Teleporter")).unwrap(), vec![t]);
}

/// No list and no placed point: nothing is yielded (the slice's case), and a second walk leaves the heap unchanged.
#[test]
fn an_empty_list_in_a_level_without_points_yields_nothing() {
    let mut x = fixture();
    x.point("Prop", "NotAPoint");
    assert_eq!(x.walk(Some("PlayerStart")).unwrap(), Vec::<ObjRef>::new());
    let before = format!("{:?}", x.vm.space.heap);
    assert_eq!(x.walk(Some("PlayerStart")).unwrap(), Vec::<ObjRef>::new());
    assert_eq!(format!("{:?}", x.vm.space.heap), before, "nothing written");
    // Review r1 of #513, item 4: a successful empty walk clears the cursor.
    let sentinel = x.unplaced("Teleporter", "Sentinel");
    let wi = x.wi;
    x.set(wi, "Cursor", Value::Object(Some(sentinel)));
    assert_eq!(x.walk(Some("PlayerStart")).unwrap(), Vec::<ObjRef>::new());
    assert_eq!(x.get(wi, "Cursor"), Value::Object(None), "an empty walk clears the cursor");
}

/// An empty list in a level that places a point, and a list that misses a placed point: `UNREACHED`, before the
/// body runs once.
#[test]
fn placed_points_the_list_does_not_reach_are_not_yet() {
    let mut x = fixture();
    let a = x.point("PlayerStart", "StartA");
    expect_not_read(x.walk(Some("PlayerStart")), super::UNREACHED);
    let b = x.point("PlayerStart", "StartB");
    x.link(&[a]);
    // Review r1 of #513, item 4: ask for the reachable point's own class, with the cursor seeded with a sentinel; the
    // refusal comes before any yield, so the cursor keeps the sentinel and the body never runs.
    let sentinel = x.unplaced("Teleporter", "Sentinel");
    let wi = x.wi;
    x.set(wi, "Cursor", Value::Object(Some(sentinel)));
    expect_not_read(x.walk(Some("PlayerStart")), super::UNREACHED);
    assert!(x.seen.borrow().is_empty(), "the body never ran");
    assert_eq!(x.get(wi, "Cursor"), Value::Object(Some(sentinel)), "the cursor keeps the sentinel");
    x.link(&[a, b]);
    assert_eq!(x.walk(Some("PlayerStart")).unwrap(), vec![a, b]);
    // A destroyed placed point the list does not reach is not counted.
    let c = x.point("PlayerStart", "Gone");
    x.set(c, "bDeleteMe", Value::Bool(true));
    assert_eq!(x.walk(Some("PlayerStart")).unwrap(), vec![a, b]);
}

/// The cycle guard: a list that comes back to a visited point is `LOOP`, a self-link included; the body never runs.
#[test]
fn a_cyclic_list_is_not_yet() {
    let mut x = fixture();
    let a = x.point("PlayerStart", "StartA");
    let b = x.point("Teleporter", "Tele");
    x.link(&[a, b]);
    x.set(b, "nextNavigationPoint", Value::Object(Some(a)));
    expect_not_read(x.walk(Some("PlayerStart")), super::LOOP);
    assert!(x.seen.borrow().is_empty(), "the body never ran");
    x.link(&[a]);
    x.set(a, "nextNavigationPoint", Value::Object(Some(a)));
    expect_not_read(x.walk(Some("PlayerStart")), super::LOOP);
}

/// A destroyed or `bDeleteMe` entry in the list: `DELETED` (whether it is skipped is not settled).
#[test]
fn a_deleted_entry_is_not_yet() {
    let mut x = fixture();
    let a = x.point("PlayerStart", "StartA");
    let b = x.point("PlayerStart", "StartB");
    x.link(&[a, b]);
    x.set(b, "bDeleteMe", Value::Bool(true));
    expect_not_read(x.walk(Some("PlayerStart")), super::DELETED);
    assert!(x.seen.borrow().is_empty(), "the body never ran");
}

/// Review r1 of #513, item 1: an entry with only `bPendingDelete` set (as `Actor.Destroy` leaves it while `Destroyed()`
/// runs, still placed) is `DELETED` too, before the cursor is written or the body runs.
#[test]
fn a_pending_delete_entry_is_not_yet() {
    let mut x = fixture();
    let a = x.point("PlayerStart", "StartA");
    let b = x.point("PlayerStart", "StartB");
    x.link(&[a, b]);
    x.set(b, "bPendingDelete", Value::Bool(true));
    let sentinel = x.unplaced("Teleporter", "Sentinel");
    let wi = x.wi;
    x.set(wi, "Cursor", Value::Object(Some(sentinel)));
    expect_not_read(x.walk(Some("PlayerStart")), super::DELETED);
    assert!(x.seen.borrow().is_empty(), "the body never ran");
    assert_eq!(x.get(wi, "Cursor"), Value::Object(Some(sentinel)), "the cursor keeps the sentinel");
}

/// Broken references: an entry the level never placed is `UNPLACED`; an entry that is not a navigation point (planted
/// past the toy decoder's class check) is `NOT_A_POINT`.
#[test]
fn broken_entries_are_not_yet() {
    let mut x = fixture();
    let a = x.point("PlayerStart", "StartA");
    let loose = x.unplaced("PlayerStart", "Loose");
    x.link(&[a, loose]);
    expect_not_read(x.walk(Some("PlayerStart")), super::UNPLACED);
    let prop = x.point("Prop", "NotAPoint");
    x.link(&[a]);
    let class = x.vm.space.heap.get(a).unwrap().class;
    let p = x.vm.space.prop(class, "nextNavigationPoint").unwrap();
    *x.vm.space.slot_mut(a, PropAddr::scalar(p)).unwrap() = Value::Object(Some(prop));
    expect_not_read(x.walk(Some("PlayerStart")), super::NOT_A_POINT);
}

/// A None `BaseClass`: `NONE_BASE`.
#[test]
fn a_none_base_class_is_not_yet() {
    let mut x = fixture();
    let a = x.point("PlayerStart", "StartA");
    x.link(&[a]);
    expect_not_read(x.walk(None), super::NONE_BASE);
}

/// Reference binds nothing: the VM's stub policy answers the declaration.
#[test]
fn reference_leaves_the_iterator_unbound() {
    let mut x = fixture_in(ParityMode::Reference);
    let r = x.walk(Some("PlayerStart"));
    assert!(matches!(&r, Err(VmError::UnboundNative(k)) if k == super::KEY), "the stub policy refuses: {r:?}");
}
