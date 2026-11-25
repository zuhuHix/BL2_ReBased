//! Link 1's owner-test set-up (lane ROUTE-A) on an invented `GearboxFramework` package with decoded class bodies
//! (AGENTS.md rule 9). AI-assisted (Claude Code).

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::{Missing, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::{ObjectSpace, Origin};
use ow_world::world::anim_node_init::{owner_implements, owner_not_decoded, owner_not_resolved};

use super::*;

static NEXT: AtomicU32 = AtomicU32::new(0);

/// Removes the store's directory when dropped.
struct Dir(PathBuf);

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// `GearboxFramework.upk` (invented): `SpecialMoveInterface`, another interface `IToyOther`; `ToyPawn`, which
/// implements the first through its super class `ToyBase` (two entries on its chain, both unresolved); `ToyPlain`,
/// whose only entry is the other interface; `ToyGone`, whose only entry is a class import from a package no store
/// holds; `ToyBrokenPawn`, below a class whose body does not decode.
fn store() -> (Vm, Dir) {
    use ow_object::class_body::CLASS_INTERFACE;
    use ow_package::testkit::reflect::{ClassBodySpec, Declarations, class_body};
    let dir = std::env::temp_dir().join(format!("ow-cli-owner-setup-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let mut d = Declarations::with_imports(&["Class", "Function", "StructProperty"]);
    let object_body = class_body(&mut d.p, &ClassBodySpec::new(0, 1, 2));
    let obj = d.p.add_export(0, "Object", object_body);
    d.default_object(obj, "Object");
    let smi = d.real_class("SpecialMoveInterface", obj, obj, 2 | CLASS_INTERFACE, &[], None);
    let other = d.real_class("IToyOther", obj, obj, 2 | CLASS_INTERFACE, &[], None);
    let base = d.real_class("ToyBase", obj, obj, 2, &[smi], None);
    d.real_class("ToyPawn", base, obj, 2, &[other], None);
    d.real_class("ToyPlain", obj, obj, 2, &[other], None);
    let missing = d.p.add_import("Core", "Package", 0, "ToyMissingPkg");
    let gone = d.p.add_import("Core", "Class", missing, "ToyGoneTarget");
    d.real_class("ToyGone", obj, obj, 2, &[gone], None);
    let broken = d.cls("ToyBroken", obj);
    d.real_class("ToyBrokenPawn", broken, obj, 2, &[smi], None);
    // Review r1 of #670: an entry naming a Function export (not a class), and one naming an interface in a second
    // package, `WillowGame.upk`, which the store has not loaded.
    let function = d.import("Function");
    let not_a_class = d.p.add_export_with(ow_package::testkit::Export::new(function, "ToyFn", Vec::new()));
    d.real_class("ToyOdd", obj, obj, 2, &[not_a_class], None);
    let far_pkg = d.p.add_import("Core", "Package", 0, "WillowGame");
    let far = d.p.add_import("Core", "Class", far_pkg, "IToyFar");
    d.real_class("ToyFarPawn", obj, obj, 2, &[far], None);
    let bytes = d.build();
    let mut w = Declarations::with_imports(&["Class", "Function", "StructProperty"]);
    let core_pkg = w.p.add_import("Core", "Package", 0, "GearboxFramework");
    let w_obj = w.p.add_import("Core", "Class", core_pkg, "Object");
    w.real_class("IToyFar", w_obj, w_obj, 2 | CLASS_INTERFACE, &[], None);
    let far_bytes = w.build();
    let files: [(&str, &[u8]); 2] = [("GearboxFramework.upk", &bytes), ("WillowGame.upk", &far_bytes)];
    let defs = DefStore::from_synthetic(&dir, &files, IdentityPolicy::FirstInScanOrder, ParityMode::Off).expect("store");
    (Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off), Dir(dir))
}

/// The pawn class at `path` (loaded, as the archetype's load would) and an owner of it. An entry's target the code
/// index does not hold yet is loaded by the set-up's resolve, which its line reports as a write.
fn pawn(vm: &mut Vm, path: &str) -> (ClassId, ow_object::ids::ObjRef) {
    let c = vm.find_class(path).unwrap();
    let o = vm.space.alloc(c, "ToyOwner_0", None, Origin::Runtime).unwrap();
    (c, o)
}

fn counters(vm: &Vm) -> (usize, usize) {
    (vm.space.heap.committed_writes() as usize, vm.defs.load_transitions() as usize)
}

/// Route (a)'s test: before the set-up the owner test refuses O5; the set-up loads the interface, resolves both
/// entries of the chain and says the yes was read from the packages' lists; afterwards the owner test answers yes
/// with no write and no load, and a second set-up resolves nothing.
#[test]
fn the_set_up_turns_o5_into_the_packages_yes() {
    let (mut vm, _dir) = store();
    let (c, owner) = pawn(&mut vm, "GearboxFramework.ToyPawn");
    vm.find_class(SPECIAL_MOVE_INTERFACE).unwrap();
    let Err(VmError::NotYet(n)) = owner_implements(&vm, owner) else { panic!("O5 before the set-up") };
    assert_eq!(&*n.what, owner_not_resolved("GearboxFramework.ToyPawn"));
    let mark = vm.log.len();
    special_move_owner_setup(&mut vm, Some(c)).unwrap();
    let lines = &vm.log[mark..];
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(lines[0].starts_with("harness input: set-up class GearboxFramework.SpecialMoveInterface: "), "{lines:?}");
    assert_eq!(
        lines[1],
        "harness input: set-up interface tables of GearboxFramework.ToyPawn for GearboxFramework.SpecialMoveInterface \
         (the lookups loaded code: a write): yes, read from the packages' interface lists; the pass covers every \
         entry on the chain; 2 previously unresolved entries resolved at set-up (a load; UNVERIFIED stand-in for the \
         game having them resolved)"
    );
    let before = counters(&vm);
    assert!(owner_implements(&vm, owner).unwrap(), "the owner test reads the resolved yes");
    assert_eq!(counters(&vm), before, "the owner test still writes and loads nothing");
    let (line, outcome) = owner_tables(&mut vm, c);
    outcome.unwrap();
    assert!(line.ends_with("yes, read from the packages' interface lists; the pass covers every entry on the chain; none was left unresolved"), "{line}");
}

/// A class whose entries name only another interface: a no, read the same way; the owner test then gives the
/// settled no (O7).
#[test]
fn the_set_up_reads_a_no() {
    let (mut vm, _dir) = store();
    let (c, owner) = pawn(&mut vm, "GearboxFramework.ToyPlain");
    // The entry's target loaded first, so the resolve loads no code of its own.
    vm.find_class("GearboxFramework.IToyOther").unwrap();
    special_move_owner_setup(&mut vm, Some(c)).unwrap();
    let line = vm.log.last().unwrap();
    assert!(line.contains("ToyPlain for GearboxFramework.SpecialMoveInterface: no, read from the packages'"), "{line}");
    assert!(line.contains("1 previously unresolved entry resolved at set-up"), "{line}");
    assert!(!line.contains("loaded code"), "the target was already loaded: {line}");
    assert!(!owner_implements(&vm, owner).unwrap());
}

/// An entry that resolves to no class: the plain resolve keeps it as "resolved to nothing", which the owner test
/// would read as O7's settled no. The strict cross-check has no answer, so the line says so and the set-up is
/// `NotYet` (`NotRead`), not the packages' answer.
#[test]
fn an_entry_that_resolves_to_no_class_refuses() {
    let (mut vm, _dir) = store();
    let (c, _) = pawn(&mut vm, "GearboxFramework.ToyGone");
    let e = special_move_owner_setup(&mut vm, Some(c)).unwrap_err();
    let line = vm.log.last().unwrap();
    assert!(line.contains(": unresolved, the strict reading has no answer (an entry or a class chain did not resolve)"), "{line}");
    assert!(line.ends_with("1 previously unresolved entry resolved at set-up (a load; UNVERIFIED stand-in for the game having it resolved)"), "{line}");
    assert!(!line.contains(": no, read"), "{line}");
    let VmError::NotYet(n) = e else { panic!("{e:?}") };
    assert_eq!((n.missing, n.record), (Missing::NotRead, ow_world::world::anim_node_init::OWNER_RECORD));
}

/// A chain whose bodies did not all decode: nothing is resolved (no structural fallback), and the owner test answers
/// O4 itself.
#[test]
fn an_undecoded_chain_resolves_nothing() {
    let (mut vm, _dir) = store();
    let (c, owner) = pawn(&mut vm, "GearboxFramework.ToyBrokenPawn");
    special_move_owner_setup(&mut vm, Some(c)).unwrap();
    let line = vm.log.last().unwrap();
    assert!(line.ends_with("the class bodies did not all decode, nothing resolved (no structural fallback is taken)"), "{line}");
    assert_eq!(vm.unresolved_interface_entries(c), 1, "nothing resolved");
    let Err(VmError::NotYet(n)) = owner_implements(&vm, owner) else { panic!("O4") };
    assert_eq!(&*n.what, owner_not_decoded("GearboxFramework.ToyBrokenPawn"));
}

/// No archetype loaded: the interface is still loaded, and the tables' line says it skipped.
#[test]
fn no_archetype_skips_the_tables() {
    let (mut vm, _dir) = store();
    special_move_owner_setup(&mut vm, None).unwrap();
    assert_eq!(vm.log.last().unwrap(), "harness input: set-up interface tables skipped (no pawn archetype loaded)");
}

/// Review r1 of #670, MED 1: an entry naming an interface in a package the store had not loaded: resolving it loads
/// that package's code, and the line says so (a write). A no, read from the lists.
#[test]
fn a_lookup_that_loads_another_package_says_so() {
    let (mut vm, _dir) = store();
    let (c, _) = pawn(&mut vm, "GearboxFramework.ToyFarPawn");
    vm.find_class(SPECIAL_MOVE_INTERFACE).unwrap();
    let (line, outcome) = owner_tables(&mut vm, c);
    outcome.unwrap();
    assert!(line.contains("ToyFarPawn for GearboxFramework.SpecialMoveInterface (the lookups loaded code: a write): no, read"), "{line}");
    let (line, _) = owner_tables(&mut vm, c);
    assert!(!line.contains("loaded code"), "nothing more to load: {line}");
}

/// Review r1 of #670, MED 2: an entry naming a Function export. The plain resolve caches it as "resolved to nothing";
/// the strict reading fails (not a class), so the set-up refuses with the lookup's failure.
#[test]
fn an_entry_that_is_not_a_class_is_a_failed_lookup() {
    let (mut vm, _dir) = store();
    let (c, _) = pawn(&mut vm, "GearboxFramework.ToyOdd");
    vm.find_class(SPECIAL_MOVE_INTERFACE).unwrap();
    let (line, outcome) = owner_tables(&mut vm, c);
    assert!(line.contains(": unresolved, an entry's lookup failed ("), "{line}");
    assert!(line.contains("1 previously unresolved entry resolved at set-up"), "{line}");
    let Err(VmError::NotYet(n)) = outcome else { panic!("{outcome:?}") };
    assert_eq!((n.missing, n.record), (Missing::NotRead, ow_world::world::anim_node_init::OWNER_RECORD));
    assert!(n.what.contains("an entry's lookup failed"), "{}", n.what);
}

/// Review r1 of #670, LOW 2: a pawn class with no code key (defined by hand) resolves nothing; the owner test then
/// answers O3 itself.
#[test]
fn a_class_without_a_code_key_resolves_nothing() {
    let (mut vm, _dir) = store();
    vm.find_class(SPECIAL_MOVE_INTERFACE).unwrap();
    let object = vm.find_class("GearboxFramework.Object").unwrap();
    let hand = vm.space.classes.define("Test.ToyHandPawn", Some(object), vec![]).unwrap();
    let (line, outcome) = owner_tables(&mut vm, hand);
    outcome.unwrap();
    assert!(line.ends_with("Test.ToyHandPawn for GearboxFramework.SpecialMoveInterface: no code key, nothing resolved"), "{line}");
}
