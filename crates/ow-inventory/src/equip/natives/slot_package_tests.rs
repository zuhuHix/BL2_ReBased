//! Lane FIX-ATTRIBUTE-SLOTS: `ApplyExternalSlotEffectModifiers`' slot read on an invented `WillowGame.upk` (AGENTS.md
//! rule 9: invented classes and values, built with the package test kit). The item class declares `AttributeSlots` as
//! a static array of slot structs, as the installed `WillowInventory` declares `AttributeSlots[19]`. Here the
//! dimension is 3, so the read is shown to take its count from the package's declaration, not from a constant.

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::ParityMode;
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_package::testkit::reflect::{property_payload, toy_struct};
use ow_package::testkit::script::ScriptToy;
use ow_package::testkit::{Export, ints};
use ow_vm::api::Vm;

use super::{attribute_slots, external_slot_effects};

/// The invented dimension of `AttributeSlots`.
const DIM: u32 = 3;
static NEXT: AtomicU32 = AtomicU32::new(0);

/// `WillowInventory` with `AttributeSlots[DIM]` of `AttributeSlotData` (the six fields the native reads).
fn package() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&[
        "Class",
        "ScriptStruct",
        "BoolProperty",
        "ByteProperty",
        "FloatProperty",
        "ObjectProperty",
        "StructProperty",
    ]);
    let object = t.class("Object", 0);
    let inventory = t.class("WillowInventory", object);
    let data = toy_struct(&mut t, inventory, "AttributeSlotData", None);
    for flag in ["bActivated", "bExternalSlot", "bRunEffectsAsSkill"] {
        t.prop("Bool", data, flag, 0, 0);
    }
    t.prop("Object", data, "AttributeToModify", 0, object);
    t.prop("Byte", data, "ModifierType", 0, 0);
    t.prop("Float", data, "ComputedModifierValue", 0, 0);
    let payload = property_payload(&t.none, DIM, 0, &ints(&[data]));
    let mut slots = Export::new(t.import("StructProperty"), "AttributeSlots", payload);
    slots.outer = inventory;
    t.p.add_export_with(slots);
    t.p.build()
}

struct Fx {
    vm: Vm,
    item: ObjRef,
    dir: PathBuf,
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn fixture() -> Fx {
    let dir = std::env::temp_dir().join(format!(
        "ow-inventory-attribute-slots-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let bytes = package();
    let defs = DefStore::from_synthetic(
        &dir,
        &[("WillowGame.upk", &bytes)],
        IdentityPolicy::FirstInScanOrder,
        ParityMode::Off,
    )
    .unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
    let class = vm.find_class("WillowGame.WillowInventory").unwrap();
    let item = ow_vm::interp::instantiate_from_defaults(&mut vm, class).unwrap();
    Fx { vm, item, dir }
}

impl Fx {
    /// Slot `i`: activated and external, not a skill, on `attribute`, PostAdd `value`.
    fn set_slot(&mut self, i: u32, attribute: ObjRef, value: f32) {
        let class = self.vm.space.heap.get(self.item).unwrap().class;
        let p = self.vm.space.prop(class, "AttributeSlots").unwrap();
        let at = PropAddr::element(p, i);
        let mut slot = self.vm.space.read(self.item, at).unwrap().clone();
        let ty = slot.as_struct().unwrap().ty;
        for (name, v) in [
            ("bActivated", Value::Bool(true)),
            ("bExternalSlot", Value::Bool(true)),
            ("bRunEffectsAsSkill", Value::Bool(false)),
            ("AttributeToModify", Value::Object(Some(attribute))),
            ("ModifierType", Value::Byte(2)),
            ("ComputedModifierValue", Value::Float(value)),
        ] {
            let id = self.vm.space.names.find(name).unwrap();
            let (k, _) = self.vm.space.classes.struct_layout(ty).unwrap().field(id).unwrap();
            slot.struct_mut().unwrap().fields[usize::try_from(k).unwrap()] = v;
        }
        self.vm.space.write(self.item, at, slot).unwrap();
    }
}

#[test]
fn the_slots_are_read_as_the_package_declares_them_a_static_array() {
    let mut x = fixture();
    let class = x.vm.space.heap.get(x.item).unwrap().class;
    let p = x.vm.space.prop(class, "AttributeSlots").unwrap();
    assert_eq!(x.vm.space.classes.decl(p).unwrap().array_dim, 3, "the dimension comes from the package");
    let object = x.vm.find_class("WillowGame.Object").unwrap();
    let attribute = x.vm.space.alloc(object, "Test.Attribute", None, Origin::Runtime).unwrap();
    // The first and the last declared slots hold an effect; the middle one keeps its zeroed default (not activated).
    x.set_slot(0, attribute, 2.0);
    x.set_slot(DIM - 1, attribute, 0.25);
    assert_eq!(attribute_slots(&x.vm, x.item, 19).unwrap().len(), 3, "a count above the dimension reads all three");
    let all = external_slot_effects(&x.vm, x.item, 19).unwrap();
    let values: Vec<f32> = all.iter().map(|e| e.base_modifier_value.base_value_constant).collect();
    assert_eq!(values, [2.0, 0.25], "every declared slot, in index order; a count above the dimension stops at it");
    assert_eq!(external_slot_effects(&x.vm, x.item, 2).unwrap().len(), 1, "the count cuts before the last slot");
}
