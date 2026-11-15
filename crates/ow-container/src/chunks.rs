//! Owner: lane W1-LZO.
//! The two container forms of BL2's packages, ported from `src/container.cpp` with the same checks in the same order:
//!
//! - **Fully compressed** ([`decode_container`]): tag, block size, compressed size, decoded size, then one
//!   (compressed, decoded) size pair per block, then the blocks, each one LZO1X stream.
//! - **Partially compressed** ([`decode_partial`]): an uncompressed 832/46 package summary whose flags carry the
//!   compressed bit and whose compression table (codec 2) lists chunks; each chunk is a fully compressed container
//!   stored elsewhere in the file. The decoded package is the bytes before the first chunk's destination followed by
//!   the decoded chunks, which must be contiguous.
//!
//! The offsets of the summary walk are the C++ reader's (`container.cpp:28-34`), not re-derived here. The limits
//! (512 MiB decoded, block size 4 KiB to 1 MiB, a power of two) are the C++ code's research limits, kept unchanged:
//! "raise only with corpus evidence".

use crate::{ContainerError, lzo};

/// The UE3 package tag, the first word of a package and of a container.
pub const PACKAGE_TAG: u32 = 0x9E2A_83C1;
/// File version 832, licensee 46: the word after the tag in a BL2 package (a container has its block size there).
pub const BL2_VERSION_WORD: u32 = 832 | (46 << 16);
/// The summary flag that marks a partially compressed package.
const COMPRESSED_FLAG: u32 = 0x0200_0000;
/// Compression codec 2 (LZO) in the partial table; the only codec read.
const CODEC_LZO: u32 = 2;
/// The C++ reader's research limit on a decoded size (`container.cpp`).
const DECODED_LIMIT: usize = 512 * 1024 * 1024;
/// The smallest destination of a package's first compressed chunk: the C++ check (`dest < 48`).
const FIRST_CHUNK_MIN_DEST: usize = 48;

/// Reads the little-endian word at `offset`, refusing a read past the end.
pub(crate) fn word(data: &[u8], offset: usize) -> Result<u32, ContainerError> {
    match offset.checked_add(4).and_then(|end| data.get(offset..end)) {
        Some(b) => Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]])),
        None => Err(ContainerError::Truncated {
            at: offset.min(data.len()),
            need: offset.saturating_add(4) - data.len(),
        }),
    }
}

fn header(reason: &'static str) -> ContainerError {
    ContainerError::BadHeader { reason }
}

/// Decodes a fully compressed container (the tag is already checked by the caller).
pub fn decode_container(input: &[u8]) -> Result<Vec<u8>, ContainerError> {
    let block = word(input, 4)? as usize;
    if !(4096..=1024 * 1024).contains(&block) || !block.is_power_of_two() {
        return Err(header(
            "unsupported package version or container block size",
        ));
    }
    let compressed = word(input, 8)? as usize;
    let decoded = word(input, 12)? as usize;
    // Bound the allocation before trusting the file's counts.
    if decoded == 0 || decoded > DECODED_LIMIT {
        return Err(header(
            "decoded size is zero or over the 512 MiB research limit",
        ));
    }
    let blocks = decoded.div_ceil(block);
    let start = 16 + blocks * 8;
    if start > input.len() {
        return Err(ContainerError::Truncated {
            at: input.len(),
            need: start - input.len(),
        });
    }
    if compressed != input.len() - start {
        return Err(header(
            "compressed size differs from the bytes after the block table",
        ));
    }

    // Check every block's sizes before decoding any of them.
    let mut sizes = Vec::with_capacity(blocks);
    let (mut total_compressed, mut total_decoded) = (0usize, 0usize);
    for i in 0..blocks {
        let c = word(input, 16 + i * 8)? as usize;
        let u = word(input, 20 + i * 8)? as usize;
        let expected = if i + 1 == blocks {
            decoded - total_decoded
        } else {
            block
        };
        if c == 0 || c > compressed - total_compressed {
            return Err(ContainerError::BadChunk {
                index: i,
                reason: "compressed size is zero or past the data",
            });
        }
        if u == 0 || u != expected {
            return Err(ContainerError::BadChunk {
                index: i,
                reason: "decoded size is not the block size (or the remainder for the last block)",
            });
        }
        total_compressed += c;
        total_decoded += u;
        sizes.push((c, u));
    }
    if total_compressed != compressed || total_decoded != decoded {
        return Err(ContainerError::BadChunk {
            index: blocks - 1,
            reason: "block sizes do not add up to the container's sizes",
        });
    }

    let mut output = vec![0u8; decoded];
    let (mut in_at, mut out_at) = (start, 0usize);
    for (c, u) in sizes {
        lzo::decompress_into(&input[in_at..in_at + c], &mut output[out_at..out_at + u]).map_err(
            |e| ContainerError::Lzo {
                at: in_at + e.at,
                reason: e.reason,
            },
        )?;
        in_at += c;
        out_at += u;
    }
    Ok(output)
}

/// Decodes a BL2 package (tag and 832/46 checked by the caller). A package without the compressed flag is returned
/// unchanged; one with it is read through its partial compression table.
pub fn decode_partial(input: &[u8]) -> Result<Vec<u8>, ContainerError> {
    // Summary: tag, version, total header size, then the folder name (an FString: a negative length counts UTF-16
    // units), then the package flags.
    let length = i64::from(word(input, 12)? as i32);
    let string_bytes = if length < 0 {
        length.unsigned_abs() * 2
    } else {
        length.unsigned_abs()
    };
    let need_end = 16u64 + string_bytes;
    if need_end > input.len() as u64 {
        return Err(ContainerError::Truncated {
            at: input.len(),
            need: usize::try_from(need_end - input.len() as u64).unwrap_or(usize::MAX),
        });
    }
    let flags_at = need_end as usize;
    if word(input, flags_at)? & COMPRESSED_FLAG == 0 {
        return Ok(input.to_vec());
    }

    // BL2 832/46 summary after the flags: the name, export and import tables, the dependency and GUID-table fields
    // (24 + 20 bytes), the GUID (16), then the generations (three words each), engine and cooker versions (8), and
    // the compression codec and chunk count. Offsets from container.cpp:28-34.
    let mut cursor = flags_at + 4 + 24 + 20 + 16;
    let generations = word(input, cursor)? as usize;
    cursor += 4;
    if generations > input.len() / 12 {
        return Err(ContainerError::BadPartialTable {
            reason: "generation count larger than the file allows",
        });
    }
    cursor += generations * 12 + 8;
    let codec = word(input, cursor)?;
    let count = word(input, cursor + 4)? as usize;
    cursor += 8;
    if codec != CODEC_LZO || count == 0 || count > (input.len() - cursor.min(input.len())) / 16 {
        return Err(ContainerError::BadPartialTable {
            reason: "unsupported codec or invalid chunk count",
        });
    }

    let (mut previous_end, mut previous_disk_end) = (0usize, cursor + count * 16);
    let mut output = Vec::new();
    for i in 0..count {
        let at = cursor + i * 16;
        let dest = word(input, at)? as usize;
        let size = word(input, at + 4)? as usize;
        let source = word(input, at + 8)? as usize;
        let stored = word(input, at + 12)? as usize;
        if size == 0
            || dest > DECODED_LIMIT
            || size > DECODED_LIMIT - dest
            || source < previous_disk_end
            || source > input.len()
            || stored > input.len() - source
            || (i > 0 && dest != previous_end)
            || (i == 0 && (dest < FIRST_CHUNK_MIN_DEST || dest > source))
        {
            return Err(ContainerError::BadPartialTable {
                reason: "invalid chunk bounds",
            });
        }
        if i == 0 {
            output.extend_from_slice(&input[..dest]);
        }
        let chunk = &input[source..source + stored];
        // Errors inside the chunk are in its own coordinates; name the table entry and the chunk's file offset.
        let in_chunk = |error| ContainerError::InPartialChunk {
            index: i,
            at: source,
            error: Box::new(error),
        };
        if word(chunk, 4).map_err(in_chunk)? == BL2_VERSION_WORD {
            return Err(ContainerError::BadPartialTable {
                reason: "chunk is a package, not a container",
            });
        }
        let decoded = crate::decode_package(chunk).map_err(in_chunk)?;
        if decoded.len() != size {
            return Err(ContainerError::BadPartialTable {
                reason: "chunk decoded size differs from the table",
            });
        }
        output.extend_from_slice(&decoded);
        previous_end = dest + size;
        previous_disk_end = source + stored;
    }
    if previous_disk_end != input.len() {
        return Err(ContainerError::TrailingPartialBytes);
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    //! Re-expressed from `tests/package_test.py` (its `container()` helper, the compressed cases and the partial case
    //! of lines 64-79). Every byte is invented (AGENTS.md rule 9); the package is the test's 125-byte synthetic one.
    use super::*;
    use crate::decode_package;

    fn pack(values: &[u32]) -> Vec<u8> {
        values.iter().flat_map(|v| v.to_le_bytes()).collect()
    }

    fn put(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn name_table() -> Vec<u8> {
        [pack(&[5]), b"None\0".to_vec(), vec![0; 8]].concat()
    }
    fn imp() -> Vec<u8> {
        pack(&[0; 7])
    }
    fn exp() -> Vec<u8> {
        pack(&[
            0xFFFF_FFFF,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            161,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ])
    }

    /// package_test.py's `valid`: an uncompressed 832/46 package with one name, import and export.
    fn valid() -> Vec<u8> {
        let header = pack(&[
            PACKAGE_TAG,
            BL2_VERSION_WORD,
            48,
            0,
            0,
            1,
            48,
            1,
            93,
            1,
            65,
            0,
        ]);
        [header, name_table(), imp(), exp()].concat()
    }

    /// package_test.py's `container()`: literal-only LZO1X blocks with end markers.
    fn container(data: &[u8], block: usize) -> Vec<u8> {
        let mut streams = Vec::new();
        let mut sizes = Vec::new();
        for chunk in data.chunks(block) {
            assert!(chunk.len() >= 4);
            let mut s = if chunk.len() <= 238 {
                vec![17 + chunk.len() as u8]
            } else {
                let rest = chunk.len() - 19;
                let mut p = vec![0u8; 1 + rest / 255];
                p.push((rest % 255) as u8 + 1);
                p
            };
            s.extend_from_slice(chunk);
            s.extend_from_slice(&[0x11, 0, 0]);
            sizes.extend([s.len() as u32, chunk.len() as u32]);
            streams.extend(s);
        }
        let mut out = pack(&[
            PACKAGE_TAG,
            block as u32,
            streams.len() as u32,
            data.len() as u32,
        ]);
        out.extend(pack(&sizes));
        out.extend(streams);
        out
    }

    /// package_test.py lines 64-74: a 112-byte summary, one table entry, 16 unused bytes, then the chunk.
    fn partial() -> (Vec<u8>, Vec<u8>) {
        let mut header = pack(&[
            PACKAGE_TAG,
            BL2_VERSION_WORD,
            128,
            0,
            COMPRESSED_FLAG,
            1,
            128,
            1,
            173,
            1,
            145,
        ]);
        header.extend(vec![0u8; 20 + 16]);
        header.extend(pack(&[1, 1, 1, 0, 0, 0, 2, 1]));
        assert_eq!(header.len(), 112);
        let mut payload = [name_table(), imp(), exp()].concat();
        put(&mut payload, 45 + 36, 241);
        let stream = container(&payload, 131072);
        header.extend(pack(&[128, payload.len() as u32, 144, stream.len() as u32]));
        header.extend(vec![0u8; 16]);
        let mut want = header[..128].to_vec();
        want.extend_from_slice(&payload);
        header.extend(stream);
        (header, want)
    }

    #[test]
    fn uncompressed_package_is_returned_unchanged() {
        assert_eq!(decode_package(&valid()).unwrap(), valid());
    }

    #[test]
    fn bad_tag_is_refused() {
        let mut bad = valid();
        put(&mut bad, 0, 0);
        assert_eq!(decode_package(&bad), Err(ContainerError::BadMagic));
        assert!(matches!(
            decode_package(&[0xC1, 0x83]),
            Err(ContainerError::Truncated { at: 0, need: 2 })
        ));
    }

    #[test]
    fn full_container_decodes() {
        let fixture = container(&valid(), 131072);
        assert_eq!(decode_package(&fixture).unwrap(), valid());
    }

    #[test]
    fn every_truncation_is_refused() {
        let fixture = container(&valid(), 131072);
        for end in 0..fixture.len() {
            assert!(decode_package(&fixture[..end]).is_err(), "end {end}");
        }
    }

    #[test]
    fn padded_multi_block_containers_decode() {
        let v = valid();
        for padding in [4096 - v.len(), 8192 - v.len(), 9000] {
            let data = [v.clone(), vec![0u8; padding]].concat();
            assert_eq!(
                decode_package(&container(&data, 4096)).unwrap(),
                data,
                "padding {padding}"
            );
        }
    }

    #[test]
    fn corrupt_container_fields_are_refused() {
        let fixture = container(&valid(), 131072);
        for (offset, value) in [(4, 0), (8, 1), (12, 0), (12, 0xFFFF_FFFF), (16, 0), (20, 1)] {
            let mut bad = fixture.clone();
            put(&mut bad, offset, value);
            assert!(decode_package(&bad).is_err(), "offset {offset}");
        }
        assert!(decode_package(&[fixture.clone(), b"garbage".to_vec()].concat()).is_err());
        let mut no_marker = fixture.clone();
        let n = no_marker.len();
        no_marker[n - 3..].fill(0);
        assert!(matches!(
            decode_package(&no_marker),
            Err(ContainerError::Lzo { .. })
        ));
    }

    #[test]
    fn block_size_must_be_a_power_of_two_in_range() {
        let v = valid();
        for block in [2048u32, 4097, 2 * 1024 * 1024] {
            let mut bad = container(&v, 131072);
            put(&mut bad, 4, block);
            assert!(
                matches!(decode_package(&bad), Err(ContainerError::BadHeader { .. })),
                "{block}"
            );
        }
    }

    #[test]
    fn bad_lzo_streams_are_refused() {
        let v = valid();
        let oversized = [vec![255u8], v.clone(), vec![0x11, 0, 0]].concat();
        for stream in [vec![0x11, 0x04, 0x00], oversized] {
            let mut input = pack(&[PACKAGE_TAG, 131072, stream.len() as u32, v.len() as u32]);
            input.extend(pack(&[stream.len() as u32, v.len() as u32]));
            input.extend(&stream);
            assert!(matches!(
                decode_package(&input),
                Err(ContainerError::Lzo { .. })
            ));
        }
        // The error offset is in file coordinates: the bad match byte sits after the 24-byte header and table.
        let stream = [0x11u8, 0x04, 0x00];
        let mut input = pack(&[PACKAGE_TAG, 131072, 3, 4, 3, 4]);
        input.extend(stream);
        assert_eq!(
            decode_package(&input),
            Err(ContainerError::Lzo {
                at: 24 + 3,
                reason: "match distance outside decoded output"
            })
        );
    }

    #[test]
    fn partial_package_decodes() {
        let (partial, want) = partial();
        assert_eq!(decode_package(&partial).unwrap(), want);
    }

    #[test]
    fn bad_partial_tables_are_refused() {
        let (partial, _) = partial();
        for (offset, value) in [
            (104, 1),
            (108, 999),
            (112, 0),
            (116, 999),
            (120, 4),
            (124, 999),
        ] {
            let mut bad = partial.clone();
            put(&mut bad, offset, value);
            assert!(decode_package(&bad).is_err(), "offset {offset}");
        }
        let mut codec = partial.clone();
        put(&mut codec, 104, 1);
        assert!(matches!(
            decode_package(&codec),
            Err(ContainerError::BadPartialTable { .. })
        ));
        assert_eq!(
            decode_package(&[partial.clone(), b"extra".to_vec()].concat()),
            Err(ContainerError::TrailingPartialBytes)
        );
        for end in 0..partial.len() {
            assert!(decode_package(&partial[..end]).is_err(), "end {end}");
        }
    }

    #[test]
    fn partial_chunk_that_is_a_package_is_refused() {
        // A chunk whose second word is 832/46 would recurse into the partial reader; C++ refuses it.
        let (mut partial, _) = partial();
        put(&mut partial, 144 + 4, BL2_VERSION_WORD);
        assert_eq!(
            decode_package(&partial),
            Err(ContainerError::BadPartialTable {
                reason: "chunk is a package, not a container"
            })
        );
    }

    #[test]
    fn errors_inside_a_partial_chunk_name_the_entry_and_its_file_offset() {
        // A broken end marker in the chunk's LZO stream: the error names table entry 0, stored at file byte 144.
        let (mut broken, _) = partial();
        let n = broken.len();
        broken[n - 3..].fill(0);
        match decode_package(&broken) {
            Err(ContainerError::InPartialChunk {
                index: 0,
                at: 144,
                error,
            }) => {
                assert!(matches!(*error, ContainerError::Lzo { .. }), "{error:?}")
            }
            other => panic!("expected InPartialChunk, got {other:?}"),
        }
        // A chunk too short to hold a container header.
        let (mut short, _) = partial();
        put(&mut short, 124, 4);
        assert_eq!(
            decode_package(&short),
            Err(ContainerError::InPartialChunk {
                index: 0,
                at: 144,
                error: Box::new(ContainerError::Truncated { at: 4, need: 4 })
            })
        );
    }

    #[test]
    fn partial_folder_name_longer_than_the_file_is_refused() {
        let (mut partial, _) = partial();
        put(&mut partial, 12, 0x8000_0000); // -2^31 UTF-16 units
        assert!(matches!(
            decode_package(&partial),
            Err(ContainerError::Truncated { .. })
        ));
        put(&mut partial, 12, 0x7FFF_FFFF);
        assert!(matches!(
            decode_package(&partial),
            Err(ContainerError::Truncated { .. })
        ));
    }

    #[test]
    fn partial_generation_count_is_bounded() {
        let (mut partial, _) = partial();
        put(&mut partial, 80, 0xFFFF_FFFF);
        assert!(matches!(
            decode_package(&partial),
            Err(ContainerError::BadPartialTable { .. })
        ));
    }
}
