//! Owner: lane SKELMESH-LOD-LAYER. A `SkeletalMesh` export's native data, read from the 832/46 package: the prefix up
//! to its LOD model count ([`read_lod_count`]), the count `SkeletalMeshComponent.SetSkeletalMesh`'s LOD row asks for
//! (NATIVE_SKELMESH_MESH_CHANGE.md row 15, `ow_world::world::skeletal::lod_count`), and the LOD models to the export's
//! end ([`read_lod_models`]). AI-assisted (Claude Code). Decision 2026-11-25-NNN.
//!
//! **Fitted, UNVERIFIED** (AGENTS.md rule 4: no public spec). Recovered from the installed packages and checked with
//! structural oracles, as decision 2026-11-25-NNN records. After the tagged properties, in order:
//! 1. the bounds: 28 bytes (an origin, a box extent, a sphere radius);
//! 2. the materials: a count, then one 4-byte object reference each;
//! 3. the origin and the rotation origin: 12 bytes each;
//! 4. the reference skeleton: a count, then one 52-byte bone each: a name (an 8-byte name-table entry), 4 bytes of
//!    flags, a rotation (16), a position (12), the child count, the parent index, and a 4-byte colour;
//! 5. the skeletal depth: the longest root-to-leaf bone chain, counting both ends;
//! 6. **the LOD model count**.
//!
//! **The checks** (each failure is [`LodCountError`], never a guess):
//! - every count fits the export's bytes;
//! - every material reference names an object of the package;
//! - every bone name is a valid name-table entry and every position and rotation number is finite;
//! - bone 0 is its own parent (the root) and every other bone's parent comes before it;
//! - the stored depth equals the depth the parent chain gives;
//! - the LOD count is between 1 and 32 and **equals the length of the mesh's own tagged `LODInfo` array** (one entry
//!   per LOD), which is read independently from the tag stream.
//!
//! The child counts are not checked. In the census they sum to the bone count less one for every mesh but one
//! composite (gestalt) mesh, so their meaning there is not settled.
//!
//! [`read_lod_count`] reads nothing past the count.
//!
//! **The LOD models** ([`read_lod_models`]): one model per LOD, each in this order, with what is checked:
//! 1. the sections: a count (at least one), then 13 bytes each: a material index (2; not checked, because a
//!    placeholder cube in the census holds 255 against one material), a chunk index (2; must name a chunk of the
//!    model), the first index (4), the triangle count (4), a sort byte;
//! 2. the index buffer: a CPU-access word (0 or 1), the index width byte (2 or 4), then an array whose element size
//!    must equal that width and whose length must be **three times the sections' triangles**;
//! 3. the active bones: a count, then 2-byte bone indices (each a bone of the skeleton);
//! 4. the chunks: a count, then each: its first vertex (**the sum of the chunks before it**), two vertex arrays that
//!    must be empty (cooked meshes strip them; a non-empty one is refused, not decoded), a bone map (2-byte bone
//!    indices), the rigid and soft vertex counts, and an influence count (1 to 4);
//! 5. a word (not checked), then the vertex count, which must **equal the chunks' vertices**;
//! 6. the required bones: a count, then one bone index byte each;
//! 7. a bulk-data block: flags (0), an element count, a size, and a position that must be **the block's own position
//!    in the package**; the `size` bytes after it are skipped;
//! 8. the vertex buffer: the UV set count (1 to 4) twice, which must agree, a full-precision UV flag (0, the only
//!    value seen), a packed-position flag (0 or 1), two finite 3-vectors, then an array whose element size must be
//!    **28 + 4 per UV set** and whose length must equal the vertex count;
//! 9. when the mesh's tagged `bHasVertexColors` is true (and only then), a colour array of 4-byte elements, one per
//!    vertex;
//! 10. **the trailing count**: the one LOD-model field whose meaning is not identified. Zero in every census LOD; a
//!     non-zero count is refused ([`LodCountError::TrailingArray`]), because its elements are not decoded.
//!
//! After the last model: the bone-name map, one (name, bone) pair per bone, where **each name must be that bone's own
//! name** in the reference skeleton and each bone appears once; then three words (zero, refused otherwise), an array of
//! either no entry or one 4-byte entry per material, and an array of 4-byte entries. **The last byte read must be the
//! export's last.** The LOD models' vertex, index and colour data are stepped over, not decoded.

use std::fmt;

use ow_package::package::Package;
use ow_package::props::{self, ArrayTyper};

use ow_package::reader::Reader;

use crate::texture::{AssetError, check, class_name, count, read_u16, real};

/// The bytes of one reference-skeleton bone (module documentation, item 4).
pub const BONE_BYTES: usize = 52;
/// The highest LOD count the reader accepts.
pub const MAX_LODS: usize = 32;
/// The bytes of one LOD-model section: material (2), chunk (2), first index (4), triangle count (4), a sort byte.
pub const SECTION_BYTES: usize = 13;
/// The fewest bytes a chunk takes: its first vertex, two empty vertex arrays, an empty bone map, three counts.
const CHUNK_MIN_BYTES: usize = 28;
/// A vertex's bytes before its UV sets, each of which adds 4 (fitted from the census, where it holds for every LOD).
pub const VERTEX_BASE_BYTES: usize = 28;

/// What the reader returns: the counts up to and including the LOD count.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkeletalLods {
    pub materials: usize,
    pub bones: usize,
    pub depth: usize,
    pub lod_count: usize,
    /// The absolute package position just after the LOD count, where the first LOD model starts.
    pub lods_at: usize,
    /// The absolute position where the export's payload ends.
    pub object_end: usize,
}

/// Why a `SkeletalMesh` prefix was refused.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum LodCountError {
    Asset(AssetError),
    /// The export is not a `SkeletalMesh`.
    NotSkeletalMesh(String),
    /// A bone's position or rotation holds a non-finite number.
    NonFinite { bone: usize },
    /// A bone's parent index is out of order.
    Parent { bone: usize, parent: i32 },
    /// A negative child count.
    Children { bone: usize, children: i32 },
    /// The stored depth differs from the parent chain's.
    Depth { stored: i32, computed: usize },
    /// The LOD count is out of range.
    LodRange(i32),
    /// The mesh has no tagged `LODInfo` array to check the count against.
    NoLodInfo,
    /// The native count and the tagged `LODInfo` length differ.
    LodInfo { native: usize, tagged: i32 },
    /// A LOD-model field outside the fitted layout: the LOD, the field, the value found.
    LodModel { lod: usize, field: &'static str, found: i64 },
    /// A LOD model's trailing count is not zero: its elements are not decoded.
    TrailingArray { lod: usize, count: i32 },
    /// The bone-name map's length, or its entry `entry`, does not match the reference skeleton.
    NameMap { entry: usize },
    /// The trailer after the name map holds a value the fitted layout does not accept.
    Trailer(i64),
    /// The fitted layout ends this many bytes before the export does.
    Leftover { bytes: usize },
}

impl fmt::Display for LodCountError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LodCountError::Asset(e) => write!(f, "{e}"),
            LodCountError::NotSkeletalMesh(class) => write!(f, "the export is a {class}, not a SkeletalMesh"),
            LodCountError::NonFinite { bone } => write!(f, "bone {bone} holds a non-finite number"),
            LodCountError::Parent { bone, parent } => write!(f, "bone {bone} names parent {parent}"),
            LodCountError::Children { bone, children } => write!(f, "bone {bone} holds a child count of {children}"),
            LodCountError::Depth { stored, computed } => {
                write!(f, "the stored skeletal depth {stored} differs from the bones' {computed}")
            }
            LodCountError::LodRange(n) => write!(f, "a LOD count of {n} (1 to {MAX_LODS} accepted)"),
            LodCountError::NoLodInfo => write!(f, "no tagged LODInfo array to check the LOD count against"),
            LodCountError::LodInfo { native, tagged } => {
                write!(f, "the LOD count {native} differs from the tagged LODInfo array's {tagged} entries")
            }
            LodCountError::LodModel { lod, field, found } => {
                write!(f, "LOD model {lod}: a {field} of {found}, outside the fitted layout")
            }
            LodCountError::TrailingArray { lod, count } => {
                write!(f, "LOD model {lod}: a trailing count of {count}, whose elements are not decoded")
            }
            LodCountError::NameMap { entry } => write!(f, "the bone-name map's entry {entry} misfits the skeleton"),
            LodCountError::Trailer(v) => write!(f, "a trailer value of {v}, outside the fitted layout"),
            LodCountError::Leftover { bytes } => write!(f, "{bytes} bytes left after the fitted layout"),
        }
    }
}

impl std::error::Error for LodCountError {}

impl<E: Into<AssetError>> From<E> for LodCountError {
    fn from(e: E) -> Self {
        LodCountError::Asset(e.into())
    }
}

/// Reads export `index`'s native prefix up to its LOD count (module documentation): its tags from `property_offset`,
/// walked with `typer`, then the fitted fields, every check made.
pub fn read_lod_count(
    package: &Package,
    index: i32,
    property_offset: usize,
    typer: &dyn ArrayTyper,
) -> Result<SkeletalLods, LodCountError> {
    read_prefix(package, index, property_offset, typer).map(|p| p.lods)
}

/// What the prefix read leaves for the LOD-model read: the counts, each bone's name-table entry, and the tagged
/// `bHasVertexColors`.
struct Prefix {
    lods: SkeletalLods,
    bone_names: Vec<(i32, i32)>,
    vertex_colors: bool,
}

fn read_prefix(
    package: &Package,
    index: i32,
    property_offset: usize,
    typer: &dyn ArrayTyper,
) -> Result<Prefix, LodCountError> {
    let class = class_name(package, index)?;
    if class != "SkeletalMesh" {
        return Err(LodCountError::NotSkeletalMesh(class));
    }
    let (_, object_end) = props::property_range(package, index, property_offset).map_err(AssetError::from)?;
    let walked = props::read_tagged(package, index, property_offset, typer).map_err(AssetError::from)?;
    let tagged = walked
        .entries
        .iter()
        .find(|e| e.name.eq_ignore_ascii_case("LODInfo") && e.index == 0)
        .and_then(|e| e.tag.as_ref())
        .and_then(|t| t.array.as_ref())
        .map(|a| a.count)
        .ok_or(LodCountError::NoLodInfo)?;
    let vertex_colors = walked
        .entries
        .iter()
        .any(|e| e.name.eq_ignore_ascii_case("bHasVertexColors") && e.value == props::PropValue::Bool(true));
    let base = package.object(index)?.offset as usize;
    let mut r = package.reader();
    r.set_pos(base + walked.end);
    r.set_limit(object_end);
    r.skip(28)?;
    let materials = count(&mut r, 4)?;
    for _ in 0..materials {
        let m = r.i32()?;
        if m != 0 {
            package.object(m)?;
        }
    }
    r.skip(24)?;
    let bones = count(&mut r, BONE_BYTES)?;
    check(bones > 0, AssetError::NegativeCount)?;
    let names = i32::try_from(package.names.len()).unwrap_or(i32::MAX);
    let mut level = Vec::with_capacity(bones);
    let mut bone_names = Vec::with_capacity(bones);
    for bone in 0..bones {
        bone_names.push(r.name(names)?);
        r.u32()?;
        for _ in 0..7 {
            if !f32::from_bits(r.u32()?).is_finite() {
                return Err(LodCountError::NonFinite { bone });
            }
        }
        let children = r.i32()?;
        if children < 0 {
            return Err(LodCountError::Children { bone, children });
        }
        let parent = r.i32()?;
        let ok = if bone == 0 { parent == 0 } else { usize::try_from(parent).is_ok_and(|p| p < bone) };
        if !ok {
            return Err(LodCountError::Parent { bone, parent });
        }
        level.push(if bone == 0 { 1 } else { level[parent as usize] + 1 });
        r.u32()?;
    }
    let computed = level.iter().copied().max().unwrap_or(0);
    let stored = r.i32()?;
    if usize::try_from(stored).ok() != Some(computed) {
        return Err(LodCountError::Depth { stored, computed });
    }
    let lods = r.i32()?;
    let lod_count = usize::try_from(lods).ok().filter(|n| (1..=MAX_LODS).contains(n)).ok_or(LodCountError::LodRange(lods))?;
    if i32::try_from(lod_count).ok() != Some(tagged) {
        return Err(LodCountError::LodInfo { native: lod_count, tagged });
    }
    let lods = SkeletalLods { materials, bones, depth: computed, lod_count, lods_at: r.pos(), object_end };
    Ok(Prefix { lods, bone_names, vertex_colors })
}

/// One LOD model's counts (module documentation, "The LOD models").
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LodModel {
    pub sections: usize,
    pub triangles: usize,
    pub chunks: usize,
    pub vertices: usize,
    /// The texture-coordinate sets of the vertex buffer.
    pub uv_sets: usize,
    /// Whether the model carries the per-vertex colour buffer (the mesh's tagged `bHasVertexColors`).
    pub vertex_colors: bool,
    /// The count word that ends the model: the one LOD-model field whose meaning is not identified. A read that
    /// succeeds has it at zero, because a non-zero count is refused ([`LodCountError::TrailingArray`]): its elements
    /// are not decoded.
    pub trailing: usize,
}

/// A `SkeletalMesh` export read to its end: the prefix's counts and every LOD model.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkeletalLodModels {
    pub prefix: SkeletalLods,
    pub models: Vec<LodModel>,
}

/// A LOD-model field outside what the fitted layout accepts.
fn misfit(lod: usize, field: &'static str, found: i64) -> LodCountError {
    LodCountError::LodModel { lod, field, found }
}

/// Reads export `index` to its end (module documentation, "The LOD models"): the prefix of [`read_lod_count`], every
/// LOD model, the bone-name map and the trailer, every check made. The last byte read must be the export's last.
pub fn read_lod_models(
    package: &Package,
    index: i32,
    property_offset: usize,
    typer: &dyn ArrayTyper,
) -> Result<SkeletalLodModels, LodCountError> {
    let Prefix { lods: prefix, bone_names, vertex_colors } = read_prefix(package, index, property_offset, typer)?;
    let mut r = package.reader();
    r.set_pos(prefix.lods_at);
    r.set_limit(prefix.object_end);
    let mut models = Vec::with_capacity(prefix.lod_count);
    for lod in 0..prefix.lod_count {
        models.push(read_model(&mut r, lod, &prefix, vertex_colors)?);
    }
    // The bone-name map: one (name, bone) pair per bone, each name the bone's own, each bone once.
    let entries = count(&mut r, 12)?;
    if entries != prefix.bones {
        return Err(LodCountError::NameMap { entry: entries });
    }
    let names = i32::try_from(package.names.len()).unwrap_or(i32::MAX);
    let mut seen = vec![false; prefix.bones];
    for entry in 0..entries {
        let name = r.name(names)?;
        match usize::try_from(r.i32()?).ok().filter(|&b| b < prefix.bones && !seen[b]) {
            Some(b) if bone_names[b] == name => seen[b] = true,
            _ => return Err(LodCountError::NameMap { entry }),
        }
    }
    // The trailer: three words (zero in the census; anything else is refused, not decoded), an array of no entry or
    // one 4-byte entry per material, and an array of 4-byte entries.
    for _ in 0..3 {
        let word = r.u32()?;
        if word != 0 {
            return Err(LodCountError::Trailer(i64::from(word)));
        }
    }
    let per_material = count(&mut r, 4)?;
    if per_material != 0 && per_material != prefix.materials {
        return Err(LodCountError::Trailer(per_material as i64));
    }
    r.skip(4 * per_material)?;
    let last = count(&mut r, 4)?;
    r.skip(4 * last)?;
    if r.pos() != prefix.object_end {
        return Err(LodCountError::Leftover { bytes: prefix.object_end.saturating_sub(r.pos()) });
    }
    Ok(SkeletalLodModels { prefix, models })
}

/// A byte at the cursor.
fn byte(r: &mut Reader<'_>) -> Result<u8, LodCountError> {
    Ok(r.take(1)?[0])
}

/// A bone index that must name a bone of the reference skeleton.
fn bone(index: u16, lod: usize, prefix: &SkeletalLods, field: &'static str) -> Result<(), LodCountError> {
    if usize::from(index) < prefix.bones { Ok(()) } else { Err(misfit(lod, field, i64::from(index))) }
}

/// One LOD model (module documentation, "The LOD models").
fn read_model(r: &mut Reader<'_>, lod: usize, prefix: &SkeletalLods, vertex_colors: bool) -> Result<LodModel, LodCountError> {
    // 1. The sections.
    let sections = count(r, SECTION_BYTES)?;
    if sections == 0 {
        return Err(misfit(lod, "section count", 0));
    }
    let mut triangles = 0usize;
    let mut section_chunks = Vec::with_capacity(sections);
    for _ in 0..sections {
        // The material index is not checked: a placeholder cube in the census holds 255 against one material.
        read_u16(r)?;
        section_chunks.push(read_u16(r)?);
        r.u32()?;
        triangles = triangles.saturating_add(r.u32()? as usize);
        byte(r)?;
    }
    // 2. The index buffer.
    let access = r.u32()?;
    if access > 1 {
        return Err(misfit(lod, "index CPU-access word", i64::from(access)));
    }
    let width = byte(r)?;
    if width != 2 && width != 4 {
        return Err(misfit(lod, "index width", i64::from(width)));
    }
    let stride = r.u32()?;
    if stride != u32::from(width) {
        return Err(misfit(lod, "index stride", i64::from(stride)));
    }
    let indices = count(r, usize::from(width))?;
    if Some(indices) != triangles.checked_mul(3) {
        return Err(misfit(lod, "index count", indices as i64));
    }
    r.skip(indices * usize::from(width))?;
    // 3. The active bones.
    let active = count(r, 2)?;
    for _ in 0..active {
        bone(read_u16(r)?, lod, prefix, "active bone")?;
    }
    // 4. The chunks, each starting where the one before ends.
    let chunks = count(r, CHUNK_MIN_BYTES)?;
    if let Some(&c) = section_chunks.iter().find(|&&c| usize::from(c) >= chunks) {
        return Err(misfit(lod, "section chunk", i64::from(c)));
    }
    let mut vertices = 0usize;
    for _ in 0..chunks {
        let first = r.u32()? as usize;
        if first != vertices {
            return Err(misfit(lod, "chunk first vertex", first as i64));
        }
        for field in ["chunk rigid vertex array", "chunk soft vertex array"] {
            let n = r.i32()?;
            if n != 0 {
                return Err(misfit(lod, field, i64::from(n)));
            }
        }
        let map = count(r, 2)?;
        for _ in 0..map {
            bone(read_u16(r)?, lod, prefix, "chunk bone map entry")?;
        }
        for field in ["chunk rigid vertex count", "chunk soft vertex count"] {
            let n = r.i32()?;
            let n = usize::try_from(n).map_err(|_| misfit(lod, field, i64::from(n)))?;
            vertices = vertices.checked_add(n).ok_or(misfit(lod, field, n as i64))?;
        }
        let influences = r.i32()?;
        if !(1..=4).contains(&influences) {
            return Err(misfit(lod, "chunk influence count", i64::from(influences)));
        }
    }
    // 5. A word not checked, then the vertex count.
    r.u32()?;
    let stated = r.u32()? as usize;
    if stated != vertices {
        return Err(misfit(lod, "vertex count", stated as i64));
    }
    // 6. The required bones, one byte each.
    let required = count(r, 1)?;
    for _ in 0..required {
        bone(u16::from(byte(r)?), lod, prefix, "required bone")?;
    }
    // 7. A bulk-data block: flags, element count, size, and its own position in the package.
    let flags = r.u32()?;
    if flags != 0 {
        return Err(misfit(lod, "bulk flags", i64::from(flags)));
    }
    r.i32()?;
    let size = r.i32()?;
    let size = usize::try_from(size).map_err(|_| misfit(lod, "bulk size", i64::from(size)))?;
    let at = r.u32()? as usize;
    if at != r.pos() {
        return Err(misfit(lod, "bulk position", at as i64));
    }
    r.skip(size)?;
    // 8. The vertex buffer.
    let uv_sets = r.u32()? as usize;
    let again = r.u32()? as usize;
    if !(1..=4).contains(&uv_sets) || again != uv_sets {
        return Err(misfit(lod, "UV set count", again as i64));
    }
    let full_uvs = r.u32()?;
    if full_uvs != 0 {
        return Err(misfit(lod, "full-precision UV flag", i64::from(full_uvs)));
    }
    let packed = r.u32()?;
    if packed > 1 {
        return Err(misfit(lod, "packed-position flag", i64::from(packed)));
    }
    for _ in 0..6 {
        real(r)?;
    }
    let stride = r.u32()? as usize;
    if stride != VERTEX_BASE_BYTES + 4 * uv_sets {
        return Err(misfit(lod, "vertex stride", stride as i64));
    }
    let buffered = count(r, stride)?;
    if buffered != vertices {
        return Err(misfit(lod, "vertex buffer count", buffered as i64));
    }
    r.skip(buffered * stride)?;
    // 9. The colour buffer, when the mesh's tag says it has one.
    if vertex_colors {
        let stride = r.u32()?;
        let colours = count(r, 4)?;
        if stride != 4 || colours != vertices {
            return Err(misfit(lod, "colour buffer count", colours as i64));
        }
        r.skip(4 * colours)?;
    }
    // 10. The trailing count.
    let trailing = r.i32()?;
    if trailing != 0 {
        return Err(LodCountError::TrailingArray { lod, count: trailing });
    }
    Ok(LodModel { sections, triangles, chunks, vertices, uv_sets, vertex_colors, trailing: 0 })
}

#[cfg(test)]
#[path = "skeletal_lods_tests.rs"]
mod tests;
