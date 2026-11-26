//! The ammo-definition preload (lane IMPL-UPDATEAMMOCOUNTS, the hub's ruling) on an invented package (AGENTS.md rule
//! 9): a globals definition with the eight ammo fields, one None, one already resident. AI-assisted (Claude Code).

use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::{ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_package::testkit::script::ScriptToy;
use ow_vm::api::Vm;

use super::{ammo_preload_line, preload_ammo_definitions};
use ow_inventory::pickup_take::AMMO;

static NEXT: AtomicU32 = AtomicU32::new(0);
/// The field left None.
const NONE_AT: usize = 4;

fn package() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class", "ObjectProperty"]);
    let object = t.class("Object", 0);
    let globals = t.class("ToyGlobalsDefinition", object);
    for (field, _, _) in AMMO {
        t.prop("Object", globals, field, 0, object);
    }
    t.prop("Object", globals, "Unrelated", 0, object);
    let ammo = t.class("ToyAmmo", object);
    let export = |t: &mut ScriptToy, class: i32, name: &str, tags: Vec<u8>| {
        let mut payload = 0i32.to_le_bytes().to_vec();
        payload.extend(tags);
        payload.extend_from_slice(&t.none);
        t.p.add_export(class, name, payload)
    };
    let mut refs = Vec::new();
    for i in 0..AMMO.len() {
        refs.push(export(&mut t, ammo, &format!("Ammo_{i}"), Vec::new()));
    }
    let unrelated = export(&mut t, ammo, "Unrelated_Thing", Vec::new());
    let mut tags = Vec::new();
    for (i, (field, _, _)) in AMMO.iter().enumerate() {
        if i != NONE_AT {
            tags.extend(t.p.object_tag(field, refs[i]));
        }
    }
    tags.extend(t.p.object_tag("Unrelated", unrelated));
    export(&mut t, globals, "GD_Globals", tags);
    t.build()
}

struct Fx {
    vm: Vm,
    dir: std::path::PathBuf,
    definition: ObjRef,
}

impl Drop for Fx {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn fx() -> Fx {
    let dir = std::env::temp_dir().join(format!("ow-cli-ammo-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = std::fs::remove_dir_all(&dir);
    let bytes = package();
    let defs = DefStore::from_synthetic(&dir, &[("GD_ToyAmmo.upk", &bytes)], IdentityPolicy::FirstInScanOrder, ParityMode::Off)
        .unwrap();
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
    let path = ObjectPath::parse("GD_ToyAmmo.GD_Globals").unwrap();
    let definition = vm.defs.find(&mut vm.space, &path).unwrap().expect("found");
    vm.log.clear();
    Fx { vm, dir, definition }
}

fn field(vm: &Vm, o: ObjRef, name: &str) -> Option<ObjRef> {
    let class = vm.space.heap.get(o).unwrap().class;
    let p = vm.space.prop(class, name).unwrap();
    match vm.space.read(o, PropAddr::scalar(p)).unwrap() {
        Value::Object(o) => *o,
        other => panic!("{other:?}"),
    }
}

fn harness(vm: &Vm) -> Vec<String> {
    vm.log.iter().filter(|l| l.starts_with("harness input:")).cloned().collect()
}

/// The exact load list: the set fields in `AMMO`'s order, each logged before its load; the None field skipped, the
/// resident one neither loaded nor logged, the unrelated reference left unloaded. A second run loads nothing.
#[test]
fn exactly_the_unloaded_ammo_definitions_are_loaded_once() {
    let mut x = fx();
    let resident = field(&x.vm, x.definition, AMMO[1].0).unwrap();
    x.vm.defs.ensure_loaded(&mut x.vm.space, resident).unwrap();
    x.vm.log.clear();
    let before = x.vm.defs.load_transitions();
    let n = preload_ammo_definitions(&mut x.vm, x.definition).unwrap();
    let expected: Vec<usize> = (0..AMMO.len()).filter(|i| *i != NONE_AT && *i != 1).collect();
    assert_eq!(n, expected.len());
    assert_eq!(x.vm.defs.load_transitions() - before, expected.len() as u64, "those objects, nothing else");
    let want: Vec<String> = expected.iter().map(|i| ammo_preload_line(&format!("GD_ToyAmmo.Ammo_{i}"))).collect();
    assert_eq!(harness(&x.vm), want);
    for i in expected {
        let a = field(&x.vm, x.definition, AMMO[i].0).unwrap();
        assert!(!x.vm.defs.is_unloaded(&x.vm.space, a), "{i} loaded");
    }
    let unrelated = field(&x.vm, x.definition, "Unrelated").unwrap();
    assert!(x.vm.defs.is_unloaded(&x.vm.space, unrelated), "nothing outside the eight");
    let loads = x.vm.defs.load_transitions();
    x.vm.log.clear();
    assert_eq!(preload_ammo_definitions(&mut x.vm, x.definition).unwrap(), 0);
    assert_eq!(x.vm.defs.load_transitions(), loads, "a second run loads nothing");
    assert!(harness(&x.vm).is_empty(), "and logs nothing: {:?}", x.vm.log);
}
