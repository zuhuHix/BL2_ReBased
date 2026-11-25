//! Tests of the `SkeletalMesh` LOD reader on invented packages (AGENTS.md rule 9; every value invented).

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
    /// The tagged `bHasVertexColors`, and whether the LOD models carry the colour buffer.
    colours_tag: bool,
    colours: bool,
    /// LOD model 0's trailing count.
    trailing: i32,
    /// Swap the bone-name map's first two bones.
    swap_names: bool,
    /// The triangle count the section claims (the index buffer holds 2).
    triangles: u32,
    /// The first chunk's first vertex.
    chunk_first: u32,
    /// Added to the bulk block's own position.
    bulk_shift: u32,
    /// Bytes added after the trailer.
    extra: usize,
    /// The `LODInfo` array's body, when it should differ from one empty tagged struct per entry.
    lod_info_body: Option<Vec<u8>>,
    /// The section's first index.
    first_index: u32,
    /// The index buffer's last index.
    last_index: u16,
    /// Every vertex's four weight bytes, and its first bone index.
    weights: [u8; 4],
    first_bone: u8,
    /// The bulk block's element count and size.
    bulk: [i32; 2],
    /// The trailer's per-material word and the length of its last array.
    per_material_word: u32,
    last_floats: u32,
    /// Bytes cut from the end of the mesh's payload, and whether a second export follows it in the file.
    cut: usize,
    next_export: bool,
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
        colours_tag: false,
        colours: false,
        trailing: 0,
        swap_names: false,
        triangles: 2,
        chunk_first: 0,
        bulk_shift: 0,
        extra: 0,
        lod_info_body: None,
        first_index: 0,
        last_index: 0,
        weights: [255, 0, 0, 0],
        first_bone: 0,
        bulk: [0, 0],
        per_material_word: 0,
        last_floats: 4,
        cut: 0,
        next_export: false,
    }
}

/// One invented LOD model (module documentation, "The LOD models") starting at absolute position `at`: one section
/// of two triangles, one chunk of three rigid vertices, one UV set.
fn lod_model(m: &Mesh<'_>, lod: usize, at: usize) -> Vec<u8> {
    // The sections.
    let mut v = words(&[1]);
    v.extend([0u8, 0, 0, 0]);
    v.extend(words(&[m.first_index, m.triangles]));
    v.push(0);
    // The index buffer.
    v.extend(words(&[1]));
    v.push(2);
    v.extend(words(&[2, 6]));
    for i in [0u16, 1, 2, 2, 1, m.last_index] {
        v.extend(i.to_le_bytes());
    }
    // The active bones, then one chunk.
    v.extend(words(&[1]));
    v.extend(1u16.to_le_bytes());
    v.extend(words(&[1, m.chunk_first, 0, 0, 1]));
    v.extend(0u16.to_le_bytes());
    v.extend(words(&[3, 0, 1]));
    // A word, the vertex count, the required bones.
    v.extend(words(&[0, 3, 1]));
    v.push(0);
    // The bulk block, naming its own position.
    v.extend(words(&[0]));
    v.extend(m.bulk[0].to_le_bytes());
    v.extend(m.bulk[1].to_le_bytes());
    let here = (at + v.len() + 4) as u32;
    v.extend(words(&[here + m.bulk_shift]));
    // The vertex buffer.
    v.extend(words(&[1, 1, 0, 1]));
    for f in [1.0f32, 1.0, 1.0, 0.0, 0.0, 0.0] {
        v.extend(f.to_le_bytes());
    }
    v.extend(words(&[32, 3]));
    for _ in 0..3 {
        // Tangents (8), bone indices (4), weights (4), then the rest of the vertex (16).
        let mut vertex = vec![0x11u8; 8];
        vertex.extend([m.first_bone, 0, 0, 0]);
        vertex.extend(m.weights);
        vertex.extend(vec![0x22u8; 16]);
        v.extend(vertex);
    }
    if m.colours {
        v.extend(words(&[4, 3]));
        v.extend(vec![0xFFu8; 12]);
    }
    v.extend((if lod == 0 { m.trailing } else { 0 }).to_le_bytes());
    v
}

fn package(m: &Mesh<'_>) -> Package {
    let mut b = PackageBuilder::new_empty();
    b.names(["None"]);
    let cls = b.add_import("Core", "Class", 0, m.class);
    let mat = b.add_import("Core", "Class", 0, "ToyMaterial");
    let mut payload = words(&[0]);
    if let Some(n) = m.lod_info {
        let none = b.none();
        let body = m.lod_info_body.clone().unwrap_or_else(|| none.repeat(n.max(0) as usize));
        payload.extend(b.array_tag("LODInfo", n, &body));
    }
    if m.colours_tag {
        payload.extend(b.bool_tag("bHasVertexColors", true));
    }
    payload.extend(b.none());
    payload.extend(vec![0u8; 28]);
    payload.extend(words(&[m.materials as u32]));
    for _ in 0..m.materials {
        payload.extend(m.material.unwrap_or(mat).to_le_bytes());
    }
    payload.extend(vec![0u8; 24]);
    payload.extend(words(&[m.claimed_bones.unwrap_or(m.bones.len() as u32)]));
    let mut bone_names = Vec::new();
    for &(name, children, parent) in &m.bones {
        let n = b.name(name);
        bone_names.push(n);
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
    // The LOD models need their absolute position: a first build finds where the payload lands.
    let next = words(&[0xDEAD_BEEF; 8]);
    let base = {
        let mut probe = b.clone();
        probe.add_export(cls, "ToyMesh", payload.clone());
        if m.next_export {
            probe.add_export(cls, "ToyNext", next.clone());
        }
        let pkg = Package::from_decoded(Default::default(), "probe".into(), probe.build()).unwrap();
        pkg.object(1).unwrap().offset as usize
    };
    for lod in 0..m.lods.clamp(0, 4) as usize {
        let at = base + payload.len();
        payload.extend(lod_model(m, lod, at));
    }
    // The bone-name map, then the trailer.
    payload.extend(words(&[m.bones.len() as u32]));
    for (i, &n) in bone_names.iter().enumerate() {
        let bone = if m.swap_names && i < 2 { 1 - i } else { i };
        payload.extend(n.to_le_bytes());
        payload.extend(0i32.to_le_bytes());
        payload.extend((bone as i32).to_le_bytes());
    }
    payload.extend(words(&[0, 0, 0, m.materials as u32]));
    for _ in 0..m.materials {
        payload.extend(words(&[m.per_material_word]));
    }
    payload.extend(words(&[m.last_floats]));
    for _ in 0..m.last_floats {
        payload.extend(1.0f32.to_le_bytes());
    }
    payload.extend(vec![0u8; m.extra]);
    payload.truncate(payload.len() - m.cut);
    let length = payload.len();
    b.add_export(cls, "ToyMesh", payload);
    if m.next_export {
        b.add_export(cls, "ToyNext", next);
    }
    let pkg = Package::from_decoded(Default::default(), "probe".into(), b.build()).unwrap();
    assert_eq!(pkg.object(1).unwrap().offset as usize, base, "the payload's place is stable");
    if m.next_export {
        assert_eq!(pkg.object(2).unwrap().offset as usize, base + length, "the second export follows at once");
    }
    pkg
}

fn read(m: &Mesh<'_>) -> Result<SkeletalLods, LodCountError> {
    read_lod_count(&package(m), 1, 4, &SchemaTyper::new_empty())
}

fn models(m: &Mesh<'_>) -> Result<SkeletalLodModels, LodCountError> {
    read_lod_models(&package(m), 1, 4, &SchemaTyper::new_empty())
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

/// The full read of a well-formed invented mesh: every LOD model's counts, the colour buffer read only when the tag
/// says so, every trailing count zero, and the read ending at the export's end.
#[test]
fn a_well_formed_mesh_reads_to_its_end() {
    let got = models(&mesh()).unwrap();
    assert_eq!(got.prefix.lod_count, 2);
    assert_eq!(got.models.len(), 2);
    for model in &got.models {
        let shape = (model.sections, model.triangles, model.chunks, model.vertices, model.uv_sets);
        assert_eq!(shape, (1, 2, 1, 3, 1));
        assert!(!model.vertex_colors);
        assert_eq!(model.trailing, 0);
    }
    let mut coloured = mesh();
    coloured.colours_tag = true;
    coloured.colours = true;
    assert!(models(&coloured).unwrap().models.iter().all(|m| m.vertex_colors));
    let mut one = mesh();
    one.lods = 1;
    one.lod_info = Some(1);
    assert_eq!(models(&one).unwrap().models.len(), 1);
}

/// The colour buffer follows the tag: a buffer without the tag, or the tag without a buffer, misfits.
#[test]
fn the_colour_buffer_follows_the_tag() {
    let mut untagged = mesh();
    untagged.colours = true;
    assert_eq!(models(&untagged), Err(LodCountError::TrailingArray { lod: 0, count: 4 }));
    let mut missing = mesh();
    missing.colours_tag = true;
    let got = models(&missing);
    assert!(matches!(got, Err(LodCountError::LodModel { lod: 0, field: "colour buffer count", .. })), "{got:?}");
}

/// A non-zero trailing count is refused, not stepped over: its elements are not decoded.
#[test]
fn a_trailing_count_is_refused() {
    let mut m = mesh();
    m.trailing = 1;
    assert_eq!(models(&m), Err(LodCountError::TrailingArray { lod: 0, count: 1 }));
}

/// The structural checks inside a LOD model: the index count meets the triangles, the chunks meet end to end, and
/// the bulk block names its own position.
#[test]
fn the_lod_model_checks_refuse_a_misfit() {
    let mut tris = mesh();
    tris.triangles = 3;
    assert_eq!(models(&tris), Err(LodCountError::LodModel { lod: 0, field: "index count", found: 6 }));
    let mut first = mesh();
    first.chunk_first = 5;
    assert_eq!(models(&first), Err(LodCountError::LodModel { lod: 0, field: "chunk first vertex", found: 5 }));
    let mut bulk = mesh();
    bulk.bulk_shift = 4;
    let got = models(&bulk);
    assert!(matches!(got, Err(LodCountError::LodModel { lod: 0, field: "bulk position", .. })), "{got:?}");
}

/// After the LOD models: the bone-name map must name each bone by its own name, and the read must end at the
/// export's end.
#[test]
fn the_name_map_and_the_end_are_checked() {
    let mut swapped = mesh();
    swapped.swap_names = true;
    assert_eq!(models(&swapped), Err(LodCountError::NameMap { entry: 0 }));
    let mut extra = mesh();
    extra.extra = 3;
    assert_eq!(models(&extra), Err(LodCountError::Leftover { bytes: 3 }));
}

/// Review r1 of #673, item 1: the decoded `LODInfo` entries are compared, not the declared count. A tag declaring two
/// entries with an empty body is refused by the walk, and so are bytes that are not two tagged structs.
#[test]
fn the_lod_info_entries_must_decode() {
    let mut empty = mesh();
    empty.lod_info_body = Some(Vec::new());
    assert!(matches!(read(&empty), Err(LodCountError::Asset(_))), "{:?}", read(&empty));
    assert!(matches!(models(&empty), Err(LodCountError::Asset(_))));
    let mut raw = mesh();
    raw.lod_info_body = Some(vec![0xEEu8; 8]);
    assert!(read(&raw).is_err(), "{:?}", read(&raw));
}

/// Review r1 of #673, item 2: a non-empty bulk block is refused (its elements are not decoded), and so is the
/// trailer's per-material word or last array in a shape not seen.
#[test]
fn undecoded_contents_are_refused() {
    for (bulk, field) in [([-1, 0], "bulk element count"), ([0, 4], "bulk size")] {
        let mut m = mesh();
        m.bulk = bulk;
        let got = models(&m);
        assert!(matches!(got, Err(LodCountError::LodModel { lod: 0, field: f, .. }) if f == field), "{got:?}");
    }
    let mut word = mesh();
    word.per_material_word = 7;
    assert_eq!(models(&word), Err(LodCountError::Trailer(7)));
    let mut five = mesh();
    five.last_floats = 5;
    assert_eq!(models(&five), Err(LodCountError::Trailer(5)));
}

/// Review r1 of #673, item 3: each section's span must fit the index buffer (checked arithmetic), and every index
/// must name a vertex of the section's chunk.
#[test]
fn each_section_span_and_index_is_checked() {
    let mut far = mesh();
    far.first_index = u32::MAX;
    let got = models(&far);
    assert!(matches!(got, Err(LodCountError::LodModel { field: "section index span", .. })), "{got:?}");
    let mut past = mesh();
    past.last_index = 3;
    assert_eq!(models(&past), Err(LodCountError::LodModel { lod: 0, field: "section index", found: 3 }));
}

/// Review r1 of #673, item 2: every vertex's influences are decoded: the weights sum to 255, and a weighted bone
/// index names an entry of its chunk's bone map.
#[test]
fn every_vertex_influence_is_checked() {
    let mut light = mesh();
    light.weights = [200, 0, 0, 0];
    assert_eq!(models(&light), Err(LodCountError::LodModel { lod: 0, field: "vertex weight total", found: 200 }));
    let mut stray = mesh();
    stray.first_bone = 1;
    assert_eq!(models(&stray), Err(LodCountError::LodModel { lod: 0, field: "vertex bone index", found: 1 }));
}

/// Review r1 of #673: the export boundary holds. With a second export right after the mesh in the file, a mesh cut
/// short is refused, never read on into the next export; uncut, the same layout reads.
#[test]
fn a_cut_export_is_refused_at_its_boundary() {
    let mut whole = mesh();
    whole.next_export = true;
    assert_eq!(models(&whole).unwrap().models.len(), 2);
    for cut in [4, 16] {
        let mut m = mesh();
        m.next_export = true;
        m.cut = cut;
        let got = models(&m);
        assert!(matches!(got, Err(LodCountError::Asset(_))), "cut {cut}: {got:?}");
    }
}
