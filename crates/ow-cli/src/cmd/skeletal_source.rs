//! Owner: lane IMPL-SKELETAL-MESH-LAYER. The harness's [`SkeletonSource`] for `ow_world::world::skeletal`: a mesh's
//! reference skeleton read with ow-assets' ActorX reader (`ow_assets::skeletal::read_psk_file`, which checks every
//! count, index and parent order) from UModel's `.psk` / `.pskx` exports of the player's own install. AI-assisted
//! (Claude Code).
//!
//! **The file of a mesh** (UModel's export layout, as decision 2026-11-16-53 used it): under an export root,
//! `<file package>/SkeletalMesh3/<object name>.psk` (or `.pskx`), the first root that has it. That maps a mesh by its
//! file package and leaf name; two meshes of one package with the same leaf would share a file (UNVERIFIED that the
//! installed data has none on the slice's meshes). The exports are game-derived and live under ignored `local/` or
//! outside the repository; they are a harness input, never a tracked file.

// No driver installs it yet: the consumers' lanes (#529's socket attach, #566's per-bone builds) install it when they
// switch over, so the harness items are unused until then.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use ow_world::world::skeletal::{ReferenceSkeleton, SkeletonBone, SkeletonSource};

/// UModel export roots.
pub struct PskExports {
    roots: Vec<PathBuf>,
}

impl PskExports {
    pub fn new(roots: Vec<PathBuf>) -> Self {
        PskExports { roots }
    }

    /// The export of `leaf` in `package`, if a root has one.
    pub fn file(&self, package: &str, leaf: &str) -> Option<PathBuf> {
        self.roots.iter().find_map(|r| {
            ["psk", "pskx"]
                .iter()
                .map(|ext| r.join(package).join("SkeletalMesh3").join(format!("{leaf}.{ext}")))
                .find(|p| p.is_file())
        })
    }
}

/// The reference skeleton of the ActorX file at `path`.
pub fn read_skeleton(path: &Path) -> Result<ReferenceSkeleton, String> {
    let mesh = ow_assets::skeletal::read_psk_file(path).map_err(|e| e.to_string())?;
    Ok(ReferenceSkeleton {
        bones: mesh.bones.iter().map(|b| SkeletonBone { name: b.name.clone(), parent: b.parent }).collect(),
    })
}

impl SkeletonSource for PskExports {
    fn skeleton(&self, package: &str, _path: &str, leaf: &str) -> Result<Option<ReferenceSkeleton>, String> {
        match self.file(package, leaf) {
            Some(f) => read_skeleton(&f).map(Some),
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The installed probe (needs the game and a UModel export, so ignored): `OPENWILLOW_BL2` the install,
    /// `OW_SKELETAL_EXPORTS` the export root, `OW_SKELETAL_MESH` the mesh's object path. It loads the mesh through the
    /// store (the harness's load), installs the source and prints counts only: the bone count and the socket count.
    #[test]
    #[ignore = "needs the installed game and a UModel export"]
    fn installed_mesh_probe() {
        let (Some(game), Some(root), Some(mesh)) = (
            std::env::var_os("OPENWILLOW_BL2"),
            std::env::var_os("OW_SKELETAL_EXPORTS"),
            std::env::var("OW_SKELETAL_MESH").ok(),
        ) else {
            panic!("set OPENWILLOW_BL2, OW_SKELETAL_EXPORTS and OW_SKELETAL_MESH");
        };
        let cooked = PathBuf::from(game).join("WillowGame").join("CookedPCConsole");
        let mut vm = crate::cmd::slice::open(cooked.to_str().unwrap(), ow_core::ParityMode::Off, ow_defs::identity::IdentityPolicy::FirstInScanOrder)
            .unwrap_or_else(|_| panic!("open"));
        let path = ow_core::ObjectPath::parse(&mesh).unwrap();
        let o = vm.defs.find(&mut vm.space, &path).unwrap().expect("the mesh");
        ow_world::world::skeletal::SkeletalMeshes::install(&mut vm, Box::new(PskExports::new(vec![PathBuf::from(root)]))).unwrap();
        let heap = format!("{:?}", vm.space.heap).len();
        let skeleton = ow_world::world::skeletal::reference_skeleton(&vm, o).expect("the reference skeleton");
        let sockets = ow_world::world::skeletal::sockets(&vm, o);
        println!(
            "PROBE bones={} sockets={}",
            skeleton.len(),
            match &sockets {
                Ok(s) => s.len().to_string(),
                Err(e) => format!("refused ({e})"),
            }
        );
        assert_eq!(format!("{:?}", vm.space.heap).len(), heap, "the layer wrote nothing");
        // The harness loads the socket objects (a load, the harness's), then the layer reads them.
        let class = vm.space.heap.get(o).unwrap().class;
        let p = vm.space.prop(class, "Sockets").unwrap();
        let list: Vec<_> = vm.space.read(o, ow_object::ids::PropAddr::scalar(p)).unwrap().elements().unwrap().to_vec();
        for v in list {
            if let ow_object::value::Value::Object(Some(s)) = v {
                vm.defs.ensure_loaded(&mut vm.space, s).unwrap();
            }
        }
        let after = ow_world::world::skeletal::sockets(&vm, o).expect("the sockets after the harness load");
        let on_bones = after.iter().filter(|s| skeleton.bone_index(&s.bone).is_some()).count();
        println!("PROBE after the harness load: sockets={} on a skeleton bone={}", after.len(), on_bones);
    }
}
