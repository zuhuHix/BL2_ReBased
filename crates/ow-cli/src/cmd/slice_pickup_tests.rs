//! Tests of the headless pickup driver's own rules (lane IMPL-PICKUP, review r2 of #452) on an invented package built
//! with `ow_package::testkit` (AGENTS.md rule 9): `ToyPickup`, a pickup class whose global `Touch` and whose `Pickup`
//! state's `Touch` set different flags, the stock `GotoState`, and `ToyController` with the slots `look` reads first.

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::{Missing, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_object::ids::PropAddr;
use ow_package::testkit::script::{Asm, CPF_OPT, FunctionSpec, Param, ScriptToy};
use ow_package::testkit::{Export, ints};
use ow_vm::api::{StubPolicy, Vm};

use super::*;

const SCRIPT: u32 = 0x1 | 0x2 | 0x20000;
const NATIVE: u32 = 0x400 | 0x1 | 0x20000;
static NEXT: AtomicU32 = AtomicU32::new(0);

fn param(kind: &str, name: &str, flags: u64) -> Param {
    Param { flags, ..Param::new(kind, name) }
}

/// A State export in `owner` with no state code (only an empty label table).
fn state(t: &mut ScriptToy, owner: i32, name: &str) -> i32 {
    let mut a = Asm::new();
    let at = a.here();
    a.stmt().raw(&[0x0C]);
    let none = t.none;
    a.raw(&none).i32(0);
    a.end();
    let label = u16::try_from(at + 1).unwrap();
    let mut payload = ints(&[0]);
    payload.extend_from_slice(&none);
    payload.extend_from_slice(&ints(&[0, 0, 0, 0, 0, 0, 0]));
    payload.extend_from_slice(&ints(&[i32::try_from(a.memory_size()).unwrap(), i32::try_from(a.bytes.len()).unwrap()]));
    payload.extend_from_slice(&a.bytes);
    payload.extend_from_slice(&0u32.to_le_bytes());
    payload.extend_from_slice(&label.to_le_bytes());
    payload.extend_from_slice(&0u16.to_le_bytes());
    payload.extend_from_slice(&ints(&[0]));
    let mut export = Export::new(t.import("State"), name, payload);
    export.outer = owner;
    t.p.add_export_with(export)
}

/// `Touch(Other)`: sets `flag` to true.
fn toy_touch(t: &mut ScriptToy, owner: i32, object: i32, flag: i32) {
    t.function(
        owner,
        "Touch",
        &[Param { type_ref: object, ..Param::new("Object", "Other") }],
        None,
        &[],
        &FunctionSpec::new(SCRIPT, "Touch"),
        |_| {
            let mut a = Asm::new();
            a.stmt().raw(&[0x14]).instance(flag).raw(&[0x27]);
            a.stmt().return_nothing();
            a.end();
            Some(a)
        },
    );
}

fn package() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class", "Function", "State", "NameProperty", "BoolProperty", "ObjectProperty"]);
    let object = t.class("Object", 0);
    let mut goto = FunctionSpec::new(NATIVE, "GotoState");
    goto.native = 113;
    t.function(
        object,
        "GotoState",
        &[
            param("Name", "NewState", CPF_OPT),
            param("Name", "Label", CPF_OPT),
            param("Bool", "bForceEvents", CPF_OPT),
            param("Bool", "bKeepStack", CPF_OPT),
        ],
        None,
        &[],
        &goto,
        |_| None,
    );
    let pickup = t.class("ToyPickup", object);
    let global = t.prop("Bool", pickup, "bTouchedGlobal", 0, 0);
    let in_state = t.prop("Bool", pickup, "bTouchedInState", 0, 0);
    toy_touch(&mut t, pickup, object, global);
    let pickup_state = state(&mut t, pickup, "Pickup");
    toy_touch(&mut t, pickup_state, object, in_state);
    // A controller with the two slots `look` reads first (the touched pickup, the HUD).
    let controller = t.class("ToyController", object);
    t.prop("Object", controller, "CurrentTouchedPickupable", 0, object);
    t.prop("Object", controller, "myHUD", 0, object);
    t.build()
}

struct Fx {
    vm: Vm,
    dir: PathBuf,
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn fx() -> Fx {
    let dir = std::env::temp_dir().join(format!("ow-cli-slice-pickup-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let bytes = package();
    let defs = DefStore::from_synthetic(&dir, &[("Engine.upk", bytes.as_slice())], IdentityPolicy::FirstInScanOrder, ParityMode::Off).unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off)
        .with_stub_policy(StubPolicy::refuse_all());
    ow_vm::natives_core::register(&mut vm).unwrap();
    Fx { vm, dir }
}

impl Fx {
    fn pickup(&mut self) -> ObjRef {
        let c = self.vm.find_class("Engine.ToyPickup").unwrap();
        ow_vm::interp::instantiate_from_defaults(&mut self.vm, c).unwrap()
    }

    fn flag(&self, o: ObjRef, name: &str) -> Value {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap();
        self.vm.space.read(o, PropAddr::scalar(p)).unwrap().clone()
    }
}

/// The touch is refused outside the `Pickup` state; once the run enters it, the state's `Touch` runs, not the
/// class's global one.
#[test]
fn the_touch_runs_the_pickup_states_handler() {
    let mut x = fx();
    let pickup = x.pickup();
    let pawn = x.pickup();
    assert!(touch(&mut x.vm, pickup, pawn).is_err(), "not in the Pickup state");
    assert_eq!(x.flag(pickup, "bTouchedGlobal"), Value::Bool(false));
    enter_pickup_state(&mut x.vm, pickup).unwrap();
    assert!(in_pickup_state(&mut x.vm, pickup).unwrap());
    touch(&mut x.vm, pickup, pawn).unwrap();
    assert_eq!(x.flag(pickup, "bTouchedInState"), Value::Bool(true), "the state's handler ran");
    assert_eq!(x.flag(pickup, "bTouchedGlobal"), Value::Bool(false), "the global one did not");
}

/// After the run's first write, a late `NotYet` is marked "stopped after partial writes" with its kind and record
/// kept, and an ordinary error passes through unchanged; before any write nothing is marked.
#[test]
fn only_a_not_yet_after_a_write_is_marked() {
    let late = after_writes(NotYet::layer_absent("a late step", "a layer", "NOTE.md").into());
    match late {
        VmError::NotYet(n) => {
            assert!(n.what.starts_with(PARTIAL_WRITES), "{n}");
            assert_eq!((n.missing.clone(), n.record), (Missing::LayerAbsent("a layer"), "NOTE.md"));
            let again = after_writes(VmError::NotYet(n.clone()));
            assert_eq!(again.to_string().matches(PARTIAL_WRITES).count(), 1, "marked once");
        }
        e => panic!("{e}"),
    }
    // A text that mentions the prefix later on is still marked at its start (review r1 of #467).
    let inside = format!("a handler ({PARTIAL_WRITES} inside)");
    match after_writes(NotYet::not_read(inside, "NOTE.md").into()) {
        VmError::NotYet(n) => assert!(n.what.starts_with(PARTIAL_WRITES), "{n}"),
        e => panic!("{e}"),
    }
    let ordinary = after_writes(VmError::Script { function: "f".into(), message: "an ordinary error".into() });
    assert!(matches!(&ordinary, VmError::Script { message, .. } if message == "an ordinary error"), "{ordinary}");
    // Through a step: an ordinary error of a written run passes unchanged.
    let mut x = fx();
    let (a, b) = (x.pickup(), x.pickup());
    let written = Pickup { weapon: a, pawn: a, controller: a, pickup: a, decoy: Some((b, b)), wrote: true };
    let e = step(&mut x.vm, "no-such-step", &written).unwrap();
    assert!(matches!(&e, VmError::Script { message, .. } if !message.contains(PARTIAL_WRITES)), "{e}");
}

/// A real `NotYet` driven through `make`'s fixture building and the gate's typed grading (review r3 of #452): once the
/// fixture preparation's first mutation succeeded (here a real write to a toy object), its `NotYet` is graded as a
/// not-yet whose text starts "stopped after partial writes", kind and record kept; one raised before any mutation is
/// graded as it is.
#[test]
fn a_not_yet_after_the_first_mutation_is_graded_as_partial_writes() {
    let mut x = fx();
    let class = x.vm.find_class("Engine.ToyPickup").unwrap();
    let target = x.pickup();
    for mutate in [true, false] {
        let mut prepare = |vm: &mut Vm, _: &str, _: bool, wrote: &mut bool| -> Result<ObjRef, VmError> {
            if mutate {
                let c = vm.space.heap.get(target)?.class;
                let p = vm.space.prop(c, "bTouchedGlobal").unwrap();
                vm.space.write(target, PropAddr::scalar(p), Value::Bool(true))?;
                *wrote = true;
            }
            Err(NotYet::not_read("the accuracy pool's setup", "NOTE.md").into())
        };
        let e = build(&mut x.vm, class, "a line", false, &mut prepare, &mut |_, _| Ok(())).unwrap_err();
        let (outcome, stop) = crate::cmd::slice_gate::classify_error("make", &e);
        assert_eq!(outcome, crate::cmd::slice_gate::Outcome::NotYet, "graded by variant");
        assert_eq!(stop.record, "NOTE.md");
        assert_eq!(stop.text.contains(PARTIAL_WRITES), mutate, "{}", stop.text);
        match e {
            VmError::NotYet(n) => assert_eq!(n.missing, Missing::NotRead),
            e => panic!("{e}"),
        }
    }
}

/// `look` in the targeting note's order (lane IMPL-PICKUP-TARGETING): the fixture's touched-slot check first (an
/// ordinary error when the decoy is not touched), then the player's HUD, which the headless world does not have: a
/// `NotYet { LayerAbsent }` naming it, marked "stopped after partial writes" in a run that has written.
#[test]
fn look_checks_the_touched_decoy_then_stops_at_the_missing_hud() {
    let mut x = fx();
    let c = x.vm.find_class("Engine.ToyController").unwrap();
    let controller = ow_vm::interp::instantiate_from_defaults(&mut x.vm, c).unwrap();
    let (aimed, decoy) = (x.pickup(), x.pickup());
    let mut run = Pickup { weapon: aimed, pawn: aimed, controller, pickup: aimed, decoy: Some((decoy, decoy)), wrote: false };
    let e = step(&mut x.vm, "look", &run).unwrap();
    assert!(matches!(&e, VmError::Script { message, .. } if message.contains("CurrentTouchedPickupable")), "{e}");
    let p = x.vm.space.prop(c, "CurrentTouchedPickupable").unwrap();
    x.vm.space.write(controller, PropAddr::scalar(p), Value::Object(Some(decoy))).unwrap();
    match step(&mut x.vm, "look", &run).unwrap() {
        VmError::NotYet(n) => {
            assert_eq!(n.missing, Missing::LayerAbsent("a HUD (the headless world spawns none)"));
            assert_eq!(n.record, ow_inventory::pickup_targeting::RECORD);
            assert!(!n.what.contains(PARTIAL_WRITES), "{n}");
        }
        e => panic!("{e}"),
    }
    run.wrote = true;
    match step(&mut x.vm, "look", &run).unwrap() {
        VmError::NotYet(n) => assert!(n.what.starts_with(PARTIAL_WRITES), "{n}"),
        e => panic!("{e}"),
    }
}

/// The globals preflight (lane IMPL-WILLOW-GLOBALS): a world without its globals object is refused with the world's
/// `NotYet { LayerAbsent }`, never a None, before the run writes anything.
#[test]
fn the_globals_preflight_refuses_a_world_without_globals() {
    let mut x = fx();
    ow_world::query::install(&mut x.vm).unwrap();
    let class = x.vm.find_class("Engine.ToyPickup").unwrap();
    match globals_preflight(&mut x.vm, class).unwrap_err() {
        VmError::NotYet(n) => assert!(matches!(n.missing, Missing::LayerAbsent(_)), "{n}"),
        e => panic!("{e}"),
    }
}

/// NATIVE_LINK67_PICKUP_GUN_INIT.md section 7 (lane IMPL-LINK67-PICKUP-GUN-INIT). A toy `prepare` that makes the gun
/// (a real write), and a toy init recording what it is given.
fn toy_gun(x: &mut Fx) -> (ObjRef, std::cell::RefCell<Vec<String>>) {
    let target = x.pickup();
    (target, std::cell::RefCell::new(Vec::new()))
}

/// Test 1, the order: each gun's init runs right after the gun is made, before anything of its drop (the toy gun has
/// no `Owner`, so the drop's first read is the error that follows the init).
#[test]
fn the_gun_s_init_runs_after_it_is_made_and_before_its_drop() {
    let mut x = fx();
    let class = x.vm.find_class("Engine.ToyPickup").unwrap();
    let (target, events) = toy_gun(&mut x);
    let mut prepare = |vm: &mut Vm, _: &str, owned: bool, wrote: &mut bool| -> Result<ObjRef, VmError> {
        let c = vm.space.heap.get(target)?.class;
        let p = vm.space.prop(c, "bTouchedGlobal").unwrap();
        vm.space.write(target, PropAddr::scalar(p), Value::Bool(true))?;
        *wrote = true;
        events.borrow_mut().push(format!("made owned={owned}"));
        Ok(target)
    };
    let mut init = |_: &mut Vm, w: ObjRef| -> Result<(), VmError> {
        events.borrow_mut().push(format!("init {}", w == target));
        Ok(())
    };
    let e = build(&mut x.vm, class, "a line", false, &mut prepare, &mut init).unwrap_err();
    assert_eq!(*events.borrow(), ["made owned=true", "init true"]);
    assert!(matches!(&e, VmError::Script { message, .. } if message.contains("declares no Owner")), "the drop comes after: {e}");
}

/// Test 2, the init's refusal: a `NotYet { NotRead }` after the gun was made is answered with the partial-writes
/// prefix, its kind kept, and the drop never starts.
#[test]
fn the_init_s_refusal_is_marked_and_nothing_is_dropped() {
    let mut x = fx();
    let class = x.vm.find_class("Engine.ToyPickup").unwrap();
    let (target, events) = toy_gun(&mut x);
    let mut prepare = |vm: &mut Vm, _: &str, _: bool, wrote: &mut bool| -> Result<ObjRef, VmError> {
        let c = vm.space.heap.get(target)?.class;
        let p = vm.space.prop(c, "bTouchedGlobal").unwrap();
        vm.space.write(target, PropAddr::scalar(p), Value::Bool(true))?;
        *wrote = true;
        Ok(target)
    };
    let mut init = |_: &mut Vm, _: ObjRef| -> Result<(), VmError> {
        events.borrow_mut().push("init".to_string());
        Err(NotYet::not_read("a toy init refusal", "TOY.md").into())
    };
    match build(&mut x.vm, class, "a line", true, &mut prepare, &mut init).unwrap_err() {
        VmError::NotYet(n) => {
            assert_eq!(n.missing, Missing::NotRead);
            assert_eq!(&*n.what, format!("{PARTIAL_WRITES}: a toy init refusal"));
            assert_eq!(n.record, "TOY.md");
        }
        e => panic!("{e}"),
    }
    assert_eq!(*events.borrow(), ["init"], "one init, the first gun's; no decoy and no drop after it");
}

/// Test 3, the decoy: link 7's second gun takes the same steps, made with no owner and then given the same init.
/// (Its place after the first gun's drop is `build`'s order; the toy world has no spawn, so the drop is not run here.)
#[test]
fn the_decoy_gets_the_same_init() {
    let mut x = fx();
    let (target, events) = toy_gun(&mut x);
    let mut prepare = |_: &mut Vm, _: &str, owned: bool, _: &mut bool| -> Result<ObjRef, VmError> {
        events.borrow_mut().push(format!("made owned={owned}"));
        Ok(target)
    };
    let mut init = |_: &mut Vm, w: ObjRef| -> Result<(), VmError> {
        events.borrow_mut().push(format!("init {}", w == target));
        Ok(())
    };
    let mut wrote = false;
    assert_eq!(gun(&mut x.vm, "a line", false, &mut prepare, &mut init, &mut wrote).unwrap(), target);
    assert_eq!(*events.borrow(), ["made owned=false", "init true"]);
}

/// Test 4, the mesh layer: a world that already has it gets no second install (its log line once), whichever gun asks.
#[test]
fn the_mesh_layer_is_installed_once_per_world() {
    let mut x = fx();
    let (a, b) = (x.pickup(), x.pickup());
    crate::cmd::inventory::equip_run::install_mesh_layer(&mut x.vm, a).unwrap();
    crate::cmd::inventory::equip_run::install_mesh_layer(&mut x.vm, b).unwrap();
    let line = crate::cmd::inventory::equip_run::LOD_LAYER_LINE;
    assert_eq!(x.vm.log.iter().filter(|l| l.as_str() == line).count(), 1, "{:?}", x.vm.log);
    assert!(x.vm.service::<ow_world::world::skeletal::SkeletalMeshes>().is_some());
}
