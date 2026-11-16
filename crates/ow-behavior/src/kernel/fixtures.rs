//! The test environment of the provider and kernel tests: a VM over the invented packages of [`packages`].

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use ow_attr::registry::AttributeSystem;
use ow_core::ParityMode;
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;
use ow_object::ids::ObjRef;
use ow_object::prefix::PropertyPrefix;
use ow_vm::api::{NativePack, Vm};

use crate::kernel::Provider;

pub mod packages;

pub use packages::*;

static NEXT: AtomicU32 = AtomicU32::new(0);

/// A VM over the code packages, `Things` and the provider packages given, with the world handlers registered.
pub struct Env {
    pub vm: Vm,
    dir: PathBuf,
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

impl Env {
    pub fn new(providers: &[(&str, Vec<u8>)]) -> Env {
        let dir = std::env::temp_dir().join(format!(
            "ow-behavior-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let files = install(providers);
        let borrowed: Vec<(&str, &[u8])> = files
            .iter()
            .map(|(n, b)| (n.as_str(), b.as_slice()))
            .collect();
        let defs = DefStore::from_synthetic(
            &dir,
            &borrowed,
            IdentityPolicy::FirstInScanOrder,
            ParityMode::Off,
        )
        .expect("synthetic store");
        let mut vm = Vm::new(
            ObjectSpace::new(),
            defs,
            Rc::new(AttributeSystem::new_empty()),
            ParityMode::Off,
        );
        crate::handlers::Pack
            .register(&mut vm)
            .expect("world handlers");
        Env { vm, dir }
    }

    /// One provider package `spec` alone (file `TestBpd.upk`).
    pub fn of(spec: &Spec) -> Env {
        Env::new(&[("TestBpd", provider_package(spec))])
    }

    /// The provider definition `path` of package `file`, loaded.
    pub fn definition(&mut self, file: &str, path: &str) -> ObjRef {
        let name = ow_core::PackageName::parse(file).unwrap();
        let (_, store) = self.vm.defs.code_mut();
        let id = store.load(&name).unwrap();
        let index = store.get(id).find_export(path).unwrap().expect("export");
        self.vm
            .defs
            .load_export(&mut self.vm.space, id, index, PropertyPrefix::Ordinary)
            .unwrap()
    }

    /// The kernel over `TestBpd`'s provider `path`, every class but CompareObject at the boundary (as `--behavior-run`).
    pub fn provider(&mut self, path: &str) -> Provider {
        let definition = self.definition("TestBpd", path);
        let mut p = Provider::new(&mut self.vm, definition).expect("provider");
        for class in [
            "WillowGame.Behavior_ToySame",
            "WillowGame.Behavior_ToyDifferent",
            "WillowGame.Behavior_ToyInputs",
        ] {
            p.report_at_boundary(class, None, Vec::new());
        }
        p
    }
}
