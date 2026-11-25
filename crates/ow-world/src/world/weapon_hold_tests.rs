//! `GetWeaponHoldDef` on invented classes (NATIVE_GET_WEAPON_HOLD_DEF.md section 4; AGENTS.md rule 9: invented
//! objects and values, the class and field names the packages'). Every call checks that nothing was written, loaded,
//! drawn or interned (test 11), with no class code loaded.

use std::rc::Rc;

use ow_attr::AttributeSystem;
use ow_core::notyet::Missing;
use ow_core::{ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ClassId, FuncId, ObjRef, PropAddr};
use ow_object::layout::{PropKind, PropertyDecl};
use ow_object::value::Value;
use ow_vm::api::{FunctionDecl, Vm};
use ow_vm::error::VmError;

use super::*;

struct Fx {
    vm: Vm,
    f: FuncId,
    body: ClassId,
    hold: ClassId,
    tag: ClassId,
}

fn decl(vm: &mut Vm, name: &str, kind: PropKind) -> PropertyDecl {
    PropertyDecl::new(vm.space.names.intern(name), kind)
}

fn param(vm: &mut Vm, name: &str, kind: PropKind) -> PropertyDecl {
    decl(vm, name, kind).with_flags(PropertyDecl::PARAM)
}

/// The classes, the declared native and the binding, in `mode`.
fn fx_in(mode: ParityMode) -> Fx {
    let mut space = ObjectSpace::new();
    let object = space.classes.define("Core.Object", None, vec![]).unwrap();
    let mut vm = Vm::new(space, DefStore::new_empty(), Rc::new(AttributeSystem::new_empty()), mode);
    let o = PropKind::Object { class: object };
    let tag = vm.space.classes.define("WillowGame.EnvironmentTagDefinition", Some(object), vec![]).unwrap();
    let props = vec![decl(&mut vm, "HoldName", PropKind::Name), decl(&mut vm, "EnvironmentTag", o.clone())];
    let hold = vm.space.classes.define("WillowGame.BodyWeaponHoldDefinition", Some(object), props).unwrap();
    let props = vec![
        decl(&mut vm, "WeaponActionParentClass", o.clone()),
        decl(&mut vm, "DefaultWeaponHoldDef", PropKind::Object { class: hold }),
        decl(&mut vm, "WeaponHoldDefs", PropKind::Array(Box::new(PropKind::Object { class: hold }))),
    ];
    let body = vm.space.classes.define("WillowGame.BodyClassDefinition", Some(object), props).unwrap();
    let params = vec![
        param(&mut vm, "HoldName", PropKind::Name),
        param(&mut vm, "EnvironmentTag", o),
        param(&mut vm, "bSearchParentClass", PropKind::Bool),
        param(&mut vm, "bFallbackToDefaultDef", PropKind::Bool),
    ];
    let path = ObjectPath::parse("WillowGame.BodyClassDefinition.GetWeaponHoldDef").unwrap();
    let f = vm.declare_function(FunctionDecl::native(path, KEY).with_owner(body).with_params(params)).unwrap();
    if mode != ParityMode::Reference {
        register(&mut vm).unwrap();
    }
    Fx { vm, f, body, hold, tag }
}

fn fx() -> Fx {
    fx_in(ParityMode::Off)
}

impl Fx {
    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap();
        self.vm.space.write(o, PropAddr::scalar(p), v).unwrap();
    }

    fn tag(&mut self, name: &str) -> ObjRef {
        self.vm.space.alloc(self.tag, name, None, Origin::Runtime).unwrap()
    }

    /// A hold definition named `hold` with tag `tag`.
    fn hold(&mut self, label: &str, hold: &str, tag: Option<ObjRef>) -> ObjRef {
        let h = self.vm.space.alloc(self.hold, label, None, Origin::Runtime).unwrap();
        let n = self.vm.space.names.intern(hold);
        self.set(h, "HoldName", Value::Name(n));
        self.set(h, "EnvironmentTag", Value::Object(tag));
        h
    }

    /// A body class with `holds` (None entries allowed), a default and a parent.
    fn body(&mut self, label: &str, holds: &[Option<ObjRef>], default: Option<ObjRef>, parent: Option<ObjRef>) -> ObjRef {
        let b = self.vm.space.alloc(self.body, label, None, Origin::Runtime).unwrap();
        self.set(b, "WeaponHoldDefs", Value::array(holds.iter().map(|h| Value::Object(*h)).collect()));
        self.set(b, "DefaultWeaponHoldDef", Value::Object(default));
        self.set(b, "WeaponActionParentClass", Value::Object(parent));
        b
    }

    fn marks(&self) -> (u64, u64, u64, u64, usize) {
        let vm = &self.vm;
        (vm.space.heap.committed_writes(), vm.rng.draws(), vm.defs.load_transitions(), vm.class_code_loads(), vm.space.names.len())
    }

    /// `body.GetWeaponHoldDef(name, tag, parent, fallback)`; asserts nothing changed (test 11).
    fn ask(&mut self, body: ObjRef, name: &str, tag: Option<ObjRef>, parent: bool, fallback: bool) -> Result<Option<ObjRef>, VmError> {
        let n = Value::Name(self.vm.space.names.intern(name));
        let before = self.marks();
        let out = self.vm.call(self.f, Some(body), vec![n, Value::Object(tag), Value::Bool(parent), Value::Bool(fallback)]);
        assert_eq!(self.marks(), before, "a pure read");
        match out?.ret {
            Value::Object(o) => Ok(o),
            other => panic!("{other:?}"),
        }
    }

    fn lazy(&mut self, class: ClassId, label: &str) -> ObjRef {
        self.vm.space.alloc_stand_in(class, label, Origin::Resource { package: 0, index: 1 }).unwrap()
    }
}

fn not_read(e: VmError) -> String {
    match e {
        VmError::NotYet(n) => {
            assert_eq!((n.missing, n.record), (Missing::NotRead, RECORD));
            n.what.to_string()
        }
        other => panic!("expected NotYet, got {other:?}"),
    }
}

fn script_message(e: VmError) -> String {
    match e {
        VmError::Script { message, .. } => message,
        other => panic!("expected a script error, got {other:?}"),
    }
}

/// Test 1, `maya:8`'s shape: entries (`Pistol`, None) and (`Unarmed`, None), a default, and a parent with no entries
/// and its own default. `('Unarmed', None, false, true)` answers the `Unarmed` entry; on the parent, its default. With
/// the parent searched (`GetAimOffsetProfileForHoldName`'s (true, true)), the same exact match.
#[test]
fn maya_s_unarmed_hold_and_the_shared_parent_s_default() {
    let mut x = fx();
    let shared_default = x.hold("WeaponHold_Player_Default", "None", None);
    let shared = x.body("BodyClass_PlayerShared", &[], Some(shared_default), None);
    let pistol = x.hold("WeaponHold_Pistol", "Pistol", None);
    let unarmed = x.hold("WeaponHold_Unarmed", "Unarmed", None);
    let default = x.hold("WeaponHold_Default", "None", None);
    let siren = x.body("BodyClass_Toy", &[Some(pistol), Some(unarmed)], Some(default), Some(shared));
    assert_eq!(x.ask(siren, "Unarmed", None, false, true).unwrap(), Some(unarmed));
    assert_eq!(x.ask(shared, "Unarmed", None, false, true).unwrap(), Some(shared_default));
    assert_eq!(x.ask(siren, "Unarmed", None, true, true).unwrap(), Some(unarmed));
}

/// Test 2: no match: the default with F true, None with F false.
#[test]
fn no_match_is_the_default_only_with_the_fallback() {
    let mut x = fx();
    let pistol = x.hold("WeaponHold_Pistol", "Pistol", None);
    let default = x.hold("WeaponHold_Default", "None", None);
    let b = x.body("Body", &[Some(pistol)], Some(default), None);
    assert_eq!(x.ask(b, "NoSuchHold", None, true, true).unwrap(), Some(default));
    assert_eq!(x.ask(b, "NoSuchHold", None, true, false).unwrap(), None);
}

/// Tests 3, 4 and 5: the candidate rule. Name matches with other tags only: the first, ahead of the default.
/// (`A`, X), (`A`, None), (`A`, Y) asked (`A`, Z): the tag-less one. (`A`, None), (`A`, Z) asked (`A`, Z): the exact
/// match after the candidate.
#[test]
fn the_name_only_candidate_and_the_exact_match() {
    let mut x = fx();
    let [tx, ty, tz] = ["X", "Y", "Z"].map(|n| x.tag(n));
    let ax = x.hold("AX", "A", Some(tx));
    let ay = x.hold("AY", "A", Some(ty));
    let default = x.hold("Default", "None", None);
    let b = x.body("Body3", &[Some(ax), Some(ay)], Some(default), None);
    assert_eq!(x.ask(b, "A", Some(tz), false, true).unwrap(), Some(ax), "the first name match");

    let a_none = x.hold("ANone", "A", None);
    let b = x.body("Body4", &[Some(ax), Some(a_none), Some(ay)], Some(default), None);
    assert_eq!(x.ask(b, "A", Some(tz), false, true).unwrap(), Some(a_none), "a later tag-less match replaces it");

    let az = x.hold("AZ", "A", Some(tz));
    let b = x.body("Body5", &[Some(a_none), Some(az)], Some(default), None);
    assert_eq!(x.ask(b, "A", Some(tz), false, true).unwrap(), Some(az), "the exact match wins");
    assert_eq!(x.ask(b, "A", None, false, true).unwrap(), Some(a_none), "an exact match with a None tag, at once");
}

/// Test 6: the parent.
#[test]
fn the_parent_lookup() {
    let mut x = fx();
    let [tx, tz] = ["X", "Z"].map(|n| x.tag(n));
    // P true, no own match: the parent's exact match.
    let parent_exact = x.hold("ParentAZ", "A", Some(tz));
    let parent = x.body("Parent", &[Some(parent_exact)], None, None);
    let child = x.body("Child", &[], None, Some(parent));
    assert_eq!(x.ask(child, "A", Some(tz), true, true).unwrap(), Some(parent_exact));
    // P false: the parent is not read (a lazy parent is no refusal).
    let lazy_parent = x.lazy(x.body, "LazyParent");
    let own_default = x.hold("OwnDefault", "None", None);
    let lone = x.body("Lone", &[], Some(own_default), Some(lazy_parent));
    assert_eq!(x.ask(lone, "A", Some(tz), false, true).unwrap(), Some(own_default));
    // An own candidate and a parent that matches by name only: the own candidate.
    let parent_name = x.hold("ParentAX", "A", Some(tx));
    let parent = x.body("Parent2", &[Some(parent_name)], None, None);
    let own = x.hold("OwnAX", "A", Some(tx));
    let child = x.body("Child2", &[Some(own)], None, Some(parent));
    assert_eq!(x.ask(child, "A", Some(tz), true, true).unwrap(), Some(own));
    // An own candidate and a parent with an exact match: the parent's.
    let parent = x.body("Parent3", &[Some(parent_exact)], None, None);
    let child = x.body("Child3", &[Some(own)], None, Some(parent));
    assert_eq!(x.ask(child, "A", Some(tz), true, true).unwrap(), Some(parent_exact));
    // No own candidate and a parent answering None (F not passed, as the child has a default): the child's default.
    let parent_default = x.hold("ParentDefault", "None", None);
    let parent = x.body("Parent4", &[], Some(parent_default), None);
    let child_default = x.hold("ChildDefault", "None", None);
    let child = x.body("Child4", &[], Some(child_default), Some(parent));
    assert_eq!(x.ask(child, "A", None, true, true).unwrap(), Some(child_default), "F is not passed on");
    // With no default of its own, F is passed on: the parent's default.
    let child = x.body("Child5", &[], None, Some(parent));
    assert_eq!(x.ask(child, "A", None, true, true).unwrap(), Some(parent_default));
    assert_eq!(x.ask(child, "A", None, true, false).unwrap(), None);
}

/// Test 6, the note's fixed cases (#728 at 7e0ed56c): the parent's "exact match" is a re-read of the returned hold.
/// - the re-read quirk: F passed on (no own default), an own candidate, and a parent whose **default** has the asked
///   name and tag: the parent's default, over the candidate;
/// - F passed on, no own default and no match anywhere: the parent's default;
/// - F passed on, no own default, an own candidate and a parent default whose name differs: the candidate;
/// - the parent's returned default a lazy handle while this class has a candidate: the not-loaded `NotRead`.
#[test]
fn the_parent_s_answer_is_re_read() {
    let mut x = fx();
    let [tx, tz] = ["X", "Z"].map(|n| x.tag(n));
    let look_alike = x.hold("ParentDefaultAZ", "A", Some(tz));
    let parent = x.body("Parent", &[], Some(look_alike), None);
    let own = x.hold("OwnAX", "A", Some(tx));
    let child = x.body("Child", &[Some(own)], None, Some(parent));
    assert_eq!(x.ask(child, "A", Some(tz), true, true).unwrap(), Some(look_alike), "the re-read makes it exact");

    let plain_default = x.hold("ParentDefault", "None", None);
    let parent = x.body("Parent2", &[], Some(plain_default), None);
    let empty = x.body("Child2", &[], None, Some(parent));
    assert_eq!(x.ask(empty, "A", Some(tz), true, true).unwrap(), Some(plain_default), "no match: the parent's default");
    let child = x.body("Child3", &[Some(own)], None, Some(parent));
    assert_eq!(x.ask(child, "A", Some(tz), true, true).unwrap(), Some(own), "an own candidate: the candidate");
    let lazy_default = x.lazy(x.hold, "LazyParentDefault");
    let parent = x.body("Parent3", &[], Some(lazy_default), None);
    let child = x.body("Child4", &[Some(own)], None, Some(parent));
    assert_eq!(not_read(x.ask(child, "A", Some(tz), true, true).unwrap_err()), HOLD_NOT_LOADED, "read before its fields");
    // With no own candidate the returned default is not read: no refusal.
    let empty = x.body("Child5", &[], None, Some(parent));
    assert_eq!(x.ask(empty, "A", Some(tz), true, true).unwrap(), Some(lazy_default));
}

/// Test 3's slice shape (the note's fix): (`A`, None) asked against [(`A`, X)]: the tagged entry is the candidate, ahead of the default.
#[test]
fn a_none_tag_asked_against_a_tagged_entry_is_the_candidate() {
    let mut x = fx();
    let tx = x.tag("X");
    let ax = x.hold("AX", "A", Some(tx));
    let default = x.hold("Default", "None", None);
    let b = x.body("Body", &[Some(ax)], Some(default), None);
    assert_eq!(x.ask(b, "A", None, false, true).unwrap(), Some(ax));
}

/// Test 7: None entries are skipped.
#[test]
fn none_entries_are_skipped() {
    let mut x = fx();
    let a = x.hold("A", "A", None);
    let b = x.body("Body", &[None, Some(a), None], None, None);
    assert_eq!(x.ask(b, "A", None, false, false).unwrap(), Some(a));
}

/// Test 8: a lazy entry, a lazy parent or a lazy receiver: the not-loaded `NotRead`, nothing loaded. The entry check
/// comes before any field of it is read: a lazy entry ahead of the match refuses.
#[test]
fn a_lazy_receiver_entry_or_parent_is_not_read() {
    let mut x = fx();
    let lazy_body = x.lazy(x.body, "LazyBody");
    assert_eq!(not_read(x.ask(lazy_body, "A", None, true, true).unwrap_err()), NOT_LOADED);
    let lazy_hold = x.lazy(x.hold, "LazyHold");
    let a = x.hold("A", "A", None);
    let b = x.body("Body", &[Some(lazy_hold), Some(a)], None, None);
    assert_eq!(not_read(x.ask(b, "A", None, false, true).unwrap_err()), HOLD_NOT_LOADED);
    let child = x.body("Child", &[], None, Some(lazy_body));
    assert_eq!(not_read(x.ask(child, "A", None, true, true).unwrap_err()), NOT_LOADED);
}

/// Test 9: malformed declarations are `VmError::Script` naming the field.
#[test]
fn malformed_declarations_are_script_errors() {
    let mut x = fx();
    let object = x.vm.space.classes.by_path("Core.Object").unwrap();
    let props = vec![
        decl(&mut x.vm, "WeaponActionParentClass", PropKind::Object { class: object }),
        decl(&mut x.vm, "DefaultWeaponHoldDef", PropKind::Object { class: object }),
        decl(&mut x.vm, "WeaponHoldDefs", PropKind::Int),
    ];
    let odd_body = x.vm.space.classes.define("Test.OddBody", Some(object), props).unwrap();
    let b = x.vm.space.alloc(odd_body, "OddBody", None, Origin::Runtime).unwrap();
    assert!(script_message(x.ask(b, "A", None, true, true).unwrap_err()).contains("WeaponHoldDefs"));

    let props = vec![decl(&mut x.vm, "HoldName", PropKind::Int), decl(&mut x.vm, "EnvironmentTag", PropKind::Object { class: object })];
    let odd_hold = x.vm.space.classes.define("Test.OddHold", Some(object), props).unwrap();
    let h = x.vm.space.alloc(odd_hold, "OddHold", None, Origin::Runtime).unwrap();
    // The body's list is declared of hold definitions, so the odd hold is written through a raw slot.
    let b = x.body("Body", &[], None, None);
    let p = x.vm.space.prop(x.body, "WeaponHoldDefs").unwrap();
    *x.vm.space.slot_mut(b, PropAddr::scalar(p)).unwrap() = Value::array(vec![Value::Object(Some(h))]);
    assert!(script_message(x.ask(b, "A", None, true, true).unwrap_err()).contains("HoldName"));

    let props = vec![decl(&mut x.vm, "WeaponHoldDefs", PropKind::Array(Box::new(PropKind::Object { class: object })))];
    let bare = x.vm.space.classes.define("Test.BareBody", Some(object), props).unwrap();
    let b = x.vm.space.alloc(bare, "Bare", None, Origin::Runtime).unwrap();
    assert!(script_message(x.ask(b, "A", None, true, true).unwrap_err()).contains("declares no DefaultWeaponHoldDef"));
}

/// Test 10: a parent chain that loops is `VmError::Script` (the port's guard).
#[test]
fn a_looping_parent_chain_is_a_script_error() {
    let mut x = fx();
    let a = x.body("A", &[], None, None);
    let b = x.body("B", &[], None, Some(a));
    x.set(a, "WeaponActionParentClass", Value::Object(Some(b)));
    assert!(script_message(x.ask(a, "A", None, true, true).unwrap_err()).contains("comes back"));
    assert_eq!(x.ask(a, "A", None, false, false).unwrap(), None, "the parent unread, no loop");
}

/// Test 12: Reference binds nothing; Off binds the key.
#[test]
fn off_binds_the_key_and_reference_does_not() {
    let x = fx();
    assert!(x.vm.has_native(KEY));
    let mut r = fx_in(ParityMode::Reference);
    ow_vm::api::NativePack::register(&crate::world::Pack, &mut r.vm).unwrap();
    assert!(!r.vm.has_native(KEY));
}
