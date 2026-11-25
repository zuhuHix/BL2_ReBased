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

use super::{MATERIAL_CLASS_INPUT, MATERIAL_INPUT, preload_material_classes, preload_material_part_material};

static NEXT: AtomicU32 = AtomicU32::new(0);

/// The material classes `BuildWeaponMaterial` asks for (`Engine`), none warmed.
fn engine() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class", "ObjectProperty"]);
    let object = t.class("Object", 0);
    let interface = t.class("MaterialInterface", object);
    let mic = t.class("MaterialInstanceConstant", interface);
    t.class("ToyMIC", mic);
    t.build()
}

fn package() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class", "ObjectProperty", "StructProperty", "ScriptStruct", "ClassProperty"]);
    let object = t.class("Object", 0);
    let engine = t.p.add_import("Core", "Package", 0, "Engine");
    let interface = t.p.add_import("Core", "Class", engine, "MaterialInterface");
    let weapon_type = t.class("ToyWeaponType", object);
    t.prop_tail("Class", weapon_type, "MaterialClass", 0, &ints(&[object, interface]));
    let material = t.class("ToyMaterialInstance", object);
    let part = t.class("ToyMaterialPart", object);
    t.prop("Object", part, "Material", 0, material);
    let holder = t.class("ToyPartHolder", object);
    t.prop("Object", holder, "Part", 0, part);
    let weapon = t.class("ToyWeapon", object);
    let data = toy_struct(&mut t, weapon, "ToyWeaponDefinitionData", None);
    t.prop("Object", data, "MaterialPartDefinition", 0, part);
    t.prop("Object", data, "WeaponTypeDefinition", 0, weapon_type);
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
    let (e, bytes) = (engine(), package());
    let defs = DefStore::from_synthetic(
        &dir,
        &[("Engine.upk", e.as_slice()), ("WillowGame.upk", bytes.as_slice())],
        IdentityPolicy::FirstInScanOrder,
        mode,
    )
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

impl Fx {
    /// A weapon whose `DefinitionData.WeaponTypeDefinition` is a runtime weapon type with `MaterialClass` set to the
    /// class at `class` (its layout defined through the code index, its code not loaded), or None.
    fn typed_weapon(&mut self, class: Option<&str>) -> (ObjRef, Option<ow_object::ids::ClassId>) {
        let material_class = class.map(|path| {
            let (code, store) = self.vm.defs.code_mut();
            let key = code.find_class(store, path).unwrap();
            code.define_layout(store, &mut self.vm.space.names, &mut self.vm.space.classes, key).unwrap()
        });
        let type_class = self.vm.find_class("WillowGame.ToyWeaponType").unwrap();
        let weapon_type = ow_vm::interp::instantiate_from_defaults(&mut self.vm, type_class).unwrap();
        let p = self.vm.space.prop(type_class, "MaterialClass").unwrap();
        self.vm.space.write(weapon_type, PropAddr::scalar(p), Value::Class(material_class)).unwrap();
        let weapon = self.weapon(None);
        let class = self.vm.space.heap.get(weapon).unwrap().class;
        let p = self.vm.space.prop(class, "DefinitionData").unwrap();
        let mut data = self.vm.space.read(weapon, PropAddr::scalar(p)).unwrap().clone();
        super::super::set_field(&self.vm, &mut data, "WeaponTypeDefinition", Value::Object(Some(weapon_type))).unwrap();
        self.vm.space.write(weapon, PropAddr::scalar(p), data).unwrap();
        (weapon, material_class)
    }

    fn class_lines(&self) -> Vec<&str> {
        self.vm.log.iter().filter(|l| l.starts_with(MATERIAL_CLASS_INPUT)).map(String::as_str).collect()
    }

    fn has_default_of(&self, c: ow_object::ids::ClassId) -> bool {
        self.vm.space.heap.handles().into_iter().any(|o| {
            self.vm.space.heap.get(o).is_ok_and(|x| x.class == c && matches!(x.origin, ow_object::heap::Origin::Default { .. }))
        })
    }
}

/// The material classes' set-up load (the hub's extension of this lane): `MaterialInstanceConstant`'s code, the
/// `MaterialClass`'s code, then its default object, each logged before and after; a second call does nothing.
#[test]
fn off_loads_the_material_classes_and_the_class_default() {
    let mut x = fx(ParityMode::Off);
    let (weapon, class) = x.typed_weapon(Some("Engine.ToyMIC"));
    let class = class.unwrap();
    assert!(!x.vm.class_code_loaded(class) && !x.has_default_of(class), "cold before");
    preload_material_classes(&mut x.vm, weapon).unwrap();
    let mic = x.vm.space.classes.by_path("Engine.MaterialInstanceConstant").unwrap();
    assert!(x.vm.class_code_loaded(mic) && x.vm.class_code_loaded(class), "both classes' code loaded");
    assert!(x.has_default_of(class), "the class default is resident");
    let lines = x.class_lines();
    assert_eq!(lines.len(), 6, "{lines:?}");
    assert!(lines[0].contains("the class code of Engine.MaterialInstanceConstant (loaded at boot"), "{}", lines[0]);
    assert!(lines[1].ends_with("the class code of Engine.MaterialInstanceConstant: loaded"), "{}", lines[1]);
    assert!(lines[3].ends_with("the class code of Engine.ToyMIC: loaded"), "{}", lines[3]);
    assert!(lines[5].ends_with("the default object of Engine.ToyMIC: loaded"), "{}", lines[5]);
    let (before, n) = (x.counts(), x.vm.log.len());
    preload_material_classes(&mut x.vm, weapon).unwrap();
    assert_eq!((x.counts(), x.vm.log.len()), (before, n), "already ready: nothing");
}

/// No `MaterialClass` (and no weapon type): only `MaterialInstanceConstant`'s code; Reference: nothing.
#[test]
fn without_a_material_class_only_the_family_class_loads_and_reference_does_nothing() {
    let mut x = fx(ParityMode::Off);
    let (weapon, _) = x.typed_weapon(None);
    preload_material_classes(&mut x.vm, weapon).unwrap();
    assert_eq!(x.class_lines().len(), 2, "{:?}", x.class_lines());
    let untyped = x.weapon(None);
    let n = x.vm.log.len();
    preload_material_classes(&mut x.vm, untyped).unwrap();
    assert_eq!(x.vm.log.len(), n, "the family class is already ready");

    let mut r = fx(ParityMode::Reference);
    let (weapon, _) = r.typed_weapon(Some("Engine.ToyMIC"));
    preload_material_classes(&mut r.vm, weapon).unwrap();
    assert!(r.class_lines().is_empty());
}
