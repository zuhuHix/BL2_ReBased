//! Owner: lane SKELMESH-LOD-LAYER. A `SkeletalMesh` export's native prefix, read from the 832/46 package up to its LOD
//! model count: the count `SkeletalMeshComponent.SetSkeletalMesh`'s LOD row asks for (NATIVE_SKELMESH_MESH_CHANGE.md
//! row 15, `ow_world::world::skeletal::lod_count`). AI-assisted (Claude Code). Decision 2026-11-19-NNN.
//!
//! **Fitted, UNVERIFIED** (AGENTS.md rule 4: no public spec). Recovered from the installed packages and checked with
//! structural oracles, as decision 2026-11-19-NNN records. After the tagged properties, in order:
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
//! Nothing past the count is read: the LOD models themselves stay undecoded.

use std::fmt;

use ow_package::package::Package;
use ow_package::props::{self, ArrayTyper};

use crate::texture::{AssetError, check, class_name, count};

/// The bytes of one reference-skeleton bone (module documentation, item 4).
pub const BONE_BYTES: usize = 52;
/// The highest LOD count the reader accepts.
pub const MAX_LODS: usize = 32;

/// What the reader returns: the counts up to and including the LOD count.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkeletalLods {
    pub materials: usize,
    pub bones: usize,
    pub depth: usize,
    pub lod_count: usize,
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
    for bone in 0..bones {
        r.name(names)?;
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
    Ok(SkeletalLods { materials, bones, depth: computed, lod_count })
}

#[cfg(test)]
#[path = "skeletal_lods_tests.rs"]
mod tests;
