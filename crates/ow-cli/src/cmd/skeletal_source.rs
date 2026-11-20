//! Owner: lane IMPL-SKELETAL-MESH-LAYER. The harness's [`SkeletonSource`] for `ow_world::world::skeletal`: a mesh's
//! reference skeleton read with ow-assets' ActorX reader (`ow_assets::skeletal::read_psk_file`, which checks every
//! count, index and parent order) from UModel's `.psk` / `.pskx` exports of the player's own install. AI-assisted
//! (Claude Code).
//!
//! **The file of a mesh** (UModel's export layout, as decision 2026-11-16-53 used it): under an export root,
//! `<file package>/SkeletalMesh3/<object name>.psk` (or `.pskx`), the first root that has it. That maps a mesh by its
//! file package and leaf name. Two meshes of one package with the same leaf would share a file, so the layer refuses
//! such a mesh before it asks (review r1 of #569). The exports are game-derived and live under ignored `local/` or
//! outside the repository; they are a harness input, never a tracked file.
//!
//! **Path safety** (review r1 of #569): the package and the leaf must each be one plain file-name component (no
//! separator, no `.` or `..`, no drive or stream colon, no NUL), and the file found must resolve, links and junctions
//! followed, to a place under the canonical export root. Anything else is the source's error, never a read.

// The slice harness installs it in Off (`combat::install_skeleton_source`, lane SLICE-HARNESS-MAYA-MESH).

use std::path::{Path, PathBuf};

use ow_world::world::skeletal::{BoneBind, ReferenceSkeleton, SkeletonBone, SkeletonSource};

/// UModel export roots.
pub struct PskExports {
    roots: Vec<PathBuf>,
}

impl PskExports {
    pub fn new(roots: Vec<PathBuf>) -> Self {
        PskExports { roots }
    }

    /// The export of `leaf` in `package`, if a root has one (module documentation, "Path safety").
    pub fn file(&self, package: &str, leaf: &str) -> Result<Option<PathBuf>, String> {
        safe_component(package)?;
        safe_component(leaf)?;
        for root in &self.roots {
            for ext in ["psk", "pskx"] {
                let candidate = root.join(package).join("SkeletalMesh3").join(format!("{leaf}.{ext}"));
                if !candidate.is_file() {
                    continue;
                }
                let base = root.canonicalize().map_err(|e| format!("the export root {}: {e}", root.display()))?;
                let real = candidate.canonicalize().map_err(|e| format!("{}: {e}", candidate.display()))?;
                if !real.starts_with(&base) {
                    return Err(format!("{} resolves outside the export root", candidate.display()));
                }
                return Ok(Some(real));
            }
        }
        Ok(None)
    }
}

/// One plain file-name component (module documentation, "Path safety").
fn safe_component(name: &str) -> Result<(), String> {
    let bad = name.is_empty()
        || name == "."
        || name == ".."
        || name.chars().any(|c| matches!(c, '/' | '\\' | ':' | '\0'));
    if bad {
        return Err(format!("{name:?} is not a single file-name component"));
    }
    Ok(())
}

/// The reference skeleton of the ActorX file at `path`.
pub fn read_skeleton(path: &Path) -> Result<ReferenceSkeleton, String> {
    let mesh = ow_assets::skeletal::read_psk_file(path).map_err(|e| e.to_string())?;
    Ok(ReferenceSkeleton {
        // The bind pose relative to the parent, as the reader gives it (lane IMPL-BONE-TRANSFORM-LAYER; UNVERIFIED that
        // it equals the mesh's reference skeleton).
        bones: mesh
            .bones
            .iter()
            .map(|b| SkeletonBone {
                name: b.name.clone(),
                parent: b.parent,
                bind: Some(BoneBind { rotation: b.bind.rotation, translation: b.bind.translation }),
            })
            .collect(),
    })
}

impl SkeletonSource for PskExports {
    fn skeleton(&self, package: &str, _path: &str, leaf: &str) -> Result<Option<ReferenceSkeleton>, String> {
        match self.file(package, leaf)? {
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
        // Review r1 of #569, item 5: the socket objects arrive unloaded, so the first ask is the layer's refusal.
        match ow_world::world::skeletal::sockets(&vm, o) {
            Err(ow_vm::error::VmError::NotYet(n)) => {
                // Review r2 of #569: the definition store's load layer, and the unloaded-socket diagnostic.
                assert!(
                    matches!(&n.missing, ow_core::Missing::LayerAbsent(l) if l.contains("the definition store's load")),
                    "{n}"
                );
                assert!(n.what.contains("the unloaded socket"), "{n}");
            }
            other => panic!("expected the unloaded-socket refusal, got {other:?}"),
        }
        println!("PROBE bones={} sockets=refused (unloaded)", skeleton.len());
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
        assert!(!after.is_empty());
        assert_eq!(on_bones, after.len(), "every socket names a bone of the skeleton");
    }

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ow-cli-psk-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Review r1 of #569, item 5: a missing export is "no data" (`Ok(None)`), a malformed ActorX file is the reader's
    /// error, two different answers.
    #[test]
    fn a_missing_export_is_not_a_malformed_one() {
        let root = temp("malformed");
        let exports = PskExports::new(vec![root.clone()]);
        assert_eq!(exports.skeleton("Toy", "Toy.Mesh", "Mesh").unwrap(), None);
        let folder = root.join("Toy").join("SkeletalMesh3");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("Mesh.psk"), b"not an ActorX file").unwrap();
        assert!(exports.skeleton("Toy", "Toy.Mesh", "Mesh").is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Review r1 of #569, item 1: a name with path components is refused, never joined.
    #[test]
    fn names_must_be_single_components() {
        let root = temp("names");
        let exports = PskExports::new(vec![root.clone()]);
        for (package, leaf) in [("Toy", "../Mesh"), ("..", "Mesh"), ("Toy", "a\\b"), ("Toy", "C:Mesh"), ("", "Mesh"), ("Toy", ".")] {
            assert!(exports.file(package, leaf).is_err(), "{package:?} {leaf:?}");
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Review r1 of #569, item 1: an export reached through a link or junction that leads outside the root is refused.
    #[test]
    fn a_link_out_of_the_root_is_refused() {
        let root = temp("link-root");
        let outside = temp("link-outside");
        let target = outside.join("SkeletalMesh3");
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join("Mesh.psk"), b"x").unwrap();
        let link = root.join("Toy");
        // Review r2 of #569: a fixture that cannot be set up fails the test rather than skipping the check.
        assert!(make_dir_link(&outside, &link), "no directory link could be made: the containment check would not run");
        let exports = PskExports::new(vec![root.clone()]);
        let err = exports.file("Toy", "Mesh").unwrap_err();
        assert!(err.contains("outside the export root"), "{err}");
        let _ = std::fs::remove_dir(&link).or_else(|_| std::fs::remove_file(&link));
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&outside);
    }

    /// A directory link at `link` to `target`: a junction on Windows (no privilege needed), a symlink elsewhere.
    fn make_dir_link(target: &Path, link: &Path) -> bool {
        #[cfg(windows)]
        {
            if std::os::windows::fs::symlink_dir(target, link).is_ok() {
                return true;
            }
            std::process::Command::new("cmd")
                .args(["/C", "mklink", "/J"])
                .arg(link)
                .arg(target)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        }
        #[cfg(not(windows))]
        {
            std::os::unix::fs::symlink(target, link).is_ok()
        }
    }
}
