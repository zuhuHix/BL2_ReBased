//! Owner: lane W1-LZO.
//! LZO1X stream decoder for one block of a package container.
//!
//! Written from the LZO1X instruction format and from the project's own pure-Python decoder
//! (`research/native_count.py`, `lzo1x_decompress`), which this file follows instruction by instruction. No code of
//! any LZO implementation is copied or translated. The Python decoder's instruction semantics were checked against
//! lzokay (MIT, `third_party/lzokay`) when it was written (its docstring says so); here lzokay is the C++ build's
//! decoder and the differential reference on the real packages. The end-marker rule follows its accept/refuse
//! behaviour as found in review r1 of PR #283.
//!
//! A stream is a sequence of instructions. Each instruction either copies literal bytes from the input or copies a
//! match from the output already written (a distance back and a length), and most end with 0 to 3 trailing literals.
//! How an opcode below 16 is read depends on how many literals the previous instruction ended with (`state`).
//!
//! Every read and every write is checked before it happens. A stream is accepted only if it ends with the
//! end-of-stream marker, the marker is the last input byte, and exactly the expected number of bytes was written.

use std::fmt;

/// Why an LZO1X stream was refused. `at` is the input offset (within this stream) where the problem was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LzoError {
    pub at: usize,
    pub reason: &'static str,
}

impl fmt::Display for LzoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "bad LZO stream at byte {}: {}", self.at, self.reason)
    }
}

impl std::error::Error for LzoError {}

/// Decodes one LZO1X stream that must produce exactly `out.len()` bytes into `out`.
pub fn decompress_into(src: &[u8], out: &mut [u8]) -> Result<(), LzoError> {
    let mut d = Decoder {
        src,
        ip: 0,
        out,
        op: 0,
    };
    d.run()
}

/// Decodes one LZO1X stream that must produce exactly `len` bytes. The caller bounds `len` before calling (the
/// container's header and block checks do).
pub fn decompress(src: &[u8], len: usize) -> Result<Vec<u8>, LzoError> {
    let mut out = vec![0u8; len];
    decompress_into(src, &mut out)?;
    Ok(out)
}

struct Decoder<'a, 'b> {
    src: &'a [u8],
    ip: usize,
    out: &'b mut [u8],
    op: usize,
}

impl Decoder<'_, '_> {
    fn err(&self, reason: &'static str) -> LzoError {
        LzoError {
            at: self.ip,
            reason,
        }
    }

    /// Reads one input byte.
    fn take(&mut self) -> Result<usize, LzoError> {
        match self.src.get(self.ip) {
            Some(&b) => {
                self.ip += 1;
                Ok(usize::from(b))
            }
            None => Err(self.err("input overrun")),
        }
    }

    /// Copies `count` literal bytes from the input to the output.
    fn literals(&mut self, count: usize) -> Result<(), LzoError> {
        if count > self.src.len() - self.ip {
            return Err(self.err("input overrun in literal run"));
        }
        if count > self.out.len() - self.op {
            return Err(self.err("output overrun in literal run"));
        }
        self.out[self.op..self.op + count].copy_from_slice(&self.src[self.ip..self.ip + count]);
        self.ip += count;
        self.op += count;
        Ok(())
    }

    /// Copies `length` bytes starting `distance` bytes back in the output. When the source overlaps the destination
    /// (distance below length), the copy runs byte by byte, so the last `distance` bytes repeat.
    fn copy_match(&mut self, distance: usize, length: usize) -> Result<(), LzoError> {
        if distance == 0 || distance > self.op {
            return Err(self.err("match distance outside decoded output"));
        }
        if length > self.out.len() - self.op {
            return Err(self.err("output overrun in match"));
        }
        let start = self.op - distance;
        if distance >= length {
            self.out.copy_within(start..start + length, self.op);
            self.op += length;
        } else {
            for _ in 0..length {
                self.out[self.op] = self.out[self.op - distance];
                self.op += 1;
            }
        }
        Ok(())
    }

    /// Run-length extension of a count whose opcode field was zero: each 0x00 byte adds 255, and the first non-zero
    /// byte ends the run and adds `field_max` plus itself.
    fn extended(&mut self, base: usize, field_max: usize) -> Result<usize, LzoError> {
        let mut length = base;
        loop {
            let b = self.take()?;
            if b != 0 {
                return Ok(length + field_max + b);
            }
            // At most 255 per input byte, so this cannot overflow a 64-bit usize for any input that fits in memory.
            length += 255;
        }
    }

    /// Reads the 16-bit little-endian distance word of a long match.
    fn word(&mut self) -> Result<usize, LzoError> {
        let lo = self.take()?;
        let hi = self.take()?;
        Ok(lo | (hi << 8))
    }

    fn run(&mut self) -> Result<(), LzoError> {
        // First byte 18..=255: (byte - 17) literals come before the first instruction. Any other first byte is an
        // ordinary instruction read with state 0.
        let first = self.take()?;
        let mut state;
        if first > 17 {
            let count = first - 17;
            self.literals(count)?;
            state = count.min(4);
        } else {
            self.ip -= 1;
            state = 0;
        }

        loop {
            let t = self.take()?;
            let trailing;
            if t < 16 {
                if state == 0 {
                    // Literal run of t + 3 bytes; t == 0 selects the extended count.
                    let count = if t == 0 { self.extended(3, 15)? } else { t + 3 };
                    self.literals(count)?;
                    state = 4;
                    continue;
                }
                let h = self.take()?;
                if state == 4 {
                    // After a literal run of 4 or more: a 3-byte match, distance 2049..=3072.
                    self.copy_match((h << 2) + (t >> 2) + 2049, 3)?;
                } else {
                    // After 1..=3 trailing literals: a 2-byte match, distance 1..=1024.
                    self.copy_match((h << 2) + (t >> 2) + 1, 2)?;
                }
                trailing = t & 3;
            } else if t >= 64 {
                // Length 3..=8 in the top 3 bits, distance 1..=2048 from 3 opcode bits and one byte.
                let h = self.take()?;
                self.copy_match((h << 3) + ((t >> 2) & 7) + 1, (t >> 5) + 1)?;
                trailing = t & 3;
            } else if t >= 32 {
                // Length in the low 5 bits (extended when zero), then a 16-bit distance word, distance 1..=16384.
                let length = if t & 31 == 0 {
                    self.extended(2, 31)?
                } else {
                    (t & 31) + 2
                };
                let word = self.word()?;
                self.copy_match((word >> 2) + 1, length)?;
                trailing = word & 3;
            } else {
                // 16..=31: long distance. Bit 3 adds 16384, the low 3 bits are the length (extended when zero), then
                // a 16-bit distance word. A distance part of zero is the end-of-stream marker.
                let high = (t & 8) << 11;
                let length = if t & 7 == 0 {
                    self.extended(2, 7)?
                } else {
                    (t & 7) + 2
                };
                let word = self.word()?;
                let distance = high + (word >> 2);
                if distance == 0 {
                    // Only the 3-byte form `0x11 0x00 0x00` ends a stream: a zero distance with any other length
                    // (opcodes 0x10 extended, 0x12..=0x17) is refused, as the C++ reference's lzokay refuses it.
                    if length != 3 {
                        return Err(self.err("end-of-stream marker with a length other than 3"));
                    }
                    break;
                }
                self.copy_match(distance + 16384, length)?;
                trailing = word & 3;
            }
            self.literals(trailing)?;
            state = trailing;
        }

        if self.ip != self.src.len() {
            return Err(self.err("input bytes after the end-of-stream marker"));
        }
        if self.op != self.out.len() {
            return Err(self.err("stream ended before the expected decoded size"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    //! Hand-built synthetic streams (invented bytes, AGENTS.md rule 9), one per instruction form, plus the refusals.
    use super::*;

    const EOS: [u8; 3] = [0x11, 0x00, 0x00];

    fn stream(parts: &[&[u8]]) -> Vec<u8> {
        parts.concat()
    }

    /// Literal-only stream in the form `tests/package_test.py`'s `container()` writes.
    fn literal_stream(data: &[u8]) -> Vec<u8> {
        assert!(data.len() >= 4);
        let mut s = if data.len() <= 238 {
            vec![17 + data.len() as u8]
        } else {
            let rest = data.len() - 19;
            let mut p = vec![0u8; 1 + rest / 255];
            p.push((rest % 255) as u8 + 1);
            p
        };
        s.extend_from_slice(data);
        s.extend_from_slice(&EOS);
        s
    }

    fn invented(len: usize) -> Vec<u8> {
        (0..len).map(|i| (i * 7 + i / 13) as u8).collect()
    }

    #[test]
    fn literal_only_streams_short_and_extended() {
        for len in [4, 5, 200, 238, 239, 240, 255 + 19, 255 + 20, 5000] {
            let data = invented(len);
            assert_eq!(
                decompress(&literal_stream(&data), len).unwrap(),
                data,
                "len {len}"
            );
        }
    }

    #[test]
    fn short_match_after_first_literals() {
        // "abcd", then t = 0x6C: length (0x6C >> 5) + 1 = 4, distance ((0x6C >> 2) & 7) + 1 = 4.
        let s = stream(&[&[17 + 4], b"abcd", &[0x6C, 0x00], &EOS]);
        assert_eq!(decompress(&s, 8).unwrap(), b"abcdabcd");
    }

    #[test]
    fn overlapping_match_repeats_the_pattern() {
        // t = 0xE0: length 8, distance 1.
        let s = stream(&[&[17 + 4], b"abcd", &[0xE0, 0x00], &EOS]);
        assert_eq!(decompress(&s, 12).unwrap(), b"abcddddddddd");
        // distance 2, length 6: "ab" repeats.
        let s = stream(&[&[17 + 4], b"wxab", &[0xA4, 0x00], &EOS]);
        assert_eq!(decompress(&s, 10).unwrap(), b"wxabababab");
    }

    #[test]
    fn trailing_literals_then_two_byte_match() {
        // t = 0x6D: length 4, distance 4, then 1 trailing literal 'z' (state 1); t = 0x04 with state 1 is a 2-byte
        // match at distance (0 << 2) + (4 >> 2) + 1 = 2.
        let s = stream(&[&[17 + 4], b"abcd", &[0x6D, 0x00], b"z", &[0x04, 0x00], &EOS]);
        assert_eq!(decompress(&s, 11).unwrap(), b"abcdabcdzdz");
    }

    #[test]
    fn three_byte_match_after_long_literal_run() {
        // 3000 literals (state 4), then t = 0x04, h = 0x01: distance (1 << 2) + 1 + 2049 = 2054, length 3.
        let data = invented(3000);
        let mut s = vec![0x00];
        let rest = 3000 - 18;
        s.extend(std::iter::repeat_n(0u8, rest / 255));
        s.push((rest % 255) as u8);
        s.extend_from_slice(&data);
        s.extend_from_slice(&[0x04, 0x01]);
        s.extend_from_slice(&EOS);
        let mut want = data.clone();
        want.extend_from_slice(&data[3000 - 2054..3000 - 2054 + 3]);
        assert_eq!(decompress(&s, 3003).unwrap(), want);
    }

    #[test]
    fn medium_match_with_extended_length() {
        // t = 0x20 (length field zero): extended length 2 + 31 + 5 = 38; distance word 99 << 2 → distance 100.
        let data = invented(100);
        let mut s = vec![0x00, 100 - 18];
        s.extend_from_slice(&data);
        s.extend_from_slice(&[0x20, 0x05]);
        s.extend_from_slice(&(99u16 << 2).to_le_bytes());
        s.extend_from_slice(&EOS);
        let mut want = data.clone();
        want.extend_from_slice(&data[..38]);
        assert_eq!(decompress(&s, 138).unwrap(), want);
    }

    #[test]
    fn long_distance_match_both_ranges() {
        let data = invented(40000);
        let lits = |s: &mut Vec<u8>| {
            let rest = data.len() - 18;
            s.push(0x00);
            s.extend(std::iter::repeat_n(0u8, rest / 255));
            s.push((rest % 255) as u8);
            s.extend_from_slice(&data);
        };
        // Bit 3 clear: distance 16384 + 616 = 17000, length (0x13 & 7) + 2 = 5.
        let mut s = Vec::new();
        lits(&mut s);
        s.push(0x13);
        s.extend_from_slice(&(616u16 << 2).to_le_bytes());
        s.extend_from_slice(&EOS);
        let mut want = data.clone();
        want.extend_from_slice(&data[40000 - 17000..40000 - 17000 + 5]);
        assert_eq!(decompress(&s, 40005).unwrap(), want);
        // Bit 3 set: distance 16384 + 16384 + 100 = 32868, length 3, two trailing literals.
        let mut s = Vec::new();
        lits(&mut s);
        s.push(0x19);
        s.extend_from_slice(&((100u16 << 2) | 2).to_le_bytes());
        s.extend_from_slice(b"QR");
        s.extend_from_slice(&EOS);
        let mut want = data.clone();
        want.extend_from_slice(&data[40000 - 32868..40000 - 32868 + 3]);
        want.extend_from_slice(b"QR");
        assert_eq!(decompress(&s, 40005).unwrap(), want);
    }

    #[test]
    fn refusals() {
        let good = stream(&[&[17 + 4], b"abcd", &EOS]);
        assert_eq!(decompress(&good, 4).unwrap(), b"abcd");
        // Empty input, and every truncation of a good stream.
        for end in 0..good.len() {
            assert!(decompress(&good[..end], 4).is_err(), "end {end}");
        }
        // Trailing byte after the marker.
        let mut extra = good.clone();
        extra.push(0);
        assert_eq!(
            decompress(&extra, 4).unwrap_err().reason,
            "input bytes after the end-of-stream marker"
        );
        // Fewer and more bytes than expected.
        assert!(decompress(&good, 5).is_err());
        assert!(decompress(&good, 3).is_err());
        // A backward reference before any output (package_test.py's b'\x11\x04\x00').
        assert_eq!(
            decompress(&[0x11, 0x04, 0x00], 4).unwrap_err().reason,
            "match distance outside decoded output"
        );
        // A match reaching further back than the output written.
        let far = stream(&[&[17 + 4], b"abcd", &[0x74, 0x00], &EOS]);
        assert!(decompress(&far, 8).is_err());
        // A match running past the expected size.
        let long = stream(&[&[17 + 4], b"abcd", &[0xE0, 0x00], &EOS]);
        assert_eq!(
            decompress(&long, 8).unwrap_err().reason,
            "output overrun in match"
        );
        // An oversized first literal run (package_test.py's bytes([255]) + valid + EOS).
        let over = stream(&[&[255], &invented(125), &EOS]);
        assert!(decompress(&over, 125).is_err());
        // Extended count that never ends.
        assert_eq!(
            decompress(&[0x00, 0x00, 0x00], 4).unwrap_err().reason,
            "input overrun"
        );
    }

    #[test]
    fn only_0x11_ends_a_stream() {
        // A zero distance with a length other than 3 is not an end marker (review r1 of PR #283): 0x12..=0x17 and
        // the extended 0x10 form are refused.
        for marker in [
            &[0x12, 0x00, 0x00][..],
            &[0x17, 0x00, 0x00],
            &[0x10, 0x01, 0x00, 0x00],
        ] {
            let s = stream(&[&[17 + 4], b"abcd", marker]);
            assert_eq!(
                decompress(&s, 4).unwrap_err().reason,
                "end-of-stream marker with a length other than 3",
                "{marker:02x?}"
            );
        }
        let good = stream(&[&[17 + 4], b"abcd", &EOS]);
        assert_eq!(decompress(&good, 4).unwrap(), b"abcd");
        // The accepted marker with the two low (trailing-literal) bits of its word set still ends the stream.
        let low_bits = stream(&[&[17 + 4], b"abcd", &[0x11, 0x03, 0x00]]);
        assert_eq!(decompress(&low_bits, 4).unwrap(), b"abcd");
    }
}
