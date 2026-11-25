//! Owner: lane SKELMESH-LOD-LAYER. A `SkeletalMesh` export's native data, read from the 832/46 package: the prefix up
//! to its LOD model count ([`read_lod_count`]), the count `SkeletalMeshComponent.SetSkeletalMesh`'s LOD row asks for
//! (NATIVE_SKELMESH_MESH_CHANGE.md row 15, `ow_world::world::skeletal::lod_count`), and the LOD models to the export's
//! end ([`read_lod_models`]). AI-assisted (Claude Code). Decision 2026-11-25-09; the reference skeleton (lane SKELMESH-REFSKELETON), decision 2026-11-25-NNN.
//!
//! **Fitted, UNVERIFIED** (AGENTS.md rule 4: no public spec). Recovered from the installed packages and checked with
//! structural oracles, as decision 2026-11-25-09 records. After the tagged properties, in order:
//! 1. the bounds: 28 bytes (an origin, a box extent, a sphere radius), seven finite floats;
//! 2. the materials: a count, then one 4-byte object reference each;
//! 3. the origin and the rotation origin: 12 bytes each;
//! 4. the reference skeleton: a count, then one 52-byte bone each: a name (an 8-byte name-table entry), 4 bytes of
//!    flags (zero in every census bone; anything else is refused, its bits not decoded), a rotation (16: a unit
//!    quaternion, its length within a thousandth of 1), a position (12), the child count, the parent index, and a
//!    4-byte colour. [`read_lod_models`] returns the bones as [`RefBone`]s (lane SKELMESH-REFSKELETON); their names,
//!    parents, rotations and positions equal, bone for bone, the `.psk` reader's for UModel's export of the same
//!    mesh in all 106 census meshes (1,398 bones);
//! 5. the skeletal depth: the longest root-to-leaf bone chain, counting both ends;
//! 6. **the LOD model count**.
//!
//! **The checks** (each failure is [`LodCountError`], never a guess):
//! - every count fits the export's bytes;
//! - every material reference names an object of the package;
//! - every bone name is a valid name-table entry and every position and rotation number is finite;
//! - bone 0 is its own parent (the root) and every other bone's parent comes before it;
//! - the stored depth equals the depth the parent chain gives;
//! - the LOD count is between 1 and 32 and **equals the number of decoded entries of the mesh's own tagged `LODInfo`
//!   array** (one entry per LOD), read independently from the tag stream. Each entry is decoded as a tagged
//!   `SkeletalMeshLODInfo` struct, and the entries must fill the tag's size exactly, so a declared count with a body
//!   that does not hold that many entries is refused (review r1 of #673, item 1).
//!
//! The child counts are not checked. In the census they sum to the bone count less one for every mesh but one
//! composite (gestalt) mesh, so their meaning there is not settled.
//!
//! [`read_lod_count`] reads nothing past the count.
//!
//! **The LOD models** ([`read_lod_models`]): one model per LOD, each in this order, with what is checked:
//! 1. the sections: a count (at least one), then 13 bytes each: a material index (2; not checked, because a
//!    placeholder cube in the census holds 255 against one material), a chunk index (2; must name a chunk of the
//!    model), the first index (4), the triangle count (4), a sort byte. After the chunks are read, **each section's
//!    span** (first index plus three per triangle, in checked arithmetic) must fit the index buffer, and every index
//!    in it must name a vertex of the section's own chunk;
//! 2. the index buffer: a CPU-access word (0 or 1), the index width byte (2 or 4), then an array whose element size
//!    must equal that width and whose length must be **three times the sections' triangles**. Every index is
//!    decoded and must name a vertex of the model;
//! 3. the active bones: a count, then 2-byte bone indices (each a bone of the skeleton);
//! 4. the chunks: a count, then each: its first vertex (**the sum of the chunks before it**), two vertex arrays that
//!    must be empty (cooked meshes strip them; a non-empty one is refused, not decoded), a bone map (2-byte bone
//!    indices), the rigid and soft vertex counts, and an influence count (1 to 4);
//! 5. **an unidentified word** (not checked; zero in the census), then the vertex count, which must **equal the
//!    chunks' vertices**;
//! 6. the required bones: a count, then one bone index byte each;
//! 7. a bulk-data block: flags (0), an element count (0), a size (0), and a position that must be **the block's own
//!    position in the package**. The block must be empty, the only state seen: a non-empty one is refused, because
//!    its elements are not decoded;
//! 8. the vertex buffer: the UV set count (1 to 4) twice, which must agree, a full-precision UV flag (0, the only
//!    value seen), a packed-position flag (0 or 1), two finite 3-vectors, then an array whose element size must be
//!    **28 + 4 per UV set** and whose length must equal the vertex count. **Every vertex is decoded field by field**
//!    (the layout is fitted from the census; each field's meaning is UNVERIFIED):
//!    - bytes 0-7, two packed vectors (by fit, the tangent basis' X and Z; a byte `b` stands for `b / 127.5 - 1`):
//!      each of unit length within 0.02 or a zero vector (shorter than 0.01), with a dot product within 0.03 of 0.
//!      Byte 3 is 127 or 128, and byte 7 (by fit, the basis' sign) is 0 or 255;
//!    - bytes 8-15, the influences: four bone indices, then four weights. The weights must sum to 255, and every
//!      weighted index must name an entry of the bone map of the chunk that holds the vertex;
//!    - bytes 16-27, the position: three finite floats inside the mesh's bounds (the prefix's origin plus or minus
//!      its box extent, with a slack of a thousandth of the largest extent);
//!    - from byte 28, one pair of 16-bit values per UV set. Sets 0 and 1 must be finite half floats. Sets 2 and 3
//!      are read as fixed-width pairs with no check: three composite (gestalt) meshes of the census hold values there
//!      that are not finite halves, so what those sets hold is not settled;
//! 9. when the mesh's tagged `bHasVertexColors` is true (and only then), a colour array of 4-byte elements, one per
//!    vertex, each read as a fixed-width value (every 4-byte value is a colour, so the stride and count checks are
//!    the checks);
//! 10. **the trailing count**: an unidentified LOD-model field, like item 5's word. Zero in every census LOD; a
//!     non-zero count is refused ([`LodCountError::TrailingArray`]), because its elements are not decoded.
//!
//! After the last model: the bone-name map, one (name, bone) pair per bone, where **each name must be that bone's own
//! name** in the reference skeleton and each bone appears once; then three words (zero, refused otherwise), an array of
//! either no entry or one word per material (each zero, the only value seen, refused otherwise), and an array of
//! exactly four finite floats (the only shape seen). **The last byte read must be the export's last.**

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
/// Where a vertex's four bone-index bytes start, followed by its four weight bytes (fitted from the census: the
/// weights sum to 255 in every vertex, and every weighted index names an entry of the vertex's chunk bone map).
pub const VERTEX_INFLUENCE_AT: usize = 8;
/// The packed-vector checks of a vertex (fitted from the census): a zero vector is shorter than this.
const PACKED_ZERO: f32 = 0.01;
/// A unit vector's length is within this of 1.
const PACKED_UNIT: f32 = 0.02;
/// The two packed vectors' dot product is at most this far from 0.
const PACKED_RIGHT_ANGLE: f32 = 0.03;
/// The UV sets whose halves must be finite (sets 2 and 3 are read unchecked; [`check_vertex`]).
const CHECKED_UV_SETS: usize = 2;
/// A bone rotation's squared length lies in `UNIT_LOW..=UNIT_HIGH`: within a thousandth of 1 in length (the census'
/// largest departure is under 2e-7). Squares, so no root is taken (rule N3).
const UNIT_LOW: f32 = 0.999 * 0.999;
const UNIT_HIGH: f32 = 1.001 * 1.001;
/// The length of the trailer's last array (the only length the census holds).
const TRAILER_FLOATS: usize = 4;

/// What the reader returns: the counts up to and including the LOD count.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct SkeletalLods {
    pub materials: usize,
    pub bones: usize,
    pub depth: usize,
    pub lod_count: usize,
    /// The absolute package position just after the LOD count, where the first LOD model starts.
    pub lods_at: usize,
    /// The absolute position where the export's payload ends.
    pub object_end: usize,
    /// The bounds' origin and box extent (item 1): every vertex position must lie within them.
    pub bounds_origin: [f32; 3],
    pub bounds_extent: [f32; 3],
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
    /// A bone's flags word is not zero (the only value seen; its bits are not decoded).
    BoneFlags { bone: usize, flags: u32 },
    /// A bone's rotation is not a unit quaternion.
    NotUnit { bone: usize },
    /// The stored depth differs from the parent chain's.
    Depth { stored: i32, computed: usize },
    /// The LOD count is out of range.
    LodRange(i32),
    /// The mesh has no tagged `LODInfo` array to check the count against.
    NoLodInfo,
    /// The tagged `LODInfo` array's elements did not decode.
    LodInfoUndecoded,
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
            LodCountError::BoneFlags { bone, flags } => write!(f, "bone {bone} holds flags {flags:#x}, not decoded"),
            LodCountError::NotUnit { bone } => write!(f, "bone {bone}'s rotation is not a unit quaternion"),
            LodCountError::Depth { stored, computed } => {
                write!(f, "the stored skeletal depth {stored} differs from the bones' {computed}")
            }
            LodCountError::LodRange(n) => write!(f, "a LOD count of {n} (1 to {MAX_LODS} accepted)"),
            LodCountError::NoLodInfo => write!(f, "no tagged LODInfo array to check the LOD count against"),
            LodCountError::LodInfoUndecoded => write!(f, "the tagged LODInfo array's elements did not decode"),
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
    skeleton: Vec<RefBone>,
}

/// One bone of the reference skeleton (module documentation, item 4), as the package stores it. Fitted and
/// UNVERIFIED in meaning: the rotation and position equal, bone for bone, what the project's `.psk` reader gives for
/// UModel's export of the same mesh (decision 2026-11-25-NNN), and that is the evidence for their convention: the
/// bone's transform relative to its parent, `p_parent = q p q* + t` (Hamilton, `(x, y, z, w)`), in the game's frame.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct RefBone {
    /// The name's text (`name_<number - 1>` for a numbered name).
    pub name: String,
    /// `None` for the root (bone 0, stored as its own parent).
    pub parent: Option<usize>,
    /// A unit quaternion `(x, y, z, w)`.
    pub rotation: [f32; 4],
    pub position: [f32; 3],
    /// The stored child count. Not checked: it equals the bone's actual children in all but two census meshes.
    pub children: usize,
    /// A 4-byte colour (every value is a colour).
    pub colour: u32,
}

/// The caller's array typer, with `LODInfo`'s elements typed as tagged `SkeletalMeshLODInfo` structs, so the walk
/// decodes them (the struct's own fields are not used here).
struct LodInfoTyper<'a>(&'a dyn ArrayTyper);

impl ArrayTyper for LodInfoTyper<'_> {
    fn element(&self, scope: &props::ArrayScope<'_>, property: &str) -> Option<props::ElementType> {
        // The export's own root tag only (review r2 of #673): a nested array of that name goes to the caller's typer.
        if scope.path.is_empty() && property.eq_ignore_ascii_case("LODInfo") {
            return Some(props::ElementType::Struct("SkeletalMeshLODInfo".into()));
        }
        self.0.element(scope, property)
    }
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
    let typer = LodInfoTyper(typer);
    let walked = props::read_tagged(package, index, property_offset, &typer).map_err(AssetError::from)?;
    // The decoded entries, not the declared count (review r1 of #673, item 1): the walker decodes each element as a
    // tagged struct and requires the elements to fill the tag's size exactly.
    let tagged = match walked.entries.iter().find(|e| e.name.eq_ignore_ascii_case("LODInfo") && e.index == 0) {
        Some(e) => match &e.value {
            props::PropValue::Array(elements) => i32::try_from(elements.len()).unwrap_or(i32::MAX),
            _ => return Err(LodCountError::LodInfoUndecoded),
        },
        None => return Err(LodCountError::NoLodInfo),
    };
    let vertex_colors = walked
        .entries
        .iter()
        .any(|e| e.name.eq_ignore_ascii_case("bHasVertexColors") && e.value == props::PropValue::Bool(true));
    let base = package.object(index)?.offset as usize;
    let mut r = package.reader();
    r.set_pos(base + walked.end);
    r.set_limit(object_end);
    let mut bounds = [0f32; 7];
    for b in &mut bounds {
        *b = real(&mut r)?;
    }
    let (bounds_origin, bounds_extent) = ([bounds[0], bounds[1], bounds[2]], [bounds[3], bounds[4], bounds[5]]);
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
    let mut skeleton = Vec::with_capacity(bones);
    for bone in 0..bones {
        let (name_index, number) = r.name(names)?;
        bone_names.push((name_index, number));
        let name = ow_package::tables::name_text(&package.names, name_index, number)?;
        let flags = r.u32()?;
        if flags != 0 {
            return Err(LodCountError::BoneFlags { bone, flags });
        }
        let mut numbers = [0f32; 7];
        for n in &mut numbers {
            *n = f32::from_bits(r.u32()?);
            if !n.is_finite() {
                return Err(LodCountError::NonFinite { bone });
            }
        }
        let rotation = [numbers[0], numbers[1], numbers[2], numbers[3]];
        let square = rotation.iter().map(|q| q * q).sum::<f32>();
        if !(UNIT_LOW..=UNIT_HIGH).contains(&square) {
            return Err(LodCountError::NotUnit { bone });
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
        let colour = r.u32()?;
        skeleton.push(RefBone {
            name,
            parent: (bone > 0).then_some(parent as usize),
            rotation,
            position: [numbers[4], numbers[5], numbers[6]],
            children: children as usize,
            colour,
        });
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
    let lods = SkeletalLods { materials, bones, depth: computed, lod_count, lods_at: r.pos(), object_end, bounds_origin, bounds_extent };
    Ok(Prefix { lods, bone_names, vertex_colors, skeleton })
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
    /// The count word that ends the model, an unidentified field (as is the word before the vertex count). A read
    /// that succeeds has it at zero, because a non-zero count is refused ([`LodCountError::TrailingArray`]): its
    /// elements are not decoded.
    pub trailing: usize,
}

/// A `SkeletalMesh` export read to its end: the prefix's counts and every LOD model.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct SkeletalLodModels {
    pub prefix: SkeletalLods,
    pub models: Vec<LodModel>,
    /// The reference skeleton, bone by bone (lane SKELMESH-REFSKELETON). Given only by a read of the whole export,
    /// so every check of the export has held.
    pub skeleton: Vec<RefBone>,
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
    let Prefix { lods: prefix, bone_names, vertex_colors, skeleton } = read_prefix(package, index, property_offset, typer)?;
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
    // The trailer, every element decoded (review r1 of #673, item 2): three words (zero in the census; anything else
    // is refused), an array of no entry or one 4-byte word per material (each zero, the only value seen), and an
    // array of exactly four finite floats (the only shape seen).
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
    for _ in 0..per_material {
        let word = r.u32()?;
        if word != 0 {
            return Err(LodCountError::Trailer(i64::from(word)));
        }
    }
    let last = count(&mut r, 4)?;
    if last != TRAILER_FLOATS {
        return Err(LodCountError::Trailer(last as i64));
    }
    for _ in 0..last {
        real(&mut r)?;
    }
    if r.pos() != prefix.object_end {
        return Err(LodCountError::Leftover { bytes: prefix.object_end.saturating_sub(r.pos()) });
    }
    Ok(SkeletalLodModels { prefix, models, skeleton })
}

/// A byte at the cursor.
fn byte(r: &mut Reader<'_>) -> Result<u8, LodCountError> {
    Ok(r.take(1)?[0])
}

/// A bone index that must name a bone of the reference skeleton.
fn bone(index: u16, lod: usize, prefix: &SkeletalLods, field: &'static str) -> Result<(), LodCountError> {
    if usize::from(index) < prefix.bones { Ok(()) } else { Err(misfit(lod, field, i64::from(index))) }
}

/// A packed unit-vector byte as a number in -1..=1 (`b / 127.5 - 1`).
fn unpack(b: u8) -> f32 {
    f32::from(b) / 127.5 - 1.0
}

/// The three packed components at `v[at..at + 3]`, and their squared length (compared against squared bounds, so
/// no root is taken: rule N3 keeps float functions in `ow_core::num`).
fn packed_vector(v: &[u8], at: usize) -> ([f32; 3], f32) {
    let x = [unpack(v[at]), unpack(v[at + 1]), unpack(v[at + 2])];
    (x, x[0] * x[0] + x[1] * x[1] + x[2] * x[2])
}

/// A half-precision float's bits hold a finite number (its exponent bits are not all set).
fn finite_half(bits: u16) -> bool {
    bits & 0x7C00 != 0x7C00
}

/// One vertex of the GPU-skin buffer, every field decoded (module documentation, item 8). The meanings are fitted
/// from the census and UNVERIFIED; the checks are the oracles that fit holds to in every census vertex.
fn check_vertex(v: &[u8], lod: usize, map: usize, uv_sets: usize, prefix: &SkeletalLods) -> Result<(), LodCountError> {
    // Bytes 0-7: two packed vectors (by fit, the tangent basis' X and Z), each unit length or a zero vector, at right
    // angles; byte 3 is 127 or 128, byte 7 (by fit, the basis' sign) 0 or 255.
    let (x, x_square) = packed_vector(v, 0);
    let (z, z_square) = packed_vector(v, 4);
    let (unit_low, unit_high) = ((1.0 - PACKED_UNIT) * (1.0 - PACKED_UNIT), (1.0 + PACKED_UNIT) * (1.0 + PACKED_UNIT));
    for (square, field) in [(x_square, "vertex packed vector X"), (z_square, "vertex packed vector Z")] {
        let zero = square < PACKED_ZERO * PACKED_ZERO;
        if !(zero || (unit_low..=unit_high).contains(&square)) {
            return Err(misfit(lod, field, (square * 1000.0) as i64));
        }
    }
    let dot = x[0] * z[0] + x[1] * z[1] + x[2] * z[2];
    if dot.abs() > PACKED_RIGHT_ANGLE {
        return Err(misfit(lod, "vertex packed vectors' angle", (dot * 1000.0) as i64));
    }
    if v[3] != 127 && v[3] != 128 {
        return Err(misfit(lod, "vertex byte 3", i64::from(v[3])));
    }
    if v[7] != 0 && v[7] != 255 {
        return Err(misfit(lod, "vertex byte 7", i64::from(v[7])));
    }
    // Bytes 8-15: the influences. The weights sum to 255, and every weighted index names an entry of the chunk's
    // bone map.
    let at = VERTEX_INFLUENCE_AT;
    let (bones, weights) = (&v[at..at + 4], &v[at + 4..at + 8]);
    let total: u32 = weights.iter().map(|&w| u32::from(w)).sum();
    if total != 255 {
        return Err(misfit(lod, "vertex weight total", i64::from(total)));
    }
    if let Some(k) = (0..4).find(|&k| weights[k] > 0 && usize::from(bones[k]) >= map) {
        return Err(misfit(lod, "vertex bone index", i64::from(bones[k])));
    }
    // Bytes 16-27: the position, three finite floats inside the mesh's bounds (origin plus or minus the extent, with
    // a slack of a thousandth of the largest extent, at least a thousandth of a unit).
    let o = &prefix.bounds_origin;
    let e = &prefix.bounds_extent;
    let slack = 1e-3 * e.iter().fold(1.0f32, |m, &x| m.max(x));
    for axis in 0..3 {
        let p = f32::from_le_bytes([v[16 + 4 * axis], v[17 + 4 * axis], v[18 + 4 * axis], v[19 + 4 * axis]]);
        if !p.is_finite() || (p - o[axis]).abs() > e[axis] + slack {
            return Err(misfit(lod, "vertex position", axis as i64));
        }
    }
    // Bytes 28 on: one pair of 16-bit values per UV set. Sets 0 and 1 are finite half floats. Sets 2 and 3 are read
    // as fixed-width 16-bit pairs with no check: in three composite (gestalt) meshes of the census they hold values
    // that are not finite halves, so their meaning there is not settled.
    for set in 0..uv_sets {
        for half in 0..2 {
            let at = VERTEX_BASE_BYTES + 4 * set + 2 * half;
            let bits = u16::from_le_bytes([v[at], v[at + 1]]);
            if set < CHECKED_UV_SETS && !finite_half(bits) {
                return Err(misfit(lod, "vertex UV", set as i64));
            }
        }
    }
    Ok(())
}

/// One LOD model (module documentation, "The LOD models").
fn read_model(r: &mut Reader<'_>, lod: usize, prefix: &SkeletalLods, vertex_colors: bool) -> Result<LodModel, LodCountError> {
    // 1. The sections: (chunk, first index, triangles) kept for the span checks after the chunks.
    let sections = count(r, SECTION_BYTES)?;
    if sections == 0 {
        return Err(misfit(lod, "section count", 0));
    }
    let mut triangles = 0usize;
    let mut spans = Vec::with_capacity(sections);
    for _ in 0..sections {
        // The material index is not checked: a placeholder cube in the census holds 255 against one material.
        read_u16(r)?;
        let chunk = usize::from(read_u16(r)?);
        let first = r.u32()? as usize;
        let tris = r.u32()? as usize;
        triangles = triangles.checked_add(tris).ok_or(misfit(lod, "section triangle count", tris as i64))?;
        byte(r)?;
        spans.push((chunk, first, tris));
    }
    // 2. The index buffer, every index decoded.
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
    let count_of_indices = count(r, usize::from(width))?;
    if Some(count_of_indices) != triangles.checked_mul(3) {
        return Err(misfit(lod, "index count", count_of_indices as i64));
    }
    let mut indices = Vec::with_capacity(count_of_indices);
    for _ in 0..count_of_indices {
        indices.push(if width == 2 { u32::from(read_u16(r)?) } else { r.u32()? } as usize);
    }
    // 3. The active bones.
    let active = count(r, 2)?;
    for _ in 0..active {
        bone(read_u16(r)?, lod, prefix, "active bone")?;
    }
    // 4. The chunks, each starting where the one before ends: (first vertex, vertex count, bone map length).
    let chunk_count = count(r, CHUNK_MIN_BYTES)?;
    let mut chunks = Vec::with_capacity(chunk_count);
    let mut vertices = 0usize;
    for _ in 0..chunk_count {
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
        let mut own = 0usize;
        for field in ["chunk rigid vertex count", "chunk soft vertex count"] {
            let n = r.i32()?;
            let n = usize::try_from(n).map_err(|_| misfit(lod, field, i64::from(n)))?;
            own = own.checked_add(n).ok_or(misfit(lod, field, n as i64))?;
        }
        vertices = vertices.checked_add(own).ok_or(misfit(lod, "chunk vertex count", own as i64))?;
        let influences = r.i32()?;
        if !(1..=4).contains(&influences) {
            return Err(misfit(lod, "chunk influence count", i64::from(influences)));
        }
        chunks.push((first, own, map));
    }
    // The sections against the index buffer and the chunks (review r1 of #673, item 3): each span fits the index
    // buffer, with checked arithmetic, and every index it covers names a vertex of the section's own chunk.
    for &(chunk, first, tris) in &spans {
        let Some(&(chunk_first, chunk_vertices, _)) = chunks.get(chunk) else {
            return Err(misfit(lod, "section chunk", chunk as i64));
        };
        let end = tris.checked_mul(3).and_then(|n| first.checked_add(n)).filter(|&e| e <= indices.len());
        let Some(end) = end else {
            return Err(misfit(lod, "section index span", first as i64));
        };
        if let Some(&i) = indices[first..end].iter().find(|&&i| i < chunk_first || i - chunk_first >= chunk_vertices) {
            return Err(misfit(lod, "section index", i as i64));
        }
    }
    if let Some(&i) = indices.iter().find(|&&i| i >= vertices) {
        return Err(misfit(lod, "index", i as i64));
    }
    // 5. An unidentified word (not checked), then the vertex count.
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
    // 7. A bulk-data block: flags, element count, size, and its own position in the package. It must be empty (the
    // only state seen): its elements are not decoded (review r1 of #673, item 2).
    let flags = r.u32()?;
    if flags != 0 {
        return Err(misfit(lod, "bulk flags", i64::from(flags)));
    }
    let elements = r.i32()?;
    if elements != 0 {
        return Err(misfit(lod, "bulk element count", i64::from(elements)));
    }
    let size = r.i32()?;
    if size != 0 {
        return Err(misfit(lod, "bulk size", i64::from(size)));
    }
    let at = r.u32()? as usize;
    if at != r.pos() {
        return Err(misfit(lod, "bulk position", at as i64));
    }
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
    // Each vertex decoded field by field (fitted, UNVERIFIED meanings; reviews r1 and r2 of #673): module
    // documentation, item 8.
    for &(_, own, map) in &chunks {
        for _ in 0..own {
            let vertex = r.take(stride)?;
            check_vertex(vertex, lod, map, uv_sets, prefix)?;
        }
    }
    // 9. The colour buffer, when the mesh's tag says it has one: one 4-byte colour per vertex, each read as a
    // fixed-width value (every 4-byte value is a colour, so the stride and count checks are the checks).
    if vertex_colors {
        let stride = r.u32()?;
        let colours = count(r, 4)?;
        if stride != 4 || colours != vertices {
            return Err(misfit(lod, "colour buffer count", colours as i64));
        }
        for _ in 0..colours {
            r.u32()?;
        }
    }
    // 10. The trailing count.
    let trailing = r.i32()?;
    if trailing != 0 {
        return Err(LodCountError::TrailingArray { lod, count: trailing });
    }
    Ok(LodModel { sections, triangles, chunks: chunk_count, vertices, uv_sets, vertex_colors, trailing: 0 })
}

#[cfg(test)]
#[path = "skeletal_lods_tests.rs"]
mod tests;
