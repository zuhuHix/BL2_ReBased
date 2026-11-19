//! `ItemActors` driven by assembled script loops through the VM's engine-iterator path (review r1 of #455). An
//! invented `WillowGame.upk` (AGENTS.md rule 9): a manager class with an item chain and the native iterator, an
//! inventory base class with `Inventory` and `bReadied`, an item class and a weapon class below it, and two script
//! functions on the manager.
//!
//! - `FirstMatch()`: `foreach ItemActors(ItemClass, Cursor, false) { if (!bOnly || Cursor.bReadied) return Cursor; }
//!   return None;`. This is the shape of the inventory resolver's walk (NATIVE_INVENTORY_CONTEXT.md section 2) with
//!   the definition test left out: the toy has no object comparison native, so "matching" here is the iterator's
//!   class filter, and the readied test is the script's, as the resolver's is.
//! - `NoneBase()`: `foreach ItemActors(None, Cursor, false) { } return Cursor;`, the VM's literal None.

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::ParityMode;
use ow_core::notyet::Missing;
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_package::testkit::script::{Asm, CPF_OUT, FunctionSpec, Param, ScriptToy};
use ow_vm::api::{StubPolicy, Vm};
use ow_vm::error::VmError;

const NATIVE_ITERATOR: u32 = 0x400 | 0x20000 | 0x4;
const SCRIPT: u32 = 0x1 | 0x2 | 0x20000;
static NEXT: AtomicU32 = AtomicU32::new(0);

fn le(words: &[i32]) -> Vec<u8> {
    words.iter().flat_map(|w| w.to_le_bytes()).collect()
}

fn package() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&[
        "Class",
        "Function",
        "ScriptStruct",
        "BoolProperty",
        "ObjectProperty",
        "ClassProperty",
    ]);
    let object = t.class("Object", 0);
    let inventory = t.class("Inventory", object);
    t.prop("Object", inventory, "Inventory", 0, inventory);
    let readied = t.prop("Bool", inventory, "bReadied", 0, 0);
    let item = t.class("ToyItem", inventory);
    t.class("ToyWeapon", inventory);
    let manager = t.class("WillowInventoryManager", object);
    t.prop("Object", manager, "ItemChain", 0, inventory);
    let cursor = t.prop("Object", manager, "Cursor", 0, inventory);
    let only = t.prop("Bool", manager, "bOnly", 0, 0);
    let class_import = t.import("Class");
    let item_class = t.prop_tail("Class", manager, "ItemClass", 0, &le(&[class_import, item]));
    let item_actors = t
        .function(
            manager,
            "ItemActors",
            &[
                Param { type_ref: inventory, ..Param::new("Object", "BaseClass") },
                Param { flags: CPF_OUT, type_ref: inventory, ..Param::new("Object", "Inv") },
                Param::new("Bool", "bOnlyReadied"),
            ],
            None,
            &[],
            &FunctionSpec::new(NATIVE_ITERATOR, "ItemActors"),
            |_| None,
        )
        .function;
    // FirstMatch (module documentation).
    t.function(
        manager,
        "FirstMatch",
        &[],
        Some(("Object", inventory)),
        &[],
        &FunctionSpec::new(SCRIPT, "FirstMatch"),
        |_| {
            let mut a = Asm::new();
            a.stmt().raw(&[0x2F]).raw(&[0x1C]).reference(item_actors);
            a.instance(item_class).instance(cursor).false_().raw(&[0x16]);
            let end = a.bytes.len();
            a.w(0);
            // `if (bOnly) { if (!Cursor.bReadied) goto next; }`, written with two conditional jumps so the toy needs
            // no negation native (the stub policy refuses every native the test does not register).
            a.stmt();
            let to_return = a.jump_if_not(|a| {
                a.instance(only);
            });
            a.stmt();
            let to_next = a.jump_if_not(|a| {
                a.context(|a| {
                    a.instance(cursor);
                }, |a| {
                    a.instance(readied);
                });
            });
            a.patch(to_return);
            a.stmt().raw(&[0x30]);
            a.stmt().raw(&[0x04]).instance(cursor);
            a.patch(to_next);
            a.stmt().raw(&[0x31]);
            a.patch(end);
            a.stmt().raw(&[0x30]);
            a.stmt().raw(&[0x04, 0x2A]);
            a.end();
            Some(a)
        },
    );
    // NoneBase (module documentation).
    t.function(
        manager,
        "NoneBase",
        &[],
        Some(("Object", inventory)),
        &[],
        &FunctionSpec::new(SCRIPT, "NoneBase"),
        |_| {
            let mut a = Asm::new();
            a.stmt().raw(&[0x2F]).raw(&[0x1C]).reference(item_actors);
            a.raw(&[0x2A]).instance(cursor).false_().raw(&[0x16]);
            let end = a.bytes.len();
            a.w(0);
            a.stmt().raw(&[0x31]);
            a.patch(end);
            a.stmt().raw(&[0x30]);
            a.stmt().raw(&[0x04]).instance(cursor);
            a.end();
            Some(a)
        },
    );
    t.p.build()
}

struct Fx {
    vm: Vm,
    manager: ObjRef,
    dir: PathBuf,
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn fixture() -> Fx {
    let dir = std::env::temp_dir().join(format!(
        "ow-inventory-itemactors-script-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let bytes = package();
    let defs =
        DefStore::from_synthetic(&dir, &[("WillowGame.upk", &bytes)], IdentityPolicy::FirstInScanOrder, ParityMode::Off)
            .unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off)
        .with_stub_policy(StubPolicy::refuse_all());
    let class = vm.find_class("WillowGame.WillowInventoryManager").unwrap();
    let manager = ow_vm::interp::instantiate_from_defaults(&mut vm, class).unwrap();
    super::register_iterators(&mut vm).unwrap();
    Fx { vm, manager, dir }
}

impl Fx {
    fn obj(&mut self, class: &str, name: &str) -> ObjRef {
        let c = self.vm.find_class(&format!("WillowGame.{class}")).unwrap();
        self.vm.space.alloc(c, name, None, Origin::Runtime).unwrap()
    }

    fn set(&mut self, o: ObjRef, name: &str, v: Value) {
        let class = self.vm.space.heap.get(o).unwrap().class;
        let p = self.vm.space.prop(class, name).unwrap();
        self.vm.space.write(o, PropAddr::scalar(p), v).unwrap();
    }

    fn call(&mut self, name: &str) -> Result<Value, VmError> {
        let f = self.vm.find_function(&format!("WillowGame.WillowInventoryManager.{name}"))?;
        let m = self.manager;
        Ok(self.vm.call(f, Some(m), vec![])?.ret)
    }
}

/// A weapon at the head (skipped by the class filter), then two items: the first unreadied, the second readied.
/// Without the readied test the first item wins; with it, the second.
#[test]
fn a_script_foreach_takes_the_first_match_and_the_readied_one_with_the_flag() {
    let mut x = fixture();
    let head = x.obj("ToyWeapon", "Head");
    let first = x.obj("ToyItem", "First");
    let second = x.obj("ToyItem", "Second");
    x.set(second, "bReadied", Value::Bool(true));
    x.set(head, "bReadied", Value::Bool(true));
    x.set(head, "Inventory", Value::Object(Some(first)));
    x.set(first, "Inventory", Value::Object(Some(second)));
    let m = x.manager;
    x.set(m, "ItemChain", Value::Object(Some(head)));
    let item = x.vm.find_class("WillowGame.ToyItem").unwrap();
    x.set(m, "ItemClass", Value::Class(Some(item)));
    assert_eq!(x.call("FirstMatch").unwrap(), Value::Object(Some(first)), "the first match wins");
    x.set(m, "bOnly", Value::Bool(true));
    assert_eq!(x.call("FirstMatch").unwrap(), Value::Object(Some(second)), "the readied match wins");
    // No readied item: the loop runs to its end and the answer is None.
    x.set(second, "bReadied", Value::Bool(false));
    assert_eq!(x.call("FirstMatch").unwrap(), Value::Object(None));
}

/// The VM's literal None as `BaseClass` is the None class: `NotYet { NotRead }`, not a data error.
#[test]
fn a_literal_none_base_class_through_the_vm_is_not_yet() {
    let mut x = fixture();
    let item = x.obj("ToyItem", "Only");
    let m = x.manager;
    x.set(m, "ItemChain", Value::Object(Some(item)));
    match x.call("NoneBase") {
        Err(VmError::NotYet(n)) => {
            assert_eq!(n.missing, Missing::NotRead);
            assert!(n.what.contains("None BaseClass"), "{}", n.what);
        }
        other => panic!("expected NotYet, got {other:?}"),
    }
}
