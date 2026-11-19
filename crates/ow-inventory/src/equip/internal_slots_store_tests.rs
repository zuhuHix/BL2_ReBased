//! P0 of `ApplyInternalSlotEffectModifiers` on an invented `WillowGame.upk` in a synthetic store (AGENTS.md rule 9;
//! review r1 of #483). Loading `ToyHolder` leaves its references as store handles not yet loaded: `ToyAttr` (whose
//! value chain names `ToyResolver`) and `ToyBroken` (a payload that cannot be read). AI-assisted (Claude Code).

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::{ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_object::ids::ObjRef;
use ow_object::value::Value;
use ow_package::testkit::script::ScriptToy;
use ow_vm::api::Vm;

use super::load_attribute;

static NEXT: AtomicU32 = AtomicU32::new(0);

fn package() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class", "ObjectProperty", "ArrayProperty"]);
    let object = t.class("Object", 0);
    let attr = t.class("ToyAttribute", object);
    for chain in ["ContextResolverChain", "ValueResolverChain"] {
        let inner = t.p.next_export();
        t.prop("Array", attr, chain, 0, inner + 1);
        t.prop("Object", inner, chain, 0, object);
    }
    let holder = t.class("Holder", object);
    t.prop("Object", holder, "Attr", 0, object);
    t.prop("Object", holder, "Broken", 0, object);
    let mut empty = 0i32.to_le_bytes().to_vec();
    empty.extend_from_slice(&t.none);
    let resolver = t.p.add_export(object, "ToyResolver", empty);
    let mut a = 0i32.to_le_bytes().to_vec();
    a.extend(t.p.array_tag("ValueResolverChain", 1, &resolver.to_le_bytes()));
    a.extend_from_slice(&t.none);
    let toy_attr = t.p.add_export(attr, "ToyAttr", a);
    // A payload that ends inside its first tag: the store cannot read it.
    let broken = t.p.add_export(attr, "ToyBroken", vec![0, 0, 0, 0, 7]);
    let mut h = 0i32.to_le_bytes().to_vec();
    h.extend(t.p.object_tag("Attr", toy_attr));
    h.extend(t.p.object_tag("Broken", broken));
    h.extend_from_slice(&t.none);
    t.p.add_export(holder, "ToyHolder", h);
    t.p.build()
}

struct Fx {
    vm: Vm,
    dir: PathBuf,
    attr: ObjRef,
    broken: ObjRef,
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn fx() -> Fx {
    let dir = std::env::temp_dir().join(format!("ow-inventory-internal-slots-store-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let bytes = package();
    let defs = DefStore::from_synthetic(&dir, &[("WillowGame.upk", &bytes)], IdentityPolicy::FirstInScanOrder, ParityMode::Off).unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
    let path = ObjectPath::parse("WillowGame.ToyHolder").unwrap();
    let holder = vm.defs.find(&mut vm.space, &path).unwrap().unwrap();
    let get = |vm: &Vm, n: &str| match crate::equip::read_prop(vm, holder, n) {
        Some(Value::Object(Some(o))) => o,
        other => panic!("{other:?}"),
    };
    let (attr, broken) = (get(&vm, "Attr"), get(&vm, "Broken"));
    assert!(vm.defs.is_unloaded(&vm.space, attr) && vm.defs.is_unloaded(&vm.space, broken));
    Fx { vm, dir, attr, broken }
}

/// P0 loads an unloaded attribute and every entry of its resolver chains.
#[test]
fn p0_loads_the_attribute_and_its_chain_entries() {
    let mut x = fx();
    let attr = x.attr;
    load_attribute(&mut x.vm, attr).unwrap();
    assert!(!x.vm.defs.is_unloaded(&x.vm.space, attr), "the attribute is loaded");
    let chain = crate::equip::read_prop(&x.vm, attr, "ValueResolverChain").unwrap();
    let entry = ow_attr::eval::object("e", &chain.elements().unwrap()[0]).unwrap().unwrap();
    assert!(!x.vm.defs.is_unloaded(&x.vm.space, entry), "its value chain's entry is loaded");
}

/// A load that fails is returned as the store gives it (typed, through `VmError::from`), with no partial-writes mark:
/// P0 runs before any write.
#[test]
fn p0_returns_a_failed_load_unchanged() {
    let mut x = fx();
    let broken = x.broken;
    let e = load_attribute(&mut x.vm, broken).unwrap_err();
    assert!(!format!("{e:?}").contains("partial writes"), "{e:?}");
}
