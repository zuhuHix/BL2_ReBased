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
    let mut t = ScriptToy::with_imports(&[
        "Class",
        "Function",
        "State",
        "NameProperty",
        "BoolProperty",
        "ObjectProperty",
        "ScriptStruct",
        "StructProperty",
        "FloatProperty",
    ]);
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
    // The fixture's gun and pawn (review of #761): what `build` reads and writes around the drop.
    let vector = ow_package::testkit::reflect::toy_struct(&mut t, object, "Vector", None);
    for axis in ["X", "Y", "Z"] {
        t.prop("Float", vector, axis, 0, 0);
    }
    let gun = t.class("ToyGun", object);
    t.prop("Object", gun, "Owner", 0, object);
    t.prop("Object", gun, "Instigator", 0, object);
    let pawn = t.class("ToyPawn", object);
    t.prop("Object", pawn, "Controller", 0, object);
    t.prop("Struct", pawn, "Location", 0, vector);
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

/// NATIVE_LINK67_PICKUP_GUN_INIT.md section 7 (lane IMPL-LINK67-PICKUP-GUN-INIT), through the whole of `build` with a
/// toy drop (review of #761): Maya's toy pawn and controller, her gun and the decoy's, each step recorded in order.
struct Gunshop {
    class: ow_object::ids::ClassId,
    owned: ObjRef,
    decoy: ObjRef,
    pawn: ObjRef,
    events: std::cell::RefCell<Vec<String>>,
}

fn gunshop(x: &mut Fx) -> Gunshop {
    let make = |x: &mut Fx, path: &str| {
        let c = x.vm.find_class(path).unwrap();
        ow_vm::interp::instantiate_from_defaults(&mut x.vm, c).unwrap()
    };
    let (owned, decoy, pawn, controller) =
        (make(x, "Engine.ToyGun"), make(x, "Engine.ToyGun"), make(x, "Engine.ToyPawn"), make(x, "Engine.ToyController"));
    let set = |x: &mut Fx, o: ObjRef, name: &str, v: Value| {
        let class = x.vm.space.heap.get(o).unwrap().class;
        let p = x.vm.space.prop(class, name).unwrap();
        x.vm.space.write(o, PropAddr::scalar(p), v).unwrap();
    };
    set(x, owned, "Owner", Value::Object(Some(pawn)));
    set(x, pawn, "Controller", Value::Object(Some(controller)));
    let class = x.vm.find_class("Engine.ToyPickup").unwrap();
    Gunshop { class, owned, decoy, pawn, events: std::cell::RefCell::new(Vec::new()) }
}

impl Gunshop {
    fn name(&self, o: ObjRef) -> &'static str {
        if o == self.owned { "gun" } else if o == self.decoy { "decoy" } else { "?" }
    }

    /// Runs `build_dropping` with toy steps that record themselves; `init` answers each gun's init.
    fn run(
        &self,
        x: &mut Fx,
        decoy: bool,
        init_answer: &dyn Fn(ObjRef) -> Result<(), VmError>,
    ) -> Result<Pickup, VmError> {
        let mut prepare = |_: &mut Vm, _: &str, owned: bool, wrote: &mut bool| -> Result<ObjRef, VmError> {
            *wrote = true;
            let g = if owned { self.owned } else { self.decoy };
            self.events.borrow_mut().push(format!("make {}", self.name(g)));
            Ok(g)
        };
        let mut init = |_: &mut Vm, g: ObjRef| -> Result<(), VmError> {
            self.events.borrow_mut().push(format!("init {}", self.name(g)));
            init_answer(g)
        };
        let mut drop = |vm: &mut Vm, class: ow_object::ids::ClassId, g: ObjRef, _: ObjRef, at: [f32; 3]| -> Result<ObjRef, VmError> {
            self.events.borrow_mut().push(format!("drop {} at {at:?}", self.name(g)));
            ow_vm::interp::instantiate_from_defaults(vm, class)
        };
        build_dropping(&mut x.vm, self.class, "a line", decoy, &mut prepare, &mut init, &mut drop)
    }
}

/// Tests 1 and 3, the order through `build`: each gun is made, then given its init, then dropped; link 7's decoy
/// takes the same steps after the first gun's drop.
#[test]
fn each_gun_is_made_then_initialised_then_dropped_the_decoy_after_the_first() {
    let mut x = fx();
    let shop = gunshop(&mut x);
    let p = shop.run(&mut x, true, &|_| Ok(())).unwrap();
    assert_eq!(p.pawn, shop.pawn);
    assert_eq!(
        *shop.events.borrow(),
        [
            "make gun",
            "init gun",
            "drop gun at [150.0, 0.0, 0.0]",
            "make decoy",
            "init decoy",
            "drop decoy at [0.0, 45.0, 0.0]",
        ]
    );
}

/// Test 2, the init's refusal: a `NotYet { NotRead }` after the gun was made is answered with the partial-writes
/// prefix, its kind and record kept, and nothing is dropped.
#[test]
fn the_init_s_refusal_is_marked_and_nothing_is_dropped() {
    let mut x = fx();
    let shop = gunshop(&mut x);
    let refuse = |_: ObjRef| -> Result<(), VmError> { Err(NotYet::not_read("a toy init refusal", "TOY.md").into()) };
    match shop.run(&mut x, true, &refuse).unwrap_err() {
        VmError::NotYet(n) => {
            assert_eq!(n.missing, Missing::NotRead);
            assert_eq!(&*n.what, format!("{PARTIAL_WRITES}: a toy init refusal"));
            assert_eq!(n.record, "TOY.md");
        }
        e => panic!("{e}"),
    }
    assert_eq!(*shop.events.borrow(), ["make gun", "init gun"], "no drop, and no decoy");
    // The decoy's init refusing: the first gun is dropped, the decoy is not.
    let mut x = fx();
    let shop = gunshop(&mut x);
    let decoy = shop.decoy;
    let refuse_decoy = move |g: ObjRef| -> Result<(), VmError> {
        if g == decoy { Err(NotYet::not_read("the decoy's toy init", "TOY.md").into()) } else { Ok(()) }
    };
    assert!(shop.run(&mut x, true, &refuse_decoy).is_err());
    assert_eq!(*shop.events.borrow(), ["make gun", "init gun", "drop gun at [150.0, 0.0, 0.0]", "make decoy", "init decoy"]);
}

/// Review of #761: `make` hands each gun `equip_run::pickup_gun_init`.
#[test]
fn make_s_init_is_the_pickup_gun_init() {
    let real: fn(&mut Vm, ObjRef) -> Result<(), VmError> = crate::cmd::inventory::equip_run::pickup_gun_init;
    assert!(std::ptr::fn_addr_eq(MAKE_INIT, real));
}

/// Review of #761: the order inside `pickup_gun_init`: link 8's five preloads, then the mesh layer, then the
/// threshold preload, then the init phase, each the function link 8's driver calls.
#[test]
fn the_pickup_gun_init_runs_its_steps_in_link_8_s_order() {
    use crate::cmd::inventory::equip_run as e;
    let expected: [(&str, e::GunInitStep); 8] = [
        ("muzzle flashes", e::preload_muzzle_flashes),
        ("gestalt mesh", e::preload_gestalt_mesh),
        ("gestalt skeletal mesh", e::preload_gestalt_skeletal_mesh),
        ("material part material", e::preload_material_part_material),
        ("material classes", e::preload_material_classes),
        ("mesh layer", e::install_mesh_layer),
        ("threshold attributes", e::preload_threshold_attributes),
        ("init", e::init_phase),
    ];
    for ((name, step), (want_name, want)) in e::GUN_INIT_STEPS.iter().zip(expected.iter()) {
        assert_eq!(name, want_name);
        assert!(std::ptr::fn_addr_eq(*step, *want), "{name}");
    }
    assert_eq!(e::GUN_INIT_STEPS.len(), expected.len());
}

// Test 4 (the mesh layer installed once per world) is `equip_run_gestalt_tests::
// the_mesh_layer_is_installed_once_and_its_answer_logged`, which `pickup_gun_init` calls through `install_mesh_layer`.
