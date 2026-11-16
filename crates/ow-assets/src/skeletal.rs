//! Owner: lane H6-ANIM (host track, stage 1). A skeletal mesh as plain data, read from UModel's ActorX `.psk` /
//! `.pskx` export of the player's own install: bones with their parents and bind pose, vertices with up to four
//! weights, triangles, sections and material names. In the game's frame (uu, X forward, Y right, Z up).
//!
//! AI-assisted (Claude Code). Decision: `docs/decisions/2026-11/2026-11-16-53-*`.
//!
//! **Why an export.** Our own decoders do not read `SkeletalMesh` from the 832/46 packages: its native layout has no
//! public spec and none has been recovered (AGENTS.md rule 4). UModel, the project's first-choice extraction backend
//! (`THIRD_PARTY.md`), writes it as ActorX; the chunk format is in [`crate::actorx`]. Native decoding of
//! `SkeletalMesh` is an open item of the decision.
//!
//! **What the reader takes from the file.**
//! - `PNTS0000`: the welded points (positions).
//! - `VTXW0000`: the wedges, which become [`SkinVertex`]es: a point, a UV and a material index. When there are at
//!   most 65,536 wedges the point index is a `u16` padded to four bytes (the padding is not read); above that it is
//!   an `i32`.
//! - `FACE0000` (16-bit wedge indices) or `FACE3200` (32-bit, 18-byte records): triangles and their material.
//!   Each triangle's first two corners are swapped back (the export's mirror swapped them), so the index order is
//!   the game's. [`SkeletalMesh::sections`] are the runs of consecutive triangles with one material, in file order.
//! - `MATT0000`: material names. `REFSKELT`: bones. `RAWWEIGHTS`: (weight, point, bone) influences.
//! - Any other chunk (`EXTRAUVS0`, `VERTEXCOLOR`, morph targets) is not read; it is listed in
//!   [`SkeletalMesh::ignored_chunks`] so a caller can see what was left out.
//!
//! **The bone convention.** A [`BoneTransform`] maps a point from the bone's frame to its parent's (for the root, to
//! the mesh's): `p_parent = q p q* + t` with Hamilton quaternions. ActorX stores every bone but the root with the
//! inverse rotation, so the reader conjugates each non-root bone's quaternion after the frame conversion. This was
//! not taken on trust: composing the installed meshes' bones this way reproduces the absolute joints of UModel's
//! own MD5 export of the same meshes to 3.3e-5 uu (Hands_Siren, Skel_SirenBody; `local/p2/h6-anim/`), while the
//! stored quaternions used as they are miss by more than 100 uu.
//!
//! **Checks.** Every count meets its chunk's bytes; every index is in range; every number is finite; every
//! quaternion is of unit length; bones are in parent order (bone 0 is the root, ActorX gives it parent 0; every other
//! bone's parent comes before it), so walking them in order composes every parent first and needs no recursion; no
//! two bones share a name; each point has one to four influences with weights in 0..=1 summing to 1 within 1e-3.
//! Nothing is clamped or renormalised.

use std::path::Path;

use ow_core::NotYet;

use crate::actorx::{
    self, ActorXError, Chunk, all_finite, f32_at, fixed_name, i32_at, near_unit, quat_at, u16_at,
    unmirror_position, unmirror_quat, vec3_at,
};

/// Most influences a vertex carries (UE3's GPU skin vertex has four).
pub const MAX_INFLUENCES: usize = 4;

/// A bone's local transform: from the bone's frame to its parent's, `p_parent = q p q* + t`.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoneTransform {
    /// Unit quaternion `(x, y, z, w)`.
    pub rotation: [f32; 4],
    /// Translation in uu.
    pub translation: [f32; 3],
}

impl BoneTransform {
    pub const IDENTITY: BoneTransform = BoneTransform {
        rotation: [0.0, 0.0, 0.0, 1.0],
        translation: [0.0; 3],
    };

    pub fn new(rotation: [f32; 4], translation: [f32; 3]) -> Self {
        BoneTransform {
            rotation,
            translation,
        }
    }
}

/// One bone of the reference skeleton.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct Bone {
    pub name: String,
    /// `None` for the root (bone 0).
    pub parent: Option<usize>,
    /// The bind (reference) pose, relative to the parent.
    pub bind: BoneTransform,
}

impl Bone {
    pub fn new(name: impl Into<String>, parent: Option<usize>, bind: BoneTransform) -> Self {
        Bone {
            name: name.into(),
            parent,
            bind,
        }
    }
}

/// One bone's share of a vertex.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Influence {
    pub bone: usize,
    pub weight: f32,
}

impl Influence {
    pub fn new(bone: usize, weight: f32) -> Self {
        Influence { bone, weight }
    }
}

/// A vertex at the bind pose: a wedge of the export (its point's position and influences, its own UV).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct SkinVertex {
    /// In uu, in the mesh's frame.
    pub position: [f32; 3],
    pub uv: [f32; 2],
    /// The welded point the vertex came from (vertices that share a point share its position and weights).
    pub point: usize,
    /// One to [`MAX_INFLUENCES`] influences.
    pub influences: Vec<Influence>,
}

impl SkinVertex {
    pub fn new(position: [f32; 3], uv: [f32; 2], point: usize, influences: Vec<Influence>) -> Self {
        SkinVertex {
            position,
            uv,
            point,
            influences,
        }
    }
}

/// A run of triangles that draw with one material.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkinSection {
    /// Index into [`SkeletalMesh::materials`].
    pub material: usize,
    /// First entry of [`SkeletalMesh::indices`] (a multiple of 3).
    pub first_index: usize,
    pub triangle_count: usize,
}

/// A skeletal mesh at its bind pose, in the game's frame.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct SkeletalMesh {
    /// The `ACTRHEAD` type flag (UModel 1590 writes 20100422).
    pub version: u32,
    pub bones: Vec<Bone>,
    pub vertices: Vec<SkinVertex>,
    /// Three vertex indices per triangle, in the game's order.
    pub indices: Vec<u32>,
    pub sections: Vec<SkinSection>,
    pub materials: Vec<String>,
    /// Welded points in the export (vertices that share a position share one).
    pub point_count: usize,
    /// Chunks the reader does not read: (name, record count).
    pub ignored_chunks: Vec<(String, usize)>,
}

impl SkinSection {
    pub fn new(material: usize, first_index: usize, triangle_count: usize) -> Self {
        SkinSection {
            material,
            first_index,
            triangle_count,
        }
    }
}

impl SkeletalMesh {
    /// A mesh from parts (for callers that build one, such as tests). Nothing is checked here: [`read_psk`] is the
    /// checked path.
    pub fn new(
        bones: Vec<Bone>,
        vertices: Vec<SkinVertex>,
        indices: Vec<u32>,
        sections: Vec<SkinSection>,
        materials: Vec<String>,
    ) -> Self {
        let point_count = vertices.len();
        SkeletalMesh {
            version: 0,
            bones,
            vertices,
            indices,
            sections,
            materials,
            point_count,
            ignored_chunks: Vec::new(),
        }
    }

    /// The index of the bone called `name`.
    pub fn bone_index(&self, name: &str) -> Option<usize> {
        self.bones.iter().position(|b| b.name == name)
    }

    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }
}

/// Reads a `.psk` or `.pskx` file.
pub fn read_psk_file(path: &Path) -> Result<SkeletalMesh, ActorXError> {
    read_psk(&actorx::read_file(path)?)
}

/// The chunks this reader reads; any other is listed as ignored.
const READ_CHUNKS: [&str; 8] = [
    "ACTRHEAD",
    "PNTS0000",
    "VTXW0000",
    "FACE0000",
    "FACE3200",
    "MATT0000",
    "REFSKELT",
    "RAWWEIGHTS",
];

/// Reads an ActorX skeletal mesh from its bytes.
pub fn read_psk(data: &[u8]) -> Result<SkeletalMesh, ActorXError> {
    let chunks = actorx::chunks(data)?;
    let first = chunks
        .first()
        .map(|c| c.name.to_string())
        .unwrap_or_default();
    if first != "ACTRHEAD" {
        return Err(ActorXError::WrongFile {
            expected: "ACTRHEAD",
            found: first,
        });
    }
    let version = actorx::require(&chunks, "ACTRHEAD")?.type_flag;
    let points = read_points(actorx::require(&chunks, "PNTS0000")?)?;
    let materials = read_materials(actorx::require(&chunks, "MATT0000")?)?;
    let wedges = read_wedges(
        actorx::require(&chunks, "VTXW0000")?,
        points.len(),
        materials.len(),
    )?;
    let faces = match (
        actorx::find(&chunks, "FACE0000")?,
        actorx::find(&chunks, "FACE3200")?,
    ) {
        (Some(_), Some(_)) => {
            return Err(ActorXError::DuplicateChunk("FACE0000 and FACE3200".into()));
        }
        (Some(c), None) => read_faces(c, false, wedges.len(), materials.len())?,
        (None, Some(c)) => read_faces(c, true, wedges.len(), materials.len())?,
        (None, None) => return Err(ActorXError::MissingChunk("FACE0000")),
    };
    let bones = read_bones(actorx::require(&chunks, "REFSKELT")?, "REFSKELT", true)?;
    let influences = read_influences(
        actorx::require(&chunks, "RAWWEIGHTS")?,
        points.len(),
        bones.len(),
    )?;

    let mut vertices = Vec::with_capacity(wedges.len());
    for w in &wedges {
        vertices.push(SkinVertex::new(
            points[w.point],
            w.uv,
            w.point,
            influences[w.point].clone(),
        ));
    }
    let mut indices = Vec::with_capacity(faces.len() * 3);
    let mut sections: Vec<SkinSection> = Vec::new();
    for (i, f) in faces.iter().enumerate() {
        indices.extend_from_slice(&f.corners);
        match sections.last_mut() {
            Some(s) if s.material == f.material => s.triangle_count += 1,
            _ => sections.push(SkinSection {
                material: f.material,
                first_index: i * 3,
                triangle_count: 1,
            }),
        }
    }
    let ignored_chunks = chunks
        .iter()
        .filter(|c| !READ_CHUNKS.contains(&c.name))
        .map(|c| (c.name.to_string(), c.count))
        .collect();
    Ok(SkeletalMesh {
        version,
        bones,
        vertices,
        indices,
        sections,
        materials,
        point_count: points.len(),
        ignored_chunks,
    })
}

fn read_points(c: Chunk<'_>) -> Result<Vec<[f32; 3]>, ActorXError> {
    c.expect_size(12)?;
    let mut out = Vec::with_capacity(c.count);
    for i in 0..c.count {
        let p = vec3_at(c.record(i), 0);
        if !all_finite(&p) {
            return Err(ActorXError::NonFinite {
                chunk: "PNTS0000",
                index: i,
            });
        }
        out.push(unmirror_position(p));
    }
    Ok(out)
}

fn read_materials(c: Chunk<'_>) -> Result<Vec<String>, ActorXError> {
    c.expect_size(88)?;
    (0..c.count)
        .map(|i| {
            fixed_name(&c.record(i)[..64])
                .map(str::to_string)
                .ok_or(ActorXError::BadName {
                    chunk: "MATT0000",
                    index: i,
                })
        })
        .collect()
}

struct Wedge {
    point: usize,
    uv: [f32; 2],
}

fn read_wedges(c: Chunk<'_>, points: usize, materials: usize) -> Result<Vec<Wedge>, ActorXError> {
    c.expect_size(16)?;
    let wide = c.count > 65536;
    let mut out = Vec::with_capacity(c.count);
    for i in 0..c.count {
        let r = c.record(i);
        let point = if wide {
            i64::from(i32_at(r, 0))
        } else {
            i64::from(u16_at(r, 0))
        };
        let point = index(point, points, "VTXW0000", i)?;
        let uv = [f32_at(r, 4), f32_at(r, 8)];
        if !all_finite(&uv) {
            return Err(ActorXError::NonFinite {
                chunk: "VTXW0000",
                index: i,
            });
        }
        index(i64::from(r[12]), materials, "VTXW0000", i)?;
        out.push(Wedge { point, uv });
    }
    Ok(out)
}

struct Face {
    corners: [u32; 3],
    material: usize,
}

fn read_faces(
    c: Chunk<'_>,
    wide: bool,
    wedges: usize,
    materials: usize,
) -> Result<Vec<Face>, ActorXError> {
    let (size, chunk) = if wide {
        (18, "FACE3200")
    } else {
        (12, "FACE0000")
    };
    c.expect_size(size)?;
    let mut out = Vec::with_capacity(c.count);
    for i in 0..c.count {
        let r = c.record(i);
        let mut w = [0u32; 3];
        for (k, slot) in w.iter_mut().enumerate() {
            let raw = if wide {
                i64::from(i32_at(r, k * 4))
            } else {
                i64::from(u16_at(r, k * 2))
            };
            let checked = index(raw, wedges, chunk, i)?;
            // In range of a Vec index and below 2^31 (wedge counts come from an i32), so it fits a u32.
            *slot = u32::try_from(checked).map_err(|_| ActorXError::IndexOutOfRange {
                chunk,
                index: i,
                value: raw,
                limit: wedges,
            })?;
        }
        let material = index(i64::from(r[if wide { 12 } else { 6 }]), materials, chunk, i)?;
        // The export's mirror swapped the first two corners; swap them back.
        out.push(Face {
            corners: [w[1], w[0], w[2]],
            material,
        });
    }
    Ok(out)
}

/// Reads a bone table (`REFSKELT` of a mesh, `BONENAMES` of an animation set). For a mesh (`skeleton`), parents
/// are checked and every non-root rotation is conjugated into [`BoneTransform`]'s convention; an animation set's
/// table carries only names (UModel writes no parents or poses there), so only the names are kept from it.
pub(crate) fn read_bones(
    c: Chunk<'_>,
    chunk: &'static str,
    skeleton: bool,
) -> Result<Vec<Bone>, ActorXError> {
    c.expect_size(120)?;
    let mut out: Vec<Bone> = Vec::with_capacity(c.count);
    for i in 0..c.count {
        let r = c.record(i);
        let name = fixed_name(&r[..64])
            .ok_or(ActorXError::BadName { chunk, index: i })?
            .to_string();
        if out.iter().any(|b| b.name == name) {
            return Err(ActorXError::DuplicateBone(name));
        }
        if !skeleton {
            out.push(Bone::new(name, None, BoneTransform::IDENTITY));
            continue;
        }
        let parent_raw = i64::from(i32_at(r, 72));
        let parent = match (i, parent_raw) {
            (0, 0) => None,
            (0, _) => {
                return Err(ActorXError::BoneOrder {
                    index: 0,
                    parent: parent_raw,
                });
            }
            (_, p) if p >= 0 && p < i64::try_from(i).unwrap_or(i64::MAX) => {
                Some(usize::try_from(p).map_err(|_| ActorXError::BoneOrder {
                    index: i,
                    parent: p,
                })?)
            }
            (_, p) => {
                return Err(ActorXError::BoneOrder {
                    index: i,
                    parent: p,
                });
            }
        };
        let q = quat_at(r, 76);
        let t = vec3_at(r, 92);
        if !all_finite(&q) || !all_finite(&t) {
            return Err(ActorXError::NonFinite { chunk, index: i });
        }
        if !near_unit(q) {
            return Err(ActorXError::NotUnit { chunk, index: i });
        }
        let q = unmirror_quat(q);
        let rotation = if parent.is_some() { conjugate(q) } else { q };
        out.push(Bone::new(
            name,
            parent,
            BoneTransform::new(rotation, unmirror_position(t)),
        ));
    }
    if skeleton && out.is_empty() {
        return Err(ActorXError::MissingChunk("REFSKELT"));
    }
    Ok(out)
}

/// `(x, y, z, w)` to `(-x, -y, -z, w)`.
pub(crate) fn conjugate(q: [f32; 4]) -> [f32; 4] {
    [-q[0], -q[1], -q[2], q[3]]
}

fn read_influences(
    c: Chunk<'_>,
    points: usize,
    bones: usize,
) -> Result<Vec<Vec<Influence>>, ActorXError> {
    c.expect_size(12)?;
    let mut out: Vec<Vec<Influence>> = vec![Vec::new(); points];
    for i in 0..c.count {
        let r = c.record(i);
        let weight = f32_at(r, 0);
        if !weight.is_finite() {
            return Err(ActorXError::NonFinite {
                chunk: "RAWWEIGHTS",
                index: i,
            });
        }
        if !(0.0..=1.0).contains(&weight) {
            return Err(ActorXError::BadWeight { index: i });
        }
        let point = index(i64::from(i32_at(r, 4)), points, "RAWWEIGHTS", i)?;
        let bone = index(i64::from(i32_at(r, 8)), bones, "RAWWEIGHTS", i)?;
        if out[point].len() == MAX_INFLUENCES {
            return Err(ActorXError::TooManyInfluences { point });
        }
        out[point].push(Influence::new(bone, weight));
    }
    for (point, list) in out.iter().enumerate() {
        if list.is_empty() {
            return Err(ActorXError::Unweighted { point });
        }
        let sum: f64 = list.iter().map(|w| f64::from(w.weight)).sum();
        if (sum - 1.0).abs() > 1e-3 {
            return Err(ActorXError::WeightSum { point });
        }
    }
    Ok(out)
}

/// `value` as an index below `limit`.
pub(crate) fn index(
    value: i64,
    limit: usize,
    chunk: &'static str,
    record: usize,
) -> Result<usize, ActorXError> {
    usize::try_from(value)
        .ok()
        .filter(|&v| v < limit)
        .ok_or(ActorXError::IndexOutOfRange {
            chunk,
            index: record,
            value,
            limit,
        })
}

/// A mesh the caller asked for that this reader cannot give: a level of detail other than the first. UModel writes
/// further LODs to `<name>_Lod<n>.psk` files only with `-lods`; this lane reads LOD 0 only.
pub fn lod_not_read(lod: usize) -> ActorXError {
    ActorXError::NotYet(NotYet::not_ported(
        format!("skeletal mesh LOD {lod} (only LOD 0 is read)"),
        "docs/decisions/2026-11/2026-11-16-53",
    ))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::actorx::build::{chunk, floats, ints, name};

    // Invented values only (AGENTS.md rule 9): a quad on two bones, written as the export writes it (mirrored).
    fn bone_record(n: &str, parent: i32, q: [f32; 4], t: [f32; 3]) -> Vec<u8> {
        let mut r = name(n, 64);
        r.extend(ints(&[0, 0, parent]));
        r.extend(floats(&q));
        r.extend(floats(&t));
        r.extend(floats(&[1.0, 0.0, 0.0, 0.0]));
        r
    }

    fn wedge(point: u16, u: f32, v: f32) -> Vec<u8> {
        let mut r = point.to_le_bytes().to_vec();
        r.extend([0, 0]);
        r.extend(floats(&[u, v]));
        r.extend([0, 0, 0, 0]);
        r
    }

    fn face(a: u16, b: u16, c: u16, mat: u8) -> Vec<u8> {
        let mut r: Vec<u8> = [a, b, c].iter().flat_map(|x| x.to_le_bytes()).collect();
        r.extend([mat, 0]);
        r.extend(1u32.to_le_bytes());
        r
    }

    fn weight(w: f32, point: i32, bone: i32) -> Vec<u8> {
        let mut r = floats(&[w]);
        r.extend(ints(&[point, bone]));
        r
    }

    pub(crate) fn quad_psk(weights: &[(f32, i32, i32)]) -> Vec<u8> {
        let mut f = chunk("ACTRHEAD", 20100422, 0, &[]);
        // Points in the export's frame: the game's (1, 2, 3) is written (1, -2, 3).
        let points = [
            [0.0, 0.0, 0.0],
            [10.0, 0.0, 0.0],
            [10.0, -10.0, 0.0],
            [0.0, -10.0, 0.0],
        ];
        f.extend(chunk("PNTS0000", 0, 12, &points.map(|p| floats(&p))));
        f.extend(chunk(
            "VTXW0000",
            0,
            16,
            &[
                wedge(0, 0.0, 0.0),
                wedge(1, 1.0, 0.0),
                wedge(2, 1.0, 1.0),
                wedge(3, 0.0, 1.0),
            ],
        ));
        f.extend(chunk(
            "FACE0000",
            0,
            12,
            &[face(1, 0, 2, 0), face(2, 0, 3, 1)],
        ));
        let mut mats = Vec::new();
        for m in ["MatA", "MatB"] {
            let mut r = name(m, 64);
            r.extend([0u8; 24]);
            mats.push(r);
        }
        f.extend(chunk("MATT0000", 0, 88, &mats));
        // A child bone rotated 90 degrees about Z in the game's frame, at (0, 5, 0) under the root. The game's
        // child rotation q = (0, 0, s, s) (s = sqrt(1/2)) is stored inverted, (0, 0, -s, s), then mirrored: (0, 0, -s, -s).
        let s = std::f32::consts::FRAC_1_SQRT_2;
        f.extend(chunk(
            "REFSKELT",
            0,
            120,
            &[
                bone_record("Root", 0, [0.0, 0.0, 0.0, 1.0], [0.0; 3]),
                bone_record("Child", 0, [0.0, 0.0, -s, -s], [0.0, -5.0, 0.0]),
            ],
        ));
        let w: Vec<Vec<u8>> = weights.iter().map(|&(w, p, b)| weight(w, p, b)).collect();
        f.extend(chunk("RAWWEIGHTS", 0, 12, &w));
        f.extend(chunk("EXTRAUVS0", 0, 8, &[floats(&[0.0, 0.0])]));
        f
    }

    fn good_weights() -> Vec<(f32, i32, i32)> {
        vec![
            (1.0, 0, 0),
            (0.25, 1, 0),
            (0.75, 1, 1),
            (1.0, 2, 1),
            (0.5, 3, 0),
            (0.5, 3, 1),
        ]
    }

    #[test]
    fn reads_a_quad_in_the_game_frame() {
        let m = read_psk(&quad_psk(&good_weights())).unwrap();
        assert_eq!(m.version, 20100422);
        assert_eq!(m.point_count, 4);
        assert_eq!(m.vertices.len(), 4);
        assert_eq!(m.vertices[2].position, [10.0, 10.0, 0.0]);
        assert_eq!(m.vertices[1].uv, [1.0, 0.0]);
        // Corners swapped back: the file's (1, 0, 2) is the game's (0, 1, 2).
        assert_eq!(m.indices, vec![0, 1, 2, 0, 2, 3]);
        assert_eq!(m.sections.len(), 2);
        assert_eq!(
            (
                m.sections[1].material,
                m.sections[1].first_index,
                m.sections[1].triangle_count
            ),
            (1, 3, 1)
        );
        assert_eq!(m.materials, vec!["MatA".to_string(), "MatB".to_string()]);
        assert_eq!(m.ignored_chunks, vec![("EXTRAUVS0".to_string(), 1)]);
        assert_eq!(m.bones[0].parent, None);
        assert_eq!(m.bones[1].parent, Some(0));
        assert_eq!(m.bones[1].bind.translation, [0.0, 5.0, 0.0]);
        let s = std::f32::consts::FRAC_1_SQRT_2;
        assert_eq!(m.bones[1].bind.rotation, [0.0, 0.0, s, s]);
        assert_eq!(m.bones[0].bind.rotation, [0.0, -0.0, 0.0, -1.0]);
        assert_eq!(m.bone_index("Child"), Some(1));
    }

    #[test]
    fn weights_sum_to_one_per_vertex() {
        let m = read_psk(&quad_psk(&good_weights())).unwrap();
        for v in &m.vertices {
            let sum: f32 = v.influences.iter().map(|i| i.weight).sum();
            assert!((sum - 1.0).abs() < 1e-6, "{sum}");
            assert!(!v.influences.is_empty() && v.influences.len() <= MAX_INFLUENCES);
        }
        assert_eq!(m.vertices[3].influences.len(), 2);
    }

    #[test]
    fn refuses_bad_weights() {
        let mut w = good_weights();
        w[1].0 = 0.2;
        assert_eq!(
            read_psk(&quad_psk(&w)),
            Err(ActorXError::WeightSum { point: 1 })
        );
        let w: Vec<_> = good_weights().into_iter().filter(|x| x.1 != 2).collect();
        assert_eq!(
            read_psk(&quad_psk(&w)),
            Err(ActorXError::Unweighted { point: 2 })
        );
        let mut w = good_weights();
        w[0] = (1.5, 0, 0);
        assert_eq!(
            read_psk(&quad_psk(&w)),
            Err(ActorXError::BadWeight { index: 0 })
        );
        let mut w = good_weights();
        w[0] = (1.0, 0, 7);
        assert!(matches!(
            read_psk(&quad_psk(&w)),
            Err(ActorXError::IndexOutOfRange {
                chunk: "RAWWEIGHTS",
                ..
            })
        ));
        let mut w = good_weights();
        w.extend([(0.0, 0, 0), (0.0, 0, 0), (0.0, 0, 0), (0.0, 0, 0)]);
        assert_eq!(
            read_psk(&quad_psk(&w)),
            Err(ActorXError::TooManyInfluences { point: 0 })
        );
    }

    #[test]
    fn refuses_bones_out_of_parent_order_and_wrong_files() {
        let mut f = quad_psk(&good_weights());
        // The child's parent field (record 1 of REFSKELT, byte 72) set to itself.
        let at = find_chunk(&f, "REFSKELT") + 120 + 72;
        f[at..at + 4].copy_from_slice(&1i32.to_le_bytes());
        assert_eq!(
            read_psk(&f),
            Err(ActorXError::BoneOrder {
                index: 1,
                parent: 1
            })
        );
        let mut g = quad_psk(&good_weights());
        g[0] = b'B';
        assert!(matches!(read_psk(&g), Err(ActorXError::WrongFile { .. })));
        let mut h = quad_psk(&good_weights());
        let at = find_chunk(&h, "REFSKELT") + 120 + 76 + 12;
        h[at..at + 4].copy_from_slice(&0.5f32.to_le_bytes());
        assert_eq!(
            read_psk(&h),
            Err(ActorXError::NotUnit {
                chunk: "REFSKELT",
                index: 1
            })
        );
    }

    /// Byte offset of the first record of chunk `name`.
    fn find_chunk(f: &[u8], name: &str) -> usize {
        let mut at = 0;
        loop {
            let size = usize::try_from(i32_at(f, at + 24)).unwrap();
            let count = usize::try_from(i32_at(f, at + 28)).unwrap();
            if fixed_name(&f[at..at + 20]) == Some(name) {
                return at + 32;
            }
            at += 32 + size * count;
        }
    }

    #[test]
    fn lods_past_the_first_are_not_yet() {
        assert!(matches!(lod_not_read(1), ActorXError::NotYet(_)));
    }
}
