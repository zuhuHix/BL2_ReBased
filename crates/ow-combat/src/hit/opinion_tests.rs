//! Tests of the opinion and friendliness rules (NATIVE_IS_FRIENDLY.md; lane IMPL-IS-FRIENDLY, decision 2026-11-19-60)
//! on two invented packages built with `ow_package::testkit` (AGENTS.md rule 9): `Engine` (actor, controller, pawn,
//! `PawnAllegiance` with its opinion lists, a toy actor that is none of the three) and `WillowGame` (`WillowPawn` with
//! its charm fields and its own `GetOpinion`). Every native is declared by the packages and reached by its path.

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::{Missing, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_package::testkit::reflect::{toy_array_of, toy_struct};
use ow_package::testkit::script::{Asm, FunctionSpec, Param, ScriptToy};
use ow_vm::api::{NativePack, StubPolicy, Vm};
use ow_vm::error::VmError;

use super::*;

const NATIVE: u32 = 0x400 | 0x20000;
const SCRIPT: u32 = 0x1 | 0x2 | 0x20000;
static NEXT: AtomicU32 = AtomicU32::new(0);

const IMPORTS: [&str; 8] = [
    "Class",
    "Function",
    "ScriptStruct",
    "BoolProperty",
    "ByteProperty",
    "ObjectProperty",
    "ArrayProperty",
    "StructProperty",
];

fn native(t: &mut ScriptToy, owner: i32, name: &str, other: i32, result: &str) {
    t.function(
        owner,
        name,
        &[Param { type_ref: other, ..Param::new("Object", "Other") }],
        Some((result, 0)),
        &[],
        &FunctionSpec::new(NATIVE, name),
        |_| None,
    );
}

/// `Engine`: `Object`, `Actor`, `Controller` (`Pawn`), `Pawn` (`Allegiance`, the natives `IsFriendly`, `GetOpinion`,
/// `IsEnemy`), `PawnAllegiance` with `AllegianceOpinion` entries, and `ToyThing`, an actor that is none of the three.
fn engine() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let object = t.class("Object", 0);
    let actor = t.class("Actor", object);
    let allegiance = t.class("PawnAllegiance", object);
    let entry = toy_struct(&mut t, allegiance, "AllegianceOpinion", None);
    t.prop("Object", entry, "Allegiance", 0, allegiance);
    t.prop("Byte", entry, "Opinion", 0, 0);
    toy_array_of(&mut t, allegiance, "MyOpinions", "Struct", entry);
    toy_array_of(&mut t, allegiance, "OtherOpinions", "Struct", entry);
    for b in ["SelfOpinion", "DefaultOpinion", "ForcedOtherOpinion"] {
        t.prop("Byte", allegiance, b, 0, 0);
    }
    t.prop("Bool", allegiance, "bForceAllOtherOpinions", 0, 0);
    let pawn = t.class("Pawn", actor);
    t.prop("Object", pawn, "Allegiance", 0, allegiance);
    let controller = t.class("Controller", actor);
    t.prop("Object", controller, "Pawn", 0, pawn);
    native(&mut t, pawn, "IsFriendly", object, "Bool");
    native(&mut t, pawn, "GetOpinion", object, "Byte");
    native(&mut t, pawn, "IsEnemy", object, "Bool");
    t.class("ToyThing", actor);
    t.build()
}

/// `WillowGame`: `WillowPawn` (`CharmedChild`, `CharmedParent`, its own `GetOpinion`) and a `WillowAIPawn`.
fn willow_game() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&IMPORTS);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let object = t.p.add_import("Core", "Class", engine, "Object");
    let pawn = t.p.add_import("Core", "Class", engine, "Pawn");
    let willow = t.class("WillowPawn", pawn);
    t.prop("Object", willow, "CharmedChild", 0, willow);
    t.prop("Object", willow, "CharmedParent", 0, willow);
    native(&mut t, willow, "GetOpinion", object, "Byte");
    t.class("WillowAIPawn", willow);
    // Overrides that resolve but cannot run (review r3 of #449): natives nobody binds, and a script that does not
    // prepare (an invented token the decoder refuses).
    let unbound = t.class("ToyUnboundPawn", willow);
    native(&mut t, unbound, "GetOpinion", object, "Byte");
    native(&mut t, unbound, "IsFriendly", object, "Bool");
    let broken = t.class("ToyBrokenPawn", willow);
    t.function(
        broken,
        "GetOpinion",
        &[Param { type_ref: object, ..Param::new("Object", "Other") }],
        Some(("Byte", 0)),
        &[],
        &FunctionSpec::new(SCRIPT, "GetOpinion"),
        |_| {
            let mut a = Asm::new();
            a.stmt().raw(&[0x5E]);
            a.end();
            Some(a)
        },
    );
    // A pawn class with a CharmedChild and no CharmedParent (invented), for the read order of the charm steps.
    let child_only = t.class("ToyChildOnly", pawn);
    t.prop("Object", child_only, "CharmedChild", 0, pawn);
    // A pawn whose GetOpinion is a script override: it answers Friendly of everyone (invented).
    let fond = t.class("ToyFondPawn", willow);
    t.function(
        fond,
        "GetOpinion",
        &[Param { type_ref: object, ..Param::new("Object", "Other") }],
        Some(("Byte", 0)),
        &[],
        &FunctionSpec::new(SCRIPT, "GetOpinion"),
        |_| {
            let mut a = Asm::new();
            a.stmt().raw(&[0x04]).byte_const(2);
            a.end();
            Some(a)
        },
    );
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

/// A VM over the two packages in `mode`; `combat` also registers the combat hit pack (after this module's natives,
/// which it registers itself).
fn fx(mode: ParityMode, combat: bool) -> Fx {
    let dir = std::env::temp_dir().join(format!(
        "ow-combat-opinion-{}-{}",
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
    if combat {
        crate::hit::Pack.register(&mut vm).unwrap();
    } else {
        register(&mut vm).unwrap();
    }
    Fx { vm, dir }
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

    fn get(&self, o: ObjRef, name: &str) -> Value {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap();
        self.vm.space.read(o, PropAddr::scalar(p)).unwrap().clone()
    }

    fn call(&mut self, path: &str, me: ObjRef, other: Option<ObjRef>) -> Result<Value, VmError> {
        let f = self.vm.find_function(path)?;
        Ok(self.vm.call(f, Some(me), vec![Value::Object(other)])?.ret)
    }

    /// `Pawn.GetOpinion` (the generic rule) as a byte.
    fn generic(&mut self, me: ObjRef, other: Option<ObjRef>) -> Result<Value, VmError> {
        self.call("Engine.Pawn.GetOpinion", me, other)
    }

    fn willow(&mut self, me: ObjRef, other: Option<ObjRef>) -> Result<Value, VmError> {
        self.call("WillowGame.WillowPawn.GetOpinion", me, other)
    }

    fn friendly(&mut self, me: ObjRef, other: Option<ObjRef>) -> Result<Value, VmError> {
        self.call("Engine.Pawn.IsFriendly", me, other)
    }

    /// An allegiance with these three opinions.
    fn allegiance(&mut self, own: u8, default: u8) -> ObjRef {
        let a = self.obj("Engine.PawnAllegiance");
        self.set(a, "SelfOpinion", Value::Byte(own));
        self.set(a, "DefaultOpinion", Value::Byte(default));
        a
    }

    /// One `AllegianceOpinion` entry naming `allegiance` with `opinion`.
    fn entry(&mut self, holder: ObjRef, allegiance: ObjRef, opinion: u8) -> Value {
        let class = self.vm.space.heap.get(holder).unwrap().class;
        let p = self.vm.space.prop(class, "MyOpinions").unwrap();
        let ty = match &self.vm.space.classes.decl(p).unwrap().kind {
            ow_object::layout::PropKind::Array(inner) => match **inner {
                ow_object::layout::PropKind::Struct(s) => s,
                _ => unreachable!(),
            },
            _ => unreachable!(),
        };
        let mut v = self.vm.space.classes.struct_defaults(ty).unwrap();
        assert!(crate::hit::access::set_field(&self.vm, &mut v, "Allegiance", Value::Object(Some(allegiance))));
        assert!(crate::hit::access::set_field(&self.vm, &mut v, "Opinion", Value::Byte(opinion)));
        v
    }

    fn willow_pawn(&mut self, allegiance: Option<ObjRef>) -> ObjRef {
        let p = self.obj("WillowGame.WillowAIPawn");
        self.set(p, "Allegiance", Value::Object(allegiance));
        p
    }
}

fn not_read(r: Result<Value, VmError>) -> String {
    match r {
        Err(VmError::NotYet(n)) => {
            assert_eq!(n.missing, Missing::NotRead, "{n}");
            n.to_string()
        }
        other => panic!("expected NotYet, got {other:?}"),
    }
}

const E: Value = Value::Byte(ENEMY);
const N: Value = Value::Byte(NEUTRAL);
const F: Value = Value::Byte(FRIENDLY);

/// Section 3, step by step: no own allegiance; no other side (None, a controller even with a pawn); the same object;
/// `MyOpinions`; `OtherOpinions`; the forced opinion; the default. A `PawnAllegiance` is its own B.
#[test]
fn the_generic_rule_in_order() {
    let mut x = fx(ParityMode::Off, false);
    let a = x.allegiance(FRIENDLY, NEUTRAL);
    let me = x.willow_pawn(None);
    let thing = x.obj("Engine.ToyThing");
    assert_eq!(x.generic(me, Some(thing)).unwrap(), N, "no own allegiance: Neutral before the other side is read");
    x.set(me, "Allegiance", Value::Object(Some(a)));
    assert_eq!(x.generic(me, None).unwrap(), N, "None");
    let friend = x.willow_pawn(Some(a));
    let controller = x.obj("Engine.Controller");
    x.set(controller, "Pawn", Value::Object(Some(friend)));
    assert_eq!(x.generic(me, Some(controller)).unwrap(), N, "a controller is not read through its pawn");
    assert_eq!(x.generic(me, Some(friend)).unwrap(), F, "the same allegiance: SelfOpinion");
    assert_eq!(x.generic(me, Some(me)).unwrap(), F, "itself, with an allegiance");
    assert_eq!(x.generic(me, Some(a)).unwrap(), F, "a PawnAllegiance is its own B");
    let b = x.allegiance(FRIENDLY, ENEMY);
    let other = x.willow_pawn(Some(b));
    assert_eq!(x.generic(me, Some(other)).unwrap(), N, "A's DefaultOpinion");
    let mine = x.entry(a, b, ENEMY);
    let theirs = x.entry(b, a, FRIENDLY);
    x.set(b, "OtherOpinions", Value::array(vec![theirs]));
    assert_eq!(x.generic(me, Some(other)).unwrap(), F, "B's OtherOpinions entry for A");
    x.set(a, "MyOpinions", Value::array(vec![mine]));
    assert_eq!(x.generic(me, Some(other)).unwrap(), E, "A's MyOpinions entry for B comes first");
    x.set(a, "MyOpinions", Value::array(vec![]));
    x.set(b, "OtherOpinions", Value::array(vec![]));
    x.set(b, "bForceAllOtherOpinions", Value::Bool(true));
    x.set(b, "ForcedOtherOpinion", Value::Byte(ENEMY));
    assert_eq!(x.generic(me, Some(other)).unwrap(), E, "B's forced opinion");
    // Two allegiances are compared as objects: one with A's values is still another allegiance.
    let twin = x.allegiance(FRIENDLY, NEUTRAL);
    let stranger = x.willow_pawn(Some(twin));
    assert_eq!(x.generic(me, Some(stranger)).unwrap(), N, "not the same object: A's DefaultOpinion");
}

/// The other sides the note does not read are `NotYet`: a pawn that is not a `WillowPawn`, and an object that is none
/// of a pawn, a `PawnAllegiance` and a controller.
#[test]
fn an_unread_other_side_is_not_yet() {
    let mut x = fx(ParityMode::Off, false);
    let a = x.allegiance(FRIENDLY, NEUTRAL);
    let me = x.willow_pawn(Some(a));
    let plain = x.obj("Engine.Pawn");
    assert!(not_read(x.generic(me, Some(plain))).contains("not a WillowPawn"));
    let thing = x.obj("Engine.ToyThing");
    assert!(not_read(x.generic(me, Some(thing))).contains("targetable"));
}

/// Section 2: the charm steps answer first, with or without an allegiance; step 3 needs this pawn to have no
/// `CharmedParent` and the other's parent to be friendly to it; a charm field on the pawn itself is `NotYet`.
#[test]
fn the_charm_layer() {
    let mut x = fx(ParityMode::Off, false);
    let a = x.allegiance(FRIENDLY, NEUTRAL);
    let q = x.willow_pawn(None);
    let child = x.willow_pawn(Some(a));
    let parent = x.willow_pawn(None);
    x.set(q, "CharmedChild", Value::Object(Some(child)));
    x.set(q, "CharmedParent", Value::Object(Some(parent)));
    assert_eq!(x.willow(q, Some(child)).unwrap(), E, "the charmed child");
    assert_eq!(x.willow(q, Some(parent)).unwrap(), F, "the charmed parent, no allegiance needed");
    // Step 3: T's parent P is friendly to Q (Q and P share an allegiance), Q has no parent.
    let (q, t, p) = (x.willow_pawn(Some(a)), x.willow_pawn(Some(a)), x.willow_pawn(Some(a)));
    x.set(t, "CharmedParent", Value::Object(Some(p)));
    assert_eq!(x.willow(q, Some(t)).unwrap(), E, "the other's parent is friendly to this pawn");
    assert_eq!(x.generic(q, Some(t)).unwrap(), F, "the generic rule alone: the same allegiance");
    let b = x.allegiance(FRIENDLY, NEUTRAL);
    x.set(p, "Allegiance", Value::Object(Some(b)));
    assert_eq!(x.willow(q, Some(t)).unwrap(), F, "a parent that is not friendly: the generic rule");
    x.set(p, "Allegiance", Value::Object(Some(a)));
    let own_parent = x.willow_pawn(Some(a));
    x.set(q, "CharmedParent", Value::Object(Some(own_parent)));
    assert_eq!(x.willow(q, Some(t)).unwrap(), F, "this pawn has a parent: step 3 does not apply");
    // A self link counts only where the answer depends on it (review r2 of #449): asking about another pawn, the
    // steps run in order; asking about itself while a charm field points at itself is the unread case.
    x.set(q, "CharmedParent", Value::Object(Some(q)));
    assert_eq!(x.willow(q, Some(t)).unwrap(), F, "a parent set (to itself): step 3 does not apply, the generic rule");
    let charmed = x.willow_pawn(None);
    x.set(q, "CharmedChild", Value::Object(Some(charmed)));
    assert_eq!(x.willow(q, Some(charmed)).unwrap(), E, "the CharmedChild match wins although CharmedParent is itself");
    assert!(not_read(x.willow(q, Some(q))).contains("itself"));
}

/// `IsFriendly` is "opinion is Friendly" by the pawn's class: the charm layer for a `WillowPawn`, the generic rule for
/// a plain pawn. A dead pawn is not special (no health is read).
#[test]
fn is_friendly_follows_the_pawns_class() {
    let mut x = fx(ParityMode::Off, false);
    let a = x.allegiance(FRIENDLY, NEUTRAL);
    let maya = x.willow_pawn(Some(a));
    let ally = x.willow_pawn(Some(a));
    assert_eq!(x.friendly(maya, Some(ally)).unwrap(), Value::Bool(true));
    x.set(maya, "CharmedChild", Value::Object(Some(ally)));
    assert_eq!(x.friendly(maya, Some(ally)).unwrap(), Value::Bool(false), "the charm layer: a charmed child is an enemy");
    let plain = x.obj("Engine.Pawn");
    x.set(plain, "Allegiance", Value::Object(Some(a)));
    assert_eq!(x.friendly(plain, Some(ally)).unwrap(), Value::Bool(true), "a plain pawn: the generic rule");
    assert_eq!(x.friendly(plain, None).unwrap(), Value::Bool(false));
    let controller = x.obj("Engine.Controller");
    x.set(controller, "Pawn", Value::Object(Some(ally)));
    assert_eq!(x.friendly(plain, Some(controller)).unwrap(), Value::Bool(false), "a controller: Neutral");
    assert_eq!(x.get(maya, "CharmedChild"), Value::Object(Some(ally)), "nothing was changed");
}

/// The combat hit pack in Off keeps this module's natives (a controller answers Neutral, not through its pawn as the
/// stand-in reads it); in Reference the stand-ins answer and `WillowPawn.GetOpinion` stays unbound.
#[test]
fn the_combat_pack_binds_the_stand_ins_in_reference_only() {
    for mode in [ParityMode::Off, ParityMode::Reference] {
        let mut x = fx(mode, true);
        let a = x.allegiance(FRIENDLY, NEUTRAL);
        let me = x.willow_pawn(Some(a));
        let ally = x.willow_pawn(Some(a));
        let controller = x.obj("Engine.Controller");
        x.set(controller, "Pawn", Value::Object(Some(ally)));
        let expected = Value::Bool(mode == ParityMode::Reference);
        assert_eq!(x.friendly(me, Some(controller)).unwrap(), expected, "{mode:?}");
        let f = x.vm.find_function("WillowGame.WillowPawn.GetOpinion").unwrap();
        let key = x.vm.function(f).unwrap().native_key.clone().unwrap();
        assert_eq!(x.vm.has_native(&key), mode == ParityMode::Off, "{mode:?}: WillowPawn.GetOpinion");
    }
    // This module alone binds nothing in Reference.
    let mut x = fx(ParityMode::Reference, false);
    for path in PATHS {
        let f = x.vm.find_function(path).unwrap();
        let key = x.vm.function(f).unwrap().native_key.clone().unwrap();
        assert!(!x.vm.has_native(&key), "{path}");
    }
}

/// A plain pawn asking about itself answers its allegiance's `SelfOpinion` before the other side is classified
/// (review r2 of #449); a distinct plain pawn is still refused.
#[test]
fn a_plain_pawn_asking_about_itself_answers_its_self_opinion() {
    let mut x = fx(ParityMode::Off, false);
    let a = x.allegiance(FRIENDLY, NEUTRAL);
    let plain = x.obj("Engine.Pawn");
    x.set(plain, "Allegiance", Value::Object(Some(a)));
    assert_eq!(x.generic(plain, Some(plain)).unwrap(), F);
    assert_eq!(x.friendly(plain, Some(plain)).unwrap(), Value::Bool(true));
    let other = x.obj("Engine.Pawn");
    let me = x.willow_pawn(Some(a));
    assert!(not_read(x.generic(me, Some(other))).contains("not a WillowPawn"));
}

/// A class object as `Other` is a named `NotYet`, not the None rule (review r2 of #449).
#[test]
fn a_class_object_is_not_yet() {
    let mut x = fx(ParityMode::Off, false);
    let a = x.allegiance(FRIENDLY, NEUTRAL);
    let me = x.willow_pawn(Some(a));
    let class = x.vm.find_class("Engine.Pawn").unwrap();
    for path in PATHS {
        let f = x.vm.find_function(path).unwrap();
        let r = x.vm.call(f, Some(me), vec![Value::Class(Some(class))]).map(|r| r.ret);
        assert!(not_read(r).contains("class object"), "{path}");
    }
}

/// `IsFriendly` resolves `GetOpinion` virtually (review r2 of #449): a script override runs, and the charm layer's
/// step 3 asks the charmed parent's own `IsFriendly`, which dispatches the same way.
#[test]
fn is_friendly_dispatches_get_opinion_through_the_vm() {
    let mut x = fx(ParityMode::Off, false);
    let fond = x.obj("WillowGame.ToyFondPawn");
    let stranger = x.willow_pawn(None);
    assert_eq!(x.friendly(fond, Some(stranger)).unwrap(), Value::Bool(true), "the script override answers Friendly");
    assert_eq!(x.willow(fond, Some(stranger)).unwrap(), N, "the stock native alone: no allegiance, Neutral");
    // Step 3: T's charmed parent is the fond pawn, whose own (overridden) opinion is Friendly of everyone.
    let (q, t) = (x.willow_pawn(None), x.willow_pawn(None));
    x.set(t, "CharmedParent", Value::Object(Some(fond)));
    assert_eq!(x.willow(q, Some(t)).unwrap(), E);
}

/// An override that resolves but cannot run is a named `NotYet` naming the resolved method, decided before any call
/// (review r3 of #449): an unbound native (whatever the stub policy) and a script that does not prepare, for the
/// `GetOpinion` dispatch and for the charm parent's `IsFriendly` dispatch.
#[test]
fn an_override_that_cannot_run_is_a_named_not_yet() {
    let mut x = fx(ParityMode::Off, false);
    let other = x.willow_pawn(None);
    for (class, method) in [
        ("WillowGame.ToyUnboundPawn", "WillowGame.ToyUnboundPawn.GetOpinion"),
        ("WillowGame.ToyBrokenPawn", "WillowGame.ToyBrokenPawn.GetOpinion"),
    ] {
        let me = x.obj(class);
        let text = not_read(x.friendly(me, Some(other)));
        assert!(text.contains(method) && text.contains("cannot run"), "{text}");
    }
    // Step 3 asks the charmed parent's own IsFriendly, which resolves to the unbound override.
    let (q, t) = (x.willow_pawn(None), x.willow_pawn(None));
    let parent = x.obj("WillowGame.ToyUnboundPawn");
    x.set(t, "CharmedParent", Value::Object(Some(parent)));
    let text = not_read(x.willow(q, Some(t)));
    assert!(text.contains("WillowGame.ToyUnboundPawn.IsFriendly"), "{text}");
}

/// Each charm field is read just before its step (review r3 of #449): a matching child answers Enemy even when the
/// pawn's class has no CharmedParent; anything past that step needs the field.
#[test]
fn a_matching_child_answers_before_the_parent_field_is_read() {
    let mut x = fx(ParityMode::Off, false);
    let me = x.obj("WillowGame.ToyChildOnly");
    let child = x.willow_pawn(None);
    x.set(me, "CharmedChild", Value::Object(Some(child)));
    assert_eq!(x.willow(me, Some(child)).unwrap(), E);
    let stranger = x.willow_pawn(None);
    assert!(x.willow(me, Some(stranger)).is_err(), "no CharmedParent: a data error past step 1");
}
