//! The slice gate's set-up object preload ([`super::preload_object`], lane LINK9-RESOLVER-PRELOAD), on an invented
//! package (AGENTS.md rule 9): the review lows of #650 and #651 (lane PRELOAD-FAILURE-TESTS). An export the store
//! cannot load gives the store's error and the failure line, with nothing else changed; a subobject held by an
//! already-loaded parent is loaded in place, so the parent's slot still holds the same object. Every value is invented.
//! AI-assisted (Claude Code).

use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::ParityMode;
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_package::testkit::script::ScriptToy;
use ow_package::testkit::{Export, ints};
use ow_vm::api::Vm;

use super::{SETUP_OBJECT_INPUT, preload_object};

static NEXT: AtomicU32 = AtomicU32::new(0);

/// `ToyPreload.upk`: `ToyCut_0`, an export cut short (no terminator), whose load fails; and `ToyParent_0`, whose
/// `Child` names its own subobject `ToyParent_0.ToyChild_0`.
fn package() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class", "ObjectProperty"]);
    let object = t.class("Object", 0);
    let thing = t.class("ToyThing", object);
    let parent = t.class("ToyParent", object);
    t.prop("Object", parent, "Child", 0, thing);
    t.p.add_export_with(Export::new(thing, "ToyCut_0", ints(&[0])));
    let parent_index = t.p.next_export();
    let mut payload = ints(&[0]);
    payload.extend(t.p.object_tag("Child", parent_index + 1));
    payload.extend_from_slice(&t.none);
    t.p.add_export_with(Export::new(parent, "ToyParent_0", payload));
    let mut payload = ints(&[0]);
    payload.extend_from_slice(&t.none);
    let mut child = Export::new(thing, "ToyChild_0", payload);
    child.outer = parent_index;
    t.p.add_export_with(child);
    t.build()
}

struct Fx {
    vm: Vm,
    dir: std::path::PathBuf,
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn fx() -> Fx {
    let dir = std::env::temp_dir().join(format!(
        "ow-cli-setup-preload-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let bytes = package();
    let defs = DefStore::from_synthetic(&dir, &[("ToyPreload.upk", &bytes)], IdentityPolicy::FirstInScanOrder, ParityMode::Off)
        .unwrap();
    let vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
    Fx { vm, dir }
}

impl Fx {
    fn find(&mut self, path: &str) -> ObjRef {
        let path = ow_core::ObjectPath::parse(path).unwrap();
        self.vm.defs.find(&mut self.vm.space, &path).unwrap().unwrap()
    }

    fn child_slot(&self, parent: ObjRef) -> Value {
        let class = self.vm.space.heap.get(parent).unwrap().class;
        let p = self.vm.space.prop(class, "Child").unwrap();
        self.vm.space.read(parent, PropAddr::scalar(p)).unwrap().clone()
    }
}

/// Every heap object with its name, slots, origin and store state.
fn snapshot(vm: &Vm) -> Vec<String> {
    vm.space
        .heap
        .handles()
        .into_iter()
        .map(|h| {
            let o = vm.space.heap.get(h).unwrap();
            let state = (vm.defs.is_unloaded(&vm.space, h), vm.defs.is_failed(&vm.space, h));
            format!("{h:?} {} {:?} {:?} {state:?}", vm.space.names.text(o.name), o.slots, o.origin)
        })
        .collect()
}

/// An export the store cannot load: the store's error comes back and the line says the load failed. Nothing else
/// changes: every object that existed is as it was (slots and store state), and the only new objects, and new names,
/// are the store's own: the failed handle for the export, and the default object of its class, which the store makes
/// when it first meets that class (its heap writes; the export itself gets no slots).
#[test]
fn an_export_that_fails_to_load_is_an_error_with_its_line() {
    let mut x = fx();
    let parent = x.find("ToyPreload.ToyParent_0");
    let before = snapshot(&x.vm);
    let names = x.vm.space.names.len();
    let (line, outcome) = preload_object(&mut x.vm, "ToyPreload.ToyCut_0");
    assert!(outcome.is_err(), "the store's error: {outcome:?}");
    assert!(line.starts_with(SETUP_OBJECT_INPUT), "{line}");
    assert!(line.contains("ToyPreload.ToyCut_0: the load failed ("), "{line}");
    let after = snapshot(&x.vm);
    assert!(before.iter().all(|l| after.contains(l)), "every earlier object as it was");
    let new: Vec<&String> = after.iter().filter(|l| !before.contains(l)).collect();
    assert_eq!(new.len(), 2, "{new:?}");
    assert!(new.iter().any(|l| l.contains(" ToyCut_0 [] ") && l.ends_with("(true, true)")), "the failed handle: {new:?}");
    assert!(new.iter().any(|l| l.contains("Default__ToyThing")), "the class's default object: {new:?}");
    // The name table grows only by those two objects' own names.
    assert_eq!(x.vm.space.names.len(), names + 2, "only the two new objects' names");
    assert!(x.vm.space.names.find("ToyCut").is_some() && x.vm.space.names.find("Default__ToyThing").is_some());
    let Value::Object(Some(child)) = x.child_slot(parent) else { panic!("the parent names its child") };
    assert!(x.vm.defs.is_unloaded(&x.vm.space, child), "the unrelated subobject stays unloaded");
}

/// A subobject held by an already-loaded parent: the preload loads it in place. The parent's slot holds the same
/// object afterwards, and it is loaded.
#[test]
fn a_subobject_held_by_a_loaded_parent_is_loaded_in_place() {
    let mut x = fx();
    let parent = x.find("ToyPreload.ToyParent_0");
    let Value::Object(Some(before)) = x.child_slot(parent) else { panic!("the parent names its child") };
    assert!(x.vm.defs.is_unloaded(&x.vm.space, before), "cold before the preload");
    let (line, outcome) = preload_object(&mut x.vm, "ToyPreload.ToyParent_0.ToyChild_0");
    outcome.unwrap();
    assert!(line.contains("loaded at set-up"), "{line}");
    assert_eq!(x.child_slot(parent), Value::Object(Some(before)), "the parent's slot holds the same object");
    assert!(!x.vm.defs.is_unloaded(&x.vm.space, before), "loaded");
    assert!(!x.vm.defs.is_failed(&x.vm.space, before));
}
