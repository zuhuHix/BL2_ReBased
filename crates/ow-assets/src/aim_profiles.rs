//! Owner: lane IMPL-AIMOFFSET-PROFILES. `AnimNodeAimOffset.Profiles` read from an export's binary-serialized array
//! (decision 2026-11-25-72). AI-assisted (Claude Code). The layout is **fitted, UNVERIFIED** (AGENTS.md rule 4):
//! recovered from the installed packages by sizes, property flags and values, not read from the serializer.
//!
//! **The layout.** The `Profiles` tag is an `ArrayProperty` whose value is a 4-byte element count, then each
//! `AimOffsetProfile` untagged, its fields in declaration order with the **transient** ones left out
//! (`AimTransformsCache`, `bDirtyAimTransformsCache`: exactly the struct's two transient fields, by their flags):
//! - `ProfileName` (a name: a name-table index and a number, 4 bytes each);
//! - `HorizontalRange`, `VerticalRange` (2 floats each);
//! - `AimComponents`: a 4-byte count, then each component: `BoneName` (a name), then nine `AimTransform`s (`LU`, `LC`,
//!   `LD`, `CU`, `CC`, `CD`, `RU`, `RC`, `RD`), each a quaternion (4 floats) and a translation (3 floats): 260 bytes;
//! - `AnimName_LU` .. `AnimName_RD` (nine names).
//!
//! So a profile is 100 bytes plus 260 per component.
//!
//! **The structural oracle** (enforced; no value is clamped): every count fits the bytes left; every name's index is
//! inside the package's name table and its number is not negative; every float is finite; every quaternion is unit
//! length (its squared length within [`QUAT_TOLERANCE`] of 1); the read ends exactly at the value's end. On the
//! installed packages it holds for all 5 aim-offset nodes (4 class packages), each read ending at its tag's end.

use std::fmt;

use ow_package::package::Package;
use ow_package::props::{self, NoArrayTypes};

/// A profile's bytes besides its components.
pub const PROFILE_FIXED_BYTES: usize = 100;
/// One component's bytes.
pub const COMPONENT_BYTES: usize = 260;
/// How far a quaternion's squared length may be from 1 (the installed data stays below 2e-5).
pub const QUAT_TOLERANCE: f32 = 1e-4;
/// The size of an `ArrayProperty` tag's header before its value: name, type, size and array index.
const ARRAY_TAG_HEADER: usize = 24;

/// One aim transform: a unit quaternion `(x, y, z, w)` and a translation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AimTransform {
    pub quaternion: [f32; 4],
    pub translation: [f32; 3],
}

/// One aim component: its bone and nine transforms, `LU` .. `RD`.
#[derive(Debug, Clone, PartialEq)]
pub struct AimComponent {
    pub bone_name: String,
    pub transforms: [AimTransform; 9],
}

/// One `AimOffsetProfile`, its names as text (`Name_N` for a name with number N + 1).
#[derive(Debug, Clone, PartialEq)]
pub struct AimProfile {
    pub profile_name: String,
    pub horizontal_range: [f32; 2],
    pub vertical_range: [f32; 2],
    pub components: Vec<AimComponent>,
    pub anim_names: [String; 9],
}

/// A refused read: which oracle failed and where (a byte offset into the value).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfilesError {
    /// `need` more bytes were needed at `at`; the value has `len`.
    Short { at: usize, need: usize, len: usize },
    /// A count that is negative or does not fit the bytes left.
    Count { at: usize, count: i32 },
    /// A name whose index is outside the name table, or whose number is negative.
    Name { at: usize, index: i32, number: i32 },
    /// A float that is not finite.
    NotFinite { at: usize },
    /// A quaternion that is not unit length.
    NotUnit { at: usize },
    /// The read ended at `end`, before the value's `len`.
    Trailing { end: usize, len: usize },
    /// The export's tag stream could not be walked, or the `Profiles` tag is not an array.
    Tags(String),
}

impl fmt::Display for ProfilesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProfilesError::Short { at, need, len } => write!(f, "{need} bytes needed at {at} of {len}"),
            ProfilesError::Count { at, count } => write!(f, "the count {count} at {at} does not fit"),
            ProfilesError::Name { at, index, number } => write!(f, "the name ({index}, {number}) at {at} is not in the name table"),
            ProfilesError::NotFinite { at } => write!(f, "the float at {at} is not finite"),
            ProfilesError::NotUnit { at } => write!(f, "the quaternion at {at} is not unit length"),
            ProfilesError::Trailing { end, len } => write!(f, "the read ended at {end} of {len}"),
            ProfilesError::Tags(why) => write!(f, "the tag stream: {why}"),
        }
    }
}

impl std::error::Error for ProfilesError {}

struct Cursor<'a> {
    b: &'a [u8],
    at: usize,
    names: &'a [String],
}

impl Cursor<'_> {
    fn bytes(&mut self, n: usize) -> Result<&[u8], ProfilesError> {
        let end = self.at.checked_add(n).filter(|&e| e <= self.b.len());
        let Some(end) = end else {
            return Err(ProfilesError::Short { at: self.at, need: n, len: self.b.len() });
        };
        let s = &self.b[self.at..end];
        self.at = end;
        Ok(s)
    }

    fn i32(&mut self) -> Result<i32, ProfilesError> {
        let s = self.bytes(4)?;
        Ok(i32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }

    fn f32(&mut self) -> Result<f32, ProfilesError> {
        let at = self.at;
        let s = self.bytes(4)?;
        let v = f32::from_le_bytes([s[0], s[1], s[2], s[3]]);
        if v.is_finite() { Ok(v) } else { Err(ProfilesError::NotFinite { at }) }
    }

    fn floats<const N: usize>(&mut self) -> Result<[f32; N], ProfilesError> {
        let mut out = [0.0; N];
        for v in &mut out {
            *v = self.f32()?;
        }
        Ok(out)
    }

    fn name(&mut self) -> Result<String, ProfilesError> {
        let at = self.at;
        let (index, number) = (self.i32()?, self.i32()?);
        let base = usize::try_from(index).ok().and_then(|i| self.names.get(i));
        match base {
            Some(base) if number == 0 => Ok(base.clone()),
            Some(base) if number > 0 => Ok(format!("{base}_{}", number - 1)),
            _ => Err(ProfilesError::Name { at, index, number }),
        }
    }

    /// A count of elements of at least `each` bytes, checked against the bytes left.
    fn count(&mut self, each: usize) -> Result<usize, ProfilesError> {
        let at = self.at;
        let count = self.i32()?;
        let left = self.b.len() - self.at;
        match usize::try_from(count) {
            Ok(n) if n.checked_mul(each).is_some_and(|need| need <= left) => Ok(n),
            _ => Err(ProfilesError::Count { at, count }),
        }
    }

    fn transform(&mut self) -> Result<AimTransform, ProfilesError> {
        let at = self.at;
        let quaternion = self.floats::<4>()?;
        let square: f32 = quaternion.iter().map(|x| x * x).sum();
        if (square - 1.0).abs() > QUAT_TOLERANCE {
            return Err(ProfilesError::NotUnit { at });
        }
        Ok(AimTransform { quaternion, translation: self.floats::<3>()? })
    }
}

/// The profiles of a `Profiles` array value (the bytes after its tag header: the count and the elements), with
/// `names` the package's name table (module documentation). Reads only.
pub fn read(value: &[u8], names: &[String]) -> Result<Vec<AimProfile>, ProfilesError> {
    let mut c = Cursor { b: value, at: 0, names };
    let count = c.count(PROFILE_FIXED_BYTES)?;
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let profile_name = c.name()?;
        let horizontal_range = c.floats::<2>()?;
        let vertical_range = c.floats::<2>()?;
        let n = c.count(COMPONENT_BYTES)?;
        let mut components = Vec::with_capacity(n);
        for _ in 0..n {
            let bone_name = c.name()?;
            let mut transforms = [AimTransform { quaternion: [0.0; 4], translation: [0.0; 3] }; 9];
            for t in &mut transforms {
                *t = c.transform()?;
            }
            components.push(AimComponent { bone_name, transforms });
        }
        let mut anim_names: [String; 9] = Default::default();
        for name in &mut anim_names {
            *name = c.name()?;
        }
        out.push(AimProfile { profile_name, horizontal_range, vertical_range, components, anim_names });
    }
    if c.at != value.len() {
        return Err(ProfilesError::Trailing { end: c.at, len: value.len() });
    }
    Ok(out)
}

/// What [`read_export`] found.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportProfiles {
    /// The `Profiles` tag's profiles, or `None` when the export has no `Profiles` tag.
    pub profiles: Option<Vec<AimProfile>>,
    /// Where the export's tag stream ended, from its payload start (just past its `None` tag).
    pub tags_end: usize,
}

/// The profiles of export `export` of `pkg`, its tags read from `property_offset` bytes into its payload: the walk
/// finds the `Profiles` tag (an `ArrayProperty`), and [`read`] reads its value with the package's name table.
pub fn read_export(pkg: &Package, export: i32, property_offset: usize) -> Result<ExportProfiles, ProfilesError> {
    let walked = props::read_tagged(pkg, export, property_offset, &NoArrayTypes).map_err(|e| ProfilesError::Tags(e.to_string()))?;
    let entry = walked.entries.iter().find(|e| e.name.eq_ignore_ascii_case("Profiles") && e.index == 0);
    let Some(entry) = entry else {
        return Ok(ExportProfiles { profiles: None, tags_end: walked.end });
    };
    let tag = entry.tag.as_ref().ok_or_else(|| ProfilesError::Tags("Profiles has no tag".to_string()))?;
    if tag.ty != "ArrayProperty" {
        return Err(ProfilesError::Tags(format!("Profiles is a {} tag, not an ArrayProperty", tag.ty)));
    }
    let base = pkg.object(export).map_err(|e| ProfilesError::Tags(e.to_string()))?.offset as usize;
    let size = usize::try_from(tag.size).map_err(|_| ProfilesError::Tags(format!("Profiles' size {}", tag.size)))?;
    let start = base + tag.offset + ARRAY_TAG_HEADER;
    let value = pkg
        .data
        .get(start..start + size)
        .ok_or(ProfilesError::Short { at: start, need: size, len: pkg.data.len() })?;
    Ok(ExportProfiles { profiles: Some(read(value, &pkg.names)?), tags_end: walked.end })
}

#[cfg(test)]
#[path = "aim_profiles_tests.rs"]
mod tests;
