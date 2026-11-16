//! Owner: lane H6-ANIM (host track, stage 1). The ActorX chunk file shared by `.psk` (skeletal mesh) and `.psa`
//! (animation set) exports, as UModel writes them, and the one conversion from the export's frame to the game's.
//!
//! AI-assisted (Claude Code). Decision: `docs/decisions/2026-11/2026-11-16-53-*`.
//!
//! **Source of the format.** ActorX is an interchange format, not the 832/46 package layout, so nothing here is a
//! package offset (AGENTS.md rule 4). The chunk and record shapes were learned by reading UModel's exporter and its
//! format header (`Exporters/ExportPsk.cpp`, `Exporters/Psk.h`, UE Viewer commit `a0bfb46`, MIT, listed in
//! `THIRD_PARTY.md`); no code was copied or translated. Every record size is checked against the chunk header's own
//! `DataSize`, and every count against the bytes left, so a file that does not meet these shapes is refused.
//!
//! **A chunk** is a 32-byte header (a 20-byte NUL-padded name, a `u32` type flag, an `i32` record size and an `i32`
//! record count) followed by `size * count` bytes. All numbers are little-endian.
//!
//! **The export's frame.** UModel writes ActorX in a mirrored frame (its exporter negates Y so that an editor's
//! import, which mirrors, gets the original back): a position's Y is negated, and a quaternion `(x, y, z, w)` is
//! written as `(x, -y, z, -w)`. [`unmirror_position`] and [`unmirror_quat`] undo it, so everything this crate
//! returns is in the game's frame: uu, X forward, Y right, Z up, left-handed, as the packages store it. A triangle's
//! first two corners are swapped by the same mirror; `skeletal` swaps them back. That the result is the game's frame
//! is checked on the installed meshes by the handedness check of `ow-host`'s `anim::report` (the `R_` bones land on
//! the right), not in the running game.

use std::fmt;

use ow_core::NotYet;

/// One record of a chunk: its name, type flag, record size, record count and the record bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chunk<'a> {
    pub name: &'a str,
    pub type_flag: u32,
    pub record_size: usize,
    pub count: usize,
    pub body: &'a [u8],
}

impl<'a> Chunk<'a> {
    /// The bytes of record `i` (`i < count`).
    pub fn record(&self, i: usize) -> &'a [u8] {
        &self.body[i * self.record_size..(i + 1) * self.record_size]
    }

    /// Refuses the chunk unless its record size is `size`.
    pub fn expect_size(&self, size: usize) -> Result<(), ActorXError> {
        if self.record_size == size {
            Ok(())
        } else {
            Err(ActorXError::RecordSize {
                chunk: self.name.to_string(),
                expected: size,
                found: self.record_size,
            })
        }
    }
}

/// Why an ActorX export was refused.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum ActorXError {
    /// The file could not be read.
    Io { path: String, message: String },
    /// A chunk header or body runs past the end of the file.
    Truncated { at: usize },
    /// A chunk header's size or count is negative, or their product overflows.
    BadHeader { at: usize },
    /// A chunk's name is not NUL-padded ASCII.
    BadChunkName { at: usize },
    /// The file does not start with the chunk this kind of file starts with.
    WrongFile {
        expected: &'static str,
        found: String,
    },
    /// A required chunk is missing.
    MissingChunk(&'static str),
    /// A chunk appears twice.
    DuplicateChunk(String),
    /// A chunk's record size is not the one its format has.
    RecordSize {
        chunk: String,
        expected: usize,
        found: usize,
    },
    /// A name field is not NUL-terminated ASCII.
    BadName { chunk: &'static str, index: usize },
    /// A number is NaN or infinite.
    NonFinite { chunk: &'static str, index: usize },
    /// A quaternion is not of unit length (its squared length is outside 0.99..=1.01).
    NotUnit { chunk: &'static str, index: usize },
    /// An index points outside its table.
    IndexOutOfRange {
        chunk: &'static str,
        index: usize,
        value: i64,
        limit: usize,
    },
    /// The bones are not in parent order (each parent before its child), or there is not exactly one root.
    BoneOrder { index: usize, parent: i64 },
    /// Two bones share a name.
    DuplicateBone(String),
    /// A point has more than four influences.
    TooManyInfluences { point: usize },
    /// A point has no influence.
    Unweighted { point: usize },
    /// An influence's weight is outside 0..=1.
    BadWeight { index: usize },
    /// A point's weights do not sum to 1 within 1e-3.
    WeightSum { point: usize },
    /// The animation info and the key table disagree.
    KeyCount { expected: usize, found: usize },
    /// An animation's first frame is not the sum of the frames before it.
    FirstFrame {
        sequence: usize,
        expected: usize,
        found: i64,
    },
    /// An animation's frame count or rate is not positive, or its bone count differs from the bone table.
    BadSequence { sequence: usize },
    /// A track names a bone the mesh does not have.
    UnknownTrackBone(String),
    /// Something this reader does not support yet.
    NotYet(NotYet),
}

impl fmt::Display for ActorXError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ActorXError::Io { path, message } => write!(f, "cannot read {path}: {message}"),
            ActorXError::Truncated { at } => {
                write!(f, "chunk at byte {at} runs past the end of the file")
            }
            ActorXError::BadHeader { at } => {
                write!(f, "chunk at byte {at} has a bad size or count")
            }
            ActorXError::BadChunkName { at } => {
                write!(f, "chunk at byte {at} has a name that is not ASCII")
            }
            ActorXError::WrongFile { expected, found } => {
                write!(f, "expected a file starting with {expected}, found {found}")
            }
            ActorXError::MissingChunk(name) => write!(f, "missing chunk {name}"),
            ActorXError::DuplicateChunk(name) => write!(f, "chunk {name} appears twice"),
            ActorXError::RecordSize {
                chunk,
                expected,
                found,
            } => {
                write!(
                    f,
                    "chunk {chunk} has records of {found} bytes, expected {expected}"
                )
            }
            ActorXError::BadName { chunk, index } => {
                write!(f, "{chunk} record {index} has a bad name")
            }
            ActorXError::NonFinite { chunk, index } => {
                write!(f, "{chunk} record {index} is not finite")
            }
            ActorXError::NotUnit { chunk, index } => {
                write!(
                    f,
                    "{chunk} record {index} has a quaternion that is not of unit length"
                )
            }
            ActorXError::IndexOutOfRange {
                chunk,
                index,
                value,
                limit,
            } => {
                write!(
                    f,
                    "{chunk} record {index} points at {value}, outside 0..{limit}"
                )
            }
            ActorXError::BoneOrder { index, parent } => {
                write!(
                    f,
                    "bone {index} has parent {parent}: bones are not in parent order with one root"
                )
            }
            ActorXError::DuplicateBone(name) => write!(f, "two bones are named {name}"),
            ActorXError::TooManyInfluences { point } => {
                write!(f, "point {point} has more than 4 influences")
            }
            ActorXError::Unweighted { point } => write!(f, "point {point} has no influence"),
            ActorXError::BadWeight { index } => {
                write!(f, "influence {index} has a weight outside 0..=1")
            }
            ActorXError::WeightSum { point } => {
                write!(f, "the weights of point {point} do not sum to 1")
            }
            ActorXError::KeyCount { expected, found } => {
                write!(
                    f,
                    "the animations need {expected} keys, the key table has {found}"
                )
            }
            ActorXError::FirstFrame {
                sequence,
                expected,
                found,
            } => {
                write!(
                    f,
                    "animation {sequence} starts at frame {found}, expected {expected}"
                )
            }
            ActorXError::BadSequence { sequence } => write!(
                f,
                "animation {sequence} has a bad frame count, rate or bone count"
            ),
            ActorXError::UnknownTrackBone(name) => {
                write!(f, "track bone {name} is not a bone of the mesh")
            }
            ActorXError::NotYet(n) => write!(f, "{n}"),
        }
    }
}

impl std::error::Error for ActorXError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ActorXError::NotYet(n) => Some(n),
            _ => None,
        }
    }
}

impl From<NotYet> for ActorXError {
    fn from(n: NotYet) -> Self {
        ActorXError::NotYet(n)
    }
}

/// Splits `data` into its chunks, in file order. Refuses a header or body that runs past the end, a negative or
/// overflowing size, and a name that is not NUL-padded ASCII.
pub fn chunks(data: &[u8]) -> Result<Vec<Chunk<'_>>, ActorXError> {
    let mut out = Vec::new();
    let mut at = 0usize;
    while at < data.len() {
        let header_end = at.checked_add(32).ok_or(ActorXError::Truncated { at })?;
        let header = data
            .get(at..header_end)
            .ok_or(ActorXError::Truncated { at })?;
        let name = fixed_name(&header[..20]).ok_or(ActorXError::BadChunkName { at })?;
        let type_flag = u32_at(header, 20);
        let size =
            usize::try_from(i32_at(header, 24)).map_err(|_| ActorXError::BadHeader { at })?;
        let count =
            usize::try_from(i32_at(header, 28)).map_err(|_| ActorXError::BadHeader { at })?;
        let bytes = size
            .checked_mul(count)
            .ok_or(ActorXError::BadHeader { at })?;
        let body_end = header_end
            .checked_add(bytes)
            .ok_or(ActorXError::BadHeader { at })?;
        let body = data
            .get(header_end..body_end)
            .ok_or(ActorXError::Truncated { at })?;
        out.push(Chunk {
            name,
            type_flag,
            record_size: size,
            count,
            body,
        });
        at = body_end;
    }
    Ok(out)
}

/// Finds the one chunk called `name`: `Ok(None)` when absent, an error when it appears twice.
pub fn find<'a>(chunks: &[Chunk<'a>], name: &str) -> Result<Option<Chunk<'a>>, ActorXError> {
    let mut found = None;
    for c in chunks {
        if c.name == name {
            if found.is_some() {
                return Err(ActorXError::DuplicateChunk(name.to_string()));
            }
            found = Some(*c);
        }
    }
    Ok(found)
}

/// Like [`find`], but the chunk must be there.
pub fn require<'a>(chunks: &[Chunk<'a>], name: &'static str) -> Result<Chunk<'a>, ActorXError> {
    find(chunks, name)?.ok_or(ActorXError::MissingChunk(name))
}

/// A NUL-padded ASCII name field: the bytes before the first NUL, which must be printable ASCII. `None` when there
/// is no NUL or a byte is not printable ASCII.
pub fn fixed_name(field: &[u8]) -> Option<&str> {
    let end = field.iter().position(|&b| b == 0)?;
    let name = &field[..end];
    if name.iter().all(|&b| (0x20..0x7f).contains(&b)) {
        std::str::from_utf8(name).ok()
    } else {
        None
    }
}

/// Little-endian readers over a record already checked to be long enough.
pub fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

pub fn i32_at(b: &[u8], at: usize) -> i32 {
    i32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

pub fn u16_at(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

pub fn f32_at(b: &[u8], at: usize) -> f32 {
    f32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

/// Three floats from `at`.
pub fn vec3_at(b: &[u8], at: usize) -> [f32; 3] {
    [f32_at(b, at), f32_at(b, at + 4), f32_at(b, at + 8)]
}

/// Four floats from `at`, as `(x, y, z, w)`.
pub fn quat_at(b: &[u8], at: usize) -> [f32; 4] {
    [
        f32_at(b, at),
        f32_at(b, at + 4),
        f32_at(b, at + 8),
        f32_at(b, at + 12),
    ]
}

/// A position from the export's mirrored frame to the game's: Y negated.
pub fn unmirror_position(p: [f32; 3]) -> [f32; 3] {
    [p[0], -p[1], p[2]]
}

/// A quaternion from the export's mirrored frame to the game's: `(x, y, z, w)` was written as `(x, -y, z, -w)`.
pub fn unmirror_quat(q: [f32; 4]) -> [f32; 4] {
    [q[0], -q[1], q[2], -q[3]]
}

/// True when every value is finite.
pub fn all_finite(values: &[f32]) -> bool {
    values.iter().all(|v| v.is_finite())
}

/// True when `q`'s squared length is within 0.99..=1.01: the stored quaternions are unit up to float rounding, and
/// one far from unit length is a broken record, not something to normalise away.
pub fn near_unit(q: [f32; 4]) -> bool {
    let n = f64::from(q[0]) * f64::from(q[0])
        + f64::from(q[1]) * f64::from(q[1])
        + f64::from(q[2]) * f64::from(q[2])
        + f64::from(q[3]) * f64::from(q[3]);
    (0.99..=1.01).contains(&n)
}

/// Reads a whole file, with the path in the error.
pub fn read_file(path: &std::path::Path) -> Result<Vec<u8>, ActorXError> {
    std::fs::read(path).map_err(|e| ActorXError::Io {
        path: path.display().to_string(),
        message: e.to_string(),
    })
}

/// Test helpers that build ActorX files from invented values (AGENTS.md rule 9).
#[cfg(test)]
pub(crate) mod build {
    /// One chunk: header and records.
    pub fn chunk(name: &str, type_flag: u32, record_size: usize, records: &[Vec<u8>]) -> Vec<u8> {
        let mut out = vec![0u8; 20];
        out[..name.len()].copy_from_slice(name.as_bytes());
        out.extend_from_slice(&type_flag.to_le_bytes());
        out.extend_from_slice(&i32::try_from(record_size).unwrap().to_le_bytes());
        out.extend_from_slice(&i32::try_from(records.len()).unwrap().to_le_bytes());
        for r in records {
            assert_eq!(r.len(), record_size, "{name}");
            out.extend_from_slice(r);
        }
        out
    }

    /// A NUL-padded name field of `len` bytes.
    pub fn name(text: &str, len: usize) -> Vec<u8> {
        let mut out = vec![0u8; len];
        out[..text.len()].copy_from_slice(text.as_bytes());
        out
    }

    pub fn floats(values: &[f32]) -> Vec<u8> {
        values.iter().flat_map(|v| v.to_le_bytes()).collect()
    }

    pub fn ints(values: &[i32]) -> Vec<u8> {
        values.iter().flat_map(|v| v.to_le_bytes()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::build::*;
    use super::*;

    #[test]
    fn splits_chunks_and_refuses_truncation() {
        let mut file = chunk("ACTRHEAD", 7, 0, &[]);
        file.extend(chunk("PNTS0000", 0, 12, &[floats(&[1.0, 2.0, 3.0])]));
        let c = chunks(&file).unwrap();
        assert_eq!(c.len(), 2);
        assert_eq!((c[0].name, c[0].type_flag, c[0].count), ("ACTRHEAD", 7, 0));
        assert_eq!(vec3_at(c[1].record(0), 0), [1.0, 2.0, 3.0]);
        assert_eq!(
            chunks(&file[..file.len() - 1]),
            Err(ActorXError::Truncated { at: 32 })
        );
        assert_eq!(chunks(&file[..10]), Err(ActorXError::Truncated { at: 0 }));
    }

    #[test]
    fn refuses_negative_sizes_bad_names_and_duplicates() {
        let mut file = chunk("PNTS0000", 0, 12, &[]);
        file[24..28].copy_from_slice(&(-1i32).to_le_bytes());
        assert_eq!(chunks(&file), Err(ActorXError::BadHeader { at: 0 }));
        let mut file = chunk("PNTS0000", 0, 0, &[]);
        file[3] = 0xff;
        assert_eq!(chunks(&file), Err(ActorXError::BadChunkName { at: 0 }));
        let mut file = chunk("A", 0, 0, &[]);
        file.extend(chunk("A", 0, 0, &[]));
        let c = chunks(&file).unwrap();
        assert_eq!(find(&c, "A"), Err(ActorXError::DuplicateChunk("A".into())));
        assert_eq!(require(&c, "B"), Err(ActorXError::MissingChunk("B")));
    }

    #[test]
    fn unmirror_is_its_own_inverse_and_negates_y() {
        assert_eq!(unmirror_position([1.0, 2.0, 3.0]), [1.0, -2.0, 3.0]);
        let q = [0.1, 0.2, 0.3, 0.9];
        assert_eq!(unmirror_quat(q), [0.1, -0.2, 0.3, -0.9]);
        assert_eq!(unmirror_quat(unmirror_quat(q)), q);
        assert!(near_unit([0.0, 0.0, 0.0, 1.0]));
        assert!(!near_unit([0.0, 0.0, 0.0, 0.9]));
        assert_eq!(fixed_name(b"Bone\0junk"), Some("Bone"));
        assert_eq!(fixed_name(b"NoTerminator"), None);
    }
}
