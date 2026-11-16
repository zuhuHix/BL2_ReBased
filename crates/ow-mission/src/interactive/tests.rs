//! Tests of the interactive-object rules and natives on invented packages (AGENTS.md rule 9: invented classes and
//! values; the names are the vocabulary the natives read).

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::registry::AttributeSystem;
use ow_core::notyet::Missing;
use ow_core::{NotYet, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_package::testkit::reflect::property_payload;
use ow_package::testkit::script::{
    Asm, CPF_OUT, CPF_PARM, CPF_RET, FunctionSpec, ScriptToy, function_payload,
};
use ow_package::testkit::{Export, ints};
use ow_vm::api::{NativePack, StubPolicy, Vm};
use ow_vm::error::VmError;

use super::*;

// ---- the pure rules ---------------------------------------------------------------------------------------------

fn usable(component: Option<bool>, object: bool) -> UseHit {
    UseHit {
        usable: Some(Usability::new(component, object)),
        ..UseHit::default()
    }
}

fn wall() -> UseHit {
    UseHit {
        blocks: true,
        ..UseHit::default()
    }
}

#[test]
fn the_nearest_usable_hit_in_front_of_a_wall_is_chosen() {
    let hits = [UseHit::default(), usable(None, true), usable(None, true), wall()];
    assert_eq!(choose_usable(&hits, true).usable, Some(1));
}

#[test]
fn a_wall_in_front_hides_a_usable_object() {
    let hits = [wall(), usable(None, true)];
    assert_eq!(choose_usable(&hits, true), UseChoice::default());
}

#[test]
fn a_skipped_hit_neither_blocks_nor_is_chosen() {
    let mut dying = usable(None, true);
    dying.skipped = true;
    dying.blocks = true;
    let hits = [dying, usable(None, true)];
    assert_eq!(choose_usable(&hits, true).usable, Some(1));
}

#[test]
fn the_component_entry_or_the_object_answer_makes_a_hit_usable() {
    // The component entry's flag, else the object's own answer.
    assert_eq!(choose_usable(&[usable(Some(true), false)], true).usable, Some(0));
    assert_eq!(choose_usable(&[usable(Some(false), true)], true).usable, Some(0));
    assert_eq!(choose_usable(&[usable(None, true)], true).usable, Some(0));
    // A usable actor whose primary use is not available is no candidate and blocks like any other hit.
    let mut closed = usable(Some(false), false);
    closed.blocks = true;
    assert_eq!(choose_usable(&[closed, usable(None, true)], true).usable, None);
}

#[test]
fn a_trade_proxy_is_remembered_but_never_chosen() {
    let proxy = UseHit {
        trade_proxy: true,
        blocks: true,
        ..UseHit::default()
    };
    let choice = choose_usable(&[proxy, usable(None, true), proxy], true);
    assert_eq!(choice.usable, Some(1));
    assert_eq!(choice.trade_proxy, Some(0));
    // The walk goes on past the chosen hit only for a trade proxy.
    let choice = choose_usable(&[usable(None, true), wall(), proxy], true);
    assert_eq!(choice, UseChoice {
        usable: Some(0),
        trade_proxy: Some(2)
    });
}

#[test]
fn a_closed_gate_chooses_nothing() {
    assert_eq!(choose_usable(&[usable(None, true)], false), UseChoice::default());
}

#[test]
fn the_evaluation_runs_at_most_thirty_times_a_second() {
    assert_eq!(next_evaluation(2.0, USABLE_OBJECT_UPDATE_RATE), Some(2.0 + 1.0 / 30.0));
    assert_eq!(next_evaluation(2.0, 0.0), None);
    assert_eq!(next_evaluation(2.0, f32::NAN), None);
}

#[test]
fn the_cost_rules_compare_with_at_most_and_pay_by_negating() {
    assert_eq!(cost_of(0, 0, 50), None);
    let cost = cost_of(1, 0, 50);
    assert_eq!(cost, Some(UseCost::new(0, 50)));
    assert_eq!(can_afford(None, 0), Ok(true));
    assert_eq!(can_afford(cost, 50), Ok(true));
    assert_eq!(can_afford(cost, 49), Ok(false));
    assert_eq!(payment(None), Ok(Payment::Nothing));
    assert_eq!(payment(cost), Ok(Payment::AddCurrency {
        currency: 0,
        delta: -50
    }));
    // Wrapping: i32::MIN stays a negative delta, which AddCurrencyOnHand clamps to 0.
    assert_eq!(payment(Some(UseCost::new(0, i32::MIN))), Ok(Payment::AddCurrency {
        currency: 0,
        delta: i32::MIN
    }));
}

#[test]
fn golden_keys_are_spent_one_by_one_and_their_count_is_not_read() {
    let key = Some(UseCost::new(GOLDEN_KEYS, 2));
    assert_eq!(payment(key), Ok(Payment::SpendGoldenKeys(2)));
    assert_eq!(payment(Some(UseCost::new(GOLDEN_KEYS, -1))), Ok(Payment::SpendGoldenKeys(0)));
    assert!(payment(Some(UseCost::new(GOLDEN_KEYS, MAX_GOLDEN_KEY_COST + 1))).is_err());
    let e = can_afford(key, 5).unwrap_err();
    assert_eq!(e.missing, Missing::NotRead);
}

// ---- the natives on invented packages ---------------------------------------------------------------------------

/// `FUNC_Final | FUNC_Defined | FUNC_Public`, `FUNC_Native | FUNC_Public`.
const SCRIPT: u32 = 0x1 | 0x2 | 0x20000;
const NATIVE: u32 = 0x400 | 0x20000;

/// The reflection helpers over one package with a root class (as `ow-combat`'s fixture).
struct Toy {
    t: ScriptToy,
    root: i32,
}

impl Toy {
    fn new() -> Toy {
        Toy {
            t: ScriptToy::new(&["Byte", "Object", "Int", "Bool", "Struct"]),
            root: 0,
        }
    }

    fn cls(&mut self, name: &str, super_ref: i32) -> i32 {
        let s = if super_ref == 0 { self.root } else { super_ref };
        self.t.class(name, s)
    }

    fn tail(&self, kind: &str, type_ref: i32) -> Vec<u8> {
        match kind {
            "Object" => ints(&[if type_ref == 0 { self.root } else { type_ref }]),
            _ => ints(&[type_ref]),
        }
    }

    fn prop(&mut self, kind: &str, owner: i32, name: &str) -> i32 {
        let tail = self.tail(kind, 0);
        self.t.prop_tail(kind, owner, name, 0, &tail)
    }

    fn prop_dim(&mut self, kind: &str, owner: i32, name: &str, dim: u32, type_ref: i32) -> i32 {
        let tail = self.tail(kind, type_ref);
        let payload = property_payload(&self.t.none, dim, 0, &tail);
        let mut e = Export::new(self.t.import(&format!("{kind}Property")), name, payload);
        e.outer = owner;
        self.t.p.add_export_with(e)
    }

    fn strukt(&mut self, owner: i32, name: &str) -> i32 {
        let mut e = Export::new(self.t.import("ScriptStruct"), name, vec![0u8; 16]);
        e.outer = owner;
        self.t.p.add_export_with(e)
    }

    /// A function: the return value, then the parameters in reverse order, then the Function export.
    fn function(
        &mut self,
        owner: i32,
        name: &str,
        params: &[(&str, &str)],
        result: Option<&str>,
        flags: u32,
    ) {
        let mut ordered: Vec<(String, String, u64)> = Vec::new();
        if let Some(kind) = result {
            ordered.push((kind.into(), "ReturnValue".into(), CPF_PARM | CPF_OUT | CPF_RET));
        }
        for (kind, n) in params.iter().rev() {
            ordered.push(((*kind).into(), (*n).into(), CPF_PARM));
        }
        let function = self.t.p.next_export() + i32::try_from(ordered.len()).unwrap();
        for (kind, child, f) in &ordered {
            let tail = self.tail(kind, 0);
            self.t.prop_tail(kind, function, child, *f, &tail);
        }
        let body = if flags == SCRIPT {
            let mut a = Asm::new();
            a.stmt().return_nothing();
            a.end();
            Some(a)
        } else {
            None
        };
        let payload = function_payload(&mut self.t.p, body.as_ref(), &FunctionSpec::new(flags, name));
        let mut e = Export::new(self.t.import("Function"), name, payload);
        e.outer = owner;
        assert_eq!(self.t.p.add_export_with(e), function);
    }
}

fn engine() -> Vec<u8> {
    let mut e = Toy::new();
    e.root = e.t.class("Object", 0);
    let actor = e.cls("Actor", 0);
    let pawn = e.cls("Pawn", actor);
    e.prop("Object", pawn, "Controller");
    let controller = e.cls("Controller", actor);
    e.prop("Object", controller, "Pawn");
    e.prop("Object", controller, "PlayerReplicationInfo");
    e.t.build()
}

fn willow_game() -> Vec<u8> {
    let mut w = Toy::new();
    let engine = w.t.p.add_import("Core", "Package", 0, "Engine");
    w.root = w.t.p.add_import("Core", "Class", engine, "Object");
    let actor = w.t.p.add_import("Core", "Class", engine, "Actor");
    let pawn = w.t.p.add_import("Core", "Class", engine, "Pawn");
    let controller = w.t.p.add_import("Core", "Class", engine, "Controller");
    let cost_fields = |w: &mut Toy, owner: i32| {
        w.prop_dim("Byte", owner, "bCostsToUse", 2, 0);
        w.prop_dim("Byte", owner, "CostsToUseType", 2, 0);
        w.prop_dim("Int", owner, "CostsToUseAmount", 2, 0);
    };
    let willow_pawn = w.cls("WillowPawn", pawn);
    let ai_pawn = w.cls("WillowAIPawn", willow_pawn);
    cost_fields(&mut w, ai_pawn);
    let object = w.cls("WillowInteractiveObject", actor);
    cost_fields(&mut w, object);
    w.function(object, "IsComponentUsable", &[("Object", "UsableComponent"), ("Byte", "UsedType")], Some("Bool"), NATIVE);
    w.cls("ToyUsable", actor);
    let pc = w.cls("WillowPlayerController", controller);
    let params = [("Object", "UsedObject"), ("Byte", "UsabilityType")];
    w.function(pc, "CanAffordToUseUsableObject", &params, Some("Bool"), NATIVE);
    w.function(pc, "PayForUsedObject", &params, None, NATIVE);
    w.function(pc, "SpendGoldenKey", &[], None, NATIVE);
    let info = w.cls("WillowPlayerReplicationInfo", actor);
    let currency = w.strukt(info, "CurrencyState");
    w.prop("Int", currency, "CurrentAmount");
    w.prop_dim("Struct", info, "Currency", 13, currency);
    w.function(info, "GetCurrencyOnHand", &[("Byte", "Type")], Some("Int"), NATIVE);
    w.function(info, "AddCurrencyOnHand", &[("Byte", "Type"), ("Int", "Delta")], None, NATIVE);
    w.function(info, "NotifyCurrencyDelegates", &[], None, SCRIPT);
    w.t.build()
}

static NEXT: AtomicU32 = AtomicU32::new(0);

/// A VM over the invented packages, with `ow-combat`'s pack (the currency natives) and this module's pack, in `mode`.
struct Env {
    vm: Vm,
    dir: PathBuf,
    keys_spent: Rc<Cell<u32>>,
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

impl Env {
    fn new(mode: ParityMode) -> Env {
        let dir = std::env::temp_dir().join(format!(
            "ow-mission-interactive-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let files = [("Engine.upk", engine()), ("WillowGame.upk", willow_game())];
        let refs: Vec<(&str, &[u8])> = files.iter().map(|(n, b)| (*n, b.as_slice())).collect();
        let defs = DefStore::from_synthetic(&dir, &refs, IdentityPolicy::FirstInScanOrder, mode).unwrap();
        let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), mode)
            .with_stub_policy(StubPolicy::refuse_all());
        ow_combat::hit::Pack.register(&mut vm).unwrap();
        Pack.register(&mut vm).unwrap();
        let keys_spent = Rc::new(Cell::new(0));
        let counter = keys_spent.clone();
        ow_combat::hit::bind_fn(&mut vm, "WillowGame.WillowPlayerController.SpendGoldenKey", move |_| {
            counter.set(counter.get() + 1);
            Ok(Value::None)
        })
        .unwrap();
        Env { vm, dir, keys_spent }
    }

    fn make(&mut self, class: &str) -> ObjRef {
        let k = self.vm.find_class(class).unwrap();
        ow_vm::interp::instantiate_from_defaults(&mut self.vm, k).unwrap()
    }

    fn write(&mut self, o: ObjRef, name: &str, index: u32, v: Value) {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let prop = self.vm.space.prop(class, name).unwrap();
        self.vm.space.write(o, PropAddr { prop, index }, v).unwrap();
    }

    /// A controller with a replication info holding `credits`.
    fn controller(&mut self, credits: i64) -> (ObjRef, ObjRef) {
        let pc = self.make("WillowGame.WillowPlayerController");
        let info = self.make("WillowGame.WillowPlayerReplicationInfo");
        ow_combat::hit::access::set(&mut self.vm, pc, "PlayerReplicationInfo", Value::Object(Some(info))).unwrap();
        ow_combat::down::set_currency_on_hand(&mut self.vm, info, 0, credits).unwrap();
        (pc, info)
    }

    /// An object of `class` whose use type `ty` costs `amount` of `currency`.
    fn costly(&mut self, class: &str, ty: u32, currency: u8, amount: i32) -> ObjRef {
        let o = self.make(class);
        self.write(o, "bCostsToUse", ty, Value::Byte(1));
        self.write(o, "CostsToUseType", ty, Value::Byte(currency));
        self.write(o, "CostsToUseAmount", ty, Value::Int(amount));
        o
    }

    fn call(&mut self, path: &str, pc: ObjRef, object: Option<ObjRef>, ty: u8) -> Result<Value, VmError> {
        let f = self.vm.find_function(path)?;
        self.vm
            .call(f, Some(pc), vec![Value::Object(object), Value::Byte(ty)])
            .map(|r| r.ret)
    }
}

fn not_yet(e: VmError) -> NotYet {
    match e {
        VmError::NotYet(n) => n,
        other => panic!("expected NotYet, got {other:?}"),
    }
}

#[test]
fn a_free_or_absent_object_is_affordable_and_paying_for_it_changes_nothing() {
    let mut env = Env::new(ParityMode::Off);
    let (pc, info) = env.controller(10);
    let free = env.make(INTERACTIVE_OBJECT);
    assert_eq!(env.call(CAN_AFFORD, pc, Some(free), 0).unwrap(), Value::Bool(true));
    assert_eq!(env.call(CAN_AFFORD, pc, None, 0).unwrap(), Value::Bool(true));
    env.call(PAY, pc, Some(free), 0).unwrap();
    assert_eq!(ow_combat::down::currency_on_hand(&mut env.vm, info, 0).unwrap(), 10);
    // The WillowPawn base never costs.
    let pawn = env.make(WILLOW_PAWN);
    assert_eq!(env.call(CAN_AFFORD, pc, Some(pawn), 0).unwrap(), Value::Bool(true));
}

#[test]
fn an_interactive_object_cost_is_compared_with_the_cash_and_charged_through_add_currency() {
    let mut env = Env::new(ParityMode::Off);
    let (pc, info) = env.controller(30);
    let chest = env.costly(INTERACTIVE_OBJECT, 0, 0, 30);
    assert_eq!(env.call(CAN_AFFORD, pc, Some(chest), 0).unwrap(), Value::Bool(true));
    env.call(PAY, pc, Some(chest), 0).unwrap();
    assert_eq!(ow_combat::down::currency_on_hand(&mut env.vm, info, 0).unwrap(), 0);
    assert_eq!(env.call(CAN_AFFORD, pc, Some(chest), 0).unwrap(), Value::Bool(false));
    // Paying does not check affordability; AddCurrencyOnHand keeps the amount at 0.
    env.call(PAY, pc, Some(chest), 0).unwrap();
    assert_eq!(ow_combat::down::currency_on_hand(&mut env.vm, info, 0).unwrap(), 0);
    // The secondary use has its own entry, here free.
    assert_eq!(env.call(CAN_AFFORD, pc, Some(chest), 1).unwrap(), Value::Bool(true));
}

#[test]
fn an_ai_pawn_reads_the_same_three_fields() {
    let mut env = Env::new(ParityMode::Off);
    let (pc, _) = env.controller(5);
    let npc = env.costly(AI_PAWN, 1, 0, 6);
    assert_eq!(env.call(CAN_AFFORD, pc, Some(npc), 1).unwrap(), Value::Bool(false));
    assert_eq!(env.call(CAN_AFFORD, pc, Some(npc), 0).unwrap(), Value::Bool(true));
}

#[test]
fn a_controller_without_a_willow_replication_info_can_afford_and_cannot_pay() {
    let mut env = Env::new(ParityMode::Off);
    let pc = env.make("WillowGame.WillowPlayerController");
    let chest = env.costly(INTERACTIVE_OBJECT, 0, 0, 30);
    assert_eq!(env.call(CAN_AFFORD, pc, Some(chest), 0).unwrap(), Value::Bool(true));
    let e = not_yet(env.call(PAY, pc, Some(chest), 0).unwrap_err());
    assert_eq!(e.missing, Missing::NotRead);
}

#[test]
fn golden_keys_fire_spend_golden_key_per_unit_and_their_count_is_not_yet() {
    let mut env = Env::new(ParityMode::Off);
    let (pc, info) = env.controller(100);
    let golden = env.costly(INTERACTIVE_OBJECT, 0, GOLDEN_KEYS, 2);
    env.call(PAY, pc, Some(golden), 0).unwrap();
    assert_eq!(env.keys_spent.get(), 2);
    assert_eq!(ow_combat::down::currency_on_hand(&mut env.vm, info, 0).unwrap(), 100);
    let e = not_yet(env.call(CAN_AFFORD, pc, Some(golden), 0).unwrap_err());
    assert_eq!(e.missing, Missing::NotRead);
}

#[test]
fn an_unread_class_and_an_unread_native_are_not_yet() {
    let mut env = Env::new(ParityMode::Off);
    let (pc, _) = env.controller(1);
    let other = env.make("WillowGame.ToyUsable");
    let e = not_yet(env.call(CAN_AFFORD, pc, Some(other), 0).unwrap_err());
    assert_eq!(e.missing, Missing::NotRead);
    let chest = env.make(INTERACTIVE_OBJECT);
    let f = env.vm.find_function(IS_COMPONENT_USABLE).unwrap();
    let e = not_yet(env.vm.call(f, Some(chest), vec![Value::Object(None), Value::Byte(0)]).unwrap_err());
    assert_eq!(e.missing, Missing::NotRead);
}

#[test]
fn a_usability_type_past_the_entries_is_bad_data() {
    let mut env = Env::new(ParityMode::Off);
    let (pc, _) = env.controller(1);
    let chest = env.make(INTERACTIVE_OBJECT);
    assert!(matches!(env.call(CAN_AFFORD, pc, Some(chest), 2), Err(VmError::Script { .. })));
}

#[test]
fn reference_mode_binds_none_of_the_natives() {
    let mut env = Env::new(ParityMode::Reference);
    let (pc, _) = env.controller(1);
    let chest = env.costly(INTERACTIVE_OBJECT, 0, 0, 30);
    // No binding: the stub policy refuses the unbound native, as before this pack existed.
    assert!(matches!(env.call(CAN_AFFORD, pc, Some(chest), 0), Err(VmError::UnboundNative(_))));
}
