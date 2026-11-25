//! The equip world's Material-part material preload ([`super::preload_material_part_material`], lane
//! LINK-8-STARTUP-MATERIAL-PRELOAD) on an invented `WillowGame` package (AGENTS.md rule 9). The package has a weapon
//! class whose `DefinitionData` names a material part, parts whose `Material` names a material export (an unloaded
//! handle once the part is loaded), a part with none, a part whose material is cut short, and a holder that leaves a
//! part itself unloaded. Off loads the material and logs it before and after; Reference does nothing.

use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::ParityMode;
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_package::testkit::ints;
use ow_package::testkit::reflect::toy_struct;
use ow_package::testkit::script::ScriptToy;
use ow_vm::api::Vm;

use super::{MATERIAL_INPUT, preload_material_part_material};

static NEXT: AtomicU32 = AtomicU32::new(0);

fn package() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class", "ObjectProperty", "StructProperty", "ScriptStruct"]);
    let object = t.class("Object", 0);
    let material = t.class("ToyMaterialInstance", object);
    let part = t.class("ToyMaterialPart", object);
    t.prop("Object", part, "Material", 0, material);
    let holder = t.class("ToyPartHolder", object);
    t.prop("Object", holder, "Part", 0, part);
    let weapon = t.class("ToyWeapon", object);
    let data = toy_struct(&mut t, weapon, "ToyWeaponDefinitionData", None);
    t.prop("Object", data, "MaterialPartDefinition", 0, part);
    t.prop("Struct", weapon, "DefinitionData", 0, data);
    // `ToyPart_Mat` names `ToyMati_0` (the export after it).
    let next = t.p.next_export();
    let part_mat = next;
    let mut payload = ints(&[0]);
    payload.extend(t.p.object_tag("Material", next + 1));
    payload.extend_from_slice(&t.none);
    t.p.add_export(part, "ToyPart_Mat", payload);
    let mut payload = ints(&[0]);
    payload.extend_from_slice(&t.none);
    t.p.add_export(material, "ToyMati_0", payload);
    // `ToyPart_Plain` names none.
    let mut payload = ints(&[0]);
    payload.extend_from_slice(&t.none);
    t.p.add_export(part, "ToyPart_Plain", payload);
    // `ToyPart_Broken` names `ToyMati_Cut` (cut short: no terminator).
    let next = t.p.next_export();
    let mut payload = ints(&[0]);
    payload.extend(t.p.object_tag("Material", next + 1));
    payload.extend_from_slice(&t.none);
    t.p.add_export(part, "ToyPart_Broken", payload);
    t.p.add_export(material, "ToyMati_Cut", ints(&[0]));
    // `ToyHolder_0` names `ToyPart_Mat`, so the part is a store handle not loaded yet when reached through it.
    let mut payload = ints(&[0]);
    payload.extend(t.p.object_tag("Part", part_mat));
    payload.extend_from_slice(&t.none);
    t.p.add_export(holder, "ToyHolder_0", payload);
    t.p.build()
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

fn fx(mode: ParityMode) -> Fx {
    let dir = std::env::temp_dir().join(format!(
        "ow-cli-material-preload-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let bytes = package();
    let defs = DefStore::from_synthetic(&dir, &[("WillowGame.upk", &bytes)], IdentityPolicy::FirstInScanOrder, mode)
        .unwrap();
    let vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), mode);
    Fx { vm, dir }
}

impl Fx {
    fn find(&mut self, path: &str) -> ObjRef {
        let path = ow_core::ObjectPath::parse(path).unwrap();
        self.vm.defs.find(&mut self.vm.space, &path).unwrap().unwrap()
    }

    /// A weapon whose `DefinitionData.MaterialPartDefinition` is `part`.
    fn weapon(&mut self, part: Option<ObjRef>) -> ObjRef {
        let class = self.vm.find_class("WillowGame.ToyWeapon").unwrap();
        let weapon = ow_vm::interp::instantiate_from_defaults(&mut self.vm, class).unwrap();
        let p = self.vm.space.prop(class, "DefinitionData").unwrap();
        let mut data = self.vm.space.read(weapon, PropAddr::scalar(p)).unwrap().clone();
        super::super::set_field(&self.vm, &mut data, "MaterialPartDefinition", Value::Object(part)).unwrap();
        self.vm.space.write(weapon, PropAddr::scalar(p), data).unwrap();
        weapon
    }

    fn material_of(&self, part: ObjRef) -> ObjRef {
        match ow_inventory::equip::read_prop(&self.vm, part, "Material") {
            Some(Value::Object(Some(m))) => m,
            other => panic!("Material: {other:?}"),
        }
    }

    fn lines(&self) -> Vec<&str> {
        self.vm.log.iter().filter(|l| l.starts_with(MATERIAL_INPUT)).map(String::as_str).collect()
    }

    /// #518's heap-write and load-transition counts, with the class-code loads.
    fn counts(&self) -> (u64, u64) {
        (self.vm.space.heap.committed_writes(), self.vm.defs.load_transitions() + self.vm.class_code_loads())
    }
}

/// Off: the part's unloaded material is loaded through the store, logged before and after; a second call does
/// nothing.
#[test]
fn off_loads_the_material_part_s_material_and_logs_it() {
    let mut x = fx(ParityMode::Off);
    let part = x.find("WillowGame.ToyPart_Mat");
    let weapon = x.weapon(Some(part));
    let material = x.material_of(part);
    assert!(x.vm.defs.is_unloaded(&x.vm.space, material), "an unloaded handle before");
    preload_material_part_material(&mut x.vm, weapon).unwrap();
    assert!(!x.vm.defs.is_unloaded(&x.vm.space, material), "loaded");
    let lines = x.lines();
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(lines[0].contains("WillowGame.ToyMati_0") && lines[0].contains("UNVERIFIED stand-in"), "{}", lines[0]);
    assert!(lines[1].ends_with("WillowGame.ToyMati_0: loaded"), "{}", lines[1]);
    let (before, n) = (x.counts(), x.vm.log.len());
    preload_material_part_material(&mut x.vm, weapon).unwrap();
    assert_eq!((x.counts(), x.vm.log.len()), (before, n), "already loaded: nothing");
}

/// No part, a part with no material, and a part that is itself an unloaded handle: nothing, no line, no load.
#[test]
fn nothing_without_a_resident_part_that_names_a_material() {
    let mut x = fx(ParityMode::Off);
    let plain = x.find("WillowGame.ToyPart_Plain");
    let holder = x.find("WillowGame.ToyHolder_0");
    let cold_part = match ow_inventory::equip::read_prop(&x.vm, holder, "Part") {
        Some(Value::Object(Some(p))) => p,
        other => panic!("Part: {other:?}"),
    };
    assert!(x.vm.defs.is_unloaded(&x.vm.space, cold_part), "the part reached through the holder is not loaded");
    for part in [None, Some(plain), Some(cold_part)] {
        let weapon = x.weapon(part);
        let before = x.counts();
        preload_material_part_material(&mut x.vm, weapon).unwrap();
        assert_eq!(x.counts(), before, "{part:?}: no write, no load");
        assert!(x.lines().is_empty(), "{part:?}: no line");
    }
    assert!(x.vm.defs.is_unloaded(&x.vm.space, cold_part), "the part is not loaded by the preload");
}

/// A material whose load fails: the error is kept and logged after the line that announced it.
#[test]
fn a_failed_material_load_keeps_its_error() {
    let mut x = fx(ParityMode::Off);
    let part = x.find("WillowGame.ToyPart_Broken");
    let weapon = x.weapon(Some(part));
    assert!(preload_material_part_material(&mut x.vm, weapon).is_err());
    let lines = x.lines();
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(lines[1].contains("WillowGame.ToyMati_Cut: failed ("), "{}", lines[1]);
}

/// Reference: nothing is loaded or logged.
#[test]
fn reference_does_nothing() {
    let mut x = fx(ParityMode::Reference);
    let part = x.find("WillowGame.ToyPart_Mat");
    let weapon = x.weapon(Some(part));
    let material = x.material_of(part);
    preload_material_part_material(&mut x.vm, weapon).unwrap();
    assert!(x.vm.defs.is_unloaded(&x.vm.space, material), "still unloaded");
    assert!(x.lines().is_empty());
}
