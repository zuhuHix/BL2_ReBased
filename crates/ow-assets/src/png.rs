//! Owner: lane W2-ASSETS. The PNG writer: the port of `png` and `pngChunk` (`src/assets.cpp`). An 8-bit RGBA image,
//! every row with filter 0, in a zlib stream of stored (uncompressed) deflate blocks of at most 65535 bytes, with its
//! own Adler-32 and chunk CRC-32, as the C++ writes it, so the bytes are the C++ bytes. No third-party crate.

use crate::texture::{AssetError, check};

/// The CRC-32 (IEEE, reflected, polynomial 0xedb88320) table. The C++ computes the same CRC bit by bit; a table gives
/// the same value faster.
const CRC_TABLE: [u32; 256] = {
    let mut table = [0u32; 256];
    let mut n = 0;
    while n < 256 {
        let mut c = n as u32;
        let mut k = 0;
        while k < 8 {
            c = (c >> 1) ^ if c & 1 != 0 { 0xedb8_8320 } else { 0 };
            k += 1;
        }
        table[n] = c;
        n += 1;
    }
    table
};

/// The CRC-32 of `bytes` (initial value all ones, final value inverted).
pub fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &b in bytes {
        crc = CRC_TABLE[((crc ^ u32::from(b)) & 0xff) as usize] ^ (crc >> 8);
    }
    !crc
}

/// The Adler-32 of `bytes`.
pub fn adler32(bytes: &[u8]) -> u32 {
    const MOD: u32 = 65521;
    let (mut a, mut b) = (1u32, 0u32);
    // At most 5552 bytes between reductions keep `b` below 2^32 (the zlib bound); the result equals the C++
    // per-byte reduction.
    for chunk in bytes.chunks(5552) {
        for &v in chunk {
            a += u32::from(v);
            b += a;
        }
        a %= MOD;
        b %= MOD;
    }
    (b << 16) | a
}

/// A chunk: length, type, data, CRC of type and data.
fn chunk(png: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    // Data is at most a few hundred MiB (the importer's limits), below 2^32.
    png.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = png.len();
    png.extend_from_slice(kind);
    png.extend_from_slice(data);
    let crc = crc32(&png[start..]);
    png.extend_from_slice(&crc.to_be_bytes());
}

/// The PNG file of a `width` x `height` image whose `pixels` are R, G, B, A bytes, rows top first.
pub fn encode(width: u32, height: u32, pixels: &[u8]) -> Result<Vec<u8>, AssetError> {
    check(width != 0 && height != 0, AssetError::PngDimensions)?;
    let row = width as usize * 4;
    check(
        (row as u64).checked_mul(u64::from(height)) == Some(pixels.len() as u64),
        AssetError::PngBufferSize,
    )?;
    let mut rows = Vec::with_capacity(height as usize * (row + 1));
    for line in pixels.chunks_exact(row) {
        rows.push(0);
        rows.extend_from_slice(line);
    }

    let mut zlib = Vec::with_capacity(rows.len() + rows.len() / 65535 * 5 + 16);
    zlib.extend_from_slice(&[0x78, 0x01]);
    let mut at = 0;
    while at < rows.len() {
        let size = (rows.len() - at).min(65535);
        zlib.push(u8::from(at + size == rows.len()));
        let size16 = size as u16;
        zlib.extend_from_slice(&size16.to_le_bytes());
        zlib.extend_from_slice(&(!size16).to_le_bytes());
        zlib.extend_from_slice(&rows[at..at + size]);
        at += size;
    }
    zlib.extend_from_slice(&adler32(&rows).to_be_bytes());

    let mut png = vec![137, 80, 78, 71, 13, 10, 26, 10];
    let mut header = Vec::with_capacity(13);
    header.extend_from_slice(&width.to_be_bytes());
    header.extend_from_slice(&height.to_be_bytes());
    header.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(&mut png, b"IHDR", &header);
    chunk(&mut png, b"IDAT", &zlib);
    chunk(&mut png, b"IEND", &[]);
    Ok(png)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksums_match_known_values() {
        assert_eq!(crc32(b"IEND"), 0xae42_6082);
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
        assert_eq!(adler32(b"Wikipedia"), 0x11e6_0398);
        // Long runs of 0xff test the deferred reduction against the per-byte one.
        let long = vec![0xffu8; 200_000];
        let (mut a, mut b) = (1u32, 0u32);
        for &v in &long {
            a = (a + u32::from(v)) % 65521;
            b = (b + a) % 65521;
        }
        assert_eq!(adler32(&long), (b << 16) | a);
    }

    #[test]
    fn a_one_pixel_image_has_the_cxx_layout() {
        let png = encode(1, 1, &[1, 2, 3, 4]).unwrap();
        let expected_idat: Vec<u8> = [
            &[0x78, 0x01, 1, 5, 0, 0xfa, 0xff, 0, 1, 2, 3, 4][..],
            &adler32(&[0, 1, 2, 3, 4]).to_be_bytes(),
        ]
        .concat();
        assert_eq!(&png[..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
        assert_eq!(&png[8..16], &[0, 0, 0, 13, b'I', b'H', b'D', b'R']);
        assert_eq!(&png[16..29], &[0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0]);
        let idat = 33;
        assert_eq!(&png[idat..idat + 8], &[0, 0, 0, 16, b'I', b'D', b'A', b'T']);
        assert_eq!(&png[idat + 8..idat + 24], &expected_idat[..]);
        assert_eq!(
            &png[png.len() - 12..],
            &[0, 0, 0, 0, b'I', b'E', b'N', b'D', 0xae, 0x42, 0x60, 0x82]
        );
    }

    #[test]
    fn large_images_split_into_stored_blocks() {
        // 128 x 128 RGBA rows make 66 176 bytes: one full 65535-byte block, then a final one.
        let png = encode(128, 128, &vec![7u8; 128 * 128 * 4]).unwrap();
        let idat = 33 + 8;
        assert_eq!(&png[idat..idat + 7], &[0x78, 0x01, 0, 0xff, 0xff, 0, 0]);
        let second = idat + 2 + 5 + 65535;
        let rest = (128 * 513 - 65535) as u16;
        assert_eq!(png[second], 1);
        assert_eq!(
            &png[second + 1..second + 5],
            &[rest.to_le_bytes(), (!rest).to_le_bytes()].concat()[..]
        );
    }

    #[test]
    fn bad_buffers_are_refused() {
        assert_eq!(encode(0, 1, &[]), Err(AssetError::PngDimensions));
        assert_eq!(encode(1, 1, &[0; 3]), Err(AssetError::PngBufferSize));
    }
}
