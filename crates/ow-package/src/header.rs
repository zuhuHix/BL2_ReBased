//! Owner: lane W1-PKG. The start of the package summary that the reader needs: tag, version 832 / licensee 46, the
//! header size, the folder name, and the counts and offsets of the name, export and import tables. The field order
//! and checks are the C++ `Package::load` (`src/package.cpp`), which recovered them; nothing here is re-derived.
//!
//! The summary fields after the import table (dependencies, GUID, generations, compression) are not read here:
//! `ow-container` reads the compression table it needs (`src/container.cpp`).

use std::fmt;

use crate::reader::{ReadError, Reader};

/// The UE3 package tag.
pub const PACKAGE_TAG: u32 = 0x9e2a83c1;
/// The file version BL2's packages carry.
pub const VERSION: u16 = 832;
/// The licensee version BL2's packages carry.
pub const LICENSEE: u16 = 46;

/// Why a summary could not be read. `Display` gives the C++ exception text.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderError {
    Read(ReadError),
    /// "invalid package magic"
    BadTag,
    /// "requires decompressed BL2 version 832/46"
    BadVersion,
    /// "invalid header size": negative or past the data's end.
    BadHeaderSize,
}

impl fmt::Display for HeaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HeaderError::Read(e) => e.fmt(f),
            HeaderError::BadTag => f.write_str("invalid package magic"),
            HeaderError::BadVersion => f.write_str("requires decompressed BL2 version 832/46"),
            HeaderError::BadHeaderSize => f.write_str("invalid header size"),
        }
    }
}

impl std::error::Error for HeaderError {}

impl From<ReadError> for HeaderError {
    fn from(e: ReadError) -> Self {
        HeaderError::Read(e)
    }
}

/// The summary fields the reader uses. Counts and offsets are kept as the signed values the file holds; the table
/// reads check them (`Reader::table`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Summary {
    pub version: u16,
    pub licensee: u16,
    pub header_size: i32,
    pub folder: String,
    pub flags: u32,
    pub name_count: i32,
    pub name_offset: i32,
    pub export_count: i32,
    pub export_offset: i32,
    pub import_count: i32,
    pub import_offset: i32,
}

/// Reads the summary at the start of decoded package bytes (`data`), as the C++ `Package::load` does before the
/// tables: tag, version word (832 | 46 << 16), header size (0..=data length), folder FString, package flags, then
/// name count/offset, export count/offset, import count/offset.
pub fn read_summary(data: &[u8]) -> Result<Summary, HeaderError> {
    let mut r = Reader::new(data);
    if r.u32()? != PACKAGE_TAG {
        return Err(HeaderError::BadTag);
    }
    let version_word = r.u32()?;
    if version_word != (u32::from(VERSION) | (u32::from(LICENSEE) << 16)) {
        return Err(HeaderError::BadVersion);
    }
    let header_size = r.i32()?;
    if header_size < 0 || header_size as usize > data.len() {
        return Err(HeaderError::BadHeaderSize);
    }
    let folder = r.string()?;
    let flags = r.u32()?;
    let name_count = r.i32()?;
    let name_offset = r.i32()?;
    let export_count = r.i32()?;
    let export_offset = r.i32()?;
    let import_count = r.i32()?;
    let import_offset = r.i32()?;
    Ok(Summary {
        version: VERSION,
        licensee: LICENSEE,
        header_size,
        folder,
        flags,
        name_count,
        name_offset,
        export_count,
        export_offset,
        import_count,
        import_offset,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn le(values: &[u32]) -> Vec<u8> {
        values.iter().flat_map(|v| v.to_le_bytes()).collect()
    }

    /// The header of `tests/package_test.py` (invented values).
    fn header() -> Vec<u8> {
        le(&[PACKAGE_TAG, 832 | (46 << 16), 48, 0, 0, 1, 48, 1, 93, 1, 65, 0])
    }

    #[test]
    fn reads_the_fields_in_the_cxx_order() {
        let s = read_summary(&header()).unwrap();
        assert_eq!((s.version, s.licensee, s.header_size), (832, 46, 48));
        assert_eq!(s.folder, "");
        assert_eq!((s.name_count, s.name_offset), (1, 48));
        assert_eq!((s.export_count, s.export_offset), (1, 93));
        assert_eq!((s.import_count, s.import_offset), (1, 65));
    }

    #[test]
    fn refuses_bad_tag_version_and_header_size() {
        let mut h = header();
        h[0] = 0;
        assert_eq!(read_summary(&h), Err(HeaderError::BadTag));
        let mut h = header();
        h[4..8].copy_from_slice(&832u32.to_le_bytes());
        assert_eq!(read_summary(&h), Err(HeaderError::BadVersion));
        let mut h = header();
        h[8..12].copy_from_slice(&49u32.to_le_bytes());
        assert_eq!(read_summary(&h), Err(HeaderError::BadHeaderSize));
        let mut h = header();
        h[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(read_summary(&h), Err(HeaderError::BadHeaderSize));
        for end in 0..header().len() {
            assert!(read_summary(&header()[..end]).is_err(), "{end}");
        }
    }
}
