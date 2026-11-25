//! Tests of `UnlockPlayerClassAchievementIfConditionsMet` (NATIVE_UNLOCK_CLASS_ACHIEVEMENT.md section 4, tests 1 to
//! 11; lane IMPL-UNLOCKPLAYERCLASSACHIEVEMENT) on two invented packages built with `ow_package::testkit` (AGENTS.md
//! rule 9). `Engine` has `Object`. `WillowGame` has:
//! - `PlayerClassDefinition` with `PlayerClassAchievementData` (of its own `PlayerClassAchievementUnlockData`), and a
//!   toy child;
//! - `WillowPlayerController` with `PlayerClass` and the native, but no `ClientUnlockAchievement`;
//! - `ToyClientController`, its child, with a native stand-in for `ClientUnlockAchievement` the tests bind, and
//!   `ToyLazyController`, a child of that whose code a test leaves unloaded;
//! - `ToyOther`, which is not a class definition, holding a same-shaped lookalike struct;
//! - a holder export whose fields are store handles not yet loaded (a controller and a class definition).
//!
//! Every value is invented; every call goes through the VM. The action-skill pack's half of test 11 is in `ow-skills`
//! (`action_skill/class_achievement_tests.rs`).

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::{Missing, NotYet, ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_package::testkit::reflect::{toy_array_of, toy_struct};
use ow_package::testkit::script::{FunctionSpec, Param, ScriptToy};
use ow_vm::api::{NativeCall, StubPolicy, Vm};
use ow_vm::error::VmError;

use super::*;

const NATIVE: u32 = 0x400 | 0x20000;
static NEXT: AtomicU32 = AtomicU32::new(0);

const IMPORTS: [&str; 9] = [
    "Class",
    "Function",
    "ScriptStruct",
    "ObjectProperty",
    "IntProperty",
    "ByteProperty",
    "StructProperty",
    "ArrayProperty",
    "BoolProperty",
];

const CLIENT_UNLOCK: &str = "WillowGame.ToyClientController.ClientUnlockAchievement";
const DEFINITION: &str = "WillowGame.PlayerClassDefinition";

fn native(t: &mut ScriptToy, owner: i32, name: &str, flags: u32, params: &[Param], result: Option<(&str, i32)>) {
    t.function(owner, name, params, result, &[], &FunctionSpec::new(flags, name), |_| None);
}

fn engine() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    t.class("Object", 0);
    t.build()
}

/// The `WillowGame` package; with `mistyped`, the entries' `IntData` is declared a byte.
fn willow_game(mistyped: bool) -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let object = t.p.add_import("Core", "Class", engine, "Object");
    let definition = t.class("PlayerClassDefinition", object);
    let data = toy_struct(&mut t, definition, "PlayerClassAchievementUnlockData", None);
    t.prop("Byte", data, "UnlockType", 0, 0);
    t.prop("Byte", data, "Achievement", 0, 0);
    t.prop(if mistyped { "Byte" } else { "Int" }, data, "IntData", 0, 0);
    toy_array_of(&mut t, definition, "PlayerClassAchievementData", "Struct", data);
    t.class("ToyChildClassDefinition", definition);
    // Not a class definition, with a struct of the same fields.
    let other = t.class("ToyOther", object);
    let look = toy_struct(&mut t, other, "PlayerClassAchievementUnlockData", None);
    t.prop("Byte", look, "UnlockType", 0, 0);
    t.prop("Byte", look, "Achievement", 0, 0);
    t.prop("Int", look, "IntData", 0, 0);
    t.prop("Struct", other, "ToyLook", 0, look);
    let controller = t.class("WillowPlayerController", object);
    t.prop("Object", controller, "PlayerClass", 0, object);
    t.prop("Int", controller, "ToyUnlocks", 0, 0);
    let params = [
        Param::new("Byte", "AchievementUnlockType"),
        Param::new("Int", "IntData"),
        Param::new("Bool", "bCanBeGreater"),
        Param::new("Bool", "bCanBeLess"),
    ];
    native(&mut t, controller, "UnlockPlayerClassAchievementIfConditionsMet", NATIVE | 0x20, &params, Some(("Bool", 0)));
    let client = t.class("ToyClientController", controller);
    native(&mut t, client, "ClientUnlockAchievement", NATIVE, &[Param::new("Int", "AchievementId")], None);
    t.class("ToyLazyController", client);
    // Store handles the store leaves unloaded, held by a holder export.
    let holder = t.class("ToyHolder", object);
    t.prop("Object", holder, "Controller", 0, object);
    t.prop("Object", holder, "Definition", 0, object);
    t.prop("Object", holder, "Other", 0, object);
    let mut empty = 0i32.to_le_bytes().to_vec();
    empty.extend_from_slice(&t.none);
    let stored_controller = t.p.add_export(client, "ToyStoredController", empty.clone());
    let stored_definition = t.p.add_export(definition, "ToyStoredClass", empty.clone());
    let stored_other = t.p.add_export(other, "ToyStoredOther", empty);
    let mut h = 0i32.to_le_bytes().to_vec();
    h.extend(t.p.object_tag("Controller", stored_controller));
    h.extend(t.p.object_tag("Definition", stored_definition));
    h.extend(t.p.object_tag("Other", stored_other));
    h.extend_from_slice(&t.none);
    t.p.add_export(holder, "ToyHolderObject", h);
    t.build()
}

/// What the test's `ClientUnlockAchievement` stand-in does on each call.
#[derive(Clone, Copy)]
enum Client {
    /// Records the id and writes nothing.
    Record,
    /// Records the id, writes the controller's `ToyUnlocks`, then refuses (the same call).
    WriteAndRefuse,
    /// Records the id and refuses at once, writing nothing.
    Refuse,
    /// Records the id; the first call returns writing nothing, a later one refuses (review r1 of #668, item 2).
    RefuseSecond,
}

#[derive(Clone, Copy)]
struct Setup {
    mode: ParityMode,
    client: Client,
    mistyped: bool,
    /// Whether the native's function is already bound, to a stand-in answering true, before [`register`] runs.
    prebound: bool,
}

const OFF: Setup = Setup { mode: ParityMode::Off, client: Client::Record, mistyped: false, prebound: false };

struct Fx {
    vm: Vm,
    dir: PathBuf,
    pc: ObjRef,
    definition: ObjRef,
    calls: Rc<RefCell<Vec<i32>>>,
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn fx_with(setup: Setup) -> Fx {
    let dir = std::env::temp_dir().join(format!(
        "ow-inventory-class-achievement-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let (e, w) = (engine(), willow_game(setup.mistyped));
    let defs = DefStore::from_synthetic(
        &dir,
        &[("Engine.upk", e.as_slice()), ("WillowGame.upk", w.as_slice())],
        IdentityPolicy::FirstInScanOrder,
        setup.mode,
    )
    .unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), setup.mode)
        .with_stub_policy(StubPolicy::refuse_all());
    for c in [DEFINITION, "WillowGame.ToyChildClassDefinition", "WillowGame.ToyOther", "WillowGame.ToyClientController"] {
        vm.find_class(c).unwrap();
    }
    let calls = Rc::new(RefCell::new(Vec::new()));
    let seen = calls.clone();
    let client = setup.client;
    let f = vm.find_function(CLIENT_UNLOCK).unwrap();
    let key = vm.function(f).unwrap().native_key.clone().unwrap();
    vm.register_native(
        &key,
        Rc::new(move |c: &mut NativeCall<'_>| {
            let id = match c.arg(0)? {
                Value::Int(i) => *i,
                other => panic!("{other:?}"),
            };
            seen.borrow_mut().push(id);
            let refuse = || Err(NotYet::not_read("an invented online subsystem", RECORD).into());
            match client {
                Client::Record => Ok(Value::None),
                Client::Refuse => refuse(),
                Client::RefuseSecond if seen.borrow().len() > 1 => refuse(),
                Client::RefuseSecond => Ok(Value::None),
                Client::WriteAndRefuse => {
                    let this = c.this.unwrap();
                    let class = c.vm.space.heap.get(this)?.class;
                    let p = c.vm.space.prop(class, "ToyUnlocks").unwrap();
                    c.vm.space.write(this, PropAddr::scalar(p), Value::Int(id))?;
                    refuse()
                }
            }
        }),
    )
    .unwrap();
    if setup.prebound {
        let f = vm.find_function(PATH).unwrap();
        vm.register_native_for(f, Rc::new(|_: &mut NativeCall<'_>| Ok(Value::Bool(true)))).unwrap();
    }
    register(&mut vm).unwrap();
    let pc = make(&mut vm, "WillowGame.ToyClientController");
    let definition = make(&mut vm, DEFINITION);
    let mut x = Fx { vm, dir, pc, definition, calls };
    x.set(pc, "PlayerClass", Value::Object(Some(definition)));
    x
}

fn fx() -> Fx {
    fx_with(OFF)
}

fn make(vm: &mut Vm, class: &str) -> ObjRef {
    let c = vm.find_class(class).unwrap();
    ow_vm::interp::instantiate_from_defaults(vm, c).unwrap()
}

/// An entry: (unlock type, achievement, threshold).
type Row = (u8, u8, i32);

impl Fx {
    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap();
        self.vm.space.write(o, PropAddr::scalar(p), v).unwrap();
    }

    /// `o`'s `PlayerClassAchievementData` set to `rows`, each entry of struct `ty`, past the heap's checks.
    fn put_rows_of(&mut self, o: ObjRef, ty: ow_object::ids::StructId, rows: &[Row]) {
        let layout = self.vm.space.classes.struct_layout(ty).unwrap();
        let mut entries = Vec::new();
        for &(unlock, achievement, int) in rows {
            let mut fields = vec![Value::None; 3];
            for (name, v) in
                [("UnlockType", Value::Byte(unlock)), ("Achievement", Value::Byte(achievement)), ("IntData", Value::Int(int))]
            {
                let (index, _) = layout.field(self.vm.space.names.find(name).unwrap()).unwrap();
                fields[usize::try_from(index).unwrap()] = v;
            }
            entries.push(Value::structure(ty, fields));
        }
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, "PlayerClassAchievementData").unwrap();
        *self.vm.space.slot_mut(o, PropAddr::scalar(p)).unwrap() = Value::array(entries);
    }

    /// The definition's entries set to `rows`.
    fn rows(&mut self, rows: &[Row]) {
        let ty = self.declared_struct();
        let d = self.definition;
        self.put_rows_of(d, ty, rows);
    }

    fn declared_struct(&self) -> ow_object::ids::StructId {
        let class = self.vm.space.heap.get(self.definition).unwrap().class;
        let p = self.vm.space.prop(class, "PlayerClassAchievementData").unwrap();
        match &self.vm.space.classes.decl(p).unwrap().kind {
            PropKind::Array(inner) => match **inner {
                PropKind::Struct(ty) => ty,
                ref k => panic!("{k:?}"),
            },
            k => panic!("{k:?}"),
        }
    }

    /// The native on `this` with `args` as given.
    fn call_on(&mut self, this: ObjRef, args: Vec<Value>) -> Result<Value, VmError> {
        let f = self.vm.find_function(PATH)?;
        Ok(self.vm.call(f, Some(this), args)?.ret)
    }

    /// The native on the controller with (type, count, bCanBeGreater, bCanBeLess).
    fn unlock(&mut self, unlock: u8, int: i32, greater: bool, less: bool) -> Result<Value, VmError> {
        let pc = self.pc;
        self.call_on(pc, vec![Value::Byte(unlock), Value::Int(int), Value::Bool(greater), Value::Bool(less)])
    }

    fn calls(&self) -> Vec<i32> {
        self.calls.borrow().clone()
    }

    /// #518's three counters, the name table's length and the heap.
    fn state(&self) -> (u64, u64, u64, usize, String) {
        (
            self.vm.space.heap.committed_writes(),
            self.vm.rng.draws(),
            self.vm.defs.load_transitions(),
            self.vm.space.names.len(),
            format!("{:?}", self.vm.space.heap),
        )
    }

    /// The holder export's field `n`: a store handle not yet loaded.
    fn held(&mut self, n: &str) -> ObjRef {
        let path = ObjectPath::parse("WillowGame.ToyHolderObject").unwrap();
        let holder = self.vm.defs.find(&mut self.vm.space, &path).unwrap().unwrap();
        match crate::equip::read_prop(&self.vm, holder, n) {
            Some(Value::Object(Some(o))) => {
                assert!(self.vm.defs.is_unloaded(&self.vm.space, o), "{n} is a handle");
                o
            }
            other => panic!("{n}: {other:?}"),
        }
    }

    /// One refusal or false answer with nothing changed: no write, draw, load, name or event.
    fn clean(&mut self, run: impl FnOnce(&mut Fx) -> Result<Value, VmError>) -> Result<Value, VmError> {
        let before = (self.state(), self.calls());
        let r = run(self);
        assert_eq!((self.state(), self.calls()), before, "nothing changed: {r:?}");
        r
    }
}

fn not_yet(r: Result<Value, VmError>) -> NotYet {
    match r {
        Err(VmError::NotYet(n)) => n,
        other => panic!("expected NotYet, got {other:?}"),
    }
}

fn script(r: Result<Value, VmError>, needle: &str) {
    match r {
        Err(VmError::Script { message, .. }) => assert!(message.contains(needle), "{message}"),
        other => panic!("expected a script error naming {needle}, got {other:?}"),
    }
}

/// Test 1: link 9's shape. One entry of type 2 with threshold 100; (2, 1, true, false) is false with no event and no
/// write; the same with `bCanBeLess` omitted, and as `Value::None`.
#[test]
fn link_9_s_first_lift_is_false_and_writes_nothing() {
    let mut x = fx();
    x.rows(&[(2, 7, 100)]);
    assert_eq!(x.clean(|x| x.unlock(2, 1, true, false)).unwrap(), Value::Bool(false));
    let pc = x.pc;
    let omitted = vec![Value::Byte(2), Value::Int(1), Value::Bool(true)];
    assert_eq!(x.clean(|x| x.call_on(pc, omitted)).unwrap(), Value::Bool(false), "bCanBeLess omitted");
    let none = vec![Value::Byte(2), Value::Int(1), Value::Bool(true), Value::None];
    assert_eq!(x.clean(|x| x.call_on(pc, none)).unwrap(), Value::Bool(false), "bCanBeLess as None");
}

/// Test 2: the same entry at a count of 100 and of 150: true, one event with the entry's achievement each.
#[test]
fn the_threshold_and_above_unlock_with_bcanbegreater() {
    let mut x = fx();
    x.rows(&[(2, 7, 100)]);
    assert_eq!(x.unlock(2, 100, true, false).unwrap(), Value::Bool(true));
    assert_eq!(x.calls(), vec![7]);
    assert_eq!(x.unlock(2, 150, true, false).unwrap(), Value::Bool(true));
    assert_eq!(x.calls(), vec![7, 7]);
}

/// Test 3: `bCanBeLess` with a count below the threshold is true; neither flag with another count is false, and the
/// exact count is true with neither flag.
#[test]
fn bcanbeless_and_neither_flag() {
    let mut x = fx();
    x.rows(&[(2, 7, 100)]);
    assert_eq!(x.unlock(2, 40, false, true).unwrap(), Value::Bool(true));
    assert_eq!(x.clean(|x| x.unlock(2, 40, false, false)).unwrap(), Value::Bool(false));
    assert_eq!(x.clean(|x| x.unlock(2, 150, false, true)).unwrap(), Value::Bool(false), "above, with only bCanBeLess");
    assert_eq!(x.unlock(2, 100, false, false).unwrap(), Value::Bool(true));
    assert_eq!(x.calls(), vec![7, 7]);
}

/// Test 4: entries passed over: an `Achievement` of 0, another type, a threshold of 0 and one below 0.
#[test]
fn entries_passed_over() {
    let mut x = fx();
    x.rows(&[(2, 0, 1), (3, 8, 1), (2, 9, 0), (2, 10, -5)]);
    for (greater, less) in [(true, false), (false, true), (true, true)] {
        assert_eq!(x.clean(|x| x.unlock(2, 1, greater, less)).unwrap(), Value::Bool(false));
        assert_eq!(x.clean(|x| x.unlock(2, 0, greater, less)).unwrap(), Value::Bool(false));
    }
}

/// Test 5: two selected entries (and a duplicate): their events in the array's order, then true.
#[test]
fn selected_entries_follow_the_array_order() {
    let mut x = fx();
    x.rows(&[(2, 30, 50), (1, 31, 1), (2, 32, 10), (2, 30, 20), (2, 33, 99)]);
    assert_eq!(x.unlock(2, 50, true, false).unwrap(), Value::Bool(true));
    assert_eq!(x.calls(), vec![30, 32, 30]);
    let ids = class_achievements_to_unlock(
        &class_achievement_data(&x.vm, x.definition).unwrap(),
        &ClassConditions { unlock_type: 2, int: 50, can_be_greater: true, can_be_less: false },
    );
    assert_eq!(ids, vec![30, 32, 30], "the shared selection gives the same ids");
}

/// Test 6: the class field None, and an object of another class, answer false with nothing written; a child of the
/// class definition is read.
#[test]
fn no_class_or_another_class_is_false() {
    let mut x = fx();
    x.rows(&[(2, 7, 1)]);
    let pc = x.pc;
    x.set(pc, "PlayerClass", Value::Object(None));
    assert_eq!(x.clean(|x| x.unlock(2, 1, true, false)).unwrap(), Value::Bool(false));
    let other = make(&mut x.vm, "WillowGame.ToyOther");
    x.set(pc, "PlayerClass", Value::Object(Some(other)));
    assert_eq!(x.clean(|x| x.unlock(2, 1, true, false)).unwrap(), Value::Bool(false));
    let child = make(&mut x.vm, "WillowGame.ToyChildClassDefinition");
    let ty = x.declared_struct();
    x.put_rows_of(child, ty, &[(2, 8, 1)]);
    x.set(pc, "PlayerClass", Value::Object(Some(child)));
    assert_eq!(x.unlock(2, 1, true, false).unwrap(), Value::Bool(true));
    assert_eq!(x.calls(), vec![8]);
}

/// Test 7: an unloaded receiver, and an unloaded player class: their `NotRead` refusals, unmarked, with nothing
/// loaded or written.
#[test]
fn an_unloaded_receiver_or_class_is_not_read() {
    let mut x = fx();
    x.rows(&[(2, 7, 1)]);
    let controller = x.held("Controller");
    let n = not_yet(x.clean(|x| {
        x.call_on(controller, vec![Value::Byte(2), Value::Int(1), Value::Bool(true), Value::Bool(false)])
    }));
    let path = ow_attr::eval::object_path(&x.vm.space, controller);
    assert_eq!((n.missing, &*n.what, n.record), (Missing::NotRead, &*receiver_not_loaded(&path), RECORD));
    assert!(x.vm.defs.is_unloaded(&x.vm.space, controller), "not loaded by the native");
    let definition = x.held("Definition");
    let pc = x.pc;
    x.set(pc, "PlayerClass", Value::Object(Some(definition)));
    let n = not_yet(x.clean(|x| x.unlock(2, 1, true, false)));
    let path = ow_attr::eval::object_path(&x.vm.space, definition);
    assert_eq!((n.missing, &*n.what, n.record), (Missing::NotRead, &*class_not_loaded(&path), RECORD));
    assert!(x.vm.defs.is_unloaded(&x.vm.space, definition), "not loaded by the native");
    // Review r1 of #668, item 1: an unloaded handle of another class answers false without a refusal (the class test
    // needs only the handle's class) and stays unloaded.
    let other = x.held("Other");
    x.set(pc, "PlayerClass", Value::Object(Some(other)));
    assert_eq!(x.clean(|x| x.unlock(2, 1, true, false)).unwrap(), Value::Bool(false));
    assert!(x.vm.defs.is_unloaded(&x.vm.space, other), "not loaded by the native");
}

/// Test 8: a malformed array (an entry field of another kind; an entry of a lookalike struct) or argument is a
/// `VmError::Script`, with nothing changed.
#[test]
fn a_malformed_array_or_argument_is_a_script_error() {
    let mut m = fx_with(Setup { mistyped: true, ..OFF });
    script(m.clean(|x| x.unlock(2, 1, true, false)), "IntData");
    let mut x = fx();
    let look = x.vm.space.classes.struct_by_path("WillowGame.ToyOther.PlayerClassAchievementUnlockData").unwrap();
    let d = x.definition;
    x.put_rows_of(d, look, &[(2, 7, 1)]);
    script(x.clean(|x| x.unlock(2, 1, true, false)), "not its declared struct");
    x.rows(&[(2, 7, 1)]);
    let pc = x.pc;
    for (i, arg, name) in [(0, Value::Int(2), "AchievementUnlockType"), (1, Value::Byte(1), "IntData"), (3, Value::Int(1), "bCanBeLess")]
    {
        let mut args = vec![Value::Byte(2), Value::Int(1), Value::Bool(true), Value::Bool(false)];
        args[i] = arg;
        script(x.clean(|x| x.call_on(pc, args)), &format!("{name} is "));
    }
}

/// Test 9: the event lookup, before any event: a receiver whose class code is not loaded is `NotRead` with nothing
/// loaded; resident code with no `ClientUnlockAchievement` is its own script error.
#[test]
fn the_event_lookup_is_write_free() {
    let mut x = fx();
    x.rows(&[(2, 7, 1)]);
    let lazy_class = {
        let vm = &mut x.vm;
        let (code, store) = vm.defs.code_mut();
        let key = code.find_class(store, "WillowGame.ToyLazyController").unwrap();
        code.define_layout(store, &mut vm.space.names, &mut vm.space.classes, key).unwrap()
    };
    let lazy = x.vm.space.alloc(lazy_class, "Lazy_0", None, Origin::Runtime).unwrap();
    let definition = x.definition;
    x.set(lazy, "PlayerClass", Value::Object(Some(definition)));
    let n = not_yet(x.clean(|x| {
        x.call_on(lazy, vec![Value::Byte(2), Value::Int(1), Value::Bool(true), Value::Bool(false)])
    }));
    assert_eq!(
        (n.missing, &*n.what, n.record),
        (Missing::NotRead, &*code_not_loaded("WillowGame.ToyLazyController"), RECORD)
    );
    let bare = make(&mut x.vm, "WillowGame.WillowPlayerController");
    x.set(bare, "PlayerClass", Value::Object(Some(definition)));
    script(
        x.clean(|x| x.call_on(bare, vec![Value::Byte(2), Value::Int(1), Value::Bool(true), Value::Bool(false)])),
        "declares no ClientUnlockAchievement",
    );
}

/// Test 10: an event that writes and then refuses is marked "stopped after partial writes"; one that refuses before
/// writing is not.
#[test]
fn partial_writes_are_marked_only_after_a_write() {
    let mut x = fx_with(Setup { client: Client::Refuse, ..OFF });
    x.rows(&[(2, 7, 1), (2, 8, 1)]);
    let n = not_yet(x.unlock(2, 1, true, false));
    assert_eq!(&*n.what, "an invented online subsystem");
    assert_eq!(x.calls(), vec![7], "the first refusal ends the native");
    let mut y = fx_with(Setup { client: Client::WriteAndRefuse, ..OFF });
    y.rows(&[(2, 7, 1), (2, 8, 1)]);
    let n = not_yet(y.unlock(2, 1, true, false));
    assert_eq!(&*n.what, "stopped after partial writes: an invented online subsystem");
    assert_eq!((n.missing, n.record), (Missing::NotRead, RECORD));
    assert_eq!(y.calls(), vec![7]);
    // Review r1 of #668, item 2: an earlier event that finished without writing leaves a later refusal unmarked; the
    // mark follows the counters, not the event count.
    let mut z = fx_with(Setup { client: Client::RefuseSecond, ..OFF });
    z.rows(&[(2, 7, 1), (2, 8, 1)]);
    let n = not_yet(z.unlock(2, 1, true, false));
    assert_eq!(&*n.what, "an invented online subsystem");
    assert_eq!(z.calls(), vec![7, 8]);
}

/// Test 11, this pack's half: keep-existing (a second registration, and the inventory registration after it, keep
/// the one binding), and under `ParityMode::Reference` nothing new is bound.
#[test]
fn registration_is_keep_existing_and_off_only() {
    let mut x = fx();
    x.rows(&[(2, 7, 1)]);
    register(&mut x.vm).unwrap();
    crate::globals::register(&mut x.vm).unwrap();
    assert_eq!(x.vm.package_bindings(KEY), 1);
    assert!(!x.vm.native_keys().any(|k| k == KEY));
    assert_eq!(x.unlock(2, 1, true, false).unwrap(), Value::Bool(true));
    // Review r1 of #668, item 3: a different handler bound first is kept by both registrations, with no
    // duplicate-registration error.
    let mut p = fx_with(Setup { prebound: true, ..OFF });
    p.rows(&[(2, 7, 100)]);
    register(&mut p.vm).unwrap();
    crate::globals::register(&mut p.vm).unwrap();
    assert_eq!(p.vm.package_bindings(KEY), 1);
    assert_eq!(p.unlock(2, 1, true, false).unwrap(), Value::Bool(true), "the stand-in answers, not the rule");
    assert!(p.calls().is_empty());
    let mut r = fx_with(Setup { mode: ParityMode::Reference, ..OFF });
    r.rows(&[(2, 7, 1)]);
    assert!(!r.vm.has_native(KEY));
    assert!(matches!(r.unlock(2, 1, true, false), Err(VmError::UnboundNative(_))));
}
