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
    /// The header of a fully compressed container is not valid (block size, decoded size, or compressed size against
    /// the bytes after the block table). Added by W1-LZO (review r1 of PR #283), additive.
    BadHeader { reason: &'static str },
    /// Entry `index` of a partial compression table names a chunk stored at file byte `at` that could not be
    /// decoded; `error` is in the chunk's own coordinates (its offsets count from `at`, its chunk indices are the
    /// nested container's blocks). Added by W1-LZO (review r1 of PR #283), additive.
    InPartialChunk {
        index: usize,
        at: usize,
        error: Box<ContainerError>,
    },
    /// Not written yet.
    NotYet(NotYet),
}

impl fmt::Display for ContainerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ContainerError::Truncated { at, need } => {
                write!(f, "truncated at byte {at}: {need} more bytes needed")
            }
            // The C++ text (`src/container.cpp`, `decode_package`), so both readers print the same line (rule R7).
            ContainerError::BadMagic => f.write_str("invalid package magic"),
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
            ContainerError::BadHeader { reason } => write!(f, "bad container header: {reason}"),
            // The inner error is not repeated here: `source()` returns it, so chained reporters print it once.
            ContainerError::InPartialChunk { index, at, .. } => write!(
                f,
                "in partial-chunk entry {index} at file byte {at} (the cause's offsets count from there)"
            ),
            ContainerError::NotYet(n) => n.fmt(f),
        }
    }
}

impl std::error::Error for ContainerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ContainerError::InPartialChunk { error, .. } => Some(error.as_ref()),
            _ => None,
        }
    }
}

/// Decodes a package file's bytes into the uncompressed package: a fully compressed container, or a package whose
/// summary carries a partial compression table, as `src/container.cpp` does. An 832/46 package without the
/// compressed flag comes back unchanged.
///
/// Status: the accept/refuse behaviour follows the C++ reader on the synthetic fixtures of `tests/package_test.py`
/// (re-expressed as this crate's unit tests); the decoded bytes of the installed packages were compared with the C++
/// reader's locally (decision 2026-11-15, W1-LZO). The error kinds are this crate's own: the C++ code throws
/// messages only.
pub fn decode_package(input: &[u8]) -> Result<Vec<u8>, ContainerError> {
    if chunks::word(input, 0)? != chunks::PACKAGE_TAG {
        return Err(ContainerError::BadMagic);
    }
    if chunks::word(input, 4)? == chunks::BL2_VERSION_WORD {
        chunks::decode_partial(input)
    } else {
        chunks::decode_container(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_is_truncated() {
        assert_eq!(
            decode_package(&[]),
            Err(ContainerError::Truncated { at: 0, need: 4 })
        );
    }

    #[test]
    fn bad_magic_prints_the_cxx_text() {
        assert_eq!(
            decode_package(&[0u8; 8]).unwrap_err().to_string(),
            "invalid package magic"
        );
    }

    #[test]
    fn tag_then_nothing_is_truncated() {
        assert_eq!(
            decode_package(&chunks::PACKAGE_TAG.to_le_bytes()),
            Err(ContainerError::Truncated { at: 4, need: 4 })
        );
    }

    /// Python's stdin loop for the gate: one path per line in, one SHA-256 hex digest per line out.
    const HASHER: &str = "import hashlib, sys\nfor line in sys.stdin:\n    \
        print(hashlib.sha256(open(line.rstrip('\\n'), 'rb').read()).hexdigest(), flush=True)";

    /// Whether the C++ reader's error message (`std::runtime_error` text from `src/container.cpp`) is the same kind
    /// of refusal as this crate's error. The C++ code has messages only, so the kinds are matched on its wording.
    fn same_kind_as_cxx(error: &ContainerError, cxx: &str) -> bool {
        let any = |words: &[&str]| words.iter().any(|w| cxx.contains(w));
        match error {
            ContainerError::BadMagic => any(&["invalid package magic"]),
            ContainerError::Truncated { .. } => {
                // C++ reports a block table cut off by the end of the file as a size mismatch. It uses that one
                // message ("compressed size/table mismatch", container.cpp:71-72) for both this case and
                // BadHeader's compressed-size case, so this gate cannot tell Truncated and BadHeader apart there.
                any(&[
                    "truncated container header",
                    "invalid summary folder length",
                    "compressed size/table mismatch",
                ])
            }
            ContainerError::BadHeader { .. } => any(&[
                "version or container block size",
                "decoded size exceeds",
                "compressed size/table mismatch",
            ]),
            ContainerError::BadChunk { .. } => {
                any(&["invalid container block sizes", "block totals mismatch"])
            }
            ContainerError::Lzo { .. } => any(&["invalid LZO block"]),
            ContainerError::BadPartialTable { .. } => any(&[
                "generation count",
                "partial compression table",
                "partial compression chunk bounds",
                "expected chunk container",
                "partial chunk decoded size",
            ]),
            ContainerError::TrailingPartialBytes => any(&["trailing partial package bytes"]),
            ContainerError::InPartialChunk { error, .. } => same_kind_as_cxx(error, cxx),
            _ => false,
        }
    }

    #[test]
    fn in_partial_chunk_leaves_its_cause_to_source() {
        use std::error::Error;
        let inner = ContainerError::Lzo {
            at: 7,
            reason: "input overrun",
        };
        let outer = ContainerError::InPartialChunk {
            index: 2,
            at: 144,
            error: Box::new(inner.clone()),
        };
        let text = outer.to_string();
        assert_eq!(
            text,
            "in partial-chunk entry 2 at file byte 144 (the cause's offsets count from there)"
        );
        assert!(!text.contains(&inner.to_string()));
        assert_eq!(
            outer.source().map(|s| s.to_string()),
            Some(inner.to_string())
        );
        assert!(inner.source().is_none());
    }

    #[test]
    fn cxx_kind_matching() {
        let header = ContainerError::BadHeader { reason: "x" };
        let block = ContainerError::BadChunk {
            index: 1,
            reason: "x",
        };
        assert!(same_kind_as_cxx(
            &ContainerError::BadMagic,
            "ow-package: invalid package magic"
        ));
        assert!(same_kind_as_cxx(
            &header,
            "unsupported package version or container block size"
        ));
        assert!(!same_kind_as_cxx(&header, "invalid container block sizes"));
        assert!(same_kind_as_cxx(&block, "invalid container block sizes"));
        let nested = ContainerError::InPartialChunk {
            index: 0,
            at: 9,
            error: Box::new(ContainerError::Lzo { at: 1, reason: "x" }),
        };
        assert!(same_kind_as_cxx(
            &nested,
            "invalid LZO block or decoded length mismatch"
        ));
        assert!(!same_kind_as_cxx(
            &ContainerError::BadMagic,
            "invalid LZO block"
        ));
    }

    /// The real-package gate (W1-LZO), run by hand on a machine with the game installed; ignored by default:
    ///
    /// `OW_LZO_GATE_GAME=<install> OW_LZO_GATE_CXX=build/Release/ow-package.exe OW_LZO_GATE_OUT=local/w1-lzo
    /// cargo test -p ow-container --release -- --ignored real_packages_match_cxx`
    ///
    /// For every `.upk` under the install (sorted paths): decode it here, write the bytes to `OUT/decoded.bin`, run
    /// the C++ reader's `--verify-decoded` on it (a byte comparison with the C++ decoder's output), and hash it with
    /// Python's `hashlib`. Where this crate refuses a package, the C++ reader must refuse it too (`--names`), with
    /// the same kind of error ([`same_kind_as_cxx`]). The per-package lines go to `OUT/real_gate.tsv` (ignored
    /// `local/`: they name game files). Game bytes never leave `OUT`, and `decoded.bin` is removed at the end.
    #[test]
    #[ignore = "needs the installed game and the C++ build; see the doc comment"]
    fn real_packages_match_cxx() {
        use std::io::{BufRead, BufReader, Write};
        use std::path::{Path, PathBuf};
        use std::process::{Command, Stdio};

        let var = |name: &str| std::env::var_os(name).unwrap_or_else(|| panic!("set {name}"));
        let game = PathBuf::from(var("OW_LZO_GATE_GAME"));
        let cxx = PathBuf::from(var("OW_LZO_GATE_CXX"));
        let out = PathBuf::from(var("OW_LZO_GATE_OUT"));
        std::fs::create_dir_all(&out).unwrap();

        fn walk(dir: &Path, found: &mut Vec<PathBuf>) {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    walk(&path, found);
                } else if path
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("upk"))
                {
                    found.push(path);
                }
            }
        }
        let mut packages = Vec::new();
        walk(&game, &mut packages);
        packages.sort();
        assert!(!packages.is_empty(), "no .upk under {}", game.display());

        let mut hasher = Command::new("python")
            .args(["-I", "-c", HASHER])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("python on PATH");
        let mut to_hasher = hasher.stdin.take().unwrap();
        let mut from_hasher = BufReader::new(hasher.stdout.take().unwrap());

        let decoded_path = out.join("decoded.bin");
        let mut tsv = std::fs::File::create(out.join("real_gate.tsv")).unwrap();
        let one_line = |bytes: &[u8]| {
            String::from_utf8_lossy(bytes)
                .trim()
                .replace(['\t', '\r', '\n'], " ")
        };
        // An error with its `source()` chain, the way a chained reporter prints it.
        let chain = |e: &ContainerError| {
            let mut text = e.to_string();
            let mut cause = std::error::Error::source(e);
            while let Some(c) = cause {
                text.push_str(": ");
                text.push_str(&c.to_string());
                cause = c.source();
            }
            text
        };
        writeln!(tsv, "package\tresult\tdecoded_bytes\tsha256\tdetail").unwrap();
        let (mut matched, mut both_refused, mut failures) = (0usize, 0usize, Vec::new());
        for package in &packages {
            let rel = package.strip_prefix(&game).unwrap().display().to_string();
            let input = std::fs::read(package).unwrap();
            match decode_package(&input) {
                Ok(bytes) => {
                    std::fs::write(&decoded_path, &bytes).unwrap();
                    let run = Command::new(&cxx)
                        .arg(package)
                        .arg("--verify-decoded")
                        .arg(&decoded_path)
                        .output()
                        .unwrap();
                    writeln!(to_hasher, "{}", decoded_path.display()).unwrap();
                    let mut hash = String::new();
                    from_hasher.read_line(&mut hash).unwrap();
                    let hash = hash.trim();
                    if run.status.success() {
                        matched += 1;
                        writeln!(tsv, "{rel}\tmatch\t{}\t{hash}\t", bytes.len()).unwrap();
                    } else {
                        let err = one_line(&run.stderr);
                        writeln!(tsv, "{rel}\tMISMATCH\t{}\t{hash}\t{err}", bytes.len()).unwrap();
                        failures.push(format!("{rel}: C++ --verify-decoded failed: {err}"));
                    }
                }
                Err(e) => {
                    let text = chain(&e);
                    let run = Command::new(&cxx)
                        .arg(package)
                        .arg("--names")
                        .output()
                        .unwrap();
                    let err = one_line(&run.stderr);
                    if run.status.success() {
                        writeln!(tsv, "{rel}\tRUST_ONLY_REFUSED\t\t\t{text}").unwrap();
                        failures.push(format!(
                            "{rel}: refused here ({text}) but the C++ reader reads it"
                        ));
                    } else if !same_kind_as_cxx(&e, &err) {
                        writeln!(tsv, "{rel}\tKIND_DIFFERS\t\t\trust: {text}; c++: {err}").unwrap();
                        failures.push(format!(
                            "{rel}: refused by both, but as {e:?} here and \"{err}\" in C++"
                        ));
                    } else {
                        both_refused += 1;
                        writeln!(tsv, "{rel}\tboth_refused\t\t\trust: {text}; c++: {err}").unwrap();
                    }
                }
            }
        }
        drop(to_hasher);
        hasher.wait().unwrap();
        let _ = std::fs::remove_file(&decoded_path);
        let summary = format!(
            "{} packages: {matched} byte-identical to C++, {both_refused} refused by both with the same error kind, {} failures",
            packages.len(),
            failures.len()
        );
        writeln!(tsv, "# {summary}").unwrap();
        println!("{summary}");
        assert!(failures.is_empty(), "{failures:#?}");
    }
}
