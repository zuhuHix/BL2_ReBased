//! Tests of `InventoryActors` (NATIVE_INVENTORY_ACTORS.md section 4; lane IMPL-INVENTORY-ACTORS). Every value is
//! invented (AGENTS.md rule 9). The walk tests run on the invented world of `crate::equip::tests`. The loop-body
//! tests (9, 9a, 4's out variable, 10) run assembled script loops through the VM over an invented `Engine.upk`.

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::notyet::Missing;
use ow_core::{ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{FuncId, ObjRef, PropAddr};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_package::testkit::script::{Asm, CPF_OUT, FunctionSpec, Param, ScriptToy};
use ow_vm::api::{Arg, FunctionDecl, NativeCall, StubPolicy, Vm};
use ow_vm::error::VmError;

use super::*;
use crate::equip::tests::World;

const KEY: &str = "InventoryManager.InventoryActors(class,object,bool)";

/// `InventoryActors(class BaseClass, out Inventory Inv, optional bool bOnlyReadied)` declared on
/// `Engine.InventoryManager`, then the iterators bound.
fn declare(w: &mut World) -> FuncId {
    let object = w.class("Core.Object");
    let owner = w.class("Engine.InventoryManager");
    let names = &mut w.vm.space.names;
    let base = PropertyDecl::new(names.intern("BaseClass"), PropKind::Class { meta: object }).with_flags(PropertyDecl::PARAM);
    let inv = PropertyDecl::new(names.intern("Inv"), PropKind::Object { class: object })
        .with_flags(PropertyDecl::PARAM | PropertyDecl::OUT);
    let readied = PropertyDecl::new(names.intern("bOnlyReadied"), PropKind::Bool)
        .with_flags(PropertyDecl::PARAM | PropertyDecl::OPTIONAL);
    let f = w
        .vm
        .declare_function(
            FunctionDecl::native(ObjectPath::parse(INVENTORY_ACTORS).unwrap(), KEY)
                .with_owner(owner)
                .with_params(vec![base, inv, readied]),
        )
        .unwrap();
    crate::manager::register_iterators(&mut w.vm).unwrap();
    f
}

/// The entries the bound iterator gives on `this`.
fn iterate(w: &mut World, f: FuncId, this: ObjRef, base: Value, readied: Option<bool>) -> Result<Vec<ObjRef>, VmError> {
    let it = w.vm.iterator_for(f).expect("InventoryActors is bound");
    let mut args = vec![Arg::new(base, false, true), Arg::new(Value::None, true, true)];
    if let Some(b) = readied {
        args.push(Arg::new(Value::Bool(b), false, true));
    }
    let mut call = NativeCall::new(&mut w.vm, f, Some(this), args);
    let out = it(&mut call)?;
    assert_eq!(out.out_param, 1, "the out argument is Inv");
    Ok(out
        .items
        .into_iter()
        .map(|v| match v {
            Value::Object(Some(o)) => o,
            other => panic!("{other:?}"),
        })
        .collect())
}

fn chain(w: &mut World, entries: &[ObjRef]) {
    let m = w.manager;
    w.set(m, "InventoryChain", Value::Object(entries.first().copied()));
    for pair in entries.windows(2) {
        w.set(pair[0], "Inventory", Value::Object(Some(pair[1])));
    }
}

fn class_value(w: &World, path: &str) -> Value {
    Value::Class(Some(w.class(path)))
}

/// #518's three counters, the class-code loads and the name table.
fn marks(vm: &Vm) -> (u64, u64, u64, u64, usize) {
    (vm.space.heap.committed_writes(), vm.rng.draws(), vm.defs.load_transitions(), vm.class_code_loads(), vm.space.names.len())
}

/// A (a `WillowWeapon`), B (an `Engine.Inventory` that is not a `WillowInventory`), C (another `WillowWeapon`).
fn abc(w: &mut World) -> [ObjRef; 3] {
    let a = w.obj("WillowGame.WillowWeapon", "A");
    let b = w.obj("Engine.Inventory", "B");
    let c = w.obj("WillowGame.WillowWeapon", "C");
    chain(w, &[a, b, c]);
    [a, b, c]
}

fn script_message(e: VmError) -> String {
    match e {
        VmError::Script { message, .. } => message,
        other => panic!("expected a script error, got {other:?}"),
    }
}

/// Tests 1 and 2: the class filter in chain order; with `bOnlyReadied` only the readied entry. B, of another class,
/// has no `bReadied`, and it is not read (the flag reads it only on a matching entry).
#[test]
fn the_weapon_chain_is_walked_in_order_with_the_class_and_readied_filters() {
    let mut w = World::new();
    let f = declare(&mut w);
    let [a, _, c] = abc(&mut w);
    let m = w.manager;
    let base = class_value(&w, "Engine.WillowInventory");
    assert_eq!(iterate(&mut w, f, m, base.clone(), None).unwrap(), vec![a, c], "the flag omitted is false");
    assert_eq!(iterate(&mut w, f, m, base.clone(), Some(false)).unwrap(), vec![a, c]);
    w.set(c, "bReadied", Value::Bool(true));
    let before = marks(&w.vm);
    assert_eq!(iterate(&mut w, f, m, base, Some(true)).unwrap(), vec![c]);
    assert_eq!(marks(&w.vm), before, "a read");
    // The item chain is not walked.
    let item = w.obj("WillowGame.WillowItem", "OnTheItemChain");
    w.set(m, "ItemChain", Value::Object(Some(item)));
    let base = class_value(&w, "WillowGame.WillowItem");
    assert!(iterate(&mut w, f, m, base, None).unwrap().is_empty());
}

/// Test 3: a None `BaseClass`, in each of the VM's null forms, is `Engine.Inventory`: A, B and C, found load-free.
/// With `Engine.Inventory` not laid out: the fallback's `NotRead`, and still no class code loaded.
#[test]
fn a_none_base_class_is_engine_inventory_found_without_a_load() {
    let mut w = World::new();
    let f = declare(&mut w);
    let abc = abc(&mut w);
    let m = w.manager;
    let before = marks(&w.vm);
    for none in [Value::Class(None), Value::Object(None), Value::Interface(None), Value::None] {
        assert_eq!(iterate(&mut w, f, m, none, None).unwrap(), abc.to_vec());
    }
    assert_eq!(marks(&w.vm), before);
    let e = iterate(&mut w, f, m, Value::Int(3), None).unwrap_err();
    assert!(script_message(e).contains("not a class"));

    // A space where Engine.Inventory is not laid out.
    let mut space = ObjectSpace::new();
    let object = space.classes.define("Core.Object", None, vec![]).unwrap();
    let chain = PropertyDecl::new(space.names.intern("InventoryChain"), PropKind::Object { class: object });
    let manager_class = space.classes.define("Engine.InventoryManager", Some(object), vec![chain]).unwrap();
    let manager = space.alloc(manager_class, "Manager_0", None, Origin::Runtime).unwrap();
    let vm = Vm::new(space, DefStore::new_empty(), Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
    let before = marks(&vm);
    match inventory_actors(&vm, manager, None, false) {
        Err(VmError::NotYet(n)) => assert_eq!((n.missing, &*n.what, n.record), (Missing::NotRead, NO_FALLBACK, RECORD)),
        other => panic!("{other:?}"),
    }
    assert_eq!(marks(&vm), before);
}

/// Test 4 (the list side; the out variable is the script test below): an empty chain gives nothing.
#[test]
fn an_empty_chain_gives_nothing() {
    let mut w = World::new();
    let f = declare(&mut w);
    let m = w.manager;
    assert!(iterate(&mut w, f, m, Value::Class(None), None).unwrap().is_empty());
}

/// Test 5: the cap counts every entry examined, given or skipped. 150 matching entries: the first 100. 99 skipped
/// entries then 5 matching: only the first matching one, the 100th examined. The 101st is not tested: a lazy handle
/// there is no refusal.
#[test]
fn the_walk_examines_one_hundred_entries_at_most() {
    let mut w = World::new();
    let f = declare(&mut w);
    let entries: Vec<ObjRef> = (0..150).map(|i| w.obj("WillowGame.WillowWeapon", &format!("W{i}"))).collect();
    chain(&mut w, &entries);
    let m = w.manager;
    let base = class_value(&w, "Engine.WillowInventory");
    assert_eq!(iterate(&mut w, f, m, base, None).unwrap(), entries[..EXAMINE_CAP].to_vec());

    let mut w = World::new();
    let f = declare(&mut w);
    let mut entries: Vec<ObjRef> = (0..99).map(|i| w.obj("Engine.Inventory", &format!("Skip{i}"))).collect();
    let matching: Vec<ObjRef> = (0..5).map(|i| w.obj("WillowGame.WillowWeapon", &format!("W{i}"))).collect();
    entries.extend(&matching);
    chain(&mut w, &entries);
    let m = w.manager;
    let base = class_value(&w, "Engine.WillowInventory");
    assert_eq!(iterate(&mut w, f, m, base.clone(), None).unwrap(), vec![matching[0]]);
    let class = w.class("WillowGame.WillowWeapon");
    let lazy = w.vm.space.alloc_stand_in(class, "Lazy", Origin::Resource { package: 0, index: 1 }).unwrap();
    w.set(matching[0], "Inventory", Value::Object(Some(lazy)));
    assert_eq!(iterate(&mut w, f, m, base, None).unwrap(), vec![matching[0]], "the 101st entry is not tested");
}

/// Test 6: a looping chain (A -> B -> A ...) is not refused: entries repeat until 100 have been examined.
#[test]
fn a_looping_chain_repeats_up_to_the_cap() {
    let mut w = World::new();
    let f = declare(&mut w);
    let a = w.obj("WillowGame.WillowWeapon", "A");
    let b = w.obj("Engine.Inventory", "B");
    chain(&mut w, &[a, b]);
    w.set(b, "Inventory", Value::Object(Some(a)));
    let m = w.manager;
    let base = class_value(&w, "Engine.Inventory");
    let got = iterate(&mut w, f, m, base, None).unwrap();
    assert_eq!(got.len(), EXAMINE_CAP);
    assert!(got.chunks(2).all(|p| p == [a, b]), "A, B, A, B, ...");
}

/// Test 7: `InventoryChain` or an entry's `Inventory` declared as something other than an object reference, a
/// missing `Inventory`, a matching entry with no `bReadied` under the flag, or a receiver that is not an inventory
/// manager: `VmError::Script`.
#[test]
fn a_chain_of_the_wrong_kind_is_a_script_error() {
    let mut w = World::new();
    let f = declare(&mut w);
    let base = class_value(&w, "Engine.Inventory");
    let manager_class = w.class("Engine.InventoryManager");
    let odd = PropertyDecl::new(w.vm.space.names.intern("InventoryChain"), PropKind::Int);
    let odd_manager = w.vm.space.classes.define("Test.OddManager", Some(manager_class), vec![odd]).unwrap();
    let m = w.vm.space.alloc(odd_manager, "Odd_0", None, Origin::Runtime).unwrap();
    let e = iterate(&mut w, f, m, base.clone(), None).unwrap_err();
    assert!(script_message(e).contains("InventoryChain"));

    let inventory = w.class("Engine.Inventory");
    let odd = PropertyDecl::new(w.vm.space.names.intern("Inventory"), PropKind::Int);
    let odd_inventory = w.vm.space.classes.define("Test.OddInventory", Some(inventory), vec![odd]).unwrap();
    let entry = w.vm.space.alloc(odd_inventory, "OddEntry", None, Origin::Runtime).unwrap();
    chain(&mut w, &[entry]);
    let m = w.manager;
    let e = iterate(&mut w, f, m, base.clone(), None).unwrap_err();
    assert!(script_message(e).contains("Inventory"));

    let object = w.class("Core.Object");
    let stray_class = w.vm.space.classes.define("Test.Stray", Some(object), vec![]).unwrap();
    let stray = w.vm.space.alloc(stray_class, "Stray", None, Origin::Runtime).unwrap();
    chain(&mut w, &[stray]);
    let e = iterate(&mut w, f, m, Value::Class(Some(object)), None).unwrap_err();
    assert!(script_message(e).contains("declares no Inventory"));

    let b = w.obj("Engine.Inventory", "B");
    chain(&mut w, &[b]);
    let e = iterate(&mut w, f, m, base.clone(), Some(true)).unwrap_err();
    assert!(script_message(e).contains("declares no bReadied"));

    let pawn = w.pawn;
    let e = iterate(&mut w, f, pawn, base, None).unwrap_err();
    assert!(script_message(e).contains("is not an Engine.InventoryManager"));
}

/// Test 8: a lazy entry handle is the not-loaded `NotRead`, before any field of it is read (its class does not
/// match here, and it is still refused), nothing loaded, and no class code loaded.
#[test]
fn a_lazy_entry_is_not_read_before_its_class_is_tested() {
    let mut w = World::new();
    let f = declare(&mut w);
    let a = w.obj("WillowGame.WillowWeapon", "A");
    let class = w.class("WillowGame.WillowWeapon");
    let lazy = w.vm.space.alloc_stand_in(class, "Lazy", Origin::Resource { package: 0, index: 1 }).unwrap();
    chain(&mut w, &[a, lazy]);
    let m = w.manager;
    let base = class_value(&w, "WillowGame.WillowItem");
    let before = marks(&w.vm);
    match iterate(&mut w, f, m, base, None) {
        Err(VmError::NotYet(n)) => assert_eq!((n.missing, &*n.what, n.record), (Missing::NotRead, NOT_LOADED, RECORD)),
        other => panic!("{other:?}"),
    }
    assert_eq!(marks(&w.vm), before);
}

/// Test 11: Reference binds nothing.
#[test]
fn reference_leaves_inventory_actors_unbound() {
    let mut w = World::new();
    w.vm.parity = ParityMode::Reference;
    let f = declare(&mut w);
    assert!(w.vm.iterator_for(f).is_none());
}

// ---- script loops through the VM (an invented Engine.upk) ------------------------------------------------------------

const NATIVE_ITERATOR: u32 = 0x400 | 0x20000 | 0x4;
const SCRIPT: u32 = 0x1 | 0x2 | 0x20000;
const VIRTUAL: u32 = 0x2 | 0x20000;
static NEXT: AtomicU32 = AtomicU32::new(0);

fn le(words: &[i32]) -> Vec<u8> {
    words.iter().flat_map(|w| w.to_le_bytes()).collect()
}

/// `Inventory` (with `Inventory`, `bReadied` and `Deny()`, false), `ToyWeapon` below it, `ToyDenier` below it whose
/// `Deny()` is true, and `InventoryManager` with the chain, the native iterator and two script functions:
/// - `Walk()`: `foreach InventoryActors(InvClass, Cursor, false) { Cursor.bReadied = true; Cursor.Inventory = Target; }
///   return Cursor;`
/// - `Query()`: `foreach InventoryActors(InvClass, Cursor, false) { if (Cursor.Deny()) return false; } return true;`,
///   the shape of `WillowInventoryManager.HandlePickupQuery`'s weapon loop.
fn package() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class", "Function", "BoolProperty", "ObjectProperty", "ClassProperty"]);
    let deny = t.p.fname("Deny", 0);
    let object = t.class("Object", 0);
    let inventory = t.class("Inventory", object);
    let link = t.prop("Object", inventory, "Inventory", 0, inventory);
    let readied = t.prop("Bool", inventory, "bReadied", 0, 0);
    let returns = |value: u8| {
        move |_: &_| {
            let mut a = Asm::new();
            a.stmt().raw(&[0x04, value]);
            a.end();
            Some(a)
        }
    };
    t.function(inventory, "Deny", &[], Some(("Bool", 0)), &[], &FunctionSpec::new(VIRTUAL, "Deny"), returns(0x28));
    t.class("ToyWeapon", inventory);
    let denier = t.class("ToyDenier", inventory);
    t.function(denier, "Deny", &[], Some(("Bool", 0)), &[], &FunctionSpec::new(VIRTUAL, "Deny"), returns(0x27));
    let manager = t.class("InventoryManager", object);
    t.prop("Object", manager, "InventoryChain", 0, inventory);
    let cursor = t.prop("Object", manager, "Cursor", 0, inventory);
    let target = t.prop("Object", manager, "Target", 0, inventory);
    let class_import = t.import("Class");
    let inv_class = t.prop_tail("Class", manager, "InvClass", 0, &le(&[class_import, inventory]));
    let actors = t
        .function(
            manager,
            "InventoryActors",
            &[
                Param { type_ref: inventory, ..Param::new("Object", "BaseClass") },
                Param { flags: CPF_OUT, type_ref: inventory, ..Param::new("Object", "Inv") },
                Param::new("Bool", "bOnlyReadied"),
            ],
            None,
            &[],
            &FunctionSpec::new(NATIVE_ITERATOR, "InventoryActors"),
            |_| None,
        )
        .function;
    let foreach = |a: &mut Asm| {
        a.stmt().raw(&[0x2F]).raw(&[0x1C]).reference(actors);
        a.instance(inv_class).instance(cursor).false_().raw(&[0x16]);
        let end = a.bytes.len();
        a.w(0);
        end
    };
    t.function(manager, "Walk", &[], Some(("Object", inventory)), &[], &FunctionSpec::new(SCRIPT, "Walk"), |_| {
        let mut a = Asm::new();
        let end = foreach(&mut a);
        a.stmt().raw(&[0x14]).context(|a| { a.instance(cursor); }, |a| { a.instance(readied); }).raw(&[0x27]);
        a.stmt().raw(&[0x0F]).context(|a| { a.instance(cursor); }, |a| { a.instance(link); }).instance(target);
        a.stmt().raw(&[0x31]);
        a.patch(end);
        a.stmt().raw(&[0x30]);
        a.stmt().raw(&[0x04]).instance(cursor);
        a.end();
        Some(a)
    });
    t.function(manager, "Query", &[], Some(("Bool", 0)), &[], &FunctionSpec::new(SCRIPT, "Query"), |_| {
        let mut a = Asm::new();
        let end = foreach(&mut a);
        a.stmt();
        let next = a.jump_if_not(|a| {
            a.context(|a| { a.instance(cursor); }, |a| { a.raw(&[0x1B]).raw(&deny).raw(&[0x16]); });
        });
        a.stmt().raw(&[0x30]);
        a.stmt().raw(&[0x04, 0x28]);
        a.patch(next);
        a.stmt().raw(&[0x31]);
        a.patch(end);
        a.stmt().raw(&[0x30]);
        a.stmt().raw(&[0x04, 0x27]);
        a.end();
        Some(a)
    });
    t.p.build()
}

struct Fx {
    vm: Vm,
    manager: ObjRef,
    dir: PathBuf,
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn fixture() -> Fx {
    let dir = std::env::temp_dir().join(format!(
        "ow-inventory-inventoryactors-script-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let bytes = package();
    let defs = DefStore::from_synthetic(&dir, &[("Engine.upk", &bytes)], IdentityPolicy::FirstInScanOrder, ParityMode::Off)
        .unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off)
        .with_stub_policy(StubPolicy::refuse_all());
    let class = vm.find_class("Engine.InventoryManager").unwrap();
    let manager = ow_vm::interp::instantiate_from_defaults(&mut vm, class).unwrap();
    crate::manager::register_iterators(&mut vm).unwrap();
    let mut x = Fx { vm, manager, dir };
    let inventory = x.vm.find_class("Engine.Inventory").unwrap();
    x.set(manager, "InvClass", Value::Class(Some(inventory)));
    x
}

impl Fx {
    fn obj(&mut self, class: &str, name: &str) -> ObjRef {
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

    fn chain(&mut self, entries: &[ObjRef]) {
        let m = self.manager;
        self.set(m, "InventoryChain", Value::Object(entries.first().copied()));
        for pair in entries.windows(2) {
            self.set(pair[0], "Inventory", Value::Object(Some(pair[1])));
        }
    }

    fn call(&mut self, name: &str) -> Result<Value, VmError> {
        let f = self.vm.find_function(&format!("Engine.InventoryManager.{name}"))?;
        let m = self.manager;
        Ok(self.vm.call(f, Some(m), vec![])?.ret)
    }

    fn visited(&self, entries: &[ObjRef]) -> Vec<bool> {
        entries.iter().map(|&o| self.get(o, "bReadied") == Value::Bool(true)).collect()
    }
}

/// Test 4 (the out variable) and test 9: a body that unlinks the current entry (`Cursor.Inventory = None`) still
/// visits the rest; the out variable is None after the loop, and after a loop over an empty chain.
#[test]
fn a_body_that_unlinks_the_current_entry_still_visits_the_rest() {
    let mut x = fixture();
    assert_eq!(x.call("Walk").unwrap(), Value::Object(None), "an empty chain");
    let abc = ["A", "B", "C"].map(|n| x.obj("ToyWeapon", n));
    x.chain(&abc);
    assert_eq!(x.call("Walk").unwrap(), Value::Object(None), "the out variable after the loop");
    assert_eq!(x.visited(&abc), [true, true, true]);
    assert!(abc.iter().all(|&o| x.get(o, "Inventory") == Value::Object(None)), "every link cut by the body");
}

/// Test 9a, **the port's model** (the snapshot, UNVERIFIED): with chain A, B, C, the body on A sets A's `Inventory`
/// to C. B is still visited, because the list was taken when the loop started (in the game B would not be). It is
/// skipped only when B has been destroyed, since `IteratorNext` skips destroyed objects.
#[test]
fn a_body_that_unlinks_a_later_entry_still_visits_it_in_the_port() {
    for destroyed in [false, true] {
        let mut x = fixture();
        let [a, b, c] = ["A", "B", "C"].map(|n| x.obj("ToyWeapon", n));
        x.chain(&[a, b, c]);
        let m = x.manager;
        x.set(m, "Target", Value::Object(Some(c)));
        if destroyed {
            x.vm.space.heap.destroy(b).unwrap();
        }
        x.call("Walk").unwrap();
        assert_eq!(x.get(a, "Inventory"), Value::Object(Some(c)), "A now links to C");
        assert_eq!(x.visited(&[a, b, c]), [true, !destroyed, true], "destroyed {destroyed}");
    }
}

/// Test 10, the shape of `WillowInventoryManager.HandlePickupQuery`'s weapon loop: a chain whose weapon's
/// `Deny()` is false does not deny, and the call goes on past the loop (true). A denying entry later in the chain
/// does deny, so the loop runs each entry's body. (The toy stands for the loop alone: the `ItemActors` step after
/// it is the existing port's, tested in `script_tests.rs`.)
#[test]
fn a_weapon_chain_that_does_not_deny_lets_the_query_go_on() {
    let mut x = fixture();
    assert_eq!(x.call("Query").unwrap(), Value::Bool(true), "an empty chain");
    let gun = x.obj("ToyWeapon", "Gun");
    x.chain(&[gun]);
    assert_eq!(x.call("Query").unwrap(), Value::Bool(true));
    let denier = x.obj("ToyDenier", "Denier");
    x.chain(&[gun, denier]);
    assert_eq!(x.call("Query").unwrap(), Value::Bool(false));
}
