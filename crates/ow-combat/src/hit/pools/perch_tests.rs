//! Tests of `WillowAIPawn.DropCombatPerch` in Off (lane IMPL-DROPCOMBATPERCH, NATIVE_DROP_COMBAT_PERCH.md section 3)
//! on invented packages with the game's class and field names (AGENTS.md rule 9). Every value is invented. AI-assisted
//! (Claude Code).

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::registry::AttributeSystem;
use ow_core::{Missing, ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_package::testkit::script::{Asm, FunctionSpec, Param, ScriptToy};
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;

use super::*;

static NEXT: AtomicU32 = AtomicU32::new(0);
const NATIVE_FLAGS: u32 = 0x400 | 0x20000;
const SCRIPT_FLAGS: u32 = 0x1 | 0x2 | 0x20000;

/// How `WillowAIPawn` declares the perch fields in the fixture.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Shape {
    /// `Role` byte, `PerchProjectile` object, `bPerchAttached` bool.
    Good,
    /// No `PerchProjectile`.
    NoProjectile,
    /// `bPerchAttached` an int.
    IntFlag,
    /// No `Role`.
    NoRole,
    /// `Role` an int.
    IntRole,
}

/// `WillowGame.upk` (invented): `WillowPawn` (`Role`, an object field `ToyLink`), `WillowAIPawn` (the perch fields as
/// `shape` says and the native `DropCombatPerch`), a script child `ToyAIPawnChild`, an invented child
/// `ToyNativeAIPawn` (listed as native by the test that needs it), `ToyPlainPawn` (a `WillowPawn` that is not an AI
/// pawn) and `ProjectileDefinition`.
fn willow(shape: Shape) -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class", "Function", "BoolProperty", "ByteProperty", "IntProperty", "ObjectProperty"]);
    let object = t.class("Object", 0);
    let definition = t.class("ProjectileDefinition", object);
    let pawn = t.class("WillowPawn", object);
    match shape {
        Shape::NoRole => {}
        Shape::IntRole => {
            t.prop("Int", pawn, "Role", 0, 0);
        }
        _ => {
            t.prop("Byte", pawn, "Role", 0, 0);
        }
    }
    t.prop("Object", pawn, "ToyLink", 0, object);
    let ai = t.class("WillowAIPawn", pawn);
    // Declared flag first: the class's own fields take slots in reverse declaration order, so `bPerchAttached` gets
    // the last slot (the short-slot test cuts it off).
    t.prop(if shape == Shape::IntFlag { "Int" } else { "Bool" }, ai, "bPerchAttached", 0, 0);
    if shape != Shape::NoProjectile {
        t.prop("Object", ai, "PerchProjectile", 0, definition);
    }
    let spec = FunctionSpec::new(NATIVE_FLAGS, "DropCombatPerch");
    t.function(ai, "DropCombatPerch", &[], None, &[], &spec, |_| None);
    let drop = t.p.name("DropCombatPerch");
    // `Died(Killer)`, a script stand-in for the installed one's call: it calls `DropCombatPerch` on itself by name, a
    // virtual call as the installed script makes it (review r1 of #626).
    let spec = FunctionSpec::new(SCRIPT_FLAGS, "Died");
    let killer = Param { type_ref: object, ..Param::new("Object", "Killer") };
    t.function(ai, "Died", &[killer], None, &[], &spec, |_| {
        let mut a = Asm::new();
        a.stmt().raw(&[0x1B]).i32(drop).i32(0).raw(&[0x16]);
        a.stmt().return_nothing();
        a.end();
        Some(a)
    });
    t.class("ToyAIPawnChild", ai);
    t.class("ToyNativeAIPawn", ai);
    // The two listed native descendants (D2), and an invented child of each.
    let crane = t.class("WillowAICranePawn", ai);
    t.class("ToyCraneChild", crane);
    let turret = t.class("DeployableTurretActor", ai);
    t.class("ToyTurretChild", turret);
    t.class("ToyPlainPawn", pawn);
    t.p.build()
}

/// `GD_ToyPerch.upk` (invented): `Holder`, a `WillowPawn` whose `ToyLink` names `Peer`, a `WillowAIPawn` export, so
/// reading the loaded holder gives `Peer` as an unloaded store handle.
fn pawns() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class"]);
    let willow = t.p.add_import("Core", "Package", 0, "WillowGame");
    let pawn = t.p.add_import("Core", "Class", willow, "WillowPawn");
    let ai = t.p.add_import("Core", "Class", willow, "WillowAIPawn");
    let mut body = ow_package::testkit::ints(&[0]);
    body.extend_from_slice(&t.none);
    let peer = t.p.add_export(ai, "Peer", body);
    let mut holder = ow_package::testkit::ints(&[0]);
    holder.extend(t.p.tag("ToyLink", "ObjectProperty", &ow_package::testkit::ints(&[peer])));
    holder.extend_from_slice(&t.none);
    t.p.add_export(pawn, "Holder", holder);
    t.p.build()
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

fn fx_with(mode: ParityMode, shape: Shape) -> Fx {
    let dir = std::env::temp_dir().join(format!("ow-combat-perch-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let (w, p) = (willow(shape), pawns());
    let files: [(&str, &[u8]); 2] = [("WillowGame.upk", &w), ("GD_ToyPerch.upk", &p)];
    let defs = DefStore::from_synthetic(&dir, &files, IdentityPolicy::FirstInScanOrder, mode).unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), mode);
    // The classes are loaded by the fixture, so no answer below is the first to load one.
    for c in ["ProjectileDefinition", "WillowPawn", "WillowAIPawn", "ToyAIPawnChild", "ToyNativeAIPawn", "ToyPlainPawn"] {
        vm.find_class(&format!("WillowGame.{c}")).unwrap();
    }
    Fx { vm, dir }
}

fn fx() -> Fx {
    fx_with(ParityMode::Off, Shape::Good)
}

impl Fx {
    fn make(&mut self, class: &str) -> ObjRef {
        let c = self.vm.find_class(&format!("WillowGame.{class}")).unwrap();
        self.vm.space.alloc(c, "ToyPawn", None, Origin::Runtime).unwrap()
    }

    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let c = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(c, name).unwrap();
        self.vm.space.write(o, PropAddr::scalar(p), v).unwrap();
    }

    /// A pawn of `class` with `Role`, `PerchProjectile` (a fresh definition when set) and `bPerchAttached` as given.
    fn pawn(&mut self, class: &str, role: u8, projectile: bool, attached: bool) -> ObjRef {
        let pawn = self.make(class);
        self.set(pawn, "Role", Value::Byte(role));
        if projectile {
            let d = self.make("ProjectileDefinition");
            self.set(pawn, "PerchProjectile", Value::Object(Some(d)));
        }
        self.set(pawn, "bPerchAttached", Value::Bool(attached));
        pawn
    }

    /// Cuts `pawn`'s slot vector just before `bPerchAttached`'s slot, after checking that `Role`'s and
    /// `PerchProjectile`'s slots come before it, so only the flag (and what follows it) is missing.
    fn short_of_attached(&mut self, pawn: ObjRef) {
        let space = &self.vm.space;
        let class = space.heap.get(pawn).unwrap().class;
        let slot = |name: &str| space.classes.resolve(class, PropAddr::scalar(space.prop(class, name).unwrap())).unwrap();
        let attached = slot("bPerchAttached");
        assert!(slot("Role") < attached && slot("PerchProjectile") < attached, "the fixture's slot order");
        self.vm.space.heap.get_mut(pawn).unwrap().slots.truncate(attached);
    }

    /// #518's three counters.
    fn counters(&self) -> (u64, u64, u64) {
        (self.vm.space.heap.committed_writes(), (self.vm.defs.load_transitions() + self.vm.class_code_loads()), self.vm.rng.draws())
    }

    /// The answer for `pawn` must be nothing, with the counters unchanged.
    fn nothing(&self, pawn: ObjRef, label: &str) {
        let before = self.counters();
        drop_combat_perch(&self.vm, pawn).unwrap_or_else(|e| panic!("{label}: {e}"));
        assert_eq!(self.counters(), before, "{label}: nothing written, loaded or drawn");
    }

    /// The answer for `pawn` must be `NotYet { NotRead }` containing `what`, with the counters unchanged.
    fn refuses_with(&self, pawn: ObjRef, native: &[&str], what: &str) {
        let before = self.counters();
        match drop_combat_perch_with(&self.vm, pawn, native) {
            Err(VmError::NotYet(n)) => {
                assert_eq!((n.missing, n.record), (Missing::NotRead, RECORD_PERCH), "{n}");
                assert!(n.what.contains(what), "{n}");
            }
            other => panic!("expected the NotYet {what:?}, got {other:?}"),
        }
        assert_eq!(self.counters(), before, "a refusal writes, loads and draws nothing");
    }

    fn refuses(&self, pawn: ObjRef, what: &str) {
        self.refuses_with(pawn, &NATIVE_AI_PAWN_DESCENDANTS, what);
    }
}

fn script_error(r: Result<(), VmError>, wanted: &str) {
    match r {
        Err(VmError::Script { message, .. }) => assert!(message.contains(wanted), "{message}"),
        other => panic!("expected a script error naming {wanted}, got {other:?}"),
    }
}

/// Test 1, the slice shape: `Role` 3, `PerchProjectile` None and `bPerchAttached` clear give nothing, through the bound
/// native too.
#[test]
fn the_slice_shape_does_nothing() {
    let mut x = fx();
    register(&mut x.vm).unwrap();
    let pawn = x.pawn("WillowAIPawn", 3, false, false);
    x.nothing(pawn, "slice");
    let f = x.vm.find_function(DROP_COMBAT_PERCH).unwrap();
    let before = x.counters();
    assert_eq!(x.vm.call(f, Some(pawn), vec![]).unwrap().ret, Value::None);
    assert_eq!(x.counters(), before);
}

/// Test 2: each guard failing alone answers nothing.
#[test]
fn each_failing_guard_alone_does_nothing() {
    let mut x = fx();
    for (label, role, projectile, attached) in
        [("Role 2", 2, true, true), ("no projectile", 3, false, true), ("flag clear", 3, true, false)]
    {
        let pawn = x.pawn("WillowAIPawn", role, projectile, attached);
        x.nothing(pawn, label);
    }
}

/// Test 3: all three holding is D3, nothing written.
#[test]
fn the_drop_itself_is_d3() {
    let mut x = fx();
    let pawn = x.pawn("WillowAIPawn", 3, true, true);
    x.refuses(pawn, PERCH_DROP);
}

/// Test 4: a script-only child takes the rule (nothing, then D3); an invented native child is D2.
#[test]
fn a_script_child_takes_the_rule_and_a_native_child_is_d2() {
    let mut x = fx();
    let child = x.pawn("ToyAIPawnChild", 3, false, false);
    x.nothing(child, "script child");
    let child = x.pawn("ToyAIPawnChild", 3, true, true);
    x.refuses(child, PERCH_DROP);
    let native = x.pawn("ToyNativeAIPawn", 3, false, false);
    x.refuses_with(native, &["WillowGame.ToyNativeAIPawn"], "the perch rule of WillowGame.ToyNativeAIPawn");
    assert_eq!(
        NATIVE_AI_PAWN_DESCENDANTS,
        ["WillowGame.WillowAICranePawn", "WillowGame.DeployableTurretActor"],
        "the list carried with the binding"
    );
}

/// Test 9: D2 for each listed class, `WillowAICranePawn` and `DeployableTurretActor`, and for an invented child of
/// each, through the list carried with the binding; a perched one refuses D2 before the guard is read.
#[test]
fn the_listed_native_descendants_are_d2() {
    let mut x = fx();
    for class in ["WillowAICranePawn", "ToyCraneChild", "DeployableTurretActor", "ToyTurretChild"] {
        for perched in [false, true] {
            let pawn = x.pawn(class, 3, perched, perched);
            x.refuses(pawn, &format!("the perch rule of WillowGame.{class} (its virtual was not read)"));
        }
    }
}

/// Test 5: a receiver outside `WillowAIPawn` (a plain `WillowPawn` subclass) is a `VmError::Script`; so is a call with
/// no receiver.
#[test]
fn a_receiver_outside_willow_ai_pawn_is_a_script_error() {
    let mut x = fx();
    let plain = x.make("ToyPlainPawn");
    script_error(drop_combat_perch(&x.vm, plain), "is not a WillowAIPawn");
    register(&mut x.vm).unwrap();
    let f = x.vm.find_function(DROP_COMBAT_PERCH).unwrap();
    assert!(matches!(x.vm.call(f, None, vec![]), Err(VmError::Script { .. })));
}

/// Test 6: an unloaded handle and a slotless stand-in are D1, each with its own text, nothing loaded.
#[test]
fn an_unresident_pawn_is_d1() {
    let mut x = fx();
    let path = ObjectPath::parse("GD_ToyPerch.Holder").unwrap();
    let holder = x.vm.defs.find(&mut x.vm.space, &path).unwrap().unwrap();
    let class = x.vm.space.heap.get(holder).unwrap().class;
    let p = x.vm.space.prop(class, "ToyLink").unwrap();
    let Value::Object(Some(peer)) = x.vm.space.read(holder, PropAddr::scalar(p)).unwrap().clone() else {
        panic!("the holder names its peer")
    };
    assert!(x.vm.defs.is_unloaded(&x.vm.space, peer));
    x.refuses(peer, "the pawn WillowGame.WillowAIPawn Peer is an unloaded handle (a load is a write)");
    assert!(x.vm.defs.is_unloaded(&x.vm.space, peer), "still unloaded");
    let ai = x.vm.find_class("WillowGame.WillowAIPawn").unwrap();
    let stand_in = x.vm.space.alloc_stand_in(ai, "ToyStandIn", Origin::Resource { package: 0, index: -1 }).unwrap();
    x.refuses(stand_in, "the pawn WillowGame.WillowAIPawn ToyStandIn is a stand-in with no properties (an object the store cannot load)");
}

/// Test 7: a missing `PerchProjectile`, a non-bool `bPerchAttached`, and a missing or non-byte `Role` are a
/// `VmError::Script` each.
#[test]
fn malformed_perch_fields_are_script_errors() {
    for (shape, wanted) in [
        (Shape::NoProjectile, "PerchProjectile is not declared"),
        (Shape::IntFlag, "bPerchAttached is not a bool"),
        (Shape::NoRole, "Role is not declared"),
        (Shape::IntRole, "Role is not a byte"),
    ] {
        let mut x = fx_with(ParityMode::Off, shape);
        let pawn = x.make("WillowAIPawn");
        let before = x.counters();
        script_error(drop_combat_perch(&x.vm, pawn), wanted);
        assert_eq!(x.counters(), before, "{shape:?}");
    }
}

/// Test 8: a key bound first keeps its binding, and a keep-existing bind after the pack keeps the pack's; under
/// Reference nothing new is bound.
#[test]
fn both_registration_orders_and_reference() {
    let mut x = fx();
    let f = x.vm.find_function(DROP_COMBAT_PERCH).unwrap();
    let key = x.vm.function(f).unwrap().native_key.clone().unwrap();
    x.vm.register_native(&key, Rc::new(|_c: &mut NativeCall<'_>| Ok(Value::Int(7)))).unwrap();
    register(&mut x.vm).unwrap();
    let pawn = x.pawn("WillowAIPawn", 3, true, true);
    assert_eq!(x.vm.call(f, Some(pawn), vec![]).unwrap().ret, Value::Int(7), "kept");

    let mut x = fx();
    register(&mut x.vm).unwrap();
    super::super::bind_fn(&mut x.vm, DROP_COMBAT_PERCH, |_c| Ok(Value::Int(7))).unwrap();
    let pawn = x.pawn("WillowAIPawn", 3, true, true);
    let f = x.vm.find_function(DROP_COMBAT_PERCH).unwrap();
    assert!(matches!(x.vm.call(f, Some(pawn), vec![]), Err(VmError::NotYet(ref n)) if n.what == PERCH_DROP), "the rule kept");

    let mut x = fx_with(ParityMode::Reference, Shape::Good);
    register(&mut x.vm).unwrap();
    let f = x.vm.find_function(DROP_COMBAT_PERCH).unwrap();
    assert!(!x.vm.is_bound(f), "unbound under Reference");
}

/// Test 10, the death path: `Died` reaches the rule in Off (review r1 of #620): `death.rs` binds `DropCombatPerch` as a presentation no-op under
/// Reference only, so in Off a perched pawn's `Died` reaches D3 and the slice shape's `Died` does nothing, whichever of
/// the two packs registers first. Under Reference the no-op answers, perched or not.
#[test]
fn died_reaches_the_rule_in_off_and_the_no_op_in_reference() {
    let died = "WillowGame.WillowAIPawn.Died";
    for death_first in [false, true] {
        let mut x = fx();
        if death_first {
            crate::death::register(&mut x.vm).unwrap();
            register(&mut x.vm).unwrap();
        } else {
            register(&mut x.vm).unwrap();
            crate::death::register(&mut x.vm).unwrap();
        }
        let f = x.vm.find_function(died).unwrap();
        let perched = x.pawn("WillowAIPawn", 3, true, true);
        let before = x.counters();
        match x.vm.call(f, Some(perched), vec![Value::Object(None)]) {
            Err(VmError::NotYet(n)) => assert!(n.what.contains(PERCH_DROP), "death first {death_first}: {n}"),
            other => panic!("death first {death_first}: expected D3, got {other:?}"),
        }
        assert_eq!(x.counters(), before, "death first {death_first}: #518's counters");
        let slice = x.pawn("WillowAIPawn", 3, false, false);
        x.vm.call(f, Some(slice), vec![Value::Object(None)]).unwrap();
    }
    let mut x = fx_with(ParityMode::Reference, Shape::Good);
    crate::death::register(&mut x.vm).unwrap();
    register(&mut x.vm).unwrap();
    let f = x.vm.find_function(died).unwrap();
    let perched = x.pawn("WillowAIPawn", 3, true, true);
    x.vm.call(f, Some(perched), vec![Value::Object(None)]).unwrap();
}

/// Review r1 of #626: the declarations are checked first, then the slots are read in the guard's order. A pawn with
/// `Role` 2 whose slot vector is short of `bPerchAttached` answers nothing, because the first guard decides and the
/// missing slot is never read; with `Role` 3 and a set projectile the missing slot is reached and is a heap error.
#[test]
fn the_first_failing_guard_answers_before_a_later_slot_is_read() {
    let mut x = fx();
    for (role, projectile) in [(2, true), (3, false)] {
        let pawn = x.pawn("WillowAIPawn", role, projectile, true);
        x.short_of_attached(pawn);
        x.nothing(pawn, "short of bPerchAttached");
    }
    let pawn = x.pawn("WillowAIPawn", 3, true, true);
    x.short_of_attached(pawn);
    assert!(matches!(drop_combat_perch(&x.vm, pawn), Err(VmError::Heap(_))), "the missing slot, once reached");
}

/// Review r1 of #626: a freed receiver is an invalid receiver, a `VmError::Script` naming the native, with #518's
/// counters unchanged.
#[test]
fn a_freed_receiver_is_a_script_error() {
    let mut x = fx();
    let pawn = x.pawn("WillowAIPawn", 3, true, true);
    x.vm.space.free(pawn).unwrap();
    let before = x.counters();
    script_error(drop_combat_perch(&x.vm, pawn), "WillowAIPawn.DropCombatPerch: the receiver is not a live object");
    assert_eq!(x.counters(), before);
}
