//! Tests of the `SkeletalMesh` LOD-count reader on invented packages (AGENTS.md rule 9; every value invented).

use ow_package::schema::SchemaTyper;
use ow_package::testkit::{PackageBuilder, words};

use super::*;

/// One invented bone: (name, child count, parent).
type Bone<'a> = (&'a str, i32, i32);

/// The knobs of an invented mesh.
struct Mesh<'a> {
    class: &'a str,
    materials: usize,
    bones: Vec<Bone<'a>>,
    depth: i32,
    lods: i32,
    lod_info: Option<i32>,
    position: f32,
    /// The bone count word, when it should differ from the bones written.
    claimed_bones: Option<u32>,
    /// The material reference, when it should differ from the material import.
    material: Option<i32>,
}

fn mesh<'a>() -> Mesh<'a> {
    Mesh {
        class: "SkeletalMesh",
        materials: 1,
        bones: vec![("ToyRoot", 2, 0), ("ToyHips", 1, 0), ("ToySpine", 0, 1), ("ToyArm", 0, 0)],
        depth: 3,
        lods: 2,
        lod_info: Some(2),
        position: 1.5,
        claimed_bones: None,
        material: None,
    }
}

fn package(m: &Mesh<'_>) -> Package {
    let mut b = PackageBuilder::new_empty();
    b.names(["None"]);
    let cls = b.add_import("Core", "Class", 0, m.class);
    let mat = b.add_import("Core", "Class", 0, "ToyMaterial");
    let mut payload = words(&[0]);
    if let Some(n) = m.lod_info {
        payload.extend(b.array_tag("LODInfo", n, &vec![0u8; 4 * n.max(0) as usize]));
    }
    payload.extend(b.none());
    payload.extend(vec![0u8; 28]);
    payload.extend(words(&[m.materials as u32]));
    for _ in 0..m.materials {
        payload.extend(m.material.unwrap_or(mat).to_le_bytes());
    }
    payload.extend(vec![0u8; 24]);
    payload.extend(words(&[m.claimed_bones.unwrap_or(m.bones.len() as u32)]));
    for &(name, children, parent) in &m.bones {
        let n = b.name(name);
        payload.extend(n.to_le_bytes());
        payload.extend(0i32.to_le_bytes());
        payload.extend(words(&[0]));
        for v in [0.0f32, 0.0, 0.0, 1.0, m.position, 0.0, 2.0] {
            payload.extend(v.to_le_bytes());
        }
        payload.extend(children.to_le_bytes());
        payload.extend(parent.to_le_bytes());
        payload.extend(words(&[u32::MAX]));
    }
    payload.extend(m.depth.to_le_bytes());
    payload.extend(m.lods.to_le_bytes());
    // The LOD models themselves: not read.
    payload.extend(vec![0xAAu8; 16]);
    b.add_export(cls, "ToyMesh", payload);
    Package::from_decoded(Default::default(), "probe".into(), b.build()).unwrap()
}

fn read(m: &Mesh<'_>) -> Result<SkeletalLods, LodCountError> {
    read_lod_count(&package(m), 1, 4, &SchemaTyper::new_empty())
}

/// A well-formed invented mesh: its counts, the depth from the parent chain, and the LOD count.
#[test]
fn a_well_formed_mesh_gives_its_lod_count() {
    let got = read(&mesh()).unwrap();
    assert_eq!((got.materials, got.bones, got.depth, got.lod_count), (1, 4, 3, 2));
    let mut one = mesh();
    one.lods = 1;
    one.lod_info = Some(1);
    assert_eq!(read(&one).unwrap().lod_count, 1);
    let mut no_materials = mesh();
    no_materials.materials = 0;
    assert_eq!(read(&no_materials).unwrap().materials, 0);
}

/// The LOD count must agree with the tagged `LODInfo` array, which must be there, and be in range.
#[test]
fn the_lod_count_is_checked_against_the_tagged_lod_info() {
    let mut differ = mesh();
    differ.lod_info = Some(3);
    assert_eq!(read(&differ), Err(LodCountError::LodInfo { native: 2, tagged: 3 }));
    let mut absent = mesh();
    absent.lod_info = None;
    assert_eq!(read(&absent), Err(LodCountError::NoLodInfo));
    for bad in [0, -1, 33] {
        let mut m = mesh();
        m.lods = bad;
        assert_eq!(read(&m), Err(LodCountError::LodRange(bad)), "{bad}");
    }
}

/// The skeleton's structural checks: the parent order, the stored depth, finite numbers, non-negative child counts.
#[test]
fn the_skeleton_checks_refuse_a_misfit() {
    let mut root = mesh();
    root.bones[0].2 = 1;
    assert_eq!(read(&root), Err(LodCountError::Parent { bone: 0, parent: 1 }));
    let mut forward = mesh();
    forward.bones[1].2 = 2;
    assert_eq!(read(&forward), Err(LodCountError::Parent { bone: 1, parent: 2 }));
    let mut depth = mesh();
    depth.depth = 4;
    assert_eq!(read(&depth), Err(LodCountError::Depth { stored: 4, computed: 3 }));
    let mut nan = mesh();
    nan.position = f32::NAN;
    assert_eq!(read(&nan), Err(LodCountError::NonFinite { bone: 0 }));
    let mut kids = mesh();
    kids.bones[2].1 = -1;
    assert_eq!(read(&kids), Err(LodCountError::Children { bone: 2, children: -1 }));
}

/// Not a `SkeletalMesh`, a bone count past the export's end, and a material reference outside the package are refused.
#[test]
fn the_class_counts_and_references_are_checked() {
    let mut other = mesh();
    other.class = "StaticMesh";
    assert_eq!(read(&other), Err(LodCountError::NotSkeletalMesh("StaticMesh".into())));
    let mut past = mesh();
    past.claimed_bones = Some(1000);
    assert_eq!(read(&past), Err(LodCountError::Asset(AssetError::ArrayPastPayload)));
    let mut stray = mesh();
    stray.material = Some(99);
    assert!(matches!(read(&stray), Err(LodCountError::Asset(_))), "{:?}", read(&stray));
    let mut one = mesh();
    one.bones = vec![("ToyRoot", 0, 0)];
    one.depth = 1;
    assert_eq!(read(&one).unwrap().bones, 1, "a one-bone mesh reads");
}
