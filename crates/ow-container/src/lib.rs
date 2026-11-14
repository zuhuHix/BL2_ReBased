//! LZO1X container of BL2's packages, fully and partially compressed: the port of `src/container.cpp` /
//! `src/container.hpp`.
//!
//! Owner: the signature of [`decode_package`] and [`ContainerError`] is fixed by W0-SCAFFOLD (a frozen contract,
//! architecture section 7); lane W1-LZO writes the body, `lzo.rs` and `chunks.rs`.
//!
//! The C++ code is the reference and uses the vendored lzokay decoder (`third_party/lzokay`, MIT). This crate gets
//! its own decoder, written from the LZO1X format and from the project's own pure-Python decoder in
//! `research/native_count.py`, so no C code is translated. The container's bounds checks move over unchanged in
//! meaning (AGENTS.md rule 5): every size and offset is checked before it is used, and a failed check is an error,
//! never a clamp.
//!
//! Done when: the synthetic container fixtures that `tests/` builds give the same results here as in the C++ build,
//! and the real packages decompress to the same bytes as `ow-package` from `build/Release`.

pub mod chunks;
pub mod lzo;

use ow_core::NotYet;
use std::fmt;

/// Why a package's bytes could not be decoded.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContainerError {
    /// The input ends at `at`, `need` more bytes were needed.
    Truncated { at: usize, need: usize },
    /// The package tag is not the UE3 package tag.
    BadMagic,
    /// Chunk `index` of the container is not valid.
    BadChunk { index: usize, reason: &'static str },
    /// The LZO stream is not valid at byte `at`.
    Lzo { at: usize, reason: &'static str },
    /// The partial compression table of a package summary is not valid.
    BadPartialTable { reason: &'static str },
    /// Bytes follow the last chunk of a partially compressed package.
    TrailingPartialBytes,
    /// Not written yet.
    NotYet(NotYet),
}

impl fmt::Display for ContainerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContainerError::Truncated { at, need } => {
                write!(f, "truncated at byte {at}: {need} more bytes needed")
            }
            ContainerError::BadMagic => f.write_str("not a UE3 package (bad tag)"),
            ContainerError::BadChunk { index, reason } => write!(f, "bad chunk {index}: {reason}"),
            ContainerError::Lzo { at, reason } => {
                write!(f, "bad LZO stream at byte {at}: {reason}")
            }
            ContainerError::BadPartialTable { reason } => {
                write!(f, "bad partial compression table: {reason}")
            }
            ContainerError::TrailingPartialBytes => {
                f.write_str("bytes after the last partially compressed chunk")
            }
            ContainerError::NotYet(n) => n.fmt(f),
        }
    }
}

impl std::error::Error for ContainerError {}

/// Decodes a package file's bytes into the uncompressed package: a fully compressed container, or a package whose
/// summary carries a partial compression table, as `src/container.cpp` does. Lane W1-LZO writes the body; until then it returns
/// `NotYet { NotPorted }`.
pub fn decode_package(input: &[u8]) -> Result<Vec<u8>, ContainerError> {
    let _ = input;
    Err(ContainerError::NotYet(NotYet::not_ported(
        "ow_container::decode_package",
        "lane W1-LZO; the C++ reference is src/container.cpp",
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ow_core::Missing;

    #[test]
    fn decode_package_says_not_ported() {
        match decode_package(&[]) {
            Err(ContainerError::NotYet(n)) => assert_eq!(n.missing, Missing::NotPorted),
            other => panic!("expected NotYet, got {other:?}"),
        }
    }
}
