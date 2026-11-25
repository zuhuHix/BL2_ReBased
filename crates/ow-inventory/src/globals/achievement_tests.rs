//! Tests of `UnlockAchievementIfConditionsMet` (NATIVE_UNLOCK_ACHIEVEMENT_IF_CONDITIONS_MET.md; lane
//! IMPL-LINK12-UNLOCK-ACHIEVEMENT) on three invented packages built with `ow_package::testkit` (AGENTS.md rule 9).
//! `Engine` has `Object`; `GearboxFramework` has `GearboxGlobals` and its definition class; `WillowGame` has
//! `WillowGlobals`, `GlobalsDefinition` with `AchievementData` (of `AchievementUtility.AchievementUnlockData`),
//! `WillowPlayerController` with the native and a native stand-in for `ClientUnlockAchievement` the tests bind,
//! `ToyBadDefinition` with a wrong `AchievementData`, and a holder export whose definition is a store handle not yet
//! loaded. Every value is invented; every call goes through the VM.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::{Missing, NotYet, ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::layout::PropKind;

use ow_object::value::Value;
use ow_package::testkit::reflect::{toy_array_of, toy_struct};
use ow_package::testkit::script::{FunctionSpec, Param, ScriptToy};
use ow_vm::api::{NativeCall, StubPolicy, Vm};
use ow_vm::error::VmError;

use super::*;

const NATIVE: u32 = 0x400 | 0x20000;
const STATIC_NATIVE: u32 = 0x2000 | NATIVE;
static NEXT: AtomicU32 = AtomicU32::new(0);

const IMPORTS: [&str; 10] = [
    "Class",
    "Function",
    "ScriptStruct",
    "ObjectProperty",
    "IntProperty",
    "ByteProperty",
    "StrProperty",
    "StructProperty",
    "ArrayProperty",
    "BoolProperty",
];

const CLIENT_UNLOCK: &str = "WillowGame.WillowPlayerController.ClientUnlockAchievement";

fn native(t: &mut ScriptToy, owner: i32, name: &str, flags: u32, params: &[Param], result: Option<(&str, i32)>) {
    t.function(owner, name, params, result, &[], &FunctionSpec::new(flags, name), |_| None);
}

fn engine() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    t.class("Object", 0);
    t.build()
}

fn gearbox() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let object = t.p.add_import("Core", "Class", engine, "Object");
    let definition = t.class("GearboxGlobalsDefinition", object);
    let globals = t.class("GearboxGlobals", object);
    t.prop("Object", globals, "TheGlobalsDefinition", 0, definition);
    native(&mut t, globals, "GetGearboxGlobals", STATIC_NATIVE, &[], Some(("Object", globals)));
    t.build()
}

/// The `WillowGame` package; with `mistyped`, `AchievementUnlockData.IntData` is declared a string.
fn willow_game_with(mistyped: bool) -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let object = t.p.add_import("Core", "Class", engine, "Object");
    let gbx = t.p.add_import("Core", "Package", 0, "GearboxFramework");
    let gearbox_globals = t.p.add_import("Core", "Class", gbx, "GearboxGlobals");
    let gearbox_definition = t.p.add_import("Core", "Class", gbx, "GearboxGlobalsDefinition");
    t.class("WillowGlobals", gearbox_globals);
    let utility = t.class("AchievementUtility", object);
    let data = toy_struct(&mut t, utility, "AchievementUnlockData", None);
    t.prop("Byte", data, "UnlockType", 0, 0);
    t.prop("Byte", data, "Achievement", 0, 0);
    t.prop("Str", data, "StringData", 0, 0);
    t.prop(if mistyped { "Str" } else { "Int" }, data, "IntData", 0, 0);
    let gd = t.class("GlobalsDefinition", gearbox_definition);
    toy_array_of(&mut t, gd, "AchievementData", "Struct", data);
    // The same field names in a struct that is not the declared one, and a list of ints.
    let bad = t.class("ToyBadDefinition", gearbox_definition);
    toy_array_of(&mut t, bad, "AchievementData", "Int", 0);
    let controller = t.class("WillowPlayerController", object);
    t.prop("Int", controller, "ToyUnlocks", 0, 0);
    let params = [
        Param::new("Byte", "AchievementUnlockType"),
        Param::new("Str", "StringData"),
        Param::new("Int", "IntData"),
        Param::new("Bool", "bCanBeGreater"),
        Param::new("Bool", "bCheckStringAndIntData"),
        Param::new("Bool", "bCanBeLess"),
    ];
    native(&mut t, controller, "UnlockAchievementIfConditionsMet", NATIVE | 0x20, &params, Some(("Bool", 0)));
    native(&mut t, controller, "ClientUnlockAchievement", NATIVE, &[Param::new("Int", "AchievementId")], None);
    // A definition export the store leaves as a handle until it is loaded, held by a holder export.
    let holder = t.class("ToyHolder", object);
    t.prop("Object", holder, "Definition", 0, object);
    let mut empty = 0i32.to_le_bytes().to_vec();
    empty.extend_from_slice(&t.none);
    let stored = t.p.add_export(gd, "ToyStoredGlobals", empty);
    let mut h = 0i32.to_le_bytes().to_vec();
    h.extend(t.p.object_tag("Definition", stored));
    h.extend_from_slice(&t.none);
    t.p.add_export(holder, "ToyHolderObject", h);
    t.build()
}

struct Fx {
    vm: Vm,
    dir: PathBuf,
    globals: ObjRef,
    gd: ObjRef,
    pc: ObjRef,
    calls: Rc<RefCell<Vec<i32>>>,
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// What the test's `ClientUnlockAchievement` stand-in does on each call.
#[derive(Clone, Copy)]
enum Client {
    /// Records the id and writes nothing.
    Record,
    /// Records the id, writes the controller's `ToyUnlocks`, and refuses from the second call on.
    WriteThenRefuse,
    /// Refuses at once, writing nothing.
    Refuse,
}

/// How the fixture's world is set up.
#[derive(Clone, Copy)]
struct Setup {
    mode: ParityMode,
    client: Client,
    /// Whether the world records its globals object.
    globals: bool,
    /// Whether the native's key is already bound (to an invented answer, true) before the module registers.
    prebound: bool,
    /// Whether `AchievementUnlockData.IntData` is declared with the wrong type.
    mistyped: bool,
    /// COLD-CLASS-AUDIT: `GlobalsDefinition` left cold (its layout through the code index, its code not loaded, its
    /// object allocated without a lookup).
    cold_definition: bool,
}

const OFF: Setup =
    Setup { mode: ParityMode::Off, client: Client::Record, globals: true, prebound: false, mistyped: false, cold_definition: false };

fn fx_with(setup: Setup) -> Fx {
    let Setup { mode, client, .. } = setup;
    let dir = std::env::temp_dir().join(format!(
        "ow-inventory-achievement-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let (e, g, w) = (engine(), gearbox(), willow_game_with(setup.mistyped));
    let defs = DefStore::from_synthetic(
        &dir,
        &[("Engine.upk", e.as_slice()), ("GearboxFramework.upk", g.as_slice()), ("WillowGame.upk", w.as_slice())],
        IdentityPolicy::FirstInScanOrder,
        mode,
    )
    .unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), mode)
        .with_stub_policy(StubPolicy::refuse_all());
    let warm: &[&str] = if setup.cold_definition {
        &["WillowGame.WillowGlobals", "WillowGame.WillowPlayerController"]
    } else {
        &["WillowGame.WillowGlobals", "WillowGame.GlobalsDefinition", "WillowGame.WillowPlayerController"]
    };
    for c in warm {
        vm.find_class(c).unwrap();
    }
    let calls = Rc::new(RefCell::new(Vec::new()));
    let seen = calls.clone();
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
            match client {
                Client::Record => Ok(Value::None),
                Client::Refuse => Err(NotYet::not_read("an invented online subsystem", RECORD).into()),
                Client::WriteThenRefuse => {
                    if seen.borrow().len() > 1 {
                        return Err(NotYet::not_read("an invented online subsystem", RECORD).into());
                    }
                    let this = c.this.unwrap();
                    let class = c.vm.space.heap.get(this)?.class;
                    let p = c.vm.space.prop(class, "ToyUnlocks").unwrap();
                    c.vm.space.write(this, PropAddr::scalar(p), Value::Int(id))?;
                    Ok(Value::None)
                }
            }
        }),
    )
    .unwrap();
    let query = ow_world::query::install(&mut vm).unwrap();
    if setup.prebound {
        let f = vm.find_function(UNLOCK_IF_CONDITIONS_MET).unwrap();
        let key = vm.function(f).unwrap().native_key.clone().unwrap();
        vm.register_native(&key, Rc::new(|_: &mut NativeCall<'_>| Ok(Value::Bool(true)))).unwrap();
    }
    crate::globals::register(&mut vm).unwrap();
    let globals = make(&mut vm, "WillowGame.WillowGlobals");
    let gd = if setup.cold_definition {
        let (code, store) = vm.defs.code_mut();
        let key = code.find_class(store, "WillowGame.GlobalsDefinition").unwrap();
        let c = code.define_layout(store, &mut vm.space.names, &mut vm.space.classes, key).unwrap();
        assert!(!vm.class_code_loaded(c), "GlobalsDefinition starts cold");
        ow_vm::interp::instantiate_from_defaults(&mut vm, c).unwrap()
    } else {
        make(&mut vm, "WillowGame.GlobalsDefinition")
    };
    let pc = make(&mut vm, "WillowGame.WillowPlayerController");
    if setup.globals {
        query.borrow_mut().set_globals(globals);
    }
    let mut x = Fx { vm, dir, globals, gd, pc, calls };
    x.set(globals, "TheGlobalsDefinition", Value::Object(Some(gd)));
    x
}

fn fx() -> Fx {
    fx_with(OFF)
}

fn make(vm: &mut Vm, class: &str) -> ObjRef {
    let c = vm.find_class(class).unwrap();
    ow_vm::interp::instantiate_from_defaults(vm, c).unwrap()
}

/// An entry: (unlock type, achievement, string, int).
type Row<'a> = (u8, u8, &'a str, i32);

impl Fx {
    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap();
        self.vm.space.write(o, PropAddr::scalar(p), v).unwrap();
    }

    /// `AchievementData` set to `rows`, each field by name.
    fn set_rows(&mut self, rows: &[Row<'_>]) {
        let class = self.vm.space.heap.get(self.gd).unwrap().class;
        let p = self.vm.space.prop(class, "AchievementData").unwrap();
        let ty = match &self.vm.space.classes.decl(p).unwrap().kind {
            PropKind::Array(inner) => match **inner {
                PropKind::Struct(ty) => ty,
                ref k => panic!("{k:?}"),
            },
            k => panic!("{k:?}"),
        };
        let layout = self.vm.space.classes.struct_layout(ty).unwrap();
        let mut entries = Vec::new();
        for &(unlock, achievement, string, int) in rows {
            let mut fields = vec![Value::None; 4];
            for (name, v) in [
                ("UnlockType", Value::Byte(unlock)),
                ("Achievement", Value::Byte(achievement)),
                ("StringData", Value::string(string)),
                ("IntData", Value::Int(int)),
            ] {
                let (index, _) = layout.field(self.vm.space.names.find(name).unwrap()).unwrap();
                fields[usize::try_from(index).unwrap()] = v;
            }
            entries.push(Value::structure(ty, fields));
        }
        self.vm.space.write(self.gd, PropAddr::scalar(p), Value::array(entries)).unwrap();
    }

    /// The native on the controller with (type, string, int, bCanBeGreater, bCheckStringAndIntData, bCanBeLess).
    fn unlock(&mut self, unlock: u8, string: &str, int: i32, flags: [bool; 3]) -> Result<Value, VmError> {
        let f = self.vm.find_function(UNLOCK_IF_CONDITIONS_MET)?;
        let args = vec![
            Value::Byte(unlock),
            Value::string(string),
            Value::Int(int),
            Value::Bool(flags[0]),
            Value::Bool(flags[1]),
            Value::Bool(flags[2]),
        ];
        Ok(self.vm.call(f, Some(self.pc), args)?.ret)
    }

    fn calls(&self) -> Vec<i32> {
        self.calls.borrow().clone()
    }

    fn heap(&self) -> String {
        format!("{:?}", self.vm.space.heap)
    }

    fn counters(&self) -> (u64, u64, u64) {
        (self.vm.space.heap.committed_writes(), self.vm.rng.draws(), (self.vm.defs.load_transitions() + self.vm.class_code_loads()))
    }
}

const NONE: [bool; 3] = [false, false, false];
const GREATER: [bool; 3] = [true, false, false];
const LESS: [bool; 3] = [false, false, true];
const BOTH: [bool; 3] = [false, true, false];

fn not_yet(r: Result<Value, VmError>) -> NotYet {
    match r {
        Err(VmError::NotYet(n)) => n,
        other => panic!("expected NotYet, got {other:?}"),
    }
}

/// The slice's case: the one entry of type 16 wants 20, so a count of 1 with `bCanBeGreater` selects nothing; the
/// answer is false and nothing is written or called.
#[test]
fn no_match_is_false_and_writes_nothing() {
    let mut x = fx();
    x.set_rows(&[(16, 7, "", 20), (3, 8, "", 1)]);
    let (heap, counters) = (x.heap(), x.counters());
    assert_eq!(x.unlock(16, "", 1, GREATER).unwrap(), Value::Bool(false));
    assert_eq!(x.unlock(16, "", 19, GREATER).unwrap(), Value::Bool(false), "any count below 20");
    assert!(x.calls().is_empty());
    assert_eq!(x.heap(), heap);
    assert_eq!(x.counters(), counters, "no write, no draw, no load");
}

/// The turn-in's call shape: `StringData` arriving as None and the last two flags omitted read as the empty string
/// and false, so the type-16 entry wanting 20 is selected at 20 and not below.
#[test]
fn omitted_arguments_are_their_zero() {
    let mut x = fx();
    x.set_rows(&[(16, 7, "", 20)]);
    let f = x.vm.find_function(UNLOCK_IF_CONDITIONS_MET).unwrap();
    let pc = x.pc;
    let call = |vm: &mut Vm, int: i32| {
        vm.call(f, Some(pc), vec![Value::Byte(16), Value::None, Value::Int(int), Value::Bool(true)]).map(|r| r.ret)
    };
    assert_eq!(call(&mut x.vm, 1).unwrap(), Value::Bool(false));
    assert_eq!(call(&mut x.vm, 20).unwrap(), Value::Bool(true));
    assert_eq!(x.calls(), vec![7]);
}

/// The string match compares without case, and an empty entry string never matches.
#[test]
fn the_string_match_ignores_case_and_wants_a_string() {
    let mut x = fx();
    x.set_rows(&[(5, 11, "ToyLevel", 0), (5, 12, "", 0)]);
    assert_eq!(x.unlock(5, "toylevel", 0, NONE).unwrap(), Value::Bool(true));
    assert_eq!(x.calls(), vec![11], "the empty-string entry is not selected");
    assert_eq!(x.unlock(5, "OtherLevel", 0, NONE).unwrap(), Value::Bool(false));
    assert_eq!(x.unlock(5, "", 0, NONE).unwrap(), Value::Bool(false), "an empty call string matches no entry");
}

/// The int match: equal, above with `bCanBeGreater`, below with `bCanBeLess`, and an entry int below 1 never.
#[test]
fn the_int_match_follows_the_two_flags() {
    let mut x = fx();
    x.set_rows(&[(9, 21, "", 10), (9, 22, "", 0)]);
    assert_eq!(x.unlock(9, "", 10, NONE).unwrap(), Value::Bool(true));
    assert_eq!(x.unlock(9, "", 11, NONE).unwrap(), Value::Bool(false));
    assert_eq!(x.unlock(9, "", 11, GREATER).unwrap(), Value::Bool(true));
    assert_eq!(x.unlock(9, "", 9, GREATER).unwrap(), Value::Bool(false));
    assert_eq!(x.unlock(9, "", 9, LESS).unwrap(), Value::Bool(true));
    assert_eq!(x.unlock(9, "", 0, LESS).unwrap(), Value::Bool(true), "below 10; the 0 entry still never matches");
    assert_eq!(x.calls(), vec![21, 21, 21, 21]);
}

/// `bCheckStringAndIntData` wants both matches; without it either one is enough.
#[test]
fn check_string_and_int_wants_both() {
    let mut x = fx();
    x.set_rows(&[(4, 31, "ToyName", 3)]);
    assert_eq!(x.unlock(4, "toyname", 99, NONE).unwrap(), Value::Bool(true), "the string alone");
    assert_eq!(x.unlock(4, "Other", 3, NONE).unwrap(), Value::Bool(true), "the int alone");
    assert_eq!(x.unlock(4, "toyname", 99, BOTH).unwrap(), Value::Bool(false));
    assert_eq!(x.unlock(4, "Other", 3, BOTH).unwrap(), Value::Bool(false));
    assert_eq!(x.unlock(4, "TOYNAME", 3, BOTH).unwrap(), Value::Bool(true));
    assert_eq!(x.calls(), vec![31, 31, 31]);
}

/// An entry whose `Achievement` is 0 is never selected, and another type is not either.
#[test]
fn achievement_zero_and_another_type_are_skipped() {
    let mut x = fx();
    x.set_rows(&[(2, 0, "", 1), (3, 40, "", 1)]);
    assert_eq!(x.unlock(2, "", 1, NONE).unwrap(), Value::Bool(false));
    assert!(x.calls().is_empty());
}

/// Every selected entry gets one call, in entry order, then the answer is true.
#[test]
fn the_calls_follow_the_entry_order() {
    let mut x = fx();
    x.set_rows(&[(6, 52, "", 3), (7, 53, "", 3), (6, 51, "", 2), (6, 50, "", 5), (6, 49, "", 1)]);
    assert_eq!(x.unlock(6, "", 2, LESS).unwrap(), Value::Bool(true));
    assert_eq!(x.calls(), vec![52, 51, 50]);
    let ids = achievements_to_unlock(
        &achievement_data(&x.vm, x.gd).unwrap(),
        &Conditions { unlock_type: 6, int: 2, can_be_less: true, ..Conditions::default() },
    );
    assert_eq!(ids, vec![52, 51, 50], "the shared selection function gives the same ids");
}

/// No globals object recorded: the globals lookup's `NotYet { LayerAbsent }`, unmarked, nothing written or called
/// (review r1 of #546, item 1).
#[test]
fn no_globals_is_the_lookup_s_layer_absent() {
    let mut x = fx_with(Setup { globals: false, ..OFF });
    x.set_rows(&[(1, 9, "", 1)]);
    let (heap, counters) = (x.heap(), x.counters());
    let n = not_yet(x.unlock(1, "", 1, NONE));
    assert!(matches!(n.missing, Missing::LayerAbsent(_)), "{n:?}");
    assert!(!n.what.starts_with("stopped after partial writes"), "{}", n.what);
    assert!(x.calls().is_empty());
    assert_eq!((x.heap(), x.counters()), (heap, counters));
}

/// A `GlobalsDefinition` whose `AchievementUnlockData` declares a field with the wrong type: `VmError::Script` naming
/// the field, raised through the native before any value is read; no call, nothing written (review r1 of #546, item
/// 2).
#[test]
fn a_mistyped_unlock_data_field_is_a_data_error_through_the_native() {
    let mut x = fx_with(Setup { mistyped: true, ..OFF });
    let (heap, counters) = (x.heap(), x.counters());
    match x.unlock(1, "", 1, NONE) {
        Err(VmError::Script { message, .. }) => assert!(message.contains("AchievementUnlockData.IntData"), "{message}"),
        other => panic!("expected a data error, got {other:?}"),
    }
    assert!(x.calls().is_empty());
    assert_eq!((x.heap(), x.counters()), (heap, counters));
}

/// The definition field None: the existing helper's first-load refusal, unmarked and before any write; a handle the
/// store has not loaded: the helper's lazy-load refusal, the handle left unloaded and no load counted.
#[test]
fn the_definition_refusals_are_the_helper_s_and_unmarked() {
    let mut x = fx();
    x.set_rows(&[(1, 9, "", 1)]);
    let globals = x.globals;
    x.set(globals, "TheGlobalsDefinition", Value::Object(None));
    let counters = x.counters();
    let n = not_yet(x.unlock(1, "", 1, NONE));
    assert!(matches!(n.missing, Missing::NotRead), "{n:?}");
    assert!(n.what.contains("first load") && !n.what.starts_with("stopped after partial writes"), "{}", n.what);
    assert_eq!(x.counters(), counters);
    let path = ObjectPath::parse("WillowGame.ToyHolderObject").unwrap();
    let holder = x.vm.defs.find(&mut x.vm.space, &path).unwrap().unwrap();
    let stored = match crate::equip::read_prop(&x.vm, holder, "Definition") {
        Some(Value::Object(Some(o))) => o,
        other => panic!("{other:?}"),
    };
    assert!(x.vm.defs.is_unloaded(&x.vm.space, stored));
    x.set(globals, "TheGlobalsDefinition", Value::Object(Some(stored)));
    let counters = x.counters();
    let n = not_yet(x.unlock(1, "", 1, NONE));
    assert!(n.what.contains("lazy load") && !n.what.starts_with("stopped after partial writes"), "{}", n.what);
    assert!(x.vm.defs.is_unloaded(&x.vm.space, stored), "not loaded by the native");
    assert_eq!(x.counters(), counters, "no load counted");
    assert!(x.calls().is_empty());
}

/// A refusal of `ClientUnlockAchievement` before any write is unmarked; one after an earlier call's write is marked
/// "stopped after partial writes: ", its kind and record kept.
#[test]
fn a_refusal_is_marked_only_after_a_write() {
    let mut x = fx_with(Setup { client: Client::Refuse, ..OFF });
    x.set_rows(&[(1, 9, "", 1), (1, 10, "", 1)]);
    let n = not_yet(x.unlock(1, "", 1, NONE));
    assert_eq!(n.what, "an invented online subsystem", "nothing was written first");
    assert_eq!(x.calls(), vec![9], "the first refusal ends the native");
    let mut y = fx_with(Setup { client: Client::WriteThenRefuse, ..OFF });
    y.set_rows(&[(1, 9, "", 1), (1, 10, "", 1)]);
    let n = not_yet(y.unlock(1, "", 1, NONE));
    assert_eq!(n.what, "stopped after partial writes: an invented online subsystem");
    assert!(matches!(n.missing, Missing::NotRead) && n.record == RECORD, "{n:?}");
    assert_eq!(y.calls(), vec![9, 10]);
}

/// `AchievementData` that is not an array of the declared struct is a data error naming the field, raised before any
/// value is read.
#[test]
fn a_wrong_achievement_data_is_a_data_error() {
    let mut x = fx();
    let bad = make(&mut x.vm, "WillowGame.ToyBadDefinition");
    let globals = x.globals;
    x.set(globals, "TheGlobalsDefinition", Value::Object(Some(bad)));
    match x.unlock(1, "", 1, NONE) {
        Err(VmError::NotYet(n)) => assert!(n.what.contains("first load"), "not a GlobalsDefinition: {n:?}"),
        other => panic!("{other:?}"),
    }
    // A GlobalsDefinition whose rows are right but whose declaration is checked: the Int list is refused by the
    // schema check directly.
    match achievement_data(&x.vm, bad) {
        Err(VmError::Script { message, .. }) => assert!(message.contains("AchievementData"), "{message}"),
        other => panic!("{other:?}"),
    }
}

/// Reference binds nothing, and a key already bound is kept (first-wins).
#[test]
fn reference_binds_nothing_and_a_bound_key_is_kept() {
    let mut r = fx_with(Setup { mode: ParityMode::Reference, ..OFF });
    r.set_rows(&[(1, 9, "", 1)]);
    assert!(matches!(r.unlock(1, "", 1, NONE), Err(VmError::UnboundNative(_))));
    let mut x = fx_with(Setup { prebound: true, ..OFF });
    x.set_rows(&[(1, 0, "", 1)]);
    assert_eq!(x.unlock(1, "", 1, NONE).unwrap(), Value::Bool(true), "the earlier binding answers");
    assert!(x.calls().is_empty());
}

/// COLD-CLASS-AUDIT (lane COLD-CLASS-AUDIT-WORLD-INVENTORY): the read-only "no match" answer with the globals
/// definition's class cold. Ignored: two code loads happen before the answer false; by reading, they are step 1's
/// `globals_definition`, whose `find_class` of `GearboxGlobalsDefinition` and `GlobalsDefinition` loads their cold
/// code.
#[test]
#[ignore = "finding: UnlockAchievementIfConditionsMet's globals_definition find_class loads cold class code on its read-only path (lane COLD-CLASS-AUDIT-WORLD-INVENTORY)"]
fn no_match_with_a_cold_globals_definition_loads_no_code() {
    let mut x = fx_with(Setup { cold_definition: true, ..OFF });
    x.set_rows(&[(16, 7, "", 20)]);
    let gd_class = x.vm.space.heap.get(x.gd).unwrap().class;
    assert!(!x.vm.class_code_loaded(gd_class), "the globals definition's class is still cold before the call");
    let before = x.vm.class_code_loads();
    assert_eq!(x.unlock(16, "", 1, GREATER).unwrap(), Value::Bool(false));
    assert_eq!(x.vm.class_code_loads(), before, "no class code loaded on the read-only path");
}
