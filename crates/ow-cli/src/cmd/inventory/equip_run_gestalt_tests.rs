//! The equip world's gestalt preload ([`super::preload_gestalt_mesh`], lane HARNESS-LINK8-GESTALT-PRELOAD) on an
//! invented `WillowGame` package (AGENTS.md rule 9): a weapon class whose `DefinitionData` names a weapon type, and two
//! weapon types, one whose `GestaltMesh` names a gestalt definition export (an unloaded handle once the type is
//! loaded) and one with none. Off loads the definition and logs it before and after; Reference does nothing.

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

use super::{GESTALT_INPUT, NEW_MESH_INPUT, preload_gestalt_mesh, preload_gestalt_skeletal_mesh};

static NEXT: AtomicU32 = AtomicU32::new(0);

fn package() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class", "ObjectProperty", "StructProperty", "ScriptStruct"]);
    let object = t.class("Object", 0);
    let gestalt = t.class("ToyGestaltDefinition", object);
    let mesh = t.class("ToySkeletalMesh", object);
    t.prop("Object", gestalt, "GestaltSkeletalMesh", 0, mesh);
    let weapon_type = t.class("ToyWeaponTypeDefinition", object);
    t.prop("Object", weapon_type, "GestaltMesh", 0, gestalt);
    let weapon = t.class("ToyWeapon", object);
    let data = toy_struct(&mut t, weapon, "ToyWeaponDefinitionData", None);
    t.prop("Object", data, "WeaponTypeDefinition", 0, weapon_type);
    t.prop("Struct", weapon, "DefinitionData", 0, data);
    // `ToyType_Gestalt` names `ToyGestalt_0` (the export after the two types); `ToyType_Plain` names none.
    let first = t.p.next_export();
    let mut payload = ints(&[0]);
    payload.extend(t.p.object_tag("GestaltMesh", first + 2));
    payload.extend_from_slice(&t.none);
    t.p.add_export(weapon_type, "ToyType_Gestalt", payload);
    let mut payload = ints(&[0]);
    payload.extend_from_slice(&t.none);
    t.p.add_export(weapon_type, "ToyType_Plain", payload);
    // `ToyGestalt_0` names `ToyMesh_0` (the export after it) as its `GestaltSkeletalMesh`.
    let next = t.p.next_export();
    let mut payload = ints(&[0]);
    payload.extend(t.p.object_tag("GestaltSkeletalMesh", next + 1));
    payload.extend_from_slice(&t.none);
    t.p.add_export(gestalt, "ToyGestalt_0", payload);
    let mut payload = ints(&[0]);
    payload.extend_from_slice(&t.none);
    t.p.add_export(mesh, "ToyMesh_0", payload);
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
        "ow-cli-gestalt-preload-{}-{}",
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

    /// A weapon whose `DefinitionData.WeaponTypeDefinition` is `weapon_type`.
    fn weapon(&mut self, weapon_type: Option<ObjRef>) -> ObjRef {
        let class = self.vm.find_class("WillowGame.ToyWeapon").unwrap();
        let weapon = ow_vm::interp::instantiate_from_defaults(&mut self.vm, class).unwrap();
        let p = self.vm.space.prop(class, "DefinitionData").unwrap();
        let mut data = self.vm.space.read(weapon, PropAddr::scalar(p)).unwrap().clone();
        super::super::set_field(&self.vm, &mut data, "WeaponTypeDefinition", Value::Object(weapon_type)).unwrap();
        self.vm.space.write(weapon, PropAddr::scalar(p), data).unwrap();
        weapon
    }

    /// The weapon type's `GestaltMesh` handle.
    fn gestalt_of(&self, weapon_type: ObjRef) -> ObjRef {
        match ow_inventory::equip::read_prop(&self.vm, weapon_type, "GestaltMesh") {
            Some(Value::Object(Some(g))) => g,
            other => panic!("GestaltMesh: {other:?}"),
        }
    }

    /// #518's heap-write and load-transition counts.
    fn counts(&self) -> (u64, u64) {
        (self.vm.space.heap.committed_writes(), self.vm.defs.load_transitions())
    }

    fn harness(&self) -> Vec<String> {
        self.vm.log.iter().filter(|l| l.starts_with(GESTALT_INPUT)).cloned().collect()
    }
}

/// Off: the cold gestalt definition is loaded, and logged before ("UNVERIFIED stand-in") and after ("loaded"). A
/// second call finds it loaded and adds nothing.
#[test]
fn off_loads_the_weapon_types_cold_gestalt_definition_and_logs_it() {
    let mut x = fx(ParityMode::Off);
    let weapon_type = x.find("WillowGame.ToyType_Gestalt");
    let gestalt = x.gestalt_of(weapon_type);
    assert!(x.vm.defs.is_unloaded(&x.vm.space, gestalt), "cold before the preload");
    let weapon = x.weapon(Some(weapon_type));
    preload_gestalt_mesh(&mut x.vm, weapon).unwrap();
    assert!(!x.vm.defs.is_unloaded(&x.vm.space, gestalt), "loaded by the preload");
    let lines = x.harness();
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(lines[0].starts_with(GESTALT_INPUT) && lines[0].contains("ToyGestalt_0"), "{lines:?}");
    assert!(lines[0].ends_with("UNVERIFIED stand-in)"), "{lines:?}");
    assert!(lines[1].ends_with("ToyGestalt_0: loaded"), "{lines:?}");
    // The warm handle: nothing logged, written or loaded (review r1 of #632, item 2).
    let before = x.counts();
    preload_gestalt_mesh(&mut x.vm, weapon).unwrap();
    assert_eq!(x.harness().len(), 2, "already loaded: nothing more");
    assert_eq!(x.counts(), before, "the warm handle writes and loads nothing");
}

/// No `GestaltMesh`, or no weapon type: nothing is loaded or logged.
#[test]
fn no_gestalt_mesh_or_no_weapon_type_is_nothing() {
    let mut x = fx(ParityMode::Off);
    let plain = x.find("WillowGame.ToyType_Plain");
    for weapon_type in [Some(plain), None] {
        let weapon = x.weapon(weapon_type);
        let before = x.counts();
        preload_gestalt_mesh(&mut x.vm, weapon).unwrap();
        assert_eq!(x.counts(), before);
    }
    assert!(x.harness().is_empty(), "{:?}", x.harness());
}

/// Reference: nothing, the definition stays cold.
#[test]
fn reference_preloads_nothing() {
    let mut x = fx(ParityMode::Reference);
    let weapon_type = x.find("WillowGame.ToyType_Gestalt");
    let gestalt = x.gestalt_of(weapon_type);
    let weapon = x.weapon(Some(weapon_type));
    let before = x.counts();
    preload_gestalt_mesh(&mut x.vm, weapon).unwrap();
    assert_eq!(x.counts(), before, "Reference writes and loads nothing (review r1 of #632, item 2)");
    assert!(x.vm.defs.is_unloaded(&x.vm.space, gestalt), "still cold");
    assert!(x.harness().is_empty());
}

impl Fx {
    fn new_mesh_lines(&self) -> Vec<String> {
        self.vm.log.iter().filter(|l| l.starts_with(NEW_MESH_INPUT)).cloned().collect()
    }

    fn mesh_of(&self, gestalt: ObjRef) -> ObjRef {
        match ow_inventory::equip::read_prop(&self.vm, gestalt, "GestaltSkeletalMesh") {
            Some(Value::Object(Some(m))) => m,
            other => panic!("GestaltSkeletalMesh: {other:?}"),
        }
    }
}

/// Lane LINK8-NEWMESH-PRELOAD, in the set-up order `prepare_equip` runs before `init`: the gestalt preload, then the
/// new-mesh preload. The cold `GestaltSkeletalMesh` is loaded and logged before ("UNVERIFIED stand-in") and after
/// ("loaded"); a second call finds it loaded and adds, writes and loads nothing.
#[test]
fn off_loads_the_gestalt_definitions_cold_mesh_after_the_gestalt_preload() {
    let mut x = fx(ParityMode::Off);
    let weapon_type = x.find("WillowGame.ToyType_Gestalt");
    let gestalt = x.gestalt_of(weapon_type);
    let weapon = x.weapon(Some(weapon_type));
    // Before the gestalt preload the definition is cold: the new-mesh preload does nothing.
    let before = x.counts();
    preload_gestalt_skeletal_mesh(&mut x.vm, weapon).unwrap();
    assert_eq!(x.counts(), before, "a cold definition: nothing");
    assert!(x.new_mesh_lines().is_empty());
    preload_gestalt_mesh(&mut x.vm, weapon).unwrap();
    let mesh = x.mesh_of(gestalt);
    assert!(x.vm.defs.is_unloaded(&x.vm.space, mesh), "cold before the preload");
    preload_gestalt_skeletal_mesh(&mut x.vm, weapon).unwrap();
    assert!(!x.vm.defs.is_unloaded(&x.vm.space, mesh), "loaded before init");
    let lines = x.new_mesh_lines();
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(lines[0].contains("ToyMesh_0") && lines[0].ends_with("UNVERIFIED stand-in)"), "{lines:?}");
    assert!(lines[1].ends_with("ToyMesh_0: loaded"), "{lines:?}");
    let before = x.counts();
    preload_gestalt_skeletal_mesh(&mut x.vm, weapon).unwrap();
    assert_eq!(x.new_mesh_lines().len(), 2, "already loaded: nothing more");
    assert_eq!(x.counts(), before, "the warm handle writes and loads nothing");
}

/// No weapon type, a type without a gestalt definition, and Reference: nothing loaded or logged.
#[test]
fn the_new_mesh_preload_is_nothing_without_a_gestalt_and_under_reference() {
    let mut x = fx(ParityMode::Off);
    let plain = x.find("WillowGame.ToyType_Plain");
    for weapon_type in [Some(plain), None] {
        let weapon = x.weapon(weapon_type);
        let before = x.counts();
        preload_gestalt_skeletal_mesh(&mut x.vm, weapon).unwrap();
        assert_eq!(x.counts(), before);
    }
    assert!(x.new_mesh_lines().is_empty());
    let mut r = fx(ParityMode::Reference);
    let weapon_type = r.find("WillowGame.ToyType_Gestalt");
    let weapon = r.weapon(Some(weapon_type));
    let before = r.counts();
    preload_gestalt_skeletal_mesh(&mut r.vm, weapon).unwrap();
    assert_eq!(r.counts(), before, "Reference writes and loads nothing");
    assert!(r.new_mesh_lines().is_empty());
}
