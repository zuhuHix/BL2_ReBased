//! NATIVE_LINK67_KEY_AND_LOOK.md section 5's tests 1 to 9 (lane IMPL-LINK67-KEY) on two invented packages built with
//! `ow_package::testkit` (AGENTS.md rule 9). The replication info's currency natives and the controller's
//! `SpendGoldenKey` are toy handlers here, so the test sees each call; every native is reached through the VM.

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::{Missing, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_package::testkit::script::{FunctionSpec, Param, ScriptToy};
use ow_vm::api::{NativeCall, StubPolicy, Vm};
use ow_vm::error::VmError;

use super::*;

const NATIVE: u32 = 0x400 | 0x20000;
static NEXT: AtomicU32 = AtomicU32::new(0);

const IMPORTS: [&str; 7] =
    ["Class", "Function", "ObjectProperty", "BoolProperty", "IntProperty", "ByteProperty", "InterfaceProperty"];

fn native(t: &mut ScriptToy, owner: i32, name: &str, params: &[Param], result: Option<(&str, i32)>) {
    t.function(owner, name, params, result, &[], &FunctionSpec::new(NATIVE, name), |_| None);
}

fn engine() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let object = t.class("Object", 0);
    let actor = t.class("Actor", object);
    t.class("IPickupable", object);
    let info = t.class("PlayerReplicationInfo", actor);
    let controller = t.class("Controller", actor);
    t.prop("Object", controller, "PlayerReplicationInfo", 0, info);
    t.class("DroppedPickup", actor);
    t.build()
}

fn willow_game() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let controller = t.p.add_import("Core", "Class", engine, "Controller");
    let info = t.p.add_import("Core", "Class", engine, "PlayerReplicationInfo");
    let dropped = t.p.add_import("Core", "Class", engine, "DroppedPickup");
    let pickupable = t.p.add_import("Core", "Class", engine, "IPickupable");
    let pickup = t.class("WillowPickup", dropped);
    t.prop("Bool", pickup, "bCostsToPickUp", 0, 0);
    t.prop("Byte", pickup, "CostsToPickUpType", 0, 0);
    t.prop("Int", pickup, "CostsToPickUpAmount", 0, 0);
    let pc = t.class("WillowPlayerController", controller);
    let arg = Param { type_ref: pickupable, ..Param::new("Interface", "Pickupable") };
    native(&mut t, pc, "CanAffordToPickUpPickupable", std::slice::from_ref(&arg), Some(("Bool", 0)));
    native(&mut t, pc, "PayForPickupable", &[arg], None);
    native(&mut t, pc, "UpdateAmmoCounts", &[Param::new("Bool", "bSilent")], None);
    native(&mut t, pc, "SpendGoldenKey", &[], None);
    let winfo = t.class("WillowPlayerReplicationInfo", info);
    t.prop("Int", winfo, "ToyMoney", 0, 0);
    native(&mut t, winfo, "GetCurrencyOnHand", &[Param::new("Byte", "FormOfCurrency")], Some(("Int", 0)));
    native(&mut t, winfo, "AddCurrencyOnHand", &[Param::new("Byte", "FormOfCurrency"), Param::new("Int", "Delta")], None);
    t.build()
}

struct Fx {
    vm: Vm,
    dir: PathBuf,
    keys_spent: Rc<Cell<u32>>,
    currency_calls: Rc<Cell<u32>>,
    /// The `SpendGoldenKey` call that refuses (0: none), and the calls so far.
    key_fail_on: Rc<Cell<u32>>,
    /// Whether a refusing handler (`SpendGoldenKey`'s, or `AddCurrencyOnHand` when `add_fails`) first writes the
    /// replication info's money (review r2 of #774).
    write_then_fail: Rc<Cell<bool>>,
    add_fails: Rc<Cell<bool>>,
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn money_prop(vm: &Vm, o: ObjRef) -> ow_object::ids::PropId {
    let class = vm.space.heap.get(o).unwrap().class;
    vm.space.prop(class, "ToyMoney").unwrap()
}

fn fx_in(mode: ParityMode) -> Fx {
    let dir = std::env::temp_dir().join(format!(
        "ow-inventory-pickup-cost-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let (e, w) = (engine(), willow_game());
    let defs = DefStore::from_synthetic(
        &dir,
        &[("Engine.upk", e.as_slice()), ("WillowGame.upk", w.as_slice())],
        IdentityPolicy::FirstInScanOrder,
        mode,
    )
    .unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), mode)
        .with_stub_policy(StubPolicy::refuse_all());
    for c in ["WillowGame.WillowPlayerController", "WillowGame.WillowPlayerReplicationInfo", "WillowGame.WillowPickup"] {
        vm.find_class(c).unwrap();
    }
    vm.find_class("Engine.DroppedPickup").unwrap();
    vm.find_class("Engine.PlayerReplicationInfo").unwrap();
    crate::pickup::register(&mut vm).unwrap();
    // Toy stand-ins for the bound currency natives (ow-combat's in the slice) and the controller's event, counting.
    let (keys_spent, currency_calls) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    let key_fail_on = Rc::new(Cell::new(0u32));
    let (write_then_fail, add_fails) = (Rc::new(Cell::new(false)), Rc::new(Cell::new(false)));
    if mode == ParityMode::Off {
        let calls = currency_calls.clone();
        vm.register_native(
            "WillowPlayerReplicationInfo.GetCurrencyOnHand(byte)",
            Rc::new(move |c: &mut NativeCall<'_>| {
                calls.set(calls.get() + 1);
                let me = c.this.unwrap();
                let p = money_prop(c.vm, me);
                Ok(c.vm.space.read(me, PropAddr::scalar(p))?.clone())
            }),
        )
        .unwrap();
        let (writes, fails) = (write_then_fail.clone(), add_fails.clone());
        vm.register_native(
            "WillowPlayerReplicationInfo.AddCurrencyOnHand(byte,int)",
            Rc::new(move |c: &mut NativeCall<'_>| {
                let me = c.this.unwrap();
                if fails.get() {
                    if writes.get() {
                        let p = money_prop(c.vm, me);
                        c.vm.space.write(me, PropAddr::scalar(p), Value::Int(-7))?;
                    }
                    return Err(ow_core::NotYet::not_read("a toy currency refusal", "TOY.md").into());
                }
                let Value::Int(delta) = *c.arg(1)? else { panic!("delta") };
                let p = money_prop(c.vm, me);
                let Value::Int(now) = *c.vm.space.read(me, PropAddr::scalar(p))? else { panic!("money") };
                c.vm.space.write(me, PropAddr::scalar(p), Value::Int(now + delta))?;
                Ok(Value::None)
            }),
        )
        .unwrap();
        let (spent, fail_on, key_calls) = (keys_spent.clone(), key_fail_on.clone(), Rc::new(Cell::new(0u32)));
        let writes = write_then_fail.clone();
        vm.register_native(
            "WillowPlayerController.SpendGoldenKey()",
            Rc::new(move |c: &mut NativeCall<'_>| {
                key_calls.set(key_calls.get() + 1);
                if key_calls.get() == fail_on.get() {
                    if writes.get() {
                        // A heap write before the refusal: the controller's replication info's money.
                        let me = c.this.unwrap();
                        let class = c.vm.space.heap.get(me)?.class;
                        let p = c.vm.space.prop(class, "PlayerReplicationInfo").unwrap();
                        c.vm.space.write(me, PropAddr::scalar(p), Value::Object(None))?;
                    }
                    return Err(ow_core::NotYet::not_read("a toy golden-key refusal", "TOY.md").into());
                }
                spent.set(spent.get() + 1);
                Ok(Value::None)
            }),
        )
        .unwrap();
    }
    Fx { vm, dir, keys_spent, currency_calls, key_fail_on, write_then_fail, add_fails }
}

fn fx() -> Fx {
    fx_in(ParityMode::Off)
}

impl Fx {
    fn obj(&mut self, class: &str) -> ObjRef {
        let c = self.vm.find_class(class).unwrap();
        ow_vm::interp::instantiate_from_defaults(&mut self.vm, c).unwrap()
    }

    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap();
        self.vm.space.write(o, PropAddr::scalar(p), v).unwrap();
    }

    fn money(&self, info: ObjRef) -> Value {
        let p = money_prop(&self.vm, info);
        self.vm.space.read(info, PropAddr::scalar(p)).unwrap().clone()
    }

    /// A controller whose replication info is a `WillowPlayerReplicationInfo` holding `money`.
    fn controller(&mut self, money: i32) -> (ObjRef, ObjRef) {
        let pc = self.obj("WillowGame.WillowPlayerController");
        let info = self.obj("WillowGame.WillowPlayerReplicationInfo");
        self.set(info, "ToyMoney", Value::Int(money));
        self.set(pc, "PlayerReplicationInfo", Value::Object(Some(info)));
        (pc, info)
    }

    /// A `WillowPickup` with its cost fields; `None` for free.
    fn pickup(&mut self, cost: Option<(u8, i32)>) -> ObjRef {
        let p = self.obj("WillowGame.WillowPickup");
        if let Some((ty, amount)) = cost {
            self.set(p, "bCostsToPickUp", Value::Bool(true));
            self.set(p, "CostsToPickUpType", Value::Byte(ty));
            self.set(p, "CostsToPickUpAmount", Value::Int(amount));
        }
        p
    }

    fn marks(&self) -> (u64, u64, u64, u64, usize) {
        let vm = &self.vm;
        (vm.class_code_loads(), vm.space.heap.committed_writes(), vm.rng.draws(), vm.defs.load_transitions(), vm.space.names.len())
    }

    fn call(&mut self, name: &str, pc: ObjRef, args: Vec<Value>) -> Result<Value, VmError> {
        let f = self.vm.find_function(&format!("WillowGame.WillowPlayerController.{name}"))?;
        Ok(self.vm.call(f, Some(pc), args)?.ret)
    }

    /// `CanAffordToPickUpPickupable`, which writes nothing.
    fn afford(&mut self, pc: ObjRef, p: Option<ObjRef>) -> Result<Value, VmError> {
        let before = self.marks();
        let r = self.call("CanAffordToPickUpPickupable", pc, vec![Value::Interface(p)]);
        assert_eq!(self.marks(), before, "CanAffordToPickUpPickupable writes, loads and interns nothing");
        r
    }

    fn pay(&mut self, pc: ObjRef, p: Option<ObjRef>) -> Result<Value, VmError> {
        self.call("PayForPickupable", pc, vec![Value::Interface(p)])
    }
}

fn not_read(r: Result<Value, VmError>) -> String {
    match r {
        Err(VmError::NotYet(n)) => {
            assert_eq!((n.missing, n.record), (Missing::NotRead, RECORD), "{n}");
            n.what.to_string()
        }
        other => panic!("expected NotYet, got {other:?}"),
    }
}

/// Tests 1 and 2: no pickupable, and a `WillowPickup` with `bCostsToPickUp` false: true, nothing written.
#[test]
fn no_pickupable_or_no_cost_is_affordable() {
    let mut x = fx();
    let (pc, _) = x.controller(0);
    assert_eq!(x.afford(pc, None).unwrap(), Value::Bool(true));
    let free = x.pickup(None);
    assert_eq!(x.afford(pc, Some(free)).unwrap(), Value::Bool(true));
    assert_eq!(x.currency_calls.get(), 0, "no currency asked");
}

/// Test 3: a currency cost of 5 (type 0): true with 5 on hand, false with 4, through `GetCurrencyOnHand`.
#[test]
fn a_currency_cost_compares_with_the_amount_on_hand() {
    let mut x = fx();
    let p = x.pickup(Some((0, 5)));
    let (pc, info) = x.controller(5);
    assert_eq!(x.afford(pc, Some(p)).unwrap(), Value::Bool(true));
    x.set(info, "ToyMoney", Value::Int(4));
    assert_eq!(x.afford(pc, Some(p)).unwrap(), Value::Bool(false));
    assert_eq!(x.currency_calls.get(), 2, "asked through the bound native");
}

/// Test 4: the same cost with the controller's `PlayerReplicationInfo` None, or a plain `PlayerReplicationInfo`: true.
#[test]
fn a_failed_cast_is_affordable() {
    let mut x = fx();
    let p = x.pickup(Some((0, 5)));
    let (pc, _) = x.controller(0);
    x.set(pc, "PlayerReplicationInfo", Value::Object(None));
    assert_eq!(x.afford(pc, Some(p)).unwrap(), Value::Bool(true));
    let plain = x.obj("Engine.PlayerReplicationInfo");
    x.set(pc, "PlayerReplicationInfo", Value::Object(Some(plain)));
    assert_eq!(x.afford(pc, Some(p)).unwrap(), Value::Bool(true));
    assert_eq!(x.currency_calls.get(), 0);
}

/// Test 5: a golden-key cost (type 3) is the golden-key count's `NotRead`.
#[test]
fn a_golden_key_cost_is_not_read() {
    let mut x = fx();
    let p = x.pickup(Some((GOLDEN_KEYS, 1)));
    let (pc, _) = x.controller(100);
    assert_eq!(not_read(x.afford(pc, Some(p))), GOLDEN_KEY_COUNT);
}

/// Test 6: a pickupable class that declares no cost fields is the cost query's `NotRead`, naming it; a lazy pickupable
/// is the not-loaded `NotRead`; a lazy replication info is its not-loaded `NotRead`, before the class test.
#[test]
fn an_unread_class_and_lazy_objects_are_not_read() {
    let mut x = fx();
    let (pc, _) = x.controller(5);
    let dropped = x.obj("Engine.DroppedPickup");
    assert_eq!(not_read(x.afford(pc, Some(dropped))), format!("{CAN_AFFORD_NAME}: {COST_QUERY} Engine.DroppedPickup"));
    let class = x.vm.find_class("WillowGame.WillowPickup").unwrap();
    let lazy = x.vm.space.alloc_stand_in(class, "LazyPickup", Origin::Default { class }).unwrap();
    assert_eq!(not_read(x.afford(pc, Some(lazy))), format!("{CAN_AFFORD_NAME}: {PICKUPABLE_NOT_LOADED}"));
    let p = x.pickup(Some((0, 5)));
    // A class with slots, so a slotless stand-in of it counts as not loaded.
    let class = x.vm.find_class("WillowGame.WillowPlayerReplicationInfo").unwrap();
    let lazy_info = x.vm.space.alloc_stand_in(class, "LazyInfo", Origin::Default { class }).unwrap();
    x.set(pc, "PlayerReplicationInfo", Value::Object(Some(lazy_info)));
    assert_eq!(not_read(x.afford(pc, Some(p))), format!("{CAN_AFFORD_NAME}: {INFO_NOT_LOADED}"));
}

/// Test 7: `PayForPickupable`. Free: nothing. Currency 5: the money goes down by 5 through `AddCurrencyOnHand`.
/// Golden keys 2: `SpendGoldenKey` twice; 0 or less: none, and no golden-key count is asked (not step 5).
#[test]
fn pay_for_pickupable_spends_the_cost() {
    let mut x = fx();
    let (pc, info) = x.controller(12);
    let free = x.pickup(None);
    let before = x.marks();
    x.pay(pc, Some(free)).unwrap();
    x.pay(pc, None).unwrap();
    assert_eq!(x.marks(), before, "free: nothing written");
    let p = x.pickup(Some((0, 5)));
    x.pay(pc, Some(p)).unwrap();
    assert_eq!(x.money(info), Value::Int(7));
    for (amount, spent) in [(2, 2), (0, 2), (-1, 2)] {
        let keys = x.pickup(Some((GOLDEN_KEYS, amount)));
        x.pay(pc, Some(keys)).unwrap();
        assert_eq!(x.keys_spent.get(), spent, "amount {amount}");
    }
    assert_eq!(x.money(info), Value::Int(7), "golden keys touch no currency");
    // A failed cast pays nothing.
    x.set(pc, "PlayerReplicationInfo", Value::Object(None));
    x.pay(pc, Some(p)).unwrap();
    assert_eq!(x.money(info), Value::Int(7));
}

/// Test 9: Reference binds none of the three keys (the stub policy answers).
#[test]
fn reference_binds_nothing() {
    let x = fx_in(ParityMode::Reference);
    for key in [CAN_AFFORD, PAY_FOR] {
        assert!(!x.vm.has_native(key), "{key}");
    }
}

/// Review of #774, LOW: `PayForPickupable`'s shared refusals name it: a lazy pickupable, an unread cost class and a
/// lazy replication info. Nothing is written.
#[test]
fn pay_for_pickupable_s_refusals_name_it() {
    let mut x = fx();
    let (pc, _) = x.controller(5);
    let class = x.vm.find_class("WillowGame.WillowPickup").unwrap();
    let lazy = x.vm.space.alloc_stand_in(class, "LazyPickup", Origin::Default { class }).unwrap();
    let dropped = x.obj("Engine.DroppedPickup");
    let p = x.pickup(Some((0, 5)));
    let class = x.vm.find_class("WillowGame.WillowPlayerReplicationInfo").unwrap();
    let lazy_info = x.vm.space.alloc_stand_in(class, "LazyInfo", Origin::Default { class }).unwrap();
    let before = x.marks();
    assert_eq!(not_read(x.pay(pc, Some(lazy))), format!("{PAY_FOR_NAME}: {PICKUPABLE_NOT_LOADED}"));
    assert_eq!(not_read(x.pay(pc, Some(dropped))), format!("{PAY_FOR_NAME}: {COST_QUERY} Engine.DroppedPickup"));
    assert_eq!(x.marks(), before, "nothing written");
    x.set(pc, "PlayerReplicationInfo", Value::Object(Some(lazy_info)));
    let before = x.marks();
    assert_eq!(not_read(x.pay(pc, Some(p))), format!("{PAY_FOR_NAME}: {INFO_NOT_LOADED}"));
    assert_eq!(x.marks(), before, "nothing written");
}

/// Review of #774, MED: a raised `SpendGoldenKey` is a write, so a later raise's refusal is marked "stopped after
/// partial writes" (kind and record kept); the first raise's refusal is not.
#[test]
fn a_later_golden_key_raise_s_refusal_is_after_partial_writes() {
    for (fail_on, marked) in [(2, true), (1, false)] {
        let mut x = fx();
        x.key_fail_on.set(fail_on);
        let (pc, _) = x.controller(0);
        let keys = x.pickup(Some((GOLDEN_KEYS, 2)));
        match x.pay(pc, Some(keys)) {
            Err(VmError::NotYet(n)) => {
                assert_eq!((n.missing, n.record), (Missing::NotRead, "TOY.md"), "{n}");
                assert_eq!(ow_core::notyet::is_partial_writes(&n.what), marked, "fail on raise {fail_on}: {}", n.what);
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(x.keys_spent.get(), fail_on - 1, "the raises before the failing one");
    }
}

/// Review r2 of #774: a handler that writes and then refuses on the FIRST call is marked "stopped after partial
/// writes" too (#518's counters moved since the native was entered), for `SpendGoldenKey` and for
/// `AddCurrencyOnHand`. One that refuses without writing stays unmarked.
#[test]
fn a_first_call_that_writes_then_refuses_is_after_partial_writes() {
    let marked_of = |r: Result<Value, VmError>| match r {
        Err(VmError::NotYet(n)) => {
            assert_eq!((n.missing, n.record), (Missing::NotRead, "TOY.md"), "{n}");
            ow_core::notyet::is_partial_writes(&n.what)
        }
        other => panic!("{other:?}"),
    };
    for writes in [true, false] {
        let mut x = fx();
        x.key_fail_on.set(1);
        x.write_then_fail.set(writes);
        let (pc, _) = x.controller(0);
        let keys = x.pickup(Some((GOLDEN_KEYS, 2)));
        assert_eq!(marked_of(x.pay(pc, Some(keys))), writes, "SpendGoldenKey, writing first: {writes}");
        let mut x = fx();
        x.add_fails.set(true);
        x.write_then_fail.set(writes);
        let (pc, _) = x.controller(9);
        let p = x.pickup(Some((0, 5)));
        assert_eq!(marked_of(x.pay(pc, Some(p))), writes, "AddCurrencyOnHand, writing first: {writes}");
    }
}
