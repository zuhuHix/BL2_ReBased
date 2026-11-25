//! The slice harness's two mesh inputs (lane SLICE-HARNESS-MAYA-MESH, hub ruling), on an invented `WillowGame` package
//! (AGENTS.md rule 9): the mesh preload loads through the store and logs the attempt and its outcome; the skeleton
//! source is installed once, logged with its roots; the roots come from `OPENWILLOW_PSK_EXPORTS` or the default.

use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::AttributeSystem;
use ow_core::ParityMode;
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_package::testkit::script::ScriptToy;
use ow_vm::api::Vm;
use ow_world::world::skeletal::SkeletalMeshes;

use super::super::{PSK_ROOTS_DEFAULT, install_skeleton_source, parse_psk_roots, preload_mesh};

static NEXT: AtomicU32 = AtomicU32::new(0);

fn package() -> Vec<u8> {
    let mut t = ScriptToy::with_imports(&["Class"]);
    let object = t.class("Object", 0);
    let mesh = t.class("ToyMesh", object);
    let mut payload = 0i32.to_le_bytes().to_vec();
    payload.extend_from_slice(&t.none);
    t.p.add_export(mesh, "ToyBody", payload);
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

fn fx() -> Fx {
    let dir = std::env::temp_dir().join(format!(
        "ow-cli-mesh-harness-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let bytes = package();
    let defs =
        DefStore::from_synthetic(&dir, &[("WillowGame.upk", &bytes)], IdentityPolicy::FirstInScanOrder, ParityMode::Off)
            .unwrap();
    let vm = Vm::new(ObjectSpace::new(), defs, Rc::new(AttributeSystem::new_empty()), ParityMode::Off);
    Fx { vm, dir }
}

fn harness_lines(vm: &Vm) -> Vec<String> {
    vm.log.iter().filter(|l| l.starts_with("harness input:")).cloned().collect()
}

#[test]
fn the_mesh_preload_loads_through_the_store_and_logs_attempt_then_outcome() {
    let mut x = fx();
    let class = x.vm.find_class("WillowGame.ToyMesh").unwrap();
    assert!(x.vm.space.live(class).next().is_none(), "nothing loaded before the preload");
    preload_mesh(&mut x.vm, "WillowGame.ToyBody", "Maya's mesh").unwrap();
    let lines = harness_lines(&x.vm);
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(lines[0].starts_with("harness input: preload WillowGame.ToyBody (Maya's mesh; UNVERIFIED"), "{lines:?}");
    assert_eq!(lines[1], "harness input: preload WillowGame.ToyBody: loaded");
    let mesh = x.vm.space.live(class).next().expect("the mesh is in memory");
    assert!(x.vm.defs.is_loaded(&x.vm.space, mesh), "loaded, not a handle");
}

#[test]
fn a_mesh_no_package_holds_is_logged_and_skipped() {
    let mut x = fx();
    preload_mesh(&mut x.vm, "WillowGame.Missing", "Maya's mesh").unwrap();
    let lines = harness_lines(&x.vm);
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert_eq!(lines[1], "harness input: preload WillowGame.Missing: found nothing");
    assert!(preload_mesh(&mut x.vm, "", "Maya's mesh").is_err(), "an empty path is no object path");
}

#[test]
fn the_skeleton_source_is_installed_once_and_logged_with_its_roots() {
    let mut x = fx();
    assert!(x.vm.service::<SkeletalMeshes>().is_none());
    // Review r1 of #592, item 1: one root that exists and one that does not, each logged absolute; the missing one is
    // marked, and the install goes ahead.
    let present = x.dir.clone();
    let absent = x.dir.join("no-such-root");
    install_skeleton_source(&mut x.vm, &[present.clone(), absent.clone()]).unwrap();
    assert!(x.vm.service::<SkeletalMeshes>().is_some(), "the layer's service is installed");
    let lines = harness_lines(&x.vm);
    assert_eq!(lines.len(), 1, "{lines:?}");
    let head = format!(
        "harness input: the skeleton source over UModel .psk exports in {};{} (missing) (file-package layout; ",
        present.display(),
        absent.display()
    );
    assert!(lines[0].starts_with(&head), "{lines:?}");
    let roots = [present];
    assert!(!lines[0].contains("  "), "one space between words: {lines:?}");
    assert!(lines[0].contains("UNVERIFIED"), "{lines:?}");
    assert!(install_skeleton_source(&mut x.vm, &roots).is_err(), "a second install is refused");
}

#[test]
fn the_roots_come_from_the_variable_else_the_default() {
    // Review r1 of #592, item 1: the default is under the repository root, not the folder the run starts in.
    let default = vec![super::super::repo_root().join(PSK_ROOTS_DEFAULT)];
    assert!(default[0].is_absolute(), "{default:?}");
    assert!(super::super::repo_root().join("Cargo.toml").is_file(), "the repository root holds the workspace");
    assert_eq!(parse_psk_roots(None), default);
    assert_eq!(parse_psk_roots(Some("")), default);
    assert_eq!(parse_psk_roots(Some(" ; ")), default);
    assert_eq!(
        parse_psk_roots(Some("a; b ;")),
        vec![std::path::PathBuf::from("a"), std::path::PathBuf::from("b")]
    );
}
