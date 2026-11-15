//! Owner: lane W1-PKG. The bounds-checked little-endian reader over a package's decoded bytes: the port of the C++
//! `Reader` (`src/package.hpp`, `src/package.cpp`).
//!
//! Every read checks its bytes first, exactly where the C++ `require()` does, against the smaller of the data's end
//! and the reader's `limit`. A failed check is a [`ReadError`], never a panic and never a wider bound (AGENTS.md
//! rule 5). The error texts are the C++ exception texts, so the CLI prints the same stderr line.

use std::fmt;

/// Why a read failed. `Display` gives the C++ reader's exception text.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadError {
    /// A read past the end of the data or the limit (C++ `require()`, `byte()`): "truncated package".
    Truncated,
    /// An FString whose byte count does not fit in `usize`: "FString is too large".
    StringTooLarge,
    /// An FString without its terminating zero (one byte, or two for UTF-16): "unterminated FString".
    UnterminatedString,
    /// A lone or mismatched UTF-16 surrogate: "invalid UTF-16 surrogate".
    InvalidSurrogate,
    /// A table count or offset outside the data (C++ `table()`): "invalid table bounds".
    InvalidTableBounds,
    /// An FName index outside the name table or a negative number: "invalid FName".
    InvalidName,
    /// An object reference outside `-imports..=exports`: "invalid object reference".
    InvalidReference,
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ReadError::Truncated => "truncated package",
            ReadError::StringTooLarge => "FString is too large",
            ReadError::UnterminatedString => "unterminated FString",
            ReadError::InvalidSurrogate => "invalid UTF-16 surrogate",
            ReadError::InvalidTableBounds => "invalid table bounds",
            ReadError::InvalidName => "invalid FName",
            ReadError::InvalidReference => "invalid object reference",
        })
    }
}

impl std::error::Error for ReadError {}

/// A cursor over decoded package bytes. `pos` may be set anywhere; reads check it. `limit` is an absolute byte
/// position past which no read goes (the C++ default is "no limit", `usize::MAX`).
#[derive(Debug, Clone, Copy)]
pub struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
    limit: usize,
}

impl<'a> Reader<'a> {
    /// A reader at position 0 with no limit.
    pub fn new(bytes: &'a [u8]) -> Self {
        Reader {
            bytes,
            pos: 0,
            limit: usize::MAX,
        }
    }

    /// The whole data, ignoring the limit (C++ `bytes()`).
    pub fn bytes(&self) -> &'a [u8] {
        self.bytes
    }

    pub fn pos(&self) -> usize {
        self.pos
    }

    /// Moves the cursor. Not checked here: the next read checks it (the C++ assigns `pos` directly).
    pub fn set_pos(&mut self, pos: usize) {
        self.pos = pos;
    }

    pub fn limit(&self) -> usize {
        self.limit
    }

    pub fn set_limit(&mut self, limit: usize) {
        self.limit = limit;
    }

    /// A copy of this reader with `limit` set.
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }

    /// The end reads may not pass: the smaller of the data's length and the limit.
    fn end(&self) -> usize {
        self.bytes.len().min(self.limit)
    }

    /// The byte at `at`, checked against the end (C++ `byte()`).
    pub fn byte(&self, at: usize) -> Result<u8, ReadError> {
        if at >= self.end() {
            return Err(ReadError::Truncated);
        }
        Ok(self.bytes[at])
    }

    /// Fails unless `count` bytes are readable at the cursor (C++ `require()`).
    pub fn require(&self, count: usize) -> Result<(), ReadError> {
        let end = self.end();
        if self.pos > end || count > end - self.pos {
            return Err(ReadError::Truncated);
        }
        Ok(())
    }

    pub fn skip(&mut self, count: usize) -> Result<(), ReadError> {
        self.require(count)?;
        self.pos += count;
        Ok(())
    }

    /// `count` bytes at the cursor, checked, and advances past them.
    pub fn take(&mut self, count: usize) -> Result<&'a [u8], ReadError> {
        self.require(count)?;
        let slice = &self.bytes[self.pos..self.pos + count];
        self.pos += count;
        Ok(slice)
    }

    pub fn u32(&mut self) -> Result<u32, ReadError> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn i32(&mut self) -> Result<i32, ReadError> {
        Ok(self.u32()? as i32)
    }

    /// An FString: an `i32` length, positive for single-byte characters (each one a code point 0-255) and negative
    /// for UTF-16 (surrogate pairs joined), including the terminating zero, which is checked and dropped. The C++
    /// `Reader::string()`, check for check.
    pub fn string(&mut self) -> Result<String, ReadError> {
        let length = i64::from(self.i32()?);
        let wide = length < 0;
        let step: usize = if wide { 2 } else { 1 };
        let characters = length.unsigned_abs();
        let byte_count = usize::try_from(characters)
            .ok()
            .and_then(|c| c.checked_mul(step))
            .ok_or(ReadError::StringTooLarge)?;
        self.require(byte_count)?;
        if wide && byte_count < 2 {
            return Err(ReadError::UnterminatedString);
        }
        let at = self.pos;
        let raw = &self.bytes[at..at + byte_count];
        if byte_count > 0 && (raw[byte_count - 1] != 0 || (wide && raw[byte_count - 2] != 0)) {
            return Err(ReadError::UnterminatedString);
        }

        let mut result = String::new();
        let mut i = 0;
        while i + step < byte_count {
            let mut codepoint = u32::from(raw[i]);
            if wide {
                codepoint |= u32::from(raw[i + 1]) << 8;
                if (0xd800..=0xdbff).contains(&codepoint) {
                    if i + 4 >= byte_count {
                        return Err(ReadError::InvalidSurrogate);
                    }
                    let low = u32::from(raw[i + 2]) | (u32::from(raw[i + 3]) << 8);
                    if !(0xdc00..=0xdfff).contains(&low) {
                        return Err(ReadError::InvalidSurrogate);
                    }
                    codepoint = 0x10000 + ((codepoint - 0xd800) << 10) + low - 0xdc00;
                    i += 2;
                } else if (0xdc00..=0xdfff).contains(&codepoint) {
                    return Err(ReadError::InvalidSurrogate);
                }
            }
            // Every code point here is below 0x110000 and not a surrogate, so this never fails; the error is the
            // honest one if it ever did.
            result.push(char::from_u32(codepoint).ok_or(ReadError::InvalidSurrogate)?);
            i += step;
        }
        self.skip(byte_count)?;
        Ok(result)
    }

    /// Checks a table of `count` entries of at least `minimum` bytes each at `offset` against the data's length (not
    /// the limit, as the C++ `table()`), then moves the cursor to `offset`.
    pub fn table(&mut self, count: i32, offset: i32, minimum: usize) -> Result<(), ReadError> {
        let size = self.bytes.len();
        if count < 0 || offset < 0 || minimum == 0 {
            return Err(ReadError::InvalidTableBounds);
        }
        let (count, offset) = (count as usize, offset as usize);
        if offset > size || count > (size - offset) / minimum {
            return Err(ReadError::InvalidTableBounds);
        }
        self.pos = offset;
        Ok(())
    }

    /// An FName: the name-table index and the number, checked against a table of `count` names (C++ `name()`).
    pub fn name(&mut self, count: i32) -> Result<(i32, i32), ReadError> {
        let index = self.i32()?;
        let number = self.i32()?;
        if index < 0 || index >= count || number < 0 {
            return Err(ReadError::InvalidName);
        }
        Ok((index, number))
    }

    /// An object reference, checked to lie in `-imports..=exports` (C++ `reference()`; 0 is "none").
    pub fn reference(&mut self, imports: i32, exports: i32) -> Result<i32, ReadError> {
        let value = i64::from(self.i32()?);
        if value < -i64::from(imports) || value > i64::from(exports) {
            return Err(ReadError::InvalidReference);
        }
        Ok(value as i32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn le(values: &[i32]) -> Vec<u8> {
        values.iter().flat_map(|v| v.to_le_bytes()).collect()
    }

    #[test]
    fn integers_are_little_endian_and_checked() {
        let data = [0x01, 0x02, 0x03, 0x04, 0xff, 0xff, 0xff, 0xff, 0x09];
        let mut r = Reader::new(&data);
        assert_eq!(r.u32(), Ok(0x0403_0201));
        assert_eq!(r.i32(), Ok(-1));
        assert_eq!(r.pos(), 8);
        assert_eq!(r.u32(), Err(ReadError::Truncated));
        assert_eq!(r.pos(), 8, "a failed read does not move the cursor");
        assert_eq!(r.byte(8), Ok(9));
        assert_eq!(r.byte(9), Err(ReadError::Truncated));
    }

    #[test]
    fn the_limit_bounds_every_read() {
        let data = [0u8; 16];
        let mut r = Reader::new(&data).with_limit(6);
        assert!(r.u32().is_ok());
        assert_eq!(r.u32(), Err(ReadError::Truncated));
        assert_eq!(r.byte(6), Err(ReadError::Truncated));
        assert_eq!(r.skip(2), Ok(()));
        assert_eq!(r.skip(1), Err(ReadError::Truncated));
        // A cursor past the end fails even a zero-byte requirement, as the C++ `pos > end` check.
        r.set_pos(7);
        assert_eq!(r.require(0), Err(ReadError::Truncated));
        r.set_pos(usize::MAX);
        assert_eq!(r.require(usize::MAX), Err(ReadError::Truncated));
    }

    #[test]
    fn single_byte_strings_map_each_byte_to_a_code_point() {
        let mut data = le(&[5]);
        data.extend_from_slice(b"Abc\xe9\0");
        let mut r = Reader::new(&data);
        assert_eq!(r.string().as_deref(), Ok("Abc\u{e9}"));
        assert_eq!(r.pos(), data.len());
        // Length 0 is the empty string with no terminator.
        let mut r = Reader::new(&[0, 0, 0, 0]);
        assert_eq!(r.string().as_deref(), Ok(""));
    }

    #[test]
    fn utf16_strings_join_surrogate_pairs() {
        // "W" U+1F600 then the terminator: 4 UTF-16 units.
        let mut data = le(&[-4]);
        for unit in [0x57u16, 0xd83d, 0xde00, 0] {
            data.extend_from_slice(&unit.to_le_bytes());
        }
        let mut r = Reader::new(&data);
        assert_eq!(r.string().as_deref(), Ok("W\u{1f600}"));
        assert_eq!(r.pos(), data.len());
    }

    #[test]
    fn bad_strings_are_refused() {
        let unterminated = [le(&[2]), b"ab".to_vec()].concat();
        assert_eq!(
            Reader::new(&unterminated).string(),
            Err(ReadError::UnterminatedString)
        );
        let wide_half_zero = [le(&[-1]), vec![0, 1]].concat();
        assert_eq!(
            Reader::new(&wide_half_zero).string(),
            Err(ReadError::UnterminatedString)
        );
        let too_long = [le(&[9]), b"ab\0".to_vec()].concat();
        assert_eq!(Reader::new(&too_long).string(), Err(ReadError::Truncated));
        let huge = le(&[i32::MIN]);
        assert_eq!(Reader::new(&huge).string(), Err(ReadError::Truncated));
        let mut lone_low = le(&[-2]);
        lone_low.extend_from_slice(&[0x00, 0xdc, 0, 0]);
        assert_eq!(
            Reader::new(&lone_low).string(),
            Err(ReadError::InvalidSurrogate)
        );
        // A high surrogate right before the terminator has no room for its low half.
        let mut cut_pair = le(&[-2]);
        cut_pair.extend_from_slice(&[0x3d, 0xd8, 0, 0]);
        assert_eq!(
            Reader::new(&cut_pair).string(),
            Err(ReadError::InvalidSurrogate)
        );
        let mut bad_low = le(&[-3]);
        bad_low.extend_from_slice(&[0x3d, 0xd8, 0x41, 0x00, 0, 0]);
        assert_eq!(
            Reader::new(&bad_low).string(),
            Err(ReadError::InvalidSurrogate)
        );
    }

    #[test]
    fn tables_are_checked_against_the_data_length() {
        let data = [0u8; 40];
        let mut r = Reader::new(&data).with_limit(4);
        assert_eq!(r.table(2, 16, 12), Ok(()));
        assert_eq!(r.pos(), 16);
        assert_eq!(r.table(3, 16, 12), Err(ReadError::InvalidTableBounds));
        assert_eq!(r.table(-1, 0, 12), Err(ReadError::InvalidTableBounds));
        assert_eq!(r.table(0, -1, 12), Err(ReadError::InvalidTableBounds));
        assert_eq!(r.table(0, 41, 12), Err(ReadError::InvalidTableBounds));
        assert_eq!(r.table(0, 40, 12), Ok(()));
    }

    #[test]
    fn names_and_references_are_range_checked() {
        let data = le(&[2, 0, 3, 1, -1, 0, 0, -1]);
        let mut r = Reader::new(&data);
        assert_eq!(r.name(3), Ok((2, 0)));
        assert_eq!(r.name(3), Err(ReadError::InvalidName));
        assert_eq!(r.name(3), Err(ReadError::InvalidName));
        assert_eq!(r.name(3), Err(ReadError::InvalidName));
        let data = le(&[-2, -3, 4, 5, 0]);
        let mut r = Reader::new(&data);
        assert_eq!(r.reference(2, 4), Ok(-2));
        assert_eq!(r.reference(2, 4), Err(ReadError::InvalidReference));
        assert_eq!(r.reference(2, 4), Ok(4));
        assert_eq!(r.reference(2, 4), Err(ReadError::InvalidReference));
        assert_eq!(r.reference(2, 4), Ok(0));
    }
}
