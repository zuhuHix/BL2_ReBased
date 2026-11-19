//! Tests of the headless pickup driver's own rules (lane IMPL-PICKUP, review r2 of #452) on an invented package built
//! with `ow_package::testkit` (AGENTS.md rule 9): `ToyPickup`, a pickup class whose global `Touch` and whose `Pickup`
//! state's `Touch` set different flags, and the stock `GotoState`.

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
        let e = build(&mut x.vm, class, "a line", false, &mut prepare).unwrap_err();
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
